//use cpal::{traits::{HostTrait, DeviceTrait}, Sample, SampleFormat, Stream, FromSample, Device};
//use hound::WavWriter;
use opus::Decoder;
use std::io::Error;
//use tokio::sync::mpsc::Receiver;


pub struct AudioDecoder{
    decoder: Decoder,
    //writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
}

// fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
//     if format.is_float() {
//         hound::SampleFormat::Float
//     } else {
//         hound::SampleFormat::Int
//     }
// }

// fn wav_spec_from_config(config: &cpal::SupportedStreamConfig) -> hound::WavSpec {
//     hound::WavSpec {
//         channels: config.channels() as _,
//         sample_rate: config.sample_rate().0 as _,
//         bits_per_sample: (config.sample_format().sample_size() * 8) as _,
//         sample_format: sample_format(config.sample_format()),
//     }
// }

impl AudioDecoder {
    pub fn new() -> Result<Self, Error> {
        let sample_rate: u32 = std::env::var("SAMPLE_RATE")
            .expect("SAMPLE_RATE env not found.")
            .parse()
            .expect("Failed to parse SAMPLE_RATE as u32");
        let decoder = opus::Decoder::new(sample_rate, opus::Channels::Stereo).unwrap();
        Ok(Self { decoder: decoder})
        // let host = cpal::default_host();
        // let device = host.default_output_device().unwrap();
        // let config = device.default_output_config().unwrap();
        // let spec = wav_spec_from_config(&config);
        // let writer = hound::WavWriter::create(path, spec).unwrap();
        // let writer = Arc::new(Mutex::new(Some(writer)));
    }
    pub fn decode(&mut self, input: Vec<u8>) -> Result<Vec<f32>, Error> {
        let mut data = vec![0.0; 960];

        match self.decoder.decode_float(&input, &mut data[..], false) {
            Ok(_) => {}
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
        Ok(data)
        
    }
   // pub fn start(&self, rx: Arc<Mutex<Receiver<f32>>>) -> Result<Stream, Error> {

        // let device = search_device("M2380A (NVIDIA High Definition Audio)".to_owned()).unwrap();
        // let config = device.default_output_config().unwrap();
        

        // let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
        // let sample_format = config.sample_format();
        // let config: cpal::StreamConfig = config.into();
        // let channels = config.channels as usize;
        

        // let stream = match sample_format {
        //     SampleFormat::F32 => device.build_output_stream(&config, move |data: &mut [f32], _: &_|  {
        //         write_data(data, channels, rx.clone())
        //     }, err_fn, None),
        //     SampleFormat::I16 => device.build_output_stream(&config,  move |data: &mut [f32], _: &_| {
        //         write_data(data, channels, rx.clone())
        //     }, err_fn, None),
        //     SampleFormat::U16 => device.build_output_stream(&config, move |data: &mut [f32], _: &_| {
        //         write_data(data, channels, rx.clone())
        //     }, err_fn, None),
        //     sample_format => panic!("Unsupported sample format '{sample_format}'")
        // }.unwrap();
        
        // Ok(stream)
  //  }
    

}

// fn write_data(output: &mut [f32], channels: usize, rx: Arc<Mutex<Receiver<f32>>>)
// {
    
//     for sample in output {
        
//         let data = match rx.lock().unwrap().try_recv() {
//             Ok(sample) => sample,
//             Err(_) => 0.0,
//         };

//         *sample =  data;
//     }

// }
 

