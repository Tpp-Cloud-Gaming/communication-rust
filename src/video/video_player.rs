use std::collections::HashMap;

use gstreamer::{glib, Element};

/// Creates the elements for the video player pipeline.
///
/// # Returns
///
/// A Result containing a HashMap with the elements if the operation was successful, otherwise an Error is returned.
pub fn create_elements() -> Result<HashMap<&'static str, Element>, glib::BoolError> {
    let mut elements = HashMap::new();

    let rtph264depay = gstreamer::ElementFactory::make("rtph264depay")
        .name("rtph264depay")
        .build()?;

    let h264parse = gstreamer::ElementFactory::make("h264parse")
        .name("h264parse")
        .build()?;

    let d3d11h264dec = gstreamer::ElementFactory::make("d3d11h264dec")
        .name("d3d11h264dec")
        .build()?;

    let queue = gstreamer::ElementFactory::make("queue")
        .name("video_player_queue")
        .build()?;

    let taginject = gstreamer::ElementFactory::make("taginject")
        .name("taginject")
        .property("tags", "title=Cloud-Gaming-Rental-Service")
        .build()
        .expect("Could not create d3d11videosink element.");

    let d3d11videosink = gstreamer::ElementFactory::make("d3d11videosink")
        .name("d3d11videosink")
        .property("emit-present", true)
        .property("fullscreen", true)
        .property_from_str("fullscreen-toggle-mode", "property")
        .build()?;

    elements.insert("depay", rtph264depay);
    elements.insert("parse", h264parse);
    elements.insert("dec", d3d11h264dec);
    elements.insert("queue", queue);
    elements.insert("taginject", taginject);
    elements.insert("sink", d3d11videosink);

    Ok(elements)
}
