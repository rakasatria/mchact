use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;

use crate::web::{middleware::AuthScope, require_scope, WebState};

#[derive(Debug, Serialize)]
struct McpServerStatus {
    name: String,
    tools: Vec<McpToolStatus>,
}

#[derive(Debug, Serialize)]
struct McpToolStatus {
    qualified_name: String,
    original_name: String,
    description: String,
}

fn mcp_config_path(state: &WebState) -> PathBuf {
    state.app_state.config.data_root_dir().join("mcp.json")
}

pub async fn api_list_mcp(
    headers: HeaderMap,
    State(state): State<WebState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    require_scope(&state, &headers, AuthScope::Read).await?;

    let definitions = state.app_state.tools.definitions();
    let mut servers: std::collections::BTreeMap<String, Vec<McpToolStatus>> =
        std::collections::BTreeMap::new();

    for def in &definitions {
        if let Some(rest) = def.description.strip_prefix("[MCP:") {
            if let Some(bracket_end) = rest.find(']') {
                let server_name = rest[..bracket_end].to_string();
                let description = rest[bracket_end + 1..].trim().to_string();
                let original_name = def
                    .name
                    .strip_prefix(&format!("mcp_{}_", server_name))
                    .unwrap_or(&def.name)
                    .to_string();

                servers
                    .entry(server_name)
                    .or_default()
                    .push(McpToolStatus {
                        qualified_name: def.name.clone(),
                        original_name,
                        description,
                    });
            }
        }
    }

    let result: Vec<McpServerStatus> = servers
        .into_iter()
        .map(|(name, tools)| McpServerStatus { name, tools })
        .collect();

    Ok(Json(json!({
        "ok": true,
        "servers": result
    })))
}

/// Read the raw mcp.json config for editing in the UI.
pub async fn api_get_mcp_config(
    headers: HeaderMap,
    State(state): State<WebState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    require_scope(&state, &headers, AuthScope::Read).await?;

    let path = mcp_config_path(&state);
    let config = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Invalid mcp.json: {e}"),
                )
            })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            json!({"mcpServers": {}})
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read mcp.json: {e}"),
            ));
        }
    };

    Ok(Json(json!({
        "ok": true,
        "config": config,
        "path": path.to_string_lossy()
    })))
}

/// Write the mcp.json config. Accepts the full config object.
/// Changes take effect after restart.
pub async fn api_put_mcp_config(
    headers: HeaderMap,
    State(state): State<WebState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    require_scope(&state, &headers, AuthScope::Write).await?;

    let config = body
        .get("config")
        .ok_or((StatusCode::BAD_REQUEST, "Missing 'config' field".to_string()))?;

    // Validate structure
    let servers = config.get("mcpServers").and_then(|v| v.as_object());
    if servers.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "config must contain 'mcpServers' object".to_string(),
        ));
    }

    // Validate each server entry has required fields
    for (name, server) in servers.unwrap() {
        let obj = server.as_object().ok_or((
            StatusCode::BAD_REQUEST,
            format!("Server '{name}' must be an object"),
        ))?;
        let transport = obj
            .get("transport")
            .and_then(|v| v.as_str())
            .unwrap_or("stdio");
        match transport {
            "stdio" => {
                if obj.get("command").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!("Server '{name}': stdio transport requires 'command'"),
                    ));
                }
            }
            "streamable_http" => {
                let endpoint = obj
                    .get("endpoint")
                    .or_else(|| obj.get("url"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if endpoint.is_empty() {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!("Server '{name}': streamable_http transport requires 'endpoint'"),
                    ));
                }
            }
            other => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("Server '{name}': unknown transport '{other}' (use 'stdio' or 'streamable_http')"),
                ));
            }
        }
    }

    let path = mcp_config_path(&state);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create directory: {e}"),
            )
        })?;
    }

    let pretty = serde_json::to_string_pretty(config).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to serialize config: {e}"),
        )
    })?;

    std::fs::write(&path, &pretty).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to write mcp.json: {e}"),
        )
    })?;

    Ok(Json(json!({
        "ok": true,
        "message": "MCP config saved. Restart mchact to apply changes."
    })))
}
