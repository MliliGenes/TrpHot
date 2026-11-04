"use strict";
const $ = (id) => document.getElementById(id);
const token = location.hash.slice(1) || sessionStorage.getItem("trphot-token") || "";
if (token) sessionStorage.setItem("trphot-token", token);
history.replaceState(null, "", location.pathname);
let state = {}, events = [], configKey = "", filesKey = "", activityKey = "";
let dirty = false, awaitingConfig = false, connected = false;
const form = $("rules");
function notice(message) { $("notice").textContent = message; $("notice").hidden = false; }
async function command(cmd, extra = {}) {
  try {
    const response = await fetch("/api/command", { method: "POST", headers: { "Content-Type": "application/json", "X-Trphot-Token": token }, body: JSON.stringify({ cmd, ...extra }) });
    if (!response.ok) throw new Error(await response.text());
    $("notice").hidden = true;
    return true;
  } catch (error) { notice(error.message); return false; }
}
document.querySelectorAll("[data-command]").forEach(button => button.addEventListener("click", () => command(button.dataset.command)));
form.addEventListener("input", () => { dirty = true; $("rules-status").textContent = "Unsaved edits · apply to update the engine"; });
form.addEventListener("submit", async event => {
  event.preventDefault();
  const data = new FormData(form);
  const config = { targets: data.get("targets").trim().split(/\s+/), makefile: data.get("makefile"), exec: data.get("exec"), ignores: data.get("ignores").split("\n").map(x => x.trim()).filter(Boolean), interval: Number(data.get("interval")), debounce: Number(data.get("debounce")) };
  if (await command("config", { config })) { awaitingConfig = JSON.stringify(config); $("rules-status").textContent = "Sent to engine; waiting for validation…"; }
});
function renderFiles() {
  const search = $("file-search").value.toLowerCase();
  const files = (state.files || []).filter(file => file.path.toLowerCase().includes(search));
  const fragment = document.createDocumentFragment();
  for (const file of files.slice(0, 1000)) {
    const row = document.createElement("div"); row.className = `file${file.ignored ? " ignored" : ""}`;
    const path = document.createElement("span"); path.textContent = file.path;
    const size = document.createElement("small"); size.textContent = file.ignored ? "ignored" : `${file.size ?? 0} B`;
    row.append(path, size); fragment.append(row);
  }
  if (!files.length || files.length > 1000) { const note = document.createElement("p"); note.className = "empty"; note.textContent = files.length ? "Showing first 1,000 matches. Filter to narrow the list." : "No matching files."; fragment.append(note); }
  $("files").replaceChildren(fragment);
}
function renderActivity() {
  const filter = $("activity-filter").value;
  const visible = events.filter(event => filter === "all" || event.type === filter);
  const fragment = document.createDocumentFragment();
  for (const event of visible) {
    const row = document.createElement("div"); row.className = `event${event.success === false ? " failed" : ""}`;
    const tag = document.createElement("span"); tag.className = "tag"; tag.textContent = event.source || event.kind || "engine";
    const text = document.createElement("span"); text.className = "text"; text.textContent = event.text ?? event.path ?? event.message ?? "";
    row.append(tag, text); fragment.append(row);
  }
  if (!visible.length) { const empty = document.createElement("p"); empty.className = "empty"; empty.textContent = "Watching for your next change."; fragment.append(empty); }
  const scroll = $("activity").scrollTop;
  $("activity").replaceChildren(fragment);
  $("activity").scrollTop = $("follow").checked ? $("activity").scrollHeight : scroll;
}
$("file-search").addEventListener("input", renderFiles);
$("activity-filter").addEventListener("change", renderActivity);
async function poll() {
  try {
    const response = await fetch("/api/state", { headers: { "X-Trphot-Token": token }, signal: AbortSignal.timeout(4000) });
    if (!response.ok) throw new Error(response.status === 403 ? "Open the full dashboard URL from your terminal." : "Dashboard unavailable");
    const data = await response.json(); state = data.state; events = data.events; connected = true;
    $("connection").textContent = "● Engine connected";
    $("root").textContent = state.root || "Initializing project…";
    $("mode").textContent = state.active ? "Automatic" : "Spectator";
    $("schedule").textContent = state.paused ? "Scheduling paused" : state.active ? "Build on file changes" : "Watching without building";
    $("arm").textContent = state.active ? "Disable auto-build" : "Enable auto-build";
    $("pause").textContent = state.paused ? "Resume" : "Pause";
    $("file-count").textContent = (state.files || []).filter(file => !file.ignored).length.toLocaleString();
    $("ignored-count").textContent = `${(state.files || []).filter(file => file.ignored).length} ignored`;
    $("build-status").textContent = state.building ? "Building…" : state.success === true ? "Passed" : state.success === false ? "Failed" : "Idle";
    $("changes").textContent = events.filter(event => event.type === "change").length;
    if (state.config) {
      const key = JSON.stringify(state.config);
      // Compare fields independently of the JSON object's key ordering.
      if (awaitingConfig && Object.entries(JSON.parse(awaitingConfig)).every(([k, v]) => JSON.stringify(state.config[k]) === JSON.stringify(v))) {
        dirty = false; awaitingConfig = false; $("rules-status").textContent = "Rules applied. Save to persist project defaults.";
      } else if (awaitingConfig) { $("rules-status").textContent = state.message || "Waiting for validation…"; }
      if (!dirty && key !== configKey) {
        for (const [name, value] of Object.entries(state.config)) form.elements[name].value = Array.isArray(value) ? value.join(name === "ignores" ? "\n" : " ") : value;
        configKey = key;
      }
    }
    const nextFiles = JSON.stringify(state.files);
    if (nextFiles !== filesKey) { filesKey = nextFiles; renderFiles(); }
    const nextActivity = events.at(-1)?.id || 0;
    if (nextActivity !== activityKey) { activityKey = nextActivity; renderActivity(); }
  } catch (error) { connected = false; $("connection").textContent = "○ Disconnected · retrying"; if (!token || error.message.includes("full dashboard")) notice(error.message); }
  document.querySelectorAll("button").forEach(button => { button.disabled = !connected; });
  setTimeout(poll, 500);
}
poll();
