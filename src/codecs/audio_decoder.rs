
use cpal::{traits::{HostTrait, DeviceTrait}, Sample, SampleFormat, Stream, FromSample, Device};
use hound::WavWriter;
use opus::Decoder;
use std::{io::{Error, BufWriter}, sync::{Mutex, Arc}, fs::File};
use tokio::sync::mpsc::Receiver;


pub struct AudioDecoder{
    decoder: Decoder,
    writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
}

fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
    if format.is_float() {
        hound::SampleFormat::Float
    } else {
        hound::SampleFormat::Int
    }
}

fn wav_spec_from_config(config: &cpal::SupportedStreamConfig) -> hound::WavSpec {
    hound::WavSpec {
        channels: config.channels() as _,
        sample_rate: config.sample_rate().0 as _,
        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
        sample_format: sample_format(config.sample_format()),
    }
}

impl AudioDecoder {
    pub fn new(path: &str) -> Result<Self, Error> {
        let sample_rate: u32 = std::env::var("SAMPLE_RATE")
            .expect("SAMPLE_RATE env not found.")
            .parse()
            .expect("Failed to parse SAMPLE_RATE as u32");
        let decoder = opus::Decoder::new(sample_rate, opus::Channels::Stereo).unwrap();

        let host = cpal::default_host();
        let device = host.default_output_device().unwrap();
        let config = device.default_output_config().unwrap();


        let spec = wav_spec_from_config(&config);
        let writer = hound::WavWriter::create(path, spec).unwrap();
        let writer = Arc::new(Mutex::new(Some(writer)));

        Ok(Self { decoder: decoder, writer: writer })
    }

    pub fn start(&self, rx: Arc<Mutex<Receiver<f32>>>) -> Result<Stream, Error> {

        let device = search_device("M2380A (NVIDIA High Definition Audio)".to_owned()).unwrap();
        let config = device.default_output_config().unwrap();
        

        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
        let sample_format = config.sample_format();
        let config: cpal::StreamConfig = config.into();
        let channels = config.channels as usize;
        

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(&config, move |data: &mut [f32], _: &_|  {
                write_data(data, channels, rx.clone())
            }, err_fn, None),
            SampleFormat::I16 => device.build_output_stream(&config,  move |data: &mut [f32], _: &_| {
                write_data(data, channels, rx.clone())
            }, err_fn, None),
            SampleFormat::U16 => device.build_output_stream(&config, move |data: &mut [f32], _: &_| {
                write_data(data, channels, rx.clone())
            }, err_fn, None),
            sample_format => panic!("Unsupported sample format '{sample_format}'")
        }.unwrap();
        
        Ok(stream)
    }
    
    pub fn decode(&mut self, input: Vec<u8>) -> Result<Vec<f32>, Error> {
        let mut data = vec![0.0; 960];

        match self.decoder.decode_float(&input, &mut data[..], false) {
            Ok(_) => {}
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }

        if let Ok(mut guard) = self.writer.try_lock() {
            if let Some(writer) = guard.as_mut() {
                for &sample in data.iter() {
                    //let sample: U = U::from_sample(sample);
                    writer.write_sample(sample).ok();
                }
            }
        }

        Ok(data)
    }
}

fn write_data(output: &mut [f32], channels: usize, rx: Arc<Mutex<Receiver<f32>>>)
{
    
    for sample in output {
        
        let data = match rx.lock().unwrap().try_recv() {
            Ok(sample) => sample,
            Err(_) => 0.0,
        };

        *sample =  data;
    }

}
 

fn search_device(name: String) -> Result<Device, Error> {
    let host = cpal::default_host();

    // Set up the input device and stream with the default input config.
    let mut device = match host.default_input_device() {
        Some(device) => device,
        None => {
            return Err(Error::new(
                std::io::ErrorKind::Other,
                "Failed to get default input device",
            ))
        }
    };

    let output_devices = match host.output_devices() {
        Ok(devices) => devices,
        Err(_) => {
            return Err(Error::new(
                std::io::ErrorKind::Other,
                "Failed to get output devices",
            ))
        }
    };

    for actual_device in output_devices {
        let actual_name = match actual_device.name() {
            Ok(n) => n,
            Err(_) => continue,
        };

        if actual_name.contains(&name) {
            device = actual_device;
            break;
        }
    }
    return Ok(device);
}