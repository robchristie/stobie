//! The dialog recipes: a modal dialog, a sheet that rises from the bottom of
//! a phone-width screen, and a confirm dialog.
//!
//! Each is modal. It is drawn over a scrim on egui's foreground order, so the
//! pointer and keyboard focus cannot reach what is behind it, and its content
//! is marked for the host, which then observes and binds nothing else; see
//! [`AppHost`](crate::AppHost) for focus placement, containment and return.
//! The application owns whether the dialog is open: it presents the dialog in
//! every pass while it is, and closes it by handling the dialog's dismiss (or
//! cancel) action, which Escape and the close control trigger.

use std::hash::Hash;

use egui::{CornerRadius, Rect, Response, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

use super::{PresentationContext, PresentationScope};
use crate::{
    ActionArgument, ActionButtonContent, ActionButtonSpec, ActionButtonState, ActionEmphasis,
    ActionKey, ActionTarget, Availability, IconId, IntoActionTarget, PaneWidthClass,
    ReadingTextSpec, TextInteraction, TextRole, TextRoleStyles, UiNode, UiRole,
    components::{action_button_size, action_hit_height},
    observation::DialogRegistration,
};

/// A dialog's width, in points at the reference font scale, unless its spec
/// says otherwise; never wider than the screen allows.
pub const DIALOG_WIDTH: f32 = 560.0;

/// A confirm dialog's width, in points at the reference font scale.
pub const CONFIRM_WIDTH: f32 = 400.0;

/// How strongly the scrim behind a dialog covers what is behind it: the
/// canvas colour at this opacity.
const SCRIM_OPACITY: f32 = 0.72;

/// Where a dialog may go.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DialogLayout {
    /// Centred on the screen at every width.
    #[default]
    Centred,
    /// A sheet: a bottom sheet across the screen while it is narrower than
    /// the wide breakpoint ([`PaneWidthClass::Wide`]), as on a phone, and
    /// centred otherwise.
    Sheet,
}

/// How tall a sheet is while it is a bottom sheet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SheetHeight {
    /// As tall as its content, leaving the top of the screen in view.
    #[default]
    Content,
    /// The whole screen, for a form that needs the room.
    Full,
}

/// Where a dialog is placed in one pass, from its layout and the screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogPlacement {
    /// Centred, with a margin of the screen around it.
    Centred,
    /// Across the bottom of the screen, as tall as its content.
    BottomSheet,
    /// Covering the screen.
    FullScreen,
}

impl DialogPlacement {
    /// The placement of a dialog laid out as `layout` and `height` on a
    /// screen `screen_width` points wide.
    pub fn for_layout(layout: DialogLayout, height: SheetHeight, screen_width: f32) -> Self {
        match layout {
            DialogLayout::Centred => Self::Centred,
            DialogLayout::Sheet
                if PaneWidthClass::from_points(screen_width) == PaneWidthClass::Wide =>
            {
                Self::Centred
            }
            DialogLayout::Sheet => match height {
                SheetHeight::Content => Self::BottomSheet,
                SheetHeight::Full => Self::FullScreen,
            },
        }
    }
}

