//! Browser-only helpers for the web build.

use wasm_bindgen::JsCast;

/// Offers `bytes` to the user as a downloaded file.
pub fn download(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(&array.buffer());
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &opts).map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a").map_err(err)?.dyn_into().map_err(|_| "anchor")?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

/// Starts the app on the page's canvas.
pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    let options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window().and_then(|w| w.document()).expect("document");
        let canvas = document
            .get_element_by_id("notebook-canvas")
            .expect("canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("canvas");
        let result = eframe::WebRunner::new()
            .start(canvas, options, Box::new(|cc| Ok(Box::new(crate::app::NotebookApp::new(cc)))))
            .await;
        if let Some(loading) = document.get_element_by_id("loading") {
            match result {
                Ok(()) => loading.remove(),
                Err(e) => loading.set_inner_html(&format!("Notebook could not start: {e:?}")),
            }
        }
    });
}
