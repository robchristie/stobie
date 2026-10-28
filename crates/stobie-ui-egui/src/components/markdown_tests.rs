//! Markdown: every element at phone and desktop widths, a long digest,
//! malformed and partial input, unsafe links, link activation, selection,
//! observation and the open-link capability through a host.

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

use egui::{Modifiers, Rect};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

use crate::{
    CodeCopySpec, DEFAULT_MARKDOWN_OPTIONS, DensityVariant, DesignTokens, MarkdownBlockKind,
    MarkdownOptions, MarkdownSpec, PresentationContext, PresentationScope, SemanticUiId,
    TextAuditFinding, TextComponentId, TextComponentKind, TextInteraction, TextLayoutObservation,
    TextOverflow, TextRole, ThemeVariant, UiNode, UiRole, UiSnapshot, audit_accesskit,
    audit_text_layouts,
};

const PHONE: f32 = 343.0;
const DESKTOP: f32 = 960.0;

/// Every element the recipe presents.
const ELEMENTS: &str = "# Morning digest\n\n\
## Papers\n\n\
###### Footnote heading\n\n\
A *quiet* **strong** ~~withdrawn~~ `inline code` [linked title](https://example.org/a) \
and <https://example.org/b> and <someone@example.org>.\n\n\
- First bullet\n- Second bullet\n  1. Nested number\n  2. Another\n     - Third level\n\n\
3. Starts at three\n4. Four\n\n\
> A quoted note\n>\n> > nested deeper\n\n\
```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n\n\
---\n\n\
After the rule.";

/// A digest of ten linked titles, each with a note.
fn digest() -> String {
    let mut source = String::from("## Morning digest\n\nTen items today, **top pick first**.\n\n");
    for item in 1..=10 {
        source.push_str(&format!(
            "{item}. [A paper about topic {item} with a descriptive title](https://example.org/papers/{item}) \
             — one line on why it matters, with `code` and *emphasis* in item {item}.\n"
        ));
    }
    source.push_str(
        "\n### Also noted\n\n- A note ([discussion](https://example.org/discussion)).\n\
         - ~~Withdrawn~~ benchmark.\n\n> Two sources were unreachable.\n\n\
         ```sh\npip install example-package\n```\n",
    );
    source
}

fn tokens() -> DesignTokens {
    DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable)
}

fn context() -> egui::Context {
    let context = egui::Context::default();
    crate::install_typography_fonts(&context);
    context.enable_accesskit();
    context
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

struct Presented {
    layouts: Vec<TextLayoutObservation>,
    snapshot: UiSnapshot,
    update: egui::accesskit::TreeUpdate,
    rect: Rect,
    shapes: Vec<egui::epaint::ClippedShape>,
}

impl Presented {
    fn links(&self) -> Vec<&UiNode> {
        self.snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::Link)
            .collect()
    }

    fn kind(&self, kind: TextComponentKind) -> Vec<&TextLayoutObservation> {
        self.layouts
            .iter()
            .filter(|layout| layout.component_id.kind == kind)
            .collect()
    }

    fn rects(&self) -> BTreeMap<TextComponentId, crate::TextRect> {
        self.layouts
            .iter()
            .map(|layout| (layout.component_id, layout.allocated_rect))
            .collect()
    }
}

fn present_with(
    context: &egui::Context,
    source: &str,
    width: f32,
    scale: f32,
    tokens: DesignTokens,
) -> Presented {
    present_copying(context, source, width, scale, tokens, None)
}

/// Present `source`, with a copy control on each code block when `copy` is
/// given.
fn present_copying(
    context: &egui::Context,
    source: &str,
    width: f32,
    scale: f32,
    tokens: DesignTokens,
    copy: Option<&CodeCopySpec<'_>>,
) -> Presented {
    present_spec(
        context,
        source,
        width,
        scale,
        tokens,
        copy,
        MarkdownSpec::default(),
    )
}

/// Present `source` under `spec`, with a copy control on each code block
/// when `copy` is given.
fn present_spec(
    context: &egui::Context,
    source: &str,
    width: f32,
    scale: f32,
    tokens: DesignTokens,
    copy: Option<&CodeCopySpec<'_>>,
    spec: MarkdownSpec<'_>,
) -> Presented {
    let mut published = None;
    let mut rect = Rect::NOTHING;
    let mut output = context.run_ui(screen(width, 20_000.0), |ui| {
        let mut p = PresentationContext::new(
            ui,
            tokens,
            scale,
            PresentationScope::new("reply"),
            SemanticUiId::root(),
        );
        rect = match copy {
            Some(copy) => p.markdown_with_code_copy(ui, "body", source, spec, copy),
            None => p.markdown(ui, "body", source, spec),
        }
        .rect;
        published = Some(p.finish(ui));
    });
    output.textures_delta.clear();
    let published = published.unwrap();
    let mut snapshot = UiSnapshot::default();
    snapshot.nodes.push(UiNode::container(
        SemanticUiId::root(),
        None,
        UiRole::Application,
        Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 20_000.0)).into(),
    ));
    snapshot.nodes.extend(published.semantic_nodes);
    Presented {
        layouts: published.text_layouts,
        snapshot,
        update: output.platform_output.accesskit_update.take().unwrap(),
        rect,
        shapes: output.shapes,
    }
}

fn present(source: &str, width: f32, scale: f32) -> Presented {
    present_with(&context(), source, width, scale, tokens())
}

fn assert_clean(presented: &Presented, label: &str) {
    let findings = audit_text_layouts(&presented.layouts);
    assert!(findings.is_empty(), "{label}: {findings:#?}");
    let parity = audit_accesskit(&presented.snapshot, &presented.update);
    assert!(parity.is_empty(), "{label}: {parity:#?}");
}

#[test]
fn every_element_is_measured_and_observed_at_phone_and_desktop_widths() {
    for width in [PHONE, DESKTOP] {
        for scale in [1.0, 1.5] {
            let label = format!("{width} at {scale}");
            let presented = present(ELEMENTS, width, scale);
            assert_clean(&presented, &label);
            let reading = presented.kind(TextComponentKind::ReadingText);
            // Three headings, a paragraph, seven list items, two quoted
            // paragraphs, the code and the closing paragraph.
            assert_eq!(reading.len(), 15, "{label}: {reading:#?}");
            for text in &reading {
                assert_eq!(text.overflow, TextOverflow::WrapUnbounded);
                assert_eq!(text.declared_max_lines, 0);
                assert_eq!(text.interaction, TextInteraction::Selectable);
                assert!(!text.truncated);
                assert!(text.allocated_rect.max_x <= width + 0.5, "{label}");
                assert!(text.painted_rect.max_x <= text.allocated_rect.max_x + 1.0);
                assert!(presented.rect.top() <= text.allocated_rect.min_y + 0.5);
                assert!(text.allocated_rect.max_y <= presented.rect.bottom() + 0.5);
            }
            let roles: Vec<_> = reading.iter().map(|text| text.role).collect();
            assert_eq!(
                &roles[..3],
                [
                    TextRole::ApplicationTitle,
                    TextRole::PaneTitle,
                    TextRole::SectionHeading
                ]
            );
            assert_eq!(
                roles
                    .iter()
                    .filter(|role| **role == TextRole::MonospaceTechnical)
                    .count(),
                1
            );
            let code = reading
                .iter()
                .find(|text| text.role == TextRole::MonospaceTechnical)
                .unwrap();
            assert!(code.line_count >= 3);
            // Four numbered items carry measured numbers; bullets are painted.
            let markers = presented.kind(TextComponentKind::ListMarker);
            assert_eq!(markers.len(), 4, "{label}");
            for marker in &markers {
                assert_eq!(
                    marker.horizontal_alignment,
                    crate::HorizontalTextAlignment::End
                );
                assert_eq!(marker.interaction, TextInteraction::Inert);
            }
            // Nested items sit further in than their parents.
            let left = |index: usize| reading[index].allocated_rect.min_x;
            assert!(left(4) > left(3), "a bullet's text is indented");
            assert!(left(6) > left(5), "a nested number is indented further");
            assert!(left(8) > left(6), "a third level is indented further still");

            // Headings are headings for assistive technology, with levels.
            let mut levels: Vec<_> = presented
                .update
                .nodes
                .iter()
                .filter(|(_, node)| node.role() == egui::accesskit::Role::Heading)
                .map(|(_, node)| node.level())
                .collect();
            levels.sort();
            assert_eq!(levels, [Some(1), Some(2), Some(6)], "{label}");

            let links = presented.links();
            let urls: Vec<_> = links
                .iter()
                .map(|link| (link.name.as_str(), link.value.as_deref().unwrap()))
                .collect();
            assert_eq!(
                urls,
                [
                    ("linked title", "https://example.org/a"),
                    ("https://example.org/b", "https://example.org/b"),
                    ("someone@example.org", "mailto:someone@example.org"),
                ]
            );
            assert!(
                links
                    .iter()
                    .all(|link| link.enabled && link.parent == Some(SemanticUiId::root()))
            );
        }
    }
}

#[test]
fn a_long_digest_reads_at_phone_width_with_every_link_and_number() {
    let source = digest();
    let phone = present(&source, PHONE, 1.0);
    assert_clean(&phone, "phone");
    let links = phone.links();
    assert_eq!(links.len(), 11);
    for (item, link) in links.iter().take(10).enumerate() {
        let item = item + 1;
        assert_eq!(
            link.name,
            format!("A paper about topic {item} with a descriptive title")
        );
        assert_eq!(
            link.value.as_deref(),
            Some(format!("https://example.org/papers/{item}").as_str())
        );
    }
    let markers = phone.kind(TextComponentKind::ListMarker);
    assert_eq!(markers.len(), 10);
    // Every number shares one column, end-aligned, "10." included.
    assert!(markers.windows(2).all(|pair| {
        (pair[0].allocated_rect.min_x - pair[1].allocated_rect.min_x).abs() < 0.01
            && (pair[0].allocated_rect.max_x - pair[1].allocated_rect.max_x).abs() < 0.01
    }));
    // The digest is taller than a phone screen and wraps its items.
    assert!(phone.rect.height() > 480.0, "{}", phone.rect.height());
    let desktop = present(&source, DESKTOP, 1.0);
    assert_clean(&desktop, "desktop");
    assert!(desktop.rect.height() < phone.rect.height());
    let lines = |presented: &Presented| {
        presented
            .kind(TextComponentKind::ReadingText)
            .iter()
            .map(|text| text.line_count)
            .sum::<u32>()
    };
    assert!(lines(&phone) > lines(&desktop));
}

#[test]
fn malformed_and_partial_input_presents_without_panicking() {
    let context = context();
    let source = digest();
    // Every prefix of a reply as it streams in.
    let boundaries: Vec<_> = source
        .char_indices()
        .map(|(index, _)| index)
        .chain([source.len()])
        .collect();
    for end in &boundaries {
        let presented = present_with(&context, &source[..*end], PHONE, 1.0, tokens());
        assert!(
            audit_text_layouts(&presented.layouts).is_empty(),
            "{:?}",
            &source[..*end]
        );
    }
    for malformed in [
        "**unclosed strong",
        "*",
        "[",
        "[label](",
        "[label](https://example.org",
        "```",
        "```rust\nfn unfinished(",
        "> ",
        "-",
        "1.",
        "1)",
        "#",
        "######## seven hashes",
        "<div>",
        "<!-- comment",
        "\u{0}\u{1}\u{7f}",
        "\r\n\r\n- crlf\r\n- items\r\n",
        "Mixed scripts: مرحبا שלום こんにちは 👩‍💻 e\u{301}",
        "\u{200b}\u{200d}\u{feff}",
        "\t\tindented\twith\ttabs",
        "[](https://example.org/empty-label)",
        "[label]()",
        "![](https://example.org/empty-alt.png)",
        "| a |\n|---|",
        "    ",
    ] {
        let presented = present_with(&context, malformed, PHONE, 1.0, tokens());
        assert_clean(&presented, &format!("{malformed:?}"));
    }
}

