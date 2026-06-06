# Security policy

Sentynyx is a privacy layer for LLMs. A vulnerability in this code is a
vulnerability in someone's prompts. We take disclosures seriously and will work
with you in good faith.

## Reporting a vulnerability

**Do not file a public GitHub issue for a security problem.**

Preferred: open a private report via GitHub's
[**Security Advisories**](https://github.com/edenadiv/sentynyx-app/security/advisories/new)
("Report a vulnerability").

Or email **security@sentynyx.com** with:

- A description of the issue and where it lives in the codebase.
- Steps to reproduce.
- The impact you observed (or could plausibly infer).
- Your contact info and whether you want public credit.

PGP is welcome but not required.

## Our commitments

- **Acknowledge within 72 hours** of receipt — from a human, not an autoresponder.
- **Triage within 7 days**, with a severity classification and a preliminary fix timeline.
- **Fix critical issues within 30 days** of triage; high within 60; medium within 90; low at the next regular release.
- **Credit you** (when you want it) in the release notes / advisory.
- **Coordinate disclosure** — we'll agree a public-disclosure date with you, usually the day the patched release ships.

This is a community open-source project; there is no paid bug-bounty program. We
acknowledge every valid report and credit researchers publicly.

## Scope

In scope:

- The Sentynyx desktop app in this repository (Tauri 2 binary; macOS, and
  Windows/Linux builds from source).
- The Vendetta detection/aliasing/re-hydration pipeline — especially anything
  that could cause **raw user text or PII to egress** when it shouldn't.
- The local data layer (SQLite store, keychain handling, audit log).

Particularly interesting classes of bug for this project:

- A path where sensitive content reaches a provider **un-aliased** (e.g. a
  detector miss with security impact, a re-hydration/aliasing ordering bug, or a
  remote endpoint mis-classified as local — see `ollama_host_is_local`).
- API keys leaking into logs, traces, telemetry, or the renderer.
- Anything that defeats the critical-class egress **block** (SSN / API key).

Out of scope:

- Findings on third-party services (Hugging Face, Ollama, the LLM providers).
  Report those to the service directly.
- Vulnerabilities requiring physical access to an already-unlocked machine.
- Social-engineering findings.
- Missing security headers / scanner output without a concrete exploit path.
- Findings against a fork that isn't this repository.
- The commercial team-cloud features (compiled out of public builds); if you
  believe you've found something there, email us.

## Safe-harbor commitment

We will not pursue legal action against researchers who:

1. Make a good-faith effort to follow this policy.
2. Do not access, modify, or destroy data beyond what is needed to demonstrate
   the vulnerability.
3. Do not disclose publicly before the coordinated date we agree.

Safe-harbor language adapted from the [disclose.io](https://disclose.io) template.
Questions about scope? Email us before you start.

Thank you for helping keep the privacy layer privacy-preserving.
