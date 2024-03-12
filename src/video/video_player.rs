use std::{collections::HashMap, io::Error, sync::mpsc::Receiver};

use gstreamer::{glib, prelude::*, Caps, Element};
use winapi::um::winuser::ShowCursor;

use crate::utils::{gstreamer_utils::read_bus, shutdown};

use super::video_const::VIDEO_PLAYER_PIPELINE_NAME;

pub async fn start_video_player(rx_video: Receiver<Vec<u8>>, shutdown: shutdown::Shutdown) {
    shutdown.add_task().await;

    // Initialize GStreamer
    if let Err(e) = gstreamer::init() {
        shutdown.notify_error(false).await;
        log::error!(
            "VIDEO PLAYER | Failed to initialize gstreamer: {}",
            e.message()
        );
        return;
    };

    // Create the caps
    let caps = gstreamer::Caps::builder("application/x-rtp")
        .field("media", "video")
        .field("clock-rate", 90000)
        .field("encoding-name", "H264")
        .build();

    let elements = match create_elements() {
        Ok(e) => e,
        Err(e) => {
            shutdown.notify_error(false).await;
            log::error!("VIDEO PLAYER | Failed to create elements: {}", e);
            return;
        }
    };

    let pipeline = match create_pipeline(elements, caps, rx_video) {
        Ok(p) => p,
        Err(e) => {
            shutdown.notify_error(false).await;
            log::error!("VIDEO PLAYER | Failed to create pipeline: {}", e);
            return;
        }
    };

    let pipeline_cpy = pipeline.clone();
    let shutdown_cpy = shutdown.clone();
    tokio::select! {
        _ = tokio::task::spawn(async move {
            read_bus(pipeline_cpy, shutdown_cpy).await;
        }) => {
            log::info!("VIDEO PLAYER | Pipeline finished");
        },
        _ = shutdown.wait_for_shutdown() => {
            log::info!("VIDEO PLAYER | Shutdown received");
        },
    }

    if let Err(e) = pipeline.set_state(gstreamer::State::Null) {
        log::error!("VIDEO PLAYER | Failed to set pipeline to null: {}", e);
    }
}

fn create_elements() -> Result<HashMap<&'static str, Element>, Error> {
    let mut elements = HashMap::new();

    let rtph264depay = gstreamer::ElementFactory::make("rtph264depay")
        .name("rtph264depay")
        .build()
        .expect("Could not create rtph264depay element.");

    let h264parse = gstreamer::ElementFactory::make("h264parse")
        .name("h264parse")
        .build()
        .expect("Could not create rtph264depay element.");

    let d3d11h264dec = gstreamer::ElementFactory::make("d3d11h264dec")
        .name("d3d11h264dec")
        .build()
        .expect("Could not create d3d11h264dec element.");

    let d3d11videosink = gstreamer::ElementFactory::make("d3d11videosink")
        .name("d3d11videosink")
        .property("emit-present", true)
        .property("fullscreen", true)
        .property_from_str("fullscreen-toggle-mode", "property")
        .build()
        .expect("Could not create d3d11videosink element.");

    elements.insert("depay", rtph264depay);
    elements.insert("parse", h264parse);
    elements.insert("dec", d3d11h264dec);
    elements.insert("sink", d3d11videosink);

    Ok(elements)
}

fn create_pipeline(
    elements: HashMap<&str, Element>,
    caps: Caps,
    rx_video: Receiver<Vec<u8>>,
) -> Result<gstreamer::Pipeline, Error> {
    let source = gstreamer_app::AppSrc::builder()
        .caps(&caps)
        .format(gstreamer::Format::Time)
        .is_live(true)
        .do_timestamp(true)
        .build();

    // Create the empty pipeline
    let pipeline = gstreamer::Pipeline::with_name(VIDEO_PLAYER_PIPELINE_NAME);

    if let Err(e) = pipeline.add_many([
        source.upcast_ref(),
        &elements["depay"],
        &elements["parse"],
        &elements["dec"],
        &elements["sink"],
    ]) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    if let Err(e) = gstreamer::Element::link_many([
        source.upcast_ref(),
        &elements["depay"],
        &elements["parse"],
        &elements["dec"],
        &elements["sink"],
    ]) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    // Start playing
    if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
        return Err(Error::new(std::io::ErrorKind::Other, e.to_string()));
    }

    source.set_callbacks(
        gstreamer_app::AppSrcCallbacks::builder()
            .need_data(move |appsrc, _| {
                let frame = rx_video.recv().unwrap();

                let buffer = gstreamer::Buffer::from_slice(frame);

                appsrc.push_buffer(buffer).unwrap();
            })
            .build(),
    );

    let videosink = &elements["sink"];
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

