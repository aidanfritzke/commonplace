// Commonplace — Tauri 2 backend.
//
// All file I/O and inference live here as commands the buildless frontend
// calls via `invoke`:
//   * folder picking / md listing / read / write  -> native fs
//   * embeddings + chat                            -> bundled llama.cpp
//     servers spawned by `engine` (see engine.rs), talked to over loopback.
//
// Cross-platform: nothing here is Windows-specific. Shipping other
// targets later is a build concern, not a code change.

mod engine;
mod index;

use std::path::PathBuf;
use std::sync::Mutex;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

// The canonical root of the currently-open vault. read_file/write_file refuse
// any path outside it — defense-in-depth so a compromised webview can't reach
// arbitrary files on disk.
#[derive(Default)]
struct Vault(Mutex<Option<PathBuf>>);

// Holds the active filesystem watcher (dropped/replaced when a new vault opens).
type VaultWatcher = notify_debouncer_mini::Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>;
#[derive(Default)]
struct WatchState(Mutex<Option<VaultWatcher>>);

fn confine(vault: &Vault, path: &str) -> Result<PathBuf, String> {
    let root = vault
        .0
        .lock()
        .map_err(|_| "vault lock poisoned")?
        .clone()
        .ok_or("no vault is open")?;
    let p = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    if p.starts_with(&root) {
        Ok(p)
    } else {
        Err("refused: path is outside the open vault".into())
    }
}

#[derive(Serialize, Debug)]
struct FileEntry {
    path: String, // absolute path (stable key)
    name: String, // file name, e.g. "ideas.md"
    rel: String,  // vault-relative path, e.g. "notes/ideas.md"
}

#[derive(Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

// ----- vault: native folder picker + markdown walk + read/write -----

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    // The dialog plugin's blocking API must not run on the main thread.
    // An async command runs on the async runtime (off the main thread),
    // so we use the non-blocking callback and wait on a channel: the
    // dialog is pumped by the main event loop, the callback fires there,
    // and our worker simply receives the result. No deadlock.
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog().file().pick_folder(move |picked| {
        let _ = tx.send(picked);
    });
    let picked = rx.recv().ok().flatten()?;
    picked
        .into_path()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
fn list_markdown(dir: String, vault: tauri::State<Vault>) -> Result<Vec<FileEntry>, String> {
    // Record the canonical vault root; this is the only directory read_file /
    // write_file will subsequently touch.
    if let Ok(canon) = std::fs::canonicalize(&dir) {
        *vault.0.lock().map_err(|_| "vault lock poisoned")? = Some(canon);
    }
    let root = std::path::Path::new(&dir);
    let mut out: Vec<FileEntry> = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
    {
        let entry = entry.map_err(|e| e.to_string())?;
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
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push(FileEntry {
            path: path.to_string_lossy().into_owned(),
            name,
            rel,
        });
    }
    out.sort_by(|a, b| a.rel.to_lowercase().cmp(&b.rel.to_lowercase()));
    Ok(out)
}

