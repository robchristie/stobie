//! The browser transport: a page opened with `?automation=1`, reached
//! through Chromium's DevTools protocol with `lantern-core`.
//!
//! Each request evaluates only the fixed dispatcher
//! `window.__STOBIE_AUTOMATION.request(json)` in the page, with
//! `Runtime.evaluate`. Nothing else in the page is evaluated. A screenshot
//! reads the canvas's box through the DOM domain, without running script.

use std::time::Instant;

use base64::Engine as _;
use lantern_core::cdp::{CdpClient, CdpError, CdpWebSocket, TargetInfo};
use lantern_core::endpoint::resolve_endpoint;
use serde_json::json;
use stobie_protocol::INSPECTION_REQUEST_BYTES;

use crate::{ClientError, Transport};

/// Which page to address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageSelector {
    /// The page whose URL is exactly this, or else the only page whose URL
    /// starts with it.
    Url(String),
    /// A DevTools target ID.
    Target(String),
    /// The only page.
    Only,
}

/// A Stobie application page in Chromium.
pub struct BrowserTransport {
    endpoint: String,
    page: PageSelector,
    websocket: Option<String>,
}

/// Classify a DevTools failure of an operation bounded by `deadline`.
///
/// lantern bounds every connect, read and write by the deadline but reports
/// a deadline that passes while it lists pages, connects to the page or
/// before it sends a command as a transport failure, as it does a refused
/// connection. Such a failure once the deadline has passed is a timeout, as
/// the socket transport and the Node client report it; otherwise the
/// browser could not be reached.
fn cdp_error(context: &str, error: CdpError, deadline: Instant) -> ClientError {
    match error {
        CdpError::Unreachable { source } | CdpError::WebSocketTransport { source, .. }
            if Instant::now() >= deadline =>
        {
            ClientError::timeout(format!(
                "{context}: the deadline passed before the browser answered: {source}"
            ))
        }
        CdpError::Unreachable { source } | CdpError::WebSocketTransport { source, .. } => {
            ClientError::connection(
                format!("{context}: {source}"),
                "check the browser is running and the endpoint is its DevTools HTTP endpoint, such as http://127.0.0.1:9222",
            )
        }
        CdpError::CommandUncertain { source } => {
            ClientError::timeout(format!("{context}: the browser did not answer: {source}"))
        }
        other => ClientError::connection(
            format!("{context}: {other:?}"),
            "check the endpoint and the page",
        ),
    }
}

