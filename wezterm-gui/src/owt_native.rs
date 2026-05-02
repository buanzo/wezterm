use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context};
use once_cell::sync::Lazy;
use owt_control::{
    validate_interface, AppliedInterface, ControlStatus, DispatchedAction, InterfaceDocument,
    RuntimeState, Scope, UiAction, UiNode,
};
use serde::Deserialize;
use serde_json::{json, Value};
use window::WindowOps;

use crate::SubCommand;

const MAX_REQUEST_BYTES: usize = 256 * 1024;

static OWT_RUNTIME: Lazy<Arc<Mutex<RuntimeState>>> =
    Lazy::new(|| Arc::new(Mutex::new(RuntimeState::default())));
static OWT_RENDER_PASSES: AtomicU64 = AtomicU64::new(0);

pub(crate) struct NativeEndpoint {
    endpoint: String,
    token: String,
}

impl NativeEndpoint {
    pub(crate) fn export_env(&self) {
        std::env::set_var("OWT_NATIVE_ENDPOINT", &self.endpoint);
        std::env::set_var("OWT_NATIVE_TOKEN", &self.token);
        std::env::set_var(
            "OWT_NATIVE_PROTOCOL",
            owt_control::PROTOCOL_VERSION.to_string(),
        );
        std::env::set_var("OWT_NATIVE_PID", std::process::id().to_string());
        std::env::set_var("OWT_CONTROL_PLANE", "native-endpoint-status");
        std::env::set_var("OWT_NATIVE_WINDOW_ONLY", "0");
        if let Err(err) = self.write_locator() {
            log::warn!("Unable to write OWT native endpoint locator: {err:#}");
        }
    }

    fn write_locator(&self) -> anyhow::Result<()> {
        let path = endpoint_locator_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("create OWT native endpoint locator directory")?;
        }
        let body = serde_json::to_vec_pretty(&json!({
            "product": "OWT",
            "endpoint": self.endpoint,
            "token": self.token,
            "protocol": owt_control::PROTOCOL_VERSION,
            "pid": std::process::id(),
            "created_at_ms": now_millis(),
        }))
        .context("serialize OWT native endpoint locator")?;
        let mut tmp_path = path.clone();
        tmp_path.set_extension("json.tmp");
        fs::write(&tmp_path, body).context("write OWT native endpoint locator temp file")?;
        fs::rename(&tmp_path, &path).context("replace OWT native endpoint locator")?;
        Ok(())
    }
}

impl Drop for NativeEndpoint {
    fn drop(&mut self) {
        let path = endpoint_locator_path();
        let Ok(content) = fs::read_to_string(&path) else {
            return;
        };
        let Ok(value) = serde_json::from_str::<Value>(&content) else {
            return;
        };
        if value.get("token").and_then(Value::as_str) == Some(self.token.as_str()) {
            let _ = fs::remove_file(path);
        }
    }
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn endpoint_locator_path() -> PathBuf {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data)
            .join("OWT")
            .join("native-endpoint.json");
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("native-endpoint.json"))
        })
        .unwrap_or_else(|| PathBuf::from("native-endpoint.json"))
}

pub(crate) fn should_start_for(cmd: Option<&SubCommand>) -> bool {
    matches!(
        cmd,
        None | Some(SubCommand::Start(_))
            | Some(SubCommand::BlockingStart(_))
            | Some(SubCommand::Ssh(_))
            | Some(SubCommand::Serial(_))
            | Some(SubCommand::Connect(_))
    )
}

pub(crate) fn start_endpoint() -> anyhow::Result<NativeEndpoint> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).context("bind OWT native endpoint")?;
    let addr = listener
        .local_addr()
        .context("resolve OWT native endpoint address")?;
    let endpoint = format!("http://127.0.0.1:{}", addr.port());
    let token = generate_token()?;
    let runtime = Arc::clone(&OWT_RUNTIME);
    let server_token = token.clone();

    std::thread::Builder::new()
        .name("owt-native-control".to_string())
        .spawn(move || run_server(listener, server_token, runtime))
        .context("spawn OWT native endpoint thread")?;

    Ok(NativeEndpoint { endpoint, token })
}

