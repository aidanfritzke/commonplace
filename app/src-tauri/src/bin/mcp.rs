// Commonplace MCP server — read-only access to a vault's notes for local AI
// agents (Claude Desktop / Claude Code) over stdio JSON-RPC.
//
// Air-gapped by design: it opens the same on-disk LanceDB index read-only and
// embeds queries via the app's local embed server (127.0.0.1:11501), so the
// Commonplace app must be running for the semantic tools. There are NO write
// tools — this server can only read.
//
// Configure in an MCP client with:
//   command = <path to this binary>
//   args    = ["--vault", "<absolute path to your vault>"]
//
// stdio transport = newline-delimited JSON-RPC 2.0. stdout carries ONLY protocol
// messages; anything diagnostic goes to stderr.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use arrow_array::StringArray;
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::DistanceType;
use serde_json::{json, Value};

const EMBED_URL: &str = "http://127.0.0.1:11501/v1/embeddings";
const TABLE: &str = "chunks";
const PROTOCOL_VERSION: &str = "2024-11-05";

fn esc(s: &str) -> String {
    s.replace('\'', "''")
}

/// Where the app stores its LanceDB index — mirrors Tauri's `app_data_dir`.
/// Windows (the app's primary OS): %APPDATA%\com.commonplace.app. A best-effort
/// XDG fallback keeps it working if ever built elsewhere.
fn db_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = PathBuf::from(std::env::var("APPDATA").ok()?);
    #[cfg(not(windows))]
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| std::env::var("HOME").ok().map(|h| Path::new(&h).join(".local/share")))?;
    Some(base.join("com.commonplace.app").join("lancedb"))
}

async fn embed(client: &reqwest::Client, text: &str) -> Result<Vec<f32>, String> {
    let body = json!({ "model": "local", "input": text });
    let resp = client
        .post(EMBED_URL)
        .json(&body)
        .send()
        .await
        .map_err(|_| "Commonplace isn't running — start the app to enable semantic search.".to_string())?;
    if !resp.status().is_success() {
        return Err(format!("embed HTTP {}", resp.status()));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let arr = v
        .pointer("/data/0/embedding")
        .and_then(|e| e.as_array())
        .ok_or("no embedding in response")?;
    Ok(arr.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect())
}

/// Vector search over the vault's chunks, deduped to one (best) hit per note.
async fn vector_search(
    vault: &str,
    query_vec: Vec<f32>,
    exclude_path: Option<&str>,
    k: usize,
) -> Result<Vec<Value>, String> {
    let dir = db_path().ok_or("cannot resolve the app data dir")?;
    let conn = lancedb::connect(dir.to_str().ok_or("bad db path")?)
        .execute()
        .await
        .map_err(|e| e.to_string())?;
    let names = conn.table_names().execute().await.map_err(|e| e.to_string())?;
    if !names.iter().any(|n| n == TABLE) {
        return Ok(vec![]);
    }
    let table = conn.open_table(TABLE).execute().await.map_err(|e| e.to_string())?;
    let mut filter = format!("vault = '{}'", esc(vault));
    if let Some(ex) = exclude_path {
        filter.push_str(&format!(" AND path <> '{}'", esc(ex)));
    }
    let stream = table
        .query()
        .only_if(filter)
        .nearest_to(query_vec)
        .map_err(|e| e.to_string())?
        .distance_type(DistanceType::Cosine)
        .select(Select::Columns(vec![
            "path".into(),
            "name".into(),
            "rel".into(),
            "text".into(),
        ]))
        .limit(k * 6)
        .execute()
        .await
        .map_err(|e| e.to_string())?;
    let batches = stream.try_collect::<Vec<_>>().await.map_err(|e| e.to_string())?;

    let mut seen = std::collections::HashSet::new();
    let mut hits = Vec::new();
    for b in &batches {
        let col = |name: &str| -> Option<&StringArray> {
            let i = b.schema().index_of(name).ok()?;
            b.column(i).as_any().downcast_ref::<StringArray>()
        };
        let (path, name, rel, text) = match (col("path"), col("name"), col("rel"), col("text")) {
            (Some(p), Some(n), Some(r), Some(t)) => (p, n, r, t),
            _ => continue,
        };
        for row in 0..b.num_rows() {
            let p = path.value(row).to_string();
            if Some(p.as_str()) == exclude_path {
                continue;
            }
            if !seen.insert(p.clone()) {
                continue;
            }
            hits.push(json!({
                "name": name.value(row),
                "rel": rel.value(row),
                "text": text.value(row),
            }));
            if hits.len() >= k {
                return Ok(hits);
            }
        }
    }
    Ok(hits)
}

fn read_note(vault: &str, rel: &str) -> Result<String, String> {
    let root = std::fs::canonicalize(vault).map_err(|e| e.to_string())?;
    let target = std::fs::canonicalize(root.join(rel)).map_err(|e| e.to_string())?;
    if !target.starts_with(&root) {
        return Err("refused: path is outside the vault".into());
    }
    std::fs::read_to_string(&target).map_err(|e| e.to_string())
}

fn list_notes(vault: &str) -> Vec<String> {
    let root = Path::new(vault);
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
        })
        .map(|e| {
            e.path()
                .strip_prefix(root)
                .unwrap_or(e.path())
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

fn tool_text(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }] })
}
fn tool_error(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }], "isError": true })
}

