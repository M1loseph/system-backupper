use std::fmt::Display;
use std::process::Output;
use std::result::Result;
use std::str;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub struct ProcessOutputError(Output);

impl StdError for ProcessOutputError {}

impl Display for ProcessOutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let std_err_human_readable = match str::from_utf8(&self.0.stderr) {
            Ok(output) => output,
            Err(_) => "",
        };
        writeln!(
            f,
            "Process returned non zero status code. StdErr: {}",
            std_err_human_readable
        )
    }
}

pub trait IntoResult<T, E> {
    fn into_result(self) -> Result<T, E>;
}

impl IntoResult<Output, ProcessOutputError> for Output {
    fn into_result(self) -> Result<Output, ProcessOutputError> {
        match self.status.success() {
            true => Ok(self),
            false => Err(ProcessOutputError(self)),
        }
    }
}
