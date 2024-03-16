use gstreamer::{glib, prelude::*, Element, Pipeline};

use std::{
    collections::HashMap,
    io::{self, Error},
    sync::Arc,
};

use tokio::sync::mpsc::Sender;
use tokio::sync::Barrier;

use crate::utils::{
    gstreamer_utils::{pull_sample, read_bus},
    shutdown,
};

use super::video_const::{ENCODER_BITRATE, GSTREAMER_FRAMES, VIDEO_CAPTURE_PIPELINE_NAME};

/// Starts the video capturer by creating the pipeline and sending the video frames throug the provided Sender.
///
/// # Arguments
///
/// * `tx_video` - `A Sender<Vec<u8>>` used to send video frames.
/// * `shutdown` - Used for graceful shutdown.
/// * `barrier` - Used for synchronization.
pub async fn start_video_capture(
    tx_video: Sender<Vec<u8>>,
    shutdown: shutdown::Shutdown,
    barrier: Arc<Barrier>,
) {
    shutdown.add_task().await;

    // Initialize GStreamer
    if let Err(e) = gstreamer::init() {
        shutdown.notify_error(false).await;
        log::error!(
            "VIDEO CAPTURE | Failed to initialize gstreamer: {}",
            e.message()
        );
        return;
    };
    barrier.wait().await;

    println!("VIDEO CAPTURE | Barrier passed");
    // let mut hwnds: Vec<(HWND, String, String)> = Vec::new();
    // unsafe { EnumWindows(Some(enumerate_callback), &mut hwnds as *mut _ as LPARAM)};

    // for (count, element) in hwnds.iter().enumerate() {
    //     println!("[{}] HWND: {:?}, Class Name:  {}, Window Text: {}", count, element.0, element.1, element.2);
    // }

    //println!("Please enter a number:");

    // let mut input = String::new();

    // //Read input from the user
    // io::stdin().read_line(&mut input)
    //     .expect("Failed to read line");

    // // Parse the input string into an integer
    // let number: usize = match input.trim().parse() {
    //     Ok(num) => num,
    //     Err(_) => {
    //         println!("Invalid input, please enter a valid number.");
    //         return; // Exit the program or handle the error as appropriate
    //     }
    // };

    //let selected = hwnds.get(number);
    let window_handle = 0_u64;
    //println!("You selected: {}", selected.2);

    let new_framerate = gstreamer::Fraction::new(GSTREAMER_FRAMES, 1);
    let caps = gstreamer::Caps::builder("video/x-raw")
        .field("framerate", new_framerate)
        .build();

    let elements = match create_elements(window_handle) {
        Ok(e) => e,
        Err(e) => {
            shutdown.notify_error(false).await;
            log::error!(
                "VIDEO CAPTURE | Failed to create elements: {}",
                e.to_string()
            );
            return;
        }
    };

    let pipeline = match create_pipeline(elements, tx_video, caps) {
        Ok(p) => p,
        Err(e) => {
            shutdown.notify_error(false).await;
            log::error!(
                "VIDEO CAPTURE | Failed to create pipeline: {}",
                e.to_string()
            );
            return;
        }
    };

    // Start playing Payload
    if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
        shutdown.notify_error(false).await;
        log::error!(
            "VIDEO CAPTURE | Failed to set the pipeline to the `Playing` state: {}",
            e.to_string()
        );
        return;
    }

    let pipeline_cpy = pipeline.clone();
    let shutdown_cpy = shutdown.clone();
    tokio::select! {
        _ = shutdown.wait_for_error() => {
            log::debug!("VIDEO CAPTURE | ERROR NOTIFIED");
        },
        _ = tokio::spawn(async move {
            read_bus(pipeline_cpy, shutdown_cpy).await;
        }) => {
            log::debug!("VIDEO CAPTURE | BUS READ FINISHED");
        }
    }

    if let Err(e) = pipeline.set_state(gstreamer::State::Null) {
        log::error!(
            "VIDEO CAPTURE | Failed to set the pipeline to the `Null` state: {}",
            e.to_string()
        );
    }
}