#[test]
fn a_streaming_reply_keeps_its_completed_blocks_in_place() {
    let context = context();
    let source = digest();
    let finished = present_with(&context, &source, PHONE, 1.0, tokens()).rects();
    let mut checked = 0;
    for (end, _) in source.match_indices('\n') {
        let partial = present_with(&context, &source[..=end], PHONE, 1.0, tokens());
        let mut rects: Vec<_> = partial.rects().into_iter().collect();
        // The last block may still be growing.
        rects.sort_by(|a, b| a.1.min_y.total_cmp(&b.1.min_y));
        let Some(last) = rects.iter().map(|(_, rect)| rect.min_y).reduce(f32::max) else {
            continue;
        };
        for (id, rect) in rects.iter().filter(|(_, rect)| rect.min_y < last) {
            assert_eq!(finished.get(id), Some(rect), "{:?}", &source[..=end]);
            checked += 1;
        }
    }
    assert!(checked > 100, "{checked}");
}

#[test]
fn deep_nesting_stops_indenting_where_the_column_runs_out() {
    let lists: String = (0..40)
        .map(|level| format!("{}- level {level}\n", "  ".repeat(level)))
        .collect();
    let quotes = format!("{} deeply quoted", ">".repeat(40));
    for source in [lists.as_str(), quotes.as_str()] {
        for width in [200.0, PHONE] {
            let presented = present(source, width, 1.5);
            assert_clean(&presented, source);
            let reading = presented.kind(TextComponentKind::ReadingText);
            assert!(!reading.is_empty());
            let minimum = tokens().geometry.minimum_hit_size.0 * 2.0 * 1.5;
            for text in reading {
                assert!(
                    text.allocated_rect.max_x - text.allocated_rect.min_x >= minimum - 0.5
                        || text.allocated_rect.max_x - text.allocated_rect.min_x >= width - 0.5,
                    "{width}: {text:?}"
                );
                assert!(text.allocated_rect.max_x <= width + 0.5);
            }
        }
    }
    // Every level's text survives, with flattened items led by their marker.
    let presented = present(&lists, 200.0, 1.5);
    let accessible: Vec<_> = presented
        .update
        .nodes
        .iter()
        .filter_map(|(_, node)| node.label().or(node.value()))
        .collect();
    for level in 0..40 {
        assert!(
            accessible
                .iter()
                .any(|label| label.ends_with(&format!("level {level}"))),
            "level {level}"
        );
    }
    assert!(accessible.iter().any(|label| label.starts_with("• level")));
}

#[test]
fn unsafe_link_schemes_keep_their_text_and_are_unavailable() {
    let source = "Read [safe](https://example.org/safe), [script](javascript:alert(1)), \
                  [data](data:text/html,hi), [vb](vbscript:msgbox), [relative](/settings), \
                  [empty]() and [file](file:///etc/passwd).";
    let presented = present(source, PHONE, 1.0);
    // Content from a model never fails the text audit.
    assert_clean(&presented, "unsafe");
    assert!(
        presented
            .layouts
            .iter()
            .all(|layout| layout.layout_error.is_none())
    );
    let links = presented.links();
    assert_eq!(links.len(), 7);
    assert!(links[0].enabled && links[0].disabled_reason.is_none());
    for link in &links[1..] {
        assert!(!link.enabled, "{}", link.name);
        assert_eq!(
            link.disabled_reason.as_deref(),
            Some(crate::INVALID_LINK_URL),
            "{}",
            link.name
        );
        let binding = crate::LinkBinding::from_node(link).unwrap();
        assert!(!binding.availability.enabled());
    }
    let unavailable: Vec<_> = presented
        .update
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == egui::accesskit::Role::Link && node.is_disabled())
        .collect();
    assert_eq!(unavailable.len(), 6);
    assert!(unavailable.iter().all(|(_, node)| {
        !node.supports_action(egui::accesskit::Action::Click)
            && node
                .description()
                .is_some_and(|description| description.contains(crate::INVALID_LINK_URL))
    }));

    // Nothing opens, by pointer or keyboard.
    let (mut harness, rects, opened) = link_harness(source, PHONE, true, MarkdownSpec::default());
    for rect in &rects[1..] {
        click(&mut harness, rect.center(), egui::PointerButton::Primary);
        click(&mut harness, rect.center(), egui::PointerButton::Middle);
    }
    assert!(take(&opened).is_empty());
    // Tab reaches only the safe link.
    harness.key_press(egui::Key::Tab);
    harness.run();
    harness.key_press(egui::Key::Tab);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        take(&opened),
        [egui::OpenUrl::new_tab("https://example.org/safe")]
    );
}

/// URLs opened, recorded inside each pass.
type Opened = Rc<RefCell<Vec<egui::OpenUrl>>>;

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

/// A harness presenting `source`; returns it, each link's node rectangle
/// and what the links open.
fn link_harness(
    source: &'static str,
    width: f32,
    enabled: bool,
    spec: MarkdownSpec<'static>,
) -> (Harness<'static>, Vec<Rect>, Opened) {
    let (harness, rects, opened, _) = following_harness(source, width, enabled, spec);
    (harness, rects, opened)
}

/// In-application links followed, recorded inside each pass.
type Followed = Rc<RefCell<Vec<crate::FollowedMarkdownLink>>>;

fn taken(followed: &Followed) -> Vec<(usize, String)> {
    std::mem::take(&mut *followed.borrow_mut())
        .into_iter()
        .map(|link| (link.index, link.destination))
        .collect()
}

