//! Reading text and links: measured text of any length for content such as a
//! message or a document, and links that open external URLs.

use egui::{Color32, CursorIcon, Rect, Response, Sense, Stroke, text::LayoutJob};

use crate::{
    DesignTokens, PresentationId, SemanticUiId, TextComponentId, TextComponentKind,
    TextInteraction, TextLayoutError, TextLayoutObservation, TextRole, TextRoleStyles, UiNode,
    UiRole, failed_measurement, measure_component_reading, paint_measured_text,
    present_accessible_measured_text, present_measured_text, reading_job,
};

/// Why a link whose URL [`link_url_allowed`] rejects is unavailable.
pub const INVALID_LINK_URL: &str = "The link's address is not an http, https or mailto URL";

/// Whether a link may open `url`: an `http://` or `https://` URL with
/// something after the scheme, or a `mailto:` address, without whitespace or control characters.
/// Other schemes, such as `javascript:` or `data:`, could run script or
/// replace the application, so links present them as unavailable.
pub fn link_url_allowed(url: &str) -> bool {
    if url.is_empty() || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    ["http://", "https://", "mailto:"]
        .into_iter()
        .any(|scheme| {
            lower
                .strip_prefix(scheme)
                .is_some_and(|rest| !rest.is_empty())
        })
}

/// How a reading surface presents its text. It always wraps at the available
/// width, to as many lines as the text needs, and is never truncated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadingTextSpec {
    /// Measured typography role; body text for prose.
    pub role: TextRole,
    /// Selectable by default. Choose Inert only where a parent owns the
    /// pointer, such as a clickable row.
    pub interaction: TextInteraction,
}

impl ReadingTextSpec {
    pub const fn new(role: TextRole) -> Self {
        Self {
            role,
            interaction: TextInteraction::Selectable,
        }
    }
}

impl Default for ReadingTextSpec {
    fn default() -> Self {
        Self::new(TextRole::Body)
    }
}

/// Present text of any length: measured, wrapped at the available width,
/// allocated at its full measured height and selectable with egui's own
/// selection and copy. Put long text in a scroll area; the scroll area may
/// clip the allocation, and the observation records that clip.
///
/// Returns the text's response, which owns one accessible label with the
/// complete text. The observation is kept while any of the text is visible
/// (or its measurement failed), so text scrolled fully out of view adds no
/// observation; the pass's coverage still counts the attempt.
pub fn reading_text(
    ui: &mut egui::Ui,
    identity: PresentationId,
    text: &str,
    spec: ReadingTextSpec,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    let width = ui.available_width().max(1.0);
    let colour = spec.role.style(tokens, font_scale).colour;
    let measured = measure_component_reading(
        ui.painter(),
        reading_job(text, spec.role, colour, tokens, font_scale),
        spec.role,
        spec.interaction,
        tokens,
        font_scale,
        width,
    );
    let (_, rect) = ui.allocate_space(egui::vec2(width, measured.size().y.max(1.0)));
    let component = TextComponentId::new(TextComponentKind::ReadingText, identity.text_instance());
    let (response, observation) = if measured.spec.interaction == TextInteraction::Selectable {
        let (response, observation) = present_measured_text(ui, &measured, rect, component, None);
        (
            response.expect("selectable measured text returns a response"),
            observation,
        )
    } else {
        present_accessible_measured_text(ui, &measured, rect, component, None)
    };
    if response.interact_rect.is_positive() {
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_author_id(identity.semantic_id().0);
        });
    }
    // Text scrolled wholly out of view adds no observation, but a reveal of
    // its semantic ID still finds it.
    crate::observation::reveal_at(ui, &identity.semantic_id(), rect);
    if ui.clip_rect().intersect(rect).is_positive() || observation.layout_error.is_some() {
        observations.push(observation);
    }
    response
}

/// Where a link opens its URL.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LinkTarget {
    /// A new browser tab, so the application and its in-memory state stay
    /// open. Native applications open the platform's default browser.
    #[default]
    NewTab,
    /// The current browser tab, replacing the application. Native
    /// applications open the platform's default browser.
    SameTab,
}

