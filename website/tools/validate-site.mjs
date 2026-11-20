import fs from "node:fs/promises";
import path from "node:path";
import config from "../config.mjs";
import { compareReleases, normalizeReleases } from "./github-releases.mjs";
import { validateSecurityCatalog } from "./validate-security.mjs";

const outputDirectory = path.resolve(import.meta.dirname, "../../build/download-website");

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function decodeAttribute(value) {
  return value.replace(/&(?:amp|quot|apos|lt|gt|#39|#x27);/g, entity => ({
    "&amp;": "&", "&quot;": '"', "&apos;": "'", "&lt;": "<", "&gt;": ">", "&#39;": "'", "&#x27;": "'",
  })[entity]);
}

function tagsFrom(html) {
  const tags = [];
  const withoutScripts = html.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, match => match.slice(0, match.indexOf(">") + 1));
  for (const match of withoutScripts.matchAll(/<([a-z][a-z\d:-]*)\b([^<>]*?)>/gi)) {
    const attributes = {};
    for (const attribute of match[2].matchAll(/([\w:-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/g)) {
      attributes[attribute[1].toLowerCase()] = decodeAttribute(attribute[2] ?? attribute[3]);
    }
    tags.push({ name: match[1].toLowerCase(), attributes });
  }
  return tags;
}

async function readRequired(relative) {
  const file = path.join(outputDirectory, relative);
  const stat = await fs.lstat(file);
  assert(stat.isFile() && !stat.isSymbolicLink() && stat.size > 0, `Missing or invalid website file: ${relative}`);
  return fs.readFile(file, "utf8");
}

async function validateLocalLinks(tags, siteUrl, sourceName) {
  for (const { attributes } of tags) {
    for (const attribute of ["href", "src"]) {
      const value = attributes[attribute];
      if (!value || value.startsWith("#")) continue;
      const url = new URL(value, `${siteUrl}/`);
      assert(["http:", "https:"].includes(url.protocol), `Unsupported link protocol in ${sourceName}.`);
      if (url.origin !== siteUrl) continue;
      const pathname = decodeURIComponent(url.pathname);
      let relative = pathname.replace(/^\/+/, "") || "index.html";
      if (pathname !== "/" && pathname.endsWith("/")) relative = path.join(relative, "index.html");
      const file = path.resolve(outputDirectory, relative);
      const boundary = path.relative(outputDirectory, file);
      assert(boundary && !boundary.startsWith("..") && !path.isAbsolute(boundary), `Local link escapes website output in ${sourceName}.`);
      const stat = await fs.lstat(file);
      assert(stat.isFile() && !stat.isSymbolicLink(), `Local link does not point to a real file: ${relative}`);
    }
  }
}

async function validate() {
  const headers = await readRequired("_headers");
  assert(/Content-Security-Policy:[^\r\n]*connect-src 'self'(?:;|$)/.test(headers), "The website Content Security Policy must allow same-origin fetch requests used by the static API tools.");
  const apiVersionIndex = JSON.parse(await readRequired("data/api/versions.json"));
  assert(Array.isArray(apiVersionIndex.versions) && apiVersionIndex.versions.length > 0, "The published API version index must contain at least one API version.");
  assert(apiVersionIndex.versions.some(entry => entry.version === apiVersionIndex.current), "The published API version index must include its current version.");
  for (const entry of apiVersionIndex.versions) {
    assert(typeof entry.version === "string" && /^[0-9]+(?:\.[0-9A-Za-z-]+)+$/.test(entry.version), "Published API versions must be safe version identifiers.");
    const versionBase = `data/api/${entry.version}`;
    const catalog = JSON.parse(await readRequired(`${versionBase}/catalog.json`));
    const examples = JSON.parse(await readRequired(`${versionBase}/examples.json`));
    const operations = JSON.parse(await readRequired(`${versionBase}/runtime-operations.json`));
    assert(catalog.version === entry.version && Array.isArray(catalog.symbols), `The API catalogue is missing or invalid for version ${entry.version}.`);
    assert(Array.isArray(examples.scenarios), `API example scenarios are missing or invalid for version ${entry.version}.`);
    assert(Array.isArray(operations.runtimeOperations), `Runtime operation declarations are missing or invalid for version ${entry.version}.`);
  }
  const html = await readRequired("index.html");
  const tags = tagsFrom(html);
  assert(/^<!doctype html>/i.test(html.trimStart()), "Website must have an HTML5 doctype.");
  assert(/<html\b[^>]*\blang="en"/i.test(html), "Website must declare its document language.");
  assert((html.match(/<h1\b/gi) || []).length === 1, "Website must have exactly one H1.");
  const title = html.match(/<title>([\s\S]*?)<\/title>/i)?.[1];
  assert(title?.includes(config.name) && /Enshrouded/i.test(title), "Website title must identify ShroudForge and Enshrouded.");
  const meta = (key, value) => tags.find(tag => tag.name === "meta" && tag.attributes[key] === value)?.attributes.content;
  assert(meta("name", "description")?.length >= 60, "Website needs a descriptive search snippet.");
  assert(meta("name", "viewport"), "Website must declare a responsive viewport.");
  const canonical = tags.find(tag => tag.name === "link" && tag.attributes.rel === "canonical")?.attributes.href;
  assert(canonical, "Website canonical URL is missing.");
  const canonicalUrl = new URL(canonical);
  const siteUrl = canonicalUrl.origin;
  assert(canonicalUrl.pathname === "/" && !canonicalUrl.search && !canonicalUrl.hash, "Website canonical URL must identify its root.");
  const expectedOrigin = new URL(process.env.WEBSITE_ORIGIN || config.siteUrl).origin;
  assert(siteUrl === expectedOrigin && canonical === `${siteUrl}/`, "Website canonical URL does not match the configured origin.");
  for (const key of ["og:title", "og:description", "og:type", "og:image", "og:url"]) {
    assert(meta("property", key), `Website metadata is missing ${key}.`);
  }
  assert(meta("property", "og:url") === canonical, "Open Graph URL must match the canonical URL.");
  assert(meta("name", "twitter:card"), "Website Twitter card metadata is missing.");
  const structuredData = [...html.matchAll(/<script\b[^>]*type=["']application\/ld\+json["'][^>]*>([\s\S]*?)<\/script>/gi)];
  assert(structuredData.length > 0, "Website structured software metadata is missing.");
  const entities = structuredData.flatMap(match => {
    const data = JSON.parse(match[1]);
    return Array.isArray(data) ? data : data["@graph"] || [data];
  });
  const software = entities.find(data => data["@type"] === "SoftwareApplication");
  assert(software?.name === config.name, "Structured data must identify ShroudForge as a SoftwareApplication.");
  assert(software.offers?.price === 0, "Software metadata must identify the free download accurately.");
  const website = entities.find(data => data["@type"] === "WebSite");
  assert(website?.name === config.name && website.url === canonical, "Structured data must identify the canonical website and its site name.");
  const indexable = siteUrl === new URL(config.siteUrl).origin;
  assert(meta("name", "robots") === (indexable ? "index, follow, max-image-preview:large" : "noindex, nofollow"), "Indexing rules must allow production and exclude alternate preview origins.");
  const navigation = html.match(/<nav\b[^>]*>([\s\S]*?)<\/nav>/i)?.[1];
  assert(navigation, "Website top navigation is missing.");
  const navLinks = [...navigation.matchAll(/<a\b[^>]*>([\s\S]*?)<\/a>/gi)].map(match => match[1].replace(/<[^>]*>/g, "").trim());
  assert(navLinks.length === 6 && navLinks[0] === "Files" && navLinks[1] === "Tools" && navLinks[2] === "Wiki" && navLinks[3] === "GitHub" && navLinks[4] === "Discord" && navLinks[5] === "YouTube", "Website navigation must contain Files, Tools, Wiki, GitHub, Discord and YouTube in that order.");
  const youtubeLink = tagsFrom(navigation).filter(tag => tag.name === "a").at(-1);
  assert(youtubeLink?.attributes.href === config.youtubeUrl, "The rightmost navigation link must point to the configured YouTube channel.");
  const repositoryUrl = `https://github.com/${config.repository}`;
  assert(tagsFrom(navigation).some(tag => tag.name === "a" && tag.attributes.href === repositoryUrl), "Website navigation must link to the project repository.");
  assert(tagsFrom(navigation).some(tag => tag.name === "a" && tag.attributes.href === config.wikiUrl), "Website navigation must link to the configured Wiki.");
  const sidebar = html.match(/<aside\b[^>]*>([\s\S]*?)<\/aside>/i)?.[1] || "";
  assert(tagsFrom(sidebar).some(tag => tag.name === "a" && tag.attributes.href === repositoryUrl), "The sidebar GitHub reference must link to the project repository.");
  const footer = html.match(/<footer\b[^>]*>([\s\S]*?)<\/footer>/i)?.[1] || "";
  const footerLinks = tagsFrom(footer).filter(tag => tag.name === "a").map(tag => tag.attributes.href);
  assert(footerLinks.indexOf(repositoryUrl) >= 0 && footerLinks[footerLinks.indexOf(repositoryUrl) + 1] === config.statusUrl, "The footer must place the Status link next to Source code.");
  assert(tags.some(tag => tag.name === "a" && tag.attributes.href === config.discordUrl), "Website Discord link does not match the project configuration.");
  assert(tags.some(tag => tag.name === "link" && tag.attributes.rel === "stylesheet" && tag.attributes.href === "/assets/styles.css"), "Website stylesheet is missing.");
  assert(tags.some(tag => tag.name === "a" && tag.attributes.href === "/tools/modding/mod-settings/" && tag.attributes["aria-current"] === undefined), "Website Tools navigation must open the Mod-Settings tool.");
  assert(tags.some(tag => tag.name === "script" && tag.attributes.src === "/assets/filters.js"), "Website release filter script is missing.");
  assert(tags.some(tag => tag.name === "script" && tag.attributes.src === "/assets/installation-guide.js"), "Website installation guide script is missing.");
  const themeScript = tags.findIndex(tag => tag.name === "script" && tag.attributes.src === "/assets/theme.js");
  const stylesheet = tags.findIndex(tag => tag.name === "link" && tag.attributes.rel === "stylesheet");
  assert(themeScript >= 0 && themeScript < stylesheet, "Saved background preference must load before the stylesheet.");
  assert(tags.some(tag => tag.name === "html" && tag.attributes["data-theme"] === "light") && meta("name", "color-scheme") === "light", "The website must use a light background by default.");
  assert(tags.some(tag => tag.name === "button" && tag.attributes.id === "theme-toggle" && tag.attributes.role === "switch" && tag.attributes["aria-checked"] === "true"), "The footer background switch is missing or inaccessible.");
  assert(!/api\.github\.com|GH_TOKEN|GITHUB_TOKEN|github_pat_|gh[pousr]_[A-Za-z0-9]{20,}/.test(html), "Website HTML must not expose GitHub API calls or access tokens.");

  const feedText = await readRequired("releases.json");
  const feed = JSON.parse(feedText);
  assert(Array.isArray(feed.releases) && feed.releases.length > 0, "Website release catalog is empty.");
  assert(Number.isFinite(Date.parse(feed.generatedAt)), "Website release catalog needs a valid build timestamp.");
  assert(!/github_pat_|gh[pousr]_[A-Za-z0-9]{20,}|GH_TOKEN|GITHUB_TOKEN/.test(feedText), "Website release catalog must not expose access tokens.");
  assert(new Set(feed.releases.map(release => release.tag)).size === feed.releases.length, "Website release tags must be unique.");
  const raw = feed.releases.map(release => ({
    tag_name: release.tag,
    prerelease: release.prerelease,
    draft: false,
    published_at: release.publishedAt,
    html_url: release.releaseUrl,
    assets: [
      { name: release.asset.name, state: "uploaded", browser_download_url: release.asset.url, size: release.asset.size, download_count: release.asset.downloadCount },
      { name: release.checksum.name, state: "uploaded", browser_download_url: release.checksum.url },
      ...(release.securityReport ? [{ name: release.securityReport.name, state: "uploaded", browser_download_url: release.securityReport.url }] : []),
    ],
  }));
  const normalized = normalizeReleases(raw, config.repository);
  assert(JSON.stringify(normalized) === JSON.stringify(feed.releases), "Website release catalog does not follow the canonical release and asset contract.");
  const links = new Set(tags.filter(tag => tag.name === "a").map(tag => tag.attributes.href));
  const securityCount = validateSecurityCatalog({ releases: feed.releases, securityReports: feed.securityReports, links, tags, repository: config.repository });
  for (let index = 0; index < feed.releases.length; index++) {
    const release = feed.releases[index];
    if (index) assert(compareReleases(feed.releases[index - 1], release) <= 0, "Website releases are not sorted by descending version and build.");
    assert(links.has(release.asset.url), `Release download is not rendered: ${release.tag}`);
    assert(links.has(release.checksum.url), `Release checksum is not rendered: ${release.tag}`);
    assert(links.has(release.releaseUrl), `Release notes are not rendered: ${release.tag}`);
  }
  const recommended = feed.releases.find(release => !release.prerelease);
  if (recommended) assert(software.softwareVersion === recommended.version, "Software metadata must describe the latest stable version.");

  for (const relative of ["assets/styles.css", "assets/filters.js", "assets/installation-guide.js", "assets/mod-settings.css", "assets/mod-settings.js", "assets/schemas.css", "assets/schemas.js", "assets/api-tools.css", "assets/api-tools.js", "assets/theme.js", "assets/hammer.png", "assets/favicon.ico", "assets/inter-latin.woff2", "licenses/Inter-OFL.txt"]) {
    await readRequired(relative);
  }
  const previewServer = await fs.readFile(path.resolve(import.meta.dirname, "preview-site.mjs"), "utf8");
  assert(previewServer.includes('".svg": "image/svg+xml"'), "The local website preview must serve SVG badge and Modloader icons with the correct image MIME type.");
  const toolScript = await readRequired("assets/mod-settings.js");
  assert(toolScript.includes("settings-disclosure-arrow") && toolScript.includes("data-disclosure-label") && toolScript.includes("'Show'") && toolScript.includes("'Hide'"), "The settings list disclosure must show a clear expand affordance.");
  const repositoryVersion = (await fs.readFile(path.resolve(import.meta.dirname, "../../VERSION"), "utf8")).trim();
  const toolRelative = "tools/modding/mod-settings/index.html";
  const toolHtml = await readRequired(toolRelative);
  const toolTags = tagsFrom(toolHtml);
  assert(/^<!doctype html>/i.test(toolHtml.trimStart()), "Mod-Settings must be an HTML5 document.");
  assert((toolHtml.match(/<h1\b/gi) || []).length === 1 && /<h1[^>]*>Mod-Settings<\/h1>/i.test(toolHtml), "Mod-Settings needs one visible title.");
  const moddingNavGroup = toolHtml.match(/<div class="tools-nav-group">\s*<div class="tools-category-label"[^>]*>Modding<\/div>([\s\S]*?)<\/div>/)?.[1] || "";
  assert(tagsFrom(moddingNavGroup).some(tag => tag.name === "a" && tag.attributes.href === "/tools/modding/mod-settings/") && tagsFrom(moddingNavGroup).some(tag => tag.name === "a" && tag.attributes.href === "/tools/modding/api-playground/") && tagsFrom(moddingNavGroup).some(tag => tag.name === "a" && tag.attributes.href === "/tools/modding/api-reference/"), "The Modding sidebar must include Mod-Settings and both API tools.");
  assert(toolHtml.includes(`value="${repositoryVersion}">${repositoryVersion}</option>`), "The preview selector must display only the current repository Modloader version.");
  assert(toolHtml.includes('data-json-editor="manifest"') && toolHtml.includes('data-json-editor="extended"'), "Mod-Settings must expose both manifest JSON editors.");
  assert(["id", "name", "version", "description", "authors", "license", "icon"].every(field => toolHtml.includes(`data-manifest-field="${field}"`)) && !toolHtml.includes('data-manifest-field="$schema"') && toolHtml.includes("data-manifest-dependencies") && toolHtml.includes("data-manifest-capabilities"), "The Mod-Settings form must expose manifest metadata, dependencies and capabilities without a manual schema-path field.");
  assert(toolHtml.includes(`href="${siteUrl}/schemas/${repositoryVersion}/manifest.schema.json"`) && toolHtml.includes(`href="${siteUrl}/schemas/${repositoryVersion}/extended.mod.schema.json"`), "Both JSON editors must link to their current versioned schemas.");
  assert(toolHtml.includes('data-line-number-list="manifest"') && toolHtml.includes('data-line-number-list="extended"') && (toolHtml.match(/data-json-warning/g) || []).length === 2, "Both JSON editors must show line numbers and accessible warning indicators.");
  assert((toolHtml.match(/data-reset-example/g) || []).length === 1 && toolHtml.indexOf("data-reset-example") < toolHtml.indexOf("LIVE MODLOADER UI"), "Reset example must appear once at the top of the tool.");
  assert(toolTags.some(tag => tag.name === "iframe" && tag.attributes.src === "/tools/modding/mod-settings/preview.html"), "Mod-Settings must load its browser-rendered Modloader preview.");
  assert(toolTags.some(tag => tag.name === "script" && tag.attributes.src === "/assets/mod-settings.js"), "Mod-Settings browser controls are missing.");
  const toolNavigation = toolHtml.match(/<nav\b[^>]*>([\s\S]*?)<\/nav>/i)?.[1] || "";
  assert(tagsFrom(toolNavigation).some(tag => tag.name === "a" && tag.attributes.href === "/tools/modding/mod-settings/"), "The Tools navigation must keep the Mod-Settings route reachable.");
  const standardsNavGroup = toolHtml.match(/<div class="tools-nav-group">\s*<div class="tools-category-label"[^>]*>Standards<\/div>([\s\S]*?)<\/div>\s*<\/div>\s*<\/aside>/)?.[0] || "";
  assert(standardsNavGroup.includes('href="/schemas/">Schemas</a>') && standardsNavGroup.includes('class="icon ') && standardsNavGroup.includes("DInput8 Standard") && tagsFrom(standardsNavGroup).some(tag => tag.name === "a" && tag.attributes.href === config.dinput8ChainStandardUrl), "The Standards category must link to Schemas and the DInput8 Wiki article with an external-link icon.");
  const example = JSON.parse(toolHtml.match(/<template id="mod-settings-example">([\s\S]*?)<\/template>/i)?.[1]?.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&") || "null");
  const starterManifest = JSON.parse(await fs.readFile(path.resolve(import.meta.dirname, "../../docs/examples/starter-mod/mod.json"), "utf8"));
  const starterExtended = JSON.parse(await fs.readFile(path.resolve(import.meta.dirname, "../../docs/examples/starter-mod/extended.mod.json"), "utf8"));
  starterManifest.$schema = `${siteUrl}/schemas/${repositoryVersion}/manifest.schema.json`;
  starterExtended.$schema = `${siteUrl}/schemas/${repositoryVersion}/extended.mod.schema.json`;
  assert(example?.version === repositoryVersion && JSON.stringify(example.manifest) === JSON.stringify(starterManifest) && JSON.stringify(example.extended) === JSON.stringify(starterExtended) && example.schemaBaseUrl === `${siteUrl}/schemas/` && Array.isArray(example.links) && example.links.length > 0, "The tool must use the current starter-mod example, versioned schema URLs, badge catalog, and Modloader version.");
  for (const link of example.links) {
    const iconFile = `assets/mod-settings-links/${link.icon}.svg`;
    await readRequired(iconFile);
  }
  const toolIcon = await readRequired("tools/modding/mod-settings/icon.svg");
  const starterIcon = await fs.readFile(path.resolve(import.meta.dirname, "../../docs/examples/starter-mod/icon.svg"));
  assert(Buffer.from(toolIcon).equals(starterIcon), "The preview mod icon must match docs/examples/starter-mod/icon.svg.");
  const previewHtml = await readRequired("tools/modding/mod-settings/preview.html");
  assert(previewHtml.includes("SHROUDFORGE MODLOADER") || previewHtml.includes("ShroudForge Modloader preview"), "The current Modloader UI preview bundle is missing.");
  assert(/<script[^>]+src="\.\/preview\.js"/i.test(previewHtml) && /<link[^>]+href="\.\/preview\.css"/i.test(previewHtml), "The browser preview must load its static frontend bundle and stylesheet.");
  const previewScript = await readRequired("tools/modding/mod-settings/preview.js");
  assert(!previewScript.includes("preview-version-chip"), "The Modloader version must not be repeated in the preview header.");
  await readRequired("tools/modding/mod-settings/preview.css");
  const css = await readRequired("assets/styles.css");
  assert(!/https?:\/\//.test(css), "Website fonts and styles must be hosted locally.");
  const script = await readRequired("assets/filters.js");
  assert(!/api\.github\.com|\bfetch\s*\(/.test(script), "Browser release filters must use the generated document without remote API requests.");
  for (const match of css.matchAll(/url\(\s*["']?([^\s"')]+)["']?\s*\)/g)) {
    const asset = new URL(match[1], `${siteUrl}/assets/styles.css`);
    await validateLocalLinks([{ attributes: { href: asset.href } }], siteUrl, "styles.css");
  }
  const notFound = await readRequired("404.html");
  assert(/<meta\b[^>]*name="robots"[^>]*content="noindex"/i.test(notFound), "The 404 document must be excluded from search indexing.");
  await validateLocalLinks(tags, siteUrl, "index.html");
  await validateLocalLinks(tagsFrom(notFound), siteUrl, "404.html");
  const ogImage = new URL(meta("property", "og:image"));
  assert(ogImage.origin === siteUrl, "Open Graph image must be hosted by this website.");
  await validateLocalLinks([{ attributes: { src: ogImage.href } }], siteUrl, "Open Graph image");
  const robots = await readRequired("robots.txt");
  assert(robots.includes("Allow: /") && robots.includes(`Sitemap: ${siteUrl}/sitemap.xml`), "Website robots.txt does not expose the canonical sitemap.");
  const sitemap = await readRequired("sitemap.xml");
  assert(sitemap.includes(`<loc>${siteUrl}/</loc>`) && sitemap.includes(`<loc>${siteUrl}/tools/modding/mod-settings/</loc>`) && sitemap.includes(`<loc>${siteUrl}/tools/modding/api-reference/</loc>`) && sitemap.includes(`<loc>${siteUrl}/tools/modding/api-playground/</loc>`) && sitemap.includes(`<loc>${siteUrl}/schemas/</loc>`) && !sitemap.includes("/standards/dinput8-chain/"), "Website sitemap must contain the homepage, Modding tools and schema catalog, with external standards linked directly.");
  await validateLocalLinks(toolTags, siteUrl, toolRelative);
  const schemaHtml = await readRequired("schemas/index.html");
  const schemaTags = tagsFrom(schemaHtml);
  assert(/^<!doctype html>/i.test(schemaHtml.trimStart()) && schemaHtml.includes("<title>Schema |") && /<h1[^>]*>Schema<\/h1>/i.test(schemaHtml), "The schema catalog must render as an HTML page titled Schema.");
  assert(!schemaHtml.includes('class="schema-standard-panel'), "The DInput8 standard card must be separate from the schema catalog page.");
  assert(schemaHtml.includes('data-schema-version aria-label="Modloader version"') && schemaHtml.includes(`value="${repositoryVersion}"`) && schemaHtml.includes('selected>'), "The schema catalog must select the current Modloader version.");
  assert(schemaTags.some(tag => tag.name === "link" && tag.attributes.rel === "canonical" && tag.attributes.href === `${siteUrl}/schemas/`), "The Schema page must declare its canonical URL.");
  const fullHtmlPages = [
    ["index.html", html],
    ["schemas/index.html", schemaHtml],
    [toolRelative, toolHtml],
    ["tools/modding/api-reference/index.html", await readRequired("tools/modding/api-reference/index.html")],
    ["tools/modding/api-playground/index.html", await readRequired("tools/modding/api-playground/index.html")],
    ["404.html", notFound],
  ];
  for (const [relative, pageHtml] of fullHtmlPages) {
    const siteFooterCount = (pageHtml.match(/<footer\b[^>]*class="site-footer"/gi) || []).length;
    assert(siteFooterCount === 1 && /<footer\b[^>]*class="site-footer"[\s\S]*?<\/footer>\s*<\/body>/i.test(pageHtml), `The shared site footer must appear once at the bottom of ${relative}.`);
  }
  await validateLocalLinks(schemaTags, siteUrl, "schemas/index.html");
  const schemaNames = ["manifest.schema.json", "extended.mod.schema.json", "mod-locale.schema.json"];
  const snapshotDirectory = path.resolve(import.meta.dirname, "../schema-snapshots");
  const versions = (await fs.readdir(snapshotDirectory, { withFileTypes: true })).filter(entry => entry.isDirectory()).map(entry => entry.name);
  assert(versions.includes(repositoryVersion), "The current versioned schema snapshot is missing.");
  for (const version of versions) {
    for (const name of schemaNames) {
      const relative = `schemas/${version}/${name}`;
      const schema = JSON.parse(await readRequired(relative));
      assert(schema.$id === `${siteUrl}/${relative}`, `Published schema $id must use its permanent versioned URL: ${relative}`);
      assert(typeof schema.$schema === "string", `Published schema must declare its JSON Schema dialect: ${relative}`);
    }
  }
  console.log(`Validated download website: ${feed.releases.length} releases, ${securityCount} security reports, Mod-Settings v${repositoryVersion}, versioned schemas, static preview assets and SEO metadata.`);
}

validate().catch(error => {
  console.error(`Download website validation failed: ${error.message}`);
  process.exitCode = 1;
});
