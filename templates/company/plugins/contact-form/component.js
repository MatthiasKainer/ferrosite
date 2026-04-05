// ferrosite-contact-form
// Progressive enhancement: plain <form> works without JS,
// this component intercepts submission and uses the CQRS worker API.

pfusch("ferrosite-contact-form", {
  endpoint: "/api/contact",
  status: "",
  errorMsg: "",
}, (state) => [
  css`
    :host { display: block; }

    .form { display: flex; flex-direction: column; gap: 1.25rem; }
    .field { display: flex; flex-direction: column; gap: 0.45rem; }

    label {
      font-size: 0.9rem;
      font-weight: 600;
      color: var(--color-text-muted);
    }

    input, textarea {
      width: 100%;
      background: rgba(255, 255, 255, 0.86);
      border: 1px solid rgba(20, 32, 51, 0.12);
      border-radius: 1rem;
      padding: 0.9rem 1rem;
      color: var(--color-text);
      font-family: var(--font-sans);
      outline: none;
      transition: border-color 0.2s ease, box-shadow 0.2s ease;
    }

    input:focus, textarea:focus {
      border-color: var(--color-primary);
      box-shadow: 0 0 0 4px rgba(255, 107, 87, 0.12);
    }

    textarea { resize: vertical; min-height: 10rem; }
    .row { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    @media (max-width: 640px) { .row { grid-template-columns: 1fr; } }

    .submit-btn {
      align-self: flex-start;
      border: none;
      border-radius: 999px;
      padding: 0.85rem 1.25rem;
      color: white;
      background: linear-gradient(135deg, var(--color-primary), #ff8d65);
      font-weight: 700;
      cursor: pointer;
    }

    .submit-btn:disabled {
      opacity: 0.7;
      cursor: not-allowed;
    }

    .alert {
      padding: 0.95rem 1rem;
      border-radius: 1rem;
      font-weight: 600;
    }

    .alert-success {
      color: var(--color-success);
      border: 1px solid rgba(20, 138, 98, 0.22);
      background: rgba(20, 138, 98, 0.08);
    }

    .alert-error {
      color: var(--color-error);
      border: 1px solid rgba(196, 76, 70, 0.22);
      background: rgba(196, 76, 70, 0.08);
    }
  `,

  state.status === "success"
    ? html.div({ class: "alert alert-success" }, "Message sent. We will reply soon.")
    : null,

  state.status === "error"
    ? html.div({ class: "alert alert-error" }, state.errorMsg || "Something went wrong. Please try again.")
    : null,

  state.status !== "success"
    ? html.form({
        class: "form",
        submit: async (e) => {
          e.preventDefault();
          if (state.status === "sending") return;

          const data = Object.fromEntries(new FormData(e.target));
          state.status = "sending";
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
            state.status = "error";
            state.errorMsg = err.message;
          }
        },
      },
      html.div({ class: "row" },
        html.div({ class: "field" },
          html.label({ for: "cf-name" }, "Name"),
          html.input({ id: "cf-name", type: "text", name: "name", required: true, autocomplete: "name", disabled: state.status === "sending" }),
        ),
        html.div({ class: "field" },
          html.label({ for: "cf-email" }, "Email"),
          html.input({ id: "cf-email", type: "email", name: "email", required: true, autocomplete: "email", disabled: state.status === "sending" }),
        ),
      ),
      html.div({ class: "field" },
        html.label({ for: "cf-subject" }, "Subject"),
        html.input({ id: "cf-subject", type: "text", name: "subject", disabled: state.status === "sending" }),
      ),
      html.div({ class: "field" },
        html.label({ for: "cf-message" }, "Message"),
        html.textarea({ id: "cf-message", name: "message", rows: 7, required: true, disabled: state.status === "sending" }),
      ),
      html.input({ type: "text", name: "_hp", style: "display:none", autocomplete: "off", tabindex: "-1" }),
      html.button({ type: "submit", class: "submit-btn", disabled: state.status === "sending" },
        state.status === "sending" ? "Sending..." : "Send message"
      ),
    )
    : null,
]);
