use std::error;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path;

use openai_token::count;
/// An error that occurs when parsing a file.
///
/// This error provides an end user friendly message describing why the file could not be parsed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseFileError {
    path: String,
    kind: ParseFileErrorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ParseFileErrorKind {
    NotFound,
    PermissionDenied,
    TokenLimit(u32),
    InvalidFormat,
}

impl ParseFileError {
    fn notfound(path: &str) -> ParseFileError {
        ParseFileError {
            path: path.to_string(),
            kind: ParseFileErrorKind::NotFound,
        }
    }

    fn denied(path: &str) -> ParseFileError {
        ParseFileError {
            path: path.to_string(),
            kind: ParseFileErrorKind::PermissionDenied,
        }
    }

    fn limit(path: &str, count: u32) -> ParseFileError {
        ParseFileError {
            path: path.to_string(),
            kind: ParseFileErrorKind::TokenLimit(count),
        }
    }

    fn format(path: &str) -> ParseFileError {
        ParseFileError {
            path: path.to_string(),
            kind: ParseFileErrorKind::InvalidFormat,
        }
    }
}

impl fmt::Display for ParseFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::ParseFileErrorKind::*;

        match self.kind {
            NotFound => write!(f, "cannot access '{}': no such file", self.path),
            PermissionDenied => write!(f, "cannot access '{}': permission denied", self.path),
            TokenLimit(ref count) => write!(
                f,
                "file '{}' token count '{}' exceeds token limit of 2000",
                self.path, count
            ),
            InvalidFormat => write!(
                f,
                "invalid format for file '{}': stream did not contain valid UTF-8",
                self.path
            ),
        }
    }
}

impl Error for ParseFileError {
    fn description(&self) -> &str {
        "invalid file"
    }
}

impl From<ParseFileError> for io::Error {
    fn from(path_err: ParseFileError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, path_err)
    }
}

/// Read a file and redirect content to openai request.
///
/// Currently only supports text files. Return an error if the file is not found or if there are no
/// permissions on the parent directories. Also returns an error if the file is too big.
pub fn parse_file(path_str: &str) -> Result<path::PathBuf, ParseFileError> {
    let path = path::PathBuf::from(path_str);
    match path.try_exists() {
        Ok(true) => (),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            return Err(ParseFileError::denied(path_str))
        }
        _ => return Err(ParseFileError::notfound(path_str)),
    }

    let buf_reader = io::BufReader::new(fs::File::open(&path).unwrap());
    let token_count = match count(buf_reader) {
        Ok(count) => count,
        Err(e) => {
            println!("{:#?}", e);
            return Err(ParseFileError::format(path_str));
        }
    };

    if token_count <= 20000 {
        println!("{}", token_count);
    } else {
        return Err(ParseFileError::limit(path_str, token_count));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_exists() {
        assert!(parse_file("/home/smahm/dump/test").is_ok())
    }

    #[test]
    fn file_not_exist() {
        assert!(parse_file("/etc/test").is_err())
    }

    #[test]
    fn file_permission_denied() {
        assert!(parse_file("/root/test").is_err())
    }
}
