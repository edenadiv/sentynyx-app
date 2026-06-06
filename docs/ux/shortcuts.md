# Sentynyx keybinding registry

Single source of truth for every keyboard shortcut in the desktop app.
Hint strings rendered in the UI, the `AboutDialog` cheat sheet, and the
actual `window.addEventListener("keydown")` handler at
`apps/desktop/src/app/App.tsx` all reference this file.

**Invariant**: every shortcut listed here is:
1. Implemented in `App.tsx`'s `keydown` handler (or another explicit handler).
2. Displayed in `AboutDialog`'s "KEYBINDINGS" section.
3. Not colliding with the **compose-surface reserved list** (§3).

Review of any shortcut change REQUIRES:
- Grep proof that the handler + cheat sheet + any hint strings agree.
- An ADR file in `docs/eng/keybindings-adr/` per the template (§4).

---

## 1. Shipped shortcuts

| Shortcut | Action | Handler location |
|---|---|---|
| `⌘↵` / `Ctrl+Enter` | Transmit current composer text | `Composer.tsx::onKey` |
| `⌘K` / `Ctrl+K` | Open command palette | `App.tsx` keydown |
| `⌘M` / `Ctrl+M` | Open consensus arena | `App.tsx` keydown |
| `⌘D` / `Ctrl+D` | Open compliance dashboard | `App.tsx` keydown |
| `⌘⇧D` / `Ctrl+Shift+D` | Toggle dev inspector | `App.tsx` keydown |
| `⌘G` / `Ctrl+G` | Open agent DAG | `App.tsx` keydown |
| `⌘O` / `Ctrl+O` | Open orbital model picker | `App.tsx` keydown |
| `⌘,` / `Ctrl+,` | Open settings | `App.tsx` keydown |
| `⌘⇧I` / `Ctrl+Shift+I` | Toggle About dialog | `App.tsx` keydown |
| `ESC` | Dismiss modals (AboutDialog, DevInspector, OrbitalPicker, etc.) | Per-scene handler |

## 2. Reserved (do not bind)

These are shortcuts the OS or the webview claims. Binding them causes
silent conflicts on various platforms.

- `⌘C` / `Ctrl+C` — copy
- `⌘V` / `Ctrl+V` — paste
- `⌘X` / `Ctrl+X` — cut
- `⌘A` / `Ctrl+A` — select all
- `⌘Z` / `Ctrl+Z` — undo
- `⌘⇧Z` / `Ctrl+Y` — redo
- `⌘F` / `Ctrl+F` — find in webview
- `⌘R` / `Ctrl+R` — reload (webview)
- `⌘W` / `Ctrl+W` — close window
- `⌘Q` / `Ctrl+Q` — quit app (macOS)
- `⌘H` / `Ctrl+H` — hide app (macOS)
- `F11` — fullscreen toggle

## 3. Compose-surface reserved list

If the user's focus is in the Composer textarea (or any `<input>` /
`<textarea>` / `[contenteditable]` element), these shortcuts belong to
the text-editor muscle memory. **Never bind them to app-level actions.**

- `⌘B` / `Ctrl+B` — bold (markdown compose surfaces)
- `⌘I` / `Ctrl+I` — italic (markdown compose surfaces) ← *lesson from bug_014*
- `⌘U` / `Ctrl+U` — underline
- `⌘E` / `Ctrl+E` — "emphasis" in some editors
- `⌘L` / `Ctrl+L` — address bar / focus URL (browsers)
- `⌘T` / `Ctrl+T` — new tab (browsers / terminals)
- `⌘N` / `Ctrl+N` — new window
- `⌘P` / `Ctrl+P` — print (browsers) OR command palette (VS Code)
- `⌘S` / `Ctrl+S` — save
- `Tab` / `Shift+Tab` — field traversal
- `Arrow keys` — caret movement
- `Home` / `End` / `Page Up` / `Page Down` — caret navigation

If an app-level action truly needs one of these, use the shifted variant
(e.g. `⌘⇧I` for About instead of `⌘I`). Precedent: `⌘⇧D` for dev
inspector, `⌘⇧I` for About.

## 4. Grep-enforced consistency

A CI step (`.github/workflows/ci-lints.yml` keybinding-sync job) runs:

```bash
# Every kb in AboutDialog's cheat sheet must exist in this file.
grep -oE '<Kb k="[^"]+"' apps/desktop/src/scenes/AboutDialog.tsx \
  | sed 's/.*="//;s/"//' \
  | while read kb; do
      grep -qF "$kb" docs/ux/shortcuts.md || { echo "MISSING IN REGISTRY: $kb"; exit 1; }
    done
```

Fails the PR if any AboutDialog shortcut isn't in this registry. The
inverse check (registry → handler) is harder to automate reliably; we
rely on the grep in PR review.

## 5. Adding a new shortcut — the process

1. Write an ADR in `docs/eng/keybindings-adr/NN-short-name.md` per
   the template. Reviewer-assigned NN.
2. Confirm the shortcut is NOT on the reserved list (§2) and NOT on
   the compose-surface list (§3) for its context.
3. Implement in the right handler (`App.tsx` for global,
   `Composer.tsx` for composer-local, per-scene for modal-local).
4. Add the row to §1 of this file.
5. Add the `<Kb k="..." v="..." />` row to `AboutDialog.tsx`.
6. If a hint string in a card/UI references the shortcut, update it
   to match.
7. Verify CI keybinding-sync passes.

## 6. Platform conventions

- `⌘` on macOS ↔ `Ctrl` on Windows/Linux. Handler checks
  `e.metaKey || e.ctrlKey`.
- `⇧` means shift.
- `⌥` means option/alt. Avoid unless necessary — conflicts with
  system-level input method switchers on some configurations.
- Use `e.key.toLowerCase()` when comparing so shift state doesn't
  change the compared character (⇧D still matches "d").
