// Auto-generated CQRS wrapper for plugin: contact-form
// Route: /api/contact
// Worker runtime: cloudflare-worker

const COMMANDS = [
  {
    "name": "SendMessage",
    "description": "Send a contact form message via email",
    "payload_schema": {
      "properties": {
        "email": {
          "format": "email",
          "type": "string"
        },
        "message": {
          "maxLength": 5000,
          "minLength": 10,
          "type": "string"
        },
        "name": {
          "maxLength": 100,
          "minLength": 1,
          "type": "string"
        },
        "subject": {
          "maxLength": 200,
          "type": "string"
        }
      },
      "required": [
        "name",
        "email",
        "message"
      ],
      "type": "object"
    }
  }
];
const QUERIES = [
  {
    "name": "GetStatus",
    "description": "Health check",
    "params_schema": {},
    "response_schema": {
      "properties": {
        "ok": {
          "type": "boolean"
        }
      },
      "type": "object"
    }
  }
];

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type",
};

function defaultCommandName() {
    return COMMANDS.length === 1 ? COMMANDS[0].name : null;
}

function wantsJsonResponse(request) {
    const accept = (request.headers.get("accept") || "").toLowerCase();
    return accept.includes("application/json");
}

function normalizeRedirectTarget(target) {
    if (typeof target !== "string") {
        return "/contact/";
    }

    if (!target.startsWith("/") || target.startsWith("//")) {
        return "/contact/";
    }

    return target;
}

function redirectLocation(payload, status) {
    const base = normalizeRedirectTarget(payload?.redirect_to);
    const url = new URL(base, "https://ferrosite.local");
    url.hash = status === "success" ? "contact-form-success" : "contact-form-error";
    return `${url.pathname}${url.search}${url.hash}`;
}

function redirectResponse(location) {
    return new Response(null, {
        status: 303,
        headers: {
            ...CORS_HEADERS,
            Location: location,
        },
    });
}

function normalizeCommandEnvelope(body) {
    const raw = body && typeof body === "object" && !Array.isArray(body) ? body : {};

    if (typeof raw.command === "string" && raw.payload && typeof raw.payload === "object" && !Array.isArray(raw.payload)) {
        return { command: raw.command, payload: raw.payload };
    }

    if (typeof raw.command === "string") {
        const { command, ...payload } = raw;
        return { command, payload };
    }

    const inferred = defaultCommandName();
    if (inferred) {
        return { command: inferred, payload: raw };
    }

    throw new Error("Command is required");
}

async function parseCommandRequest(request) {
    const contentType = (request.headers.get("content-type") || "").toLowerCase();

    if (contentType.includes("application/json")) {
        return normalizeCommandEnvelope(await request.json());
    }

    if (contentType.includes("application/x-www-form-urlencoded") || contentType.includes("multipart/form-data")) {
        return normalizeCommandEnvelope(Object.fromEntries((await request.formData()).entries()));
    }

    const raw = await request.text();
    if (!raw.trim()) {
        return normalizeCommandEnvelope({});
    }

    try {
        return normalizeCommandEnvelope(JSON.parse(raw));
    } catch (err) {
        if (contentType.includes("text/plain") || raw.includes("=")) {
            return normalizeCommandEnvelope(Object.fromEntries(new URLSearchParams(raw).entries()));
        }

        throw err;
    }
}

// Contact form Cloudflare Worker
// Implements CQRS handlers: handleCommand + handleQuery
// Deployed as a sandboxed lambda via wrangler

// Command handler
async function handleCommand(command, payload, env, ctx) {
  switch (command) {
    case "SendMessage":
      return sendContactEmail(payload, env);
    default:
      throw new Error(`Unknown command: ${command}`);
  }
}

// Query handler
async function handleQuery(query, params, env, ctx) {
  switch (query) {
    case "GetStatus":
      return { ok: true, timestamp: new Date().toISOString() };
    default:
      throw new Error(`Unknown query: ${query}`);
  }
}

// Email delivery via Resend
async function sendContactEmail(payload, env) {
  const { name, email, subject, message } = payload;

  if (!env.RESEND_API_KEY) throw new Error("RESEND_API_KEY not configured");
  if (!env.TO_EMAIL) throw new Error("TO_EMAIL not configured");

  const body = JSON.stringify({
    from: "contact-form@yourdomain.com",
    to: [env.TO_EMAIL],
    reply_to: email,
    subject: subject || `Contact form message from ${name}`,
    html: `
      <h2>New contact form message</h2>
      <p><strong>From:</strong> ${escapeHtml(name)} &lt;${escapeHtml(email)}&gt;</p>
      <p><strong>Subject:</strong> ${escapeHtml(subject || "-")}</p>
      <hr>
      <div style="white-space: pre-wrap">${escapeHtml(message)}</div>
    `,
    text: `From: ${name} <${email}>\n\n${message}`,
  });

  const res = await fetch("https://api.resend.com/emails", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${env.RESEND_API_KEY}`,
      "Content-Type": "application/json",
    },
    body,
  });

  if (!res.ok) {
    const errText = await res.text();
    throw new Error(`Resend API error ${res.status}: ${errText}`);
  }

  return { sent: true };
}

function escapeHtml(str) {
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}


export default {
  async fetch(request, env, ctx) {
    if (request.method === "OPTIONS") {
      return new Response(null, { headers: CORS_HEADERS });
    }

    const url = new URL(request.url);
        let submittedPayload = {};

    try {
      if (request.method === "POST") {
                const { command, payload } = await parseCommandRequest(request);
                submittedPayload = payload;

        const known = COMMANDS.find(c => c.name === command);
        if (!known) {
          return Response.json({ error: `Unknown command: ${command}` }, { status: 400 });
        }

        const result = await handleCommand(command, payload, env, ctx);
                if (!wantsJsonResponse(request)) {
                    return redirectResponse(redirectLocation(payload, "success"));
                }
        return Response.json({ ok: true, result }, { headers: CORS_HEADERS });
      }

      if (request.method === "GET") {
        const query = url.searchParams.get("query");
        const params = Object.fromEntries(url.searchParams);

        const known = QUERIES.find(q => q.name === query);
        if (!known) {
          return Response.json({ error: `Unknown query: ${query}` }, { status: 400 });
        }

        const result = await handleQuery(query, params, env, ctx);
        return Response.json({ ok: true, result }, { headers: CORS_HEADERS });
      }

      return Response.json({ error: "Method not allowed" }, { status: 405, headers: CORS_HEADERS });
        } catch (err) {
            if (request.method === "POST" && !wantsJsonResponse(request)) {
                return redirectResponse(redirectLocation(submittedPayload, "error"));
            }
      return Response.json({ error: err.message }, { status: 500, headers: CORS_HEADERS });
    }
  }
};
