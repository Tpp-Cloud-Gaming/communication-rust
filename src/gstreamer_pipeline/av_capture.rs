use gstreamer::{prelude::*, Element, Pipeline};

use std::{
    collections::HashMap, io::{self, Error}, sync::Arc
};

use tokio::sync::mpsc::Sender;
use tokio::sync::Barrier;

use crate::{sound::audio_capture, utils::{
    gstreamer_utils::{pull_sample, read_bus},
    shutdown,
}, video::{video_capture, video_const::GSTREAMER_FRAMES}};

pub const PIPELINE_NAME: &str = "AUDIO VIDEO CAPTURE";


/// Creates a GStreamer pipeline used for video and audio capture.
///
/// # Arguments
///
/// * `tx_video` - A `Sender<Vec<u8>>` used to send audio frames.
/// * `tx_audio` - `A Sender<Vec<u8>>` used to send audio frames.
/// * `video_elements` - A HashMap containing the GStreamer video elements required for the pipeline.
/// * `audio_elements` - A HashMap containing the GStreamer video elements required for the pipeline.
/// * `video_caps` - The capabilities of the video data to be captured.
/// * `audio_caps` - The capabilities of the audio data to be captured.
///
/// # Returns
///  A Result containing the constructed GStreamer pipeline in case of success. Otherwise
/// error is returned.
fn create_pipeline(
    video_elements: HashMap<&str, Element>,
    audio_elements: HashMap<&str, Element>,
    tx_video: Sender<Vec<u8>>,
    tx_audio: Sender<Vec<u8>>,
    video_caps: gstreamer::Caps,
    audio_caps: gstreamer::Caps,
    shutdown: shutdown::Shutdown
) -> Result<Pipeline, Error> {
    let video_sink = gstreamer_app::AppSink::builder()
        .caps(&gstreamer::Caps::builder("application/x-rtp").build())
        .build();

    let audio_sink = gstreamer_app::AppSink::builder()
        .caps(&gstreamer::Caps::builder("application/x-rtp").build())
        .build();


    let pipeline = gstreamer::Pipeline::with_name(PIPELINE_NAME);

    if let Err(e) = pipeline.add_many([
        &video_elements["src"],
        &video_elements["queue"],
        &video_elements["convert"],
        &video_elements["enc"],
        &video_elements["pay"],
        &video_sink.upcast_ref(),
        &audio_elements["src"],
        &audio_elements["queue"],
        &audio_elements["convert"],
        &audio_elements["sample"],
        &audio_elements["enc"],
        &audio_elements["pay"],
        &audio_sink.upcast_ref(),
    ]) {
        return Err(Error::new(io::ErrorKind::Other, e.to_string()));
    }

    if let Err(e) = video_elements["src"].link_filtered(&video_elements["queue"], &video_caps) {
        return Err(Error::new(io::ErrorKind::Other, e.to_string()));
    };

    if let Err(e) = gstreamer::Element::link_many([
        &video_elements["queue"],
        &video_elements["convert"],
        &video_elements["enc"],
        &video_elements["pay"],
        &video_sink.upcast_ref(),
    ]) {
        return Err(Error::new(io::ErrorKind::Other, e.to_string()));
    };

    if let Err(e) = audio_elements["src"].link_filtered(&audio_elements["queue"], &audio_caps) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    if let Err(e) = gstreamer::Element::link_many([
        &audio_elements["queue"],
        &audio_elements["convert"],
        &audio_elements["sample"],
        &audio_elements["enc"],
        &audio_elements["pay"],
        &audio_sink.upcast_ref(),
    ]) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    let mut shutdown_clone = shutdown.clone();
    tokio::spawn(async move {
        shutdown_clone.add_task("Video pull sample").await;
        loop {
            if let Err(_e) = pull_sample(&video_sink, tx_video.clone()).await.map_err(|err| {
                log::error!("VIDEO CAPTURE | {}", err);
            }) {
                shutdown_clone
                    .notify_error(false, "failed pushing video sample")
                    .await;
                log::error!("RECEIVER | Failed pushing video sample");
                break;
            }
        }
        println!("salgo del loop de video PIBE");

    });
    // video_sink.set_callbacks(
    //     gstreamer_app::AppSinkCallbacks::builder()
    //         .new_sample(
    //             move |appsink| match pull_sample(appsink, tx_video.clone()) {
    //                 Ok(_) => Ok(gstreamer::FlowSuccess::Ok),
    //                 Err(err) => {
    //                     log::error!("VIDEO CAPTURE | {}", err);
    //                     let shutdown_cpy = shutdown.clone();
    //                     let _ = Box::pin(async move {    
    //                         shutdown_cpy.notify_error(false, "Video capture Set callbacks").await;
    //                         log::error!("SENDER | Notify error sended");
                            
    //                     });
    //                     Err(gstreamer::FlowError::Error)
    //                 }
    //             },
    //         )
    //         .build(),
    // );

    let mut shutdown_clone = shutdown.clone();
    tokio::spawn(async move {
        shutdown_clone.add_task("Audio pull sample").await;
        loop {
            if let Err(_e) = pull_sample(&audio_sink, tx_audio.clone()).await.map_err(|err| {
                log::error!("AUDIO CAPTURE | {}", err);
            }) {
                shutdown_clone
                    .notify_error(false, "failed pushing audio sample")
                    .await;
                log::error!("RECEIVER | Failed pushing audio sample");
                break;
            }
        }
        println!("salgo del loop de audio PIBE");
    });
    // audio_sink.set_callbacks(
    //     gstreamer_app::AppSinkCallbacks::builder()
    //         .new_sample(
    //             move |appsink| match pull_sample(appsink, tx_audio.clone()) {
    //                 Ok(_) => Ok(gstreamer::FlowSuccess::Ok),
    //                 Err(err) => {
    //                     log::error!("AUDIO CAPTURE | {}", err);
    //                     Err(gstreamer::FlowError::Error)
    //                 }
    //             },
    //         )
    //         .build(),
    // );


    Ok(pipeline)
}


