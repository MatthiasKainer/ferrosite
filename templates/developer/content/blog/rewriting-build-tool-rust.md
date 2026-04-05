---
title = "Why I Rewrote Our Build Tool in Rust"
slug = "rewriting-build-tool-rust"
slot = "article-body"
date = "2024-03-15"
author = "Horst Mustermann"
tags = ["rust", "tooling", "performance", "devex"]
categories = ["Engineering"]
featured = true
cover_alt = "Terminal showing build output with Rust compilation times"
description = "Our Python build tool worked — until it didn't. Here's what happened when we rewrote it in Rust, what we learned, and why the 6-week investment paid off in the first month."
---

## The Problem

Every large codebase eventually develops a build tool problem. Ours started as a 200-line Python script in 2018 and had, by early 2024, grown into a 14,000-line monster that took 40 seconds just to *start* before doing any actual work.

40 seconds. Every invocation. Before a single file had been touched.

The startup time alone was killing our iteration loop. Engineers were batching changes — which meant longer feedback cycles, which meant bugs that should have been caught immediately weren't caught until review.

Something had to change.

## Why Rust (and Not Go, Zig, etc.)

I'll be honest: I was already writing Rust for other things and wanted an excuse to use it more. But the justification was genuine:

**Memory model.** Build tools do a lot of graph traversal, parallel execution, and shared state. Rust's ownership system makes concurrent access errors a compile-time failure rather than a 3am incident.

**Binary size and distribution.** A single static binary with no runtime dependencies. No "please install Python 3.11" or "you need node >= 18". Just a binary.

**Compile-time guarantees.** The places where our Python tool would crash at runtime with `AttributeError: 'NoneType' object has no attribute 'sources'` — Rust makes those impossible to compile in the first place.

**Ecosystem.** `clap` for CLI parsing, `rayon` for parallelism, `tokio` for async I/O when we needed it. The ecosystem has matured enormously.

## The Architecture

```rust
// The core pipeline — railway oriented, all pure functions
pub fn run_build(config: &BuildConfig) -> BuildResult<BuildReport> {
    collect_targets(config)
        .and_then(|targets| resolve_dependencies(&targets))
        .and_then(|graph| topological_sort(&graph))
        .and_then(|order| execute_parallel(&order, config))
        .map(|results| aggregate_report(results))
}
```

The key insight we applied: pure functions everywhere possible, side effects only at the edges. The `collect_targets` function doesn't touch the filesystem directly — it returns a description of what *should* be collected, and the caller decides when to execute it.

This made testing dramatically easier. Pure functions with deterministic inputs are trivially unit testable.

## The Results

After 6 weeks of development and 2 weeks of gradual rollout:

| Metric | Python | Rust | Improvement |
|--------|--------|------|-------------|
| Cold start | 40s | 0.8s | **50x** |
| Incremental build | 12s | 1.1s | **11x** |
| Full rebuild | 4m 20s | 38s | **6.8x** |
| Binary size | ~200MB (venv) | 6MB | **33x smaller** |

The cold start improvement was the one engineers noticed immediately. Changing one file and seeing the result in under 2 seconds — versus waiting a minute — fundamentally changes how you work.

## What We Got Wrong

**We underestimated the plugin system.** The Python tool had an ad-hoc plugin system where people had dropped arbitrary `.py` files into a `hooks/` directory over the years. Replicating that flexibility in a compiled language required more thought. We ended up using dynamic loading via `libloading`, which works but added complexity.

**Cross-compilation.** Our monorepo builds for three target architectures. Getting Rust cross-compilation right took longer than we expected, especially for musl targets.

**The long tail of edge cases.** At 14,000 lines, the Python tool had accumulated a *lot* of accumulated tribal knowledge. Subtle behaviour that nobody had documented but that 3 teams depended on. A proper test suite would have caught this earlier.

## Conclusion

The rewrite was worth it. Not just for the performance numbers — though those matter — but for what the new codebase enables. It's readable, testable, and the type system prevents entire categories of bugs.

If your build tool is the thing slowing your team down, it might be time to take a hard look at what it's actually doing and whether the language it's written in is still the right choice.

---

*Have questions or want to compare notes? Reach out on [GitHub](https://github.com/horstmustermann) or [LinkedIn](https://linkedin.com/in/horstmustermann).*
