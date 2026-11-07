import fs from "node:fs/promises";
import path from "node:path";
import config from "../config.mjs";
import { renderPage } from "../src/render-page.mjs";
import { readGithubReleases } from "./github-releases.mjs";
import { readSecurityReports } from "./security-reports.mjs";

const websiteDirectory = path.resolve(import.meta.dirname, "..");
const repositoryDirectory = path.resolve(websiteDirectory, "..");
const buildDirectory = path.join(repositoryDirectory, "build");
const outputDirectory = path.join(buildDirectory, "download-website");
const browserAssets = ["styles.css", "filters.js", "installation-guide.js", "theme.js"];

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
  const generatedAt = new Date().toISOString();
  const html = renderPage({ releases, securityReports, generatedAt, siteUrl, config });
  if (typeof html !== "string" || !html.includes("<!doctype html>")) throw new Error("The website renderer must return a complete HTML document.");
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
  await fs.writeFile(path.join(outputDirectory, "404.html"), renderNotFound(siteUrl), "utf8");
  await fs.writeFile(path.join(outputDirectory, "releases.json"), `${JSON.stringify({ generatedAt, releases, securityReports }, null, 2)}\n`, "utf8");
  await fs.writeFile(path.join(outputDirectory, "robots.txt"), `User-agent: *\nAllow: /\nSitemap: ${siteUrl}/sitemap.xml\n`, "utf8");
  await fs.writeFile(path.join(outputDirectory, "sitemap.xml"), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>${escapeHtml(siteUrl)}/</loc><lastmod>${generatedAt}</lastmod></url></urlset>\n`, "utf8");
  console.log(`Built ${releases.length} ShroudForge releases into build/download-website.`);
}

build().catch(error => {
  console.error(`Download website build failed: ${error.message}`);
  process.exitCode = 1;
});
