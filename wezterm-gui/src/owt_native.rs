use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context};
use once_cell::sync::Lazy;
use owt_control::{
    diff_interface_documents, interface_validation_warnings, reflow_interface_documents_for_scene,
    validate_interface, ActionKind, ActionProvenance, AppliedInterface, ControlStatus,
    DispatchedAction, EditedInterface, FocusedTableCell, FocusedTableGroup, FocusedTableRow,
    InterfaceDocument, InterfaceEditOperation, InterfaceLifecyclePatch, InterfaceLifecycleState,
    InterfaceOwner, InterfaceRuntimeStatus, LayoutPreview, PatchedInterfaceLayout,
    PatchedInterfaceLifecycle, PermissionDecision, ReplacedInterface, RequestApprovalState,
    RuntimeState, Scope, TableCellFocusMovement, TableFocusMovement, UiAction, UiNode, UiNodeKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use window::WindowOps;

use crate::SubCommand;

const MAX_REQUEST_BYTES: usize = 256 * 1024;
const INTERFACE_PACK_KIND: &str = "owt.interface_pack";
const INTERFACE_PACK_SCHEMA_VERSION: u16 = 1;

static OWT_RUNTIME: Lazy<Arc<Mutex<RuntimeState>>> =
    Lazy::new(|| Arc::new(Mutex::new(RuntimeState::default())));
static OWT_RENDER_PASSES: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub(crate) struct SavedInterfaceSummary {
    pub(crate) store_id: String,
    pub(crate) title: Option<String>,
    pub(crate) interface_id: Option<String>,
    pub(crate) owner_label: Option<String>,
    pub(crate) owner_key: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct LoadedSavedInterface {
    pub(crate) store_id: String,
    pub(crate) title: String,
    pub(crate) applied: AppliedInterface,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingPermissionRequestSummary {
    pub(crate) request_id: String,
    pub(crate) request_kind: String,
    pub(crate) interface_id: String,
    pub(crate) action_id: Option<String>,
    pub(crate) label: String,
    pub(crate) kind: Option<ActionKind>,
    pub(crate) target: Option<String>,
    pub(crate) source: Option<String>,
}

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

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
            let validation_warnings = interface_validation_warnings(&document);
            let warning_count = validation_warnings.len();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "validated": true,
                "interface_id": &document.id,
                "scope": &document.scope,
                "validation_warnings": validation_warnings,
                "warning_count": warning_count,
                "render_projection": render_projection_payload(&document),
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
            let (document, layout_override) = interface_document_with_layout_overrides(document);
            let tab_title = document.resolved_tab_title();
            let render_projection = render_projection_payload(&document);
            let validation_warnings = interface_validation_warnings(&document);
            let warning_count = validation_warnings.len();
            let applied = match apply_interface_without_layout_overrides(runtime, document) {
                Ok(applied) => applied,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            invalidate_owt_windows_with_tab_title(tab_title.clone());
            let body = serde_json::to_string(&json!({
                "ok": true,
                "applied": applied,
                "tab_title": tab_title,
                "render_projection": render_projection,
                "layout_override": layout_override,
                "validation_warnings": validation_warnings,
                "warning_count": warning_count,
                "native_rendering_ready": native_rendering_ready(),
                "message": "interface accepted by native OWT runtime; rendering is confirmed when per-interface render status advances"
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
            let validation_warnings = interface_validation_warnings(&document);
            let warning_count = validation_warnings.len();
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
                "validation_warnings": validation_warnings,
                "warning_count": warning_count,
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
            let tab_title = loaded.document.resolved_tab_title();
            let validation_warnings = interface_validation_warnings(&loaded.document);
            let warning_count = validation_warnings.len();
            let applied = match apply_interface(runtime, loaded.document.clone()) {
                Ok(applied) => applied,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            invalidate_owt_windows_with_tab_title(tab_title.clone());
            let body = serde_json::to_string(&json!({
                "ok": true,
                "loaded": true,
                "applied": applied,
                "tab_title": tab_title,
                "requested_id": request.id,
                "store_id": loaded.store_id,
                "interface_id": &loaded.document.id,
                "scope": &loaded.document.scope,
                "path": loaded.path.display().to_string(),
                "render_projection": render_projection_payload(&loaded.document),
                "validation_warnings": validation_warnings,
                "warning_count": warning_count,
                "native_rendering_ready": native_rendering_ready(),
                "message": "interface loaded from the local OWT profile store and accepted by the native runtime; rendering is confirmed when per-interface render status advances"
            }))
            .context("serialize OWT native load response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/export_interface" | "/export_interface") => {
            let request = match parse_export_interface_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let (document, source) = match resolve_export_interface_document(runtime, &request) {
                Ok(resolved) => resolved,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let pack = match export_interface_pack_from_document(&document, &request, &source) {
                Ok(pack) => pack,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let package_id = pack
                .get("manifest")
                .and_then(|manifest| manifest.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("owt-interface-pack")
                .to_string();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "exported": true,
                "package_id": package_id,
                "interface_id": &document.id,
                "scope": &document.scope,
                "source": source,
                "pack": pack,
                "message": "interface exported as an inert inline OWT interface pack"
            }))
            .context("serialize OWT native export_interface response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/import_interface" | "/import_interface") => {
            let request = match parse_import_interface_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let (document, permissions, source) =
                match import_interface_document_from_request(&request) {
                    Ok(imported) => imported,
                    Err(err) => {
                        return write_json_error_response(&mut stream, 400, &err.to_string())
                    }
                };
            let validation_warnings = interface_validation_warnings(&document);
            let warning_count = validation_warnings.len();

            let saved = if request.save() {
                match save_interface_document(request.store_id().as_deref(), &document) {
                    Ok(saved) => Some(json!({
                        "store_id": saved.store_id,
                        "path": saved.path.display().to_string(),
                    })),
                    Err(err) => {
                        return write_json_error_response(&mut stream, 400, &err.to_string())
                    }
                }
            } else {
                None
            };

            let applied = if request.apply() {
                let tab_title = document.resolved_tab_title();
                let applied = match apply_interface(runtime, document.clone()) {
                    Ok(applied) => applied,
                    Err(err) => {
                        return write_json_error_response(&mut stream, 400, &err.to_string())
                    }
                };
                invalidate_owt_windows_with_tab_title(tab_title.clone());
                Some(json!({
                    "applied": applied,
                    "tab_title": tab_title,
                    "render_projection": render_projection_payload(&document),
                    "native_rendering_ready": native_rendering_ready(),
                }))
            } else {
                None
            };

            let body = serde_json::to_string(&json!({
                "ok": true,
                "imported": true,
                "saved": saved,
                "applied": applied,
                "interface_id": &document.id,
                "scope": &document.scope,
                "source": source,
                "permissions": permissions,
                "validation_warnings": validation_warnings,
                "warning_count": warning_count,
                "message": "interface pack imported as inert semantic data; no actions or shell commands were executed"
            }))
            .context("serialize OWT native import_interface response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/replace_interface" | "/replace_interface") => {
            let request = match parse_replace_interface_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let replacement = match replace_runtime_interface(runtime, request) {
                Ok(replacement) => replacement,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let render_projection =
                render_projection_payload_for_interface(runtime, &replacement.applied.interface_id);
            let validation_warnings = validation_warnings_payload_for_interface(
                runtime,
                &replacement.applied.interface_id,
            );
            let warning_count = validation_warnings.as_array().map(Vec::len).unwrap_or(0);
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "replaced": replacement,
                "render_projection": render_projection,
                "validation_warnings": validation_warnings,
                "warning_count": warning_count,
                "native_rendering_ready": native_rendering_ready(),
                "message": "interface replaced in native OWT runtime"
            }))
            .context("serialize OWT native replace_interface response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/edit_interface" | "/edit_interface") => {
            let request = match parse_edit_interface_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let feedback_mode = request.feedback_mode();
            let repaint_mode = request.repaint_mode();
            let dry_run = request.dry_run;
            let edited = match edit_runtime_interface(runtime, &request) {
                Ok(edited) => edited,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            if !dry_run {
                invalidate_owt_windows();
            }

            let applied = edited.applied.clone();
            let interface_id = applied.interface_id.clone();
            let interface_id_for_feedback = interface_id.clone();
            let mut payload = json!({
                "ok": true,
                "edited": true,
                "operation": "edit_interface",
                "feedback_mode": feedback_mode,
                "repaint_mode": repaint_mode,
                "dry_run": dry_run,
                "interface_id": interface_id,
                "applied": applied,
                "operations": edited.operations,
                "message": if dry_run {
                    "interface edit sequence validated without mutating native runtime"
                } else {
                    "interface edit sequence accepted by native OWT runtime"
                }
            });
            if feedback_mode != "minimal" {
                let validation_warnings =
                    validation_warnings_payload_for_interface(runtime, &interface_id_for_feedback);
                let warning_count = validation_warnings.as_array().map(Vec::len).unwrap_or(0);
                payload["validation_warnings"] = validation_warnings;
                payload["warning_count"] = json!(warning_count);
                payload["render_projection"] = json!(render_projection_payload_for_interface(
                    runtime,
                    &interface_id_for_feedback
                ));
                payload["native_rendering_ready"] = json!(native_rendering_ready());
                payload["native_render_passes"] = json!(OWT_RENDER_PASSES.load(Ordering::Relaxed));
            }
            if matches!(feedback_mode.as_str(), "snapshot" | "full") {
                payload["snapshot_available"] = json!(false);
                payload["snapshot_message"] =
                    json!("native screenshot/snapshot capture is not wired to edit_interface yet");
            }
            if feedback_mode == "full" {
                payload["recent_events"] = recent_events_payload(runtime, 16);
            }

            let body = serde_json::to_string(&payload)
                .context("serialize OWT native edit_interface response")?;
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
            let validation_warnings =
                validation_warnings_payload_for_interface(runtime, &applied.interface_id);
            let warning_count = validation_warnings.as_array().map(Vec::len).unwrap_or(0);
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "updated": true,
                "operation": "update_node",
                "node_id": node_id,
                "parent_id": parent_id,
                "applied": applied,
                "validation_warnings": validation_warnings,
                "warning_count": warning_count,
                "native_rendering_ready": native_rendering_ready(),
                "message": "node update accepted atomically by native OWT runtime; rendering is confirmed when per-interface render status advances"
            }))
            .context("serialize OWT native update_node response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/bind_action" | "/bind_action") => {
            let request = match parse_bind_action_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let node_id = match request.node_id() {
                Some(node_id) => node_id,
                None => {
                    return write_json_error_response(
                        &mut stream,
                        400,
                        "bind_action requires node_id",
                    )
                }
            };
            let action = match request.bound_action() {
                Ok(action) => action,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let action_id = action.id.clone();
            let applied = match bind_runtime_action(runtime, request, &node_id, action) {
                Ok(applied) => applied,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let validation_warnings =
                validation_warnings_payload_for_interface(runtime, &applied.interface_id);
            let warning_count = validation_warnings.as_array().map(Vec::len).unwrap_or(0);
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "bound": true,
                "operation": "bind_action",
                "node_id": node_id,
                "action_id": action_id,
                "applied": applied,
                "validation_warnings": validation_warnings,
                "warning_count": warning_count,
                "native_rendering_ready": native_rendering_ready(),
                "message": "action binding accepted atomically by native OWT runtime; rendering is confirmed when per-interface render status advances"
            }))
            .context("serialize OWT native bind_action response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        (
            "POST",
            "/owt/preview_reflow" | "/preview_reflow" | "/owt/validate_layout" | "/validate_layout",
        ) => {
            let request = match parse_patch_interface_layout_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let preview = match preview_runtime_layout(runtime, request) {
                Ok(preview) => preview,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let layout_scene_plan = preview.scene_plan.clone();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "previewed": true,
                "operation": "preview_reflow",
                "render_projection": render_projection_payload_for_layout_preview(runtime, &preview),
                "layout_scene_plan": layout_scene_plan,
                "preview": preview,
                "native_rendering_ready": native_rendering_ready(),
                "message": "layout preview computed without mutating the native OWT interface"
            }))
            .context("serialize OWT native preview_reflow response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/patch_interface_layout" | "/patch_interface_layout") => {
            let request = match parse_patch_interface_layout_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let patched = match patch_runtime_layout(runtime, request) {
                Ok(patched) => patched,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let render_projection =
                render_projection_payload_for_interface(runtime, &patched.applied.interface_id);
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "patched": true,
                "operation": "patch_interface_layout",
                "node_id": patched.node_id,
                "prior_properties": patched.prior_properties,
                "applied_properties": patched.applied_properties,
                "render_projection": render_projection,
                "applied": patched.applied,
                "native_rendering_ready": native_rendering_ready(),
                "message": "interface layout intent patched atomically in native OWT runtime; rendering is confirmed when per-interface render status advances"
            }))
            .context("serialize OWT native patch_interface_layout response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/patch_interface_lifecycle" | "/patch_interface_lifecycle") => {
            let request = match parse_patch_interface_lifecycle_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let patched = match patch_runtime_lifecycle(runtime, request) {
                Ok(patched) => patched,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let body = serde_json::to_string(&json!({
                "ok": true,
                "operation": "patch_interface_lifecycle",
                "patched": patched,
                "native_rendering_ready": native_rendering_ready(),
                "message": "interface lifecycle intent patched in native OWT runtime"
            }))
            .context("serialize OWT native patch_interface_lifecycle response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        (
            "POST",
            "/owt/retire_interface"
            | "/retire_interface"
            | "/owt/remove_interface"
            | "/remove_interface",
        ) => {
            let request = match parse_retire_interface_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let remove_saved = request.remove_saved();
            let store_id = request.store_id();
            let retired = match retire_runtime_interface(runtime, &request) {
                Ok(retired) => retired,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let removed_saved = if remove_saved {
                match delete_saved_interface_document(
                    store_id.as_deref().unwrap_or(retired.interface_id.as_str()),
                ) {
                    Ok(path) => path.map(|path| path.display().to_string()),
                    Err(err) => {
                        return write_json_error_response(&mut stream, 400, &err.to_string())
                    }
                }
            } else {
                None
            };
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "retired": retired,
                "removed_saved_path": removed_saved,
                "native_rendering_ready": native_rendering_ready(),
                "message": "interface retired from the native OWT runtime"
            }))
            .context("serialize OWT native retire_interface response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/request_refresh" | "/request_refresh") => {
            let request = match parse_request_refresh_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let requested = match request_runtime_refresh(runtime, &request) {
                Ok(requested) => requested,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "refresh_requested": requested,
                "approval_required": requested.approval_required,
                "approval_state": requested.approval_state,
                "native_rendering_ready": native_rendering_ready(),
                "message": if requested.approval_required {
                    "refresh execution intent recorded as pending terminal-owned approval; external execution is not implicit"
                } else {
                    "refresh intent recorded in native OWT runtime; external execution is not implicit"
                }
            }))
            .context("serialize OWT native request_refresh response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        (
            "GET" | "POST",
            "/owt/action_requests"
            | "/action_requests"
            | "/owt/list_action_requests"
            | "/list_action_requests",
        ) => {
            let request = match parse_list_action_requests_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let body = match list_action_requests_payload(runtime, &request).and_then(|payload| {
                serde_json::to_string(&payload)
                    .context("serialize OWT native list_action_requests response")
            }) {
                Ok(body) => body,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            write_json_literal_response(&mut stream, 200, &body)
        }
        (
            "POST",
            "/owt/diff_interface"
            | "/diff_interface"
            | "/owt/compare_snapshot"
            | "/compare_snapshot",
        ) => {
            let request = match parse_diff_interface_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let (left, right, left_source, right_source) =
                match resolve_diff_documents(runtime, &request) {
                    Ok(resolved) => resolved,
                    Err(err) => {
                        return write_json_error_response(&mut stream, 400, &err.to_string())
                    }
                };
            let diff = diff_interface_documents(&left, &right);
            let body = serde_json::to_string(&json!({
                "ok": true,
                "diff": diff,
                "left_source": left_source,
                "right_source": right_source,
                "message": "semantic interface diff computed without mutating native OWT runtime"
            }))
            .context("serialize OWT native diff_interface response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        ("POST", "/owt/dispatch_action" | "/dispatch_action") => {
            let request = match parse_dispatch_action_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let dispatched = match dispatch_runtime_action(
                runtime,
                request.interface_id.as_deref(),
                &request.action_id,
                false,
                false,
                request.external_execution_requested(),
                request.provenance(),
            ) {
                Ok(dispatched) => dispatched,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            invalidate_owt_windows();
            let body = serde_json::to_string(&json!({
                "ok": true,
                "dispatched": dispatched,
                "approval_required": dispatched.approval_required,
                "approval_state": dispatched.approval_state,
                "native_rendering_ready": native_rendering_ready(),
                "message": if dispatched.approval_required {
                    "action dispatch and external execution intent recorded as pending terminal-owned approval; native OWT did not run commands"
                } else {
                    "action dispatch recorded in native OWT runtime"
                }
            }))
            .context("serialize OWT native dispatch response")?;
            write_json_literal_response(&mut stream, 200, &body)
        }
        (
            "POST",
            "/owt/execute_action_request" | "/execute_action_request" | "/owt/execute_action",
        ) => {
            let request = match parse_execute_action_request_body(&parsed) {
                Ok(request) => request,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            let payload = match execute_approved_action_request(runtime, &request) {
                Ok(payload) => payload,
                Err(err) => return write_json_error_response(&mut stream, 400, &err.to_string()),
            };
            invalidate_owt_windows();
            let body = serde_json::to_string(&payload)
                .context("serialize OWT native execute_action_request response")?;
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
    let active_interfaces = active_interface_snapshots_from_state(&state);
    let active_scope = active_interfaces
        .first()
        .map(|document| document.scope.clone());
    let native_rendering_ready = native_rendering_ready_from_state(&state, &active_interfaces);
    let mut status = ControlStatus::native_ready(active_scope);
    status.native_rendering_ready = native_rendering_ready;
    status.native_render_passes = OWT_RENDER_PASSES.load(Ordering::Relaxed);
    status.last_dispatched_action = state.last_dispatched_action.clone();
    status.last_dispatched_action_by_interface = state.last_dispatched_action_by_interface.clone();
    status.interface_statuses = interface_statuses_from_state(&state, &active_interfaces);
    status.layout_scene_plan = Some(state.layout_scene_plan());
    status.recent_events = state.recent_events(24);
    status.recent_action_requests = state.recent_action_requests(None, 24);
    status.recent_refresh_requests = state.recent_refresh_requests(None, 24);
    status.message = if state.interfaces.is_empty() {
        "native OWT status endpoint is ready; no LCARS interface is applied yet".to_string()
    } else {
        format!(
            "native OWT runtime has {} applied interface(s), {} active surface(s); native LCARS panel rendering is attached",
            state.interfaces.len(),
            active_interfaces.len()
        )
    };
    Ok(status)
}

pub(crate) fn active_interface_snapshots() -> Vec<InterfaceDocument> {
    let Ok(state) = OWT_RUNTIME.lock() else {
        return Vec::new();
    };
    active_interface_snapshots_from_state(&state)
}

pub(crate) fn pending_permission_requests() -> Vec<PendingPermissionRequestSummary> {
    let Ok(state) = OWT_RUNTIME.lock() else {
        return Vec::new();
    };
    let mut requests = Vec::new();
    for request in &state.action_requests {
        if request.approval_state != RequestApprovalState::Pending {
            continue;
        }
        let Some(request_id) = request.request_id.clone() else {
            continue;
        };
        let interface_id = request.interface_id.clone().unwrap_or_default();
        requests.push(PendingPermissionRequestSummary {
            request_id,
            request_kind: "action".to_string(),
            interface_id,
            action_id: Some(request.action_id.clone()),
            label: request.label.clone(),
            kind: Some(request.kind.clone()),
            target: request.target.clone().or_else(|| request.command.clone()),
            source: request.source.clone(),
        });
    }
    for request in &state.refresh_requests {
        if request.approval_state != RequestApprovalState::Pending {
            continue;
        }
        let Some(request_id) = request.request_id.clone() else {
            continue;
        };
        requests.push(PendingPermissionRequestSummary {
            request_id,
            request_kind: "refresh".to_string(),
            interface_id: request.interface_id.clone(),
            action_id: request.action_id.clone(),
            label: request
                .label
                .clone()
                .unwrap_or_else(|| "Refresh".to_string()),
            kind: request.kind.clone(),
            target: request.target.clone().or_else(|| request.command.clone()),
            source: request.source.clone(),
        });
    }
    requests
}

fn active_interface_snapshot_from(
    runtime: &Arc<Mutex<RuntimeState>>,
) -> anyhow::Result<Option<InterfaceDocument>> {
    let state = runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?;
    Ok(active_interface_snapshots_from_state(&state)
        .into_iter()
        .next())
}

fn active_interface_snapshots_from_state(state: &RuntimeState) -> Vec<InterfaceDocument> {
    let mut documents = Vec::new();
    let now_unix = current_unix_seconds();
    for interface_id in state.active_interface_ids_for_scene() {
        let lifecycle = state
            .lifecycle_by_interface
            .get(&interface_id)
            .map(|lifecycle| effective_lifecycle_state(lifecycle, now_unix));
        if lifecycle
            .as_ref()
            .map(|lifecycle| lifecycle.hidden || lifecycle.expired)
            .unwrap_or(false)
        {
            continue;
        }
        if let Some(document) = state.interfaces.get(&interface_id) {
            let mut document = document.clone();
            if let Some(lifecycle) = lifecycle.as_ref() {
                inject_lifecycle_overlay(&mut document, lifecycle, now_unix);
            }
            inject_staleness_overlay(&mut document, now_unix);
            documents.push(document);
        }
    }
    reflow_interface_documents_for_scene(&mut documents);
    documents
}

fn effective_lifecycle_state(
    lifecycle: &InterfaceLifecycleState,
    now_unix: u64,
) -> InterfaceLifecycleState {
    let mut lifecycle = lifecycle.clone();
    if lifecycle
        .expires_at_unix
        .map(|expires_at_unix| expires_at_unix <= now_unix)
        .unwrap_or(false)
    {
        lifecycle.expired = true;
    }
    lifecycle
}

fn inject_lifecycle_overlay(
    document: &mut InterfaceDocument,
    lifecycle: &InterfaceLifecycleState,
    now_unix: u64,
) {
    if !lifecycle_has_visible_metadata(lifecycle) {
        return;
    }
    let Some(root) = document.nodes.first_mut() else {
        return;
    };

    root.properties
        .insert("lifecycle_pinned".to_string(), lifecycle.pinned.to_string());
    root.properties.insert(
        "lifecycle_state".to_string(),
        lifecycle_state_value(lifecycle, now_unix).to_string(),
    );
    if let Some(expires_at_unix) = lifecycle.expires_at_unix {
        root.properties.insert(
            "lifecycle_expires_at_unix".to_string(),
            expires_at_unix.to_string(),
        );
        root.properties.insert(
            "lifecycle_remaining_seconds".to_string(),
            expires_at_unix.saturating_sub(now_unix).to_string(),
        );
    }
    if let Some(ttl_seconds) = lifecycle.ttl_seconds {
        root.properties
            .insert("lifecycle_ttl_seconds".to_string(), ttl_seconds.to_string());
    }

    let mut badge = UiNode::new("owt.lifecycle.status", UiNodeKind::Badge);
    badge.label = Some("LIFECYCLE".to_string());
    badge.text = Some(lifecycle_badge_text(lifecycle, now_unix));
    badge.role = Some("lifecycle".to_string());
    badge.properties.insert(
        "state".to_string(),
        lifecycle_state_value(lifecycle, now_unix).to_string(),
    );
    badge
        .properties
        .insert("ephemeral".to_string(), "true".to_string());
    root.children.push(badge);
}

fn lifecycle_has_visible_metadata(lifecycle: &InterfaceLifecycleState) -> bool {
    lifecycle.pinned
        || lifecycle.hidden
        || lifecycle.expired
        || lifecycle.expires_at_unix.is_some()
        || lifecycle.ttl_seconds.is_some()
        || lifecycle
            .message
            .as_deref()
            .map(|message| !message.trim().is_empty())
            .unwrap_or(false)
}

fn lifecycle_state_value(lifecycle: &InterfaceLifecycleState, now_unix: u64) -> &'static str {
    if lifecycle.expired {
        "expired"
    } else if lifecycle.hidden {
        "hidden"
    } else if lifecycle
        .expires_at_unix
        .map(|expires_at_unix| expires_at_unix.saturating_sub(now_unix) <= 30)
        .unwrap_or(false)
    {
        "expiring"
    } else if lifecycle.expires_at_unix.is_some() || lifecycle.ttl_seconds.is_some() {
        "ttl"
    } else if lifecycle.pinned {
        "pinned"
    } else {
        "active"
    }
}

fn lifecycle_badge_text(lifecycle: &InterfaceLifecycleState, now_unix: u64) -> String {
    let mut parts = Vec::new();
    if lifecycle.pinned {
        parts.push("PINNED".to_string());
    }
    if lifecycle.expired {
        parts.push("EXPIRED".to_string());
    } else if let Some(expires_at_unix) = lifecycle.expires_at_unix {
        parts.push(format!(
            "TTL {}",
            compact_lifecycle_duration(expires_at_unix.saturating_sub(now_unix))
        ));
    } else if let Some(ttl_seconds) = lifecycle.ttl_seconds {
        parts.push(format!("TTL {}", compact_lifecycle_duration(ttl_seconds)));
    }
    if let Some(message) = lifecycle
        .message
        .as_deref()
        .filter(|message| !message.trim().is_empty())
    {
        parts.push(message.trim().to_ascii_uppercase());
    }
    if parts.is_empty() {
        parts.push("ACTIVE".to_string());
    }
    parts.join(" / ")
}

fn compact_lifecycle_duration(seconds: u64) -> String {
    if seconds >= 86_400 {
        format!("{}D {}H", seconds / 86_400, (seconds % 86_400) / 3_600)
    } else if seconds >= 3_600 {
        format!("{}H {}M", seconds / 3_600, (seconds % 3_600) / 60)
    } else if seconds >= 60 {
        format!("{}M {}S", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}S")
    }
}

fn inject_staleness_overlay(document: &mut InterfaceDocument, now_unix: u64) {
    let Some(root) = document.nodes.first_mut() else {
        return;
    };
    let Some(collected_at_unix) = numeric_root_property(root, &["collected_at_unix"]) else {
        return;
    };
    let Some(stale_after_seconds) =
        numeric_root_property(root, &["stale_after_seconds", "refresh_interval_seconds"])
    else {
        return;
    };
    if stale_after_seconds == 0 {
        return;
    }

    let age_seconds = now_unix.saturating_sub(collected_at_unix);
    root.properties
        .insert("freshness_age_seconds".to_string(), age_seconds.to_string());
    root.properties.insert(
        "freshness_stale_after_seconds".to_string(),
        stale_after_seconds.to_string(),
    );

    if age_seconds <= stale_after_seconds {
        root.properties
            .insert("freshness_state".to_string(), "fresh".to_string());
        return;
    }

    root.properties
        .insert("freshness_state".to_string(), "stale".to_string());
    let mut badge = UiNode::new("owt.freshness.status", UiNodeKind::Badge);
    badge.label = Some("FRESHNESS".to_string());
    badge.text = Some(format!(
        "STALE {} / LIMIT {}",
        compact_lifecycle_duration(age_seconds),
        compact_lifecycle_duration(stale_after_seconds)
    ));
    badge.role = Some("freshness".to_string());
    badge
        .properties
        .insert("state".to_string(), "stale".to_string());
    badge
        .properties
        .insert("ephemeral".to_string(), "true".to_string());
    root.children.push(badge);
}

fn numeric_root_property(root: &UiNode, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        root.properties
            .get(*key)
            .and_then(|value| value.trim().parse::<u64>().ok())
    })
}

fn interface_statuses_from_state(
    state: &RuntimeState,
    active_interfaces: &[InterfaceDocument],
) -> Vec<InterfaceRuntimeStatus> {
    let active_ids = active_interfaces
        .iter()
        .map(|document| document.id.as_str())
        .collect::<BTreeSet<_>>();
    let active_documents_by_id = active_interfaces
        .iter()
        .map(|document| (document.id.as_str(), document))
        .collect::<BTreeMap<_, _>>();
    state
        .interfaces
        .values()
        .map(|document| {
            let projection_document = active_documents_by_id
                .get(document.id.as_str())
                .copied()
                .unwrap_or(document);
            InterfaceRuntimeStatus {
                interface_id: document.id.clone(),
                title: document.title.clone(),
                scope: document.scope.clone(),
                owner: Some(InterfaceOwner::from_scope(&document.scope)),
                accepted: true,
                active: active_ids.contains(document.id.as_str()),
                native_render_passes: state
                    .last_render_pass_by_interface
                    .get(&document.id)
                    .copied()
                    .unwrap_or(0),
                render_projection: render_projection_status_map(projection_document),
                last_dispatched_action: state.last_dispatched_action_for_interface(&document.id),
                lifecycle: state
                    .lifecycle_by_interface
                    .get(&document.id)
                    .map(|lifecycle| effective_lifecycle_state(lifecycle, current_unix_seconds())),
                focused_table_cell: focused_table_cell_status(document),
            }
        })
        .collect()
}

fn focused_table_cell_status(document: &InterfaceDocument) -> Option<FocusedTableCell> {
    document
        .nodes
        .iter()
        .find_map(|node| focused_table_cell_status_in_node(document, node))
}

fn focused_table_cell_status_in_node(
    document: &InterfaceDocument,
    node: &UiNode,
) -> Option<FocusedTableCell> {
    if node.kind == UiNodeKind::Table {
        if let Some(column) = focused_column_from_table(node) {
            let columns = status_table_columns(node);
            let column_index = columns
                .iter()
                .position(|candidate| candidate.eq_ignore_ascii_case(&column))
                .unwrap_or(0);
            let focused_column = columns.get(column_index).cloned().unwrap_or(column);
            return Some(FocusedTableCell {
                interface_id: document.id.clone(),
                scope: document.scope.clone(),
                table_node_id: node.id.clone(),
                focused_column,
                column_index,
                movement: None,
            });
        }
    }
    node.children
        .iter()
        .find_map(|child| focused_table_cell_status_in_node(document, child))
}

fn focused_column_from_table(node: &UiNode) -> Option<String> {
    for key in [
        "focused_column",
        "focus_column",
        "selected_column",
        "focused_cell",
        "focus_cell",
        "selected_cell",
    ] {
        if let Some(value) = node.properties.get(key) {
            let raw = value.trim();
            if !raw.is_empty() {
                return Some(raw.to_string());
            }
        }
    }
    None
}

fn status_table_columns(node: &UiNode) -> Vec<String> {
    node.properties
        .get("columns")
        .map(|value| split_status_table_cells(value))
        .unwrap_or_default()
}

fn split_status_table_cells(source: &str) -> Vec<String> {
    let delimiter = if source.contains('|') {
        '|'
    } else if source.contains('\t') {
        '\t'
    } else {
        ','
    };
    source
        .split(delimiter)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn native_rendering_ready_from_state(
    state: &RuntimeState,
    active_interfaces: &[InterfaceDocument],
) -> bool {
    active_interfaces.iter().any(|document| {
        state
            .last_render_pass_by_interface
            .get(&document.id)
            .copied()
            .unwrap_or(0)
            > 0
    })
}

pub(crate) fn last_dispatched_action_for_interface(interface_id: &str) -> Option<DispatchedAction> {
    OWT_RUNTIME
        .lock()
        .ok()
        .and_then(|state| state.last_dispatched_action_for_interface(interface_id))
}

pub(crate) fn native_rendering_ready() -> bool {
    let Ok(state) = OWT_RUNTIME.lock() else {
        return false;
    };
    let active_interfaces = active_interface_snapshots_from_state(&state);
    native_rendering_ready_from_state(&state, &active_interfaces)
}

fn render_projection_payload(document: &InterfaceDocument) -> Value {
    serde_json::to_value(
        crate::termwindow::render::owt_lcars::describe_lcars_render_projection(document),
    )
    .unwrap_or_else(|err| {
        json!({
            "renderer_path": "unknown",
            "error": err.to_string(),
        })
    })
}

fn render_projection_status_map(document: &InterfaceDocument) -> BTreeMap<String, String> {
    let projection =
        crate::termwindow::render::owt_lcars::describe_lcars_render_projection(document);
    let mut values = BTreeMap::new();
    values.insert("visible".to_string(), projection.visible.to_string());
    values.insert(
        "renderer_path".to_string(),
        projection.renderer_path.to_string(),
    );
    values.insert("layout".to_string(), projection.layout.to_string());
    values.insert("origin".to_string(), projection.origin.to_string());
    values.insert(
        "reservation".to_string(),
        projection.reservation.to_string(),
    );
    values.insert(
        "orientation".to_string(),
        projection.orientation.to_string(),
    );
    values.insert(
        "reserves_terminal_space".to_string(),
        projection.reserves_terminal_space.to_string(),
    );
    values.insert(
        "presentation_tier".to_string(),
        projection.presentation_tier.to_string(),
    );
    values.insert(
        "solved_edge".to_string(),
        projection.solved_edge.to_string(),
    );
    values.insert(
        "reserved_terminal".to_string(),
        projection.reserved_terminal.to_string(),
    );
    values.insert(
        "stable_from_previous".to_string(),
        projection.stable_from_previous.to_string(),
    );
    values.insert(
        "occlusion_risk".to_string(),
        projection.occlusion_risk.to_string(),
    );
    if let Some(label_fit_state) = projection.label_fit_state {
        values.insert("label_fit_state".to_string(), label_fit_state);
    }
    if let Some(collapse_reason) = projection.collapse_reason {
        values.insert("collapse_reason".to_string(), collapse_reason);
    }
    if let Some(anchor) = projection.anchor {
        values.insert("anchor".to_string(), anchor);
    }
    if let Some(z_order) = projection.z_order {
        values.insert("z_order".to_string(), z_order.to_string());
    }
    if let Some(priority) = projection.priority {
        values.insert("priority".to_string(), priority.to_string());
    }
    if let Some(min_terminal_cells) = projection.min_terminal_cells {
        values.insert("min_terminal_cells".to_string(), min_terminal_cells);
    }
    if let Some(collapse_policy) = projection.collapse_policy {
        values.insert("collapse_policy".to_string(), collapse_policy);
    }
    if let Some(floating_anchor) = projection.floating_anchor {
        values.insert("floating_anchor".to_string(), floating_anchor);
    }
    if let Some(floating_x) = projection.floating_x {
        values.insert("floating_x".to_string(), floating_x);
    }
    if let Some(floating_y) = projection.floating_y {
        values.insert("floating_y".to_string(), floating_y);
    }
    if let Some(floating_width) = projection.floating_width {
        values.insert("floating_width".to_string(), floating_width);
    }
    if let Some(floating_height) = projection.floating_height {
        values.insert("floating_height".to_string(), floating_height);
    }
    if let Some(structural_profile) = projection.structural_profile {
        values.insert(
            "structural_profile".to_string(),
            structural_profile.to_string(),
        );
    }
    if let Some(requested_profile) = projection.requested_profile {
        values.insert("requested_profile".to_string(), requested_profile);
    }
    if let Some(profile_family) = projection.profile_family {
        values.insert("profile_family".to_string(), profile_family);
    }
    if let Some(table_density) = projection.table_density {
        values.insert("table_density".to_string(), table_density);
    }
    if let Some(cohort_key) = projection.cohort_key {
        values.insert("cohort_key".to_string(), cohort_key);
    }
    if let Some(state_key) = projection.state_key {
        values.insert("state_key".to_string(), state_key);
    }
    if let Some(severity_key) = projection.severity_key {
        values.insert("severity_key".to_string(), severity_key);
    }
    if let Some(lifecycle_controls) = projection.lifecycle_controls {
        values.insert("lifecycle_controls".to_string(), lifecycle_controls);
    }
    if let Some(action_roles) = projection.action_roles {
        values.insert("action_roles".to_string(), action_roles);
    }
    if let Some(refresh_policy) = projection.refresh_policy {
        values.insert("refresh_policy".to_string(), refresh_policy);
    }
    if let Some(drilldown_policy) = projection.drilldown_policy {
        values.insert("drilldown_policy".to_string(), drilldown_policy);
    }
    values
}

fn render_projection_payload_for_interface(
    runtime: &Arc<Mutex<RuntimeState>>,
    interface_id: &str,
) -> Option<Value> {
    let state = runtime.lock().ok()?;
    state
        .interfaces
        .get(interface_id)
        .map(render_projection_payload)
}

fn validation_warnings_payload_for_interface(
    runtime: &Arc<Mutex<RuntimeState>>,
    interface_id: &str,
) -> Value {
    runtime
        .lock()
        .ok()
        .and_then(|state| state.interfaces.get(interface_id).cloned())
        .map(|document| json!(interface_validation_warnings(&document)))
        .unwrap_or_else(|| json!([]))
}

fn recent_events_payload(runtime: &Arc<Mutex<RuntimeState>>, limit: usize) -> Value {
    runtime
        .lock()
        .ok()
        .map(|state| json!(state.recent_events(limit)))
        .unwrap_or_else(|| json!([]))
}

fn render_projection_payload_for_layout_preview(
    runtime: &Arc<Mutex<RuntimeState>>,
    preview: &LayoutPreview,
) -> Option<Value> {
    let state = runtime.lock().ok()?;
    let mut document = state.interfaces.get(&preview.target.interface_id)?.clone();
    if let Some(node) = find_preview_node_mut(&mut document.nodes, &preview.node_id) {
        node.properties = preview.preview_properties.clone();
    }
    Some(render_projection_payload(&document))
}

fn find_preview_node_mut<'a>(nodes: &'a mut [UiNode], node_id: &str) -> Option<&'a mut UiNode> {
    for node in nodes {
        if node.id == node_id {
            return Some(node);
        }
        if let Some(child) = find_preview_node_mut(&mut node.children, node_id) {
            return Some(child);
        }
    }
    None
}

pub(crate) fn record_lcars_render_pass(interface_ids: &[String]) {
    let render_pass = OWT_RENDER_PASSES.fetch_add(1, Ordering::Relaxed) + 1;
    if let Ok(mut state) = OWT_RUNTIME.lock() {
        state.record_rendered_interfaces(interface_ids, render_pass);
    }
}

pub(crate) fn patch_interface_layout_properties(
    interface_id: Option<&str>,
    properties: BTreeMap<String, String>,
) -> anyhow::Result<PatchedInterfaceLayout> {
    let requested_properties = properties.clone();
    let patched = OWT_RUNTIME
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .patch_interface_layout(interface_id, None, None, properties)
        .map_err(|err| anyhow!("{err}"))?;
    if let Err(err) = persist_layout_override_from_patch(&patched, &requested_properties) {
        log::warn!(
            "OWT LCARS layout override persistence failed for {}: {err:#}",
            patched.applied.interface_id
        );
    }
    invalidate_owt_windows();
    Ok(patched)
}

pub(crate) fn saved_interface_summaries() -> anyhow::Result<Vec<SavedInterfaceSummary>> {
    list_saved_interfaces()?
        .into_iter()
        .map(saved_interface_summary_from_value)
        .collect()
}

pub(crate) fn load_saved_interface(id: &str) -> anyhow::Result<LoadedSavedInterface> {
    let loaded = load_interface_document(id)?;
    let title = loaded.document.title.clone();
    let tab_title = loaded.document.resolved_tab_title();
    let applied = apply_interface(&OWT_RUNTIME, loaded.document)?;
    invalidate_owt_windows_with_tab_title(tab_title);
    Ok(LoadedSavedInterface {
        store_id: loaded.store_id,
        title,
        applied,
    })
}

pub(crate) fn dispatch_action(
    interface_id: Option<&str>,
    action_id: &str,
) -> anyhow::Result<DispatchedAction> {
    let native_execution = renderer_native_execution_candidate(interface_id, action_id)?;
    let external_execution_requested = native_execution.is_none()
        && renderer_action_requests_external_execution(interface_id, action_id)?;
    let permission_granted = external_execution_requested;
    let mut dispatched = dispatch_runtime_action(
        &OWT_RUNTIME,
        interface_id,
        action_id,
        permission_granted,
        false,
        external_execution_requested,
        ActionProvenance {
            source: Some("native_renderer:surface_action".to_string()),
            requested_at_unix: Some(current_unix_seconds()),
            ..ActionProvenance::default()
        },
    )?;
    if let Some(native_execution) = native_execution {
        let outcome = execute_native_terminal_action(
            &native_execution.action,
            native_execution.root_allowlisted,
        );
        dispatched = OWT_RUNTIME
            .lock()
            .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
            .record_action_execution_result(
                &native_execution.interface_id,
                &native_execution.action.id,
                outcome.executed,
                outcome.status,
                outcome.message,
            )
            .map_err(|err| anyhow!("{err}"))?;
    }
    invalidate_owt_windows();
    Ok(dispatched)
}

#[derive(Clone, Debug)]
struct RendererNativeExecution {
    interface_id: String,
    action: UiAction,
    root_allowlisted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeExecutionOutcome {
    executed: bool,
    status: String,
    message: String,
}

fn renderer_native_execution_candidate(
    interface_id: Option<&str>,
    action_id: &str,
) -> anyhow::Result<Option<RendererNativeExecution>> {
    let state = OWT_RUNTIME
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?;
    Ok(renderer_native_execution_candidate_from_state(
        &state,
        interface_id,
        action_id,
    ))
}

fn renderer_native_execution_candidate_from_state(
    state: &RuntimeState,
    interface_id: Option<&str>,
    action_id: &str,
) -> Option<RendererNativeExecution> {
    let Ok((resolved_interface_id, _scope, action)) =
        state.declared_action_for_interface(interface_id, action_id)
    else {
        return None;
    };
    if !native_terminal_action_supported(&action) {
        return None;
    }
    let root_allowlisted = state.action_is_root_allowlisted(&resolved_interface_id, &action);
    Some(RendererNativeExecution {
        interface_id: resolved_interface_id,
        action,
        root_allowlisted,
    })
}

fn native_terminal_action_supported(action: &UiAction) -> bool {
    match action.kind {
        ActionKind::Open => action
            .target
            .as_deref()
            .and_then(native_open_target)
            .is_some(),
        ActionKind::Run => !action.argv.is_empty(),
        _ => false,
    }
}

fn execute_native_terminal_action(
    action: &UiAction,
    root_allowlisted: bool,
) -> NativeExecutionOutcome {
    match action.kind {
        ActionKind::Open => execute_native_open_action(action),
        ActionKind::Run => execute_native_run_action(action, root_allowlisted),
        _ => NativeExecutionOutcome {
            executed: false,
            status: "unsupported_native_action".to_string(),
            message: "action kind is not a native terminal action".to_string(),
        },
    }
}

fn execute_native_open_action(action: &UiAction) -> NativeExecutionOutcome {
    let Some(target) = action.target.as_deref().and_then(native_open_target) else {
        return NativeExecutionOutcome {
            executed: false,
            status: "missing_open_target".to_string(),
            message: "open action has no native-openable target".to_string(),
        };
    };
    wezterm_open_url::open_url(&target);
    NativeExecutionOutcome {
        executed: true,
        status: "native_open_requested".to_string(),
        message: format!("native OWT requested platform open for {}", action.id),
    }
}

fn execute_native_run_action(action: &UiAction, root_allowlisted: bool) -> NativeExecutionOutcome {
    if !root_allowlisted {
        return NativeExecutionOutcome {
            executed: false,
            status: "blocked_allowlist_missing".to_string(),
            message:
                "run action requires a root action allowlist entry before native OWT can fork it"
                    .to_string(),
        };
    }
    let argv = action
        .argv
        .iter()
        .map(|arg| arg.trim())
        .filter(|arg| !arg.is_empty())
        .collect::<Vec<_>>();
    let Some(program) = argv.first().copied() else {
        return NativeExecutionOutcome {
            executed: false,
            status: "missing_argv".to_string(),
            message: "run action has no argv program".to_string(),
        };
    };
    let mode = action
        .mode
        .as_deref()
        .map(normalize_action_mode)
        .unwrap_or_else(|| "background".to_string());
    if !matches!(mode.as_str(), "background" | "spawn" | "fork" | "direct") {
        return NativeExecutionOutcome {
            executed: false,
            status: "unsupported_mode".to_string(),
            message: format!("native OWT does not support run mode {mode:?} yet"),
        };
    }

    let mut command = Command::new(program);
    command.args(argv.iter().skip(1).copied());
    if let Some(cwd) = action
        .cwd
        .as_deref()
        .map(str::trim)
        .filter(|cwd| !cwd.is_empty())
    {
        command.current_dir(cwd);
    }
    match command.spawn() {
        Ok(child) => NativeExecutionOutcome {
            executed: true,
            status: "native_process_spawned".to_string(),
            message: format!("native OWT spawned {} as pid {}", action.id, child.id()),
        },
        Err(err) => NativeExecutionOutcome {
            executed: false,
            status: "spawn_failed".to_string(),
            message: format!("native OWT failed to spawn {}: {err}", action.id),
        },
    }
}

fn normalize_action_mode(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn native_open_target(target: &str) -> Option<String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        return Some(trimmed.to_string());
    }

    let mut value = if lowered.starts_with("file:") {
        normalize_file_target(trimmed)
    } else if lowered.starts_with("path:") {
        trimmed[5..].trim().to_string()
    } else if lowered.starts_with("winfile:") {
        trimmed[8..].trim().to_string()
    } else {
        trimmed.to_string()
    };

    if value.is_empty() {
        return None;
    }
    value = expand_home_path(&percent_decode_path(&value));
    if cfg!(windows) {
        value = normalize_windows_open_target(&value);
    }
    Some(value)
}

fn normalize_file_target(target: &str) -> String {
    let mut value = target[5..].trim().to_string();
    if value.starts_with("///") {
        value = value[2..].to_string();
    }
    value
}

fn expand_home_path(value: &str) -> String {
    if value == "~" {
        return env::var("HOME").unwrap_or_else(|_| value.to_string());
    }
    if let Some(rest) = value.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    value.to_string()
}

fn percent_decode_path(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                output.push((high * 16 + low) as char);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index] as char);
        index += 1;
    }
    output
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn normalize_windows_open_target(value: &str) -> String {
    if let Some(converted) = windows_drive_path_from_wsl_mount(value) {
        return converted;
    }
    if value.starts_with('/') {
        let distro = env::var("OWT_WSL_DISTRO_NAME")
            .or_else(|_| env::var("WSL_DISTRO_NAME"))
            .unwrap_or_else(|_| "Ubuntu".to_string());
        let path = value.trim_start_matches('/').replace('/', "\\");
        return format!(r"\\wsl.localhost\{distro}\{path}");
    }
    value.to_string()
}

fn windows_drive_path_from_wsl_mount(value: &str) -> Option<String> {
    let rest = value.strip_prefix("/mnt/")?;
    let mut chars = rest.chars();
    let drive = chars.next()?;
    if !drive.is_ascii_alphabetic() || chars.next() != Some('/') {
        return None;
    }
    let tail = chars.as_str().replace('/', "\\");
    Some(format!("{}:\\{}", drive.to_ascii_uppercase(), tail))
}

fn renderer_action_requests_external_execution(
    interface_id: Option<&str>,
    action_id: &str,
) -> anyhow::Result<bool> {
    let state = OWT_RUNTIME
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?;
    Ok(renderer_action_requests_external_execution_from_state(
        &state,
        interface_id,
        action_id,
    ))
}

fn renderer_action_requests_external_execution_from_state(
    state: &RuntimeState,
    interface_id: Option<&str>,
    action_id: &str,
) -> bool {
    let target_action = if let Some(interface_id) = interface_id.filter(|value| !value.is_empty()) {
        state
            .actions_by_interface
            .get(interface_id)
            .and_then(|actions| actions.get(action_id))
    } else {
        state.actions.get(action_id)
    };
    let Some(action) = target_action else {
        return false;
    };
    match action.kind {
        ActionKind::Run => {
            action.argv.is_empty()
                && (action
                    .command
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
                    || action
                        .target
                        .as_deref()
                        .map(|value| !value.trim().is_empty())
                        .unwrap_or(false))
        }
        ActionKind::Network
        | ActionKind::Refresh
        | ActionKind::Destructive
        | ActionKind::CredentialSensitive => {
            action
                .command
                .as_deref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
                || action
                    .target
                    .as_deref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
        }
        _ => false,
    }
}

pub(crate) fn approve_permission_request(request_id: &str) -> anyhow::Result<PermissionDecision> {
    let decision = OWT_RUNTIME
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .approve_permission_request(request_id)
        .map_err(|err| anyhow!("{err}"))?;
    invalidate_owt_windows();
    Ok(decision)
}

pub(crate) fn deny_permission_request(request_id: &str) -> anyhow::Result<PermissionDecision> {
    let decision = OWT_RUNTIME
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .deny_permission_request(request_id)
        .map_err(|err| anyhow!("{err}"))?;
    invalidate_owt_windows();
    Ok(decision)
}

pub(crate) fn dispatch_focused_table_row_action(
    interface_id: Option<&str>,
) -> anyhow::Result<DispatchedAction> {
    let dispatched = OWT_RUNTIME
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .dispatch_focused_table_row_action(interface_id, None)
        .map_err(|err| anyhow!("{err}"))?;
    invalidate_owt_windows();
    Ok(dispatched)
}

pub(crate) fn focus_table_row_relative(
    interface_id: Option<&str>,
    movement: TableFocusMovement,
) -> anyhow::Result<FocusedTableRow> {
    let focused = OWT_RUNTIME
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .focus_table_row_relative(interface_id, None, movement)
        .map_err(|err| anyhow!("{err}"))?;
    invalidate_owt_windows();
    Ok(focused)
}

pub(crate) fn focus_table_cell_relative(
    interface_id: Option<&str>,
    movement: TableCellFocusMovement,
) -> anyhow::Result<FocusedTableCell> {
    let focused = OWT_RUNTIME
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .focus_table_cell_relative(interface_id, None, movement)
        .map_err(|err| anyhow!("{err}"))?;
    invalidate_owt_windows();
    Ok(focused)
}

pub(crate) fn focus_table_cell(
    interface_id: Option<&str>,
    column: &str,
) -> anyhow::Result<FocusedTableCell> {
    let focused = OWT_RUNTIME
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .focus_table_cell(interface_id, None, column)
        .map_err(|err| anyhow!("{err}"))?;
    invalidate_owt_windows();
    Ok(focused)
}

pub(crate) fn toggle_focused_table_group_mode(
    interface_id: Option<&str>,
) -> anyhow::Result<FocusedTableGroup> {
    let focused = OWT_RUNTIME
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .toggle_focused_table_group_mode(interface_id, None)
        .map_err(|err| anyhow!("{err}"))?;
    invalidate_owt_windows();
    Ok(focused)
}

fn invalidate_owt_windows() {
    invalidate_owt_windows_with_tab_title(None);
}

fn invalidate_owt_windows_with_tab_title(tab_title: Option<String>) {
    promise::spawn::spawn_into_main_thread(async move {
        if let Some(front_end) = crate::frontend::try_front_end() {
            for gui_window in front_end.gui_windows() {
                let tab_title = tab_title.clone();
                gui_window
                    .window
                    .notify(crate::termwindow::TermWindowNotif::Apply(Box::new(
                        move |term_window| {
                            if let Some(tab_title) = tab_title.as_deref() {
                                term_window.apply_owt_scope_tab_title(tab_title);
                            }
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

#[derive(Default, Deserialize)]
struct ExportInterfaceRequest {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    interface: Option<Value>,
    #[serde(default)]
    store_id: Option<String>,
    #[serde(default)]
    document: Option<InterfaceDocument>,
    #[serde(default)]
    package_id: Option<String>,
    #[serde(default)]
    pack_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    permissions: Option<Value>,
    #[serde(default)]
    changelog: Option<String>,
    #[serde(default)]
    samples: Option<Value>,
    #[serde(default)]
    screenshots: Option<Value>,
}

#[derive(Default, Deserialize)]
struct ImportInterfaceRequest {
    #[serde(default)]
    pack: Option<Value>,
    #[serde(default)]
    package: Option<Value>,
    #[serde(default)]
    document: Option<InterfaceDocument>,
    #[serde(default)]
    interface: Option<Value>,
    #[serde(default)]
    permissions: Option<Value>,
    #[serde(default)]
    store_id: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    save: Option<bool>,
    #[serde(default)]
    apply: Option<bool>,
}

impl ImportInterfaceRequest {
    fn save(&self) -> bool {
        self.save.unwrap_or(true)
    }

    fn apply(&self) -> bool {
        self.apply.unwrap_or(false)
    }

    fn store_id(&self) -> Option<String> {
        self.store_id
            .as_deref()
            .or(self.id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }
}

#[derive(Deserialize)]
struct ReplaceInterfaceRequest {
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    interface: Option<Value>,
    #[serde(default)]
    target_interface_id: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default)]
    store_id: Option<String>,
    #[serde(default)]
    replacement_id: Option<String>,
    #[serde(default)]
    document: Option<InterfaceDocument>,
    #[serde(default)]
    replacement: Option<InterfaceDocument>,
    #[serde(default)]
    preserve_lifecycle: Option<bool>,
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

#[derive(Deserialize)]
struct EditInterfaceRequest {
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    interface: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    operations: Vec<InterfaceEditOperation>,
    #[serde(default)]
    dry_run: bool,
    #[serde(default)]
    feedback_mode: Option<String>,
    #[serde(default)]
    repaint_mode: Option<String>,
}

impl EditInterfaceRequest {
    fn interface_id(&self) -> Option<String> {
        self.interface_id
            .as_deref()
            .or(self.interface.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn feedback_mode(&self) -> String {
        normalize_feedback_mode(self.feedback_mode.as_deref())
    }

    fn repaint_mode(&self) -> String {
        self.repaint_mode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("after_batch")
            .to_ascii_lowercase()
    }
}

fn normalize_feedback_mode(value: Option<&str>) -> String {
    match value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("status")
        .to_ascii_lowercase()
        .as_str()
    {
        "minimal" => "minimal".to_string(),
        "snapshot" => "snapshot".to_string(),
        "full" => "full".to_string(),
        _ => "status".to_string(),
    }
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

impl ReplaceInterfaceRequest {
    fn target_interface_id(&self) -> Option<String> {
        self.interface_id
            .as_deref()
            .or(self.target_interface_id.as_deref())
            .or(self.target.as_deref())
            .or_else(|| self.interface.as_ref().and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn store_id(&self) -> Option<String> {
        self.store_id
            .as_deref()
            .or(self.replacement_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn preserve_lifecycle(&self) -> bool {
        self.preserve_lifecycle.unwrap_or(true)
    }

    fn replacement_document(&self) -> anyhow::Result<InterfaceDocument> {
        if let Some(document) = &self.document {
            return validate_interface_document(document.clone());
        }
        if let Some(document) = &self.replacement {
            return validate_interface_document(document.clone());
        }
        if let Some(interface_value) = &self.interface {
            if interface_value.is_object() {
                let document: InterfaceDocument =
                    serde_json::from_value(interface_value.clone())
                        .context("parse OWT native replacement interface document")?;
                return validate_interface_document(document);
            }
        }
        if let Some(store_id) = self.store_id() {
            return Ok(load_interface_document(&store_id)?.document);
        }
        Err(anyhow!(
            "replace_interface requires document, replacement, interface object, store_id, or replacement_id"
        ))
    }
}

#[derive(Deserialize)]
struct PatchInterfaceLayoutRequest {
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default)]
    node_id: Option<String>,
    #[serde(default)]
    node: Option<String>,
    #[serde(default)]
    root_id: Option<String>,
    #[serde(default)]
    layout: Option<Value>,
    #[serde(default)]
    profile: Option<Value>,
    #[serde(default)]
    placement: Option<Value>,
    #[serde(default)]
    dock: Option<Value>,
    #[serde(default)]
    docking: Option<Value>,
    #[serde(default)]
    origin: Option<Value>,
    #[serde(default)]
    anchor: Option<Value>,
    #[serde(default)]
    panel_origin: Option<Value>,
    #[serde(default)]
    surface_origin: Option<Value>,
    #[serde(default)]
    orientation: Option<Value>,
    #[serde(default)]
    axis: Option<Value>,
    #[serde(default)]
    flow: Option<Value>,
    #[serde(default)]
    reservation: Option<Value>,
    #[serde(default)]
    reserve: Option<Value>,
    #[serde(default)]
    terminal_space: Option<Value>,
    #[serde(default)]
    visible: Option<Value>,
    #[serde(default)]
    active: Option<Value>,
    #[serde(default)]
    enabled: Option<Value>,
    #[serde(default)]
    hidden: Option<Value>,
    #[serde(default)]
    display: Option<Value>,
    #[serde(default)]
    viewport_columns: Option<Value>,
    #[serde(default)]
    viewport_cols: Option<Value>,
    #[serde(default)]
    terminal_columns: Option<Value>,
    #[serde(default)]
    terminal_cols: Option<Value>,
    #[serde(default)]
    viewport_rows: Option<Value>,
    #[serde(default)]
    terminal_rows: Option<Value>,
    #[serde(default)]
    min_terminal_cells: Option<Value>,
    #[serde(default)]
    minimum_terminal_cells: Option<Value>,
    #[serde(default)]
    min_terminal_columns: Option<Value>,
    #[serde(default)]
    min_terminal_cols: Option<Value>,
    #[serde(default)]
    min_terminal_rows: Option<Value>,
    #[serde(default)]
    minimum_terminal_rows: Option<Value>,
    #[serde(default)]
    properties: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct BindActionRequest {
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    interface: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default)]
    node_id: Option<String>,
    #[serde(default)]
    node: Option<String>,
    #[serde(default)]
    target_node_id: Option<String>,
    #[serde(default)]
    action: Option<UiAction>,
    #[serde(default)]
    action_id: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    kind: Option<ActionKind>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    argv: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    requires_confirmation: Option<bool>,
    #[serde(default)]
    confirmation_required: Option<bool>,
}

impl BindActionRequest {
    fn interface_id(&self) -> Option<String> {
        self.interface_id
            .as_deref()
            .or(self.interface.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn node_id(&self) -> Option<String> {
        self.node_id
            .as_deref()
            .or(self.node.as_deref())
            .or(self.target_node_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn bound_action(&self) -> anyhow::Result<UiAction> {
        if let Some(action) = &self.action {
            return Ok(action.clone());
        }
        let action_id = self
            .action_id
            .as_deref()
            .or(self.id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("bind_action requires action or action_id"))?;
        let label = self
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(action_id);
        let mut action = UiAction::new(
            action_id,
            label,
            self.kind.clone().unwrap_or(ActionKind::Inspect),
        );
        action.command = self.command.clone();
        action.argv = self.argv.clone();
        action.cwd = self.cwd.clone();
        action.mode = self.mode.clone();
        action.target = self.target.clone();
        action.requires_confirmation = self.requires_confirmation.unwrap_or(false)
            || self.confirmation_required.unwrap_or(false);
        Ok(action)
    }
}

#[derive(Deserialize)]
struct PatchInterfaceLifecycleRequest {
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    interface: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default)]
    pinned: Option<bool>,
    #[serde(default)]
    pin: Option<bool>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default)]
    visible: Option<bool>,
    #[serde(default)]
    display: Option<String>,
    #[serde(default)]
    restore_previous: Option<bool>,
    #[serde(default)]
    restore: Option<bool>,
    #[serde(default)]
    expire_now: Option<bool>,
    #[serde(default)]
    expired: Option<bool>,
    #[serde(default)]
    clear_expiration: Option<bool>,
    #[serde(default)]
    expires_at_unix: Option<u64>,
    #[serde(default)]
    expires_at_epoch: Option<u64>,
    #[serde(default)]
    ttl_seconds: Option<u64>,
    #[serde(default)]
    ttl: Option<u64>,
}

#[derive(Deserialize)]
struct RetireInterfaceRequest {
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    interface: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default)]
    store_id: Option<String>,
    #[serde(default)]
    remove_saved: Option<bool>,
    #[serde(default)]
    delete_saved: Option<bool>,
}

#[derive(Deserialize)]
struct RequestRefreshRequest {
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    interface: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default)]
    action_id: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    permission_granted: Option<bool>,
    #[serde(default)]
    permitted: Option<bool>,
    #[serde(default)]
    execute: Option<bool>,
    #[serde(default)]
    external_execution: Option<bool>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    request_source: Option<String>,
    #[serde(default)]
    requested_by: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default)]
    requested_at_unix: Option<u64>,
}

#[derive(Default, Deserialize)]
struct ListActionRequestsRequest {
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    interface: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    include_actions: Option<bool>,
    #[serde(default)]
    include_refreshes: Option<bool>,
    #[serde(default)]
    pending_only: Option<bool>,
}

#[derive(Deserialize)]
struct DiffInterfaceRequest {
    #[serde(default)]
    left: Option<Value>,
    #[serde(default)]
    right: Option<Value>,
    #[serde(default)]
    left_id: Option<String>,
    #[serde(default)]
    right_id: Option<String>,
    #[serde(default)]
    baseline_id: Option<String>,
    #[serde(default)]
    candidate_id: Option<String>,
}

impl PatchInterfaceLayoutRequest {
    fn target_node_id(&self) -> Option<String> {
        self.node_id
            .as_deref()
            .or(self.node.as_deref())
            .or(self.root_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn layout_properties(&self) -> anyhow::Result<BTreeMap<String, String>> {
        let mut properties = BTreeMap::new();
        for (key, value) in [
            ("layout", &self.layout),
            ("profile", &self.profile),
            ("placement", &self.placement),
            ("dock", &self.dock),
            ("docking", &self.docking),
            ("origin", &self.origin),
            ("anchor", &self.anchor),
            ("panel_origin", &self.panel_origin),
            ("surface_origin", &self.surface_origin),
            ("orientation", &self.orientation),
            ("axis", &self.axis),
            ("flow", &self.flow),
            ("reservation", &self.reservation),
            ("reserve", &self.reserve),
            ("terminal_space", &self.terminal_space),
            ("visible", &self.visible),
            ("active", &self.active),
            ("enabled", &self.enabled),
            ("hidden", &self.hidden),
            ("display", &self.display),
            ("viewport_columns", &self.viewport_columns),
            ("viewport_cols", &self.viewport_cols),
            ("terminal_columns", &self.terminal_columns),
            ("terminal_cols", &self.terminal_cols),
            ("viewport_rows", &self.viewport_rows),
            ("terminal_rows", &self.terminal_rows),
            ("min_terminal_cells", &self.min_terminal_cells),
            ("minimum_terminal_cells", &self.minimum_terminal_cells),
            ("min_terminal_columns", &self.min_terminal_columns),
            ("min_terminal_cols", &self.min_terminal_cols),
            ("min_terminal_rows", &self.min_terminal_rows),
            ("minimum_terminal_rows", &self.minimum_terminal_rows),
        ] {
            insert_layout_property(&mut properties, key, value.as_ref())?;
        }

        for (key, value) in &self.properties {
            if is_layout_property_key(key) {
                insert_layout_property(&mut properties, key, Some(value))?;
            }
        }

        if properties.is_empty() {
            return Err(anyhow!(
                "patch_interface_layout requires at least one layout property"
            ));
        }

        Ok(properties)
    }
}

impl PatchInterfaceLifecycleRequest {
    fn interface_id(&self) -> Option<String> {
        self.interface_id
            .as_deref()
            .or(self.interface.as_deref())
            .or(self.id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn lifecycle_patch(&self) -> InterfaceLifecyclePatch {
        let display = self.display.as_deref().map(str::trim).unwrap_or("");
        let display_lower = display.to_ascii_lowercase();
        let hidden = self.hidden.or_else(|| {
            self.visible
                .map(|visible| !visible)
                .or_else(|| match display_lower.as_str() {
                    "hide" | "hidden" | "none" => Some(true),
                    "show" | "visible" | "restore" | "restored" => Some(false),
                    _ => None,
                })
        });
        let ttl_seconds = self.ttl_seconds.or(self.ttl);
        let expires_at_unix = self.expires_at_unix.or(self.expires_at_epoch);
        let needs_clock = ttl_seconds.is_some() || expires_at_unix.is_some();

        InterfaceLifecyclePatch {
            pinned: self.pinned.or(self.pin),
            hidden,
            restore_previous: self.restore_previous.unwrap_or(false)
                || self.restore.unwrap_or(false)
                || matches!(display_lower.as_str(), "restore" | "restored"),
            expire_now: self.expire_now.unwrap_or(false) || self.expired.unwrap_or(false),
            clear_expiration: self.clear_expiration.unwrap_or(false),
            expires_at_unix,
            ttl_seconds,
            now_unix: needs_clock.then(current_unix_seconds),
        }
    }
}

impl RetireInterfaceRequest {
    fn interface_id(&self) -> Option<String> {
        self.interface_id
            .as_deref()
            .or(self.interface.as_deref())
            .or(self.id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn store_id(&self) -> Option<String> {
        self.store_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn remove_saved(&self) -> bool {
        self.remove_saved.unwrap_or(false) || self.delete_saved.unwrap_or(false)
    }
}

impl RequestRefreshRequest {
    fn interface_id(&self) -> Option<String> {
        self.interface_id
            .as_deref()
            .or(self.interface.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn action_id(&self) -> Option<String> {
        self.action_id
            .as_deref()
            .or(self.action.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn permission_granted(&self) -> bool {
        self.permission_granted.unwrap_or(false) || self.permitted.unwrap_or(false)
    }

    fn external_execution_requested(&self) -> bool {
        self.execute.unwrap_or(false) || self.external_execution.unwrap_or(false)
    }

    fn provenance(&self) -> ActionProvenance {
        ActionProvenance {
            source: self
                .source
                .as_deref()
                .or(self.request_source.as_deref())
                .or(self.requested_by.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .or_else(|| Some("native_http:request_refresh".to_string())),
            request_id: self
                .request_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            requested_at_unix: Some(self.requested_at_unix.unwrap_or_else(current_unix_seconds)),
        }
    }
}

impl ListActionRequestsRequest {
    fn interface_id(&self) -> Option<String> {
        self.interface_id
            .as_deref()
            .or(self.interface.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    fn limit(&self) -> usize {
        self.limit.unwrap_or(24).clamp(1, 128)
    }

    fn include_actions(&self) -> bool {
        self.include_actions.unwrap_or(true)
    }

    fn include_refreshes(&self) -> bool {
        self.include_refreshes.unwrap_or(true)
    }

    fn pending_only(&self) -> bool {
        self.pending_only.unwrap_or(false)
    }
}

impl DiffInterfaceRequest {
    fn left_id(&self) -> Option<&str> {
        self.left_id
            .as_deref()
            .or(self.baseline_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    fn right_id(&self) -> Option<&str> {
        self.right_id
            .as_deref()
            .or(self.candidate_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
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

fn parse_export_interface_body(
    parsed: &ParsedHttpRequest,
) -> anyhow::Result<ExportInterfaceRequest> {
    let body = parsed.body.trim();
    if body.is_empty() {
        return Ok(ExportInterfaceRequest::default());
    }

    if body.starts_with('"') {
        let id: String = serde_json::from_str(body).context("parse OWT native export id")?;
        return Ok(ExportInterfaceRequest {
            id: Some(id),
            ..ExportInterfaceRequest::default()
        });
    }

    let request: ExportInterfaceRequest =
        serde_json::from_str(body).context("parse OWT native export_interface request")?;
    Ok(request)
}

fn parse_import_interface_body(
    parsed: &ParsedHttpRequest,
) -> anyhow::Result<ImportInterfaceRequest> {
    let body = parsed.body.trim();
    if body.is_empty() {
        return Err(anyhow!(
            "import_interface requires pack, package, document, or interface"
        ));
    }

    let value: Value =
        serde_json::from_str(body).context("parse OWT native import_interface request")?;
    let mut request: ImportInterfaceRequest = serde_json::from_value(value.clone())
        .context("parse OWT native import_interface request fields")?;
    if looks_like_interface_pack(&value) {
        request.pack = Some(value);
    }
    if request.pack.is_none() {
        request.pack = request.package.clone();
    }
    if request.pack.is_none()
        && request.document.is_none()
        && !request
            .interface
            .as_ref()
            .map(Value::is_object)
            .unwrap_or(false)
    {
        return Err(anyhow!(
            "import_interface requires pack, package, document, or interface object"
        ));
    }
    Ok(request)
}

fn parse_replace_interface_body(
    parsed: &ParsedHttpRequest,
) -> anyhow::Result<ReplaceInterfaceRequest> {
    let request: ReplaceInterfaceRequest =
        serde_json::from_str(&parsed.body).context("parse OWT native replace_interface request")?;
    request.replacement_document()?;
    Ok(request)
}

fn parse_update_node_body(parsed: &ParsedHttpRequest) -> anyhow::Result<UpdateNodeRequest> {
    serde_json::from_str(&parsed.body).context("parse OWT native update_node request")
}

fn parse_edit_interface_body(parsed: &ParsedHttpRequest) -> anyhow::Result<EditInterfaceRequest> {
    serde_json::from_str(&parsed.body).context("parse OWT native edit_interface request")
}

fn parse_bind_action_body(parsed: &ParsedHttpRequest) -> anyhow::Result<BindActionRequest> {
    let request: BindActionRequest =
        serde_json::from_str(&parsed.body).context("parse OWT native bind_action request")?;
    if request.node_id().is_none() {
        return Err(anyhow!("bind_action requires node_id"));
    }
    request.bound_action()?;
    Ok(request)
}

fn parse_patch_interface_layout_body(
    parsed: &ParsedHttpRequest,
) -> anyhow::Result<PatchInterfaceLayoutRequest> {
    let request: PatchInterfaceLayoutRequest = serde_json::from_str(&parsed.body)
        .context("parse OWT native patch_interface_layout request")?;
    request.layout_properties()?;
    Ok(request)
}

fn parse_patch_interface_lifecycle_body(
    parsed: &ParsedHttpRequest,
) -> anyhow::Result<PatchInterfaceLifecycleRequest> {
    let request: PatchInterfaceLifecycleRequest = serde_json::from_str(&parsed.body)
        .context("parse OWT native patch_interface_lifecycle request")?;
    if request.lifecycle_patch().is_empty() {
        return Err(anyhow!(
            "patch_interface_lifecycle requires at least one pin, visibility, expiration, or restore value"
        ));
    }
    Ok(request)
}

fn parse_retire_interface_body(
    parsed: &ParsedHttpRequest,
) -> anyhow::Result<RetireInterfaceRequest> {
    let body = parsed.body.trim();
    if body.is_empty() {
        return Ok(RetireInterfaceRequest {
            interface_id: None,
            interface: None,
            id: None,
            scope: None,
            store_id: None,
            remove_saved: None,
            delete_saved: None,
        });
    }
    serde_json::from_str(body).context("parse OWT native retire_interface request")
}

fn parse_request_refresh_body(parsed: &ParsedHttpRequest) -> anyhow::Result<RequestRefreshRequest> {
    let body = parsed.body.trim();
    if body.is_empty() {
        return Ok(RequestRefreshRequest {
            interface_id: None,
            interface: None,
            scope: None,
            action_id: None,
            action: None,
            permission_granted: None,
            permitted: None,
            execute: None,
            external_execution: None,
            source: None,
            request_source: None,
            requested_by: None,
            request_id: None,
            requested_at_unix: None,
        });
    }
    serde_json::from_str(body).context("parse OWT native request_refresh request")
}

fn parse_list_action_requests_body(
    parsed: &ParsedHttpRequest,
) -> anyhow::Result<ListActionRequestsRequest> {
    let body = parsed.body.trim();
    if parsed.method == "GET" || body.is_empty() {
        return Ok(ListActionRequestsRequest::default());
    }
    serde_json::from_str(body).context("parse OWT native list_action_requests request")
}

fn parse_diff_interface_body(parsed: &ParsedHttpRequest) -> anyhow::Result<DiffInterfaceRequest> {
    let body = parsed.body.trim();
    if body.is_empty() {
        return Ok(DiffInterfaceRequest {
            left: None,
            right: None,
            left_id: None,
            right_id: None,
            baseline_id: None,
            candidate_id: None,
        });
    }
    serde_json::from_str(body).context("parse OWT native diff_interface request")
}

fn insert_layout_property(
    properties: &mut BTreeMap<String, String>,
    key: &str,
    value: Option<&Value>,
) -> anyhow::Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    if !is_layout_property_key(key) {
        return Ok(());
    }
    let Some(value) = layout_property_value_to_string(key, value)? else {
        return Ok(());
    };
    properties.insert(key.trim().to_string(), value);
    Ok(())
}

fn is_layout_property_key(key: &str) -> bool {
    matches!(
        key.trim(),
        "layout"
            | "profile"
            | "placement"
            | "dock"
            | "docking"
            | "origin"
            | "anchor"
            | "panel_origin"
            | "surface_origin"
            | "orientation"
            | "axis"
            | "flow"
            | "reservation"
            | "reserve"
            | "terminal_space"
            | "visible"
            | "active"
            | "enabled"
            | "hidden"
            | "display"
            | "viewport_columns"
            | "viewport_cols"
            | "terminal_columns"
            | "terminal_cols"
            | "view_columns"
            | "view_cols"
            | "viewport_rows"
            | "terminal_rows"
            | "view_rows"
            | "viewport_lines"
            | "terminal_lines"
            | "min_terminal_cells"
            | "minimum_terminal_cells"
            | "min_grid_cells"
            | "min_terminal_columns"
            | "minimum_terminal_columns"
            | "min_terminal_cols"
            | "min_terminal_rows"
            | "minimum_terminal_rows"
            | "min_terminal_lines"
            | "allow_terminal_overlay"
            | "floating_anchor"
            | "float_anchor"
            | "widget_anchor"
            | "floating_x"
            | "float_x"
            | "widget_x"
            | "surface_x"
            | "floating_y"
            | "float_y"
            | "widget_y"
            | "surface_y"
            | "floating_width"
            | "float_width"
            | "widget_width"
            | "surface_width"
            | "floating_height"
            | "float_height"
            | "widget_height"
            | "surface_height"
    )
}

fn layout_property_value_to_string(key: &str, value: &Value) -> anyhow::Result<Option<String>> {
    let output = match value {
        Value::Null => return Ok(None),
        Value::String(value) => value.trim().to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => {
            return Err(anyhow!(
                "patch_interface_layout property {key} must be a string, boolean, number, or null"
            ))
        }
    };

    if output.is_empty() {
        Ok(None)
    } else {
        Ok(Some(output))
    }
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

fn layout_override_store_dir() -> PathBuf {
    interface_store_dir()
        .parent()
        .map(|parent| parent.join("layout-overrides"))
        .unwrap_or_else(|| PathBuf::from("layout-overrides"))
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

fn layout_override_file_path(id: &str) -> (String, PathBuf) {
    let store_id = sanitize_interface_id(id);
    let path = layout_override_store_dir().join(format!("{store_id}.json"));
    (store_id, path)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InterfaceLayoutOverride {
    schema_version: u16,
    interface_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scope: Option<Scope>,
    node_id: String,
    properties: BTreeMap<String, String>,
    updated_at_unix: u64,
}

fn interface_document_with_layout_overrides(
    mut document: InterfaceDocument,
) -> (InterfaceDocument, Option<InterfaceLayoutOverride>) {
    let Some(layout_override) = load_interface_layout_override(&document.id) else {
        return (document, None);
    };
    if layout_override
        .scope
        .as_ref()
        .is_some_and(|scope| scope != &document.scope)
    {
        return (document, None);
    }
    if layout_override.properties.is_empty() {
        return (document, None);
    }

    let node_id = layout_override.node_id.trim();
    if node_id.is_empty() {
        return (document, None);
    }
    let Some(node) = find_preview_node_mut(&mut document.nodes, node_id) else {
        return (document, None);
    };
    for (key, value) in &layout_override.properties {
        node.properties.insert(key.clone(), value.clone());
    }
    (document, Some(layout_override))
}

fn persist_layout_override_from_patch(
    patched: &PatchedInterfaceLayout,
    requested_properties: &BTreeMap<String, String>,
) -> anyhow::Result<Option<InterfaceLayoutOverride>> {
    let Some(properties) = persistent_layout_override_properties(requested_properties) else {
        return Ok(None);
    };
    save_interface_layout_override(
        &patched.applied.interface_id,
        Some(&patched.applied.scope),
        &patched.node_id,
        properties,
    )
    .map(Some)
}

fn save_interface_layout_override(
    interface_id: &str,
    scope: Option<&Scope>,
    node_id: &str,
    properties: BTreeMap<String, String>,
) -> anyhow::Result<InterfaceLayoutOverride> {
    let (_store_id, path) = layout_override_file_path(interface_id);
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("OWT layout override store path has no parent"))?;
    fs::create_dir_all(dir).context("create OWT layout override store")?;
    let layout_override = InterfaceLayoutOverride {
        schema_version: 1,
        interface_id: interface_id.to_string(),
        scope: scope.cloned(),
        node_id: node_id.to_string(),
        properties,
        updated_at_unix: current_unix_seconds(),
    };
    let body =
        serde_json::to_vec_pretty(&layout_override).context("serialize OWT layout override")?;
    fs::write(&path, body).context("write OWT layout override")?;
    Ok(layout_override)
}

fn load_interface_layout_override(interface_id: &str) -> Option<InterfaceLayoutOverride> {
    let (_store_id, path) = layout_override_file_path(interface_id);
    let content = fs::read_to_string(path).ok()?;
    let layout_override: InterfaceLayoutOverride = serde_json::from_str(&content).ok()?;
    if layout_override.schema_version != 1 {
        return None;
    }
    let override_id = layout_override.interface_id.trim();
    if override_id != interface_id.trim() && override_id != sanitize_interface_id(interface_id) {
        return None;
    }
    Some(layout_override)
}

fn persistent_layout_override_properties(
    requested_properties: &BTreeMap<String, String>,
) -> Option<BTreeMap<String, String>> {
    let has_position_intent = requested_properties
        .keys()
        .any(|key| persistent_layout_position_key(key));
    if !has_position_intent {
        return None;
    }

    let properties = requested_properties
        .iter()
        .filter(|(key, _)| persistent_layout_override_key(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    (!properties.is_empty()).then_some(properties)
}

fn persistent_layout_position_key(key: &str) -> bool {
    matches!(
        key.trim(),
        "layout"
            | "placement"
            | "dock"
            | "docking"
            | "origin"
            | "panel_origin"
            | "surface_origin"
            | "reservation"
            | "reserve"
            | "terminal_space"
            | "floating_anchor"
            | "float_anchor"
            | "widget_anchor"
            | "floating_x"
            | "float_x"
            | "widget_x"
            | "surface_x"
            | "floating_y"
            | "float_y"
            | "widget_y"
            | "surface_y"
    )
}

fn persistent_layout_override_key(key: &str) -> bool {
    matches!(
        key.trim(),
        "active"
            | "allow_terminal_overlay"
            | "anchor"
            | "axis"
            | "display"
            | "dock"
            | "docking"
            | "enabled"
            | "flow"
            | "floating_anchor"
            | "floating_height"
            | "floating_width"
            | "floating_x"
            | "floating_y"
            | "float_anchor"
            | "float_height"
            | "float_width"
            | "float_x"
            | "float_y"
            | "hidden"
            | "layout"
            | "orientation"
            | "origin"
            | "panel_origin"
            | "placement"
            | "profile"
            | "reservation"
            | "reserve"
            | "surface_height"
            | "surface_origin"
            | "surface_width"
            | "surface_x"
            | "surface_y"
            | "terminal_space"
            | "visible"
            | "widget_anchor"
            | "widget_height"
            | "widget_width"
            | "widget_x"
            | "widget_y"
    )
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

fn resolve_export_interface_document(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: &ExportInterfaceRequest,
) -> anyhow::Result<(InterfaceDocument, String)> {
    if let Some(document) = &request.document {
        let document = validate_interface_document(document.clone())?;
        return Ok((document, "export:inline:document".to_string()));
    }
    if let Some(interface) = &request.interface {
        if interface.is_object() {
            let document: InterfaceDocument = serde_json::from_value(interface.clone())
                .context("parse OWT native export interface document")?;
            let document = validate_interface_document(document)?;
            return Ok((document, "export:inline:interface".to_string()));
        }
        if let Some(id) = interface
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return resolve_interface_document_by_id(runtime, id, "export");
        }
        return Err(anyhow!(
            "export interface must be an object or interface id string"
        ));
    }

    if let Some(id) = request
        .interface_id
        .as_deref()
        .or(request.store_id.as_deref())
        .or(request.id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return resolve_interface_document_by_id(runtime, id, "export");
    }

    let document = active_interface_snapshot_from(runtime)?
        .ok_or_else(|| anyhow!("export_interface requires an active interface, id, or document"))?;
    let source = format!("export:active:{}", document.id);
    Ok((document, source))
}

fn export_interface_pack_from_document(
    document: &InterfaceDocument,
    request: &ExportInterfaceRequest,
    source: &str,
) -> anyhow::Result<Value> {
    let permissions = normalized_pack_permissions(request.permissions.as_ref())?;
    let changelog = request
        .changelog
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("{value}\n"))
        .unwrap_or_else(|| "# Changelog\n\n- Exported from native OWT endpoint.\n".to_string());
    let version = request
        .version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("0.1.0");
    let package_id = request
        .package_id
        .as_deref()
        .or(request.pack_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(sanitize_interface_id)
        .unwrap_or_else(|| sanitize_interface_id(&document.id));
    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&document.title);
    let source_label = request
        .source
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(source);

    let interface_bytes =
        serde_json::to_vec_pretty(document).context("serialize OWT pack interface")?;
    let permissions_bytes =
        serde_json::to_vec_pretty(&permissions).context("serialize OWT pack permissions")?;
    let changelog_bytes = changelog.as_bytes();
    let files = vec![
        inline_pack_file_entry("interface.json", &interface_bytes),
        inline_pack_file_entry("permissions.json", &permissions_bytes),
        inline_pack_file_entry("CHANGELOG.md", changelog_bytes),
    ];

    let mut pack = json!({
        "kind": INTERFACE_PACK_KIND,
        "schema_version": INTERFACE_PACK_SCHEMA_VERSION,
        "manifest": {
            "kind": INTERFACE_PACK_KIND,
            "schema_version": INTERFACE_PACK_SCHEMA_VERSION,
            "id": package_id,
            "title": title,
            "version": version,
            "created_at_unix": current_unix_seconds(),
            "source": source_label,
            "interface": {
                "id": &document.id,
                "title": &document.title,
                "schema_version": document.schema_version,
                "scope": &document.scope,
                "file": "interface.json",
                "sha256": sha256_bytes(&interface_bytes),
            },
            "permissions": permissions,
            "files": files,
            "notes": [
                "This inline pack is inert metadata plus semantic interface data.",
                "Importing it must not execute actions or shell commands implicitly."
            ],
        },
        "interface": document,
        "permissions": normalized_pack_permissions(request.permissions.as_ref())?,
        "changelog": changelog,
    });

    if let Some(samples) = request.samples.as_ref() {
        pack["samples"] = samples.clone();
    }
    if let Some(screenshots) = request.screenshots.as_ref() {
        pack["screenshots"] = screenshots.clone();
    }

    Ok(pack)
}

fn import_interface_document_from_request(
    request: &ImportInterfaceRequest,
) -> anyhow::Result<(InterfaceDocument, Value, String)> {
    if let Some(pack) = request.pack.as_ref() {
        validate_interface_pack(pack)?;
        let interface = pack
            .get("interface")
            .ok_or_else(|| anyhow!("interface pack is missing inline interface document"))?;
        let document: InterfaceDocument = serde_json::from_value(interface.clone())
            .context("parse OWT interface pack document")?;
        let document = validate_interface_document(document)?;
        let permissions = inert_import_permissions_from_pack(pack)?;
        let source = pack
            .get("manifest")
            .and_then(|manifest| manifest.get("id"))
            .and_then(Value::as_str)
            .map(|id| format!("import:pack:{id}"))
            .unwrap_or_else(|| "import:pack".to_string());
        return Ok((document, permissions, source));
    }

    if let Some(document) = &request.document {
        let document = validate_interface_document(document.clone())?;
        let permissions = inert_import_permissions(request.permissions.as_ref())?;
        return Ok((document, permissions, "import:inline:document".to_string()));
    }

    if let Some(interface) = &request.interface {
        if interface.is_object() {
            let document: InterfaceDocument = serde_json::from_value(interface.clone())
                .context("parse OWT import interface document")?;
            let document = validate_interface_document(document)?;
            let permissions = inert_import_permissions(request.permissions.as_ref())?;
            return Ok((document, permissions, "import:inline:interface".to_string()));
        }
    }

    Err(anyhow!(
        "import_interface requires pack, package, document, or interface object"
    ))
}

fn looks_like_interface_pack(value: &Value) -> bool {
    value
        .get("kind")
        .and_then(Value::as_str)
        .map(|kind| kind == INTERFACE_PACK_KIND)
        .unwrap_or(false)
}

fn validate_interface_pack(pack: &Value) -> anyhow::Result<()> {
    if pack.get("kind").and_then(Value::as_str) != Some(INTERFACE_PACK_KIND) {
        return Err(anyhow!("not an OWT interface pack"));
    }
    if pack.get("schema_version").and_then(Value::as_u64)
        != Some(INTERFACE_PACK_SCHEMA_VERSION as u64)
    {
        return Err(anyhow!("unsupported OWT interface pack schema"));
    }
    if pack.get("interface").is_none() {
        return Err(anyhow!("OWT interface pack requires inline interface"));
    }
    inert_import_permissions_from_pack(pack)?;
    Ok(())
}

fn inert_import_permissions_from_pack(pack: &Value) -> anyhow::Result<Value> {
    let permissions = pack_permissions_from_pack(pack)?;
    validate_inert_import_permissions(&permissions)?;
    Ok(permissions)
}

fn pack_permissions_from_pack(pack: &Value) -> anyhow::Result<Value> {
    let permissions = pack.get("permissions").or_else(|| {
        pack.get("manifest")
            .and_then(|manifest| manifest.get("permissions"))
    });
    normalized_pack_permissions(permissions)
}

fn inert_import_permissions(value: Option<&Value>) -> anyhow::Result<Value> {
    let permissions = normalized_pack_permissions(value)?;
    validate_inert_import_permissions(&permissions)?;
    Ok(permissions)
}

fn validate_inert_import_permissions(permissions: &Value) -> anyhow::Result<()> {
    validate_inert_permission_value(permissions, "external_execution", &["not_granted", "none"])?;
    validate_inert_permission_value(permissions, "network", &["not_granted", "none"])?;
    validate_inert_permission_value(
        permissions,
        "native_endpoint_authority",
        &["not_included", "not_granted", "none"],
    )?;
    validate_inert_permission_value(permissions, "filesystem", &["read_pack_only", "none"])?;
    Ok(())
}

fn validate_inert_permission_value(
    permissions: &Value,
    key: &str,
    allowed_values: &[&str],
) -> anyhow::Result<()> {
    let Some(value) = permissions.get(key) else {
        return Ok(());
    };
    let Some(value) = value.as_str() else {
        return Err(anyhow!(
            "import_interface inert permissions require {key} to be one of: {}",
            allowed_values.join(", ")
        ));
    };
    if allowed_values
        .iter()
        .any(|allowed| value.eq_ignore_ascii_case(allowed))
    {
        return Ok(());
    }
    Err(anyhow!(
        "import_interface refuses permission {key}={value}; imported packs must not grant execution, network, endpoint, or filesystem authority"
    ))
}

fn normalized_pack_permissions(value: Option<&Value>) -> anyhow::Result<Value> {
    let mut permissions = default_pack_permissions();
    let Some(value) = value else {
        return Ok(permissions);
    };
    let Value::Object(map) = value else {
        return Err(anyhow!("interface pack permissions must be a JSON object"));
    };
    let Some(output) = permissions.as_object_mut() else {
        return Err(anyhow!("default interface pack permissions are invalid"));
    };
    for (key, value) in map {
        output.insert(key.clone(), value.clone());
    }
    Ok(permissions)
}

fn default_pack_permissions() -> Value {
    json!({
        "external_execution": "not_granted",
        "network": "not_granted",
        "filesystem": "read_pack_only",
    })
}

fn inline_pack_file_entry(path: &str, bytes: &[u8]) -> Value {
    json!({
        "path": path,
        "size_bytes": bytes.len(),
        "sha256": sha256_bytes(bytes),
    })
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    bytes_to_hex(&digest.finalize())
}

fn resolve_diff_documents(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: &DiffInterfaceRequest,
) -> anyhow::Result<(InterfaceDocument, InterfaceDocument, String, String)> {
    let (left, left_source) =
        resolve_diff_document(runtime, request.left.as_ref(), request.left_id(), "left")?;
    let (right, right_source) =
        resolve_diff_document(runtime, request.right.as_ref(), request.right_id(), "right")?;
    Ok((left, right, left_source, right_source))
}

fn resolve_diff_document(
    runtime: &Arc<Mutex<RuntimeState>>,
    value: Option<&Value>,
    id: Option<&str>,
    side: &str,
) -> anyhow::Result<(InterfaceDocument, String)> {
    if let Some(value) = value {
        if value.is_object() {
            let document: InterfaceDocument =
                serde_json::from_value(value.clone()).context("parse diff interface document")?;
            validate_interface(&document).map_err(|err| anyhow!("{err}"))?;
            return Ok((document, format!("{side}:inline")));
        }
        if let Some(id) = value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return resolve_diff_document_by_id(runtime, id, side);
        }
        return Err(anyhow!(
            "{side} diff document must be an object or interface id string"
        ));
    }

    if let Some(id) = id {
        return resolve_diff_document_by_id(runtime, id, side);
    }

    let document = active_interface_snapshot_from(runtime)?
        .ok_or_else(|| anyhow!("{side} diff document omitted and no active interface exists"))?;
    let source = format!("{side}:active:{}", document.id);
    Ok((document, source))
}

fn resolve_diff_document_by_id(
    runtime: &Arc<Mutex<RuntimeState>>,
    id: &str,
    side: &str,
) -> anyhow::Result<(InterfaceDocument, String)> {
    resolve_interface_document_by_id(runtime, id, side)
}

fn resolve_interface_document_by_id(
    runtime: &Arc<Mutex<RuntimeState>>,
    id: &str,
    source_prefix: &str,
) -> anyhow::Result<(InterfaceDocument, String)> {
    if matches!(id, "active" | "current" | "runtime") {
        let document = active_interface_snapshot_from(runtime)?.ok_or_else(|| {
            anyhow!("{source_prefix} requested active interface but none is active")
        })?;
        let source = format!("{source_prefix}:active:{}", document.id);
        return Ok((document, source));
    }

    if let Some(document) = runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .interfaces
        .get(id)
        .cloned()
    {
        return Ok((document, format!("{source_prefix}:runtime:{id}")));
    }

    let loaded = load_interface_document(id)?;
    Ok((
        loaded.document,
        format!("{source_prefix}:saved:{}", loaded.store_id),
    ))
}

fn delete_saved_interface_document(id: &str) -> anyhow::Result<Option<PathBuf>> {
    let (_store_id, path) = interface_file_path(id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(Some(path)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("remove saved OWT interface document"),
    }
}

fn list_interfaces_payload(runtime: &Arc<Mutex<RuntimeState>>) -> anyhow::Result<Value> {
    let (runtime_interfaces, owner_groups) = {
        let state = runtime
            .lock()
            .map_err(|_| anyhow!("OWT runtime lock poisoned"))?;
        let active_ids = state
            .active_interface_by_scope
            .values()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut owner_groups = BTreeMap::<String, (InterfaceOwner, Vec<String>, usize)>::new();
        let runtime_interfaces = state
            .interfaces
            .values()
            .map(|document| {
                let owner = InterfaceOwner::from_scope(&document.scope);
                let active = active_ids.contains(&document.id);
                let entry = owner_groups
                    .entry(owner.scope_key.clone())
                    .or_insert_with(|| (owner.clone(), Vec::new(), 0));
                entry.1.push(document.id.clone());
                if active {
                    entry.2 += 1;
                }
                json!({
                    "id": &document.id,
                    "title": &document.title,
                    "scope": &document.scope,
                    "owner": owner,
                    "theme": &document.theme,
                    "active": active,
                    "lifecycle": state.lifecycle_by_interface.get(&document.id),
                })
            })
            .collect::<Vec<_>>();
        let owner_groups = owner_groups
            .into_values()
            .map(|(owner, interface_ids, active_count)| {
                json!({
                    "owner": owner,
                    "count": interface_ids.len(),
                    "active_count": active_count,
                    "interface_ids": interface_ids,
                })
            })
            .collect::<Vec<_>>();
        (runtime_interfaces, owner_groups)
    };
    let saved_interfaces = list_saved_interfaces()?;

    Ok(json!({
        "ok": true,
        "store_dir": interface_store_dir().display().to_string(),
        "runtime": runtime_interfaces,
        "owner_groups": owner_groups,
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
                item["owner"] = json!(InterfaceOwner::from_scope(&document.scope));
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

fn saved_interface_summary_from_value(value: Value) -> anyhow::Result<SavedInterfaceSummary> {
    let store_id = value
        .get("store_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|store_id| !store_id.is_empty())
        .ok_or_else(|| anyhow!("saved OWT interface entry is missing store_id"))?
        .to_string();
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToString::to_string);
    let interface_id = value
        .get("interface_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|interface_id| !interface_id.is_empty())
        .map(ToString::to_string);
    let owner_label = saved_interface_owner_label(&value);
    let owner_key = saved_interface_owner_key(&value);

    Ok(SavedInterfaceSummary {
        store_id,
        title,
        interface_id,
        owner_label,
        owner_key,
    })
}

fn saved_interface_owner_key(value: &Value) -> Option<String> {
    let owner = value.get("owner").or_else(|| value.get("scope"))?;
    let kind = owner.get("kind").and_then(Value::as_str)?.trim();
    let id = owner.get("id").and_then(Value::as_str)?.trim();
    if kind.is_empty() || id.is_empty() {
        return None;
    }
    Some(format!("{}:{}", kind.to_ascii_lowercase(), id))
}

fn saved_interface_owner_label(value: &Value) -> Option<String> {
    let owner = value.get("owner").or_else(|| value.get("scope"))?;
    let kind = owner.get("kind").and_then(Value::as_str)?.trim();
    let id = owner.get("id").and_then(Value::as_str)?.trim();
    if kind.is_empty() || id.is_empty() {
        return None;
    }
    Some(format!(
        "{} {}",
        kind.replace('_', " ").to_ascii_uppercase(),
        compact_owner_id(id)
    ))
}

fn compact_owner_id(id: &str) -> String {
    let trimmed = id.trim().trim_end_matches(['/', '\\']);
    trimmed
        .rsplit(['/', '\\', ':'])
        .find(|part| !part.trim().is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

#[derive(Deserialize)]
struct DispatchActionRequest {
    action_id: String,
    #[serde(default)]
    interface_id: Option<String>,
    #[serde(default)]
    permission_granted: Option<bool>,
    #[serde(default)]
    permitted: Option<bool>,
    #[serde(default)]
    confirmation_granted: Option<bool>,
    #[serde(default)]
    confirmed: Option<bool>,
    #[serde(default)]
    execute: Option<bool>,
    #[serde(default)]
    external_execution: Option<bool>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    request_source: Option<String>,
    #[serde(default)]
    requested_by: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default)]
    requested_at_unix: Option<u64>,
}

impl DispatchActionRequest {
    fn permission_granted(&self) -> bool {
        self.permission_granted.unwrap_or(false) || self.permitted.unwrap_or(false)
    }

    fn confirmation_granted(&self) -> bool {
        self.confirmation_granted.unwrap_or(false) || self.confirmed.unwrap_or(false)
    }

    fn external_execution_requested(&self) -> bool {
        self.execute.unwrap_or(false) || self.external_execution.unwrap_or(false)
    }

    fn provenance(&self) -> ActionProvenance {
        ActionProvenance {
            source: self
                .source
                .as_deref()
                .or(self.request_source.as_deref())
                .or(self.requested_by.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .or_else(|| Some("native_http:dispatch_action".to_string())),
            request_id: self
                .request_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            requested_at_unix: Some(self.requested_at_unix.unwrap_or_else(current_unix_seconds)),
        }
    }
}

fn parse_dispatch_action_body(parsed: &ParsedHttpRequest) -> anyhow::Result<DispatchActionRequest> {
    serde_json::from_str(&parsed.body).context("parse OWT native dispatch request")
}

#[derive(Deserialize)]
struct ExecuteActionRequest {
    request_id: String,
    #[serde(default)]
    confirmation_granted: Option<bool>,
    #[serde(default)]
    confirmed: Option<bool>,
}

impl ExecuteActionRequest {
    fn request_id(&self) -> &str {
        self.request_id.trim()
    }

    fn confirmation_granted(&self) -> bool {
        self.confirmation_granted.unwrap_or(false) || self.confirmed.unwrap_or(false)
    }
}

fn parse_execute_action_request_body(
    parsed: &ParsedHttpRequest,
) -> anyhow::Result<ExecuteActionRequest> {
    serde_json::from_str(&parsed.body).context("parse OWT native execute_action_request request")
}

fn validate_interface_document(document: InterfaceDocument) -> anyhow::Result<InterfaceDocument> {
    validate_interface(&document).map_err(|err| anyhow!("{err}"))?;
    Ok(document)
}

fn apply_interface(
    runtime: &Arc<Mutex<RuntimeState>>,
    document: InterfaceDocument,
) -> anyhow::Result<AppliedInterface> {
    let (document, _) = interface_document_with_layout_overrides(document);
    apply_interface_without_layout_overrides(runtime, document)
}

fn apply_interface_without_layout_overrides(
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

fn edit_runtime_interface(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: &EditInterfaceRequest,
) -> anyhow::Result<EditedInterface> {
    let interface_id = request.interface_id();
    let scope = request.scope.clone();
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .edit_interface(
            interface_id.as_deref(),
            scope.as_ref(),
            request.operations.clone(),
            request.dry_run,
        )
        .map_err(|err| anyhow!("{err}"))
}

fn bind_runtime_action(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: BindActionRequest,
    node_id: &str,
    action: UiAction,
) -> anyhow::Result<AppliedInterface> {
    let interface_id = request.interface_id();
    let scope = request.scope.clone();
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .bind_action(interface_id.as_deref(), scope.as_ref(), node_id, action)
        .map_err(|err| anyhow!("{err}"))
}

fn patch_runtime_layout(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: PatchInterfaceLayoutRequest,
) -> anyhow::Result<PatchedInterfaceLayout> {
    let interface_id = request.interface_id.clone();
    let scope = request.scope.clone();
    let node_id = request.target_node_id();
    let properties = request.layout_properties()?;
    let requested_properties = properties.clone();
    let patched = runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .patch_interface_layout(
            interface_id.as_deref(),
            scope.as_ref(),
            node_id.as_deref(),
            properties,
        )
        .map_err(|err| anyhow!("{err}"))?;
    if let Err(err) = persist_layout_override_from_patch(&patched, &requested_properties) {
        log::warn!(
            "OWT LCARS layout override persistence failed for {}: {err:#}",
            patched.applied.interface_id
        );
    }
    Ok(patched)
}

fn preview_runtime_layout(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: PatchInterfaceLayoutRequest,
) -> anyhow::Result<owt_control::LayoutPreview> {
    let interface_id = request.interface_id.clone();
    let scope = request.scope.clone();
    let node_id = request.target_node_id();
    let properties = request.layout_properties()?;
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .preview_interface_layout(
            interface_id.as_deref(),
            scope.as_ref(),
            node_id.as_deref(),
            properties,
        )
        .map_err(|err| anyhow!("{err}"))
}

fn patch_runtime_lifecycle(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: PatchInterfaceLifecycleRequest,
) -> anyhow::Result<PatchedInterfaceLifecycle> {
    let interface_id = request.interface_id();
    let scope = request.scope.clone();
    let patch = request.lifecycle_patch();
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .patch_interface_lifecycle(interface_id.as_deref(), scope.as_ref(), patch)
        .map_err(|err| anyhow!("{err}"))
}

fn replace_runtime_interface(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: ReplaceInterfaceRequest,
) -> anyhow::Result<ReplacedInterface> {
    let target_interface_id = request.target_interface_id();
    let scope = request.scope.clone();
    let preserve_lifecycle = request.preserve_lifecycle();
    let replacement = request.replacement_document()?;
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .replace_interface(
            target_interface_id.as_deref(),
            scope.as_ref(),
            replacement,
            preserve_lifecycle,
        )
        .map_err(|err| anyhow!("{err}"))
}

fn retire_runtime_interface(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: &RetireInterfaceRequest,
) -> anyhow::Result<owt_control::RetiredInterface> {
    let interface_id = request.interface_id();
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .retire_interface(interface_id.as_deref(), request.scope.as_ref())
        .map_err(|err| anyhow!("{err}"))
}

fn request_runtime_refresh(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: &RequestRefreshRequest,
) -> anyhow::Result<owt_control::RequestedRefresh> {
    let interface_id = request.interface_id();
    let action_id = request.action_id();
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .request_refresh_with_provenance(
            interface_id.as_deref(),
            request.scope.as_ref(),
            action_id.as_deref(),
            false,
            request.external_execution_requested(),
            request.provenance(),
        )
        .map_err(|err| anyhow!("{err}"))
}

fn list_action_requests_payload(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: &ListActionRequestsRequest,
) -> anyhow::Result<Value> {
    let state = runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?;
    let explicit_interface_id = request.interface_id();
    let scope_filter_requested = request.scope.is_some() && explicit_interface_id.is_none();
    let scope_interface_id = request
        .scope
        .as_ref()
        .and_then(|scope| state.active_interface_by_scope.get(&scope.key()).cloned());
    let interface_id = explicit_interface_id.or(scope_interface_id);
    let unresolved_scope_filter = scope_filter_requested && interface_id.is_none();
    let limit = request.limit();

    let mut action_requests = if request.include_actions() && !unresolved_scope_filter {
        state.recent_action_requests(interface_id.as_deref(), limit)
    } else {
        Vec::new()
    };
    let mut refresh_requests = if request.include_refreshes() && !unresolved_scope_filter {
        state.recent_refresh_requests(interface_id.as_deref(), limit)
    } else {
        Vec::new()
    };
    if request.pending_only() {
        action_requests.retain(|request| !request.executed);
        refresh_requests.retain(|request| !request.executed);
    }

    let pending_action_count = action_requests
        .iter()
        .filter(|request| !request.executed)
        .count();
    let pending_refresh_count = refresh_requests
        .iter()
        .filter(|request| !request.executed)
        .count();

    Ok(json!({
        "ok": true,
        "interface_id": interface_id,
        "scope_resolved": !unresolved_scope_filter,
        "limit": limit,
        "pending_only": request.pending_only(),
        "action_requests": action_requests,
        "refresh_requests": refresh_requests,
        "pending_action_count": pending_action_count,
        "pending_refresh_count": pending_refresh_count,
        "message": "native OWT action and refresh request queue inspected without executing commands"
    }))
}

fn dispatch_runtime_action(
    runtime: &Arc<Mutex<RuntimeState>>,
    interface_id: Option<&str>,
    action_id: &str,
    permission_granted: bool,
    confirmation_granted: bool,
    external_execution_requested: bool,
    provenance: ActionProvenance,
) -> anyhow::Result<DispatchedAction> {
    runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .dispatch_action_for_interface_with_intent_and_provenance(
            interface_id,
            action_id,
            permission_granted,
            confirmation_granted,
            external_execution_requested,
            provenance,
        )
        .map_err(|err| anyhow!("{err}"))
}

fn execute_approved_action_request(
    runtime: &Arc<Mutex<RuntimeState>>,
    request: &ExecuteActionRequest,
) -> anyhow::Result<Value> {
    let request_id = request.request_id();
    if request_id.is_empty() {
        return Err(anyhow!("request_id must not be empty"));
    }

    let (queued, native_execution) = {
        let state = runtime
            .lock()
            .map_err(|_| anyhow!("OWT runtime lock poisoned"))?;
        let queued = state
            .action_requests
            .iter()
            .rev()
            .find(|queued| queued.request_id.as_deref() == Some(request_id))
            .cloned()
            .ok_or_else(|| anyhow!("permission request {request_id} was not found"))?;
        let native_execution = if queued.approval_state == RequestApprovalState::Granted
            && queued.permission_granted
        {
            renderer_native_execution_candidate_from_state(
                &state,
                queued.interface_id.as_deref(),
                &queued.action_id,
            )
        } else {
            None
        };
        (queued, native_execution)
    };

    if queued.executed {
        return Ok(json!({
            "ok": true,
            "request_id": request_id,
            "executed": false,
            "already_executed": true,
            "approval_state": queued.approval_state,
            "execution_status": queued.execution_status.clone(),
            "execution_message": queued.execution_message.clone(),
            "dispatched": queued,
            "message": "native OWT did not execute again because the request was already completed"
        }));
    }

    if queued.approval_state == RequestApprovalState::Denied {
        return Ok(json!({
            "ok": true,
            "request_id": request_id,
            "executed": false,
            "approval_required": false,
            "approval_state": queued.approval_state,
            "execution_status": "denied",
            "execution_message": "native OWT refused execution because the request was denied",
            "dispatched": queued,
        }));
    }

    if queued.approval_state != RequestApprovalState::Granted || !queued.permission_granted {
        return Ok(json!({
            "ok": true,
            "request_id": request_id,
            "executed": false,
            "approval_required": true,
            "approval_state": queued.approval_state,
            "execution_status": "approval_required",
            "execution_message": "native OWT requires terminal-owned approval before executing this request",
            "dispatched": queued,
        }));
    }

    if !request.confirmation_granted() {
        return Ok(json!({
            "ok": true,
            "request_id": request_id,
            "executed": false,
            "approval_required": false,
            "approval_state": queued.approval_state,
            "execution_status": "confirmation_required",
            "execution_message": "native OWT requires confirmation_granted=true before executing an approved request",
            "dispatched": queued,
        }));
    }

    let Some(native_execution) = native_execution else {
        let status = "unsupported_native_action";
        let message =
            "approved request does not resolve to a native-open or allowlist-governed run action";
        let dispatched = runtime
            .lock()
            .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
            .record_action_request_execution_result(request_id, false, status, message)
            .map_err(|err| anyhow!("{err}"))?;
        return Ok(json!({
            "ok": true,
            "request_id": request_id,
            "executed": false,
            "approval_state": dispatched.approval_state,
            "execution_status": status,
            "execution_message": message,
            "dispatched": dispatched,
        }));
    };

    let outcome =
        execute_native_terminal_action(&native_execution.action, native_execution.root_allowlisted);
    let dispatched = runtime
        .lock()
        .map_err(|_| anyhow!("OWT runtime lock poisoned"))?
        .record_action_request_execution_result(
            request_id,
            outcome.executed,
            outcome.status.clone(),
            outcome.message.clone(),
        )
        .map_err(|err| anyhow!("{err}"))?;

    Ok(json!({
        "ok": true,
        "request_id": request_id,
        "executed": outcome.executed,
        "interface_id": native_execution.interface_id,
        "action_id": native_execution.action.id,
        "root_allowlisted": native_execution.root_allowlisted,
        "approval_state": dispatched.approval_state,
        "execution_status": outcome.status,
        "execution_message": outcome.message,
        "dispatched": dispatched,
    }))
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
        active_interface_snapshots_from_state, bytes_to_hex, current_unix_seconds,
        dispatch_runtime_action, export_interface_pack_from_document,
        import_interface_document_from_request, interface_document_with_layout_overrides,
        interface_statuses_from_state, layout_override_file_path, list_action_requests_payload,
        list_interfaces_payload, parse_bind_action_body, parse_diff_interface_body,
        parse_dispatch_action_body, parse_edit_interface_body, parse_export_interface_body,
        parse_http_request, parse_import_interface_body, parse_list_action_requests_body,
        parse_patch_interface_lifecycle_body, parse_replace_interface_body,
        parse_request_refresh_body, parse_retire_interface_body, parse_save_interface_body,
        persist_layout_override_from_patch, render_projection_payload,
        render_projection_status_map, renderer_action_requests_external_execution_from_state,
        renderer_native_execution_candidate_from_state, request_runtime_refresh,
        required_http_request_len, sanitize_interface_id, save_interface_layout_override,
        saved_interface_summary_from_value, validation_warnings_payload_for_interface,
        windows_drive_path_from_wsl_mount, ListActionRequestsRequest,
    };
    use owt_control::{
        ActionKind, ActionProvenance, InterfaceDocument, InterfaceLifecycleState,
        PatchedInterfaceLayout, RequestApprovalState, RuntimeState, Scope, ScopeKind, UiAction,
        UiNode, UiNodeKind,
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

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
    fn render_projection_payload_exposes_actual_renderer_contract() {
        let mut document = InterfaceDocument::new(
            "demo.projection",
            "DEMO PROJECTION",
            Scope::new(ScopeKind::Session, "demo.projection"),
        );
        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        root.properties
            .insert("dock".to_string(), "bottom-right".to_string());
        root.properties
            .insert("profile".to_string(), "block_composition".to_string());
        root.properties
            .insert("anchor".to_string(), "edge-sticky".to_string());
        root.properties
            .insert("z_order".to_string(), "3".to_string());
        root.properties
            .insert("priority".to_string(), "8".to_string());
        root.properties
            .insert("min_terminal_cells".to_string(), "96x30".to_string());
        root.properties
            .insert("collapse_policy".to_string(), "tab".to_string());
        root.properties
            .insert("floating_anchor".to_string(), "center".to_string());
        root.properties
            .insert("floating_x".to_string(), "640".to_string());
        root.properties
            .insert("floating_y".to_string(), "360".to_string());
        root.properties
            .insert("floating_width".to_string(), "520".to_string());
        root.properties
            .insert("floating_height".to_string(), "260".to_string());
        root.properties
            .insert("profile_family".to_string(), "semantic_blocks".to_string());
        root.properties
            .insert("table_density".to_string(), "balanced".to_string());
        root.properties
            .insert("cohort_key".to_string(), "lane".to_string());
        root.properties
            .insert("state_key".to_string(), "status".to_string());
        root.properties
            .insert("severity_key".to_string(), "risk".to_string());
        root.properties.insert(
            "lifecycle_controls".to_string(),
            "refresh,pin,hide".to_string(),
        );
        root.properties.insert(
            "action_roles".to_string(),
            "refresh,open,inspect".to_string(),
        );
        root.properties.insert(
            "refresh_policy".to_string(),
            "explicit_snapshot".to_string(),
        );
        root.properties
            .insert("drilldown_policy".to_string(), "lane_focus".to_string());
        document.nodes.push(root);

        let payload = render_projection_payload(&document);

        assert_eq!(payload["renderer_path"], "structural_lcars");
        assert_eq!(payload["layout"], "bottom_strip");
        assert_eq!(payload["origin"], "bottom_right");
        assert_eq!(payload["structural_profile"], "block_composition");
        assert_eq!(payload["anchor"], "edge-sticky");
        assert_eq!(payload["z_order"], 3);
        assert_eq!(payload["priority"], 8);
        assert_eq!(payload["min_terminal_cells"], "96x30");
        assert_eq!(payload["collapse_policy"], "tab");
        assert!(payload.get("floating_anchor").is_none());
        assert!(payload.get("floating_x").is_none());
        assert!(payload.get("floating_y").is_none());
        assert!(payload.get("floating_width").is_none());
        assert!(payload.get("floating_height").is_none());
        assert_eq!(payload["profile_family"], "semantic_blocks");
        assert_eq!(payload["table_density"], "balanced");
        assert_eq!(payload["cohort_key"], "lane");
        assert_eq!(payload["state_key"], "status");
        assert_eq!(payload["severity_key"], "risk");
        assert_eq!(payload["lifecycle_controls"], "refresh,pin,hide");
        assert_eq!(payload["action_roles"], "refresh,open,inspect");
        assert_eq!(payload["refresh_policy"], "explicit_snapshot");
        assert_eq!(payload["drilldown_policy"], "lane_focus");
        let status = render_projection_status_map(&document);
        assert_eq!(
            status.get("renderer_path").map(String::as_str),
            Some("structural_lcars")
        );
        assert_eq!(
            status.get("structural_profile").map(String::as_str),
            Some("block_composition")
        );
        assert_eq!(
            status.get("anchor").map(String::as_str),
            Some("edge-sticky")
        );
        assert_eq!(status.get("z_order").map(String::as_str), Some("3"));
        assert_eq!(status.get("priority").map(String::as_str), Some("8"));
        assert_eq!(
            status.get("min_terminal_cells").map(String::as_str),
            Some("96x30")
        );
        assert_eq!(
            status.get("collapse_policy").map(String::as_str),
            Some("tab")
        );
        assert!(status.get("floating_anchor").is_none());
        assert!(status.get("floating_x").is_none());
        assert!(status.get("floating_y").is_none());
        assert!(status.get("floating_width").is_none());
        assert!(status.get("floating_height").is_none());
        assert_eq!(
            status.get("profile_family").map(String::as_str),
            Some("semantic_blocks")
        );
        assert_eq!(
            status.get("table_density").map(String::as_str),
            Some("balanced")
        );
        assert_eq!(status.get("cohort_key").map(String::as_str), Some("lane"));
        assert_eq!(status.get("state_key").map(String::as_str), Some("status"));
        assert_eq!(status.get("severity_key").map(String::as_str), Some("risk"));
        assert_eq!(
            status.get("lifecycle_controls").map(String::as_str),
            Some("refresh,pin,hide")
        );
        assert_eq!(
            status.get("action_roles").map(String::as_str),
            Some("refresh,open,inspect")
        );
        assert_eq!(
            status.get("refresh_policy").map(String::as_str),
            Some("explicit_snapshot")
        );
        assert_eq!(
            status.get("drilldown_policy").map(String::as_str),
            Some("lane_focus")
        );
    }

    #[test]
    fn validation_warning_payload_reports_missing_keyboard_fallback() {
        let runtime = Arc::new(Mutex::new(RuntimeState::default()));
        let mut document = InterfaceDocument::new(
            "demo.keyboard",
            "DEMO KEYBOARD",
            Scope::new(ScopeKind::Session, "demo.keyboard"),
        );
        document.allowed_action_kinds = vec![ActionKind::Inspect];
        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        root.properties
            .insert("action_paging".to_string(), "false".to_string());
        for index in 1..=10 {
            let action_id = format!("inspect.{index}");
            document.actions.push(UiAction::new(
                action_id.clone(),
                format!("Inspect {index}"),
                ActionKind::Inspect,
            ));
            let mut button = UiNode::new(format!("button.{index}"), UiNodeKind::Button);
            button.action_id = Some(action_id);
            root.children.push(button);
        }
        document.nodes.push(root);
        runtime.lock().unwrap().apply_interface(document).unwrap();

        let warnings = validation_warnings_payload_for_interface(&runtime, "demo.keyboard");

        assert_eq!(warnings.as_array().map(Vec::len), Some(1));
        assert_eq!(warnings[0]["code"], "keyboard_fallback_missing");
        assert_eq!(warnings[0]["node_id"], "button.10");
    }

    #[test]
    fn validation_warning_payload_reports_missing_action_allowlist() {
        let runtime = Arc::new(Mutex::new(RuntimeState::default()));
        let mut document = InterfaceDocument::new(
            "demo.allowlist",
            "DEMO ALLOWLIST",
            Scope::new(ScopeKind::Session, "demo.allowlist"),
        );
        document.allowed_action_kinds = vec![ActionKind::Refresh];
        let mut action = UiAction::new("kernel.refresh", "Refresh Kernel", ActionKind::Refresh);
        action.target = Some("local:/proc".to_string());
        document.actions.push(action);
        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        let mut button = UiNode::new("button.refresh", UiNodeKind::Button);
        button.action_id = Some("kernel.refresh".to_string());
        root.children.push(button);
        document.nodes.push(root);
        runtime.lock().unwrap().apply_interface(document).unwrap();

        let warnings = validation_warnings_payload_for_interface(&runtime, "demo.allowlist");

        assert_eq!(warnings.as_array().map(Vec::len), Some(1));
        assert_eq!(warnings[0]["code"], "action_allowlist_missing");
        assert_eq!(warnings[0]["action_id"], "kernel.refresh");
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
    fn persisted_layout_override_reapplies_to_default_document() {
        let interface_id = format!(
            "demo.layout.override.{}.{}",
            std::process::id(),
            current_unix_seconds()
        );
        let mut document = InterfaceDocument::new(
            interface_id.clone(),
            "DEMO LAYOUT",
            Scope::new(ScopeKind::Project, "/tmp/demo-layout"),
        );
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));

        let properties = BTreeMap::from([
            ("layout".to_string(), "bottom_strip".to_string()),
            ("placement".to_string(), "bottom-right".to_string()),
            ("origin".to_string(), "bottom-right".to_string()),
            ("dock".to_string(), "bottom_right".to_string()),
            ("reservation".to_string(), "reserved".to_string()),
            ("reserve".to_string(), "true".to_string()),
            ("visible".to_string(), "true".to_string()),
            ("hidden".to_string(), "false".to_string()),
            ("display".to_string(), "block".to_string()),
        ]);
        save_interface_layout_override(
            &interface_id,
            Some(&document.scope),
            "panel.root",
            properties,
        )
        .unwrap();

        let (prepared, layout_override) = interface_document_with_layout_overrides(document);

        assert!(layout_override.is_some());
        assert_eq!(
            prepared.nodes[0]
                .properties
                .get("layout")
                .map(String::as_str),
            Some("bottom_strip")
        );
        assert_eq!(
            prepared.nodes[0]
                .properties
                .get("origin")
                .map(String::as_str),
            Some("bottom-right")
        );

        let (_store_id, path) = layout_override_file_path(&interface_id);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn manual_layout_patch_persists_as_layout_override() {
        let interface_id = format!(
            "demo.layout.patch.override.{}.{}",
            std::process::id(),
            current_unix_seconds()
        );
        let patched = PatchedInterfaceLayout {
            applied: owt_control::AppliedInterface {
                scope: Scope::new(ScopeKind::Project, "/tmp/demo-layout"),
                interface_id: interface_id.clone(),
            },
            node_id: "panel.root".to_string(),
            prior_properties: BTreeMap::new(),
            applied_properties: BTreeMap::new(),
        };
        let move_patch = BTreeMap::from([
            ("layout".to_string(), "bottom_strip".to_string()),
            ("placement".to_string(), "bottom-right".to_string()),
            ("reservation".to_string(), "reserved".to_string()),
            ("visible".to_string(), "true".to_string()),
        ]);

        let persisted = persist_layout_override_from_patch(&patched, &move_patch)
            .unwrap()
            .expect("layout override persisted");

        assert_eq!(persisted.interface_id, interface_id);
        assert_eq!(persisted.node_id, "panel.root");
        assert_eq!(
            persisted.properties.get("layout").map(String::as_str),
            Some("bottom_strip")
        );

        let mut document = InterfaceDocument::new(
            interface_id.clone(),
            "DEMO LAYOUT",
            Scope::new(ScopeKind::Project, "/tmp/demo-layout"),
        );
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        let (prepared, layout_override) = interface_document_with_layout_overrides(document);

        assert!(layout_override.is_some());
        assert_eq!(
            prepared.nodes[0]
                .properties
                .get("layout")
                .map(String::as_str),
            Some("bottom_strip")
        );
        assert_eq!(
            prepared.nodes[0]
                .properties
                .get("placement")
                .map(String::as_str),
            Some("bottom-right")
        );

        let (_store_id, path) = layout_override_file_path(&interface_id);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn layout_override_persistence_ignores_visibility_only_patches() {
        let patched = PatchedInterfaceLayout {
            applied: owt_control::AppliedInterface {
                scope: Scope::new(ScopeKind::Project, "/tmp/demo-layout"),
                interface_id: "demo.visibility.only".to_string(),
            },
            node_id: "panel.root".to_string(),
            prior_properties: BTreeMap::new(),
            applied_properties: BTreeMap::new(),
        };
        let visibility_only = BTreeMap::from([
            ("visible".to_string(), "false".to_string()),
            ("hidden".to_string(), "true".to_string()),
            ("display".to_string(), "none".to_string()),
        ]);

        assert!(
            persist_layout_override_from_patch(&patched, &visibility_only)
                .unwrap()
                .is_none()
        );
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

    #[test]
    fn export_interface_pack_includes_inert_manifest_and_hashes() {
        let mut document = InterfaceDocument::new(
            "demo.pack",
            "DEMO PACK",
            Scope::new(ScopeKind::Session, "demo.pack"),
        );
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        let request = parse_export_interface_body(
            &parse_http_request("POST /owt/export_interface HTTP/1.1\r\n\r\n").unwrap(),
        )
        .unwrap();

        let pack = export_interface_pack_from_document(&document, &request, "test:inline").unwrap();

        assert_eq!(pack["kind"], "owt.interface_pack");
        assert_eq!(pack["manifest"]["id"], "demo.pack");
        assert_eq!(pack["interface"]["id"], "demo.pack");
        assert_eq!(pack["permissions"]["external_execution"], "not_granted");
        let interface_file = pack["manifest"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["path"] == "interface.json")
            .expect("interface entry");
        assert_eq!(
            interface_file["sha256"].as_str().unwrap_or_default().len(),
            64
        );
    }

    #[test]
    fn import_interface_pack_stays_inert_by_default() {
        let mut document = InterfaceDocument::new(
            "demo.import",
            "DEMO IMPORT",
            Scope::new(ScopeKind::Session, "demo.import"),
        );
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        let export_request = parse_export_interface_body(
            &parse_http_request("POST /owt/export_interface HTTP/1.1\r\n\r\n").unwrap(),
        )
        .unwrap();
        let pack =
            export_interface_pack_from_document(&document, &export_request, "test:inline").unwrap();
        let body = json!({
            "pack": pack,
            "save": false,
            "apply": false
        })
        .to_string();
        let raw = format!(
            "POST /owt/import_interface HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );
        let parsed = parse_http_request(&raw).unwrap();

        let request = parse_import_interface_body(&parsed).unwrap();
        let (imported, permissions, source) =
            import_interface_document_from_request(&request).unwrap();

        assert_eq!(imported.id, "demo.import");
        assert!(!request.save());
        assert!(!request.apply());
        assert_eq!(permissions["external_execution"], "not_granted");
        assert_eq!(source, "import:pack:demo.import");
    }

    #[test]
    fn import_interface_rejects_permission_grants() {
        let mut document = InterfaceDocument::new(
            "demo.import.danger",
            "DEMO IMPORT DANGER",
            Scope::new(ScopeKind::Session, "demo.import.danger"),
        );
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        let export_request = parse_export_interface_body(
            &parse_http_request("POST /owt/export_interface HTTP/1.1\r\n\r\n").unwrap(),
        )
        .unwrap();
        let mut pack =
            export_interface_pack_from_document(&document, &export_request, "test:inline").unwrap();
        pack["permissions"]["external_execution"] = json!("granted");
        let body = json!({
            "pack": pack,
            "save": false,
            "apply": true
        })
        .to_string();
        let raw = format!(
            "POST /owt/import_interface HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );
        let parsed = parse_http_request(&raw).unwrap();

        let request = parse_import_interface_body(&parsed).unwrap();
        let err = import_interface_document_from_request(&request).unwrap_err();

        assert!(err.to_string().contains("refuses permission"));
    }

    #[test]
    fn replace_interface_accepts_target_and_inline_document() {
        let body = "{\"interface_id\":\"demo.old\",\"document\":{\"schema_version\":1,\"id\":\"demo.new\",\"title\":\"DEMO\",\"scope\":{\"kind\":\"project\",\"id\":\"/tmp/demo\"},\"nodes\":[{\"id\":\"panel.root\",\"kind\":\"panel\"}]}}";
        let raw = format!(
            "POST /owt/replace_interface HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );
        let parsed = parse_http_request(&raw).unwrap();

        let request = parse_replace_interface_body(&parsed).unwrap();
        assert_eq!(request.target_interface_id().as_deref(), Some("demo.old"));
        assert!(request.preserve_lifecycle());
        assert_eq!(request.replacement_document().unwrap().id, "demo.new");
    }

    #[test]
    fn bind_action_accepts_inline_action_fields() {
        let body = "{\"interface\":\"demo.interface\",\"node_id\":\"button.refresh\",\"action_id\":\"kernel.refresh\",\"label\":\"Refresh\",\"kind\":\"run\",\"argv\":[\"owt-check\",\"--dry-run\"],\"cwd\":\"/tmp\",\"mode\":\"background\"}";
        let raw = format!(
            "POST /owt/bind_action HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );
        let parsed = parse_http_request(&raw).unwrap();

        let request = parse_bind_action_body(&parsed).unwrap();
        assert_eq!(request.interface_id().as_deref(), Some("demo.interface"));
        assert_eq!(request.node_id().as_deref(), Some("button.refresh"));
        let action = request.bound_action().unwrap();
        assert_eq!(action.id, "kernel.refresh");
        assert_eq!(action.kind, ActionKind::Run);
        assert_eq!(action.argv, vec!["owt-check", "--dry-run"]);
        assert_eq!(action.cwd.as_deref(), Some("/tmp"));
        assert_eq!(action.mode.as_deref(), Some("background"));
    }

    #[test]
    fn edit_interface_accepts_ordered_operations_and_feedback_mode() {
        let body = "{\"interface_id\":\"demo.interface\",\"feedback_mode\":\"full\",\"operations\":[{\"operation\":\"add_node\",\"parent_id\":\"panel.root\",\"node\":{\"id\":\"badge.ready\",\"kind\":\"badge\",\"label\":\"READY\"}},{\"operation\":\"highlight_node\",\"node_id\":\"badge.ready\",\"mode\":\"pulse\"}]}";
        let raw = format!(
            "POST /owt/edit_interface HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );
        let parsed = parse_http_request(&raw).unwrap();

        let request = parse_edit_interface_body(&parsed).unwrap();
        assert_eq!(request.interface_id().as_deref(), Some("demo.interface"));
        assert_eq!(request.feedback_mode(), "full");
        assert_eq!(request.repaint_mode(), "after_batch");
        assert_eq!(request.operations.len(), 2);
    }

    #[test]
    fn retire_request_accepts_interface_and_store_flags() {
        let parsed = parse_http_request(
            "POST /owt/retire_interface HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: 87\r\n\
             \r\n\
             {\"interface\":\"demo.interface\",\"store_id\":\"demo.slot\",\"delete_saved\":true}",
        )
        .unwrap();

        let request = parse_retire_interface_body(&parsed).unwrap();
        assert_eq!(request.interface_id().as_deref(), Some("demo.interface"));
        assert_eq!(request.store_id().as_deref(), Some("demo.slot"));
        assert!(request.remove_saved());
    }

    #[test]
    fn request_refresh_accepts_permissioned_action_intent() {
        let parsed = parse_http_request(
            "POST /owt/request_refresh HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: 123\r\n\
             \r\n\
             {\"interface\":\"demo.interface\",\"action_id\":\"kernel.refresh\",\"permission_granted\":true,\"execute\":true,\"source\":\"mcp:owt_current\",\"request_id\":\"refresh-001\",\"requested_at_unix\":1779160100}",
        )
        .unwrap();

        let request = parse_request_refresh_body(&parsed).unwrap();
        assert_eq!(request.interface_id().as_deref(), Some("demo.interface"));
        assert_eq!(request.action_id().as_deref(), Some("kernel.refresh"));
        assert!(request.permission_granted());
        assert!(request.external_execution_requested());
        let provenance = request.provenance();
        assert_eq!(provenance.source.as_deref(), Some("mcp:owt_current"));
        assert_eq!(provenance.request_id.as_deref(), Some("refresh-001"));
        assert_eq!(provenance.requested_at_unix, Some(1779160100));
    }

    #[test]
    fn request_runtime_refresh_records_http_permission_claim_as_pending_approval() {
        let mut document = InterfaceDocument::new(
            "demo.interface",
            "DEMO",
            Scope::new(ScopeKind::Session, "demo"),
        );
        document.allowed_action_kinds = vec![ActionKind::Refresh];
        document.actions.push(UiAction::new(
            "kernel.refresh",
            "Refresh Kernel",
            ActionKind::Refresh,
        ));
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        let runtime = Arc::new(Mutex::new(RuntimeState::default()));
        runtime.lock().unwrap().apply_interface(document).unwrap();
        let parsed = parse_http_request(
            "POST /owt/request_refresh HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: 123\r\n\
             \r\n\
             {\"interface\":\"demo.interface\",\"action_id\":\"kernel.refresh\",\"permission_granted\":true,\"execute\":true,\"source\":\"mcp:owt_current\",\"request_id\":\"refresh-http-001\",\"requested_at_unix\":1779160100}",
        )
        .unwrap();
        let request = parse_request_refresh_body(&parsed).unwrap();

        let requested = request_runtime_refresh(&runtime, &request).unwrap();

        assert_eq!(requested.approval_state, RequestApprovalState::Pending);
        assert!(requested.approval_required);
        assert!(!requested.permission_granted);
        assert_eq!(requested.request_id.as_deref(), Some("refresh-http-001"));
    }

    #[test]
    fn list_action_requests_accepts_filter_options() {
        let parsed = parse_http_request(
            "POST /owt/list_action_requests HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: 88\r\n\
             \r\n\
             {\"interface\":\"demo.interface\",\"limit\":5,\"include_actions\":true,\"include_refreshes\":false,\"pending_only\":true}",
        )
        .unwrap();

        let request = parse_list_action_requests_body(&parsed).unwrap();
        assert_eq!(request.interface_id().as_deref(), Some("demo.interface"));
        assert_eq!(request.limit(), 5);
        assert!(request.include_actions());
        assert!(!request.include_refreshes());
        assert!(request.pending_only());

        let get = parse_http_request("GET /owt/action_requests HTTP/1.1\r\n\r\n").unwrap();
        let request = parse_list_action_requests_body(&get).unwrap();
        assert_eq!(request.limit(), 24);
        assert!(request.include_actions());
        assert!(request.include_refreshes());
    }

    #[test]
    fn list_action_requests_payload_reports_recent_runtime_requests() {
        let mut document = InterfaceDocument::new(
            "demo.interface",
            "DEMO",
            Scope::new(ScopeKind::Session, "demo"),
        );
        document.allowed_action_kinds = vec![ActionKind::Open, ActionKind::Refresh];
        let mut open = UiAction::new("open.workspace", "Open workspace", ActionKind::Open);
        open.command = Some("xdg-open .".to_string());
        document.actions.push(open);
        let mut refresh = UiAction::new("kernel.refresh", "Refresh Kernel", ActionKind::Refresh);
        refresh.target = Some("local:/proc".to_string());
        document.actions.push(refresh);
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));

        let runtime = Arc::new(Mutex::new(RuntimeState::default()));
        {
            let mut state = runtime.lock().unwrap();
            state.apply_interface(document).unwrap();
            state
                .dispatch_action_for_interface_with_intent(
                    Some("demo.interface"),
                    "open.workspace",
                    true,
                    false,
                    true,
                )
                .unwrap();
            state
                .request_refresh(
                    Some("demo.interface"),
                    None,
                    Some("kernel.refresh"),
                    true,
                    true,
                )
                .unwrap();
        }

        let request = ListActionRequestsRequest {
            interface_id: Some("demo.interface".to_string()),
            limit: Some(10),
            pending_only: Some(true),
            ..ListActionRequestsRequest::default()
        };
        let payload = list_action_requests_payload(&runtime, &request).unwrap();

        assert_eq!(payload["pending_action_count"], json!(1));
        assert_eq!(payload["pending_refresh_count"], json!(1));
        assert_eq!(payload["action_requests"][0]["executed"], json!(false));
        assert_eq!(
            payload["action_requests"][0]["approval_state"],
            json!("granted")
        );
        assert_eq!(payload["refresh_requests"][0]["executed"], json!(false));
        assert_eq!(
            payload["refresh_requests"][0]["approval_state"],
            json!("granted")
        );
        assert_eq!(payload["refresh_requests"][0]["kind"], json!("refresh"));
        assert_eq!(
            payload["refresh_requests"][0]["target"],
            json!("local:/proc")
        );
    }

    #[test]
    fn dispatch_action_accepts_permissioned_execution_intent() {
        let parsed = parse_http_request(
            "POST /owt/dispatch_action HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: 117\r\n\
             \r\n\
             {\"interface_id\":\"demo.interface\",\"action_id\":\"kernel.refresh\",\"permission_granted\":true,\"confirmed\":true,\"execute\":true,\"source\":\"mcp:owt_current\",\"request_id\":\"dispatch-001\",\"requested_at_unix\":1779160000}",
        )
        .unwrap();

        let request = parse_dispatch_action_body(&parsed).unwrap();
        assert_eq!(request.interface_id.as_deref(), Some("demo.interface"));
        assert_eq!(request.action_id, "kernel.refresh");
        assert!(request.permission_granted());
        assert!(request.confirmation_granted());
        assert!(request.external_execution_requested());
        let provenance = request.provenance();
        assert_eq!(provenance.source.as_deref(), Some("mcp:owt_current"));
        assert_eq!(provenance.request_id.as_deref(), Some("dispatch-001"));
        assert_eq!(provenance.requested_at_unix, Some(1779160000));
    }

    #[test]
    fn dispatch_runtime_action_records_http_permission_claim_as_pending_approval() {
        let mut document = InterfaceDocument::new(
            "demo.interface",
            "DEMO",
            Scope::new(ScopeKind::Session, "demo"),
        );
        document.allowed_action_kinds = vec![ActionKind::Open];
        document.actions.push(UiAction::new(
            "open.workspace",
            "Open workspace",
            ActionKind::Open,
        ));
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        let runtime = Arc::new(Mutex::new(RuntimeState::default()));
        runtime.lock().unwrap().apply_interface(document).unwrap();
        let parsed = parse_http_request(
            "POST /owt/dispatch_action HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: 117\r\n\
             \r\n\
             {\"interface_id\":\"demo.interface\",\"action_id\":\"open.workspace\",\"permission_granted\":true,\"confirmed\":true,\"execute\":true,\"source\":\"mcp:owt_current\",\"request_id\":\"dispatch-http-001\",\"requested_at_unix\":1779160000}",
        )
        .unwrap();
        let request = parse_dispatch_action_body(&parsed).unwrap();

        let dispatched = dispatch_runtime_action(
            &runtime,
            request.interface_id.as_deref(),
            &request.action_id,
            false,
            false,
            request.external_execution_requested(),
            request.provenance(),
        )
        .unwrap();

        assert_eq!(dispatched.approval_state, RequestApprovalState::Pending);
        assert!(dispatched.approval_required);
        assert!(!dispatched.permission_granted);
        assert!(!dispatched.confirmation_granted);
        assert_eq!(dispatched.request_id.as_deref(), Some("dispatch-http-001"));
    }

    #[test]
    fn renderer_open_file_action_uses_native_execution_without_external_runner() {
        let mut document = InterfaceDocument::new(
            "demo.folders",
            "FOLDERS",
            Scope::new(ScopeKind::Session, "demo.folders"),
        );
        document.allowed_action_kinds = vec![ActionKind::Open, ActionKind::Inspect];
        let mut open = UiAction::new("folders.open.tools", "OPEN TOOLS", ActionKind::Open);
        open.target = Some("file:/home/buanzo/git/tools".to_string());
        document.actions.push(open);
        document.actions.push(UiAction::new(
            "folders.inspect.tools",
            "TOOLS",
            ActionKind::Inspect,
        ));
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        let mut runtime = RuntimeState::default();
        runtime.apply_interface(document).unwrap();

        assert!(!renderer_action_requests_external_execution_from_state(
            &runtime,
            Some("demo.folders"),
            "folders.open.tools",
        ));
        let native = renderer_native_execution_candidate_from_state(
            &runtime,
            Some("demo.folders"),
            "folders.open.tools",
        )
        .expect("native open action");
        assert_eq!(native.interface_id, "demo.folders");
        assert!(!native.root_allowlisted);
        assert!(!renderer_action_requests_external_execution_from_state(
            &runtime,
            Some("demo.folders"),
            "folders.inspect.tools",
        ));

        let runtime = Arc::new(Mutex::new(runtime));
        let dispatched = dispatch_runtime_action(
            &runtime,
            Some("demo.folders"),
            "folders.open.tools",
            false,
            false,
            false,
            ActionProvenance {
                source: Some("native_renderer:surface_action".to_string()),
                ..ActionProvenance::default()
            },
        )
        .unwrap();

        assert_eq!(dispatched.approval_state, RequestApprovalState::NotRequired);
        assert!(!dispatched.approval_required);
        assert!(!dispatched.permission_granted);
        assert!(!dispatched.external_execution_requested);
    }

    #[test]
    fn renderer_run_argv_action_uses_native_execution_candidate() {
        let mut document = InterfaceDocument::new(
            "demo.actions",
            "ACTIONS",
            Scope::new(ScopeKind::Session, "demo.actions"),
        );
        document.allowed_action_kinds = vec![ActionKind::Run];
        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        root.properties.insert(
            "execution_allowlist".to_string(),
            "runner.exec=owt-check --dry-run".to_string(),
        );
        document.nodes.push(root);
        let mut action = UiAction::new("runner.exec", "RUN", ActionKind::Run);
        action.argv = vec!["owt-check".to_string(), "--dry-run".to_string()];
        document.actions.push(action);
        let mut runtime = RuntimeState::default();
        runtime.apply_interface(document).unwrap();

        assert!(!renderer_action_requests_external_execution_from_state(
            &runtime,
            Some("demo.actions"),
            "runner.exec",
        ));
        let native = renderer_native_execution_candidate_from_state(
            &runtime,
            Some("demo.actions"),
            "runner.exec",
        )
        .expect("native run action");
        assert!(native.root_allowlisted);
    }

    #[test]
    fn wsl_paths_convert_to_windows_file_browser_targets() {
        assert_eq!(
            windows_drive_path_from_wsl_mount("/mnt/c/Users/Buanzo/Desktop"),
            Some("C:\\Users\\Buanzo\\Desktop".to_string())
        );
    }

    #[test]
    fn diff_interface_accepts_baseline_and_candidate_ids() {
        let body = "{\"baseline_id\":\"panel.before\",\"candidate_id\":\"panel.after\"}";
        let raw = format!(
            "POST /owt/diff_interface HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );
        let parsed = parse_http_request(&raw).unwrap();

        let request = parse_diff_interface_body(&parsed).unwrap();
        assert_eq!(request.left_id(), Some("panel.before"));
        assert_eq!(request.right_id(), Some("panel.after"));
    }

    #[test]
    fn patch_interface_lifecycle_accepts_pin_hide_and_ttl() {
        let body =
            "{\"interface\":\"demo.interface\",\"pin\":true,\"display\":\"hide\",\"ttl_seconds\":60}";
        let raw = format!(
            "POST /owt/patch_interface_lifecycle HTTP/1.1\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );
        let parsed = parse_http_request(&raw).unwrap();

        let request = parse_patch_interface_lifecycle_body(&parsed).unwrap();
        let patch = request.lifecycle_patch();
        assert_eq!(request.interface_id().as_deref(), Some("demo.interface"));
        assert_eq!(patch.pinned, Some(true));
        assert_eq!(patch.hidden, Some(true));
        assert_eq!(patch.ttl_seconds, Some(60));
        assert!(patch.now_unix.is_some());
    }

    #[test]
    fn active_snapshot_injects_visible_lifecycle_ttl_badge() {
        let mut state = RuntimeState::default();
        let mut document = InterfaceDocument::new(
            "demo.lifecycle",
            "DEMO LIFECYCLE",
            Scope::new(ScopeKind::Session, "demo.lifecycle"),
        );
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        state.apply_interface(document).unwrap();
        state.lifecycle_by_interface.insert(
            "demo.lifecycle".to_string(),
            InterfaceLifecycleState {
                interface_id: "demo.lifecycle".to_string(),
                pinned: true,
                hidden: false,
                expired: false,
                expires_at_unix: Some(current_unix_seconds() + 90),
                ttl_seconds: Some(120),
                previous_hidden: None,
                message: None,
            },
        );

        let snapshots = active_interface_snapshots_from_state(&state);

        assert_eq!(snapshots.len(), 1);
        let root = &snapshots[0].nodes[0];
        assert_eq!(
            root.properties.get("lifecycle_pinned").map(String::as_str),
            Some("true")
        );
        assert!(root.properties.contains_key("lifecycle_remaining_seconds"));
        let badge = root
            .children
            .iter()
            .find(|node| node.id == "owt.lifecycle.status")
            .expect("lifecycle badge");
        assert_eq!(badge.label.as_deref(), Some("LIFECYCLE"));
        assert!(badge.text.as_deref().unwrap_or_default().contains("PINNED"));
        assert!(badge.text.as_deref().unwrap_or_default().contains("TTL"));
    }

    #[test]
    fn active_snapshot_injects_stale_freshness_badge() {
        let mut state = RuntimeState::default();
        let mut document = InterfaceDocument::new(
            "demo.stale",
            "DEMO STALE",
            Scope::new(ScopeKind::Session, "demo.stale"),
        );
        let mut root = UiNode::new("panel.root", UiNodeKind::Panel);
        root.properties.insert(
            "collected_at_unix".to_string(),
            current_unix_seconds().saturating_sub(90).to_string(),
        );
        root.properties
            .insert("refresh_interval_seconds".to_string(), "30".to_string());
        document.nodes.push(root);
        state.apply_interface(document).unwrap();

        let snapshots = active_interface_snapshots_from_state(&state);

        assert_eq!(snapshots.len(), 1);
        let root = &snapshots[0].nodes[0];
        assert_eq!(
            root.properties.get("freshness_state").map(String::as_str),
            Some("stale")
        );
        assert_eq!(
            root.properties
                .get("freshness_stale_after_seconds")
                .map(String::as_str),
            Some("30")
        );
        let badge = root
            .children
            .iter()
            .find(|node| node.id == "owt.freshness.status")
            .expect("freshness badge");
        assert_eq!(badge.label.as_deref(), Some("FRESHNESS"));
        assert!(badge.text.as_deref().unwrap_or_default().contains("STALE"));
    }

    #[test]
    fn elapsed_lifecycle_ttl_is_omitted_from_active_render_status() {
        let mut state = RuntimeState::default();
        let mut document = InterfaceDocument::new(
            "demo.expired",
            "DEMO EXPIRED",
            Scope::new(ScopeKind::Session, "demo.expired"),
        );
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        state.apply_interface(document).unwrap();
        state.lifecycle_by_interface.insert(
            "demo.expired".to_string(),
            InterfaceLifecycleState {
                interface_id: "demo.expired".to_string(),
                pinned: false,
                hidden: false,
                expired: false,
                expires_at_unix: Some(1),
                ttl_seconds: Some(1),
                previous_hidden: None,
                message: None,
            },
        );

        let snapshots = active_interface_snapshots_from_state(&state);
        let statuses = interface_statuses_from_state(&state, &snapshots);

        assert!(snapshots.is_empty());
        assert!(!statuses[0].active);
        assert!(statuses[0].lifecycle.as_ref().unwrap().expired);
    }

    #[test]
    fn interface_status_projects_scope_owner() {
        let mut state = RuntimeState::default();
        let mut document = InterfaceDocument::new(
            "demo.project",
            "DEMO PROJECT",
            Scope::new(ScopeKind::Project, "/home/buanzo/git/tools"),
        );
        document
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        state.apply_interface(document).unwrap();

        let snapshots = active_interface_snapshots_from_state(&state);
        let statuses = interface_statuses_from_state(&state, &snapshots);
        let owner = statuses[0].owner.as_ref().expect("interface owner");

        assert_eq!(owner.kind, ScopeKind::Project);
        assert_eq!(owner.id, "/home/buanzo/git/tools");
        assert_eq!(owner.scope_key, "Project:/home/buanzo/git/tools");
    }

    #[test]
    fn interface_status_uses_reflowed_projection_for_compact_button_conflict() {
        let mut state = RuntimeState::default();
        let mut button = InterfaceDocument::new(
            "carriersingles.renders.button",
            "CARRIERSINGLES",
            Scope::new(
                ScopeKind::Project,
                "/home/buanzo/git/tools/python/carriersingles",
            ),
        );
        let mut button_root = UiNode::new("panel.root", UiNodeKind::Panel);
        button_root
            .properties
            .insert("profile".to_string(), "single_button".to_string());
        button.nodes.push(button_root);

        let mut primary = InterfaceDocument::new(
            "genetica.native.scope",
            "GENETICA LOCAL ONLY",
            Scope::new(ScopeKind::Project, "/home/buanzo/git/tools/data/genetica"),
        );
        primary
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));

        state.apply_interface(button).unwrap();
        state.apply_interface(primary).unwrap();

        let snapshots = active_interface_snapshots_from_state(&state);
        let statuses = interface_statuses_from_state(&state, &snapshots);
        let button_status = statuses
            .iter()
            .find(|status| status.interface_id == "carriersingles.renders.button")
            .unwrap();

        assert_eq!(
            snapshots.first().map(|document| document.id.as_str()),
            Some("genetica.native.scope")
        );
        assert_eq!(
            button_status
                .render_projection
                .get("layout")
                .map(String::as_str),
            Some("bottom_strip")
        );
        assert_eq!(
            button_status
                .render_projection
                .get("reservation")
                .map(String::as_str),
            Some("reserved")
        );
    }

    #[test]
    fn interface_status_exposes_focused_table_cell() {
        let mut state = RuntimeState::default();
        let mut document = InterfaceDocument::new(
            "demo.focus",
            "DEMO FOCUS",
            Scope::new(ScopeKind::Session, "demo.focus"),
        );
        let mut table = UiNode::new("demo.table", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|state".to_string());
        table
            .properties
            .insert("focused_column".to_string(), "kernel".to_string());
        document.nodes.push(table);
        state.apply_interface(document).unwrap();

        let snapshots = active_interface_snapshots_from_state(&state);
        let statuses = interface_statuses_from_state(&state, &snapshots);
        let focused = statuses[0]
            .focused_table_cell
            .as_ref()
            .expect("focused table cell");

        assert_eq!(focused.interface_id, "demo.focus");
        assert_eq!(focused.table_node_id, "demo.table");
        assert_eq!(focused.focused_column, "kernel");
        assert_eq!(focused.column_index, 1);
    }

    #[test]
    fn list_interfaces_groups_runtime_interfaces_by_owner() {
        let mut state = RuntimeState::default();
        let scope = Scope::new(ScopeKind::Project, "/home/buanzo/git/tools");
        let mut first = InterfaceDocument::new("demo.project.first", "FIRST", scope.clone());
        first
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        let mut second = InterfaceDocument::new("demo.project.second", "SECOND", scope);
        second
            .nodes
            .push(UiNode::new("panel.root", UiNodeKind::Panel));
        state.apply_interface(first).unwrap();
        state.apply_interface(second).unwrap();
        let runtime = std::sync::Arc::new(std::sync::Mutex::new(state));

        let payload = list_interfaces_payload(&runtime).unwrap();
        let groups = payload["owner_groups"].as_array().expect("owner groups");
        let project_group = groups
            .iter()
            .find(|group| group["owner"]["id"] == "/home/buanzo/git/tools")
            .expect("project owner group");

        assert_eq!(project_group["owner"]["kind"], "project");
        assert_eq!(project_group["count"], 2);
        assert_eq!(project_group["active_count"], 1);
        assert_eq!(project_group["interface_ids"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn saved_interface_summary_projects_owner_label() {
        let summary = saved_interface_summary_from_value(json!({
            "store_id": "kernel-left",
            "interface_id": "tools.kernel.left",
            "title": "Kernel Left",
            "owner": {
                "kind": "project",
                "id": "/home/buanzo/git/tools",
                "scope_key": "Project:/home/buanzo/git/tools"
            }
        }))
        .unwrap();

        assert_eq!(summary.owner_label.as_deref(), Some("PROJECT tools"));
        assert_eq!(
            summary.owner_key.as_deref(),
            Some("project:/home/buanzo/git/tools")
        );
    }
}
