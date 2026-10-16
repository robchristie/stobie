//! Native and browser entry points for an [`AppHost`].

use super::{AppHost, Application};

impl<A: Application> eframe::App for AppHost<A> {
    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.present(ui);
        #[cfg(target_arch = "wasm32")]
        if !self.startup_reported {
            // The first pass's UI is complete on the CPU; the page's startup
            // loader, when present, records it once and removes its overlay.
            self.startup_reported = true;
            web::startup_mark("workspace_ui_complete");
        }
        if self.app.take_save_request()
            && let Some(storage) = frame.storage_mut()
        {
            self.app.save(storage);
            storage.flush();
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.app.save(storage);
    }
}

/// Run a hosted application in a native window titled `app_name`. Automation
/// starts when `STOBIE_AUTOMATION_SOCKET` names an absolute socket path.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native<A: Application>(
    app_name: &str,
    options: eframe::NativeOptions,
    create: impl FnOnce(&eframe::CreationContext<'_>) -> A + 'static,
) -> eframe::Result {
    eframe::run_native(
        app_name,
        options,
        Box::new(move |creation| {
            if let Some(render_state) = &creation.wgpu_render_state {
                super::renderer::record_renderer(
                    &creation.egui_ctx,
                    super::renderer::native_identity(&render_state.adapter.get_info()),
                );
            }
            Ok(Box::new(AppHost::new(&creation.egui_ctx, create(creation))))
        }),
    )
}

#[cfg(target_arch = "wasm32")]
pub use web::WebHost;

#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub use web::startup_mark as __startup_mark;

