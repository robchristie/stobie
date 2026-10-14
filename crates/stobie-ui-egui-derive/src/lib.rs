//! Derive macros for `stobie-ui-egui`. Use them through the `stobie_ui_egui`
//! re-exports; generated code names `::stobie_ui_egui` unless the enum sets
//! `#[action(crate = "path")]`.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};
use syn::{
    Data, DeriveInput, Error, Expr, Fields, Ident, Lit, LitBool, LitInt, LitStr, Path, UnOp,
    Variant, ext::IdentExt, meta::ParseNestedMeta, parse_macro_input, spanned::Spanned,
};

/// Implement `ActionKey` and `ActionSet` for an enum of unit variants.
///
/// The enum takes `#[action(prefix = "app")]`. Each variant takes
/// `#[action(label = "…", description = "…")]` and, optionally, `compact`,
/// `shortcut` (for example `"Cmd+Shift+Z"`), `scope` (`"application"`,
/// `"pane"` or `"active_pane"`), `id`, `physical_only = "reason"` and an
/// `argument`: `"text"` (with an optional `max_length`), `"boolean"`,
/// `"number"` (with `minimum`, `maximum` and an optional `integer = true`) or
/// `"choice"` (with `choices = Type`, a `ChoiceArgument`). The stable ID
/// defaults to `<prefix>.<kebab-case variant>`. See the `stobie_ui_egui`
/// re-export for a compiled example.
#[proc_macro_derive(Action, attributes(action))]
pub fn derive_action(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

const VARIANT_KEYS: &str = "label, description, compact, shortcut, scope, id, physical_only, argument, max_length, minimum, maximum, integer or choices";

struct EnumOptions {
    prefix: Option<LitStr>,
    krate: Path,
}

#[derive(Default)]
struct VariantOptions {
    label: Option<LitStr>,
    description: Option<LitStr>,
    compact: Option<LitStr>,
    shortcut: Option<(LitStr, Shortcut)>,
    scope: Option<Ident>,
    id: Option<LitStr>,
    physical_only: Option<LitStr>,
    argument: Option<LitStr>,
    max_length: Option<LitInt>,
    minimum: Option<(Span, f64)>,
    maximum: Option<(Span, f64)>,
    integer: Option<LitBool>,
    choices: Option<Path>,
}

/// A variant's declared argument.
enum Argument {
    Text {
        max_length: Option<u32>,
    },
    Boolean,
    Number {
        minimum: f64,
        maximum: f64,
        integer: bool,
    },
    Choice(Path),
}

struct ActionVariant {
    ident: Ident,
    id: String,
    id_span: Span,
    label: LitStr,
    description: LitStr,
    compact: Option<LitStr>,
    shortcut: Option<(LitStr, Shortcut)>,
    scope: Ident,
    physical_only: Option<LitStr>,
    argument: Option<Argument>,
}

#[derive(Clone, PartialEq, Eq)]
struct Shortcut {
    command: bool,
    shift: bool,
    alt: bool,
    key: &'static str,
}

impl Shortcut {
    fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.command {
            parts.push("Cmd");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.push(self.key);
        parts.join("+")
    }
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let Data::Enum(data) = &input.data else {
        return Err(Error::new_spanned(
            &input.ident,
            "#[derive(Action)] supports only enums of unit variants",
        ));
    };
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "action enums cannot be generic",
        ));
    }
    if data.variants.is_empty() {
        return Err(Error::new_spanned(
            &input.ident,
            "an action enum needs at least one variant",
        ));
    }
    let options = enum_options(input)?;
    let mut errors: Option<Error> = None;
    let mut push = |error: Error| match &mut errors {
        Some(errors) => errors.combine(error),
        None => errors = Some(error),
    };
    let mut variants = Vec::new();
    for variant in &data.variants {
        match action_variant(variant, options.prefix.as_ref()) {
            Ok(action) => {
                if let Some(previous) = variants
                    .iter()
                    .find(|previous: &&ActionVariant| previous.id == action.id)
                {
                    push(Error::new(
                        action.id_span,
                        format!(
                            "stable ID `{}` is already used by `{}`",
                            action.id, previous.ident
                        ),
                    ));
                }
                if let Some((literal, shortcut)) = &action.shortcut
                    && let Some(previous) = variants.iter().find(|previous| {
                        previous
                            .shortcut
                            .as_ref()
                            .is_some_and(|(_, other)| other == shortcut)
                    })
                {
                    push(Error::new(
                        literal.span(),
                        format!(
                            "shortcut `{}` is already bound to `{}`",
                            shortcut.display(),
                            previous.ident
                        ),
                    ));
                }
                variants.push(action);
            }
            Err(error) => push(error),
        }
    }
    if let Some(errors) = errors {
        return Err(errors);
    }
    Ok(generate(&input.ident, &options.krate, &variants))
}

