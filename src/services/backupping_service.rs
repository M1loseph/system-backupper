use crate::lock::{LockError, LockManager};
use crate::model::{
    BackupId, BackupMetadata, BackupMetadataRepository, BackupRepository, BackupTarget,
    BackupTargetKind, BackupType, ConfiguredBackupTarget, RandomId, RepositoryError,
};
use crate::services::BackupHealthCheckError;
use chrono::Local;
use log::info;
use std::result::Result;
use std::sync::Arc;

use super::backup_strategy::{
    BackupStrategy, MongoDBCompressedBackupStrategy, PostgresCompressedBackupStrategy,
};
use super::errors::{BackupCreateError, BackupFindError, BackupRestoreError};

static METADATA_SAVE_RETRIES: u8 = 3;

pub trait BackuppingService: Send + Sync {
    fn create_backup(
        &self,
        target_name: &str,
        backup_type: BackupType,
    ) -> Result<BackupMetadata, BackupCreateError>;

    fn read_all_backups(&self) -> Result<Vec<BackupMetadata>, BackupFindError>;

    fn restore_backup(
        &self,
        target_name: &str,
        backup_id: u64,
        drop: bool,
    ) -> Result<(), BackupRestoreError>;

    fn read_all_configured_targets(&self) -> &Vec<ConfiguredBackupTarget>;

    fn check_if_target_is_healthy(&self, target_name: &str)
        -> Result<bool, BackupHealthCheckError>;
}

pub struct BackuppingServiceImpl {
    lock_manager: Arc<LockManager>,
    backup_repository: Arc<dyn BackupRepository>,
    backup_metadata_repository: Arc<dyn BackupMetadataRepository>,
    mongodb_strategy: Arc<MongoDBCompressedBackupStrategy>,
    postgres_strategy: Arc<PostgresCompressedBackupStrategy>,
    backup_targets: Vec<ConfiguredBackupTarget>,
}

impl BackuppingService for BackuppingServiceImpl {
    fn create_backup(
        &self,
        target_name: &str,
        backup_type: BackupType,
    ) -> Result<BackupMetadata, BackupCreateError> {
        let backup_target = self
            .find_target_by_name(target_name)
            .ok_or_else(|| BackupCreateError::BackupTargetNotFound(target_name.to_string()))?;
        let _lock = self.lock_manager.lock(target_name)?;

        info!(
            "Starting backing up target {} at {}",
            target_name,
            Local::now()
        );

        let strategy = self.pick_strategy_by_target_kind(&backup_target.target_kind);

        let (blob, backup_format) = strategy.create_backup(&backup_target.connection_string)?;
        let blob_size = blob.len() as u64;

        let backup_metadata: BackupMetadata = (|| -> Result<BackupMetadata, BackupCreateError> {
            let mut i = 1;
            loop {
                let backup_metadata = BackupMetadata {
                    backup_id: BackupId::random(),
                    created_at: Local::now().fixed_offset(),
                    backup_size_bytes: blob_size,
                    backup_target: BackupTarget {
                        name: backup_target.target_name.clone(),
                        kind: backup_target.target_kind.clone(),
                    },
                    backup_type: backup_type.clone(),
                    backup_format: backup_format.clone(),
                };

                match self.backup_metadata_repository.save(&backup_metadata) {
                    Ok(_) => return Ok(backup_metadata),
                    Err(err) => {
                        match err {
                            RepositoryError::IdAlreadyExists { id } => {
                                if i == METADATA_SAVE_RETRIES {
                                    return Err(BackupCreateError::from(err));
                                } else {
                                    info!("Creating backup failed - id {id} already exists. Retrying...");
                                }
                            }
                            _ => return Err(BackupCreateError::from(err)),
                        }
                    }
                };
                i += 1;
            }
        })()?;
        if let Err(err) = self.backup_repository.save(&backup_metadata, blob) {
            self.backup_metadata_repository
                .delete_by_id(backup_metadata.backup_id)?;
            return Err(BackupCreateError::from(err));
        }

        Ok(backup_metadata)
    }

    fn read_all_backups(&self) -> Result<Vec<BackupMetadata>, BackupFindError> {
        let backups = self.backup_metadata_repository.find_all()?;
        Ok(backups)
    }

