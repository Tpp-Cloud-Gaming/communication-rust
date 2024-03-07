use gstreamer::{element_error, glib, prelude::*, Bus, Element, Pipeline};
use std::{
    collections::HashMap,
    io::{self, Error},
    sync::mpsc::Sender,
    thread::sleep,
    time::Duration,
};
use winapi::{
    shared::{
        minwindef::{BOOL, LPARAM, TRUE},
        windef::HWND,
    },
    um::winuser::{EnumWindows, GetClassNameW, GetWindowTextW, IsWindowEnabled, IsWindowVisible},
};

use crate::utils::shutdown::{self, Shutdown};

use super::video_const::{GSTREAMER_FRAMES, GSTREAMER_INITIAL_SLEEP};

unsafe extern "system" fn enumerate_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let hwnds: &mut Vec<(HWND, String, String)> =
        &mut *(lparam as *mut Vec<(HWND, String, String)>);

    let mut class_name = [0u16; 256];
    let mut window_text = [0u16; 256];

    // Get class name of the window
    GetClassNameW(hwnd, class_name.as_mut_ptr(), 256);

    // Get window text
    GetWindowTextW(hwnd, window_text.as_mut_ptr(), 256);

    // Convert window text and class name to Rust strings
    let binding = String::from_utf16(&window_text).unwrap();
    let window_text_as_str = binding.trim_matches(char::from(0));
    let binding = String::from_utf16(&class_name).unwrap();
    let class_name_as_str = binding.trim_matches(char::from(0));

    //let a = &lparam as *mut Vec<isize>;
    if IsWindowVisible(hwnd) == TRUE
        && IsWindowEnabled(hwnd) == TRUE
        && !window_text_as_str.is_empty()
    {
        hwnds.push((
            hwnd,
            class_name_as_str.to_string(),
            window_text_as_str.to_string(),
        ));
    }

    TRUE
}

pub async fn start_video_capture(tx_video: Sender<Vec<u8>>, shutdown: shutdown::Shutdown) {
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
    sleep(Duration::from_secs(GSTREAMER_INITIAL_SLEEP));

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

    //let selected = hwnds.get(number).unwrap();
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
        return;
    }
}

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

    let mfh264enc = gstreamer::ElementFactory::make("amfh264enc")
        .name("amfh264enc")
        .property_from_str("usage", "ultra-low-latency")
        .property(
            "bitrate",
            <gstreamer::glib::Value as From<u32>>::from(10000),
        )
        .build()?;
    // let mfh264enc = gstreamer::ElementFactory::make("mfh264enc")
    //     .name("mfh264enc")
    //     .property("low-latency", true)
    //     .build()
    //     .expect("Could not create mfh264enc element.");

    let rtph264pay = gstreamer::ElementFactory::make("rtph264pay")
        .name("rtph264pay")
        .build()?;

    elements.insert("src", d3d11screencapturesrc);
    elements.insert("convert", videoconvert);
    elements.insert("enc", mfh264enc);
    elements.insert("pay", rtph264pay);

    return Ok(elements);
}

fn create_pipeline(
    elements: HashMap<&str, Element>,
    tx_video: Sender<Vec<u8>>,
    caps: gstreamer::Caps,
) -> Result<Pipeline, Error> {
    let sink = gstreamer_app::AppSink::builder()
        // Tell the appsink what format we want.
        // This can be set after linking the two objects, because format negotiation between
        // both elements will happen during pre-rolling of the pipeline.
        .caps(&gstreamer::Caps::builder("application/x-rtp").build())
        .build();

    // Create the empty pipeline
    let pipeline = gstreamer::Pipeline::with_name("pipeline");

    // Build the pipeline Note that we are NOT linking the source at this
    // point. We will do it later.
    if let Err(e) = pipeline.add_many([
        &elements["src"],
        &elements["convert"],
        &elements["enc"],
        &elements["pay"],
        /*&h264parse,*/ &sink.upcast_ref(), /*, &d3d11h264dec, &d3d11videosink */
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
        /*&h264parse,*/ &sink.upcast_ref(), /*, &d3d11h264dec, &d3d11videosink */
    ]) {
        return Err(Error::new(io::ErrorKind::Other, e.to_string()));
    };

    //TODO: modularizar y handlear errores
    sink.set_callbacks(
        gstreamer_app::AppSinkCallbacks::builder()
            // Add a handler to the "new-sample" signal.
            .new_sample(move |appsink| {
                // Pull the sample in question out of the appsink's buffer.
                let sample = appsink.pull_sample().unwrap();

                let buffer = sample.buffer().ok_or_else(|| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to get buffer from appsink")
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
                tx_video
                    .send(samples.to_vec())
                    .expect("Error enviando sample");

                Ok(gstreamer::FlowSuccess::Ok)
            })
            .build(),
    );
    return Ok(pipeline);
}

async fn read_bus(pipeline: Pipeline, shutdown: shutdown::Shutdown) {
    // Wait until error or EOS
    let bus = match pipeline.bus() {
        Some(b) => b,
        None => {
            shutdown.notify_error(false).await;
            log::error!("VIDEO CAPTURE | Pipeline bus not found");
            return;
        }
    };

    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;

        match msg.view() {
            MessageView::Element(element) => {
                log::debug!("Element message received: {:?}", element);
                continue;
            }
            MessageView::Error(err) => {
                log::error!(
                    "Error received from element {:?} {}",
                    err.src().map(|s| s.path_string()),
                    err.error()
                );
                shutdown.notify_error(false).await;
                break;
            }
            MessageView::StateChanged(state_changed) => {
                if state_changed.src().map(|s| s == &pipeline).unwrap_or(false) {
                    log::debug!(
                        "Pipeline state changed from {:?} to {:?}",
                        state_changed.old(),
                        state_changed.current()
                    );
                }
            }
            MessageView::Eos(..) => {
                shutdown.notify_error(false).await;
                break;
            }
            _ => (),
        }
    }
}
