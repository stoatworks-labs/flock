# flock's hosted demo

flock controls BirdDog PLAY units over the LAN, so it can't be hosted in any
useful sense — a page on the public internet has no devices to talk to. What is
hosted at <https://stoatworks-labs.com/flock/> is a **click-through demo**:
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
| `serve-demo.py` | Serves the built site with GitHub Pages' headers, for local checking (vendored) |
| `deploy-pages.sh` | Pushes the built site to the `gh-pages` branch (vendored) |
| `demo-fixtures.json` | The recording. Regenerate it; don't hand-edit it |

The vendored files come from `stoatworks-backend/pages-demo`. Fix them there
and copy out, or the copies drift.

## Rebuilding and publishing

```bash
demo/record-demo.sh                                  # record + assemble
demo/serve-demo.py --dir demo/dist --base /flock/    # check it locally first
demo/deploy-pages.sh --dist demo/dist --label "flock demo"
```

## Rules the demo has to keep

- **It always says it's a demo.** The shim adds a banner it can't be built
  without. A visitor must never think they're looking at live devices.
- **Writes go nowhere.** Save, Reboot and Remove are answered but change
  nothing, and the banner says so when one is clicked. Don't "improve" this into
  a fake success message.
- **Fixtures are recorded, never authored.** If an endpoint is missing from the
  demo, record it — don't write plausible JSON by hand, because that is exactly
  how a demo starts showing behaviour the software doesn't have.
