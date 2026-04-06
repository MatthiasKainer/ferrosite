use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use walkdir::WalkDir;

use crate::config::{load_site_config_for_root, template_dir};
use crate::deploy::generate_cloudflare_worker;
use crate::error::{io_with_path, SiteError, SiteResult};
use crate::pipeline::build::{build_site, BuildContext, BuildReport};
use crate::plugin::{bundled_plugins_dir, site_plugins_dir, Plugin};
use crate::url::decode_url_path;

const LOCAL_RUNNER_SCRIPT: &str = r#"import { Buffer } from "node:buffer";
import { pathToFileURL } from "node:url";

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf8");
}

const raw = await readStdin();
const envelope = JSON.parse(raw);
const moduleUrl = pathToFileURL(envelope.worker_path).href;
const workerModule = await import(moduleUrl);

const headers = new Headers(envelope.request.headers || {});
const requestInit = {
  method: envelope.request.method,
  headers,
};

if (envelope.request.body_base64) {
  requestInit.body = Buffer.from(envelope.request.body_base64, "base64");
}

const waitUntilPromises = [];
const ctx = {
  waitUntil(promise) {
    waitUntilPromises.push(Promise.resolve(promise));
  },
  passThroughOnException() {},
};

const request = new Request(envelope.request.url, requestInit);
const response = await workerModule.default.fetch(request, envelope.env || {}, ctx);
await Promise.allSettled(waitUntilPromises);

const responseHeaders = {};
response.headers.forEach((value, key) => {
  responseHeaders[key] = value;
});

const bodyBuffer = Buffer.from(new Uint8Array(await response.arrayBuffer()));
process.stdout.write(JSON.stringify({
  status: response.status,
  headers: responseHeaders,
  body_base64: bodyBuffer.toString("base64"),
}));
"#;

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub host: String,
    pub port: u16,
    pub no_build: bool,
}

#[derive(Debug, Clone)]
struct RunState {
    output_dir: PathBuf,
    plugin_runtime: Option<PluginRuntime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchSnapshot {
    entries: BTreeMap<PathBuf, WatchEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchEntry {
    kind: WatchEntryKind,
    len: u64,
    modified_unix_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatchEntryKind {
    Missing,
    File,
    Directory,
}

#[derive(Debug, Clone)]
struct PluginRuntime {
    node_bin: String,
    runner_script: PathBuf,
    workers: HashMap<String, LocalWorker>,
}

#[derive(Debug, Clone)]
struct LocalWorker {
    plugin_name: String,
    worker_path: PathBuf,
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    reason: &'static str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Debug, Serialize)]
struct WorkerEnvelope<'a> {
    worker_path: &'a str,
    env: &'a HashMap<String, String>,
    request: WorkerRequest<'a>,
}

#[derive(Debug, Serialize)]
struct WorkerRequest<'a> {
    method: &'a str,
    url: String,
    headers: &'a HashMap<String, String>,
    body_base64: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkerResponse {
    status: u16,
    headers: HashMap<String, String>,
    body_base64: String,
}

pub fn run_site(site_root: &Path, options: &RunOptions) -> SiteResult<()> {
    if !options.no_build {
        build_site(site_root)?;
    }

    let bind_addr = format!("{}:{}", options.host, options.port);
    let listener = TcpListener::bind(&bind_addr).map_err(|e| {
        SiteError::Build(format!(
            "Failed to bind local server at {}: {}",
            bind_addr, e
        ))
    })?;

    let state = Arc::new(RwLock::new(load_run_state(site_root)?));
    let initial_state = read_run_state(&state)?;
    let output_dir = initial_state.output_dir.clone();
    println!("🌐 Serving {}", output_dir.display());
    println!("   URL: http://{}", bind_addr);
    println!(
        "   Rebuilds: {}",
        if options.no_build {
            "disabled (--no-build)"
        } else {
            "enabled (polling for changes)"
        }
    );
    if let Some(runtime) = &initial_state.plugin_runtime {
        println!("   Plugin routes:");
        for route in runtime.workers.keys() {
            println!("   - {}", route);
        }
    }
    println!("   Press Ctrl+C to stop");

    if !options.no_build {
        let site_root = site_root.to_path_buf();
        let state = Arc::clone(&state);
        std::thread::spawn(move || watch_and_rebuild(site_root, state));
    }

    let bind_addr = Arc::new(bind_addr);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                let bind_addr = Arc::clone(&bind_addr);
                std::thread::spawn(move || {
                    if let Err(err) = handle_connection(stream, &state, &bind_addr) {
                        eprintln!("run: {}", err);
                    }
                });
            }
            Err(err) => eprintln!("run: accept failed: {}", err),
        }
    }