/// [`link_harness`], also returning the in-application links followed.
fn following_harness(
    source: &'static str,
    width: f32,
    enabled: bool,
    spec: MarkdownSpec<'static>,
) -> (Harness<'static>, Vec<Rect>, Opened, Followed) {
    let rects = Rc::new(RefCell::new(Vec::new()));
    let opened = Opened::default();
    let followed = Followed::default();
    let (observed, recorded, follows) = (rects.clone(), opened.clone(), followed.clone());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(width, 600.0))
        .build_ui(move |ui| {
            crate::install_typography_fonts(ui.ctx());
            ui.add_enabled_ui(enabled, |ui| {
                let mut p = PresentationContext::new(
                    ui,
                    tokens(),
                    1.0,
                    PresentationScope::new("reply"),
                    SemanticUiId::root(),
                );
                p.markdown(ui, "body", source, spec);
                follows.borrow_mut().extend(p.followed_link().cloned());
                *observed.borrow_mut() = p
                    .finish(ui)
                    .semantic_nodes
                    .iter()
                    .filter(|node| node.role == UiRole::Link)
                    .map(|node| {
                        Rect::from_min_max(
                            egui::pos2(node.rect.min_x, node.rect.min_y),
                            egui::pos2(node.rect.max_x, node.rect.max_y),
                        )
                    })
                    .collect();
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
    let rects = rects.borrow().clone();
    (harness, rects, opened, followed)
}

const URL: &str = "https://example.org/papers/retrieval-heads";
const SENTENCE: &str =
    "Read [the paper on retrieval heads](https://example.org/papers/retrieval-heads) today.";

#[test]
fn links_in_text_open_a_new_tab_from_pointer_keyboard_and_accesskit() {
    let (mut harness, rects, opened) =
        link_harness(SENTENCE, DESKTOP, true, MarkdownSpec::default());
    assert_eq!(rects.len(), 1);
    assert!(take(&opened).is_empty(), "nothing opens without activation");
    let link = rects[0];
    assert!(!crate::host::new_tab_links_installed(&harness.ctx));
    click(&mut harness, link.center(), egui::PointerButton::Primary);
    assert_eq!(take(&opened), [egui::OpenUrl::new_tab(URL)]);
    // Without an `AppHost`, the link itself makes the browser open new tabs
    // without an opener.
    assert!(crate::host::new_tab_links_installed(&harness.ctx));
    // A middle click also asks for a new tab.
    click(&mut harness, link.center(), egui::PointerButton::Middle);
    assert_eq!(take(&opened), [egui::OpenUrl::new_tab(URL)]);
    // The words around the link are text, not the link.
    click(
        &mut harness,
        egui::pos2(link.left() - 12.0, link.center().y),
        egui::PointerButton::Primary,
    );
    click(
        &mut harness,
        egui::pos2(link.right() + 12.0, link.center().y),
        egui::PointerButton::Primary,
    );
    assert!(take(&opened).is_empty());

    // Keyboard: Tab to the link, then Enter or Space.
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert!(
        harness
            .ctx
            .memory(|memory| memory.focused())
            .is_some_and(|focused| focused
                == PresentationScope::new("reply")
                    .instance("body")
                    .child(("link", 0_usize))
                    .egui_id())
    );
    for key in [egui::Key::Enter, egui::Key::Space] {
        harness.key_press(key);
        harness.run();
        assert_eq!(take(&opened), [egui::OpenUrl::new_tab(URL)], "{key:?}");
    }
    // The focused link reports focus in its semantic node and AccessKit.
    let node = harness.get_by_role(egui::accesskit::Role::Link);
    assert_eq!(
        node.accesskit_node().label().as_deref(),
        Some("the paper on retrieval heads")
    );
    assert_eq!(node.accesskit_node().data().url(), Some(URL));
    node.click_accesskit();
    harness.run();
    assert_eq!(take(&opened), [egui::OpenUrl::new_tab(URL)]);
}

/// Links the application classifies: in-application, external,
/// unavailable with its reason, an unsafe URL it calls external, an unsafe
/// URL it calls in-application, and one past the end of its list.
const CLASSIFIED: &str = "Read [Setting up](guide.md#setting-up), [the site](https://example.org/site), \
    [the lost page](lost.md), [a script](javascript:alert(1)), [another script](javascript:alert(2)) \
    and [later](https://example.org/later).";

const LOST: &str = "No document is named lost.md";

const KINDS: [crate::MarkdownLinkKind<'static>; 5] = [
    crate::MarkdownLinkKind::InApplication,
    crate::MarkdownLinkKind::External,
    crate::MarkdownLinkKind::Unavailable(LOST),
    crate::MarkdownLinkKind::External,
    crate::MarkdownLinkKind::InApplication,
];

const CLASSIFIED_SPEC: MarkdownSpec<'static> = MarkdownSpec {
    interaction: TextInteraction::Selectable,
    link_description: Some("Opens in a new tab"),
    options: DEFAULT_MARKDOWN_OPTIONS,
    heading_ids: &[],
    link_kinds: &KINDS,
    footnote_ids: &[],
};

#[test]
fn the_application_classifies_each_link_and_cannot_loosen_the_url_rules() {
    assert_eq!(MarkdownSpec::default().link_kinds, &[]);
    assert_eq!(
        crate::MarkdownLinkKind::default(),
        crate::MarkdownLinkKind::External
    );
    let map = crate::markdown_source_map(CLASSIFIED, DEFAULT_MARKDOWN_OPTIONS, 0);
    assert_eq!(map.links.len(), 6);
    assert_eq!(map.links[0].destination, "guide.md#setting-up");
    for width in [PHONE, DESKTOP] {
        let presented = present_spec(
            &context(),
            CLASSIFIED,
            width,
            1.0,
            tokens(),
            None,
            CLASSIFIED_SPEC,
        );
        assert_clean(&presented, &format!("{width}"));
        let links = presented.links();
        assert_eq!(
            links
                .iter()
                .map(|link| (
                    link.name.as_str(),
                    link.value.as_deref(),
                    link.enabled,
                    link.description.as_deref(),
                    link.disabled_reason.as_deref()
                ))
                .collect::<Vec<_>>(),
            [
                ("Setting up", Some("guide.md#setting-up"), true, None, None),
                (
                    "the site",
                    Some("https://example.org/site"),
                    true,
                    Some("Opens in a new tab"),
                    None
                ),
                ("the lost page", Some("lost.md"), false, None, Some(LOST)),
                (
                    "a script",
                    Some("javascript:alert(1)"),
                    false,
                    Some("Opens in a new tab"),
                    Some(crate::INVALID_LINK_URL)
                ),
                (
                    "another script",
                    Some("javascript:alert(2)"),
                    true,
                    None,
                    None
                ),
                (
                    "later",
                    Some("https://example.org/later"),
                    true,
                    Some("Opens in a new tab"),
                    None
                ),
            ],
            "{width}"
        );
        // Agents see the same availability, with the application's reason.
        let lost = crate::LinkBinding::from_node(links[2]).unwrap();
        assert_eq!(lost.availability.disabled_reason(), Some(LOST));
        // AccessKit agrees: the in-application link is an activatable link
        // to its destination; the unavailable one says why.
        let accessible = |label: &str| {
            presented
                .update
                .nodes
                .iter()
                .map(|(_, node)| node)
                .find(|node| {
                    node.role() == egui::accesskit::Role::Link && node.label() == Some(label)
                })
                .unwrap()
                .clone()
        };
        let setting_up = accessible("Setting up");
        assert_eq!(setting_up.url(), Some("guide.md#setting-up"));
        assert!(setting_up.supports_action(egui::accesskit::Action::Click));
        assert!(!setting_up.is_disabled());
        assert_eq!(setting_up.description(), None);
        let lost = accessible("the lost page");
        assert!(lost.is_disabled());
        assert!(!lost.supports_action(egui::accesskit::Action::Click));
        assert_eq!(
            lost.description(),
            Some(format!("Unavailable: {LOST}").as_str())
        );
    }

    let (mut harness, rects, opened, followed) =
        following_harness(CLASSIFIED, DESKTOP, true, CLASSIFIED_SPEC);
    assert_eq!(rects.len(), 6);
    assert!(take(&opened).is_empty() && taken(&followed).is_empty());
    // Pointer: the in-application link is followed, by a primary or a middle
    // click, and opens nothing.
    for button in [egui::PointerButton::Primary, egui::PointerButton::Middle] {
        click(&mut harness, rects[0].center(), button);
        assert_eq!(
            taken(&followed),
            [(0, "guide.md#setting-up".to_owned())],
            "{button:?}"
        );
        assert!(take(&opened).is_empty(), "{button:?}");
    }
    // Nothing is reported again without another activation.
    harness.run();
    assert!(taken(&followed).is_empty());
    // The external links open a new tab, as before, and are not reported.
    click(
        &mut harness,
        rects[1].center(),
        egui::PointerButton::Primary,
    );
    click(
        &mut harness,
        rects[5].center(),
        egui::PointerButton::Primary,
    );
    assert_eq!(
        take(&opened),
        [
            egui::OpenUrl::new_tab("https://example.org/site"),
            egui::OpenUrl::new_tab("https://example.org/later")
        ]
    );
    assert!(taken(&followed).is_empty());
    // The unavailable links do nothing, whatever the application said.
    for rect in &rects[2..4] {
        click(&mut harness, rect.center(), egui::PointerButton::Primary);
        click(&mut harness, rect.center(), egui::PointerButton::Middle);
    }
    assert!(take(&opened).is_empty() && taken(&followed).is_empty());
    // An unsafe URL the application follows itself is reported, never opened.
    click(
        &mut harness,
        rects[4].center(),
        egui::PointerButton::Primary,
    );
    assert_eq!(taken(&followed), [(4, "javascript:alert(2)".to_owned())]);
    assert!(take(&opened).is_empty());

    // Keyboard: Tab reaches the available links in order; Enter and Space
    // follow or open each.
    let focus = |harness: &Harness<'_>| {
        harness
            .ctx
            .memory(|memory| memory.focused())
            .and_then(|focused| {
                (0..6_usize).find(|index| {
                    focused
                        == PresentationScope::new("reply")
                            .instance("body")
                            .child(("link", *index))
                            .egui_id()
                })
            })
    };
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert_eq!(focus(&harness), Some(0));
    for key in [egui::Key::Enter, egui::Key::Space] {
        harness.key_press(key);
        harness.run();
        assert_eq!(
            taken(&followed),
            [(0, "guide.md#setting-up".to_owned())],
            "{key:?}"
        );
        assert!(take(&opened).is_empty(), "{key:?}");
    }
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert_eq!(focus(&harness), Some(1));
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        take(&opened),
        [egui::OpenUrl::new_tab("https://example.org/site")]
    );
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert_eq!(
        focus(&harness),
        Some(4),
        "unavailable links are no Tab stop"
    );

    // AccessKit: activating the in-application link follows it.
    harness
        .get_all_by_role(egui::accesskit::Role::Link)
        .find(|node| node.accesskit_node().label().as_deref() == Some("Setting up"))
        .unwrap()
        .click_accesskit();
    harness.run();
    assert_eq!(taken(&followed), [(0, "guide.md#setting-up".to_owned())]);
    assert!(take(&opened).is_empty());

    // In a disabled region nothing is followed.
    let (mut harness, rects, opened, followed) =
        following_harness(CLASSIFIED, DESKTOP, false, CLASSIFIED_SPEC);
    click(
        &mut harness,
        rects[0].center(),
        egui::PointerButton::Primary,
    );
    assert!(take(&opened).is_empty() && taken(&followed).is_empty());
}

#[test]
fn a_wrapped_link_opens_from_every_line_it_spans() {
    let source = "See [a deliberately long article title that wraps over several lines of a narrow column](https://example.org/long) now.";
    let (mut harness, rects, opened) = link_harness(source, 180.0, true, MarkdownSpec::default());
    let link = rects[0];
    assert!(link.height() > 40.0, "{link:?}");
    // The first line's fragment ends at the right; the last starts at the left.
    click(
        &mut harness,
        egui::pos2(link.right() - 4.0, link.top() + 4.0),
        egui::PointerButton::Primary,
    );
    assert_eq!(
        take(&opened),
        [egui::OpenUrl::new_tab("https://example.org/long")]
    );
    click(
        &mut harness,
        egui::pos2(link.left() + 4.0, link.bottom() - 4.0),
        egui::PointerButton::Primary,
    );
    assert_eq!(
        take(&opened),
        [egui::OpenUrl::new_tab("https://example.org/long")]
    );
    // One link is one Tab stop and one accessible link.
    assert_eq!(
        harness.get_all_by_role(egui::accesskit::Role::Link).count(),
        1
    );
}

#[test]
fn disabled_and_inert_presentations_behave_as_declared() {
    let (mut harness, rects, opened) =
        link_harness(SENTENCE, DESKTOP, false, MarkdownSpec::default());
    click(
        &mut harness,
        rects[0].center(),
        egui::PointerButton::Primary,
    );
    harness.key_press(egui::Key::Tab);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(take(&opened).is_empty());
    let node = harness.get_by_role(egui::accesskit::Role::Link);
    assert!(node.accesskit_node().is_disabled());

    // Inert text still lets its links open; the text is not selectable.
    let inert = MarkdownSpec {
        interaction: TextInteraction::Inert,
        ..MarkdownSpec::default()
    };
    let (mut harness, rects, opened) = link_harness(SENTENCE, DESKTOP, true, inert);
    click(
        &mut harness,
        rects[0].center(),
        egui::PointerButton::Primary,
    );
    assert_eq!(take(&opened), [egui::OpenUrl::new_tab(URL)]);
    let context = context();
    let mut output = context.run_ui(screen(DESKTOP, 400.0), |ui| {
        let mut p = PresentationContext::new(
            ui,
            tokens(),
            1.0,
            PresentationScope::new("reply"),
            SemanticUiId::root(),
        );
        p.markdown(ui, "body", ELEMENTS, inert);
        let published = p.finish(ui);
        assert!(
            published
                .text_layouts
                .iter()
                .all(|layout| layout.interaction == TextInteraction::Inert)
        );
    });
    output.textures_delta.clear();
}

#[test]
fn dragging_across_a_paragraph_selects_and_copies_it_links_included() {
    let rect = Rc::new(Cell::new(Rect::NOTHING));
    let observed = rect.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(DESKTOP, 300.0))
        .build_ui(move |ui| {
            crate::install_typography_fonts(ui.ctx());
            let mut p = PresentationContext::new(
                ui,
                tokens(),
                1.0,
                PresentationScope::new("reply"),
                SemanticUiId::root(),
            );
            let response = p.markdown(ui, "body", SENTENCE, MarkdownSpec::default());
            observed.set(response.rect);
            let _ = p.finish(ui);
        });
    harness.run();
    let rect = rect.get();
    // The drag starts on plain text and crosses the link.
    let start = rect.left_top() + egui::vec2(0.5, 4.0);
    let end = rect.right_bottom() - egui::vec2(0.5, 4.0);
    harness.hover_at(start);
    harness.drag_at(start);
    harness.hover_at(end);
    harness.drop_at(end);
    harness.step();
    harness.event(egui::Event::Copy);
    harness.step();
    const PLAIN: &str = "Read the paper on retrieval heads today.";
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(text) if text == PLAIN)),
        "{:?}",
        harness.output().platform_output.commands
    );
}

#[test]
fn hovering_a_link_keeps_the_layout() {
    let (mut harness, rects, _) = link_harness(SENTENCE, DESKTOP, true, MarkdownSpec::default());
    let before = harness
        .get_by_role(egui::accesskit::Role::Link)
        .accesskit_node()
        .data()
        .bounds()
        .unwrap();
    harness.hover_at(rects[0].center());
    harness.run();
    let after = harness
        .get_by_role(egui::accesskit::Role::Link)
        .accesskit_node()
        .data()
        .bounds()
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        harness.output().platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
}

#[test]
fn a_scroll_area_publishes_only_visible_blocks_and_links() {
    let context = context();
    let source = digest();
    let mut output = context.run_ui(screen(PHONE, 600.0), |ui| {
        let mut p = PresentationContext::new(
            ui,
            tokens(),
            1.0,
            PresentationScope::new("conversation"),
            SemanticUiId::root(),
        );
        egui::ScrollArea::vertical()
            .max_height(240.0)
            .show(ui, |ui| {
                p.markdown(ui, "reply", &source, MarkdownSpec::default());
            });
        let published = p.finish(ui);
        assert!(audit_text_layouts(&published.text_layouts).is_empty());
        assert!(published.coverage.attempted_components > published.coverage.measured_components);
        assert!(
            published
                .text_layouts
                .iter()
                .all(|layout| { layout.allocated_rect.min_y < layout.clip_rect.max_y })
        );
        let links = published
            .semantic_nodes
            .iter()
            .filter(|node| node.role == UiRole::Link)
            .count();
        assert!((1..11).contains(&links), "{links}");
    });
    output.textures_delta.clear();
}