fn run_server(listener: TcpListener, token: String, runtime: Arc<Mutex<RuntimeState>>) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let token = token.clone();
                let runtime = Arc::clone(&runtime);
                std::thread::spawn(move || {
                    if let Err(err) = handle_connection(stream, &token, &runtime) {
                        log::trace!("OWT native endpoint request failed: {err:#}");
                    }
                });
            }
            Err(err) => log::trace!("OWT native endpoint accept failed: {err:#}"),
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    token: &str,
    runtime: &Arc<Mutex<RuntimeState>>,
) -> anyhow::Result<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .context("set OWT native endpoint read timeout")?;

    let request = read_http_request(&mut stream)?;
    let parsed = parse_http_request(&request)?;

    if !parsed.authorized(token) {
        return write_json_literal_response(
            &mut stream,
            401,
            r#"{"error":"unauthorized","message":"missing or invalid OWT native token"}"#,
        );
    }

    match (parsed.method, parsed.path_without_query()) {
        ("GET", "/" | "/owt/status" | "/status") => {
            let status = build_status(runtime)?;
            let body = serde_json::to_string(&status).context("serialize OWT native status")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/validate_interface" | "/validate_interface") => {
            let document = match parse_interface_body(&parsed)
                .and_then(|document| validate_interface_document(document))
            {
                Ok(document) => document,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let body = serde_json::to_string(&json!({
                "ok": true,
                "validated": true,
                "interface_id": &document.id,
                "scope": &document.scope,
                "native_rendering_ready": native_rendering_ready(),
                "message": "interface document is valid; native LCARS rendering is available when the interface is applied"
            }))
            .context("serialize OWT native validate response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/apply_interface" | "/apply_interface") => {
            let document = match parse_interface_body(&parsed) {
                Ok(document) => document,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let applied = match apply_interface(runtime, document) {
                Ok(applied) => applied,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "applied": applied,
                "native_rendering_ready": native_rendering_ready(),
                "message": "interface applied to native OWT runtime; native LCARS panel rendering is attached"
            }))
            .context("serialize OWT native apply response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("GET", "/owt/interfaces" | "/interfaces") => {
            let body = match list_interfaces_payload(runtime) {
                Ok(payload) => payload,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let body =
                serde_json::to_string(&body).context("serialize OWT native interfaces response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/save_interface" | "/save_interface") => {
            let request = match parse_save_interface_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let document = match request.document {
                Some(document) => match validate_interface_document(document) {
                    Ok(document) => document,
                    Err(err) => {
                        return write_json_error_response(&mut stream, 400, &err.to_string())
                    }
                },
                None => match active_interface_snapshot_from(runtime) {
                    Ok(Some(document)) => document,
                    Ok(None) => {
                        return write_json_error_response(
                            &mut stream,
                            400,
                            "save_interface requires an active interface or a document",
                        )
                    }
                    Err(err) => {
                        return write_json_error_response(&mut stream, 400, &err.to_string())
                    }
                },
            };
            let saved = match save_interface_document(request.id.as_deref(), &document) {
                Ok(saved) => saved,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let body = serde_json::to_string(&json!({
                "ok": true,
                "saved": true,
                "interface_id": &document.id,
                "scope": &document.scope,
                "store_id": saved.store_id,
                "path": saved.path.display().to_string(),
                "store_dir": interface_store_dir().display().to_string(),
                "message": "interface saved in the local OWT profile store"
            }))
            .context("serialize OWT native save response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/load_interface" | "/load_interface") => {
            let request = match parse_load_interface_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let loaded = match load_interface_document(&request.id) {
                Ok(loaded) => loaded,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let applied = match apply_interface(runtime, loaded.document.clone()) {
                Ok(applied) => applied,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "loaded": true,
                "applied": applied,
                "requested_id": request.id,
                "store_id": loaded.store_id,
                "interface_id": &loaded.document.id,
                "scope": &loaded.document.scope,
                "path": loaded.path.display().to_string(),
                "native_rendering_ready": native_rendering_ready(),
                "message": "interface loaded from the local OWT profile store and applied to the native runtime"
            }))
            .context("serialize OWT native load response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/update_node" | "/update_node") => {
            let request = match parse_update_node_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let node_id = request.node.id.clone();
            let parent_id = request.parent_id();
            let applied = match update_runtime_node(runtime, request) {
                Ok(applied) => applied,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "updated": true,
                "operation": "update_node",
                "node_id": node_id,
                "parent_id": parent_id,
                "applied": applied,
                "native_rendering_ready": native_rendering_ready(),
                "message": "node update applied atomically to native OWT runtime"
            }))
            .context("serialize OWT native update_node response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/dispatch_action" | "/dispatch_action") => {
            let request = match parse_dispatch_action_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let dispatched = match dispatch_runtime_action(runtime, &request.action_id) {
                Ok(dispatched) => dispatched,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "dispatched": dispatched,
                "native_rendering_ready": native_rendering_ready(),
                "message": "action dispatch recorded in native OWT runtime"
            }))
            .context("serialize OWT native dispatch response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/runtime" | "/runtime") | ("GET", "/owt/runtime" | "/runtime") => {
            let state = runtime
                .lock()
                .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
                .clone();
            let body =
                serde_json::to_string(&state).context("serialize OWT native runtime state")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("GET", _) | ("POST", _) => write_json_literal_response(
            &mut stream,
            404,
            r#"{"error":"not_found","message":"unknown OWT native endpoint path"}"#,
        ),
        _ => write_json_literal_response(
            &mut stream,
            405,
            r#"{"error":"method_not_allowed","message":"unsupported OWT native endpoint method"}"#,
        ),
    }
}

fn read_http_request(stream: &mut TcpStream) -> anyhow::Result<String> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];

    loop {
        let n = stream.read(&mut chunk).context("read OWT native request")?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);

        if buffer.len() > MAX_REQUEST_BYTES {
            return Err(anyhow!("OWT native request exceeded maximum size"));
        }

        if let Some(required_len) = required_http_request_len(&buffer)? {
            if buffer.len() >= required_len {
                break;
            }
        }
    }

    String::from_utf8(buffer).context("decode OWT native request")
}

fn required_http_request_len(buffer: &[u8]) -> anyhow::Result<Option<usize>> {
    let Some(header_end) = header_end_offset(buffer) else {
        return Ok(None);
    };
    let header = std::str::from_utf8(&buffer[..header_end]).context("decode OWT native headers")?;
    let mut content_length = 0_usize;
    for line in header.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value
                    .trim()
                    .parse::<usize>()
                    .context("parse OWT native content length")?;
            }
        }
    }
    Ok(Some(header_end + content_length))
}

