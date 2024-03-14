use crate::utils::shutdown::{self};
use gstreamer::{prelude::*, Pipeline};
use gstreamer_app::{AppSink, AppSrc};
use std::{
    io::{self, Error},
    sync::mpsc::Receiver,
};
use tokio::{runtime::Runtime, sync::mpsc::Sender};

pub async fn read_bus(pipeline: Pipeline, shutdown: shutdown::Shutdown) {
    // Wait until error or EOS
    let pipeline_name = pipeline.name();

    let bus = match pipeline.bus() {
        Some(b) => b,
        None => {
            shutdown.notify_error(false).await;
            log::error!("{pipeline_name} | Pipeline bus not found");
            return;
        }
    };

    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;

        match msg.view() {
            MessageView::Error(err) => {
                log::error!(
                    "{pipeline_name} | Error received from element {:?} {}",
                    err.src().map(|s| s.path_string()),
                    err.error()
                );
                shutdown.notify_error(false).await;
                break;
            }
            MessageView::StateChanged(state_changed) => {
                if state_changed.src().map(|s| s == &pipeline).unwrap_or(false) {
                    log::info!(
                        "{pipeline_name} | Pipeline state changed from {:?} to {:?}",
                        state_changed.old(),
                        state_changed.current()
                    );
                }
            }
            MessageView::Eos(..) => {
                log::info!("{pipeline_name} | End of stream received");
                break;
            }
            _ => (),
        }
    }
}

pub fn pull_sample(appsink: &AppSink, tx: Sender<Vec<u8>>) -> Result<(), Error> {
    // Pull the sample in question out of the appsink's buffer.
    let sample = appsink
        .pull_sample()
        .map_err(|_| Error::new(io::ErrorKind::Other, "Error pulling sample from appsink"))?;

    let buffer = sample
        .buffer()
        .ok_or_else(|| Error::new(io::ErrorKind::Other, "Error getting buffer"))?;

    let map = buffer
        .map_readable()
        .map_err(|_| Error::new(io::ErrorKind::Other, "Error reading buffer"))?;

    let samples = map.as_slice();
    let rt =
        Runtime::new().map_err(|_| Error::new(io::ErrorKind::Other, "Error creating Runtime"))?;

    rt.block_on(async {
        match tx.send(samples.to_vec()).await {
            Ok(result) => result,
            Err(_) => log::error!("APPSINK | Error sending sample"),
        };
    });

    Ok(())
}

pub fn push_sample(appsrc: &AppSrc, rx: &Receiver<Vec<u8>>) -> Result<(), Error> {
    let frame = rx
        .recv()
        .map_err(|_| Error::new(io::ErrorKind::Other, "Error reading buffer"))?;

    let buffer = gstreamer::Buffer::from_slice(frame);

    appsrc
        .push_buffer(buffer)
        .map_err(|_| Error::new(io::ErrorKind::Other, "Error pushing buffer"))?;

    Ok(())
}
