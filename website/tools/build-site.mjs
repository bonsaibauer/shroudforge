import fs from "node:fs/promises";
import path from "node:path";
import config from "../config.mjs";
import { renderPage } from "../src/render-page.mjs";
import { renderModSettingsPage } from "../src/render-tool-page.mjs";
import { renderSchemasPage } from "../src/render-schemas-page.mjs";
import { readGithubReleases } from "./github-releases.mjs";
import { readSecurityReports } from "./security-reports.mjs";

const websiteDirectory = path.resolve(import.meta.dirname, "..");
const repositoryDirectory = path.resolve(websiteDirectory, "..");
const buildDirectory = path.join(repositoryDirectory, "build");
const outputDirectory = path.join(buildDirectory, "download-website");
const browserAssets = ["styles.css", "filters.js", "installation-guide.js", "mod-settings.css", "mod-settings.js", "schemas.css", "schemas.js", "theme.js"];
const modSettingsRelativePath = path.join("tools", "modding", "mod-settings");
const schemasRelativePath = "schemas";
const schemaSnapshotDirectory = path.join(websiteDirectory, "schema-snapshots");
const schemaNames = ["manifest.schema.json", "extended.mod.schema.json", "mod-locale.schema.json"];
const starterModDirectory = path.join(repositoryDirectory, "docs", "examples", "starter-mod");

export function resolveSiteUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error("Website origin must be a valid HTTPS origin.");
  }
  const local = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  if ((url.protocol !== "https:" && !(local && url.protocol === "http:")) ||
      url.username || url.password || url.pathname !== "/" || url.search || url.hash) {
    throw new Error("Website origin must be an HTTPS origin without a path, credentials, query, or fragment. HTTP is allowed for localhost only.");
  }
  return url.origin;
}

async function inspectPath(file) {
  try {
    return await fs.lstat(file);
  } catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}

async function assertRegularTree(directory) {
  const stat = await inspectPath(directory);
  if (!stat) return;
  if (stat.isSymbolicLink()) throw new Error(`Symbolic links are not allowed in website build paths: ${directory}`);
  if (!stat.isDirectory() && !stat.isFile()) throw new Error(`Unsupported website filesystem entry: ${directory}`);
  if (stat.isDirectory()) {
    for (const entry of await fs.readdir(directory)) await assertRegularTree(path.join(directory, entry));
  }
}

async function prepareOutputDirectory() {
  const relative = path.relative(buildDirectory, outputDirectory);
  if (relative !== "download-website" || path.isAbsolute(relative) || path.resolve(outputDirectory) !== path.join(repositoryDirectory, "build", "download-website")) {
    throw new Error("Website output directory escapes its build boundary.");
  }
  const buildStat = await inspectPath(buildDirectory);
  if (buildStat && (!buildStat.isDirectory() || buildStat.isSymbolicLink())) throw new Error("The repository build directory must be a real directory.");
  await fs.mkdir(buildDirectory, { recursive: true });
  const realBuild = await fs.realpath(buildDirectory);
  const realRepository = await fs.realpath(repositoryDirectory);
  if (path.relative(realRepository, realBuild) !== "build") throw new Error("The real build directory escapes the repository.");
  await assertRegularTree(outputDirectory);
  await fs.rm(outputDirectory, { recursive: true, force: true });
}

