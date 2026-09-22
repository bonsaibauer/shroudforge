import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
const definitions = path.join(root, "Shroudforge_API", "definitions");
const dataDirectory = path.join(root, "site", "data");
const output = path.join(dataDirectory, "api.json");
const version = fs.readFileSync(path.join(root, "VERSION"), "utf8").trim();
const files = walk(definitions).filter(file => file.endsWith(".lua")).sort();
const symbols = [];
const classAliases = {
  AssetManager: "game.assets",
  BufferFactory: "shroudforge.buffer",
  Game: "game",
  GuidHelper: "game.guid",
  Hasher: "shroudforge.hasher",
  Image: "shroudforge.image.Image",
  IO: "shroudforge.io",
  RuntimeApi: "runtime",
  RuntimeEcsApi: "runtime.ecs",
  RuntimeFeatureStatus: "runtime.status",
  ShroudForgeApi: "shroudforge",
  ShroudForge: "shroudforge",
  ShroudForgeLogApi: "shroudforge.log",
  ShroudForgeNotification: "shroudforge.notifications.Notice",
  ShroudForgeNotifications: "shroudforge.notifications",
  ShroudForgeSettings: "shroudforge.settings",
  ShroudForgeUi: "shroudforge.ui",
  TypeRegistry: "game.types",
};

const snapshots = fs.readdirSync(dataDirectory, { withFileTypes: true })
  .filter(entry => entry.isDirectory() && fs.existsSync(path.join(dataDirectory, entry.name, "profile.json")))
  .map(entry => {
    const profile = JSON.parse(fs.readFileSync(path.join(dataDirectory, entry.name, "profile.json"), "utf8"));
    return { name: entry.name, captured: Date.parse(profile.game_version.split("|")[2]) || 0 };
  })
  .sort((left, right) => right.captured - left.captured);
if (!snapshots.length) throw new Error("site/data contains no game snapshot");
fs.writeFileSync(path.join(dataDirectory, "current.json"), JSON.stringify({ snapshot: snapshots[0].name }, null, 2) + "\n");

for (const file of files) {
  const relative = path.relative(definitions, file).replaceAll("\\", "/");
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
  let documentation = [];
  let currentClass = "";
  for (const line of lines) {
    const doc = line.match(/^---\s?(.*)$/);
    if (doc) {
      const value = doc[1].trim();
      const classMatch = value.match(/^@class\s+(\S+)/);
      if (classMatch) currentClass = classMatch[1];
      const fieldMatch = value.match(/^@field\s+(\S+)\s+([^\s]+)(?:\s+--\s*(.*))?/);
      if (fieldMatch && currentClass) {
        const owner = publicClassName(currentClass);
        const name = `${owner}.${fieldMatch[1]}`;
        symbols.push({
          kind: "field",
          namespace: namespaceOf(name),
          name,
          signature: `${name}: ${fieldMatch[2]}`,
          params: [],
          returns: [fieldMatch[2]],
          description: fieldMatch[3] || "",
          source: relative,
        });
      }
      documentation.push(value);
      continue;
    }
    const functionMatch = line.match(/^function\s+([^\s(]+)\(([^)]*)\)\s*end/);
    if (functionMatch) {
      const name = publicName(functionMatch[1]);
      const params = documentation.flatMap(value => {
        const match = value.match(/^@param\s+(\S+)\s+([^\s]+)(?:\s+--\s*(.*))?/);
        return match ? [{ name: match[1], type: match[2], description: match[3] || "" }] : [];
      });
      const returns = documentation.flatMap(value => {
        const match = value.match(/^@return\s+([^\s]+(?:\s*,\s*[^\s]+)*)/);
        return match ? [match[1]] : [];
      });
      const description = documentation.filter(value => value && !value.startsWith("@")).join(" ");
      symbols.push({
        kind: "function",
        namespace: namespaceOf(name),
        name,
        signature: `${name}(${functionMatch[2]})`,
        params,
        returns,
        description,
        source: relative,
      });
      documentation = [];
      continue;
    }
    documentation = line.trim() ? [] : documentation;
  }
}

fs.writeFileSync(output, JSON.stringify({ version, symbols }, null, 2) + "\n");
console.log(`Generated ${symbols.length} implemented Lua symbols for ShroudForge API ${version}.`);

function publicName(name) {
  const aliases = [
    ["TypeRegistry.", "game.types."],
    ["AssetManager.", "game.assets."],
    ["GuidHelper.", "game.guid."],
    ["log.", "shroudforge.log."],
    ["io.", "shroudforge.io."],
    ["buffer.", "shroudforge.buffer."],
    ["integer.", "shroudforge.integer."],
    ["hasher.", "shroudforge.hasher."],
    ["image.", "shroudforge.image."],
    ["shroudforge_notifications.", "shroudforge.notifications."],
    ["shroudforge_settings.", "shroudforge.settings."],
    ["shroudforge_ui.", "shroudforge.ui."],
  ];
  if (name === "require") return "shroudforge.require";
  if (name === "typeof") return "shroudforge.typeof";
  const alias = aliases.find(([internal]) => name.startsWith(internal));
  return alias ? alias[1] + name.slice(alias[0].length) : name;
}

function publicClassName(name) {
  if (name.startsWith("integer.")) return `shroudforge.${name}`;
  return classAliases[name] || name;
}

function namespaceOf(name) {
  if (name.startsWith("game.") || /^(Type|StructField|EnumField|Attribute|Resource|Content)[:.]/.test(name)) return "game";
  if (name.startsWith("runtime.")) return "runtime";
  if (name.startsWith("shroudforge.")) return "shroudforge";
  if (name.startsWith("Buffer:") || name.startsWith("Image:")) return "shroudforge";
  return "other";
}

function walk(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(file) : [file];
  });
}
