#!/usr/bin/env node
/**
 * Record a running app's backend responses into demo-fixtures.json, for
 * demo-shim.js to replay on GitHub Pages.
 *
 * Point it at the app running in ITS OWN mock/simulated-device mode. What gets
 * recorded is then real output from the real backend — the demo is a replay,
 * not a fabrication. That distinction is the whole point: a hand-written
 * fixture is a guess about what the software does, and guesses drift.
 *
 * Usage:
 *   node record-fixtures.mjs --base http://localhost:8080 \
 *        --app "flock" --repo https://github.com/stoatworks-labs/flock \
 *        --get /api/state --get /api/discovery/scan \
 *        --ws /ws --ws-seconds 12 \
 *        --expand '/api/state:devices[].id:/api/devices/{}/status' \
 *        --out demo-fixtures.json
 *
 * --expand walks a recorded response for ids and records a per-id endpoint for
 * each, so clicking through to a device in the demo shows that device's data.
 */

import { writeFileSync } from 'node:fs';
// Node 22+ ships a global WebSocket, so the recorder needs no dependencies.

function parseArgs(argv) {
  const out = { get: [], expand: [], ws: [], wsSeconds: 10, out: 'demo-fixtures.json' };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    const next = () => argv[++i];
    if (a === '--base') out.base = next();
    else if (a === '--app') out.app = next();
    else if (a === '--repo') out.repo = next();
    else if (a === '--get') out.get.push(next());
    else if (a === '--expand') out.expand.push(next());
    else if (a === '--ws') out.ws.push(next());
    else if (a === '--ws-seconds') out.wsSeconds = Number(next());
    else if (a === '--header') out.header = next();
    else if (a === '--out') out.out = next();
    else throw new Error(`unknown argument: ${a}`);
  }
  if (!out.base) throw new Error('--base is required');
  return out;
}

const args = parseArgs(process.argv);
const headers = args.header ? Object.fromEntries([args.header.split(':').map((s) => s.trim())]) : {};

const http = {};

async function record(path) {
  const url = new URL(path, args.base);
  let res;
  try {
    res = await fetch(url, { headers });
  } catch (err) {
    console.error(`  ! ${path} — ${err.message}`);
    return null;
  }
  const contentType = res.headers.get('content-type') ?? 'application/json';

  // Binary responses (device preview snapshots) are kept as base64 so the shim
  // can hand the app a real Blob rather than a mangled string.
  if (!/json|text|xml|javascript/.test(contentType)) {
    const bytes = Buffer.from(await res.arrayBuffer());
    http[`GET ${url.pathname}${url.search}`] = {
      status: res.status, contentType, base64: bytes.toString('base64'),
    };
    console.log(`  ${res.status} GET ${url.pathname}${url.search} (${bytes.length} bytes, binary)`);
    return null;
  }

  const text = await res.text();
  let body = text;
  if (contentType.includes('json')) {
    try { body = JSON.parse(text); } catch { /* keep the raw text */ }
  }
  http[`GET ${url.pathname}${url.search}`] = { status: res.status, contentType, body };
  console.log(`  ${res.status} GET ${url.pathname}${url.search} (${text.length} bytes)`);
  return body;
}

/** Pull values out of a recorded body with a `a.b[].c` style path. */
function pluck(value, path) {
  let current = [value];
  for (const segment of path.split('.')) {
    const isArray = segment.endsWith('[]');
    const name = isArray ? segment.slice(0, -2) : segment;
    current = current.flatMap((v) => {
      if (v == null) return [];
      const next = name ? v[name] : v;
      if (next == null) return [];
      return isArray && Array.isArray(next) ? next : [next];
    });
  }
  return [...new Set(current.filter((v) => v != null))];
}

console.log(`Recording ${args.app ?? args.base}`);
const bodies = {};
for (const path of args.get) bodies[path] = await record(path);

for (const spec of args.expand) {
  // form: <source path>:<value path>:<template with {}>
  const idx1 = spec.indexOf(':');
  const idx2 = spec.indexOf(':', idx1 + 1);
  const source = spec.slice(0, idx1);
  const valuePath = spec.slice(idx1 + 1, idx2);
  const template = spec.slice(idx2 + 1);
  const body = bodies[source] ?? (await record(source));
  const ids = pluck(body, valuePath);
  console.log(`  expand ${source} -> ${ids.length} id(s) for ${template}`);
  for (const id of ids) await record(template.replace('{}', encodeURIComponent(id)));
}

const wsRecordings = [];
for (const path of args.ws) {
  const url = new URL(path, args.base);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  console.log(`  ws ${url.pathname} for ${args.wsSeconds}s`);
  const frames = await new Promise((resolve) => {
    const collected = [];
    let last = Date.now();
    const socket = new WebSocket(url);
    const stop = setTimeout(() => { socket.close(); resolve(collected); }, args.wsSeconds * 1000);
    socket.addEventListener('message', (event) => {
      const now = Date.now();
      collected.push({ dt: now - last, data: String(event.data) });
      last = now;
    });
    socket.addEventListener('error', () => {
      console.error(`  ! ws ${url.pathname} — connection error`);
      clearTimeout(stop);
      resolve(collected);
    });
    socket.addEventListener('close', () => { clearTimeout(stop); resolve(collected); });
  });
  console.log(`    ${frames.length} frame(s)`);
  wsRecordings.push({ url: url.pathname, frames });
}

const fixtures = {
  meta: {
    app: args.app ?? args.base,
    repo: args.repo ?? '',
    recordedFrom: args.base,
    note: 'Recorded from the real backend running against simulated devices.',
  },
  http,
  ws: wsRecordings,
};

writeFileSync(args.out, JSON.stringify(fixtures, null, 2));
console.log(`Wrote ${args.out}: ${Object.keys(http).length} response(s), ${wsRecordings.reduce((n, w) => n + w.frames.length, 0)} ws frame(s)`);

// A WebSocket that the server never closed keeps the event loop alive, so the
// recorder would otherwise hang here having already done all its work.
process.exit(0);
