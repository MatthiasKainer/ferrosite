#!/usr/bin/env node
// ssr/render.mjs — pfusch Server-Side Renderer
// Uses Puppeteer to pre-render web components via Shadow DOM serialisation.
//
// Usage:
//   node render.mjs <path-to-html-file> [--root ./dist] [--route /] [--timeout 30000] [--port 7621]
//   node render.mjs --manifest .ferrosite-cache/ssr-batch.json
//
// Single-file mode outputs the fully pre-rendered HTML to stdout.
// Batch mode reuses one browser across many pages and writes rendered HTML back
// to the output files listed in the manifest.
//
// Based on: https://github.com/MatthiasKainer/pfusch/tree/main/showcase/social-example-ssr
// Uses getHTML({ includeShadowRoots: true, serializableShadowRoots: true })

import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "fs";
import { resolve, dirname, extname, join } from "path";
import { fileURLToPath } from "url";
import { createServer } from "http";

const __dirname = dirname(fileURLToPath(import.meta.url));

// ── CLI args ───────────────────────────────────────────────────────────────────

const args = process.argv.slice(2);
const htmlFile = args[0]?.startsWith("--") ? "" : args[0];

function getArgValue(flag, fallback) {
  const index = args.indexOf(flag);
  return index !== -1 ? args[index + 1] : fallback;
}

const timeout = parseInt(getArgValue("--timeout", "30000"), 10);
const port = parseInt(getArgValue("--port", "7621"), 10);
const manifestPath = getArgValue("--manifest", "");

if (!htmlFile && !manifestPath) {
  console.error("Usage: node render.mjs <html-file> [--root dir] [--route /path] [--timeout ms] [--port n]");
  console.error("   or: node render.mjs --manifest path/to/ssr-batch.json");
  process.exit(1);
}

const htmlPath = htmlFile ? resolve(htmlFile) : "";
const rootDir = htmlFile ? resolve(getArgValue("--root", dirname(htmlPath))) : "";
const routePath = normalizeRoute(getArgValue("--route", "/"));

// ── Local static file server used during the SSR pass ──────────────────────────

function normalizeRoute(route) {
  if (!route || route === "/") {
    return "/";
  }

  return route.startsWith("/") ? route : `/${route}`;
}

function sanitizeRequestPath(pathname) {
  const segments = pathname.split("/").filter(Boolean);
  const safeSegments = [];

  for (const segment of segments) {
    if (segment === "." || segment === ".." || segment.includes("\\")) {
      return null;
    }
    safeSegments.push(segment);
  }

  return safeSegments;
}

function resolveRequestPath(rootPath, pathname) {
  const safeSegments = sanitizeRequestPath(pathname);
  if (safeSegments === null) {
    return null;
  }

  const direct = join(rootPath, ...safeSegments);
  if (existsSync(direct) && statSync(direct).isFile()) {
    return direct;
  }

  const index = join(rootPath, ...safeSegments, "index.html");
  if (existsSync(index) && statSync(index).isFile()) {
    return index;
  }

  if (safeSegments.length === 0) {
    const rootIndex = join(rootPath, "index.html");
    if (existsSync(rootIndex) && statSync(rootIndex).isFile()) {
      return rootIndex;
    }
  }

  return null;
}

function guessMimeType(filePath) {
  switch (extname(filePath)) {
    case ".html":
      return "text/html; charset=utf-8";
    case ".css":
      return "text/css; charset=utf-8";
    case ".js":
    case ".mjs":
      return "application/javascript; charset=utf-8";
    case ".json":
      return "application/json; charset=utf-8";
    case ".svg":
      return "image/svg+xml";
    case ".png":
      return "image/png";
    case ".jpg":
    case ".jpeg":
      return "image/jpeg";
    case ".gif":
      return "image/gif";
    case ".webp":
      return "image/webp";
    case ".ico":
      return "image/x-icon";
    case ".txt":
      return "text/plain; charset=utf-8";
    case ".wasm":
      return "application/wasm";
    default:
      return "application/octet-stream";
  }
}

async function serveRoot(rootPath, requestedPort) {
  return new Promise((resolveServer) => {
    const server = createServer((req, res) => {
      const requestUrl = new URL(req.url ?? "/", "http://127.0.0.1");
      const filePath = resolveRequestPath(rootPath, decodeURIComponent(requestUrl.pathname));

      if (!filePath) {
        res.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
        res.end("Not found");
        return;
      }

      const body = readFileSync(filePath);
      res.writeHead(200, {
        "Content-Type": guessMimeType(filePath),
        "Content-Length": body.length,
      });

      if (req.method === "HEAD") {
        res.end();
        return;
      }

      res.end(body);
    });

    server.listen(requestedPort, "127.0.0.1", () => {
      const address = server.address();
      const actualPort = typeof address === "object" && address ? address.port : requestedPort;
      resolveServer({ server, baseUrl: `http://127.0.0.1:${actualPort}` });
    });
  });
}

function closeServer(server) {
  return new Promise((resolveClose) => {
    server.close(() => resolveClose());
  });
}

