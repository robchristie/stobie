use wasm_bindgen::prelude::*;
use workspace_runtime::{DecodeRequest, decode};

#[wasm_bindgen]
pub fn decode_request(value: JsValue) -> Result<JsValue, JsValue> {
    let request: DecodeRequest = serde_wasm_bindgen::from_value(value)?;
    serde_wasm_bindgen::to_value(&decode(request)).map_err(Into::into)
}
