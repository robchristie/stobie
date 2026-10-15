//! The browser transport: a page opened with `?automation=1`, reached
//! through Chromium's DevTools protocol with `lantern-core`.
//!
//! Each request evaluates only the fixed dispatcher
//! `window.__STOBIE_AUTOMATION.request(json)` in the page, with
//! `Runtime.evaluate`. Nothing else in the page is evaluated or read, apart
//! from the canvas's rectangle for a screenshot.

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
        let rectangle = socket
            .call(
                "Runtime.evaluate",
                Some(json!({
                    "expression": "(() => { const canvas = document.querySelector('canvas'); if (!canvas) return null; const r = canvas.getBoundingClientRect(); return { x: r.left, y: r.top, width: r.width, height: r.height }; })()",
                    "returnByValue": true,
                })),
            )
            .map_err(|error| cdp_error("cannot measure the canvas", error))?;
        let clip = &rectangle["result"]["value"];
        if clip.is_null() {
            return Err(ClientError::new(
                "capture_failed",
                "the page has no canvas",
                "attach to a Stobie application page",
            ));
        }
        let shot = socket
            .call(
                "Page.captureScreenshot",
                Some(json!({
                    "format": "png",
                    "clip": { "x": clip["x"], "y": clip["y"], "width": clip["width"], "height": clip["height"], "scale": 1 },
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
        if reply.get("exceptionDetails").is_some() {
            return Err(ClientError::connection(
                "the page has no Stobie automation dispatcher",
                "open the application with ?automation=1 in its URL",
            ));
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
        format!("browser {} page {:?}", self.endpoint, self.page)
    }
}
