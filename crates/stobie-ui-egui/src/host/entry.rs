//! Native and browser entry points for an [`AppHost`].

use super::{AppHost, Application};

impl<A: Application> eframe::App for AppHost<A> {
    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.present(ui);
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
        Box::new(move |creation| Ok(Box::new(AppHost::new(&creation.egui_ctx, create(creation))))),
    )
}

#[cfg(target_arch = "wasm32")]
pub use web::WebHost;

#[cfg(target_arch = "wasm32")]
mod web {
    use std::marker::PhantomData;

    use wasm_bindgen::JsValue;

    use crate::{AppHost, Application, Inspection};

    /// A browser runner for a hosted application. [`web_handle!`](crate::web_handle)
    /// exports it to JavaScript; applications rarely use it directly.
    pub struct WebHost<A: Application> {
        runner: eframe::WebRunner,
        application: PhantomData<fn() -> A>,
    }

    impl<A: Application> Default for WebHost<A> {
        fn default() -> Self {
            console_error_panic_hook::set_once();
            Self {
                runner: eframe::WebRunner::new(),
                application: PhantomData,
            }
        }
    }

    impl<A: Application> WebHost<A> {
        /// Start the application in `canvas`; the promise settles once it runs.
        pub fn start(
            &self,
            canvas: web_sys::HtmlCanvasElement,
            create: impl FnOnce(&eframe::CreationContext<'_>) -> A + 'static,
        ) -> js_sys::Promise {
            let runner = self.runner.clone();
            wasm_bindgen_futures::future_to_promise(async move {
                runner
                    .start(
                        canvas,
                        eframe::WebOptions::default(),
                        Box::new(move |creation| {
                            Ok(Box::new(AppHost::new(&creation.egui_ctx, create(creation))))
                        }),
                    )
                    .await?;
                Ok(JsValue::UNDEFINED)
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
        }
    }
}

/// Export a browser `WebHandle` for a hosted application from a `cdylib`.
/// The crate needs `wasm-bindgen` as a direct dependency, at the version of
/// its bindgen CLI, because the generated bindings name it.
///
/// The handle offers `new()`, `start(canvas)`, `enable_automation(instance)`,
/// `automation_request(json)` and `destroy()`. Add application-specific
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
                self.host.start(canvas, $create)
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
