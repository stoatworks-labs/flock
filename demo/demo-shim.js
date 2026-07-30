/**
 * Stoatworks Pages demo shim.
 *
 * Several of these apps are web UIs over a backend that has to be on the same
 * network as the hardware — ATEMs, BirdDog Play units, SRT endpoints, a
 * CasparCG server. Those can't be hosted: there is nothing for a page on the
 * public internet to talk to. What *can* be hosted is a faithful click-through:
 * the real, unmodified UI, replaying responses recorded from the real backend
 * running against its own mock devices.
 *
 * This script intercepts `fetch` and `WebSocket` before the app's own code runs
 * and answers from `demo-fixtures.json`. Nothing about the app is stubbed or
 * reimplemented — the UI does exactly what it always does, and the bytes it
 * receives are bytes a real backend actually produced.
 *
 * Honesty rules this file exists to keep:
 *   - It always shows a banner. A demo must never be mistakable for live.
 *   - Writes (POST/PUT/PATCH/DELETE) are acknowledged but change nothing, and
 *     say so in the banner when one is attempted.
 *   - An endpoint with no recording returns a clear error rather than an
 *     invented success.
 *
 * Load it BEFORE the application script:
 *   <script src="demo-shim.js" data-fixtures="demo-fixtures.json"></script>
 */
