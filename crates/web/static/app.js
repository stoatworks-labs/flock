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

  function matchesFilter(device) {
    if (state.activeGroup && !device.tags.includes(state.activeGroup)) return false;
    if (!state.search) return true;
    const needle = state.search.toLowerCase();
    return (
      device.name.toLowerCase().includes(needle) ||
      device.host.toLowerCase().includes(needle) ||
      device.tags.some((t) => t.toLowerCase().includes(needle))
    );
  }

  function renderGroups() {
    const container = el("groups-list");
    const tags = Object.keys(state.groups).sort();
    const chips = [`<button class="chip ${state.activeGroup === "" ? "chip-active" : ""}" data-group="">All devices</button>`];
    for (const tag of tags) {
      const active = state.activeGroup === tag ? "chip-active" : "";
      chips.push(`<button class="chip ${active}" data-group="${escapeAttr(tag)}">${escapeHtml(tag)} (${state.groups[tag].length})</button>`);
    }
    container.innerHTML = chips.join("");
    container.querySelectorAll(".chip").forEach((btn) => {
      btn.addEventListener("click", () => {
        state.activeGroup = btn.dataset.group;
        render();
      });
    });

    const batchBtn = el("batch-edit-btn");
    if (state.activeGroup) {
      batchBtn.disabled = false;
      batchBtn.textContent = `Batch edit "${state.activeGroup}" (${(state.groups[state.activeGroup] || []).length})`;
    } else {
      batchBtn.disabled = true;
      batchBtn.textContent = "Batch edit group…";
    }
  }

  function renderDeviceList() {
    const container = el("device-list");
    const visible = state.devices.filter(matchesFilter);
    if (visible.length === 0) {
      container.innerHTML = `<div class="devices-empty">No devices match.</div>`;
      return;
    }
    container.innerHTML = visible
      .map(
        (d) => `
      <div class="device-row ${d.id === state.selectedId ? "selected" : ""}" data-id="${escapeAttr(idStr(d.id))}">
        <span class="status-dot"></span>
        <span class="name-block">
          <span class="name">${escapeHtml(d.name)}</span>
          <span class="host">${escapeHtml(d.host)}</span>
        </span>
      </div>`
      )
      .join("");
    container.querySelectorAll(".device-row").forEach((row) => {
      row.addEventListener("click", () => selectDevice(row.dataset.id));
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

  function startBatchEdit() {
    if (!state.activeGroup) return;
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
      preview.innerHTML = `<div id="preview-placeholder">Select a device to view its preview and settings</div>`;
      el("tab-content").innerHTML = "";
      return;
    }

    tabBar.hidden = false;
    selectedActions.hidden = false;
    preview.innerHTML = `<div id="preview-placeholder">${escapeHtml(device.name)} — live preview needs real hardware (Phase 2)</div>`;

    tabBar.querySelectorAll(".tab-btn").forEach((btn) => {
      btn.classList.toggle("tab-active", btn.dataset.tab === state.activeTab);
    });

    renderTabContent(device);
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
    return field(labelFor(id), `<input id="f-${id}" type="text" value="${escapeAttr(v)}" placeholder="${escapeAttr(placeholder)}" ${readOnly}>`);
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

  function decodeForm(s, opts = {}) {
    return `
      <div class="settings-card">
        <h3>Decode source${opts.batch ? " (batch)" : ""}</h3>
        <div class="field-grid">
          ${textField("selected_source", s.selected_source, opts)}
          ${textField("failover_source", s.failover_source, opts)}
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
        selected_source: val("selected_source") || null,
        available_sources: [],
        failover_source: val("failover_source") || null,
        screensaver_mode: val("screensaver_mode"),
        color_space: val("color_space"),
        ndi_audio_enabled: checked("ndi_audio_enabled"),
        tally_mode: val("tally_mode"),
      };
    }
    const patch = {};
    setIfPresent(patch, "selected_source", val("selected_source"));
    setIfPresent(patch, "failover_source", val("failover_source"));
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

  async function runScan() {
    const container = el("discovery-results");
    container.innerHTML = `<div class="discovery-empty">Scanning…</div>`;
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
    renderGroups();
    renderDeviceList();
    renderCenter();
  }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
  }
  function escapeAttr(s) {
    return escapeHtml(s);
  }

  // ---------- wiring ----------

  function init() {
    const savedTheme = localStorage.getItem("flock-theme") || "dark";
    el("theme-select").value = savedTheme;
    applyTheme(savedTheme);
    el("theme-select").addEventListener("change", (e) => applyTheme(e.target.value));

    el("search").addEventListener("input", (e) => {
      state.search = e.target.value;
      renderDeviceList();
    });

    el("tab-bar").querySelectorAll(".tab-btn").forEach((btn) => {
      btn.addEventListener("click", () => {
        state.activeTab = btn.dataset.tab;
        renderCenter();
      });
    });

    el("batch-edit-btn").addEventListener("click", startBatchEdit);
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

    loadState().then(connectWs);
  }

  document.addEventListener("DOMContentLoaded", init);
})();
