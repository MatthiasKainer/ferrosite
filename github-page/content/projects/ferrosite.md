---
title = "ferrosite"
slug = "ferrosite"
slot = "project-body"
page_scope = "projects"
status = "active"
tech_stack = ["Rust", "MiniJinja", "Markdown", "Cloudflare Workers"]
repo_url = "https://github.com/MatthiasKainer/ferrosite"
live_url = "https://matthiaskainer.github.io/ferrosite"
start_date = "2024"
description = "The static site generator behind matthias-kainer.de. Designed for understandable content pipelines, HTML-first output, and lightweight deployment."
---

## What it is

Ferrosite is a static site generator written in Rust. It favors explicit content, small template contracts, and a build pipeline that stays readable when you come back to it later.

## What makes it different

- Content is assigned to typed slots rather than copied into fixed page files.
- Pages render as complete HTML and can be progressively enhanced with web components.
- The same CLI handles scaffold, edit, build, preview, and deploy workflows.

## Good fit

Ferrosite works well when you want a product page, documentation site, personal site, or small company site that remains mostly static and easy to reason about.

## Less good fit

If you need a giant ecosystem, plugin marketplace gravity, or a full visual CMS, this probably is not the right tool.
