use eframe::egui;
use serde::Serialize;
use stobie_ui_egui::{
    ActionArgument, ActionButtonContent, ActionKey, ActionScope, ActionSpec, ActionTarget,
    ArgumentContract, Availability, AvatarSize, AvatarSpec, BoundAction, DesignTokens,
    HeaderControl, HeaderControlResponse, IconId, NavigationContent, NavigationDetail,
    NavigationItemSpec, NavigationOptions, NavigationPane, NavigationSpec, PresentationContext,
    PresentationScope, ReadingTextSpec, SemanticUiId, TextRole, observe_visible_only,
};

use super::StoryReport;
use crate::catalogue::StoryId;

/// Consumer-owned state for the heading stories: what is pinned and open,
/// and what the reader did.
#[derive(Clone, Debug, Default, Serialize)]
pub struct HeaderFixtureState {
    #[serde(skip)]
    story: Option<StoryId>,
    pub pinned: bool,
    pub muted: bool,
    pub open: Option<String>,
    pub details: bool,
    pub hidden: u32,
    pub edits: u32,
}

impl HeaderFixtureState {
    /// Each story starts from its own fixture: Grace pinned, and in the
    /// navigation story her conversation open.
    fn reset_for(&mut self, story: StoryId) {
        if self.story == Some(story) {
            return;
        }
        *self = Self {
            story: Some(story),
            pinned: true,
            open: (story == StoryId::NavigationHeader).then(|| "grace".into()),
            ..Self::default()
        };
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum HeaderAction {
    Open,
    Back,
    Details,
    Pin,
    Mute,
    Hide,
    EditProfile,
    Edit,
    Share,
    Search,
    Settings,
}

impl ActionKey for HeaderAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Open => "gallery.header.open",
            Self::Back => "gallery.header.back",
            Self::Details => "gallery.header.details",
            Self::Pin => "gallery.header.pin",
            Self::Mute => "gallery.header.mute",
            Self::Hide => "gallery.header.hide",
            Self::EditProfile => "gallery.header.edit_profile",
            Self::Edit => "gallery.header.edit",
            Self::Share => "gallery.header.share",
            Self::Search => "gallery.header.search",
            Self::Settings => "gallery.header.settings",
        }
    }
    fn specification(self) -> ActionSpec<Self> {
        let (label, description) = match self {
            Self::Open => ("Open", "Open the conversation with this ID"),
            Self::Back => ("Back", "Return to the previous level"),
            Self::Details => ("Details", "Show or hide this conversation's details"),
            Self::Pin => ("Pinned", "Pin or unpin this conversation"),
            Self::Mute => ("Muted", "Mute or unmute this conversation"),
            Self::Hide => ("Hide", "Hide this conversation from the list"),
            Self::EditProfile => ("Edit profile", "Edit this person's profile"),
            Self::Edit => ("Edit", "Edit this section"),
            Self::Share => ("Share", "Share a link to this list"),
            Self::Search => ("Search", "Search this list"),
            Self::Settings => ("Settings", "Change this list's settings"),
        };
        ActionSpec {
            id: self,
            label,
            description,
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Application,
        }
    }
    fn argument(self) -> ArgumentContract {
        match self {
            Self::Open | Self::Pin | Self::Hide => ArgumentContract::Text {
                max_length: Some(16),
            },
            _ => ArgumentContract::None,
        }
    }
}

const GRACE: &str = "Grace Brewster Murray Hopper";

const PEOPLE: [(&str, &str); 3] = [
    ("ada", "Ada Lovelace"),
    ("grace", GRACE),
    ("katherine", "Katherine Coleman Goble Johnson"),
];

pub(super) fn story(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut HeaderFixtureState,
    report: &mut StoryReport,
) {
    state.reset_for(story);
    let mut p = PresentationContext::new(
        ui,
        *tokens,
        scale,
        PresentationScope::new("gallery.header"),
        SemanticUiId::new("gallery.story"),
    );
    let clicked = if story == StoryId::NavigationHeader {
        navigation(ui, &mut p, state, report)
    } else {
        observe_visible_only(ui, |ui| headings(ui, &mut p, state))
    };
    for control in clicked {
        if !control.response.clicked() {
            continue;
        }
        match control.target.action {
            HeaderAction::Pin => state.pinned = !state.pinned,
            HeaderAction::Mute => state.muted = !state.muted,
            HeaderAction::Hide => state.hidden += 1,
            _ => state.edits += 1,
        }
        report.repaint = Some("header control");
    }
    report.presented(&p.finish(ui));
}

