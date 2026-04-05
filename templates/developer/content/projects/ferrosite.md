---
title = "ferrosite"
slug = "ferrosite"
slot = "project-body"
page_scope = "projects"
status = "active"
tech_stack = ["Rust", "minijinja", "pfusch", "Cloudflare Workers"]
repo_url = "https://github.com/horstmustermann/ferrosite"
live_url = "https://ferrosite.dev"
start_date = "2024"
description = "A railway-oriented static site generator in Rust. Supports atomic-design slot system, pfusch web components, SSR via Puppeteer, and one-command deploy to Cloudflare/AWS/Azure."
---

## Overview

Ferrosite is a static site generator built around two core ideas:
**railway-oriented programming** (every operation is a `Result<T, E>`, side effects are at the edges) and **atomic design slot assignment** (content is assigned to typed positions on the page, not pasted into templates).

Built primarily as a playground for exploring how far Rust's type system can be pushed in the domain of content pipelines.

## Architecture

The build pipeline is a pure function composition:

1. Collect articles (IO edge)
2. Parse frontmatter → typed `Article` structs (pure)
3. Assign to slot map (pure)
4. Assemble pages (pure)
5. Render via minijinja templates (pure-ish)
6. Write output (IO edge)

Every step returns `SiteResult<T>`. Errors accumulate and are surfaced together.

## The Slot System

Articles declare their destination via `slot` in frontmatter:

```yaml
---
title: "Why I Rewrote Our Build Tool"
slot: "article-body"
date: "2024-03-15"
tags: ["rust", "tooling"]
---
```

Forty slot types cover atoms (badge, image), molecules (project-card, timeline-entry), organisms (hero, blog-feed), and layout regions (header-brand, footer-about).

## pfusch Integration

Pages use pfusch web components for interactive UI. Static rendering via Puppeteer `getHTML({includeShadowRoots: true})` for true SSR.
