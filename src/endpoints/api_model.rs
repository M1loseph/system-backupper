use crate::model::{self, BackupMetadata};
use chrono::{DateTime, FixedOffset};
use serde::Serialize;

#[derive(Serialize)]
pub struct BackupTarget {
    pub name: String,
    pub kind: BackupTargetKind,
}

#[derive(Serialize)]
pub struct ArchiveBackupResponse {
    pub backup_id: u64,
    pub created_at: DateTime<FixedOffset>,
    pub backup_size_bytes: u64,
    pub backup_target: BackupTarget,
    pub backup_type: BackupType,
    pub backup_format: BackupFormat,
}

impl From<BackupMetadata> for ArchiveBackupResponse {
    fn from(value: BackupMetadata) -> Self {
        Self {
            backup_id: value.backup_id,
            created_at: value.created_at,
            backup_size_bytes: value.backup_size_bytes,
            backup_target: BackupTarget {
                name: value.backup_target.name,
                kind: value.backup_target.kind.into(),
            },
            backup_type: value.backup_type.into(),
            backup_format: value.backup_format.into(),
        }
    }
}

#[derive(Serialize)]
pub struct BackupHealthCheckResponse {
    pub is_healthy: bool,
}

#[derive(Serialize)]
pub enum BackupType {
    #[serde(rename = "MANUAL")]
    Manual,
    #[serde(rename = "SCHEDULED")]
    Scheduled,
}

impl From<model::BackupType> for BackupType {
    fn from(value: model::BackupType) -> Self {
        match value {
            model::BackupType::Manual => Self::Manual,
            model::BackupType::Scheduled => Self::Scheduled,
        }
    }
}

#[derive(Serialize)]
pub enum BackupTargetKind {
    #[serde(rename = "MONGODB")]
    MongoDB,
    #[serde(rename = "POSTGRES")]
    Postgres,
}

impl From<model::BackupTargetKind> for BackupTargetKind {
    fn from(value: model::BackupTargetKind) -> Self {
        match value {
            model::BackupTargetKind::MongoDB => Self::MongoDB,
            model::BackupTargetKind::Postgres => Self::Postgres,
        }
    }
}

#[derive(Serialize, Debug)]
pub enum ErrorCode {
    #[serde(rename = "BACKUP_TARGET_LOCKED")]
    BackupTargetLocked,
    #[serde(rename = "BACKUP_TARGET_NOT_FOUND")]
    BackupTargetNotFound,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
    #[serde(rename = "BACKUP_NOT_FOUND")]
    BackupNotFound,
}

#[derive(Serialize, Debug)]
pub struct ApiError {
    pub error_code: ErrorCode,
    pub message: String,
}

#[derive(Serialize, Debug)]
pub enum BackupFormat {
    #[serde(rename = "TAR_GZ")]
    TarGz,
    #[serde(rename = "ARCHIVE_GZ")]
    ArchiveGz,
}

impl From<model::BackupFormat> for BackupFormat {
    fn from(value: model::BackupFormat) -> Self {
        match value {
            model::BackupFormat::ArchiveGz => Self::ArchiveGz,
            model::BackupFormat::TarGz => Self::TarGz,
        }
    }
}

#[derive(Serialize)]
pub struct BackupTargetResponse {
    pub name: String,
    pub kind: BackupTargetKind,
    pub host: Option<String>,
}
