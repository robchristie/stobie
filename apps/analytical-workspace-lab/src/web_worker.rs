use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use polyorama_runtime::{DecodeEvent, DecodeRequest};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

pub struct BrowserWorker {
    worker: Worker,
    events: Rc<RefCell<VecDeque<DecodeEvent>>>,
    failures: Rc<RefCell<VecDeque<String>>>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(ErrorEvent)>,
    _on_message_error: Closure<dyn FnMut(MessageEvent)>,
}

impl BrowserWorker {
    pub fn new(context: egui::Context) -> Result<Self, wasm_bindgen::JsValue> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        options.set_name("polyorama-scalar-decoder");
        let worker = Worker::new_with_options("worker.js", &options)?;
        let events = Rc::new(RefCell::new(VecDeque::new()));
        let failures = Rc::new(RefCell::new(VecDeque::new()));
        let sink = events.clone();
        let malformed_sink = failures.clone();
        let message_context = context.clone();
        let on_message = Closure::wrap(Box::new(move |message: MessageEvent| {
            match serde_wasm_bindgen::from_value(message.data()) {
                Ok(event) => sink.borrow_mut().push_back(event),
                Err(error) => malformed_sink.borrow_mut().push_back(format!(
                    "browser worker returned an invalid message: {error}"
                )),
            }
            message_context.request_repaint();
        }) as Box<dyn FnMut(MessageEvent)>);
        let error_sink = failures.clone();
        let error_context = context.clone();
        let on_error = Closure::wrap(Box::new(move |error: ErrorEvent| {
            error_sink.borrow_mut().push_back(format!(
                "browser worker failed at {}:{}: {}",
                error.filename(),
                error.lineno(),
                error.message()
            ));
            error_context.request_repaint();
        }) as Box<dyn FnMut(ErrorEvent)>);
        let transport_sink = failures.clone();
        let on_message_error = Closure::wrap(Box::new(move |_message: MessageEvent| {
            transport_sink
                .borrow_mut()
                .push_back("browser worker message deserialisation failed".into());
            context.request_repaint();
        }) as Box<dyn FnMut(MessageEvent)>);
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        worker.set_onmessageerror(Some(on_message_error.as_ref().unchecked_ref()));
        Ok(Self {
            worker,
            events,
            failures,
            _on_message: on_message,
            _on_error: on_error,
            _on_message_error: on_message_error,
        })
    }

    pub fn submit(&self, request: &DecodeRequest) -> Result<(), wasm_bindgen::JsValue> {
        self.worker
            .post_message(&serde_wasm_bindgen::to_value(request)?)
    }

    pub fn drain(&self) -> Vec<DecodeEvent> {
        self.events.borrow_mut().drain(..).collect()
    }

    pub fn drain_failures(&self) -> Vec<String> {
        self.failures.borrow_mut().drain(..).collect()
    }
}
