import fs from "node:fs";
import path from "node:path";
import { readApiVersion } from "./read-api-version.mjs";

const root = path.resolve(import.meta.dirname, "../..");
const data = path.join(root, "build", "website-data");
const current = JSON.parse(fs.readFileSync(path.join(data, "current.json"), "utf8"));
const snapshot = path.join(data, current.snapshot);
const profile = JSON.parse(fs.readFileSync(path.join(snapshot, "profile.json"), "utf8"));
const types = JSON.parse(fs.readFileSync(path.join(snapshot, "types.json"), "utf8"));
const resources = JSON.parse(fs.readFileSync(path.join(snapshot, "resources.json"), "utf8"));
const api = JSON.parse(fs.readFileSync(path.join(data, "api.json"), "utf8"));
const version = readApiVersion(root);
const shroudforgeVersion = fs.readFileSync(path.join(root, "VERSION"), "utf8").trim();

if (api.version !== version) throw new Error(`site API ${api.version} does not match shroudforge-api package version ${version}`);
if (api.shroudforge_version !== shroudforgeVersion) throw new Error(`site release ${api.shroudforge_version} does not match VERSION ${shroudforgeVersion}`);
if (Object.keys(types).length !== profile.type_count) throw new Error("type count does not match profile");
if (Object.keys(resources).length !== profile.resource_type_count) throw new Error("resource type count does not match profile");
const fieldCount = Object.values(types).reduce((sum, type) => sum + Object.keys(type.fields || {}).length, 0);
if (fieldCount !== profile.field_count) throw new Error("field count does not match profile");
for (const [name, type] of Object.entries(types)) {
  for (const forbidden of ["name_hash", "impact_hash", "qualified_hash", "internal_hash"]) {
    if (Object.hasOwn(type, forbidden)) throw new Error(`public type ${name} exposes ${forbidden}`);
  }
}
if (!api.symbols.some(symbol => symbol.name === "runtime.phase")) throw new Error("runtime phase field is missing");
if (!api.symbols.some(symbol => symbol.name === "runtime.has")) throw new Error("runtime API is missing");
if (!api.symbols.some(symbol => symbol.name === "runtime.lifecycle.on_load")) throw new Error("runtime lifecycle API is missing");
if (!api.symbols.some(symbol => symbol.name === "loader.features.export")) throw new Error("EML loader feature API is missing");
if (!api.symbols.some(symbol => symbol.name === "loader.has_mod")) throw new Error("EML loader registry API is missing");
if (!api.symbols.some(symbol => symbol.name === "game.types.get")) throw new Error("game type API is missing");
if (!api.symbols.some(symbol => symbol.name === "game.assets.get_resource")) throw new Error("asset API is missing");
if (!api.symbols.some(symbol => symbol.name === "shroudforge.log.info")) throw new Error("ShroudForge logging API is missing");
if (!api.symbols.some(symbol => symbol.name === "io.read")) throw new Error("EML file API is missing");
if (!api.symbols.some(symbol => symbol.name === "buffer.create")) throw new Error("EML buffer API is missing");
for (const forbidden of ["get_by_qualified_hash", "get_by_impact_hash", "name_hash", "impact_hash", "qualified_hash", "internal_hash"]) {
  if (api.symbols.some(symbol => symbol.name.includes(forbidden))) {
    throw new Error(`hash-based public type API remains: ${forbidden}`);
  }
}
for (const symbol of api.symbols) {
  if (!["game", "runtime", "shroudforge", "eml"].includes(symbol.namespace)) {
    throw new Error(`unexpected public API namespace '${symbol.namespace}' on ${symbol.name}`);
  }
}
const publicSource = JSON.stringify(api).toLowerCase();
for (const forbidden of ["native bridge", "runtime patches", "api->", "create_patch", "set_patch_enabled"]) {
  if (publicSource.includes(forbidden)) throw new Error(`unsupported API token in public site: ${forbidden}`);
}
console.log(`Validated ShroudForge API ${version} from release ${shroudforgeVersion}: ${api.symbols.length} Lua symbols, ${profile.type_count} types, ${profile.field_count} fields, ${profile.resource_type_count} resource types.`);
