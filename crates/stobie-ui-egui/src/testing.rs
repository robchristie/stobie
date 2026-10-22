//! Host tests for an [`Application`]: an egui_kittest harness that runs the
//! application on an [`AppHost`] as eframe does, in one shared context and
//! at exactly the harness's size. Enable the crate's `test-support` feature
//! in the application's dev-dependencies:
//!
//! ```toml
//! [dev-dependencies]
//! stobie-ui-egui = { path = "...", features = ["test-support"] }
//! ```
//!
//! [`egui_kittest`] is re-exported here, so its queries need no dependency
//! of their own: `use stobie_ui_egui::testing::egui_kittest::kittest::Queryable;`.
//!
//! The [Web Push double](crate::web_push::PushDouble) is behind the same
//! feature.
//!
//! ```text
//! use stobie_ui_egui::testing::host_harness;
//!
//! let mut harness = host_harness(egui::vec2(390.0, 844.0), Notes::default());
//! harness.state().intent_sender().send(NotesIntent::Arrived(note)).unwrap();
//! harness.run();
//! assert_eq!(harness.state().app().notes().len(), 1);
//! ```

use std::ops::{Deref, DerefMut};

pub use egui_kittest;

use egui::{UiBuilder, Vec2};
use egui_kittest::{Harness, HarnessBuilder};

use crate::{AppHost, Application, HostVisibility};

/// A harness running an application on its host; see [`host_harness`].
pub type HostHarness<A> = Harness<'static, TestHost<A>>;

/// The harness's state: the [`AppHost`], which it dereferences to, so
/// `harness.state().app()` and `harness.state_mut().app_mut()` read and
/// change the application.
pub struct TestHost<A: Application> {
    create: Option<Create<A>>,
    host: Option<AppHost<A>>,
}

/// Creates the application from the harness's context.
type Create<A> = Box<dyn FnOnce(&egui::Context) -> A>;

impl<A: Application> Deref for TestHost<A> {
    type Target = AppHost<A>;

    fn deref(&self) -> &AppHost<A> {
        self.host
            .as_ref()
            .expect("the harness creates the host in its first pass")
    }
}

impl<A: Application> DerefMut for TestHost<A> {
    fn deref_mut(&mut self) -> &mut AppHost<A> {
        self.host
            .as_mut()
            .expect("the harness creates the host in its first pass")
    }
}

/// A harness of `size` points running `app` on an [`AppHost`] in the
/// harness's own context, so that sent intents and repaint requests reach
/// the harness. The application is laid out at exactly `size`, with no
/// margin, as eframe lays it out in a window or a browser canvas of that
/// size: the root semantic node's rect is the harness's, and a breakpoint
/// such as [`PaneWidthClass`](crate::PaneWidthClass)'s 720 points flips at
/// that width.
pub fn host_harness<A: Application>(size: impl Into<Vec2>, app: A) -> HostHarness<A> {
    host_harness_with(Harness::builder().with_size(size), move |_| app)
}

/// [`host_harness`] from a builder of the caller's (size, pixels per point,
/// step length), creating the application from the harness's context, as
/// eframe's creator does. An application that needs eframe's creation
/// context can make one with `eframe::CreationContext::_new_kittest`.
///
/// As in eframe, the host exists before the first pass: [`Application::started`]
/// runs with a sender that wakes the harness, and the design system is the
/// application's, applied after the harness's own defaults. Like any
/// egui_kittest harness, it runs until it settles before it returns.
pub fn host_harness_with<A: Application>(
    builder: HarnessBuilder<TestHost<A>>,
    create: impl FnOnce(&egui::Context) -> A + 'static,
) -> HostHarness<A> {
    let state = TestHost {
        create: Some(Box::new(create)),
        host: None,
    };
    builder.build_ui_state(run_pass, state)
}

/// Mark the harness's page or window hidden or visible, as a hidden
/// browser page or a minimised window reports it to eframe. While hidden,
/// each step runs what eframe runs, [`Application::visibility_changed`] and
/// sent intents, and no UI pass.
pub fn set_visibility<A: Application>(harness: &mut HostHarness<A>, visibility: HostVisibility) {
    harness
        .input_mut()
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .occluded = Some(!visibility.is_visible());
}

/// One harness step, as eframe runs it: `App::logic`, then, unless the
/// viewport is hidden, the UI pass over the whole viewport.
fn run_pass<A: Application>(ui: &mut egui::Ui, state: &mut TestHost<A>) {
    let context = ui.ctx().clone();
    let Some(host) = &mut state.host else {
        // eframe creates the application before its first pass; the harness
        // has no context until it runs one, so this pass only creates it.
        let create = state.create.take().expect("the host is created once");
        state.host = Some(AppHost::new(&context, create(&context)));
        context.request_repaint();
        return;
    };
    if host.before_pass(&context) {
        return;
    }
    // egui_kittest frames its page in an 8-point margin; eframe gives the
    // application the whole viewport.
    let mut root = ui.new_child(UiBuilder::new().max_rect(context.viewport_rect()));
    host.present(&mut root);
}

#[cfg(test)]
mod tests;
