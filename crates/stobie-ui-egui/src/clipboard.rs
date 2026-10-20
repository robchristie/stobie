//! The copy recipe's clipboard route and each copy control's state.
//!
//! A copy control asks for a write when it is pressed or invoked. Natively,
//! and in a browser without the web host, the write goes through egui's
//! `CopyText` output, which the platform integration performs; the control
//! then shows success, because it cannot observe a failure. The web host
//! installs [`ClipboardWrites`], which takes each pass's writes at the end of
//! that pass and calls `navigator.clipboard.writeText` itself. eframe runs
//! the pass that handles a pointer release inside the `pointerup` handler, so
//! a tap or click writes inside the user's gesture, as Safari requires. The
//! write's outcome is recorded on its control, which shows it on its next
//! pass.
//!
//! A key press or an activation through the ARIA layer reaches egui only on
//! the pass after its event, outside the gesture, where Safari refuses a
//! write. So the web host also writes inside those events: Enter or Space on
//! the canvas while a copy control has focus, and a click on a copy
//! control's layer element (as VoiceOver's double tap sends), write the text
//! that control presented in the latest pass. The pass that then handles the
//! press does not write again.

use egui::{Context, Id};

/// Where a copy control is in copying its text.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CopyState {
    /// Showing its label, ready to copy.
    #[default]
    Ready,
    /// Waiting for the browser to accept a write.
    Pending,
    /// The text was copied; the control shows its confirmation.
    Copied,
    /// The platform refused the write; the control shows its failure.
    Failed,
}

/// One copy control's state, kept in egui's temporary data.
#[derive(Clone, Debug, Default)]
pub(crate) struct Control {
    pub state: CopyState,
    /// When the outcome shown began, in egui input time, set by the first
    /// pass that shows it.
    pub since: Option<f64>,
    /// The latest write; an earlier write's outcome is ignored.
    pub write: u64,
    /// An agent asked to copy; the control copies on its next pass.
    pub requested: bool,
    /// A hash of the text, kept in memory only, and the revision that
    /// discovery publishes for it, so a binding goes stale when the text
    /// changes without publishing anything derived from the text.
    pub text_hash: Option<u64>,
    pub revision: u64,
    /// The web host wrote inside the event that the next pass handles as a
    /// press, so that pass does not write again.
    pub gestured: bool,
}

fn control_id(widget: Id) -> Id {
    widget.with("stobie.copy")
}

#[cfg(test)]
pub(crate) fn control(context: &Context, widget: Id) -> Control {
    context.data(|data| data.get_temp(control_id(widget)).unwrap_or_default())
}

pub(crate) fn update<R>(
    context: &Context,
    widget: Id,
    change: impl FnOnce(&mut Control) -> R,
) -> R {
    context.data_mut(|data| change(data.get_temp_mut_or_default(control_id(widget))))
}

/// Ask the copy control `widget` to copy its text on its next pass, as a
/// press would.
pub(crate) fn request(context: &Context, widget: Id) {
    update(context, widget, |control| control.requested = true);
}

fn web_writes_id() -> Id {
    Id::new("stobie.clipboard.web-writes")
}

/// Writes queued in the current pass for the web host's plugin.
#[derive(Clone, Default)]
struct Queue(Vec<(Id, u64, String)>);

fn queue_id() -> Id {
    Id::new("stobie.clipboard.queue")
}

fn web_writes(context: &Context) -> bool {
    context.data(|data| data.get_temp::<bool>(web_writes_id()).unwrap_or(false))
}

/// An enabled copy control presented in a pass: its button, the control and
/// the text, kept in memory only.
#[derive(Clone)]
struct Target {
    button: Id,
    widget: Id,
    text: String,
}

/// The copy controls presented in the pass under way and in the latest
/// finished pass, for the web host's writes inside an event.
#[derive(Clone, Default)]
struct Targets {
    current: Vec<Target>,
    latest: Vec<Target>,
}

fn targets_id() -> Id {
    Id::new("stobie.clipboard.targets")
}

