use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value};

const SOURCE_PATH: &str = "design/tokens/polyorama.tokens.json";
const GENERATED_PATH: &str = "crates/polyorama-ui-egui/src/generated_tokens.rs";
const THEMES: [&str; 4] = ["light", "dark", "light-high-contrast", "dark-high-contrast"];
const DENSITIES: [&str; 2] = ["compact", "comfortable"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenType {
    Color,
    Dimension,
    Number,
    Duration,
    FontSize,
    FontWeight,
}

impl TokenType {
    fn parse(name: &str, path: &str) -> Result<Self> {
        match name {
            "color" => Ok(Self::Color),
            "dimension" => Ok(Self::Dimension),
            "number" => Ok(Self::Number),
            "duration" => Ok(Self::Duration),
            "fontSize" => Ok(Self::FontSize),
            "fontWeight" => Ok(Self::FontWeight),
            _ => bail!(
                "token {path} uses unsupported $type {name:?}; supported types are color, dimension, number, duration, fontSize and fontWeight"
            ),
        }
    }
}

#[derive(Clone, Debug)]
struct TokenDefinition {
    token_type: TokenType,
    value: Value,
}

#[derive(Clone, Debug)]
struct OverrideDefinition {
    declared_type: Option<TokenType>,
    value: Value,
}

#[derive(Clone, Debug)]
struct TokenDocument {
    base: BTreeMap<String, TokenDefinition>,
    themes: BTreeMap<String, BTreeMap<String, OverrideDefinition>>,
    densities: BTreeMap<String, BTreeMap<String, OverrideDefinition>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ResolvedValue {
    Color([u8; 4]),
    Number(f64),
}

pub fn generate(root: &Path) -> Result<()> {
    let generated = generate_from_file(root)?;
    let output = root.join(GENERATED_PATH);
    fs::write(&output, generated).with_context(|| format!("write {}", output.display()))?;
    println!("generated {}", output.display());
    Ok(())
}

pub fn check(root: &Path) -> Result<()> {
    let generated = generate_from_file(root)?;
    let output = root.join(GENERATED_PATH);
    let checked_in = fs::read_to_string(&output)
        .with_context(|| format!("read checked-in generated tokens at {}", output.display()))?;
    if checked_in != generated {
        bail!(
            "generated tokens are stale: run `cargo xtask tokens generate` and commit {}",
            output.display()
        );
    }
    println!(
        "design tokens are valid and {} is current",
        output.display()
    );
    Ok(())
}

fn generate_from_file(root: &Path) -> Result<String> {
    let source_path = root.join(SOURCE_PATH);
    let source = fs::read_to_string(&source_path)
        .with_context(|| format!("read token source {}", source_path.display()))?;
    let document = parse_source(&source)?;
    generate_rust(&document)
}

fn parse_source(source: &str) -> Result<TokenDocument> {
    let root: Value = serde_json::from_str(source).map_err(|error| {
        anyhow!(
            "token source JSON is invalid (numbers must be finite and within the f64 range): {error}"
        )
    })?;
    let object = root
        .as_object()
        .ok_or_else(|| anyhow!("token source root must be a JSON object"))?;
    reject_unknown_extensions(object, "root", &["$description", "$themes", "$densities"])?;

    let mut base = BTreeMap::new();
    for (name, value) in object {
        if !name.starts_with('$') {
            collect_base(value, name, &mut base)?;
        }
    }
    if base.is_empty() {
        bail!("token source contains no token definitions");
    }
    let themes = collect_variants(object, "$themes", &base)?;
    let densities = collect_variants(object, "$densities", &base)?;
    require_variant_names("theme", themes.keys(), &THEMES)?;
    require_variant_names("density", densities.keys(), &DENSITIES)?;
    validate_override_domains(&themes, "theme", &["colour."])?;
    validate_override_domains(&densities, "density", &["spacing.", "geometry."])?;

    let document = TokenDocument {
        base,
        themes,
        densities,
    };
    // Resolve every supported combination during validation. This catches
    // missing aliases and cycles introduced only by a variant override.
    for theme in THEMES {
        for density in DENSITIES {
            resolve_variant(&document, theme, density)?;
        }
    }
    Ok(document)
}

fn collect_base(
    value: &Value,
    path: &str,
    output: &mut BTreeMap<String, TokenDefinition>,
) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("group or token {path} must be a JSON object"))?;
    if object.contains_key("$value") || object.contains_key("$type") {
        reject_unknown_extensions(object, path, &["$description", "$type", "$value"])?;
        if object.keys().any(|key| !key.starts_with('$')) {
            bail!("token {path} cannot contain child tokens");
        }
        let type_name = object
            .get("$type")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("token {path} must define a string $type"))?;
        let token_type = TokenType::parse(type_name, path)?;
        let token_value = object
            .get("$value")
            .ok_or_else(|| anyhow!("token {path} must define $value"))?
            .clone();
        validate_direct_shape(path, token_type, &token_value)?;
        if output
            .insert(
                path.to_owned(),
                TokenDefinition {
                    token_type,
                    value: token_value,
                },
            )
            .is_some()
        {
            bail!("duplicate token path {path}");
        }
        return Ok(());
    }

    reject_unknown_extensions(object, path, &["$description"])?;
    let mut child_count = 0;
    for (name, child) in object {
        if !name.starts_with('$') {
            child_count += 1;
            collect_base(child, &format!("{path}.{name}"), output)?;
        }
    }
    if child_count == 0 {
        bail!("token group {path} is empty");
    }
    Ok(())
}

