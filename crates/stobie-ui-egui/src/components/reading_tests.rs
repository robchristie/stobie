//! Reading text and links: wrapping at phone and desktop widths, unbounded
//! line counts, measurement, observation, selection and link activation.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use egui::{Color32, Modifiers, Rect};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

use super::reading::contrast_ratio;
use crate::{
    AccessKitMismatch, ApplicationTheme, ContentTextSpec, DensityVariant, DesignTokens, LinkSpec,
    LinkTarget, PresentationContext, PresentationScope, ReadingTextSpec, SemanticUiId,
    TextAuditFinding, TextComponentKind, TextInteraction, TextLayoutError, TextLayoutObservation,
    TextOverflow, TextRole, TextRoleStyles, TextSpec, ThemeVariant, TypographyProfile, UiNode,
    UiRole, UiSnapshot, audit_accesskit, audit_text_layouts, link_colour, measure_text,
    reading_text, text_audit_coverage,
};

/// A reply longer than eight lines at every width, with a paragraph break and
/// a long unbroken token.
const REPLY: &str = "Here is what changed overnight. The evaluation suite now covers \
    reasoning, coding and retrieval tasks, and the new runs finished without errors. \
    Three papers stand out: one on efficient on-device inference, one on reinforcement \
    learning after pre-training, and one on interpretability of attention heads.\n\n\
    The longest identifier in the logs was \
    evaluation-run-2026-10-19-reasoning-coding-retrieval-final, which wraps rather than \
    running past the edge. Each item below links to its source, and every source opens \
    in a new tab so this conversation stays where it is. Ask me to expand any of them, \
    or to compare them with last week's digest.";

const PHONE: f32 = 343.0;
const DESKTOP: f32 = 960.0;
const VARIANTS: [ThemeVariant; 4] = [
    ThemeVariant::Light,
    ThemeVariant::Dark,
    ThemeVariant::LightHighContrast,
    ThemeVariant::DarkHighContrast,
];

fn context() -> egui::Context {
    let context = egui::Context::default();
    crate::install_typography_fonts(&context);
    context.enable_accesskit();
    context
}

fn tokens() -> DesignTokens {
    DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable)
}

fn screen(width: f32, height: f32) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, height),
        )),
        ..Default::default()
    }
}

/// Present `text` once at `width` and return its observation and response rect.
fn present_reading(
    text: &str,
    width: f32,
    scale: f32,
) -> (TextLayoutObservation, Rect, egui::accesskit::TreeUpdate) {
    let context = context();
    let mut observed = None;
    let mut output = context.run_ui(screen(width, 4_000.0), |ui| {
        let mut layouts = Vec::new();
        let response = reading_text(
            ui,
            PresentationScope::new("reading").instance("reply"),
            text,
            ReadingTextSpec::default(),
            &tokens(),
            scale,
            &mut layouts,
        );
        assert_eq!(layouts.len(), 1);
        observed = Some((layouts.remove(0), response.rect));
    });
    output.textures_delta.clear();
    let (observation, rect) = observed.unwrap();
    (
        observation,
        rect,
        output.platform_output.accesskit_update.take().unwrap(),
    )
}

