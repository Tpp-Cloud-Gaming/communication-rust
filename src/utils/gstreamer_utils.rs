use gstreamer::{prelude::*, Pipeline};
use crate::utils::shutdown::{self};

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