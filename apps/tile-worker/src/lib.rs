use polyorama_runtime::{DecodeRequest, prepare_and_decode};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn decode_request(value: JsValue) -> Result<JsValue, JsValue> {
    let request: DecodeRequest = serde_wasm_bindgen::from_value(value)?;
    serde_wasm_bindgen::to_value(&prepare_and_decode(request)).map_err(Into::into)
}
