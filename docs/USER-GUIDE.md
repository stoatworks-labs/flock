# flock user guide

flock gives you **one control panel for every BirdDog Play decoder you own**.

Out of the box, each Play has its own BirdUI web page — fine for one unit, painful for twenty.
flock discovers them all, shows them in one place, and lets you change settings across a whole
group at once.

> **Status:** flock has been used against **both a simulated device and a real one**,
> including live reads and a **real settings write** — routing an actual NDI source to a
> physical unit's HDMI output. That's stronger evidence than most tools of this kind carry.
> It's still described by its author as an early-stage hobby project, and the README's Status
> section defines exactly which operations are covered. A successful write on one path isn't
> proof of every path.

---

## Getting started

1. Run flock (as its own desktop tray app, or `cargo run -p flock`).
2. Open the web UI.
3. **Scan** — flock probes the local subnet for Play devices.
4. Add the ones you want to manage. Each needs its BirdUI password.

Passwords are **encrypted before being written to disk**, and never returned by the API once
stored.

---

## What you can do

For each device:

- **Status** — is it up, what's it doing
- **Decode** — which NDI or SRT source it's playing. **This is the one you'll use most.**
- **Network** — addressing
- **System** — device-level settings
- **Reboot**
- **Preview** — a look at the output

### Group operations — the point of flock

Tag your devices (by room, by hall, by purpose), then apply a settings tab to **every device
with that tag in one action**.

> **This writes to every tagged device at once.** Check what's in a tag before you fire a
> group operation — a mistake here is multiplied by the size of the group, and these are
> decoders driving screens people may be watching.

---

## Locking it down

By default flock has **no login**, on the assumption it's on a trusted production LAN.

To require a password, set `admin_password` under `[web]` in the config. Then every page and
API call needs a session.

Two things to know:

- **Sessions live in memory only.** Restarting flock logs everyone out. That's intentional for
  a single-operator tool.
- **After 5 failed logins there's a 30-second lockout**, applied process-wide rather than per
  client — there's only one password, so a client guessing wrong briefly pauses everyone's
  next attempt.

Even with a password set, flock is designed for a **trusted operations network**. Don't put it
on the public internet.

---

## Troubleshooting

**Scan finds nothing.**
flock and the devices must be on the same subnet. Check for a firewall, and for client
isolation on the wireless network. Discovery uses a subnet probe, so it won't cross a router.

**A device shows as unreachable after working.**
Its IP probably changed. Check the Network tab, or re-scan.

**A settings write is rejected.**
Usually a wrong or changed BirdUI password. Re-enter it on the device.

**I can't see the password I entered.**
By design — it's redacted in every response and encrypted on disk. Re-enter it if you need to
change it.

**Everyone got logged out.**
flock restarted. Sessions are in-memory.

**Login says I'm locked out.**
Five failed attempts triggers a 30-second cooldown. Wait it out.

---

## A note on care

flock writes settings to **real decoders on a live network**. A bad decode change doesn't fail
a test — it can black out a screen in front of an audience.

Prefer reading before writing, check group membership before group operations, and if you're
developing against flock rather than operating it, use the mock device backend instead of real
hardware.