/// Creates GStreamer elements required for the video capture pipeline.
///
/// # Returns
///  A Result containing:
/// * A `HashMap` of Gstreamer elements in case of success.
/// * A `glib::BoolError` in case of error
fn create_elements(window_handle: u64) -> Result<HashMap<&'static str, Element>, glib::BoolError> {
    let mut elements = HashMap::new();
    // Create the elements
    let d3d11screencapturesrc = gstreamer::ElementFactory::make("d3d11screencapturesrc")
        .name("d3d11screencapturesrc")
        .property("show-cursor", true)
        .property("window-handle", window_handle)
        .build()?;

    let videoconvert = gstreamer::ElementFactory::make("videoconvert")
        .name("videoconvert")
        .build()?;

    let m264enc = if let Ok(enc) = gstreamer::ElementFactory::make("amfh264enc")
        .name("amfh264enc")
        .property_from_str("usage", "ultra-low-latency")
        .property(
            "bitrate",
            <gstreamer::glib::Value as From<u32>>::from(ENCODER_BITRATE),
        )
        .build()
    {
        enc
    } else {
        gstreamer::ElementFactory::make("mfh264enc")
            .name("mfh264enc")
            .property("low-latency", true)
            .property("bitrate", <gstreamer::glib::Value as From<u32>>::from(3000))
            .build()?
    };

    let rtph264pay = gstreamer::ElementFactory::make("rtph264pay")
        .name("rtph264pay")
        .build()?;

    elements.insert("src", d3d11screencapturesrc);
    elements.insert("convert", videoconvert);
    elements.insert("enc", m264enc);
    elements.insert("pay", rtph264pay);

    Ok(elements)
}

/// Creates a GStreamer pipeline used for video capture.
///
/// # Arguments
///
/// * `tx_video` - A `Sender<Vec<u8>>` used to send audio frames.
/// * `elements` - A HashMap containing the GStreamer elements required for the pipeline.
/// * `caps` - The capabilities of the audio data to be captured.
///
/// # Returns
///  A Result containing the constructed GStreamer pipeline in case of success. Otherwise
/// error is returned.
fn create_pipeline(
    elements: HashMap<&str, Element>,
    tx_video: Sender<Vec<u8>>,
    caps: gstreamer::Caps,
) -> Result<Pipeline, Error> {
    let sink = gstreamer_app::AppSink::builder()
        .caps(&gstreamer::Caps::builder("application/x-rtp").build())
        .build();

    let pipeline = gstreamer::Pipeline::with_name(VIDEO_CAPTURE_PIPELINE_NAME);

    if let Err(e) = pipeline.add_many([
        &elements["src"],
        &elements["convert"],
        &elements["enc"],
        &elements["pay"],
        &sink.upcast_ref(),
    ]) {
        return Err(Error::new(io::ErrorKind::Other, e.to_string()));
    }

    if let Err(e) = elements["src"].link_filtered(&elements["convert"], &caps) {
        return Err(Error::new(io::ErrorKind::Other, e.to_string()));
    };

    if let Err(e) = gstreamer::Element::link_many([
        &elements["convert"],
        &elements["enc"],
        &elements["pay"],
        &sink.upcast_ref(),
    ]) {
        return Err(Error::new(io::ErrorKind::Other, e.to_string()));
    };

    sink.set_callbacks(
        gstreamer_app::AppSinkCallbacks::builder()
            .new_sample(
                move |appsink| match pull_sample(appsink, tx_video.clone()) {
                    Ok(_) => Ok(gstreamer::FlowSuccess::Ok),
                    Err(err) => {
                        log::error!("VIDEO CAPTURE | {}", err);
                        Err(gstreamer::FlowError::Error)
                    }
                },
            )
            .build(),
    );
    Ok(pipeline)
}
