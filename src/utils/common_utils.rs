use std::env;
use std::io::Error;

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
