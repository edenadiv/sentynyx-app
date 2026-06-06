import { useEffect, useState } from "react";
import type { CSSProperties } from "react";

type UpdateState =
  | { kind: "idle" }
  | { kind: "available"; version: string; notes: string | null }
  | { kind: "downloading"; percent: number | null }
  | { kind: "ready"; version: string }
  | { kind: "error"; msg: string };

/// Listens for tauri-plugin-updater events on app launch and whenever the
/// user (or a background task) triggers a check. Shows a bottom-right toast
/// on update-available, progress during download, and "restart to install"
/// when the download completes.
///
/// The plugin is configured in tauri.conf.json. If the `active: false` flag
/// is set (pre-release), the `checkForUpdate` call resolves to null and this
/// component is a no-op.
export function UpdateToast() {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        // Dynamic import so browser-preview builds don't try to resolve
        // the Tauri plugin.
        const { check } = await import("@tauri-apps/plugin-updater");
        const update = await check();
        if (cancelled || !update) return;
        setState({
          kind: "available",
          version: update.version,
          notes: update.body ?? null,
        });
      } catch (e) {
        // Swallow — "no updater configured" is an expected state in pre-release.
        console.debug("update check skipped:", e);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const startDownload = async () => {
    setState({ kind: "downloading", percent: null });
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (!update) {
        setState({ kind: "idle" });
        return;
      }
      let total = 0;
      let received = 0;
      await update.downloadAndInstall(e => {
        switch (e.event) {
          case "Started":
            total = e.data.contentLength ?? 0;
            break;
          case "Progress":
            received += e.data.chunkLength;
            setState({
              kind: "downloading",
              percent: total > 0 ? Math.round((received / total) * 100) : null,
            });
            break;
          case "Finished":
            setState({ kind: "ready", version: update.version });
            break;
        }
      });
    } catch (e) {
      setState({ kind: "error", msg: String(e) });
    }
  };

  const restart = async () => {
    try {
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) {
      setState({ kind: "error", msg: String(e) });
    }
  };

  if (state.kind === "idle") return null;

  return (
    <div style={ut.wrap}>
      {state.kind === "available" && (
        <>
          <div style={ut.title}>
            <span style={ut.sparkle}>◆</span>
            Update {state.version} available
          </div>
          {state.notes && (
            <div style={ut.notes}>{state.notes.slice(0, 120)}{state.notes.length > 120 ? "…" : ""}</div>
          )}
          <div style={ut.actions}>
            <button onClick={() => setState({ kind: "idle" })} style={ut.linkBtn}>Later</button>
            <button onClick={startDownload} style={ut.primaryBtn}>Download</button>
          </div>
        </>
      )}
      {state.kind === "downloading" && (
        <div style={ut.title}>
          <span style={ut.sparkle}>↓</span>
          Downloading update{state.percent !== null ? ` · ${state.percent}%` : "…"}
        </div>
      )}
      {state.kind === "ready" && (
        <>
          <div style={ut.title}>
            <span style={ut.sparkle}>✓</span>
            Update {state.version} installed
          </div>
          <div style={ut.notes}>Restart to finish.</div>
          <div style={ut.actions}>
            <button onClick={() => setState({ kind: "idle" })} style={ut.linkBtn}>Later</button>
            <button onClick={restart} style={ut.primaryBtn}>Restart now</button>
          </div>
        </>
      )}
      {state.kind === "error" && (
        <div style={ut.title}>
          <span style={{ ...ut.sparkle, color: "#ff6b9d" }}>✕</span>
          Update failed: {state.msg.slice(0, 80)}
        </div>
      )}
    </div>
  );
}

const ut: Record<string, CSSProperties> = {
  wrap: {
    position: "fixed", bottom: 24, right: 24, zIndex: 85,
    padding: "14px 18px", background: "rgba(10,13,20,0.96)",
    border: "1px solid rgba(242,255,43,0.35)", borderRadius: 8,
    color: "#e5e9f0", fontFamily: "Inter, sans-serif", fontSize: 13,
    maxWidth: 340, boxShadow: "0 4px 28px rgba(0,0,0,0.5)",
  },
  title: { display: "flex", alignItems: "center", gap: 8, fontWeight: 500 },
  sparkle: { color: "var(--neon, #f2ff2b)", fontSize: 14 },
  notes: { fontSize: 11, color: "#9ba3b4", marginTop: 6, lineHeight: 1.5 },
  actions: { display: "flex", gap: 8, marginTop: 12, justifyContent: "flex-end" },
  linkBtn: {
    padding: "5px 10px", fontSize: 11, background: "transparent",
    color: "#9ba3b4", border: "none", cursor: "pointer",
  },
  primaryBtn: {
    padding: "5px 12px", fontSize: 11, fontWeight: 600,
    background: "var(--neon, #f2ff2b)", color: "#000",
    border: "none", borderRadius: 4, cursor: "pointer",
  },
};