fn enum_options(input: &DeriveInput) -> syn::Result<EnumOptions> {
    let mut options = EnumOptions {
        prefix: None,
        krate: syn::parse_quote!(::stobie_ui_egui),
    };
    let mut krate_seen = false;
    for attribute in input.attrs.iter().filter(|a| a.path().is_ident("action")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                let value = string(&meta, &mut options.prefix, "prefix")?;
                if value.value().ends_with('.') {
                    return Err(Error::new(
                        value.span(),
                        "omit the trailing `.`; the derive adds it",
                    ));
                }
                identity(&value)
            } else if meta.path.is_ident("crate") {
                if krate_seen {
                    return Err(meta.error("duplicate `crate`"));
                }
                krate_seen = true;
                options.krate = meta.value()?.parse::<LitStr>()?.parse()?;
                Ok(())
            } else {
                Err(meta.error(
                    "unknown enum attribute; expected `prefix` or `crate`, and put per-action keys on variants",
                ))
            }
        })?;
    }
    Ok(options)
}

fn action_variant(variant: &Variant, prefix: Option<&LitStr>) -> syn::Result<ActionVariant> {
    if !matches!(variant.fields, Fields::Unit) {
        return Err(Error::new_spanned(
            &variant.fields,
            "action variants cannot carry data; declare a value with `argument = \"…\"` instead",
        ));
    }
    let mut options = VariantOptions::default();
    let mut annotated = false;
    for attribute in variant.attrs.iter().filter(|a| a.path().is_ident("action")) {
        annotated = true;
        attribute.parse_nested_meta(|meta| variant_option(&meta, &mut options))?;
    }
    let missing = |key| {
        Error::new_spanned(
            &variant.ident,
            format!(
                "`{}` needs `{key}`: #[action(label = \"…\", description = \"…\")]",
                variant.ident
            ),
        )
    };
    if !annotated {
        return Err(missing("label"));
    }
    let argument = argument(&variant.ident, &mut options)?;
    if let (Some(_), Some((literal, _))) = (&argument, &options.shortcut) {
        return Err(Error::new(
            literal.span(),
            "an action with an argument cannot have a shortcut: a key press supplies no value",
        ));
    }
    let label = options.label.ok_or_else(|| missing("label"))?;
    let description = options.description.ok_or_else(|| missing("description"))?;
    let id = match (&options.id, prefix) {
        (Some(id), _) => id.value(),
        (None, Some(prefix)) => format!(
            "{}.{}",
            prefix.value(),
            kebab(&variant.ident.unraw().to_string())
        ),
        (None, None) => {
            return Err(Error::new_spanned(
                &variant.ident,
                "add #[action(prefix = \"…\")] to the enum or give this variant an explicit `id`",
            ));
        }
    };
    Ok(ActionVariant {
        ident: variant.ident.clone(),
        id_span: options
            .id
            .as_ref()
            .map_or_else(|| variant.ident.span(), LitStr::span),
        id,
        label,
        description,
        compact: options.compact,
        shortcut: options.shortcut,
        scope: options
            .scope
            .unwrap_or_else(|| Ident::new("Application", Span::call_site())),
        physical_only: options.physical_only,
        argument,
    })
}

