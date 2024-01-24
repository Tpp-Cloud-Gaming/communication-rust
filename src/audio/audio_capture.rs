use std::io::Error;
use std::sync::mpsc::Sender;

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    Device, FromSample, Sample, Stream, SupportedStreamConfig,
};

use crate::audio::audio_utils::search_device;

/// Allows user to capture audio from a output device.
pub struct AudioCapture {
    /// Output device to be captured.
    device: Device,
    /// Output device config.
    config: SupportedStreamConfig,
    /// Flow of audio data from the selected audio device.
    stream: Option<Stream>,
    /// Channel where the audio data is writen
    sender: Sender<Vec<f32>>,
}

impl AudioCapture {
    /// Returns a AudioCapture
    ///
    /// # Arguments
    ///
    /// * `device_name` - A string that represents the device name
    /// * `sender` - A channel where AudioCapture writes the output device audio data.
    pub fn new(device_name: String, sender: Sender<Vec<f32>>) -> Result<Self, Error> {
        let device = search_device(device_name)?;

        let device_name = match device.name() {
            Ok(name) => name,
            Err(_) => {
                return Err(Error::new(
                    std::io::ErrorKind::Other,
                    format!("Error getting device name"),
                ))
            }
        };

        log::info!("Device find: {}", device_name);

        let config = match device.default_output_config() {
            Ok(config) => config,
            Err(_) => {
                return Err(Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to get default device config",
                ))
            }
        };
        log::info!("Device Config: {:?}", config);

        Ok(Self {
            device,
            config,
            stream: None,
            sender,
        })
    }

    /// Starts capturing audio from the chosen output device.
    pub fn start(&mut self) -> Result<Stream, Error> {
        let err_fn = move |err| {
            log::debug!("An error occurred on stream: {}", err);
        };

        let config_cpy = self.config.clone();
        let send_cpy = self.sender.clone();

        let stream = match self.config.sample_format() {
            cpal::SampleFormat::I8 => match self.device.build_input_stream(
                &config_cpy.into(),
                move |data, _: &_| write_input_data::<i8, i8>(data, send_cpy.clone()).unwrap(),
                err_fn,
                None,
            ) {
                Ok(stream) => stream,
                Err(_) => {
                    return Err(Error::new(
                        std::io::ErrorKind::Other,
                        format!("Error writing input data"),
                    ))
                }
            },
            cpal::SampleFormat::I16 => match self.device.build_input_stream(
                &config_cpy.into(),
                move |data, _: &_| write_input_data::<i16, i16>(data, send_cpy.clone()).unwrap(),
                err_fn,
                None,
            ) {
                Ok(stream) => stream,
                Err(_) => {
                    return Err(Error::new(
                        std::io::ErrorKind::Other,
                        format!("Error writing input data"),
                    ))
                }
            },
            cpal::SampleFormat::I32 => match self.device.build_input_stream(
                &config_cpy.into(),
                move |data, _: &_| write_input_data::<i32, i32>(data, send_cpy.clone()).unwrap(),
                err_fn,
                None,
            ) {
                Ok(stream) => stream,
                Err(_) => {
                    return Err(Error::new(
                        std::io::ErrorKind::Other,
                        format!("Error writing input data"),
                    ))
                }
            },
            cpal::SampleFormat::F32 => match self.device.build_input_stream(
                &config_cpy.into(),
                move |data, _: &_| write_input_data::<f32, f32>(data, send_cpy.clone()).unwrap(), //TODO: sacar unwrap
                err_fn,
                None,
            ) {
                Ok(stream) => stream,
                Err(_) => {
                    return Err(Error::new(
                        std::io::ErrorKind::Other,
                        format!("Error writing input data"),
                    ))
                }
            },
            sample_format => {
                return Err(Error::new(
                    std::io::ErrorKind::Other,
                    format!("Unsupported sample format {:?}", sample_format),
                ));
            }
        };

        match stream.play() {
            Ok(_) => return Ok(stream),
            Err(_) => {
                return Err(Error::new(
                    std::io::ErrorKind::Other,
                    "Error playing stream",
                ))
            }
        };
    }

    /// Stops audio capture.
    pub fn stop(&mut self) -> Result<(), Error> {
        match self.stream.take() {
            Some(stream) => drop(stream),
            None => {}
        };
        Ok(())
    }
}

/// Writes data on the sender.
///
/// # Arguments
///
/// * `input` - Data to be writen
/// * `sender` - Channel where data is writed.
fn write_input_data<T, U>(input: &[f32], sender: Sender<Vec<f32>>) -> Result<(), Error>
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{
    match sender.send(input.to_vec()) {
        Ok(_) => return Ok(()),
        Err(_) => {
            return Err(Error::new(
                std::io::ErrorKind::Other,
                format!("Error writing input data"),
            ))
        }
    }
}
