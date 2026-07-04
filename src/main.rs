mod app_config;
mod connection_pool;
mod endpoints;
mod errorstack;
mod file_system_repositories;
mod jobs;
mod lock;
mod model;
mod process;
mod services;

use anyhow::Result;
use app_config::AppConfig;
use connection_pool::ConnectionPool;
use dotenv;
use endpoints::router_builder;
use file_system_repositories::{FileSystemBackupRepository, SQLiteBackupMetadataRepository};
use jobs::{CronJobs, ScheduledBackupJob};
use lock::LockManager;
use migrations::{MigrationRunner, MigrationRunnerConfiguration, SqliteClientAdapter};
use services::{
    BackuppingService, BackuppingServiceImpl, MongoDBCompressedBackupStrategy,
    PostgresCompressedBackupStrategy,
};
use std::sync::{atomic::AtomicBool, Arc};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    signal_hook::flag::register_conditional_shutdown(
        signal_hook::consts::SIGTERM,
        0,
        Arc::new(AtomicBool::new(true)),
    )?;
    dotenv::read_env_file();
    env_logger::init();

    let app_config = AppConfig::create_from_environment();

    let sqlite_connection_pool =
        ConnectionPool::new(&app_config.db_path, app_config.db_connection_pool_size)?;

    {
        let connection = sqlite_connection_pool.get_random_connection();
        let sqlite_adapter = SqliteClientAdapter::new(&connection);
        let migrator =
            MigrationRunner::new(MigrationRunnerConfiguration::default(), sqlite_adapter);
        migrator.run_migrations().await?;
    }

    let lock_manager = Arc::new(LockManager::new(app_config.locks_directory)?);
    let backup_repository = Arc::new(FileSystemBackupRepository::new(app_config.target_directory));
    let backup_metadata_repoository =
        Arc::new(SQLiteBackupMetadataRepository::new(sqlite_connection_pool).unwrap());

    let mongodb_strategy = Arc::new(MongoDBCompressedBackupStrategy::new(
        app_config.mongodump_config_file_path,
    )?);
    let postgres_strategy = Arc::new(PostgresCompressedBackupStrategy::new());

    let backupping_service: Arc<dyn BackuppingService> = Arc::new(BackuppingServiceImpl::new(
        lock_manager.clone(),
        backup_repository.clone(),
        backup_metadata_repoository.clone(),
        mongodb_strategy,
        postgres_strategy,
        app_config.backup_targets,
    ));

    let mut cron_jobs = CronJobs::new();
    for job in app_config.cyclic_backups {
        let task = ScheduledBackupJob::new(job.target_name, backupping_service.clone());
        cron_jobs.start(job.cron_schedule, task)?;
    }

    let router = router_builder(backupping_service.clone());

    let bind_address = format!("0.0.0.0:{}", app_config.server_port);
    let listener = TcpListener::bind(bind_address).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