    Ok(())
}

fn load_run_state(site_root: &Path) -> SiteResult<RunState> {
    let ctx = BuildContext::load(site_root)?;
    let output_dir = site_root.join(&ctx.config.build.output_dir);
    if !output_dir.exists() {
        return Err(SiteError::Build(format!(
            "Output directory '{}' not found. Run 'ferrosite build' first or omit '--no-build'.",
            output_dir.display()
        )));
    }

    let plugin_runtime = PluginRuntime::prepare(&ctx, &output_dir)?;

    Ok(RunState {
        output_dir,
        plugin_runtime,
    })
}

fn read_run_state(state: &RwLock<RunState>) -> SiteResult<RunState> {
    state
        .read()
        .map_err(|_| SiteError::Build("Run state lock poisoned".into()))
        .map(|guard| guard.clone())
}

fn replace_run_state(state: &RwLock<RunState>, next: RunState) -> SiteResult<()> {
    state
        .write()
        .map_err(|_| SiteError::Build("Run state lock poisoned".into()))
        .map(|mut guard| {
            *guard = next;
        })
}

fn watch_and_rebuild(site_root: PathBuf, state: Arc<RwLock<RunState>>) {
    let poll_interval = Duration::from_millis(750);
    let mut snapshot = collect_watch_snapshot(&site_root);

    println!(
        "   Watching for changes under site content, templates, assets, plugins, and SSR files"
    );

    loop {
        std::thread::sleep(poll_interval);

        let next_snapshot = collect_watch_snapshot(&site_root);
        if next_snapshot == snapshot {
            continue;
        }

        let changes = describe_snapshot_changes(&site_root, &snapshot, &next_snapshot);

        if changes.is_empty() {
            println!("🔁 Change detected. Rebuilding…");
        } else {
            println!("🔁 Change detected: {}", changes.join(", "));
            println!("   Rebuilding…");
        }

        match rebuild_and_refresh(&site_root, state.as_ref()) {
            Ok(report) => {
                println!(
                    "✅ Rebuild complete: {} page(s), {} article(s) → {}",
                    report.pages_built,
                    report.articles_processed,
                    report.output_dir.display()
                );
            }
            Err(err) => eprintln!("run: rebuild failed: {}", err),
        }

        snapshot = collect_watch_snapshot(&site_root);
    }
}

fn rebuild_and_refresh(site_root: &Path, state: &RwLock<RunState>) -> SiteResult<BuildReport> {
    let report = build_site(site_root)?;
    let next_state = load_run_state(site_root)?;
    replace_run_state(state, next_state)?;
    Ok(report)
}

fn collect_watch_snapshot(site_root: &Path) -> WatchSnapshot {
    let mut entries = BTreeMap::new();

    for root in watched_roots(site_root) {
        collect_path_snapshot(&root, &mut entries);
    }

    WatchSnapshot { entries }
}

fn watched_roots(site_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        site_root.join("ferrosite.toml"),
        site_root.join("content"),
        site_root.join("assets"),
        site_root.join("plugins"),
        site_root.join("ssr"),
    ];

    if let Ok(config) = load_site_config_for_root(site_root) {
        roots.push(site_root.join(&config.build.content_dir));
        roots.push(site_root.join(&config.build.assets_dir));

        roots.push(site_plugins_dir(
            site_root,
            config.plugins.plugins_dir.as_deref(),
        ));
        roots.push(bundled_plugins_dir());
        roots.push(template_dir(site_root, &config.build.template));
    }

    roots.sort();
    roots.dedup();
    roots
}

fn collect_path_snapshot(path: &Path, entries: &mut BTreeMap<PathBuf, WatchEntry>) {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            entries.insert(
                path.to_path_buf(),
                WatchEntry {
                    kind: WatchEntryKind::Missing,
                    len: 0,
                    modified_unix_ms: 0,
                },
            );
            return;
        }
        Err(_) => {
            entries.insert(
                path.to_path_buf(),
                WatchEntry {
                    kind: WatchEntryKind::Missing,
                    len: 0,
                    modified_unix_ms: 0,
                },
            );
            return;
        }
    };

    if metadata.is_dir() {
        for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
            let entry_path = entry.path().to_path_buf();
            let Ok(entry_meta) = entry.metadata() else {
                continue;
            };
            entries.insert(entry_path, snapshot_entry(&entry_meta));
        }
    } else {
        entries.insert(path.to_path_buf(), snapshot_entry(&metadata));
    }
}

