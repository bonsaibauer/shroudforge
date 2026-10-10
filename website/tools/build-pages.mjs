import fs from "node:fs";
import path from "node:path";

const website = path.resolve(import.meta.dirname, "..");
const pagesRoot = path.join(website, "pages");
const output = path.join(pagesRoot, "index.js");

const pages = walk(pagesRoot)
  .filter(file => path.basename(file) === "page.json")
  .map(file => {
    const directory = path.dirname(file);
    const metadata = JSON.parse(fs.readFileSync(file, "utf8"));
    for (const locale of ["de", "en"]) {
      const content = path.join(directory, `${locale}.md`);
      if (!fs.existsSync(content)) throw new Error(`Missing ${locale}.md for ${metadata.id}`);
    }
    return {
      ...metadata,
      path: path.relative(website, directory).replaceAll("\\", "/") + "/",
    };
  })
  .sort((left, right) => (left.order ?? 1000) - (right.order ?? 1000) || left.id.localeCompare(right.id));

const ids = new Set();
for (const page of pages) {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(page.id)) throw new Error(`Invalid page id: ${page.id}`);
  if (ids.has(page.id)) throw new Error(`Duplicate page id: ${page.id}`);
  ids.add(page.id);
}

fs.writeFileSync(output, `export default ${JSON.stringify(pages, null, 2)};\n`);
console.log(`Indexed ${pages.length} Markdown pages for the ShroudForge website.`);

function walk(directory) {
  if (!fs.existsSync(directory)) return [];
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(file) : [file];
  });
}
