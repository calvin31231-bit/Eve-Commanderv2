import { useEffect, useState } from "react";
import { api, isTauri } from "./ipc";
import { HUBS, renderHub, portraitUrl } from "./hubs";
import { Starfield } from "./Starfield";
import { AgentAvatar, type Mood } from "./AgentAvatar";
import type { Character, CharacterStatusView, LocalIntel, Notification, ServerStatus, Severity, SystemSafetyView } from "./types";
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

// Hubs are deep-linkable via the URL hash (e.g. #combat) so a view can be
// restored on launch, linked to, or popped into its own window later.
function initialHub(): string {
  const fromHash = window.location.hash.replace(/^#/, "");
  return HUBS.some((h) => h.id === fromHash) ? fromHash : "home";
}

export default function App() {
  const [activeHub, setActiveHub] = useState(initialHub);

  function selectHub(id: string) {
    setActiveHub(id);
    window.location.hash = id;
  }
  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [alerts, setAlerts] = useState<Notification[]>([]);
  const [localIntel, setLocalIntel] = useState<LocalIntel | null>(null);
  const [safety, setSafety] = useState<SystemSafetyView | null>(null);

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
      api.listNotifications().then(setAlerts).catch(() => undefined);
      api.getLocalIntel().then(setLocalIntel).catch(() => undefined);
    };
    refresh();
    const timer = window.setInterval(refresh, 5000);
    return () => window.clearInterval(timer);
  }, []);

  // System safety makes several zKill calls per refresh, so poll it slowly.
  useEffect(() => {
    if (!isTauri()) return;
    const refreshSafety = () =>
      api.getSystemSafety().then(setSafety).catch(() => undefined);
    refreshSafety();
    const timer = window.setInterval(refreshSafety, 30000);
    return () => window.clearInterval(timer);
  }, []);

  const activeCharacter = characters.find((c) => c.active) ?? null;
  const [charStatus, setCharStatus] = useState<CharacterStatusView | null>(null);

  useEffect(() => {
    if (!isTauri() || !activeCharacter) {
      setCharStatus(null);
      return;
    }
    const load = () =>
      api.getCharacterStatus(activeCharacter.id).then(setCharStatus).catch(() => undefined);
    load();
    const t = window.setInterval(load, 30000);
    return () => window.clearInterval(t);
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
      setStatusError(String(e));
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
            title={h.label}
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
        {renderHub(activeHub, { status, statusError, characters, onLogin, onSelectCharacter }, activeCharacter)}
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
          <h3>Fleet Proximity</h3>
          <div className="sa-empty">Fleet members and their distance from you (Phase 5).</div>
        </div>
        <div className="sa-section">
          <h3>Alerts</h3>
          {alerts.length === 0 ? (
            <div className="sa-empty">Fuel timers, job completions, intel pings (Phase 4).</div>
          ) : (
            <ul className="alert-list">
              {alerts.map((a) => (
                <li key={a.key} className={`alert sev-${a.severity.toLowerCase()}${a.read ? " read" : ""}`}>
                  <span className="alert-title">{a.title}</span>
                  <span className="alert-body">{a.body}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </aside>
    </div>
  );
}
