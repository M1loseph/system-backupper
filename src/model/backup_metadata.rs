use chrono::{DateTime, FixedOffset};
use rand;
use std::fmt::Display;

pub type BackupId = u64;

pub type Backup = Vec<u8>;

pub trait RandomId {
    fn random() -> Self;
}

impl RandomId for BackupId {
    fn random() -> Self {
        rand::random()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackupTargetKind {
    MongoDB,
    Postgres,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum BackupType {
    Manual,
    Scheduled,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackupFormat {
    ArchiveGz,
    TarGz,
}

#[derive(Debug)]
pub struct BackupTarget {
    pub kind: BackupTargetKind,
    pub name: String,
}

#[derive(Debug)]
pub struct BackupMetadata {
    pub backup_id: BackupId,
    pub created_at: DateTime<FixedOffset>,
    pub backup_size_bytes: u64,
    pub backup_target: BackupTarget,
    pub backup_type: BackupType,
    pub backup_format: BackupFormat,
}

impl Display for BackupFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupFormat::ArchiveGz => write!(f, "ArchiveGz"),
            BackupFormat::TarGz => write!(f, "TarGz"),
        }
    }
}

impl Display for BackupTargetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupTargetKind::MongoDB => write!(f, "MongoDB"),
            BackupTargetKind::Postgres => write!(f, "Postgres"),
        }
    }
}