/// A link to an external URL. The label is what the link says; the URL is
/// where it goes.
#[derive(Clone, Copy, Debug)]
pub struct LinkSpec<'a> {
    /// Visible text and accessible name. A blank label shows the URL.
    pub label: &'a str,
    /// The URL to open: http, https or mailto (see [`link_url_allowed`]).
    /// The application decides whether a URL from an untrusted source, such
    /// as a model's reply, should be shown as a link at all; Stobie only
    /// refuses schemes that could run script or replace the application.
    pub url: &'a str,
    /// Additional context for assistive technology, such as where the link
    /// leads or that it opens a new tab, in the application's language.
    pub description: Option<&'a str>,
    pub target: LinkTarget,
    /// Measured typography role; body text by default.
    pub role: TextRole,
}

impl<'a> LinkSpec<'a> {
    pub const fn new(label: &'a str, url: &'a str) -> Self {
        Self {
            label,
            url,
            description: None,
            target: LinkTarget::NewTab,
            role: TextRole::Body,
        }
    }

    /// The text the link shows and is named by.
    pub fn visible_label(&self) -> &'a str {
        if self.label.trim().is_empty() {
            self.url
        } else {
            self.label
        }
    }
}

/// The colour a link paints in: the theme's accent where it keeps 4.5:1
/// against both the canvas and panel surfaces content sits on, otherwise
/// primary text. The underline marks a link either way, so colour is never
/// the only cue.
pub fn link_colour(tokens: &DesignTokens) -> Color32 {
    let accent = tokens.colours.accent_primary;
    let readable = [tokens.colours.surface_canvas, tokens.colours.surface_panel]
        .into_iter()
        .all(|surface| contrast_ratio(accent.into(), surface.into()) >= 4.5);
    if readable {
        accent.into()
    } else {
        tokens.colours.text_primary.into()
    }
}