#[test]
fn identities_are_stable_across_widths_and_appearances() {
    let context = context();
    let wide = present_with(&context, ELEMENTS, DESKTOP, 1.0, tokens());
    let narrow = present_with(
        &context,
        ELEMENTS,
        PHONE,
        1.5,
        DesignTokens::resolve(ThemeVariant::LightHighContrast, DensityVariant::Compact),
    );
    let ids = |presented: &Presented| {
        (
            presented.rects().into_keys().collect::<Vec<_>>(),
            presented
                .links()
                .iter()
                .map(|link| link.id.clone())
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(ids(&wide), ids(&narrow));
}

#[test]
fn the_text_audit_still_catches_markdown_text_outside_its_allocation() {
    let presented = present(ELEMENTS, PHONE, 1.0);
    let mut broken = presented.layouts.clone();
    let text = broken
        .iter_mut()
        .find(|layout| layout.component_id.kind == TextComponentKind::ReadingText)
        .unwrap();
    text.allocated_rect.max_y = text.allocated_rect.min_y + 4.0;
    assert!(
        audit_text_layouts(&broken)
            .iter()
            .any(|finding| matches!(finding, TextAuditFinding::TextOutsideAllocation { .. }))
    );
}

#[test]
fn links_after_shaped_clusters_keep_their_whole_hit_area_and_label() {
    // Combining marks, a keycap, ZWJ emoji and accented letters shape into
    // clusters whose glyphs do not each report their own character.
    const PREFIXES: [&str; 6] = [
        "plain ",
        "q\u{303}q\u{303}q\u{303} ",
        "1\u{fe0f}\u{20e3} ",
        "\u{1f3f3}\u{fe0f}\u{200d}\u{1f308} ",
        "\u{1f469}\u{200d}\u{1f4bb} a\u{323}\u{308} ",
        "Größere ",
    ];
    let mut widths = Vec::new();
    for prefix in PREFIXES {
        let source: &'static str = Box::leak(
            format!("{prefix}[Bericht über Größen](https://example.org/de) danach.")
                .into_boxed_str(),
        );
        let presented = present(source, DESKTOP, 1.0);
        let links = presented.links();
        assert_eq!(links.len(), 1, "{prefix:?}");
        assert_eq!(links[0].name, "Bericht über Größen", "{prefix:?}");
        widths.push(links[0].rect.max_x - links[0].rect.min_x);

        let (mut harness, rects, opened) =
            link_harness(source, DESKTOP, true, MarkdownSpec::default());
        let link = rects[0];
        // The first and last glyphs open the link; just outside does not.
        for x in [link.left() + 1.0, link.right() - 1.0] {
            click(
                &mut harness,
                egui::pos2(x, link.center().y),
                egui::PointerButton::Primary,
            );
            assert_eq!(
                take(&opened),
                [egui::OpenUrl::new_tab("https://example.org/de")],
                "{prefix:?} at {x}"
            );
        }
        click(
            &mut harness,
            egui::pos2(link.right() + 12.0, link.center().y),
            egui::PointerButton::Primary,
        );
        assert!(take(&opened).is_empty(), "{prefix:?}");
    }
    // The label's hit area is the same width whatever precedes it.
    assert!(
        widths.iter().all(|width| (width - widths[0]).abs() < 0.5),
        "{widths:?}"
    );
}

const CODE_COPY: CodeCopySpec<'static> = CodeCopySpec::new("Copy code", "Copied", "Couldn't copy");

fn copy_controls(presented: &Presented) -> Vec<&UiNode> {
    presented
        .snapshot
        .nodes
        .iter()
        .filter(|node| node.role == UiRole::Button)
        .collect()
}

fn code_copy_id(index: usize) -> SemanticUiId {
    PresentationScope::new("reply")
        .instance((&"body", "stobie.code-copy", index))
        .semantic_id()
}

#[test]
fn a_code_blocks_copy_control_sits_in_a_strip_above_its_text() {
    for width in [PHONE, DESKTOP] {
        for scale in [1.0, 1.5] {
            let label = format!("{width} at {scale}");
            let plain = present(ELEMENTS, width, scale);
            let copying = present_copying(
                &context(),
                ELEMENTS,
                width,
                scale,
                tokens(),
                Some(&CODE_COPY),
            );
            assert_clean(&copying, &label);
            // Only the opted-in presentation has a control.
            assert!(copy_controls(&plain).is_empty(), "{label}");
            let controls = copy_controls(&copying);
            assert_eq!(controls.len(), 1, "{label}");
            let control = controls[0];
            assert_eq!(control.name, "Copy code");
            assert_eq!(control.id, code_copy_id(0));
            assert!(control.enabled);

            // Every block keeps its place, except that the code and what
            // follows it move down by the strip, less the padding it
            // replaces.
            let tokens = tokens();
            let strip = crate::components::icon_action_side(&tokens, scale);
            let shift = strip - tokens.spacing.block.0;
            let reading = |presented: &Presented| -> Vec<(TextRole, Rect)> {
                presented
                    .kind(TextComponentKind::ReadingText)
                    .iter()
                    .map(|text| (text.role, text_rect(text.allocated_rect)))
                    .collect()
            };
            let (before, after) = (reading(&plain), reading(&copying));
            assert_eq!(before.len(), after.len(), "{label}");
            let code = before
                .iter()
                .position(|(role, _)| *role == TextRole::MonospaceTechnical)
                .unwrap();
            for (index, ((_, was), (_, now))) in before.iter().zip(&after).enumerate() {
                let expected = if index < code {
                    *was
                } else {
                    was.translate(egui::vec2(0.0, shift))
                };
                assert!(
                    (now.min - expected.min).length() < 0.5
                        && (now.size() - expected.size()).length() < 0.5,
                    "{label}: block {index} {now:?} {expected:?}"
                );
            }
            assert!(
                (copying.rect.height() - plain.rect.height() - shift).abs() < 0.5,
                "{label}"
            );
            // The control is at the strip's trailing end, above the code,
            // and at least the touch target.
            let text = after[code].1;
            let control = Rect::from(control.rect);
            let minimum = tokens.geometry.minimum_hit_size.0;
            assert!(control.width() >= minimum && control.height() >= minimum - 0.5);
            assert!(control.bottom() <= text.top() + 0.5, "{label}: {control:?}");
            assert!(control.right() <= width + 0.5, "{label}: {control:?}");
            assert!(
                control.right() >= text.right(),
                "{label}: {control:?} {text:?}"
            );
            assert!(control.left() > text.left(), "{label}: {control:?}");
        }
    }
}

#[test]
fn several_code_blocks_have_distinct_names_keyed_by_their_position() {
    let one = "Install:\n\n```sh\npip install example\n```\n";
    let presented = present_copying(&context(), one, PHONE, 1.0, tokens(), Some(&CODE_COPY));
    let names: Vec<_> = copy_controls(&presented)
        .iter()
        .map(|node| (node.id.clone(), node.name.clone()))
        .collect();
    assert_eq!(names, [(code_copy_id(0), "Copy code".to_owned())]);

    // As the reply grows, the first control keeps its key and its name;
    // code in a list item and in a quote has its own control too.
    let three = format!(
        "{one}\nThen:\n\n- Run it:\n\n  ```\n  example --check\n  ```\n\n\
         > ```\n> quoted\n> ```\n"
    );
    let presented = present_copying(&context(), &three, PHONE, 1.0, tokens(), Some(&CODE_COPY));
    assert_clean(&presented, "three blocks");
    let names: Vec<_> = copy_controls(&presented)
        .iter()
        .map(|node| (node.id.clone(), node.name.clone()))
        .collect();
    assert_eq!(
        names,
        [
            (code_copy_id(0), "Copy code".to_owned()),
            (code_copy_id(1), "Copy code 2".to_owned()),
            (code_copy_id(2), "Copy code 3".to_owned()),
        ]
    );
    // Each control is above its own block's text.
    let code: Vec<_> = presented
        .kind(TextComponentKind::ReadingText)
        .into_iter()
        .filter(|text| text.role == TextRole::MonospaceTechnical)
        .map(|text| text_rect(text.allocated_rect))
        .collect();
    assert_eq!(code.len(), 3);
    for (control, text) in copy_controls(&presented).iter().zip(&code) {
        let control = Rect::from(control.rect);
        assert!(control.bottom() <= text.top() + 0.5, "{control:?} {text:?}");
        assert!(text.top() - control.bottom() < 1.0, "{control:?} {text:?}");
    }
}

fn text_rect(rect: crate::TextRect) -> Rect {
    Rect::from_min_max(
        egui::pos2(rect.min_x, rect.min_y),
        egui::pos2(rect.max_x, rect.max_y),
    )
}

#[test]
fn a_code_block_shows_a_tab_as_four_spaces() {
    // Copy keeps the tab; the block lays it out as four spaces.
    let code = |source: &str| {
        let presented = present(source, PHONE, 1.0);
        let text = presented
            .layouts
            .iter()
            .find(|text| text.role == TextRole::MonospaceTechnical)
            .cloned()
            .expect("a code block");
        (text.painted_rect, text.line_count)
    };
    assert_eq!(
        code("```\nfor x in xs:\n\tprint(x)\n```"),
        code("```\nfor x in xs:\n    print(x)\n```")
    );
}

/// A long document using every extension a knowledge repository enables,
/// with CRLF line endings and multibyte text.
const EXTENDED: &str = "# Café 漢字\r\n\r\n\
A paragraph with ~~struck~~ text, $x^2$ maths and a note[^n] 🙂.\r\n\r\n\
- [x] done ✓\r\n- [ ] öffen\r\n\r\n\
| Name | Wert |\r\n| :--- | ---: |\r\n| α | **1** |\r\n\r\n\
$$\r\nE = mc^2\r\n$$\r\n\r\n\
> Quoted 🙂\r\n\r\n\
```text\r\ncode ü\r\n```\r\n\r\n\
<div>raw ü</div>\r\n\r\n\
***\r\n\r\n\
[^n]: The note, ünïcode.\r\n";

const DOCUMENT: MarkdownOptions = MarkdownOptions::ENABLE_TABLES
    .union(MarkdownOptions::ENABLE_FOOTNOTES)
    .union(MarkdownOptions::ENABLE_STRIKETHROUGH)
    .union(MarkdownOptions::ENABLE_TASKLISTS)
    .union(MarkdownOptions::ENABLE_MATH);

#[test]
fn the_default_options_are_strikethrough_only() {
    assert_eq!(
        DEFAULT_MARKDOWN_OPTIONS,
        MarkdownOptions::ENABLE_STRIKETHROUGH
    );
    assert_eq!(MarkdownSpec::default().options, DEFAULT_MARKDOWN_OPTIONS);
}

#[test]
fn the_source_map_names_each_presented_text_block() {
    let spec = MarkdownSpec {
        options: DOCUMENT,
        ..MarkdownSpec::default()
    };
    let map = crate::markdown_source_map(EXTENDED, DOCUMENT, 0);
    let texts: Vec<_> = map
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                block.kind,
                MarkdownBlockKind::Paragraph
                    | MarkdownBlockKind::Heading
                    | MarkdownBlockKind::CodeBlock
                    | MarkdownBlockKind::HtmlBlock
                    | MarkdownBlockKind::MetadataBlock
            )
        })
        .collect();
    // The one table presents each of its four cells as text of its own.
    assert_eq!(
        map.blocks
            .iter()
            .filter(|block| block.kind == MarkdownBlockKind::Table)
            .count(),
        1
    );
    for width in [PHONE, DESKTOP] {
        for scale in [1.0, 1.5] {
            let label = format!("{width} at {scale}");
            let presented = present_spec(&context(), EXTENDED, width, scale, tokens(), None, spec);
            assert_clean(&presented, &label);
            let reading = presented.kind(TextComponentKind::ReadingText);
            assert_eq!(reading.len(), texts.len() + 4, "{label}: {texts:#?}");
            let lines: Vec<&str> = reading
                .iter()
                .map(|text| text.content_text().unwrap_or_default())
                .collect();
            for cell in ["Name", "Wert", "α", "1"] {
                assert!(lines.contains(&cell), "{label}: {lines:#?}");
            }
            assert!(lines.contains(&"The note, ünïcode."), "{label}");
        }
    }
    // The default options present the same source without the extensions.
    let presented = present(EXTENDED, DESKTOP, 1.0);
    assert_clean(&presented, "default");
    assert_eq!(
        presented.kind(TextComponentKind::ReadingText).len(),
        crate::markdown_source_map(EXTENDED, DEFAULT_MARKDOWN_OPTIONS, 0)
            .blocks
            .iter()
            .filter(|block| !matches!(
                block.kind,
                MarkdownBlockKind::List
                    | MarkdownBlockKind::ListItem
                    | MarkdownBlockKind::BlockQuote
                    | MarkdownBlockKind::Rule
            ))
            .count()
    );
}

