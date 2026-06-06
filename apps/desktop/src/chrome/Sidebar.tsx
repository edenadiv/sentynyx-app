import { SentynyxMark } from "./SentynyxMark";
import { sx } from "./styles";
import type { Conversation } from "../lib/types";

interface Props {
  activeId: string;
  conversations: Conversation[];
  redactionsWeek: number;
  egressCleanPct: number;
  onSelect: (id: string) => void;
  onNew: () => void;
  onOpenSettings: () => void;
}

export function Sidebar({ activeId, conversations, redactionsWeek, egressCleanPct, onSelect, onNew, onOpenSettings }: Props) {
  return (
    <aside style={sx.side}>
      <div style={sx.brand}>
        <SentynyxMark size={30} />
        <div>
          <div style={sx.brandName}>SENTYNYX</div>
          <div style={sx.brandSub}>AI · OS · v1.4</div>
        </div>
      </div>

      <button style={sx.newBtn} onClick={onNew}>
        <span style={{ fontSize:16, lineHeight:1 }}>+</span>
        <span>New transmission</span>
        <span style={sx.kbd}>⌘N</span>
      </button>

      <div style={sx.sideSection}>
        <div style={sx.sideHead}>
          <span>Orbit</span>
          <span style={{ opacity:0.4 }}>{conversations.length}</span>
        </div>
        <div style={{ display:"flex", flexDirection:"column", gap:2 }}>
          {conversations.map(c => (
            <button key={c.id}
              onClick={() => onSelect(c.id)}
              style={{ ...sx.convo, ...(activeId === c.id ? sx.convoActive : {}) }}>
              <span style={{ ...sx.convoDot, background: activeId === c.id ? "var(--neon)" : "rgba(255,255,255,0.25)" }} />
              <span style={sx.convoTitle}>{c.title}</span>
              {c.shield && <span title="Vendetta protected" style={sx.shield}>⛨</span>}
              <span style={sx.convoTime}>{c.time}</span>
            </button>
          ))}
        </div>
      </div>

      <div style={{ marginTop:"auto" }}>
        <div style={sx.vendCard}>
          <div style={{ display:"flex", alignItems:"center", justifyContent:"space-between", marginBottom:10 }}>
            <span style={{ fontSize:10, letterSpacing:2, color:"var(--ink-2)" }}>VENDETTA ENGINE</span>
            <span style={sx.live}><span style={sx.liveDot} />LIVE</span>
          </div>
          <div style={{ fontSize:12, color:"var(--ink-1)", lineHeight:1.5 }}>
            Scanning all outbound traffic. <span style={{ color:"var(--neon)" }}>{redactionsWeek.toLocaleString()}</span> redactions this week.
          </div>
          <div style={sx.meter}>
            <div style={{ ...sx.meterBar, width:`${egressCleanPct}%` }} />
          </div>
          <div style={{ display:"flex", justifyContent:"space-between", fontSize:10, color:"var(--ink-3)", marginTop:6 }}>
            <span>Egress clean</span><span>{egressCleanPct}%</span>
          </div>
        </div>
        <div style={sx.userRow}>
          <div style={sx.avatar}>KA</div>
          <div style={{ flex:1, minWidth:0 }}>
            <div style={{ fontSize:12, fontWeight:500 }}>Kai Alvarez</div>
            <div style={{ fontSize:10, color:"var(--ink-3)" }}>Halcyon · Admin</div>
          </div>
          <button style={sx.iconBtn} title="Settings" onClick={onOpenSettings}>⚙</button>
        </div>
      </div>
    </aside>
  );
}