#[tauri::command]
fn read_file(path: String, vault: tauri::State<Vault>) -> Result<String, String> {
    let safe = confine(&vault, &path)?;
    std::fs::read_to_string(&safe).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_file(path: String, content: String, vault: tauri::State<Vault>) -> Result<(), String> {
    let safe = confine(&vault, &path)?;
    std::fs::write(&safe, content).map_err(|e| e.to_string())
}

/// Create a new note directly under `root`. The name is reduced to a single
/// file-name component (no directories, no traversal), gets a `.md` extension,
/// and is seeded with a title heading. `root` must be the canonical vault root.
fn create_note(root: &std::path::Path, name: &str) -> Result<FileEntry, String> {
    // keep only the final path component — strips any dirs/`..` the name carries
    let stem = std::path::Path::new(name.trim())
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("invalid name")?
        .to_string();
    if stem.is_empty() {
        return Err("name is empty".into());
    }
    let mut fname = stem;
    if !fname.to_lowercase().ends_with(".md") {
        fname.push_str(".md");
    }

    let path = root.join(&fname);
    // belt-and-suspenders: the parent must be exactly the (canonical) vault root
    let parent = std::fs::canonicalize(path.parent().ok_or("invalid path")?)
        .map_err(|e| e.to_string())?;
    if parent != root {
        return Err("refused: path is outside the open vault".into());
    }
    if path.exists() {
        return Err("a note with that name already exists".into());
    }
    let title = fname.strip_suffix(".md").unwrap_or(&fname);
    std::fs::write(&path, format!("# {title}\n\n")).map_err(|e| e.to_string())?;

    Ok(FileEntry {
        path: path.to_string_lossy().into_owned(),
        name: fname.clone(),
        rel: fname,
    })
}

#[tauri::command]
fn create_file(name: String, vault: tauri::State<Vault>) -> Result<FileEntry, String> {
    let root = vault
        .0
        .lock()
        .map_err(|_| "vault lock poisoned")?
        .clone()
        .ok_or("no vault is open")?;
    create_note(&root, &name)
}

/// Move a note to the OS recycle bin (recoverable), confined to the vault.
#[tauri::command]
fn delete_file(path: String, vault: tauri::State<Vault>) -> Result<(), String> {
    let safe = confine(&vault, &path)?;
    trash::delete(&safe).map_err(|e| e.to_string())
}

/// Watch the vault for external .md changes and emit `vault-changed` (debounced)
/// so the UI can live-refresh. Replaces any previous watcher.
#[tauri::command]
fn watch_vault(
    app: tauri::AppHandle,
    dir: String,
    watch: tauri::State<WatchState>,
) -> Result<(), String> {
    use notify_debouncer_mini::notify::RecursiveMode;
    use notify_debouncer_mini::{new_debouncer, DebounceEventResult};

    let app2 = app.clone();
    let mut debouncer = new_debouncer(
        std::time::Duration::from_millis(500),
        move |res: DebounceEventResult| {
            if let Ok(events) = res {
                let touched_md = events.iter().any(|e| {
                    e.path
                        .extension()
                        .map(|x| x.eq_ignore_ascii_case("md"))
                        .unwrap_or(false)
                });
                if touched_md {
                    let _ = app2.emit("vault-changed", ());
                }
            }
        },
    )
    .map_err(|e| e.to_string())?;
    debouncer
        .watcher()
        .watch(std::path::Path::new(&dir), RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;
    *watch.0.lock().map_err(|_| "watch lock poisoned")? = Some(debouncer);
    Ok(())
}

#[tauri::command]
fn active_model() -> String {
    engine::CHAT_MODEL_NAME.to_string()
}

// ----- inference: stream chat from the bundled llama-server -----

// Kept the name `ollama_chat` for frontend compatibility; it now talks to the
// local llama.cpp chat server's OpenAI-compatible streaming endpoint.
#[tauri::command]
async fn ollama_chat(
    messages: Vec<ChatMessage>,
    model: String,
    on_token: Channel<String>,
) -> Result<(), String> {
    let _ = model; // the loaded model is fixed per-server; field is ignored
    if !engine::wait_health(engine::CHAT_PORT, 180).await {
        return Err("chat model is still loading (timed out)".into());
    }
    let client = reqwest::Client::new();
    let msgs: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
        .collect();
    let body = serde_json::json!({ "model": "local", "messages": msgs, "stream": true });

    let resp = client
        .post(format!("http://{}:{}/v1/chat/completions", engine::HOST, engine::CHAT_PORT))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("chat HTTP {}", resp.status()));
    }

    // OpenAI-style SSE: lines of `data: {json}` ending with `data: [DONE]`.
    // Reassemble across chunk boundaries; emit each choices[0].delta.content.
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| e.to_string())?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(nl) = buf.find('\n') {
            let line: String = buf.drain(..=nl).collect();
            let line = line.trim();
            if line.is_empty() || !line.starts_with("data:") {
                continue;
            }
            let payload = line[5..].trim();
            if payload == "[DONE]" {
                return Ok(());
            }
            if let Ok(j) = serde_json::from_str::<serde_json::Value>(payload) {
                if let Some(tok) = j.pointer("/choices/0/delta/content").and_then(|c| c.as_str()) {
                    if !tok.is_empty() {
                        let _ = on_token.send(tok.to_string());
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(Vault::default());
            app.manage(WatchState::default());
            app.manage(engine::EngineState::default());
            // The engine is started by the frontend (via `start_engine`) once it
            // has confirmed the models are present / finished the first-run
            // download. This keeps a model-less first launch from silently
            // spawning nothing.
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pick_folder,
            list_markdown,
            read_file,
            write_file,
            create_file,
            delete_file,
            watch_vault,
            active_model,
            ollama_chat,
            engine::engine_ready,
            engine::engine_model_name,
            engine::models_present,
            engine::start_engine,
            engine::download_models,
            index::index_vault,
            index::search_notes,
            index::related_notes,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Commonplace")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                engine::stop(app_handle); // kill llama-server processes
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn confine_allows_inside_rejects_outside_and_traversal() {
        let base = std::env::temp_dir().join(format!("cp_confine_{}", std::process::id()));
        let vault_dir = base.join("vault");
        fs::create_dir_all(&vault_dir).unwrap();
        let inside = vault_dir.join("note.md");
        fs::write(&inside, "hi").unwrap();
        let outside = base.join("secret.txt");
        fs::write(&outside, "secret").unwrap();

        let vault = Vault(Mutex::new(Some(fs::canonicalize(&vault_dir).unwrap())));

        // a real note inside the vault is allowed
        assert!(confine(&vault, inside.to_str().unwrap()).is_ok());
        // a sibling file outside the vault is refused
        assert!(confine(&vault, outside.to_str().unwrap()).is_err());
        // a traversal that resolves above the vault is refused
        let traversal = vault_dir.join("..").join("secret.txt");
        assert!(confine(&vault, traversal.to_str().unwrap()).is_err());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn confine_refuses_when_no_vault_open() {
        let vault = Vault(Mutex::new(None));
        // even an absolute system path is refused when no vault is open
        assert!(confine(&vault, "C:/Windows/System32/drivers/etc/hosts").is_err());
    }

    #[test]
    fn create_note_sanitizes_and_confines() {
        let base = std::env::temp_dir().join(format!("cp_create_{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let root = fs::canonicalize(&base).unwrap();

        // plain name -> .md appended, file created in the vault
        let e = create_note(&root, "ideas").unwrap();
        assert_eq!(e.name, "ideas.md");
        assert!(root.join("ideas.md").exists());

        // existing .md kept; duplicate rejected
        assert_eq!(create_note(&root, "ideas.md").unwrap_err(), "a note with that name already exists");

        // traversal in the name is stripped to a bare filename (stays in vault)
        let e2 = create_note(&root, "../../escape").unwrap();
        assert_eq!(e2.name, "escape.md");
        assert!(root.join("escape.md").exists());
        assert!(!base.join("..").join("..").join("escape.md").exists());

        // empty / dot names rejected
        assert!(create_note(&root, "   ").is_err());
        assert!(create_note(&root, "..").is_err());

        let _ = fs::remove_dir_all(&base);
    }
}
