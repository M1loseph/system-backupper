use crate::model::{BackupTargetKind, ConfiguredBackupTarget, CyclicBackup};

pub struct AppConfig {
    pub mongodump_config_file_path: String,
    pub locks_directory: String,
    pub target_directory: String,
    pub db_path: String,
    pub db_connection_pool_size: u32,
    pub server_port: u32,
    pub backup_targets: Vec<ConfiguredBackupTarget>,
    pub cyclic_backups: Vec<CyclicBackup>,
}

impl AppConfig {
    pub fn create_from_environment() -> Self {
        let mongodump_config_file_path = env::read_or_default(
            &Self::to_env_variable_name("MONGO_DUMP_CONFIG_FILE"),
            "local/working/config",
        );
        let locks_directory = env::read_or_default(
            &Self::to_env_variable_name("LOCKS_DIRECTORY"),
            "local/working/locks",
        );
        let target_directory = env::read_or_default(
            &Self::to_env_variable_name("TARGET_DIRECTORY"),
            "local/results",
        );
        let backup_targets = env::read(&Self::to_env_variable_name("BACKUP_TARGETS"));
        let backup_targets = Self::parse_backup_targets(backup_targets);
        let db_path = env::read_or_default(
            &Self::to_env_variable_name("DB_PATH"),
            "local/db/db.sqlite3",
        );
        let db_connection_pool_size =
            env::read_int_or_default(&Self::to_env_variable_name("DB_CONNECTION_POOL_SIZE"), 3);
        let server_port =
            env::read_int_or_default(&Self::to_env_variable_name("SERVER_PORT"), 2000);
        let cyclic_backups =
            env::read_or_default(&Self::to_env_variable_name("CYCLIC_BACKUPS"), "");
        let cyclic_backups = Self::parse_cyclic_backups(cyclic_backups);

        Self {
            mongodump_config_file_path,
            locks_directory,
            target_directory,
            db_path,
            db_connection_pool_size,
            server_port,
            backup_targets,
            cyclic_backups,
        }
    }

    fn to_env_variable_name(key_suffix: &str) -> String {
        format!("SB_{key_suffix}")
    }

    fn parse_cyclic_backups(env_variable: String) -> Vec<CyclicBackup> {
        env_variable
            .split(";")
            .filter(|part| !part.is_empty())
            .map(|part| part.split(","))
            .map(|parts| {
                let parts: Vec<&str> = parts.collect();
                if parts.len() != 2 {
                    panic!(
                        "Invalid number of coma-seperated elements in {}",
                        parts.join(",")
                    );
                }
                let target_name = parts[0].to_string();
                let cron = parts[1].to_string();
                CyclicBackup {
                    target_name,
                    cron_schedule: cron,
                }
            })
            .collect()
    }

    fn parse_backup_targets(env_variable: String) -> Vec<ConfiguredBackupTarget> {
        env_variable
            .split(";")
            .filter(|part| !part.is_empty())
            .map(|part| part.split(","))
            .map(|parts| {
                let parts: Vec<&str> = parts.collect();
                if parts.len() != 3 {
                    panic!(
                        "Invalid number of coma-seperated elements in {}",
                        parts.join(",")
                    );
                }
                let name = parts[0].to_string();
                let backup_target: BackupTargetKind = BackupTargetKind::from(parts[1]);
                let connection_string = parts[2].to_string();
                ConfiguredBackupTarget {
                    target_name: name,
                    target_kind: backup_target,
                    connection_string,
                }
            })
            .collect()
    }
}

impl From<&str> for BackupTargetKind {
    fn from(value: &str) -> Self {
        match value {
            "MongoDB" => BackupTargetKind::MongoDB,
            "Postgres" => BackupTargetKind::Postgres,
            _ => panic!("Unexpected backup target {}", value),
        }
    }
}

mod env {
    use std::env::VarError;

    pub fn read(key: &str) -> String {
        read_env_variable(key)
            .unwrap_or_else(|| panic!("Missing required environment variable {key}"))
    }

    pub fn read_or_default(key: &str, default: &str) -> String {
        read_env_variable(key).unwrap_or_else(|| default.to_string())
    }

    pub fn read_int_or_default(key: &str, default: u32) -> u32 {
        read_env_variable(key)
            .map(|value| {
                value.parse().unwrap_or_else(|_| {
                    panic!(
                "Failed to parse content of environment variable {}. Expected unsigned integer.",
                key
            )
                })
            })
            .unwrap_or(default)
    }

    fn read_env_variable(key: &str) -> Option<String> {
        match std::env::var(&key) {
            Ok(value) => Some(value),
            Err(err) => match err {
                VarError::NotPresent => None,
                VarError::NotUnicode(_) => {
                    panic!("Unable to decode value of environment variable {key}")
                }
            },
        }
    }
}
