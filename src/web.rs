use crate::FractalViewerApp;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
async fn wasm_main() -> Result<(), JsValue> {
    console_log::init().expect("error initialising logger");

    let canvas = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("fv_canvas"))
        .expect("Failed to get canvas element!");

    let runner = eframe::WebRunner::new();
    runner
        .start(
            canvas.into(),
            eframe::WebOptions::default(),
            Box::new(|cc| Ok(Box::new(FractalViewerApp::new(cc).unwrap()))),
        )
        .await
}