/// Record that the enabled copy control `widget`, with button `button`,
/// presented `text` in this pass. Only the web host's route keeps it.
pub(crate) fn present(context: &Context, button: Id, widget: Id, text: &str) {
    if web_writes(context) {
        context.data_mut(|data| {
            data.get_temp_mut_or_default::<Targets>(targets_id())
                .current
                .push(Target {
                    button,
                    widget,
                    text: text.to_owned(),
                });
        });
    }
}

/// Write now, between passes, for the copy control presented in the latest
/// pass whose button `chosen` picks, and mark the control so the next pass
/// does not write again. Returns whether it queued a write.
#[cfg_attr(not(all(target_arch = "wasm32", feature = "eframe")), allow(dead_code))]
fn write_between_passes(context: &Context, chosen: impl Fn(Id) -> bool) -> bool {
    let target = context.data(|data| {
        data.get_temp::<Targets>(targets_id()).and_then(|targets| {
            targets
                .latest
                .into_iter()
                .find(|target| chosen(target.button))
        })
    });
    let Some(target) = target else {
        return false;
    };
    write(context, target.widget, &target.text);
    update(context, target.widget, |control| control.gestured = true);
    true
}

/// Write `text` for the copy control `widget`: queue it for the web host's
/// plugin, or hand it to egui and count it copied.
pub(crate) fn write(context: &Context, widget: Id, text: &str) {
    let write = update(context, widget, |control| {
        control.write += 1;
        control.state = CopyState::Pending;
        control.since = None;
        control.write
    });
    if web_writes(context) {
        context.data_mut(|data| {
            data.get_temp_mut_or_default::<Queue>(queue_id()).0.push((
                widget,
                write,
                text.to_owned(),
            ));
        });
    } else {
        context.copy_text(text.to_owned());
        settle(context, widget, write, true);
    }
}

/// Record a write's outcome on its control, unless a later write replaced it.
pub(crate) fn settle(context: &Context, widget: Id, write: u64, copied: bool) {
    update(context, widget, |control| {
        if control.write == write {
            control.state = if copied {
                CopyState::Copied
            } else {
                CopyState::Failed
            };
            control.since = None;
        }
    });
}

/// Take the writes queued since the last call.
#[cfg_attr(not(all(target_arch = "wasm32", feature = "eframe")), allow(dead_code))]
fn take_queue(context: &Context) -> Vec<(Id, u64, String)> {
    context
        .data_mut(|data| std::mem::take(&mut data.get_temp_mut_or_default::<Queue>(queue_id()).0))
}

/// The web host's clipboard route: each pass's copy writes, made at the end
/// of that pass with their outcomes recorded.
#[cfg(all(target_arch = "wasm32", feature = "eframe"))]
pub(crate) struct ClipboardWrites;

#[cfg(all(target_arch = "wasm32", feature = "eframe"))]
impl ClipboardWrites {
    pub(crate) fn install(context: &Context) {
        context.data_mut(|data| data.insert_temp(web_writes_id(), true));
        context.add_plugin(Self);
    }
}

#[cfg(all(target_arch = "wasm32", feature = "eframe"))]
impl egui::Plugin for ClipboardWrites {
    fn debug_name(&self) -> &'static str {
        "stobie.clipboard"
    }

    fn on_begin_pass(&mut self, ui: &mut egui::Ui) {
        ui.ctx().data_mut(|data| {
            data.get_temp_mut_or_default::<Targets>(targets_id())
                .current
                .clear();
        });
    }

    fn output_hook(&mut self, context: &egui::Context, _output: &mut egui::FullOutput) {
        flush(context);
        context.data_mut(|data| {
            let targets = data.get_temp_mut_or_default::<Targets>(targets_id());
            targets.latest = std::mem::take(&mut targets.current);
        });
    }
}

/// Inside a key press on the canvas: Enter or Space presses the focused egui
/// widget on the next pass, so if that is a copy control, write now.
#[cfg(all(target_arch = "wasm32", feature = "eframe"))]
pub(crate) fn key_pressed(context: &Context, key: &str) {
    if !matches!(key, "Enter" | " ") {
        return;
    }
    if let Some(focused) = context.memory(|memory| memory.focused())
        && write_between_passes(context, |button| button == focused)
    {
        flush(context);
    }
}

/// Make the writes queued between passes now, inside the event.
#[cfg(all(target_arch = "wasm32", feature = "eframe"))]
fn flush(context: &Context) {
    for (widget, write, text) in take_queue(context) {
        write_web(context, widget, write, &text);
    }
}