/// A modal dialog or sheet: a title, a close control and the content the
/// application presents in it.
///
/// The dismiss action closes it. The close control presents it, named by its
/// label (such as "Close"), and Escape triggers it; so does a press on the
/// scrim when [`Self::backdrop_dismisses`] says so. While it is disabled, as
/// while the dialog's work is under way, none of them closes the dialog; while
/// it is hidden, no close control is shown.
///
/// The application's state says whether a dialog is open; its actions open
/// and close it. Present it after what is behind it, in every pass while it
/// is open. A confirm ([`ConfirmSpec`]) asks before an action:
///
/// ```
/// use serde::Serialize;
/// use stobie_ui_egui::{
///     Action, ActionEmphasis, ActionTarget, Application, ApplicationIdentity, Availability,
///     ConfirmSpec, DialogSpec, Frame, PresentationScope, ReadingTextSpec, SemanticUiId,
///     application_identity,
/// };
///
/// #[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
/// #[action(prefix = "account")]
/// enum AccountAction {
///     #[action(label = "Sign out", description = "Ask before signing out")]
///     SignOut,
///     #[action(label = "Sign out", description = "Sign out of this account")]
///     ConfirmSignOut,
///     #[action(label = "Stay signed in", description = "Keep this account signed in")]
///     StaySignedIn,
///     #[action(label = "Profile", description = "Edit the profile")]
///     EditProfile,
///     #[action(label = "Close", description = "Close the profile")]
///     CloseProfile,
/// }
///
/// #[derive(Default)]
/// struct Account { signed_in: bool, confirming: bool, editing: bool }
///
/// impl Application for Account {
///     type Action = AccountAction;
///     type Intent = ();
///     type UiState = ();
///
///     fn identity(&self) -> ApplicationIdentity {
///         application_identity!("Account")
///     }
///
///     fn availability(&self, target: ActionTarget<AccountAction>) -> Availability {
///         match target.action {
///             AccountAction::SignOut | AccountAction::ConfirmSignOut if !self.signed_in => {
///                 Availability::Disabled { reason: "Not signed in".into() }
///             }
///             _ => Availability::Enabled,
///         }
///     }
///
///     fn ui(&self, ui: &mut egui::Ui, _: &mut (), frame: &mut Frame<'_, AccountAction, ()>) {
///         let mut p = frame.presentation(ui, PresentationScope::new("account"), SemanticUiId::root());
///         frame.action(ui, &mut p, AccountAction::EditProfile, ActionEmphasis::Normal);
///         frame.action(ui, &mut p, AccountAction::SignOut, ActionEmphasis::Normal);
///         // A sheet: along the bottom on a phone, covering it when full
///         // height, and centred on a desktop.
///         if self.editing {
///             let spec = DialogSpec::new("Profile", AccountAction::CloseProfile)
///                 .sheet()
///                 .full_height();
///             frame.dialog(ui, &mut p, "profile", &spec, |ui, _, p| {
///                 p.reading_text(ui, "hint", "Your name appears in every conversation.",
///                     ReadingTextSpec::default());
///             });
///         }
///         // Cancel comes first and takes focus; Escape cancels.
///         if self.confirming {
///             frame.confirm(ui, &mut p, "sign-out", &ConfirmSpec::new(
///                 "Sign out?",
///                 "You will need to sign in again to use this account.",
///                 AccountAction::ConfirmSignOut,
///                 AccountAction::StaySignedIn,
///             ));
///         }
///     }
///
///     fn apply(&mut self, _: (), _: &mut ()) -> Result<bool, String> {
///         Ok(false)
///     }
///
///     fn perform(&mut self, target: ActionTarget<AccountAction>, _: &mut ()) -> Result<bool, String> {
///         match target.action {
///             AccountAction::SignOut => self.confirming = true,
///             AccountAction::ConfirmSignOut => (self.signed_in, self.confirming) = (false, false),
///             AccountAction::StaySignedIn => self.confirming = false,
///             AccountAction::EditProfile => self.editing = true,
///             AccountAction::CloseProfile => self.editing = false,
///         }
///         Ok(true)
///     }
///
///     fn report_error(&mut self, _: String) {}
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct DialogSpec<'a, A> {
    /// The dialog's title, which names it.
    pub title: &'a str,
    /// The action that closes the dialog without its work.
    pub dismiss: A,
    /// Where it goes: centred, or a sheet.
    pub layout: DialogLayout,
    /// How tall it is while a bottom sheet.
    pub sheet_height: SheetHeight,
    /// Whether a press on the scrim dismisses it. Off by default, so a press
    /// beside a form does not lose what was typed.
    pub backdrop_dismisses: bool,
    /// Its width while centred, in points at the reference font scale.
    pub width: f32,
}

