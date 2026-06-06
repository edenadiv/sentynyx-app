# ADR-NN: \<verb the shortcut performs\>

> **Template.** Copy to `docs/eng/keybindings-adr/NN-short-name.md` (NN
> = next sequential number, pad to 2 digits). Fill all sections; leave
> the heading numbered.

## Status
\<proposed | accepted | superseded by ADR-MM\>

## Context
Why do we need this shortcut? What workflow does it accelerate? How
often will users trigger it?

## Decision
The binding is `⌘⇧X` / `Ctrl+Shift+X`. It triggers `<handler>`.

## Alternatives considered

| Binding | Why rejected |
|---|---|
| `⌘X` | Reserved on macOS (cut). |
| `⌘Shift+X` | Chosen — fits the ⌘⇧ pattern we use for app-chrome actions. |
| `⌥X` | Conflicts with macOS input method switching under some locales. |

## Compose-surface check
\<Walk through `docs/ux/shortcuts.md` §3. Is `⌘X` bound to a compose
surface action? If the answer involves "but our composer isn't really
markdown", you're wrong — Slack users hit ⌘B for bold even if the
composer doesn't render it. Use the shifted variant.\>

## Platform compatibility

- [ ] macOS: tested on Sonoma 14.x and Sequoia 15.x in dev.
- [ ] Windows: handler fires with `Ctrl+Shift+X` on Windows 11.
- [ ] Linux: handler fires with `Ctrl+Shift+X` on Ubuntu 22/24.

## Implementation checklist

- [ ] Handler added at \<file:line\>.
- [ ] Row added to §1 of `docs/ux/shortcuts.md`.
- [ ] `<Kb k="⌘⇧X" v="..." />` added to `AboutDialog.tsx`.
- [ ] Any hint strings in the UI that reference the shortcut match.
- [ ] `ci-lints.yml` keybinding-sync job is green.

## References

- Issue: \<link\>
- Related ADR: \<if any\>
- PR: \<link once open\>
