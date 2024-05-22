use std::{
    collections::HashMap,
    io::{Error, ErrorKind},
    sync::mpsc::Receiver,
};

use gstreamer::{glib, prelude::*, Element, Pipeline};

use crate::utils::{
    gstreamer_utils::{push_sample, read_bus},
    shutdown,
};

use super::audio_const::AUDIO_PLAYER_PIPELINE_NAME;


/// Creates the elements for the audio player pipeline.
///
/// # Returns
///
/// A Result containing a HashMap with the elements if the operation was successful, otherwise an Error is returned.
fn create_elements() -> Result<HashMap<&'static str, Element>, glib::BoolError> {
    let mut elements = HashMap::new();

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

    elements.insert("depay", rtpopusdepay);
    elements.insert("parse", opusparse);
    elements.insert("dec", opusdec);
    elements.insert("convert", audioconvert);
    elements.insert("sample", audioresample);
    elements.insert("sink", autoaudiosink);

    Ok(elements)
}

