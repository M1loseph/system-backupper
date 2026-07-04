use std::error::Error as StdError;
use std::fmt;

pub fn to_error_stack(f: &mut fmt::Formatter<'_>, err: &dyn StdError) -> Result<(), fmt::Error> {
    writeln!(f, "{}", err)?;

    let mut cause = err.source();
    while let Some(nested_err) = cause {
        writeln!(f, "\n\tCaused by:\n\t{}", nested_err)?;

        cause = nested_err.source();
    }
    Ok(())
}
