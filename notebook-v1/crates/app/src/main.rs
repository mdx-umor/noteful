//! Notebook v1: a handwriting notebook for iPad and Apple Pencil, written in Rust.

mod app;
#[cfg(feature = "autotest")]
mod autotest;
mod editor;
mod icons;
mod library_view;
mod prefs;
mod saver;
mod textures;
mod theme;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Notebook")
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([420.0, 400.0]),
        // 4× MSAA keeps ink edges smooth at every zoom level.
        multisampling: if cfg!(feature = "autotest") { 0 } else { 4 },
        #[cfg(feature = "autotest")]
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Notebook",
        options,
        Box::new(|cc| Ok(Box::new(app::NotebookApp::new(cc)))),
    )
}
