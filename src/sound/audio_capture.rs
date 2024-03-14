use std::collections::HashMap;
use std::io::Error;
use std::sync::Arc;

use gstreamer::{glib, prelude::*, Caps, Element, Pipeline};
use tokio::sync::mpsc::Sender;
use tokio::sync::Barrier;

use super::audio_const::AUDIO_CAPTURE_PIPELINE_NAME;
use crate::utils::gstreamer_utils::{pull_sample, read_bus};
use crate::utils::shutdown;

/// Starts the audio capturer by creating the pipeline and sending the audio frames throug the provided Sender.
///
/// # Arguments
///
/// * `tx_audio` - `A Sender<Vec<u8>>` used to send audio frames.
/// * `shutdown` - Used for graceful shutdown.
/// * `barrier` - A used for synchronization.
pub async fn start_audio_capture(
    tx_audio: Sender<Vec<u8>>,
    shutdown: shutdown::Shutdown,
    barrier: Arc<Barrier>,
) {
    shutdown.add_task().await;

    if let Err(e) = gstreamer::init() {
        log::error!(
            "AUDIO CAPTURE | Error initializing GStreamer: {}",
            e.to_string()
        );
        shutdown.notify_error(false).await;
        return;
    };

    barrier.wait().await;
    println!("AUDIO CAPTURE | Barrier released");

    let caps = gstreamer::Caps::builder("audio/x-raw")
        //.field("rate", 48000)
        .field("channels", 2)
        .build();

    let elements = match create_elements() {
        Ok(e) => e,
        Err(e) => {
            log::error!("AUDIO CAPTURE | Error creating elements: {}", e.message);
            shutdown.notify_error(false).await;
            return;
        }
    };

    let pipeline = match create_pipeline(tx_audio, elements, caps) {
        Ok(p) => p,
        Err(e) => {
            log::error!("AUDIO CAPTURE | Error creating pipeline: {}", e.to_string());
            shutdown.notify_error(false).await;
            return;
        }
    };

    let pipeline_cpy = pipeline.clone();
    let shutdown_cpy = shutdown.clone();
    tokio::select! {
        _ = shutdown.wait_for_shutdown() => {
            log::info!("AUDIO CAPTURE | Shutdown received");
        }
        _ = tokio::spawn(async move {
            read_bus(pipeline_cpy, shutdown_cpy).await;
        }) => {
            log::info!("AUDIO CAPTURE | Pipeline finished");
        }
    }

    pipeline
        .set_state(gstreamer::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}

/// Creates GStreamer elements required for audio capture pipeline.
///
/// # Returns
/// A Result containing:
/// * A `HashMap` of Gstreamer elements in case of success.
/// * A `glib::BoolError` in case of error
fn create_elements() -> Result<HashMap<&'static str, Element>, glib::BoolError> {
    let mut elements = HashMap::new();

    // Create the elements
    let wasapi2src = gstreamer::ElementFactory::make("wasapi2src")
        .name("wasapi2src")
        .property("loopback", true)
        .property("low-latency", true)
        .build()?;

    let queue = gstreamer::ElementFactory::make("queue")
        .name("queue")
        .build()?;

    let audioconvert = gstreamer::ElementFactory::make("audioconvert")
        .name("audioconvert")
        .build()?;

    let audioresample = gstreamer::ElementFactory::make("audioresample")
        .name("audioresample")
        .build()?;

    let opusenc = gstreamer::ElementFactory::make("opusenc")
        .name("opusenc")
        .build()?;

    let rtpopuspay = gstreamer::ElementFactory::make("rtpopuspay")
        .name("rtpopuspay")
        .build()?;

    elements.insert("src", wasapi2src);
    elements.insert("queue", queue);
    elements.insert("convert", audioconvert);
    elements.insert("sample", audioresample);
    elements.insert("enc", opusenc);
    elements.insert("pay", rtpopuspay);

    Ok(elements)
}

/// Creates a GStreamer pipeline used for audio capture.
///
/// # Arguments
///
/// * `tx_audio` - A `Sender<Vec<u8>>` used to send audio frames.
/// * `elements` - A HashMap containing the GStreamer elements required for the pipeline.
/// * `caps` - The capabilities of the audio data to be captured.
///
/// # Returns
/// A Result containing:
/// * The constructed GStreamer pipeline in case of success.
/// * A `stdio::Error` in case of error.
fn create_pipeline(
    tx_audio: Sender<Vec<u8>>,
    elements: HashMap<&str, Element>,
    caps: Caps,
) -> Result<Pipeline, Error> {
    let sink = gstreamer_app::AppSink::builder()
        .caps(&gstreamer::Caps::builder("application/x-rtp").build())
        .build();

    let pipeline = gstreamer::Pipeline::with_name(AUDIO_CAPTURE_PIPELINE_NAME);

    if let Err(e) = pipeline.add_many([
        &elements["src"],
        &elements["queue"],
        &elements["convert"],
        &elements["sample"],
        &elements["enc"],
        &elements["pay"],
        &sink.upcast_ref(),
    ]) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    if let Err(e) = elements["src"].link_filtered(&elements["queue"], &caps) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    if let Err(e) = gstreamer::Element::link_many([
        &elements["queue"],
        &elements["convert"],
        &elements["sample"],
        &elements["enc"],
        &elements["pay"],
        &sink.upcast_ref(),
    ]) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    // Otra opcion podria ser: pay (pad probe) fakesink
    sink.set_callbacks(
        gstreamer_app::AppSinkCallbacks::builder()
            .new_sample(
                move |appsink| match pull_sample(appsink, tx_audio.clone()) {
                    Ok(_) => Ok(gstreamer::FlowSuccess::Ok),
                    Err(err) => {
                        log::error!("AUDIO CAPTURE | {}", err);
                        Err(gstreamer::FlowError::Error)
                    }
                },
            )
            .build(),
    );

    // Start playing
    if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    Ok(pipeline)
}
