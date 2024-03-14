use std::{
    collections::HashMap,
    io::{Error, ErrorKind},
    sync::mpsc::Receiver,
};

use gstreamer::{glib, prelude::*, Element, Pipeline};

use crate::utils::{gstreamer_utils::read_bus, shutdown};

use super::audio_const::AUDIO_PLAYER_PIPELINE_NAME;

pub async fn start_audio_player(rx_audio: Receiver<Vec<u8>>, shutdown: shutdown::Shutdown) {
    // Initialize GStreamer
    if let Err(e) = gstreamer::init() {
        log::error!(
            "AUDIO PLAYER | Error initializing GStreamer: {}",
            e.to_string()
        );
        shutdown.notify_error(false).await;
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
            shutdown.notify_error(false).await;
            return;
        }
    };

    let pipeline = match create_pipeline(elements, rx_audio, source) {
        Ok(p) => p,
        Err(e) => {
            log::error!("AUDIO PLAYER | Error creating pipeline: {}", e.to_string());
            shutdown.notify_error(false).await;
            return;
        }
    };

    let pipeline_cpy = pipeline.clone();
    let shutdown_cpy = shutdown.clone();
    tokio::select! {
        _ = shutdown.wait_for_shutdown() => {
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

    //TODO: handle error
    source.set_callbacks(
        // Since our appsrc element operates in pull mode (it asks us to provide data),
        // we add a handler for the need-data callback and provide new data from there.
        // In our case, we told gstreamer that we do 2 frames per second. While the
        // buffers of all elements of the pipeline are still empty, this will be called
        // a couple of times until all of them are filled. After this initial period,
        // this handler will be called (on average) twice per second.
        gstreamer_app::AppSrcCallbacks::builder()
            .need_data(move |appsrc, _| {
                // appsrc already handles the error here

                let frame = rx_audio.recv().unwrap();

                let buffer = gstreamer::Buffer::from_slice(frame);

                appsrc.push_buffer(buffer).unwrap();
            })
            .build(),
    );
    Ok(pipeline)
}
