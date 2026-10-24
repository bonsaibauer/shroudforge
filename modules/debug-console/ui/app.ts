const levels = ["All", "Trace", "Debug", "Info", "Warning", "Error"];
// Lines kept in the list; older ones leave it as new ones arrive. The full
// log tail stays available to filters, search and Copy Logs.
const MAX_SHOWN_LINES = 2000;
const emptyLog = () => ({ text: "", state: "missing", lines: 0 });
const data = { game: emptyLog(), loader: emptyLog() };
const paths = { game: "enshrouded.log", loader: "shroudforge/logs/shroudforge-client.log" };
let activeSource = "game";
let levelIndex = 0;
let minimumLevel = "INFO";
let alertReview = false;
let paused = false;
let follow = true;
let frozen = "";
const savePreferences = () => window.ipc.postMessage(JSON.stringify({defaultSource:activeSource,levelFilter:["ALL","TRACE","DEBUG","INFO","WARN","ERROR"][levelIndex],autoScroll:follow}));

const byId = (id) => document.getElementById(id);
const log = byId("log");
const search = byId("search");

function classify(line) {
  if (/\[TRACE\]|\[T\s/.test(line)) return "trace";
  if (/\[DEBUG\]|\[D\s/.test(line)) return "debug";
  if (/\[WARN(?:ING)?\]|\[W\s/.test(line)) return "warn";
  if (/\[ERROR\]|\[E\s/.test(line)) return "error";
  return "info";
}

function accepted(line) {
  const needle = search.value.trim().toLocaleLowerCase();
  if (needle && !line.toLocaleLowerCase().includes(needle)) return false;
  const severity = ["trace", "debug", "info", "warn", "error"].indexOf(classify(line));
  const minimum = Math.max(0, ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"].indexOf(minimumLevel));
  if (severity < minimum) return false;
  if (levelIndex === 0) return true;
  return classify(line) === ["", "trace", "debug", "info", "warn", "error"][levelIndex];
}

function countLines(text) {
  let count = 0;
  for (let index = text.indexOf("\n"); index !== -1; index = text.indexOf("\n", index + 1)) count += 1;
  return count;
}

// Applies one log update from the host: "replace" its text, or "append"
// complete lines (keeping at most maxChars). Returns the appended text, ""
// when nothing changed, or null when the text was replaced.
function applyLogUpdate(name, update, maxChars) {
  if (!update) return "";
  if (update.mode === "replace") {
    data[name] = { text: update.text, state: update.state, lines: countLines(update.text) };
    return null;
  }
  const current = data[name];
  current.text += update.text;
  current.state = update.state;
  current.lines += countLines(update.text);
  if (current.text.length > maxChars) {
    const cut = current.text.indexOf("\n", current.text.length - maxChars);
    if (cut !== -1) {
      current.lines -= countLines(current.text.slice(0, cut + 1));
      current.text = current.text.slice(cut + 1);
    }
  }
  return update.text;
}

function lineElement(value) {
  const line = document.createElement("div");
  line.className = `line ${classify(value)}`;
  line.textContent = value;
  return line;
}

function showStats() {
  byId("stats").textContent = `${log.querySelectorAll(".line").length.toLocaleString()} shown of ${data[activeSource].lines.toLocaleString()} lines`;
  byId("game-count").textContent = data.game.lines.toLocaleString();
  byId("loader-count").textContent = data.loader.lines.toLocaleString();
}

// The view the list was last built for: a change rebuilds it.
let lastRender = "";
const viewKey = () => [activeSource, levelIndex, minimumLevel, search.value, paused, follow].join("\u0000");

function render() {
  lastRender = viewKey();
  const tail = data[activeSource];
  const source = paused ? frozen : tail.text;
  const visible = source ? source.split(/\r?\n/).filter(Boolean).filter(accepted) : [];
  log.replaceChildren();
  if (!visible.length) {
    const empty = document.createElement("div");
    empty.className = "empty";
    if (source) empty.textContent = "No log entries match the current filter.";
    else if (tail.state === "missing") empty.textContent = "Log file is not available yet.";
    else if (tail.state === "error") empty.textContent = "Log file could not be read.";
    else empty.textContent = "No log entries yet.";
    log.append(empty);
  } else {
    const fragment = document.createDocumentFragment();
    for (const value of visible.slice(-MAX_SHOWN_LINES)) fragment.append(lineElement(value));
    log.append(fragment);
  }
  byId("source").textContent = paths[activeSource];
  byId("source").title = paths[activeSource];
  showStats();
  if (follow) log.scrollTop = log.scrollHeight;
}

// Adds the accepted lines of newly appended text to the list.
function appendLines(text) {
  const added = text.split(/\r?\n/).filter(Boolean).filter(accepted);
  if (added.length) {
    if (log.querySelector(".empty")) return render();
    const fragment = document.createDocumentFragment();
    for (const value of added) fragment.append(lineElement(value));
    log.append(fragment);
    for (let excess = log.childElementCount - MAX_SHOWN_LINES; excess > 0; excess -= 1) log.firstElementChild.remove();
    if (follow) log.scrollTop = log.scrollHeight;
  }
  showStats();
}

window.__shroudforgeUpdate = (next) => {
  minimumLevel = alertReview ? "TRACE" : (next.minimumLevel || "INFO");
  if (next.preferences) {
    activeSource = next.preferences.defaultSource === "loader" ? "loader" : "game";
    levelIndex = Math.max(0,["ALL","TRACE","DEBUG","INFO","WARN","ERROR"].indexOf(next.preferences.levelFilter));
    follow = next.preferences.autoScroll;
  }
  if (next.autoOpen) {
    alertReview = true;
    activeSource = "loader";
    levelIndex = 0;
    paused = false;
    frozen = "";
    follow = true;
  }
  document.querySelectorAll(".tab").forEach(item => item.classList.toggle("active",item.dataset.tab===activeSource));
  byId("level").textContent = `Display: ${levels[levelIndex]}`;
  byId("level").classList.toggle("active",levelIndex!==0);
  byId("follow").textContent = `Auto-Scroll: ${follow ? "On" : "Off"}`;
  byId("follow").classList.toggle("active",follow);
  const maxChars = Number(next.preferences && next.preferences.tailBytes) || 1048576;
  const added = {
    game: applyLogUpdate("game", next.game, maxChars),
    loader: applyLogUpdate("loader", next.loader, maxChars),
  };
  paths.game = next.paths?.game || paths.game;
  paths.loader = next.paths?.loader || paths.loader;
  document.querySelector('[data-tab="game"]').title = paths.game;
  document.querySelector('[data-tab="loader"]').title = paths.loader;
  byId("session").textContent = next.connected ? "CLIENT" : "DESKTOP";
  byId("mode").classList.toggle("online", Boolean(next.connected));
  byId("mode").classList.toggle("desktop", !next.connected);
  if (paused) return showStats();
  if (viewKey() !== lastRender || added[activeSource] === null) render();
  else appendLines(added[activeSource]);
};

document.querySelectorAll(".tab").forEach((tab) => tab.addEventListener("click", () => {
  activeSource = tab.dataset.tab;
  savePreferences();
  paused = false;
  byId("pause").textContent = "Pause";
  document.querySelectorAll(".tab").forEach((item) => item.classList.toggle("active", item === tab));
  render();
}));

search.addEventListener("input", render);
byId("level").addEventListener("click", () => {
  levelIndex = (levelIndex + 1) % levels.length;
  savePreferences();
  byId("level").textContent = `Display: ${levels[levelIndex]}`;
  byId("level").classList.toggle("active", levelIndex !== 0);
  render();
});
byId("pause").addEventListener("click", () => {
  paused = !paused;
  if (paused) frozen = data[activeSource].text;
  byId("pause").textContent = paused ? "Resume" : "Pause";
  byId("pause").classList.toggle("active", paused);
  render();
});
byId("follow").addEventListener("click", () => {
  follow = !follow;
  savePreferences();
  byId("follow").textContent = `Auto-Scroll: ${follow ? "On" : "Off"}`;
  byId("follow").classList.toggle("active", follow);
  render();
});
byId("copy").addEventListener("click", async () => {
  const source = paused ? frozen : data[activeSource].text;
  const text = source.split(/\r?\n/).filter(Boolean).filter(accepted).join("\n");
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const temporary = document.createElement("textarea");
    temporary.value = text;
    temporary.style.position = "fixed";
    temporary.style.opacity = "0";
    document.body.append(temporary);
    temporary.select();
    document.execCommand("copy");
    temporary.remove();
  }
  byId("copy").textContent = "Copied";
  setTimeout(() => byId("copy").textContent = "Copy Logs", 1500);
});
byId("close").addEventListener("click", () => window.ipc.postMessage("hide"));
byId("minimize").addEventListener("click", () => window.ipc.postMessage("minimize-window"));
byId("maximize").addEventListener("click", () => window.ipc.postMessage("toggle-maximize-window"));
document.addEventListener("mousedown", (event) => {
  if (!event.target.closest("button,input") && event.target.closest("[data-drag-region]")) {
    window.ipc.postMessage("drag");
  }
});

render();