impl<'a, A> DialogSpec<'a, A> {
    /// A centred dialog titled `title`, closed by `dismiss`.
    pub fn new(title: &'a str, dismiss: A) -> Self {
        Self {
            title,
            dismiss,
            layout: DialogLayout::Centred,
            sheet_height: SheetHeight::Content,
            backdrop_dismisses: false,
            width: DIALOG_WIDTH,
        }
    }

    /// A sheet: a bottom sheet at phone widths, centred on a wide screen.
    pub fn sheet(mut self) -> Self {
        self.layout = DialogLayout::Sheet;
        self
    }

    /// A sheet that covers the screen at phone widths.
    pub fn full_height(mut self) -> Self {
        self.layout = DialogLayout::Sheet;
        self.sheet_height = SheetHeight::Full;
        self
    }

    /// Whether a press on the scrim dismisses it, as for a sheet with no
    /// typed work to lose.
    pub fn backdrop_dismisses(mut self, dismisses: bool) -> Self {
        self.backdrop_dismisses = dismisses;
        self
    }

    /// Its width while centred, in points at the reference font scale.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// What a dialog did in one pass.
#[derive(Clone, Debug)]
pub struct DialogResponse<A: ActionKey, R> {
    /// What the content returned.
    pub inner: R,
    /// Whether the close control, Escape or the scrim dismissed the dialog
    /// this pass. Trigger the dismiss action.
    pub dismissed: bool,
    /// The dismiss action's resolved target.
    pub dismiss: ActionTarget<A>,
    /// The close control's response; its rectangle is empty while dismiss is
    /// hidden.
    pub close: Response,
    pub placement: DialogPlacement,
    /// The dialog's surface.
    pub rect: Rect,
}

/// A confirm dialog: a title, a message and two actions, one confirming the
/// action it asks about and one cancelling it.
///
/// Cancel has focus when the dialog opens, so a stray Return does not
/// confirm. Escape cancels, while cancel is enabled; the scrim does nothing.
#[derive(Clone, Copy, Debug)]
pub struct ConfirmSpec<'a, A> {
    /// The question, such as "Sign out?", which names the dialog.
    pub title: &'a str,
    /// What confirming does, which describes the dialog.
    pub message: &'a str,
    /// The action asked about, labelled with what it does ("Sign out").
    pub confirm: A,
    /// The action that keeps things as they are.
    pub cancel: A,
    /// The confirm control's emphasis: primary unless the spec says.
    pub confirm_emphasis: ActionEmphasis,
}

impl<'a, A> ConfirmSpec<'a, A> {
    /// A confirm titled `title` and described by `message`, asking before
    /// `confirm`, with `cancel` to keep things as they are.
    pub fn new(title: &'a str, message: &'a str, confirm: A, cancel: A) -> Self {
        Self {
            title,
            message,
            confirm,
            cancel,
            confirm_emphasis: ActionEmphasis::Primary,
        }
    }

    /// The confirm control's emphasis.
    pub fn confirm_emphasis(mut self, emphasis: ActionEmphasis) -> Self {
        self.confirm_emphasis = emphasis;
        self
    }
}

/// What a confirm dialog did in one pass.
#[derive(Clone, Debug)]
pub struct ConfirmResponse<A: ActionKey> {
    /// Whether confirm was pressed this pass. Trigger the confirm action.
    pub confirmed: bool,
    /// Whether cancel was pressed, or Escape cancelled, this pass. Trigger
    /// the cancel action.
    pub cancelled: bool,
    pub confirm: ActionTarget<A>,
    pub cancel: ActionTarget<A>,
    pub confirm_response: Response,
    pub cancel_response: Response,
    /// The dialog's surface.
    pub rect: Rect,
}

/// The parts of a dialog that differ between the recipes.
struct Chrome<'s> {
    scope: PresentationScope,
    title: &'s str,
    description: Option<&'s str>,
    role: UiRole,
    geometry: Geometry,
    /// Width reserved at the end of the header for a close control.
    close_width: f32,
    /// Height reserved below the scrolling body for fixed controls.
    footer_height: f32,
    /// Whether Escape dismisses the dialog.
    escape_dismisses: bool,
}