pub async fn start_capture(
    tx_video: Sender<Vec<u8>>,
    tx_audio: Sender<Vec<u8>>,
    shutdown: &mut shutdown::Shutdown,
    barrier: Arc<Barrier>,
    game_id: u64,
) {
    shutdown.add_task("Capture").await;


    barrier.wait().await;

    println!("CAPTURE | Barrier passed");

    let new_framerate = gstreamer::Fraction::new(GSTREAMER_FRAMES, 1);
    let video_caps = gstreamer::Caps::builder("video/x-raw")
        .field("framerate", new_framerate)
        .build();

    let video_elements = match video_capture::create_elements(game_id) {
        Ok(e) => e,
        Err(e) => {
            log::error!(
                "CAPTURE | Failed to create video elements: {}",
                e.to_string()
            );
            shutdown.notify_error(false, "create elements video capture").await;
            return;
        }
    };

    let audio_caps = gstreamer::Caps::builder("audio/x-raw")
    //.field("rate", 48000)
    .field("channels", 2)
    .build();

    let audio_elements = match audio_capture::create_elements() {
        Ok(e) => e,
        Err(e) => {
            log::error!("CAPTURE | Error creating  audio elements: {}", e.message);
            shutdown.notify_error(false, "Create elements audio capture").await;
            return;
        }
    };

    let pipeline = match create_pipeline(video_elements, audio_elements, tx_video, tx_audio, video_caps, audio_caps, shutdown.clone()) {
        Ok(p) => p,
        Err(e) => {
            shutdown.notify_error(false,"crate pipeline video capture").await;
            log::error!(
                "CAPTURE | Failed to create pipeline: {}",
                e.to_string()
            );
            return;
        }
    };

    // Start playing Payload
    if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
        shutdown.notify_error(false, "failed set to playing audio video capture").await;
        log::error!(
            "CAPTURE | Failed to set the pipeline to the `Playing` state: {}",
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

    // tokio::select! {
    //     _ = shutdown.wait_for_error() => {
    //         log::error!("CAPTURE | ERROR NOTIFIED");
    //     },
    //     _ = tokio::spawn(async move {
    //         read_bus(pipeline_cpy, shutdown_cpy).await;
    //     }) => {
    //         log::debug!("CAPTURE | BUS READ FINISHED");
    //     }
    // }
    // log::error!("CAPTURE | About to set null state");
    // if let Err(e) = pipeline.set_state(gstreamer::State::Null) {
    //     log::error!(
    //         "CAPTURE | Failed to set the pipeline to the `Null` state: {}",
    //         e.to_string()
    //     );
    // }
}


