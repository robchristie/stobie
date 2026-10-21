#![cfg(test)]
//! Headings with controls through the host, at a phone's width: a heading
//! row keeps its controls whole and in sight beside a long title, moving them
//! to a line of their own when they would crowd it; the navigation recipe's
//! header holds an avatar, a subtitle and controls of the content's pane;
//! and a control pushed past the end of its row by a heading is reported
//! rather than dropped.

use egui::Vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde::Serialize;

use super::*;
use crate::{
    Action, AvatarSize, AvatarSpec, BoundAction, HeaderControl, NavigationContent,
    NavigationDetail, NavigationOptions, NavigationPane, NavigationSpec, PresentationScope,
    ReadingTextSpec, SemanticAuditFinding,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "profile")]
enum Act {
    #[action(
        label = "Mute",
        description = "Mute or unmute this conversation",
        argument = "text",
        max_length = 8
    )]
    Mute,
    #[action(label = "Edit", description = "Edit this profile")]
    Edit,
    #[action(label = "Hide", description = "Hide this conversation")]
    Hide,
    #[action(label = "Search", description = "Search this conversation")]
    Search,
    #[action(label = "Share", description = "Share a link to this conversation")]
    Share,
    #[action(
        label = "Settings",
        description = "Change this conversation's settings"
    )]
    Settings,
    #[action(label = "Copy", description = "Copy this conversation")]
    Copy,
    #[action(label = "Save", description = "Save this conversation")]
    Save,
    #[action(label = "Back", description = "Return to the previous level")]
    Back,
    #[action(label = "Details", description = "Show or hide this person's details")]
    Details,
}

/// The header controls after Mute, in the order they are added.
const MORE: [(Act, IconId); 7] = [
    (Act::Edit, IconId::Edit),
    (Act::Hide, IconId::Remove),
    (Act::Search, IconId::Search),
    (Act::Share, IconId::Link),
    (Act::Settings, IconId::Settings),
    (Act::Copy, IconId::Copy),
    (Act::Save, IconId::Save),
];

const LONG: &str = "Grace Brewster Murray Hopper, rear admiral and computing pioneer";

#[derive(Clone, Copy, Debug)]
enum Show {
    /// A heading and then a control in `ui.horizontal`.
    Squeezed,
    /// A heading row with Mute and this many more controls.
    Row(usize),
    /// The navigation recipe with Mute and this many more header controls.
    Navigation(usize),
}

struct Profile {
    show: Show,
    title: &'static str,
    muted: bool,
    inset: bool,
    performed: Vec<(Act, ActionArgument)>,
}

impl Profile {
    fn new(show: Show, title: &'static str) -> Self {
        Self {
            show,
            title,
            muted: false,
            inset: true,
            performed: Vec::new(),
        }
    }

    fn controls(&self, more: usize) -> Vec<HeaderControl<Act>> {
        let mute = BoundAction::new(Act::Mute, "p1").with_subject("Grace");
        std::iter::once(HeaderControl::icon(mute, IconId::Notifications).pressed(self.muted))
            .chain(
                MORE.iter()
                    .take(more)
                    .map(|(action, icon)| HeaderControl::icon(*action, *icon)),
            )
            .collect()
    }
}

