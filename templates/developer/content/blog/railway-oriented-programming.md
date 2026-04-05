---
title = "Railway Oriented Programming in Rust"
slug = "railway-oriented-programming-rust"
slot = "article-body"
date = "2024-01-20"
author = "Horst Mustermann"
tags = ["rust", "functional", "error-handling", "patterns"]
categories = ["Engineering"]
description = "How Rust's Result type and the ? operator make railway oriented programming not just possible but natural — and why you should structure your code this way."
---

## What Is Railway Oriented Programming?

The metaphor comes from F# developer Scott Wlaschin: imagine your program as a railway track. The happy path is one rail; the failure path is another. At every switch point (function call), you either stay on the happy track or divert to the failure track — and once you're on the failure track, you stay there, bypassing all subsequent happy-track operations, until you reach the end of the line where errors are handled.

In code, this maps directly to `Result<T, E>`.

## The Problem With Traditional Error Handling

Consider fetching a user, validating their permissions, loading their data, and transforming it:

```rust
// Traditional: nested, hard to follow, error handling mixed with logic
fn get_user_dashboard(user_id: u64) -> Dashboard {
    let user = match fetch_user(user_id) {
        Ok(u) => u,
        Err(e) => {
            log::error!("fetch failed: {}", e);
            return Dashboard::error("User not found");
        }
    };
    let perms = match check_permissions(&user) {
        Ok(p) => p,
        Err(e) => return Dashboard::error("Unauthorized"),
    };
    // ... and so on
}
```

The error handling dominates. The actual logic — fetch, check, load, transform — is buried in match arms.

## The Railway Solution

```rust
// Railway oriented: logic is the spine, errors are handled at the edge
fn get_user_dashboard(user_id: u64) -> Result<Dashboard, AppError> {
    fetch_user(user_id)
        .and_then(|user| check_permissions(user))
        .and_then(|user| load_dashboard_data(user))
        .map(|data| transform_to_dashboard(data))
}
```

The `?` operator makes this even more ergonomic:

```rust
fn get_user_dashboard(user_id: u64) -> Result<Dashboard, AppError> {
    let user = fetch_user(user_id)?;
    let user = check_permissions(user)?;
    let data = load_dashboard_data(user)?;
    Ok(transform_to_dashboard(data))
}
```

Both are equivalent. Use `and_then` when chaining feels natural; use `?` when you need intermediate values in scope.

## Pure Functions as Railway Cars

The railway metaphor works best when each function in the chain is **pure** — it takes input, produces output, has no side effects, and given the same input always returns the same output.

```rust
// Pure: deterministic, testable, no hidden state
fn validate_email(email: &str) -> Result<Email, ValidationError> {
    if email.contains('@') && email.contains('.') {
        Ok(Email(email.to_string()))
    } else {
        Err(ValidationError::InvalidEmail(email.to_string()))
    }
}

// Side effect: happens only at the edge
fn save_user(user: ValidatedUser) -> Result<UserId, DbError> {
    database.insert(user)  // <-- only here do we touch external state
}
```

By keeping pure functions at the centre and side effects at the edges, your logic becomes trivially testable and your side effects become auditable.

## Collecting Multiple Errors

Sometimes you want to validate several things in parallel and collect *all* failures, not just the first. Standard `?` stops at the first error. Here's a pattern for accumulating:

```rust
pub fn collect_results<T, E>(results: Vec<Result<T, E>>) -> Result<Vec<T>, Vec<E>> {
    let (oks, errs): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);
    if errs.is_empty() {
        Ok(oks.into_iter().map(Result::unwrap).collect())
    } else {
        Err(errs.into_iter().map(Result::unwrap_err).collect())
    }
}

// Usage: validate all fields, surface all errors at once
let results = vec![
    validate_name(&form.name),
    validate_email(&form.email),
    validate_phone(&form.phone),
];
let validated = collect_results(results).map_err(|errs| FormErrors(errs))?;
```

## The `thiserror` Ecosystem

Defining clean error types is half the work. `thiserror` gives you derive macros that generate `Display` and `From` implementations:

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("User {0} not found")]
    UserNotFound(u64),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Database error: {source}")]
    Database { #[from] source: sqlx::Error },
}
```

The `#[from]` attribute generates `From<sqlx::Error> for AppError`, so `?` works seamlessly across error type boundaries.

## When NOT to Use It

Railway orientation is powerful but not universal:

- **Performance-critical hot paths**: `Result` boxing and monadic chaining add overhead. In a tight loop processing millions of items, direct pattern matching may be faster.
- **Truly panicky situations**: If your program *cannot* meaningfully continue (OOM, corrupted invariant), `panic!` is appropriate. Don't `Result`-wrap everything out of ceremony.
- **Simple scripts**: A 20-line build script can just unwrap everything and let it crash. Save the ceremony for production code.

## Wrapping Up

Railway Oriented Programming in Rust isn't a library or a framework — it's an attitude. Use `Result` as your primary control flow mechanism. Keep functions pure. Put side effects at the edges. Let the type system track what can fail.

The compile-time guarantees you get in return are substantial: no unhandled exceptions, no silent failures, no "works on my machine" surprises when the database is unavailable.

The train, once you're on it, goes exactly where you tell it.
