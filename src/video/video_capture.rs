use std::{io, sync::mpsc::Sender};
use gstreamer::{element_error,prelude::*};
use winapi::{ shared::{minwindef::{BOOL, LPARAM, TRUE}, windef::HWND}, um::winuser::{EnumWindows, GetClassNameW, GetWindowTextW, IsWindowEnabled, IsWindowVisible}};


unsafe extern "system" fn enumerate_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {

    let hwnds: &mut Vec<(HWND, String, String)> = &mut *(lparam as *mut Vec<(HWND, String, String)>);

    let mut class_name = [0u16; 256];
    let mut window_text = [0u16; 256];

    // Get class name of the window
    GetClassNameW(hwnd, class_name.as_mut_ptr(), 256);

    // Get window text
    GetWindowTextW(hwnd, window_text.as_mut_ptr(), 256);

    // Convert window text and class name to Rust strings
    let binding = String::from_utf16(&window_text).unwrap();
    let window_text_as_str = binding.trim_matches(char::from(0));
    let binding = String::from_utf16(&class_name).unwrap();
    let class_name_as_str = binding.trim_matches(char::from(0));

    
    //let a = &lparam as *mut Vec<isize>;
    if IsWindowVisible(hwnd) == TRUE && IsWindowEnabled(hwnd) == TRUE && !window_text_as_str.is_empty(){
        hwnds.push((hwnd, class_name_as_str.to_string(), window_text_as_str.to_string()));
    }
    
    TRUE
}


pub fn run(tx_video: Sender<Vec<u8>>) {
    // Initialize GStreamer
    gstreamer::init().unwrap();

    let mut hwnds: Vec<(HWND, String, String)> = Vec::new();
    unsafe { EnumWindows(Some(enumerate_callback), &mut hwnds as *mut _ as LPARAM)};

    // for (count, element) in hwnds.iter().enumerate() {        
    //     println!("[{}] HWND: {:?}, Class Name:  {}, Window Text: {}", count, element.0, element.1, element.2);
    // }
    

    //println!("Please enter a number:");

   // let mut input = String::new();

    // Read input from the user
    // io::stdin().read_line(&mut input)
    //     .expect("Failed to read line");

    // // Parse the input string into an integer
    // let number: usize = match input.trim().parse() {
    //     Ok(num) => num,
    //     Err(_) => {
    //         println!("Invalid input, please enter a valid number.");
    //         return; // Exit the program or handle the error as appropriate
    //     }
    // };

   // let selected = hwnds.get(number).unwrap();
   // let window_handle = selected.0 as u64;
   // println!("You selected: {}", selected.2);
    
    let window_handle = 0 as u64;

    // Create the elements
    let d3d11screencapturesrc = gstreamer::ElementFactory::make("d3d11screencapturesrc")
        .name("d3d11screencapturesrc")
        .property("show-cursor", true)
        .property("window-handle", window_handle)
        .build()
        .expect("Could not create d3d11screencapturesrc element.");

    let d3d11convert = gstreamer::ElementFactory::make("d3d11convert")
        .name("d3d11convert")
        .build()
        .expect("Could not create d3d11convert element.");

    let mfh264enc = gstreamer::ElementFactory::make("mfh264enc")
        .name("mfh264enc")
        .property("low-latency", true)
        .build()
        .expect("Could not create mfh264enc element.");

    
    let sink = gstreamer_app::AppSink::builder()
        // Tell the appsink what format we want.
        // This can be set after linking the two objects, because format negotiation between
        // both elements will happen during pre-rolling of the pipeline.
        .caps(
            &gstreamer_video::VideoCapsBuilder::for_encoding("video/x-h264").build(),
        )
        .build();

        
    /*let d3d11h264dec = gstreamer::ElementFactory::make("d3d11h264dec")
        .name("d3d11h264dec")
        .build()
        .expect("Could not create d3d11h264dec element.");

    let d3d11videosink = gstreamer::ElementFactory::make("d3d11videosink")
        .name("d3d11videosink")
        .build()
        .expect("Could not create d3d11videosink element.");*/


    // Create the empty pipeline
    let pipeline = gstreamer::Pipeline::with_name("pipeline");

    // Build the pipeline Note that we are NOT linking the source at this
    // point. We will do it later.
    pipeline.add_many([&d3d11screencapturesrc, &d3d11convert, &mfh264enc, /*&h264parse,*/ &sink.upcast_ref()/*, &d3d11h264dec, &d3d11videosink */] ).unwrap();

    gstreamer::Element::link_many([&d3d11screencapturesrc, &d3d11convert, &mfh264enc, /*&h264parse,*/ &sink.upcast_ref()/*, &d3d11h264dec, &d3d11videosink */])
        .expect("Elements could not be linked.");


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
                        ("Failed to get buffer from appsink")
                    );

                    gstreamer::FlowError::Error
                })?;

                // At this point, buffer is only a reference to an existing memory region somewhere.
                // When we want to access its content, we have to map it while requesting the required
                // mode of access (read, read/write).
                // This type of abstraction is necessary, because the buffer in question might not be
                // on the machine's main memory itself, but rather in the GPU's memory.
                // So mapping the buffer makes the underlying memory region accessible to us.
                // See: https://gstreamer.freedesktop.org/documentation/plugin-development/advanced/allocation.html
                let map = buffer.map_readable().map_err(|_| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to map buffer readable")
                    );

                    gstreamer::FlowError::Error
                })?;

                // We know what format the data in the memory region has, since we requested
                // it by setting the appsink's caps. So what we do here is interpret the
                // memory region we mapped as an array of signed 16 bit integers.
                let samples = map.as_slice();
                tx_video.send(samples.to_vec()).expect("Error enviando sample");
                

                Ok(gstreamer::FlowSuccess::Ok)
            })
            .build(),
    );

    
    // Start playing
    pipeline
        .set_state(gstreamer::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");


    
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

// fn main() {
//     run()
// }