#[cfg(target_arch = "wasm32")]
#[path = "aria_dom.rs"]
mod aria_dom;

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::{Cell, RefCell};
    use std::marker::PhantomData;
    use std::rc::Rc;

    use eframe::egui_wgpu::wgpu::{self, Backends};
    use wasm_bindgen::{JsCast, JsValue};

    use super::super::renderer::{self, RendererRequest, WebGpuUnavailable};
    use crate::{AppHost, Application, Inspection, RendererIdentity};

    /// A browser runner for a hosted application. [`web_handle!`](crate::web_handle)
    /// exports it to JavaScript; applications rarely use it directly.
    pub struct WebHost<A: Application> {
        runner: eframe::WebRunner,
        /// The key of the ARIA layer this host installed, if any.
        layer: Rc<Cell<Option<u32>>>,
        application: PhantomData<fn() -> A>,
    }

    impl<A: Application> Default for WebHost<A> {
        fn default() -> Self {
            console_error_panic_hook::set_once();
            Self {
                runner: eframe::WebRunner::new(),
                layer: Rc::default(),
                application: PhantomData,
            }
        }
    }

    impl<A: Application> WebHost<A> {
        /// Start the application in `canvas`; the promise settles once it runs.
        /// Unless [`Application::web_accessibility_layer`] opts out, the host
        /// mirrors the accessibility tree into a hidden ARIA layer beside it.
        ///
        /// The host prefers WebGPU and falls back to WebGL2 when WebGPU is
        /// missing, has no adapter or fails to start; `?renderer=webgpu` or
        /// `?renderer=webgl2` in the page URL forces one. A WebGL2 retry after
        /// a WebGPU failure replaces `canvas` with a fresh copy, because a
        /// canvas keeps its first context. The canvas carries the outcome in
        /// `data-stobie-renderer`, `-adapter`, `-selection` and `-reason`.
        pub fn start(
            &self,
            canvas: web_sys::HtmlCanvasElement,
            create: impl FnOnce(&eframe::CreationContext<'_>) -> A + 'static,
        ) -> js_sys::Promise {
            let runner = self.runner.clone();
            let layer = self.layer.clone();
            wasm_bindgen_futures::future_to_promise(async move {
                let request = RendererRequest::from_query(&page_query())
                    .map_err(|error| JsValue::from_str(&error))?;
                let webgpu_present = navigator_has_gpu();
                // eframe consumes a creator only once its painter starts, so
                // every attempt takes the one application creator from here.
                let create = Rc::new(RefCell::new(Some(create)));
                let attempt = |canvas: web_sys::HtmlCanvasElement,
                               options: eframe::WebOptions,
                               webgpu: Option<WebGpuUnavailable>| {
                    let create = create.clone();
                    let layer = layer.clone();
                    let layer_canvas = canvas.clone();
                    runner.start(
                        canvas,
                        options,
                        Box::new(move |creation| {
                            if let Some(render_state) = &creation.wgpu_render_state {
                                let info = render_state.adapter.get_info();
                                let backend = renderer::backend_name(info.backend);
                                let webgpu = match webgpu {
                                    // The default attempt ran on WebGL2 because
                                    // eframe's detection found no adapter.
                                    None if backend != "webgpu" => Some(if webgpu_present {
                                        WebGpuUnavailable::NoAdapter
                                    } else {
                                        WebGpuUnavailable::Missing
                                    }),
                                    other => other,
                                };
                                let (selection, reason) =
                                    renderer::browser_selection(request, backend, webgpu.as_ref());
                                let identity = RendererIdentity {
                                    backend: backend.into(),
                                    adapter: info.name,
                                    selection,
                                    reason,
                                };
                                mark_canvas(&layer_canvas, &identity);
                                renderer::record_renderer(&creation.egui_ctx, identity);
                            }
                            let create = create
                                .borrow_mut()
                                .take()
                                .expect("the application is created once");
                            let app = create(creation);
                            // A restarted host replaces its previous layer.
                            if let Some(previous) = layer.take() {
                                super::aria_dom::remove(previous);
                            }
                            if app.web_accessibility_layer() {
                                layer.set(super::aria_dom::install(
                                    &creation.egui_ctx,
                                    &layer_canvas,
                                    &app.identity().name,
                                ));
                            }
                            Ok(Box::new(AppHost::new(&creation.egui_ctx, app)))
                        }),
                    )
                };
                let failed = |webgpu: Option<WebGpuUnavailable>, webgl2: Option<String>| {
                    JsValue::from_str(&renderer::startup_failure(
                        request,
                        webgpu.as_ref(),
                        webgl2.as_deref(),
                    ))
                };
                match request {
                    RendererRequest::WebGpu if !webgpu_present => {
                        return Err(failed(Some(WebGpuUnavailable::Missing), None));
                    }
                    RendererRequest::WebGpu => {
                        let options = with_backends(Backends::BROWSER_WEBGPU);
                        let error = match attempt(canvas, options, None).await {
                            Ok(()) => return Ok(JsValue::UNDEFINED),
                            Err(error) => describe(&error),
                        };
                        let webgpu = if wgpu::util::is_browser_webgpu_supported().await {
                            WebGpuUnavailable::Failed(error)
                        } else {
                            WebGpuUnavailable::NoAdapter
                        };
                        return Err(failed(Some(webgpu), None));
                    }
                    RendererRequest::WebGl2 => {
                        let options = with_backends(Backends::GL);
                        return match attempt(canvas, options, None).await {
                            Ok(()) => Ok(JsValue::UNDEFINED),
                            Err(error) => Err(failed(None, Some(describe(&error)))),
                        };
                    }
                    RendererRequest::Auto => {}
                }
                // Unchanged eframe defaults: WebGPU when it has an adapter,
                // otherwise WebGL2.
                let error = match attempt(canvas.clone(), eframe::WebOptions::default(), None).await
                {
                    Ok(()) => return Ok(JsValue::UNDEFINED),
                    Err(error) => describe(&error),
                };
                // The application was already created, so the renderer
                // started and something later failed; a retry cannot help.
                if create.borrow().is_none() {
                    return Err(JsValue::from_str(&error));
                }
                if !webgpu_present {
                    return Err(failed(Some(WebGpuUnavailable::Missing), Some(error)));
                }
                if !wgpu::util::is_browser_webgpu_supported().await {
                    return Err(failed(Some(WebGpuUnavailable::NoAdapter), Some(error)));
                }
                // WebGPU had an adapter but did not start. Retry on WebGL2 in
                // a fresh canvas, because this one holds a WebGPU context.
                let webgpu = WebGpuUnavailable::Failed(error);
                let fresh = replace_canvas(&canvas)?;
                match attempt(fresh, with_backends(Backends::GL), Some(webgpu.clone())).await {
                    Ok(()) => Ok(JsValue::UNDEFINED),
                    Err(error) => Err(failed(Some(webgpu), Some(describe(&error)))),
                }
            })
        }

        /// Read the running host, for application-specific read-only hooks.
        pub fn with_host<R>(&self, read: impl FnOnce(&AppHost<A>) -> R) -> Result<R, JsValue> {
            let host = self
                .runner
                .app_mut::<AppHost<A>>()
                .ok_or_else(|| JsValue::from_str("the application is not running"))?;
            Ok(read(&host))
        }

        /// Change the running host from a page hook, for example with
        /// [`AppHost::submit`]. Runs on the application thread between passes.
        pub fn with_host_mut<R>(
            &self,
            change: impl FnOnce(&mut AppHost<A>) -> R,
        ) -> Result<R, JsValue> {
            let mut host = self
                .runner
                .app_mut::<AppHost<A>>()
                .ok_or_else(|| JsValue::from_str("the application is not running"))?;
            Ok(change(&mut host))
        }

        /// Enable the fixed automation dispatcher with a fresh per-startup nonce,
        /// such as `crypto.randomUUID()`.
        pub fn enable_automation(&self, instance: &str) -> Result<(), JsValue> {
            let mut host = self
                .runner
                .app_mut::<AppHost<A>>()
                .ok_or_else(|| JsValue::from_str("the application is not running"))?;
            if host.inspection().is_none() {
                let identity = host.identity.clone();
                let inspection = Inspection::with_instance(identity.name, identity.build, instance)
                    .map_err(|error| JsValue::from_str(&error.to_string()))?;
                host.enable_inspection(inspection);
            }
            Ok(())
        }

        /// Handle one versioned automation request on the application thread.
        pub fn automation_request(&self, request: &str) -> Result<String, JsValue> {
            let mut host = self
                .runner
                .app_mut::<AppHost<A>>()
                .ok_or_else(|| JsValue::from_str("the application is not running"))?;
            host.dispatch_json(request)
                .map_err(|error| JsValue::from_str(&error))
        }

        pub fn destroy(&self) {
            self.runner.destroy();
            if let Some(key) = self.layer.take() {
                super::aria_dom::remove(key);
            }
        }
    }

    /// eframe's default browser options restricted to `backends`.
    fn with_backends(backends: Backends) -> eframe::WebOptions {
        let mut options = eframe::WebOptions::default();
        if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup
        {
            setup.instance_descriptor.backends = backends;
        }
        options
    }

    fn page_query() -> String {
        web_sys::window()
            .and_then(|window| window.location().search().ok())
            .unwrap_or_default()
    }

    /// Report a startup milestone to the shared startup loader
    /// (`browser-startup.js`) when the page installed it. Its marks are
    /// one-shot, so an application that marks a milestone itself keeps its own
    /// earlier mark.
    pub fn startup_mark(name: &str) {
        let global = js_sys::global();
        if let Ok(mark) = js_sys::Reflect::get(&global, &JsValue::from_str("__stobieStartupMark"))
            && let Some(mark) = mark.dyn_ref::<js_sys::Function>()
        {
            let _ = mark.call1(&JsValue::NULL, &JsValue::from_str(name));
        }
    }

    fn navigator_has_gpu() -> bool {
        web_sys::window().is_some_and(|window| {
            js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("gpu"))
                .is_ok_and(|gpu| !gpu.is_undefined() && !gpu.is_null())
        })
    }

    /// A startup error's message, as `renderer::error_line` trims it.
    fn describe(error: &JsValue) -> String {
        let text = error
            .as_string()
            .or_else(|| {
                error
                    .dyn_ref::<js_sys::Error>()
                    .map(|error| error.message().into())
            })
            .unwrap_or_else(|| format!("{error:?}"));
        renderer::error_line(&text)
    }

    /// Put a context-free copy of `canvas` in its place and return it.
    fn replace_canvas(
        canvas: &web_sys::HtmlCanvasElement,
    ) -> Result<web_sys::HtmlCanvasElement, JsValue> {
        let fresh: web_sys::HtmlCanvasElement = canvas.clone_node()?.dyn_into()?;
        canvas.replace_with_with_node_1(&fresh)?;
        Ok(fresh)
    }

    fn mark_canvas(canvas: &web_sys::HtmlCanvasElement, identity: &RendererIdentity) {
        let _ = canvas.set_attribute("data-stobie-renderer", &identity.backend);
        let _ = canvas.set_attribute("data-stobie-renderer-adapter", &identity.adapter);
        let _ = canvas.set_attribute(
            "data-stobie-renderer-selection",
            identity.selection.as_str(),
        );
        match &identity.reason {
            Some(reason) => {
                let _ = canvas.set_attribute("data-stobie-renderer-reason", reason);
            }
            None => {
                let _ = canvas.remove_attribute("data-stobie-renderer-reason");
            }
        }
    }
}