fn collect_variants(
    root: &Map<String, Value>,
    extension: &str,
    base: &BTreeMap<String, TokenDefinition>,
) -> Result<BTreeMap<String, BTreeMap<String, OverrideDefinition>>> {
    let variants = root
        .get(extension)
        .ok_or_else(|| anyhow!("token source must define {extension}"))?
        .as_object()
        .ok_or_else(|| anyhow!("{extension} must be an object"))?;
    let mut output = BTreeMap::new();
    for (variant, value) in variants {
        let mut overrides = BTreeMap::new();
        collect_overrides(value, variant, "", base, &mut overrides)?;
        output.insert(variant.clone(), overrides);
    }
    Ok(output)
}

fn collect_overrides(
    value: &Value,
    variant: &str,
    path: &str,
    base: &BTreeMap<String, TokenDefinition>,
    output: &mut BTreeMap<String, OverrideDefinition>,
) -> Result<()> {
    let location = if path.is_empty() {
        format!("variant {variant}")
    } else {
        format!("variant {variant} token {path}")
    };
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("{location} must be a JSON object"))?;
    if object.contains_key("$value") || object.contains_key("$type") {
        if path.is_empty() {
            bail!("variant {variant} must contain token paths");
        }
        reject_unknown_extensions(object, &location, &["$description", "$type", "$value"])?;
        if object.keys().any(|key| !key.starts_with('$')) {
            bail!("{location} cannot contain child tokens");
        }
        let base_definition = base
            .get(path)
            .ok_or_else(|| anyhow!("variant {variant} overrides unknown token {path}"))?;
        let declared_type = object
            .get("$type")
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| anyhow!("{location} $type must be a string"))
                    .and_then(|name| TokenType::parse(name, path))
            })
            .transpose()?;
        if declared_type.is_some_and(|declared| declared != base_definition.token_type) {
            bail!("variant {variant} changes the type of token {path}");
        }
        let override_value = object
            .get("$value")
            .ok_or_else(|| anyhow!("{location} must define $value"))?
            .clone();
        validate_direct_shape(path, base_definition.token_type, &override_value)?;
        output.insert(
            path.to_owned(),
            OverrideDefinition {
                declared_type,
                value: override_value,
            },
        );
        return Ok(());
    }
    reject_unknown_extensions(object, &location, &["$description"])?;
    for (name, child) in object {
        if !name.starts_with('$') {
            let child_path = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            collect_overrides(child, variant, &child_path, base, output)?;
        }
    }
    Ok(())
}

fn reject_unknown_extensions(
    object: &Map<String, Value>,
    path: &str,
    allowed: &[&str],
) -> Result<()> {
    for name in object.keys().filter(|name| name.starts_with('$')) {
        if !allowed.contains(&name.as_str()) {
            bail!("{path} uses unsupported construct {name}");
        }
    }
    Ok(())
}

