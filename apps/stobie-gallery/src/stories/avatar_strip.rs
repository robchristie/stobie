use eframe::egui;
use serde::Serialize;
use stobie_ui_egui::{
    ActionArgument, ActionKey, ActionScope, ActionSpec, ActionTarget, ArgumentContract,
    Availability, AvatarFace, AvatarItemSpec, AvatarOutline, AvatarSize, AvatarSpec,
    ContentTextSpec, DesignTokens, PresentationContext, PresentationScope, SemanticUiId,
    TextInteraction, TextOverflow, TextRole, observe_visible_only,
};

use super::StoryReport;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum PinnedAction {
    Open,
}

impl ActionKey for PinnedAction {
    fn stable_id(self) -> &'static str {
        "gallery.pinned.open"
    }
    fn specification(self) -> ActionSpec<Self> {
        ActionSpec {
            id: self,
            label: "Open conversation",
            description: "Open the conversation with this ID",
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Application,
        }
    }
    fn argument(self) -> ArgumentContract {
        ArgumentContract::Text {
            max_length: Some(16),
        }
    }
}

/// How a pinned item's avatar looks: a person's initials or a shape.
#[derive(Clone, Copy)]
enum Figure {
    Person,
    Shape(AvatarOutline<'static>, egui::Color32),
}

/// The pinned people and assistants: one with a long name, one away.
const PINNED: [(&str, &str, Figure); 7] = [
    ("ada", "Ada", Figure::Person),
    (
        "planner",
        "Planner",
        Figure::Shape(
            AvatarOutline::HEXAGON,
            egui::Color32::from_rgb(242, 178, 51),
        ),
    ),
    ("grace", "Grace Brewster Murray Hopper", Figure::Person),
    (
        "librarian",
        "Librarian",
        Figure::Shape(AvatarOutline::CIRCLE, egui::Color32::from_rgb(92, 158, 238)),
    ),
    ("edsger", "Edsger", Figure::Person),
    ("barbara", "Barbara", Figure::Person),
    (
        "gardener",
        "Gardener",
        Figure::Shape(
            AvatarOutline::ROUNDED_SQUARE,
            egui::Color32::from_rgb(96, 196, 150),
        ),
    ),
];

/// The pinned person who is away, so opening them is unavailable.
const AWAY: &str = "barbara";

const HEADING: ContentTextSpec = ContentTextSpec {
    role: TextRole::SectionHeading,
    overflow: TextOverflow::Ellipsis,
    max_lines: 1,
    interaction: TextInteraction::Inert,
};

/// Consumer-owned state: which pinned item is open.
#[derive(Clone, Debug)]
pub struct AvatarStripFixtureState {
    pub open: &'static str,
    pub opened: u32,
}

impl Default for AvatarStripFixtureState {
    fn default() -> Self {
        Self {
            open: "planner",
            opened: 0,
        }
    }
}

/// A strip of pinned people and assistants whose avatars are the controls:
/// the open one current, one with a long name elided, one unavailable. At
/// phone width the strip scrolls sideways.
pub(super) fn story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut AvatarStripFixtureState,
    report: &mut StoryReport,
) {
    observe_visible_only(ui, |ui| strip(ui, tokens, scale, state, report));
}

fn strip(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut AvatarStripFixtureState,
    report: &mut StoryReport,
) {
    let mut p = PresentationContext::new(
        ui,
        *tokens,
        scale,
        PresentationScope::new("gallery.pinned"),
        SemanticUiId::new("gallery.story"),
    );
    p.content(ui, "heading", "Pinned", HEADING);
    let opened = p.avatar_strip(ui, "strip", |ui, p| {
        let mut opened = None;
        for (id, name, figure) in PINNED {
            let avatar = match figure {
                Figure::Person => AvatarSpec::person(name, AvatarSize::Strip),
                Figure::Shape(outline, fill) => {
                    AvatarSpec::shape(outline, fill, AvatarFace::EYES, AvatarSize::Strip)
                }
            };
            let spec = AvatarItemSpec::new(avatar, name).selected(state.open == id);
            let availability = if id == AWAY {
                Availability::Disabled {
                    reason: "Barbara is away".into(),
                }
            } else {
                Availability::Enabled
            };
            let clicked = p
                .avatar_item(
                    ui,
                    PinnedAction::Open,
                    &spec,
                    ActionTarget::application(PinnedAction::Open),
                    &availability,
                    &ActionArgument::from(id),
                )
                .clicked();
            if clicked {
                opened = Some(id);
            }
        }
        opened
    });
    if let Some(id) = opened {
        state.open = id;
        state.opened += 1;
        report.repaint = Some("pinned item opened");
    }
    report.presented(&p.finish(ui));
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
        scale: f32,
        size: egui::Vec2,
    ) -> (stobie_ui_egui::UiSnapshot, egui::FullOutput) {
        let context = egui::Context::default();
        context.enable_accesskit();
        render(
            &context,
            StoryProbe {
                story: StoryId::AvatarsStrip,
                tokens: DesignTokens::resolve(variant, DensityVariant::Comfortable),
                scale,
                clip: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                theme: ApplicationTheme::analytical(),
                preferences: Default::default(),
                heading: false,
            },
            size,
        )
    }

    fn items(snapshot: &stobie_ui_egui::UiSnapshot) -> Vec<(&str, bool, bool)> {
        snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::NavigationItem)
            .map(|node| (node.name.as_str(), node.selected, node.enabled))
            .collect()
    }

    #[test]
    fn each_pinned_avatar_is_one_navigation_item_named_by_its_caption() {
        let (snapshot, mut output) = probe(ThemeVariant::Dark, 1.0, egui::vec2(616.0, 400.0));
        assert!(snapshot.text_audit.is_empty(), "{:#?}", snapshot.text_audit);
        assert_eq!(
            items(&snapshot),
            [
                ("Ada", false, true),
                ("Planner", true, true),
                ("Grace Brewster Murray Hopper", false, true),
                ("Librarian", false, true),
                ("Edsger", false, true),
                ("Barbara", false, false),
                ("Gardener", false, true),
            ]
        );
        let update = output.platform_output.accesskit_update.take().unwrap();
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{findings:?}");
        output.textures_delta.clear();
    }

    #[test]
    fn at_phone_width_the_strip_scrolls_and_publishes_what_it_shows() {
        let (snapshot, mut output) = probe(
            ThemeVariant::LightHighContrast,
            1.5,
            egui::vec2(296.0, 400.0),
        );
        let shown = items(&snapshot);
        assert!(!shown.is_empty() && shown.len() < PINNED.len(), "{shown:?}");
        let findings = audit_text_layouts(&snapshot.text);
        assert!(findings.is_empty(), "{findings:#?}");
        let update = output.platform_output.accesskit_update.take().unwrap();
        assert!(audit_accesskit(&snapshot, &update).is_empty());
        output.textures_delta.clear();
    }
}
