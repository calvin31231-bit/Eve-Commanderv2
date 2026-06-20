import { useEffect, useState } from "react";
import { api, isTauri } from "./ipc";
import { HUBS, renderHub } from "./hubs";
import type { Character, Notification, ServerStatus } from "./types";
import "./app.css";

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

    // Poll the notification center for the Alerts rail.
    const refreshAlerts = () =>
      api.listNotifications().then(setAlerts).catch(() => undefined);
    refreshAlerts();
    const timer = window.setInterval(refreshAlerts, 5000);
    return () => window.clearInterval(timer);
  }, []);

  const activeCharacter = characters.find((c) => c.active) ?? null;

  async function onLogin() {
    try {
      // One backend round-trip: opens the system browser, captures the loopback
      // redirect, and resolves with the added character.
      const character = await api.login();
      setCharacters((prev) => {
        const without = prev.filter((c) => c.id !== character.id);
        return [...without, character];
      });
    } catch (e) {
      setStatusError(String(e));
    }
  }

  return (
    <div className="app-shell">
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
          <strong>{activeCharacter ? activeCharacter.name : "No active character"}</strong>
        </span>
        <span className="pill">📍 —</span>
        <span className="pill">
          Safety <span className="badge safe">—</span>
        </span>
        <span className="pill">⏱ Training —</span>
        <span className="spacer" />
        <span className="pill">
          <span className={`conn-dot ${isTauri() ? (statusError ? "err" : "ok") : ""}`} />
          {isTauri() ? "ESI" : "preview"}
        </span>
      </header>

      {/* Main canvas */}
      <main className="main-canvas">
        {renderHub(activeHub, { status, statusError, characters, onLogin }, activeCharacter)}
      </main>

      {/* Right Situational Awareness rail — always present in every hub */}
      <aside className="sa-rail">
        <div className="sa-section">
          <h3>System Safety / Killfeed</h3>
          <div className="sa-empty">Live kills in your system &amp; neighbors appear here (Phase 4).</div>
        </div>
        <div className="sa-section">
          <h3>Local Threat Scanner</h3>
          <div className="sa-empty">Copy Local in-game to flag Safe / Pirate / Danger pilots (Phase 4).</div>
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
