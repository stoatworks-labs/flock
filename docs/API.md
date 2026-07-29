# flock API reference

flock serves a JSON HTTP API and a WebSocket. Configuration lives in a TOML file (see
`config/example.toml`).

---

## Authentication

**Optional, and off by default.** With no `[web].admin_password` configured, flock behaves as
a trusted-LAN-only tool with no login — its historical behaviour.

Set `[web].admin_password` and **every route requires a valid session**, except:

- the static frontend (`/`, `/app.js`, `/style.css`)
- `/health`
- `/api/login` and `/api/logout` themselves

Those must stay reachable before a session exists.

### `POST /api/login`
Returns a `flock_session` cookie on success.

**Sessions are a random token in an in-memory set** — no persistence, no expiry. A process
restart logs everyone out. That's a deliberate trade-off for a single-operator LAN tool, not
an oversight.

**Brute-force guard:** after **5 failed attempts** there's a **30-second lockout**. It is
tracked **process-wide, not per client** — there's only one password, so there's no useful
notion of "which caller" to scope it to. A client guessing wrong pauses everyone's next
attempt briefly.

### `POST /api/logout`

---

## State

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/health` | Liveness |
| `GET` | `/api/state` | Full fleet state |
| `GET` | `/ws` | Live updates |

---

## Devices

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/api/devices` | Add a device |
| `PUT` | `/api/devices/:id` | Update |
| `DELETE` | `/api/devices/:id` | Remove |
| `GET` | `/api/devices/:id/status` | Live status |
| `GET` `POST` | `/api/devices/:id/network` | Network settings |
| `GET` `POST` | `/api/devices/:id/decode` | Decode settings — **what the device is playing** |
| `GET` `POST` | `/api/devices/:id/system` | System settings |
| `POST` | `/api/devices/:id/reboot` | Reboot |
| `GET` | `/api/devices/:id/preview` | Preview image |

> **`POST` on these endpoints writes to real hardware.** A decode change re-routes what a
> physical decoder is putting on its output — potentially a screen someone is watching.
> Reboot does what it says.

**Passwords are redacted in API responses** (`Device::redacted`). A device's BirdUI password
is never returned once stored.

---

## Discovery

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/api/discovery/scan` | Scan the network for devices |
| `GET` | `/api/ndi/sources` | Available NDI sources |

---

## Settings

| Method | Path | Purpose |
|---|---|---|
| `GET` `PUT` | `/api/settings` | Application settings |
| `POST` | `/api/settings/push-discovery-server` | Push the NDI discovery server address to devices |

---

## Groups

### `POST /api/groups/:tag/:tab`
Apply a settings tab to every device carrying a tag — the fleet operation flock exists for.

**This writes the same change to many devices at once.** Check the tag membership before
firing it.

---

## Credential storage

Device passwords are **encrypted before they touch disk**. `registry.json` previously held
every device's BirdUI password in plain text — acceptable for a demo, not for a file that
might end up in a backup or a sync folder.

- The key lives in its own file beside the registry, generated on first run.
- On Unix that key file is **chmod 600**. It is the only thing between `registry.json` and
  every device's plaintext password — **don't relax its permissions, and don't commit it.**
- Encrypted values carry a prefix so decryption can distinguish them from a plaintext
  password written before encryption existed. No real BirdUI password will legitimately start
  with it, so migration is unambiguous.

This is separate from redaction: redaction governs what the API returns, encryption governs
what sits on disk.

---

## Configuration

See `config/example.toml`.

```toml
[web]
# admin_password = "..."   # omit for no-auth trusted-LAN mode
```