#[test]
fn reading_text_wraps_past_eight_lines_at_phone_and_desktop_widths() {
    let mut counts = Vec::new();
    for width in [PHONE, DESKTOP] {
        for scale in [1.0, 1.25, 1.5] {
            let (observation, rect, update) = present_reading(REPLY, width, scale);
            assert!(
                audit_text_layouts(std::slice::from_ref(&observation)).is_empty(),
                "{:#?}",
                audit_text_layouts(std::slice::from_ref(&observation))
            );
            assert_eq!(observation.overflow, TextOverflow::WrapUnbounded);
            assert_eq!(observation.declared_max_lines, 0);
            assert_eq!(
                observation.component_id.kind,
                TextComponentKind::ReadingText
            );
            assert_eq!(observation.interaction, TextInteraction::Selectable);
            assert!(!observation.truncated);
            // Wrapped to the width, not clipped or elided.
            let painted = observation.painted_rect;
            assert!(painted.max_x - painted.min_x <= width + 0.5);
            assert!((rect.width() - width).abs() < 0.5);
            // The allocation is the measured height: every line is reserved.
            let line_height = TextRole::Body.style(&tokens(), scale).line_height;
            let height = painted.max_y - painted.min_y;
            assert!((rect.height() - height).abs() < 0.5);
            // egui rounds each row to whole points.
            let lines = observation.line_count as f32;
            assert!(
                (height - line_height.round() * lines).abs() < 1.0,
                "height {height}, line height {line_height}, lines {}",
                observation.line_count
            );
            if width == PHONE {
                assert!(
                    observation.line_count > 8,
                    "a phone-width reply must exceed the bounded recipes' eight lines, got {}",
                    observation.line_count
                );
            }
            // One accessible label carries the complete text.
            let labels: Vec<_> = update
                .nodes
                .iter()
                .filter(|(_, node)| node.label().or(node.value()) == Some(REPLY))
                .collect();
            assert_eq!(labels.len(), 1);
            assert_eq!(labels[0].1.role(), egui::accesskit::Role::Label);
            assert_eq!(
                labels[0].1.author_id(),
                Some(
                    PresentationScope::new("reading")
                        .instance("reply")
                        .semantic_id()
                        .0
                        .as_str()
                )
            );
            counts.push((width, scale, observation.line_count));
        }
    }
    // Narrower and larger text takes more lines.
    let lines = |width: f32, scale: f32| {
        counts
            .iter()
            .find(|(w, s, _)| *w == width && *s == scale)
            .unwrap()
            .2
    };
    assert!(lines(PHONE, 1.0) > lines(DESKTOP, 1.0));
    assert!(lines(PHONE, 1.5) > lines(PHONE, 1.0));
}

#[test]
fn line_counts_are_complete_beyond_the_old_byte_limit() {
    let text = (1..=400)
        .map(|line| format!("Line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let (observation, rect, _) = present_reading(&text, PHONE, 1.0);
    assert_eq!(observation.line_count, 400);
    assert!(audit_text_layouts(&[observation]).is_empty());
    let line_height = TextRole::Body.style(&tokens(), 1.0).line_height;
    assert!(
        (rect.height() - line_height.round() * 400.0).abs() < 1.0,
        "height {}, line height {line_height}",
        rect.height()
    );
}

#[test]
fn bounded_recipes_reject_unbounded_overflow_as_a_visible_failure() {
    let context = context();
    let mut output = context.run_ui(screen(400.0, 300.0), |ui| {
        let spec = TextSpec {
            overflow: TextOverflow::WrapUnbounded,
            max_lines: 0,
            ..TextSpec::single_line(TextRole::Body, TextOverflow::Wrap)
        };
        assert_eq!(
            measure_text(ui.painter(), "Text", spec, &tokens(), 1.0, 200.0).err(),
            Some(TextLayoutError::UnboundedOverflow)
        );
        let mut p = PresentationContext::new(
            ui,
            tokens(),
            1.0,
            PresentationScope::new("bounded"),
            SemanticUiId::root(),
        );
        p.content(
            ui,
            "content",
            "A bounded label asked to wrap without a limit",
            ContentTextSpec {
                role: TextRole::Body,
                overflow: TextOverflow::WrapUnbounded,
                max_lines: 0,
                interaction: TextInteraction::Selectable,
            },
        );
        let published = p.finish(ui);
        assert_eq!(published.coverage.failed_components, 1);
        assert!(
            audit_text_layouts(&published.text_layouts)
                .iter()
                .any(|finding| matches!(
                    finding,
                    TextAuditFinding::LayoutFailed {
                        error: TextLayoutError::UnboundedOverflow,
                        ..
                    }
                ))
        );
    });
    output.textures_delta.clear();
}

#[test]
fn audit_holds_unbounded_text_to_its_allocation_but_not_a_scroll_clip() {
    let (observation, _, _) = present_reading(REPLY, PHONE, 1.0);
    // A scroll surface may clip the allocation.
    let mut clipped = observation.clone();
    clipped.clip_rect.max_y = clipped.clip_rect.min_y + 20.0;
    assert!(audit_text_layouts(&[clipped]).is_empty());
    // The allocation itself must hold all of the text.
    let mut overflowing = observation.clone();
    overflowing.allocated_rect.max_y = overflowing.allocated_rect.min_y + 20.0;
    assert!(
        audit_text_layouts(&[overflowing])
            .iter()
            .any(|finding| matches!(finding, TextAuditFinding::TextOutsideAllocation { .. }))
    );
    // Unbounded text declares no limit and is never truncated.
    let mut limited = observation.clone();
    limited.declared_max_lines = 3;
    assert!(
        audit_text_layouts(&[limited])
            .iter()
            .any(|finding| matches!(finding, TextAuditFinding::UnexpectedLineCount { .. }))
    );
    let mut truncated = observation;
    truncated.truncated = true;
    assert!(
        audit_text_layouts(&[truncated])
            .iter()
            .any(|finding| matches!(finding, TextAuditFinding::UndeclaredTruncation { .. }))
    );
}

#[test]
fn a_scroll_area_keeps_visible_reading_text_and_omits_hidden_text_with_honest_coverage() {
    let context = context();
    let mut output = context.run_ui(screen(PHONE, 600.0), |ui| {
        let mut p = PresentationContext::new(
            ui,
            tokens(),
            1.0,
            PresentationScope::new("conversation"),
            SemanticUiId::root(),
        );
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                // The first reply overflows the 200-point viewport; the
                // second starts below it.
                p.reading_text(ui, "first", REPLY, ReadingTextSpec::default());
                p.reading_text(ui, "second", REPLY, ReadingTextSpec::default());
            });
        let published = p.finish(ui);
        assert_eq!(
            published.text_layouts.len(),
            1,
            "hidden text adds no observation"
        );
        let visible = &published.text_layouts[0];
        assert!(visible.clip_rect.max_y - visible.clip_rect.min_y <= 200.5);
        assert!(visible.allocated_rect.max_y > visible.clip_rect.max_y);
        assert!(audit_text_layouts(&published.text_layouts).is_empty());
        // Both measurements count as attempts.
        assert_eq!(published.coverage.attempted_components, 2);
        assert_eq!(published.coverage.successful_components, 2);
        assert_eq!(published.coverage.measured_components, 1);
        assert_eq!(
            text_audit_coverage(ui.ctx(), &published.text_layouts).attempted_components,
            2
        );
    });
    output.textures_delta.clear();
}