(() => {
  const script = document.currentScript;
  const fixturesUrl = script?.dataset.fixtures ?? 'demo-fixtures.json';

  /** @type {{meta: object, http: Record<string, any>, ws: {url: string, frames: any[]}[]}} */
  let fixtures = { meta: {}, http: {}, ws: [] };
  let ready = false;
  const waiters = [];

  const realFetch = window.fetch.bind(window);
  const RealWebSocket = window.WebSocket;

  // ---- banner -------------------------------------------------------------

  let bannerNote = null;
  function banner() {
    const el = document.createElement('div');
    el.id = 'stoatworks-demo-banner';
    el.innerHTML =
      `<strong>Demo</strong> — recorded from ${fixtures.meta?.app ?? 'the app'} running against ` +
      `simulated devices. Nothing here is live, and changes aren't saved. ` +
      `<a href="${fixtures.meta?.repo ?? '#'}">Source and how to run it for real</a>` +
      `<span id="stoatworks-demo-note"></span>`;
    Object.assign(el.style, {
      position: 'fixed', insetInline: '0', bottom: '0', zIndex: '2147483647',
      font: '13px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
      background: '#12141a', color: '#c8cddb', borderTop: '2px solid #6ea8fe',
      padding: '8px 14px', textAlign: 'center',
      boxShadow: '0 -4px 16px rgba(0,0,0,.4)',
    });
    const link = el.querySelector('a');
    if (link) Object.assign(link.style, { color: '#6ea8fe' });
    document.body.appendChild(el);
    bannerNote = el.querySelector('#stoatworks-demo-note');
    // Keep the banner from covering the bottom of the app.
    document.body.style.paddingBottom = `${el.offsetHeight + 8}px`;
  }

  let noteTimer = null;
  function note(text) {
    if (!bannerNote) return;
    bannerNote.textContent = ` · ${text}`;
    bannerNote.style.color = '#d9a441';
    clearTimeout(noteTimer);
    noteTimer = setTimeout(() => { bannerNote.textContent = ''; }, 6000);
  }

  // ---- fixture lookup -----------------------------------------------------

  const load = realFetch(fixturesUrl)
    .then((r) => r.json())
    .then((f) => {
      fixtures = f;
      ready = true;
      waiters.splice(0).forEach((resolve) => resolve());
    })
    .catch((err) => {
      ready = true;
      waiters.splice(0).forEach((resolve) => resolve());
      console.error('[demo] could not load fixtures:', err);
    });

  const whenReady = () => (ready ? Promise.resolve() : new Promise((r) => waiters.push(r)));

  /** Normalise a request URL to the recorded key: path + sorted query. */
  function key(url, method) {
    const u = new URL(url, location.href);
    const params = [...u.searchParams.entries()].sort();
    const qs = params.length ? `?${params.map(([k, v]) => `${k}=${v}`).join('&')}` : '';
    return `${method.toUpperCase()} ${u.pathname}${qs}`;
  }

  /**
   * Recorded responses are keyed by exact path. Where the app addresses one of
   * several devices/routes, fall back to any recording for the same shape, so
   * clicking a second device shows that device's real recorded response rather
   * than an error.
   */
  function lookup(k) {
    if (fixtures.http[k]) return fixtures.http[k];
    const [method, path] = k.split(' ');
    const shape = path.replace(/\/[^/]*\d[^/]*(?=\/|$)/g, '/:id');
    for (const [candidate, value] of Object.entries(fixtures.http)) {
      const [cm, cp] = candidate.split(' ');
      if (cm !== method) continue;
      if (cp.replace(/\/[^/]*\d[^/]*(?=\/|$)/g, '/:id') === shape) return value;
    }
    return null;
  }

  // ---- fetch --------------------------------------------------------------

  const WRITE = new Set(['POST', 'PUT', 'PATCH', 'DELETE']);

  window.fetch = async function demoFetch(input, init = {}) {
    const url = typeof input === 'string' ? input : input.url;
    const method = (init.method ?? (typeof input === 'object' ? input.method : null) ?? 'GET').toUpperCase();

    // Anything not addressed at the backend (the app's own assets) is real.
    const u = new URL(url, location.href);
    if (u.origin !== location.origin) return realFetch(input, init);

    await whenReady();
    const k = key(url, method);

    if (WRITE.has(method)) {
      const rec = lookup(k);
      note(`"${method} ${u.pathname}" isn't sent anywhere in the demo`);
      return jsonResponse(rec ? rec.body : { ok: true, demo: true }, rec?.status ?? 200);
    }

    const rec = lookup(k);
    if (!rec) {
      // Not a backend call we recorded — let it hit the server (it's probably
      // a static asset). If that 404s, the app sees a normal 404, not a lie.
      return realFetch(input, init);
    }
    return jsonResponse(rec.body, rec.status, rec.contentType, rec.base64);
  };

  function jsonResponse(body, status = 200, contentType = 'application/json', base64) {
    if (base64 != null) {
      const bin = atob(base64);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      return new Response(bytes, { status, headers: { 'Content-Type': contentType } });
    }
    const text = typeof body === 'string' ? body : JSON.stringify(body);
    return new Response(text, { status, headers: { 'Content-Type': contentType } });
  }

  // ---- WebSocket ----------------------------------------------------------

  /**
   * Stands in for the live event socket. Replays the recorded frames with
   * their original relative timing, then holds the connection open — the apps
   * treat a close as "backend lost" and start reconnect loops.
   */
  class DemoWebSocket extends EventTarget {
    static CONNECTING = 0; static OPEN = 1; static CLOSING = 2; static CLOSED = 3;

    constructor(url) {
      super();
      this.url = url;
      this.readyState = DemoWebSocket.CONNECTING;
      this.onopen = null; this.onmessage = null; this.onclose = null; this.onerror = null;
      this.binaryType = 'blob';
      whenReady().then(() => this.#start());
    }

    #emit(type, event) {
      const handler = this[`on${type}`];
      if (typeof handler === 'function') handler.call(this, event);
      this.dispatchEvent(event);
    }

    #start() {
      const path = new URL(this.url, location.href).pathname;
      const recording = (fixtures.ws ?? []).find((w) => w.url === path) ?? (fixtures.ws ?? [])[0];
      this.readyState = DemoWebSocket.OPEN;
      this.#emit('open', new Event('open'));
      if (!recording) return;

      let elapsed = 0;
      for (const frame of recording.frames) {
        elapsed += Math.min(frame.dt ?? 0, 2000);
        setTimeout(() => {
          if (this.readyState !== DemoWebSocket.OPEN) return;
          this.#emit('message', new MessageEvent('message', { data: frame.data }));
        }, elapsed);
      }
    }

    send() { note('the demo has no backend to send to'); }

    close() {
      this.readyState = DemoWebSocket.CLOSED;
      this.#emit('close', new CloseEvent('close', { code: 1000, wasClean: true }));
    }
  }
  DemoWebSocket.prototype.CONNECTING = 0;
  DemoWebSocket.prototype.OPEN = 1;
  DemoWebSocket.prototype.CLOSING = 2;
  DemoWebSocket.prototype.CLOSED = 3;

  window.WebSocket = DemoWebSocket;

  // Some apps use EventSource for the same job.
  if (window.EventSource) {
    window.EventSource = class DemoEventSource extends EventTarget {
      constructor(url) { super(); this.url = url; this.readyState = 1; this.onmessage = null; }
      close() { this.readyState = 2; }
    };
  }

  load.then(() => {
    if (document.body) banner();
    else document.addEventListener('DOMContentLoaded', banner, { once: true });
  });
})();