/// Check that argument keys match the declared kind and build the argument.
fn argument(ident: &Ident, options: &mut VariantOptions) -> syn::Result<Option<Argument>> {
    let kind = options.argument.as_ref().map(LitStr::value);
    let unexpected = |key: &str, span: Span| {
        Err(Error::new(
            span,
            match &kind {
                Some(kind) => format!("`{key}` does not apply to a {kind} argument"),
                None => format!("`{key}` needs an `argument`"),
            },
        ))
    };
    let max_length = match (&options.max_length, kind.as_deref()) {
        (Some(literal), Some("text")) => Some(literal.base10_parse::<u32>()?),
        (Some(literal), _) => return unexpected("max_length", literal.span()),
        (None, _) => None,
    };
    if !matches!(kind.as_deref(), Some("number")) {
        for (key, value) in [("minimum", &options.minimum), ("maximum", &options.maximum)] {
            if let Some((span, _)) = value {
                return unexpected(key, *span);
            }
        }
        if let Some(literal) = &options.integer {
            return unexpected("integer", literal.span());
        }
    }
    if let (Some(path), false) = (&options.choices, kind.as_deref() == Some("choice")) {
        return unexpected("choices", path.span());
    }
    let Some(literal) = &options.argument else {
        return Ok(None);
    };
    Ok(Some(match literal.value().as_str() {
        "text" => Argument::Text { max_length },
        "boolean" => Argument::Boolean,
        "number" => {
            let (Some((_, minimum)), Some((span, maximum))) = (options.minimum, options.maximum)
            else {
                return Err(Error::new_spanned(
                    ident,
                    "a number argument needs `minimum` and `maximum`",
                ));
            };
            if minimum > maximum {
                return Err(Error::new(span, "`maximum` is less than `minimum`"));
            }
            Argument::Number {
                minimum,
                maximum,
                integer: options.integer.as_ref().is_some_and(LitBool::value),
            }
        }
        "choice" => Argument::Choice(options.choices.take().ok_or_else(|| {
            Error::new_spanned(ident, "a choice argument needs `choices = Type`")
        })?),
        _ => {
            return Err(Error::new(
                literal.span(),
                "argument must be \"text\", \"boolean\", \"number\" or \"choice\"",
            ));
        }
    }))
}

/// A finite number literal, optionally negated.
fn number(meta: &ParseNestedMeta, slot: &mut Option<(Span, f64)>, key: &str) -> syn::Result<()> {
    if slot.is_some() {
        return Err(meta.error(format!("duplicate `{key}`")));
    }
    let expression: Expr = meta.value()?.parse()?;
    let span = expression.span();
    let (negated, literal) = match &expression {
        Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => (true, &*unary.expr),
        expression => (false, expression),
    };
    let value = match literal {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Int(int) => int.base10_parse::<f64>()?,
            Lit::Float(float) => float.base10_parse::<f64>()?,
            _ => return Err(Error::new(span, format!("`{key}` must be a number"))),
        },
        _ => return Err(Error::new(span, format!("`{key}` must be a number"))),
    };
    let value = if negated { -value } else { value };
    if !value.is_finite() {
        return Err(Error::new(span, format!("`{key}` must be finite")));
    }
    *slot = Some((span, value));
    Ok(())
}

fn variant_option(meta: &ParseNestedMeta, options: &mut VariantOptions) -> syn::Result<()> {
    let key = meta
        .path
        .get_ident()
        .map(Ident::to_string)
        .unwrap_or_default();
    match key.as_str() {
        "label" => string(meta, &mut options.label, "label").map(drop),
        "description" => string(meta, &mut options.description, "description").map(drop),
        "compact" => string(meta, &mut options.compact, "compact").map(drop),
        "physical_only" => string(meta, &mut options.physical_only, "physical_only").map(drop),
        "id" => identity(&string(meta, &mut options.id, "id")?),
        "argument" => string(meta, &mut options.argument, "argument").map(drop),
        "max_length" => {
            if options.max_length.is_some() {
                return Err(meta.error("duplicate `max_length`"));
            }
            options.max_length = Some(meta.value()?.parse()?);
            Ok(())
        }
        "minimum" => number(meta, &mut options.minimum, "minimum"),
        "maximum" => number(meta, &mut options.maximum, "maximum"),
        "integer" => {
            if options.integer.is_some() {
                return Err(meta.error("duplicate `integer`"));
            }
            options.integer = Some(meta.value()?.parse()?);
            Ok(())
        }
        "choices" => {
            if options.choices.is_some() {
                return Err(meta.error("duplicate `choices`"));
            }
            options.choices = Some(meta.value()?.parse()?);
            Ok(())
        }
        "shortcut" => {
            if options.shortcut.is_some() {
                return Err(meta.error("duplicate `shortcut`"));
            }
            let literal: LitStr = meta.value()?.parse()?;
            let shortcut = parse_shortcut(&literal)?;
            options.shortcut = Some((literal, shortcut));
            Ok(())
        }
        "scope" => {
            if options.scope.is_some() {
                return Err(meta.error("duplicate `scope`"));
            }
            let literal: LitStr = meta.value()?.parse()?;
            let scope = match literal.value().as_str() {
                "application" => "Application",
                "pane" => "Pane",
                "active_pane" => "ActivePane",
                _ => {
                    return Err(Error::new(
                        literal.span(),
                        "scope must be \"application\", \"pane\" or \"active_pane\"",
                    ));
                }
            };
            options.scope = Some(Ident::new(scope, literal.span()));
            Ok(())
        }
        _ => Err(meta.error(format!("unknown action attribute; expected {VARIANT_KEYS}"))),
    }
}

