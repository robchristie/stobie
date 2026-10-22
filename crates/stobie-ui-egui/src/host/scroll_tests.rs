#![cfg(test)]
//! Scrolling recipes in a hosted application: an anchored scroll area keeps
//! the reader's place when rows arrive above them, a footer keeps to the
//! foot of a short pane, and a message bubble with a Copy inside settles
//! wherever it is scrolled.

use egui::{Pos2, Vec2};
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, AnchoredScrollArea, ContentTextSpec, CopyContent, CopySpec, InspectionOperation,
    InspectionResult, InspectionSelector, MessageBubbleSpec, MessageSide, PresentationContext,
    PresentationScope, ReadingTextSpec, ReceiptState, ScrollRows, TextInteraction, TextOverflow,
    TextRole, UiRect, UiRole,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "thread")]
enum Act {
    #[action(label = "Load older", description = "Show older messages")]
    LoadOlder,
}

/// The height of the scrolled view.
const VIEW: f32 = 400.0;

/// The view's top on screen: the harness frames its page in 8 points.
const TOP: f32 = 8.0;

/// A page of older messages.
const PAGE: i64 = 20;

/// How a message reads: every third one is long enough to wrap.
fn text(id: i64) -> String {
    if id % 3 == 0 {
        format!(
            "Message {id}: the hall is booked for Friday from six until late, the key \
             is at the front desk, and the caretaker asks that the last person out \
             turns off the heaters."
        )
    } else {
        format!("Message {id}")
    }
}

/// Each message's bubble is named by its own sender, so a test can find it.
fn sender(id: i64) -> String {
    format!("Sender {id}")
}

/// A conversation in a scroll area of fixed height, newest at the foot.
struct Thread {
    /// The first and last messages shown, oldest first.
    first: i64,
    last: i64,
    /// Whether the area keeps the reader's place.
    anchored: bool,
    stick_to_bottom: bool,
    /// Whether each bubble holds a Copy and a labelled Copy below its text.
    copy_inside: bool,
    /// Lines of the footer under the messages; none presents no footer.
    footer: usize,
    /// Present the area inside an `observe_visible_only` region, as a long
    /// scrolled list is.
    visible_only: bool,
    /// A fixed offset to show, as a scroll to it would.
    offset: Option<f32>,
    performed: usize,
}

impl Default for Thread {
    fn default() -> Self {
        Self {
            first: 0,
            last: 29,
            anchored: true,
            stick_to_bottom: true,
            copy_inside: false,
            footer: 0,
            visible_only: true,
            offset: None,
            performed: 0,
        }
    }
}

impl Thread {
    fn messages(&self, ui: &mut Ui, frame: &mut Frame<'_, Act, ()>) {
        let mut area = egui::ScrollArea::vertical()
            .max_height(VIEW)
            .min_scrolled_height(VIEW)
            .stick_to_bottom(self.stick_to_bottom)
            .auto_shrink(false);
        if let Some(offset) = self.offset {
            area = area.vertical_scroll_offset(offset);
        }
        let mut content = |ui: &mut Ui, mut rows: Option<&mut ScrollRows>| {
            let mut p =
                frame.presentation(ui, PresentationScope::new("thread"), SemanticUiId::root());
            frame.action(ui, &mut p, Act::LoadOlder, ActionEmphasis::Normal);
            for id in self.first..=self.last {
                match rows.as_deref_mut() {
                    Some(rows) => rows.row(ui, id, |ui| self.message(ui, &mut p, id)),
                    None => self.message(ui, &mut p, id),
                }
            }
            if self.footer > 0 {
                match rows.as_deref_mut() {
                    Some(rows) => rows.footer(ui, |ui| self.footer(ui, &mut p)),
                    None => self.footer(ui, &mut p),
                }
            }
            p.finish(ui);
        };
        if self.anchored {
            AnchoredScrollArea::new("thread", area).show(ui, |ui, rows| content(ui, Some(rows)));
        } else {
            area.show(ui, |ui| content(ui, None));
        }
    }

