// The top-level hubs. The 25+ feature modules from the ROADMAP collapse into a
// small set of intent-based hubs on the left rail (see ROADMAP "UI/UX
// Strategy"). Phase 0 ships the shell + Home; other hubs are placeholders that
// later phases fill in.

import type { ReactNode } from "react";
import type { Character, ServerStatus } from "./types";

export interface Hub {
  id: string;
  label: string;
  icon: string;
}

export const HUBS: Hub[] = [
  { id: "home", label: "Home", icon: "◎" },
  { id: "character", label: "Character", icon: "☺" },
  { id: "economy", label: "Economy", icon: "₿" },
  { id: "combat", label: "Combat & Intel", icon: "⚔" },
  { id: "navigation", label: "Navigation & Logistics", icon: "➤" },
  { id: "corp", label: "Corp & Fleet", icon: "⛨" },
  { id: "tools", label: "Tools", icon: "⚙" },
];

function Placeholder({ title, blurb }: { title: string; blurb: string }): ReactNode {
  return (
    <>
      <h1>{title}</h1>
      <div className="sub">{blurb}</div>
      <div className="card">
        <span className="badge caution">Planned</span>
        <p style={{ color: "var(--text-dim)" }}>
          This hub lands in a later phase. See <code>docs/ROADMAP.md</code> for the module
          breakdown and the phase it ships in.
        </p>
      </div>
    </>
  );
}

interface HomeProps {
  status: ServerStatus | null;
  statusError: string | null;
  characters: Character[];
  onLogin: () => void;
}

function Home({ status, statusError, characters, onLogin }: HomeProps): ReactNode {
  return (
    <>
      <h1>Welcome, Capsuleer</h1>
      <div className="sub">Your at-a-glance command center.</div>

      <div className="card-grid">
        <div className="card">
          <h3>Tranquility status</h3>
          {status ? (
            <p className="mono" style={{ fontSize: 24 }}>
              {status.players.toLocaleString()} <span style={{ color: "var(--text-dim)", fontSize: 13 }}>online</span>
            </p>
          ) : (
            <p style={{ color: statusError ? "var(--danger)" : "var(--text-dim)" }}>
              {statusError ?? "Loading…"}
            </p>
          )}
        </div>

        <div className="card">
          <h3>Characters</h3>
          {characters.length === 0 ? (
            <>
              <p style={{ color: "var(--text-dim)" }}>No characters yet.</p>
              <button className="primary" onClick={onLogin}>
                Log in with EVE
              </button>
            </>
          ) : (
            <ul style={{ margin: 0, paddingLeft: 18 }}>
              {characters.map((c) => (
                <li key={c.id}>
                  {c.name} {c.active && <span className="badge safe">active</span>}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </>
  );
}

export function renderHub(hubId: string, home: HomeProps): ReactNode {
  switch (hubId) {
    case "home":
      return <Home {...home} />;
    case "character":
      return <Placeholder title="Character" blurb="Skills, wallet, assets, clones, mining, mail." />;
    case "economy":
      return <Placeholder title="Economy" blurb="Market, industry, reactions, PI, reprocessing." />;
    case "combat":
      return <Placeholder title="Combat & Intel" blurb="Killboard, intel map, D-scan, threat scanner, fitting, AAR." />;
    case "navigation":
      return <Placeholder title="Navigation & Logistics" blurb="Routes, capital/JF, courier, bookmarks." />;
    case "corp":
      return <Placeholder title="Corp & Fleet" blurb="Members, SRP, structures, fleet boss, recruitment." />;
    case "tools":
      return <Placeholder title="Tools" blurb="Reprocessing, insurance, LP, abyssal calculators." />;
    default:
      return <Placeholder title="Unknown" blurb="" />;
  }
}
