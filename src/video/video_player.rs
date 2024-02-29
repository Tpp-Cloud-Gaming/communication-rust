use std::sync::mpsc::Receiver;

use gstreamer::prelude::*;

pub fn run(rx_video: Receiver<Vec<u8>>) {
    // Initialize GStreamer
    gstreamer::init().unwrap();
    
    // Create the caps
    let caps = gstreamer::Caps::builder("application/x-rtp")
        .field("media", "video")
        .field("clock-rate", 90000)
        .field("encoding-name", "H264")
        .build();
    

    let source = gstreamer_app::AppSrc::builder()
        .caps(&caps)
        .format(gstreamer::Format::Time)
        .is_live(true)
        .do_timestamp(true)
        .build();

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
        .build()
        .expect("Could not create d3d11videosink element.");

    
    // Create the empty pipeline
    let pipeline = gstreamer::Pipeline::with_name("pipeline");

    pipeline.add_many([source.upcast_ref(), &rtph264depay,&h264parse,  &d3d11h264dec, &d3d11videosink]).unwrap();
    gstreamer::Element::link_many([source.upcast_ref(), &rtph264depay,&h264parse, &d3d11h264dec, &d3d11videosink]).unwrap();

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
                
                let frame = rx_video.recv().unwrap();

                //println!("{:?}", appsrc.current_level_bytes());
                // println!("APPSRC: {:?}", frame );
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