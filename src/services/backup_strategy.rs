use std::path::Path;
use std::process::Stdio;
use std::{fs, process::Command};

use crate::model::{Backup, BackupFormat};
use crate::process::IntoResult;
use anyhow::{anyhow, Error, Result};
use flate2::write::{GzDecoder, GzEncoder};
use flate2::Compression;
use log::info;
use std::io::Write;
use url::Url;
use urlencoding::decode;

pub trait BackupStrategy: Send + Sync {
    fn is_target_healthy(&self, connection_string: &str) -> Result<bool>;

    fn create_backup(&self, connection_string: &str) -> Result<(Backup, BackupFormat)>;

    fn restore_backup(&self, connection_string: &str, drop: bool, backup: Backup) -> Result<()>;
}

static FILE_NAME_CHARACTERS: u32 = 16;

pub struct MongoDBCompressedBackupStrategy {
    mongodump_config_file_folder: String,
}

struct ConfigFileRAII {
    path: String,
}

impl Drop for ConfigFileRAII {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl MongoDBCompressedBackupStrategy {
    pub fn new(mongodump_config_file_folder: String) -> Result<Self> {
        fs::create_dir_all(&mongodump_config_file_folder)?;
        Ok(MongoDBCompressedBackupStrategy {
            mongodump_config_file_folder,
        })
    }

    fn create_config_file(&self, connection_string: &str) -> Result<ConfigFileRAII> {
        let file_name = self.random_file_name();
        let mongodump_config_file_path =
            Path::new(&self.mongodump_config_file_folder).join(file_name);
        let file_content = format!("uri: {connection_string}");

        fs::write(&mongodump_config_file_path, file_content)?;

        let config_file = ConfigFileRAII {
            path: mongodump_config_file_path.to_str().unwrap().to_string(),
        };
        Ok(config_file)
    }

    fn random_file_name(&self) -> String {
        let file_name = (0..FILE_NAME_CHARACTERS)
            .into_iter()
            .map(|_| rand::random::<char>())
            .collect::<String>();
        format!("{file_name}.yaml")
    }
}

impl BackupStrategy for MongoDBCompressedBackupStrategy {
    fn is_target_healthy(&self, connection_string: &str) -> Result<bool> {
        info!("Checking MongoDB target health...");
        let output = Command::new("mongosh")
            .args(["--eval", "db.adminCommand('ping')", connection_string])
            .output()?;

        Ok(output.status.success())
    }

    fn create_backup(&self, connection_string: &str) -> Result<(Backup, BackupFormat)> {
        let config_file = self.create_config_file(connection_string)?;
        info!("Created temporary mongodump config file. Starting dump process...");
        let output = Command::new("mongodump")
            .args(["--config", &config_file.path, "--gzip", "--archive"])
            .stderr(Stdio::piped())
            .output()?
            .into_result()?;
        let blob = output.stdout;
        Ok((blob, BackupFormat::ArchiveGz))
    }

    fn restore_backup(&self, connection_string: &str, drop: bool, backup: Backup) -> Result<()> {
        let config_file = self.create_config_file(connection_string)?;
        let mut args = vec!["--config", &config_file.path, "--gzip", "--archive"];
        if drop {
            args.push("--drop");
        }

        let mut child = Command::new("mongorestore")
            .args(args)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        child.stdin.take().unwrap().write_all(&backup)?;

        let _ = child.wait_with_output()?.into_result()?;
        Ok(())
    }
}

struct PostgresOptions {
    password: String,
    username: String,
    port: u32,
    host: String,
    database: String,
}

pub struct PostgresCompressedBackupStrategy {}

impl PostgresCompressedBackupStrategy {
    pub fn new() -> Self {
        Self {}
    }