/// Parse one non-blank string value and reject a repeated key.
fn string(meta: &ParseNestedMeta, slot: &mut Option<LitStr>, key: &str) -> syn::Result<LitStr> {
    if slot.is_some() {
        return Err(meta.error(format!("duplicate `{key}`")));
    }
    let literal: LitStr = meta.value()?.parse()?;
    if literal.value().trim().is_empty() {
        return Err(Error::new(
            literal.span(),
            format!("`{key}` cannot be blank"),
        ));
    }
    *slot = Some(literal.clone());
    Ok(literal)
}

fn identity(literal: &LitStr) -> syn::Result<()> {
    if literal
        .value()
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        Err(Error::new(
            literal.span(),
            "stable identities cannot contain whitespace or control characters",
        ))
    } else {
        Ok(())
    }
}

fn kebab(name: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let mut kebab = String::new();
    for (index, &character) in characters.iter().enumerate() {
        if character.is_uppercase() {
            let previous = index.checked_sub(1).map(|i| characters[i]);
            let next = characters.get(index + 1);
            let word_start = previous.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit())
                || (previous.is_some_and(char::is_uppercase)
                    && next.is_some_and(|n| n.is_lowercase()));
            if word_start {
                kebab.push('-');
            }
            kebab.extend(character.to_lowercase());
        } else if character == '_' {
            kebab.push('-');
        } else {
            kebab.push(character);
        }
    }
    kebab
}

/// Accepted spellings for each `ShortcutKey` variant. Matching ignores case.
const KEYS: &[(&str, &[&str])] = &[
    ("A", &["a"]),
    ("B", &["b"]),
    ("C", &["c"]),
    ("D", &["d"]),
    ("E", &["e"]),
    ("F", &["f"]),
    ("G", &["g"]),
    ("H", &["h"]),
    ("I", &["i"]),
    ("J", &["j"]),
    ("K", &["k"]),
    ("L", &["l"]),
    ("M", &["m"]),
    ("N", &["n"]),
    ("O", &["o"]),
    ("P", &["p"]),
    ("Q", &["q"]),
    ("R", &["r"]),
    ("S", &["s"]),
    ("T", &["t"]),
    ("U", &["u"]),
    ("V", &["v"]),
    ("W", &["w"]),
    ("X", &["x"]),
    ("Y", &["y"]),
    ("Z", &["z"]),
    ("Zero", &["0"]),
    ("One", &["1"]),
    ("Two", &["2"]),
    ("Three", &["3"]),
    ("Four", &["4"]),
    ("Five", &["5"]),
    ("Six", &["6"]),
    ("Seven", &["7"]),
    ("Eight", &["8"]),
    ("Nine", &["9"]),
    ("Enter", &["enter", "return"]),
    ("Escape", &["escape", "esc"]),
    ("Space", &["space"]),
    ("Backspace", &["backspace"]),
    ("Delete", &["delete", "del"]),
    ("Insert", &["insert"]),
    ("Home", &["home"]),
    ("End", &["end"]),
    ("PageUp", &["pageup"]),
    ("PageDown", &["pagedown"]),
    ("ArrowUp", &["arrowup", "up"]),
    ("ArrowDown", &["arrowdown", "down"]),
    ("ArrowLeft", &["arrowleft", "left"]),
    ("ArrowRight", &["arrowright", "right"]),
    ("Comma", &["comma", ","]),
    ("Period", &["period", "."]),
    ("Minus", &["minus", "-"]),
    ("Equals", &["equals", "="]),
    ("Slash", &["slash", "/"]),
    ("F1", &["f1"]),
    ("F2", &["f2"]),
    ("F3", &["f3"]),
    ("F4", &["f4"]),
    ("F5", &["f5"]),
    ("F6", &["f6"]),
    ("F7", &["f7"]),
    ("F8", &["f8"]),
    ("F9", &["f9"]),
    ("F10", &["f10"]),
    ("F11", &["f11"]),
    ("F12", &["f12"]),
];

