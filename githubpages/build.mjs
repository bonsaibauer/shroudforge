import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const source = path.resolve(import.meta.dirname);
const output = path.join(root, "build", "githubpages");
const expectedOutput = path.resolve(root, "build", "githubpages");
const configuration = JSON.parse(fs.readFileSync(path.join(source, "redirect-routes.json"), "utf8").replace(/^\uFEFF/, ""));
const pageRoutes = Object.create(null);

if (!configuration.default || !configuration.routes || !configuration.views) {
  throw new Error("githubpages/redirect-routes.json must define a default, article routes, and view aliases.");
}

const allowedHosts = new Set(["wiki.shroudforge.net", "shroudforge.net", "www.nexusmods.com"]);
for (const [id, destination] of Object.entries(configuration.routes)) {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(id)) throw new Error(`Invalid legacy article ID: ${id}`);
  validateDestination(destination, allowedHosts, id);
  pageRoutes[`doc-${id}`] = destination;
}
for (const [view, destination] of Object.entries(configuration.views)) {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(view)) throw new Error(`Invalid legacy view alias: ${view}`);
  validateDestination(destination, allowedHosts, view);
  if (pageRoutes[view] && pageRoutes[view] !== destination) throw new Error(`Legacy route alias conflicts with an article ID: ${view}`);
  pageRoutes[view] = destination;
}
validateDestination(configuration.default, allowedHosts, "default");

if (Object.keys(configuration.routes).length !== 177) {
  throw new Error(`Expected all 177 former article IDs, found ${Object.keys(configuration.routes).length}.`);
}
if (path.resolve(output) !== expectedOutput || path.relative(root, output).split(path.sep)[0] !== "build") {
  throw new Error("GitHub Pages output must stay inside build/githubpages.");
}
const oldOutput = fs.existsSync(output) ? fs.lstatSync(output) : null;
if (oldOutput?.isSymbolicLink() || (oldOutput && !oldOutput.isDirectory())) {
  throw new Error("GitHub Pages output must be a real directory before it is rebuilt.");
}

fs.rmSync(output, { recursive: true, force: true });
fs.mkdirSync(path.join(output, "en"), { recursive: true });
fs.mkdirSync(path.join(output, "de"), { recursive: true });
const template = fs.readFileSync(path.join(source, "index.html"), "utf8");
for (const [directory, prefix] of [[".", "./"], ["en", "../"], ["de", "../"]]) {
  fs.writeFileSync(path.join(output, directory, "index.html"), template.replaceAll("{{ASSET_PREFIX}}", prefix));
}
fs.copyFileSync(path.join(source, "redirect.css"), path.join(output, "redirect.css"));
fs.copyFileSync(path.join(source, "redirect.js"), path.join(output, "redirect.js"));
fs.writeFileSync(path.join(output, "redirect-routes.js"), `export default ${JSON.stringify(pageRoutes, null, 2)};\n`);
fs.mkdirSync(path.join(output, "media"), { recursive: true });
fs.copyFileSync(path.join(root, "website", "public", "assets", "hammer.png"), path.join(output, "media", "hammer.png"));

console.log(`Built the minimal GitHub Pages site with ${Object.keys(configuration.routes).length} article URLs and ${Object.keys(configuration.views).length} view aliases.`);

function validateDestination(value, allowedHosts, id) {
  let destination;
  try {
    destination = new URL(value);
  } catch {
    throw new Error(`${id}: destination must be an absolute URL.`);
  }
  if (destination.protocol !== "https:" || !allowedHosts.has(destination.hostname)) {
    throw new Error(`${id}: unsupported destination ${value}`);
  }
}
