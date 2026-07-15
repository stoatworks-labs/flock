use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::Json;
use serde::{Deserialize, Serialize};

use flock_core::{Device, DeviceCredentials, DeviceId, DeviceMode};

use crate::error::{parse_device_id, ApiError};
use crate::state::AppState;

#[derive(Serialize)]
pub struct StateResponse {
    pub devices: Vec<Device>,
    pub groups: std::collections::BTreeMap<String, Vec<DeviceId>>,
}

pub fn build_state_response(state: &AppState) -> StateResponse {
    StateResponse {
        devices: state
            .registry
            .list()
            .into_iter()
            .map(Device::redacted)
            .collect(),
        groups: state.registry.groups(),
    }
}

pub async fn get_state(State(state): State<AppState>) -> Json<StateResponse> {
    Json(build_state_response(&state))
}

#[derive(Deserialize)]
pub struct DeviceRequest {
    pub name: String,
    pub host: String,
    pub mode: DeviceMode,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub discovered: bool,
}

pub async fn create_device(
    State(state): State<AppState>,
    Json(body): Json<DeviceRequest>,
) -> Result<Json<Device>, ApiError> {
    let device = Device {
        id: DeviceId::new(),
        name: body.name,
        host: body.host,
        mode: body.mode,
        tags: body.tags,
        credentials: DeviceCredentials {
            password: body.password,
        },
        discovered: body.discovered,
    };
    state.registry.upsert(device.clone())?;
    Ok(Json(device.redacted()))
}

pub async fn update_device(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DeviceRequest>,
) -> Result<Json<Device>, ApiError> {
    let id = parse_device_id(&id)?;
    let mut device = state.registry.get(&id).ok_or(ApiError::NotFound)?;
    device.name = body.name;
    device.host = body.host;
    device.mode = body.mode;
    device.tags = body.tags;
    if let Some(password) = body.password {
        if !password.is_empty() {
            device.credentials.password = Some(password);
        }
    }
    state.registry.upsert(device.clone())?;
    Ok(Json(device.redacted()))
}

pub async fn delete_device(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<()>, ApiError> {
    let id = parse_device_id(&id)?;
    state.registry.remove(&id)?.ok_or(ApiError::NotFound)?;
    Ok(Json(()))
}

async fn resolve(
    state: &AppState,
    id: &str,
) -> Result<std::sync::Arc<dyn flock_core::DeviceClient>, ApiError> {
    let id = parse_device_id(id)?;
    let device = state.registry.get(&id).ok_or(ApiError::NotFound)?;
    Ok(state.provider.client_for(&device))
}

pub async fn get_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<flock_core::DeviceStatus>, ApiError> {
    let client = resolve(&state, &id).await?;
    Ok(Json(client.status().await?))
}

pub async fn get_network(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<flock_core::NetworkSettings>, ApiError> {
    let client = resolve(&state, &id).await?;
    Ok(Json(client.network_settings().await?))
}

pub async fn set_network(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<flock_core::NetworkSettings>,
) -> Result<Json<()>, ApiError> {
    let client = resolve(&state, &id).await?;
    client.set_network_settings(body).await?;
    Ok(Json(()))
}

pub async fn get_encode(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<flock_core::EncodeSettings>, ApiError> {
    let client = resolve(&state, &id).await?;
    Ok(Json(client.encode_settings().await?))
}

pub async fn set_encode(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<flock_core::EncodeSettings>,
) -> Result<Json<()>, ApiError> {
    let client = resolve(&state, &id).await?;
    client.set_encode_settings(body).await?;
    Ok(Json(()))
}

pub async fn get_decode(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<flock_core::DecodeSettings>, ApiError> {
    let client = resolve(&state, &id).await?;
    Ok(Json(client.decode_settings().await?))
}

pub async fn set_decode(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<flock_core::DecodeSettings>,
) -> Result<Json<()>, ApiError> {
    let client = resolve(&state, &id).await?;
    client.set_decode_settings(body).await?;
    Ok(Json(()))
}

pub async fn get_system(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<flock_core::SystemSettings>, ApiError> {
    let client = resolve(&state, &id).await?;
    Ok(Json(client.system_settings().await?))
}

pub async fn set_system(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<flock_core::SystemSettings>,
) -> Result<Json<()>, ApiError> {
    let client = resolve(&state, &id).await?;
    client.set_system_settings(body).await?;
    Ok(Json(()))
}

pub async fn reboot_device(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<()>, ApiError> {
    let client = resolve(&state, &id).await?;
    client.reboot().await?;
    Ok(Json(()))
}

pub async fn scan_discovery(
    State(state): State<AppState>,
) -> Result<Json<Vec<flock_discovery::DiscoveredHost>>, ApiError> {
    let found = state.discovery.scan(Duration::from_secs(3)).await?;
    let known_hosts: std::collections::HashSet<String> =
        state.registry.list().into_iter().map(|d| d.host).collect();
    let unadded = found
        .into_iter()
        .filter(|h| !known_hosts.contains(&h.host))
        .collect();
    Ok(Json(unadded))
}
