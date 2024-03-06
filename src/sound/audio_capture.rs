use std::sync::mpsc::Sender;

use std::{io, thread::sleep, time::Duration};

use gstreamer::{element_error, prelude::*};

pub fn run(tx_audio: Sender<Vec<u8>>) {
    // Initialize GStreamer
    gstreamer::init().unwrap();

    sleep(Duration::from_secs(30));

    // Create the elements
    let wasapi2src = gstreamer::ElementFactory::make("wasapi2src")
        .name("wasapi2src")
        .build()
        .expect("Could not create wasapi2src element.");

    let audioconvert = gstreamer::ElementFactory::make("audioconvert")
        .name("audioconvert")
        .build()
        .expect("Could not create audioconvert element.");

    let audioresample = gstreamer::ElementFactory::make("audioresample")
        .name("audioresample")
        .build()
        .expect("Could not create audioresample element.");

    let opusenc = gstreamer::ElementFactory::make("opusenc")
        .name("opusenc")
        .build()
        .expect("Could not create opusenc element.");

    let rtpopuspay = gstreamer::ElementFactory::make("rtpopuspay")
        .name("rtpopuspay")
        .build()
        .expect("Could not create rtpopuspay element.");

    let sink = gstreamer_app::AppSink::builder()
        .caps(&gstreamer::Caps::builder("application/x-rtp").build())
        .build();

    let pipeline = gstreamer::Pipeline::with_name("pipeline_audio");

    pipeline
        .add_many([
            &wasapi2src,
            &audioconvert,
            &audioresample,
            &opusenc,
            &rtpopuspay,
            &sink.upcast_ref(),
        ])
        .unwrap();

    gstreamer::Element::link_many([
        &wasapi2src,
        &audioconvert,
        &audioresample,
        &opusenc,
        &rtpopuspay,
        &sink.upcast_ref(),
    ])
    .expect("Elements could not be linked.");

    // Otra opcion podria ser: pay (pad probe) fakesink
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
                        ("Failed to get buffer from appsink in audio pipeline")
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
                tx_audio
                    .send(samples.to_vec())
                    .expect("Error enviando sample de audio");

                Ok(gstreamer::FlowSuccess::Ok)
            })
            .build(),
    );

    // Start playing
    pipeline
        .set_state(gstreamer::State::Playing)
        .expect("Unable to set the pipeline audio to the `Playing` state");

    // Wait until error or EOS
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;

        match msg.view() {
            MessageView::Element(element) => {
                println!("{:?}", element);
            }
            MessageView::Error(err) => {
                eprintln!(
                    "Error received from element {:?} {}",
                    err.src().map(|s| s.path_string()),
                    err.error()
                );
                eprintln!("Debugging information: {:?}", err.debug());
                break;
            }
            MessageView::StateChanged(state_changed) => {
                if state_changed.src().map(|s| s == &pipeline).unwrap_or(false) {
                    println!(
                        "Pipeline state changed from {:?} to {:?}",
                        state_changed.old(),
                        state_changed.current()
                    );
                }
            }
            MessageView::Eos(..) => break,
            _ => (),
        }
    }

    pipeline
        .set_state(gstreamer::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}
