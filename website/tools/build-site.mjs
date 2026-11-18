import fs from "node:fs/promises";
import path from "node:path";
import config from "../config.mjs";
import { renderPage } from "../src/render-page.mjs";
import { renderModSettingsPage } from "../src/render-tool-page.mjs";
import { renderSchemasPage } from "../src/render-schemas-page.mjs";
import { renderApiPlaygroundPage, renderApiReferencePage } from "../src/render-api-tools.mjs";
import { readGithubReleases } from "./github-releases.mjs";
import { readSecurityReports } from "./security-reports.mjs";

const websiteDirectory = path.resolve(import.meta.dirname, "..");
const repositoryDirectory = path.resolve(websiteDirectory, "..");
const buildDirectory = path.join(repositoryDirectory, "build");
const outputDirectory = path.join(buildDirectory, "download-website");
const browserAssets = ["styles.css", "filters.js", "installation-guide.js", "mod-settings.css", "mod-settings.js", "schemas.css", "schemas.js", "api-tools.css", "api-tools.js", "theme.js"];
const modSettingsRelativePath = path.join("tools", "modding", "mod-settings");
const apiReferenceRelativePath = path.join("tools", "modding", "api-reference");
const apiPlaygroundRelativePath = path.join("tools", "modding", "api-playground");
const schemasRelativePath = "schemas";
const schemaSnapshotDirectory = path.join(websiteDirectory, "schema-snapshots");
const apiSnapshotDirectory = path.join(websiteDirectory, "api-snapshots");
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

function compareSemverDescending(left, right) {
	const a = left.split(".").map(Number)
	const b = right.split(".").map(Number)
	return b[0] - a[0] || b[1] - a[1] || b[2] - a[2]
}

async function readApiCatalogs(currentCatalog, currentExamples, currentOperations) {
	const snapshots = [{ catalog: currentCatalog, examples: currentExamples, operations: currentOperations, current: true }]
	const entries = await fs.readdir(apiSnapshotDirectory, { withFileTypes: true })
	for (const entry of entries) {
		if (!entry.isDirectory()) continue
		if (!/^\d+\.\d+\.\d+$/.test(entry.name)) throw new Error(`API snapshot directory must use MAJOR.MINOR.PATCH: ${entry.name}`)
		if (entry.name === currentCatalog.version) continue
		const directory = path.join(apiSnapshotDirectory, entry.name)
		const catalog = JSON.parse(await fs.readFile(path.join(directory, "catalog.json"), "utf8"))
		const examples = JSON.parse(await fs.readFile(path.join(directory, "examples.json"), "utf8"))
		const operations = JSON.parse(await fs.readFile(path.join(directory, "runtime-operations.json"), "utf8"))
		if (catalog.version !== entry.name) throw new Error(`API snapshot folder ${entry.name} does not match catalog version ${catalog.version}.`)
		if (!Array.isArray(catalog.symbols) || !Array.isArray(examples.scenarios) || !Array.isArray(operations.runtimeOperations)) {
			throw new Error(`API snapshot ${entry.name} is missing symbols, examples, or runtime operation declarations.`)
		}
		snapshots.push({ catalog, examples, operations, current: false })
	}
	snapshots.sort((left, right) => compareSemverDescending(left.catalog.version, right.catalog.version))
	return snapshots
}

