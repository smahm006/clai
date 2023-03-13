use openai::cli;
use std::error::Error;
use std::path::PathBuf;
use std::process;

use std::time::Instant;
fn main() {
    let now = Instant::now();
    if let Err(err) = try_main() {
        eprintln!("clai: {}", err);
        process::exit(1);
    }
    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);
}

fn try_main() -> Result<Option<PathBuf>, Box<dyn Error>> {
    Ok(Some(cli::parse_file("/etc/hosts")?))
}
