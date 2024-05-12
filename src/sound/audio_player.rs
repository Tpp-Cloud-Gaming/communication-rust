use std::{
    collections::HashMap,
    io::{Error, ErrorKind},
    sync::mpsc::Receiver,
};

use gstreamer::{glib, prelude::*, Element, Pipeline};

use crate::utils::{
    gstreamer_utils::{push_sample, read_bus},
    shutdown,
};

use super::audio_const::AUDIO_PLAYER_PIPELINE_NAME;

/// Starts the audio player by creating the pipeline and reading the audio frames from the provided Receiver.
///
/// # Arguments
///
/// * `rx_video` - A `Receiver<Vec<u8>>` for receiving video frames.
/// * `shutdown` - A shutdown handle for managing the finalization of the thread.
pub async fn start_audio_player(rx_audio: Receiver<Vec<u8>>, shutdown: shutdown::Shutdown) {
    // Initialize GStreamer
    if let Err(e) = gstreamer::init() {
        log::error!(
            "AUDIO PLAYER | Error initializing GStreamer: {}",
            e.to_string()
        );
        shutdown.notify_error(false, "Start audio player audio_player").await;
        return;
    };

    // Create the caps
    let caps = gstreamer::Caps::builder("application/x-rtp")
        .field("media", "audio")
        .field("payload", 96)
        .field("clock-rate", 48000)
        .field("encoding-name", "OPUS")
        .build();

    let source = gstreamer_app::AppSrc::builder()
        .caps(&caps)
        .format(gstreamer::Format::Time)
        .is_live(true)
        .do_timestamp(true)
        .build();

    let elements = match create_elements() {
        Ok(e) => e,
        Err(e) => {
            log::error!("AUDIO PLAYER | Error creating elements: {}", e.message);
            shutdown.notify_error(false, "Error creating elements audio player").await;
            return;
        }
    };

    let pipeline = match create_pipeline(elements, rx_audio, source) {
        Ok(p) => p,
        Err(e) => {
            log::error!("AUDIO PLAYER | Error creating pipeline: {}", e.to_string());
            shutdown.notify_error(false, "Error creating pipeline audio player").await;
            return;
        }
    };

    let pipeline_cpy = pipeline.clone();
    let shutdown_cpy = shutdown.clone();
    tokio::select! {
        _ = shutdown.wait_for_error( ) => {
            log::info!("AUDIO PLAYER | Shutdown received");
        }
        _ = tokio::spawn(async move {
            read_bus(pipeline_cpy, shutdown_cpy).await;
        }) => {
            log::info!("AUDIO PLAYER | Pipeline finished");
        }
    }

    pipeline
        .set_state(gstreamer::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}

/// Creates the elements for the audio player pipeline.
///
/// # Returns
///
/// A Result containing a HashMap with the elements if the operation was successful, otherwise an Error is returned.
fn create_elements() -> Result<HashMap<&'static str, Element>, glib::BoolError> {
    let mut elements = HashMap::new();

    let rtpopusdepay = gstreamer::ElementFactory::make("rtpopusdepay")
        .name("rtpopusdepay")
        .build()?;

    let opusparse = gstreamer::ElementFactory::make("opusparse")
        .name("opusparse")
        .build()?;

    let opusdec = gstreamer::ElementFactory::make("opusdec")
        .name("opusdec")
        .build()?;

    let audioconvert = gstreamer::ElementFactory::make("audioconvert")
        .name("audioconvert")
        .build()?;

    let audioresample = gstreamer::ElementFactory::make("audioresample")
        .name("audioresample")
        .build()?;

    let autoaudiosink = gstreamer::ElementFactory::make("autoaudiosink")
        .name("autoaudiosink")
        .build()?;

    elements.insert("depay", rtpopusdepay);
    elements.insert("parse", opusparse);
    elements.insert("dec", opusdec);
    elements.insert("convert", audioconvert);
    elements.insert("sample", audioresample);
    elements.insert("sink", autoaudiosink);

    Ok(elements)
}

/// Creates the pipeline for the audio player.
///
/// # Arguments
///
/// * `elements` - A HashMap containing the elements for the pipeline.
/// * `caps` - The Caps for the pipeline.
/// * `rx_audio` - A `Receiver<Vec<u8>>` for receiving video frames.
///
/// # Returns
///
/// A Result containing the pipeline if the operation was successful, otherwise an Error is returned.
fn create_pipeline(
    elements: HashMap<&str, Element>,
    rx_audio: Receiver<Vec<u8>>,
    source: gstreamer_app::AppSrc,
) -> Result<Pipeline, Error> {
    // Create the empty pipeline
    let pipeline = gstreamer::Pipeline::with_name(AUDIO_PLAYER_PIPELINE_NAME);

    if let Err(e) = pipeline.add_many([
        source.upcast_ref(),
        &elements["depay"],
        &elements["parse"],
        &elements["dec"],
        &elements["convert"],
        &elements["sample"],
        &elements["sink"],
    ]) {
        return Err(Error::new(ErrorKind::Other, e.to_string()));
    }
    if let Err(e) = gstreamer::Element::link_many([
        source.upcast_ref(),
        &elements["depay"],
        &elements["parse"],
        &elements["dec"],
        &elements["convert"],
        &elements["sample"],
        &elements["sink"],
    ]) {
        return Err(Error::new(ErrorKind::Other, e.to_string()));
    };

    // Start playing
    if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
        return Err(Error::new(ErrorKind::Other, e.to_string()));
    }

    source.set_callbacks(
        gstreamer_app::AppSrcCallbacks::builder()
            .need_data(move |appsrc, _| {
                push_sample(appsrc, &rx_audio)
                    .map_err(|err| {
                        log::error!("AUDIO PLAYER | {}", err);
                    })
                    .unwrap();
            })
            .build(),
    );
    Ok(pipeline)
}
