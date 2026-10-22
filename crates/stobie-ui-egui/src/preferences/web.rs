//! The browser's text size, read from computed styles through `js_sys` so
//! that every browser build has it, with or without eframe.

use std::cell::Cell;

use js_sys::{Array, Function, Reflect};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

thread_local! {
    /// The last font scale read, until an event that can follow a change of
    /// text size clears it.
    static SCALE: Cell<Option<f32>> = const { Cell::new(None) };
    static WATCHING: Cell<bool> = const { Cell::new(false) };
}

/// The page's font scale, read again after it regains focus, is shown again
/// or is resized. Asking costs nothing in between, so the host can ask in
/// every pass. A text size changed in the browser's or the system's settings
/// is changed elsewhere, so the page sees one of these events on the way
/// back, and eframe runs a pass for each of them; the host then reapplies the
/// design system and records why it repaints.
pub(super) fn font_scale() -> f32 {
    if let Some(scale) = SCALE.get() {
        return scale;
    }
    watch();
    let root = document_element().and_then(|root| font_size(&root));
    let scale = super::font_scale_for(root.as_deref(), system_body_font_size().as_deref());
    SCALE.set(Some(scale));
    scale
}

fn watch() {
    if WATCHING.replace(true) {
        return;
    }
    let global = JsValue::from(js_sys::global());
    let Some(document) = get(&global, "document") else {
        return;
    };
    let forget = Closure::<dyn Fn()>::new(|| SCALE.set(None));
    let listener: &Function = forget.as_ref().unchecked_ref();
    for (target, event) in [
        (&global, "focus"),
        (&global, "pageshow"),
        (&global, "resize"),
        (&document, "visibilitychange"),
    ] {
        call(
            target,
            "addEventListener",
            &Array::of2(&event.into(), listener),
        );
    }
    // The listeners live as long as the page.
    forget.forget();
}

fn document_element() -> Option<JsValue> {
    get(
        &get(&js_sys::global().into(), "document")?,
        "documentElement",
    )
}

/// The computed font size of `element`, such as `"16px"`.
fn font_size(element: &JsValue) -> Option<String> {
    let style = call(
        &js_sys::global().into(),
        "getComputedStyle",
        &Array::of1(element),
    )?;
    get(&style, "fontSize")?.as_string()
}

/// WebKit's system body font size, which follows Dynamic Type on iOS, read
/// from a hidden element that is removed before anything paints. Other
/// engines do not know the font, and give `None`.
fn system_body_font_size() -> Option<String> {
    let global = JsValue::from(js_sys::global());
    let supports = Array::of2(&"font".into(), &"-apple-system-body".into());
    if !call(&get(&global, "CSS")?, "supports", &supports)?.as_bool()? {
        return None;
    }
    let document = get(&global, "document")?;
    let root = get(&document, "documentElement")?;
    let probe = call(&document, "createElement", &Array::of1(&"span".into()))?;
    let style = get(&probe, "style")?;
    for (property, value) in [
        ("font", "-apple-system-body"),
        ("position", "absolute"),
        ("visibility", "hidden"),
    ] {
        call(
            &style,
            "setProperty",
            &Array::of2(&property.into(), &value.into()),
        )?;
    }
    call(&root, "appendChild", &Array::of1(&probe))?;
    let size = font_size(&probe);
    call(&probe, "remove", &Array::new());
    size
}

fn get(object: &JsValue, key: &str) -> Option<JsValue> {
    Reflect::get(object, &key.into())
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

/// `object[method](...arguments)`, or `None` where it is missing or throws.
fn call(object: &JsValue, method: &str, arguments: &Array) -> Option<JsValue> {
    let function: Function = get(object, method)?.dyn_into().ok()?;
    Reflect::apply(&function, object, arguments).ok()
}
