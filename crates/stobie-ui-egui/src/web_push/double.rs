//! A native stand-in for the browser's Web Push, for host tests.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::{NotificationPermission, PushEvent, PushState, PushSupport};

/// A call an application made on a [`WebPush`](super::WebPush) connected to
/// a [`PushDouble`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PushCall {
    /// [`WebPush::start`](super::WebPush::start).
    Start,
    /// [`WebPush::subscribe`](super::WebPush::subscribe), with the key as
    /// given.
    Subscribe { application_server_key: String },
    /// [`WebPush::unsubscribe`](super::WebPush::unsubscribe).
    Unsubscribe,
    /// [`WebPush::refresh`](super::WebPush::refresh).
    Refresh,
    /// [`WebPush::set_app_badge`](super::WebPush::set_app_badge).
    SetAppBadge(u64),
}

/// Web Push for host tests, in place of the browser's. While a double is
/// installed on a thread, every [`WebPush::start`](super::WebPush::start) on
/// that thread connects to it instead of reporting push as unsupported: the
/// double records each call, and the test delivers the browser's results.
///
/// - `start` and `refresh` deliver [`PushEvent::State`] at once with the
///   double's state, which is supported, permission not yet asked and no
///   subscription unless [`Self::set_state`] changes it; `support()` and
///   `permission()` read it too.
/// - `subscribe` and `unsubscribe` deliver nothing: the test delivers
///   [`PushEvent::Subscribed`], [`PushEvent::Unsubscribed`] or
///   [`PushEvent::Failed`] with [`Self::deliver`], as the browser would after
///   its permission prompt. A key that is not a VAPID public key fails at
///   once, as it does in the browser.
/// - `set_app_badge` records the count and returns whether badges are
///   available, true unless [`Self::set_badges`] says otherwise.
/// - [`Self::deliver`] also delivers what the service worker would: a
///   [`PushEvent::Route`] for a tapped notification.
///
/// Install the double before the host starts the application, and keep it
/// for the test; dropping it uninstalls it.
///
/// ```text
/// let push = PushDouble::install();
/// let mut harness = host_harness(egui::vec2(390.0, 844.0), Chat::default());
/// harness.get_by_label("Turn on notifications").click();
/// harness.run();
/// assert!(matches!(push.calls()[..], [PushCall::Start, PushCall::Subscribe { .. }]));
/// push.deliver(PushEvent::Subscribed(subscription));
/// harness.run();
/// ```
pub struct PushDouble {
    double: Rc<Double>,
}

/// What a connected [`WebPush`](super::WebPush) shares with its double.
pub(super) struct Double {
    state: RefCell<PushState>,
    badges: Cell<bool>,
    badge: Cell<Option<u64>>,
    calls: RefCell<Vec<PushCall>>,
    /// The latest connected `WebPush`'s delivery, while it lives.
    deliver: RefCell<Option<Delivery>>,
}

/// A connected `WebPush`'s delivery, which the double does not keep alive.
type Delivery = Weak<dyn Fn(PushEvent)>;

thread_local! {
    static INSTALLED: RefCell<Option<Rc<Double>>> = const { RefCell::new(None) };
}

impl PushDouble {
    /// Install a double on this thread, replacing any other.
    pub fn install() -> Self {
        let double = Rc::new(Double {
            state: RefCell::new(PushState {
                support: PushSupport::Supported,
                permission: Some(NotificationPermission::Default),
                subscription: None,
            }),
            badges: Cell::new(true),
            badge: Cell::new(None),
            calls: RefCell::default(),
            deliver: RefCell::default(),
        });
        INSTALLED.with(|installed| *installed.borrow_mut() = Some(double.clone()));
        Self { double }
    }

    /// The state that `start`, `refresh`, `support()` and `permission()`
    /// report from now on, for example a permission the test has granted
    /// or a subscription made in an earlier launch.
    pub fn set_state(&self, state: PushState) {
        *self.double.state.borrow_mut() = state;
    }

    /// Whether `set_app_badge` reports badges as available.
    pub fn set_badges(&self, available: bool) {
        self.double.badges.set(available);
    }

    /// Every call so far, in order.
    pub fn calls(&self) -> Vec<PushCall> {
        self.double.calls.borrow().clone()
    }

    /// Every call since the previous `take_calls`, in order.
    pub fn take_calls(&self) -> Vec<PushCall> {
        std::mem::take(&mut *self.double.calls.borrow_mut())
    }

    /// The latest count given to `set_app_badge`, if any; 0 is a cleared
    /// badge.
    pub fn badge(&self) -> Option<u64> {
        self.double.badge.get()
    }

    /// Deliver `event` to the application through its sender, as the
    /// browser or the service worker would; the host applies it at the
    /// start of its next pass. Returns false when no `WebPush` is connected,
    /// because none started or the application dropped it.
    pub fn deliver(&self, event: PushEvent) -> bool {
        let deliver = self
            .double
            .deliver
            .borrow()
            .as_ref()
            .and_then(Weak::upgrade);
        match deliver {
            Some(deliver) => {
                deliver(event);
                true
            }
            None => false,
        }
    }
}

impl Drop for PushDouble {
    fn drop(&mut self) {
        INSTALLED.with(|installed| {
            let mut installed = installed.borrow_mut();
            if installed
                .as_ref()
                .is_some_and(|double| Rc::ptr_eq(double, &self.double))
            {
                *installed = None;
            }
        });
    }
}

impl Double {
    /// Connect a starting `WebPush` to the installed double, if any.
    pub(super) fn connect(deliver: &Rc<dyn Fn(PushEvent)>) -> Option<Rc<Self>> {
        let double = INSTALLED.with(|installed| installed.borrow().clone())?;
        *double.deliver.borrow_mut() = Some(Rc::downgrade(deliver));
        double.record(PushCall::Start);
        Some(double)
    }

    pub(super) fn record(&self, call: PushCall) {
        self.calls.borrow_mut().push(call);
    }

    pub(super) fn state(&self) -> PushState {
        self.state.borrow().clone()
    }

    pub(super) fn set_app_badge(&self, count: u64) -> bool {
        self.record(PushCall::SetAppBadge(count));
        self.badge.set(Some(count));
        self.badges.get()
    }
}
