import { useEffect, useState } from "react";
import { api, isTauri } from "./ipc";
import { HUBS, renderHub } from "./hubs";
import type { Character, ServerStatus } from "./types";
import "./app.css";

export default function App() {
  const [activeHub, setActiveHub] = useState("home");
  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [characters, setCharacters] = useState<Character[]>([]);

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
  }, []);

  const activeCharacter = characters.find((c) => c.active) ?? null;

  async function onLogin() {
    try {
      const url = await api.beginLogin();
      // In the desktop shell this opens the system browser; here we just log it.
      console.info("authorize url:", url);
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
            onClick={() => setActiveHub(h.id)}
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
        {renderHub(activeHub, { status, statusError, characters, onLogin })}
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
          <div className="sa-empty">Fuel timers, job completions, intel pings (Phase 4).</div>
        </div>
      </aside>
    </div>
  );
}
