use crate::{
    connection_pool::ConnectionPool,
    model::{
        Backup, BackupFormat, BackupId, BackupMetadata, BackupMetadataRepository, BackupRepository,
        BackupTargetKind, BackupType, RepositoryError, RepositoryResult,
    },
};
use crate::{errorstack::to_error_stack, model::BackupTarget};
use log::info;
use rusqlite::Error as RustqliteError;
use rusqlite::{params, ErrorCode, Row};
use std::{
    error::Error as StdError,
    fmt::{Debug, Display},
    fs::{self, File},
    io::{ErrorKind, Write},
    num::ParseIntError,
    path::Path,
};

static MONGODB_DIR: &str = "mongodb";
static POSTGRES_DIR: &str = "postgres";
static PRIMARY_KEY_CONSTRAINT_VIOLATION_ERROR_MESSAGE: &str =
    "UNIQUE constraint failed: backup_metadata.backup_id";

pub struct EnumNotFoundError(String);

impl StdError for EnumNotFoundError {}

impl Display for EnumNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Did not find a matching enum value for input {}", self.0)
    }
}

impl Debug for EnumNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        to_error_stack(f, self)
    }
}

trait EnumStringSQLForm: Sized {
    fn to_sql(&self) -> &str;

    fn from_sql(sql: String) -> Result<Self, EnumNotFoundError>;
}

impl<'a> TryFrom<&Row<'a>> for BackupMetadata {
    type Error = RepositoryError;

    fn try_from(row: &Row<'a>) -> Result<Self, Self::Error> {
        let backup_target = BackupTarget {
            kind: BackupTargetKind::from_sql(row.get(3)?)?,
            name: row.get(4)?,
        };
        let metadata = Self {
            backup_id: row.get::<usize, String>(0)?.parse::<u64>()?,
            created_at: row.get(1)?,
            backup_size_bytes: row.get::<usize, String>(2)?.parse::<u64>()?,
            backup_target: backup_target,
            backup_type: BackupType::from_sql(row.get(5)?)?,
            backup_format: BackupFormat::from_sql(row.get(6)?)?,
        };
        Ok(metadata)
    }
}

pub struct SQLiteBackupMetadataRepository {
    connection_pool: ConnectionPool,
}

impl SQLiteBackupMetadataRepository {
    pub fn new(connection_pool: ConnectionPool) -> RepositoryResult<Self> {
        Ok(Self { connection_pool })
    }
}

impl BackupMetadataRepository for SQLiteBackupMetadataRepository {
    fn save(&self, backup_metadata: &BackupMetadata) -> RepositoryResult<()> {
        let query = r#"
            INSERT INTO "backup_metadata"("backup_id", "created_at", "backup_size_bytes", "backup_target_kind", "backup_target_name", "backup_type", "backup_format")
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#;
        let connection = self.connection_pool.get_random_connection();
        let mut statement = connection.prepare(query)?;

        let query_result = statement.execute(params![
            backup_metadata.backup_id.to_string(),
            backup_metadata.created_at,
            backup_metadata.backup_size_bytes.to_string(),
            backup_metadata.backup_target.kind.to_sql(),
            backup_metadata.backup_target.name,
            backup_metadata.backup_type.to_sql(),
            backup_metadata.backup_format.to_sql(),
        ]);
        query_result.map(|_| ()).map_err(|err| {
            if let RustqliteError::SqliteFailure(err, message) = &err {
                let message = message.as_deref();
                if err.code == ErrorCode::ConstraintViolation
                    && message == Some(PRIMARY_KEY_CONSTRAINT_VIOLATION_ERROR_MESSAGE)
                {
                    return RepositoryError::IdAlreadyExists {
                        id: backup_metadata.backup_id,
                    };
                }
            }
            RepositoryError::from(err)
        })
    }

    fn find_by_id(&self, id: BackupId) -> RepositoryResult<Option<BackupMetadata>> {
        let query = r#"
            SELECT "backup_id", "created_at", "backup_size_bytes", "backup_target_kind", "backup_target_name", "backup_type", "backup_format"
            FROM "backup_metadata"
            WHERE "backup_id" = ?1
        "#;
        let connection = self.connection_pool.get_random_connection();
        let mut statement = connection.prepare(query)?;
        let mut query_result = statement.query(params![id.to_string()])?;
        let backup_metadata = query_result
            .next()?
            .map(|row| BackupMetadata::try_from(row));
        match backup_metadata {
            None => Ok(None),
            Some(backup_metadata) => backup_metadata.map(|result| Some(result)),
        }
    }

    fn find_all(&self) -> RepositoryResult<Vec<BackupMetadata>> {
        let query = r#"
            SELECT "backup_id", "created_at", "backup_size_bytes", "backup_target_kind", "backup_target_name", "backup_type", "backup_format"
            FROM "backup_metadata"
        "#;
        let connection = self.connection_pool.get_random_connection();
        let mut connection = connection.prepare(query)?;
        let rows = connection.query(params![])?;
        rows.and_then(|row| BackupMetadata::try_from(row)).collect()
    }

