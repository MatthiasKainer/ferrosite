# How To Create A Template

This guide documents the actual template contract used by ferrosite today. It
is based on reading the scaffolding path, the build pipeline, and the bundled
`developer` and `company` templates.

If you want the day-to-day site editing workflow after a template has been
scaffolded, see
[`HOWTO-BUILD-EDIT-OPERATE-YOUR-SITE.md`](HOWTO-BUILD-EDIT-OPERATE-YOUR-SITE.md).

## 1. Template directory contract

Create a folder under `templates/<template-name>/`.

Supported files and folders:

```text
templates/<template-name>/
├── assets/           # copied to dist/assets/
├── components/       # optional pfusch .js components
├── content/          # bundled starter markdown content
├── layouts/          # required minijinja html layouts
├── theme.toml        # optional theme token file
└── ferrosite.toml    # optional starter site config used by `ferrosite new`
```

Only `layouts/` is required for rendering. Everything else is optional, but a
template intended for `ferrosite new` should usually include at least
`assets/`, `content/`, `theme.toml`, and `ferrosite.toml`.

## 2. Required layout filenames

Ferrosite resolves layout names from `PageType::layout_name()`. In practice a
general-purpose template should provide these files:

```text
layouts/base.html
layouts/home.html
layouts/about.html
layouts/blog.html
layouts/contact.html
layouts/projects.html
layouts/post.html
```

Important detail:

- Project detail pages also render through `post.html`.
- If a page-specific layout is missing, ferrosite falls back to `base.html`.
- If `layouts/` does not exist, the build fails.

## 3. What `ferrosite new` does with a template

When a user runs:

```bash
ferrosite new my-site --template company
```

ferrosite copies the entire template directory into the new site root, then
rewrites `ferrosite.toml` with the answers collected from the CLI.

More precisely:

- `content/`, `assets/`, `plugins/`, and `ferrosite.toml` are copied into the
  new site root
- `layouts/`, `components/`, and `theme.toml` are materialized into
  `templates/<template-name>/` so they act as live site-local template
  overrides

What gets overwritten intentionally:

- `site.title`
- `site.description`
- `site.base_url`
- `site.language`
- `site.author.name`
- `site.author.bio`
- `site.author.avatar`
- `site.social.github`
- `site.social.linkedin`
- `build.template`
- provider-specific deploy project naming fields

What is preserved from the template config:

- plugin enablement
- provider choice
- extra site metadata not touched by the scaffold writer
- template-specific defaults you add outside the rewritten fields

That means `ferrosite.toml` inside a template should contain the shape and
defaults you want, but you should expect the scaffold step to replace the main
identity fields.

## 4. How content is loaded

Ferrosite loads content from two places:

1. the site's `content/`
2. the template's `content/`

If both exist, site content wins when the relative path matches.

This is useful for starter content:

- ship realistic placeholder pages in the template
- let users replace them file by file without touching layouts

## 5. Slot-driven rendering rules

Templates do not query the filesystem directly. They render whatever lands in
the page `slots` map.

The key build rules are:

- `home` page receives articles with `page_scope = "home"` plus global slots
- `about` page receives articles with `page_scope = "about"` plus global slots
- `blog` listing is assembled automatically from blog posts
- `projects` listing is assembled automatically from project pages
- `contact` currently receives only global slots

Global slots are:

- any article with `page_scope = "*"`
- layout-region slots such as `header-brand`, `nav-item`, `footer-about`, and
  `footer-nav-column`

## 6. Important current limitation: contact page slots

Today, `assemble_contact_page()` clones only the global slot map. It does not
pull in `page_scope = "contact"` articles.

Implication:

- if your contact layout needs a `contact-form` slot, mark that content as
  `page_scope = "*"` for now
- or change the pipeline if you want true contact-scoped content

This is the main non-obvious behavior discovered while building the `company`
template.

## 7. Blog and project listings are mostly automatic

You do not need to create explicit `article-card` or `project-card` markdown
for standard listings.

Ferrosite already does this:

- blog listing collects `article-body` articles and files under `content/blog/`
- projects listing collects `project-body` articles and files under
  `content/projects/`
- detail pages are generated automatically from those same source files

Practical recommendation:

- create posts under `content/blog/*.md` with `slot = "article-body"`
- create case studies under `content/projects/*.md` with `slot = "project-body"`
- let the listing layouts render those assembled cards

## 8. Theme tokens available in CSS

`theme.toml` is converted into CSS custom properties and injected into every
page before your stylesheet loads.

Available variables:

```css
--color-primary
--color-primary-dark
--color-accent
--color-bg
--color-surface
--color-text
--color-text-muted
--color-border
--color-code-bg
--color-success
--color-warning
--color-error
--font-sans
--font-mono
--font-heading
--font-size-base
--font-size-lg
--font-size-xl
--font-size-2xl
--font-size-3xl
--line-height
--spacing-unit
--container-max
--sidebar-width
--header-height
--dock-height
```

Use them in `assets/main.css` so templates stay themeable.

## 9. Components and plugins

Two different extension points exist:

### Template components

Put `.js` files in `components/` when you want pfusch custom elements loaded on
every page of sites using the template.

Notes:

- all `.js` files in `components/` are concatenated into one module block
- `pfusch.js` itself is injected from the configured CDN automatically
- missing `components/` is fine

### Bundled plugins

Templates do not need to carry their own copy of Ferrosite's built-in plugins.
Instead, enable shared plugins in the template's `ferrosite.toml`:

```toml
[plugins]
enabled = ["contact-form"]
```

At build/runtime, Ferrosite resolves plugins from:

1. the site's own `plugins/` directory
2. Ferrosite's bundled `plugins/` directory

That means a built-in template can reference `contact-form` without shipping a
duplicate plugin directory inside the template.

If you need a template-specific custom plugin that Ferrosite does not bundle,
ship it as a site-level plugin in the scaffolded project or install it via
`ferrosite plugin add <git-url>`. Template authors should treat shared bundled
plugins as references, not copied assets.

Good use cases:

- contact forms
- lead capture
- simple API-backed interactive widgets

## 10. Minimal workflow to build a new template

1. Create `templates/<name>/layouts/base.html`.
2. Add the page layouts you need: usually `home`, `about`, `blog`, `projects`,
   `contact`, and `post`.
3. Add `theme.toml` and an `assets/main.css`.
4. Add starter markdown content under `content/`.
5. Add `ferrosite.toml` with `build.template = "<name>"`.
6. If needed, enable shared plugins in `ferrosite.toml`.
7. Scaffold a disposable site with `ferrosite new tmp-site --template <name>`.
8. Run `ferrosite build` inside that disposable site.
9. Open the generated HTML and verify slot usage, missing assets, and nav/footer
   links.

## 11. Practical starter checklist

- Keep placeholder content obviously fake but structurally realistic.
- Always include nav and footer content, or add sane layout fallbacks.
- Prefer normal HTML first; add custom components only when they earn their
  complexity.
- Make the homepage strong. It is the first thing users inspect after
  scaffolding.
- Test `ferrosite new ... --template <name>` instead of only building inside
  the ferrosite repo.
- Remember that site-level content overrides template content by relative path.

## 12. Recommended verification commands

From the ferrosite repo:

```bash
cargo run -- new tmp-site --template company --yolo
cd tmp-site
cargo run -- build
```

If the template enables plugins, this scaffold-and-build flow is still the best
verification because it exercises the same plugin resolution path real users
depend on.