async function readSchemaSnapshots(siteUrl, currentVersion) {
  await assertRegularTree(schemaSnapshotDirectory);
  const entries = await fs.readdir(schemaSnapshotDirectory, { withFileTypes: true });
  const versions = entries
    .filter(entry => entry.isDirectory() && /^\d+\.\d+\.\d+$/.test(entry.name))
    .map(entry => entry.name)
    .sort((left, right) => {
      const a = left.split(".").map(Number);
      const b = right.split(".").map(Number);
      return b[0] - a[0] || b[1] - a[1] || b[2] - a[2];
    });
  if (versions.length !== entries.length) throw new Error("Schema snapshots may only contain versioned directories using MAJOR.MINOR.PATCH.");
  if (!versions.includes(currentVersion)) throw new Error(`Schema snapshots are missing the current Modloader version ${currentVersion}.`);

  const published = [];
  for (const version of versions) {
    for (const name of schemaNames) {
      const file = path.join(schemaSnapshotDirectory, version, name);
      const stat = await fs.lstat(file);
      if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`Schema snapshot must be a regular file: ${path.relative(websiteDirectory, file)}`);
      const text = await fs.readFile(file, "utf8");
      const schema = JSON.parse(text);
      if (!schema || typeof schema !== "object" || Array.isArray(schema) || typeof schema.$id !== "string") {
        throw new Error(`Schema snapshot needs a top-level object and $id: ${path.relative(websiteDirectory, file)}`);
      }
      if (version === currentVersion) {
        const sourcePath = path.join(repositoryDirectory, "modules", "modloader", "manifest", name);
        const source = JSON.parse(await fs.readFile(sourcePath, "utf8"));
        const sourceContent = { ...source };
        const snapshotContent = { ...schema };
        delete sourceContent.$id;
        delete snapshotContent.$id;
        if (JSON.stringify(sourceContent) !== JSON.stringify(snapshotContent)) {
          throw new Error(`Current ${version} schema snapshot differs from modules/modloader/manifest/${name}.`);
        }
      }
      const schemaUrl = new URL(`/schemas/${version}/${name}`, siteUrl).href;
      schema.$id = schemaUrl;
      published.push({ version, name, contents: `${JSON.stringify(schema, null, 2)}\n` });
    }
  }
  return { versions, files: published };
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, character => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[character]);
}

function renderNotFound(siteUrl) {
  return `<!doctype html>
<html lang="en" data-theme="light"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="robots" content="noindex"><meta name="theme-color" content="#ffffff"><meta name="color-scheme" content="light"><title>Page not found · ${escapeHtml(config.name)}</title><link rel="icon" href="/assets/favicon.ico"><script src="/assets/theme.js"></script><link rel="stylesheet" href="/assets/styles.css"></head>
<body><main class="not-found"><h1>Page not found</h1><p>The page you requested does not exist.</p><a href="${escapeHtml(siteUrl)}/">Return to ${escapeHtml(config.name)} downloads</a></main></body></html>\n`;
}

