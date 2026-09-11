use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
fn encode_message(value: &serde_json::Value) -> Result<JsValue, serde_wasm_bindgen::Error> {
    value.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
}
use crate::engine::{Event, Job, WorkerMetrics, pacing_now_ms};
use polyorama_runtime::RegionalRequest;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

#[derive(Serialize, Deserialize)]
struct TransferPixels {
    width: u32,
    height: u32,
    layout: polyorama_core::SampleLayout,
    precision: u8,
    #[serde(with = "serde_wasm_bindgen::preserve")]
    samples: js_sys::Uint16Array,
}
#[derive(Deserialize)]
struct TransferCompletion {
    request: RegionalRequest,
    pixels: TransferPixels,
    metrics: WorkerMetrics,
}
fn decode_event(value: JsValue) -> Result<Event, JsValue> {
    let completion = js_sys::Reflect::get(&value, &JsValue::from_str("Completed"))?;
    if completion.is_undefined() {
        return serde_wasm_bindgen::from_value(value).map_err(Into::into);
    }
    let c: TransferCompletion = serde_wasm_bindgen::from_value(completion)?;
    if c.pixels.samples.length() as usize > c.request.max_decoded_bytes / 4 {
        return Ok(Event::Failed {
            request: Some(c.request),
            error: "transferred output exceeds reservation".into(),
            metrics: c.metrics,
        });
    }
    // Typed transfer plus one exact-sized WASM copy stays within the reserved pair.
    let pixels = polyorama_core::RegionalPixels {
        width: c.pixels.width,
        height: c.pixels.height,
        layout: c.pixels.layout,
        precision: c.pixels.precision,
        samples: c.pixels.samples.to_vec(),
    };
    Ok(Event::Completed {
        request: c.request,
        pixels,
        metrics: c.metrics,
    })
}
pub struct Executor {
    worker: Worker,
    events: Rc<RefCell<VecDeque<Event>>>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _error: Closure<dyn FnMut(ErrorEvent)>,
}
impl Executor {
    pub fn new(server: String, compressed: usize, context: egui::Context) -> Self {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        options.set_name("emuella-shared-regional-decoder");
        let worker = Worker::new_with_options("worker.js", &options).expect("browser Worker");
        let events = Rc::new(RefCell::new(VecDeque::new()));
        let sink = events.clone();
        let repaint = context.clone();
        let message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let received_ms = pacing_now_ms();
            let mut event = decode_event(event.data()).unwrap_or_else(|e| Event::Failed {
                request: None,
                error: format!("{e:?}"),
                metrics: WorkerMetrics::default(),
            });
            if let Some(timing) = event.metrics_mut().and_then(|m| m.timing.as_mut()) {
                timing.received_ms = Some(received_ms);
            }
            sink.borrow_mut().push_back(event);
            repaint.request_repaint();
        }) as Box<dyn FnMut(MessageEvent)>);
        let sink = events.clone();
        let error = Closure::wrap(Box::new(move |event: ErrorEvent| {
            sink.borrow_mut().push_back(Event::Failed {
                request: None,
                error: event.message(),
                metrics: WorkerMetrics::default(),
            });
            context.request_repaint();
        }) as Box<dyn FnMut(ErrorEvent)>);
        worker.set_onmessage(Some(message.as_ref().unchecked_ref()));
        worker.set_onerror(Some(error.as_ref().unchecked_ref()));
        worker
            .post_message(
                &encode_message(
                    &serde_json::json!({"kind":"init","server":server,"compressed":compressed}),
                )
                .expect("init encoding"),
            )
            .expect("worker init");
        Self {
            worker,
            events,
            _message: message,
            _error: error,
        }
    }
    pub fn submit(&self, job: Job) -> Result<()> {
        self.worker
            .post_message(
                &encode_message(&serde_json::json!({"kind":"job","job":job}))
                    .map_err(|e| anyhow!(e.to_string()))?,
            )
            .map_err(|e| anyhow!(format!("{e:?}")))
    }
    pub fn cancel(&self, request: &RegionalRequest) {
        let _ = self.worker.post_message(
            &encode_message(&serde_json::json!({"kind":"cancel","request":request})).unwrap(),
        );
    }
    pub fn drain(&self) -> Vec<Event> {
        self.events.borrow_mut().drain(..).collect()
    }
}
impl Drop for Executor {
    fn drop(&mut self) {
        self.worker.set_onmessage(None);
        self.worker.set_onerror(None);
        self.worker.terminate();
    }
}