#[test]
fn drag_selection_copies_the_complete_multi_line_text() {
    let rect = Rc::new(Cell::new(Rect::NOTHING));
    let observed = rect.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(PHONE, 600.0))
        .build_ui(move |ui| {
            crate::install_typography_fonts(ui.ctx());
            let ready = ui.fonts(|fonts| {
                fonts
                    .definitions()
                    .families
                    .contains_key(&TextRole::Body.style(&tokens(), 1.0).font_id.family)
            });
            if !ready {
                ui.ctx().request_discard("install reading fonts first");
                return;
            }
            let mut layouts = Vec::new();
            let response = reading_text(
                ui,
                PresentationScope::new("copy").instance("reply"),
                REPLY,
                ReadingTextSpec::default(),
                &tokens(),
                1.0,
                &mut layouts,
            );
            assert!(layouts[0].line_count > 8);
            observed.set(response.rect);
        });
    harness.run();
    let rect = rect.get();
    let start = rect.left_top() + egui::vec2(0.5, 4.0);
    let end = rect.right_bottom() - egui::vec2(0.5, 4.0);
    harness.hover_at(start);
    harness.drag_at(start);
    harness.hover_at(end);
    harness.drop_at(end);
    harness.step();
    let label = harness.get_by_role_and_label(egui::accesskit::Role::Label, REPLY);
    assert!(label.accesskit_node().text_selection().is_some());
    harness.event(egui::Event::Copy);
    harness.step();
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(text) if text == REPLY)),
        "{:?}",
        harness.output().platform_output.commands
    );
}

/// URLs a link opened, recorded inside each pass.
type Opened = Rc<RefCell<Vec<egui::OpenUrl>>>;

/// A harness presenting one link; returns it, the link's rect and what it opens.
fn link_harness(spec: LinkSpec<'static>, enabled: bool) -> (Harness<'static>, Rect, Opened) {
    let rect = Rc::new(Cell::new(Rect::NOTHING));
    let opened = Opened::default();
    let (observed, recorded) = (rect.clone(), opened.clone());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(PHONE, 200.0))
        .build_ui(move |ui| {
            crate::install_typography_fonts(ui.ctx());
            ui.add_enabled_ui(enabled, |ui| {
                let mut p = PresentationContext::new(
                    ui,
                    tokens(),
                    1.0,
                    PresentationScope::new("links"),
                    SemanticUiId::root(),
                );
                let response = p.link(ui, "source", spec);
                observed.set(response.rect);
                let _ = p.finish(ui);
            });
            ui.ctx().output(|output| {
                recorded
                    .borrow_mut()
                    .extend(output.commands.iter().filter_map(|command| match command {
                        egui::OutputCommand::OpenUrl(open) => Some(open.clone()),
                        _ => None,
                    }));
            });
        });
    harness.run();
    let rect = rect.get();
    (harness, rect, opened)
}

