// dev-nav-menu: Responsive animated navigation menu
pfusch("dev-nav-menu", {
  items: "[]",
  currentPath: "",
  open: false,
}, (state) => {
  const items = typeof state.items === "string"
    ? JSON.parse(state.items || "[]")
    : (state.items || []);

  return [
    css`
      :host { display: block; }
      .nav-list { display: flex; gap: 0.25rem; list-style: none; margin: 0; padding: 0; }
      .nav-link {
        display: flex; align-items: center; gap: 0.4rem;
        padding: 0.45rem 0.8rem; border-radius: 8px; font-size: 0.9rem; font-weight: 500;
        text-decoration: none; color: var(--color-text-muted); transition: all 0.15s;
      }
      .nav-link:hover { color: var(--color-text); background: rgba(255,255,255,0.06); }
      .nav-link.active { color: var(--color-primary); background: rgba(14,165,233,0.1); }
      @media (max-width: 768px) {
        .nav-list { flex-direction: column; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 12px; padding: 0.5rem; position: absolute; top: calc(var(--header-height) + 0.5rem); right: 1rem; min-width: 180px; display: none; z-index: 100; }
        .nav-list.open { display: flex; }
      }
    `,
    html.ul({ class: `nav-list ${state.open ? "open" : ""}` },
      ...items.map(item =>
        html.li({},
          html.a({
            href: item.frontmatter?.url || item.url_path || "#",
            class: `nav-link ${state.currentPath === (item.frontmatter?.url || item.url_path) ? "active" : ""}`,
            ...(item.frontmatter?.external ? { target: "_blank", rel: "noopener noreferrer" } : {}),
          },
          item.frontmatter?.icon ? html.span({ "aria-hidden": "true" }, item.frontmatter.icon) : null,
          item.frontmatter?.title || "",
          )
        )
      )
    )
  ];
});

// dev-dock: macOS-style icon dock with magnification effect
pfusch("dev-dock", {
  items: "[]",
  hovered: -1,
}, (state) => {
  const items = typeof state.items === "string"
    ? JSON.parse(state.items || "[]")
    : (state.items || []);

  return [
    css`
      :host { display: block; }
      .dock {
        display: flex; align-items: flex-end; gap: 0.5rem;
        padding: 0.6rem 1rem; background: rgba(30,41,59,0.8);
        backdrop-filter: blur(20px); border: 1px solid var(--color-border);
        border-radius: 20px; width: fit-content; margin: 0 auto;
      }
      .dock-item {
        display: flex; flex-direction: column; align-items: center; gap: 0.25rem;
        text-decoration: none; cursor: pointer; transition: transform 0.15s;
        position: relative;
      }
      .dock-icon {
        width: 48px; height: 48px; background: var(--color-surface);
        border-radius: 12px; display: flex; align-items: center; justify-content: center;
        font-size: 1.6rem; border: 1px solid var(--color-border);
        transition: transform 0.15s, box-shadow 0.15s;
      }
      .dock-item:hover .dock-icon { transform: translateY(-8px) scale(1.2); box-shadow: 0 8px 24px rgba(0,0,0,0.4); }
      .dock-label {
        font-size: 0.65rem; color: var(--color-text-muted); opacity: 0; transition: opacity 0.15s;
        position: absolute; bottom: 100%; white-space: nowrap;
        background: var(--color-surface); padding: 0.2rem 0.5rem; border-radius: 4px;
        border: 1px solid var(--color-border);
      }
      .dock-item:hover .dock-label { opacity: 1; }
      .dock-divider { width: 1px; height: 40px; background: var(--color-border); margin: 0 0.25rem; align-self: center; }
    `,
    html.nav({ class: "dock", "aria-label": "Dock" },
      ...items.map((item, i) => [
        item.frontmatter?.icon === "|"
          ? html.div({ class: "dock-divider", "aria-hidden": "true" })
          : html.a({
              href: item.frontmatter?.url || item.url_path || "#",
              class: "dock-item",
              title: item.frontmatter?.title,
              ...(item.frontmatter?.external ? { target: "_blank", rel: "noopener noreferrer" } : {}),
            },
            html.div({ class: "dock-icon" }, item.frontmatter?.icon || "📄"),
            html.span({ class: "dock-label" }, item.frontmatter?.title || ""),
          )
      ]).flat()
    )
  ];
});