    fn parse_connection_string(&self, connection_string: &str) -> Result<PostgresOptions> {
        let url = Url::parse(connection_string)?;
        let password = url.password().ok_or(anyhow!(
            "Missing password in the postgres connection string"
        ))?;
        let password = decode(password)
            .map_err(|e| {
                Error::from(e).context("Failed to URL decode password from the connection string")
            })?
            .into_owned();

        let username = url.username();
        let username = decode(username)
            .map_err(|e| {
                Error::from(e).context("Failed to URL decode username from the connection string")
            })?
            .into_owned();

        let port = url
            .port()
            .ok_or(anyhow!("Missing port in the postgres connection string"))?;

        let host = url
            .host_str()
            .ok_or(anyhow!("Missing host in the postgres connection string"))?;
        let host = decode(host)
            .map_err(|e| {
                Error::from(e).context("Failed to URL decode host from the connection string")
            })?
            .into_owned();

        let path = decode(url.path())
            .map_err(|e| {
                Error::from(e)
                    .context("Failed to URL decode database name from the connection string")
            })?
            .into_owned();
        let database = path.strip_prefix("/").ok_or(anyhow!(
            "Missing database name in the postgres connection string"
        ))?;

        Ok(PostgresOptions {
            password,
            username,
            port: port.into(),
            host,
            database: database.to_string(),
        })
    }
}

impl BackupStrategy for PostgresCompressedBackupStrategy {
    fn is_target_healthy(&self, connection_string: &str) -> Result<bool> {
        let pg_dump_options = self.parse_connection_string(connection_string)?;

        let process_output = Command::new("psql")
            .args([
                "--username",
                &pg_dump_options.username,
                "--port",
                &pg_dump_options.port.to_string(),
                "--host",
                &pg_dump_options.host,
                "--dbname",
                &pg_dump_options.database,
                "--command",
                "SELECT 1;",
            ])
            .env("PGPASSWORD", &pg_dump_options.password)
            .output()?;

        Ok(process_output.status.success())
    }

    fn create_backup(&self, connection_string: &str) -> Result<(Backup, BackupFormat)> {
        let pg_dump_options = self.parse_connection_string(connection_string)?;

        let process_output = Command::new("pg_dump")
            .args([
                "--username",
                &pg_dump_options.username,
                "--port",
                &pg_dump_options.port.to_string(),
                "--host",
                &pg_dump_options.host,
                "--format",
                "tar",
                &pg_dump_options.database,
            ])
            .env("PGPASSWORD", &pg_dump_options.password)
            .stderr(Stdio::piped())
            .output()?
            .into_result()?;

        let database_dump = process_output.stdout;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(database_dump.as_slice())?;
        let compressed_dump = encoder.finish()?;
        Ok((compressed_dump, BackupFormat::TarGz))
    }

    fn restore_backup(&self, connection_string: &str, drop: bool, backup: Backup) -> Result<()> {
        let pg_restore_options = self.parse_connection_string(connection_string)?;
        let port = pg_restore_options.port.to_string();

        let mut args = vec![
            "--username",
            &pg_restore_options.username,
            "--port",
            &port,
            "--host",
            &pg_restore_options.host,
            "--dbname",
            &pg_restore_options.database,
            "--single-transaction",
        ];
        if drop {
            args.extend_from_slice(&["--clean", "--if-exists"]);
        }

        let mut decoder = GzDecoder::new(Vec::new());
        decoder.write_all(&backup)?;
        let decompressed_backup = decoder.finish()?;

        let mut process = Command::new("pg_restore")
            .args(args)
            .env("PGPASSWORD", &pg_restore_options.password)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        process
            .stdin
            .take()
            .unwrap()
            .write_all(&decompressed_backup)?;
        let _ = process.wait_with_output()?.into_result()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_parse_connection_string_plaintext() {
        let strategy = PostgresCompressedBackupStrategy::new();
        let conn_str = "postgres://user:password@localhost:5432/mydb";
        let options = strategy.parse_connection_string(conn_str).unwrap();

        assert_eq!(options.username, "user");
        assert_eq!(options.password, "password");
        assert_eq!(options.host, "localhost");
        assert_eq!(options.port, 5432);
        assert_eq!(options.database, "mydb");
    }

    #[test]
    fn should_parse_connection_string_and_decode_all_components() {
        let strategy = PostgresCompressedBackupStrategy::new();
        let conn_str = "postgres://us%40er:pa%24%24word@local%68ost:5432/my%2Fdb";
        let options = strategy.parse_connection_string(conn_str).unwrap();

        assert_eq!(options.username, "us@er");
        assert_eq!(options.password, "pa$$word");
        assert_eq!(options.host, "localhost");
        assert_eq!(options.port, 5432);
        assert_eq!(options.database, "my/db");
    }
}