fn tools_schema() -> Value {
    json!([
        {
            "name": "list_notes",
            "description": "List every note in the vault (vault-relative paths). No arguments.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "read_note",
            "description": "Read a note's full Markdown by its vault-relative path (e.g. \"ai/ideas.md\").",
            "inputSchema": {
                "type": "object",
                "properties": { "rel": { "type": "string", "description": "vault-relative path" } },
                "required": ["rel"]
            }
        },
        {
            "name": "search",
            "description": "Semantic search across the vault for a free-text query. Returns the most relevant notes. Requires the Commonplace app to be running.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "k": { "type": "integer", "description": "max results (default 5)" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "related_notes",
            "description": "Find notes most similar in meaning to a given note (by its vault-relative path). Requires the Commonplace app to be running.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "rel": { "type": "string", "description": "vault-relative path of the seed note" },
                    "k": { "type": "integer", "description": "max results (default 5)" }
                },
                "required": ["rel"]
            }
        }
    ])
}

fn as_k(args: &Value, default: usize) -> usize {
    args.get("k").and_then(|v| v.as_u64()).map(|n| n as usize).filter(|n| *n > 0).unwrap_or(default)
}

async fn call_tool(name: &str, args: &Value, vault: &str, client: &reqwest::Client) -> Value {
    match name {
        "list_notes" => {
            let notes = list_notes(vault);
            if notes.is_empty() {
                tool_text("(no notes found in the vault)")
            } else {
                tool_text(notes.join("\n"))
            }
        }
        "read_note" => match args.get("rel").and_then(|v| v.as_str()) {
            Some(rel) => match read_note(vault, rel) {
                Ok(text) => tool_text(text),
                Err(e) => tool_error(e),
            },
            None => tool_error("missing required argument: rel"),
        },
        "search" => match args.get("query").and_then(|v| v.as_str()) {
            Some(query) => {
                let q: String = query.chars().take(2000).collect();
                match embed(client, &q).await {
                    Ok(vec) => match vector_search(vault, vec, None, as_k(args, 5)).await {
                        Ok(hits) => tool_text(format_hits(&hits)),
                        Err(e) => tool_error(e),
                    },
                    Err(e) => tool_error(e),
                }
            }
            None => tool_error("missing required argument: query"),
        },
        "related_notes" => match args.get("rel").and_then(|v| v.as_str()) {
            Some(rel) => {
                let seed = match read_note(vault, rel) {
                    Ok(t) => t,
                    Err(e) => return tool_error(e),
                };
                let seed: String = seed.chars().take(2000).collect();
                let self_path = Path::new(vault).join(rel).to_string_lossy().replace('\\', "/");
                match embed(client, &seed).await {
                    Ok(vec) => match vector_search(vault, vec, Some(&self_path), as_k(args, 5)).await {
                        Ok(hits) => tool_text(format_hits(&hits)),
                        Err(e) => tool_error(e),
                    },
                    Err(e) => tool_error(e),
                }
            }
            None => tool_error("missing required argument: rel"),
        },
        other => tool_error(format!("unknown tool: {other}")),
    }
}

fn format_hits(hits: &[Value]) -> String {
    if hits.is_empty() {
        return "(no matching notes)".into();
    }
    hits.iter()
        .map(|h| {
            let rel = h.get("rel").and_then(|v| v.as_str()).unwrap_or("");
            let text = h.get("text").and_then(|v| v.as_str()).unwrap_or("");
            format!("[{}]\n{}", rel, text)
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}
fn err(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

async fn handle(req: &Value, vault: &str, client: &reqwest::Client) -> Option<Value> {
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = req.get("id").cloned();
    // notifications (no id) get no response
    let id = match id {
        Some(v) if !v.is_null() => v,
        _ => return None,
    };
    match method {
        "initialize" => Some(ok(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "commonplace", "version": env!("CARGO_PKG_VERSION") }
            }),
        )),
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tools_schema() }))),
        "tools/call" => {
            let params = req.get("params").cloned().unwrap_or(json!({}));
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            Some(ok(id, call_tool(name, &args, vault, client).await))
        }
        _ => Some(err(id, -32601, "method not found")),
    }
}

fn parse_vault() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--vault" {
            return args.next();
        }
        if let Some(v) = a.strip_prefix("--vault=") {
            return Some(v.to_string());
        }
    }
    std::env::var("COMMONPLACE_VAULT").ok()
}

#[tokio::main]
async fn main() {
    let vault = match parse_vault() {
        Some(v) => v,
        None => {
            eprintln!("commonplace-mcp: no vault given. Pass --vault <path> or set COMMONPLACE_VAULT.");
            std::process::exit(2);
        }
    };
    eprintln!("commonplace-mcp: serving vault {vault}");
    let client = reqwest::Client::new();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(_) => break,
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(resp) = handle(&req, &vault, &client).await {
            if writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap()).is_err() {
                break;
            }
            let _ = stdout.flush();
        }
    }
}
