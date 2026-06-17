// Commonplace — persistent vector index (LanceDB).
//
// The embeddings live in a single LanceDB table ("chunks"), namespaced by a
// `vault` column so multiple vaults can share one store. A small manifest.json
// tracks each file's mtime, so reopening a vault only re-embeds files that
// actually changed — the index survives restarts and reopening is instant.
//
// Embedding and vector search happen here in Rust; the frontend just calls
// `index_vault`, `search_notes`, and `related_notes`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::types::Float32Type;
use arrow_array::{FixedSizeListArray, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::{DistanceType, Table};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::Manager;

const TABLE: &str = "chunks";

#[derive(Serialize)]
pub struct Hit {
    pub path: String,
    pub name: String,
    pub rel: String,
    pub text: String,
}

#[derive(Serialize)]
pub struct NoteLinks {
    pub rel: String,
    pub name: String,
    pub links: Vec<String>, // [[targets]] this note points at (deduped)
}

// manifest: vault -> (file path -> mtime seconds)
type Manifest = HashMap<String, HashMap<String, i64>>;

// ---------- paths ----------

fn app_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
fn db_path(app: &tauri::AppHandle) -> Result<String, String> {
    let p = app_dir(app)?.join("lancedb");
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().into_owned())
}
fn manifest_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_dir(app)?.join("manifest.json"))
}
fn load_manifest(app: &tauri::AppHandle) -> Manifest {
    manifest_path(app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn save_manifest(app: &tauri::AppHandle, m: &Manifest) -> Result<(), String> {
    let p = manifest_path(app)?;
    std::fs::write(p, serde_json::to_string(m).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

// ---------- helpers ----------

fn esc(s: &str) -> String {
    s.replace('\'', "''")
}

fn mtime_of(p: &Path) -> i64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// Paragraph chunking — mirrors the v0 logic (<=900 chars, drop tiny fragments).
fn chunk(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split("\n\n") {
        let p = para.trim();
        if p.chars().count() <= 30 {
            continue;
        }
        if p.len() <= 900 {
            out.push(p.to_string());
        } else {
            let bytes = p.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                let end = (i + 900).min(bytes.len());
                // respect char boundaries
                let mut e = end;
                while e < bytes.len() && (bytes[e] & 0xC0) == 0x80 {
                    e += 1;
                }
                out.push(String::from_utf8_lossy(&p.as_bytes()[i..e]).into_owned());
                i += 800;
            }
        }
    }
    out
}

// Extract [[wikilink]] targets from note text, in document order, deduped.
// Ignores fenced code blocks; inner text with a stray '[' is skipped.
fn extract_links(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut rest = line;
        while let Some(open) = rest.find("[[") {
            let after = &rest[open + 2..];
            match after.find("]]") {
                Some(close) => {
                    let inner = after[..close].trim();
                    if !inner.is_empty() && !inner.contains('[') && !out.iter().any(|x| x == inner) {
                        out.push(inner.to_string());
                    }
                    rest = &after[close + 2..];
                }
                None => break,
            }
        }
    }
    out
}

struct FileRow {
    path: String,
    name: String,
    rel: String,
}

fn walk_markdown(root: &Path) -> Vec<FileRow> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let is_md = path
            .extension()
            .map(|x| x.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if !is_md {
            continue;
        }
        out.push(FileRow {
            path: path.to_string_lossy().into_owned(),
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            rel: path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/"),
        });
    }
    out
}

async fn embed_text(client: &reqwest::Client, text: &str) -> Result<Vec<f32>, String> {
    // Bundled llama-server embedding endpoint (OpenAI-compatible).
    let body = serde_json::json!({ "model": "local", "input": text });
    let resp = client
        .post(format!(
            "http://{}:{}/v1/embeddings",
            crate::engine::HOST,
            crate::engine::EMBED_PORT
        ))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("embeddings HTTP {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let arr = v
        .pointer("/data/0/embedding")
        .and_then(|e| e.as_array())
        .ok_or("no embedding in response")?;
    Ok(arr
        .iter()
        .filter_map(|x| x.as_f64().map(|f| f as f32))
        .collect())
}

fn chunks_schema(dim: i32) -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("vault", DataType::Utf8, false),
        Field::new("path", DataType::Utf8, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("rel", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
            false,
        ),
    ]))
}

// One embedded chunk awaiting insertion.
struct Chunk {
    vault: String,
    path: String,
    name: String,
    rel: String,
    text: String,
    vec: Vec<f32>,
}