/// Export a browser `WebHandle` for a hosted application from a `cdylib`.
/// The crate needs `wasm-bindgen` as a direct dependency, at the version of
/// its bindgen CLI, because the generated bindings name it.
///
/// The handle offers `new()`, `start(canvas)`, `enable_automation(instance)`,
/// `automation_request(json)` and `destroy()`. With the shared startup loader
/// on the page, the handle and host report construction and the first
/// complete UI pass to it. Add application-specific
/// read-only methods in another `#[wasm_bindgen] impl WebHandle` block, using
/// `self.host.with_host(...)`.
///
/// ```text
/// #[cfg(target_arch = "wasm32")]
/// stobie_ui_egui::web_handle!(Notes, |_creation| Notes::new());
/// ```
#[macro_export]
macro_rules! web_handle {
    ($application:ty, $create:expr) => {
        #[$crate::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub struct WebHandle {
            host: $crate::WebHost<$application>,
        }

        #[$crate::__private::wasm_bindgen::prelude::wasm_bindgen]
        impl WebHandle {
            #[wasm_bindgen(constructor)]
            pub fn new() -> Self {
                Self {
                    host: ::std::default::Default::default(),
                }
            }

            /// Start the application in `canvas`; await the returned promise.
            pub fn start(
                &self,
                canvas: $crate::__private::web_sys::HtmlCanvasElement,
            ) -> $crate::__private::js_sys::Promise {
                self.host.start(canvas, |creation| {
                    $crate::__private::startup_mark("application_construct_begin");
                    let application = ($create)(creation);
                    $crate::__private::startup_mark("application_construct_end");
                    application
                })
            }

            /// Enable the fixed, versioned automation dispatcher.
            pub fn enable_automation(
                &self,
                instance: &str,
            ) -> ::std::result::Result<(), $crate::__private::wasm_bindgen::JsValue> {
                self.host.enable_automation(instance)
            }

            /// Handle one automation request. Enabled explicitly by the page.
            pub fn automation_request(
                &self,
                request: &str,
            ) -> ::std::result::Result<
                ::std::string::String,
                $crate::__private::wasm_bindgen::JsValue,
            > {
                self.host.automation_request(request)
            }

            pub fn destroy(&self) {
                self.host.destroy();
            }
        }

        impl ::std::default::Default for WebHandle {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}