    fn message(&self, ui: &mut Ui, p: &mut PresentationContext, id: i64) {
        let side = if id % 2 == 0 {
            MessageSide::Own
        } else {
            MessageSide::Other
        };
        let body = text(id);
        let sender = sender(id);
        p.message_bubble(
            ui,
            ("message", id),
            MessageBubbleSpec {
                side,
                sender: &sender,
            },
            |ui, p| {
                p.reading_text(ui, ("text", id), &body, ReadingTextSpec::default());
                if self.copy_inside {
                    ui.horizontal(|ui| {
                        p.copy(
                            ui,
                            ("copy", id),
                            &CopySpec {
                                content: CopyContent::Icon,
                                ..CopySpec::new(&body, "Copy message", "Copied", "Couldn't copy")
                            },
                        );
                        p.copy(
                            ui,
                            ("copy-labelled", id),
                            &CopySpec::new(&body, "Copy text", "Copied", "Couldn't copy"),
                        );
                    });
                }
            },
        );
    }

    fn footer(&self, ui: &mut Ui, p: &mut PresentationContext) {
        for line in 0..self.footer {
            p.content(
                ui,
                ("footer", line),
                &footer_line(line),
                ContentTextSpec {
                    role: TextRole::Secondary,
                    overflow: TextOverflow::Ellipsis,
                    max_lines: 1,
                    interaction: TextInteraction::Inert,
                },
            );
        }
    }
}

fn footer_line(line: usize) -> String {
    format!("Signed in as Ada, line {line}")
}