fn header_end_offset(buffer: &[u8]) -> Option<usize> {
    find_subslice(buffer, b"\r\n\r\n")
        .map(|offset| offset + 4)
        .or_else(|| find_subslice(buffer, b"\n\n").map(|offset| offset + 2))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn build_status(runtime: &Arc<Mutex<RuntimeState>>) -> anyhow::Result<ControlStatus> {
    let state = runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?;
    let active_scope = state
        .active_interface_by_scope
        .values()
        .next()
        .and_then(|interface_id| state.interfaces.get(interface_id))
        .map(|document| document.scope.clone());
    let native_rendering_ready = active_scope.is_some();
    let mut status = ControlStatus::native_ready(active_scope);
    status.native_rendering_ready = native_rendering_ready;
    status.native_render_passes = OWT_RENDER_PASSES.load(Ordering::Relaxed);
    status.last_dispatched_action = state.last_dispatched_action.clone();
    status.message = if state.interfaces.is_empty() {
        "native OWT status endpoint is ready; no LCARS interface is applied yet".to_string()
    } else {
        format!(
            "native OWT runtime has {} applied interface(s); native LCARS panel rendering is attached",
            state.interfaces.len()
        )
    };
    Ok(status)
}

pub(crate) fn active_interface_snapshot() -> Option<InterfaceDocument> {
    let state = OWT_RUNTIME.lock().ok()?;
    state
        .active_interface_by_scope
        .values()
        .next()
        .and_then(|interface_id| state.interfaces.get(interface_id))
        .cloned()
}

fn active_interface_snapshot_from(
    runtime: &Arc<Mutex<RuntimeState>>,
) -> anyhow::Result<Option<InterfaceDocument>> {
    let state = runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?;
    Ok(state
        .active_interface_by_scope
        .values()
        .next()
        .and_then(|interface_id| state.interfaces.get(interface_id))
        .cloned())
}

pub(crate) fn last_dispatched_action_snapshot() -> Option<DispatchedAction> {
    OWT_RUNTIME
        .lock()
        .ok()
        .and_then(|state| state.last_dispatched_action.clone())
}

pub(crate) fn native_rendering_ready() -> bool {
    active_interface_snapshot().is_some()
}

pub(crate) fn record_lcars_render_pass() {
    OWT_RENDER_PASSES.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn dispatch_action(action_id: &str) -> anyhow::Result<DispatchedAction> {
    let dispatched = dispatch_runtime_action(&OWT_RUNTIME, action_id)?;
    invalidate_owt_windows();
    Ok(dispatched)
}

fn invalidate_owt_windows() {
    promise::spawn::spawn_into_main_thread(async {
        if let Some(front_end) = crate::frontend::try_front_end() {
            for gui_window in front_end.gui_windows() {
                gui_window
                    .window
                    .notify(crate::termwindow::TermWindowNotif::Apply(Box::new(
                        |term_window| {
                            term_window.reflow_owt_lcars_layout();
                        },
                    )));
            }
        }
    })
    .detach();
}

fn parse_interface_body(parsed: &ParsedHttpRequest) -> anyhow::Result<InterfaceDocument> {
    serde_json::from_str(&parsed.body).context("parse OWT native interface document")
}

#[derive(Debug)]
struct SaveInterfaceRequest {
    id: Option<String>,
    document: Option<InterfaceDocument>,
}

#[derive(Deserialize)]
struct LoadInterfaceRequest {
    id: String,
}

#[derive(Deserialize)]
struct UpdateNodeRequest {
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default)]
    parent_id: Option<String>,
    #[serde(default)]
    parent: Option<String>,
    node: UiNode,
    #[serde(default)]
    action: Option<UiAction>,
    #[serde(default)]
    replace: bool,
}