/// Inside a click on the ARIA layer's element for AccessKit node `node`: if
/// it is a copy control's button, write now. The click's request still
/// reaches egui, which presses the control without writing again.
#[cfg(all(target_arch = "wasm32", feature = "eframe"))]
pub(crate) fn layer_clicked(context: &Context, node: egui::accesskit::NodeId) {
    if write_between_passes(context, |button| button.accesskit_id() == node) {
        flush(context);
    }
}

/// Call `navigator.clipboard.writeText` now, inside the event handler running
/// this pass, and record the promise's outcome when it settles.
#[cfg(all(target_arch = "wasm32", feature = "eframe"))]
fn write_web(context: &Context, widget: Id, write: u64, text: &str) {
    use wasm_bindgen::JsCast;
    // The clipboard exists only in a secure context.
    let clipboard = web_sys::window()
        .filter(web_sys::Window::is_secure_context)
        .and_then(|window| js_sys::Reflect::get(&window.navigator(), &"clipboard".into()).ok())
        .filter(|clipboard| !clipboard.is_undefined() && !clipboard.is_null())
        .map(JsCast::unchecked_into::<web_sys::Clipboard>);
    let Some(clipboard) = clipboard else {
        settle(context, widget, write, false);
        context.request_repaint();
        return;
    };
    let promise = clipboard.write_text(text);
    let context = context.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let copied = wasm_bindgen_futures::JsFuture::from(promise).await.is_ok();
        settle(&context, widget, write, copied);
        context.request_repaint();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_write_replaces_an_earlier_outcome() {
        let context = Context::default();
        let widget = Id::new("copy");
        context.data_mut(|data| data.insert_temp(web_writes_id(), true));
        write(&context, widget, "first");
        write(&context, widget, "second");
        let queued = take_queue(&context);
        assert_eq!(
            queued
                .iter()
                .map(|(_, write, text)| (*write, text.as_str()))
                .collect::<Vec<_>>(),
            [(1, "first"), (2, "second")]
        );
        assert_eq!(control(&context, widget).state, CopyState::Pending);
        settle(&context, widget, 1, false);
        assert_eq!(control(&context, widget).state, CopyState::Pending);
        settle(&context, widget, 2, true);
        assert_eq!(control(&context, widget).state, CopyState::Copied);
        assert!(take_queue(&context).is_empty());
    }

    #[test]
    fn a_write_between_passes_uses_the_latest_pass_and_is_not_repeated() {
        let context = Context::default();
        let (button, widget) = (Id::new("button"), Id::new("copy"));
        context.data_mut(|data| data.insert_temp(web_writes_id(), true));
        present(&context, button, widget, "ABC-123");
        // Nothing is chosen before a pass finishes.
        assert!(!write_between_passes(&context, |id| id == button));
        context.data_mut(|data| {
            let targets = data.get_temp_mut_or_default::<Targets>(targets_id());
            targets.latest = std::mem::take(&mut targets.current);
        });
        assert!(!write_between_passes(&context, |id| id == widget));
        assert!(write_between_passes(&context, |id| id == button));
        let queued = take_queue(&context);
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].2, "ABC-123");
        let control = control(&context, widget);
        assert!(control.gestured);
        assert_eq!(control.state, CopyState::Pending);
    }

    #[test]
    fn without_the_web_route_nothing_is_kept_for_writes_between_passes() {
        let context = Context::default();
        present(&context, Id::new("button"), Id::new("copy"), "ABC-123");
        assert!(
            context
                .data(|data| data.get_temp::<Targets>(targets_id()))
                .is_none()
        );
    }

    #[test]
    fn without_the_web_route_a_write_is_egui_output_and_counts_as_copied() {
        let context = Context::default();
        let widget = Id::new("copy");
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            write(ui.ctx(), widget, "ABC-123");
        });
        output.textures_delta.clear();
        assert!(output.platform_output.commands.iter().any(
            |command| matches!(command, egui::OutputCommand::CopyText(text) if text == "ABC-123")
        ));
        assert_eq!(control(&context, widget).state, CopyState::Copied);
        assert!(take_queue(&context).is_empty());
    }
}
