# Third-Party Licenses

Commonplace itself is MIT-licensed (see `LICENSE`). It builds on, and at runtime
downloads, the following third-party components, each under its own license. None
of these weights/binaries are redistributed in this repository — the engine is
fetched at build time (`app/scripts/fetch-engine.ps1`) and the models are
downloaded by the app on first launch.

## Bundled / linked software

| Component | Role | License |
|-----------|------|---------|
| [llama.cpp](https://github.com/ggml-org/llama.cpp) | inference engine (`llama-server`) | MIT |
| [CodeMirror 6](https://codemirror.net/) | editor (vendored in `app/src/vendor/`) | MIT |
| [Tauri](https://tauri.app/) | application shell | MIT / Apache-2.0 |
| [LanceDB](https://github.com/lancedb/lancedb) | vector index | Apache-2.0 |
| Rust crates (reqwest, serde, tokio, notify, trash, etc.) | backend deps | MIT / Apache-2.0 |

## Models (downloaded on first run)

| Model | Role | License |
|-------|------|---------|
| [Phi-3.5-mini-instruct](https://huggingface.co/microsoft/Phi-3.5-mini-instruct) | chat / RAG answers | MIT |
| [nomic-embed-text v1.5](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5) | embeddings | Apache-2.0 |

Both model licenses permit free and commercial use and redistribution. If you
swap in a different model, check its license — some otherwise-open model families
ship certain sizes (e.g. Qwen2.5-3B) under restrictive research-only licenses.

Full license texts are available at the linked sources.
