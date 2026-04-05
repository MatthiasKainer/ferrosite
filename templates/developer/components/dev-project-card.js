// @ssr
// dev-project-card: A single portfolio project card
pfusch("dev-project-card", {
  title: "",
  excerpt: "",
  url: "",
  repo: "",
  live: "",
  status: "active",
  stack: "[]",
}, (state) => {
  const techStack = typeof state.stack === "string"
    ? JSON.parse(state.stack || "[]")
    : (state.stack || []);

  const statusColors = {
    active:   { bg: "var(--color-success)", text: "#fff" },
    wip:      { bg: "var(--color-warning)", text: "#000" },
    archived: { bg: "var(--color-text-muted)", text: "#fff" },
  };
  const sc = statusColors[state.status] || statusColors.active;

  return [
    css`
      :host { display: block; }
      .card {
        background: var(--color-surface);
        border: 1px solid var(--color-border);
        border-radius: 12px;
        padding: 1.5rem;
        height: 100%;
        display: flex;
        flex-direction: column;
        gap: 0.75rem;
        transition: border-color 0.2s, transform 0.2s, box-shadow 0.2s;
        text-decoration: none;
        color: inherit;
      }
      .card:hover {
        border-color: var(--color-primary);
        transform: translateY(-2px);
        box-shadow: 0 8px 24px rgba(0,0,0,0.3);
      }
      .card-top { display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem; }
      .card-title { font-size: 1.1rem; font-weight: 600; color: var(--color-text); margin: 0; }
      .status-badge {
        font-size: 0.7rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;
        padding: 0.2rem 0.5rem; border-radius: 999px; white-space: nowrap; flex-shrink: 0;
      }
      .excerpt { font-size: 0.9rem; color: var(--color-text-muted); line-height: 1.6; flex: 1; margin: 0; }
      .stack { display: flex; flex-wrap: wrap; gap: 0.35rem; margin-top: auto; }
      .tech-tag {
        font-family: var(--font-mono); font-size: 0.75rem;
        background: rgba(14,165,233,0.1); color: var(--color-primary);
        border: 1px solid rgba(14,165,233,0.2); padding: 0.15rem 0.5rem; border-radius: 4px;
      }
      .card-links { display: flex; gap: 0.5rem; margin-top: 0.5rem; }
      .card-link {
        font-size: 0.8rem; font-weight: 500; color: var(--color-text-muted);
        text-decoration: none; display: flex; align-items: center; gap: 0.25rem;
        padding: 0.25rem 0.6rem; border: 1px solid var(--color-border); border-radius: 6px;
        transition: color 0.15s, border-color 0.15s;
      }
      .card-link:hover { color: var(--color-primary); border-color: var(--color-primary); }
    `,
    html.a({ href: state.url, class: "card" },
      html.div({ class: "card-top" },
        html.h3({ class: "card-title" }, state.title),
        state.status ? html.span({
          class: "status-badge",
          style: `background:${sc.bg};color:${sc.text}`
        }, state.status) : null,
      ),
      html.p({ class: "excerpt" }, state.excerpt),
      techStack.length ? html.div({ class: "stack" },
        ...techStack.map(t => html.span({ class: "tech-tag" }, t))
      ) : null,
      (state.repo || state.live) ? html.div({ class: "card-links" },
        state.repo ? html.a({ href: state.repo, class: "card-link", target: "_blank", rel: "noopener noreferrer",
          click: (e) => e.stopPropagation() }, "⌥ Source") : null,
        state.live ? html.a({ href: state.live, class: "card-link", target: "_blank", rel: "noopener noreferrer",
          click: (e) => e.stopPropagation() }, "↗ Live") : null,
      ) : null,
    )
  ];
});

// dev-project-grid: Filterable grid of projects
pfusch("dev-project-grid", {
  projects: "[]",
  filter: "all",
}, (state) => {
  const projects = typeof state.projects === "string"
    ? JSON.parse(state.projects || "[]")
    : (state.projects || []);

  const allStacks = [...new Set(
    projects.flatMap(p => p.frontmatter?.tech_stack || [])
  )].sort();

  const filtered = state.filter === "all"
    ? projects
    : projects.filter(p => (p.frontmatter?.tech_stack || []).includes(state.filter));

  return [
    css`
      :host { display: block; }
      .filter-bar { display: flex; flex-wrap: wrap; gap: 0.5rem; margin-bottom: 2rem; }
      .filter-btn {
        font-size: 0.85rem; font-weight: 500; padding: 0.4rem 0.9rem;
        border-radius: 999px; border: 1px solid var(--color-border);
        background: transparent; color: var(--color-text-muted); cursor: pointer;
        transition: all 0.15s;
      }
      .filter-btn:hover, .filter-btn.active {
        background: var(--color-primary); color: #fff; border-color: var(--color-primary);
      }
      .grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
        gap: 1.5rem;
      }
    `,
    allStacks.length > 1 ? html.div({ class: "filter-bar" },
      html.button({
        class: `filter-btn ${state.filter === "all" ? "active" : ""}`,
        click: () => state.filter = "all"
      }, `All (${projects.length})`),
      ...allStacks.map(s =>
        html.button({
          class: `filter-btn ${state.filter === s ? "active" : ""}`,
          click: () => state.filter = s
        }, s)
      )
    ) : null,
    html.div({ class: "grid" },
      ...filtered.map(p =>
        html["dev-project-card"]({
          title: p.frontmatter?.title || "",
          excerpt: p.excerpt || "",
          url: p.url_path || "#",
          repo: p.frontmatter?.repo_url || "",
          live: p.frontmatter?.live_url || "",
          status: p.frontmatter?.status || "active",
          stack: JSON.stringify(p.frontmatter?.tech_stack || []),
        })
      )
    )
  ];
});