/// What the link opened since the last call.
fn take(opened: &Opened) -> Vec<egui::OpenUrl> {
    std::mem::take(&mut *opened.borrow_mut())
}

fn click(harness: &mut Harness<'_>, at: egui::Pos2, button: egui::PointerButton) {
    harness.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run();
}

const URL: &str = "https://example.org/papers/attention-heads";

#[test]
fn a_link_requests_a_new_tab_by_default_from_pointer_keyboard_and_accesskit() {
    let (mut harness, rect, opened) = link_harness(LinkSpec::new("Read the paper", URL), true);
    assert!(take(&opened).is_empty(), "nothing opens without activation");
    assert!(rect.height() >= tokens().geometry.minimum_hit_size.0);
    assert!(rect.width() >= tokens().geometry.minimum_hit_size.0);
    assert!(!crate::host::new_tab_links_installed(&harness.ctx));
    click(&mut harness, rect.center(), egui::PointerButton::Primary);
    assert_eq!(take(&opened), [egui::OpenUrl::new_tab(URL)]);
    // Without an `AppHost`, the link itself makes the browser open new tabs
    // without an opener.
    assert!(crate::host::new_tab_links_installed(&harness.ctx));

    // Keyboard: Tab to the link, then Enter or Space.
    let id = PresentationScope::new("links").instance("source").egui_id();
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), Some(id));
    assert!(take(&opened).is_empty());
    for key in [egui::Key::Enter, egui::Key::Space] {
        harness.key_press(key);
        harness.run();
        assert_eq!(take(&opened), [egui::OpenUrl::new_tab(URL)], "{key:?}");
    }

    // Assistive technology activation takes the same route.
    let node = harness.get_by_role(egui::accesskit::Role::Link);
    assert_eq!(
        node.accesskit_node().label().as_deref(),
        Some("Read the paper")
    );
    assert_eq!(node.accesskit_node().data().url(), Some(URL));
    node.click_accesskit();
    harness.run();
    assert_eq!(take(&opened), [egui::OpenUrl::new_tab(URL)]);
}

#[test]
fn same_tab_links_ask_for_the_same_tab_unless_opened_in_the_background() {
    let (mut harness, rect, opened) = link_harness(
        LinkSpec {
            target: LinkTarget::SameTab,
            ..LinkSpec::new("Continue", URL)
        },
        true,
    );
    click(&mut harness, rect.center(), egui::PointerButton::Primary);
    assert_eq!(
        take(&opened),
        [egui::OpenUrl {
            url: URL.into(),
            new_tab: false
        }]
    );
    assert!(!crate::host::new_tab_links_installed(&harness.ctx));
    // A middle click opens in the background, which is always a new tab.
    click(&mut harness, rect.center(), egui::PointerButton::Middle);
    assert_eq!(take(&opened), [egui::OpenUrl::new_tab(URL)]);
}

#[test]
fn a_disabled_link_opens_nothing_and_says_so() {
    let (mut harness, rect, opened) = link_harness(LinkSpec::new("Read the paper", URL), false);
    click(&mut harness, rect.center(), egui::PointerButton::Primary);
    harness.key_press(egui::Key::Tab);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(take(&opened).is_empty());
    let node = harness.get_by_role(egui::accesskit::Role::Link);
    assert!(node.accesskit_node().is_disabled());
    assert!(
        !node
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Click)
    );
}