async function readWorldEditorApiReferences(catalog) {
	const modDirectory = path.join(repositoryDirectory, "mods", "world-editor")
	const sourceDirectory = path.join(modDirectory, "src")
	await assertRegularTree(sourceDirectory)
	const manifest = JSON.parse(await fs.readFile(path.join(modDirectory, "mod.json"), "utf8"))
	const files = []
	async function collectLuaFiles(directory) {
		for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
			const file = path.join(directory, entry.name)
			if (entry.isSymbolicLink()) throw new Error(`World Editor source references cannot include symbolic links: ${file}`)
			if (entry.isDirectory()) await collectLuaFiles(file)
			else if (entry.isFile() && entry.name.endsWith(".lua")) files.push(file)
		}
	}
	await collectLuaFiles(sourceDirectory)
	files.sort((left, right) => left.localeCompare(right))
	const catalogNames = new Set(catalog.symbols.map(symbol => symbol.name))
	const references = new Map()
	for (const file of files) {
		const source = (await fs.readFile(file, "utf8"))
			.replace(/--\[(=*)\[[\s\S]*?\]\1\]/g, "")
			.replace(/--[^\n]*/g, "")
		const names = new Set([
			...[...source.matchAll(/\bruntime(?:\.[A-Za-z_][A-Za-z0-9_]*)+\s*\(/g)].map(match => match[0].replace(/\s*\($/, "")),
			...[...source.matchAll(/["'](runtime(?:\.[A-Za-z_][A-Za-z0-9_]*)+)["']/g)].map(match => match[1]),
		])
		const relative = path.relative(repositoryDirectory, file).split(path.sep).join("/")
		for (const name of names) {
			if (!catalogNames.has(name)) continue
			if (!references.has(name)) references.set(name, [])
			references.get(name).push({
				path: relative,
				url: `https://github.com/${config.repository}/blob/${catalog.shroudforge_version}/${relative}`,
			})
		}
	}
	return {
		apiVersion: catalog.version,
		mod: { id: manifest.id, name: manifest.name, version: manifest.version },
		references: Object.fromEntries([...references.entries()].sort(([left], [right]) => left.localeCompare(right))),
	}
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
  const generatedWebsiteData = path.join(buildDirectory, "website", "data");
  const apiCatalog = JSON.parse(await fs.readFile(path.join(generatedWebsiteData, "api.json"), "utf8"));
  if (!/^\d+\.\d+\.\d+$/.test(apiCatalog.version) || !Array.isArray(apiCatalog.symbols)) {
    throw new Error("Generated API catalogue must contain an API SemVer and symbols array.");
  }
  if (apiCatalog.shroudforge_version !== version) {
    throw new Error(`Generated API catalogue release ${apiCatalog.shroudforge_version} does not match ShroudForge ${version}.`);
  }
  const apiExamples = await readJson(path.join("website", "api-examples.json"));
  const exampleSettings = Object.fromEntries(Object.entries((await readJson(path.join("docs", "examples", "starter-mod", "extended.mod.json"))).settings || {}).map(([key, setting]) => [key, setting && typeof setting === "object" && Object.hasOwn(setting, "value") ? setting.value : setting]));
  const currentGame = JSON.parse(await fs.readFile(path.join(generatedWebsiteData, "current.json"), "utf8"));
  const gameSnapshotDirectory = path.join(generatedWebsiteData, currentGame.snapshot);
  const gameProfile = JSON.parse(await fs.readFile(path.join(gameSnapshotDirectory, "profile.json"), "utf8"));
  const gameSnapshot = { id: currentGame.snapshot, gameVersion: gameProfile.game_version, profile: gameProfile };
  const apiExamplesDocument = {
    version: apiCatalog.version,
    shroudforgeVersion: apiCatalog.shroudforge_version,
    gameSnapshot,
    fixtures: { settings: exampleSettings },
    scenarios: apiExamples,
  };
  const runtimeOperations = await readJson(path.join("modules", "mod-runtime", "lua", "shroudforge", "v1", "runtime-operations.json"));
  const apiCatalogs = await readApiCatalogs(apiCatalog, apiExamplesDocument, runtimeOperations);
  const worldEditorReferences = await readWorldEditorApiReferences(apiCatalog);
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
  const apiReferenceHtml = renderApiReferencePage({ config, siteUrl });
  const apiPlaygroundHtml = renderApiPlaygroundPage({ config, siteUrl });
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
  for (const [relative, html] of [[apiReferenceRelativePath, apiReferenceHtml], [apiPlaygroundRelativePath, apiPlaygroundHtml]]) {
	const directory = path.join(outputDirectory, relative)
	await fs.mkdir(directory, { recursive: true })
	await fs.writeFile(path.join(directory, "index.html"), html, "utf8")
  }
  const apiDataDirectory = path.join(outputDirectory, "data", "api")
  await fs.mkdir(apiDataDirectory, { recursive: true })
  const apiVersionIndex = {
	current: apiCatalog.version,
	versions: apiCatalogs.map(({ catalog, current }) => ({
		version: catalog.version,
		shroudforgeVersion: catalog.shroudforge_version || null,
		symbolCount: catalog.symbols.length,
		current,
	})),
  }
  await fs.writeFile(path.join(apiDataDirectory, "versions.json"), `${JSON.stringify(apiVersionIndex, null, 2)}\n`, "utf8")
  for (const snapshot of apiCatalogs) {
	const directory = path.join(apiDataDirectory, snapshot.catalog.version)
	await fs.mkdir(directory, { recursive: true })
	await fs.writeFile(path.join(directory, "catalog.json"), `${JSON.stringify(snapshot.catalog)}\n`, "utf8")
	await fs.writeFile(path.join(directory, "examples.json"), `${JSON.stringify(snapshot.examples)}\n`, "utf8")
	await fs.writeFile(path.join(directory, "runtime-operations.json"), `${JSON.stringify(snapshot.operations)}\n`, "utf8")
	const evidence = snapshot.current
		? worldEditorReferences
		: { apiVersion: snapshot.catalog.version, mod: null, references: {} }
	await fs.writeFile(path.join(directory, "world-editor-references.json"), `${JSON.stringify(evidence)}\n`, "utf8")
  }
  const gameDataDirectory = path.join(outputDirectory, "data", "game")
  const gameOutputSnapshot = path.join(gameDataDirectory, currentGame.snapshot)
  await fs.mkdir(gameOutputSnapshot, { recursive: true })
  await fs.writeFile(path.join(gameDataDirectory, "current.json"), `${JSON.stringify({ snapshot: currentGame.snapshot })}\n`, "utf8")
  for (const name of ["profile.json", "types.json", "resources.json"]) {
	await fs.copyFile(path.join(gameSnapshotDirectory, name), path.join(gameOutputSnapshot, name))
  }
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
  await fs.writeFile(path.join(outputDirectory, "sitemap.xml"), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>${escapeHtml(siteUrl)}/</loc><lastmod>${generatedAt}</lastmod></url><url><loc>${escapeHtml(siteUrl)}/${modSettingsRelativePath.replaceAll(path.sep, "/")}/</loc><lastmod>${generatedAt}</lastmod></url><url><loc>${escapeHtml(siteUrl)}/${apiReferenceRelativePath.replaceAll(path.sep, "/")}/</loc><lastmod>${generatedAt}</lastmod></url><url><loc>${escapeHtml(siteUrl)}/${apiPlaygroundRelativePath.replaceAll(path.sep, "/")}/</loc><lastmod>${generatedAt}</lastmod></url><url><loc>${escapeHtml(siteUrl)}/schemas/</loc><lastmod>${generatedAt}</lastmod></url></urlset>\n`, "utf8");
  console.log(`Built ${releases.length} ShroudForge releases and API ${apiCatalog.version} into build/download-website.`);
}

build().catch(error => {
  console.error(`Download website build failed: ${error.message}`);
  process.exitCode = 1;
});
