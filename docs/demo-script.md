# Sentynyx 60-second demo script

Capture target: 16:9 MP4 at 1920×1080 (or 3840×2160 for retina), 30fps.
Tooling: OBS for the screen capture, Descript for the voiceover + cuts.
Export as H.264 for maximum browser compatibility; target under 8 MB.

## Storyboard

| sec | on-screen | voiceover |
|-----|-----------|-----------|
| 0–6 | Fresh launch, BootSequence animation runs. App lands on the empty state; OnboardingCard visible. | "Every ChatGPT prompt your company sends is leaking customer data." |
| 6–14 | Cursor clicks into the composer. Start typing: `Email sarah.chen@halcyon.io about the layoffs next quarter.` The live highlights appear: yellow on email + "Sarah Chen", teal on "layoffs" after the 350 ms pause. | "Sentynyx sits on your machine and redacts sensitive spans before they leave. Regex catches the email and the name. Semantic NER catches what regex can't." |
| 14–22 | Cmd+Enter. XrayBeam animation fires. Transcript shows the aliased form (`⟦email_01⟧`, `⟦person_01⟧`). Stream begins below. | "The payload going to GPT-4 is fully redacted. The response streams back to you — we swap the originals back locally." |
| 22–32 | Click the VendettaPanel toggle. Show the audit log: each redaction with its alias, source (regex / NER), timestamp. | "Every redaction is logged. Tamper-evident audit chain. GDPR-aligned export on demand." |
| 32–42 | Cmd+Shift+D. DevInspector opens. Point at timings: regex 2 ms, NER 38 ms, TTFT 840 ms. Hover the wire-payload section. | "For engineers: the dev inspector shows every stage's timing, and the exact aliased payload the provider received. Copy-paste it into any playground to reproduce." |
| 42–52 | Type a prompt with an SSN. PolicyViolation fires. Click "USE LOCAL". The send re-runs against the on-device Qwen model. Response streams. | "For content that shouldn't leave the machine at all — SSNs, compliance-flagged content — flip to the on-device Qwen model. Fully private. No API calls." |
| 52–60 | Fade to landing page with the waitlist form visible. | "Sentynyx. Privacy layer for every LLM. Early access at sentynyx.com." |

## Capture checklist

- [ ] Fresh terminal, run the app release build (not dev) so there are no
      Tauri devtools badges / hot-reload indicators.
- [ ] Hide the dev server logs window — use the built .dmg so the binary
      runs stand-alone.
- [ ] Turn off notifications (Focus mode on macOS).
- [ ] Pre-clear `~/Library/Application Support/Sentynyx/sentynyx.db` so
      the onboarding card shows.
- [ ] Set window size to 1440×900; record at 2x.
- [ ] Disable cursor highlighting in OBS (looks hokey).

## Voice direction

Flat, confident, fast. 180 words / minute. No upward inflection. Model
this on [Figma's 2020 launch video](https://www.youtube.com/watch?v=ByZ6CcV4-SM) —
tight, functional, zero sizzle.

## Export targets

- `apps/site/demo.mp4` — 1080p H.264 MP4, under 8 MB (use Handbrake if needed).
- `apps/site/demo-poster.png` — frame at ~8 s (the live-typing highlight moment).
- `apps/site/og.png` — 1200×630 social card, screenshot of the transcript
  with the aliased payload visible.
