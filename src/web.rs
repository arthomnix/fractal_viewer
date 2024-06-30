use crate::FractalViewerApp;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
async fn wasm_main() -> Result<(), JsValue> {
    console_log::init().expect("error initialising logger");
    let runner = eframe::WebRunner::new();
    runner
        .start(
            "fv_canvas",
            eframe::WebOptions::default(),
            Box::new(|cc| Box::new(FractalViewerApp::new(cc).unwrap())),
        )
        .await
}
