
use cpal::traits::{HostTrait, DeviceTrait};
use hound::WavWriter;
use opus::Decoder;
use std::{io::{Error, BufWriter}, sync::{Mutex, Arc}, fs::File};


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





