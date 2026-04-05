# ferrosite 🔩

> A static site generator in Rust - powered by atomic design, slots and web components.

## Philosophy

**Keep hosting simple** - Create a static website for everything that can be static. Dynamic content only via plugins.

**Design Slots** - content is not pasted into templates. Every markdown article declares a `slot` in its frontmatter. The build system routes it to the correct position on the correct page. Forty slot types cover atoms, molecules, organisms, and layout regions.

**Progressive Enhancement** - pages render as complete HTML. [pfusch](https://github.com/MatthiasKainer/pfusch) web components add interactivity without replacing static content. An optional Puppeteer SSR pass pre-renders shadow DOM for zero-JS environments.

**Exception-free library** - every operation is a `Result<T, SiteError>`. Pure functions transform data; side effects happen only at pipeline edges. Nothing panics in library code.

---

## Quick Start

```bash
# Install
cargo install ferrosite

# Create a new site from the developer template
ferrosite new my-site --template developer
# OR Create a new site from the company template
ferrosite new my-site --template company
# OR Create a new site from a GitHub template repository
ferrosite new my-site --template https://github.com/user/ferrosite-template.git

# Skip the interactive questions and use defaults
ferrosite new my-site --template developer --yolo

cd my-site

# Create content interactively
ferrosite add article
ferrosite add page

# Build
ferrosite build

# Preview locally (reload on changes), including plugin worker routes
ferrosite run

# Ship (build + deploy)
ferrosite ship
```

---

## Project Structure

```
my-site/
├── ferrosite.toml          # Site configuration
├── content/                # Your markdown articles
│   ├── home.md             # Hero section content
│   ├── about.md            # About page body
│   ├── blog/               # Blog posts (slot: article-body)
│   ├── projects/           # Project details (slot: project-body)
│   └── skills/             # Skill groups (slot: skill-group)
├── assets/                 # Static assets (images, fonts, extra CSS)
├── plugins/                # Dynamic plugins (lambdas + web components)
│   └── contact-form/
│       ├── manifest.toml
│       ├── component.js    # pfusch component
│       └── worker.js       # Cloudflare Worker / Lambda handler
├── templates/              # Override bundled templates (optional)
│   └── developer/
│       ├── theme.toml
│       ├── layouts/        # Jinja2 HTML layouts
│       └── components/     # pfusch .js component files
└── dist/                   # Build output (gitignored)
```

Local markdown images are collected during the build, rewritten to `/static/media/...`, and emitted as optimized files in `dist/static/media/`.

Bundled templates currently include `developer` and `company`.

- To build your own template, see [`templates/HOWTO-CREATE-A-TEMPLATE.md`](templates/HOWTO-CREATE-A-TEMPLATE.md).
- To edit, build, and operate a site built from a template, see [`HOWTO-BUILD-EDIT-OPERATE-YOUR-SITE.md`](HOWTO-BUILD-EDIT-OPERATE-YOUR-SITE.md).

---

## The Slot System

Every markdown article has a `slot` field in its YAML frontmatter:

```markdown
---
title: "Why I Rewrote Our Build Tool in Rust"
slot: "article-body"          # <-- this routes the article
date: "2024-03-15"
tags: ["rust", "tooling"]
page_scope: "*"               # which pages include this ("*", "home", "blog", ...)
order: 0                      # sort position within the slot
weight: 80                    # prominence (higher = more featured)
---

Your markdown content here...
```

### Slot Tiers (Atomic Design)

| Tier | Examples |
|------|---------|
| **Atom** | `text-block`, `image`, `badge`, `link-button`, `code-snippet`, `stat-number` |
| **Molecule** | `article-card`, `project-card`, `skill-group`, `social-link`, `timeline-entry`, `nav-item`, `dock-item` |
| **Organism** | `hero`, `blog-feed`, `project-grid`, `skills-matrix`, `career-timeline`, `contact-form` |
| **Region** | `header-brand`, `footer-about`, `footer-nav-column`, `sidebar-widget` |

---

## Commands

```bash
ferrosite new <name>         # Scaffold a new site (interactive by default)
ferrosite add article        # Interactive blog post scaffold in content/blog/
ferrosite add project        # Interactive project scaffold in content/projects/
ferrosite add page           # Slot-based page content scaffold + optional nav item
ferrosite add nav            # Standalone nav-item content scaffold
ferrosite edit <selector>    # Update frontmatter by path, slug, filename, or title
ferrosite assign-slot <selector> <slot>
                             # Re-route content to a different slot/page_scope
ferrosite reorder            # Interactively reorder entries in a slot (defaults to nav-item)
ferrosite build              # Build the site into dist/
ferrosite build --ssr        # Build + Puppeteer SSR pass
ferrosite run                # build + serve locally with plugin runners
ferrosite check              # Validate config and content
ferrosite slots              # List all article slot assignments
ferrosite ssr-setup          # Scaffold ssr/ and install Puppeteer deps
ferrosite setup-ssr          # Alias for ssr-setup because I can't remember my own cmds
ferrosite config             # Print resolved configuration
ferrosite deploy             # Deploy dist/ to configured provider
ferrosite ship               # build + deploy
```

### Authoring workflow

The authoring commands create or update markdown files with the frontmatter
Ferrosite already understands, so they slot straight into the existing build
pipeline.

```bash
# Interactive blog post
ferrosite add article

# Scriptable article creation
ferrosite add article --title "Launch notes" --tags launch,product --yolo

# Create about-page content and a matching nav item
ferrosite add page --title "About" --page-scope about --slot about-body

# Create a standalone nav item
ferrosite add nav --title "Services" --url /services/

# Update frontmatter for an existing file
ferrosite edit rewriting-build-tool-rust --title "Why We Rebuilt Our Tooling"

# Reassign an existing content file to a new slot
ferrosite assign-slot content/home.md hero --page-scope home

# Interactively reorder navigation entries and save fresh order values
ferrosite reorder --slot nav-item
```

`ferrosite edit` accepts a site-local path like `content/about.md`, a filename
stem like `nav-about`, or a unique slug/title match. Use `--open` with
`ferrosite add ...` or `ferrosite edit ...` to jump straight into your editor.

`ferrosite add page` scaffolds content that participates in an existing routed
page or slot. It does not, by itself, create a brand new template/layout route;
for that you still need to update the template layouts.

`ferrosite reorder` focuses on one slot at a time, shows the current order, and
accepts `u <n>`, `d <n>`, or `m <from> <to>` before writing normalized `order`
values back into the matching markdown files.

---

## SSR (Server-Side Rendering)

Run the setup command to scaffold `ssr/`, install Puppeteer, and enable SSR in `ferrosite.toml`:

```bash
ferrosite ssr-setup
```

That writes:

```toml
[build.ssr]
enabled = true
node_bin = "node"
package_manager_bin = "npm"
timeout_ms = 30000
```

If you prefer another package manager, set `package_manager_bin = "pnpm"` or run `ferrosite ssr-setup --package-manager-bin yarn`.

During `ferrosite build --ssr`, each generated HTML page is served locally and rendered by Puppeteer using `getHTML({ includeShadowRoots: true, serializableShadowRoots: true })`. The rendered shadow DOM is written back to the output HTML, giving you fully pre-rendered components with zero-JS content.

---

## Plugins

Plugins bring their own pfusch web component *and* a lambda worker. The worker implements a CQRS interface:

- `POST /api/plugin-route` with `{ command, payload }` - mutates state
- `GET  /api/plugin-route?query=QueryName&...` - reads state

The build system generates a CQRS wrapper around your worker code automatically.
For local end-to-end testing, `ferrosite run` serves the static site and dispatches plugin routes through a local worker runner.

```toml
# plugins/contact-form/manifest.toml
[plugin]
name = "contact-form"
slots = ["contact-form"]
component_file = "component.js"
worker_file = "worker.js"
worker_route = "/api/contact"
worker_runtime = "cloudflare-worker"
required_env = ["RESEND_API_KEY", "TO_EMAIL"]
```

Manage plugins from the CLI:

```bash
# Install a bundled plugin shipped with ferrosite
ferrosite plugin add contact-form

# Install a plugin from GitHub via git clone
ferrosite plugin add https://github.com/user/repo.git

# Remove a plugin, then inspect the printed file list for remaining references
ferrosite plugin remove contact-form
```

`plugin remove` also has the alias `plugin uninstall`, and `plugin add` has the alias `plugin install`.

---

## Deploy

### Cloudflare Pages (recommended - free tier)

```toml
[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "my-site"
account_id   = "YOUR_CF_ACCOUNT_ID"
```

```bash
npm install -g wrangler
wrangler login
ferrosite ship
```

Static files → Cloudflare Pages (free).  
Plugin workers → Cloudflare Workers (free tier: 100k requests/day).

### AWS S3 + CloudFront

```toml
[deploy]
provider = "aws"

[deploy.aws]
bucket_name = "my-site-bucket"
region      = "eu-central-1"
cloudfront_distribution_id = "EXAMPLEID"
```

### Azure Static Web Apps

```toml
[deploy]
provider = "azure"

[deploy.azure]
resource_group = "my-rg"
app_name       = "my-site"
```

---

## Architecture (Rust)

```
src/
├── error.rs            # SiteError + SiteResult<T> + collect_results()
├── config/             # TOML config loading and theme tokens
├── content/
│   ├── frontmatter.rs  # YAML frontmatter parsing (pure)
│   ├── slot.rs         # SlotType enum (40 types, atomic design tiers)
│   ├── article.rs      # Article model + markdown rendering (pure)
│   └── page.rs         # Page assembly + SlotMap (pure)
├── template/
│   ├── engine.rs       # minijinja template engine wrapper
│   └── component.rs    # pfusch component registry + HTML generation
├── plugin/
│   └── mod.rs          # Plugin manifests, CQRS worker generation
├── pipeline/
│   └── build.rs        # Full ROP pipeline: collect→slot→assemble→render→write
├── deploy/
│   └── mod.rs          # Cloudflare / AWS / Azure deployers
└── main.rs             # CLI (clap)
```

### Railway-Oriented Pipeline

```rust
pub fn build_site(site_root: &Path) -> SiteResult<BuildReport> {
    BuildContext::load(site_root)              // IO edge
        .and_then(|ctx| {
            collect_articles(&ctx)            // IO edge
                .and_then(|arts| build_global_slot_map(&arts))  // pure
                .and_then(|slots| assemble_pages(&arts, &slots, &ctx.config))  // pure
                .and_then(|pages| render_pages(&pages, &ctx))   // pure-ish
                .and_then(|rendered| write_output(&rendered, &output_dir))  // IO edge
        })
}
```

Every function returns `SiteResult<T>`. Errors propagate automatically via `?`. No `unwrap()` in library code.

---

## License

MIT