async function build() {
  const siteUrl = resolveSiteUrl(process.env.WEBSITE_ORIGIN || config.siteUrl);
  const releases = await readGithubReleases(config.repository);
  if (!releases.length) throw new Error("No complete ShroudForge releases were found. Refusing to publish an empty download website.");
  const securityReports = await readSecurityReports(releases, config.repository);
  const version = (await fs.readFile(path.join(repositoryDirectory, "VERSION"), "utf8")).trim();
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`VERSION must use MAJOR.MINOR.PATCH: ${version}`);
  const schemaCatalog = await readSchemaSnapshots(siteUrl, version);
  const readJson = async relative => JSON.parse(await fs.readFile(path.join(repositoryDirectory, relative), "utf8"));
  const exampleManifest = await readJson(path.join("docs", "examples", "starter-mod", "mod.json"));
  const exampleExtended = await readJson(path.join("docs", "examples", "starter-mod", "extended.mod.json"));
  const websiteManifest = { ...exampleManifest, $schema: new URL(`/schemas/${version}/manifest.schema.json`, siteUrl).href };
  const websiteExtended = { ...exampleExtended, $schema: new URL(`/schemas/${version}/extended.mod.schema.json`, siteUrl).href };
  const linkOrder = await readJson(path.join("modules", "modloader", "mod_link_order.json"));
  const linkLocale = await readJson(path.join("modules", "ui-resources", "locales", "en-GB.json"));
  const linkCatalog = await Promise.all(linkOrder.map(async id => {
    const definition = await readJson(path.join("modules", "modloader", "ui", "links", `${id}.json`));
    if (definition.id !== id || typeof definition.icon !== "string") throw new Error(`Invalid current Modloader link definition: ${id}`);
    const iconName = path.basename(definition.icon);
    if (!/^[A-Za-z0-9._-]+$/.test(iconName)) throw new Error(`Invalid Modloader badge icon name: ${iconName}`);
    return {
      id,
      label: linkLocale[definition.labelKey] || id,
      description: linkLocale[definition.descriptionKey] || "Mod page link",
      icon: iconName,
      iconUrl: `/assets/mod-settings-links/${iconName}.svg`,
      appearance: definition.appearance || "",
    };
  }));
  const generatedAt = new Date().toISOString();
  const html = renderPage({ releases, securityReports, generatedAt, siteUrl, config });
  if (typeof html !== "string" || !html.includes("<!doctype html>")) throw new Error("The website renderer must return a complete HTML document.");
  const toolHtml = renderModSettingsPage({
    config,
    siteUrl,
    version,
    manifest: websiteManifest,
    extended: websiteExtended,
    links: linkCatalog,
  });
  const schemasHtml = renderSchemasPage({
    config,
    siteUrl,
    schemaVersions: schemaCatalog.versions,
    currentVersion: version,
  });
  const publicDirectory = path.join(websiteDirectory, "public");
  await assertRegularTree(publicDirectory);
  for (const name of browserAssets) {
    const file = path.join(websiteDirectory, "src", name);
    const stat = await fs.lstat(file);
    if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`Website source asset must be a real file: ${name}`);
  }
  await prepareOutputDirectory();
  await fs.cp(publicDirectory, outputDirectory, { recursive: true, force: false, errorOnExist: true });
  await fs.mkdir(path.join(outputDirectory, "assets"), { recursive: true });
  for (const name of browserAssets) {
    await fs.copyFile(path.join(websiteDirectory, "src", name), path.join(outputDirectory, "assets", name));
  }
  await fs.writeFile(path.join(outputDirectory, "index.html"), html, "utf8");
  const schemasDirectory = path.join(outputDirectory, schemasRelativePath);
  await fs.mkdir(schemasDirectory, { recursive: true });
  await fs.writeFile(path.join(schemasDirectory, "index.html"), schemasHtml, "utf8");
  for (const schema of schemaCatalog.files) {
    const versionDirectory = path.join(schemasDirectory, schema.version);
    await fs.mkdir(versionDirectory, { recursive: true });
    await fs.writeFile(path.join(versionDirectory, schema.name), schema.contents, "utf8");
  }
  const toolDirectory = path.join(outputDirectory, modSettingsRelativePath);
  await fs.mkdir(toolDirectory, { recursive: true });
  await fs.writeFile(path.join(toolDirectory, "index.html"), toolHtml, "utf8");
  await fs.copyFile(
    path.join(starterModDirectory, exampleManifest.icon),
    path.join(toolDirectory, "icon.svg"),
  );
  const badgeIconDirectory = path.join(outputDirectory, "assets", "mod-settings-links");
  await fs.mkdir(badgeIconDirectory, { recursive: true });
  for (const link of linkCatalog) {
    await fs.copyFile(
      path.join(repositoryDirectory, "modules", "modloader", "ui", "links", `${link.icon}.svg`),
      path.join(badgeIconDirectory, `${link.icon}.svg`),
    );
  }
  await fs.writeFile(path.join(outputDirectory, "404.html"), renderNotFound(siteUrl), "utf8");
  await fs.writeFile(path.join(outputDirectory, "releases.json"), `${JSON.stringify({ generatedAt, releases, securityReports }, null, 2)}\n`, "utf8");
  await fs.writeFile(path.join(outputDirectory, "robots.txt"), `User-agent: *\nAllow: /\nSitemap: ${siteUrl}/sitemap.xml\n`, "utf8");
  await fs.writeFile(path.join(outputDirectory, "sitemap.xml"), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>${escapeHtml(siteUrl)}/</loc><lastmod>${generatedAt}</lastmod></url><url><loc>${escapeHtml(siteUrl)}/${modSettingsRelativePath.replaceAll(path.sep, "/")}/</loc><lastmod>${generatedAt}</lastmod></url><url><loc>${escapeHtml(siteUrl)}/schemas/</loc><lastmod>${generatedAt}</lastmod></url></urlset>\n`, "utf8");
  console.log(`Built ${releases.length} ShroudForge releases into build/download-website.`);
}

build().catch(error => {
  console.error(`Download website build failed: ${error.message}`);
  process.exitCode = 1;
});