use crate::engine::Engine;
use emuella_viewer_source::Manifest;
use wasm_bindgen::prelude::*;
fn js(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
/// Instantiated exclusively inside worker.js; never used by the canvas application.
#[wasm_bindgen]
pub struct WorkerClient {
    engine: Engine,
    reader: Option<emuella_viewer_source::ResponseReader>,
}
#[wasm_bindgen]
impl WorkerClient {
    #[wasm_bindgen(constructor)]
    pub fn new(compressed: usize) -> Self {
        Self {
            engine: Engine::new(compressed),
            reader: None,
        }
    }
    pub fn register(&mut self, value: JsValue) -> Result<(), JsValue> {
        let m: Manifest = serde_wasm_bindgen::from_value(value)?;
        self.engine.client.register(m).map_err(js)
    }
    pub fn missing(&mut self, value: JsValue) -> Result<JsValue, JsValue> {
        let j: Job = serde_wasm_bindgen::from_value(value)?;
        serde_wasm_bindgen::to_value(
            &self
                .engine
                .client
                .missing_tiles(&j.manifest.tid, &j.region())
                .map_err(js)?,
        )
        .map_err(Into::into)
    }
    pub fn descriptor(&mut self, tid: &str, tile: u16, bytes: &[u8]) -> Result<(), JsValue> {
        self.engine
            .client
            .install_descriptor(tid, tile, bytes)
            .map_err(js)
    }
    pub fn query(&mut self, value: JsValue) -> Result<String, JsValue> {
        let j: Job = serde_wasm_bindgen::from_value(value)?;
        self.engine.metrics.requests += 1;
        emuella_viewer_source::checked(
            self.engine
                .client
                .request(&j.manifest.tid, &j.region(), (256 << 10) as u64)
                .map_err(js)?
                .query(),
        )
        .map_err(js)
    }
    pub fn begin(&mut self, tid: &str, value: JsValue) -> Result<(), JsValue> {
        // A rejected response must not leave an earlier reader available for admission.
        self.reader = None;
        let headers: Vec<(String, String)> = serde_wasm_bindgen::from_value(value)?;
        self.reader = Some(
            self.engine
                .client
                .begin_response_headers(
                    tid,
                    headers
                        .iter()
                        .map(|(name, value)| (name.as_str(), value.as_str())),
                )
                .map_err(js)?,
        );
        Ok(())
    }
    pub fn receive(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        self.engine
            .client
            .receive(
                self.reader.as_mut().ok_or_else(|| js("no response"))?,
                bytes,
            )
            .map_err(js)
    }
    pub fn abandon_response(&mut self) {
        self.reader = None;
    }
    pub fn finish(&mut self) -> Result<(), JsValue> {
        self.engine
            .client
            .finish(self.reader.take().ok_or_else(|| js("no response"))?)
            .map_err(js)
    }
    pub fn ready(&mut self, value: JsValue) -> Result<bool, JsValue> {
        let j: Job = serde_wasm_bindgen::from_value(value)?;
        self.engine
            .client
            .ready(&j.manifest.tid, &j.region())
            .map_err(js)
    }
    pub fn decode(&mut self, value: JsValue) -> Result<JsValue, JsValue> {
        let job: Job = serde_wasm_bindgen::from_value(value)?;
        let pixels = self.engine.decode(&job).map_err(js)?;
        let transfer = TransferPixels {
            width: pixels.width,
            height: pixels.height,
            layout: pixels.layout,
            precision: pixels.precision,
            samples: js_sys::Uint16Array::from(pixels.samples.as_slice()),
        };
        serde_wasm_bindgen::to_value(&transfer).map_err(Into::into)
    }
    pub fn metrics(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.engine.snapshot()).map_err(Into::into)
    }
}
