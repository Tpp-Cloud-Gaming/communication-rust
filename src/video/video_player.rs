use std::sync::mpsc::Receiver;

use gstreamer::prelude::*;

pub fn run(rx_video: Receiver<Vec<u8>>) {
    // Initialize GStreamer
    gstreamer::init().unwrap();


    let width = 1920;
    let height = 1080;

    // Create caps for H.264
    let caps = gstreamer::Caps::builder("video/x-h264")
        .field("width", &width)
        .field("height", &height)
        .field("stream-format", &"byte-stream") // Set stream-format to byte-stream for H.264
        .field("alignment", &"au") // Set alignment to au for H.264
        .field("profile", &"baseline") // Set profile to baseline for H.264
        .build();

    let source = gstreamer_app::AppSrc::builder()
        .caps(&caps)
        .format(gstreamer::Format::Time)
        .build();

    let d3d11h264dec = gstreamer::ElementFactory::make("d3d11h264dec")
        .name("d3d11h264dec")
        .build()
        .expect("Could not create d3d11h264dec element.");



    let d3d11videosink = gstreamer::ElementFactory::make("d3d11videosink")
        .name("d3d11videosink")
        .build()
        .expect("Could not create d3d11videosink element.");

    
    // Create the empty pipeline
    let pipeline = gstreamer::Pipeline::with_name("pipeline");

    pipeline.add_many([source.upcast_ref(), &d3d11h264dec, &d3d11videosink]).unwrap();
    gstreamer::Element::link_many([source.upcast_ref(), &d3d11h264dec, &d3d11videosink]).unwrap();

     // Start playing
    pipeline
    .set_state(gstreamer::State::Playing)
    .expect("Unable to set the pipeline to the `Playing` state");

    let mut i = 0;
    source.set_callbacks(
        // Since our appsrc element operates in pull mode (it asks us to provide data),
        // we add a handler for the need-data callback and provide new data from there.
        // In our case, we told gstreamer that we do 2 frames per second. While the
        // buffers of all elements of the pipeline are still empty, this will be called
        // a couple of times until all of them are filled. After this initial period,
        // this handler will be called (on average) twice per second.
        gstreamer_app::AppSrcCallbacks::builder()
            .need_data(move |appsrc, _| {
                // We only produce 100 frames
                i += 1;

                // appsrc already handles the error here
                let frame = rx_video.recv().unwrap();
                let buffer = gstreamer::Buffer::from_slice(frame);
                
                let _  = appsrc.push_buffer(buffer);

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