    fn delete_by_id(&self, id: BackupId) -> RepositoryResult<bool> {
        let query = r#"
            DELETE FROM "backup_metadata"
            WHERE "backup_id" = ?1
        "#;
        let connection = self.connection_pool.get_random_connection();
        let mut statement = connection.prepare(query)?;
        let affected_rows = statement.execute(params![id.to_string()])?;
        return Ok(affected_rows == 1);
    }
}

pub struct FileSystemBackupRepository {
    target_directory: String,
}

impl FileSystemBackupRepository {
    pub fn new(target_directory: String) -> Self {
        Self { target_directory }
    }

    fn to_file_name(&self, backup_metadata: &BackupMetadata) -> String {
        let id = backup_metadata.backup_id.to_string();
        let file_extension = match backup_metadata.backup_format {
            BackupFormat::ArchiveGz => "gz",
            BackupFormat::TarGz => "tar.gz",
        };
        format!("{id}.{file_extension}")
    }

    fn backup_directory(&self, backup_target: &BackupTargetKind) -> &str {
        match backup_target {
            BackupTargetKind::MongoDB => MONGODB_DIR,
            BackupTargetKind::Postgres => POSTGRES_DIR,
        }
    }
}

impl BackupRepository for FileSystemBackupRepository {
    fn save(&self, backup_metadata: &BackupMetadata, backup: Backup) -> RepositoryResult<()> {
        let output_dir = Path::new(&self.target_directory)
            .join(self.backup_directory(&backup_metadata.backup_target.kind))
            .join(self.to_file_name(&backup_metadata));

        info!(
            "Saving {} backup in {:?}...",
            backup_metadata.backup_target.name, output_dir
        );

        if let Some(parent) = output_dir.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut output_file = File::create(output_dir)?;
        output_file.write_all(&backup)?;
        output_file.flush()?;
        Ok(())
    }

    fn find_by_metadata(
        &self,
        backup_location: &BackupMetadata,
    ) -> RepositoryResult<Option<Backup>> {
        let path = Path::new(&self.target_directory)
            .join(self.backup_directory(&backup_location.backup_target.kind))
            .join(self.to_file_name(&backup_location));

        info!("Loading backup from file {:?}...", path);
        match std::fs::read(path) {
            Ok(file_content) => Ok(Some(file_content)),
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    return Ok(None);
                }
                return Err(RepositoryError::from(err))?;
            }
        }
    }
}

impl From<rusqlite::Error> for RepositoryError {
    fn from(value: rusqlite::Error) -> Self {
        Self::new_unknown(value)
    }
}

impl From<ParseIntError> for RepositoryError {
    fn from(value: ParseIntError) -> Self {
        Self::new_unknown(value)
    }
}

impl From<std::io::Error> for RepositoryError {
    fn from(value: std::io::Error) -> Self {
        Self::new_unknown(value)
    }
}

impl From<EnumNotFoundError> for RepositoryError {
    fn from(value: EnumNotFoundError) -> Self {
        Self::new_unknown(value)
    }
}

impl EnumStringSQLForm for BackupTargetKind {
    fn to_sql(&self) -> &str {
        match self {
            BackupTargetKind::MongoDB => "MONGODB",
            BackupTargetKind::Postgres => "POSTGRES",
        }
    }

    fn from_sql(sql: String) -> Result<Self, EnumNotFoundError> {
        match sql.as_str() {
            "MONGODB" => Ok(BackupTargetKind::MongoDB),
            "POSTGRES" => Ok(BackupTargetKind::Postgres),
            _ => Err(EnumNotFoundError(sql)),
        }
    }
}

impl EnumStringSQLForm for BackupType {
    fn to_sql(&self) -> &str {
        match self {
            BackupType::Manual => "MANUAL",
            BackupType::Scheduled => "SCHEDULED",
        }
    }

    fn from_sql(sql: String) -> Result<Self, EnumNotFoundError> {
        match sql.as_str() {
            "MANUAL" => Ok(BackupType::Manual),
            "SCHEDULED" => Ok(BackupType::Scheduled),
            _ => Err(EnumNotFoundError(sql)),
        }
    }
}

impl EnumStringSQLForm for BackupFormat {
    fn to_sql(&self) -> &str {
        match self {
            BackupFormat::ArchiveGz => "ARCHIVE_GZ",
            BackupFormat::TarGz => "TAR_GZ",
        }
    }

    fn from_sql(sql: String) -> Result<Self, EnumNotFoundError> {
        match sql.as_str() {
            "ARCHIVE_GZ" => Ok(BackupFormat::ArchiveGz),
            "TAR_GZ" => Ok(BackupFormat::TarGz),
            _ => Err(EnumNotFoundError(sql)),
        }
    }
}
