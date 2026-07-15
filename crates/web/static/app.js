(() => {
  "use strict";

  const state = {
    devices: [],
    groups: {},
    activeGroup: "",
    search: "",
    selectedId: null,
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
        <span class="mode-badge">${d.mode}</span>
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
    state.activeTab = "status";
    render();
  }

  function renderCenter() {
    const device = state.selectedId ? findDevice(state.selectedId) : null;
    const tabBar = el("tab-bar");
    const preview = el("preview-box");
    const selectedActions = el("selected-actions");

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
      } else if (state.activeTab === "encode") {
        const settings = await api(`/api/devices/${id}/encode`);
        container.innerHTML = encodeForm(settings);
        wireSaveForm(id, "encode", collectEncodeForm);
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

  function field(label, inner) {
    return `<label>${escapeHtml(label)}${inner}</label>`;
  }
  function textField(id, value) {
    return field(labelFor(id), `<input id="f-${id}" type="text" value="${escapeAttr(value ?? "")}">`);
  }
  function numberField(id, value) {
    return field(labelFor(id), `<input id="f-${id}" type="number" value="${escapeAttr(value ?? 0)}">`);
  }
  function checkboxField(id, checked) {
    return `<label class="inline-label"><span>${escapeHtml(labelFor(id))}</span><input id="f-${id}" type="checkbox" ${checked ? "checked" : ""}></label>`;
  }
  function selectField(id, value, options) {
    const opts = options.map((o) => `<option value="${escapeAttr(o)}" ${o === value ? "selected" : ""}>${escapeHtml(o)}</option>`).join("");
    return field(labelFor(id), `<select id="f-${id}">${opts}</select>`);
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

  function networkForm(s) {
    return `
      <div class="settings-card">
        <h3>Network</h3>
        <div class="field-grid">
          ${selectField("config_method", s.config_method, ["dhcp", "static"])}
          ${textField("ip_address", s.ip_address)}
          ${textField("subnet_mask", s.subnet_mask)}
          ${textField("gateway_address", s.gateway_address)}
          ${numberField("dhcp_timeout_secs", s.dhcp_timeout_secs)}
          ${textField("fallback_ip_address", s.fallback_ip_address)}
          ${textField("fallback_subnet_mask", s.fallback_subnet_mask)}
          ${textField("birddog_name", s.birddog_name)}
          ${checkboxField("wifi_enabled", s.wifi_enabled)}
          ${selectField("ndi_transmit_method", s.ndi_transmit_method, ["TCP", "UDP", "R-UDP"])}
          ${textField("multicast_net_prefix", s.multicast_net_prefix)}
          ${textField("multicast_net_mask", s.multicast_net_mask)}
          ${numberField("multicast_ttl", s.multicast_ttl)}
          ${checkboxField("ndi_discovery_server_enabled", s.ndi_discovery_server_enabled)}
          ${textField("ndi_discovery_server_ips", (s.ndi_discovery_server_ips || []).join(", "))}
        </div>
      </div>
      ${saveRow()}`;
  }
  function collectNetworkForm() {
    return {
      config_method: val("config_method"),
      ip_address: val("ip_address"),
      subnet_mask: val("subnet_mask"),
      gateway_address: val("gateway_address"),
      dhcp_timeout_secs: Number(val("dhcp_timeout_secs")),
      fallback_ip_address: val("fallback_ip_address"),
      fallback_subnet_mask: val("fallback_subnet_mask"),
      birddog_name: val("birddog_name"),
      wifi_enabled: checked("wifi_enabled"),
      ndi_transmit_method: val("ndi_transmit_method"),
      multicast_net_prefix: val("multicast_net_prefix"),
      multicast_net_mask: val("multicast_net_mask"),
      multicast_ttl: Number(val("multicast_ttl")),
      ndi_discovery_server_enabled: checked("ndi_discovery_server_enabled"),
      ndi_discovery_server_ips: csvToList(val("ndi_discovery_server_ips")),
    };
  }

  // ---------- Encode tab ----------

  function encodeForm(s) {
    return `
      <div class="settings-card">
        <h3>Primary encode (NDI HX / UVC)</h3>
        <div class="field-grid">
          ${selectField("primary_protocol", s.primary_protocol, ["ndihx", "uvc"])}
          ${checkboxField("primary_enabled", s.primary_enabled)}
          ${textField("ndi_stream_name", s.ndi_stream_name)}
          ${textField("ndi_groups", (s.ndi_groups || []).join(", "))}
          ${textField("video_format", s.video_format)}
          ${textField("video_compression", s.video_compression)}
          ${textField("bitrate_mode", s.bitrate_mode)}
          ${numberField("bitrate_kbps", s.bitrate_kbps)}
        </div>
      </div>
      <div class="settings-card">
        <h3>Secondary stream (SRT / RTMP-RTSP)</h3>
        <div class="field-grid">
          ${selectField("secondary_protocol", s.secondary_protocol, ["none", "srt", "rtmprtsp"])}
          ${selectField("secondary_connection_type", s.secondary_connection_type, ["caller", "listener"])}
          ${numberField("secondary_port", s.secondary_port)}
          ${numberField("secondary_latency_ms", s.secondary_latency_ms)}
          ${textField("secondary_encryption", s.secondary_encryption)}
          ${textField("secondary_passphrase", s.secondary_passphrase)}
          ${textField("secondary_connection_url", s.secondary_connection_url)}
        </div>
      </div>
      ${saveRow()}`;
  }
  function collectEncodeForm() {
    return {
      primary_protocol: val("primary_protocol"),
      primary_enabled: checked("primary_enabled"),
      ndi_stream_name: val("ndi_stream_name"),
      ndi_groups: csvToList(val("ndi_groups")),
      video_format: val("video_format"),
      video_compression: val("video_compression"),
      bitrate_mode: val("bitrate_mode"),
      bitrate_kbps: Number(val("bitrate_kbps")),
      secondary_protocol: val("secondary_protocol"),
      secondary_connection_type: val("secondary_connection_type"),
      secondary_port: Number(val("secondary_port")),
      secondary_latency_ms: Number(val("secondary_latency_ms")),
      secondary_encryption: val("secondary_encryption"),
      secondary_passphrase: val("secondary_passphrase") || null,
      secondary_connection_url: val("secondary_connection_url") || null,
    };
  }

  // ---------- Decode tab ----------

  function decodeForm(s) {
    return `
      <div class="settings-card">
        <h3>Decode source</h3>
        <div class="field-grid">
          ${textField("selected_source", s.selected_source)}
          ${textField("available_sources", (s.available_sources || []).join(", "))}
          ${textField("failover_source", s.failover_source)}
          ${textField("screensaver_mode", s.screensaver_mode)}
        </div>
      </div>
      ${saveRow()}`;
  }
  function collectDecodeForm() {
    return {
      selected_source: val("selected_source") || null,
      available_sources: csvToList(val("available_sources")),
      failover_source: val("failover_source") || null,
      screensaver_mode: val("screensaver_mode"),
    };
  }

  // ---------- System tab ----------

  function systemForm(s) {
    return `
      <div class="settings-card">
        <h3>System</h3>
        <div class="field-grid">
          ${textField("firmware_version", s.firmware_version)}
          ${textField("remote_ip_list", (s.remote_ip_list || []).join(", "))}
          ${textField("ndi_group_list", (s.ndi_group_list || []).join(", "))}
          ${selectField("ui_mode", s.ui_mode, ["Dark", "Light"])}
        </div>
      </div>
      ${saveRow()}`;
  }
  function collectSystemForm() {
    return {
      firmware_version: val("firmware_version"),
      remote_ip_list: csvToList(val("remote_ip_list")),
      ndi_group_list: csvToList(val("ndi_group_list")),
      ui_mode: val("ui_mode"),
    };
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
    el("df-mode").value = device.mode;
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
      mode: el("df-mode").value,
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
          mode: d.mode,
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