fn build_batch(schema: SchemaRef, rows: &[Chunk], dim: i32) -> Result<RecordBatch, String> {
    let vault = StringArray::from(rows.iter().map(|r| r.vault.as_str()).collect::<Vec<_>>());
    let path = StringArray::from(rows.iter().map(|r| r.path.as_str()).collect::<Vec<_>>());
    let name = StringArray::from(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>());
    let rel = StringArray::from(rows.iter().map(|r| r.rel.as_str()).collect::<Vec<_>>());
    let text = StringArray::from(rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>());
    let vectors = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        rows.iter()
            .map(|r| Some(r.vec.iter().map(|x| Some(*x)).collect::<Vec<_>>())),
        dim,
    );
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(vault),
            Arc::new(path),
            Arc::new(name),
            Arc::new(rel),
            Arc::new(text),
            Arc::new(vectors),
        ],
    )
    .map_err(|e| e.to_string())
}

async fn connect(app: &tauri::AppHandle) -> Result<lancedb::Connection, String> {
    lancedb::connect(&db_path(app)?)
        .execute()
        .await
        .map_err(|e| e.to_string())
}

async fn open_chunks(conn: &lancedb::Connection) -> Result<Option<Table>, String> {
    let names = conn.table_names().execute().await.map_err(|e| e.to_string())?;
    if names.iter().any(|n| n == TABLE) {
        Ok(Some(
            conn.open_table(TABLE).execute().await.map_err(|e| e.to_string())?,
        ))
    } else {
        Ok(None)
    }
}

// ---------- commands ----------

/// Incrementally sync a vault's index: embed new/changed files, drop removed
/// ones, skip unchanged. Safe to call on every open and after every save.
#[tauri::command]
pub async fn index_vault(
    app: tauri::AppHandle,
    dir: String,
    on_status: Channel<String>,
) -> Result<(), String> {
    if !crate::engine::wait_health(crate::engine::EMBED_PORT, 90).await {
        let _ = on_status.send("model not reachable".into());
        return Err("embedding model is still loading (timed out)".into());
    }
    let client = reqwest::Client::new();
    let conn = connect(&app).await?;
    let mut table = open_chunks(&conn).await?;

    let mut manifest = load_manifest(&app);
    let prev = manifest.get(&dir).cloned().unwrap_or_default();
    let mut next: HashMap<String, i64> = HashMap::new();

    let files = walk_markdown(Path::new(&dir));
    let on_disk: std::collections::HashSet<&str> = files.iter().map(|f| f.path.as_str()).collect();

    // Files removed from disk since last time -> delete their rows.
    if let Some(t) = &table {
        for old_path in prev.keys() {
            if !on_disk.contains(old_path.as_str()) {
                let pred = format!("vault = '{}' AND path = '{}'", esc(&dir), esc(old_path));
                t.delete(&pred).await.map_err(|e| e.to_string())?;
            }
        }
    }

    let total = files.len();
    let mut pending: Vec<Chunk> = Vec::new();
    let mut changed = 0usize;

    for (i, f) in files.iter().enumerate() {
        let mt = mtime_of(Path::new(&f.path));
        next.insert(f.path.clone(), mt);

        let unchanged = table.is_some() && prev.get(&f.path).copied() == Some(mt);
        if unchanged {
            continue;
        }
        changed += 1;
        let _ = on_status.send(format!("indexing {}/{}…", i + 1, total));

        // re-embed: drop any stale rows for this file first
        if let Some(t) = &table {
            let pred = format!("vault = '{}' AND path = '{}'", esc(&dir), esc(&f.path));
            t.delete(&pred).await.map_err(|e| e.to_string())?;
        }

        let text = match std::fs::read_to_string(&f.path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        for ch in chunk(&text) {
            let vec = embed_text(&client, &ch).await?;
            pending.push(Chunk {
                vault: dir.clone(),
                path: f.path.clone(),
                name: f.name.clone(),
                rel: f.rel.clone(),
                text: ch,
                vec,
            });
        }
    }

    if !pending.is_empty() {
        let dim = pending[0].vec.len() as i32;
        let schema = chunks_schema(dim);
        if table.is_none() {
            let t = conn
                .create_empty_table(TABLE, schema.clone())
                .execute()
                .await
                .map_err(|e| e.to_string())?;
            table = Some(t);
        }
        let batch = build_batch(schema, &pending, dim)?;
        table
            .as_ref()
            .unwrap()
            .add(batch)
            .execute()
            .await
            .map_err(|e| e.to_string())?;
    }

    manifest.insert(dir.clone(), next);
    save_manifest(&app, &manifest)?;

    let _ = changed;
    let _ = on_status.send("indexed".to_string());
    Ok(())
}

fn read_hits(batches: &[RecordBatch], exclude_path: Option<&str>, k: usize) -> Vec<Hit> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<Hit> = Vec::new();
    for b in batches {
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
            if let Some(ex) = exclude_path {
                if p == ex {
                    continue;
                }
            }
            if !seen.insert(p.clone()) {
                continue; // dedupe to one (best) chunk per note
            }
            out.push(Hit {
                path: p,
                name: name.value(row).to_string(),
                rel: rel.value(row).to_string(),
                text: text.value(row).to_string(),
            });
            if out.len() >= k {
                return out;
            }
        }
    }
    out
}

