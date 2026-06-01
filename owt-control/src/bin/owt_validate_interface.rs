use owt_control::{interface_validation_warnings, validate_interface, InterfaceDocument};
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn usage(program: &str) {
    eprintln!("Usage: {program} [--pretty] <interface.json> [interface.json ...]");
}

fn validate_path(path: &Path) -> serde_json::Value {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            return json!({
                "path": path,
                "ok": false,
                "error": format!("cannot read interface document: {err}"),
            });
        }
    };
    let document: InterfaceDocument = match serde_json::from_str(&content) {
        Ok(document) => document,
        Err(err) => {
            return json!({
                "path": path,
                "ok": false,
                "error": format!("cannot parse InterfaceDocument: {err}"),
            });
        }
    };
    if let Err(err) = validate_interface(&document) {
        return json!({
            "path": path,
            "ok": false,
            "interface_id": document.id,
            "title": document.title,
            "error": err.to_string(),
        });
    }
    let warnings = interface_validation_warnings(&document);
    json!({
        "path": path,
        "ok": true,
        "interface_id": document.id,
        "title": document.title,
        "warning_count": warnings.len(),
        "warnings": warnings,
    })
}

fn main() {
    let mut pretty = false;
    let mut paths = Vec::new();
    let mut args = env::args();
    let program = args
        .next()
        .unwrap_or_else(|| "owt-validate-interface".to_string());
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => {
                usage(&program);
                return;
            }
            "--pretty" => pretty = true,
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    if paths.is_empty() {
        usage(&program);
        std::process::exit(64);
    }
    let results: Vec<_> = paths.iter().map(|path| validate_path(path)).collect();
    let ok = results
        .iter()
        .all(|result| result.get("ok").and_then(|value| value.as_bool()) == Some(true));
    let output = json!({
        "ok": ok,
        "validated": results.len(),
        "results": results,
    });
    let encoded = if pretty {
        serde_json::to_string_pretty(&output)
    } else {
        serde_json::to_string(&output)
    };
    match encoded {
        Ok(body) => println!("{body}"),
        Err(err) => {
            eprintln!("cannot serialize validation result: {err}");
            std::process::exit(1);
        }
    }
    if !ok {
        std::process::exit(1);
    }
}
