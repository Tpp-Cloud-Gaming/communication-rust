use opus::Encoder;

use std::io::Error;

use crate::utils::webrtc_const::ENCODE_BUFFER_SIZE;

use crate::utils::webrtc_const::SAMPLE_RATE;

pub struct AudioEncoder {
    encoder: Encoder,
}

impl AudioEncoder {
    /// Returns new instance of audio encoder.
    pub fn new() -> Result<Self, Error> {
        let encoder =
            opus::Encoder::new(SAMPLE_RATE, opus::Channels::Stereo, opus::Application::Voip)
                .unwrap();

        Ok(Self { encoder })
    }

    /// Returns an encoded opus sample as a Vec<f32>
    /// # Arguments
    ///
    /// * `input` - Vec<u8> that represents an audio sample
    pub fn encode(&mut self, data: Vec<f32>) -> Result<Vec<u8>, opus::Error> {
        match self.encoder.encode_vec_float(&data, ENCODE_BUFFER_SIZE) {
            Ok(buffer) => Ok(buffer),
            Err(e) => Err(e),
        }
    }
}