impl Application for Profile {
    type Action = Act;
    type Intent = ();
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Profile")
    }

    fn availability(&self, _target: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn availability_with(
        &self,
        _target: ActionTarget<Act>,
        _argument: &ActionArgument,
    ) -> Availability {
        Availability::Enabled
    }

    fn ui(&self, ui: &mut Ui, _view: &mut (), frame: &mut Frame<'_, Act, ()>) {
        let mut p = frame.presentation(ui, PresentationScope::new("profile"), SemanticUiId::root());
        match self.show {
            Show::Squeezed => {
                // In the first of two columns, which clips what goes past
                // its edge.
                let mut column = ui.available_rect_before_wrap();
                column.max.x = column.center().x;
                ui.scope_builder(egui::UiBuilder::new().max_rect(column), |ui| {
                    ui.set_clip_rect(column);
                    ui.horizontal(|ui| {
                        p.heading(ui, "title", self.title);
                        frame.icon_action(
                            ui,
                            &mut p,
                            Act::Edit,
                            IconId::Edit,
                            ActionEmphasis::QuietBorderless,
                        );
                    });
                });
            }
            Show::Row(more) => {
                frame.heading_row(ui, &mut p, "title", self.title, &self.controls(more));
                p.reading_text(ui, "body", "Joined in 1952", ReadingTextSpec::default());
            }
            Show::Navigation(more) => {
                let spec = NavigationSpec {
                    list_title: "People",
                    content: Some(NavigationContent {
                        title: self.title,
                        reference: None,
                        detail: Some(NavigationDetail {
                            title: "About Grace",
                            shown: false,
                            toggle: Act::Details,
                            icon: IconId::Info,
                        }),
                    }),
                    back: Act::Back,
                };
                let options = self
                    .controls(more)
                    .into_iter()
                    .fold(NavigationOptions::new(), NavigationOptions::with_control)
                    .with_avatar(AvatarSpec::person("Grace", AvatarSize::Header))
                    .with_subtitle("Online")
                    .with_body_inset(NavigationPane::Content, self.inset);
                frame.navigation_with(ui, &mut p, "navigation", &spec, &options, |ui, p, _, _| {
                    p.reading_text(
                        ui,
                        "messages",
                        "Messages with Grace",
                        ReadingTextSpec::default(),
                    );
                });
            }
        }
    }

    fn apply(&mut self, _intent: (), _view: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    fn perform(&mut self, target: ActionTarget<Act>, _view: &mut ()) -> Result<bool, String> {
        self.performed.push((target.action, ActionArgument::None));
        Ok(true)
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<Act>,
        argument: ActionArgument,
        _view: &mut (),
    ) -> Result<bool, String> {
        if target.action == Act::Mute {
            self.muted = !self.muted;
        }
        self.performed.push((target.action, argument));
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Profile>>,
}

const PHONE: f32 = 390.0;

fn harness(app: Profile) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(PHONE, 640.0))
        .build_ui_state(
            |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    host.present(ui);
                }
            },
            Probe::default(),
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    harness.state_mut().host = Some(AppHost::new(&context, app));
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Profile> {
    harness.state().host.as_ref().unwrap()
}

