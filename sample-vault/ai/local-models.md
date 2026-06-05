# Notes on running models locally

- Quantized GGUF + llama.cpp runs surprisingly well on CPU. Q4_K_M is the sweet spot
  between size and quality.
- Small models (3–4B) are fine for *grounded* tasks — summarize, extract, answer from
  context. They fall apart on open-ended reasoning, so don't ask them to.
- Embeddings are cheap and underrated. A tiny model like nomic-embed is plenty good
  for retrieval.
- Keep the model on a leash: text in, text out, no tools. The retrieval does the heavy
  lifting, not the model's memory.
- Privacy is the whole point — nothing leaves the machine.