fn require_variant_names<'a>(
    kind: &str,
    actual: impl Iterator<Item = &'a String>,
    required: &[&str],
) -> Result<()> {
    let actual = actual.map(String::as_str).collect::<BTreeSet<_>>();
    let required = required.iter().copied().collect::<BTreeSet<_>>();
    if actual != required {
        bail!(
            "token source {kind} variants must be exactly {}; found {}",
            required.iter().copied().collect::<Vec<_>>().join(", "),
            actual.iter().copied().collect::<Vec<_>>().join(", ")
        );
    }
    Ok(())
}

fn validate_override_domains(
    variants: &BTreeMap<String, BTreeMap<String, OverrideDefinition>>,
    kind: &str,
    allowed_prefixes: &[&str],
) -> Result<()> {
    for (variant, overrides) in variants {
        for path in overrides.keys() {
            if !allowed_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
            {
                bail!(
                    "{kind} {variant} cannot override token {path}; {kind} overrides are restricted to {}",
                    allowed_prefixes.join(" or ")
                );
            }
        }
    }
    Ok(())
}

fn validate_direct_shape(path: &str, token_type: TokenType, value: &Value) -> Result<()> {
    if alias_target(value)?.is_some() {
        return Ok(());
    }
    match token_type {
        TokenType::Color if value.as_str().is_some() => Ok(()),
        TokenType::Dimension
        | TokenType::Number
        | TokenType::Duration
        | TokenType::FontSize
        | TokenType::FontWeight
            if value.is_number() =>
        {
            let number = value
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or_else(|| anyhow!("token {path} must contain a finite number"))?;
            if !(number as f32).is_finite() {
                bail!("token {path} must remain finite when represented as f32");
            }
            if matches!(
                token_type,
                TokenType::Dimension | TokenType::Duration | TokenType::FontSize
            ) && number < 0.0
            {
                bail!("token {path} cannot be negative");
            }
            if token_type == TokenType::Duration
                && (number.fract() != 0.0 || number > f64::from(u32::MAX))
            {
                bail!("duration token {path} must be a whole number of milliseconds within u32");
            }
            if token_type == TokenType::FontWeight
                && (number.fract() != 0.0 || !(1.0..=1000.0).contains(&number))
            {
                bail!("fontWeight token {path} must be a whole number from 1 to 1000");
            }
            Ok(())
        }
        TokenType::Color => bail!("color token {path} must be #RRGGBB, #RRGGBBAA or an alias"),
        _ => bail!("token {path} must contain a finite JSON number or an alias"),
    }
}

fn alias_target(value: &Value) -> Result<Option<&str>> {
    let Some(text) = value.as_str() else {
        return Ok(None);
    };
    if !text.starts_with('{') && !text.ends_with('}') {
        return Ok(None);
    }
    let target = text
        .strip_prefix('{')
        .and_then(|text| text.strip_suffix('}'))
        .filter(|target| !target.is_empty() && !target.contains(['{', '}']))
        .ok_or_else(|| anyhow!("malformed token alias {text:?}; expected {{path.to.token}}"))?;
    Ok(Some(target))
}

fn resolve_variant(
    document: &TokenDocument,
    theme: &str,
    density: &str,
) -> Result<BTreeMap<String, ResolvedValue>> {
    let mut definitions = document.base.clone();
    apply_variant(&mut definitions, document.themes.get(theme), "theme", theme)?;
    apply_variant(
        &mut definitions,
        document.densities.get(density),
        "density",
        density,
    )?;
    let mut output = BTreeMap::new();
    let mut visiting = Vec::new();
    for path in definitions.keys() {
        resolve_token(path, &definitions, &mut output, &mut visiting)?;
    }
    Ok(output)
}

fn apply_variant(
    definitions: &mut BTreeMap<String, TokenDefinition>,
    overrides: Option<&BTreeMap<String, OverrideDefinition>>,
    kind: &str,
    name: &str,
) -> Result<()> {
    let overrides = overrides.ok_or_else(|| anyhow!("unknown {kind} variant {name}"))?;
    for (path, override_definition) in overrides {
        let definition = definitions
            .get_mut(path)
            .ok_or_else(|| anyhow!("{kind} {name} overrides missing token {path}"))?;
        if override_definition
            .declared_type
            .is_some_and(|declared| declared != definition.token_type)
        {
            bail!("{kind} {name} changes the type of token {path}");
        }
        definition.value = override_definition.value.clone();
    }
    Ok(())
}