/// Present a link and a reading text in one context; return the snapshot and
/// AccessKit update.
fn present_semantics(
    width: f32,
    spec: LinkSpec<'_>,
) -> (
    UiSnapshot,
    egui::accesskit::TreeUpdate,
    Vec<TextLayoutObservation>,
) {
    let context = context();
    let root = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 600.0));
    let mut published = None;
    let mut output = context.run_ui(screen(width, 600.0), |ui| {
        let mut p = PresentationContext::new(
            ui,
            tokens(),
            1.0,
            PresentationScope::new("message"),
            SemanticUiId::root(),
        );
        p.reading_text(ui, "body", REPLY, ReadingTextSpec::default());
        p.link(ui, "source", spec);
        published = Some(p.finish(ui));
    });
    output.textures_delta.clear();
    let published = published.unwrap();
    let mut snapshot = UiSnapshot::default();
    snapshot.nodes.push(UiNode::container(
        SemanticUiId::root(),
        None,
        UiRole::Application,
        root.into(),
    ));
    snapshot.nodes.extend(published.semantic_nodes);
    (
        snapshot,
        output.platform_output.accesskit_update.take().unwrap(),
        published.text_layouts,
    )
}

#[test]
fn links_are_observed_as_link_nodes_with_their_url_and_matching_accesskit() {
    let spec = LinkSpec {
        description: Some("Opens in a new tab"),
        ..LinkSpec::new("Read the paper", URL)
    };
    let (snapshot, update, layouts) = present_semantics(PHONE, spec);
    let link = snapshot
        .nodes
        .iter()
        .find(|node| node.role == UiRole::Link)
        .unwrap();
    assert_eq!(link.name, "Read the paper");
    assert_eq!(link.value.as_deref(), Some(URL));
    assert_eq!(link.description.as_deref(), Some("Opens in a new tab"));
    assert_eq!(
        link.id,
        PresentationScope::new("message")
            .instance("source")
            .semantic_id()
    );
    assert!(link.enabled && !link.text_selectable);
    assert!(
        audit_accesskit(&snapshot, &update).is_empty(),
        "{:#?}",
        audit_accesskit(&snapshot, &update)
    );
    assert!(audit_text_layouts(&layouts).is_empty());
    let kinds: Vec<_> = layouts
        .iter()
        .map(|layout| layout.component_id.kind)
        .collect();
    assert_eq!(
        kinds,
        [TextComponentKind::ReadingText, TextComponentKind::Link]
    );
    assert_eq!(layouts[1].interaction, TextInteraction::Inert);
    assert_eq!(layouts[1].overflow, TextOverflow::WrapUnbounded);

    // Parity catches a link whose AccessKit URL disagrees.
    let mut wrong = snapshot.clone();
    wrong
        .nodes
        .iter_mut()
        .find(|node| node.role == UiRole::Link)
        .unwrap()
        .value = Some("https://example.org/other".into());
    assert!(
        audit_accesskit(&wrong, &update)
            .iter()
            .any(|finding| matches!(finding, AccessKitMismatch::Url { .. }))
    );
}

#[test]
fn long_link_labels_wrap_instead_of_truncating_and_blank_labels_show_the_url() {
    let label = "A deliberately long article title that cannot fit on one line of a phone";
    let (snapshot, _, layouts) = present_semantics(PHONE / 2.0, LinkSpec::new(label, URL));
    let link_layout = layouts
        .iter()
        .find(|layout| layout.component_id.kind == TextComponentKind::Link)
        .unwrap();
    assert!(link_layout.line_count > 1);
    assert!(!link_layout.truncated);
    assert!(audit_text_layouts(&layouts).is_empty());
    assert!(snapshot.nodes.iter().any(|node| node.name == label));

    let (snapshot, _, _) = present_semantics(PHONE, LinkSpec::new("  ", URL));
    assert!(
        snapshot
            .nodes
            .iter()
            .any(|node| node.role == UiRole::Link && node.name == URL)
    );
}

#[test]
fn link_colour_meets_text_contrast_in_every_reference_mode_or_falls_back_to_primary_text() {
    for variant in VARIANTS {
        let tokens = DesignTokens::resolve(variant, DensityVariant::Comfortable);
        let colour = link_colour(&tokens);
        assert_eq!(
            colour,
            Color32::from(tokens.colours.accent_primary),
            "{variant:?}"
        );
        let minimum = match variant {
            ThemeVariant::LightHighContrast | ThemeVariant::DarkHighContrast => 7.0,
            _ => 4.5,
        };
        for surface in [tokens.colours.surface_canvas, tokens.colours.surface_panel] {
            let ratio = contrast_ratio(colour, surface.into());
            assert!(ratio >= minimum, "{variant:?}: {ratio:.2}");
        }
    }
    // An accent too faint for text leaves the link in primary text; its
    // underline still marks it.
    let mut faint = tokens();
    faint.colours.accent_primary = faint.colours.surface_panel;
    assert_eq!(
        link_colour(&faint),
        Color32::from(faint.colours.text_primary)
    );
    let theme = ApplicationTheme::analytical();
    let resolved = theme.resolve(
        ThemeVariant::Light,
        DensityVariant::Compact,
        TypographyProfile::Reading,
    );
    assert_eq!(
        link_colour(&resolved),
        Color32::from(resolved.colours.accent_primary)
    );
}

