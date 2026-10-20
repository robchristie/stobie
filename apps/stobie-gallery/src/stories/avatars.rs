use eframe::egui;
use stobie_ui_egui::{
    AvatarFace, AvatarOutline, AvatarSize, AvatarSpec, ContentTextSpec, DesignTokens,
    OutlineSegment, PresentationContext, PresentationScope, SemanticUiId, TextInteraction,
    TextOverflow, TextRole, avatar_size, observe_visible_only,
};

use super::StoryReport;

/// Shapes an application supplies as data, as a consumer would: a blob, a
/// drop, a cloud with concave dips, and a softened triangle.
const BLOB: AvatarOutline<'static> = AvatarOutline::new(
    [0.5, 0.08],
    &[
        OutlineSegment::Cubic([0.601, 0.078], [0.745, 0.228], [0.812, 0.34]),
        OutlineSegment::Cubic([0.878, 0.452], [0.95, 0.657], [0.898, 0.75]),
        OutlineSegment::Cubic([0.846, 0.843], [0.631, 0.901], [0.5, 0.9]),
        OutlineSegment::Cubic([0.369, 0.899], [0.159, 0.837], [0.11, 0.745]),
        OutlineSegment::Cubic([0.061, 0.653], [0.141, 0.461], [0.206, 0.35]),
        OutlineSegment::Cubic([0.271, 0.239], [0.399, 0.082], [0.5, 0.08]),
    ],
);

const DROP: AvatarOutline<'static> = AvatarOutline::new(
    [0.5, 0.04],
    &[
        OutlineSegment::Cubic([0.62, 0.22], [0.9, 0.45], [0.9, 0.62]),
        OutlineSegment::Cubic([0.9, 0.85], [0.72, 0.98], [0.5, 0.98]),
        OutlineSegment::Cubic([0.28, 0.98], [0.1, 0.85], [0.1, 0.62]),
        OutlineSegment::Cubic([0.1, 0.45], [0.38, 0.22], [0.5, 0.04]),
    ],
);

const CLOUD: AvatarOutline<'static> = AvatarOutline::new(
    [0.27, 0.8],
    &[
        OutlineSegment::Line([0.75, 0.8]),
        OutlineSegment::Cubic([0.857, 0.8], [0.944, 0.716], [0.95, 0.61]),
        OutlineSegment::Cubic([0.955, 0.504], [0.876, 0.412], [0.77, 0.401]),
        OutlineSegment::Cubic([0.77, 0.312], [0.723, 0.229], [0.646, 0.184]),
        OutlineSegment::Cubic([0.568, 0.139], [0.473, 0.139], [0.395, 0.183]),
        OutlineSegment::Cubic([0.318, 0.228], [0.27, 0.311], [0.27, 0.4]),
        OutlineSegment::Cubic([0.16, 0.4], [0.07, 0.49], [0.07, 0.6]),
        OutlineSegment::Cubic([0.07, 0.71], [0.16, 0.8], [0.27, 0.8]),
    ],
);

const TRIANGLE: AvatarOutline<'static> = AvatarOutline::new(
    [0.597, 0.267],
    &[
        OutlineSegment::Line([0.843, 0.693]),
        OutlineSegment::Quadratic([0.94, 0.86], [0.746, 0.86]),
        OutlineSegment::Line([0.254, 0.86]),
        OutlineSegment::Quadratic([0.06, 0.86], [0.157, 0.693]),
        OutlineSegment::Line([0.403, 0.267]),
        OutlineSegment::Quadratic([0.5, 0.1], [0.597, 0.267]),
    ],
);

const fn rgb(red: u8, green: u8, blue: u8) -> egui::Color32 {
    egui::Color32::from_rgb(red, green, blue)
}