fn resolve_token(
    path: &str,
    definitions: &BTreeMap<String, TokenDefinition>,
    output: &mut BTreeMap<String, ResolvedValue>,
    visiting: &mut Vec<String>,
) -> Result<ResolvedValue> {
    if let Some(value) = output.get(path) {
        return Ok(*value);
    }
    if let Some(cycle_start) = visiting.iter().position(|candidate| candidate == path) {
        let mut cycle = visiting[cycle_start..].to_vec();
        cycle.push(path.to_owned());
        bail!("token alias cycle: {}", cycle.join(" -> "));
    }
    let definition = definitions
        .get(path)
        .ok_or_else(|| anyhow!("token alias references missing token {path}"))?;
    visiting.push(path.to_owned());
    let value = if let Some(target) = alias_target(&definition.value)? {
        let target_definition = definitions
            .get(target)
            .ok_or_else(|| anyhow!("token {path} aliases missing token {target}"))?;
        if definition.token_type != target_definition.token_type {
            bail!(
                "token {path} ({:?}) aliases {target} ({:?}) with a different type",
                definition.token_type,
                target_definition.token_type
            );
        }
        resolve_token(target, definitions, output, visiting)?
    } else {
        match definition.token_type {
            TokenType::Color => ResolvedValue::Color(parse_colour(
                definition
                    .value
                    .as_str()
                    .ok_or_else(|| anyhow!("color token {path} must contain a string"))?,
                path,
            )?),
            TokenType::Dimension
            | TokenType::Number
            | TokenType::Duration
            | TokenType::FontSize
            | TokenType::FontWeight => {
                let number = definition
                    .value
                    .as_f64()
                    .filter(|number| number.is_finite())
                    .ok_or_else(|| anyhow!("token {path} must contain a finite number"))?;
                ResolvedValue::Number(number)
            }
        }
    };
    visiting.pop();
    output.insert(path.to_owned(), value);
    Ok(value)
}

fn parse_colour(value: &str, path: &str) -> Result<[u8; 4]> {
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| anyhow!("color token {path} must start with #"))?;
    if hex.len() != 6 && hex.len() != 8 {
        bail!("color token {path} must use #RRGGBB or #RRGGBBAA");
    }
    let mut channels = [0, 0, 0, 255];
    for (index, channel) in channels.iter_mut().enumerate().take(hex.len() / 2) {
        *channel = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .with_context(|| format!("color token {path} contains invalid hexadecimal digits"))?;
    }
    Ok(channels)
}

