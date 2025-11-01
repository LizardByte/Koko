mod connection;
mod decoder;
mod input;
mod ui;

use anyhow::Result;
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting Koko Remote Desktop Client");

    // Run the UI
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Koko Remote Desktop Client"),
        ..Default::default()
    };

    eframe::run_native(
        "Koko Client",
        native_options,
        Box::new(|cc| Ok(Box::new(ui::ClientApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run client: {}", e))
}
