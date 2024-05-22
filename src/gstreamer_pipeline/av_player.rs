use std::{collections::HashMap, io::Error, sync::mpsc::Receiver};

use gstreamer::{glib, prelude::*, Caps, Element};
use winapi::um::winuser::ShowCursor;

use crate::{sound::audio_capture, utils::{
    gstreamer_utils::{push_sample, read_bus},
    shutdown,
}, video::video_player};

pub const PIPELINE_NAME: &str = "AUDIO VIDEO PLAYER";


/// Starts the audio and video player by creating the pipeline and reading the video and audio frames from the provided Receiver.
///
/// # Arguments
///
/// * `rx_video` - A Receiver for receiving video frames.
/// * `rx_audio` - A Receiver for receiving audio frames.
/// * `shutdown` - A shutdown handle for managing the finalization of the thread.
pub async fn start_player(rx_video: Receiver<Vec<u8>>, rx_audio: Receiver<Vec<u8>>, shutdown:&mut shutdown::Shutdown) {
    shutdown.add_task("Start player").await;

    // Initialize GStreamer
    if let Err(e) = gstreamer::init() {
        shutdown.notify_error(false, "failed initialize gstreamer player").await;
        log::error!(
            "PLAYER | Failed to initialize gstreamer: {}",
            e.message()
        );
        return;
    };

    // Create the caps
    let video_caps = gstreamer::Caps::builder("application/x-rtp")
        .field("media", "video")
        .field("clock-rate", 90000)
        .field("encoding-name", "H264")
        .build();

    let video_elements = match video_player::create_elements() {
        Ok(e) => e,
        Err(e) => {
            shutdown.notify_error(false, "").await;
            log::error!("PLAYER | Failed to create video elements: {}", e);
            return;
        }
    };

    // Create the caps
    let audio_caps = gstreamer::Caps::builder("application/x-rtp")
        .field("media", "audio")
        .field("payload", 96)
        .field("clock-rate", 48000)
        .field("encoding-name", "OPUS")
        .build();

    let audio_elements = match audio_capture::create_elements() {
        Ok(e) => e,
        Err(e) => {
            log::error!("AUDIO PLAYER | Error creating elements: {}", e.message);
            shutdown.notify_error(false, "Error creating elements audio player").await;
            return;
        }
    };

    let pipeline = match create_pipeline(video_elements, audio_elements, video_caps, audio_caps, rx_video, rx_audio) {
        Ok(p) => p,
        Err(e) => {
            shutdown.notify_error(false, "").await;
            log::error!("PLAYER | Failed to create pipeline: {}", e);
            return;
        }
    };

    // Start playing Payload
    if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
        shutdown.notify_error(false, "failed set to playing audio video player").await;
        log::error!(
            "PLAYER | Failed to set the pipeline to the `Playing` state: {}",
            e.to_string()
        );
        return;
    }

    let pipeline_cpy = pipeline.clone();
    let shutdown_cpy = shutdown.clone();
    tokio::select! {
        _ = tokio::task::spawn(async move {
            read_bus(pipeline_cpy, shutdown_cpy).await;
        }) => {
            log::info!("PLAYER | Pipeline finished");
        },
        _ = shutdown.wait_for_error() => {
            log::info!("PLAYER | Shutdown received");
        },
    }

    if let Err(e) = pipeline.set_state(gstreamer::State::Null) {
        log::error!("PLAYER | Failed to set pipeline to null: {}", e);
    }
}



/// Creates the pipeline for the audio and video player.
///
/// # Arguments
///
/// * `video_elements` - A HashMap containing the video elements for the pipeline.
/// * `audio_elements` - A HashMap containing the audio elements for the pipeline.
/// * `video_caps` - The Video Caps for the pipeline.
/// * `audio_caps` - The Audio Caps for the pipeline.
/// * `rx_video` - A Receiver for receiving video frames.
/// * `rx_audio` - A Receiver for receiving audio frames.
///
/// # Returns
///
/// A Result containing the pipeline if the operation was successful, otherwise an Error is returned.
fn create_pipeline(
    video_elements: HashMap<&str, Element>,
    audio_elements: HashMap<&str, Element>,
    video_caps: Caps,
    audio_caps: Caps,
    rx_video: Receiver<Vec<u8>>,
    rx_audio: Receiver<Vec<u8>>,
) -> Result<gstreamer::Pipeline, Error> {
    let video_source = gstreamer_app::AppSrc::builder()
        .caps(&video_caps)
        .format(gstreamer::Format::Time)
        .is_live(true)
        .do_timestamp(true)
        .build();

    let audio_source = gstreamer_app::AppSrc::builder()
        .caps(&audio_caps)
        .format(gstreamer::Format::Time)
        .is_live(true)
        .do_timestamp(true)
        .build();

    // Create the empty pipeline
    let pipeline = gstreamer::Pipeline::with_name(PIPELINE_NAME);

    if let Err(e) = pipeline.add_many([
        video_source.upcast_ref(),
        &video_elements["depay"],
        &video_elements["parse"],
        &video_elements["dec"],
        &video_elements["sink"],
        audio_source.upcast_ref(),
        &audio_elements["depay"],
        &audio_elements["parse"],
        &audio_elements["dec"],
        &audio_elements["convert"],
        &audio_elements["sample"],
        &audio_elements["sink"],
    ]) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    if let Err(e) = gstreamer::Element::link_many([
        video_source.upcast_ref(),
        &video_elements["depay"],
        &video_elements["parse"],
        &video_elements["dec"],
        &video_elements["sink"],
    ]) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    if let Err(e) = gstreamer::Element::link_many([
        audio_source.upcast_ref(),
        &audio_elements["depay"],
        &audio_elements["parse"],
        &audio_elements["dec"],
        &audio_elements["convert"],
        &audio_elements["sample"],
        &audio_elements["sink"],
    ]) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    };

    video_source.set_callbacks(
        gstreamer_app::AppSrcCallbacks::builder()
            .need_data(move |appsrc, _| {
                push_sample(appsrc, &rx_video)
                    .map_err(|err| {
                        log::error!("VIDEO PLAYER | {}", err);
                    })
                    .unwrap();
            })
            .build(),
    );

    audio_source.set_callbacks(
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

    let videosink = &video_elements["sink"];
    videosink.connect_closure(
        "present",
        false,
        glib::closure!(move |_sink: &gstreamer::Element,
                             _device: &gstreamer::Object,
                             _rtv_raw: glib::Pointer| {
            unsafe {
                ShowCursor(0);
            }
        }),
    );

    Ok(pipeline)
}