# Eval scripts

Companion tools for the Rust `eval --compare` binary. These live outside the
Rust workspace because they pull in Python-only deps (spaCy, Presidio) that
aren't worth the build-time weight.

## Presidio head-to-head

Runs Microsoft Presidio's `AnalyzerEngine` on the same `prompts.json` that
Sentynyx's compare mode consumes, and emits metrics in the matching JSON
shape — so the blog post can diff the two side-by-side without any extra
glue.

### Setup

```bash
cd apps/desktop/src-tauri
python3 -m venv .venv
source .venv/bin/activate
pip install presidio-analyzer presidio-anonymizer
python3 -m spacy download en_core_web_lg
```

The spaCy model is ~560 MB; one-time cost.

### Run

```bash
# From src-tauri/
python3 eval/scripts/presidio_compare.py eval/prompts.json > eval/reports/presidio.json

# Sentynyx side:
cargo run --release --bin eval -- compare > eval/reports/sentynyx.json
```

Each file is wrapped in `<!-- sentynyx-compare:begin --> … <!-- sentynyx-compare:end -->`
sentinels — the blog-post generator extracts the JSON inside and renders the
head-to-head table.

### Fair-comparison notes

- We map Presidio entity types to the Sentynyx `Kind` schema
  (`EMAIL_ADDRESS → EMAIL`, `PERSON → PERSON_NER`, etc.). Presidio
  categories without a Sentynyx counterpart (credit cards, passport
  numbers, US driver licenses) are dropped — they'd be an unfair FP against
  Presidio otherwise.
- Sentynyx's regex hardcodes a tiny set of names / companies as `NAME` /
  `COMPANY`; the corpus reserves those for prompts where the hardcoded
  entry is actually present, and uses `PERSON_NER` / `ORG_NER` everywhere
  else. Presidio's `PERSON` maps to `PERSON_NER` for the same reason —
  both are semantic/statistical detectors.
- Latency numbers are single-threaded, one prompt at a time. Neither side
  batches.
