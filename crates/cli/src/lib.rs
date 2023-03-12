/*!
This crate provides common routines used in command line applications, with a focus on routines
useful for sending data to openAI's chatgpt3.5 model. A key focus of this crate is to improve
failure modes and provide user friendly error messages when things go wrong.
 */
#![deny(missing_docs)]

mod file;

pub use crate::file::parse_file;

/// Returns true if stdin is believed to be readable.
///
/// Examples of readable stdin:
/// redirecting a file to stdin (clai robot << script.py)
/// piping stdout of command to stdin (cat script.py | clai robot)
pub fn is_readable_stdin() -> bool {
    fn imp() -> bool {
        use same_file::Handle;
        use std::os::unix::fs::FileTypeExt;

        let ft = match Handle::stdin().and_then(|h| h.as_file().metadata()) {
            Err(_) => return false,
            Ok(md) => md.file_type(),
        };
        ft.is_file() || ft.is_fifo() || ft.is_socket()
    }
    println!("{}", is_tty_stdin());
    println!("{}", is_tty_stdout());
    imp() && !is_tty_stdin()
}

/// Returns true if and only if stdin is believed to be connected to a tty or console
pub fn is_tty_stdin() -> bool {
    atty::is(atty::Stream::Stdin)
}

/// Returns true if and only if stdout is believed to be connected to a tty or console
pub fn is_tty_stdout() -> bool {
    atty::is(atty::Stream::Stdout)
}

/// Returns true if and only if stderr is believed to be connected to a tty or console
pub fn is_tty_stderr() -> bool {
    atty::is(atty::Stream::Stderr)
}