#[test]
fn hovering_a_link_keeps_its_allocation() {
    let (mut harness, rect, _) = link_harness(LinkSpec::new("Read the paper", URL), true);
    harness.hover_at(rect.center());
    harness.step();
    let hovered = harness.get_by_role(egui::accesskit::Role::Link);
    let bounds = hovered.accesskit_node().data().bounds().unwrap();
    assert!((bounds.x0 as f32 - rect.left()).abs() < 0.5);
    assert!((bounds.x1 as f32 - rect.right()).abs() < 0.5);
    assert!((bounds.y1 as f32 - rect.bottom()).abs() < 0.5);
}

#[test]
fn reading_identity_is_stable_across_layouts() {
    let scope = PresentationScope::new("stable");
    let context = context();
    let mut ids = Vec::new();
    for layout in ["wide", "narrow"] {
        let mut output = context.run_ui(screen(DESKTOP, 600.0), |ui| {
            ui.push_id(layout, |ui| {
                let mut p =
                    PresentationContext::new(ui, tokens(), 1.0, scope, SemanticUiId::root());
                let body = p.reading_text(ui, "body", "Short", ReadingTextSpec::default());
                let link = p.link(ui, "source", LinkSpec::new("Source", URL));
                ids.push((body.id, link.id, p.finish(ui).text_layouts));
            });
        });
        output.textures_delta.clear();
    }
    assert_eq!(ids[0].0, ids[1].0);
    assert_eq!(ids[0].1, ids[1].1);
    assert_eq!(
        ids[0].2.iter().map(|l| l.component_id).collect::<Vec<_>>(),
        ids[1].2.iter().map(|l| l.component_id).collect::<Vec<_>>()
    );
}

#[test]
fn only_http_https_and_mailto_urls_are_allowed() {
    for url in [
        URL,
        "http://example.org",
        "HTTPS://Example.org/Path?q=1#top",
        "mailto:someone@example.org",
    ] {
        assert!(crate::link_url_allowed(url), "{url}");
    }
    for url in [
        "",
        "   ",
        "javascript:alert(1)",
        "JavaScript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "vbscript:msgbox",
        "file:///etc/passwd",
        "ftp://example.org",
        "/relative/path",
        "https://",
        "mailto:",
        " https://example.org",
        "https://example.org/a b",
        "https://example.org/\nnext",
    ] {
        assert!(!crate::link_url_allowed(url), "{url:?}");
    }
}

#[test]
fn rejected_urls_paint_a_diagnostic_and_open_nothing() {
    for url in ["javascript:alert(document.domain)", "", "  "] {
        let spec = LinkSpec::new("Read the paper", url);
        let (snapshot, update, layouts) = present_semantics(PHONE, spec);
        let link = snapshot
            .nodes
            .iter()
            .find(|node| node.role == UiRole::Link)
            .unwrap();
        assert!(!link.enabled, "{url:?}");
        assert_eq!(
            link.disabled_reason.as_deref(),
            Some(crate::INVALID_LINK_URL)
        );
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{url:?}: {findings:#?}");
        let failure = layouts
            .iter()
            .find(|layout| layout.component_id.kind == TextComponentKind::Link)
            .unwrap();
        assert_eq!(failure.layout_error, Some(TextLayoutError::InvalidUrl));
        assert!(audit_text_layouts(&layouts).iter().any(|finding| matches!(
            finding,
            TextAuditFinding::LayoutFailed {
                error: TextLayoutError::InvalidUrl,
                ..
            }
        )));
        let binding = crate::LinkBinding::from_node(link).unwrap();
        assert!(!binding.availability.enabled());

        let (mut harness, rect, opened) = link_harness(LinkSpec::new("Read the paper", url), true);
        click(&mut harness, rect.center(), egui::PointerButton::Primary);
        click(&mut harness, rect.center(), egui::PointerButton::Middle);
        harness.key_press(egui::Key::Tab);
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        assert!(take(&opened).is_empty(), "{url:?}");
    }
}
