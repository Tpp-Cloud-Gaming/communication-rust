use std::collections::HashMap;

use gstreamer::{glib, Element};

/// Creates the elements for the audio player pipeline.
///
/// # Returns
///
/// A Result containing a HashMap with the elements if the operation was successful, otherwise an Error is returned.
pub fn create_elements() -> Result<HashMap<&'static str, Element>, glib::BoolError> {
    let mut elements = HashMap::new();

    let queue = gstreamer::ElementFactory::make("queue")
        .name("queue")
        .build()?;

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

    elements.insert("queue", queue);
    elements.insert("depay", rtpopusdepay);
    elements.insert("parse", opusparse);
    elements.insert("dec", opusdec);
    elements.insert("convert", audioconvert);
    elements.insert("sample", audioresample);
    elements.insert("sink", autoaudiosink);

    Ok(elements)
}
