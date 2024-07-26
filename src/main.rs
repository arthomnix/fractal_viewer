use eframe::NativeOptions;
use fractal_viewer::FractalViewerApp;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let options = NativeOptions::default();
    eframe::run_native(
        "fractal_viewer",
        options,
        Box::new(|cc| Ok(Box::new(FractalViewerApp::new(cc).unwrap()))),
    )
}
