//! Renderer backend selection and reporting.
//!
//! In a browser the host prefers WebGPU and falls back to WebGL2 through
//! wgpu's GL backend; `?renderer=webgpu` or `?renderer=webgl2` forces one.
//! Natively, wgpu chooses as before. Either way the host records which
//! backend and adapter it started on.

use crate::{RendererIdentity, RendererSelection};

/// The renderer the host started on, or `None` before startup and in tests.
pub fn renderer_identity(context: &egui::Context) -> Option<RendererIdentity> {
    context.data(|data| data.get_temp(renderer_id()))
}

pub(crate) fn record_renderer(context: &egui::Context, identity: RendererIdentity) {
    context.data_mut(|data| data.insert_temp(renderer_id(), identity));
}

fn renderer_id() -> egui::Id {
    egui::Id::new("stobie.renderer")
}

/// The renderer a page asked for with its `renderer` query parameter.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RendererRequest {
    /// WebGPU when it starts, otherwise WebGL2.
    Auto,
    WebGpu,
    WebGl2,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl RendererRequest {
    /// Read the first `renderer` parameter of a URL query such as
    /// `?automation=1&renderer=webgl2`.
    pub(crate) fn from_query(query: &str) -> Result<Self, String> {
        let value =
            query
                .trim_start_matches('?')
                .split('&')
                .find_map(|pair| match pair.split_once('=') {
                    Some(("renderer", value)) => Some(value),
                    None if pair == "renderer" => Some(""),
                    _ => None,
                });
        match value {
            None => Ok(Self::Auto),
            Some("webgpu") => Ok(Self::WebGpu),
            Some("webgl2") => Ok(Self::WebGl2),
            Some(other) => Err(format!(
                "unknown renderer override {other:?}; use ?renderer=webgpu or ?renderer=webgl2"
            )),
        }
    }

    fn forced_reason(self) -> Option<String> {
        match self {
            Self::Auto => None,
            Self::WebGpu => Some("forced by ?renderer=webgpu".into()),
            Self::WebGl2 => Some("forced by ?renderer=webgl2".into()),
        }
    }
}

/// Why WebGPU is not the running backend.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WebGpuUnavailable {
    /// The page has no `navigator.gpu`, as in insecure contexts and browsers
    /// without WebGPU.
    Missing,
    /// `navigator.gpu.requestAdapter()` returned no adapter.
    NoAdapter,
    /// An adapter existed, but starting on it failed, for example in
    /// `requestDevice`.
    Failed(String),
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl WebGpuUnavailable {
    pub(crate) fn reason(&self) -> String {
        match self {
            Self::Missing => "WebGPU is unavailable (no navigator.gpu)".into(),
            Self::NoAdapter => "WebGPU returned no adapter".into(),
            Self::Failed(error) => format!("WebGPU failed to start: {error}"),
        }
    }
}

/// How a browser host came to run on `backend`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn browser_selection(
    request: RendererRequest,
    backend: &str,
    webgpu: Option<&WebGpuUnavailable>,
) -> (RendererSelection, Option<String>) {
    if let Some(reason) = request.forced_reason() {
        return (RendererSelection::Forced, Some(reason));
    }
    match (backend, webgpu) {
        ("webgpu", _) | (_, None) => (RendererSelection::Preferred, None),
        (_, Some(unavailable)) => (RendererSelection::Fallback, Some(unavailable.reason())),
    }
}

/// The startup error when no renderer started. `webgpu` explains WebGPU's
/// absence when WebGL2 was attempted after it.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn startup_failure(
    request: RendererRequest,
    webgpu: Option<&WebGpuUnavailable>,
    webgl2: Option<&str>,
) -> String {
    let forced = request.forced_reason().unwrap_or_default();
    let webgpu = match (request, webgpu) {
        (RendererRequest::WebGl2, _) => format!("WebGPU was not tried ({forced})"),
        (_, Some(unavailable)) => unavailable.reason(),
        (_, None) => "WebGPU failed".into(),
    };
    let webgl2 = match (request, webgl2) {
        (RendererRequest::WebGpu, _) => format!("WebGL2 was not tried ({forced})"),
        (_, Some(error)) => format!("WebGL2 failed: {error}"),
        (_, None) => "WebGL2 was not tried".into(),
    };
    format!("No supported renderer: {webgpu}; {webgl2}")
}

/// The first line of a startup error, without a JavaScript stack or the
/// `JsValue(…)` wrapper of eframe's debug formatting.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn error_line(text: &str) -> String {
    let single_line = !text.trim_end().contains('\n');
    let mut line = text.lines().next().unwrap_or_default();
    if let Some(inner) = line.strip_prefix("JsValue(") {
        line = if single_line {
            inner.strip_suffix(')').unwrap_or(inner)
        } else {
            inner
        };
    }
    line.chars().take(400).collect()
}

/// A stable backend name: `webgpu` or `webgl2` in a browser, wgpu's own
/// name (`vulkan`, `metal`, `dx12`, `gl`) natively.
#[cfg(feature = "eframe")]
pub(crate) fn backend_name(backend: eframe::egui_wgpu::wgpu::Backend) -> &'static str {
    use eframe::egui_wgpu::wgpu::Backend;
    match backend {
        Backend::Gl if cfg!(target_arch = "wasm32") => "webgl2",
        other => other.to_str(),
    }
}

