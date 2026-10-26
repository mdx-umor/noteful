//! Things the browser does for us: picking files, opening links, keeping the
//! screen awake. On other platforms they are quiet no-ops.

/// A picked file, already turned into JPEG pages by the page script.
pub struct Imported {
    /// "image", "pdf", "cancel" or "error".
    pub kind: String,
    /// File name, or the error message.
    pub name: String,
    /// (width, height in points or pixels, JPEG bytes) per page.
    pub pages: Vec<(f32, f32, Vec<u8>)>,
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::Imported;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = NBIO, js_name = start)]
        fn nbio_start(accept: &str);
        #[wasm_bindgen(js_namespace = NBIO, js_name = poll)]
        fn nbio_poll() -> JsValue;
        #[wasm_bindgen(js_namespace = NBIO, js_name = openUrl)]
        fn nbio_open_url(url: &str);
        #[wasm_bindgen(js_namespace = NBIO, js_name = keepAwake)]
        fn nbio_keep_awake(on: bool);
    }

    pub fn pick_file(accept: &str) -> bool {
        nbio_start(accept);
        true
    }

    fn get(o: &JsValue, k: &str) -> JsValue {
        js_sys::Reflect::get(o, &JsValue::from_str(k)).unwrap_or(JsValue::UNDEFINED)
    }

    pub fn poll_import() -> Option<Imported> {
        let v = nbio_poll();
        if v.is_null() || v.is_undefined() {
            return None;
        }
        let kind = get(&v, "kind").as_string().unwrap_or_default();
        let name = get(&v, "name").as_string().unwrap_or_default();
        let mut pages = Vec::new();
        let arr = js_sys::Array::from(&get(&v, "pages"));
        for p in arr.iter() {
            let w = get(&p, "w").as_f64().unwrap_or(0.0) as f32;
            let h = get(&p, "h").as_f64().unwrap_or(0.0) as f32;
            let bytes = js_sys::Uint8Array::new(&get(&p, "jpeg")).to_vec();
            if w > 0.0 && h > 0.0 && !bytes.is_empty() {
                pages.push((w, h, bytes));
            }
        }
        Some(Imported { kind, name, pages })
    }

    pub fn open_url(url: &str) {
        nbio_open_url(url);
    }

    pub fn keep_awake(on: bool) {
        nbio_keep_awake(on);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::Imported;

    pub fn pick_file(_accept: &str) -> bool {
        false
    }

    pub fn poll_import() -> Option<Imported> {
        None
    }

    pub fn open_url(url: &str) {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let _ = std::process::Command::new(opener).arg(url).spawn();
    }

    pub fn keep_awake(_on: bool) {}
}

pub use imp::*;

/// Accept string for the file picker.
pub const ACCEPT_ANY: &str = "application/pdf,image/*";
pub const ACCEPT_IMAGE: &str = "image/*";
