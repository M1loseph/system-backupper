use super::{Backup, BackupId, BackupMetadata};
use crate::errorstack::to_error_stack;
use std::fmt::Display;
use std::{error::Error as StdError, fmt::Debug};

pub enum RepositoryError {
    IdAlreadyExists { id: u64 },
    Unknown(Box<dyn StdError>),
}

impl Debug for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        to_error_stack(f, self)
    }
}

impl RepositoryError {
    pub fn new_unknown(cause: impl StdError + 'static) -> Self {
        Self::Unknown(Box::new(cause))
    }
}

impl StdError for RepositoryError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            RepositoryError::IdAlreadyExists { id: _ } => None,
            RepositoryError::Unknown(cause) => Some(cause.as_ref()),
        }
    }
}

impl Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::IdAlreadyExists { id } => write!(f, "Id {id} is already in use."),
            RepositoryError::Unknown(_) => {
                write!(
                    f,
                    "An unknown error has occurred when accessing the database."
                )
            }
        }
    }
}

pub type RepositoryResult<T> = std::result::Result<T, RepositoryError>;

pub trait BackupMetadataRepository: Send + Sync {
    fn save(&self, backup_metadata: &BackupMetadata) -> RepositoryResult<()>;

    fn find_by_id(&self, id: BackupId) -> RepositoryResult<Option<BackupMetadata>>;

    fn delete_by_id(&self, id: BackupId) -> RepositoryResult<bool>;

    fn find_all(&self) -> RepositoryResult<Vec<BackupMetadata>>;
}

pub trait BackupRepository: Send + Sync {
    fn save(&self, backup_metadata: &BackupMetadata, blob: Backup) -> RepositoryResult<()>;

    fn find_by_metadata(
        &self,
        backup_metadata: &BackupMetadata,
    ) -> RepositoryResult<Option<Backup>>;
}
