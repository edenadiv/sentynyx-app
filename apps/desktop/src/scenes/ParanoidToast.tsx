import { useEffect, useState } from "react";
import { onParanoidHit, isTauri } from "../lib/ipc";
import type { ParanoidHit } from "../lib/ipc";

export function ParanoidToast() {
  const [hit, setHit] = useState<ParanoidHit | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    const p = onParanoidHit(h => {
      setHit(h);
      setTimeout(() => setHit(null), 6000);
    });
    return () => { p.then(u => u()); };
  }, []);

  if (!hit) return null;

  return (
    <div style={{
      position: "fixed", bottom: 24, right: 24, zIndex: 80,
      padding: "12px 18px", background: "rgba(10,13,20,0.96)",
      border: "1px solid rgba(242,255,43,0.4)", borderRadius: 6,
      color: "#e5e9f0", fontFamily: "Inter, sans-serif", fontSize: 13,
      maxWidth: 360, boxShadow: "0 4px 24px rgba(0,0,0,0.4)",
    }}>
      <span style={{ color: "var(--neon, #f2ff2b)", marginRight: 6 }}>✦</span>
      Paranoid scan: found {hit.count} additional sensitive span{hit.count === 1 ? "" : "s"} — aliased retroactively.
    </div>
  );
}
