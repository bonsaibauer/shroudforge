const levels = ["All", "Trace", "Debug", "Info", "Warning", "Error"];
const emptyTail = () => ({ text: "", state: "missing" });
const data = { game: emptyTail(), loader: emptyTail(), diagnostics: emptyTail() };
const paths = { game: "enshrouded.log", loader: "shroudforge.log", diagnostics: "shroudforge.log [diagnostics]" };
let activeSource = "game";
let levelIndex = 0;
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
  if (levelIndex === 0) return true;
  return classify(line) === ["", "trace", "debug", "info", "warn", "error"][levelIndex];
}

function render() {
  const tail = data[activeSource];
  const source = paused ? frozen : tail.text;
  const lines = source ? source.split(/\r?\n/).filter(Boolean) : [];
  const visible = lines.filter(accepted);
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
    for (const value of visible) {
      const line = document.createElement("div");
      line.className = `line ${classify(value)}`;
      line.textContent = value;
      fragment.append(line);
    }
    log.append(fragment);
  }
  byId("source").textContent = paths[activeSource];
  byId("source").title = paths[activeSource];
  byId("stats").textContent = `${visible.length.toLocaleString()} of ${lines.length.toLocaleString()} lines`;
  byId("game-count").textContent = count(data.game.text);
  byId("loader-count").textContent = count(data.loader.text);
  byId("diagnostics-count").textContent = count(data.diagnostics.text);
  if (follow) log.scrollTop = log.scrollHeight;
}

function count(value) {
  return value ? value.split(/\r?\n/).filter(Boolean).length.toLocaleString() : "0";
}

window.__shroudforgeUpdate = (next) => {
  if (next.preferences) {
    activeSource = next.preferences.defaultSource;
    levelIndex = Math.max(0,["ALL","TRACE","DEBUG","INFO","WARN","ERROR"].indexOf(next.preferences.levelFilter));
    follow = next.preferences.autoScroll;
    document.querySelectorAll(".tab").forEach(item => item.classList.toggle("active",item.dataset.tab===activeSource));
    byId("level").textContent = `Level: ${levels[levelIndex]}`;
    byId("follow").textContent = `Auto-Scroll: ${follow ? "On" : "Off"}`;
    byId("follow").classList.toggle("active",follow);
  }
  data.game = next.game || emptyTail();
  data.loader = next.loader || emptyTail();
  data.diagnostics = {state:data.loader.state,text:data.loader.text.split(/\r?\n/).filter(line=>line.includes('[diagnostics]')).join('\n')};
  paths.game = next.paths?.game || paths.game;
  paths.loader = next.paths?.loader || paths.loader;
  document.querySelector('[data-tab="game"]').title = paths.game;
  document.querySelector('[data-tab="loader"]').title = paths.loader;
  byId("session").textContent = next.connected ? "CLIENT" : "STANDALONE";
  byId("mode").classList.toggle("online", Boolean(next.connected));
  byId("mode").classList.toggle("standalone", !next.connected);
  if (!paused) render();
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
  byId("level").textContent = `Level: ${levels[levelIndex]}`;
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
  const text = [...log.querySelectorAll(".line")].map((line) => line.textContent).join("\n");
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
document.addEventListener("mousedown", (event) => {
  if (!event.target.closest("button,input") && event.target.closest("[data-drag-region]")) {
    window.ipc.postMessage("drag");
  }
});

render();