/// Headings at the top level, in a quote and in a list item, one of them
/// with multibyte text.
const OUTLINE: &str = "# Field notes\n\nIntro.\n\n## Méthode et matériel\n\n\
> ### Quoted heading\n\n- ### Listed heading\n  In an item.\n\n## Results\n\n\
### Unnamed\n\n## Past the end\n";

/// The application's IDs for `OUTLINE`'s headings: the sixth is left empty
/// and the seventh has none.
fn outline_ids() -> Vec<SemanticUiId> {
    [
        "notes.field-notes",
        "notes.methode",
        "notes.quoted",
        "notes.listed",
        "notes.results",
        "",
    ]
    .into_iter()
    .map(SemanticUiId::new)
    .collect()
}

#[test]
fn headings_carry_the_applications_ids_as_heading_nodes() {
    let ids = outline_ids();
    let spec = MarkdownSpec {
        heading_ids: &ids,
        ..MarkdownSpec::default()
    };
    let map = crate::markdown_source_map(OUTLINE, spec.options, 0);
    assert_eq!(map.headings.len(), 7);
    let mut identities = Vec::new();
    for width in [PHONE, DESKTOP] {
        for scale in [1.0, 1.5] {
            let label = format!("{width} at {scale}");
            let presented = present_spec(&context(), OUTLINE, width, scale, tokens(), None, spec);
            assert_clean(&presented, &label);
            let headings: Vec<_> = presented
                .snapshot
                .nodes
                .iter()
                .filter(|node| node.role == UiRole::Heading)
                .collect();
            // Each named heading, in document order, named by its text.
            assert_eq!(
                headings
                    .iter()
                    .map(|node| (node.id.0.as_str(), node.name.as_str()))
                    .collect::<Vec<_>>(),
                ids[..5]
                    .iter()
                    .zip(&map.headings)
                    .map(|(id, heading)| (id.0.as_str(), heading.text.as_str()))
                    .collect::<Vec<_>>(),
                "{label}"
            );
            for (node, heading) in headings.iter().zip(&map.headings) {
                assert_eq!(node.parent, Some(SemanticUiId::root()), "{label}");
                assert!(node.enabled && !node.focused, "{label}");
                let accessible = presented
                    .update
                    .nodes
                    .iter()
                    .map(|(_, accessible)| accessible)
                    .find(|accessible| accessible.author_id() == Some(node.id.0.as_str()))
                    .unwrap_or_else(|| panic!("{label}: {} in the AccessKit tree", node.id.0));
                assert_eq!(accessible.role(), egui::accesskit::Role::Heading);
                assert_eq!(accessible.level(), Some(usize::from(heading.level)));
            }
            // Headings without an ID keep the recipe's own identity: still
            // AccessKit headings, with no node.
            let unnamed = presented
                .update
                .nodes
                .iter()
                .filter(|(_, accessible)| {
                    accessible.role() == egui::accesskit::Role::Heading
                        && !accessible
                            .author_id()
                            .is_some_and(|author| author.starts_with("notes."))
                })
                .count();
            assert_eq!(unnamed, 2, "{label}");
            identities.push(presented.rects().into_keys().collect::<Vec<_>>());
        }
    }
    // A named heading keeps its identity at every width and text size.
    assert!(identities.windows(2).all(|pair| pair[0] == pair[1]));
    // By default no heading publishes a node.
    let presented = present(OUTLINE, PHONE, 1.0);
    assert_clean(&presented, "default");
    assert!(
        presented
            .snapshot
            .nodes
            .iter()
            .all(|node| node.role != UiRole::Heading)
    );
}

#[test]
fn identities_do_not_depend_on_an_item_flattening_at_narrow_widths() {
    // At 200 points the deeper levels are too narrow to indent, so the
    // deepest item, which opens with code, is led by its marker on a line of
    // its own.
    let mut source: String = (0..6)
        .map(|level| format!("{}- level {level}\n", "  ".repeat(level)))
        .collect();
    let indent = "  ".repeat(6);
    source.push_str(&format!(
        "{indent}- ```\n{indent}  code\n{indent}  ```\n\n\
         {indent}  ### Deep heading\n\n{indent}  After.\n"
    ));
    let source = source.as_str();
    let map = crate::markdown_source_map(source, DEFAULT_MARKDOWN_OPTIONS, 0);
    assert_eq!(map.headings.len(), 1);
    let ids = [SemanticUiId::new("notes.deep")];
    for heading_ids in [&[][..], &ids[..]] {
        let spec = MarkdownSpec {
            heading_ids,
            ..MarkdownSpec::default()
        };
        let wide = present_spec(&context(), source, DESKTOP, 1.5, tokens(), None, spec);
        let narrow = present_spec(&context(), source, 200.0, 1.5, tokens(), None, spec);
        assert_clean(&wide, "wide");
        assert_clean(&narrow, "narrow");
        let wide_ids: Vec<_> = wide.rects().into_keys().collect();
        let narrow_ids: Vec<_> = narrow.rects().into_keys().collect();
        // The narrow layout adds the marker's line and keeps every block.
        assert_eq!(narrow_ids.len(), wide_ids.len() + 1);
        assert!(wide_ids.iter().all(|id| narrow_ids.contains(id)));
        let marker = narrow
            .kind(TextComponentKind::ReadingText)
            .into_iter()
            .filter_map(|text| text.content_text())
            .any(|text| text == "•");
        assert!(marker, "the flattened item's marker is presented");
        let headings = |presented: &Presented| {
            presented
                .snapshot
                .nodes
                .iter()
                .filter(|node| node.role == UiRole::Heading)
                .map(|node| node.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(headings(&wide), heading_ids);
        assert_eq!(headings(&narrow), heading_ids);
    }
}

/// Task lists, footnotes and maths, with a task whose text pulldown-cmark
/// gives outside its item, a definition opening with a list, and display
/// maths with CRLF line endings.
const NOTES: &str = "## Reading list\r\n\r\n\
- [x] Read the *survey* chapter\r\n\
- [ ] Draft the summary[^draft]\r\n\r\n\
1. [x] Numbered and done\r\n\
2. [ ]\r\n\r\n   \
Its text after a blank line.\r\n\r\n\
The energy is $E = mc^2$ inline, as noted[^Draft].\r\n\r\n\
$$\r\n\\int_0^1 x\\,dx = \\frac{1}{2}\r\n$$\r\n\r\n\
> - [x] Quoted and done\r\n\r\n\
[^draft]: - A definition opening with a list.\r\n    \
- Its second item.\r\n";

fn notes_spec(footnote_ids: &[SemanticUiId]) -> MarkdownSpec<'_> {
    MarkdownSpec {
        options: DOCUMENT,
        footnote_ids,
        ..MarkdownSpec::default()
    }
}

fn nodes_of(presented: &Presented, role: UiRole) -> Vec<&UiNode> {
    presented
        .snapshot
        .nodes
        .iter()
        .filter(|node| node.role == role)
        .collect()
}

fn accessible<'a>(presented: &'a Presented, id: &SemanticUiId) -> &'a egui::accesskit::Node {
    presented
        .update
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| node.author_id() == Some(id.0.as_str()))
        .unwrap_or_else(|| panic!("{} in the AccessKit tree", id.0))
}

#[test]
fn task_lists_footnotes_and_maths_present_at_every_width_size_and_contrast() {
    let ids = [SemanticUiId::new("notes.footnote.draft")];
    let map = crate::markdown_source_map(NOTES, DOCUMENT, 0);
    assert_eq!(map.footnotes.len(), 1);
    let mut identities = Vec::new();
    for theme in [ThemeVariant::Dark, ThemeVariant::LightHighContrast] {
        let tokens = DesignTokens::resolve(theme, DensityVariant::Comfortable);
        for width in [PHONE, DESKTOP] {
            for scale in [1.0, 1.5] {
                let label = format!("{theme:?} {width} at {scale}");
                let presented = present_spec(
                    &context(),
                    NOTES,
                    width,
                    scale,
                    tokens,
                    None,
                    notes_spec(&ids),
                );
                assert_clean(&presented, &label);
                // Each task's read-only box, named by its text, in order.
                let tasks = nodes_of(&presented, UiRole::Checkbox);
                assert_eq!(
                    tasks
                        .iter()
                        .map(|node| (node.name.as_str(), node.checked, node.enabled))
                        .collect::<Vec<_>>(),
                    [
                        ("Read the survey chapter", Some(true), false),
                        ("Draft the summary", Some(false), false),
                        ("Numbered and done", Some(true), false),
                        ("Task", Some(false), false),
                        ("Quoted and done", Some(true), false),
                    ],
                    "{label}"
                );
                for node in &tasks {
                    let box_ = accessible(&presented, &node.id);
                    assert_eq!(box_.role(), egui::accesskit::Role::CheckBox, "{label}");
                    assert!(box_.is_disabled(), "{label}");
                    assert!(!box_.supports_action(egui::accesskit::Action::Click));
                    assert_eq!(box_.description(), None, "{label}");
                }
                // No task's marker is left as text.
                let lines: Vec<&str> = presented
                    .kind(TextComponentKind::ReadingText)
                    .into_iter()
                    .filter_map(|text| text.content_text())
                    .collect();
                assert!(
                    lines
                        .iter()
                        .all(|line| !line.contains("[ ]") && !line.contains("[x]")),
                    "{label}: {lines:#?}"
                );
                // Maths keeps its source, without delimiters or CR.
                assert!(
                    lines.contains(&"\\int_0^1 x\\,dx = \\frac{1}{2}"),
                    "{label}: {lines:#?}"
                );
                assert!(
                    lines.contains(&"The energy is E = mc^2 inline, as noted[1]."),
                    "{label}: {lines:#?}"
                );
                assert!(lines.iter().all(|line| !line.contains('\r')), "{label}");
                // Both references link to the one definition.
                let references: Vec<_> = presented
                    .links()
                    .into_iter()
                    .filter(|node| node.name == "[1]")
                    .collect();
                assert_eq!(references.len(), 2, "{label}");
                for reference in references {
                    assert!(reference.enabled, "{label}");
                    assert_eq!(reference.description, None, "{label}");
                    let accessible = accessible(&presented, &reference.id);
                    assert!(accessible.supports_action(egui::accesskit::Action::Click));
                }
                // The definition is a section with the application's ID,
                // below every reference.
                let sections: Vec<_> = nodes_of(&presented, UiRole::Section)
                    .into_iter()
                    .filter(|node| node.id == ids[0])
                    .collect();
                assert_eq!(sections.len(), 1, "{label}");
                assert_eq!(sections[0].name, "[1]");
                let marker = presented
                    .kind(TextComponentKind::ListMarker)
                    .into_iter()
                    .any(|text| text.content_text() == Some("[1]"));
                assert!(marker, "{label}: the definition's number is presented");
                identities.push(presented.rects().into_keys().collect::<Vec<_>>());
            }
        }
    }
    // Every block keeps its identity at every width, size and contrast.
    assert!(identities.windows(2).all(|pair| pair[0] == pair[1]));
    // At desktop width, each box sits in the marker column, left of its
    // text and within its first line.
    let presented = present_spec(
        &context(),
        NOTES,
        DESKTOP,
        1.0,
        tokens(),
        None,
        notes_spec(&ids),
    );
    let first = nodes_of(&presented, UiRole::Checkbox)[0].rect;
    let text = presented
        .kind(TextComponentKind::ReadingText)
        .into_iter()
        .find(|text| text.content_text() == Some("Read the survey chapter"))
        .expect("the first task's text")
        .allocated_rect;
    assert!(first.max_x <= text.min_x, "{first:?} {text:?}");
    assert!(
        first.min_y >= text.min_y && first.max_y <= text.max_y,
        "{first:?} {text:?}"
    );
}