/// Built-in and application shapes, each in its own colour, with the face
/// placed where the shape's body is.
const SHAPES: [(&str, AvatarOutline<'static>, egui::Color32, [f32; 2]); 7] = [
    (
        "Circle",
        AvatarOutline::CIRCLE,
        rgb(92, 158, 238),
        [0.5, 0.46],
    ),
    (
        "Rounded square",
        AvatarOutline::ROUNDED_SQUARE,
        rgb(96, 196, 150),
        [0.5, 0.46],
    ),
    (
        "Hexagon",
        AvatarOutline::HEXAGON,
        rgb(244, 190, 72),
        [0.5, 0.46],
    ),
    ("Blob", BLOB, rgb(236, 124, 104), [0.5, 0.48]),
    ("Drop", DROP, rgb(150, 122, 228), [0.5, 0.64]),
    ("Cloud", CLOUD, rgb(176, 210, 238), [0.52, 0.54]),
    ("Triangle", TRIANGLE, rgb(54, 66, 92), [0.5, 0.66]),
];

const PEOPLE: [(&str, &str); 6] = [
    ("Ada Lovelace", "Ada Lovelace"),
    (
        "Grace Brewster Murray Hopper",
        "Grace Brewster Murray Hopper",
    ),
    ("Cher", "Cher"),
    ("émile zola", "émile zola"),
    ("\u{738b}\u{5c0f}\u{660e}", "A name the fonts cannot draw"),
    ("", "No name"),
];

/// A small landscape drawn in code, so the story needs no image file.
fn photo(ctx: &egui::Context) -> egui::load::SizedTexture {
    let id = egui::Id::new("gallery.avatars.photo");
    let handle = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id));
    let handle = handle.unwrap_or_else(|| {
        let (width, height) = (96_usize, 64_usize);
        let mut pixels = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            for x in 0..width {
                let (fx, fy) = (x as f32, y as f32);
                let hill = 44.0 + 7.0 * (fx / 14.0).sin();
                let sun = (fx - 66.0).hypot(fy - 18.0) < 9.0;
                let colour: [u8; 3] = if fy > hill {
                    [64, 150 - (fy - hill) as u8 * 2, 82]
                } else if sun {
                    [250, 214, 92]
                } else {
                    [120 + y as u8, 172 + y as u8, 236]
                };
                pixels.extend_from_slice(&colour);
            }
        }
        let image = egui::ColorImage::from_rgb([width, height], &pixels);
        let handle = ctx.load_texture("gallery.avatars.photo", image, Default::default());
        ctx.data_mut(|data| data.insert_temp(id, handle.clone()));
        handle
    });
    egui::load::SizedTexture::from_handle(&handle)
}

const CAPTION: ContentTextSpec = ContentTextSpec {
    role: TextRole::Body,
    overflow: TextOverflow::Ellipsis,
    max_lines: 1,
    interaction: TextInteraction::Inert,
};

const HEADING: ContentTextSpec = ContentTextSpec {
    role: TextRole::SectionHeading,
    overflow: TextOverflow::Ellipsis,
    max_lines: 1,
    interaction: TextInteraction::Inert,
};

/// The avatar figures, at every size, in a page that scrolls: it publishes
/// only what is visible.
pub(super) fn story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    report: &mut StoryReport,
) {
    observe_visible_only(ui, |ui| figures(ui, tokens, scale, report));
}

fn figures(ui: &mut egui::Ui, tokens: &DesignTokens, scale: f32, report: &mut StoryReport) {
    let mut p = PresentationContext::new(
        ui,
        *tokens,
        scale,
        PresentationScope::new("gallery.avatars"),
        SemanticUiId::new("gallery.story"),
    );
    let photo = photo(ui.ctx());
    let gap = tokens.spacing.section.0;
    egui::ScrollArea::vertical()
        .id_salt("avatars-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            p.scoped(ui, "sizes", |ui, p| {
                p.content(ui, "heading", "Sizes", HEADING);
                let hexagon = AvatarSpec::shape(
                    AvatarOutline::HEXAGON,
                    SHAPES[2].2,
                    AvatarFace::EYES,
                    AvatarSize::Header,
                );
                cells(ui, p, tokens, scale, |ui, p, cell| {
                    for (key, size, label) in [
                        ("header", AvatarSize::Header, "Header"),
                        ("list", AvatarSize::List, "List"),
                        ("strip", AvatarSize::Strip, "Strip"),
                    ] {
                        cell(ui, p, key, &AvatarSpec { size, ..hexagon }, label);
                    }
                });
            });
            ui.add_space(gap);
            p.scoped(ui, "people", |ui, p| {
                p.content(ui, "heading", "People", HEADING);
                cells(ui, p, tokens, scale, |ui, p, cell| {
                    for (index, (name, label)) in PEOPLE.into_iter().enumerate() {
                        let key = format!("person-{index}");
                        cell(
                            ui,
                            p,
                            &key,
                            &AvatarSpec::person(name, AvatarSize::List),
                            label,
                        );
                    }
                });
            });
            ui.add_space(gap);
            p.scoped(ui, "shapes", |ui, p| {
                p.content(ui, "heading", "Shapes with faces", HEADING);
                cells(ui, p, tokens, scale, |ui, p, cell| {
                    for (index, (label, outline, fill, centre)) in SHAPES.into_iter().enumerate() {
                        let face = AvatarFace::Eyes { centre };
                        let spec = AvatarSpec::shape(outline, fill, face, AvatarSize::List);
                        cell(ui, p, &format!("shape-{index}"), &spec, label);
                    }
                });
            });
            ui.add_space(gap);
            p.scoped(ui, "images", |ui, p| {
                p.content(ui, "heading", "Images", HEADING);
                cells(ui, p, tokens, scale, |ui, p, cell| {
                    let person = AvatarSpec::person("Ada Lovelace", AvatarSize::List);
                    cell(ui, p, "circle", &person.with_image(photo), "Photo");
                    let shape = AvatarSpec::shape(
                        AvatarOutline::HEXAGON,
                        SHAPES[2].2,
                        AvatarFace::EYES,
                        AvatarSize::List,
                    );
                    cell(
                        ui,
                        p,
                        "hexagon",
                        &shape.with_image(photo),
                        "Photo in a hexagon",
                    );
                    cell(ui, p, "loading", &person, "Photo still loading");
                });
            });
        });
    report.presented(&p.finish(ui));
}

