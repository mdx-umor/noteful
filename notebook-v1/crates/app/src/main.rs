//! Notebook v1: a handwriting notebook for iPad and Apple Pencil, written in Rust.

mod app;
mod applog;
#[cfg(feature = "autotest")]
mod autotest;
mod editor;
mod icons;
mod library_view;
mod prefs;
mod saver;
mod textures;
mod theme;

/// Where crash reports go. On iPad this is visible in Files › On My iPad › Notebook.
fn crash_log_path() -> std::path::PathBuf {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    if cfg!(target_os = "ios") {
        home.join("Documents/Notebook-crash-log.txt")
    } else {
        std::env::temp_dir().join("Notebook-crash-log.txt")
    }
}

fn write_crash_log(msg: &str) {
    let text = format!("Notebook v1 crash report\n\n{msg}\n");
    log::error!("{msg}");
    let path = crash_log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, &text);
    eprintln!("{text}");
}

/// Ask the GPU only for limits it actually has. The generic defaults request 16
/// shader varyings, but iPad GPUs (Metal) report 15, which made start-up fail.
fn gpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    setup.device_descriptor = std::sync::Arc::new(|adapter| {
        let supported = adapter.limits();
        eframe::wgpu::DeviceDescriptor {
            label: Some("notebook device"),
            required_limits: eframe::wgpu::Limits {
                max_texture_dimension_2d: supported.max_texture_dimension_2d.min(8192),
                ..supported
            },
            ..Default::default()
        }
    });
    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

fn main() -> eframe::Result {
    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        write_crash_log(&format!("{info}\n\n{bt}"));
    }));
    applog::init();
    log::info!("Notebook starting (HOME={:?})", std::env::var_os("HOME"));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Notebook")
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([420.0, 400.0]),
        // 4× MSAA keeps ink edges smooth at every zoom level.
        multisampling: if cfg!(feature = "autotest") { 0 } else { 4 },
        #[cfg(feature = "autotest")]
        renderer: eframe::Renderer::Glow,
        wgpu_options: gpu_options(),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Notebook",
        options,
        Box::new(|cc| {
            log::info!("Window ready; creating app");
            let app = app::NotebookApp::new(cc);
            log::info!("App created");
            Ok(Box::new(app))
        }),
    );
    match &result {
        Err(e) => write_crash_log(&format!("Startup failed: {e}")),
        Ok(()) => log::info!("Event loop finished normally"),
    }
    result
}
