use crate::{
    errorstack::to_error_stack,
    lock::LockError,
    model::{BackupId, BackupTargetKind},
};
use std::{
    error::Error as StdError,
    fmt::{self, Debug, Display},
};

pub enum BackupCreateError {
    BackupTargetLocked(String),
    BackupTargetNotFound(String),
    Unknown(Box<dyn StdError>),
}

impl StdError for BackupCreateError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            BackupCreateError::BackupTargetLocked(_) => None,
            BackupCreateError::BackupTargetNotFound(_) => None,
            BackupCreateError::Unknown(error) => Some(error.as_ref()),
        }
    }
}

impl Debug for BackupCreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        to_error_stack(f, self)
    }
}

impl Display for BackupCreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupCreateError::BackupTargetLocked(backup_target) => write!(
                f,
                "Backup target {backup_target} is undergoing another operation."
            ),
            BackupCreateError::BackupTargetNotFound(backup_target) => {
                write!(f, "Backup target {backup_target} was not found.")
            }
            BackupCreateError::Unknown(_) => write!(f, "An unknown error has occurred."),
        }
    }
}

pub enum BackupRestoreError {
    BackupTargetNotFound {
        name: String,
    },
    BackupTargetLocked {
        name: String,
    },
    BackupDoesNotExist(BackupId),
    InconsistantData(BackupId),
    IncompatibleKind {
        from: BackupTargetKind,
        to: BackupTargetKind,
    },
    Unknown(Box<dyn StdError>),
}

impl StdError for BackupRestoreError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            BackupRestoreError::BackupTargetNotFound { name: _ } => None,
            BackupRestoreError::BackupTargetLocked { name: _ } => None,
            BackupRestoreError::BackupDoesNotExist(_) => None,
            BackupRestoreError::InconsistantData(_) => None,
            BackupRestoreError::IncompatibleKind { from: _, to: _ } => None,
            BackupRestoreError::Unknown(error) => Some(error.as_ref()),
        }
    }
}

impl Debug for BackupRestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        to_error_stack(f, self)
    }
}

impl Display for BackupRestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupRestoreError::BackupTargetNotFound { name } => {
                write!(f, "Backup target {} was not found.", name)
            }
            BackupRestoreError::BackupTargetLocked { name } => {
                write!(f, "Backup target {} is undergoing another operation.", name)
            }
            BackupRestoreError::BackupDoesNotExist(id) => {
                write!(f, "Did not find backup with id {}.", id)
            }
            BackupRestoreError::Unknown(_) => write!(f, "An unexpected error occurred."),
            BackupRestoreError::InconsistantData(backup_id) => {
                write!(f, "Missing backup binary for backup_id {}", backup_id)
            }
            BackupRestoreError::IncompatibleKind { from, to } => {
                write!(f, "Can't restore backup of kind {from} to target {to}")
            }
        }
    }
}

pub enum BackupFindError {
    Unknown(Box<dyn StdError>),
}

impl StdError for BackupFindError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            BackupFindError::Unknown(error) => Some(error.as_ref()),
        }
    }
}

impl Display for BackupFindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupFindError::Unknown(_) => write!(f, "An unexpected error occurred."),
        }
    }
}

impl Debug for BackupFindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        to_error_stack(f, self)
    }
}

pub enum BackupHealthCheckError {
    BackupTargetNotFound { name: String },
    BackupTargetLocked { name: String, cause: LockError },
    Unknown(anyhow::Error),
}

impl StdError for BackupHealthCheckError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            BackupHealthCheckError::BackupTargetNotFound { name: _ } => None,
            BackupHealthCheckError::Unknown(error) => Some(error.as_ref()),
            BackupHealthCheckError::BackupTargetLocked { name: _, cause } => Some(cause),
        }
    }
}

impl Debug for BackupHealthCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        to_error_stack(f, self)
    }
}

impl Display for BackupHealthCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupHealthCheckError::BackupTargetNotFound { name } => {
                write!(f, "Backup target {} was not found.", name)
            }
            BackupHealthCheckError::Unknown(_) => write!(f, "An unexpected error occurred."),
            BackupHealthCheckError::BackupTargetLocked {
                name,
                cause: _cause,
            } => write!(f, "Backup target {} is undergoing another operation.", name),
        }
    }
}

impl From<anyhow::Error> for BackupHealthCheckError {
    fn from(value: anyhow::Error) -> Self {
        Self::Unknown(value)
    }
}