// dev-social-icon: Inline SVG social platform icon
pfusch("dev-social-icon", {
  platform: "github",
}, (state) => {
  const icons = {
    github:   `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0 0 24 12c0-6.63-5.37-12-12-12z"/></svg>`,
    linkedin: `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433a2.062 2.062 0 0 1-2.063-2.065 2.064 2.064 0 1 1 2.063 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z"/></svg>`,
    twitter:  `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-4.714-6.231-5.401 6.231H2.744l7.73-8.835L1.254 2.25H8.08l4.259 5.63 5.904-5.63zm-1.161 17.52h1.833L7.084 4.126H5.117z"/></svg>`,
    mastodon: `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M23.268 5.313c-.35-2.578-2.617-4.61-5.304-5.004C17.51.242 15.792 0 11.813 0h-.03c-3.98 0-4.835.242-5.288.309C3.882.692 1.496 2.518.917 5.127.64 6.412.61 7.837.661 9.143c.074 1.874.088 3.745.26 5.611.118 1.24.325 2.47.62 3.68.55 2.237 2.777 4.098 4.96 4.857 2.336.792 4.849.923 7.256.38.265-.061.527-.132.786-.213.585-.184 1.27-.39 1.774-.753a.057.057 0 0 0 .023-.043v-1.809a.052.052 0 0 0-.02-.041.053.053 0 0 0-.046-.01 20.282 20.282 0 0 1-4.709.545c-2.73 0-3.463-1.284-3.674-1.818a5.593 5.593 0 0 1-.319-1.433.053.053 0 0 1 .066-.054c1.517.363 3.072.546 4.632.546.376 0 .75 0 1.125-.01 1.57-.044 3.224-.124 4.768-.422.038-.008.077-.015.11-.024 2.435-.464 4.753-1.92 4.989-5.604.008-.145.03-1.52.03-1.67.002-.512.167-3.63-.024-5.545zm-3.748 9.195h-2.561V8.29c0-1.309-.55-1.976-1.67-1.976-1.23 0-1.846.79-1.846 2.35v3.403h-2.546V8.663c0-1.56-.617-2.35-1.848-2.35-1.112 0-1.668.668-1.67 1.977v6.218H4.822V8.102c0-1.31.337-2.35 1.011-3.12.696-.77 1.608-1.164 2.74-1.164 1.311 0 2.302.5 2.962 1.498l.638 1.06.638-1.06c.66-.999 1.65-1.498 2.96-1.498 1.13 0 2.043.395 2.74 1.164.675.77 1.012 1.81 1.012 3.12z"/></svg>`,
    rss:      `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M6.18 15.64a2.18 2.18 0 0 1 2.18 2.18C8.36 19.01 7.38 20 6.18 20C4.98 20 4 19.01 4 17.82a2.18 2.18 0 0 1 2.18-2.18M4 4.44A15.56 15.56 0 0 1 19.56 20h-2.83A12.73 12.73 0 0 0 4 7.27V4.44m0 5.66a9.9 9.9 0 0 1 9.9 9.9h-2.83A7.07 7.07 0 0 0 4 12.93V10.1z"/></svg>`,
  };

  const iconSvg = icons[state.platform] || `<svg viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="12" r="10"/></svg>`;

  return [
    css`
      :host { display: inline-flex; align-items: center; justify-content: center; width: 1.2em; height: 1.2em; }
      svg { width: 100%; height: 100%; }
    `,
    html.raw`${iconSvg}`,
  ];
});

