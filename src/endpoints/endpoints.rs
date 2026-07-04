use super::api_model::ArchiveBackupResponse;
use crate::{
    endpoints::api_model::{ApiError, BackupHealthCheckResponse, BackupTargetResponse, ErrorCode},
    model::BackupType,
    services::{BackupCreateError, BackupHealthCheckError, BackupRestoreError, BackuppingService},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use log::{error, info, warn};
use std::{collections::HashMap, sync::Arc};
use url::Url;

async fn backups_create(
    State(backupping_service): State<Arc<dyn BackuppingService>>,
    Path(target_name): Path<String>,
) -> Result<(StatusCode, Json<ArchiveBackupResponse>), (StatusCode, Json<ApiError>)> {
    match backupping_service.create_backup(&target_name, BackupType::Manual) {
        Ok(backup) => {
            let response = ArchiveBackupResponse::from(backup);
            Ok((StatusCode::CREATED, Json(response)))
        }
        Err(err) => match err {
            BackupCreateError::BackupTargetLocked(_) => {
                info!("Abandoning backup - the target is locked");
                let response = ApiError {
                    error_code: ErrorCode::BackupTargetLocked,
                    message: format!("{}", err),
                };
                Err((StatusCode::LOCKED, Json(response)))
            }
            BackupCreateError::BackupTargetNotFound(_) => {
                info!("Abandoning backup - did not find the target");
                let response = ApiError {
                    error_code: ErrorCode::BackupTargetNotFound,
                    message: format!("{}", err),
                };
                Err((StatusCode::NOT_FOUND, Json(response)))
            }
            BackupCreateError::Unknown(_) => {
                error!("Abandoning backup. Error:\n{:?}", err);
                let response = ApiError {
                    error_code: ErrorCode::InternalError,
                    message: format!("{}", err),
                };
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response)))
            }
        },
    }
}

async fn backups_read_all(
    State(backupping_service): State<Arc<dyn BackuppingService>>,
) -> Result<Json<Vec<ArchiveBackupResponse>>, (StatusCode, Json<ApiError>)> {
    info!("Got request to list all backups");
    match backupping_service.read_all_backups() {
        Ok(backups) => {
            let response: Vec<ArchiveBackupResponse> = backups
                .into_iter()
                .map(|backup| ArchiveBackupResponse::from(backup))
                .collect();
            Ok(Json(response))
        }
        Err(err) => {
            error!("An error has occurred when listing backups.\n{:?}", err);
            let response_body = ApiError {
                error_code: ErrorCode::InternalError,
                message: format!("{}", err),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response_body)))
        }
    }
}

async fn configured_targets_check_is_healthy(
    State(backupping_service): State<Arc<dyn BackuppingService>>,
    Path(target_name): Path<String>,
) -> Result<Json<BackupHealthCheckResponse>, (StatusCode, Json<ApiError>)> {
    match backupping_service.check_if_target_is_healthy(&target_name) {
        Ok(is_healthy) => Ok(Json(BackupHealthCheckResponse { is_healthy })),
        Err(err) => match &err {
            BackupHealthCheckError::BackupTargetNotFound { .. } => {
                let error = ApiError {
                    error_code: ErrorCode::BackupTargetNotFound,
                    message: format!("{}", err),
                };
                Err((StatusCode::NOT_FOUND, Json(error)))
            }
            BackupHealthCheckError::Unknown(_) => {
                let error = ApiError {
                    error_code: ErrorCode::InternalError,
                    message: format!("{}", err),
                };
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
            }
            BackupHealthCheckError::BackupTargetLocked { .. } => {
                let error = ApiError {
                    error_code: ErrorCode::BackupTargetLocked,
                    message: format!("{}", err),
                };
                Err((StatusCode::LOCKED, Json(error)))
            }
        },
    }
}

async fn configured_targets_read_all(
    State(backupping_service): State<Arc<dyn BackuppingService>>,
) -> Json<Vec<BackupTargetResponse>> {
    let targets = backupping_service.read_all_configured_targets();
    let response_body: Vec<BackupTargetResponse> = targets
        .iter()
        .map(|target| {
            let host = Url::parse(&target.connection_string)
                .ok()
                .and_then(|uri| uri.host_str().map(|host| host.to_string()));
            BackupTargetResponse {
                host,
                name: target.target_name.clone(),
                kind: target.target_kind.clone().into(),
            }
        })
        .collect();

    Json(response_body)
}

