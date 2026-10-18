//! Intents from outside the host's passes: socket messages, timer ticks and
//! fetch results.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

/// The host's queue of intents sent from outside its passes.
pub(super) type IntentQueue<I> = Arc<Mutex<VecDeque<I>>>;

pub(super) fn lock<I>(queue: &Mutex<VecDeque<I>>) -> MutexGuard<'_, VecDeque<I>> {
    // A sender that panicked mid-push leaves a valid queue.
    queue.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Sends intents into a running [`AppHost`](super::AppHost) from outside its
/// passes, and wakes it.
///
/// The host gives one to [`Application::started`](super::Application::started)
/// and returns clones from [`AppHost::intent_sender`](super::AppHost::intent_sender).
/// Hand clones to the transports that receive external events: a browser
/// WebSocket's message callback, a timer, a fetch future, or a native thread.
/// A sender is `Send` and `Sync` when the intent is `Send`, so it crosses
/// threads natively; in the browser everything runs on the page's thread.
///
/// Each sent intent waits in the host's queue. The host applies queued intents
/// in the order they were sent, through [`Application::apply`](super::Application::apply),
/// at the start of its next pass, before `prepare` and the UI, so that pass
/// presents them. Sending into an empty queue asks egui for that pass; further
/// intents sent before it runs join the same pass. Natively, eframe also
/// applies them while the window is minimised, without presenting. In the
/// browser, eframe runs no pass while the page is hidden, so intents wait until
/// it is shown.
///
/// Every send therefore costs one pass, even when its intent changes nothing.
/// Filter transport traffic such as heartbeats and acknowledgements before it
/// reaches the sender, and send the events that can change presented state.
pub struct IntentSender<I> {
    queue: Weak<Mutex<VecDeque<I>>>,
    context: egui::Context,
}

impl<I> IntentSender<I> {
    pub(super) fn new(queue: &IntentQueue<I>, context: &egui::Context) -> Self {
        Self {
            queue: Arc::downgrade(queue),
            context: context.clone(),
        }
    }

    /// Queue `intent` for the host's next pass, waking the host if nothing
    /// was queued. Fails, returning the intent, once the host has stopped,
    /// for example after `WebHandle::destroy` or when the native window
    /// closed; a transport should then close its connection.
    pub fn send(&self, intent: I) -> Result<(), HostStopped<I>> {
        let Some(queue) = self.queue.upgrade() else {
            return Err(HostStopped(intent));
        };
        let wake = {
            let mut queue = lock(&queue);
            queue.push_back(intent);
            queue.len() == 1
        };
        if wake {
            self.context.request_repaint();
        }
        Ok(())
    }

    /// Whether the host has stopped, so that sends fail.
    pub fn is_stopped(&self) -> bool {
        self.queue.strong_count() == 0
    }
}

impl<I> Clone for IntentSender<I> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            context: self.context.clone(),
        }
    }
}

impl<I> fmt::Debug for IntentSender<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IntentSender")
            .field("stopped", &self.is_stopped())
            .finish_non_exhaustive()
    }
}

/// The intent an [`IntentSender`] could not deliver because its host has
/// stopped.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct HostStopped<I>(pub I);

// Intents need not implement `Debug`, so the error does not show its intent.
impl<I> fmt::Debug for HostStopped<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HostStopped(..)")
    }
}

impl<I> fmt::Display for HostStopped<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the application host has stopped")
    }
}

impl<I> std::error::Error for HostStopped<I> {}
