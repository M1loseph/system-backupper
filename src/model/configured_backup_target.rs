use super::BackupTargetKind;

pub struct ConfiguredBackupTarget {
    pub target_name: String,
    pub target_kind: BackupTargetKind,
    pub connection_string: String,
}