#[test]
fn the_default_options_keep_task_footnote_and_maths_source() {
    let presented = present(NOTES, DESKTOP, 1.0);
    assert_clean(&presented, "default");
    assert!(nodes_of(&presented, UiRole::Checkbox).is_empty());
    let lines: Vec<&str> = presented
        .kind(TextComponentKind::ReadingText)
        .into_iter()
        .filter_map(|text| text.content_text())
        .collect();
    assert!(lines.contains(&"[x] Read the survey chapter"), "{lines:#?}");
    assert!(
        lines.contains(&"The energy is $E = mc^2$ inline, as noted[^Draft]."),
        "{lines:#?}"
    );
}

type Published = Rc<RefCell<Vec<UiNode>>>;

/// A harness presenting a reference far above its footnote's definition
/// in a scroll area 400 points high.
fn footnote_harness() -> (Harness<'static>, Published, SemanticUiId) {
    let source: &'static str = Box::leak(
        format!(
            "Start with the note[^far].\n\n{}[^far]: The far note.\n",
            "A paragraph to scroll past.\n\n".repeat(40)
        )
        .into_boxed_str(),
    );
    let ids: &'static [SemanticUiId] = Box::leak(Box::new([SemanticUiId::new("notes.far")]));
    let nodes = Published::default();
    let recorded = nodes.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(DESKTOP, 400.0))
        .build_ui(move |ui| {
            crate::install_typography_fonts(ui.ctx());
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut p = PresentationContext::new(
                    ui,
                    tokens(),
                    1.0,
                    PresentationScope::new("reply"),
                    SemanticUiId::root(),
                );
                p.markdown(ui, "body", source, notes_spec(ids));
                assert!(p.followed_link().is_none(), "a footnote is not reported");
                *recorded.borrow_mut() = p.finish(ui).semantic_nodes;
            });
        });
    harness.run();
    (harness, nodes, ids[0].clone())
}

#[test]
fn a_footnote_reference_scrolls_its_definition_into_view() {
    let shown =
        |nodes: &Published, id: &SemanticUiId| nodes.borrow().iter().any(|node| node.id == *id);
    // By pointer.
    let (mut harness, nodes, definition) = footnote_harness();
    assert!(
        !shown(&nodes, &definition),
        "the definition starts below the fold"
    );
    let reference = nodes
        .borrow()
        .iter()
        .find(|node| node.role == UiRole::Link)
        .cloned()
        .expect("the reference");
    assert_eq!(reference.name, "[1]");
    assert_eq!(reference.value.as_deref(), Some("#far"));
    let centre = egui::pos2(
        (reference.rect.min_x + reference.rect.max_x) * 0.5,
        (reference.rect.min_y + reference.rect.max_y) * 0.5,
    );
    click(&mut harness, centre, egui::PointerButton::Primary);
    for _ in 0..4 {
        harness.run();
    }
    assert!(shown(&nodes, &definition), "the definition is in view");
    // By keyboard: Tab to the reference, then Enter.
    let (mut harness, nodes, definition) = footnote_harness();
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert!(
        nodes
            .borrow()
            .iter()
            .any(|node| node.name == "[1]" && node.focused)
    );
    harness.key_press(egui::Key::Enter);
    for _ in 0..4 {
        harness.run();
    }
    assert!(shown(&nodes, &definition), "the definition is in view");
}

#[test]
fn a_named_heading_opening_a_narrow_item_is_named_by_its_own_text() {
    let mut source: String = (0..6)
        .map(|level| format!("{}- level {level}\n", "  ".repeat(level)))
        .collect();
    source.push_str(&format!("{}- ### Deep heading\n", "  ".repeat(6)));
    let ids = [SemanticUiId::new("notes.deep")];
    let spec = MarkdownSpec {
        heading_ids: &ids,
        ..MarkdownSpec::default()
    };
    for width in [200.0, DESKTOP] {
        let presented = present_spec(&context(), &source, width, 1.5, tokens(), None, spec);
        assert_clean(&presented, &format!("{width}"));
        let headings = nodes_of(&presented, UiRole::Heading);
        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].name, "Deep heading", "{width}");
        // Its AccessKit label and value too are its own text, without the
        // marker.
        let heading = accessible(&presented, &ids[0]);
        assert_eq!(heading.value(), Some("Deep heading"), "{width}");
        assert!(
            heading.label().is_none_or(|label| label == "Deep heading"),
            "{width}: {:?}",
            heading.label()
        );
    }
    // At 200 points the item is too narrow to indent: its marker leads the
    // heading's text but not its name.
    let narrow = present_spec(&context(), &source, 200.0, 1.5, tokens(), None, spec);
    // Its text runs hold the marker, from which assistive technology would
    // name a heading without a label; its label names it by its own text.
    let heading = accessible(&narrow, &ids[0]);
    assert_eq!(heading.label(), Some("Deep heading"));
    let runs: String = heading
        .children()
        .iter()
        .filter_map(|child| narrow.update.nodes.iter().find(|(id, _)| id == child))
        .filter_map(|(_, node)| node.value())
        .collect();
    assert!(runs.starts_with("• Deep"), "{runs:?}");
    assert!(
        narrow
            .kind(TextComponentKind::ReadingText)
            .into_iter()
            .any(|text| text.content_text() == Some("• Deep heading"))
    );
}

#[derive(
    crate::Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize,
)]
#[action(prefix = "notes")]
enum Act {
    #[action(label = "Close", description = "Close the notes")]
    Close,
}

#[test]
fn a_followed_link_reaches_the_root_from_every_nested_context() {
    const LINKED: &str = "Read [Setting up](guide.md#setting-up) first.";
    const IN_APPLICATION: [crate::MarkdownLinkKind<'static>; 1] =
        [crate::MarkdownLinkKind::InApplication];
    let spec = MarkdownSpec {
        link_kinds: &IN_APPLICATION,
        ..MarkdownSpec::default()
    };
    for context_kind in ["scoped", "bubble", "dialog", "code copy"] {
        let rects = Rc::new(RefCell::new(Vec::<Rect>::new()));
        let followed = Followed::default();
        let (observed, follows) = (rects.clone(), followed.clone());
        let mut harness = Harness::builder()
            .with_size(egui::vec2(DESKTOP, 600.0))
            .build_ui(move |ui| {
                crate::install_typography_fonts(ui.ctx());
                let mut p = PresentationContext::new(
                    ui,
                    tokens(),
                    1.0,
                    PresentationScope::new("reply"),
                    SemanticUiId::root(),
                );
                match context_kind {
                    "scoped" => p.scoped(ui, "item", |ui, p| {
                        p.markdown(ui, "body", LINKED, spec);
                    }),
                    "bubble" => p.message_bubble(
                        ui,
                        ("message", 1),
                        crate::MessageBubbleSpec {
                            side: crate::MessageSide::Other,
                            sender: "Ada",
                        },
                        |ui, p| {
                            p.markdown(ui, "body", LINKED, spec);
                        },
                    ),
                    "dialog" => {
                        let dialog = crate::DialogSpec::new("Notes", Act::Close);
                        p.dialog(
                            ui,
                            "notes",
                            &dialog,
                            |_, _| crate::Availability::Enabled,
                            |ui, p| {
                                p.markdown(ui, "body", LINKED, spec);
                            },
                        );
                    }
                    _ => {
                        p.markdown_with_code_copy(ui, "body", LINKED, spec, &CODE_COPY);
                    }
                }
                follows.borrow_mut().extend(p.followed_link().cloned());
                *observed.borrow_mut() = p
                    .finish(ui)
                    .semantic_nodes
                    .iter()
                    .filter(|node| node.role == UiRole::Link)
                    .map(|node| {
                        Rect::from_min_max(
                            egui::pos2(node.rect.min_x, node.rect.min_y),
                            egui::pos2(node.rect.max_x, node.rect.max_y),
                        )
                    })
                    .collect();
            });
        harness.run();
        harness.run();
        let link = rects.borrow().first().copied().expect(context_kind);
        click(&mut harness, link.center(), egui::PointerButton::Primary);
        assert_eq!(
            taken(&followed),
            [(0, "guide.md#setting-up".to_owned())],
            "{context_kind}"
        );
    }
}

/// A wide table, its columns aligned start, centre, end and end, with a
/// link and wrapping notes, and a narrow one.
const TABLES: &str = "| Pool | Tide | Area (m²) | Depth (m) | Temperature (°C) | Species | Notes |\r\n\
| :--- | :---: | ---: | ---: | ---: | --- | --- |\r\n\
| North shelf | Low | 12.5 | 0.4 | 14 | Limpets, chitons | Sheltered by a basalt ledge; see the [survey](#survey) |\r\n\
| South gully | High | 3 | 1.2 | 15 | Anemones, nudibranchs | Surge at the mouth |\r\n\
| Outer reef | | 140 | 2.6 | 12 | Kelp | |\r\n\r\n\
| Name | Count |\r\n| --- | ---: |\r\n| Crab | 4 |\r\n";

fn tables_spec() -> MarkdownSpec<'static> {
    MarkdownSpec {
        options: DOCUMENT,
        ..MarkdownSpec::default()
    }
}

/// The AccessKit tree's nodes by ID.
fn accesskit_nodes(
    presented: &Presented,
) -> BTreeMap<egui::accesskit::NodeId, &egui::accesskit::Node> {
    presented
        .update
        .nodes
        .iter()
        .map(|(id, node)| (*id, node))
        .collect()
}

/// A cell's role, row index and column index.
type AccessibleCell = (egui::accesskit::Role, Option<usize>, Option<usize>);

/// A presented table as assistive technology sees it: its row and column
/// counts, and each row's index and cells as (role, row, column).
#[derive(Debug, PartialEq)]
struct AccessibleTable {
    rows: Option<usize>,
    columns: Option<usize>,
    cells: Vec<(Option<usize>, Vec<AccessibleCell>)>,
    top: f64,
}