/// Pinned, as a toggle carrying the person's ID.
fn pin(state: &HeaderFixtureState, id: &str, name: &str) -> HeaderControl<HeaderAction> {
    HeaderControl::icon(
        BoundAction::new(HeaderAction::Pin, id).with_subject(name),
        IconId::Check,
    )
    .pressed(state.pinned)
}

/// Section headings with one, two and five controls: beside a short title,
/// beside a long one that elides, and on a line of their own when they
/// would crowd the title.
fn headings(
    ui: &mut egui::Ui,
    p: &mut PresentationContext,
    state: &HeaderFixtureState,
) -> Vec<HeaderControlResponse<HeaderAction>> {
    let enabled = |_, _: &ActionArgument| Availability::Enabled;
    let gap = p.tokens().spacing.section.0;
    let mut clicked = Vec::new();
    egui::ScrollArea::vertical()
        .id_salt("heading-controls")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let sections: [(&str, &str, Vec<HeaderControl<HeaderAction>>); 3] = [
                (
                    "Morning routines",
                    "Three routines run before nine.",
                    vec![HeaderControl::icon(HeaderAction::Edit, IconId::Edit)],
                ),
                (
                    GRACE,
                    "Rear admiral and computing pioneer.",
                    vec![
                        pin(state, "grace", GRACE),
                        HeaderControl::new(HeaderAction::EditProfile, ActionButtonContent::Text),
                    ],
                ),
                (
                    "Shared shopping list for the weekend",
                    "Bread, apples and coffee.",
                    vec![
                        HeaderControl::icon(HeaderAction::Edit, IconId::Edit),
                        HeaderControl::icon(HeaderAction::Share, IconId::Link),
                        HeaderControl::icon(HeaderAction::Search, IconId::Search),
                        HeaderControl::icon(HeaderAction::Mute, IconId::Notifications)
                            .pressed(state.muted),
                        HeaderControl::icon(HeaderAction::Settings, IconId::Settings),
                    ],
                ),
            ];
            for (index, (title, body, controls)) in sections.iter().enumerate() {
                p.scoped(ui, ("section", index), |ui, p| {
                    let row = p.heading_row(ui, "heading", title, controls, enabled);
                    clicked.extend(row.controls);
                    p.reading_text(ui, "body", body, ReadingTextSpec::new(TextRole::Body));
                });
                ui.add_space(gap);
            }
        });
    clicked
}

/// The navigation recipe with the open conversation's avatar, status and
/// controls in its header.
fn navigation(
    ui: &mut egui::Ui,
    p: &mut PresentationContext,
    state: &mut HeaderFixtureState,
    report: &mut StoryReport,
) -> Vec<HeaderControlResponse<HeaderAction>> {
    let open = state
        .open
        .as_deref()
        .and_then(|id| PEOPLE.iter().find(|(person, _)| *person == id))
        .copied();
    let about = open.map(|(_, name)| format!("About {name}"));
    let spec = NavigationSpec {
        list_title: "Conversations",
        content: open.map(|(_, name)| NavigationContent {
            title: name,
            reference: None,
            detail: about.as_deref().map(|title| NavigationDetail {
                title,
                shown: state.details,
                toggle: HeaderAction::Details,
                icon: IconId::Info,
            }),
        }),
        back: HeaderAction::Back,
    };
    let options = match open {
        Some((id, name)) => NavigationOptions::new()
            .with_avatar(AvatarSpec::person(name, AvatarSize::Header))
            .with_subtitle("Last seen today at 09:41")
            .with_control(pin(state, id, name))
            .with_control(HeaderControl::icon(
                BoundAction::new(HeaderAction::Hide, id).with_subject(name),
                IconId::Remove,
            ))
            .with_control(HeaderControl::icon(HeaderAction::EditProfile, IconId::Edit)),
        None => NavigationOptions::new(),
    };
    let selected = state.open.clone();
    let mut opened = None;
    let (response, controls) = p.navigation_with(
        ui,
        "navigation",
        &spec,
        &options,
        |_, _| Availability::Enabled,
        |ui, p, pane| match pane {
            NavigationPane::List => {
                for (id, name) in PEOPLE {
                    let response = p.bound_navigation_item(
                        ui,
                        HeaderAction::Open,
                        NavigationItemSpec {
                            target: ActionTarget::application(HeaderAction::Open),
                            icon: IconId::User,
                            label: name,
                            description: "Open the conversation",
                            selected: selected.as_deref() == Some(id),
                            availability: Availability::Enabled,
                            badge: None,
                        },
                        &ActionArgument::from(id),
                    );
                    if response.clicked() {
                        opened = Some(id);
                    }
                }
            }
            NavigationPane::Content | NavigationPane::Detail | NavigationPane::Empty => {
                ui.add_space(p.tokens().spacing.section.0);
                let text = match pane {
                    NavigationPane::Content => "The hall is booked for Friday.",
                    NavigationPane::Detail => "Joined in 1952.",
                    _ => "Choose a conversation",
                };
                p.reading_text(ui, "body", text, ReadingTextSpec::new(TextRole::Body));
            }
        },
    );
    for pane in &response.panes {
        report.presented(&pane.observations);
    }
    if response
        .back
        .as_ref()
        .is_some_and(|back| back.response.clicked())
    {
        if state.details {
            state.details = false;
        } else {
            state.open = None;
        }
        report.repaint = Some("navigation back");
    }
    if response
        .toggle
        .as_ref()
        .is_some_and(|toggle| toggle.response.clicked())
    {
        state.details = !state.details;
        report.repaint = Some("details toggled");
    }
    if let Some(id) = opened {
        state.open = Some(id.into());
        report.repaint = Some("conversation opened");
    }
    controls
}