fn snapshot_entry(metadata: &std::fs::Metadata) -> WatchEntry {
    let kind = if metadata.is_dir() {
        WatchEntryKind::Directory
    } else {
        WatchEntryKind::File
    };

    let modified_unix_ms = metadata
        .modified()
        .ok()
        .and_then(|timestamp| timestamp.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    WatchEntry {
        kind,
        len: metadata.len(),
        modified_unix_ms,
    }
}

fn describe_snapshot_changes(
    site_root: &Path,
    previous: &WatchSnapshot,
    current: &WatchSnapshot,
) -> Vec<String> {
    let mut changes = Vec::new();

    for (path, entry) in &current.entries {
        match previous.entries.get(path) {
            None => changes.push(format!("created {}", display_watch_path(site_root, path))),
            Some(old) if old != entry => {
                let label = match (old.kind, entry.kind) {
                    (WatchEntryKind::Missing, _) => "created",
                    (_, WatchEntryKind::Missing) => "removed",
                    _ => "updated",
                };
                changes.push(format!("{} {}", label, display_watch_path(site_root, path)));
            }
            _ => {}
        }
    }

    for path in previous.entries.keys() {
        if !current.entries.contains_key(path) {
            changes.push(format!("removed {}", display_watch_path(site_root, path)));
        }
    }

    changes.sort();
    if changes.len() > 6 {
        let remaining = changes.len() - 6;
        changes.truncate(6);
        changes.push(format!("and {} more change(s)", remaining));
    }
    changes
}

fn display_watch_path(site_root: &Path, path: &Path) -> String {
    match path.strip_prefix(site_root) {
        Ok(relative) if !relative.as_os_str().is_empty() => relative.display().to_string(),
        Ok(_) => ".".into(),
        Err(_) => path.display().to_string(),
    }
}

impl PluginRuntime {
    fn prepare(ctx: &BuildContext, output_dir: &Path) -> SiteResult<Option<Self>> {
        if ctx.plugins.is_empty() {
            return Ok(None);
        }

        let node_bin = ctx.config.build.ssr.node_bin.clone();
        let node_available = Command::new(&node_bin)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok();
        if !node_available {
            return Err(SiteError::Build(format!(
                "Plugin runner requires '{}' in PATH, but it was not found.",
                node_bin
            )));
        }

        let runtime_dir = output_dir.join("_ferrosite");
        let workers_dir = runtime_dir.join("workers");
        std::fs::create_dir_all(&workers_dir)?;

        let runner_script = runtime_dir.join("worker-runner.mjs");
        std::fs::write(&runner_script, LOCAL_RUNNER_SCRIPT)?;

        let mut workers = HashMap::new();
        for plugin in ctx.plugins.workers() {
            warn_missing_env(plugin);

            let worker_path = workers_dir.join(format!("{}.worker.mjs", plugin.manifest.name));
            std::fs::write(&worker_path, generate_cloudflare_worker(plugin))?;

            workers.insert(
                normalize_route(&plugin.manifest.worker_route),
                LocalWorker {
                    plugin_name: plugin.manifest.name.clone(),
                    worker_path,
                },
            );
        }

        Ok(Some(Self {
            node_bin,
            runner_script,
            workers,
        }))
    }

    fn maybe_handle(
        &self,
        request: &HttpRequest,
        server_addr: &str,
    ) -> SiteResult<Option<HttpResponse>> {
        let path = normalize_route(request_path(&request.target));
        let Some(worker) = self.workers.get(&path) else {
            return Ok(None);
        };

        let worker_path = worker.worker_path.to_str().ok_or_else(|| {
            SiteError::Build(format!(
                "Worker path for plugin '{}' is not valid UTF-8",
                worker.plugin_name
            ))
        })?;

        let env: HashMap<String, String> = std::env::vars().collect();
        let body_base64 = (!request.body.is_empty())
            .then(|| base64::engine::general_purpose::STANDARD.encode(&request.body));
        let envelope = WorkerEnvelope {
            worker_path,
            env: &env,
            request: WorkerRequest {
                method: &request.method,
                url: format!("http://{}{}", server_addr, request.target),
                headers: &request.headers,
                body_base64,
            },
        };

        let payload = serde_json::to_vec(&envelope)?;
        let mut child = Command::new(&self.node_bin)
            .arg(&self.runner_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| SiteError::Build(format!("Failed to launch plugin runner: {}", e)))?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(&payload)?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| SiteError::Build(format!("Plugin runner failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(Some(text_response(
                500,
                "Internal Server Error",
                format!("Plugin runner failed: {}", stderr.trim()),
                "text/plain; charset=utf-8",
            )));
        }

        let worker_response: WorkerResponse = serde_json::from_slice(&output.stdout)?;
        let body = base64::engine::general_purpose::STANDARD
            .decode(worker_response.body_base64.as_bytes())
            .map_err(|e| SiteError::Build(format!("Invalid worker response body: {}", e)))?;

        let mut headers: Vec<(String, String)> = worker_response.headers.into_iter().collect();
        headers.push(("Content-Length".into(), body.len().to_string()));
        headers.push(("Connection".into(), "close".into()));

        Ok(Some(HttpResponse {
            status: worker_response.status,
            reason: reason_phrase(worker_response.status),
            headers,
            body,
        }))
    }
}

