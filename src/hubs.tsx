// The top-level hubs. The 25+ feature modules from the ROADMAP collapse into a
// small set of intent-based hubs on the left rail (see ROADMAP "UI/UX
// Strategy"). Phase 0 ships the shell + Home; other hubs are placeholders that
// later phases fill in.

import { useEffect, useState, type ReactNode } from "react";
import { api, isTauri } from "./ipc";
import type {
  CashflowSummary,
  Character,
  CharacterSheet,
  ClonesView,
  HoldingsView,
  IndustryJobView,
  MailHeader,
  MailView,
  MarketView,
  MiningView,
  ServerStatus,
} from "./types";

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

const ISK = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });

function formatDuration(seconds: number): string {
  if (seconds <= 0) return "complete";
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

// ESI ref_types are snake_case (e.g. "market_transaction"); show them as words.
function prettyRefType(refType: string): string {
  return refType.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

function shortDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

// A minimal EVEmail reader: header list with click-to-read body.
function MailCard({ character }: { character: Character }): ReactNode {
  const [headers, setHeaders] = useState<MailHeader[]>([]);
  const [openId, setOpenId] = useState<number | null>(null);
  const [open, setOpen] = useState<MailView | null>(null);

  useEffect(() => {
    setHeaders([]);
    setOpenId(null);
    setOpen(null);
    if (!isTauri()) return;
    api.getMailHeaders(character.id).then(setHeaders).catch(() => undefined);
  }, [character.id]);

  function toggle(h: MailHeader) {
    if (openId === h.mail_id) {
      setOpenId(null);
      setOpen(null);
      return;
    }
    setOpenId(h.mail_id);
    setOpen(null);
    api.getMail(character.id, h.mail_id).then(setOpen).catch(() => undefined);
  }

  if (headers.length === 0) return null;
  const unread = headers.filter((h) => !h.is_read).length;

  return (
    <div className="card" style={{ marginTop: 16 }}>
      <h3>Mail {unread > 0 && <span style={{ color: "var(--accent)", fontWeight: 400 }}>· {unread} unread</span>}</h3>
      <ul className="mail-list">
        {headers.slice(0, 12).map((h) => (
          <li key={h.mail_id}>
            <button className={`mail-row${h.is_read ? "" : " unread"}`} onClick={() => toggle(h)}>
              <span className="mail-subject">{h.subject || "(no subject)"}</span>
              <span className="mail-date">{shortDate(h.timestamp)}</span>
            </button>
            {openId === h.mail_id && (
              <pre className="mail-body">{open ? open.body : "Loading…"}</pre>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

function CharacterHub({ character }: { character: Character | null }): ReactNode {
  const [sheet, setSheet] = useState<CharacterSheet | null>(null);
  const [holdings, setHoldings] = useState<HoldingsView | null>(null);
  const [clones, setClones] = useState<ClonesView | null>(null);
  const [cashflow, setCashflow] = useState<CashflowSummary | null>(null);
  const [mining, setMining] = useState<MiningView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setSheet(null);
    setHoldings(null);
    setClones(null);
    setCashflow(null);
    setMining(null);
    setError(null);
    if (!character) return;
    if (!isTauri()) {
      setError("Design preview — connect the desktop shell to load live data.");
      return;
    }
    setLoading(true);
    api
      .getCharacterSheet(character.id)
      .then(setSheet)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
    // Assets are heavier (paginated) — load independently; failures are
    // non-fatal to the rest of the hub.
    api
      .getTopHoldings(character.id, 8)
      .then(setHoldings)
      .catch(() => undefined);
    api
      .getClones(character.id)
      .then(setClones)
      .catch(() => undefined);
    api
      .getCashflow(character.id)
      .then(setCashflow)
      .catch(() => undefined);
    api
      .getMining(character.id)
      .then(setMining)
      .catch(() => undefined);
  }, [character?.id]);

  if (!character) {
    return (
      <>
        <h1>Character</h1>
        <div className="sub">No active character. Add one from Home, then select it.</div>
      </>
    );
  }

  return (
    <>
      <h1>{character.name}</h1>
      <div className="sub">Skills, training, and wallet.</div>
      {error && !sheet && <div className="card"><p style={{ color: "var(--text-dim)" }}>{error}</p></div>}
      {loading && <div className="card"><p style={{ color: "var(--text-dim)" }}>Loading…</p></div>}
      {sheet && (
        <div className="card-grid">
          <div className="card">
            <h3>Wallet</h3>
            <p className="mono" style={{ fontSize: 22 }}>
              {ISK.format(sheet.wallet_balance)} <span style={{ color: "var(--text-dim)", fontSize: 13 }}>ISK</span>
            </p>
          </div>
          <div className="card">
            <h3>Skill points</h3>
            <p className="mono" style={{ fontSize: 22 }}>{ISK.format(sheet.total_sp)}</p>
            <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
              {sheet.skill_count} skills · {sheet.maxed_count} at level V
              {sheet.unallocated_sp ? ` · ${ISK.format(sheet.unallocated_sp)} unallocated` : ""}
            </p>
          </div>
          <div className="card">
            <h3>Training</h3>
            {sheet.queue_len === 0 ? (
              <p style={{ color: "var(--caution)" }}>Skill queue is empty.</p>
            ) : (
              <>
                <p className="mono" style={{ fontSize: 22 }}>
                  {sheet.queue_seconds_remaining != null ? formatDuration(sheet.queue_seconds_remaining) : "—"}
                </p>
                <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
                  {sheet.queue_len} skill{sheet.queue_len === 1 ? "" : "s"} queued
                </p>
              </>
            )}
          </div>
          {clones && (
            <div className="card">
              <h3>Clones</h3>
              <p className="mono" style={{ fontSize: 22 }}>{clones.jump_clone_count}</p>
              <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
                jump clone{clones.jump_clone_count === 1 ? "" : "s"} · {clones.active_implant_count} active implant
                {clones.active_implant_count === 1 ? "" : "s"}
              </p>
              {clones.implants.length > 0 && (
                <ul className="implant-list">
                  {clones.implants.map((i) => (
                    <li key={i.type_id}>{i.name}</li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </div>
      )}
      {cashflow && cashflow.entry_count > 0 && (
        <div className="card" style={{ marginTop: 16 }}>
          <h3>Cashflow <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {cashflow.entry_count} journal entries</span></h3>
          <div className="cashflow-totals">
            <span className="pos">+{ISK.format(cashflow.income)}</span>
            <span className="neg">{ISK.format(cashflow.expenses)}</span>
            <span className={cashflow.net >= 0 ? "pos" : "neg"}>
              net {cashflow.net >= 0 ? "+" : ""}{ISK.format(cashflow.net)} ISK
            </span>
          </div>
          <table className="holdings">
            <tbody>
              {cashflow.by_ref_type.map((r) => (
                <tr key={r.ref_type}>
                  <td>{prettyRefType(r.ref_type)}</td>
                  <td className={`mono num ${r.total >= 0 ? "pos" : "neg"}`}>
                    {r.total >= 0 ? "+" : ""}{ISK.format(r.total)}
                  </td>
                  <td className="loc">×{r.count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {holdings && holdings.groups.length > 0 && (
        <div className="card" style={{ marginTop: 16 }}>
          <h3>Top holdings <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {ISK.format(holdings.total_value)} ISK est.</span></h3>
          <table className="holdings">
            <tbody>
              {holdings.groups.map((h) => (
                <tr key={h.type_id}>
                  <td>{h.name}</td>
                  <td className="mono num">×{ISK.format(h.quantity)}</td>
                  <td className="mono num pos">{ISK.format(h.value)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {mining && mining.total_units > 0 && (
        <div className="card" style={{ marginTop: 16 }}>
          <h3>Mining <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {ISK.format(mining.total_value)} ISK over {mining.day_count} day{mining.day_count === 1 ? "" : "s"}</span></h3>
          <table className="holdings">
            <tbody>
              {mining.ores.map((o) => (
                <tr key={o.type_id}>
                  <td>{o.name}</td>
                  <td className="mono num">×{ISK.format(o.quantity)}</td>
                  <td className="mono num pos">{ISK.format(o.value)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <MailCard character={character} />
    </>
  );
}

function EconomyHub({ character }: { character: Character | null }): ReactNode {
  const [jobs, setJobs] = useState<IndustryJobView[]>([]);
  const [market, setMarket] = useState<MarketView | null>(null);
  const [loadedAt, setLoadedAt] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [, forceTick] = useState(0);

  useEffect(() => {
    setJobs([]);
    setMarket(null);
    setError(null);
    if (!character) return;
    if (!isTauri()) {
      setError("Design preview — connect the desktop shell to load live data.");
      return;
    }
    setLoadedAt(Date.now());
    api
      .getIndustryJobs(character.id)
      .then(setJobs)
      .catch((e) => setError(String(e)));
    api
      .getMarketOrders(character.id)
      .then(setMarket)
      .catch(() => undefined);
  }, [character?.id]);

  // Tick once a second so the countdowns advance without re-polling ESI.
  const hasTimers = jobs.length > 0 || (market?.orders.length ?? 0) > 0;
  useEffect(() => {
    if (!hasTimers) return;
    const t = window.setInterval(() => forceTick((n) => n + 1), 1000);
    return () => window.clearInterval(t);
  }, [hasTimers]);

  if (!character) {
    return (
      <>
        <h1>Economy</h1>
        <div className="sub">No active character. Add one from Home, then select it.</div>
      </>
    );
  }

  const elapsed = loadedAt ? Math.floor((Date.now() - loadedAt) / 1000) : 0;
  const countdown = (seconds: number) => {
    const remaining = Math.max(0, seconds - elapsed);
    return remaining === 0 ? "Ready" : formatDuration(remaining);
  };

  return (
    <>
      <h1>Economy</h1>
      <div className="sub">Industry jobs, market, and more.</div>
      {error && <div className="card"><p style={{ color: "var(--text-dim)" }}>{error}</p></div>}
      <div className="card">
        <h3>Industry jobs {jobs.length > 0 && <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {jobs.length} active</span>}</h3>
        {jobs.length === 0 && !error ? (
          <p style={{ color: "var(--text-dim)" }}>No jobs in progress.</p>
        ) : (
          <table className="holdings">
            <tbody>
              {jobs.map((j) => {
                const remaining = Math.max(0, j.seconds_remaining - elapsed);
                return (
                  <tr key={j.job_id}>
                    <td>{j.item_name}</td>
                    <td className="loc">{j.activity}{j.runs > 1 ? ` ×${j.runs}` : ""}</td>
                    <td className={`mono num ${remaining === 0 ? "pos" : ""}`}>
                      {remaining === 0 ? "Ready" : formatDuration(remaining)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
      {market && (
        <div className="card" style={{ marginTop: 16 }}>
          <h3>Market orders <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {market.sell_count} sell · {market.buy_count} buy</span></h3>
          <div className="cashflow-totals">
            <span className="pos">{ISK.format(market.sell_value)} ISK listed</span>
            <span className="neg">{ISK.format(market.total_escrow)} ISK escrow</span>
          </div>
          {market.orders.length > 0 && (
            <table className="holdings">
              <tbody>
                {market.orders.map((o) => (
                  <tr key={o.order_id}>
                    <td>
                      <span className={o.is_buy_order ? "neg" : "pos"}>{o.is_buy_order ? "BUY" : "SELL"}</span> {o.item_name}
                    </td>
                    <td className="mono num">{ISK.format(o.price)}</td>
                    <td className="loc">{countdown(o.seconds_remaining)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}
    </>
  );
}

export function renderHub(hubId: string, home: HomeProps, activeCharacter: Character | null): ReactNode {
  switch (hubId) {
    case "home":
      return <Home {...home} />;
    case "character":
      return <CharacterHub character={activeCharacter} />;
    case "economy":
      return <EconomyHub character={activeCharacter} />;
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
