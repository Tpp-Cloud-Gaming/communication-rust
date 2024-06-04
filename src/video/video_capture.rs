use gstreamer::{glib, Element};

use std::collections::HashMap;

use super::video_const::ENCODER_BITRATE;

/// Creates GStreamer elements required for the video capture pipeline.
///
/// # Returns
///  A Result containing:
/// * A `HashMap` of Gstreamer elements in case of success.
/// * A `glib::BoolError` in case of error
pub fn create_elements(
    window_handle: u64,
) -> Result<HashMap<&'static str, Element>, glib::BoolError> {
    let mut elements = HashMap::new();
    // Create the elements
    let d3d11screencapturesrc = gstreamer::ElementFactory::make("d3d11screencapturesrc")
        .name("d3d11screencapturesrc")
        .property("show-cursor", true)
        .property("window-handle", window_handle)
        .property_from_str("capture-api", "wgc")
        .property("adapter", 0)
        .build()?;

    let queue = gstreamer::ElementFactory::make("queue")
        .name("video_capture_queue")
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
        .property_from_str("aggregate-mode", "zero-latency") //SET WEBRTC MODE
        .build()?;

    elements.insert("src", d3d11screencapturesrc);
    elements.insert("queue", queue);
    elements.insert("convert", videoconvert);
    elements.insert("enc", m264enc);
    elements.insert("pay", rtph264pay);

    Ok(elements)
}