fn warn_missing_env(plugin: &Plugin) {
    let missing: Vec<&str> = plugin
        .manifest
        .required_env
        .iter()
        .map(String::as_str)
        .filter(|key| std::env::var(key).is_err())
        .collect();

    if !missing.is_empty() {
        eprintln!(
            "warning: plugin '{}' is missing env vars: {}",
            plugin.manifest.name,
            missing.join(", ")
        );
    }
}

fn handle_connection(
    mut stream: TcpStream,
    state: &RwLock<RunState>,
    server_addr: &str,
) -> SiteResult<()> {
    let state = read_run_state(state)?;
    let request = match read_request(&mut stream)? {
        Some(request) => request,
        None => return Ok(()),
    };

    let response = if let Some(runtime) = &state.plugin_runtime {
        if let Some(plugin_response) = runtime.maybe_handle(&request, server_addr)? {
            plugin_response
        } else {
            serve_static(&state.output_dir, &request)?
        }
    } else {
        serve_static(&state.output_dir, &request)?
    };

    write_response(&mut stream, &request.method, response)?;
    Ok(())
}

fn read_request(stream: &mut TcpStream) -> SiteResult<Option<HttpRequest>> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(SiteError::from)?;

    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let headers_end = loop {
        let read = match stream.read(&mut chunk) {
            Ok(read) => read,
            Err(err) if is_benign_read_error(&err) => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        if read == 0 {
            return Ok(None);
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(pos) = find_headers_end(&buffer) {
            break pos;
        }
        if buffer.len() > 1024 * 1024 {
            return Err(SiteError::Build("Request headers too large".into()));
        }
    };

    let headers_raw = String::from_utf8_lossy(&buffer[..headers_end]);
    let mut lines = headers_raw.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| SiteError::Build("Malformed HTTP request".into()))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| SiteError::Build("Missing HTTP method".into()))?
        .to_string();
    let target = request_parts
        .next()
        .ok_or_else(|| SiteError::Build("Missing HTTP target".into()))?
        .to_string();

    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = headers_end + 4;
    let mut body = buffer[body_start..].to_vec();

    while body.len() < content_length {
        let read = match stream.read(&mut chunk) {
            Ok(read) => read,
            Err(err) if is_benign_read_error(&err) => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);

    Ok(Some(HttpRequest {
        method,
        target,
        headers,
        body,
    }))
}

fn serve_static(output_dir: &Path, request: &HttpRequest) -> SiteResult<HttpResponse> {
    let method = request.method.as_str();
    if method != "GET" && method != "HEAD" {
        return Ok(text_response(
            405,
            "Method Not Allowed",
            "Method not allowed".into(),
            "text/plain; charset=utf-8",
        ));
    }

    let rel_path = resolve_static_path(output_dir, request_path(&request.target));
    if let Some(path) = rel_path {
        let body = match std::fs::read(&path) {
            Ok(body) => body,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                eprintln!(
                    "run: static file missing while serving '{}': {}",
                    request.target,
                    path.display()
                );
                return Ok(text_response(
                    404,
                    "Not Found",
                    format!("Not found: {}", request.target),
                    "text/plain; charset=utf-8",
                ));
            }
            Err(err) => return Err(io_with_path(&path, "reading static file")(err)),
        };
        let mime = guess_mime_type(&path);
        return Ok(binary_response(200, "OK", body, mime));
    }

    let fallback_404 = output_dir.join("404").join("index.html");
    if fallback_404.exists() {
        let body = match std::fs::read(&fallback_404) {
            Ok(body) => body,
            Err(err) if err.kind() == ErrorKind::NotFound => Vec::new(),
            Err(err) => return Err(io_with_path(&fallback_404, "reading 404 page")(err)),
        };
        if body.is_empty() {
            return Ok(text_response(
                404,
                "Not Found",
                "Not found".into(),
                "text/plain; charset=utf-8",
            ));
        }
        return Ok(binary_response(
            404,
            "Not Found",
            body,
            "text/html; charset=utf-8",
        ));
    }

    Ok(text_response(
        404,
        "Not Found",
        "Not found".into(),
        "text/plain; charset=utf-8",
    ))
}

