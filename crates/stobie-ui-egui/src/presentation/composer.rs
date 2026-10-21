//! The composer recipe: a message field that grows with its text, with a send
//! control and optional leading and trailing controls.

use std::hash::Hash;

use egui::{Key, Rect, Response, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use stobie_core::PaneId;

use super::PresentationContext;
use crate::{
    ActionArgument, ActionButtonContent, ActionButtonSpec, ActionButtonState, ActionEmphasis,
    ActionKey, ActionTarget, ArgumentContract, Availability, IconId, IntoActionTarget,
    NativeTextControlKind, NativeTextEdit, TextRole, TextRoleStyles, UiNode, UiRole,
    components::{action_button_size, action_hit_height},
};

/// Lines a composer's field grows to before it scrolls, unless its spec says.
pub const COMPOSER_DEFAULT_MAX_ROWS: usize = 6;

/// One control of a composer: the action it performs and how it shows,
/// as an icon, a label or both. An icon-only control is named by its
/// action's label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComposerControl<A> {
    action: A,
    pane: Option<PaneId>,
    content: ActionButtonContent,
}

impl<A: ActionKey> ComposerControl<A> {
    /// The action shown as an icon, named by its label.
    pub fn icon(action: A, icon: IconId) -> Self {
        Self::new(action, ActionButtonContent::IconOnly(icon))
    }

    /// The action shown as its label.
    pub fn label(action: A) -> Self {
        Self::new(action, ActionButtonContent::Text)
    }

    /// The action shown as an icon followed by its label.
    pub fn icon_label(action: A, icon: IconId) -> Self {
        Self::new(action, ActionButtonContent::IconLabel(icon))
    }

    fn new(action: A, content: ActionButtonContent) -> Self {
        Self {
            action,
            pane: None,
            content,
        }
    }

    /// Perform a pane-scoped action for `pane`, rather than for the
    /// presentation's own pane.
    pub fn in_pane(mut self, pane: PaneId) -> Self {
        self.pane = Some(pane);
        self
    }

    /// The action this control performs.
    pub fn action(&self) -> A {
        self.action
    }

    /// How the control shows.
    pub fn content(&self) -> ActionButtonContent {
        self.content
    }

    fn target(&self, pane: Option<PaneId>) -> ActionTarget<A> {
        self.action.into_target(self.pane.or(pane))
    }
}

/// A composer: a multiline field for a draft the application owns, a send
/// control, and optional controls before the field (such as attach) and
/// after it (such as a microphone).
///
/// The send action takes a text argument (`#[action(argument = "text")]`):
/// sending triggers it with the draft. Its availability for the draft,
/// [`Application::availability_with`](crate::Application::availability_with),
/// decides whether it can send: return `Disabled` with a reason for a draft
/// with nothing to send, or `Hidden` to show the trailing control alone in
/// its place.
#[derive(Clone, Copy, Debug)]
pub struct ComposerSpec<'a, A> {
    /// The field's accessible name, such as "Message".
    pub name: &'a str,
    /// Text shown while the draft is empty, such as "Message Ada".
    pub hint: &'a str,
    /// Lines the field grows to before it scrolls (at least one).
    pub max_rows: usize,
    /// The control before the field, if any.
    pub leading: Option<ComposerControl<A>>,
    /// The control after the field, before send, if any.
    pub trailing: Option<ComposerControl<A>>,
    /// The send control, whose action takes the draft as its text argument.
    pub send: ComposerControl<A>,
}

impl<'a, A> ComposerSpec<'a, A> {
    /// A composer named `name` with only a send control.
    pub fn new(name: &'a str, send: ComposerControl<A>) -> Self {
        Self {
            name,
            hint: "",
            max_rows: COMPOSER_DEFAULT_MAX_ROWS,
            leading: None,
            trailing: None,
            send,
        }
    }

    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    pub fn max_rows(mut self, rows: usize) -> Self {
        self.max_rows = rows;
        self
    }

    pub fn leading(mut self, control: ComposerControl<A>) -> Self {
        self.leading = Some(control);
        self
    }

    pub fn trailing(mut self, control: ComposerControl<A>) -> Self {
        self.trailing = Some(control);
        self
    }
}

