import fs from "node:fs/promises";
import path from "node:path";
import config from "../config.mjs";
import { renderPage, renderSiteFooter } from "../src/render-page.mjs";
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

async function readGameSnapshots(dataDirectory, currentSnapshot) {
	const entries = await fs.readdir(dataDirectory, { withFileTypes: true })
	const snapshots = []
	for (const entry of entries) {
		if (!entry.isDirectory()) continue
		const directory = path.join(dataDirectory, entry.name)
		const files = await Promise.all(["profile.json", "types.json", "resources.json"].map(async name => {
			const file = path.join(directory, name)
			const stat = await fs.lstat(file).catch(error => error.code === "ENOENT" ? null : Promise.reject(error))
			if (!stat || !stat.isFile() || stat.isSymbolicLink()) throw new Error(`Game snapshot ${entry.name} is missing a regular ${name} file.`)
			return file
		}))
		const profile = JSON.parse(await fs.readFile(files[0], "utf8"))
		const gameVersion = typeof profile.game_version === "string" ? profile.game_version : ""
		const gameBuild = gameVersion.split("|")[0]
		if (!/^\d+$/.test(gameBuild)) throw new Error(`Game snapshot ${entry.name} has an invalid game build: ${gameVersion}`)
		const capturedAt = Date.parse(gameVersion.split("|").at(-1)) || 0
		snapshots.push({ snapshot: entry.name, gameBuild, capturedAt })
	}
	if (!snapshots.length) throw new Error("The generated website data has no complete game snapshots.")
	if (!snapshots.some(snapshot => snapshot.snapshot === currentSnapshot)) throw new Error(`Current game snapshot is not published: ${currentSnapshot}`)
	snapshots.sort((left, right) => Number(right.snapshot === currentSnapshot) - Number(left.snapshot === currentSnapshot) || right.capturedAt - left.capturedAt || Number(right.gameBuild) - Number(left.gameBuild))
	const uniqueBuilds = new Set()
	return snapshots.filter(snapshot => {
		if (uniqueBuilds.has(snapshot.gameBuild)) return false
		uniqueBuilds.add(snapshot.gameBuild)
		return true
	})
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, character => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[character]);
}

