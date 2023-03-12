use openai::cli;
use std::error::Error;
use std::path::PathBuf;
use std::process;

fn main() {
    if let Err(err) = try_main() {
        eprintln!("clai: {}", err);
        process::exit(1);
    }
}

fn try_main() -> Result<Option<PathBuf>, Box<dyn Error>> {
    Ok(Some(cli::parse_file("/home/smahm/dump/test")?))
}