type Cell<'a> = dyn Fn(&mut egui::Ui, &mut PresentationContext, &str, &AvatarSpec<'_>, &str) + 'a;

/// Lay cells out in rows that wrap, each an avatar with its caption beside
/// it, as a list row would place them.
fn cells(
    ui: &mut egui::Ui,
    p: &mut PresentationContext,
    tokens: &DesignTokens,
    scale: f32,
    add: impl FnOnce(&mut egui::Ui, &mut PresentationContext, &Cell<'_>),
) {
    let side = avatar_size(tokens, AvatarSize::Strip, scale);
    let inline = tokens.spacing.inline.0;
    // As many columns of at least 180 points as fit, sharing the width.
    let available = ui.available_width();
    let gap = ui.spacing().item_spacing.x;
    let minimum = 180.0 * scale.clamp(1.0, 1.5);
    let columns = ((available + gap) / (minimum + gap)).floor().max(1.0);
    let width = ((available - (columns - 1.0) * gap) / columns).max(1.0);
    let cell = move |ui: &mut egui::Ui,
                     p: &mut PresentationContext,
                     key: &str,
                     spec: &AvatarSpec<'_>,
                     label: &str| {
        let size = egui::vec2(width, side);
        ui.allocate_ui_with_layout(
            size,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_size(size);
                ui.set_max_width(width);
                p.scoped(ui, key.to_owned(), |ui, p| {
                    p.avatar(ui, spec);
                    ui.add_space(inline);
                    p.content(ui, "caption", label, CAPTION);
                });
            },
        );
    };
    ui.horizontal_wrapped(|ui| add(ui, p, &cell));
}

#[cfg(test)]
mod tests {
    use stobie_ui_egui::{
        ApplicationTheme, DensityVariant, ThemeVariant, UiRole, audit_accesskit, audit_text_layouts,
    };

    use super::*;
    use crate::catalogue::StoryId;
    use crate::stories::probe::{StoryProbe, render};

    fn probe(
        variant: ThemeVariant,
        density: DensityVariant,
        scale: f32,
        size: egui::Vec2,
    ) -> (stobie_ui_egui::UiSnapshot, egui::FullOutput) {
        let context = egui::Context::default();
        context.enable_accesskit();
        render(
            &context,
            StoryProbe {
                story: StoryId::AvatarsFigures,
                tokens: DesignTokens::resolve(variant, density),
                scale,
                clip: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                theme: ApplicationTheme::analytical(),
                preferences: Default::default(),
                heading: false,
            },
            size,
        )
    }

    fn meshes(output: &egui::FullOutput) -> Vec<std::sync::Arc<egui::Mesh>> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Mesh(mesh) => Some(mesh.clone()),
                _ => None,
            })
            .collect()
    }

    fn painted_text(output: &egui::FullOutput) -> Vec<String> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn avatars_are_decorative_beside_their_captions_in_both_themes() {
        for (variant, density) in [
            (ThemeVariant::Dark, DensityVariant::Comfortable),
            (ThemeVariant::Light, DensityVariant::Compact),
        ] {
            let (snapshot, mut output) = probe(variant, density, 1.0, egui::vec2(616.0, 1200.0));
            assert!(snapshot.text_audit.is_empty(), "{:#?}", snapshot.text_audit);
            let painted = painted_text(&output);
            for text in [
                "Sizes",
                "Ada Lovelace",
                "No name",
                "Cloud",
                "Photo still loading",
                "AL",
            ] {
                assert!(painted.iter().any(|p| p == text), "{text} in {painted:?}");
            }
            // Four headings and 19 captions are measured; the initials
            // painted inside avatars are not, and avatars publish no nodes.
            assert_eq!(snapshot.text.len(), 23);
            let roles: Vec<_> = snapshot.nodes.iter().map(|node| node.role).collect();
            assert_eq!(roles, [UiRole::Application], "{:#?}", snapshot.nodes);
            // Every shape is filled, and the photo fills two outlines.
            let meshes = meshes(&output);
            let photo = meshes
                .iter()
                .filter(|mesh| mesh.texture_id != egui::TextureId::default())
                .count();
            assert_eq!(photo, 2);
            let update = output.platform_output.accesskit_update.take().unwrap();
            let findings = audit_accesskit(&snapshot, &update);
            assert!(findings.is_empty(), "{findings:?}");
            output.textures_delta.clear();
        }
    }

    #[test]
    fn a_narrow_column_at_larger_text_elides_long_captions() {
        let (snapshot, mut output) = probe(
            ThemeVariant::LightHighContrast,
            DensityVariant::Comfortable,
            1.5,
            egui::vec2(296.0, 576.0),
        );
        let findings = audit_text_layouts(&snapshot.text);
        assert!(findings.is_empty(), "{findings:#?}");
        assert!(snapshot.text.iter().any(|text| text.truncated));
        output.textures_delta.clear();
    }
}
