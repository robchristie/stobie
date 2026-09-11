#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut memory =
        emuella_viewer::memory::MemoryMarkers::from_env().map_err(std::io::Error::other)?;
    if let Some(markers) = memory.as_mut() {
        markers
            .emit("startup", "startup", serde_json::Value::Null)
            .map_err(std::io::Error::other)?;
    }
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| args.windows(2).find(|v| v[0] == flag).map(|v| v[1].clone());
    let server = value("--server").unwrap_or_else(|| "http://127.0.0.1:8088".into());
    let compressed = value("--compressed-mib")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(64)
        << 20;
    let decoded = value("--decoded-mib")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(16)
        << 20;
    let gpu = value("--gpu-mib")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(64)
        << 20;
    let output = value("--script-output");
    let script = output.is_some();
    let workload =
        value("--workload").map(|path| std::fs::read_to_string(path).expect("read workload"));
    let diagnostic = emuella_viewer::DiagnosticOptions {
        enabled: args.iter().any(|a| a == "--native-diagnostics") || memory.is_some(),
        authored_immediate: args.iter().any(|a| a == "--authored-immediate-completion"),
    };
    if diagnostic.authored_immediate && (!diagnostic.enabled || !script || workload.is_none()) {
        return Err(std::io::Error::other("authored immediate completion requires --native-diagnostics, --script-output and --workload").into());
    }
    eframe::run_native(
        "Emuella image viewer",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1440., 900.])
                .with_min_inner_size([960., 640.]),
            ..Default::default()
        },
        Box::new(move |cc| {
            if let Some(markers) = memory.as_mut() {
                markers
                    .emit("graphics-ready", "graphics", serde_json::Value::Null)
                    .map_err(std::io::Error::other)?;
            }
            let mut app = emuella_viewer::ViewerApp::new_diagnostic(
                cc,
                server,
                (compressed, decoded, gpu),
                script,
                output,
                diagnostic,
            );
            app.attach_memory_markers(memory);
            if let Some(json) = workload {
                app.set_script_workload(&json)
                    .map_err(std::io::Error::other)?;
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(Into::into)
}
#[cfg(target_arch = "wasm32")]
fn main() {}
