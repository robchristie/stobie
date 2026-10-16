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

fn cdp_error(context: &str, error: CdpError) -> ClientError {
    match error {
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

    /// The selected page, resolved once.
    pub fn target(&mut self, deadline: Instant) -> Result<TargetInfo, ClientError> {
        let endpoint = resolve_endpoint(Some(&self.endpoint), None)
            .expect("validated when the transport was made");
        let targets = CdpClient::new(endpoint)
            .with_deadline(deadline)
            .targets()
            .map_err(|error| cdp_error("cannot list the browser's pages", error))?;
        let pages: Vec<&TargetInfo> = targets
            .iter()
            .filter(|target| target.kind == "page")
            .collect();
        let describe = |pages: &[&TargetInfo]| {
            pages
                .iter()
                .map(|page| format!("{} {}", page.id, page.url.as_deref().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let selected: Vec<&TargetInfo> = match &self.page {
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
            PageSelector::Only => pages.clone(),
        };
        match selected.as_slice() {
            [page] => {
                self.websocket = page.web_socket_debugger_url.clone();
                Ok((*page).clone())
            }
            [] => Err(ClientError::connection(
                format!(
                    "no page matches; the browser's pages are: {}",
                    describe(&pages)
                ),
                "open the application with ?automation=1, then pass --page URL or --target ID",
            )),
            _ => Err(ClientError::usage(format!(
                "several pages match: {}",
                describe(&selected)
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
            .map_err(|error| cdp_error("cannot connect to the page", error))
    }

    /// A PNG of the page's canvas.
    pub fn screenshot(&mut self, deadline: Instant) -> Result<Vec<u8>, ClientError> {
        let mut socket = self.socket(deadline)?;
        let measure = |error| cdp_error("cannot measure the canvas", error);
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
            .map_err(|error| cdp_error("the screenshot failed", error))?;
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
/// `?automation=1`, or still starting or failed to start, as the startup
/// report of the shared loader (`browser-startup.js`) says.
fn missing_dispatcher(socket: &mut CdpWebSocket) -> ClientError {
    let state = socket
        .call(
            "Runtime.evaluate",
            Some(json!({
                "expression": "(() => { const s = window.__STOBIE_STARTUP; return { \
                    automation: new URL(location.href).searchParams.get('automation') === '1', \
                    status: s?.status ?? null, \
                    failure: s?.failure ? `${s.failure.phase}: ${s.failure.message}` : null }; })()",
                "returnByValue": true,
            })),
        )
        .ok()
        .map(|reply| reply["result"]["value"].clone())
        .unwrap_or_default();
    if let Some(failure) = state["failure"].as_str() {
        return ClientError::connection(
            format!("the application on the page failed to start ({failure})"),
            "fix the failure the page shows; without GPU acceleration, Chromium may need \
             --use-angle=swiftshader --enable-unsafe-swiftshader for WebGL2",
        );
    }
    // Only the shared startup loader reports that it is still starting; a
    // page without that report keeps the immediate error.
    if state["automation"] == true && state["status"] == "starting" {
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
            .map_err(|error| cdp_error("the request could not reach the page", error))?;
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