impl BrowserTransport {
    /// Address `page` in the browser whose DevTools HTTP endpoint is
    /// `endpoint`, such as `http://127.0.0.1:9222` (as `lantern browser
    /// endpoint` prints it).
    pub fn new(endpoint: &str, page: PageSelector) -> Result<Self, ClientError> {
        let resolved = resolve_endpoint(Some(endpoint), None).map_err(|error| {
            ClientError::usage(format!(
                "{endpoint:?} is not a local DevTools endpoint ({error:?}); use http://127.0.0.1:PORT"
            ))
        })?;
        Ok(Self {
            endpoint: resolved.display,
            page,
            websocket: None,
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// The selected page, resolved once. When none matches, the error is
    /// one [`no_page_matches`] recognises; `attach` waits for a page that
    /// the browser has opened but whose URL its target list does not show
    /// yet, and every other command fails at once.
    pub fn target(&mut self, deadline: Instant) -> Result<TargetInfo, ClientError> {
        let endpoint = resolve_endpoint(Some(&self.endpoint), None)
            .expect("validated when the transport was made");
        let targets = CdpClient::new(endpoint)
            .with_deadline(deadline)
            .targets()
            .map_err(|error| cdp_error("cannot list the browser's pages", error, deadline))?;
        let pages: Vec<&TargetInfo> = targets
            .iter()
            .filter(|target| target.kind == "page")
            .collect();
        match select(&pages, &self.page).as_slice() {
            [page] => {
                self.websocket = page.web_socket_debugger_url.clone();
                Ok((*page).clone())
            }
            [] => Err(ClientError::connection(
                format!(
                    "{NO_PAGE_MATCHES}; the browser's pages are: {}",
                    describe_pages(&pages)
                ),
                "open the application with ?automation=1, then pass --page URL or --target ID",
            )),
            selected => Err(ClientError::usage(format!(
                "several pages match: {}",
                describe_pages(selected)
            ))),
        }
    }

    fn socket(&mut self, deadline: Instant) -> Result<CdpWebSocket, ClientError> {
        if self.websocket.is_none() {
            self.target(deadline)?;
        }
        let url = self.websocket.clone().ok_or_else(|| {
            ClientError::connection(
                "the page has no DevTools WebSocket; another client may be attached",
                "close other DevTools clients of this page",
            )
        })?;
        CdpWebSocket::connect_until(&url, deadline)
            .map_err(|error| cdp_error("cannot connect to the page", error, deadline))
    }

    /// A PNG of the page's canvas.
    pub fn screenshot(&mut self, deadline: Instant) -> Result<Vec<u8>, ClientError> {
        let mut socket = self.socket(deadline)?;
        let measure = |error| cdp_error("cannot measure the canvas", error, deadline);
        let document = socket
            .call("DOM.getDocument", Some(json!({ "depth": 0 })))
            .map_err(measure)?;
        let canvas = socket
            .call(
                "DOM.querySelector",
                Some(json!({ "nodeId": document["root"]["nodeId"], "selector": "canvas" })),
            )
            .map_err(measure)?;
        let no_canvas = || {
            ClientError::new(
                "capture_failed",
                "the page has no canvas",
                "attach to a Stobie application page",
            )
        };
        let node = canvas["nodeId"]
            .as_u64()
            .filter(|node| *node != 0)
            .ok_or_else(no_canvas)?;
        let model = socket
            .call("DOM.getBoxModel", Some(json!({ "nodeId": node })))
            .map_err(measure)?;
        // The border quad's corners, clockwise from the top left.
        let quad: Vec<f64> = model["model"]["border"]
            .as_array()
            .map(|quad| quad.iter().filter_map(serde_json::Value::as_f64).collect())
            .unwrap_or_default();
        let [left, top, right, _, _, bottom, ..] = quad[..] else {
            return Err(no_canvas());
        };
        let shot = socket
            .call(
                "Page.captureScreenshot",
                Some(json!({
                    "format": "png",
                    "clip": { "x": left, "y": top, "width": right - left, "height": bottom - top, "scale": 1 },
                })),
            )
            .map_err(|error| cdp_error("the screenshot failed", error, deadline))?;
        let data = shot["data"].as_str().ok_or_else(|| {
            ClientError::new(
                "capture_failed",
                "the browser returned no image",
                "retry the capture",
            )
        })?;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|error| {
                ClientError::new("capture_failed", error.to_string(), "retry the capture")
            })
    }
}

/// The error for a page without a dispatcher: not opened with
/// `?automation=1`, or still starting or failed to start, as the page and the
/// startup report of the shared loader (`browser-startup.js`) say.
fn missing_dispatcher(socket: &mut CdpWebSocket) -> ClientError {
    let state = socket
        .call(
            "Runtime.evaluate",
            Some(json!({
                "expression": "(() => { const s = window.__STOBIE_STARTUP; return { \
                    automation: new URL(location.href).searchParams.get('automation') === '1', \
                    loaded: document.readyState === 'complete', \
                    status: s?.status ?? null, \
                    failure: s?.failure ? `${s.failure.phase}: ${s.failure.message}` : null }; })()",
                "returnByValue": true,
            })),
        )
        .ok()
        .map(|reply| reply["result"]["value"].clone())
        .unwrap_or_default();
    classify_missing(&state)
}

/// Classify a page without a dispatcher from its evaluated state.
fn classify_missing(state: &serde_json::Value) -> ClientError {
    if let Some(failure) = state["failure"].as_str() {
        return ClientError::connection(
            format!("the application on the page failed to start ({failure})"),
            "fix the failure the page shows; without GPU acceleration, Chromium may need \
             --use-angle=swiftshader --enable-unsafe-swiftshader for WebGL2",
        );
    }
    // A page opened for automation is still starting while the shared loader
    // says so, or, before the loader has run, while the document loads. A
    // loaded page with no report or a ready one keeps the immediate error.
    let starting =
        state["status"] == "starting" || (state["status"].is_null() && state["loaded"] == false);
    if state["automation"] == true && starting {
        return ClientError::connection(
            NOT_STARTED,
            "wait for the page to finish loading and retry; `stobie attach` waits for it",
        );
    }
    ClientError::connection(
        "the page has no Stobie automation dispatcher",
        "open the application with ?automation=1 in its URL; if it has that, the \
         application has not started yet or failed to start (see the browser console)",
    )
}

/// A page opened with `?automation=1` whose application has not yet
/// installed its dispatcher.
pub const NOT_STARTED: &str = "the application on the page has not started yet";

impl Transport for BrowserTransport {
    fn request(&mut self, body: &str, deadline: Instant) -> Result<String, ClientError> {
        if body.len() > INSPECTION_REQUEST_BYTES {
            return Err(ClientError::usage("the request exceeds 256 KiB"));
        }
        let mut socket = self.socket(deadline)?;
        // A JSON string is a JavaScript string literal.
        let argument = serde_json::to_string(body).expect("strings serialise");
        let reply = socket
            .call(
                "Runtime.evaluate",
                Some(json!({
                    "expression": format!("window.__STOBIE_AUTOMATION.request({argument})"),
                    "returnByValue": true,
                    "awaitPromise": true,
                })),
            )
            .map_err(|error| cdp_error("the request could not reach the page", error, deadline))?;
        if let Some(exception) = reply.get("exceptionDetails") {
            let description = exception["exception"]["description"]
                .as_str()
                .or_else(|| exception["text"].as_str())
                .unwrap_or_default();
            // A missing dispatcher fails reading `request` from undefined;
            // any other exception came from the dispatcher itself.
            let missing = description.contains("__STOBIE_AUTOMATION")
                || description.contains("(reading 'request')");
            if !missing {
                return Err(ClientError::new(
                    "protocol_violation",
                    format!(
                        "the page's dispatcher threw: {}",
                        description.lines().next().unwrap_or_default()
                    ),
                    "reload the page; if it persists, report this application",
                ));
            }
            return Err(missing_dispatcher(&mut socket));
        }
        reply["result"]["value"]
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| {
                ClientError::new(
                    "protocol_violation",
                    "the page's dispatcher returned no text",
                    "check that this is a Stobie application page",
                )
            })
    }

    fn describe(&self) -> String {
        let page = match &self.page {
            PageSelector::Url(url) => url.clone(),
            PageSelector::Target(target) => format!("target {target}"),
            PageSelector::Only => "(the only page)".into(),
        };
        format!("browser {} page {page}", self.endpoint)
    }
}

const NO_PAGE_MATCHES: &str = "no page matches";

/// Whether `error` says that no browser page matched the selector.
pub fn no_page_matches(error: &ClientError) -> bool {
    error.code == "connection_failed" && error.message.starts_with(NO_PAGE_MATCHES)
}

fn describe_pages(pages: &[&TargetInfo]) -> String {
    pages
        .iter()
        .map(|page| format!("{} {}", page.id, page.url.as_deref().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The pages `selector` names: by target ID, by exact URL or else URL prefix,
/// or every page.
fn select<'a>(pages: &[&'a TargetInfo], selector: &PageSelector) -> Vec<&'a TargetInfo> {
    match selector {
        PageSelector::Target(id) => pages
            .iter()
            .copied()
            .filter(|page| &page.id == id)
            .collect(),
        PageSelector::Url(url) => {
            let exact: Vec<_> = pages
                .iter()
                .copied()
                .filter(|page| page.url.as_deref() == Some(url.as_str()))
                .collect();
            if exact.is_empty() {
                pages
                    .iter()
                    .copied()
                    .filter(|page| {
                        page.url
                            .as_deref()
                            .is_some_and(|page| page.starts_with(url.as_str()))
                    })
                    .collect()
            } else {
                exact
            }
        }
        PageSelector::Only => pages.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(id: &str, url: &str) -> TargetInfo {
        TargetInfo {
            id: id.into(),
            kind: "page".into(),
            title: None,
            url: Some(url.into()),
            attached: None,
            browser_context_id: None,
            web_socket_debugger_url: None,
        }
    }

    #[test]
    fn pages_are_selected_by_target_exact_url_or_prefix() {
        let (a, b, blank) = (
            page("A", "http://127.0.0.1:8080/?automation=1"),
            page("B", "http://127.0.0.1:8080/?automation=1&renderer=webgl2"),
            page("C", ""),
        );
        let pages = [&a, &b, &blank];
        let ids = |selector: PageSelector| -> Vec<String> {
            select(&pages, &selector)
                .iter()
                .map(|page| page.id.clone())
                .collect()
        };
        assert_eq!(
            ids(PageSelector::Url(a.url.clone().unwrap())),
            ["A"],
            "exact wins"
        );
        assert_eq!(
            ids(PageSelector::Url(
                "http://127.0.0.1:8080/?automation=1&r".into()
            )),
            ["B"]
        );
        assert_eq!(ids(PageSelector::Target("C".into())), ["C"]);
        assert_eq!(ids(PageSelector::Only).len(), 3);
        // A page still navigating shows an empty URL, which matches nothing
        // until its navigation appears.
        assert!(ids(PageSelector::Url("http://127.0.0.1:9000/".into())).is_empty());
    }

    /// A DevTools endpoint whose one page shows an empty URL for its first
    /// `blank` listings, as Chromium's can just after a navigation commits.
    fn devtools(blank: usize) -> String {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for (listing, stream) in listener.incoming().enumerate() {
                let mut stream = stream.unwrap();
                let mut request = [0; 1024];
                let _ = stream.read(&mut request);
                let url = if listing < blank {
                    ""
                } else {
                    "http://127.0.0.1:8080/?automation=1"
                };
                let body = format!(r#"[{{"id":"P","type":"page","url":"{url}"}}]"#);
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        format!("http://{address}")
    }

    #[test]
    fn a_page_that_does_not_match_yet_fails_at_once_with_a_recognisable_error() {
        let endpoint = devtools(1);
        let mut transport = BrowserTransport::new(
            &endpoint,
            PageSelector::Url("http://127.0.0.1:8080/?automation=1".into()),
        )
        .unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        let started = Instant::now();
        let error = transport.target(deadline).unwrap_err();
        assert!(no_page_matches(&error), "{error:?}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "no waiting"
        );
        // The next listing shows the page's URL, as attach's retry sees.
        assert_eq!(transport.target(deadline).unwrap().id, "P");
        let wrong = ClientError::usage("several pages match: P");
        assert!(!no_page_matches(&wrong));
    }

    /// A transport whose page WebSocket is at `address`, without listing
    /// pages first.
    fn page_at(address: std::net::SocketAddr) -> BrowserTransport {
        let mut transport = BrowserTransport::new(
            &format!("http://{address}"),
            PageSelector::Target("P".into()),
        )
        .unwrap();
        transport.websocket = Some(format!("ws://{address}/devtools/page/P"));
        transport
    }

    #[test]
    fn a_deadline_that_passes_before_the_request_reaches_the_page_is_a_timeout() {
        use std::time::Duration;
        // A browser that accepts the connection but never completes the
        // WebSocket handshake: the deadline passes while connecting, as it
        // can when a wait's last poll starts just before its deadline. The
        // Node client and the socket transport report that as a timeout.
        let stalled = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut transport = page_at(stalled.local_addr().unwrap());
        let error = transport
            .request("{}", Instant::now() + Duration::from_millis(200))
            .unwrap_err();
        assert_eq!(error.code, "timeout", "{error:?}");
        assert_eq!(error.class(), crate::ErrorClass::Deadline);
        // A deadline that has already passed is a timeout without connecting.
        let error = transport.request("{}", Instant::now()).unwrap_err();
        assert_eq!(error.code, "timeout", "{error:?}");
        drop(stalled);
        // A browser that is gone, with time to spare, is a connection failure.
        let gone = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = gone.local_addr().unwrap();
        drop(gone);
        let error = page_at(address)
            .request("{}", Instant::now() + Duration::from_secs(5))
            .unwrap_err();
        assert_eq!(error.code, "connection_failed", "{error:?}");
    }

    fn classify(state: serde_json::Value) -> String {
        classify_missing(&state).message
    }

    #[test]
    fn a_page_without_a_dispatcher_is_starting_failed_or_not_automated() {
        let no_dispatcher = "the page has no Stobie automation dispatcher";
        // Before the loader has run: no report while the document loads.
        assert_eq!(
            classify(json!({"automation": true, "loaded": false, "status": null})),
            NOT_STARTED
        );
        // The loader reports that it is starting, loaded or not.
        assert_eq!(
            classify(json!({"automation": true, "loaded": true, "status": "starting"})),
            NOT_STARTED
        );
        // A failed start says why, whatever else holds.
        assert!(
            classify(
                json!({"automation": true, "loaded": true, "status": "failed",
                "failure": "framework_start: No supported renderer"})
            )
            .contains("failed to start (framework_start: No supported renderer)")
        );
        // Loaded without a report (a bootstrap without the loader), ready
        // without a dispatcher, or not opened with ?automation=1.
        for state in [
            json!({"automation": true, "loaded": true, "status": null}),
            json!({"automation": true, "loaded": true, "status": "ready"}),
            json!({"automation": false, "loaded": false, "status": "starting"}),
            json!(null),
        ] {
            assert_eq!(classify(state.clone()), no_dispatcher, "{state}");
        }
    }
}
