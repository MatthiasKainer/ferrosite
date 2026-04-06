# How To Build, Edit, And Operate Your Site

This guide is for people running a Ferrosite site day to day.

It focuses on the practical workflow after you scaffold a site with
`ferrosite new`: how to edit content, how templates work, how slots are
assigned, how layouts pull content in, and how to build, preview, and ship the
result.

If you want to build a brand new reusable template, also read
[`templates/HOWTO-CREATE-A-TEMPLATE.md`](templates/HOWTO-CREATE-A-TEMPLATE.md).

## 1. Mental model: what Ferrosite actually does

Ferrosite is not a “drop markdown into a page” generator.

Instead, it works like this:

1. it reads markdown articles from `content/`
2. it parses frontmatter from each file
3. each article declares a `slot = "..."`
4. the build pipeline groups articles into slot buckets
5. layouts render those buckets using `slots['slot-name']`
6. the generated site is written to `dist/`

That means three parts must agree:

- markdown content declares the right slot
- layouts render that slot
- the article's `page_scope` places it on the intended page

When something does not appear on the page, one of those three is usually the
reason.

---

## 2. Normal workflow

Typical daily cycle:

```bash
# 1. inspect current configuration and content health
ferrosite check

# 2. create or update content
ferrosite add article
ferrosite edit content/about.md --open

# 3. verify slot routing if needed
ferrosite slots

# 4. build the site
ferrosite build

# 5. preview locally with plugin routes enabled
ferrosite run
```

Common command meanings:

- `ferrosite check` validates config, content loading, and slot parsing
- `ferrosite add article` interactively creates a blog post in `content/blog/`
- `ferrosite add project` interactively creates a project in `content/projects/`
- `ferrosite add page` scaffolds slot-based page content and can add a nav item
- `ferrosite add nav` creates a `nav-item` markdown file for global navigation
- `ferrosite edit <selector>` updates frontmatter for an existing content file
- `ferrosite assign-slot <selector> <slot>` re-routes an existing file to a new slot
- `ferrosite reorder` interactively reorders entries in a slot and saves new `order` values
- `ferrosite slots` lists discovered markdown files and their slot assignments
- `ferrosite build` writes the static site to `dist/`
- `ferrosite build --ssr` also runs the optional Puppeteer SSR pass
- `ferrosite run` builds and serves the site locally, including plugin workers
- `ferrosite deploy` deploys the already-built site
- `ferrosite ship` builds and deploys in one step
- `ferrosite config` prints the resolved site configuration

Use `ferrosite slots` more often than you think. It is the fastest way to
confirm whether your markdown ended up in the slot you intended.

### 2.1 Fast authoring commands

For everyday edits, the authoring commands are usually faster than creating
files by hand:

```bash
# Start an interactive blog post scaffold
ferrosite add article

# Create a project case study without prompts
ferrosite add project --title "Orbit Control" --tech-stack Rust,Axum,SQLite --yolo

# Scaffold page content for an existing routed page and add a nav entry
ferrosite add page --title "About" --page-scope about --slot about-body

# Create a standalone nav item
ferrosite add nav --title "Services" --url /services/

# Update slot/frontmatter on an existing file
ferrosite edit nav-about --url /about/
ferrosite assign-slot content/home.md hero --page-scope home

# Reorder nav items interactively
ferrosite reorder --slot nav-item
```

Selector rules for `ferrosite edit` and `ferrosite assign-slot`:

- a site-local path like `content/about.md`
- a filename stem like `nav-about`
- a unique slug like `rewriting-build-tool-rust`
- a unique exact title match

Use `--open` on `ferrosite add ...` or `ferrosite edit ...` if you want the
command to launch your configured editor immediately after creating or locating
the file.

For `ferrosite reorder`, the command shows the current entry list and accepts:

- `u <n>` to move entry `n` up one position
- `d <n>` to move entry `n` down one position
- `m <from> <to>` to move an entry directly to a target position
- `s` to save and rewrite `order` values
- `q` to quit without changing any files

