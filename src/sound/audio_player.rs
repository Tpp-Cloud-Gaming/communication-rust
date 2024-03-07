use std::sync::mpsc::Receiver;

use gstreamer::prelude::*;

pub fn run(rx_audio: Receiver<Vec<u8>>) {
    // Initialize GStreamer
    gstreamer::init().unwrap();

    // Create the caps
    let caps = gstreamer::Caps::builder("application/x-rtp")
        .field("media", "audio")
        .field("payload", 96)
        .field("clock-rate", 48000)
        .field("encoding-name", "OPUS")
        .build();


    let source = gstreamer_app::AppSrc::builder()
        .caps(&caps)
        .format(gstreamer::Format::Time)
        .is_live(true)
        .do_timestamp(true)
        .build();


    let rtpopusdepay = gstreamer::ElementFactory::make("rtpopusdepay")
        .name("rtpopusdepay")
        .build()
        .expect("Could not create rtpopusdepay element.");

    let opusparse = gstreamer::ElementFactory::make("opusparse")
        .name("opusparse")
        .build()
        .expect("Could not create rtph264depay element.");

    let opusdec = gstreamer::ElementFactory::make("opusdec")
        .name("opusdec")
        .build()
        .expect("Could not create opusdec element.");

    let audioconvert = gstreamer::ElementFactory::make("audioconvert")
        .name("audioconvert")
        .build()
        .expect("Could not create audioconvert element.");

    let audioresample = gstreamer::ElementFactory::make("audioresample")
        .name("audioresample")
        .build()
        .expect("Could not create audioresample element.");

    let autoaudiosink = gstreamer::ElementFactory::make("autoaudiosink")
        .name("autoaudiosink")
        .build()
        .expect("Could not create audioresample element.");

    // Create the empty pipeline
    let pipeline = gstreamer::Pipeline::with_name("pipeline");

    pipeline
        .add_many([
            source.upcast_ref(),
            &rtpopusdepay,
            &opusparse,
            &opusdec,
            &audioconvert,
            &audioresample,
            &autoaudiosink,
        ])
        .unwrap();
    gstreamer::Element::link_many([
        source.upcast_ref(),
        &rtpopusdepay,
        &opusparse,
        &opusdec,
        &audioconvert,
        &audioresample,
        &autoaudiosink,
    ])
    .unwrap();

    // Start playing
    pipeline
        .set_state(gstreamer::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    source.set_callbacks(
        // Since our appsrc element operates in pull mode (it asks us to provide data),
        // we add a handler for the need-data callback and provide new data from there.
        // In our case, we told gstreamer that we do 2 frames per second. While the
        // buffers of all elements of the pipeline are still empty, this will be called
        // a couple of times until all of them are filled. After this initial period,
        // this handler will be called (on average) twice per second.
        gstreamer_app::AppSrcCallbacks::builder()
            .need_data(move |appsrc, _| {
                // appsrc already handles the error here

                let frame = rx_audio.recv().unwrap();
                
                let buffer = gstreamer::Buffer::from_slice(frame);

                appsrc.push_buffer(buffer).unwrap();
            })
            .build(),
    );

    // Wait until error or EOS
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;

        match msg.view() {
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

