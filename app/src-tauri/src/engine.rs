// Commonplace — bundled inference engine (llama.cpp).
//
// On startup the app spawns two local llama-server processes — one for chat,
// one for embeddings — pointed at GGUF models bundled as app resources. They
// listen only on 127.0.0.1, are owned by the app, and are killed on exit.
// This is what makes Commonplace self-contained: no Ollama, nothing running
// in the background once the window closes.
//
// We spawn with std::process (working dir = engine folder so the ggml DLLs
// resolve) rather than Tauri's externalBin sidecar, which avoids the
// Windows DLL-adjacency problems that mechanism has.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use futures_util::StreamExt;
use tauri::ipc::Channel;
use tauri::Manager;

pub const HOST: &str = "127.0.0.1";
pub const CHAT_PORT: u16 = 11500;
pub const EMBED_PORT: u16 = 11501;

pub const CHAT_MODEL_FILE: &str = "Qwen2.5-3B-Instruct-Q4_K_M.gguf";
pub const EMBED_MODEL_FILE: &str = "nomic-embed-text-v1.5.f16.gguf";
pub const CHAT_MODEL_NAME: &str = "Qwen2.5-3B-Instruct";

// First-run download sources (HEAD-verified). Used only when the models are not
// already present in the model dir.
const CHAT_MODEL_URL: &str =
    "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf";
const EMBED_MODEL_URL: &str =
    "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5-GGUF/resolve/main/nomic-embed-text-v1.5.f16.gguf";

#[derive(Default)]
pub struct EngineState {
    children: Mutex<Vec<Child>>,
    #[allow(dead_code)]
    job: Mutex<isize>, // Windows Job Object handle (kept alive for the app's lifetime)
}

#[derive(serde::Serialize, Clone)]
pub struct DownloadProgress {
    file: String,
    downloaded: u64,
    total: u64,
    done: bool,
}

// A Job Object with KILL_ON_JOB_CLOSE: every assigned process is terminated by
// the OS when the last job handle closes — which happens when OUR process dies,
// for ANY reason (graceful exit, panic, force-kill, Task Manager). This is the
// backstop that guarantees no orphaned llama-server processes.
#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
        JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub fn create() -> isize {
        unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() {
                return 0;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            h as isize
        }
    }

    pub fn assign(job: isize, child: &std::process::Child) {
        if job == 0 {
            return;
        }
        unsafe {
            AssignProcessToJobObject(job as HANDLE, child.as_raw_handle() as HANDLE);
        }
    }
}

fn engine_dir(app: &tauri::AppHandle) -> PathBuf {
    if let Ok(d) = std::env::var("COMMONPLACE_ENGINE_DIR") {
        return PathBuf::from(d);
    }
    app.path()
        .resource_dir()
        .map(|r| r.join("resources").join("engine"))
        .unwrap_or_else(|_| PathBuf::from("resources/engine"))
}

fn model_dir(app: &tauri::AppHandle) -> PathBuf {
    if let Ok(d) = std::env::var("COMMONPLACE_MODEL_DIR") {
        return PathBuf::from(d); // dev override
    }
    // Models are large (GBs) and can't be embedded in a single NSIS installer
    // (32-bit makensis caps the payload near 2 GB). So: use bundled models if
    // present, otherwise fall back to the app-data dir, where installed builds
    // keep them (provisioned on first run).
    if let Ok(rd) = app.path().resource_dir() {
        let bundled = rd.join("resources").join("models");
        if bundled.join(CHAT_MODEL_FILE).exists() {
            return bundled;
        }
    }
    app.path()
        .app_data_dir()
        .map(|d| d.join("models"))
        .unwrap_or_else(|_| PathBuf::from("models"))
}

#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
fn no_window(_cmd: &mut Command) {}

fn spawn_server(engine: &PathBuf, model: PathBuf, port: u16, embedding: bool) -> Option<Child> {
    let exe = engine.join("llama-server.exe");
    if !exe.exists() || !model.exists() {
        eprintln!(
            "[engine] missing exe or model: exe={} model={}",
            exe.display(),
            model.display()
        );
        return None;
    }
    let mut cmd = Command::new(&exe);
    cmd.current_dir(engine) // so ggml*.dll next to the exe resolve
        .arg("--model")
        .arg(&model)
        .arg("--host")
        .arg(HOST)
        .arg("--port")
        .arg(port.to_string())
        .arg("--ctx-size")
        .arg(if embedding { "2048" } else { "8192" })
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if embedding {
        cmd.arg("--embedding").arg("--pooling").arg("mean");
    }
    no_window(&mut cmd);
    match cmd.spawn() {
        Ok(child) => Some(child),
        Err(e) => {
            eprintln!("[engine] failed to spawn llama-server on {port}: {e}");
            None
        }
    }
}

