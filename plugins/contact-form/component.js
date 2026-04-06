// ferrosite-contact-form
// Progressive enhancement: plain <form> works without JS,
// this component intercepts submission and uses the CQRS worker API.

pfusch("ferrosite-contact-form", {
  endpoint: "/api/contact",
  status: "",          // "" | "sending" | "success" | "error"
  errorMsg: "",
}, (state, trigger, { children }) => [
  css`
    :host { display: block; }

    .form { display: flex; flex-direction: column; gap: 1.25rem; }

    .field { display: flex; flex-direction: column; gap: 0.4rem; }

    label {
      font-size: 0.875rem; font-weight: 500;
      color: var(--color-text-muted);
    }

    input, textarea {
      background: var(--color-surface);
      border: 1px solid var(--color-border);
      border-radius: 8px;
      padding: 0.65rem 1rem;
      color: var(--color-text);
      font-family: var(--font-sans);
      font-size: 0.9rem;
      outline: none;
      transition: border-color 0.2s, box-shadow 0.2s;
      width: 100%;
    }
    input:focus, textarea:focus {
      border-color: var(--color-primary);
      box-shadow: 0 0 0 3px rgba(14,165,233,0.15);
    }
    textarea { resize: vertical; min-height: 140px; }

    .row { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    @media (max-width: 520px) { .row { grid-template-columns: 1fr; } }

    .submit-btn {
      align-self: flex-start;
      display: inline-flex; align-items: center; gap: 0.5rem;
      background: var(--color-primary); color: #fff;
      border: none; border-radius: 8px;
      padding: 0.7rem 1.6rem; font-size: 1rem; font-weight: 600;
      font-family: var(--font-sans); cursor: pointer;
      transition: background 0.15s, opacity 0.15s;
    }
    .submit-btn:disabled { opacity: 0.6; cursor: not-allowed; }
    .submit-btn:not(:disabled):hover { background: var(--color-primary-dark); }

    .spinner {
      width: 16px; height: 16px; border: 2px solid rgba(255,255,255,0.3);
      border-top-color: #fff; border-radius: 50%;
      animation: spin 0.7s linear infinite;
    }
    @keyframes spin { to { transform: rotate(360deg); } }

    .alert {
      padding: 1rem 1.25rem; border-radius: 8px;
      font-size: 0.9rem; font-weight: 500;
    }
    .alert-success {
      background: rgba(34,197,94,0.12);
      border: 1px solid rgba(34,197,94,0.3);
      color: var(--color-success);
    }
    .alert-error {
      background: rgba(239,68,68,0.12);
      border: 1px solid rgba(239,68,68,0.3);
      color: var(--color-error);
    }
  `,

  // Success state
  state.status === "success"
    ? html.div({ class: "alert alert-success" },
        "Message sent! I'll get back to you within a day or two.")
    : null,

  // Error state
  state.status === "error"
    ? html.div({ class: "alert alert-error" },
        `${state.errorMsg || "Something went wrong. Please try again or email directly."}`)
    : null,

  // Form (always rendered for progressive enhancement)
  state.status !== "success"
    ? html.form({
        class: "form",
        submit: async (e) => {
          e.preventDefault();
          if (state.status === "sending") return;

          const form = e.target;
          const data = Object.fromEntries(new FormData(form));
          state.status  = "sending";
          state.errorMsg = "";

          try {
            const res = await fetch(state.endpoint, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                command: "SendMessage",
                payload: data,
              }),
            });

            const json = await res.json();
            if (!res.ok || !json.ok) throw new Error(json.error || "Request failed");
            state.status = "success";
          } catch (err) {
            state.status   = "error";
            state.errorMsg = err.message;
          }
        },
      },

      html.div({ class: "row" },
        html.div({ class: "field" },
          html.label({ for: "cf-name" }, "Your Name"),
          html.input({ type: "text", id: "cf-name", name: "name",
            required: true, autocomplete: "name", placeholder: "Jane Smith",
            disabled: state.status === "sending" }),
        ),
        html.div({ class: "field" },
          html.label({ for: "cf-email" }, "Email Address"),
          html.input({ type: "email", id: "cf-email", name: "email",
            required: true, autocomplete: "email", placeholder: "jane@example.com",
            disabled: state.status === "sending" }),
        ),
      ),

      html.div({ class: "field" },
        html.label({ for: "cf-subject" }, "Subject (optional)"),
        html.input({ type: "text", id: "cf-subject", name: "subject",
          placeholder: "Project inquiry, collaboration, ...",
          disabled: state.status === "sending" }),
      ),

      html.div({ class: "field" },
        html.label({ for: "cf-message" }, "Message"),
        html.textarea({ id: "cf-message", name: "message",
          required: true, rows: 6, placeholder: "Tell me about your project...",
          disabled: state.status === "sending" }),
      ),

      html.input({ type: "text", name: "_hp", style: "display:none",
        autocomplete: "off", tabindex: "-1" }),

      html.button({
        type: "submit",
        class: "submit-btn",
        disabled: state.status === "sending",
      },
        state.status === "sending"
          ? [html.div({ class: "spinner" }), "Sending..."]
          : "Send Message"
      ),
    )
    : null,
]);
