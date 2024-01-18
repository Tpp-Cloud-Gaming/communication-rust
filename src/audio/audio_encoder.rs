
use opus::Encoder;

use std::io::Error;

pub struct AudioEncoder {
    encoder: Encoder,
}

impl AudioEncoder {
    pub fn new() -> Result<Self, Error> {

        let sample_rate: u32 = std::env::var("SAMPLE_RATE")
            .expect("SAMPLE_RATE env not found.")
            .parse()
            .expect("Failed to parse SAMPLE_RATE as u32");

        let encoder =
            opus::Encoder::new(sample_rate, opus::Channels::Stereo, opus::Application::Voip)
                .unwrap();

        Ok(Self {
            encoder: encoder,
        })
    }

    pub fn encode(&mut self, data: Vec<f32>) -> Result<Vec<u8>, opus::Error> {
        match self.encoder.encode_vec_float(&data, 960) {
            Ok(buffer) => Ok(buffer),
            Err(e) => Err(e),
        }
    }
    
}