fn accessible_tables(presented: &Presented) -> Vec<AccessibleTable> {
    use egui::accesskit::Role;
    let nodes = accesskit_nodes(presented);
    let mut tables: Vec<_> = nodes
        .values()
        .filter(|node| node.role() == Role::Table)
        .map(|table| AccessibleTable {
            rows: table.row_count(),
            columns: table.column_count(),
            cells: table
                .children()
                .iter()
                .map(|id| nodes[id])
                .filter(|row| row.role() == Role::Row)
                .map(|row| {
                    let cells = row
                        .children()
                        .iter()
                        .map(|id| nodes[id])
                        .map(|cell| (cell.role(), cell.row_index(), cell.column_index()))
                        .collect();
                    (row.row_index(), cells)
                })
                .collect(),
            top: table.bounds().map_or(0.0, |bounds| bounds.y0),
        })
        .collect();
    tables.sort_by(|a, b| a.top.total_cmp(&b.top));
    tables
}

fn reading_text<'a>(presented: &'a Presented, text: &str) -> &'a TextLayoutObservation {
    presented
        .kind(TextComponentKind::ReadingText)
        .into_iter()
        .find(|layout| layout.content_text() == Some(text))
        .unwrap_or_else(|| panic!("{text:?} is presented"))
}

#[test]
fn tables_present_as_a_grid_or_stacked_at_every_width_size_and_contrast() {
    use egui::accesskit::Role::{Cell, ColumnHeader};
    for theme in [ThemeVariant::Dark, ThemeVariant::LightHighContrast] {
        let tokens = DesignTokens::resolve(theme, DensityVariant::Comfortable);
        for scale in [1.0, 1.5] {
            let mut identities = Vec::new();
            for width in [DESKTOP, PHONE] {
                let label = format!("{theme:?} {width} at {scale}");
                let presented = present_spec(
                    &context(),
                    TABLES,
                    width,
                    scale,
                    tokens,
                    None,
                    tables_spec(),
                );
                assert_clean(&presented, &label);
                let tables = accessible_tables(&presented);
                assert_eq!(tables.len(), 2, "{label}: {tables:#?}");
                let lines: Vec<&str> = presented
                    .kind(TextComponentKind::ReadingText)
                    .into_iter()
                    .filter_map(|text| text.content_text())
                    .collect();
                for text in presented.kind(TextComponentKind::ReadingText) {
                    assert_eq!(text.interaction, TextInteraction::Selectable);
                    assert!(!text.truncated, "{label}");
                    assert!(text.allocated_rect.max_x <= width + 0.5, "{label}");
                }
                // The narrow table is a grid at every width: a header row of
                // column headers, then its body row of cells.
                assert_eq!(
                    tables[1],
                    AccessibleTable {
                        rows: Some(2),
                        columns: Some(2),
                        cells: vec![
                            (
                                Some(0),
                                vec![
                                    (ColumnHeader, Some(0), Some(0)),
                                    (ColumnHeader, Some(0), Some(1))
                                ]
                            ),
                            (
                                Some(1),
                                vec![(Cell, Some(1), Some(0)), (Cell, Some(1), Some(1))]
                            ),
                        ],
                        top: tables[1].top,
                    },
                    "{label}"
                );
                // The link in a cell is presented once, whatever the layout.
                let links = presented.links();
                assert_eq!(links.len(), 1, "{label}");
                assert_eq!(links[0].name, "survey");
                let wide = &tables[0];
                assert_eq!(wide.columns, Some(7), "{label}");
                if width == DESKTOP {
                    // A grid: the header row, then three rows; the empty
                    // cells are cells without text.
                    assert_eq!(wide.rows, Some(4), "{label}");
                    assert_eq!(wide.cells.len(), 4, "{label}");
                    for (index, (row, cells)) in wide.cells.iter().enumerate() {
                        assert_eq!(*row, Some(index), "{label}");
                        let role = if index == 0 { ColumnHeader } else { Cell };
                        let expected: Vec<_> = (0..7)
                            .map(|column| (role, Some(index), Some(column)))
                            .collect();
                        assert_eq!(cells, &expected, "{label}");
                    }
                    for cell in ["Pool", "North shelf", "12.5", "Low", "Kelp"] {
                        assert!(lines.contains(&cell), "{label}: {lines:#?}");
                    }
                    // Each column's text is placed as the column is aligned.
                    let rect = |text: &str| reading_text(&presented, text).allocated_rect;
                    let (low, high) = (rect("Low"), rect("High"));
                    let centre = |rect: crate::TextRect| (rect.min_x + rect.max_x) * 0.5;
                    assert!((centre(low) - centre(high)).abs() < 0.5, "{label}");
                    assert!(low.min_x > rect("Tide").min_x - 0.5, "{label}");
                    let (wide_area, narrow_area) = (rect("12.5"), rect("3"));
                    assert!((wide_area.max_x - narrow_area.max_x).abs() < 0.5, "{label}");
                    assert!(narrow_area.min_x > wide_area.min_x, "{label}");
                    assert!((rect("North shelf").min_x - rect("Pool").min_x).abs() < 0.5);
                    // Rows stack in order, each cell's text below the last.
                    assert!(rect("North shelf").min_y > rect("Pool").max_y, "{label}");
                    assert!(rect("South gully").min_y > rect("North shelf").max_y);
                } else {
                    // Stacked: each body row a record, its cells led by
                    // their headers, its empty cells left out.
                    assert_eq!(wide.rows, Some(3), "{label}");
                    let records: Vec<_> = wide
                        .cells
                        .iter()
                        .map(|(row, cells)| {
                            (
                                *row,
                                cells
                                    .iter()
                                    .map(|(role, _, column)| (*role, *column))
                                    .collect::<Vec<_>>(),
                            )
                        })
                        .collect();
                    let all = (0..7)
                        .map(|column| (Cell, Some(column)))
                        .collect::<Vec<_>>();
                    let sparse = [0, 2, 3, 4, 5].map(|column| (Cell, Some(column))).to_vec();
                    assert_eq!(
                        records,
                        [(Some(0), all.clone()), (Some(1), all), (Some(2), sparse)],
                        "{label}"
                    );
                    for line in [
                        "Pool: North shelf",
                        "Tide: Low",
                        "Area (m²): 140",
                        "Species: Kelp",
                    ] {
                        assert!(lines.contains(&line), "{label}: {lines:#?}");
                    }
                    assert!(!lines.contains(&"Pool"), "{label}: no header row");
                    assert!(
                        lines.contains(&"Notes: Sheltered by a basalt ledge; see the survey"),
                        "{label}: {lines:#?}"
                    );
                    // A record's lines start together, one below another.
                    let (pool, tide) = (
                        reading_text(&presented, "Pool: North shelf").allocated_rect,
                        reading_text(&presented, "Tide: Low").allocated_rect,
                    );
                    assert!((pool.min_x - tide.min_x).abs() < 0.5, "{label}");
                    assert!(tide.min_y >= pool.max_y - 0.5, "{label}");
                }
                identities.push(
                    presented
                        .rects()
                        .into_keys()
                        .collect::<std::collections::BTreeSet<_>>(),
                );
            }
            // A stacked cell keeps the identity it has in the grid.
            assert!(
                identities[1].is_subset(&identities[0]),
                "{theme:?} at {scale}"
            );
        }
    }
}

#[test]
fn a_table_cell_wraps_within_its_column() {
    let presented = present_spec(
        &context(),
        TABLES,
        600.0,
        1.0,
        tokens(),
        None,
        tables_spec(),
    );
    assert_clean(&presented, "600");
    let tables = accessible_tables(&presented);
    assert_eq!(tables[0].rows, Some(4), "a grid at 600: {tables:#?}");
    let notes = reading_text(&presented, "Sheltered by a basalt ledge; see the survey");
    assert!(notes.line_count > 1, "{notes:#?}");
    assert!(notes.allocated_rect.max_x <= 600.5);
    // The row grows to its tallest cell: the next row starts below it.
    let next = reading_text(&presented, "South gully").allocated_rect;
    assert!(
        next.min_y > notes.allocated_rect.max_y,
        "{next:?} {notes:#?}"
    );
}

/// Tables in a quote and in a list item, and one with no body rows.
const NESTED_TABLES: &str = "> | Quoted | Table |\n> | --- | --- |\n> | q1 | q2 |\n\n\
- An item\n\n  | Listed | Table |\n  | --- | --- |\n  | l1 | l2 |\n\n\
| Only | Header |\n| --- | --- |\n";

#[test]
fn a_table_presents_in_a_quote_in_a_list_item_and_with_no_body_rows() {
    for scale in [1.0, 1.5] {
        for width in [DESKTOP, PHONE] {
            let label = format!("{width} at {scale}");
            let presented = present_spec(
                &context(),
                NESTED_TABLES,
                width,
                scale,
                tokens(),
                None,
                tables_spec(),
            );
            assert_clean(&presented, &label);
            let tables = accessible_tables(&presented);
            let counts: Vec<_> = tables.iter().map(|t| (t.rows, t.columns)).collect();
            assert_eq!(
                counts,
                [(Some(2), Some(2)), (Some(2), Some(2)), (Some(1), Some(2))],
                "{label}"
            );
            for text in presented.kind(TextComponentKind::ReadingText) {
                assert!(text.allocated_rect.max_x <= width + 0.5, "{label}");
            }
            // The quoted and listed tables are indented with their blocks.
            let left = |text: &str| reading_text(&presented, text).allocated_rect.min_x;
            assert!(left("q1") > left("Only"), "{label}");
            assert!(left("l1") > left("Only"), "{label}");
        }
    }
}

/// A wide table whose header holds a link and a footnote reference, with a
/// body row whose cells are all empty.
const LINKED_HEADER: &str = "## Counts\n\n\
| Pool | Tide [table](https://example.org/tides) | Area (m²) | Depth (m) | Temperature (°C) | Species | Notes[^n] |\n\
| --- | --- | --- | --- | --- | --- | --- |\n\
| North shelf | Low | 12.5 | 0.4 | 14 | Limpets | Sheltered |\n\
| | | | | | | |\n\
| South gully | High | 3 | 1.2 | 15 | Anemones | Surge |\n\n\
[^n]: Notes from the second tide.\n";

