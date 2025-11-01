//! X11 screen capture implementation using GStreamer.

use crate::capture::{CaptureConfig, Frame, HardwareAccel};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;

/// Start X11 screen capture
pub async fn start_capture(
    config: CaptureConfig,
    frame_sender: broadcast::Sender<Frame>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize GStreamer
    gst::init()?;

    let monitors = super::monitor::get_monitors()?;
    log::info!("Detected {} monitors", monitors.len());

    // Create a capture task for each monitor
    for monitor_idx in &config.monitors {
        if *monitor_idx >= monitors.len() {
            log::warn!("Monitor index {} out of range, skipping", monitor_idx);
            continue;
        }

        let monitor = &monitors[*monitor_idx];
        log::info!(
            "Starting capture for monitor {}: {} ({}x{}+{}+{})",
            monitor.index,
            monitor.name,
            monitor.width,
            monitor.height,
            monitor.x,
            monitor.y
        );

        let config_clone = config.clone();
        let sender_clone = frame_sender.clone();
        let monitor_clone = monitor.clone();

        tokio::spawn(async move {
            if let Err(e) = capture_monitor(monitor_clone, config_clone, sender_clone).await {
                log::error!("Capture error for monitor {}: {}", monitor_clone.index, e);
            }
        });
    }

    Ok(())
}

/// Capture a single monitor using GStreamer
async fn capture_monitor(
    monitor: super::monitor::MonitorInfo,
    config: CaptureConfig,
    frame_sender: broadcast::Sender<Frame>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Build GStreamer pipeline based on hardware acceleration
    let pipeline_str = build_pipeline(&monitor, &config)?;
    log::info!(
        "GStreamer pipeline for monitor {}: {}",
        monitor.index,
        pipeline_str
    );

    let pipeline = gst::parse::launch(&pipeline_str)?;
    let pipeline = pipeline
        .dynamic_cast::<gst::Pipeline>()
        .map_err(|_| "Failed to cast to Pipeline")?;

    // Get the appsink element to receive encoded frames
    let appsink = pipeline
        .by_name("sink")
        .ok_or("Failed to find appsink element")?
        .dynamic_cast::<gst_app::AppSink>()
        .map_err(|_| "Failed to cast to AppSink")?;

    // Configure appsink callbacks
    let monitor_idx = monitor.index;
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Error)?;
                let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;

                let frame = Frame {
                    data: map.as_slice().to_vec(),
                    width: monitor.width,
                    height: monitor.height,
                    timestamp: buffer.pts().map(|pts| pts.nseconds() / 1000).unwrap_or(0),
                    monitor_index: monitor_idx,
                };

                // Send frame to subscribers (ignore if no receivers)
                let _ = frame_sender.send(frame);

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    // Start the pipeline
    pipeline.set_state(gst::State::Playing)?;

    // Wait for pipeline to finish or error
    let bus = pipeline.bus().ok_or("Pipeline without bus")?;

    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => {
                log::info!("Monitor {} capture ended", monitor_idx);
                break;
            }
            MessageView::Error(err) => {
                log::error!(
                    "Monitor {} capture error: {} ({})",
                    monitor_idx,
                    err.error(),
                    err.debug().unwrap_or_else(|| "".into())
                );
                break;
            }
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null)?;
    Ok(())
}

/// Build GStreamer pipeline string based on configuration
fn build_pipeline(
    monitor: &super::monitor::MonitorInfo,
    config: &CaptureConfig,
) -> Result<String, Box<dyn std::error::Error>> {
    let source = format!(
        "ximagesrc startx={} starty={} endx={} endy={} use-damage=false",
        monitor.x,
        monitor.y,
        monitor.x + monitor.width as i32,
        monitor.y + monitor.height as i32
    );

    let framerate_caps = format!("video/x-raw,framerate={}/1", config.framerate);

    // Determine encoder based on hardware acceleration
    let encoder = match config.hw_accel {
        HardwareAccel::None => {
            format!(
                "x264enc tune=zerolatency speed-preset={} bitrate={} key-int-max={}",
                config.quality.preset,
                config.quality.bitrate,
                config.framerate * 2 // Keyframe every 2 seconds
            )
        }
        HardwareAccel::Nvenc => {
            // NVIDIA NVENC encoder
            format!(
                "nvh264enc preset=low-latency-hq rc-mode=cbr bitrate={} gop-size={}",
                config.quality.bitrate,
                config.framerate * 2
            )
        }
        HardwareAccel::Vaapi => {
            // AMD/Intel VAAPI encoder
            format!(
                "vaapih264enc rate-control=cbr bitrate={} keyframe-period={}",
                config.quality.bitrate,
                config.framerate * 2
            )
        }
        HardwareAccel::Auto => {
            // Try hardware first, fall back to software
            if is_nvenc_available() {
                format!(
                    "nvh264enc preset=low-latency-hq rc-mode=cbr bitrate={} gop-size={}",
                    config.quality.bitrate,
                    config.framerate * 2
                )
            } else if is_vaapi_available() {
                format!(
                    "vaapih264enc rate-control=cbr bitrate={} keyframe-period={}",
                    config.quality.bitrate,
                    config.framerate * 2
                )
            } else {
                format!(
                    "x264enc tune=zerolatency speed-preset={} bitrate={} key-int-max={}",
                    config.quality.preset,
                    config.quality.bitrate,
                    config.framerate * 2
                )
            }
        }
    };

    let pipeline = format!(
        "{} ! {} ! videoconvert ! {} ! h264parse ! video/x-h264,stream-format=byte-stream ! \
         appsink name=sink",
        source, framerate_caps, encoder
    );

    Ok(pipeline)
}

/// Check if NVENC is available
fn is_nvenc_available() -> bool {
    // Check if nvenc plugin is available in GStreamer
    let registry = gst::Registry::get();
    registry.find_plugin("nvcodec").is_some()
}

/// Check if VAAPI is available
fn is_vaapi_available() -> bool {
    // Check if vaapi plugin is available in GStreamer
    let registry = gst::Registry::get();
    registry.find_plugin("vaapi").is_some()
}