fn generate_rust(document: &TokenDocument) -> Result<String> {
    let mut theme_values = Vec::new();
    for theme in THEMES {
        theme_values.push((theme, resolve_variant(document, theme, "comfortable")?));
    }
    let mut density_values = Vec::new();
    for density in DENSITIES {
        density_values.push((density, resolve_variant(document, "dark", density)?));
    }
    let common = resolve_variant(document, "dark", "comfortable")?;

    let theme_constants = theme_values
        .iter()
        .map(|(name, values)| generate_colour_constant(name, values))
        .collect::<Result<Vec<_>>>()?
        .join("\n\n");
    let density_constants = density_values
        .iter()
        .map(|(name, values)| generate_density_constant(name, values))
        .collect::<Result<Vec<_>>>()?
        .join("\n\n");

    Ok(format!(
        r#"// @generated by `cargo xtask tokens generate` from design/tokens/polyorama.tokens.json.
// Do not edit by hand. This file is compiled directly; runtime JSON parsing is forbidden.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgba8 {{
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}}

impl From<Rgba8> for egui::Color32 {{
    fn from(value: Rgba8) -> Self {{
        Self::from_rgba_unmultiplied(value.red, value.green, value.blue, value.alpha)
    }}
}}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Points(pub f32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ratio(pub f32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FontWeight(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Milliseconds(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeVariant {{
    Light,
    Dark,
    LightHighContrast,
    DarkHighContrast,
}}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DensityVariant {{
    Compact,
    Comfortable,
}}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColourTokens {{
    pub surface_canvas: Rgba8,
    pub surface_panel: Rgba8,
    pub surface_raised: Rgba8,
    pub border_subtle: Rgba8,
    pub text_primary: Rgba8,
    pub text_muted: Rgba8,
    pub accent_primary: Rgba8,
    pub accent_on_accent: Rgba8,
    pub selection_background: Rgba8,
    pub focus_ring: Rgba8,
    pub status_success: Rgba8,
    pub status_warning: Rgba8,
    pub status_error: Rgba8,
}}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpacingTokens {{
    pub unit: Points,
    pub inline: Points,
    pub block: Points,
    pub section: Points,
}}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryTokens {{
    pub application_bar_height: Points,
    pub control_height: Points,
    pub control_padding_x: Points,
    pub control_padding_y: Points,
    pub control_radius: Points,
    pub minimum_hit_size: Points,
}}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypographyTokens {{
    pub body_size: Points,
    pub body_weight: FontWeight,
    pub label_size: Points,
    pub label_weight: FontWeight,
    pub line_height: Ratio,
}}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionTokens {{
    pub quick: Milliseconds,
}}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DesignTokens {{
    pub colours: ColourTokens,
    pub spacing: SpacingTokens,
    pub geometry: GeometryTokens,
    pub typography: TypographyTokens,
    pub motion: MotionTokens,
}}

impl DesignTokens {{
    pub const fn resolve(theme: ThemeVariant, density: DensityVariant) -> Self {{
        let colours = match theme {{
            ThemeVariant::Light => COLOURS_LIGHT,
            ThemeVariant::Dark => COLOURS_DARK,
            ThemeVariant::LightHighContrast => COLOURS_LIGHT_HIGH_CONTRAST,
            ThemeVariant::DarkHighContrast => COLOURS_DARK_HIGH_CONTRAST,
        }};
        let (spacing, geometry) = match density {{
            DensityVariant::Compact => (SPACING_COMPACT, GEOMETRY_COMPACT),
            DensityVariant::Comfortable => (SPACING_COMFORTABLE, GEOMETRY_COMFORTABLE),
        }};
        Self {{
            colours,
            spacing,
            geometry,
            typography: TYPOGRAPHY,
            motion: MOTION,
        }}
    }}
}}

{theme_constants}

{density_constants}

const TYPOGRAPHY: TypographyTokens = TypographyTokens {{
    body_size: Points({body_size}),
    body_weight: FontWeight({body_weight}),
    label_size: Points({label_size}),
    label_weight: FontWeight({label_weight}),
    line_height: Ratio({line_height}),
}};

const MOTION: MotionTokens = MotionTokens {{
    quick: Milliseconds({motion_quick}),
}};
"#,
        body_size = number_literal(required_number(&common, "typography.bodySize")?),
        body_weight = required_u16(&common, "typography.bodyWeight")?,
        label_size = number_literal(required_number(&common, "typography.labelSize")?),
        label_weight = required_u16(&common, "typography.labelWeight")?,
        line_height = number_literal(required_number(&common, "typography.lineHeight")?),
        motion_quick = required_u32(&common, "motion.quick")?,
    ))
}

fn generate_colour_constant(
    name: &str,
    values: &BTreeMap<String, ResolvedValue>,
) -> Result<String> {
    let constant = name.replace('-', "_").to_uppercase();
    let fields = [
        ("surface_canvas", "colour.surface.canvas"),
        ("surface_panel", "colour.surface.panel"),
        ("surface_raised", "colour.surface.raised"),
        ("border_subtle", "colour.border.subtle"),
        ("text_primary", "colour.text.primary"),
        ("text_muted", "colour.text.muted"),
        ("accent_primary", "colour.accent.primary"),
        ("accent_on_accent", "colour.accent.onAccent"),
        ("selection_background", "colour.selection.background"),
        ("focus_ring", "colour.focus.ring"),
        ("status_success", "colour.status.success"),
        ("status_warning", "colour.status.warning"),
        ("status_error", "colour.status.error"),
    ];
    let mut lines = Vec::new();
    for (field, path) in fields {
        lines.push(format!(
            "    {field}: {},",
            colour_literal(required_colour(values, path)?)
        ));
    }
    Ok(format!(
        "const COLOURS_{constant}: ColourTokens = ColourTokens {{\n{}\n}};",
        lines.join("\n")
    ))
}

fn generate_density_constant(
    name: &str,
    values: &BTreeMap<String, ResolvedValue>,
) -> Result<String> {
    let constant = name.to_uppercase();
    Ok(format!(
        "const SPACING_{constant}: SpacingTokens = SpacingTokens {{\n    unit: Points({}),\n    inline: Points({}),\n    block: Points({}),\n    section: Points({}),\n}};\n\nconst GEOMETRY_{constant}: GeometryTokens = GeometryTokens {{\n    application_bar_height: Points({}),\n    control_height: Points({}),\n    control_padding_x: Points({}),\n    control_padding_y: Points({}),\n    control_radius: Points({}),\n    minimum_hit_size: Points({}),\n}};",
        number_literal(required_number(values, "spacing.unit")?),
        number_literal(required_number(values, "spacing.inline")?),
        number_literal(required_number(values, "spacing.block")?),
        number_literal(required_number(values, "spacing.section")?),
        number_literal(required_number(values, "geometry.applicationBarHeight")?),
        number_literal(required_number(values, "geometry.controlHeight")?),
        number_literal(required_number(values, "geometry.controlPaddingX")?),
        number_literal(required_number(values, "geometry.controlPaddingY")?),
        number_literal(required_number(values, "geometry.controlRadius")?),
        number_literal(required_number(values, "geometry.minimumHitSize")?),
    ))
}

fn required_colour(values: &BTreeMap<String, ResolvedValue>, path: &str) -> Result<[u8; 4]> {
    match values.get(path) {
        Some(ResolvedValue::Color(value)) => Ok(*value),
        Some(_) => bail!("required generated token {path} is not a color"),
        None => bail!("required generated token {path} is missing"),
    }
}

fn required_number(values: &BTreeMap<String, ResolvedValue>, path: &str) -> Result<f64> {
    match values.get(path) {
        Some(ResolvedValue::Number(value)) => Ok(*value),
        Some(_) => bail!("required generated token {path} is not numeric"),
        None => bail!("required generated token {path} is missing"),
    }
}

fn required_u32(values: &BTreeMap<String, ResolvedValue>, path: &str) -> Result<u32> {
    let value = required_number(values, path)?;
    if value < 0.0 || value > f64::from(u32::MAX) || value.fract() != 0.0 {
        bail!("required generated token {path} is not a valid u32");
    }
    Ok(value as u32)
}

fn required_u16(values: &BTreeMap<String, ResolvedValue>, path: &str) -> Result<u16> {
    let value = required_number(values, path)?;
    if value < 0.0 || value > f64::from(u16::MAX) || value.fract() != 0.0 {
        bail!("required generated token {path} is not a valid u16");
    }
    Ok(value as u16)
}

fn colour_literal(value: [u8; 4]) -> String {
    format!(
        "Rgba8 {{\n        red: {},\n        green: {},\n        blue: {},\n        alpha: {},\n    }}",
        value[0], value[1], value[2], value[3]
    )
}

fn number_literal(value: f64) -> String {
    let mut literal = value.to_string();
    if !literal.contains(['.', 'e', 'E']) {
        literal.push_str(".0");
    }
    literal
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(base: &str, themes: &str, densities: &str) -> String {
        format!(
            r#"{{
                "tokens": {base},
                "$themes": {{
                    "light": {themes}, "dark": {themes},
                    "light-high-contrast": {themes}, "dark-high-contrast": {themes}
                }},
                "$densities": {{ "compact": {densities}, "comfortable": {densities} }}
            }}"#
        )
    }

    #[test]
    fn parses_supported_types_and_aliases() {
        let input = source(
            r##"{
                "colour": { "$type": "color", "$value": "#102030" },
                "alias": { "$type": "color", "$value": "{tokens.colour}" },
                "size": { "$type": "dimension", "$value": 4 },
                "ratio": { "$type": "number", "$value": 1.25 },
                "duration": { "$type": "duration", "$value": 80 }
                ,"fontSize": { "$type": "fontSize", "$value": 13 }
                ,"fontWeight": { "$type": "fontWeight", "$value": 600 }
            }"##,
            "{}",
            "{}",
        );
        let document = parse_source(&input).unwrap();
        let values = resolve_variant(&document, "dark", "compact").unwrap();
        assert_eq!(
            values["tokens.alias"],
            ResolvedValue::Color([16, 32, 48, 255])
        );
    }

    #[test]
    fn rejects_unsupported_type_and_construct() {
        let input = source(
            r#"{ "bad": { "$type": "shadow", "$value": 1 } }"#,
            "{}",
            "{}",
        );
        assert!(
            parse_source(&input)
                .unwrap_err()
                .to_string()
                .contains("unsupported $type")
        );

        let input = source(
            r#"{ "bad": { "$type": "number", "$value": 1, "$extensions": {} } }"#,
            "{}",
            "{}",
        );
        assert!(
            parse_source(&input)
                .unwrap_err()
                .to_string()
                .contains("unsupported construct")
        );
    }

    #[test]
    fn rejects_alias_type_mismatch_and_missing_reference() {
        let input = source(
            r##"{
                "colour": { "$type": "color", "$value": "#102030" },
                "bad": { "$type": "number", "$value": "{tokens.colour}" }
            }"##,
            "{}",
            "{}",
        );
        assert!(
            parse_source(&input)
                .unwrap_err()
                .to_string()
                .contains("different type")
        );

        let input = source(
            r#"{ "bad": { "$type": "number", "$value": "{tokens.missing}" } }"#,
            "{}",
            "{}",
        );
        assert!(
            parse_source(&input)
                .unwrap_err()
                .to_string()
                .contains("missing token")
        );
    }

    #[test]
    fn rejects_variant_override_type_change() {
        let input = r#"{
            "value": { "$type": "number", "$value": 1 },
            "$themes": {
                "light": { "value": { "$type": "dimension", "$value": 2 } },
                "dark": {}, "light-high-contrast": {}, "dark-high-contrast": {}
            },
            "$densities": { "compact": {}, "comfortable": {} }
        }"#;
        assert!(
            parse_source(input)
                .unwrap_err()
                .to_string()
                .contains("changes the type")
        );
    }

    #[test]
    fn rejects_cross_domain_and_common_token_overrides() {
        let theme_changes_spacing = r##"{
            "colour": { "$type": "color", "$value": "#000000" },
            "spacing": { "$type": "dimension", "$value": 4 },
            "$themes": {
                "light": { "spacing": { "$value": 8 } },
                "dark": {}, "light-high-contrast": {}, "dark-high-contrast": {}
            },
            "$densities": { "compact": {}, "comfortable": {} }
        }"##;
        assert!(
            parse_source(theme_changes_spacing)
                .unwrap_err()
                .to_string()
                .contains("theme overrides are restricted to colour.")
        );

        let density_changes_colour = r##"{
            "colour": { "$type": "color", "$value": "#000000" },
            "$themes": {
                "light": {}, "dark": {}, "light-high-contrast": {}, "dark-high-contrast": {}
            },
            "$densities": {
                "compact": { "colour": { "$value": "#ffffff" } },
                "comfortable": {}
            }
        }"##;
        assert!(
            parse_source(density_changes_colour)
                .unwrap_err()
                .to_string()
                .contains("density overrides are restricted to spacing. or geometry.")
        );

        let theme_changes_typography = r#"{
            "typography": { "body": { "$type": "fontSize", "$value": 13 } },
            "$themes": {
                "light": { "typography": { "body": { "$value": 14 } } },
                "dark": {}, "light-high-contrast": {}, "dark-high-contrast": {}
            },
            "$densities": { "compact": {}, "comfortable": {} }
        }"#;
        assert!(
            parse_source(theme_changes_typography)
                .unwrap_err()
                .to_string()
                .contains("theme overrides are restricted to colour.")
        );
    }

    #[test]
    fn rejects_alias_cycle() {
        let input = source(
            r#"{
                "a": { "$type": "number", "$value": "{tokens.b}" },
                "b": { "$type": "number", "$value": "{tokens.a}" }
            }"#,
            "{}",
            "{}",
        );
        assert!(
            parse_source(&input)
                .unwrap_err()
                .to_string()
                .contains("alias cycle")
        );
    }

    #[test]
    fn rejects_non_finite_number() {
        let input = source(
            r#"{ "bad": { "$type": "number", "$value": 1e400 } }"#,
            "{}",
            "{}",
        );
        assert!(
            parse_source(&input)
                .unwrap_err()
                .to_string()
                .contains("finite")
        );

        let input = source(
            r#"{ "bad": { "$type": "number", "$value": 3.5e38 } }"#,
            "{}",
            "{}",
        );
        assert!(
            parse_source(&input)
                .unwrap_err()
                .to_string()
                .contains("finite when represented as f32")
        );
    }

    #[test]
    fn theme_and_density_overrides_resolve_independently() {
        let input = r##"{
            "colour": { "primary": { "$type": "color", "$value": "#000000" } },
            "spacing": { "unit": { "$type": "dimension", "$value": 4 } },
            "$themes": {
                "light": { "colour": { "primary": { "$value": "#ffffff" } } },
                "dark": {},
                "light-high-contrast": { "colour": { "primary": { "$value": "#eeeeee" } } },
                "dark-high-contrast": { "colour": { "primary": { "$value": "#111111" } } }
            },
            "$densities": {
                "compact": { "spacing": { "unit": { "$value": 2 } } },
                "comfortable": { "spacing": { "unit": { "$value": 6 } } }
            }
        }"##;
        let document = parse_source(input).unwrap();
        let values = resolve_variant(&document, "light", "compact").unwrap();
        assert_eq!(
            values["colour.primary"],
            ResolvedValue::Color([255, 255, 255, 255])
        );
        assert_eq!(values["spacing.unit"], ResolvedValue::Number(2.0));
    }

    #[test]
    fn generation_is_deterministic_and_checked_in_output_has_no_drift() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let first = generate_from_file(root).unwrap();
        let second = generate_from_file(root).unwrap();
        assert_eq!(first, second);
        let checked_in = fs::read_to_string(root.join(GENERATED_PATH)).unwrap();
        assert_eq!(checked_in, first, "run `cargo xtask tokens generate`");
    }

    fn luminance(colour: [u8; 4]) -> f64 {
        let linear = |channel: u8| {
            let channel = f64::from(channel) / 255.0;
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(colour[0]) + 0.7152 * linear(colour[1]) + 0.0722 * linear(colour[2])
    }

    fn contrast(a: [u8; 4], b: [u8; 4]) -> f64 {
        let (lighter, darker) = if luminance(a) >= luminance(b) {
            (luminance(a), luminance(b))
        } else {
            (luminance(b), luminance(a))
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn authored_text_pairs_meet_declared_contrast_targets() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = fs::read_to_string(root.join(SOURCE_PATH)).unwrap();
        let document = parse_source(&source).unwrap();
        for theme in THEMES {
            let values = resolve_variant(&document, theme, "comfortable").unwrap();
            let target = if theme.contains("high-contrast") {
                7.0
            } else {
                4.5
            };
            for (foreground, background) in [
                ("colour.text.primary", "colour.surface.canvas"),
                ("colour.text.primary", "colour.surface.panel"),
                ("colour.text.muted", "colour.surface.panel"),
                ("colour.accent.onAccent", "colour.accent.primary"),
                ("colour.text.primary", "colour.selection.background"),
                ("colour.status.success", "colour.surface.panel"),
                ("colour.status.warning", "colour.surface.panel"),
                ("colour.status.error", "colour.surface.panel"),
            ] {
                let ratio = contrast(
                    required_colour(&values, foreground).unwrap(),
                    required_colour(&values, background).unwrap(),
                );
                assert!(
                    ratio >= target,
                    "{theme} {foreground} on {background} contrast {ratio:.2} is below {target:.1}:1"
                );
            }
            let focus_ratio = contrast(
                required_colour(&values, "colour.focus.ring").unwrap(),
                required_colour(&values, "colour.surface.panel").unwrap(),
            );
            assert!(
                focus_ratio >= 3.0,
                "{theme} focus ring on panel contrast {focus_ratio:.2} is below 3:1"
            );
        }
    }
}