/// The identity of a native host's renderer. `WGPU_BACKEND` forces wgpu's
/// choice, so it is reported as the reason.
#[cfg(all(feature = "eframe", not(target_arch = "wasm32")))]
pub(crate) fn native_identity(info: &eframe::egui_wgpu::wgpu::AdapterInfo) -> RendererIdentity {
    let forced = std::env::var("WGPU_BACKEND")
        .ok()
        .filter(|value| !value.is_empty());
    RendererIdentity {
        backend: backend_name(info.backend).into(),
        adapter: info.name.clone(),
        selection: if forced.is_some() {
            RendererSelection::Forced
        } else {
            RendererSelection::Preferred
        },
        reason: forced.map(|value| format!("forced by WGPU_BACKEND={value}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_renderer_query_parameter_selects_or_rejects_an_override() {
        for (query, expected) in [
            ("", Ok(RendererRequest::Auto)),
            ("?automation=1", Ok(RendererRequest::Auto)),
            ("?renderer=webgpu", Ok(RendererRequest::WebGpu)),
            ("?automation=1&renderer=webgl2", Ok(RendererRequest::WebGl2)),
            (
                "renderer=webgl2&renderer=webgpu",
                Ok(RendererRequest::WebGl2),
            ),
            ("?prerenderer=webgl2", Ok(RendererRequest::Auto)),
        ] {
            assert_eq!(RendererRequest::from_query(query), expected, "{query}");
        }
        for query in ["?renderer=webgl", "?renderer", "?renderer=WebGL2"] {
            let error = RendererRequest::from_query(query).unwrap_err();
            assert!(error.contains("?renderer=webgl2"), "{query}: {error}");
        }
    }

    #[test]
    fn selection_names_the_preferred_fallback_and_forced_backends() {
        let auto = RendererRequest::Auto;
        assert_eq!(
            browser_selection(auto, "webgpu", None),
            (RendererSelection::Preferred, None)
        );
        assert_eq!(
            browser_selection(auto, "webgl2", Some(&WebGpuUnavailable::Missing)),
            (
                RendererSelection::Fallback,
                Some("WebGPU is unavailable (no navigator.gpu)".into())
            )
        );
        assert_eq!(
            browser_selection(auto, "webgl2", Some(&WebGpuUnavailable::NoAdapter)).1,
            Some("WebGPU returned no adapter".into())
        );
        assert_eq!(
            browser_selection(
                auto,
                "webgl2",
                Some(&WebGpuUnavailable::Failed("device refused".into()))
            )
            .1,
            Some("WebGPU failed to start: device refused".into())
        );
        assert_eq!(
            browser_selection(RendererRequest::WebGl2, "webgl2", None),
            (
                RendererSelection::Forced,
                Some("forced by ?renderer=webgl2".into())
            )
        );
        assert_eq!(
            browser_selection(RendererRequest::WebGpu, "webgpu", None).0,
            RendererSelection::Forced
        );
    }

    #[test]
    fn a_startup_failure_names_each_backend_and_its_cause() {
        assert_eq!(
            startup_failure(
                RendererRequest::Auto,
                Some(&WebGpuUnavailable::Missing),
                Some("canvas.getContext() returned null")
            ),
            "No supported renderer: WebGPU is unavailable (no navigator.gpu); WebGL2 failed: canvas.getContext() returned null"
        );
        assert_eq!(
            startup_failure(
                RendererRequest::Auto,
                Some(&WebGpuUnavailable::Failed("device refused".into())),
                Some("no context")
            ),
            "No supported renderer: WebGPU failed to start: device refused; WebGL2 failed: no context"
        );
        assert_eq!(
            startup_failure(
                RendererRequest::WebGpu,
                Some(&WebGpuUnavailable::Missing),
                None
            ),
            "No supported renderer: WebGPU is unavailable (no navigator.gpu); WebGL2 was not tried (forced by ?renderer=webgpu)"
        );
        assert_eq!(
            startup_failure(RendererRequest::WebGl2, None, Some("no context")),
            "No supported renderer: WebGPU was not tried (forced by ?renderer=webgl2); WebGL2 failed: no context"
        );
    }

    #[test]
    fn startup_errors_keep_only_their_message() {
        assert_eq!(
            error_line(
                "JsValue(Error: device refused\nError: device refused\n    at requestDevice)"
            ),
            "Error: device refused"
        );
        assert_eq!(
            error_line("JsValue(TypeError: no adapter)"),
            "TypeError: no adapter"
        );
        assert_eq!(
            error_line("failed to create wgpu surface: (none)"),
            "failed to create wgpu surface: (none)"
        );
        assert_eq!(error_line(&"x".repeat(500)).len(), 400);
    }

    #[test]
    fn the_recorded_identity_is_read_back_from_the_context() {
        let context = egui::Context::default();
        assert_eq!(renderer_identity(&context), None);
        let identity = RendererIdentity {
            backend: "webgl2".into(),
            adapter: "ANGLE".into(),
            selection: RendererSelection::Fallback,
            reason: Some("WebGPU returned no adapter".into()),
        };
        record_renderer(&context, identity.clone());
        assert_eq!(renderer_identity(&context), Some(identity));
    }

    #[cfg(feature = "eframe")]
    #[test]
    fn backend_names_are_stable() {
        use eframe::egui_wgpu::wgpu::Backend;
        assert_eq!(backend_name(Backend::BrowserWebGpu), "webgpu");
        assert_eq!(backend_name(Backend::Vulkan), "vulkan");
        assert_eq!(backend_name(Backend::Gl), "gl");
    }
}
