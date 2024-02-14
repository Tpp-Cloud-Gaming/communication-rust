use opus::Encoder;

use std::io::Error;

use crate::utils::webrtc_const::ENCODE_BUFFER_SIZE;

use crate::utils::webrtc_const::SAMPLE_RATE;

pub struct AudioEncoder {
    encoder: Encoder,
}

impl AudioEncoder {
    pub fn new() -> Result<Self, Error> {
        let encoder =
            opus::Encoder::new(SAMPLE_RATE, opus::Channels::Stereo, opus::Application::Voip)
                .unwrap();

        Ok(Self { encoder })
    }

    pub fn encode(&mut self, data: Vec<f32>) -> Result<Vec<u8>, opus::Error> {
        match self.encoder.encode_vec_float(&data, ENCODE_BUFFER_SIZE) {
            Ok(buffer) => Ok(buffer),
            Err(e) => Err(e),
        }
    }
}
