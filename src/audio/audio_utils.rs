use std::io::Error;

use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device,
};

pub fn search_device(name: String) -> Result<Device, Error> {
    let host = cpal::default_host();

    // Set up the input device and stream with the default input config.
    let mut _device = match host.default_input_device() {
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
            _device = actual_device;
            // TODO: este break puede generar error !!!
            break;
        }
    }
    Ok(host.default_output_device().unwrap())
}
