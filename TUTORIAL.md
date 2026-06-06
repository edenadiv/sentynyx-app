# Sentynyx tutorial — from zero to your first shielded prompt

This walks you through installing Sentynyx, sending your first prompt through the Vendetta privacy perimeter, and using all three model modes (cloud BYOK, local Ollama, on-device). ~10 minutes.

> **No install, just looking?** Open `Sentynyx.html` in any browser for a simulated interactive demo, or jump to [Part 5](#5-the-interface-at-a-glance) to learn the UI first.

---

## 1. Install

### Option A — macOS app (fastest)

```bash
curl -fsSL https://raw.githubusercontent.com/edenadiv/sentynyx-app/main/scripts/install.sh | bash
```

This downloads the latest signed `.dmg`, verifies it, and drops `Sentynyx.app` into `/Applications`. Or `brew install --cask edenadiv/tap/sentynyx`, or download the `.dmg` from [Releases](https://github.com/edenadiv/sentynyx-app/releases).

### Option B — build from source (any OS)

```bash
git clone https://github.com/edenadiv/sentynyx-app.git
cd sentynyx-app/apps/desktop
pnpm install
./scripts/stage-sidecar.sh
pnpm tauri dev
```

You need [Rust](https://rustup.rs), [pnpm](https://pnpm.io), and (Linux only) the GTK/WebKit deps listed in the README.

### First launch: the model download

On first run Sentynyx fetches its detection models (~1.1 GB total: GLiNER for semantic NER + a small LLM for the "paranoid" scan) from Hugging Face. This is a **one-time, SHA-verified** download. You can use cloud models while it runs; the semantic layers light up when it finishes. Watch progress in the top-bar chip or **Settings → Models**.

---

## 2. Pick how you want to run models

Open **Settings** (`⌘,`). You have three independent options — use any or all.

### A) Bring your own cloud key (OpenAI / Anthropic / Google / xAI)

1. In Settings, find the provider, paste your API key (`sk-…`, `sk-ant-…`, `AIza…`, `xai-…`).
2. Click **Save**. Sentynyx validates the key live and stores it in your **OS keychain** — it never reaches the UI layer or disk in plaintext.
3. The provider's models are now selectable in the picker.

### B) Local models via Ollama (zero egress)

1. Install [Ollama](https://ollama.com) and pull a model:
   ```bash
   ollama pull llama3.2
   ```
2. Back in Sentynyx → **Settings → Local models · Ollama** → **Check connection**. You should see `✓ reachable · N models installed`.
3. Your pulled models now appear in the picker under the **Ollama** group. No API key needed.

Because a localhost Ollama runs entirely on your machine, Sentynyx sends prompts to it **raw** — there's no third party to hide them from. (If you set a *remote* Ollama URL, Sentynyx detects the egress and aliases the prompt anyway.)

### C) The bundled on-device model

**Settings → Models → Paranoid mode (Qwen 2.5 0.5B) → Download.** Once present, pick **Sentynyx Local** in the model picker for fully offline chat.

---

## 3. Send your first shielded prompt

1. Pick a model: click the model pill in the top bar, or press `⌘O` for the orbital picker.
2. In the composer, type something with sensitive data, e.g.:
   > *Email Sarah Chen (sarah.chen@acme.com) about the Q3 layoffs and wire $4,250,000 to account 123-45-6789.*
3. Watch the **Vendetta panel** (right side) populate as you type — each detected entity is mapped to an alias.
4. Hit **Transmit**. You'll see the X-ray sweep animate the redaction, then the reply streams in.

What just happened:
- `sarah.chen@acme.com` → `⟦email_01⟧`, `Sarah Chen` → `⟦person_01⟧`, `$4,250,000` → `⟦amount_01⟧` **before** anything left your machine.
- The model saw only the aliased prompt. The reply was re-hydrated locally so *you* see the real values.
- `123-45-6789` is an SSN — a **critical class**. The send is **blocked** entirely and logged, never transmitted. (Use a local model, or remove it, to proceed.)

Toggle the response between **"You see"** (re-hydrated) and **"Model saw"** (aliased) to verify with your own eyes.

---

## 4. Verify it yourself (the whole point)

Open the **Dev Inspector** with `⌘⇧D`. For every send it shows:
- per-stage timings (regex / NER / paranoid),
- the **exact aliased payload** that went over the wire — copy it into any LLM playground to confirm it contains no raw PII,
- the streamed response before/after re-hydration.

This is the auditable proof: what the provider received never contained your secrets.

---

## 5. The interface at a glance

| Surface | Trigger | What it does |
| --- | --- | --- |
| Model picker | model pill / `⌘O` | Choose any model from any provider (incl. Ollama) |
| Vendetta panel | right sidebar | Live detected entities → aliases; Masked / Aliased / Raw views |
| Dev Inspector | `⌘⇧D` | Per-send timings + exact wire payload |
| Consensus | `⌘M` | One prompt → three models in parallel |
| Compliance | `⌘D` | Audit feed from the local hash-chained log |
| Command palette | `⌘K` | Fuzzy search everything |
| Settings | `⌘,` | Keys, models, Ollama, data export/delete |

---

## 6. Troubleshooting

- **"No API key configured"** → Settings (`⌘,`), add the provider key.
- **Ollama models don't appear** → make sure `ollama serve` is running, then Settings → Local models → Check connection. Restart Sentynyx to re-scan.
- **First send is slow / semantic layers off** → the model download is still running; check Settings → Models.
- **A prompt was blocked** → it contained an SSN or API key (critical egress classes). Remove it or switch to a local model.
- **Reset everything** → Settings → Data → Delete all local data (removes the DB, audit log, downloaded models, and keychain entries).

---

You're set. Everything runs locally; your raw text is yours. To understand the internals or contribute, see [CONTRIBUTING.md](CONTRIBUTING.md) and [`vendetta.rs`](apps/desktop/src-tauri/src/vendetta.rs).
