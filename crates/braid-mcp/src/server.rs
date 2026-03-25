use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use crate::tools::McpToolRegistry;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }
}

fn handle_request(registry: &McpToolRegistry, req: &JsonRpcRequest) -> JsonRpcResponse {
    let id = req.id.clone().unwrap_or(Value::Null);

    match req.method.as_str() {
        "initialize" => JsonRpcResponse::success(
            id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "braid-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        ),
        "notifications/initialized" => {
            // Notification — no response needed, but return an ack
            JsonRpcResponse::success(id, Value::Null)
        }
        "tools/list" => {
            let tools: Vec<Value> = registry
                .list_tools()
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.parameters,
                    })
                })
                .collect();
            JsonRpcResponse::success(id, serde_json::json!({ "tools": tools }))
        }
        "tools/call" => {
            let name = req.params["name"].as_str().unwrap_or("");
            let arguments = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Object(Default::default()));

            match registry.call_tool(name, arguments) {
                Ok(result) => JsonRpcResponse::success(
                    id,
                    serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": result.output
                        }]
                    }),
                ),
                Err(e) => JsonRpcResponse::error(id, -32602, e.to_string()),
            }
        }
        _ => JsonRpcResponse::error(id, -32601, format!("method not found: {}", req.method)),
    }
}

/// Send a JSON-RPC response with Content-Length framing.
async fn send_response(stdout: &mut tokio::io::Stdout, resp: &JsonRpcResponse) -> Result<()> {
    let json = serde_json::to_string(resp)?;
    let header = format!("Content-Length: {}\r\n\r\n", json.len());
    stdout.write_all(header.as_bytes()).await?;
    stdout.write_all(json.as_bytes()).await?;
    stdout.flush().await?;
    Ok(())
}

/// Run the MCP server over stdio using Content-Length framed JSON-RPC (MCP spec).
pub async fn run_mcp_server(registry: McpToolRegistry) -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);

    loop {
        // Read headers until blank line, extract Content-Length
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                return Ok(()); // EOF
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break; // end of headers
            }
            if let Some(rest) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = rest.trim().parse().ok();
            }
        }

        let len = match content_length {
            Some(n) => n,
            None => continue, // no Content-Length header, skip
        };

        // Read exactly len bytes for the body
        let mut body = vec![0u8; len];
        reader.read_exact(&mut body).await?;

        let req: JsonRpcRequest = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(Value::Null, -32700, format!("parse error: {e}"));
                send_response(&mut stdout, &resp).await?;
                continue;
            }
        };

        // Notifications don't need responses
        if req.method == "notifications/initialized" {
            continue;
        }

        let resp = handle_request(&registry, &req);
        send_response(&mut stdout, &resp).await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::echo::echo_tool;

    fn make_registry() -> McpToolRegistry {
        McpToolRegistry::new(|call| {
            let input: serde_json::Value =
                serde_json::from_str(&call.input).unwrap_or(serde_json::Value::Null);
            let message = input["message"]
                .as_str()
                .unwrap_or("no message")
                .to_string();
            Ok(braid_model::ToolResult {
                name: call.name,
                output: message,
            })
        })
        .register(echo_tool())
    }

    fn make_request(method: &str, params: Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::Number(1.into())),
            method: method.into(),
            params,
        }
    }

    #[test]
    fn handle_initialize() {
        let registry = make_registry();
        let req = make_request("initialize", Value::Object(Default::default()));
        let resp = handle_request(&registry, &req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn handle_tools_list() {
        let registry = make_registry();
        let req = make_request("tools/list", Value::Object(Default::default()));
        let resp = handle_request(&registry, &req);
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "echo");
    }

    #[test]
    fn handle_tools_call() {
        let registry = make_registry();
        let req = make_request(
            "tools/call",
            serde_json::json!({
                "name": "echo",
                "arguments": {"message": "hello world"}
            }),
        );
        let resp = handle_request(&registry, &req);
        let result = resp.result.unwrap();
        let content = result["content"].as_array().unwrap();
        assert_eq!(content[0]["text"], "hello world");
    }

    #[test]
    fn handle_unknown_method() {
        let registry = make_registry();
        let req = make_request("unknown/method", Value::Object(Default::default()));
        let resp = handle_request(&registry, &req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn handle_unknown_tool_call() {
        let registry = make_registry();
        let req = make_request(
            "tools/call",
            serde_json::json!({
                "name": "nonexistent",
                "arguments": {}
            }),
        );
        let resp = handle_request(&registry, &req);
        assert!(resp.error.is_some());
    }
}