If you do not pass `--slot`, the interactive prompt defaults to `nav-item`,
which makes site navigation reordering quick.

---

## 3. The site structure you edit

After `ferrosite new my-site --template company` (or `developer`), the site you
operate usually looks like this:

```text
my-site/
├── ferrosite.toml
├── content/
├── assets/
├── plugins/
├── templates/
│   └── <template-name>/
│       ├── layouts/
│       ├── components/
│       ├── content/
│       ├── assets/
│       └── theme.toml
└── dist/
```

The important edit points are:

- `ferrosite.toml` → site identity, routing, deployment, layout options
- `content/` → your markdown articles, posts, projects, nav items, footer copy
- `templates/<active-template>/layouts/` → page HTML layouts
- `templates/<active-template>/theme.toml` → theme tokens exposed as CSS vars
- `templates/<active-template>/components/` → pfusch custom elements loaded on pages
- `plugins/` → site-local plugin overrides and custom/git-installed plugins
- `assets/` → static images, fonts, CSS, downloads, extra files

Important rule:

- site-local `templates/<active-template>/...` is checked first
- bundled crate templates are only used as a fallback
- site-local `plugins/...` is checked first
- bundled Ferrosite plugins are used as the fallback

That means you can start from a bundled template and override only the files you
need in your site. The same now applies to plugins: built-in templates can just
enable a shared plugin in `ferrosite.toml`, and your site only needs a local
`plugins/<name>/` directory if you want to override or add one.

---

## 4. How content is discovered

Ferrosite loads markdown from two places:

1. your site's `content/`
2. your active template's `content/`

If the same relative path exists in both places, the site file wins.

This gives you a very practical override model:

- keep starter content in the template
- replace individual files in your site without modifying the original template

Example:

```text
templates/company/content/home.md   # starter hero
content/home.md                     # your override
```

If both files exist, `content/home.md` wins.

---

## 5. Frontmatter fields you will use most

Every markdown file needs frontmatter at the top.

Minimal example:

```markdown
---
title = "Home"
slot = "nav-item"
page_scope = "*"
url = "/"
order = 1
---
```

The most important fields are:

- `title` → human label, page title, or card title
- `slot` → where this article should render
- `page_scope` → which page gets it (`*`, `home`, `about`, `blog`, `projects`)
- `order` → lower values appear earlier inside a slot
- `weight` → ranking/priority metadata used when needed
- `slug` → URL slug override for posts and projects
- `description` → summary used by listings and metadata
- `url` → useful for nav, links, and contact-form endpoints

Then there are slot-specific fields such as:

- hero fields: `headline`, `sub_headline`, `cta_label`, `cta_url`
- blog fields: `date`, `tags`, `author`, `featured`, `cover_image`
- project fields: `tech_stack`, `repo_url`, `live_url`, `status`
- timeline fields: `company`, `role`, `location`, `start_date`, `end_date`
- stats fields: `value`, `label`, `suffix`

If a layout references a frontmatter field, you can usually add it directly to
the matching markdown file.

---

## 6. How to assign slots correctly

Slots are the routing system for content.

Some common slots:

### Site-wide and navigation

- `header-brand`
- `header-action`
- `nav-item`
- `footer-about`
- `footer-nav-column`
- `footer-bottom`
- `social-link`

These are effectively global. They are shared across pages.

### Homepage and landing sections

- `hero`
- `feature-card`
- `stat-number`
- `testimonial`
- `project-card`
- `article-card`

These are usually rendered on `home`.

### About page

- `about-body`
- `timeline-entry`
- `career-timeline`
- `skill-group`

These usually use `page_scope = "about"`.

### Blog and projects

- `article-body`
- `project-body`
- `table-of-contents`
- `code-snippet`
- `download-item`

These drive the detail pages.

### Contact page