/// The audits are clean and every Stobie node is in the AccessKit tree.
fn assert_clean(harness: &Harness<'_, Probe>) {
    let snapshot = host(harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    assert!(snapshot.text_audit.is_empty(), "{:?}", snapshot.text_audit);
}

/// The header controls' rectangles, in the order given, and the screen's.
fn controls(harness: &Harness<'_, Probe>, more: usize) -> Vec<crate::UiRect> {
    let snapshot = host(harness).ui_snapshot();
    std::iter::once(Act::Mute)
        .chain(MORE.iter().take(more).map(|(action, _)| *action))
        .map(|action| {
            snapshot
                .by_action(action)
                .next()
                .unwrap_or_else(|| panic!("{action:?} is not presented"))
                .rect
        })
        .collect()
}

fn heading(harness: &Harness<'_, Probe>, title: &str) -> egui::Rect {
    harness
        .get_by_role_and_label(egui::accesskit::Role::Heading, title)
        .rect()
}

fn on_screen(rect: crate::UiRect) -> bool {
    rect.min_x >= 0.0 && rect.max_x <= PHONE && rect.is_positive()
}

#[test]
fn a_control_after_a_heading_in_a_horizontal_row_is_reported_not_dropped() {
    let harness = harness(Profile::new(Show::Squeezed, LONG));
    // The heading takes the row's whole width, which pushes Edit past the
    // column's edge, out of sight; it keeps its node and the audit names it.
    let snapshot = host(&harness).ui_snapshot();
    let edit = snapshot
        .by_action(Act::Edit)
        .next()
        .expect("Edit keeps its node");
    assert!(
        snapshot
            .semantic_audit
            .contains(&SemanticAuditFinding::InvalidRect {
                id: edit.id.clone()
            }),
        "{:?}",
        snapshot.semantic_audit
    );
}

#[test]
fn a_heading_row_keeps_one_or_two_controls_whole_beside_a_long_title() {
    for more in [0, 1] {
        let mut harness = harness(Profile::new(Show::Row(more), LONG));
        harness.run();
        assert_clean(&harness);
        let response = controls(&harness, more);
        let title = heading(&harness, LONG);
        for control in &response {
            assert!(on_screen(*control), "{more}: {control:?}");
            // Whole: at least a minimum hit target.
            assert!(control.max_x - control.min_x >= 32.0, "{control:?}");
            // On the heading's line, after it.
            assert!(control.min_x >= title.max.x, "{control:?} {title:?}");
            assert!(control.min_y < title.max.y && control.max_y > title.min.y);
        }
        // AccessKit holds each control too.
        harness.get_by_label("Mute Grace");
        if more == 1 {
            harness.get_by_label("Edit");
        }
        // The heading elides in the room left (the clean text audit says
        // it declares that), keeping its whole name.
        assert!(title.min.x >= 0.0);
    }
}

#[test]
fn a_heading_row_gives_crowding_controls_a_line_of_their_own() {
    let mut harness = harness(Profile::new(Show::Row(7), LONG));
    harness.run();
    assert_clean(&harness);
    let title = heading(&harness, LONG);
    let response = controls(&harness, 7);
    for control in &response {
        assert!(on_screen(*control), "{control:?}");
        assert!(control.min_y >= title.max.y, "{control:?} {title:?}");
    }
    // In the order given: along a line, or on to the next when eight touch
    // targets overfill a phone's line; each line is end-aligned.
    for pair in response.windows(2) {
        let along = pair[0].max_x <= pair[1].min_x && pair[0].min_y == pair[1].min_y;
        assert!(along || pair[1].min_y >= pair[0].max_y, "{pair:?}");
        if !along {
            assert!((pair[0].max_x - response[response.len() - 1].max_x).abs() < 0.5);
        }
    }
    assert!(response[0].min_y < response[response.len() - 1].min_y);
    // What follows the row sits below the controls.
    let body = host(&harness)
        .ui_snapshot()
        .text
        .iter()
        .find(|text| {
            matches!(&text.content, Some(crate::TextContent::Excerpt { text, .. })
                if text.starts_with("Joined"))
        })
        .unwrap()
        .allocated_rect;
    assert!(
        response.iter().all(|control| body.min_y >= control.max_y),
        "{body:?}"
    );
}

#[test]
fn a_heading_rows_controls_trigger_their_actions_with_their_arguments() {
    let mut harness = harness(Profile::new(Show::Row(1), "Grace"));
    harness.get_by_label("Mute Grace").click();
    harness.run();
    harness.get_by_label("Edit").click();
    harness.run();
    assert_eq!(
        host(&harness).app().performed,
        [
            (Act::Mute, ActionArgument::Text("p1".into())),
            (Act::Edit, ActionArgument::None),
        ]
    );
    // Mute is a toggle whose pressed state the application owns.
    let mute = host(&harness)
        .ui_snapshot()
        .by_action(Act::Mute)
        .next()
        .unwrap();
    assert_eq!(mute.checked, Some(true));
}

/// The content pane's rectangle and semantic ID.
fn content_pane(harness: &Harness<'_, Probe>, title: &str) -> (crate::UiRect, SemanticUiId) {
    let snapshot = host(harness).ui_snapshot();
    let pane = snapshot
        .by_role(UiRole::Pane)
        .find(|node| node.name == title)
        .unwrap();
    (pane.rect, pane.id.clone())
}

#[test]
fn the_content_header_holds_back_an_avatar_a_subtitle_and_three_controls_on_one_line() {
    let mut harness = harness(Profile::new(Show::Navigation(2), "Grace"));
    harness.run();
    assert_clean(&harness);
    let snapshot = host(&harness).ui_snapshot();
    let (pane, id) = content_pane(&harness, "Grace");
    let back = snapshot.by_action(Act::Back).next().unwrap().rect;
    let toggle = snapshot.by_action(Act::Details).next().unwrap();
    let response = controls(&harness, 2);
    // Back, then the controls in order, then the toggle, all centred on one
    // line within the pane.
    let centre = (back.min_y + back.max_y) / 2.0;
    let mut previous = back.max_x;
    for control in response.iter().chain([&toggle.rect]) {
        assert!(control.min_x >= previous, "{control:?}");
        assert!(((control.min_y + control.max_y) / 2.0 - centre).abs() < 0.01);
        assert!(pane.contains(*control, 0.0), "{control:?}");
        previous = control.max_x;
    }
    // The title and the subtitle sit between Back and the first control.
    let title = heading(&harness, "Grace");
    assert!(title.min.x > back.max_x && title.max.x <= response[0].min_x);
    let subtitle = harness.get_by_label("Online").rect();
    assert!(
        subtitle.min.y >= title.max.y - 0.5,
        "{subtitle:?} {title:?}"
    );
    assert!(subtitle.max.y <= back.max_y + 8.0, "{subtitle:?} {back:?}");
    // The controls belong to the content's pane, here and in AccessKit.
    for action in [Act::Mute, Act::Edit, Act::Hide] {
        let node = snapshot.by_action(action).next().unwrap();
        assert_eq!(node.parent.as_ref(), Some(&id), "{action:?}");
    }
    assert_eq!(toggle.parent.as_ref(), Some(&id));
    let region = harness.get_by_role_and_label(egui::accesskit::Role::Region, "Grace");
    region.get_by_label("Mute Grace");
    region.get_by_label("Edit");
    region.get_by_label("Hide");
    // And they trigger their actions, with their arguments.
    harness.get_by_label("Mute Grace").click();
    harness.run();
    harness.get_by_label("Hide").click();
    harness.run();
    assert_eq!(
        host(&harness).app().performed,
        [
            (Act::Mute, ActionArgument::Text("p1".into())),
            (Act::Hide, ActionArgument::None),
        ]
    );
}

#[test]
fn a_long_title_elides_and_crowding_controls_take_a_second_header_line() {
    let mut harness = harness(Profile::new(Show::Navigation(5), LONG));
    harness.run();
    assert_clean(&harness);
    let snapshot = host(&harness).ui_snapshot();
    let (pane, _) = content_pane(&harness, LONG);
    let back = snapshot.by_action(Act::Back).next().unwrap().rect;
    let toggle = snapshot.by_action(Act::Details).next().unwrap().rect;
    let response = controls(&harness, 5);
    // Every control shows, whole, on a line below Back, inside the pane.
    for control in response.iter().chain([&toggle]) {
        assert!(pane.contains(*control, 0.0), "{control:?}");
        assert!(control.min_y >= back.max_y, "{control:?}");
        assert!(control.max_x - control.min_x >= 32.0, "{control:?}");
    }
    // The title elides on the first line, which it now has to itself.
    let title = heading(&harness, LONG);
    assert!(title.max.x <= pane.max_x && title.max.y <= back.max_y + 8.0);
    // The body starts below the header's second line.
    let messages = snapshot
        .text
        .iter()
        .find(|text| {
            matches!(&text.content, Some(crate::TextContent::Excerpt { text, .. })
                if text.starts_with("Messages"))
        })
        .unwrap()
        .allocated_rect;
    assert!(messages.min_y >= toggle.max_y, "{messages:?}");
}

#[test]
fn a_content_body_inset_can_be_turned_off() {
    for inset in [true, false] {
        let mut app = Profile::new(Show::Navigation(0), "Grace");
        app.inset = inset;
        let mut harness = harness(app);
        harness.run();
        assert_clean(&harness);
        let (pane, _) = content_pane(&harness, "Grace");
        let messages = host(&harness)
            .ui_snapshot()
            .text
            .iter()
            .find(|text| {
                matches!(&text.content, Some(crate::TextContent::Excerpt { text, .. })
                    if text.starts_with("Messages"))
            })
            .unwrap()
            .allocated_rect;
        let gap = messages.min_x - pane.min_x;
        if inset {
            assert!(gap > 0.0, "{gap}");
        } else {
            assert!(gap.abs() < 0.01, "{gap}");
        }
    }
}