async fn configured_targets_restore_backup(
    State(backupping_service): State<Arc<dyn BackuppingService>>,
    Path((target_name, backup_id)): Path<(String, u64)>,
    Query(query_params): Query<HashMap<String, String>>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let drop = query_params
        .get("drop")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    if drop {
        warn!(
            r#""drop" parameter is present and set to true - restoring backup will perform drop operation"#
        );
    }
    info!("Starting restore backup procedure. Requested backup ID = {backup_id}");
    match backupping_service.restore_backup(&target_name, backup_id, drop) {
        Ok(()) => Ok(StatusCode::OK),
        Err(err) => match &err {
            BackupRestoreError::BackupTargetLocked { name } => {
                warn!("Backup target {} is locked", name);
                let response_body = ApiError {
                    error_code: ErrorCode::BackupTargetLocked,
                    message: format!("{}", err),
                };
                Err((StatusCode::LOCKED, Json(response_body)))
            }
            BackupRestoreError::BackupTargetNotFound { name: _ } => {
                warn!("{}", err);
                let response_body = ApiError {
                    error_code: ErrorCode::BackupTargetNotFound,
                    message: format!("{}", err),
                };
                Err((StatusCode::NOT_FOUND, Json(response_body)))
            }
            BackupRestoreError::BackupDoesNotExist(_) => {
                warn!("Backup does not exist. Backup ID = {}", backup_id);
                let response_body = ApiError {
                    error_code: ErrorCode::BackupNotFound,
                    message: format!("{}", err),
                };
                Err((StatusCode::NOT_FOUND, Json(response_body)))
            }
            _ => {
                error!(
                    "Unexpected error has occurred when restoring the backup.\n{:?}",
                    err
                );
                let response_body = ApiError {
                    error_code: ErrorCode::InternalError,
                    message: format!("{}", err),
                };
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(response_body)))
            }
        },
    }
}

async fn healthy() -> &'static str {
    "OK"
}

