# flock's hosted demo

flock controls BirdDog PLAY units over the LAN, so it can't be hosted in any
useful sense — a page on the public internet has no devices to talk to. What is
hosted at <https://flock.stoatworks-labs.com> is a **click-through demo**:
flock's real, unmodified web UI, replaying responses recorded from flock itself
running against its own simulated devices (`crates/device-mock`).

Everything you see there is output flock actually produced. Nothing is
mocked up in the design sense, and nothing is live.

## What's here

| File | What it is |
|---|---|
| `record-demo.sh` | Rebuilds the whole thing: starts flock in mock mode, records, assembles |
| `record-fixtures.mjs` | Records a running backend's responses (vendored, shared across repos) |
| `demo-shim.js` | Intercepts `fetch`/`WebSocket` in the page and replays the recording (vendored) |
| `build-demo.sh` | Assembles `crates/web/static` + shim + fixtures into a publishable site (vendored) |
| `serve-demo.py` | Serves the built site with a static host's headers, for local checking (vendored) |
| `demo-fixtures.json` | The recording. Regenerate it; don't hand-edit it |
| `dist/` | **Committed build output** — what Cloudflare Pages serves |

The vendored files come from `stoatworks-backend/pages-demo`. Fix them there
and copy out, or the copies drift.

## Rebuilding and publishing

```bash
demo/record-demo.sh                                  # record + assemble
demo/serve-demo.py --dir demo/dist    # check it locally first
git add demo/dist && git commit && git push   # Cloudflare publishes it
```

Cloudflare Pages publishes `demo/dist` from the repo with **no build command**.
It has to be committed: assembling the demo means running the app against its
mock devices and capturing what it says, which a build container can't do.

## Rules the demo has to keep

- **It always says it's a demo.** The shim adds a banner it can't be built
  without. A visitor must never think they're looking at live devices.
- **Writes go nowhere.** Save, Reboot and Remove are answered but change
  nothing, and the banner says so when one is clicked. Don't "improve" this into
  a fake success message.
- **Fixtures are recorded, never authored.** If an endpoint is missing from the
  demo, record it — don't write plausible JSON by hand, because that is exactly
  how a demo starts showing behaviour the software doesn't have.