// ── Main SSR routines ──────────────────────────────────────────────────────────

async function launchBrowser() {
  const puppeteer = (await import("puppeteer")).default;

  return puppeteer.launch({
    headless: "new",
    args: [
      "--no-sandbox",
      "--disable-setuid-sandbox",
      "--enable-experimental-web-platform-features",
    ],
  });
}

async function renderUrl(browser, url, timeoutMs, label = url) {
  const page = await browser.newPage();

  page.on("console", (msg) => {
    if (msg.type() === "error") {
      process.stderr.write(`[page error] ${label}: ${msg.text()}\n`);
    }
  });

  try {
    await page.goto(url, {
      waitUntil: "networkidle0",
      timeout: timeoutMs,
    });

    await page.waitForFunction(
      () => {
        const components = document.querySelectorAll(
          "dev-project-card, dev-blog-card, dev-blog-filter, dev-nav-menu, " +
          "dev-dock, dev-timeline-entry, dev-toc, dev-share-buttons, " +
          "dev-social-icon, ferrosite-contact-form, dev-project-grid"
        );
        if (components.length === 0) return true;
        return [...components].every((el) => el.shadowRoot !== null);
      },
      { timeout: timeoutMs }
    );

    await page.evaluate(() => {
      const STYLE_MARKER_ATTR = "data-ferrosite-ssr-adopted-styles";

      function serializeSheet(sheet) {
        try {
          return Array.from(sheet.cssRules || [])
            .map((rule) => rule.cssText)
            .join("\n");
        } catch {
          return "";
        }
      }

      function materializeRootStyles(root) {
        if (!root || !("adoptedStyleSheets" in root)) {
          return;
        }

        const cssText = Array.from(root.adoptedStyleSheets || [])
          .map(serializeSheet)
          .filter(Boolean)
          .join("\n");

        if (!cssText) {
          return;
        }

        const container = root instanceof Document ? root.head : root;
        if (!container) {
          return;
        }

        let styleEl = container.querySelector(`style[${STYLE_MARKER_ATTR}]`);
        if (!styleEl) {
          styleEl = document.createElement("style");
          styleEl.setAttribute(STYLE_MARKER_ATTR, "true");
          container.insertBefore(styleEl, container.firstChild);
        }

        styleEl.textContent = cssText;
      }

      function walk(node) {
        if (!node) {
          return;
        }

        if (node.shadowRoot) {
          materializeRootStyles(node.shadowRoot);
          for (const child of node.shadowRoot.children) {
            walk(child);
          }
        }

        for (const child of node.children || []) {
          walk(child);
        }
      }

      walk(document.documentElement);
    });

    const html = await page.$eval("html", (el) =>
      el.getHTML({ includeShadowRoots: true, serializableShadowRoots: true })
    );

    return `<!DOCTYPE html>\n<html ${html}`;
  } finally {
    await page.close();
  }
}

async function runWithConcurrency(items, concurrency, worker) {
  const limit = Math.max(1, Math.min(concurrency, items.length || 1));
  let nextIndex = 0;

  await Promise.all(
    Array.from({ length: limit }, async () => {
      while (true) {
        const index = nextIndex++;
        if (index >= items.length) {
          return;
        }

        await worker(items[index], index);
      }
    })
  );
}

async function renderBatch(manifest) {
  const { server, baseUrl } = await serveRoot(resolve(manifest.rootDir), 0);
  const browser = await launchBrowser();

  try {
    await runWithConcurrency(
      manifest.jobs || [],
      Number.parseInt(`${manifest.concurrency ?? 2}`, 10) || 2,
      async (job) => {
        const url = `${baseUrl}${normalizeRoute(job.routePath)}`;
        const renderedHtml = await renderUrl(browser, url, manifest.timeoutMs, job.routePath);
        mkdirSync(dirname(job.outputPath), { recursive: true });
        writeFileSync(job.outputPath, renderedHtml, "utf8");
      }
    );
  } finally {
    await browser.close();
    await closeServer(server);
  }
}

async function renderSingleFile(htmlPathArg, rootPathArg, routePathArg, timeoutMs, requestedPort) {
  const { server, baseUrl } = await serveRoot(rootPathArg, requestedPort);
  const browser = await launchBrowser();

  try {
    const renderedHtml = await renderUrl(browser, `${baseUrl}${routePathArg}`, timeoutMs, routePathArg);
    process.stdout.write(renderedHtml);
  } finally {
    await browser.close();
    await closeServer(server);
  }
}

// ── Entry point ────────────────────────────────────────────────────────────────

async function main() {
  if (manifestPath) {
    const manifest = JSON.parse(readFileSync(resolve(manifestPath), "utf8"));
    await renderBatch(manifest);
    return;
  }

  await renderSingleFile(htmlPath, rootDir, routePath, timeout, port);
}

main().catch((err) => {
  process.stderr.write(`SSR failed: ${err.message}\n`);
  if (htmlPath) {
    process.stdout.write(readFileSync(htmlPath, "utf8"));
  }
  process.exit(1);
});