pub fn router_builder(backupping_service: Arc<dyn BackuppingService>) -> Router {
    Router::new()
        .route("/api/v1/targets", get(configured_targets_read_all))
        .route(
            "/api/v1/targets/{target_name}/backups/{backup_id}",
            post(configured_targets_restore_backup),
        )
        .route(
            "/api/v1/targets/{target_name}/health",
            post(configured_targets_check_is_healthy),
        )
        .route("/api/v1/backups", get(backups_read_all))
        .route("/api/v1/backups/{target_name}", post(backups_create))
        .route("/internal/status/health", get(healthy))
        .with_state(backupping_service)
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use super::*;
    use crate::{
        lock::LockError,
        model::{
            BackupFormat, BackupMetadata, BackupTarget, BackupTargetKind, ConfiguredBackupTarget,
        },
        services::BackupFindError,
    };
    use axum_test::TestServer;
    use chrono::DateTime;
    use serde_json::json;

    struct BackuppingServiceMock {
        create_backup_mock:
            Option<fn(&str, BackupType) -> Result<BackupMetadata, BackupCreateError>>,
        read_all_backups_mock: Option<fn() -> Result<Vec<BackupMetadata>, BackupFindError>>,
        restore_backup_mock: Option<fn(&str, u64, bool) -> Result<(), BackupRestoreError>>,
        configured_targets: Option<Vec<ConfiguredBackupTarget>>,
        check_if_target_is_healthy_mock: Option<fn(&str) -> Result<bool, BackupHealthCheckError>>,
    }

    impl BackuppingServiceMock {
        fn new() -> Self {
            Self {
                create_backup_mock: None,
                read_all_backups_mock: None,
                restore_backup_mock: None,
                configured_targets: None,
                check_if_target_is_healthy_mock: None,
            }
        }

        fn with_create_backup_mock(
            mut self,
            mock: fn(&str, BackupType) -> Result<BackupMetadata, BackupCreateError>,
        ) -> Self {
            self.create_backup_mock = Some(mock);
            self
        }

        fn with_read_all_backups_mock(
            mut self,
            mock: fn() -> Result<Vec<BackupMetadata>, BackupFindError>,
        ) -> Self {
            self.read_all_backups_mock = Some(mock);
            self
        }

        fn with_restore_backup_mock(
            mut self,
            mock: fn(&str, u64, bool) -> Result<(), BackupRestoreError>,
        ) -> Self {
            self.restore_backup_mock = Some(mock);
            self
        }

        fn with_configured_targets(mut self, targets: Vec<ConfiguredBackupTarget>) -> Self {
            self.configured_targets = Some(targets);
            self
        }

        fn with_check_if_target_is_healthy_mock(
            mut self,
            mock: fn(&str) -> Result<bool, BackupHealthCheckError>,
        ) -> Self {
            self.check_if_target_is_healthy_mock = Some(mock);
            self
        }
    }

    impl BackuppingService for BackuppingServiceMock {
        fn create_backup(
            &self,
            target_name: &str,
            backup_type: BackupType,
        ) -> Result<BackupMetadata, BackupCreateError> {
            (self
                .create_backup_mock
                .expect("You did not mock create_backup method"))(
                target_name, backup_type
            )
        }

        fn read_all_backups(&self) -> Result<Vec<BackupMetadata>, BackupFindError> {
            (self
                .read_all_backups_mock
                .expect("You did not mock read_all_backups method"))()
        }

        fn restore_backup(
            &self,
            target_name: &str,
            backup_id: u64,
            drop: bool,
        ) -> Result<(), crate::services::BackupRestoreError> {
            (self
                .restore_backup_mock
                .expect("You did not mock restore_backup method"))(
                target_name, backup_id, drop
            )
        }

        fn read_all_configured_targets(&self) -> &Vec<ConfiguredBackupTarget> {
            self.configured_targets
                .as_ref()
                .expect("You did not mock read_all_configured_targets method")
        }

        fn check_if_target_is_healthy(
            &self,
            target_name: &str,
        ) -> Result<bool, crate::services::BackupHealthCheckError> {
            (self
                .check_if_target_is_healthy_mock
                .expect("You did not mock check_if_target_is_healthy method"))(
                target_name
            )
        }
    }

    #[tokio::test]
    async fn should_return_backup_metadata_when_backup_was_created() {
        // given
        let service = Arc::new(BackuppingServiceMock::new().with_create_backup_mock(
            |target_name, backup_type| {
                assert_eq!(backup_type, BackupType::Manual);
                assert_eq!(target_name, "testTarget");
                Ok(BackupMetadata {
                    backup_id: 1,
                    created_at: DateTime::from_str("2023-03-15T12:00:00Z").unwrap(),
                    backup_size_bytes: 1024,
                    backup_target: BackupTarget {
                        kind: BackupTargetKind::MongoDB,
                        name: "test".into(),
                    },
                    backup_type: BackupType::Manual,
                    backup_format: BackupFormat::ArchiveGz,
                })
            },
        ));

        let test_server = TestServer::new(router_builder(service)).unwrap();

        // when
        let response = test_server.post("/api/v1/backups/testTarget").await;

        // then
        response.assert_status(StatusCode::CREATED);
        response.assert_header("content-type", "application/json");
        response.assert_json(&json!({
            "backup_id": 1,
            "created_at": "2023-03-15T12:00:00Z",
            "backup_size_bytes": 1024,
            "backup_target": {
                "kind": "MONGODB",
                "name": "test"
            },
            "backup_type": "MANUAL",
            "backup_format": "ARCHIVE_GZ"
        }));
    }

    #[tokio::test]
    async fn should_return_423_when_backup_is_in_progress() {
        // given
        let service = Arc::new(BackuppingServiceMock::new().with_create_backup_mock(
            |target_name, backup_type| {
                assert_eq!(backup_type, BackupType::Manual);
                Err(BackupCreateError::BackupTargetLocked(
                    target_name.to_string(),
                ))
            },
        ));

        let test_server = TestServer::new(router_builder(service)).unwrap();

        // when
        let response = test_server.post("/api/v1/backups/testTarget").await;

        // then
        response.assert_status(StatusCode::LOCKED);
        response.assert_header("content-type", "application/json");
        response.assert_json(&json!({
            "error_code": "BACKUP_TARGET_LOCKED",
            "message": "Backup target testTarget is undergoing another operation."
        }));
    }

    #[tokio::test]
    async fn should_return_health_response() {
        // given
        let backupping_service_mock = BackuppingServiceMock::new()
            .with_check_if_target_is_healthy_mock(|target_name| {
                assert_eq!(target_name, "testTarget");
                Ok(true)
            });

        let server = TestServer::new(router_builder(Arc::new(backupping_service_mock))).unwrap();

        // when
        let response = server.post("/api/v1/targets/testTarget/health").await;

        // then
        response.assert_status(StatusCode::OK);
        response.assert_json(&json!({
            "is_healthy": true
        }));
    }

    #[tokio::test]
    async fn should_return_lock_status_code_when_target_is_locked() {
        // given
        let backupping_service_mock = BackuppingServiceMock::new()
            .with_check_if_target_is_healthy_mock(|target_name| {
                assert_eq!(target_name, "testTarget");
                Err(BackupHealthCheckError::BackupTargetLocked {
                    name: "testTarget".into(),
                    cause: LockError::LockAlreadyExists("testTarget".into()),
                })
            });

        let server = TestServer::new(router_builder(Arc::new(backupping_service_mock))).unwrap();

        // when
        let response = server.post("/api/v1/targets/testTarget/health").await;

        // then
        response.assert_status(StatusCode::LOCKED);
        response.assert_json(&json!({
            "error_code": "BACKUP_TARGET_LOCKED",
            "message": "Backup target testTarget is undergoing another operation."
        }));
    }

    #[tokio::test]
    async fn should_return_healthy_status() {
        // given
        let service = Arc::new(BackuppingServiceMock::new());
        let test_server = TestServer::new(router_builder(service)).unwrap();

        // when
        let response = test_server.get("/internal/status/health").await;

        // then
        response.assert_status(StatusCode::OK);
        response.assert_text("OK");
    }

    #[tokio::test]
    async fn should_restore_backup_without_drop() {
        // given
        let service = Arc::new(BackuppingServiceMock::new().with_restore_backup_mock(
            |target_name, backup_id, drop| {
                assert_eq!(target_name, "testTarget");
                assert_eq!(backup_id, 123);
                assert_eq!(drop, false);
                Ok(())
            },
        ));

        let test_server = TestServer::new(router_builder(service)).unwrap();

        // when
        let response = test_server
            .post("/api/v1/targets/testTarget/backups/123")
            .await;

        // then
        response.assert_status(StatusCode::OK);
    }

    #[tokio::test]
    async fn should_restore_backup_with_drop() {
        // given
        let service = Arc::new(BackuppingServiceMock::new().with_restore_backup_mock(
            |target_name, backup_id, drop| {
                assert_eq!(target_name, "testTarget");
                assert_eq!(backup_id, 123);
                assert_eq!(drop, true);
                Ok(())
            },
        ));

        let test_server = TestServer::new(router_builder(service)).unwrap();

        // when
        let response = test_server
            .post("/api/v1/targets/testTarget/backups/123?drop=true")
            .await;

        // then
        response.assert_status(StatusCode::OK);
    }

    #[tokio::test]
    async fn should_return_all_configured_targets() {
        // given
        let service = Arc::new(BackuppingServiceMock::new().with_configured_targets(vec![
            ConfiguredBackupTarget {
                target_name: "testTarget1".into(),
                target_kind: BackupTargetKind::MongoDB,
                connection_string: "mongodb://localhost:27017".into(),
            },
            ConfiguredBackupTarget {
                target_name: "testTarget2".into(),
                target_kind: BackupTargetKind::Postgres,
                connection_string: "postgresql://user:password@myremote:5432/db".into(),
            },
        ]));

        let test_server = TestServer::new(router_builder(service)).unwrap();

        // when
        let response = test_server.get("/api/v1/targets").await;

        // then
        response.assert_status(StatusCode::OK);
        response.assert_json(&json!([
            {
                "name": "testTarget1",
                "kind": "MONGODB",
                "host": "localhost"
            },
            {
                "name": "testTarget2",
                "kind": "POSTGRES",
                "host": "myremote"
            }
        ]));
    }

    #[tokio::test]
    async fn should_return_all_backups() {
        // given
        let service = Arc::new(BackuppingServiceMock::new().with_read_all_backups_mock(|| {
            Ok(vec![
                BackupMetadata {
                    backup_id: 1,
                    created_at: DateTime::from_str("2023-03-15T12:00:00Z").unwrap(),
                    backup_size_bytes: 1024,
                    backup_target: BackupTarget {
                        kind: BackupTargetKind::MongoDB,
                        name: "testTarget1".into(),
                    },
                    backup_type: BackupType::Manual,
                    backup_format: BackupFormat::ArchiveGz,
                },
                BackupMetadata {
                    backup_id: 2,
                    created_at: DateTime::from_str("2023-03-16T12:00:00Z").unwrap(),
                    backup_size_bytes: 2048,
                    backup_target: BackupTarget {
                        kind: BackupTargetKind::Postgres,
                        name: "testTarget2".into(),
                    },
                    backup_type: BackupType::Scheduled,
                    backup_format: BackupFormat::ArchiveGz,
                },
            ])
        }));

        let test_server = TestServer::new(router_builder(service)).unwrap();

        // when
        let response = test_server.get("/api/v1/backups").await;

        // then
        response.assert_status(StatusCode::OK);
        response.assert_json(&json!([
            {
                "backup_id": 1,
                "created_at": "2023-03-15T12:00:00Z",
                "backup_size_bytes": 1024,
                "backup_target": {
                    "kind": "MONGODB",
                    "name": "testTarget1"
                },
                "backup_type": "MANUAL",
                "backup_format": "ARCHIVE_GZ"
            },
            {
                "backup_id": 2,
                "created_at": "2023-03-16T12:00:00Z",
                "backup_size_bytes": 2048,
                "backup_target": {
                    "kind": "POSTGRES",
                    "name": "testTarget2"
                },
                "backup_type": "SCHEDULED",
                "backup_format": "ARCHIVE_GZ"
            }
        ]));
    }
}