impl UpdateNodeRequest {
    fn parent_id(&self) -> Option<String> {
        self.parent_id
            .as_deref()
            .or(self.parent.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }
}

#[derive(Debug)]
struct SavedInterface {
    store_id: String,
    path: PathBuf,
}

#[derive(Debug)]
struct LoadedInterface {
    store_id: String,
    path: PathBuf,
    document: InterfaceDocument,
}

fn parse_save_interface_body(parsed: &ParsedHttpRequest) -> anyhow::Result<SaveInterfaceRequest> {
    let body = parsed.body.trim();
    if body.is_empty() {
        return Ok(SaveInterfaceRequest {
            id: None,
            document: None,
        });
    }

    let value: Value = serde_json::from_str(body).context("parse OWT native save request")?;
    let id = value
        .get("id")
        .or_else(|| value.get("store_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToString::to_string);

    if let Some(document_value) = value.get("document").or_else(|| value.get("interface")) {
        let document = serde_json::from_value(document_value.clone())
            .context("parse OWT native save request document")?;
        return Ok(SaveInterfaceRequest {
            id,
            document: Some(document),
        });
    }

    if looks_like_interface_document(&value) {
        let document =
            serde_json::from_value(value).context("parse OWT native interface document")?;
        return Ok(SaveInterfaceRequest {
            id: None,
            document: Some(document),
        });
    }

    Ok(SaveInterfaceRequest { id, document: None })
}

fn parse_load_interface_body(parsed: &ParsedHttpRequest) -> anyhow::Result<LoadInterfaceRequest> {
    let request: LoadInterfaceRequest =
        serde_json::from_str(&parsed.body).context("parse OWT native load request")?;
    if request.id.trim().is_empty() {
        return Err(anyhow!("load_interface requires id"));
    }
    Ok(LoadInterfaceRequest {
        id: request.id.trim().to_string(),
    })
}

fn parse_update_node_body(parsed: &ParsedHttpRequest) -> anyhow::Result<UpdateNodeRequest> {
    serde_json::from_str(&parsed.body).context("parse OWT native update_node request")
}

fn looks_like_interface_document(value: &Value) -> bool {
    value.get("schema_version").is_some() && value.get("nodes").is_some()
}

fn interface_store_dir() -> PathBuf {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data).join("OWT").join("interfaces");
    }

    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("interfaces")))
        .unwrap_or_else(|| PathBuf::from("interfaces"))
}