#[test]
fn a_stacked_table_counts_only_its_records_and_keeps_its_header_links() {
    use egui::accesskit::Role::{Cell, ColumnHeader};
    let ids = [SemanticUiId::new("counts.note")];
    for scale in [1.0, 1.5] {
        for width in [DESKTOP, PHONE] {
            let label = format!("{width} at {scale}");
            let presented = present_spec(
                &context(),
                LINKED_HEADER,
                width,
                scale,
                tokens(),
                None,
                notes_spec(&ids),
            );
            assert_clean(&presented, &label);
            // The header's link and reference are presented once and work,
            // whatever the layout.
            let links: Vec<_> = presented
                .links()
                .into_iter()
                .map(|node| (node.name.as_str(), node.enabled))
                .collect();
            assert_eq!(links, [("table", true), ("[1]", true)], "{label}");
            let tables = accessible_tables(&presented);
            assert_eq!(tables.len(), 1, "{label}");
            let rows: Vec<_> = tables[0]
                .cells
                .iter()
                .map(|(row, cells)| (*row, cells.first().map(|cell| (cell.0, cell.1))))
                .collect();
            if width == DESKTOP {
                // The grid keeps the empty row as a row of empty cells.
                assert_eq!(tables[0].rows, Some(4), "{label}");
                assert_eq!(rows.len(), 4, "{label}");
            } else {
                // Stacked: the linked header row is a record of its own,
                // and the empty row is neither presented nor counted.
                assert_eq!(tables[0].rows, Some(3), "{label}");
                assert_eq!(
                    rows,
                    [
                        (Some(0), Some((ColumnHeader, Some(0)))),
                        (Some(1), Some((Cell, Some(1)))),
                        (Some(2), Some((Cell, Some(2)))),
                    ],
                    "{label}"
                );
                let lines: Vec<&str> = presented
                    .kind(TextComponentKind::ReadingText)
                    .into_iter()
                    .filter_map(|text| text.content_text())
                    .collect();
                // The leads repeat the header's text, without its links.
                for line in [
                    "Tide table",
                    "Notes[1]",
                    "Tide table: Low",
                    "Notes[1]: Surge",
                ] {
                    assert!(lines.contains(&line), "{label}: {lines:#?}");
                }
                // ARIA counts the records alone, one-based.
                let tree = crate::aria::project(&presented.update, 1.0);
                let table = tree
                    .elements
                    .values()
                    .find(|element| {
                        element.attributes.get("role").map(String::as_str) == Some("table")
                    })
                    .expect("an ARIA table");
                assert_eq!(table.attributes["aria-rowcount"], "3", "{label}");
                let mut indices: Vec<_> = tree
                    .elements
                    .values()
                    .filter(|element| {
                        element.attributes.get("role").map(String::as_str) == Some("row")
                    })
                    .map(|row| row.attributes["aria-rowindex"].clone())
                    .collect();
                indices.sort();
                assert_eq!(indices, ["1", "2", "3"], "{label}");
            }
        }
    }
}

#[test]
fn a_stacked_tables_opening_rule_is_set_apart_from_a_heading() {
    let tokens = tokens();
    for (width, gap) in [
        (PHONE, tokens.spacing.section.0),
        (DESKTOP, tokens.spacing.block.0),
    ] {
        let presented = present_spec(
            &context(),
            LINKED_HEADER,
            width,
            1.0,
            tokens,
            None,
            tables_spec(),
        );
        let heading = reading_text(&presented, "Counts").allocated_rect;
        let table = accessible_tables(&presented)[0].top as f32;
        assert!(
            (table - heading.max_y - gap).abs() < 0.5,
            "{width}: {} under the heading",
            table - heading.max_y
        );
    }
}

#[test]
fn a_table_projects_as_an_aria_table() {
    let presented = present_spec(
        &context(),
        TABLES,
        DESKTOP,
        1.0,
        tokens(),
        None,
        tables_spec(),
    );
    let tree = crate::aria::project(&presented.update, 1.0);
    let roles = |role: &str| {
        tree.elements
            .values()
            .filter(|element| element.attributes.get("role").map(String::as_str) == Some(role))
            .collect::<Vec<_>>()
    };
    let tables = roles("table");
    assert_eq!(tables.len(), 2);
    assert!(
        tables
            .iter()
            .any(|table| table.attributes["aria-rowcount"] == "4"
                && table.attributes["aria-colcount"] == "7")
    );
    assert_eq!(roles("row").len(), 6);
    assert_eq!(roles("columnheader").len(), 9);
    assert_eq!(roles("cell").len(), 23);
    // Each cell's text is within its cell.
    let pool = roles("columnheader")
        .into_iter()
        .find(|header| header.attributes["aria-colindex"] == "1" && header.children.len() == 1)
        .expect("a first column header with its text");
    assert_eq!(pool.attributes["aria-rowindex"], "1");
}

#[test]
fn the_default_options_keep_a_table_as_its_source() {
    let presented = present(TABLES, DESKTOP, 1.0);
    assert_clean(&presented, "default");
    assert!(accessible_tables(&presented).is_empty());
    let lines: Vec<&str> = presented
        .kind(TextComponentKind::ReadingText)
        .into_iter()
        .filter_map(|text| text.content_text())
        .collect();
    assert_eq!(lines.len(), 2, "{lines:#?}");
    assert!(
        lines[1].starts_with("| Name | Count | | --- | ---: |"),
        "{lines:#?}"
    );
}

/// The rectangles painted with `fill`, in paint order.
fn filled(presented: &Presented, fill: egui::Color32) -> Vec<(usize, Rect)> {
    presented
        .shapes
        .iter()
        .enumerate()
        .filter_map(|(index, clipped)| match &clipped.shape {
            egui::Shape::Rect(rect) if rect.fill == fill => Some((index, rect.rect)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_quoted_tables_header_row_shows_its_raised_surface() {
    for theme in [ThemeVariant::Dark, ThemeVariant::LightHighContrast] {
        let tokens = DesignTokens::resolve(theme, DensityVariant::Comfortable);
        let presented = present_spec(
            &context(),
            NESTED_TABLES,
            DESKTOP,
            1.0,
            tokens,
            None,
            tables_spec(),
        );
        let header = text_rect(reading_text(&presented, "Quoted").allocated_rect);
        let quoted = text_rect(reading_text(&presented, "q1").allocated_rect);
        // The raised surface lies behind the header row, inside the quote,
        // and nothing painted later covers it.
        let raised: egui::Color32 = tokens.colours.surface_raised.into();
        let fills: Vec<_> = filled(&presented, raised)
            .into_iter()
            .filter(|(_, rect)| rect.contains_rect(header))
            .collect();
        assert_eq!(fills.len(), 1, "{theme:?}: {fills:?}");
        let (painted, fill) = fills[0];
        assert!(
            !fill.contains_rect(quoted),
            "{theme:?}: the body row is not raised"
        );
        assert!(fill.min.x > 0.0, "{theme:?}: indented with the quote");
        let covered = presented.shapes[painted + 1..].iter().any(|clipped| {
            matches!(&clipped.shape, egui::Shape::Rect(rect)
                if rect.fill != egui::Color32::TRANSPARENT && rect.rect.intersects(header))
        });
        assert!(!covered, "{theme:?}");
        // The quote paints no surface of its own for the fill to vanish into.
        let canvas: egui::Color32 = tokens.colours.surface_canvas.into();
        assert!(crate::components::reading::contrast_ratio(raised, canvas) > 1.05);
    }
}

#[test]
fn an_open_task_box_keeps_three_to_one_against_every_surface() {
    for theme in [
        ThemeVariant::Dark,
        ThemeVariant::Light,
        ThemeVariant::DarkHighContrast,
        ThemeVariant::LightHighContrast,
    ] {
        let tokens = DesignTokens::resolve(theme, DensityVariant::Comfortable);
        let presented = present_spec(
            &context(),
            NOTES,
            DESKTOP,
            1.0,
            tokens,
            None,
            notes_spec(&[]),
        );
        let open = nodes_of(&presented, UiRole::Checkbox)
            .into_iter()
            .find(|node| node.name == "Draft the summary")
            .expect("the open task");
        let open = Rect::from_min_max(
            egui::pos2(open.rect.min_x, open.rect.min_y),
            egui::pos2(open.rect.max_x, open.rect.max_y),
        );
        let edges: Vec<egui::Color32> = presented
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect) if rect.rect == open => Some(rect.stroke.color),
                _ => None,
            })
            .collect();
        assert_eq!(edges.len(), 1, "{theme:?}");
        // A state indicator keeps 3:1 (WCAG 1.4.11) against each surface
        // a document sits on.
        for surface in [
            tokens.colours.surface_canvas,
            tokens.colours.surface_panel,
            tokens.colours.surface_raised,
        ] {
            let ratio = crate::components::reading::contrast_ratio(edges[0], surface.into());
            assert!(ratio >= 3.0, "{theme:?}: {ratio:.2}");
        }
    }
}

/// Drag from the start of `first`'s text to the end of `last`'s in
/// `source` presented `width` wide at `scale`, copy, and return the text.
fn copy_across(source: &'static str, width: f32, scale: f32, first: &str, last: &str) -> String {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(width, 1_000.0))
        .build_ui(move |ui| {
            crate::install_typography_fonts(ui.ctx());
            let mut p = PresentationContext::new(
                ui,
                tokens(),
                scale,
                PresentationScope::new("reply"),
                SemanticUiId::root(),
            );
            let _ = p.markdown(ui, "body", source, tables_spec());
            let _ = p.finish(ui);
        });
    harness.run();
    let start = harness.get_by_label(first).rect();
    let end = harness.get_by_label(last).rect();
    let (start, end) = (
        start.left_top() + egui::vec2(1.0, start.height() * 0.5),
        end.right_bottom() - egui::vec2(1.0, end.height() * 0.25),
    );
    harness.hover_at(start);
    harness.drag_at(start);
    harness.hover_at(end);
    harness.drop_at(end);
    harness.step();
    harness.event(egui::Event::Copy);
    harness.step();
    let commands = &harness.output().platform_output.commands;
    commands
        .iter()
        .find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{commands:?}"))
}

#[test]
fn a_table_copies_stacked_wrapped_and_at_150_percent() {
    const SMALL: &str = "| Name | Count |\n| --- | ---: |\n| Crab | 4 |\n| Whelk | 12 |";
    for scale in [1.0, 1.5] {
        assert_eq!(
            copy_across(SMALL, DESKTOP, scale, "Name", "12"),
            "Name Count\nCrab 4\nWhelk 12",
            "a grid at {scale}"
        );
    }
    let stacked = copy_across(SMALL, 100.0, 1.0, "Name: Crab", "Count: 12");
    // Each record's lines, a blank line between records.
    assert_eq!(
        stacked, "Name: Crab\nCount: 4\n\nName: Whelk\nCount: 12",
        "stacked"
    );
    let wrapped = copy_across(
        "| Note | Count |\n| --- | ---: |\n| A long note that wraps across lines | 4 |\n| Short | 12 |",
        200.0,
        1.0,
        "Note",
        "12",
    );
    // A row to a line, its wrapped cell's text unbroken. egui spaces text
    // copied by its distance from the last text copied, here the short
    // cell ending the tall row, so a blank line follows that row.
    assert_eq!(
        wrapped, "Note Count\nA long note that wraps across lines 4\n\nShort 12",
        "wrapped"
    );
}

#[test]
fn dragging_across_a_table_selects_and_copies_its_cells() {
    let rect = Rc::new(Cell::new(Rect::NOTHING));
    let observed = rect.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(DESKTOP, 300.0))
        .build_ui(move |ui| {
            crate::install_typography_fonts(ui.ctx());
            let mut p = PresentationContext::new(
                ui,
                tokens(),
                1.0,
                PresentationScope::new("reply"),
                SemanticUiId::root(),
            );
            let response = p.markdown(
                ui,
                "body",
                "| Name | Count |\n| --- | ---: |\n| Crab | 4 |\n| Whelk | 12 |",
                tables_spec(),
            );
            observed.set(response.rect);
            let _ = p.finish(ui);
        });
    harness.run();
    let rect = rect.get();
    let table = harness
        .get_by_role(egui::accesskit::Role::Table)
        .accesskit_node()
        .data()
        .bounds()
        .unwrap();
    // From the first header's text to the last cell's.
    let start = rect.left_top() + egui::vec2(8.0, 6.0);
    let end = egui::pos2(table.x1 as f32 - 8.0, table.y1 as f32 - 6.0);
    harness.hover_at(start);
    harness.drag_at(start);
    harness.hover_at(end);
    harness.drop_at(end);
    harness.step();
    harness.event(egui::Event::Copy);
    harness.step();
    let commands = &harness.output().platform_output.commands;
    let copied = commands
        .iter()
        .find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{commands:?}"));
    assert_eq!(copied, "Name Count\nCrab 4\nWhelk 12");
}