- `contact-form`
- `social-link`
- optional `hero`

Important current behavior:

- the contact page currently clones the global slot map
- that means `contact-form` content should usually use `page_scope = "*"`

If you set `page_scope = "contact"` for a `contact-form` article today, it will
not automatically appear unless the pipeline changes.

---

## 7. `page_scope` rules that matter in practice

Use these values deliberately:

- `page_scope = "*"` → global/shared content
- `page_scope = "home"` → homepage content
- `page_scope = "about"` → about page content
- `page_scope = "blog"` → blog listing and blog-related content
- `page_scope = "projects"` → projects listing and project-related content

Practical guidance:

- navigation and footer items should almost always use `*`
- homepage hero sections should usually use `home`
- about body content should use `about`
- blog posts should use `blog`
- project case studies should use `projects`
- contact form content should currently use `*`

Remember that some layout-region slots are treated as global even if you give
them a page-specific scope.

---

## 8. Common content patterns

### Hero content

```markdown
---
title = "Signal Forge"
slot = "hero"
page_scope = "home"
headline = "Turn a strong market story into a working growth engine."
sub_headline = "Signal Forge designs product positioning, digital experiences, and AI-enabled operating systems."
cta_label = "See the work"
cta_url = "/projects/"
---
Homepage intro copy goes here.
```

### Navigation item

```markdown
---
title = "About"
slot = "nav-item"
page_scope = "*"
order = 2
url = "/about/"
---
```

### Blog post

```markdown
---
title = "Designing AI Workflows Operators Trust"
slug = "designing-ai-workflows-operators-trust"
slot = "article-body"
page_scope = "blog"
date = "2026-02-27"
tags = ["ai", "operations"]
description = "AI adoption accelerates when teams can see what the system knows and where escalation works."
---
Your article body here.
```

### Project case study

```markdown
---
title = "Orbit Control"
slug = "orbit-control"
slot = "project-body"
page_scope = "projects"
status = "Launch platform"
tech_stack = ["Positioning", "CMS", "Analytics"]
description = "Repositioned an industrial software company and shipped a new demand-generation website."
---
Your project write-up here.
```

### Contact form endpoint marker

```markdown
---
title = "Contact form"
slot = "contact-form"
page_scope = "*"
url = "/api/contact"
---
Dynamic form endpoint powered by a plugin.
```

---

## 9. Blog and project pages are mostly automatic

You usually do not need to manually author listing cards for normal blog and
project pages.

Ferrosite already does this:

- files in `content/blog/` with `slot = "article-body"` become blog detail pages
- those same files are also collected into the blog listing
- files in `content/projects/` with `slot = "project-body"` become project detail pages
- those same files are also collected into the projects listing

So the usual pattern is:

- put blog posts in `content/blog/*.md`
- put case studies in `content/projects/*.md`
- let the layouts render `article-card` or `project-card` collections assembled by the build

Only create manual `article-card` or `project-card` markdown if you want custom
cards separate from the generated listing flow.

---

## 10. How layouts consume slots

Layouts are Minijinja HTML templates in:

```text
templates/<active-template>/layouts/
```

Common filenames:

```text
base.html
home.html
about.html
blog.html
contact.html
projects.html
post.html
```

The layout typically checks for a slot and renders one or many articles from it.

Single-item slot example:

```html
{% if slots['hero'] %}
  {% set hero = slots['hero'][0] %}
  <h1>{{ hero.frontmatter.headline | default(value=hero.frontmatter.title) }}</h1>
  <div>{{ hero.html_body | safe }}</div>
{% endif %}
```

Multi-item slot example:

```html
{% if slots['feature-card'] %}
  {% for item in slots['feature-card'] %}
  <article>
    <h3>{{ item.frontmatter.title }}</h3>
    <div>{{ item.html_body | safe }}</div>
  </article>
  {% endfor %}
{% endif %}
```

Useful article properties available in layouts include:

- `frontmatter` → all parsed metadata fields
- `html_body` → rendered markdown body
- `excerpt` → summary text
- `url_path` → generated URL for posts/projects/cards

Useful site values include:

- `site.title`
- `site.description`
- `site.author.*`
- `site.social.*`
- `site.layout.*`

If a page-specific layout is missing, Ferrosite falls back to `base.html`.

---

## 11. How to safely edit a template

When changing the structure or appearance of the site, work in this order:

1. decide which slot should power the section
2. create or update the markdown article using that slot
3. update the matching layout to render that slot
4. run `ferrosite slots` to verify assignment
5. run `ferrosite build` and inspect the output

Example: add a new homepage section.

### Step A: create content

```markdown
---
title = "Systems thinking"
slot = "feature-card"
page_scope = "home"
order = 10
icon = "03"
---
We connect messaging, design, and delivery so growth work survives real operations.
```

### Step B: render it in the homepage layout

```html
{% if slots['feature-card'] %}
  {% for item in slots['feature-card'] %}
  <article class="value-card">
    <h3>{{ item.frontmatter.title }}</h3>
    <div>{{ item.html_body | safe }}</div>
  </article>
  {% endfor %}
{% endif %}
```

That is the core Ferrosite loop: markdown declares, layout renders.

---

## 12. How theme and styling work

Each template can define `theme.toml`.

Ferrosite converts it into CSS custom properties and injects them into the
page. Typical variables include:

```css
--color-primary
--color-primary-dark
--color-accent
--color-bg
--color-surface
--color-text
--color-text-muted
--color-border
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
```

Practical workflow:

- use `theme.toml` for design tokens
- use `assets/main.css` (or your stylesheet) for layout and component styling
- keep layout HTML semantic and let CSS do the heavy visual work

---

## 13. Components vs plugins

Ferrosite has two different extension points.

### Template components

Put `.js` files in `templates/<active-template>/components/` when you want a
pfusch custom element or client-side enhancement loaded as part of the theme.

Use this for:

- interactive presentational components
- UI behaviors that do not need a server worker
- custom elements rendered on multiple pages

### Plugins

Put plugin folders in `plugins/` when you need a worker and possibly a web
component.

Use this for:

- contact forms
- API-backed widgets
- data collection
- command/query workflows

`ferrosite run` is especially useful here because it serves the site locally and
also routes plugin requests through the local worker runner.

### How to update an existing plugin

There is currently no dedicated `ferrosite plugin update` command.

Use one of these workflows instead.

#### A. update a plugin you maintain locally

If the plugin already lives in your site's `plugins/` directory and you own the
code:

1. edit the plugin files directly:
  - `plugins/<plugin-name>/manifest.toml`
  - `plugins/<plugin-name>/component.js`
  - `plugins/<plugin-name>/worker.js`
2. if you changed the plugin name, slots, route, or component tag, also update
  the matching references in your content and layouts
3. keep the plugin listed in `plugins.enabled` inside `ferrosite.toml`
4. run `ferrosite run` and test the page plus worker route together

#### B. update a plugin installed from git

If the plugin directory is a git checkout:

```bash
cd plugins/<plugin-name>
git pull
```

Then:

1. review any manifest changes
2. check whether new environment variables are required
3. run `ferrosite run` to verify the updated component and worker

#### C. refresh a bundled plugin shipped with ferrosite

Bundled plugins are copied into your site at install time. They do not update
automatically when ferrosite itself changes.

To refresh one:

```bash
ferrosite plugin remove <plugin-name>
ferrosite plugin add <plugin-name>
```

Important:

- `plugin remove` deletes the installed plugin directory from your site
- it also prints files that still reference that plugin so you can review them
- if you made local changes inside `plugins/<plugin-name>/`, back them up first

#### Recommended verification checklist

After any plugin update:

1. confirm `plugins.enabled` still contains the plugin name
2. confirm the worker route in `manifest.toml` still matches your forms or fetch calls
3. confirm required env vars are set for local/dev/prod
4. run `ferrosite run`
5. exercise the page that renders the plugin and the worker endpoint it calls

---

## 14. Operating the contact form

The bundled company template ships a contact-form plugin and a contact page
layout that does this:

- if a `contact-form` slot exists, it renders `<ferrosite-contact-form>`
- otherwise it falls back to a plain HTML form

Recommended workflow:

1. keep the `contact-form` markdown file in place
2. set its `url` field to the worker endpoint you want to use
3. configure the plugin secrets required by that worker
4. use `ferrosite run` for local end-to-end testing
5. verify production env vars before deploying

If the dynamic form is not ready yet, the static fallback still gives you a
usable page structure.

---

## 15. Site configuration fields worth revisiting

In `ferrosite.toml`, these fields commonly matter during operation:

```toml
[site]
title = "My Site"
description = "What the site is about"
base_url = "https://example.com"
language = "en"

[site.author]
name = "Your Name"
email = "hello@example.com"

[build]
template = "company"
content_dir = "content"
output_dir = "dist"
assets_dir = "assets"
minify = true

[layout]
menu = true
dock = false
sidebar = false

[routes]
blog_post_path = "/blog/{slug}/"
projects_path = "/projects/{slug}/"
```

Operationally important notes:

- `build.template` selects the active template directory
- `output_dir` decides where generated files go
- `layout.*` toggles template-exposed UI behavior
- `routes.*` influences generated post/project URLs

---

## 16. SSR: when to use it

Use SSR when your components render important content inside shadow DOM and you
want the final HTML to include that output.

Typical workflow:

```bash
ferrosite ssr-setup
ferrosite build --ssr
```

You usually do not need SSR for a mostly static site with minimal enhancement.
Turn it on when pre-rendered component HTML materially improves SEO,
accessibility, or zero-JS rendering.

Ferrosite now runs SSR in batches: it skips pages that do not contain
SSR-marked components, reuses one Puppeteer browser for the whole pass, and
limits parallel page rendering with `[build.ssr].concurrency` so the renderer
does not fan out into one Chromium process per page.

---

## 17. Safe release checklist

Before shipping:

1. run `ferrosite check`
2. run `ferrosite slots` if you changed content structure
3. run `ferrosite build`
4. inspect `dist/` locally
5. run `ferrosite run` for plugin-backed features
6. confirm deploy credentials and worker secrets
7. run `ferrosite ship`

If the site is heavily template-driven, also click through:

- home
- about
- blog listing
- one blog post
- projects listing
- one project page
- contact

---

## 18. Common mistakes and fast fixes

### “My content does not render.”

Check:

- does the markdown file have valid frontmatter?
- is `slot = "..."` a real slot name?
- does the target layout render `slots['that-slot']`?
- does `page_scope` match the page you expect?
- does `ferrosite slots` show the file at all?

### “The page builds but the section is empty.”

Usually one of these:

- the layout expects a slot that no article fills
- the article uses a page scope the page does not receive
- the layout expects a field the frontmatter does not define

### “My contact form is missing.”

Check:

- is there a markdown file with `slot = "contact-form"`?
- is it using `page_scope = "*"`?
- is the plugin installed and enabled?
- does `ferrosite run` show the worker route working locally?

### “My override did not take effect.”

Check:

- did you put it in `content/` or `templates/<active-template>/...`?
- does the relative path match the file you meant to override?
- is `build.template` set to the template you are editing?

---

## 19. A good rule of thumb

When deciding where to make a change:

- change `content/` when the structure is correct and only the content changes
- change `layouts/` when the presentation or page composition changes
- change `theme.toml` and CSS when the design changes
- change `components/` for client-side UI behavior
- change `plugins/` for dynamic/server-backed behavior

If you keep those boundaries clean, Ferrosite stays easy to reason about.
