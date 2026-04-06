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