// dev-timeline-entry: A single career timeline item
pfusch("dev-timeline-entry", {
  role: "",
  company: "",
  location: "",
  start: "",
  end: "Present",
  body: "",
}, (state) => [
  css`
    :host { display: block; }
    .entry {
      display: grid; grid-template-columns: 1fr auto;
      gap: 0.5rem 1.5rem; padding: 1.5rem;
      background: var(--color-surface); border: 1px solid var(--color-border);
      border-radius: 12px; position: relative;
    }
    .entry::before {
      content: ''; position: absolute; left: -1px; top: 1.8rem; bottom: -1px;
      width: 2px; background: linear-gradient(to bottom, var(--color-primary), transparent);
    }
    .role { font-size: 1rem; font-weight: 600; color: var(--color-text); margin: 0; }
    .company { font-size: 0.875rem; color: var(--color-primary); font-weight: 500; }
    .location { font-size: 0.8rem; color: var(--color-text-muted); }
    .period { font-size: 0.8rem; color: var(--color-text-muted); white-space: nowrap; text-align: right; font-family: var(--font-mono); }
    .body { grid-column: 1 / -1; font-size: 0.9rem; color: var(--color-text-muted); line-height: 1.6; }
    .body p { margin: 0 0 0.5rem; }
  `,
  html.div({ class: "entry" },
    html.div({},
      html.h3({ class: "role" }, state.role),
      state.company ? html.div({ class: "company" }, state.company) : null,
      state.location ? html.div({ class: "location" }, `📍 ${state.location}`) : null,
    ),
    html.div({ class: "period" },
      state.start || state.end
        ? `${state.start} — ${state.end}`
        : null
    ),
    state.body ? html.div({ class: "body" }, html.raw`${state.body}`) : null,
  )
]);

// dev-toc: Floating auto-generated table of contents
pfusch("dev-toc", {
  contentSelector: ".post-body",
  open: false,
}, (state) => {
  return [
    css`
      :host { display: block; }
      .toc-toggle {
        display: flex; align-items: center; gap: 0.4rem;
        font-size: 0.8rem; font-weight: 500; color: var(--color-text-muted);
        background: var(--color-surface); border: 1px solid var(--color-border);
        padding: 0.4rem 0.8rem; border-radius: 8px; cursor: pointer;
        margin-bottom: 1rem; transition: color 0.15s;
      }
      .toc-toggle:hover { color: var(--color-primary); }
      .toc-list { list-style: none; margin: 0; padding: 0; display: ${state.open ? 'block' : 'none'}; }
      .toc-item a {
        font-size: 0.82rem; color: var(--color-text-muted); text-decoration: none;
        display: block; padding: 0.2rem 0; transition: color 0.15s;
      }
      .toc-item a:hover { color: var(--color-primary); }
      .toc-item.h3 { padding-left: 1rem; }
      .toc-item.h4 { padding-left: 2rem; }
    `,
    html.button({
      class: "toc-toggle",
      click: () => state.open = !state.open,
    }, state.open ? "▾" : "▸", " Table of Contents"),
    html.ul({ class: "toc-list", id: "toc-list" }),
  ];
});

// dev-share-buttons: Social sharing buttons
pfusch("dev-share-buttons", {
  url: "",
  title: "",
}, (state) => [
  css`
    :host { display: flex; gap: 0.5rem; flex-wrap: wrap; }
    .share-btn {
      display: inline-flex; align-items: center; gap: 0.4rem;
      font-size: 0.8rem; font-weight: 500; padding: 0.35rem 0.8rem;
      border-radius: 6px; text-decoration: none; transition: opacity 0.15s;
    }
    .share-btn:hover { opacity: 0.8; }
    .share-twitter { background: #000; color: #fff; }
    .share-linkedin { background: #0077b5; color: #fff; }
    .share-copy { background: var(--color-surface); color: var(--color-text); border: 1px solid var(--color-border); cursor: pointer; }
  `,
  html.a({
    href: `https://twitter.com/intent/tweet?url=${encodeURIComponent(state.url)}&text=${encodeURIComponent(state.title)}`,
    class: "share-btn share-twitter", target: "_blank", rel: "noopener noreferrer"
  }, "Share on X"),
  html.a({
    href: `https://www.linkedin.com/shareArticle?url=${encodeURIComponent(state.url)}&title=${encodeURIComponent(state.title)}`,
    class: "share-btn share-linkedin", target: "_blank", rel: "noopener noreferrer"
  }, "Share on LinkedIn"),
  html.button({
    class: "share-btn share-copy",
    click: () => { navigator.clipboard?.writeText(state.url); }
  }, "Copy link"),
]);