fn sanitize_interface_id(id: &str) -> String {
    let mut output = String::new();
    for ch in id.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }

    let trimmed = output
        .trim_matches(|ch| matches!(ch, '.' | '-' | '_'))
        .chars()
        .take(96)
        .collect::<String>();

    if trimmed.is_empty() {
        "interface".to_string()
    } else {
        trimmed
    }
}

fn interface_file_path(id: &str) -> (String, PathBuf) {
    let store_id = sanitize_interface_id(id);
    let path = interface_store_dir().join(format!("{store_id}.json"));
    (store_id, path)
}

fn save_interface_document(
    store_id: Option<&str>,
    document: &InterfaceDocument,
) -> anyhow::Result<SavedInterface> {
    validate_interface(document).map_err(|err| anyhow!("{err}"))?;
    let desired_id = store_id.unwrap_or(&document.id);
    let (store_id, path) = interface_file_path(desired_id);
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("OWT interface store path has no parent"))?;
    fs::create_dir_all(dir).context("create OWT interface store")?;
    let body = serde_json::to_vec_pretty(document).context("serialize OWT interface document")?;
    fs::write(&path, body).context("write OWT interface document")?;
    Ok(SavedInterface { store_id, path })
}

fn load_interface_document(id: &str) -> anyhow::Result<LoadedInterface> {
    let (store_id, path) = interface_file_path(id);
    let body = fs::read(&path).with_context(|| {
        format!(
            "read OWT interface document from local store id '{}'",
            store_id
        )
    })?;
    let document: InterfaceDocument =
        serde_json::from_slice(&body).context("parse saved OWT interface document")?;
    validate_interface(&document).map_err(|err| anyhow!("{err}"))?;
    Ok(LoadedInterface {
        store_id,
        path,
        document,
    })
}

fn list_interfaces_payload(runtime: &Arc<Mutex<RuntimeState>>) -> anyhow::Result<Value> {
    let runtime_interfaces = {
        let state = runtime
            .lock()
            .map_err(|_| anyhow!("OWT runtime lock poisoned"))?;
        state
            .interfaces
            .values()
            .map(|document| {
                json!({
                    "id": &document.id,
                    "title": &document.title,
                    "scope": &document.scope,
                    "theme": &document.theme,
                    "active": state
                        .active_interface_by_scope
                        .values()
                        .any(|interface_id| interface_id == &document.id),
                })
            })
            .collect::<Vec<_>>()
    };
    let saved_interfaces = list_saved_interfaces()?;

    Ok(json!({
        "ok": true,
        "store_dir": interface_store_dir().display().to_string(),
        "runtime": runtime_interfaces,
        "saved": saved_interfaces,
    }))
}