async fn vector_search(
    app: &tauri::AppHandle,
    dir: &str,
    query_vec: Vec<f32>,
    filter: String,
    fetch: usize,
) -> Result<Vec<RecordBatch>, String> {
    let conn = connect(app).await?;
    let table = match open_chunks(&conn).await? {
        Some(t) => t,
        None => return Ok(vec![]),
    };
    let _ = dir;
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
        .limit(fetch)
        .execute()
        .await
        .map_err(|e| e.to_string())?;
    stream.try_collect::<Vec<_>>().await.map_err(|e| e.to_string())
}

/// Retrieve the top-k chunks most relevant to a free-text query (for "ask").
#[tauri::command]
pub async fn search_notes(
    app: tauri::AppHandle,
    dir: String,
    query: String,
    k: usize,
) -> Result<Vec<Hit>, String> {
    if !crate::engine::wait_health(crate::engine::EMBED_PORT, 60).await {
        return Err("embedding model is still loading (timed out)".into());
    }
    // cap query length so it can't exceed the embed model's context window
    let query: String = query.chars().take(2000).collect();
    let client = reqwest::Client::new();
    let qv = embed_text(&client, &query).await?;
    let filter = format!("vault = '{}'", esc(&dir));
    let batches = vector_search(&app, &dir, qv, filter, k).await?;
    Ok(read_hits(&batches, None, k))
}

/// Find notes closest in meaning to the given seed text, excluding `current`.
#[tauri::command]
pub async fn related_notes(
    app: tauri::AppHandle,
    dir: String,
    current: String,
    text: String,
    k: usize,
) -> Result<Vec<Hit>, String> {
    if !crate::engine::wait_health(crate::engine::EMBED_PORT, 60).await {
        return Err("embedding model is still loading (timed out)".into());
    }
    let client = reqwest::Client::new();
    let seed: String = text.chars().take(2000).collect();
    let qv = embed_text(&client, &seed).await?;
    let filter = format!("vault = '{}' AND path <> '{}'", esc(&dir), esc(&current));
    // fetch extra so dedupe-by-note still yields k distinct notes
    let batches = vector_search(&app, &dir, qv, filter, k * 6).await?;
    Ok(read_hits(&batches, Some(&current), k))
}

/// Walk the vault and return each note's outbound [[wikilinks]]. Pure file I/O
/// (no embeddings/engine), so it works regardless of model state. Feeds both the
/// backlinks panel and the knowledge-map's explicit edges on the frontend.
#[tauri::command]
pub fn vault_links(dir: String) -> Result<Vec<NoteLinks>, String> {
    let mut out = Vec::new();
    for f in walk_markdown(Path::new(&dir)) {
        let text = std::fs::read_to_string(&f.path).unwrap_or_default();
        out.push(NoteLinks {
            rel: f.rel,
            name: f.name,
            links: extract_links(&text),
        });
    }
    Ok(out)
}