#[cfg(test)]
mod tests {
    use eframe::egui;
    use stobie_ui_egui::{
        ApplicationTheme, DensityVariant, SemanticAuditFinding, ThemeVariant, UiRole,
        audit_accesskit, audit_text_layouts,
    };

    use super::*;
    use crate::stories::probe::{StoryProbe, render};

    /// The story at a phone's width, less the Gallery surface's margins.
    fn probe(story: StoryId, scale: f32) -> stobie_ui_egui::UiSnapshot {
        let size = egui::vec2(366.0, 720.0);
        let context = egui::Context::default();
        context.enable_accesskit();
        let (snapshot, mut output) = render(
            &context,
            StoryProbe {
                story,
                tokens: DesignTokens::resolve(ThemeVariant::Light, DensityVariant::Comfortable),
                scale,
                clip: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                theme: ApplicationTheme::analytical(),
                preferences: Default::default(),
                heading: false,
            },
            size,
        );
        let findings = audit_text_layouts(&snapshot.text);
        assert!(findings.is_empty(), "{findings:#?}");
        // The probe presents the story without the Gallery's story pane,
        // its nodes' parent.
        let story = SemanticUiId::new("gallery.story");
        let findings: Vec<_> = snapshot
            .semantic_audit
            .iter()
            .filter(|finding| {
                !matches!(finding, SemanticAuditFinding::MissingParent { parent, .. }
                    if *parent == story)
            })
            .collect();
        assert!(findings.is_empty(), "{findings:?}");
        let update = output.platform_output.accesskit_update.take().unwrap();
        let mismatches = audit_accesskit(&snapshot, &update);
        assert!(mismatches.is_empty(), "{mismatches:?}");
        output.textures_delta.clear();
        snapshot
    }

    fn buttons(snapshot: &stobie_ui_egui::UiSnapshot) -> Vec<(&str, Option<bool>)> {
        snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::Button)
            .map(|node| (node.name.as_str(), node.checked))
            .collect()
    }

    #[test]
    fn every_heading_control_shows_at_a_phones_width() {
        for scale in [1.0, 1.5] {
            let snapshot = probe(StoryId::HeadingControls, scale);
            assert_eq!(
                buttons(&snapshot),
                [
                    ("Edit", None),
                    ("Pinned Grace Brewster Murray Hopper", Some(true)),
                    ("Edit profile", None),
                    ("Edit", None),
                    ("Share", None),
                    ("Search", None),
                    ("Muted", Some(false)),
                    ("Settings", None),
                ],
                "{scale}"
            );
        }
    }

    #[test]
    fn the_conversation_header_holds_its_controls_at_a_phones_width() {
        for scale in [1.0, 1.5] {
            let snapshot = probe(StoryId::NavigationHeader, scale);
            assert_eq!(
                buttons(&snapshot),
                [
                    ("Back", None),
                    ("Pinned Grace Brewster Murray Hopper", Some(true)),
                    ("Hide Grace Brewster Murray Hopper", None),
                    ("Edit profile", None),
                    ("Details", Some(false)),
                ],
                "{scale}"
            );
        }
    }
}