const SHIFTED_CHARACTERS: &[&str] = &[
    "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Comma",
    "Period", "Minus", "Equals", "Slash",
];

fn parse_shortcut(literal: &LitStr) -> syn::Result<Shortcut> {
    let error = |message: String| Error::new(literal.span(), message);
    let value = literal.value();
    let parts: Vec<&str> = value.split('+').map(str::trim).collect();
    if parts.iter().any(|part| part.is_empty()) {
        return Err(error(format!(
            "shortcut `{value}` has an empty part; write modifiers then one key, such as \"Cmd+Shift+Z\""
        )));
    }
    let (key, modifiers) = parts.split_last().expect("split yields one part");
    let mut shortcut = Shortcut {
        command: false,
        shift: false,
        alt: false,
        key: "",
    };
    for modifier in modifiers {
        let slot = match modifier.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "ctrl" | "control" => &mut shortcut.command,
            "shift" => &mut shortcut.shift,
            "alt" | "option" => &mut shortcut.alt,
            _ => {
                return Err(error(format!(
                    "`{modifier}` is not a modifier; use Cmd (or Ctrl), Shift and Alt before one key"
                )));
            }
        };
        if *slot {
            return Err(error(format!(
                "`{modifier}` repeats a modifier; Cmd and Ctrl both name the platform command modifier"
            )));
        }
        *slot = true;
    }
    let lower = key.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "cmd" | "command" | "ctrl" | "control" | "shift" | "alt" | "option"
    ) {
        return Err(error(format!(
            "shortcut `{value}` needs a key after its modifiers"
        )));
    }
    if lower == "tab" {
        return Err(error(
            "Tab is reserved for keyboard focus traversal".to_owned(),
        ));
    }
    shortcut.key = KEYS
        .iter()
        .find(|(_, spellings)| spellings.contains(&lower.as_str()))
        .map(|(variant, _)| *variant)
        .ok_or_else(|| {
            error(format!(
                "unknown key `{key}`; use A–Z, 0–9, Enter, Escape, Space, Backspace, Delete, Insert, Home, End, PageUp, PageDown, ArrowUp, ArrowDown, ArrowLeft, ArrowRight, Comma, Period, Minus, Equals, Slash or F1–F12"
            ))
        })?;
    // egui matches the logical key, and Shift changes the character these keys
    // produce on common layouts, so such a chord would never fire.
    if shortcut.shift && SHIFTED_CHARACTERS.contains(&shortcut.key) {
        return Err(error(format!(
            "Shift changes the character `{key}` produces, so this chord never reaches the application; bind a letter or named key instead"
        )));
    }
    Ok(shortcut)
}