#[derive(Serialize)]
pub struct SemanticEdge {
    pub s: String, // source note rel
    pub d: String, // dest note rel
    pub score: f32,
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Semantic edges for the knowledge map: for each note, its top-`k` nearest other
/// notes by mean-pooled embedding, above a cosine `threshold`. Reads the stored
/// vectors only (no engine needed), aggregates per note, computes pairwise cosine,
/// and dedups into undirected edges. Milliseconds for a personal vault. Notes too
/// short to have any embedded chunk simply have no semantic edges (still a node via
/// `vault_links`).
#[tauri::command]
pub async fn semantic_edges(
    app: tauri::AppHandle,
    dir: String,
    k: usize,
    threshold: f32,
) -> Result<Vec<SemanticEdge>, String> {
    let conn = connect(&app).await?;
    let table = match open_chunks(&conn).await? {
        Some(t) => t,
        None => return Ok(vec![]),
    };
    let stream = table
        .query()
        .only_if(format!("vault = '{}'", esc(&dir)))
        .select(Select::Columns(vec!["rel".into(), "vector".into()]))
        .limit(10_000_000)
        .execute()
        .await
        .map_err(|e| e.to_string())?;
    let batches = stream.try_collect::<Vec<_>>().await.map_err(|e| e.to_string())?;

    // mean-pool the per-chunk vectors into one vector per note
    let mut sums: HashMap<String, Vec<f32>> = HashMap::new();
    let mut counts: HashMap<String, f32> = HashMap::new();
    for b in &batches {
        let ri = b.schema().index_of("rel").map_err(|e| e.to_string())?;
        let vi = b.schema().index_of("vector").map_err(|e| e.to_string())?;
        let rels = b
            .column(ri)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("rel column type")?;
        let vecs = b
            .column(vi)
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .ok_or("vector column type")?;
        for row in 0..b.num_rows() {
            let rel = rels.value(row).to_string();
            let inner = vecs.value(row);
            let f = inner
                .as_any()
                .downcast_ref::<arrow_array::Float32Array>()
                .ok_or("vector item type")?;
            let entry = sums.entry(rel.clone()).or_insert_with(|| vec![0.0; f.len()]);
            for (i, x) in f.values().iter().enumerate() {
                entry[i] += *x;
            }
            *counts.entry(rel).or_insert(0.0) += 1.0;
        }
    }

    // mean + L2-normalize, so cosine similarity is just the dot product
    let notes: Vec<(String, Vec<f32>)> = sums
        .into_iter()
        .map(|(rel, mut v)| {
            let c = counts[&rel];
            for x in &mut v {
                *x /= c;
            }
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut v {
                    *x /= norm;
                }
            }
            (rel, v)
        })
        .collect();

    // top-k nearest per note above threshold, deduped to undirected edges
    let n = notes.len();
    let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut edges: Vec<SemanticEdge> = Vec::new();
    for i in 0..n {
        let mut sims: Vec<(usize, f32)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| (j, dot(&notes[i].1, &notes[j].1)))
            .filter(|&(_, s)| s >= threshold)
            .collect();
        sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (j, s) in sims.into_iter().take(k) {
            let key = if i < j { (i, j) } else { (j, i) };
            if seen.insert(key) {
                edges.push(SemanticEdge {
                    s: notes[i].0.clone(),
                    d: notes[j].0.clone(),
                    score: s,
                });
            }
        }
    }
    Ok(edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_links_finds_targets_dedups_and_skips_fences() {
        let t = "See [[Alpha]] and [[ Beta Note ]].\n\n\
                 ```\n[[NotALink]]\n```\n\
                 More [[Gamma]] then [[Alpha]] again.";
        let ls = extract_links(t);
        assert_eq!(ls, vec!["Alpha", "Beta Note", "Gamma"]); // order preserved, deduped
        assert!(!ls.iter().any(|x| x == "NotALink")); // inside a code fence
    }

    #[test]
    fn extract_links_handles_no_links_and_multibyte() {
        assert!(extract_links("plain note, no links — café ☕").is_empty());
        // a multibyte char right before a link must not panic on slicing
        assert_eq!(extract_links("café [[Idée]]"), vec!["Idée"]);
    }

    #[test]
    fn esc_doubles_single_quotes() {
        // SQL-string escaping for LanceDB only_if/delete filters.
        assert_eq!(esc("plain"), "plain");
        assert_eq!(esc("a'b"), "a''b");
        // an injection attempt stays inert: quotes are doubled, so it can't
        // break out of the string literal.
        assert_eq!(esc("' OR 1=1; --"), "'' OR 1=1; --");
        assert_eq!(esc(r"C:\notes\a'b.md"), r"C:\notes\a''b.md");
    }

    #[test]
    fn chunk_skips_tiny_and_caps_size() {
        assert!(chunk("").is_empty());
        assert!(chunk("short line").is_empty()); // < 30 chars -> dropped
        let para = "word ".repeat(400); // ~2000 chars, single paragraph
        let cs = chunk(&para);
        assert!(cs.len() > 1, "huge paragraph should split");
        for c in &cs {
            assert!(c.len() <= 900, "chunk exceeds cap: {}", c.len());
        }
    }

    #[test]
    fn chunk_handles_multibyte_without_panic() {
        // long run of 2-byte chars; slicing must not panic and stays valid UTF-8
        let para = "é".repeat(900); // 1800 bytes
        let cs = chunk(&para);
        assert!(!cs.is_empty());
        for c in &cs {
            assert!(c.chars().all(|ch| ch == 'é' || ch == '\u{FFFD}'));
        }
    }

    #[test]
    fn chunk_separates_paragraphs() {
        let text = "First paragraph that is definitely long enough to keep.\n\nSecond paragraph also clearly long enough to be kept.";
        let cs = chunk(text);
        assert_eq!(cs.len(), 2);
    }
}