fn list_saved_interfaces() -> anyhow::Result<Vec<Value>> {
    let dir = interface_store_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).context("read OWT interface store"),
    };

    let mut saved = Vec::new();
    for entry in entries {
        let entry = entry.context("read OWT interface store entry")?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(store_id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        let metadata = entry.metadata().ok();
        let mut item = json!({
            "store_id": store_id,
            "path": path.display().to_string(),
            "bytes": metadata.as_ref().map(|metadata| metadata.len()).unwrap_or(0),
        });
        if let Ok(body) = fs::read(&path) {
            if let Ok(document) = serde_json::from_slice::<InterfaceDocument>(&body) {
                item["interface_id"] = json!(&document.id);
                item["title"] = json!(&document.title);
                item["scope"] = json!(&document.scope);
                item["theme"] = json!(&document.theme);
            }
        }
        saved.push(item);
    }

    saved.sort_by(|a, b| {
        let a = a
            .get("store_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let b = b
            .get("store_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        a.cmp(b)
    });
    Ok(saved)
}

#[derive(Deserialize)]
struct DispatchActionRequest {
    action_id: String,
}

fn parse_dispatch_action_body(parsed: &ParsedHttpRequest) -> anyhow::Result<DispatchActionRequest> {
    serde_json::from_str(&parsed.body).context("parse OWT native dispatch request")
}

fn validate_interface_document(document: InterfaceDocument) -> anyhow::Result<InterfaceDocument> {
    validate_interface(&document).map_err(|err| anyhow!("{err}"))?;
    Ok(document)
}

fn apply_interface(
    runtime: &Arc<Mutex<RuntimeState>>,
    document: InterfaceDocument,
) -> anyhow::Result<AppliedInterface> {
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .apply_interface(document)
        .map_err(|err| anyhow!("{err}"))
}

fn update_runtime_node(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: UpdateNodeRequest,
) -> anyhow::Result<AppliedInterface> {
    let interface_id = request.interface_id.clone();
    let scope = request.scope.clone();
    let parent_id = request.parent_id();
    let node = request.node;
    let action = request.action;
    let replace = request.replace;
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .update_node(
            interface_id.as_deref(),
            scope.as_ref(),
            parent_id.as_deref(),
            node,
            action,
            replace,
        )
        .map_err(|err| anyhow!("{err}"))
}

fn dispatch_runtime_action(
    runtime: &Arc<Mutex<RuntimeState>>,
    action_id: &str,
) -> anyhow::Result<DispatchedAction> {
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .dispatch_action(action_id)
        .map_err(|err| anyhow!("{err}"))
}

fn write_json_error_response(
    stream: &mut TcpStream,
    status_code: u16,
    message: &str,
) -> anyhow::Result<()> {
    let body = serde_json::to_string(&json!({
        "error": "bad_request",
        "message": message
    }))
    .context("serialize OWT native error response")?;
    write_json_literal_response(stream, status_code, &body)
}

fn write_json_literal_response(
    stream: &mut TcpStream,
    status_code: u16,
    body: &str,
) -> anyhow::Result<()> {
    let status_text = match status_code {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status_code} {status_text}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .context("write OWT native response")
}

fn generate_token() -> anyhow::Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).context("generate OWT native endpoint token")?;
    Ok(bytes_to_hex(&bytes))
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedHttpRequest<'a> {
    method: &'a str,
    path: &'a str,
    authorization: Option<String>,
    body: &'a str,
}

impl<'a> ParsedHttpRequest<'a> {
    fn authorized(&self, token: &str) -> bool {
        if let Some(value) = &self.authorization {
            if value.trim() == format!("Bearer {token}") {
                return true;
            }
        }

        self.query_param("token")
            .is_some_and(|value| value == token)
    }