    fn restore_backup(
        &self,
        target_name: &str,
        backup_id: u64,
        drop: bool,
    ) -> Result<(), BackupRestoreError> {
        let backup_target = self.find_target_by_name(target_name).ok_or_else(|| {
            BackupRestoreError::BackupTargetNotFound {
                name: target_name.to_string(),
            }
        })?;
        let _lock = self.lock_manager.lock(target_name)?;

        let backup_metada = self
            .backup_metadata_repository
            .find_by_id(backup_id)?
            .ok_or(BackupRestoreError::BackupDoesNotExist(backup_id))?;

        if backup_metada.backup_target.kind != backup_target.target_kind {
            return Err(BackupRestoreError::IncompatibleKind {
                from: backup_metada.backup_target.kind.clone(),
                to: backup_target.target_kind.clone(),
            });
        }

        let backup = self
            .backup_repository
            .find_by_metadata(&backup_metada)?
            .ok_or_else(|| BackupRestoreError::InconsistantData(backup_id))?;

        let strategy = self.pick_strategy_by_target_kind(&backup_target.target_kind);
        strategy.restore_backup(&backup_target.connection_string, drop, backup)?;

        Ok(())
    }

    fn read_all_configured_targets(&self) -> &Vec<ConfiguredBackupTarget> {
        &self.backup_targets
    }

    fn check_if_target_is_healthy(
        &self,
        target_name: &str,
    ) -> Result<bool, BackupHealthCheckError> {
        let _lock = self.lock_manager.lock(target_name)?;
        let backup_target = self.find_target_by_name(target_name).ok_or_else(|| {
            BackupHealthCheckError::BackupTargetNotFound {
                name: target_name.to_string(),
            }
        })?;

        let strategy = self.pick_strategy_by_target_kind(&backup_target.target_kind);
        strategy
            .is_target_healthy(&backup_target.connection_string)
            .map_err(|e| BackupHealthCheckError::Unknown(e))
    }
}

impl BackuppingServiceImpl {
    pub fn new(
        lock_manager: Arc<LockManager>,
        backup_repository: Arc<dyn BackupRepository>,
        backup_metadata_repository: Arc<dyn BackupMetadataRepository>,
        mongodb_strategy: Arc<MongoDBCompressedBackupStrategy>,
        postgres_strategy: Arc<PostgresCompressedBackupStrategy>,
        backup_targets: Vec<ConfiguredBackupTarget>,
    ) -> Self {
        Self {
            lock_manager,
            backup_repository,
            backup_metadata_repository,
            mongodb_strategy,
            postgres_strategy,
            backup_targets,
        }
    }

    fn find_target_by_name(&self, name: &str) -> Option<&ConfiguredBackupTarget> {
        self.backup_targets
            .iter()
            .find(|target| target.target_name == name)
    }

    fn pick_strategy_by_target_kind(&self, target_kind: &BackupTargetKind) -> &dyn BackupStrategy {
        match target_kind {
            BackupTargetKind::MongoDB => self.mongodb_strategy.as_ref(),
            BackupTargetKind::Postgres => self.postgres_strategy.as_ref(),
        }
    }
}

impl From<LockError> for BackupCreateError {
    fn from(lock_error: LockError) -> Self {
        match lock_error {
            LockError::LockAlreadyExists(lock_key) => Self::BackupTargetLocked(lock_key),
            _ => Self::Unknown(Box::new(lock_error)),
        }
    }
}

impl From<RepositoryError> for BackupCreateError {
    fn from(err: RepositoryError) -> Self {
        Self::Unknown(Box::new(err))
    }
}

impl From<LockError> for BackupRestoreError {
    fn from(lock_error: LockError) -> Self {
        match lock_error {
            LockError::LockAlreadyExists(lock_key) => Self::BackupTargetLocked { name: lock_key },
            _ => Self::Unknown(Box::new(lock_error)),
        }
    }
}

impl From<RepositoryError> for BackupRestoreError {
    fn from(err: RepositoryError) -> Self {
        Self::Unknown(Box::new(err))
    }
}

impl From<RepositoryError> for BackupFindError {
    fn from(value: RepositoryError) -> Self {
        Self::Unknown(Box::new(value))
    }
}

impl From<std::io::Error> for BackupRestoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Unknown(Box::new(value))
    }
}

impl From<anyhow::Error> for BackupRestoreError {
    fn from(value: anyhow::Error) -> Self {
        Self::Unknown(value.into())
    }
}

impl From<anyhow::Error> for BackupCreateError {
    fn from(value: anyhow::Error) -> Self {
        Self::Unknown(value.into())
    }
}

impl From<LockError> for BackupHealthCheckError {
    fn from(lock_error: LockError) -> Self {
        match &lock_error {
            LockError::LockAlreadyExists(lock_key) => BackupHealthCheckError::BackupTargetLocked {
                name: lock_key.clone(),
                cause: lock_error,
            },
            _ => BackupHealthCheckError::Unknown(anyhow::Error::new(lock_error)),
        }
    }
}
