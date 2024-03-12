use std::collections::HashMap;
use std::io::Error;
use std::sync::Arc;

use gstreamer::{element_error, glib, prelude::*, Caps, Element, Pipeline};
use tokio::runtime::Runtime;
use tokio::sync::Barrier;
use tokio::sync::mpsc::Sender;

use crate::utils::gstreamer_utils::read_bus;
use crate::utils::shutdown;
use super::audio_const::AUDIO_CAPTURE_PIPELINE_NAME;

pub async fn start_audio_capture(
    tx_audio: Sender<Vec<u8>>,
    shutdown: shutdown::Shutdown,
    barrier: Arc<Barrier>,
) {
    shutdown.add_task().await;

    // Initialize GStreamer
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
    //TODO: handleo de errores al igual que en video
    sink.set_callbacks(
        gstreamer_app::AppSinkCallbacks::builder()
            // Add a handler to the "new-sample" signal.
            .new_sample(move |appsink| {
                // Pull the sample in question out of the appsink's buffer.
                let sample = appsink.pull_sample().map_err(|_| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to pull sample")
                    );
                    gstreamer::FlowError::Error
                })?;

                let buffer = sample.buffer().ok_or_else(|| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to get buffer from appsink in audio pipeline")
                    );
                    gstreamer::FlowError::Error
                })?;

                let map = buffer.map_readable().map_err(|_| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to map buffer readable")
                    );

                    gstreamer::FlowError::Error
                })?;

                let samples = map.as_slice();

                let rt = Runtime::new().map_err(|_| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to create runtime")
                    );
                    gstreamer::FlowError::Error
                })?;
                rt.block_on(async {
                    
                    tx_audio
                        .send(samples.to_vec())
                        .await
                        .expect("Error sending audio sample");
                });


                Ok(gstreamer::FlowSuccess::Ok)
            })
            .build(),
    );

    // Start playing
    if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    Ok(pipeline)
}
