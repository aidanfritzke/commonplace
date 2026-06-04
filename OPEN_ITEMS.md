# Commonplace — Open Items

Tracked, not yet done. Deferred deliberately.

## Engine / packaging
- [x] **Crash-safe engine cleanup.** DONE — children assigned to a Windows Job
  Object with `KILL_ON_JOB_CLOSE` in `engine.rs`; the OS kills them when the app
  process dies for any reason. Verified by force-kill (0 orphans).
- [x] **Produce the installer.** DONE — NSIS `Commonplace_0.1.0_x64-setup.exe`
  (~42 MB) at `app/src-tauri/target/release/bundle/nsis/`. Models are NOT embedded
  (32-bit makensis can't build a >2 GB installer), so the installer ships app +
  engine only and loads models from the app-data dir.
- [x] **First-run model provisioning.** DONE — on launch the app checks for the
  models and, if missing, shows a setup overlay and downloads both GGUFs (~2.1 GB,
  progress bar) into `%APPDATA%\com.commonplace.app\models`, then starts the engine.
  Verified end-to-end. The 42 MB installer is now distributable to a fresh machine
  (needs internet on first launch). Future polish: resumable download (currently a
  failed download restarts the file), checksum verification, and a retry button.

## Editor / polish (later)
- [x] **Fonts offline.** DONE — dropped the Google Fonts CDN; the app now uses
  system fonts (Courier New writing surface, system mono chrome). Fully offline.
- [ ] Persist **UI state** (theme, last vault/file) via app config.
- [ ] Batch embeddings via a single multi-input request for faster first-index on
  large vaults.
- [ ] In-app **chat-model switcher** (list installed models, pick, persist) — only
  relevant if we re-expose model choice; currently Phi-3.5-mini is fixed.

## Release polish (nice-to-have)
- [ ] Add a **screenshot/GIF** to the README (placeholder Releases link too).
- [ ] Replace the default Tauri **app icon** with a custom one (cosmetic).
- [ ] Resumable + checksummed model download; retry button on the setup overlay.
- [ ] The bundle identifier `com.commonplace.app` triggers a harmless macOS
  `.app` warning at build; rename before any macOS build (would change the
  app-data path, so not now).
