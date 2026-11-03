import fs from "node:fs/promises";
import http from "node:http";
import path from "node:path";

const directory = await fs.realpath(path.resolve(import.meta.dirname, "../../build/download-website"))
  .catch(() => { throw new Error("Build the download website first with npm run downloads:check."); });
const boundary = directory + path.sep;
const port = Number(process.env.PORT || 4173);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error("PORT must be between 1 and 65535.");
const types = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".png": "image/png",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
  ".txt": "text/plain; charset=utf-8",
  ".xml": "application/xml; charset=utf-8",
};

const server = http.createServer(async (request, response) => {
  try {
    if (request.method !== "GET" && request.method !== "HEAD") {
      response.writeHead(405, { Allow: "GET, HEAD" }).end();
      return;
    }
    const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
    const candidate = path.resolve(directory, "." + (pathname.endsWith("/") ? pathname + "index.html" : pathname));
    if (!candidate.startsWith(boundary)) {
      response.writeHead(400).end("Invalid path");
      return;
    }
    let file = await fs.realpath(candidate).catch(() => null);
    let status = 200;
    if (!file || !file.startsWith(boundary) || !(await fs.stat(file)).isFile()) {
      file = path.join(directory, "404.html");
      status = 404;
    }
    const content = await fs.readFile(file);
    response.writeHead(status, {
      "Content-Type": types[path.extname(file)] || "application/octet-stream",
      "Content-Length": content.byteLength,
      "Cache-Control": "no-store",
      "X-Content-Type-Options": "nosniff",
    });
    response.end(request.method === "HEAD" ? undefined : content);
  } catch {
    response.writeHead(400).end("Invalid request");
  }
});

server.listen(port, "127.0.0.1", () => console.log(`Local: http://127.0.0.1:${port}`));
