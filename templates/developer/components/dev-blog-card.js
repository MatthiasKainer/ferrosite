// @ssr
// dev-blog-card: A single blog post preview card
pfusch("dev-blog-card", {
  title: "",
  excerpt: "",
  url: "",
  date: "",
  readingTime: 0,
  tags: "[]",
  cover: "",
}, (state) => {
  const tags = typeof state.tags === "string"
    ? JSON.parse(state.tags || "[]")
    : (state.tags || []);

  const fmtDate = (d) => {
    if (!d) return "";
    try { return new Date(d).toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" }); }
    catch { return d; }
  };

  return [
    css`
      :host { display: block; }
      .card {
        background: var(--color-surface); border: 1px solid var(--color-border);
        border-radius: 12px; overflow: hidden; display: flex; flex-direction: column;
        text-decoration: none; color: inherit; height: 100%;
        transition: border-color 0.2s, transform 0.2s, box-shadow 0.2s;
      }
      .card:hover { border-color: var(--color-primary); transform: translateY(-2px); box-shadow: 0 8px 24px rgba(0,0,0,0.3); }
      .card-cover { aspect-ratio: 16/9; overflow: hidden; }
      .card-cover img { width: 100%; height: 100%; object-fit: cover; }
      .card-cover-placeholder { width: 100%; height: 100%; background: linear-gradient(135deg, var(--color-primary) 0%, var(--color-accent) 100%); }
      .card-body { padding: 1.5rem; flex: 1; display: flex; flex-direction: column; gap: 0.5rem; }
      .card-meta { display: flex; align-items: center; gap: 0.5rem; font-size: 0.8rem; color: var(--color-text-muted); }
      .meta-sep { opacity: 0.5; }
      .card-title { font-size: 1.05rem; font-weight: 600; color: var(--color-text); margin: 0; line-height: 1.4; }
      .card-excerpt { font-size: 0.875rem; color: var(--color-text-muted); line-height: 1.6; flex: 1; margin: 0; }
      .card-tags { display: flex; flex-wrap: wrap; gap: 0.3rem; margin-top: auto; padding-top: 0.5rem; }
      .tag {
        font-size: 0.72rem; font-weight: 500; padding: 0.15rem 0.5rem; border-radius: 4px;
        background: rgba(139,92,246,0.12); color: var(--color-accent); border: 1px solid rgba(139,92,246,0.2);
      }
    `,
    html.a({ href: state.url, class: "card" },
      html.div({ class: "card-cover" },
        state.cover
          ? html.img({ src: state.cover, alt: state.title, loading: "lazy" })
          : html.div({ class: "card-cover-placeholder" })
      ),
      html.div({ class: "card-body" },
        html.div({ class: "card-meta" },
          state.date ? html.time({ datetime: state.date }, fmtDate(state.date)) : null,
          state.date && state.readingTime ? html.span({ class: "meta-sep" }, "·") : null,
          state.readingTime ? html.span({}, `${state.readingTime} min read`) : null,
        ),
        html.h2({ class: "card-title" }, state.title),
        html.p({ class: "card-excerpt" }, state.excerpt),
        tags.length ? html.div({ class: "card-tags" },
          ...tags.map(t => html.span({ class: "tag" }, `#${t}`))
        ) : null,
      )
    )
  ];
});

// dev-blog-filter: A filterable / searchable blog listing
pfusch("dev-blog-filter", {
  posts: "[]",
  search: "",
  activeTag: "",
}, (state) => {
  const posts = typeof state.posts === "string"
    ? JSON.parse(state.posts || "[]")
    : (state.posts || []);

  const allTags = [...new Set(posts.flatMap(p => p.frontmatter?.tags || []))].sort();

  const filtered = posts.filter(p => {
    const matchesSearch = !state.search ||
      p.frontmatter?.title?.toLowerCase().includes(state.search.toLowerCase()) ||
      p.excerpt?.toLowerCase().includes(state.search.toLowerCase());
    const matchesTag = !state.activeTag ||
      (p.frontmatter?.tags || []).includes(state.activeTag);
    return matchesSearch && matchesTag;
  });

  // Sort by date descending
  filtered.sort((a, b) => {
    const da = new Date(a.frontmatter?.date || 0);
    const db = new Date(b.frontmatter?.date || 0);
    return db - da;
  });

  return [
    css`
      :host { display: block; }
      .toolbar { display: flex; flex-wrap: wrap; gap: 1rem; margin-bottom: 2rem; align-items: center; }
      .search-box {
        flex: 1; min-width: 200px; padding: 0.6rem 1rem;
        background: var(--color-surface); border: 1px solid var(--color-border);
        border-radius: 8px; color: var(--color-text); font-size: 0.9rem;
        font-family: var(--font-sans); outline: none;
        transition: border-color 0.2s;
      }
      .search-box:focus { border-color: var(--color-primary); }
      .tag-filters { display: flex; flex-wrap: wrap; gap: 0.4rem; }
      .tag-btn {
        font-size: 0.8rem; font-weight: 500; padding: 0.3rem 0.7rem;
        border-radius: 999px; border: 1px solid var(--color-border);
        background: transparent; color: var(--color-text-muted); cursor: pointer; transition: all 0.15s;
      }
      .tag-btn:hover, .tag-btn.active { background: var(--color-accent); color: #fff; border-color: var(--color-accent); }
      .posts-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 1.5rem; }
      .count { font-size: 0.85rem; color: var(--color-text-muted); margin-bottom: 1rem; }
      .empty { text-align: center; padding: 4rem 1rem; color: var(--color-text-muted); }
    `,
    html.div({ class: "toolbar" },
      html.input({
        class: "search-box",
        type: "search",
        placeholder: "Search posts…",
        value: state.search,
        input: (e) => state.search = e.target.value,
      }),
    ),
    allTags.length ? html.div({ class: "tag-filters" },
      html.button({
        class: `tag-btn ${!state.activeTag ? "active" : ""}`,
        click: () => state.activeTag = ""
      }, "All"),
      ...allTags.map(t => html.button({
        class: `tag-btn ${state.activeTag === t ? "active" : ""}`,
        click: () => state.activeTag = t
      }, `#${t}`))
    ) : null,
    html.p({ class: "count" }, `${filtered.length} post${filtered.length !== 1 ? "s" : ""}`),
    filtered.length ? html.div({ class: "posts-grid" },
      ...filtered.map(p =>
        html["dev-blog-card"]({
          title: p.frontmatter?.title || "",
          excerpt: p.excerpt || "",
          url: p.url_path || "#",
          date: p.frontmatter?.date || "",
          "reading-time": p.reading_time || 0,
          tags: JSON.stringify(p.frontmatter?.tags || []),
          cover: p.frontmatter?.cover_image || "",
        })
      )
    ) : html.div({ class: "empty" }, "No posts match your search."),
  ];
});
