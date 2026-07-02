import { useEffect, useState } from "react";
import { api, isTauri } from "./ipc";
import { HUBS, PALETTE_TARGETS, navigateTo, renderHub, portraitUrl } from "./hubs";
import { t, useLang } from "./i18n";
import { Starfield } from "./Starfield";
import { AgentAvatar, type Mood } from "./AgentAvatar";
import type { Character, CharacterStatusView, FleetView, LocalIntel, Notification, PodRiskView, ServerStatus, Severity, SystemSafetyView } from "./types";
import "./app.css";

function fmtDuration(seconds: number): string {
  if (seconds <= 0) return "done";
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

const SEV_RANK: Record<Severity, number> = { Info: 1, Warning: 2, Critical: 3 };

// Compact ISK formatter for the rail (e.g. "1.2B", "340M").
function fmtIsk(v: number): string {
  if (v >= 1e9) return `${(v / 1e9).toFixed(1)}B`;
  if (v >= 1e6) return `${(v / 1e6).toFixed(0)}M`;
  if (v >= 1e3) return `${(v / 1e3).toFixed(0)}K`;
  return `${Math.round(v)}`;
}

// Hubs are deep-linkable via the URL hash (e.g. #combat or #corp/srp — the
// part before "/" picks the hub, the rest the sub-tab) so a view can be
// restored on launch, linked to, or jumped to from the command palette.
function hubFromHash(): string {
  const fromHash = window.location.hash.replace(/^#/, "").split("/")[0];
  return HUBS.some((h) => h.id === fromHash) ? fromHash : "home";
}

// Simple subsequence fuzzy match: every query char must appear in order.
// Earlier + tighter matches score higher; null = no match.
function fuzzyScore(query: string, target: string): number | null {
  const q = query.toLowerCase();
  const t = target.toLowerCase();
  let ti = 0;
  let score = 0;
  for (const c of q) {
    const found = t.indexOf(c, ti);
    if (found === -1) return null;
    score += found - ti; // gaps cost; contiguous runs are free
    ti = found + 1;
  }
  return score + t.length * 0.01; // prefer shorter targets on ties
}

// Command palette (Ctrl/⌘-K): fuzzy-jump to any hub or sub-tab.
function CommandPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [sel, setSel] = useState(0);

  useEffect(() => {
    if (open) {
      setQuery("");
      setSel(0);
    }
  }, [open]);

  if (!open) return null;

  const matches = PALETTE_TARGETS
    .map((tgt) => ({ tgt, score: query ? fuzzyScore(query, tgt.label) : 0 }))
    .filter((m): m is { tgt: (typeof PALETTE_TARGETS)[number]; score: number } => m.score !== null)
    .sort((a, b) => a.score - b.score)
    .slice(0, 12);
  const clamped = Math.min(sel, Math.max(0, matches.length - 1));

  function go(i: number) {
    const m = matches[i];
    if (!m) return;
    navigateTo(m.tgt.hub, m.tgt.sub);
    onClose();
  }

  return (
    <div className="palette-overlay" onClick={onClose}>
      <div className="palette" onClick={(e) => e.stopPropagation()}>
        <input
          autoFocus
          value={query}
          placeholder="Jump to… (type to filter)"
          onChange={(e) => {
            setQuery(e.target.value);
            setSel(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            else if (e.key === "ArrowDown") setSel((s) => Math.min(s + 1, matches.length - 1));
            else if (e.key === "ArrowUp") setSel((s) => Math.max(s - 1, 0));
            else if (e.key === "Enter") go(clamped);
          }}
        />
        <ul>
          {matches.map((m, i) => (
            <li
              key={m.tgt.label}
              className={i === clamped ? "active" : ""}
              onMouseEnter={() => setSel(i)}
              onClick={() => go(i)}
            >
              {m.tgt.label}
            </li>
          ))}
          {matches.length === 0 && <li className="empty">No matches.</li>}
        </ul>
      </div>
    </div>
  );
}

export default function App() {
  useLang(); // re-render the chrome when the UI language changes
  const [activeHub, setActiveHub] = useState(hubFromHash);
  const [paletteOpen, setPaletteOpen] = useState(false);

  function selectHub(id: string) {
    setActiveHub(id);
    window.location.hash = id;
  }

  // Follow hash changes (palette jumps, back/forward) and own the Ctrl/⌘-K
  // shortcut globally.
  useEffect(() => {
    const onHash = () => setActiveHub(hubFromHash());
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      }
    };
    window.addEventListener("hashchange", onHash);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("hashchange", onHash);
      window.removeEventListener("keydown", onKey);
    };
  }, []);
  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [loginBusy, setLoginBusy] = useState(false);
  const [loginError, setLoginError] = useState<string | null>(null);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [alerts, setAlerts] = useState<Notification[]>([]);
  const refreshAlerts = () => api.listNotifications().then(setAlerts).catch(() => undefined);
  const [localIntel, setLocalIntel] = useState<LocalIntel | null>(null);
  const [safety, setSafety] = useState<SystemSafetyView | null>(null);
  const [podRisk, setPodRisk] = useState<PodRiskView | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      setStatusError("Design preview — desktop shell not attached");
      return;
    }
    api
      .serverStatus()
      .then(setStatus)
      .catch((e) => setStatusError(String(e)));
    api.listCharacters().then(setCharacters).catch(() => undefined);

    // Poll the notification center + local-log intel for the rail. Both are
    // cheap local reads, so a tight 5s cadence keeps the rail feeling live.
    const refresh = () => {
      refreshAlerts();
      api.getLocalIntel().then(setLocalIntel).catch(() => undefined);
    };
    refresh();
    const timer = window.setInterval(refresh, 5000);
    return () => window.clearInterval(timer);
  }, []);

  // System safety makes several zKill calls per refresh, so poll it slowly.
  useEffect(() => {
    if (!isTauri()) return;
    const refreshSafety = () => {
      api.getSystemSafety().then(setSafety).catch(() => undefined);
      api.getPodRisk().then(setPodRisk).catch(() => undefined);
    };
    refreshSafety();
    const timer = window.setInterval(refreshSafety, 30000);
    return () => window.clearInterval(timer);
  }, []);

  const activeCharacter = characters.find((c) => c.active) ?? null;
  const [charStatus, setCharStatus] = useState<CharacterStatusView | null>(null);
  const [fleet, setFleet] = useState<FleetView | null>(null);

  useEffect(() => {
    if (!isTauri() || !activeCharacter) {
      setCharStatus(null);
      setFleet(null);
      return;
    }
    const load = () => {
      api.getCharacterStatus(activeCharacter.id).then(setCharStatus).catch(() => undefined);
      // Fleet membership changes often; the read is cache-served (~5s) so a
      // 15s poll keeps the proximity panel live without hammering ESI.
      api.getFleet(activeCharacter.id).then(setFleet).catch(() => setFleet(null));
    };
    load();
    const timer = window.setInterval(load, 15000);
    return () => window.clearInterval(timer);
  }, [activeCharacter?.id]);

  // Aura's mood reflects the app state: unread alert severity, connection, roster.
  const unread = alerts.filter((a) => !a.read);
  const topSev = unread.reduce((m, a) => Math.max(m, SEV_RANK[a.severity]), 0);
  let mood: Mood = "calm";
  let auraStatus = "All systems nominal.";
  if (!isTauri()) {
    mood = "idle";
    auraStatus = "Standing by — desktop shell not attached.";
  } else if (statusError) {
    mood = "warning";
    auraStatus = "I'm having trouble reaching ESI.";
  } else if (topSev >= 3) {
    mood = "critical";
    auraStatus = `${unread.length} alert${unread.length === 1 ? "" : "s"} need your attention.`;
  } else if (topSev >= 2) {
    mood = "warning";
    auraStatus = `${unread.length} thing${unread.length === 1 ? "" : "s"} worth a look.`;
  } else if (characters.length > 0) {
    mood = "happy";
    auraStatus = "Everything looks good, capsuleer.";
  } else {
    auraStatus = "Ready when you are.";
  }

  async function reloadCharacters() {
    const list = await api.listCharacters();
    setCharacters(list);
    return list;
  }

  async function onLogin() {
    if (!isTauri()) {
      setLoginError("Login runs in the desktop app, not the browser preview.");
      return;
    }
    setLoginBusy(true);
    setLoginError(null);
    try {
      // One backend round-trip: opens the system browser, captures the loopback
      // redirect, and resolves with the added character.
      const character = await api.login();
      const list = await reloadCharacters();
      // First character (or none active yet) becomes the active one, so the
      // hubs have something to show immediately.
      if (!list.some((c) => c.active)) {
        await api.setActiveCharacter(character.id);
        await reloadCharacters();
      }
    } catch (e) {
      setLoginError(String(e));
      setStatusError(String(e));
    } finally {
      setLoginBusy(false);
    }
  }

  async function onSelectCharacter(characterId: number) {
    try {
      await api.setActiveCharacter(characterId);
      await reloadCharacters();
    } catch (e) {
      setStatusError(String(e));
    }
  }

  return (
    <div className="app-shell">
      <Starfield />
      {/* Left hub rail */}
      <nav className="hub-rail">
        <div className="brand" title="EVE Commander">
          EC
        </div>
        {HUBS.map((h) => (
          <button
            key={h.id}
            className={h.id === activeHub ? "active" : ""}
            title={t(`hub.${h.id}`)}
            onClick={() => selectHub(h.id)}
          >
            {h.icon}
          </button>
        ))}
      </nav>

      {/* Top context bar — always-foreground identity & status */}
      <header className="context-bar">
        <span className="pill">
          {activeCharacter && (
            <img className="avatar-sm" src={portraitUrl(activeCharacter.id, 32)} alt="" width={20} height={20} />
          )}
          <strong>{activeCharacter ? activeCharacter.name : "No active character"}</strong>
          {charStatus && (
            <span className={`online-tag ${charStatus.online ? "on" : "off"}`}>
              {charStatus.online ? "online" : "offline"}
            </span>
          )}
        </span>
        <span className="pill">📍 {charStatus?.system_name ?? "—"}</span>
        {charStatus?.ship_type_name && (
          <span className="pill" title={charStatus.ship_type_name}>
            🚀 {charStatus.ship_name || charStatus.ship_type_name}
          </span>
        )}
        <span className="pill">
          ⏱{" "}
          {charStatus?.training
            ? `${charStatus.training}${
                charStatus.training_seconds_remaining != null
                  ? ` · ${fmtDuration(charStatus.training_seconds_remaining)}`
                  : ""
              }`
            : "—"}
        </span>
        <span className="spacer" />
        <span className="pill">
          <span className={`conn-dot ${isTauri() ? (statusError ? "err" : "ok") : ""}`} />
          {isTauri() ? "ESI" : "preview"}
        </span>
      </header>

      {/* Main canvas */}
      <main className="main-canvas">
        {renderHub(activeHub, { status, statusError, characters, onLogin, onSelectCharacter, loginBusy, loginError }, activeCharacter)}
      </main>

      {/* Right Situational Awareness rail — always present in every hub */}
      <aside className="sa-rail">
        <div className="sa-section agent-section">
          <AgentAvatar mood={mood} />
          <div className="agent-meta">
            <div className="agent-name">Aura <span className="agent-tag">AI</span></div>
            <div className="agent-status">{auraStatus}</div>
          </div>
        </div>
        <div className="sa-section">
          <h3>System Safety</h3>
          {!safety || !safety.found || !safety.current ? (
            <div className="sa-empty">
              Recent kills in your system &amp; neighbours appear here once a character is active and
              in space.
            </div>
          ) : (
            <div className="safety-block">
              <div className={`safety-headline sev-${safety.level.toLowerCase()}`}>{safety.message}</div>
              <ul className="safety-list">
                <li className="safety-current">
                  <span>{safety.current.name} <span className="safety-sec">{safety.current.security.toFixed(1)}</span></span>
                  <span className="mono">{safety.current.kills_last_hour} kills</span>
                </li>
                {safety.neighbors.slice(0, 8).map((n) => (
                  <li key={n.system_id}>
                    <span>{n.name} <span className="safety-sec">{n.security.toFixed(1)}</span></span>
                    <span className={`mono${n.kills_last_hour > 0 ? " neg" : ""}`}>{n.kills_last_hour}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
        {podRisk && podRisk.found && podRisk.implant_count > 0 && (
          <div className="sa-section">
            <h3>Pod Risk</h3>
            <div className={`safety-block${podRisk.danger ? " sev-danger" : ""}`}>
              <div className={`safety-headline${podRisk.danger ? " sev-danger" : ""}`}>
                {podRisk.message}
              </div>
              <ul className="safety-list">
                <li className="safety-current">
                  <span>{podRisk.implant_count} implant(s)</span>
                  <span className={`mono${podRisk.danger ? " neg" : ""}`}>{fmtIsk(podRisk.implant_value)}</span>
                </li>
              </ul>
            </div>
          </div>
        )}
        <div className="sa-section">
          <h3>Local{localIntel?.system ? ` · ${localIntel.system}` : ""}</h3>
          {!localIntel || localIntel.speakers.length === 0 ? (
            <div className="sa-empty">
              Pilots who speak in Local appear here from your chat logs. Use the Combat &amp; Intel hub
              to paste-scan the full Local roster.
            </div>
          ) : (
            <ul className="local-list">
              {localIntel.speakers.slice(0, 12).map((name) => (
                <li key={name}>{name}</li>
              ))}
            </ul>
          )}
        </div>
        <div className="sa-section">
          <h3>Fleet Proximity{fleet?.in_fleet ? ` · ${fleet.member_count}` : ""}</h3>
          {!activeCharacter ? (
            <div className="sa-empty">Select a character to see its fleet.</div>
          ) : !fleet || !fleet.in_fleet ? (
            <div className="sa-empty">
              Not in a fleet. Members, their system, and ship appear here when you join one
              (needs the fleet read scope).
            </div>
          ) : (
            <ul className="local-list">
              {[...fleet.members]
                .sort((a, b) => (a.jumps ?? 999) - (b.jumps ?? 999))
                .slice(0, 20)
                .map((m) => (
                  <li key={m.character_id}>
                    <span>{m.name}</span>
                    <span style={{ color: "var(--text-dim)", fontSize: 11, marginLeft: 6 }}>
                      {m.jumps != null && (
                        <strong style={{ color: m.jumps === 0 ? "var(--safe)" : "var(--text)" }}>
                          {m.jumps === 0 ? "here" : `${m.jumps}j`}{" · "}
                        </strong>
                      )}
                      {m.system}{m.ship ? ` · ${m.ship}` : ""}
                    </span>
                  </li>
                ))}
            </ul>
          )}
        </div>
        <div className="sa-section">
          <h3>
            Alerts
            {unread.length > 0 && <span className="badge caution" style={{ marginLeft: 6 }}>{unread.length}</span>}
            {alerts.length > 0 && (
              <button
                className="sa-action"
                title="Mark all read"
                onClick={() => api.markNotificationsRead().then(refreshAlerts).catch(() => undefined)}
              >
                ✓ all
              </button>
            )}
          </h3>
          {alerts.length === 0 ? (
            <div className="sa-empty">
              Skill-queue and structure-fuel alerts collect here (evaluated every minute).
            </div>
          ) : (
            <ul className="alert-list">
              {alerts.map((a) => (
                <li key={a.key} className={`alert sev-${a.severity.toLowerCase()}${a.read ? " read" : ""}`}>
                  <span className="alert-title">
                    {a.title}
                    <button
                      className="sa-action"
                      title="Dismiss"
                      onClick={() => api.dismissNotification(a.key).then(refreshAlerts).catch(() => undefined)}
                    >
                      ×
                    </button>
                  </span>
                  <span className="alert-body">{a.body}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </aside>
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} />
    </div>
  );
}
