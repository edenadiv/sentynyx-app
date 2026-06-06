#!/usr/bin/env python3
"""
Presidio head-to-head against the Sentynyx eval corpus.

Runs Microsoft Presidio's AnalyzerEngine on the same `prompts.json` that the
Rust `eval --compare` binary consumes, and emits metrics in the same JSON
shape so the blog post can diff them directly.

Setup:
    python3 -m venv .venv && source .venv/bin/activate
    pip install presidio-analyzer presidio-anonymizer
    python3 -m spacy download en_core_web_lg

Run:
    python3 eval/scripts/presidio_compare.py eval/prompts.json

Output is printed to stdout between `<!-- sentynyx-compare:begin -->` /
`<!-- sentynyx-compare:end -->` sentinels so the blog post builder can
extract it.
"""

from __future__ import annotations

import json
import sys
from collections import defaultdict


# Map Presidio entity types to the Sentynyx `Kind` schema so head-to-head
# scoring uses a shared vocabulary. Unmapped Presidio entities (credit cards,
# passport numbers, etc.) are dropped — Sentynyx doesn't cover those kinds
# yet, so counting them would be an unfair FP against Presidio.
PRESIDIO_TO_SENTYNYX: dict[str, str] = {
    "EMAIL_ADDRESS": "EMAIL",
    "PHONE_NUMBER": "PHONE",
    "US_SSN": "SSN",
    "IP_ADDRESS": "IP",
    "URL": "URL",
    # Presidio's PERSON is closer to our regex NAME (explicit list) + PERSON_NER
    # (semantic). We map to PERSON_NER because it's semantic detection on both
    # sides; the corpus marks hardcoded names as NAME and everything else as
    # PERSON_NER, which lets us compare like-for-like.
    "PERSON": "PERSON_NER",
    "LOCATION": "LOCATION_NER",
    "ORGANIZATION": "ORG_NER",
}


def score(actual: list[tuple[str, str]], expected: list[tuple[str, str]]):
    """
    Match actual vs expected on (kind, raw) tuples. Mirrors the Rust
    `score()` in eval/src/main.rs — exact-match, first-unmatched wins.
    """
    tp = fp = 0
    matched = [False] * len(expected)
    for a_kind, a_raw in actual:
        hit = next(
            (i for i, (e_kind, e_raw) in enumerate(expected)
             if not matched[i] and e_kind == a_kind and e_raw == a_raw),
            None,
        )
        if hit is not None:
            matched[hit] = True
            tp += 1
        else:
            fp += 1
    fn = sum(1 for m in matched if not m)
    return tp, fp, fn


def prf(tp: int, fp: int, fn: int):
    p = 1.0 if tp + fp == 0 else tp / (tp + fp)
    r = 1.0 if tp + fn == 0 else tp / (tp + fn)
    f1 = 0.0 if p + r == 0 else 2 * p * r / (p + r)
    return p, r, f1


def main():
    if len(sys.argv) < 2:
        print("usage: presidio_compare.py <path/to/prompts.json>", file=sys.stderr)
        sys.exit(2)

    from presidio_analyzer import AnalyzerEngine

    corpus_path = sys.argv[1]
    with open(corpus_path) as fh:
        corpus = json.load(fh)

    analyzer = AnalyzerEngine()

    total_tp = total_fp = total_fn = 0
    per_kind: dict[str, dict[str, int]] = defaultdict(lambda: {"tp": 0, "fp": 0, "fn": 0})
    latencies_ms: list[float] = []

    import time

    for prompt in corpus["prompts"]:
        text: str = prompt["text"]
        expected = [(e["kind"], e["raw"]) for e in prompt["expected"]]

        t0 = time.perf_counter()
        results = analyzer.analyze(text=text, language="en")
        latencies_ms.append((time.perf_counter() - t0) * 1000)

        actual: list[tuple[str, str]] = []
        for r in results:
            kind = PRESIDIO_TO_SENTYNYX.get(r.entity_type)
            if kind is None:
                continue
            raw = text[r.start:r.end]
            actual.append((kind, raw))

        # Group by kind for per-kind scoring, same as Rust compare mode.
        expected_by_kind: dict[str, list[tuple[str, str]]] = defaultdict(list)
        for e in expected:
            expected_by_kind[e[0]].append(e)
        actual_by_kind: dict[str, list[tuple[str, str]]] = defaultdict(list)
        for a in actual:
            actual_by_kind[a[0]].append(a)
        all_kinds = set(expected_by_kind) | set(actual_by_kind)
        for kind in all_kinds:
            tp, fp, fn = score(
                actual_by_kind.get(kind, []),
                expected_by_kind.get(kind, []),
            )
            per_kind[kind]["tp"] += tp
            per_kind[kind]["fp"] += fp
            per_kind[kind]["fn"] += fn
            total_tp += tp
            total_fp += fp
            total_fn += fn

    latencies_ms.sort()
    n = len(latencies_ms)
    avg_ms = sum(latencies_ms) / n if n else 0.0
    p95_ms = latencies_ms[min(int(n * 0.95), n - 1)] if n else 0.0
    p99_ms = latencies_ms[min(int(n * 0.99), n - 1)] if n else 0.0

    p, r, f1 = prf(total_tp, total_fp, total_fn)

    per_kind_out = {}
    for kind, m in per_kind.items():
        kp, kr, kf1 = prf(m["tp"], m["fp"], m["fn"])
        per_kind_out[kind] = {
            "tp": m["tp"], "fp": m["fp"], "fn": m["fn"],
            "precision": round(kp, 4), "recall": round(kr, 4), "f1": round(kf1, 4),
        }

    out = {
        "configs": [
            {
                "name": "presidio",
                "total": {
                    "tp": total_tp, "fp": total_fp, "fn": total_fn,
                    "precision": round(p, 4),
                    "recall": round(r, 4),
                    "f1": round(f1, 4),
                },
                "avg_ms": round(avg_ms, 2),
                "p95_ms": round(p95_ms, 2),
                "p99_ms": round(p99_ms, 2),
                "per_kind": per_kind_out,
            }
        ]
    }

    print("\n<!-- sentynyx-compare:begin -->")
    print(json.dumps(out, indent=2))
    print("<!-- sentynyx-compare:end -->")


if __name__ == "__main__":
    main()