/// True once both model files are present in the model dir.
#[tauri::command]
pub fn models_present(app: tauri::AppHandle) -> bool {
    let m = model_dir(&app);
    m.join(CHAT_MODEL_FILE).exists() && m.join(EMBED_MODEL_FILE).exists()
}

/// Spawn the chat + embed servers and remember the child processes so we can
/// kill them on exit. Idempotent: a second call while running is a no-op.
/// Non-blocking — models load in the background; callers use `wait_health`.
#[tauri::command]
pub fn start_engine(app: tauri::AppHandle, state: tauri::State<EngineState>) -> Result<(), String> {
    let mut kids = state.children.lock().map_err(|_| "engine lock poisoned")?;
    if !kids.is_empty() {
        return Ok(()); // already running
    }
    let engine = engine_dir(&app);
    let models = model_dir(&app);

    #[cfg(windows)]
    {
        let mut j = state.job.lock().map_err(|_| "job lock poisoned")?;
        if *j == 0 {
            *j = job::create();
        }
    }

    let specs = [
        (models.join(CHAT_MODEL_FILE), CHAT_PORT, false),
        (models.join(EMBED_MODEL_FILE), EMBED_PORT, true),
    ];
    for (model, port, embedding) in specs {
        if let Some(c) = spawn_server(&engine, model, port, embedding) {
            #[cfg(windows)]
            {
                let j = *state.job.lock().map_err(|_| "job lock poisoned")?;
                job::assign(j, &c); // OS kills it if we die unexpectedly
            }
            kids.push(c);
        }
    }
    Ok(())
}

/// Download any missing model files into the model dir, streaming progress over
/// the channel. Runs on first launch (or after the user clears the models).
#[tauri::command]
pub async fn download_models(
    app: tauri::AppHandle,
    on_progress: Channel<DownloadProgress>,
) -> Result<(), String> {
    let dir = model_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (file, url) in [
        (CHAT_MODEL_FILE, CHAT_MODEL_URL),
        (EMBED_MODEL_FILE, EMBED_MODEL_URL),
    ] {
        let dest = dir.join(file);
        if dest.exists() {
            continue;
        }
        download_file(url, &dest, file, &on_progress).await?;
    }
    Ok(())
}

async fn download_file(
    url: &str,
    dest: &std::path::Path,
    name: &str,
    ch: &Channel<DownloadProgress>,
) -> Result<(), String> {
    use std::io::Write;

    let client = reqwest::Client::new();
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);

    // write to a .part file, then rename — so an interrupted download is never
    // mistaken for a complete model on the next launch.
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        downloaded += bytes.len() as u64;
        if downloaded - last_emit >= 4_000_000 {
            last_emit = downloaded;
            let _ = ch.send(DownloadProgress {
                file: name.to_string(),
                downloaded,
                total,
                done: false,
            });
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    let _ = ch.send(DownloadProgress {
        file: name.to_string(),
        downloaded,
        total,
        done: true,
    });
    Ok(())
}

/// Kill the spawned servers. Called on app exit.
pub fn stop(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<EngineState>() {
        if let Ok(mut kids) = state.children.lock() {
            for child in kids.iter_mut() {
                let _ = child.kill();
            }
            kids.clear();
        }
    }
}

/// Poll a server's /health until it reports ready (model loaded) or timeout.
/// llama-server returns 503 while loading, 200 once ready.
pub async fn wait_health(port: u16, max_secs: u64) -> bool {
    let client = reqwest::Client::new();
    let url = format!("http://{HOST}:{port}/health");
    let start = std::time::Instant::now();
    loop {
        if let Ok(r) = client.get(&url).timeout(Duration::from_secs(3)).send().await {
            if r.status().is_success() {
                return true;
            }
        }
        if start.elapsed().as_secs() >= max_secs {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// True when both servers are loaded and ready (used by the UI on boot).
#[tauri::command]
pub async fn engine_ready() -> bool {
    let client = reqwest::Client::new();
    let ok = |port: u16| {
        let client = client.clone();
        async move {
            client
                .get(format!("http://{HOST}:{port}/health"))
                .timeout(Duration::from_secs(2))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        }
    };
    ok(CHAT_PORT).await && ok(EMBED_PORT).await
}

#[tauri::command]
pub fn engine_model_name() -> String {
    CHAT_MODEL_NAME.to_string()
}
