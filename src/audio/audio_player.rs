use crate::audio::audio_utils::search_device;

use cpal::{traits::DeviceTrait, SampleFormat, Stream, Device};
use std::{io::Error, sync::{Mutex, Arc}};
use tokio::sync::mpsc::Receiver;



pub struct AudioPlayer{
    rx: Arc<Mutex<Receiver<f32>>>,
    device: Device,
    config:cpal::StreamConfig,
    sample_format: SampleFormat,
}

impl AudioPlayer {


    pub fn new(device: &str, rx:Arc<Mutex<Receiver<f32>>>) -> Result<Self,Error> {

        let device = search_device(device.to_owned()).unwrap();
        let config = device.default_output_config().unwrap();
        let sample_format = config.sample_format();
        let config: cpal::StreamConfig = config.into();

        Ok(Self { rx, device, config, sample_format })
        
    }

    pub fn start(&self) -> Result<Stream , Error> {

        let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);

        let rx_clone = self.rx.clone();
    
        
        let stream = match self.sample_format {
            SampleFormat::F32 => self.device.build_output_stream(&self.config, move |data: &mut [f32], _: &_|  {
                write_data(data, rx_clone.clone())
            }, err_fn, None),
            SampleFormat::I16 => self.device.build_output_stream(&self.config,  move |data: &mut [f32], _: &_| {
                write_data(data, rx_clone.clone())
            }, err_fn, None),
            SampleFormat::U16 => self.device.build_output_stream(&self.config, move |data: &mut [f32], _: &_| {
                write_data(data, rx_clone.clone())
            }, err_fn, None),
            sample_format => panic!("Unsupported sample format '{sample_format}'")
        }.unwrap();
        
        Ok(stream)
    }

}


fn write_data(output: &mut [f32], rx:  Arc<Mutex<Receiver<f32>>>)
{
    
    for sample in output {
        
        let data = match rx.lock().unwrap().try_recv() {
            Ok(sample) => {
                sample},
            Err(_) => 0.0,
        };

        *sample =  data;
    }

}