impl Application for Thread {
    type Action = Act;
    type Intent = ();
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Thread")
    }

    fn availability(&self, _target: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn ui(&self, ui: &mut Ui, _: &mut (), frame: &mut Frame<'_, Act, ()>) {
        if self.visible_only {
            crate::observe_visible_only(ui, |ui| self.messages(ui, frame));
        } else {
            self.messages(ui, frame);
        }
    }

    fn apply(&mut self, _intent: (), _: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    fn perform(&mut self, _target: ActionTarget<Act>, _: &mut ()) -> Result<bool, String> {
        self.first -= PAGE;
        self.performed += 1;
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Thread>>,
}

fn harness(thread: Thread) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(800.0, 700.0))
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
    harness.state_mut().host = Some(AppHost::new(&context, thread));
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Thread> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<Thread> {
    harness.state_mut().host.as_mut().unwrap()
}

fn accesskit_update(harness: &Harness<'_, Probe>) -> egui::accesskit::TreeUpdate {
    fn collect(
        node: egui_kittest::kittest::AccessKitNode<'_>,
        update: &mut egui::accesskit::TreeUpdate,
    ) {
        update.nodes.push((node.locate().0, node.data().clone()));
        for child in node.children() {
            collect(child, update);
        }
    }
    let root = harness.kittest_state().root();
    let mut update = egui::accesskit::TreeUpdate {
        nodes: Vec::new(),
        tree: Some(egui::accesskit::Tree::new(root.locate().0)),
        tree_id: egui::accesskit::TreeId::ROOT,
        focus: root.locate().0,
    };
    collect(root, &mut update);
    update
}

/// The pass audits clean and leaves no repaint waiting.
fn assert_clean(harness: &Harness<'_, Probe>) {
    let snapshot = host(harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    assert!(snapshot.text_audit.is_empty(), "{:?}", snapshot.text_audit);
    let mismatches = crate::audit_accesskit(snapshot, &accesskit_update(harness));
    assert!(mismatches.is_empty(), "{mismatches:?}");
    let reasons = host(harness).repaint_reasons();
    assert!(reasons.is_empty(), "{reasons:?}");
}

/// Change the application, as an arriving page would, and ask for a pass.
fn change(harness: &mut Harness<'_, Probe>, change: impl FnOnce(&mut Thread)) {
    change(host_mut(harness).app_mut());
    host_mut(harness).request_repaint("test");
}

fn bubble(harness: &Harness<'_, Probe>, id: i64) -> Option<UiRect> {
    let name = sender(id);
    host(harness)
        .ui_snapshot()
        .nodes
        .iter()
        .find(|node| node.role == UiRole::Section && node.name == name)
        .map(|node| node.rect)
}

fn node(harness: &Harness<'_, Probe>, name: &str) -> Option<UiRect> {
    host(harness)
        .ui_snapshot()
        .nodes
        .iter()
        .find(|node| node.name == name)
        .map(|node| node.rect)
}

/// Where a line of the footer was laid out, found by its words, or `None`
/// when it is wholly out of view, as such text is not observed.
fn footer_rect(harness: &Harness<'_, Probe>, line: usize) -> Option<UiRect> {
    let expected = footer_line(line);
    let rect = host(harness)
        .ui_snapshot()
        .text
        .iter()
        .find(|text| match &text.content {
            Some(crate::TextContent::Excerpt { text, .. }) => *text == expected,
            _ => false,
        })?
        .allocated_rect;
    Some(UiRect {
        min_x: rect.min_x,
        min_y: rect.min_y,
        max_x: rect.max_x,
        max_y: rect.max_y,
    })
}

/// The first message whose bubble starts in view.
fn first_in_view(harness: &Harness<'_, Probe>) -> i64 {
    let app = host(harness).app();
    (app.first..=app.last)
        .find(|id| bubble(harness, *id).is_some_and(|rect| rect.min_y >= TOP))
        .expect("a message in view")
}

/// Turn the mouse wheel over the view: positive scrolls up, towards older
/// messages.
fn wheel(harness: &mut Harness<'_, Probe>, points: f32) {
    harness.hover_at(Pos2::new(400.0, TOP + VIEW / 2.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, points),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    // egui eases a wheel's scroll over several frames of its own.
    harness.run_steps(16);
    harness.run();
}

#[test]
fn a_reader_scrolled_up_keeps_their_place_when_older_rows_arrive() {
    let mut harness = harness(Thread::default());
    wheel(&mut harness, 300.0);
    let reading = first_in_view(&harness);
    let before = bubble(&harness, reading).unwrap();
    change(&mut harness, |thread| thread.first -= PAGE);
    // The first frame that shows the older rows already keeps the place:
    // the area lays the pass out again rather than showing the jump.
    harness.step();
    assert_eq!(bubble(&harness, reading), Some(before));
    harness.run();
    assert_eq!(bubble(&harness, reading), Some(before));
    assert_clean(&harness);
    // A second page keeps it too, and the reader can still scroll up into
    // the rows that arrived.
    change(&mut harness, |thread| thread.first -= PAGE);
    harness.run();
    assert_eq!(bubble(&harness, reading), Some(before));
    wheel(&mut harness, 200.0);
    let after = bubble(&harness, reading).unwrap();
    assert!(
        after.min_y > before.min_y + 100.0,
        "{before:?} then {after:?}"
    );
}

#[test]
fn without_anchoring_older_rows_push_the_reader_down() {
    // The behaviour the anchored area corrects: egui keeps the offset from
    // the top, so the row the reader was at moves down and out of view.
    let mut harness = harness(Thread {
        // Positions out of view are read, so every node is observed.
        visible_only: false,
        anchored: false,
        ..Thread::default()
    });
    wheel(&mut harness, 300.0);
    let reading = first_in_view(&harness);
    let before = bubble(&harness, reading).unwrap();
    change(&mut harness, |thread| thread.first -= PAGE);
    harness.run();
    let after = bubble(&harness, reading).unwrap();
    assert!(after.min_y > TOP + VIEW, "{before:?} then {after:?}");
}

#[test]
fn a_reader_at_the_end_with_stick_to_bottom_sees_new_rows_and_keeps_the_end() {
    let mut harness = harness(Thread::default());
    let newest = bubble(&harness, 29).unwrap();
    assert!(newest.max_y <= TOP + VIEW + 0.5, "{newest:?}");
    // A new message at the end scrolls into view, as `stick_to_bottom` does.
    change(&mut harness, |thread| thread.last += 1);
    harness.run();
    let end = bubble(&harness, 30).unwrap();
    assert!(
        (end.max_y - newest.max_y).abs() < 1.0,
        "{newest:?} then {end:?}"
    );
    // Older messages above leave the end in view, from the first frame.
    change(&mut harness, |thread| thread.first -= PAGE);
    harness.step();
    assert_eq!(bubble(&harness, 30), Some(end));
    harness.run();
    assert_eq!(bubble(&harness, 30), Some(end));
    assert_clean(&harness);
}

#[test]
fn a_reader_at_the_end_without_stick_to_bottom_keeps_the_rows_in_view() {
    let mut harness = harness(Thread {
        // Positions out of view are read, so every node is observed.
        visible_only: false,
        stick_to_bottom: false,
        ..Thread::default()
    });
    wheel(&mut harness, -20_000.0);
    let end = bubble(&harness, 29).unwrap();
    assert!(end.max_y <= TOP + VIEW + 0.5, "{end:?}");
    change(&mut harness, |thread| thread.first -= PAGE);
    harness.step();
    assert_eq!(bubble(&harness, 29), Some(end));
    harness.run();
    assert_eq!(bubble(&harness, 29), Some(end));
    // A new message arrives below the view, where the reader can scroll to.
    change(&mut harness, |thread| thread.last += 1);
    harness.run();
    assert_eq!(bubble(&harness, 29), Some(end));
    let arrived = bubble(&harness, 30).unwrap();
    assert!(arrived.min_y >= TOP + VIEW - 1.0, "{arrived:?}");
}

#[test]
fn an_out_of_view_load_older_control_is_invoked_through_its_action() {
    let mut harness = harness(Thread::default());
    host_mut(&mut harness).enable_inspection(Inspection::new("thread-test"));
    harness.run();
    // Scrolled to the newest message, the control at the top is out of
    // view, so a visible-only region does not observe it.
    assert_eq!(node(&harness, "Load older"), None);
    let inspection = host(&harness).inspection().unwrap();
    let reply = inspection.handle(InspectionRequest {
        version: 1,
        request_id: "discover".into(),
        instance: Some(inspection.instance()),
        operation: InspectionOperation::Discover {
            selector: InspectionSelector {
                capability: Some(Act::LoadOlder.stable_id().into()),
                ..Default::default()
            },
            limit: 8,
            cursor: None,
        },
    });
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    assert_eq!(capabilities.len(), 1, "{capabilities:?}");
    let reply = inspection.handle(InspectionRequest {
        version: 1,
        request_id: "older".into(),
        instance: Some(inspection.instance()),
        operation: InspectionOperation::Invoke {
            selector: InspectionSelector {
                id: Some(capabilities[0].id.clone()),
                ..Default::default()
            },
            expected: capabilities[0].target.clone(),
            arguments: serde_json::Value::Null,
        },
    });
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("invocation failed: {:?}", reply.error);
    };
    assert_eq!(receipt.state, ReceiptState::Queued, "{receipt:?}");
    let end = bubble(&harness, 29).unwrap();
    harness.run();
    assert_eq!(host(&harness).app().performed, 1);
    assert_eq!(host(&harness).app().first, -PAGE);
    assert_eq!(bubble(&harness, 29), Some(end));
    assert_clean(&harness);
}

/// The footer's last line ends at the view's foot.
fn at_foot(rect: UiRect) -> bool {
    (rect.max_y - (TOP + VIEW)).abs() <= 1.0
}

#[test]
fn a_footer_sits_at_the_foot_of_a_short_pane_and_after_a_long_list() {
    let mut harness = harness(Thread {
        // Positions out of view are read, so every node is observed.
        visible_only: false,
        last: 1,
        footer: 1,
        ..Thread::default()
    });
    let foot = footer_rect(&harness, 0).unwrap();
    assert!(at_foot(foot), "{foot:?}");
    // It needs no repaint from the application or the host to stay there.
    assert!(host(&harness).repaint_reasons().is_empty());
    // A taller footer still ends at the foot, from its first frame.
    change(&mut harness, |thread| thread.footer = 2);
    harness.step();
    let second = footer_rect(&harness, 1).unwrap();
    assert!(at_foot(second), "{second:?}");
    harness.run();
    assert!(at_foot(footer_rect(&harness, 1).unwrap()));
    assert!(host(&harness).repaint_reasons().is_empty());
    // A long list puts it after the last row, out of view until the reader
    // scrolls to the end.
    change(&mut harness, |thread| {
        thread.footer = 1;
        thread.stick_to_bottom = false;
        thread.last = 40;
    });
    harness.run();
    // The last row is below the view, and the footer after it is wholly out
    // of view, so its text is not observed.
    let last = bubble(&harness, 40).unwrap();
    assert!(last.min_y > TOP + VIEW, "{last:?}");
    assert_eq!(footer_rect(&harness, 0), None);
    wheel(&mut harness, -20_000.0);
    let last = bubble(&harness, 40).unwrap();
    let foot = footer_rect(&harness, 0).unwrap();
    assert!(foot.min_y >= last.max_y, "{last:?} then {foot:?}");
    assert!(foot.min_y - last.max_y < 40.0, "{last:?} then {foot:?}");
    assert!(at_foot(foot), "{foot:?}");
    // A short list again puts it back at the foot.
    change(&mut harness, |thread| thread.last = 1);
    harness.run();
    let foot = footer_rect(&harness, 0).unwrap();
    assert!(at_foot(foot), "{foot:?}");
    assert_clean(&harness);
}

#[test]
fn a_bubble_whose_text_is_scrolled_away_keeps_its_width_and_settles() {
    // A long message whose text is above the view and whose Copy row is at
    // the view's top. Text out of view skips its paint, so a bubble that
    // measured what painted would narrow to its Copy row, wrap its text
    // into view, widen again and never settle.
    let fresh = harness(Thread {
        // Positions out of view are read, so every node is observed.
        visible_only: false,
        first: 3,
        copy_inside: true,
        stick_to_bottom: false,
        offset: Some(0.0),
        ..Thread::default()
    });
    let whole = bubble(&fresh, 3).unwrap();
    let copy = node(&fresh, "Copy message").unwrap();
    assert!(whole.max_x - whole.min_x > 300.0, "{whole:?}");
    // Show the same conversation from its start scrolled to just above that
    // Copy row, so the bubble is first presented only partly in view.
    let offset = copy.min_y - TOP - 2.0;
    let mut harness = harness(Thread {
        // Positions out of view are read, so every node is observed.
        visible_only: false,
        first: 3,
        copy_inside: true,
        stick_to_bottom: false,
        offset: Some(offset),
        ..Thread::default()
    });
    let partly = bubble(&harness, 3).unwrap();
    assert!(partly.min_y < TOP && partly.max_y > TOP, "{partly:?}");
    assert!(
        partly.max_x - partly.min_x >= whole.max_x - whole.min_x - 1.0,
        "{whole:?} then {partly:?}"
    );
    // Hovering and focusing its Copy controls settles and keeps the width.
    let copy = node(&harness, "Copy message").unwrap();
    harness.hover_at(Pos2::new(
        (copy.min_x + copy.max_x) / 2.0,
        (copy.min_y + copy.max_y) / 2.0,
    ));
    harness.run();
    for _ in 0..3 {
        harness.key_press(egui::Key::Tab);
        harness.run();
    }
    assert_eq!(bubble(&harness, 3), Some(partly));
    // Scrolled wholly into view, it fits its content again and settles.
    change(&mut harness, |thread| thread.offset = Some(0.0));
    harness.run();
    assert_eq!(bubble(&harness, 3), Some(whole));
    assert!(host(&harness).repaint_reasons().is_empty());
}

#[test]
fn a_long_conversation_with_copy_inside_its_bubbles_settles() {
    let mut harness = harness(Thread {
        copy_inside: true,
        last: 59,
        ..Thread::default()
    });
    // Each change settles within the harness's default steps: hovering a
    // Copy, focusing one, reading back through history and loading pages.
    let copy = host(&harness)
        .ui_snapshot()
        .nodes
        .iter()
        .rev()
        .find(|node| node.name == "Copy message")
        .map(|node| node.rect)
        .unwrap();
    harness.hover_at(Pos2::new(
        (copy.min_x + copy.max_x) / 2.0,
        (copy.min_y + copy.max_y) / 2.0,
    ));
    harness.run();
    harness.key_press(egui::Key::Tab);
    harness.run();
    for _ in 0..3 {
        wheel(&mut harness, 250.0);
        let reading = first_in_view(&harness);
        let before = bubble(&harness, reading).unwrap();
        change(&mut harness, |thread| thread.first -= PAGE);
        harness.run();
        assert_eq!(bubble(&harness, reading), Some(before));
    }
    assert_clean(&harness);
}