/// One of a composer's controls as presented: its resolved target and its
/// response. A hidden control's response has an empty rectangle.
#[derive(Clone, Debug)]
pub struct ComposerControlResponse<A: ActionKey> {
    pub target: ActionTarget<A>,
    pub response: Response,
}

/// What a composer did in one pass.
#[derive(Clone, Debug)]
pub struct ComposerResponse<A: ActionKey> {
    /// The field's response; its rectangle is the part of the field shown.
    pub field: Response,
    /// Whether this pass's typing or pasting changed the draft.
    pub changed: bool,
    /// The draft to send, when Return or the send control sent it while
    /// send was available. Trigger the send action with it as its text.
    pub submitted: Option<String>,
    pub leading: Option<ComposerControlResponse<A>>,
    pub trailing: Option<ComposerControlResponse<A>>,
    pub send: ComposerControlResponse<A>,
}

impl PresentationContext {
    /// Present a composer editing `draft`, and report what it did; hosted
    /// applications use [`Frame::composer`](crate::Frame::composer), which
    /// also emits the edit and triggers the actions. `availability` gives
    /// each control's availability; send's is asked for with the draft as
    /// its text argument.
    ///
    /// - The field starts at one line, grows with its text up to
    ///   `spec.max_rows` lines and then scrolls. In the browser it is a
    ///   [`NativeTextEdit`], so on a touch device a tap makes it the
    ///   platform's own field.
    /// - With a keyboard, Return sends and Shift+Return types a newline;
    ///   Cmd or Ctrl+Return also sends. In an active native field on a touch
    ///   device, Return is the platform's and types a newline, and the send
    ///   control sends; a hardware keyboard's Cmd+Return still sends.
    /// - Return sends only while send is enabled for the draft; otherwise
    ///   it does nothing. Holding Return sends once.
    /// - The field keeps focus when the send control is pressed, and takes
    ///   it after a send, so typing continues and a touch keyboard stays up.
    /// - The field is observed as a multiline text input named `spec.name`,
    ///   so agents set its text with `set-text`. The send control carries
    ///   the draft as its argument, so an agent invokes it by its ID, or by
    ///   the action with the draft as its value, without one.
    ///
    /// # Panics
    ///
    /// If the send action does not take a text argument.
    pub fn composer<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: &ComposerSpec<'_, A>,
        draft: &mut String,
        availability: impl Fn(ActionTarget<A>, &ActionArgument) -> Availability,
    ) -> ComposerResponse<A> {
        self.check_pass(ui);
        let send_target = spec.send.target(self.pane);
        assert!(
            matches!(send_target.action.argument(), ArgumentContract::Text { .. }),
            "a composer's send action `{}` must take a text argument",
            send_target.action.stable_id()
        );
        let scope = self.scope.child(key);
        let field = scope.instance("field");
        let field_id = field.egui_id();
        let tokens = self.tokens;
        let font_scale = self.font_scale;

        // Availability before this pass's edits lays the controls out; a
        // change that hides or shows one lays them out again next pass.
        let send_availability = |text: &str| {
            let argument = ActionArgument::Text(text.to_owned());
            match availability(send_target, &argument) {
                Availability::Enabled => match send_target.action.argument().validate(&argument) {
                    Ok(()) => Availability::Enabled,
                    Err(error) => Availability::Disabled {
                        reason: error.message.into(),
                    },
                },
                other => other,
            }
        };
        let leading = spec.leading.map(|control| {
            let target = control.target(self.pane);
            (control, target, availability(target, &ActionArgument::None))
        });
        let trailing = spec.trailing.map(|control| {
            let target = control.target(self.pane);
            (control, target, availability(target, &ActionArgument::None))
        });
        let before = send_availability(draft);

        let pad = tokens.spacing.unit.0;
        let gap = tokens.spacing.unit.0;
        // Each shown control's width and hit height; an icon-only control's
        // touch target is taller than a labelled one's.
        let size_of = |control: &ComposerControl<A>, availability: &Availability| {
            if !availability.visible() {
                return (0.0, 0.0);
            }
            let label = control.action.specification().label;
            let size = action_button_size(ui, label, control.content, &tokens, font_scale);
            (size.intrinsic, size.hit_height)
        };
        let (leading_width, leading_hit) = leading
            .as_ref()
            .map_or((0.0, 0.0), |(control, _, availability)| {
                size_of(control, availability)
            });
        let (trailing_width, trailing_hit) = trailing
            .as_ref()
            .map_or((0.0, 0.0), |(control, _, availability)| {
                size_of(control, availability)
            });
        let (send_width, send_hit) = size_of(&spec.send, &before);
        // The controls' row, which one line of the field fills.
        let hit = action_hit_height(&tokens, font_scale)
            .max(leading_hit)
            .max(trailing_hit)
            .max(send_hit);

        // The field's height last pass bottom-aligns the controls before it.
        let height_id = field_id.with("stobie.composer.height");
        let last_height = ui
            .ctx()
            .data(|data| data.get_temp::<f32>(height_id))
            .unwrap_or(hit)
            .max(hit);
        let width = ui.available_width();
        let surface = Rect::from_min_size(ui.cursor().min, vec2(width, last_height + pad * 2.0));
        let background = ui.painter().add(egui::Shape::Noop);
        let inner = surface.shrink(pad);
        let control_rect = |left: f32, width: f32, bottom: f32| {
            Rect::from_min_size(pos2(left, bottom - hit), vec2(width, hit))
        };
        let present = |context: &mut Self,
                       ui: &mut Ui,
                       rect: Rect,
                       name: &'static str,
                       control: &ComposerControl<A>,
                       target: ActionTarget<A>,
                       availability: Availability,
                       emphasis: ActionEmphasis,
                       argument: Option<serde_json::Value>| {
            let identity = scope.instance(name).action_identity();
            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                context.present_action(
                    ui,
                    &identity,
                    ActionButtonSpec {
                        target,
                        availability,
                        state: ActionButtonState::Momentary,
                        emphasis,
                        compact: false,
                    },
                    control.content,
                    None,
                    argument,
                )
            })
            .inner
        };

        let mut left = inner.min.x;
        let leading = leading.map(|(control, target, availability)| {
            let rect = control_rect(left, leading_width, inner.max.y);
            if leading_width > 0.0 {
                left += leading_width + gap;
            }
            let response = present(
                self,
                ui,
                rect,
                "leading",
                &control,
                target,
                availability,
                ActionEmphasis::QuietBorderless,
                None,
            );
            ComposerControlResponse { target, response }
        });
        let mut right = inner.max.x;
        let send_left = right - send_width;
        if send_width > 0.0 {
            right = send_left - gap;
        }
        let trailing_left = right - trailing_width;
        if trailing_width > 0.0 {
            right = trailing_left - gap;
        }

        // The field.
        let had_focus = ui.memory(|memory| memory.has_focus(field_id));
        let body = TextRole::Body.style(&tokens, font_scale);
        let line_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&body.font_id));
        // One line fills the controls' height.
        let spare = (hit - line_height).max(0.0);
        let top = (spare / 2.0).floor().min(127.0);
        let bottom = (spare - top).ceil().min(127.0);
        let horizontal = tokens
            .geometry
            .control_padding_x
            .0
            .round()
            .clamp(0.0, 127.0) as i8;
        let field_rect = Rect::from_min_max(
            pos2(left, inner.min.y),
            pos2(right.max(left + 1.0), f32::INFINITY),
        );
        let before_text = draft.clone();
        let (field_response, ()) = ui
            .scope_builder(UiBuilder::new().max_rect(field_rect), |ui| {
                self.native(ui, NativeTextControlKind::TextEdit, |ui| {
                    let response = NativeTextEdit::multiline(draft, field_id)
                        .font(body.font_id.clone())
                        .hint_text(spec.hint)
                        .desired_rows(1)
                        .max_rows(spec.max_rows)
                        .desired_width(field_rect.width())
                        .margin(egui::Margin {
                            left: horizontal,
                            right: horizontal,
                            top: top as i8,
                            bottom: bottom as i8,
                        })
                        .frame(false)
                        .return_key(egui::KeyboardShortcut::new(
                            egui::Modifiers::SHIFT,
                            Key::Enter,
                        ))
                        .show(ui);
                    (response, ())
                })
            })
            .inner;
        let changed = *draft != before_text;
        let field_height = field_response.rect.height().max(hit);
        if (field_height - last_height).abs() > 0.5 {
            ui.ctx()
                .data_mut(|data| data.insert_temp(height_id, field_height));
            ui.ctx().request_repaint();
        }
        let semantic_id = field.semantic_id();
        ui.ctx().accesskit_node_builder(field_response.id, |node| {
            node.set_label(spec.name);
            node.set_author_id(semantic_id.0.clone());
        });
        let mut node = UiNode::container(
            semantic_id,
            Some(self.parent.clone()),
            UiRole::MultilineTextInput,
            field_response.rect.into(),
        );
        node.name = spec.name.to_owned();
        node.focused = field_response.has_focus();
        node.enabled = ui.is_enabled();
        if self.domain_reference.is_some() {
            node.domain_reference = self.domain_reference.clone();
        }
        if field_response.rect.is_positive() {
            self.push_node(ui, node);
        }
        let returned = had_focus && take_return(ui);

        // The controls after the field, at the bottom of this pass's field.
        let bottom = inner.min.y + field_height;
        let trailing = trailing.map(|(control, target, availability)| {
            let rect = control_rect(trailing_left, trailing_width, bottom);
            let response = present(
                self,
                ui,
                rect,
                "trailing",
                &control,
                target,
                availability,
                ActionEmphasis::QuietBorderless,
                None,
            );
            ComposerControlResponse { target, response }
        });
        let after = send_availability(draft);
        if after.visible() != before.visible() || changed {
            // Show or hide send, and the draft's availability, next pass.
            ui.ctx().request_repaint();
        }
        let send_rect = control_rect(send_left, send_width, bottom);
        let send_response = present(
            self,
            ui,
            send_rect,
            "send",
            &spec.send,
            send_target,
            if before.visible() {
                after.clone()
            } else {
                Availability::Hidden
            },
            ActionEmphasis::Primary,
            Some(serde_json::Value::String(draft.clone())),
        );
        let submitted =
            ((returned && after.enabled()) || send_response.clicked()).then(|| draft.clone());

        // A press on send keeps the field's focus, and a send gives it focus.
        let pointer_on_send = ui.input(|input| {
            (input.pointer.any_down() || input.pointer.any_released())
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|pointer| send_response.interact_rect.contains(pointer))
        });
        if (had_focus && pointer_on_send) || submitted.is_some() {
            ui.memory_mut(|memory| memory.request_focus(field_id));
        }

        // The surface: a text field's ground holding the field and controls.
        let surface = Rect::from_min_size(surface.min, vec2(width, field_height + pad * 2.0));
        let focused = ui.memory(|memory| memory.has_focus(field_id));
        let stroke = Stroke::new(
            1.0,
            if focused {
                tokens.colours.focus_ring
            } else {
                tokens.colours.border_control
            },
        );
        ui.painter().set(
            background,
            egui::epaint::RectShape::new(
                surface,
                tokens.geometry.control_radius.0,
                ui.visuals().text_edit_bg_color(),
                stroke,
                egui::StrokeKind::Inside,
            ),
        );
        ui.allocate_rect(surface, Sense::hover());

        ComposerResponse {
            field: field_response,
            changed,
            submitted,
            leading,
            trailing,
            send: ComposerControlResponse {
                target: send_target,
                response: send_response,
            },
        }
    }
}

/// Take this pass's Return presses that send: Return without Shift or Alt,
/// with or without Cmd or Ctrl. Report whether any was a fresh press, not a
/// repeat. Shift+Return stays for the field, which types a newline.
fn take_return(ui: &Ui) -> bool {
    ui.input_mut(|input| {
        let mut pressed_once = false;
        input.events.retain(|event| match event {
            egui::Event::Key {
                key: Key::Enter,
                pressed,
                repeat,
                modifiers,
                ..
            } if !modifiers.shift && !modifiers.alt => {
                pressed_once |= *pressed && !*repeat;
                false
            }
            _ => true,
        });
        pressed_once
    })
}
