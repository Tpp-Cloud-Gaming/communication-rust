use std::{collections::HashMap, io::Error, sync::mpsc::Receiver};

use gstreamer::{glib, prelude::*, Caps, Element};
use winapi::um::winuser::ShowCursor;

use crate::utils::{
    gstreamer_utils::{push_sample, read_bus},
    shutdown,
};

use super::video_const::VIDEO_PLAYER_PIPELINE_NAME;


/// Creates the elements for the video player pipeline.
///
/// # Returns
///
/// A Result containing a HashMap with the elements if the operation was successful, otherwise an Error is returned.
pub fn create_elements() -> Result<HashMap<&'static str, Element>, Error> {
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


