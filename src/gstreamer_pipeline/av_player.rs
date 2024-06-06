use std::{
    collections::HashMap,
    io::Error,
    sync::{mpsc::Receiver, Arc},
};

use gstreamer::{glib, prelude::*, Caps, Element};
use tokio::sync::Barrier;
use winapi::um::winuser::ShowCursor;

use crate::{
    sound::audio_player,
    utils::{
        gstreamer_utils::{push_sample, read_bus},
        shutdown,
    },
    video::video_player,
};

pub const PIPELINE_NAME: &str = "AUDIO VIDEO PLAYER";

/// Starts the audio and video player by creating the pipeline and reading the video and audio frames from the provided Receiver.
///
/// # Arguments
///
/// * `rx_video` - A Receiver for receiving video frames.
/// * `rx_audio` - A Receiver for receiving audio frames.
/// * `shutdown` - A shutdown handle for managing the finalization of the thread.
pub async fn start_player(
    rx_video: Receiver<(bool, Vec<u8>)>,
    rx_audio: Receiver<(bool, Vec<u8>)>,
    shutdown: &mut shutdown::Shutdown,
    barrier: Arc<Barrier>,
) {
    barrier.wait().await;
    shutdown.add_task("Start player").await;

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

    let audio_elements = match audio_player::create_elements() {
        Ok(e) => e,
        Err(e) => {
            log::error!("AUDIO PLAYER | Error creating elements: {}", e.message);
            shutdown
                .notify_error(false, "Error creating elements audio player")
                .await;
            return;
        }
    };

    let pipeline = match create_pipeline(
        video_elements,
        audio_elements,
        video_caps,
        audio_caps,
        rx_video,
        rx_audio,
        shutdown.clone(),
    ) {
        Ok(p) => p,
        Err(e) => {
            shutdown.notify_error(false, "").await;
            log::error!("PLAYER | Failed to create pipeline: {}", e);
            return;
        }
    };

    // Start playing Payload
    if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
        shutdown
            .notify_error(false, "failed set to playing audio video player")
            .await;
        log::error!(
            "PLAYER | Failed to set the pipeline to the `Playing` state: {}",
            e.to_string()
        );
        return;
    }

    let pipeline_cpy = pipeline.clone();
    let mut shutdown_cpy = shutdown.clone();

    let handle_read_bus = tokio::task::spawn(async move {
        read_bus(pipeline_cpy, &mut shutdown_cpy).await;
    });

    tokio::select! {
        _ = shutdown.wait_for_error() => {
            log::info!("PLAYER | Shutdown received");
        },
    }

    if let Err(e) = pipeline.set_state(gstreamer::State::Null) {
        log::error!("PLAYER | Failed to set pipeline to null: {}", e);
    } else {
        println!("SE CAMBIA EL ESTADO A NULL");
    }

    handle_read_bus.abort();
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
    rx_video: Receiver<(bool, Vec<u8>)>,
    rx_audio: Receiver<(bool, Vec<u8>)>,
    shutdown: shutdown::Shutdown,
) -> Result<gstreamer::Pipeline, Error> {
    let video_source = gstreamer_app::AppSrc::builder()
        .caps(&video_caps)
        .block(true)
        .format(gstreamer::Format::Time)
        .is_live(true)
        .do_timestamp(true)
        .build();

    let audio_source = gstreamer_app::AppSrc::builder()
        .caps(&audio_caps)
        .block(true)
        .format(gstreamer::Format::Time)
        .is_live(true)
        .do_timestamp(true)
        .build();

    // Create the empty pipeline
    let pipeline = gstreamer::Pipeline::with_name(PIPELINE_NAME);

    if let Err(e) = pipeline.add_many([
        video_source.upcast_ref(),
        &video_elements["jitter"],
        &video_elements["depay"],
        &video_elements["parse"],
        &video_elements["dec"],
        &video_elements["queue"],
        &video_elements["sink"],
        audio_source.upcast_ref(),
        &audio_elements["queue"],
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
        &video_elements["jitter"],
        &video_elements["depay"],
        &video_elements["parse"],
        &video_elements["dec"],
        &video_elements["queue"],
        &video_elements["sink"],
    ]) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    if let Err(e) = gstreamer::Element::link_many([
        audio_source.upcast_ref(),
        &audio_elements["queue"],
        &audio_elements["depay"],
        &audio_elements["parse"],
        &audio_elements["dec"],
        &audio_elements["convert"],
        &audio_elements["sample"],
        &audio_elements["sink"],
    ]) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    };

    let mut shutdown_clone = shutdown.clone();
    tokio::spawn(async move {
        shutdown_clone.add_task("Video push sample").await;
        loop {
            if let Err(_e) = push_sample(&video_source, &rx_video).map_err(|err| {
                log::error!("VIDEO PLAYER | {}", err);
            }) {
                shutdown_clone
                    .notify_error(false, "failed pushing video sample")
                    .await;
                log::error!("RECEIVER | Failed pushing video sample");
                break;
            }
        }
    });

    let mut shutdown_cpy = shutdown.clone();
    tokio::spawn(async move {
        shutdown_cpy.add_task("Audio push sample").await;
        loop {
            if let Err(_e) = push_sample(&audio_source, &rx_audio).map_err(|err| {
                log::error!("AUDIO PLAYER | {}", err);
            }) {
                shutdown_cpy
                    .notify_error(false, "failed pushing audio sample")
                    .await;
                log::error!("RECEIVER | Failed pushing audio sample");
                break;
            }
        }
    });

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