/// WCAG 2.2 contrast between two opaque colours.
pub(crate) fn contrast_ratio(foreground: Color32, background: Color32) -> f64 {
    let luminance = |colour: Color32| {
        let linear = |channel: u8| {
            let value = f64::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(colour.r()) + 0.7152 * linear(colour.g()) + 0.0722 * linear(colour.b())
    };
    let (a, b) = (luminance(foreground), luminance(background));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Present a link that opens `spec.url`: in a new browser tab by default, or
/// the platform's default browser natively. A modifier or middle click always
/// opens a new tab. Pointer, keyboard (Tab, then Enter or Space) and
/// assistive-technology activation share one route. In the browser the new
/// tab opens with `noopener` and `noreferrer`, however the application
/// started, so the linked page cannot reach back into the application's tab.
///
/// A URL that [`link_url_allowed`] rejects, including a blank one, paints a
/// visible diagnostic, records a failing observation
/// ([`TextLayoutError::InvalidUrl`]) and presents an unavailable link that
/// opens nothing.
///
/// The underlined label wraps at the available width without a line limit
/// and is never truncated. The allocation hugs the label, with half a spacing
/// unit either side so the focus ring clears it, and keeps the token minimum
/// hit size; the label is start-aligned within it and centred vertically.
/// Hovering shows the URL and a pointing-hand cursor; keyboard focus adds the
/// token focus ring. The label is inert: activation owns the
/// pointer. Low-level callers publish the matching [`link_semantic_node`];
/// [`crate::PresentationContext::link`] records both automatically.
pub fn link(
    ui: &mut egui::Ui,
    spec: LinkSpec<'_>,
    identity: PresentationId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    let valid = link_url_allowed(spec.url);
    let label = spec.visible_label();
    let enabled = ui.is_enabled() && valid;
    let foreground = if enabled {
        link_colour(tokens)
    } else {
        tokens.colours.text_muted.into()
    };
    let available = ui.available_width().max(1.0);
    let minimum = tokens.geometry.minimum_hit_size.0;
    // Half a spacing unit each side keeps the focus ring off the glyphs.
    let inset = tokens.spacing.unit.0 * 0.5;
    let label_width = (available - inset * 2.0).max(1.0);
    // The hovered and pressed underline is heavier; measure both so the
    // allocation never moves with hover.
    let measure = |ui: &egui::Ui, underline: f32| {
        if !valid {
            return failed_measurement(
                ui.painter(),
                label,
                TextInteraction::Inert,
                TextLayoutError::InvalidUrl,
                tokens,
                label_width,
            );
        }
        let mut job: LayoutJob = reading_job(label, spec.role, foreground, tokens, font_scale);
        for section in &mut job.sections {
            section.format.underline = Stroke::new(underline, foreground);
        }
        measure_component_reading(
            ui.painter(),
            job,
            spec.role,
            TextInteraction::Inert,
            tokens,
            font_scale,
            label_width,
        )
    };
    let resting = measure(ui, 1.0);
    let size = resting.size();
    let (_, rect) = ui.allocate_space(egui::vec2(
        (size.x + inset * 2.0)
            .max(minimum)
            .min(available.max(minimum)),
        size.y.max(minimum),
    ));
    let response = ui.interact(
        rect,
        identity.egui_id(),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if !enabled {
        // An unavailable link opens nothing, whatever its sense reports.
    } else if response.clicked_with_open_in_background()
        || (response.clicked() && spec.target == LinkTarget::NewTab)
    {
        crate::open_new_tab(ui.ctx(), spec.url);
    } else if response.clicked() {
        ui.ctx().open_url(egui::OpenUrl {
            url: spec.url.to_owned(),
            new_tab: false,
        });
    }
    if response.interact_rect.is_positive() {
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, enabled, label));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            use egui::accesskit::{Action, Role};
            node.set_role(Role::Link);
            node.set_label(label);
            node.set_url(spec.url);
            node.set_author_id(identity.semantic_id().0);
            match (spec.description, valid) {
                (description, false) => node.set_description(
                    crate::semantics::unavailable_description(description, INVALID_LINK_URL),
                ),
                (Some(description), true) => node.set_description(description),
                (None, true) => node.clear_description(),
            }
            let bounds = response.interact_rect;
            node.set_bounds(egui::accesskit::Rect {
                x0: f64::from(bounds.min.x),
                y0: f64::from(bounds.min.y),
                x1: f64::from(bounds.max.x),
                y1: f64::from(bounds.max.y),
            });
            if enabled {
                node.add_action(Action::Click);
            } else {
                node.set_disabled();
                node.remove_action(Action::Click);
            }
        });
    }
    let emphasised = enabled && (response.hovered() || response.is_pointer_button_down_on());
    let measured = if emphasised {
        measure(ui, 2.0)
    } else {
        resting
    };
    let text_rect = Rect::from_min_size(
        egui::pos2(rect.left() + inset, rect.center().y - size.y * 0.5),
        egui::vec2((rect.width() - inset * 2.0).max(size.x), size.y),
    );
    let component = TextComponentId::new(TextComponentKind::Link, identity.text_instance());
    let observation = paint_measured_text(ui.painter(), &measured, text_rect, component, None);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            tokens.geometry.control_radius.0,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    if ui.clip_rect().intersect(text_rect).is_positive() || observation.layout_error.is_some() {
        observations.push(observation);
    }
    if response.hovered() && enabled {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    response.on_hover_text(spec.url)
}

/// The link's semantic node: role [`UiRole::Link`], named by its label, with
/// the URL as its value. A fully hidden link has none.
pub fn link_semantic_node(
    response: &Response,
    spec: LinkSpec<'_>,
    identity: PresentationId,
    parent: SemanticUiId,
) -> Option<UiNode> {
    let rect = response.interact_rect;
    if !rect.is_positive() {
        return None;
    }
    let mut node = UiNode::container(
        identity.semantic_id(),
        Some(parent),
        UiRole::Link,
        rect.into(),
    );
    node.name = spec.visible_label().to_owned();
    node.description = spec.description.map(str::to_owned);
    let valid = link_url_allowed(spec.url);
    node.enabled = response.enabled() && valid;
    node.focused = response.has_focus();
    node.value = Some(spec.url.to_owned());
    if !valid {
        node.disabled_reason = Some(INVALID_LINK_URL.to_owned());
    }
    Some(node)
}