/// A dialog's size and placement in one pass.
#[derive(Clone, Copy, Debug)]
struct Geometry {
    placement: DialogPlacement,
    screen: Rect,
    /// The surface's width.
    width: f32,
    /// The width inside the surface's padding and border.
    inner_width: f32,
    max_height: f32,
    padding: f32,
}

const BORDER: f32 = 1.0;

impl Geometry {
    fn new(
        ui: &Ui,
        placement: DialogPlacement,
        preferred_width: f32,
        tokens: &crate::DesignTokens,
        font_scale: f32,
    ) -> Self {
        let screen = ui.ctx().content_rect();
        let margin = tokens.spacing.section.0 * 2.0;
        let padding = tokens.spacing.section.0;
        let (width, max_height) = match placement {
            DialogPlacement::Centred => (
                (preferred_width * font_scale).min(screen.width() - margin * 2.0),
                screen.height() - margin * 2.0,
            ),
            // The top of the screen stays in view, so it reads as a sheet.
            DialogPlacement::BottomSheet => (
                screen.width(),
                screen.height() - tokens.geometry.application_bar_height.0,
            ),
            DialogPlacement::FullScreen => (screen.width(), screen.height()),
        };
        let width = width.floor().max(padding * 2.0 + BORDER * 2.0 + 1.0);
        Self {
            placement,
            screen,
            width,
            inner_width: width - padding * 2.0 - BORDER * 2.0,
            max_height: max_height.max(1.0),
            padding,
        }
    }
}

/// The rectangles a recipe fills after the body.
struct Slots {
    close: Rect,
    footer: Rect,
    /// Whether this is the first frame the dialog is shown in.
    newly_shown: bool,
}

struct ModalPass<R, F> {
    inner: R,
    after: F,
    escape: bool,
    backdrop: bool,
    rect: Rect,
}

