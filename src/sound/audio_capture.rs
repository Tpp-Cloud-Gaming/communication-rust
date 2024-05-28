use std::collections::HashMap;

use gstreamer::{glib, Element};




/// Creates GStreamer elements required for audio capture pipeline.
///
/// # Returns
/// A Result containing:
/// * A `HashMap` of Gstreamer elements in case of success.
/// * A `glib::BoolError` in case of error
pub fn create_elements() -> Result<HashMap<&'static str, Element>, glib::BoolError> {
    let mut elements = HashMap::new();

    // Create the elements
    let wasapi2src = gstreamer::ElementFactory::make("wasapi2src")
        .name("wasapi2src")
        .property("loopback", true)
        .property("low-latency", true)
        .build()?;

    let queue = gstreamer::ElementFactory::make("queue")
        .name("audio_capture_queue")
        .build()?;

    let audioconvert = gstreamer::ElementFactory::make("audioconvert")
        .name("audioconvert")
        .build()?;

    let audioresample = gstreamer::ElementFactory::make("audioresample")
        .name("audioresample")
        .build()?;

    let opusenc = gstreamer::ElementFactory::make("opusenc")
        .name("opusenc")
        .build()?;

    let rtpopuspay = gstreamer::ElementFactory::make("rtpopuspay")
        .name("rtpopuspay")
        .build()?;

    elements.insert("src", wasapi2src);
    elements.insert("queue", queue);
    elements.insert("convert", audioconvert);
    elements.insert("sample", audioresample);
    elements.insert("enc", opusenc);
    elements.insert("pay", rtpopuspay);

    Ok(elements)
}