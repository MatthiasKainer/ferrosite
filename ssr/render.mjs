#!/usr/bin/env node
// ssr/render.mjs — pfusch Server-Side Renderer
// Uses Puppeteer to pre-render web components via Shadow DOM serialisation.
//
// Usage:
//   node render.mjs <path-to-html-file> [--root ./dist] [--route /] [--timeout 30000] [--port 7621]
//
// Outputs the fully pre-rendered HTML to stdout.
// The Rust build pipeline reads this output to replace the intermediate file.
//
// Based on: https://github.com/MatthiasKainer/pfusch/tree/main/showcase/social-example-ssr
// Uses getHTML({ includeShadowRoots: true, serializableShadowRoots: true })

import { existsSync, readFileSync, statSync } from "fs";
import { resolve, dirname, extname, join } from "path";
import { fileURLToPath } from "url";
import { createServer } from "http";

const __dirname = dirname(fileURLToPath(import.meta.url));

// ── CLI args ───────────────────────────────────────────────────────────────────

const args = process.argv.slice(2);
const htmlFile = args[0];

function getArgValue(flag, fallback) {
  const index = args.indexOf(flag);
  return index !== -1 ? args[index + 1] : fallback;
}

const timeout = parseInt(getArgValue("--timeout", "30000"), 10);
const port = parseInt(getArgValue("--port", "7621"), 10);

if (!htmlFile) {
  console.error("Usage: node render.mjs <html-file> [--root dir] [--route /path] [--timeout ms] [--port n]");
  process.exit(1);
}

const htmlPath = resolve(htmlFile);
const rootDir = resolve(getArgValue("--root", dirname(htmlPath)));
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

function resolveRequestPath(rootDir, pathname) {
  const safeSegments = sanitizeRequestPath(pathname);
  if (safeSegments === null) {
    return null;
  }

  const direct = join(rootDir, ...safeSegments);
  if (existsSync(direct) && statSync(direct).isFile()) {
    return direct;
  }

  const index = join(rootDir, ...safeSegments, "index.html");
  if (existsSync(index) && statSync(index).isFile()) {
    return index;
  }

  if (safeSegments.length === 0) {
    const rootIndex = join(rootDir, "index.html");
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

async function serveFile(rootDir, routePath, port) {
  return new Promise((resolveServer) => {
    const server = createServer((req, res) => {
      const requestUrl = new URL(req.url ?? "/", `http://127.0.0.1:${port}`);
      const filePath = resolveRequestPath(rootDir, decodeURIComponent(requestUrl.pathname));

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

    server.listen(port, "127.0.0.1", () => {
      resolveServer({ server, url: `http://127.0.0.1:${port}${routePath}` });
    });
  });
}

// ── Main SSR routine ───────────────────────────────────────────────────────────

async function ssr(url, timeoutMs) {
  // Dynamic import of puppeteer (installed separately in ssr/)
  const puppeteer = (await import("puppeteer")).default;

  const browser = await puppeteer.launch({
    headless: "new",
    args: [
      "--no-sandbox",
      "--disable-setuid-sandbox",
      "--enable-experimental-web-platform-features",  // needed for declarative shadow DOM
    ],
  });

  const page = await browser.newPage();

  // Suppress console noise from the page itself
  page.on("console", (msg) => {
    if (msg.type() === "error") {
      process.stderr.write(`[page error] ${msg.text()}\n`);
    }
  });

  await page.goto(url, {
    waitUntil: "networkidle0",
    timeout: timeoutMs,
  });

  // Wait for pfusch components to hydrate
  await page.waitForFunction(
    () => {
      // TODO: That's a bit brittle, find a better way
      const components = document.querySelectorAll(
        "dev-project-card, dev-blog-card, dev-blog-filter, dev-nav-menu, " +
        "dev-dock, dev-timeline-entry, dev-toc, dev-share-buttons, " +
        "dev-social-icon, ferrosite-contact-form, dev-project-grid"
      );
      if (components.length === 0) return true; // no components = fine
      // All components must have a shadow root
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

  // Capture fully serialised HTML including shadow roots
  // This is the key pfusch SSR technique from the showcase
  const html = await page.$eval("html", (el) =>
    el.getHTML({ includeShadowRoots: true, serializableShadowRoots: true })
  );

  await browser.close();
  return `<!DOCTYPE html>\n<html ${html}`;
}

// ── Entry point ────────────────────────────────────────────────────────────────

async function main() {
  const { server, url } = await serveFile(rootDir, routePath, port);

  try {
    const renderedHtml = await ssr(url, timeout);
    server.close();
    process.stdout.write(renderedHtml);
  } catch (err) {
    server.close();
    process.stderr.write(`SSR failed: ${err.message}\n`);
    // On failure, output the original HTML unchanged so the build doesn't break
    process.stdout.write(readFileSync(htmlPath, "utf8"));
    process.exit(1);
  }
}

main().catch((err) => {
  process.stderr.write(`Fatal: ${err.message}\n`);
  process.exit(1);
});
