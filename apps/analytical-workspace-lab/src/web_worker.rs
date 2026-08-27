use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{MessageEvent, Worker, WorkerOptions, WorkerType};
use workspace_runtime::{DecodeEvent, DecodeRequest};

pub struct BrowserWorker {
    worker: Worker,
    events: Rc<RefCell<VecDeque<DecodeEvent>>>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
}

impl BrowserWorker {
    pub fn new(context: egui::Context) -> Result<Self, wasm_bindgen::JsValue> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        options.set_name("polyorama-scalar-decoder");
        let worker = Worker::new_with_options("worker.js", &options)?;
        let events = Rc::new(RefCell::new(VecDeque::new()));
        let sink = events.clone();
        let on_message = Closure::wrap(Box::new(move |message: MessageEvent| {
            if let Ok(event) = serde_wasm_bindgen::from_value(message.data()) {
                sink.borrow_mut().push_back(event);
                context.request_repaint();
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        Ok(Self {
            worker,
            events,
            _on_message: on_message,
        })
    }

    pub fn submit(&self, request: &DecodeRequest) -> Result<(), wasm_bindgen::JsValue> {
        self.worker
            .post_message(&serde_wasm_bindgen::to_value(request)?)
    }

    pub fn drain(&self) -> Vec<DecodeEvent> {
        self.events.borrow_mut().drain(..).collect()
    }
}