fn generate(name: &Ident, krate: &Path, variants: &[ActionVariant]) -> TokenStream2 {
    let idents: Vec<_> = variants.iter().map(|v| &v.ident).collect();
    let ids = variants.iter().map(|v| &v.id);
    let specifications = variants.iter().map(|variant| {
        let ActionVariant {
            ident,
            label,
            description,
            scope,
            ..
        } = variant;
        let compact = match &variant.compact {
            Some(compact) => quote!(::core::option::Option::Some(#compact)),
            None => quote!(::core::option::Option::None),
        };
        let shortcut = match &variant.shortcut {
            Some((literal, shortcut)) => {
                let Shortcut {
                    command,
                    shift,
                    alt,
                    key,
                } = shortcut;
                // Span the key at the literal so a table/enum drift points at it.
                let key = Ident::new(key, literal.span());
                quote_spanned!(literal.span()=> ::core::option::Option::Some(#krate::ActionShortcut {
                    command: #command,
                    shift: #shift,
                    alt: #alt,
                    key: #krate::ShortcutKey::#key,
                }))
            }
            None => quote!(::core::option::Option::None),
        };
        quote! {
            Self::#ident => #krate::ActionSpec {
                id: self,
                label: #label,
                description: #description,
                compact_label: #compact,
                shortcut: #shortcut,
                scope: #krate::ActionScope::#scope,
            }
        }
    });
    let physical: Vec<_> = variants
        .iter()
        .filter_map(|v| v.physical_only.as_ref().map(|reason| (&v.ident, reason)))
        .collect();
    let invocation = if physical.is_empty() {
        quote!()
    } else {
        let arms = physical.iter().map(|(ident, reason)| {
            quote!(Self::#ident => #krate::ActionInvocation::PhysicalOnly { reason: #reason },)
        });
        quote! {
            fn invocation(self) -> #krate::ActionInvocation {
                #[allow(unreachable_patterns)]
                match self {
                    #(#arms)*
                    _ => #krate::ActionInvocation::Semantic,
                }
            }
        }
    };
    let arguments: Vec<_> = variants
        .iter()
        .filter_map(|variant| {
            let ident = &variant.ident;
            let contract = match variant.argument.as_ref()? {
                Argument::Text { max_length } => {
                    let max_length = match max_length {
                        Some(limit) => quote!(::core::option::Option::Some(#limit)),
                        None => quote!(::core::option::Option::None),
                    };
                    quote!(#krate::ArgumentContract::Text { max_length: #max_length })
                }
                Argument::Boolean => quote!(#krate::ArgumentContract::Boolean),
                Argument::Number {
                    minimum,
                    maximum,
                    integer,
                } => {
                    let minimum = proc_macro2::Literal::f64_suffixed(*minimum);
                    let maximum = proc_macro2::Literal::f64_suffixed(*maximum);
                    quote!(#krate::ArgumentContract::Number {
                        minimum: #minimum,
                        maximum: #maximum,
                        integer: #integer,
                    })
                }
                Argument::Choice(path) => {
                    quote_spanned!(path.span()=> #krate::ArgumentContract::Choice {
                        options: <#path as #krate::ChoiceArgument>::options(),
                    })
                }
            };
            Some(quote!(Self::#ident => #contract,))
        })
        .collect();
    let argument = if arguments.is_empty() {
        quote!()
    } else {
        quote! {
            fn argument(self) -> #krate::ArgumentContract {
                #[allow(unreachable_patterns)]
                match self {
                    #(#arguments)*
                    _ => #krate::ArgumentContract::None,
                }
            }
        }
    };
    quote! {
        impl #krate::ActionKey for #name {
            fn stable_id(self) -> &'static str {
                match self {
                    #(Self::#idents => #ids,)*
                }
            }

            fn specification(self) -> #krate::ActionSpec<Self> {
                match self {
                    #(#specifications,)*
                }
            }

            #invocation

            #argument
        }

        impl #krate::ActionSet for #name {
            const ALL: &'static [Self] = &[#(Self::#idents),*];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Ident, IdentExt, kebab};

    #[test]
    fn kebab_case_splits_words_acronyms_and_underscores() {
        assert_eq!(kebab("Apply"), "apply");
        assert_eq!(kebab("ResetFilters"), "reset-filters");
        assert_eq!(kebab("ToggleReviewed"), "toggle-reviewed");
        assert_eq!(kebab("OpenHTTPLink"), "open-http-link");
        assert_eq!(kebab("Tool2Mode"), "tool2-mode");
        assert_eq!(kebab("Snake_Case"), "snake-case");
        let raw: Ident = syn::parse_quote!(r#Move);
        assert_eq!(kebab(&raw.unraw().to_string()), "move");
    }
}
