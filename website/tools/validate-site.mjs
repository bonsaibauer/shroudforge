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
      const relative = decodeURIComponent(url.pathname).replace(/^\/+/, "") || "index.html";
      const file = path.resolve(outputDirectory, relative);
      const boundary = path.relative(outputDirectory, file);
      assert(boundary && !boundary.startsWith("..") && !path.isAbsolute(boundary), `Local link escapes website output in ${sourceName}.`);
      const stat = await fs.lstat(file);
      assert(stat.isFile() && !stat.isSymbolicLink(), `Local link does not point to a real file: ${relative}`);
    }
  }
}

async function validate() {
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
  assert(navLinks.length === 5 && navLinks[0] === "Files" && navLinks[1] === "Wiki" && navLinks[2] === "GitHub" && navLinks[3] === "Discord" && navLinks[4] === "YouTube", "Website navigation must contain Files, Wiki, GitHub, Discord and YouTube in that order.");
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

  for (const relative of ["assets/styles.css", "assets/filters.js", "assets/installation-guide.js", "assets/theme.js", "assets/hammer.png", "assets/favicon.ico", "assets/inter-latin.woff2", "licenses/Inter-OFL.txt"]) {
    await readRequired(relative);
  }
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
  assert(sitemap.includes(`<loc>${siteUrl}/</loc>`), "Website sitemap does not contain the canonical homepage.");
  console.log(`Validated download website: ${feed.releases.length} complete releases, ${securityCount} verified security reports, canonical downloads, local assets and SEO metadata.`);
}

validate().catch(error => {
  console.error(`Download website validation failed: ${error.message}`);
  process.exitCode = 1;
});
