//! Live SRT preview: spawns `ffmpeg` to connect to the exact same `srt://`
//! endpoint a device's Decode tab is configured to pull from, streaming
//! multipart JPEG (`mpjpeg`) straight through as the HTTP response body - a
//! browser `<img>` tag renders `multipart/x-mixed-replace` natively, no
//! frontend decoding needed. See docs/architecture.md's "Live SRT preview"
//! section for what this can't cover (NDI, SRT listener mode) and why.

use std::process::Stdio;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use flock_core::DecodeSettings;

use crate::error::{parse_device_id, ApiError};
use crate::state::AppState;

/// ffmpeg's `mpjpeg` muxer's own default boundary string.
const MPJPEG_BOUNDARY: &str = "ffmpeg";

pub async fn get_preview(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let device_id = parse_device_id(&id)?;
    let device = state.registry.get(&device_id).ok_or(ApiError::NotFound)?;
    let client = state.provider.client_for(&device);
    let settings = client.decode_settings().await?;

    let Some(url) = srt_preview_url(&settings) else {
        return Ok(unavailable_response(&settings));
    };

    let spawned = Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-i",
            &url,
            "-an",
            "-r",
            "10",
            "-q:v",
            "5",
            "-f",
            "mpjpeg",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // The client (browser tab) closing the connection drops the axum
        // response body, which drops the ReceiverStream, which ends the
        // bridging task below and drops `child` - kill_on_drop is what
        // actually stops ffmpeg at that point rather than leaking it.
        .kill_on_drop(true)
        .spawn();

    let mut child = match spawned {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((
                StatusCode::SERVICE_UNAVAILABLE,
                "ffmpeg not found on PATH - live preview needs an ffmpeg build with SRT \
                 input support (most distro packages of plain `ffmpeg` lack this; see \
                 docs/architecture.md's \"Live SRT preview\" section)",
            )
                .into_response());
        }
        Err(e) => return Err(anyhow::anyhow!("failed to spawn ffmpeg: {e}").into()),
    };
    let mut stdout = child.stdout.take().expect("stdout was piped");

    let (tx, rx) = mpsc::channel::<std::io::Result<Vec<u8>>>(4);
    tokio::spawn(async move {
        let _child = child; // kept alive (for kill_on_drop) until this task ends
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(Ok(buf[..n].to_vec())).await.is_err() {
                        break; // receiver dropped - client disconnected
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    break;
                }
            }
        }
    });

    let body = Body::from_stream(ReceiverStream::new(rx));
    Ok((
        [(
            header::CONTENT_TYPE,
            format!("multipart/x-mixed-replace; boundary={MPJPEG_BOUNDARY}"),
        )],
        body,
    )
        .into_response())
}

/// Builds the same `srt://` URL BirdUI's own JS would (see
/// `build_srt_connection_url` in `flock-device-http` for the confirmed
/// query-parameter semantics this mirrors) - `None` whenever there's no
/// independently-dialable source to preview.
fn srt_preview_url(settings: &DecodeSettings) -> Option<String> {
    if settings.source_type != "SRT" || settings.srt_connection_type == "listener" {
        return None;
    }
    let ip = settings
        .srt_ip_address
        .as_deref()
        .filter(|s| !s.is_empty())?;
    let port = settings.srt_port?;
    let mut url = format!(
        "srt://{ip}:{port}?mode={}&latency={}",
        settings.srt_connection_type, settings.srt_latency_ms
    );
    if settings.srt_encryption_enabled {
        let keylen = settings.srt_encryption_key_length.as_deref().unwrap_or("0");
        url.push_str(&format!("&pbkeylen={keylen}"));
        if let Some(passphrase) = &settings.srt_passphrase {
            url.push_str(&format!("&passphrase={passphrase}"));
        }
    }
    if settings.srt_connection_type == "caller" {
        if let Some(stream_id) = &settings.srt_stream_id {
            url.push_str(&format!("&streamid={stream_id}"));
        }
    }
    Some(url)
}

fn unavailable_response(settings: &DecodeSettings) -> Response {
    let msg = if settings.source_type != "SRT" {
        "Live preview is only available for SRT decode sources - NDI would need the \
         proprietary NDI SDK, which flock deliberately doesn't bundle (see \
         docs/architecture.md's \"NDI source routing model\" section)."
    } else if settings.srt_connection_type == "listener" {
        "Live preview isn't available in SRT listener mode - the upstream source connects \
         to the Play, so there's no independent endpoint flock can also connect to."
    } else {
        "No SRT source configured yet - set an IP address and port on the Decode tab first."
    };
    (StatusCode::NOT_IMPLEMENTED, msg).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_settings() -> DecodeSettings {
        DecodeSettings {
            source_type: "SRT".to_string(),
            selected_source: None,
            available_sources: vec![],
            failover_source: None,
            srt_connection_type: "caller".to_string(),
            srt_stream_name: None,
            srt_ip_address: Some("10.0.0.5".to_string()),
            srt_port: Some(9000),
            srt_latency_ms: 200,
            srt_encryption_enabled: false,
            srt_encryption_key_length: None,
            srt_passphrase: None,
            srt_stream_id: None,
            srt_available_sources: vec![],
            screensaver_mode: "BlackSS".to_string(),
            color_space: "YUV".to_string(),
            ndi_audio_enabled: false,
            tally_mode: "TallyOff".to_string(),
        }
    }

    #[test]
    fn builds_caller_url_with_streamid() {
        let mut s = base_settings();
        s.srt_stream_id = Some("mystream".to_string());
        assert_eq!(
            srt_preview_url(&s).as_deref(),
            Some("srt://10.0.0.5:9000?mode=caller&latency=200&streamid=mystream")
        );
    }

    #[test]
    fn builds_encrypted_url_with_passphrase_and_keylen() {
        let mut s = base_settings();
        s.srt_encryption_enabled = true;
        s.srt_encryption_key_length = Some("32".to_string());
        s.srt_passphrase = Some("hunter2hunter2".to_string());
        assert_eq!(
            srt_preview_url(&s).as_deref(),
            Some(
                "srt://10.0.0.5:9000?mode=caller&latency=200&pbkeylen=32&passphrase=hunter2hunter2"
            )
        );
    }

    #[test]
    fn none_for_ndi_source_type() {
        let mut s = base_settings();
        s.source_type = "NDI".to_string();
        assert_eq!(srt_preview_url(&s), None);
    }

    #[test]
    fn none_for_listener_mode() {
        let mut s = base_settings();
        s.srt_connection_type = "listener".to_string();
        assert_eq!(srt_preview_url(&s), None);
    }

    #[test]
    fn none_without_ip_configured() {
        let mut s = base_settings();
        s.srt_ip_address = None;
        assert_eq!(srt_preview_url(&s), None);
    }
}
