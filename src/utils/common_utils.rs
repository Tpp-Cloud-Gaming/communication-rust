use std::env;
use std::io::Error;

pub fn must_read_stdin() -> Result<String, Error> {
    let mut line = String::new();

    std::io::stdin().read_line(&mut line)?;
    line = line.trim().to_owned();
    println!();

    Ok(line)
}

pub fn get_args() -> Option<String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        None
    } else {
        Some(args[1].to_string())
    }
}
