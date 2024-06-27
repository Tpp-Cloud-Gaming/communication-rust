use std::env;
use std::io::Error;

use crate::front_connection::front_protocol::FrontConnection;

use super::shutdown::Shutdown;

/// Reads line from standard input.
///
/// # Returns
///
/// A [`Result`] containing the trimmed input line on success.
/// [`Error`] in case of error.
pub fn must_read_stdin() -> Result<String, Error> {
    let mut line = String::new();

    std::io::stdin().read_line(&mut line)?;
    line = line.trim().to_owned();
    println!();

    Ok(line)
}

/// Gets command-line argument.
///
/// # Returns
///
/// A [`Option`] with a `String` containing the argument.
pub fn get_args() -> Option<String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        None
    } else {
        Some(args[1].to_string())
    }
}

pub fn wait_disconnect(mut shutdown: Shutdown) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        shutdown.add_task("wait_disconnect").await;
        tokio::select! {
            _ = shutdown.wait_for_error() => {
                println!("Ended by shutdown signal");
            }
            f = FrontConnection::new("3132") => {
                match f {
                    Ok(mut front_connection) => {
                        let _ = front_connection.waiting_to_disconnect().await;
                        println!("Ended by disconnect signal");
                        shutdown.notify_error(false, "").await;
                    },
                    Err(_) => {
                        shutdown.notify_error(false, "Front connection").await;
                    }
                }
            }
        }
    })
}