    fn path_without_query(&self) -> &'a str {
        self.path
            .split_once('?')
            .map(|(path, _)| path)
            .unwrap_or(self.path)
    }

    fn query_param(&self, key: &str) -> Option<&'a str> {
        let (_, query) = self.path.split_once('?')?;
        for pair in query.split('&') {
            let (candidate, value) = pair.split_once('=')?;
            if candidate == key {
                return Some(value);
            }
        }
        None
    }
}

fn parse_http_request(request: &str) -> anyhow::Result<ParsedHttpRequest<'_>> {
    let (head, body) = request
        .split_once("\r\n\r\n")
        .or_else(|| request.split_once("\n\n"))
        .ok_or_else(|| anyhow!("missing OWT native HTTP header terminator"))?;
    let mut lines = head.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| anyhow!("missing OWT native request line"))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| anyhow!("missing OWT native request method"))?;
    let path = request_parts
        .next()
        .ok_or_else(|| anyhow!("missing OWT native request path"))?;

    let mut authorization = None;
    for line in lines {
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("authorization") {
                authorization = Some(value.trim().to_string());
            }
        }
    }

    Ok(ParsedHttpRequest {
        method,
        path,
        authorization,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        bytes_to_hex, parse_http_request, parse_save_interface_body, required_http_request_len,
        sanitize_interface_id,
    };

    #[test]
    fn parses_bearer_auth_status_request() {
        let parsed =
            parse_http_request("GET /owt/status HTTP/1.1\r\nAuthorization: Bearer abc123\r\n\r\n")
                .unwrap();

        assert_eq!(parsed.path_without_query(), "/owt/status");
        assert!(parsed.authorized("abc123"));
        assert!(!parsed.authorized("wrong"));
    }

    #[test]
    fn parses_query_token_status_request() {
        let parsed = parse_http_request("GET /status?token=abc123 HTTP/1.1\r\n\r\n").unwrap();

        assert_eq!(parsed.path_without_query(), "/status");
        assert!(parsed.authorized("abc123"));
    }

    #[test]
    fn parses_post_body() {
        let parsed = parse_http_request(
            "POST /owt/apply_interface HTTP/1.1\r\n\
             Authorization: Bearer abc123\r\n\
             Content-Type: application/json\r\n\
             Content-Length: 13\r\n\
             \r\n\
             {\"ok\":true}",
        )
        .unwrap();

        assert_eq!(parsed.method, "POST");
        assert_eq!(parsed.path_without_query(), "/owt/apply_interface");
        assert!(parsed.authorized("abc123"));
        assert_eq!(parsed.body, "{\"ok\":true}");
    }

    #[test]
    fn required_request_len_includes_declared_body() {
        let prefix = b"POST /x HTTP/1.1\r\nContent-Length: 5\r\n\r\n";
        assert_eq!(
            required_http_request_len(b"POST /x HTTP/1.1\r\nContent-Length: 5\r\n\r\nhelloextra")
                .unwrap(),
            Some(prefix.len() + 5)
        );
    }

    #[test]
    fn encodes_tokens_as_hex() {
        assert_eq!(bytes_to_hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    }

    #[test]
    fn sanitizes_interface_ids_for_local_store_paths() {
        assert_eq!(sanitize_interface_id("../Genetica Panel"), "Genetica_Panel");
        assert_eq!(sanitize_interface_id("***"), "interface");
    }

    #[test]
    fn save_request_accepts_document_wrapper() {
        let parsed = parse_http_request(
            "POST /owt/save_interface HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: 199\r\n\
             \r\n\
             {\"id\":\"demo.slot\",\"document\":{\"schema_version\":1,\"id\":\"demo.interface\",\"title\":\"DEMO\",\"scope\":{\"kind\":\"project\",\"id\":\"/tmp/demo\"},\"nodes\":[{\"id\":\"panel.root\",\"kind\":\"panel\"}]}}",
        )
        .unwrap();

        let request = parse_save_interface_body(&parsed).unwrap();
        assert_eq!(request.id.as_deref(), Some("demo.slot"));
        assert_eq!(request.document.unwrap().id, "demo.interface");
    }
}
