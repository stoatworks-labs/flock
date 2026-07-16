use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::Json;
use serde::{Deserialize, Serialize};

use flock_core::{AppSettings, Device, DeviceCredentials, DeviceId};

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
    let found = state.discovery.scan().await?;
    let known_hosts: std::collections::HashSet<String> =
        state.registry.list().into_iter().map(|d| d.host).collect();
    let unadded = found
        .into_iter()
        .filter(|h| !known_hosts.contains(&h.host))
        .collect();
    Ok(Json(unadded))
}

/// flock's own centralized NDI source list (mDNS-based) - what the Decode
/// tab's source pickers suggest from, replacing having to query each
/// individual Play's own `:8080/List` endpoint just to see what's out
/// there. See docs/architecture.md.
pub async fn get_ndi_sources(
    State(state): State<AppState>,
) -> Result<Json<Vec<flock_discovery::NdiSource>>, ApiError> {
    Ok(Json(
        state.discovery.ndi_sources(Duration::from_secs(3)).await?,
    ))
}

// ---------- app settings ----------

pub async fn get_app_settings(State(state): State<AppState>) -> Json<AppSettings> {
    Json(state.app_settings.get())
}

pub async fn set_app_settings(
    State(state): State<AppState>,
    Json(settings): Json<AppSettings>,
) -> Result<Json<AppSettings>, ApiError> {
    state.app_settings.set(settings)?;
    Ok(Json(state.app_settings.get()))
}

/// Pushes the configured discovery server address out to *every*
/// registered device's Network settings (`ndi_discovery_server_ips`/
/// `_enabled`) - the one place flock's own knowledge of a discovery server
/// actually does something, since flock itself can't query that server
/// directly (no public protocol spec - see docs/architecture.md). Unlike
/// batch-edit-by-group, this always targets the whole fleet, since "all
/// devices" isn't a real tag to batch-edit against.
pub async fn push_discovery_server(
    State(state): State<AppState>,
) -> Result<Json<Vec<BatchOutcome>>, ApiError> {
    let server = state
        .app_settings
        .get()
        .discovery_server
        .ok_or_else(|| ApiError::BadRequest("no discovery server configured".to_string()))?;

    let devices = state.registry.list();
    let mut outcomes = Vec::with_capacity(devices.len());
    for device in devices {
        let client = state.provider.client_for(&device);
        let result = async {
            let mut settings = client.network_settings().await?;
            settings.ndi_discovery_server_enabled = true;
            settings.ndi_discovery_server_ips = vec![server.clone()];
            client.set_network_settings(settings).await
        }
        .await;
        outcomes.push(BatchOutcome {
            device_id: device.id,
            device_name: device.name,
            ok: result.is_ok(),
            error: result.err().map(|e| e.to_string()),
        });
    }
    Ok(Json(outcomes))
}

// ---------- batch group settings ----------
//
// Applies a partial patch (only the keys the caller actually sent) to every
// device in a tag-derived group, for one settings tab at a time. Each device
// keeps whatever it already had for fields the patch didn't mention - the
// merge happens against that device's own current settings, not a shared
// template, so a batch edit never clobbers per-device values it wasn't
// asked to change.

#[derive(Serialize)]
pub struct BatchOutcome {
    pub device_id: DeviceId,
    pub device_name: String,
    pub ok: bool,
    pub error: Option<String>,
}

pub async fn apply_group_settings(
    State(state): State<AppState>,
    Path((tag, tab)): Path<(String, String)>,
    Json(patch): Json<serde_json::Value>,
) -> Result<Json<Vec<BatchOutcome>>, ApiError> {
    let members = state
        .registry
        .groups()
        .get(&tag)
        .cloned()
        .unwrap_or_default();
    let mut outcomes = Vec::with_capacity(members.len());
    for id in members {
        let Some(device) = state.registry.get(&id) else {
            continue;
        };
        let client = state.provider.client_for(&device);
        let result = apply_patch_for_tab(client.as_ref(), &tab, &patch).await;
        outcomes.push(BatchOutcome {
            device_id: id,
            device_name: device.name,
            ok: result.is_ok(),
            error: result.err().map(|e| e.to_string()),
        });
    }
    Ok(Json(outcomes))
}

async fn apply_patch_for_tab(
    client: &dyn flock_core::DeviceClient,
    tab: &str,
    patch: &serde_json::Value,
) -> anyhow::Result<()> {
    match tab {
        "network" => {
            let mut current = serde_json::to_value(client.network_settings().await?)?;
            merge_json(&mut current, patch);
            client
                .set_network_settings(serde_json::from_value(current)?)
                .await
        }
        "decode" => {
            let mut current = serde_json::to_value(client.decode_settings().await?)?;
            merge_json(&mut current, patch);
            client
                .set_decode_settings(serde_json::from_value(current)?)
                .await
        }
        "system" => {
            let mut current = serde_json::to_value(client.system_settings().await?)?;
            merge_json(&mut current, patch);
            client
                .set_system_settings(serde_json::from_value(current)?)
                .await
        }
        other => anyhow::bail!("unknown settings tab: {other}"),
    }
}

fn merge_json(current: &mut serde_json::Value, patch: &serde_json::Value) {
    if let (Some(current_obj), Some(patch_obj)) = (current.as_object_mut(), patch.as_object()) {
        for (key, value) in patch_obj {
            current_obj.insert(key.clone(), value.clone());
        }
    }
}
