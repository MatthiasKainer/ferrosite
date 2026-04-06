import { Buffer } from "node:buffer";
import { pathToFileURL } from "node:url";

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf8");
}

const raw = await readStdin();
const envelope = JSON.parse(raw);
const moduleUrl = pathToFileURL(envelope.worker_path).href;
const workerModule = await import(moduleUrl);

const headers = new Headers(envelope.request.headers || {});
const requestInit = {
  method: envelope.request.method,
  headers,
};

if (envelope.request.body_base64) {
  requestInit.body = Buffer.from(envelope.request.body_base64, "base64");
}

const waitUntilPromises = [];
const ctx = {
  waitUntil(promise) {
    waitUntilPromises.push(Promise.resolve(promise));
  },
  passThroughOnException() {},
};

const request = new Request(envelope.request.url, requestInit);
const response = await workerModule.default.fetch(request, envelope.env || {}, ctx);
await Promise.allSettled(waitUntilPromises);

const responseHeaders = {};
response.headers.forEach((value, key) => {
  responseHeaders[key] = value;
});

const bodyBuffer = Buffer.from(new Uint8Array(await response.arrayBuffer()));
process.stdout.write(JSON.stringify({
  status: response.status,
  headers: responseHeaders,
  body_base64: bodyBuffer.toString("base64"),
}));