fn is_benign_read_error(err: &std::io::Error) -> bool {
    matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

fn resolve_static_path(output_dir: &Path, request_path: &str) -> Option<PathBuf> {
    let relative = sanitize_path(request_path)?;

    let direct = output_dir.join(&relative);
    if direct.is_file() {
        return Some(direct);
    }

    let index = output_dir.join(&relative).join("index.html");
    if index.is_file() {
        return Some(index);
    }

    if relative.as_os_str().is_empty() {
        let root_index = output_dir.join("index.html");
        if root_index.is_file() {
            return Some(root_index);
        }
    }

    None
}

fn sanitize_path(request_path: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for segment in request_path.trim_start_matches('/').split('/') {
        if segment.is_empty() {
            continue;
        }
        let decoded = decode_url_path(segment)?;
        if decoded == "."
            || decoded == ".."
            || decoded.contains('/')
            || decoded.contains('\\')
            || decoded.contains('\0')
        {
            return None;
        }
        path.push(decoded);
    }
    Some(path)
}

fn write_response(stream: &mut TcpStream, method: &str, response: HttpResponse) -> SiteResult<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\n",
        response.status, response.reason
    )?;

    for (name, value) in &response.headers {
        write!(stream, "{}: {}\r\n", name, value)?;
    }
    write!(stream, "\r\n")?;

    if method != "HEAD" {
        stream.write_all(&response.body)?;
    }
    stream.flush()?;
    Ok(())
}

fn text_response(
    status: u16,
    reason: &'static str,
    body: String,
    content_type: &str,
) -> HttpResponse {
    binary_response(status, reason, body.into_bytes(), content_type)
}

fn binary_response(
    status: u16,
    reason: &'static str,
    body: Vec<u8>,
    content_type: &str,
) -> HttpResponse {
    HttpResponse {
        status,
        reason,
        headers: vec![
            ("Content-Type".into(), content_type.into()),
            ("Content-Length".into(), body.len().to_string()),
            ("Connection".into(), "close".into()),
        ],
        body,
    }
}

fn find_headers_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn request_path(target: &str) -> &str {
    target.split('?').next().unwrap_or("/")
}

fn normalize_route(route: &str) -> String {
    let trimmed = route.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".into();
    }

    let without_slash = trimmed.trim_end_matches('/');
    if without_slash.starts_with('/') {
        without_slash.to_string()
    } else {
        format!("/{}", without_slash)
    }
}

fn guess_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_plugin_routes() {
        assert_eq!(normalize_route("/api/contact/"), "/api/contact");
        assert_eq!(normalize_route("api/contact"), "/api/contact");
        assert_eq!(normalize_route("/"), "/");
    }

    #[test]
    fn resolves_directory_index_files() {
        let temp = tempfile::tempdir().unwrap();
        let about = temp.path().join("about");
        std::fs::create_dir_all(&about).unwrap();
        std::fs::write(about.join("index.html"), "<h1>About</h1>").unwrap();

        let resolved = resolve_static_path(temp.path(), "/about").unwrap();
        assert_eq!(resolved, about.join("index.html"));
    }

    #[test]
    fn resolves_percent_encoded_static_paths() {
        let temp = tempfile::tempdir().unwrap();
        let image_path = temp.path().join("static").join("hero image.png");
        std::fs::create_dir_all(image_path.parent().unwrap()).unwrap();
        std::fs::write(&image_path, b"png").unwrap();

        let resolved = resolve_static_path(temp.path(), "/static/hero%20image.png").unwrap();
        assert_eq!(resolved, image_path);
    }
}