function renderNotFound({ siteUrl, config, generatedAt }) {
  return `<!doctype html>
<html lang="en" data-theme="light"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="robots" content="noindex"><meta name="theme-color" content="#ffffff"><meta name="color-scheme" content="light"><title>Page not found · ${escapeHtml(config.name)}</title><link rel="icon" href="/assets/favicon.ico"><script src="/assets/theme.js"></script><link rel="stylesheet" href="/assets/styles.css"></head>
<body><main class="not-found"><h1>Page not found</h1><p>The page you requested does not exist.</p><a href="${escapeHtml(siteUrl)}/">Return to ${escapeHtml(config.name)} downloads</a></main>${renderSiteFooter({ config, generatedAt })}</body></html>\n`;
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
  const generatedWebsiteData = path.join(buildDirectory, "website-data");
  const apiCatalog = JSON.parse(await fs.readFile(path.join(generatedWebsiteData, "api.json"), "utf8"));
  if (!/^\d+\.\d+\.\d+$/.test(apiCatalog.version) || !Array.isArray(apiCatalog.symbols)) {
    throw new Error("Generated API catalogue must contain an API SemVer and symbols array.");
  }
  if (apiCatalog.shroudforge_version !== version) {
    throw new Error(`Generated API catalogue release ${apiCatalog.shroudforge_version} does not match ShroudForge ${version}.`);
  }
  const apiExamples = await readJson(path.join("website", "api-examples.json"));
  const sfSurvivalCapture = await readJson(path.join("website", "api-example-captures", "sf-survival-live.json"));
  const settingValues = extended => Object.fromEntries(Object.entries(extended.settings || {}).map(([key, setting]) => [key, setting && typeof setting === "object" && Object.hasOwn(setting, "value") ? setting.value : setting]));
  const sfSurvivalDefaults = settingValues(await readJson(path.join("mods", "sf-survival", "extended.mod.json")));
  const capturedSettingEntries = (sfSurvivalCapture.calls || []).flatMap(call => {
    const match = /^shroudforge\.settings\.get\("([^"]+)", false\)$/.exec(call.call || "");
    const result = call.returns?.[0]?.value;
    return match && call.ok === true && call.return_count === 1 && result?.lua_type === "boolean"
      ? [[match[1], result.value]]
      : [];
  });
  const sfSurvivalSettings = Object.fromEntries(capturedSettingEntries);
  const survivalScenario = apiExamples.find(scenario => scenario.id === "read-mod-setting");
  const survivalKeyInput = survivalScenario?.inputs?.find(input => input.name === "key");
  if (survivalScenario?.fixture !== "sfSurvivalLive" || survivalKeyInput?.type !== "select" ||
      !Array.isArray(survivalKeyInput.options) ||
      JSON.stringify([...survivalKeyInput.options].sort()) !== JSON.stringify(Object.keys(sfSurvivalSettings).sort()) ||
      JSON.stringify(Object.keys(sfSurvivalSettings).sort()) !== JSON.stringify(Object.keys(sfSurvivalDefaults).sort()) ||
      Object.values(sfSurvivalSettings).some(value => typeof value !== "boolean") ||
      sfSurvivalCapture.status !== "complete" || sfSurvivalCapture.mod_id !== "sf-survival" ||
      sfSurvivalCapture.process_target !== "client" || typeof sfSurvivalCapture.world_session?.value !== "number" || sfSurvivalCapture.world_session.value === 0 ||
      sfSurvivalCapture.safety?.world_values_changed !== false) {
    throw new Error("The SF Survival API example must match every Boolean setting captured from the live client and its declared package setting keys.");
  }
  const capturedCallNames = new Set((sfSurvivalCapture.calls || []).map(call => call.call));
  const referencedCallNames = new Set();
  for (const scenario of apiExamples.filter(item => item.fixture === "sfSurvivalLive")) {
    const inputName = scenario.captureInput || scenario.inputs?.[0]?.name;
    const selectedCalls = scenario.captureCalls || Object.values(scenario.captureCallsByInput || {}).flat();
    if (!selectedCalls.length || selectedCalls.some(callName => !capturedCallNames.has(callName))) {
      throw new Error(`SF Survival API scenario ${scenario.id} must reference calls present in the live capture.`);
    }
    if (scenario.captureCallsByInput && (!inputName || Object.keys(scenario.captureCallsByInput).some(value =>
      !scenario.inputs?.find(input => input.name === inputName)?.options?.includes(value)))) {
      throw new Error(`SF Survival API scenario ${scenario.id} must map every capture selector to an input option.`);
    }
    for (const callName of selectedCalls) referencedCallNames.add(callName);
  }
  const unreferencedCapturedCalls = [...capturedCallNames].filter(callName => !referencedCallNames.has(callName));
  if (unreferencedCapturedCalls.length) {
    throw new Error(`SF Survival API scenarios must expose every captured call. Unused: ${unreferencedCapturedCalls.join(", ")}`);
  }
  const currentGame = JSON.parse(await fs.readFile(path.join(generatedWebsiteData, "current.json"), "utf8"));
  const gameSnapshotDirectory = path.join(generatedWebsiteData, currentGame.snapshot);
  const gameProfile = JSON.parse(await fs.readFile(path.join(gameSnapshotDirectory, "profile.json"), "utf8"));
  const gameSnapshot = { id: currentGame.snapshot, gameVersion: gameProfile.game_version, profile: gameProfile };
  const gameSnapshots = await readGameSnapshots(generatedWebsiteData, currentGame.snapshot);
  const apiExamplesDocument = {
    version: apiCatalog.version,
    shroudforgeVersion: apiCatalog.shroudforge_version,
    gameSnapshot,
    fixtures: { sfSurvivalLive: sfSurvivalCapture },
    scenarios: apiExamples,
  };
  const runtimeOperations = await readJson(path.join("modules", "mod-runtime", "lua", "shroudforge", "v1", "runtime-operations.json"));
  const apiCatalogs = await readApiCatalogs(apiCatalog, apiExamplesDocument, runtimeOperations);
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
    generatedAt,
    version,
    manifest: websiteManifest,
    extended: websiteExtended,
    links: linkCatalog,
  });
  const schemasHtml = renderSchemasPage({
    config,
    siteUrl,
    generatedAt,
    schemaVersions: schemaCatalog.versions,
    currentVersion: version,
  });
  const apiReferenceHtml = renderApiReferencePage({ config, siteUrl, generatedAt });
  const apiPlaygroundHtml = renderApiPlaygroundPage({ config, siteUrl, generatedAt });
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
  }
  const gameDataDirectory = path.join(outputDirectory, "data", "game")
  await fs.mkdir(gameDataDirectory, { recursive: true })
  await fs.writeFile(path.join(gameDataDirectory, "current.json"), `${JSON.stringify({ snapshot: currentGame.snapshot })}\n`, "utf8")
  await fs.writeFile(path.join(gameDataDirectory, "builds.json"), `${JSON.stringify({
	current: currentGame.snapshot,
	builds: gameSnapshots.map(({ snapshot, gameBuild }) => ({ snapshot, gameBuild })),
  }, null, 2)}\n`, "utf8")
  for (const { snapshot } of gameSnapshots) {
	const sourceSnapshot = path.join(generatedWebsiteData, snapshot)
	const outputSnapshot = path.join(gameDataDirectory, snapshot)
	await fs.mkdir(outputSnapshot, { recursive: true })
	for (const name of ["profile.json", "types.json", "resources.json"]) {
		await fs.copyFile(path.join(sourceSnapshot, name), path.join(outputSnapshot, name))
	}
  }
  const badgeIconDirectory = path.join(outputDirectory, "assets", "mod-settings-links");
  await fs.mkdir(badgeIconDirectory, { recursive: true });
  for (const link of linkCatalog) {
    await fs.copyFile(
      path.join(repositoryDirectory, "modules", "modloader", "ui", "links", `${link.icon}.svg`),
      path.join(badgeIconDirectory, `${link.icon}.svg`),
    );
  }
  await fs.writeFile(path.join(outputDirectory, "404.html"), renderNotFound({ siteUrl, config, generatedAt }), "utf8");
  await fs.writeFile(path.join(outputDirectory, "releases.json"), `${JSON.stringify({ generatedAt, releases, securityReports }, null, 2)}\n`, "utf8");
  await fs.writeFile(path.join(outputDirectory, "robots.txt"), `User-agent: *\nAllow: /\nSitemap: ${siteUrl}/sitemap.xml\n`, "utf8");
  await fs.writeFile(path.join(outputDirectory, "sitemap.xml"), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>${escapeHtml(siteUrl)}/</loc><lastmod>${generatedAt}</lastmod></url><url><loc>${escapeHtml(siteUrl)}/${modSettingsRelativePath.replaceAll(path.sep, "/")}/</loc><lastmod>${generatedAt}</lastmod></url><url><loc>${escapeHtml(siteUrl)}/${apiReferenceRelativePath.replaceAll(path.sep, "/")}/</loc><lastmod>${generatedAt}</lastmod></url><url><loc>${escapeHtml(siteUrl)}/${apiPlaygroundRelativePath.replaceAll(path.sep, "/")}/</loc><lastmod>${generatedAt}</lastmod></url><url><loc>${escapeHtml(siteUrl)}/schemas/</loc><lastmod>${generatedAt}</lastmod></url></urlset>\n`, "utf8");
  console.log(`Built ${releases.length} ShroudForge releases and API ${apiCatalog.version} into build/download-website.`);
}

build().catch(error => {
  console.error(`Download website build failed: ${error.message}`);
  process.exitCode = 1;
});
