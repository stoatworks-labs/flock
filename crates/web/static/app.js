(() => {
  "use strict";

  const state = {
    devices: [],
    groups: {},
    activeGroup: "",
    search: "",
    selectedId: null,
    batchMode: false,
    activeTab: "status",
    editingId: null,
    // Tags present here are collapsed; everything else (including the
    // synthetic "" / "All devices" group) starts expanded.
    collapsedGroups: new Set(),
    // Which device's live preview is currently loaded (or null) - guards
    // against re-spawning the preview's ffmpeg process on every WS-driven
    // re-render (renderCenter runs far more often than the selection
    // actually changes).
    previewLoadedFor: null,
  };

  const el = (id) => document.getElementById(id);

  // ---------- data loading ----------

  async function api(path, opts) {
    const res = await fetch(path, opts);
    if (!res.ok) {
      const text = await res.text().catch(() => res.statusText);
      throw new Error(text || `${res.status} ${res.statusText}`);
    }
    const contentType = res.headers.get("content-type") || "";
    return contentType.includes("application/json") ? res.json() : null;
  }

  async function loadState() {
    const data = await api("/api/state");
    state.devices = data.devices;
    state.groups = data.groups;
    render();
    applyHashSelection();
  }

  // Supports deep-linking to a device+tab via #<id-or-name>/<tab>, e.g.
  // #stage cam play/network, or to a group's batch-edit view via
  // #group:<tag>/<tab>, e.g. #group:stage/network - mainly useful for
  // scripted screenshots, but also a handy bookmarkable link for a human.
  function applyHashSelection() {
    const hash = decodeURIComponent(location.hash.replace(/^#/, ""));
    if (!hash) return;
    const [matcher, tab] = hash.split("/");
    if (!matcher) return;

    if (matcher.startsWith("group:")) {
      const tag = matcher.slice("group:".length);
      if (!(tag in state.groups)) return;
      state.activeGroup = tag;
      state.batchMode = true;
      state.selectedId = null;
      if (tab) state.activeTab = tab;
      render();
      return;
    }

    const device = state.devices.find(
      (d) => idStr(d.id) === matcher || d.name.toLowerCase() === matcher.toLowerCase()
    );
    if (!device) return;
    state.selectedId = idStr(device.id);
    if (tab) state.activeTab = tab;
    render();
  }

  function connectWs() {
    const proto = location.protocol === "https:" ? "wss" : "ws";
    const ws = new WebSocket(`${proto}://${location.host}/ws`);
    ws.onopen = () => el("conn-indicator").classList.add("live");
    ws.onclose = () => {
      el("conn-indicator").classList.remove("live");
      setTimeout(connectWs, 2000);
    };
    ws.onerror = () => ws.close();
    ws.onmessage = (evt) => {
      const data = JSON.parse(evt.data);
      state.devices = data.devices;
      state.groups = data.groups;
      render();
    };
  }

  // ---------- rendering: left panel ----------

  // Search-only now - group membership is handled by which section a
  // device is nested under, not by filtering a single flat list.
  function matchesFilter(device) {
    if (!state.search) return true;
    const needle = state.search.toLowerCase();
    return (
      device.name.toLowerCase().includes(needle) ||
      device.host.toLowerCase().includes(needle) ||
      device.tags.some((t) => t.toLowerCase().includes(needle))
    );
  }

  // Nested view: groups listed vertically, each with an expand arrow to
  // reveal its members. Clicking a group's header batch-edits that whole
  // group in one step (no separate "batch edit" button); clicking the
  // arrow only toggles expansion. A device in multiple groups appears
  // nested under each one - that's the point, groups are derived from
  // tags, not exclusive membership. "All devices" is a synthetic
  // pseudo-group (not a real tag) so its header just clears any
  // selection/batch-mode instead of offering to batch-edit it.
  function renderGroupTree() {
    const container = el("group-tree");
    const tags = Object.keys(state.groups).sort();
    const sections = [{ tag: "", label: "All devices", members: state.devices.map((d) => idStr(d.id)) }];
    for (const tag of tags) {
      sections.push({ tag, label: tag, members: state.groups[tag].map(idStr) });
    }

    const html = sections
      .map(({ tag, label, members }) => {
        const devices = members.map((id) => findDevice(id)).filter(Boolean).filter(matchesFilter);
        if (state.search && devices.length === 0) return "";

        const expanded = !state.collapsedGroups.has(tag);
        const isActive = state.batchMode && state.activeGroup === tag && tag !== "";
        const memberRows = devices
          .map(
            (d) => `
          <div class="device-row ${idStr(d.id) === state.selectedId ? "selected" : ""}" data-id="${escapeAttr(idStr(d.id))}">
            <span class="status-dot"></span>
            <span class="name-block">
              <span class="name">${escapeHtml(d.name)}</span>
              <span class="host">${escapeHtml(d.host)}</span>
            </span>
          </div>`
          )
          .join("");

        return `
          <div class="group-section">
            <div class="group-header ${isActive ? "active" : ""}" data-tag="${escapeAttr(tag)}">
              <span class="expand-arrow ${expanded ? "expanded" : ""}" data-arrow="${escapeAttr(tag)}">▸</span>
              <span class="group-name">${escapeHtml(label)}</span>
              <span class="group-count">${devices.length}</span>
            </div>
            ${expanded ? `<div class="group-members">${memberRows || '<div class="devices-empty">No devices.</div>'}</div>` : ""}
          </div>`;
      })
      .join("");

    container.innerHTML = html || `<div class="devices-empty">No devices match.</div>`;

    container.querySelectorAll(".expand-arrow").forEach((arrow) => {
      arrow.addEventListener("click", (e) => {
        e.stopPropagation();
        const tag = arrow.dataset.arrow;
        if (state.collapsedGroups.has(tag)) {
          state.collapsedGroups.delete(tag);
        } else {
          state.collapsedGroups.add(tag);
        }
        renderGroupTree();
      });
    });
    container.querySelectorAll(".group-header").forEach((header) => {
      header.addEventListener("click", () => {
        const tag = header.dataset.tag;
        if (tag) {
          startBatchEdit(tag);
        } else {
          state.selectedId = null;
          state.batchMode = false;
          render();
        }
      });
    });
    container.querySelectorAll(".device-row").forEach((row) => {
      row.addEventListener("click", (e) => {
        e.stopPropagation();
        selectDevice(row.dataset.id);
      });
    });
  }

  function idStr(id) {
    // Device.id serializes as the raw UUID string via serde (newtype transparent).
    return typeof id === "string" ? id : String(id);
  }

  function findDevice(id) {
    return state.devices.find((d) => idStr(d.id) === id);
  }

  // ---------- rendering: center panel ----------

  function selectDevice(id) {
    state.selectedId = id;
    state.batchMode = false;
    state.activeTab = "status";
    render();
  }

  function startBatchEdit(tag) {
    if (!tag) return;
    state.activeGroup = tag;
    state.batchMode = true;
    state.selectedId = null;
    if (state.activeTab === "status") state.activeTab = "network";
    render();
  }

  function renderCenter() {
    const tabBar = el("tab-bar");
    const preview = el("preview-box");
    const selectedActions = el("selected-actions");

    if (state.batchMode && !state.activeGroup) state.batchMode = false;

    if (state.batchMode) {
      const memberIds = (state.groups[state.activeGroup] || []).map(idStr);
      tabBar.hidden = false;
      selectedActions.hidden = true;
      if (state.previewLoadedFor !== null && activePreviewAbort) activePreviewAbort.abort();
      state.previewLoadedFor = null;
      preview.innerHTML = `<div id="preview-placeholder">Batch editing "${escapeHtml(state.activeGroup)}" — ${memberIds.length} device${memberIds.length === 1 ? "" : "s"}</div>`;
      if (state.activeTab === "status") state.activeTab = "network";
      tabBar.querySelectorAll(".tab-btn").forEach((btn) => {
        btn.hidden = btn.dataset.tab === "status";
        btn.classList.toggle("tab-active", btn.dataset.tab === state.activeTab);
      });
      renderBatchTabContent(state.activeGroup);
      return;
    }

    tabBar.querySelectorAll(".tab-btn").forEach((btn) => {
      btn.hidden = false;
    });

    const device = state.selectedId ? findDevice(state.selectedId) : null;

    if (!device) {
      tabBar.hidden = true;
      selectedActions.hidden = true;
      if (state.previewLoadedFor !== null && activePreviewAbort) activePreviewAbort.abort();
      state.previewLoadedFor = null;
      preview.innerHTML = `<div id="preview-placeholder">Select a device to view its preview and settings</div>`;
      el("tab-content").innerHTML = "";
      return;
    }

    tabBar.hidden = false;
    selectedActions.hidden = false;
    if (state.previewLoadedFor !== idStr(device.id)) {
      state.previewLoadedFor = idStr(device.id);
      loadPreview(device);
    }

    tabBar.querySelectorAll(".tab-btn").forEach((btn) => {
      btn.classList.toggle("tab-active", btn.dataset.tab === state.activeTab);
    });

    renderTabContent(device);
  }

  // Live preview is only possible for an SRT decode source in caller/
  // rendezvous mode (flock dials the same srt:// endpoint the Play itself
  // decodes from) - the backend figures out availability and returns a
  // plain-text explanation otherwise.
  //
  // The backend streams multipart/x-mixed-replace (ffmpeg's own mpjpeg
  // muxer) - a plain <img src="..."> pointed at it looks like the obvious
  // way to consume that, but modern Chrome no longer reliably renders
  // multipart/x-mixed-replace via <img> at all (confirmed: the request
  // succeeds, frames arrive, and the <img> just never updates). So this
  // fetches the stream itself and manually parses out each JPEG part,
  // swapping the <img>'s src to a fresh blob: URL per frame instead - more
  // code, but works regardless of browser-level multipart support.
  let activePreviewAbort = null;

  async function loadPreview(device) {
    const preview = el("preview-box");
    const deviceId = idStr(device.id);
    preview.innerHTML = `<div id="preview-placeholder">Loading preview…</div>`;
    if (activePreviewAbort) activePreviewAbort.abort();
    const controller = new AbortController();
    activePreviewAbort = controller;

    try {
      const res = await fetch(`/api/devices/${deviceId}/preview`, {
        signal: controller.signal,
      });
      if (!res.ok) {
        const msg = await res.text().catch(() => res.statusText);
        if (state.previewLoadedFor === deviceId) {
          preview.innerHTML = `<div id="preview-placeholder">${escapeHtml(msg)}</div>`;
        }
        return;
      }
      if (state.previewLoadedFor !== deviceId) {
        controller.abort();
        return;
      }
      preview.innerHTML = `<img class="preview-stream" alt="${escapeAttr(device.name)} live preview">`;
      await streamMjpegFrames(res, preview.querySelector(".preview-stream"));
    } catch (err) {
      if (err.name === "AbortError") return; // expected when switching devices
      if (state.previewLoadedFor === deviceId) {
        preview.innerHTML = `<div id="preview-placeholder">Preview failed: ${escapeHtml(err.message)}</div>`;
      }
    }
  }

  // Parses ffmpeg's mpjpeg output (boundary + "Content-length: N" header +
  // N raw JPEG bytes, repeated) directly off the response body's byte
  // stream and updates `imgEl.src` to a fresh object URL per frame,
  // revoking the previous one so frames don't leak memory.
  async function streamMjpegFrames(res, imgEl) {
    const contentType = res.headers.get("content-type") || "";
    const boundaryName = contentType.split("boundary=")[1]?.trim() || "ffmpeg";
    const boundaryBytes = new TextEncoder().encode(`--${boundaryName}`);
    const headerEndBytes = new Uint8Array([13, 10, 13, 10]); // \r\n\r\n
    const reader = res.body.getReader();
    let buf = new Uint8Array(0);
    let currentUrl = null;

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buf = concatBytes(buf, value);

        let frame;
        while ((frame = extractMjpegFrame(buf, boundaryBytes, headerEndBytes))) {
          const blob = new Blob([frame.bytes], { type: "image/jpeg" });
          const newUrl = URL.createObjectURL(blob);
          imgEl.src = newUrl;
          if (currentUrl) URL.revokeObjectURL(currentUrl);
          currentUrl = newUrl;
          buf = frame.rest;
        }
      }
    } finally {
      if (currentUrl) URL.revokeObjectURL(currentUrl);
    }
  }

  function concatBytes(a, b) {
    const out = new Uint8Array(a.length + b.length);
    out.set(a, 0);
    out.set(b, a.length);
    return out;
  }

  function indexOfBytes(haystack, needle, from) {
    outer: for (let i = from; i <= haystack.length - needle.length; i++) {
      for (let j = 0; j < needle.length; j++) {
        if (haystack[i + j] !== needle[j]) continue outer;
      }
      return i;
    }
    return -1;
  }

  // Returns {bytes, rest} for the next complete frame in `buf`, or null if
  // a full frame isn't buffered yet (wait for more data from the reader).
  function extractMjpegFrame(buf, boundaryBytes, headerEndBytes) {
    const boundaryAt = indexOfBytes(buf, boundaryBytes, 0);
    if (boundaryAt === -1) return null;
    const headerStart = boundaryAt + boundaryBytes.length;
    const headerEnd = indexOfBytes(buf, headerEndBytes, headerStart);
    if (headerEnd === -1) return null;
    const headerText = new TextDecoder().decode(buf.slice(headerStart, headerEnd));
    const lengthMatch = headerText.match(/Content-length:\s*(\d+)/i);
    const bodyStart = headerEnd + headerEndBytes.length;
    if (!lengthMatch) {
      // Malformed part header - skip past it rather than looping forever.
      return { bytes: new Uint8Array(0), rest: buf.slice(bodyStart) };
    }
    const contentLength = parseInt(lengthMatch[1], 10);
    const bodyEnd = bodyStart + contentLength;
    if (buf.length < bodyEnd) return null; // frame body not fully buffered yet
    return { bytes: buf.slice(bodyStart, bodyEnd), rest: buf.slice(bodyEnd) };
  }

  async function renderTabContent(device) {
    const container = el("tab-content");
    container.innerHTML = `<div class="settings-card">Loading…</div>`;
    const id = idStr(device.id);
    try {
      if (state.activeTab === "status") {
        const status = await api(`/api/devices/${id}/status`);
        container.innerHTML = statusView(status);
      } else if (state.activeTab === "network") {
        const settings = await api(`/api/devices/${id}/network`);
        container.innerHTML = networkForm(settings);
        wireSaveForm(id, "network", collectNetworkForm);
      } else if (state.activeTab === "decode") {
        const settings = await api(`/api/devices/${id}/decode`);
        container.innerHTML = decodeForm(settings);
        toggleSourceType();
        wireSaveForm(id, "decode", collectDecodeForm);
      } else if (state.activeTab === "system") {
        const settings = await api(`/api/devices/${id}/system`);
        container.innerHTML = systemForm(settings);
        wireSaveForm(id, "system", collectSystemForm);
      }
    } catch (err) {
      container.innerHTML = `<div class="settings-card">Failed to load: ${escapeHtml(err.message)}</div>`;
    }
  }

  function wireSaveForm(deviceId, tab, collectFn) {
    const btn = el("save-btn");
    if (!btn) return;
    btn.addEventListener("click", async () => {
      const body = collectFn();
      const statusEl = el("save-status");
      try {
        await api(`/api/devices/${deviceId}/${tab}`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(body),
        });
        statusEl.textContent = "Saved";
        statusEl.classList.add("visible");
        setTimeout(() => statusEl.classList.remove("visible"), 1500);
        // Decode settings changing (source, connection details, ...) can
        // change what the live preview should show - force it to reload
        // rather than keep displaying whatever was live before the save.
        if (tab === "decode" && state.previewLoadedFor === deviceId) {
          state.previewLoadedFor = null;
          renderCenter();
        }
      } catch (err) {
        statusEl.textContent = `Error: ${err.message}`;
        statusEl.classList.add("visible");
      }
    });
  }

  // Batch-edit tabs (Network/Decode/System only - Status is per-device and
  // has no meaning across a group). Forms render blank with a "leave
  // unchanged" placeholder/option per field, and only fields the operator
  // actually touched are sent - each target device merges the patch into
  // its own current settings, so untouched fields keep their per-device
  // values instead of being overwritten with a shared default.
  async function renderBatchTabContent(tag) {
    const container = el("tab-content");
    const opts = { batch: true };
    if (state.activeTab === "network") {
      container.innerHTML = networkForm({}, opts);
      wireBatchSaveForm(tag, "network", () => collectNetworkForm(opts));
    } else if (state.activeTab === "decode") {
      container.innerHTML = decodeForm({}, opts);
      // Batch edit fields default to "leave unchanged", so both the NDI and
      // SRT groups stay visible here (unlike the per-device toggle) in case
      // the operator wants to touch either regardless of source_type.
      wireBatchSaveForm(tag, "decode", () => collectDecodeForm(opts));
    } else if (state.activeTab === "system") {
      container.innerHTML = systemForm({}, opts);
      wireBatchSaveForm(tag, "system", () => collectSystemForm(opts));
    }
  }

  function wireBatchSaveForm(tag, tab, collectFn) {
    const btn = el("save-btn");
    if (!btn) return;
    btn.addEventListener("click", async () => {
      const patch = collectFn();
      const statusEl = el("save-status");
      try {
        const outcomes = await api(`/api/groups/${encodeURIComponent(tag)}/${tab}`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(patch),
        });
        const failed = outcomes.filter((o) => !o.ok);
        statusEl.textContent =
          failed.length === 0
            ? `Applied to ${outcomes.length} device${outcomes.length === 1 ? "" : "s"}`
            : `Applied to ${outcomes.length - failed.length}/${outcomes.length} — failed: ${failed.map((f) => f.device_name).join(", ")}`;
        statusEl.classList.add("visible");
      } catch (err) {
        statusEl.textContent = `Error: ${err.message}`;
        statusEl.classList.add("visible");
      }
    });
  }

  function setIfPresent(patch, key, v) {
    if (v !== "") patch[key] = v;
  }
  function setIfPresentNumber(patch, key, v) {
    if (v !== "") patch[key] = Number(v);
  }
  function setIfPresentBool(patch, key, v) {
    if (v !== "") patch[key] = v === "true";
  }
  function setIfPresentList(patch, key, v) {
    if (v !== "") patch[key] = csvToList(v);
  }

  function field(label, inner) {
    return `<label>${escapeHtml(label)}${inner}</label>`;
  }
  function textField(id, value, opts = {}) {
    const v = opts.batch ? "" : value ?? "";
    const placeholder = opts.batch ? "— leave unchanged —" : "";
    const readOnly = opts.readOnly && !opts.batch ? "readonly" : "";
    const list = opts.list ? `list="${escapeAttr(opts.list)}"` : "";
    // A <datalist> filters its suggestions by the input's *current* text,
    // so a field that already has a value (e.g. the currently-selected
    // source) would otherwise only ever suggest itself, hiding every other
    // option. Clearing the field on focus reveals the full list; blurring
    // without typing or picking anything restores what was there.
    const revealOnFocus = opts.list
      ? `onfocus="this.dataset.prev=this.value; this.dataset.touched=''; this.value='';" oninput="this.dataset.touched='1';" onblur="if(!this.dataset.touched) this.value=this.dataset.prev || '';"`
      : "";
    return field(labelFor(id), `<input id="f-${id}" type="text" value="${escapeAttr(v)}" placeholder="${escapeAttr(placeholder)}" ${readOnly} ${list} ${revealOnFocus}>`);
  }
  function numberField(id, value, opts = {}) {
    const v = opts.batch ? "" : value ?? 0;
    const placeholder = opts.batch ? "— leave unchanged —" : "";
    return field(labelFor(id), `<input id="f-${id}" type="number" value="${escapeAttr(v)}" placeholder="${escapeAttr(placeholder)}">`);
  }
  function checkboxField(id, checkedVal, opts = {}) {
    if (opts.batch) {
      return field(
        labelFor(id),
        `<select id="f-${id}"><option value="" selected>— leave unchanged —</option><option value="true">On</option><option value="false">Off</option></select>`
      );
    }
    return `<label class="inline-label"><span>${escapeHtml(labelFor(id))}</span><input id="f-${id}" type="checkbox" ${checkedVal ? "checked" : ""}></label>`;
  }
  function selectField(id, value, options, opts = {}) {
    const optionsHtml = opts.batch
      ? `<option value="" selected>— leave unchanged —</option>` +
        options.map((o) => `<option value="${escapeAttr(o)}">${escapeHtml(o)}</option>`).join("")
      : options.map((o) => `<option value="${escapeAttr(o)}" ${o === value ? "selected" : ""}>${escapeHtml(o)}</option>`).join("");
    const onchange = opts.onchange ? `onchange="${escapeAttr(opts.onchange)}"` : "";
    return field(labelFor(id), `<select id="f-${id}" ${onchange}>${optionsHtml}</select>`);
  }
  // Like selectField, but for the rare case where the real device's option
  // *value* isn't fit for display (e.g. SRT's encryption key length is a
  // numeric key size, not a name) - pairs is [[value, label], ...].
  function labeledSelectField(id, value, pairs, opts = {}) {
    const optionsHtml = opts.batch
      ? `<option value="" selected>— leave unchanged —</option>` +
        pairs.map(([v, l]) => `<option value="${escapeAttr(v)}">${escapeHtml(l)}</option>`).join("")
      : pairs
          .map(([v, l]) => `<option value="${escapeAttr(v)}" ${v === (value ?? "") ? "selected" : ""}>${escapeHtml(l)}</option>`)
          .join("");
    return field(labelFor(id), `<select id="f-${id}">${optionsHtml}</select>`);
  }
  function labelFor(id) {
    return id.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
  }
  function saveRow() {
    return `<div class="save-row"><button class="btn btn-primary" id="save-btn">Save</button><span class="save-status" id="save-status"></span></div>`;
  }
  function val(id) {
    return document.getElementById(`f-${id}`).value;
  }
  function checked(id) {
    return document.getElementById(`f-${id}`).checked;
  }
  function csvToList(s) {
    return s.split(",").map((x) => x.trim()).filter(Boolean);
  }

  // ---------- Status tab (read-only) ----------

  function statusView(s) {
    const rows = Object.entries(s)
      .map(([k, v]) => `<label><span style="color:var(--text-dim);font-size:0.75rem">${escapeHtml(labelFor(k))}</span><div>${escapeHtml(String(v))}</div></label>`)
      .join("");
    return `<div class="settings-card"><h3>Dashboard</h3><div class="field-grid">${rows}</div></div>`;
  }

  // ---------- Network tab ----------

  function networkForm(s, opts = {}) {
    return `
      <div class="settings-card">
        <h3>Network${opts.batch ? " (batch)" : ""}</h3>
        <div class="field-grid">
          ${selectField("config_method", s.config_method, ["dhcp", "static"], opts)}
          ${textField("ip_address", s.ip_address, opts)}
          ${textField("subnet_mask", s.subnet_mask, opts)}
          ${textField("gateway_address", s.gateway_address, opts)}
          ${numberField("dhcp_timeout_secs", s.dhcp_timeout_secs, opts)}
          ${textField("fallback_ip_address", s.fallback_ip_address, opts)}
          ${textField("fallback_subnet_mask", s.fallback_subnet_mask, opts)}
          ${textField("birddog_name", s.birddog_name, opts)}
          ${selectField("ndi_transmit_method", s.ndi_transmit_method, ["TCP", "UDP", "Multicast", "RUDP"], opts)}
          ${selectField("ndi_receive_method", s.ndi_receive_method, ["TCP", "UDP", "Multicast", "RUDP"], opts)}
          ${textField("multicast_net_prefix", s.multicast_net_prefix, opts)}
          ${textField("multicast_net_mask", s.multicast_net_mask, opts)}
          ${numberField("multicast_ttl", s.multicast_ttl, opts)}
          ${checkboxField("ndi_discovery_server_enabled", s.ndi_discovery_server_enabled, opts)}
          ${textField("ndi_discovery_server_ips", (s.ndi_discovery_server_ips || []).join(", "), opts)}
        </div>
      </div>
      ${saveRow()}`;
  }
  function collectNetworkForm(opts = {}) {
    if (!opts.batch) {
      return {
        config_method: val("config_method"),
        ip_address: val("ip_address"),
        subnet_mask: val("subnet_mask"),
        gateway_address: val("gateway_address"),
        dhcp_timeout_secs: Number(val("dhcp_timeout_secs")),
        fallback_ip_address: val("fallback_ip_address"),
        fallback_subnet_mask: val("fallback_subnet_mask"),
        birddog_name: val("birddog_name"),
        ndi_transmit_method: val("ndi_transmit_method"),
        ndi_receive_method: val("ndi_receive_method"),
        multicast_net_prefix: val("multicast_net_prefix"),
        multicast_net_mask: val("multicast_net_mask"),
        multicast_ttl: Number(val("multicast_ttl")),
        ndi_discovery_server_enabled: checked("ndi_discovery_server_enabled"),
        ndi_discovery_server_ips: csvToList(val("ndi_discovery_server_ips")),
      };
    }
    const patch = {};
    setIfPresent(patch, "config_method", val("config_method"));
    setIfPresent(patch, "ip_address", val("ip_address"));
    setIfPresent(patch, "subnet_mask", val("subnet_mask"));
    setIfPresent(patch, "gateway_address", val("gateway_address"));
    setIfPresentNumber(patch, "dhcp_timeout_secs", val("dhcp_timeout_secs"));
    setIfPresent(patch, "fallback_ip_address", val("fallback_ip_address"));
    setIfPresent(patch, "fallback_subnet_mask", val("fallback_subnet_mask"));
    setIfPresent(patch, "birddog_name", val("birddog_name"));
    setIfPresent(patch, "ndi_transmit_method", val("ndi_transmit_method"));
    setIfPresent(patch, "ndi_receive_method", val("ndi_receive_method"));
    setIfPresent(patch, "multicast_net_prefix", val("multicast_net_prefix"));
    setIfPresent(patch, "multicast_net_mask", val("multicast_net_mask"));
    setIfPresentNumber(patch, "multicast_ttl", val("multicast_ttl"));
    setIfPresentBool(patch, "ndi_discovery_server_enabled", val("ndi_discovery_server_enabled"));
    setIfPresentList(patch, "ndi_discovery_server_ips", val("ndi_discovery_server_ips"));
    return patch;
  }

  // ---------- Decode tab ----------

  // Which of the NDI/SRT field groups is shown follows `source_type`,
  // mirroring BirdUI's own "Source Selection" toggle (screenshotted from a
  // real, firmware-updated Play - see docs/architecture.md for how much of
  // the underlying field mapping is still unconfirmed).
  function toggleSourceType() {
    const t = document.getElementById("f-source_type").value;
    const ndi = document.getElementById("ndi-source-fields");
    const srt = document.getElementById("srt-source-fields");
    if (ndi) ndi.style.display = t === "SRT" ? "none" : "";
    if (srt) srt.style.display = t === "SRT" ? "" : "none";
  }
  window.toggleSourceType = toggleSourceType;

  function decodeForm(s, opts = {}) {
    const sourceType = opts.batch ? "" : s.source_type || "NDI";
    return `
      <div class="settings-card">
        <h3>Decode source${opts.batch ? " (batch)" : ""}</h3>
        <div class="field-grid">
          ${selectField("source_type", sourceType, ["NDI", "SRT"], { ...opts, onchange: "toggleSourceType()" })}
        </div>
        <div class="field-grid" id="ndi-source-fields">
          ${textField("selected_source", s.selected_source, { ...opts, list: "ndi-sources-datalist" })}
          ${textField("failover_source", s.failover_source, { ...opts, list: "ndi-sources-datalist" })}
        </div>
        <div class="field-grid" id="srt-source-fields">
          ${selectField("srt_connection_type", opts.batch ? "" : s.srt_connection_type || "caller", ["caller", "listener", "rendezvous"], opts)}
          ${textField("srt_stream_name", s.srt_stream_name, opts)}
          ${textField("srt_ip_address", s.srt_ip_address, opts)}
          ${numberField("srt_port", s.srt_port, opts)}
          ${numberField("srt_latency_ms", s.srt_latency_ms, opts)}
          ${checkboxField("srt_encryption_enabled", s.srt_encryption_enabled, opts)}
          ${labeledSelectField("srt_encryption_key_length", opts.batch ? null : s.srt_encryption_key_length, [["", "None"], ["32", "AES-256"], ["24", "AES-192"], ["16", "AES-128"]], opts)}
          ${textField("srt_passphrase", s.srt_passphrase, opts)}
          ${textField("srt_stream_id", s.srt_stream_id, opts)}
        </div>
        <div class="field-grid">
          ${selectField("screensaver_mode", s.screensaver_mode, ["CaptureSS", "BlackSS", "BirdDogSS"], opts)}
          ${selectField("color_space", s.color_space, ["YUV", "RGB"], opts)}
          ${checkboxField("ndi_audio_enabled", s.ndi_audio_enabled, opts)}
          ${selectField("tally_mode", s.tally_mode, ["TallyOn", "TallyOff", "VideoMode"], opts)}
        </div>
      </div>
      ${saveRow()}`;
  }
  function collectDecodeForm(opts = {}) {
    if (!opts.batch) {
      return {
        source_type: val("source_type"),
        selected_source: val("selected_source") || null,
        available_sources: [],
        failover_source: val("failover_source") || null,
        srt_connection_type: val("srt_connection_type"),
        srt_stream_name: val("srt_stream_name") || null,
        srt_ip_address: val("srt_ip_address") || null,
        srt_port: val("srt_port") ? Number(val("srt_port")) : null,
        srt_latency_ms: Number(val("srt_latency_ms")),
        srt_encryption_enabled: checked("srt_encryption_enabled"),
        srt_encryption_key_length: val("srt_encryption_key_length") || null,
        srt_passphrase: val("srt_passphrase") || null,
        srt_stream_id: val("srt_stream_id") || null,
        srt_available_sources: [],
        screensaver_mode: val("screensaver_mode"),
        color_space: val("color_space"),
        ndi_audio_enabled: checked("ndi_audio_enabled"),
        tally_mode: val("tally_mode"),
      };
    }
    const patch = {};
    setIfPresent(patch, "source_type", val("source_type"));
    setIfPresent(patch, "selected_source", val("selected_source"));
    setIfPresent(patch, "failover_source", val("failover_source"));
    setIfPresent(patch, "srt_connection_type", val("srt_connection_type"));
    setIfPresent(patch, "srt_stream_name", val("srt_stream_name"));
    setIfPresent(patch, "srt_ip_address", val("srt_ip_address"));
    setIfPresentNumber(patch, "srt_port", val("srt_port"));
    setIfPresentNumber(patch, "srt_latency_ms", val("srt_latency_ms"));
    setIfPresentBool(patch, "srt_encryption_enabled", val("srt_encryption_enabled"));
    setIfPresent(patch, "srt_encryption_key_length", val("srt_encryption_key_length"));
    setIfPresent(patch, "srt_passphrase", val("srt_passphrase"));
    setIfPresent(patch, "srt_stream_id", val("srt_stream_id"));
    setIfPresent(patch, "screensaver_mode", val("screensaver_mode"));
    setIfPresent(patch, "color_space", val("color_space"));
    setIfPresentBool(patch, "ndi_audio_enabled", val("ndi_audio_enabled"));
    setIfPresent(patch, "tally_mode", val("tally_mode"));
    return patch;
  }

  // ---------- System tab ----------

  function systemForm(s, opts = {}) {
    return `
      <div class="settings-card">
        <h3>System${opts.batch ? " (batch)" : ""}</h3>
        <div class="field-grid">
          ${textField("firmware_version", s.firmware_version, { ...opts, readOnly: true })}
          ${textField("remote_ip_list", (s.remote_ip_list || []).join(", "), opts)}
          ${textField("ndi_group_list", (s.ndi_group_list || []).join(", "), opts)}
        </div>
      </div>
      ${saveRow()}`;
  }
  function collectSystemForm(opts = {}) {
    if (!opts.batch) {
      return {
        firmware_version: val("firmware_version"),
        remote_ip_list: csvToList(val("remote_ip_list")),
        ndi_group_list: csvToList(val("ndi_group_list")),
      };
    }
    const patch = {};
    setIfPresentList(patch, "remote_ip_list", val("remote_ip_list"));
    setIfPresentList(patch, "ndi_group_list", val("ndi_group_list"));
    return patch;
  }

  // ---------- right panel: discovery + add/remove + local settings ----------

  // flock's own centralized NDI source list (see docs/architecture.md) -
  // suggests values for the Decode tab's Selected/Failover Source fields
  // via a <datalist>, refreshed alongside a device scan.
  async function loadNdiSources() {
    try {
      const sources = await api("/api/ndi/sources");
      el("ndi-sources-datalist").innerHTML = sources
        .map((s) => `<option value="${escapeAttr(s.name)}">`)
        .join("");
    } catch (err) {
      // Non-fatal - the source fields just fall back to plain free-text
      // entry with no suggestions.
    }
  }

  async function runScan() {
    const container = el("discovery-results");
    container.innerHTML = `<div class="discovery-empty">Scanning…</div>`;
    loadNdiSources();
    try {
      const found = await api("/api/discovery/scan");
      if (found.length === 0) {
        container.innerHTML = `<div class="discovery-empty">No new devices found.</div>`;
        return;
      }
      container.innerHTML = found
        .map(
          (h) => `<div class="discovery-item"><span class="host">${escapeHtml(h.name)} — ${escapeHtml(h.host)}</span><button class="btn" data-host="${escapeAttr(h.host)}" data-name="${escapeAttr(h.name)}">Add</button></div>`
        )
        .join("");
      container.querySelectorAll("button[data-host]").forEach((btn) => {
        btn.addEventListener("click", () => {
          el("df-name").value = btn.dataset.name;
          el("df-host").value = btn.dataset.host;
          el("device-form-id").value = "";
          el("df-submit").textContent = "Add device";
          document.getElementById("add-device-heading").scrollIntoView({ behavior: "smooth" });
        });
      });
    } catch (err) {
      container.innerHTML = `<div class="discovery-empty">Scan failed: ${escapeHtml(err.message)}</div>`;
    }
  }

  function resetDeviceForm() {
    state.editingId = null;
    el("device-form-id").value = "";
    el("device-form").reset();
    el("df-submit").textContent = "Add device";
    el("df-cancel").hidden = true;
    el("add-device-heading").textContent = "Add device manually";
  }

  function startEditDevice(device) {
    state.editingId = idStr(device.id);
    el("device-form-id").value = state.editingId;
    el("df-name").value = device.name;
    el("df-host").value = device.host;
    el("df-tags").value = device.tags.join(", ");
    el("df-password").value = "";
    el("df-submit").textContent = "Save changes";
    el("df-cancel").hidden = false;
    el("add-device-heading").textContent = "Edit device";
  }

  async function submitDeviceForm(evt) {
    evt.preventDefault();
    const body = {
      name: el("df-name").value.trim(),
      host: el("df-host").value.trim(),
      tags: csvToList(el("df-tags").value),
      password: el("df-password").value || null,
      discovered: false,
    };
    const editingId = el("device-form-id").value;
    try {
      if (editingId) {
        await api(`/api/devices/${editingId}`, {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(body),
        });
      } else {
        await api("/api/devices", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(body),
        });
      }
      resetDeviceForm();
      await loadState();
    } catch (err) {
      alert(`Failed to save device: ${err.message}`);
    }
  }

  async function removeSelectedDevice() {
    if (!state.selectedId) return;
    const device = findDevice(state.selectedId);
    if (!device) return;
    if (!confirm(`Remove "${device.name}" from flock? This only forgets it locally — the physical device is unaffected.`)) return;
    await api(`/api/devices/${state.selectedId}`, { method: "DELETE" });
    state.selectedId = null;
    await loadState();
  }

  async function rebootSelectedDevice() {
    if (!state.selectedId) return;
    await api(`/api/devices/${state.selectedId}/reboot`, { method: "POST" });
    alert("Reboot command sent.");
  }

  function exportRegistry() {
    const blob = new Blob([JSON.stringify(state.devices, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "flock-registry.json";
    a.click();
    URL.revokeObjectURL(url);
  }

  async function importRegistry(file) {
    const text = await file.text();
    const devices = JSON.parse(text);
    for (const d of devices) {
      await api("/api/devices", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name: d.name,
          host: d.host,
          tags: d.tags || [],
          password: null,
          discovered: false,
        }),
      });
    }
    await loadState();
  }

  // ---------- theme ----------

  function applyTheme(theme) {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("flock-theme", theme);
  }

  // ---------- render orchestration ----------

  function render() {
    renderGroupTree();
    renderCenter();
  }

  // ---------- app settings (NDI discovery server) ----------

  async function loadAppSettings() {
    try {
      const settings = await api("/api/settings");
      el("discovery-server-input").value = settings.discovery_server || "";
    } catch (err) {
      // Non-fatal - leave the field blank/whatever the user last typed.
    }
  }

  async function saveDiscoveryServer() {
    const statusEl = el("discovery-server-status");
    const value = el("discovery-server-input").value.trim();
    try {
      await api("/api/settings", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ discovery_server: value || null }),
      });
      statusEl.textContent = "Saved";
      statusEl.classList.add("visible");
      setTimeout(() => statusEl.classList.remove("visible"), 1500);
    } catch (err) {
      statusEl.textContent = `Error: ${err.message}`;
      statusEl.classList.add("visible");
    }
  }

  async function pushDiscoveryServer() {
    const statusEl = el("discovery-server-status");
    try {
      const outcomes = await api("/api/settings/push-discovery-server", { method: "POST" });
      const failed = outcomes.filter((o) => !o.ok);
      statusEl.textContent =
        failed.length === 0
          ? `Pushed to ${outcomes.length} device${outcomes.length === 1 ? "" : "s"}`
          : `Pushed to ${outcomes.length - failed.length}/${outcomes.length} — failed: ${failed.map((f) => f.device_name).join(", ")}`;
      statusEl.classList.add("visible");
    } catch (err) {
      statusEl.textContent = `Error: ${err.message}`;
      statusEl.classList.add("visible");
    }
  }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
  }
  function escapeAttr(s) {
    return escapeHtml(s);
  }

  // ---------- auth ----------
  //
  // flock's own login gate is optional (off unless the operator configures
  // admin_password) - a plain GET on /api/state is enough to tell which
  // mode we're in: 401 means a session is required, anything else means
  // there's no gate at all and the app can start immediately.

  async function isAuthed() {
    const res = await fetch("/api/state");
    return res.status !== 401;
  }

  function showLoginScreen() {
    el("app").hidden = true;
    el("login-screen").hidden = false;
    el("login-password").focus();
  }

  function hideLoginScreen() {
    el("login-screen").hidden = true;
    el("app").hidden = false;
    el("logout-btn").hidden = false;
  }

  async function submitLogin(e) {
    e.preventDefault();
    const errorEl = el("login-error");
    errorEl.textContent = "";
    try {
      await api("/api/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ password: el("login-password").value }),
      });
      el("login-password").value = "";
      hideLoginScreen();
      startApp();
    } catch (err) {
      errorEl.textContent = "Incorrect password";
    }
  }

  async function logout() {
    await fetch("/api/logout", { method: "POST" }).catch(() => {});
    location.reload();
  }

  // ---------- wiring ----------

  async function init() {
    el("login-form").addEventListener("submit", submitLogin);
    el("logout-btn").addEventListener("click", logout);

    if (!(await isAuthed())) {
      showLoginScreen();
      return;
    }
    hideLoginScreen();
    startApp();
  }

  function startApp() {
    const savedTheme = localStorage.getItem("flock-theme") || "dark";
    el("theme-select").value = savedTheme;
    applyTheme(savedTheme);
    el("theme-select").addEventListener("change", (e) => applyTheme(e.target.value));

    el("search").addEventListener("input", (e) => {
      state.search = e.target.value;
      renderGroupTree();
    });

    el("tab-bar").querySelectorAll(".tab-btn").forEach((btn) => {
      btn.addEventListener("click", () => {
        state.activeTab = btn.dataset.tab;
        renderCenter();
      });
    });

    el("scan-btn").addEventListener("click", runScan);
    el("device-form").addEventListener("submit", submitDeviceForm);
    el("df-cancel").addEventListener("click", resetDeviceForm);
    el("edit-device-btn").addEventListener("click", () => {
      const device = findDevice(state.selectedId);
      if (device) startEditDevice(device);
    });
    el("remove-device-btn").addEventListener("click", removeSelectedDevice);
    el("reboot-device-btn").addEventListener("click", rebootSelectedDevice);
    el("export-btn").addEventListener("click", exportRegistry);
    el("import-file").addEventListener("change", (e) => {
      if (e.target.files[0]) importRegistry(e.target.files[0]);
    });
    el("save-discovery-server-btn").addEventListener("click", saveDiscoveryServer);
    el("push-discovery-server-btn").addEventListener("click", pushDiscoveryServer);

    loadAppSettings();
    loadNdiSources();
    loadState().then(connectWs);
  }

  document.addEventListener("DOMContentLoaded", init);
})();
