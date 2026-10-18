//! A browser WebSocket client for the feed. It reconnects with a growing
//! delay and catches up from the last sequence number it received.
//!
//! Each connection opens `<feed URL>?after=<seq>`, where `seq` is the highest
//! sequence number received so far (0 at first), and the server sends every
//! event after it. The client forwards only events newer than that number, so
//! a server that replays an overlap wakes the application for nothing. On
//! close it waits 250 ms, doubling up to 8 s, then reconnects; a connection
//! that opens resets the delay. It reports each change of connection as an
//! intent, and stops when the application host stops.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::{Rc, Weak};

use stobie_ui_egui::IntentSender;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{CloseEvent, MessageEvent, Url, WebSocket};

use crate::{Connection, FeedEvent, FeedIntent};

const FIRST_RETRY_MS: u32 = 250;
const LAST_RETRY_MS: u32 = 8_000;

/// The feed URL for this page: `?feed=<url>` from the page's address, or
/// `feed` beside the page, over `ws:` or `wss:` to match the page.
pub fn page_feed_url() -> String {
    let location = web_sys::window().expect("a window").location();
    let href = location.href().unwrap_or_default();
    let page = Url::new(&href).ok();
    if let Some(feed) = page
        .as_ref()
        .and_then(|page| page.search_params().get("feed"))
    {
        return feed;
    }
    match Url::new_with_base("feed", &href) {
        Ok(url) => {
            url.set_protocol(if url.protocol() == "https:" {
                "wss:"
            } else {
                "ws:"
            });
            url.set_search("");
            url.href()
        }
        Err(_) => "ws://localhost/feed".into(),
    }
}

/// An open feed connection, reconnecting until dropped.
pub struct FeedSocket {
    inner: Rc<Inner>,
}

struct Inner {
    url: String,
    intents: IntentSender<FeedIntent>,
    /// The highest sequence number forwarded.
    after: Cell<u64>,
    /// Failed attempts since the last open connection.
    attempt: Cell<u32>,
    stopped: Cell<bool>,
    socket: RefCell<Option<WebSocket>>,
    timer: Cell<Option<i32>>,
    /// One set of callbacks, created once and shared by every connection:
    /// dropping a callback while it runs would fail, and a callback may
    /// reconnect.
    callbacks: OnceCell<Callbacks>,
}

struct Callbacks {
    open: Closure<dyn FnMut()>,
    message: Closure<dyn FnMut(MessageEvent)>,
    close: Closure<dyn FnMut(CloseEvent)>,
    retry: Closure<dyn FnMut()>,
}

impl FeedSocket {
    /// Connect to the feed at `url` and send its events to `intents`.
    pub fn open(url: &str, intents: IntentSender<FeedIntent>) -> Self {
        let inner = Rc::new(Inner {
            url: url.to_owned(),
            intents,
            after: Cell::new(0),
            attempt: Cell::new(0),
            stopped: Cell::new(false),
            socket: RefCell::new(None),
            timer: Cell::new(None),
            callbacks: OnceCell::new(),
        });
        let _ = inner.callbacks.set(Callbacks::new(Rc::downgrade(&inner)));
        inner.connect();
        Self { inner }
    }

    /// The sequence number the next connection catches up from.
    pub fn after(&self) -> u64 {
        self.inner.after.get()
    }
}

impl Drop for FeedSocket {
    fn drop(&mut self) {
        self.inner.stop();
    }
}

impl Callbacks {
    fn new(inner: Weak<Inner>) -> Self {
        let on = |inner: &Weak<Inner>, run: fn(&Inner)| {
            let inner = inner.clone();
            move || {
                if let Some(inner) = inner.upgrade() {
                    run(&inner);
                }
            }
        };
        let message = {
            let inner = inner.clone();
            move |event: MessageEvent| {
                if let (Some(inner), Some(text)) = (inner.upgrade(), event.data().as_string()) {
                    inner.receive(&text);
                }
            }
        };
        let close = {
            let inner = inner.clone();
            move |_: CloseEvent| {
                if let Some(inner) = inner.upgrade() {
                    inner.socket.borrow_mut().take();
                    inner.retry_later();
                }
            }
        };
        Self {
            open: Closure::new(on(&inner, Inner::opened)),
            message: Closure::new(message),
            close: Closure::new(close),
            retry: Closure::new(on(&inner, |inner| {
                inner.timer.set(None);
                inner.connect();
            })),
        }
    }
}

impl Inner {
    fn callbacks(&self) -> &Callbacks {
        self.callbacks.get().expect("callbacks are set at open")
    }

    /// Send one intent; a stopped host stops the transport.
    fn send(&self, intent: FeedIntent) {
        if self.intents.send(intent).is_err() {
            self.stop();
        }
    }

    fn connect(&self) {
        if self.stopped.get() {
            return;
        }
        self.send(FeedIntent::Connection(Connection::Connecting));
        if self.stopped.get() {
            return;
        }
        let socket = Url::new(&self.url).and_then(|url| {
            url.search_params()
                .set("after", &self.after.get().to_string());
            WebSocket::new(&url.href())
        });
        let Ok(socket) = socket else {
            // An invalid URL never connects; say so instead of retrying.
            self.send(FeedIntent::Connection(Connection::Closed));
            return;
        };
        let callbacks = self.callbacks();
        socket.set_onopen(Some(callbacks.open.as_ref().unchecked_ref()));
        socket.set_onmessage(Some(callbacks.message.as_ref().unchecked_ref()));
        // A failed connection also closes, so close covers errors.
        socket.set_onclose(Some(callbacks.close.as_ref().unchecked_ref()));
        *self.socket.borrow_mut() = Some(socket);
    }

    fn opened(&self) {
        self.attempt.set(0);
        self.send(FeedIntent::Connection(Connection::Open));
    }

    fn receive(&self, text: &str) {
        // Heartbeats, unknown messages and replayed events never reach the
        // application, so they cost it nothing.
        let Some(event) = FeedEvent::parse(text) else {
            return;
        };
        if event.seq <= self.after.get() {
            return;
        }
        self.after.set(event.seq);
        self.send(FeedIntent::Event(event));
    }

    fn retry_later(&self) {
        if self.stopped.get() {
            return;
        }
        let attempt = self.attempt.get() + 1;
        self.attempt.set(attempt);
        let delay_ms = FIRST_RETRY_MS
            .saturating_mul(1 << (attempt - 1).min(5))
            .min(LAST_RETRY_MS);
        self.send(FeedIntent::Connection(Connection::Waiting {
            attempt,
            delay_ms,
        }));
        if self.stopped.get() {
            return;
        }
        let timer = web_sys::window().and_then(|window| {
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    self.callbacks().retry.as_ref().unchecked_ref(),
                    delay_ms as i32,
                )
                .ok()
        });
        self.timer.set(timer);
    }

    fn stop(&self) {
        if self.stopped.replace(true) {
            return;
        }
        if let (Some(timer), Some(window)) = (self.timer.take(), web_sys::window()) {
            window.clear_timeout_with_handle(timer);
        }
        if let Some(socket) = self.socket.borrow_mut().take() {
            socket.set_onopen(None);
            socket.set_onmessage(None);
            socket.set_onclose(None);
            let _ = socket.close();
        }
    }
}