impl PresentationContext {
    /// Present a modal dialog or sheet, with `content` inside it, and report
    /// what it did; hosted applications use
    /// [`Frame::dialog`](crate::Frame::dialog), which also triggers dismiss.
    /// Present it in every pass while it is open, after what is behind it.
    ///
    /// - It is observed as a [`UiRole::Dialog`] named by its title, holding
    ///   what `content` presents, and is marked modal for assistive
    ///   technology. While it is presented, a host observes and binds only
    ///   it and its content.
    /// - Centred, it is `spec.width` points wide at the reference font scale,
    ///   no wider than the screen allows. As a bottom sheet it spans the
    ///   screen and is as tall as its content, up to the screen's height
    ///   less the application bar's; full height, it covers the screen.
    /// - Content taller than the room scrolls, below a fixed title.
    /// - It sits within egui's content rectangle, which the web shell keeps
    ///   clear of the home-screen app's safe areas and the on-screen
    ///   keyboard.
    pub fn dialog<A: ActionKey, R>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: &DialogSpec<'_, A>,
        availability: impl Fn(ActionTarget<A>, &ActionArgument) -> Availability,
        content: impl FnOnce(&mut Ui, &mut PresentationContext) -> R,
    ) -> DialogResponse<A, R> {
        self.check_pass(ui);
        let tokens = self.tokens;
        let font_scale = self.font_scale;
        let dismiss = spec.dismiss.into_target(self.pane);
        let dismiss_availability = availability(dismiss, &ActionArgument::None);
        let placement = DialogPlacement::for_layout(
            spec.layout,
            spec.sheet_height,
            ui.ctx().content_rect().width(),
        );
        let geometry = Geometry::new(ui, placement, spec.width, &tokens, font_scale);
        let close_content = ActionButtonContent::IconOnly(IconId::Close);
        let close_width = if dismiss_availability.visible() {
            action_button_size(
                ui,
                dismiss.action.specification().label,
                close_content,
                &tokens,
                font_scale,
            )
            .intrinsic
        } else {
            0.0
        };
        let scope = self.scope.child(key);
        let close_identity = scope.instance("stobie.dialog.close").action_identity();
        let pass = self.present_modal(
            ui,
            Chrome {
                scope,
                title: spec.title,
                description: None,
                role: UiRole::Dialog,
                geometry,
                close_width,
                footer_height: 0.0,
                escape_dismisses: dismiss_availability.enabled(),
            },
            content,
            |ui, child, slots| {
                if !dismiss_availability.visible() {
                    return ui.interact(Rect::NOTHING, close_identity.widget_id, Sense::hover());
                }
                ui.scope_builder(UiBuilder::new().max_rect(slots.close), |ui| {
                    child.present_action(
                        ui,
                        &close_identity,
                        ActionButtonSpec {
                            target: dismiss,
                            availability: dismiss_availability.clone(),
                            state: ActionButtonState::Momentary,
                            emphasis: ActionEmphasis::QuietBorderless,
                            compact: false,
                        },
                        close_content,
                        None,
                        None,
                    )
                })
                .inner
            },
        );
        let dismissed = pass.after.clicked()
            || pass.escape
            || (spec.backdrop_dismisses && dismiss_availability.enabled() && pass.backdrop);
        DialogResponse {
            inner: pass.inner,
            dismissed,
            dismiss,
            close: pass.after,
            placement,
            rect: pass.rect,
        }
    }

    /// Present a confirm dialog and report what it did; hosted applications
    /// use [`Frame::confirm`](crate::Frame::confirm), which also triggers the
    /// chosen action. Present it in every pass while it is open.
    ///
    /// - It is observed as a [`UiRole::AlertDialog`] named by its title and
    ///   described by its message, holding the message and its two controls,
    ///   and is centred at every width.
    /// - Cancel takes focus when it opens, and comes first: cancel then
    ///   confirm, at the end of a row, or stacked across the dialog when the
    ///   row does not fit.
    /// - Escape cancels while cancel is enabled. A press on the scrim does
    ///   nothing.
    pub fn confirm<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: &ConfirmSpec<'_, A>,
        availability: impl Fn(ActionTarget<A>, &ActionArgument) -> Availability,
    ) -> ConfirmResponse<A> {
        self.check_pass(ui);
        let tokens = self.tokens;
        let font_scale = self.font_scale;
        let confirm = spec.confirm.into_target(self.pane);
        let cancel = spec.cancel.into_target(self.pane);
        let confirm_availability = availability(confirm, &ActionArgument::None);
        let cancel_availability = availability(cancel, &ActionArgument::None);
        let geometry = Geometry::new(
            ui,
            DialogPlacement::Centred,
            CONFIRM_WIDTH,
            &tokens,
            font_scale,
        );
        let hit = action_hit_height(&tokens, font_scale);
        let gap = tokens.spacing.inline.0;
        let width_of = |target: ActionTarget<A>, availability: &Availability| {
            if !availability.visible() {
                return 0.0;
            }
            action_button_size(
                ui,
                target.action.specification().label,
                ActionButtonContent::Text,
                &tokens,
                font_scale,
            )
            .intrinsic
        };
        let cancel_width = width_of(cancel, &cancel_availability);
        let confirm_width = width_of(confirm, &confirm_availability);
        let in_row = cancel_width + gap + confirm_width <= geometry.inner_width;
        let footer_height = if in_row { hit } else { hit * 2.0 + gap };
        let scope = self.scope.child(key);
        let cancel_identity = scope.instance("stobie.confirm.cancel").action_identity();
        let confirm_identity = scope.instance("stobie.confirm.confirm").action_identity();
        let emphasis = spec.confirm_emphasis;
        let message = spec.message;
        let pass = self.present_modal(
            ui,
            Chrome {
                scope,
                title: spec.title,
                description: Some(spec.message),
                role: UiRole::AlertDialog,
                geometry,
                close_width: 0.0,
                footer_height,
                escape_dismisses: cancel_availability.enabled(),
            },
            |ui, child| {
                child.reading_text(
                    ui,
                    "stobie.confirm.message",
                    message,
                    ReadingTextSpec {
                        role: TextRole::Body,
                        interaction: TextInteraction::Inert,
                    },
                );
            },
            |ui, child, slots| {
                let footer = slots.footer;
                let (cancel_rect, confirm_rect) = if in_row {
                    let confirm_rect = Rect::from_min_size(
                        pos2(footer.right() - confirm_width, footer.top()),
                        vec2(confirm_width, hit),
                    );
                    let right = if confirm_width > 0.0 {
                        confirm_rect.left() - gap
                    } else {
                        footer.right()
                    };
                    let cancel_rect = Rect::from_min_size(
                        pos2(right - cancel_width, footer.top()),
                        vec2(cancel_width, hit),
                    );
                    (cancel_rect, confirm_rect)
                } else {
                    (
                        Rect::from_min_size(footer.min, vec2(footer.width(), hit)),
                        Rect::from_min_size(
                            pos2(footer.left(), footer.top() + hit + gap),
                            vec2(footer.width(), hit),
                        ),
                    )
                };
                let mut present = |ui: &mut Ui,
                                   rect: Rect,
                                   identity: &crate::ActionButtonIdentity,
                                   target: ActionTarget<A>,
                                   availability: &Availability,
                                   emphasis: ActionEmphasis| {
                    if !availability.visible() {
                        return ui.interact(Rect::NOTHING, identity.widget_id, Sense::hover());
                    }
                    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                        child.present_action(
                            ui,
                            identity,
                            ActionButtonSpec {
                                target,
                                availability: availability.clone(),
                                state: ActionButtonState::Momentary,
                                emphasis,
                                compact: false,
                            },
                            ActionButtonContent::Text,
                            None,
                            None,
                        )
                    })
                    .inner
                };
                let cancel_response = present(
                    ui,
                    cancel_rect,
                    &cancel_identity,
                    cancel,
                    &cancel_availability,
                    ActionEmphasis::Normal,
                );
                let confirm_response = present(
                    ui,
                    confirm_rect,
                    &confirm_identity,
                    confirm,
                    &confirm_availability,
                    emphasis,
                );
                if slots.newly_shown && cancel_availability.enabled() {
                    cancel_response.request_focus();
                }
                (cancel_response, confirm_response)
            },
        );
        let (cancel_response, confirm_response) = pass.after;
        ConfirmResponse {
            confirmed: confirm_response.clicked(),
            cancelled: cancel_response.clicked() || pass.escape,
            confirm,
            cancel,
            confirm_response,
            cancel_response,
            rect: pass.rect,
        }
    }

    /// Present the shared modal surface: the scrim, the surface, its title,
    /// the scrolling body and whatever the recipe adds after it, with the
    /// dialog's semantic and AccessKit nodes and its observation region.
    fn present_modal<R, F>(
        &mut self,
        ui: &mut Ui,
        chrome: Chrome<'_>,
        body: impl FnOnce(&mut Ui, &mut PresentationContext) -> R,
        after: impl FnOnce(&mut Ui, &mut PresentationContext, Slots) -> F,
    ) -> ModalPass<R, F> {
        let context = ui.ctx().clone();
        let tokens = self.tokens;
        let font_scale = self.font_scale;
        let Geometry {
            placement,
            screen,
            width,
            inner_width,
            max_height,
            padding,
        } = chrome.geometry;
        let identity = chrome.scope.instance("stobie.dialog");
        let semantic_id = identity.semantic_id();
        let area_id = identity.egui_id();

        // The height measured last pass places the surface; a change is laid
        // out again at once.
        let height_id = area_id.with("stobie.dialog.height");
        let height = match placement {
            DialogPlacement::FullScreen => max_height,
            _ => context
                .data(|data| data.get_temp::<f32>(height_id))
                .unwrap_or(max_height)
                .min(max_height),
        };
        let left_top = match placement {
            DialogPlacement::Centred => pos2(
                (screen.center().x - width / 2.0).max(screen.left()),
                (screen.center().y - height / 2.0).max(screen.top()),
            ),
            DialogPlacement::BottomSheet => pos2(screen.left(), screen.bottom() - height),
            DialogPlacement::FullScreen => screen.min,
        }
        .round();
        // A dialog absent from the previous frame has just opened.
        let shown_id = area_id.with("stobie.dialog.shown");
        let frame = context.cumulative_frame_nr();
        let newly_shown = context.data_mut(|data| {
            let last = data.get_temp::<u64>(shown_id);
            data.insert_temp(shown_id, frame);
            last.is_none_or(|last| last + 1 < frame)
        });

        let radius = tokens.geometry.control_radius.0.round().clamp(0.0, 255.0) as u8;
        let corner_radius = match placement {
            DialogPlacement::Centred => CornerRadius::same(radius),
            DialogPlacement::BottomSheet => CornerRadius {
                nw: radius,
                ne: radius,
                sw: 0,
                se: 0,
            },
            DialogPlacement::FullScreen => CornerRadius::ZERO,
        };
        let surface_frame = egui::Frame::NONE
            .fill(tokens.colours.surface_panel.into())
            .stroke(Stroke::new(BORDER, tokens.colours.border_control))
            .corner_radius(corner_radius)
            .inner_margin(egui::Margin::same(padding.round().clamp(0.0, 127.0) as i8));
        let scrim =
            egui::Color32::from(tokens.colours.surface_canvas).linear_multiply(SCRIM_OPACITY);
        let area = egui::Area::new(area_id)
            .kind(egui::UiKind::Modal)
            .sense(Sense::hover())
            .order(egui::Order::Foreground)
            .interactable(true)
            .constrain(false)
            .fixed_pos(left_top);
        let hosted = crate::observation::is_enabled(&context, self.viewport);
        let hit = action_hit_height(&tokens, font_scale);
        let gap = tokens.spacing.block.0.max(tokens.spacing.unit.0);
        let heading_height = TextRole::SectionHeading
            .style(&tokens, font_scale)
            .line_height
            + tokens.spacing.block.0;
        let header_height = hit.max(heading_height);
        let footer_part = if chrome.footer_height > 0.0 {
            gap + chrome.footer_height
        } else {
            0.0
        };
        let body_max =
            (max_height - padding * 2.0 - BORDER * 2.0 - header_height - gap - footer_part)
                .max(hit);
        let parent = self.parent.clone();
        let domain_reference = self.domain_reference.clone();
        let pane = self.pane;

        let modal = egui::Modal::new(area_id)
            .area(area)
            .frame(surface_frame)
            .backdrop_color(scrim)
            .show(&context, |content| {
                content.set_width(inner_width);
                let registration = DialogRegistration {
                    id: semantic_id.clone(),
                    layer: content.layer_id(),
                    widget: content.unique_id(),
                };
                crate::observation::within_dialog(&context, registration, || {
                    let mut child = PresentationContext::new(
                        content,
                        tokens,
                        font_scale,
                        chrome.scope.child("stobie.dialog.content"),
                        semantic_id.clone(),
                    );
                    child.domain_reference = domain_reference;
                    child.pane = pane;
                    // The body scrolls, so only what is in view is observed.
                    child.visible_only = true;

                    // The title, with room at its end for a close control.
                    let (_, header) = content.allocate_space(vec2(inner_width, header_height));
                    let close = Rect::from_min_size(
                        pos2(
                            header.right() - chrome.close_width,
                            header.top() + (header_height - hit) / 2.0,
                        ),
                        vec2(chrome.close_width, hit),
                    );
                    let title_right = if chrome.close_width > 0.0 {
                        close.left() - gap
                    } else {
                        header.right()
                    };
                    let title_rect = Rect::from_min_max(
                        pos2(
                            header.left(),
                            header.top() + (header_height - heading_height) / 2.0,
                        ),
                        pos2(
                            title_right.max(header.left() + 1.0),
                            header.top() + (header_height + heading_height) / 2.0,
                        ),
                    );
                    content.scope_builder(UiBuilder::new().max_rect(title_rect), |ui| {
                        child.heading(ui, "stobie.dialog.title", chrome.title)
                    });
                    content.add_space(gap);

                    // The body, which scrolls when it is taller than the room.
                    let mut scroll = egui::ScrollArea::vertical()
                        .id_salt(area_id.with("stobie.dialog.body"))
                        .max_height(body_max)
                        .auto_shrink([false, true]);
                    if placement == DialogPlacement::FullScreen {
                        scroll = scroll
                            .min_scrolled_height(body_max)
                            .auto_shrink([false, false]);
                    }
                    let inner = scroll
                        .show(content, |ui| {
                            ui.set_width(ui.available_width());
                            body(ui, &mut child)
                        })
                        .inner;

                    // Fixed controls below the body.
                    let footer = if chrome.footer_height > 0.0 {
                        content.add_space(gap);
                        content
                            .allocate_space(vec2(inner_width, chrome.footer_height))
                            .1
                    } else {
                        Rect::NOTHING
                    };
                    // The close control comes after the content, so keyboard
                    // focus starts in the content.
                    let after = after(
                        content,
                        &mut child,
                        Slots {
                            close,
                            footer,
                            newly_shown,
                        },
                    );

                    let surface = content.min_rect().expand(padding + BORDER);
                    content
                        .ctx()
                        .accesskit_node_builder(content.unique_id(), |node| {
                            node.set_role(match chrome.role {
                                UiRole::AlertDialog => egui::accesskit::Role::AlertDialog,
                                _ => egui::accesskit::Role::Dialog,
                            });
                            node.set_modal();
                            node.set_label(chrome.title);
                            if let Some(description) = chrome.description {
                                node.set_description(description);
                            }
                            node.set_author_id(semantic_id.0.clone());
                            node.set_bounds(egui::accesskit::Rect {
                                x0: surface.min.x.into(),
                                y0: surface.min.y.into(),
                                x1: surface.max.x.into(),
                                y1: surface.max.y.into(),
                            });
                        });
                    // The dialog's node leads the nodes it holds.
                    let mut node = UiNode::container(
                        semantic_id.clone(),
                        Some(parent),
                        chrome.role,
                        surface.into(),
                    );
                    node.name = chrome.title.to_owned();
                    node.description = chrome.description.map(ToOwned::to_owned);
                    node.pane = pane;
                    child.semantic_nodes.insert(0, node);
                    child.node_clips.insert(0, content.clip_rect());
                    // A hosted dialog registers inside its region; otherwise
                    // this context publishes what it presented.
                    let unhosted = if hosted {
                        drop(child);
                        None
                    } else {
                        Some(child)
                    };
                    (inner, after, surface, unhosted)
                })
            });
        let (inner, after, rect, unhosted) = modal.inner;
        if let Some(mut child) = unhosted {
            self.text_layouts.append(&mut child.text_layouts);
            self.semantic_nodes.append(&mut child.semantic_nodes);
            self.node_clips.append(&mut child.node_clips);
            self.raw_presentations.append(&mut child.raw_presentations);
        }

        let measured = modal.response.rect.height();
        if placement != DialogPlacement::FullScreen && (measured - height).abs() > 0.5 {
            context.data_mut(|data| data.insert_temp(height_id, measured));
            context.request_discard("a dialog is placed at its measured height");
            if !context.will_discard() {
                context.request_repaint();
            }
        }
        let escape = chrome.escape_dismisses
            && modal.is_top_modal
            && !modal.any_popup_open
            && context
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        ModalPass {
            inner,
            after,
            escape,
            backdrop: modal.backdrop_response.clicked(),
            rect,
        }
    }
}
