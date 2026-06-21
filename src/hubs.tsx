// The top-level hubs. The 25+ feature modules from the ROADMAP collapse into a
// small set of intent-based hubs on the left rail (see ROADMAP "UI/UX
// Strategy"). Phase 0 ships the shell + Home; other hubs are placeholders that
// later phases fill in.

import { useEffect, useState, type ReactNode } from "react";
import { api, isTauri } from "./ipc";
import type {
  AccountOverview,
  AppSettings,
  CashflowSummary,
  Character,
  CharacterAttributes,
  CharacterGroup,
  Contract,
  CharacterProfile,
  QueuedSkillView,
  TransactionView,
  CharacterSheet,
  ClonesView,
  HoldingsView,
  IndustryJobView,
  ItemHit,
  LocationValueView,
  MailHeader,
  MarketBrowse,
  MailView,
  MarketView,
  MiningView,
  ServerStatus,
  TradeOpportunity,
  ReprocessView,
  BuildPlanView,
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
  onSelectCharacter: (characterId: number) => void;
}

function Home({ status, statusError, characters, onLogin, onSelectCharacter }: HomeProps): ReactNode {
  const [account, setAccount] = useState<AccountOverview | null>(null);

  useEffect(() => {
    if (!isTauri() || characters.length === 0) {
      setAccount(null);
      return;
    }
    api.getAccountOverview().then(setAccount).catch(() => undefined);
  }, [characters.length]);

  return (
    <>
      <h1>Welcome, Capsuleer</h1>
      <div className="sub">Your at-a-glance command center.</div>

      {account && account.characters.length > 0 && (
        <div className="card account-card">
          <div className="account-headline">
            <div>
              <h3>Account net worth</h3>
              <p className="mono account-networth">{ISK.format(account.total_net_worth)} <span>ISK</span></p>
            </div>
            <div className="account-subtotals">
              <span>{ISK.format(account.total_wallet)} <small>wallet</small></span>
              <span>{ISK.format(account.total_asset_value)} <small>assets</small></span>
              <span>{ISK.format(account.total_sp)} <small>SP</small></span>
            </div>
          </div>
          {account.characters.length > 1 && (
            <table className="holdings account-breakdown">
              <thead>
                <tr><th>Character</th><th>Net worth</th><th>Wallet</th><th>SP</th></tr>
              </thead>
              <tbody>
                {account.characters.map((c) => (
                  <tr key={c.character_id}>
                    <td>{c.name}</td>
                    <td className="mono num pos">{ISK.format(c.net_worth)}</td>
                    <td className="mono num">{ISK.format(c.wallet_balance)}</td>
                    <td className="mono num">{ISK.format(c.total_sp)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

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
            <>
              <ul className="char-list">
                {characters.map((c) => (
                  <li key={c.id}>
                    <button
                      className={`char-row${c.active ? " active" : ""}`}
                      onClick={() => onSelectCharacter(c.id)}
                      title={c.active ? "Active character" : "Make active"}
                    >
                      <span className="char-row-id">
                        <img className="avatar" src={portraitUrl(c.id, 32)} alt="" width={24} height={24} loading="lazy" />
                        {c.name}
                      </span>
                      {c.active && <span className="badge safe">active</span>}
                    </button>
                  </li>
                ))}
              </ul>
              <button className="primary" style={{ marginTop: 10 }} onClick={onLogin}>
                Add character
              </button>
            </>
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

// EVE's public image server serves character portraits (no auth).
export function portraitUrl(id: number, size = 64): string {
  return `https://images.evetech.net/characters/${id}/portrait?size=${size}`;
}

function secColor(sec: number): string {
  if (sec >= 4.5) return "var(--safe)";
  if (sec >= 0) return "var(--text-dim)";
  if (sec > -4.5) return "var(--caution)";
  return "var(--danger)";
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
    // Mark read on the server and reflect it locally.
    if (!h.is_read) {
      api.markMailRead(character.id, h.mail_id).catch(() => undefined);
      setHeaders((prev) => prev.map((x) => (x.mail_id === h.mail_id ? { ...x, is_read: true } : x)));
    }
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
              <span className="mail-main">
                <span className="mail-subject">{h.subject || "(no subject)"}</span>
                <span className="mail-from">{h.from_name}</span>
              </span>
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
  const [locations, setLocations] = useState<LocationValueView[]>([]);
  const [clones, setClones] = useState<ClonesView | null>(null);
  const [cashflow, setCashflow] = useState<CashflowSummary | null>(null);
  const [mining, setMining] = useState<MiningView | null>(null);
  const [profile, setProfile] = useState<CharacterProfile | null>(null);
  const [queue, setQueue] = useState<QueuedSkillView[]>([]);
  const [attrs, setAttrs] = useState<CharacterAttributes | null>(null);
  const [txns, setTxns] = useState<TransactionView[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setSheet(null);
    setHoldings(null);
    setLocations([]);
    setClones(null);
    setCashflow(null);
    setMining(null);
    setProfile(null);
    setQueue([]);
    setAttrs(null);
    setTxns([]);
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
      .getAssetsByLocation(character.id, 6)
      .then(setLocations)
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
    api
      .getCharacterProfile(character.id)
      .then(setProfile)
      .catch(() => undefined);
    api.getSkillQueue(character.id).then(setQueue).catch(() => undefined);
    api.getAttributes(character.id).then(setAttrs).catch(() => undefined);
    api.getTransactions(character.id, 10).then(setTxns).catch(() => undefined);
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
      <div className="char-header">
        <img className="portrait" src={portraitUrl(character.id, 128)} alt="" width={56} height={56} loading="lazy" />
        <div>
          <h1>{character.name}</h1>
          <div className="sub">
            {profile ? (
              <>
                {profile.corporation}
                {profile.alliance ? ` · ${profile.alliance}` : ""}
                {" · "}
                <span style={{ color: secColor(profile.security_status) }}>
                  {profile.security_status.toFixed(1)} sec
                </span>
              </>
            ) : (
              "Skills, training, wallet, assets, and mail."
            )}
          </div>
        </div>
      </div>
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
                {queue.length > 0 && (
                  <ul className="queue-list">
                    {queue.slice(0, 5).map((q) => (
                      <li key={q.queue_position}>
                        <span className={q.queue_position === 0 ? "training-now" : ""}>{q.name}</span>
                        <span className="mono">{q.seconds_remaining > 0 ? formatDuration(q.seconds_remaining) : "done"}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </>
            )}
          </div>
          {attrs && (
            <div className="card">
              <h3>Attributes</h3>
              <ul className="attr-list">
                <li><span>Intelligence</span><span className="mono">{attrs.intelligence}</span></li>
                <li><span>Memory</span><span className="mono">{attrs.memory}</span></li>
                <li><span>Perception</span><span className="mono">{attrs.perception}</span></li>
                <li><span>Willpower</span><span className="mono">{attrs.willpower}</span></li>
                <li><span>Charisma</span><span className="mono">{attrs.charisma}</span></li>
              </ul>
              {attrs.bonus_remaps != null && (
                <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 6 }}>
                  {attrs.bonus_remaps} bonus remap{attrs.bonus_remaps === 1 ? "" : "s"} available
                </p>
              )}
            </div>
          )}
          {clones && (
            <div className="card">
              <h3>Clones</h3>
              <p className="mono" style={{ fontSize: 22 }}>{clones.jump_clone_count}</p>
              <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
                jump clone{clones.jump_clone_count === 1 ? "" : "s"} · {clones.active_implant_count} active implant
                {clones.active_implant_count === 1 ? "" : "s"}
                {clones.home_location_name ? ` · home: ${clones.home_location_name}` : ""}
              </p>
              {clones.implants.length > 0 && (
                <>
                  <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "8px 0 2px" }}>Active clone</p>
                  <ul className="implant-list">
                    {clones.implants.map((i) => (
                      <li key={i.type_id}>{i.name}</li>
                    ))}
                  </ul>
                </>
              )}
              {clones.jump_clones.map((jc) => (
                <div key={jc.jump_clone_id} style={{ marginTop: 8 }}>
                  <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "0 0 2px" }}>
                    {jc.name || "Jump clone"} · {jc.location_name}
                  </p>
                  <ul className="implant-list">
                    {jc.implants.length === 0 ? (
                      <li style={{ opacity: 0.6 }}>no implants</li>
                    ) : (
                      jc.implants.map((i) => <li key={i.type_id}>{i.name}</li>)
                    )}
                  </ul>
                </div>
              ))}
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
      {txns.length > 0 && (
        <div className="card" style={{ marginTop: 16 }}>
          <h3>Recent transactions</h3>
          <table className="holdings">
            <tbody>
              {txns.map((t, i) => (
                <tr key={i}>
                  <td>
                    <span className={t.is_buy ? "neg" : "pos"}>{t.is_buy ? "BUY" : "SELL"}</span> {t.item_name}
                  </td>
                  <td className="mono num">×{ISK.format(t.quantity)}</td>
                  <td className={`mono num ${t.is_buy ? "neg" : "pos"}`}>{ISK.format(t.total)}</td>
                  <td className="loc">{shortDate(t.date)}</td>
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
      {locations.length > 0 && (
        <div className="card" style={{ marginTop: 16 }}>
          <h3>Assets by location</h3>
          <table className="holdings">
            <tbody>
              {locations.map((l, i) => (
                <tr key={i}>
                  <td>{l.location_name}</td>
                  <td className="loc">{l.item_count} item{l.item_count === 1 ? "" : "s"}</td>
                  <td className="mono num pos">{ISK.format(l.value)}</td>
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

function MarketBrowser(): ReactNode {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<ItemHit[]>([]);
  const [sel, setSel] = useState<ItemHit | null>(null);
  const [data, setData] = useState<MarketBrowse | null>(null);

  useEffect(() => {
    if (!isTauri() || q.trim().length < 2) {
      setHits([]);
      return;
    }
    const t = window.setTimeout(() => {
      api.searchItems(q.trim(), 8).then(setHits).catch(() => undefined);
    }, 200);
    return () => window.clearTimeout(t);
  }, [q]);

  function pick(h: ItemHit) {
    setSel(h);
    setHits([]);
    setQ(h.name);
    setData(null);
    api.getMarketBrowse(h.type_id).then(setData).catch(() => undefined);
  }

  return (
    <div className="card market-browser">
      <h3>Market <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· Jita / The Forge</span></h3>
      <div className="market-search">
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search an item…" />
        {hits.length > 0 && (
          <ul className="market-results">
            {hits.map((h) => (
              <li key={h.type_id}><button onClick={() => pick(h)}>{h.name}</button></li>
            ))}
          </ul>
        )}
      </div>
      {sel && data && (
        <div className="market-quote">
          <div className="market-name">{sel.name}</div>
          <div className="cashflow-totals">
            <span className="pos">{data.quote.best_sell != null ? `${ISK.format(data.quote.best_sell)} sell` : "no sell"}</span>
            <span className="neg">{data.quote.best_buy != null ? `${ISK.format(data.quote.best_buy)} buy` : "no buy"}</span>
            {data.quote.spread_pct != null && (
              <span>{(data.quote.spread_pct * 100).toFixed(1)}% spread</span>
            )}
          </div>
          <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
            30d avg {ISK.format(data.history.avg_30d)} · daily vol {ISK.format(data.history.daily_volume_30d)} ·{" "}
            {data.quote.sell_orders} sell / {data.quote.buy_orders} buy orders
          </p>
          {data.history.recent.length > 1 && <Sparkline values={data.history.recent} />}
          {data.hubs.length > 0 && (
            <table className="holdings" style={{ marginTop: 10 }}>
              <thead>
                <tr>
                  <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Hub</th>
                  <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Sell</th>
                  <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Buy</th>
                </tr>
              </thead>
              <tbody>
                {data.hubs.map((hb) => (
                  <tr key={hb.hub}>
                    <td>{hb.hub}</td>
                    <td className="mono num pos">{hb.best_sell != null ? ISK.format(hb.best_sell) : "—"}</td>
                    <td className="mono num neg">{hb.best_buy != null ? ISK.format(hb.best_buy) : "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {data.insurance && data.insurance.length > 0 && (
            <table className="holdings" style={{ marginTop: 10 }}>
              <thead>
                <tr><th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Insurance</th><th></th><th></th></tr>
              </thead>
              <tbody>
                {data.insurance.map((lv) => (
                  <tr key={lv.name}>
                    <td>{lv.name}</td>
                    <td className="mono num neg">{ISK.format(lv.cost)}</td>
                    <td className="mono num pos">{ISK.format(lv.payout)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}
    </div>
  );
}

function Sparkline({ values }: { values: number[] }): ReactNode {
  const w = 240;
  const h = 36;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const pts = values
    .map((v, i) => `${(i / (values.length - 1)) * w},${h - ((v - min) / span) * h}`)
    .join(" ");
  return (
    <svg className="sparkline" viewBox={`0 0 ${w} ${h}`} width={w} height={h} preserveAspectRatio="none">
      <polyline points={pts} fill="none" stroke="var(--accent)" strokeWidth="1.5" />
    </svg>
  );
}

function StationScanner(): ReactNode {
  const [rows, setRows] = useState<TradeOpportunity[] | null>(null);
  const [loading, setLoading] = useState(false);

  function scan() {
    if (!isTauri()) return;
    setLoading(true);
    api
      .scanStationTrades()
      .then(setRows)
      .catch(() => setRows([]))
      .finally(() => setLoading(false));
  }

  return (
    <div className="card station-scanner">
      <h3>
        Station Trade Scanner{" "}
        <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· Jita</span>
      </h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Best buy→sell flips on liquid items, net of 3% broker + 4.5% tax.
      </p>
      <button onClick={scan} disabled={loading}>
        {loading ? "Scanning…" : rows ? "Rescan" : "Scan"}
      </button>
      {rows && rows.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Item</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Margin</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Profit/u</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Daily</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((o) => (
              <tr key={o.type_id}>
                <td>{o.name}</td>
                <td className="mono num">{(o.margin_pct * 100).toFixed(1)}%</td>
                <td className="mono num pos">{ISK.format(o.profit_per_unit)}</td>
                <td className="mono num">{ISK.format(o.daily_potential)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {rows && rows.length === 0 && !loading && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>No profitable flips found.</p>
      )}
    </div>
  );
}

// Shared item search box: resolves a typed query to an ItemHit and reports the
// pick to the parent. Used by the reprocessing and build-planner cards.
function ItemPicker({
  placeholder,
  onPick,
}: {
  placeholder: string;
  onPick: (hit: ItemHit) => void;
}): ReactNode {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<ItemHit[]>([]);

  useEffect(() => {
    if (!isTauri() || q.trim().length < 2) {
      setHits([]);
      return;
    }
    const t = window.setTimeout(() => {
      api.searchItems(q.trim(), 8).then(setHits).catch(() => undefined);
    }, 200);
    return () => window.clearTimeout(t);
  }, [q]);

  return (
    <div className="market-search">
      <input value={q} onChange={(e) => setQ(e.target.value)} placeholder={placeholder} />
      {hits.length > 0 && (
        <ul className="market-results">
          {hits.map((h) => (
            <li key={h.type_id}>
              <button
                onClick={() => {
                  onPick(h);
                  setHits([]);
                  setQ(h.name);
                }}
              >
                {h.name}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function ReprocessCalc(): ReactNode {
  const [sel, setSel] = useState<ItemHit | null>(null);
  const [units, setUnits] = useState(100);
  const [eff, setEff] = useState(70);
  const [data, setData] = useState<ReprocessView | null>(null);
  const [missing, setMissing] = useState(false);

  function run(item: ItemHit, u: number, e: number) {
    setData(null);
    setMissing(false);
    api
      .reprocessItem(item.type_id, u, e / 100)
      .then((r) => (r ? setData(r) : setMissing(true)))
      .catch(() => setMissing(true));
  }

  return (
    <div className="card reprocess-calc">
      <h3>Reprocessing Calculator</h3>
      <ItemPicker
        placeholder="Search an ore or item…"
        onPick={(h) => {
          setSel(h);
          run(h, units, eff);
        }}
      />
      {sel && (
        <div className="cashflow-totals" style={{ marginTop: 8, gap: 12 }}>
          <label style={{ fontSize: 12 }}>
            Units{" "}
            <input
              type="number"
              value={units}
              style={{ width: 90 }}
              onChange={(e) => {
                const u = Number(e.target.value) || 0;
                setUnits(u);
                if (sel) run(sel, u, eff);
              }}
            />
          </label>
          <label style={{ fontSize: 12 }}>
            Efficiency %{" "}
            <input
              type="number"
              value={eff}
              style={{ width: 70 }}
              onChange={(e) => {
                const v = Number(e.target.value) || 0;
                setEff(v);
                if (sel) run(sel, units, v);
              }}
            />
          </label>
        </div>
      )}
      {missing && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
          No reprocessing data for this item — needs the full prebuilt SDE.
        </p>
      )}
      {data && (
        <>
          <table className="holdings" style={{ marginTop: 10 }}>
            <tbody>
              {data.yields.map((y) => (
                <tr key={y.type_id}>
                  <td>{y.name}</td>
                  <td className="mono num">{y.quantity.toLocaleString()}</td>
                  <td className="mono num pos">{ISK.format(y.value)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="cashflow-totals" style={{ marginTop: 8 }}>
            <span className="pos">Refined {ISK.format(data.refined_value)}</span>
            <span>Sell {ISK.format(data.sell_value)}</span>
            <span className={data.advantage >= 0 ? "pos" : "neg"}>
              {data.advantage >= 0 ? "Refine" : "Sell"} +{ISK.format(Math.abs(data.advantage))}
            </span>
          </div>
        </>
      )}
    </div>
  );
}

function BuildPlanner(): ReactNode {
  const [sel, setSel] = useState<ItemHit | null>(null);
  const [runs, setRuns] = useState(1);
  const [me, setMe] = useState(10);
  const [activity, setActivity] = useState("manufacturing");
  const [data, setData] = useState<BuildPlanView | null>(null);
  const [missing, setMissing] = useState(false);

  function run(item: ItemHit, r: number, m: number, act: string) {
    setData(null);
    setMissing(false);
    api
      .planBuild(item.type_id, r, m, act)
      .then((p) => (p ? setData(p) : setMissing(true)))
      .catch(() => setMissing(true));
  }

  return (
    <div className="card build-planner">
      <h3>Build Planner</h3>
      <ItemPicker
        placeholder="Search an item to build…"
        onPick={(h) => {
          setSel(h);
          run(h, runs, me, activity);
        }}
      />
      {sel && (
        <div className="cashflow-totals" style={{ marginTop: 8, gap: 12 }}>
          <label style={{ fontSize: 12 }}>
            Runs{" "}
            <input
              type="number"
              value={runs}
              style={{ width: 70 }}
              onChange={(e) => {
                const r = Number(e.target.value) || 1;
                setRuns(r);
                if (sel) run(sel, r, me, activity);
              }}
            />
          </label>
          <label style={{ fontSize: 12 }}>
            ME{" "}
            <input
              type="number"
              value={me}
              style={{ width: 60 }}
              onChange={(e) => {
                const m = Number(e.target.value) || 0;
                setMe(m);
                if (sel) run(sel, runs, m, activity);
              }}
            />
          </label>
          <label style={{ fontSize: 12 }}>
            Activity{" "}
            <select
              value={activity}
              onChange={(e) => {
                setActivity(e.target.value);
                if (sel) run(sel, runs, me, e.target.value);
              }}
            >
              <option value="manufacturing">Manufacturing</option>
              <option value="reaction">Reaction</option>
              <option value="invention">Invention</option>
            </select>
          </label>
        </div>
      )}
      {missing && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
          No blueprint data for this item — needs the full prebuilt SDE.
        </p>
      )}
      {data && (
        <>
          <table className="holdings" style={{ marginTop: 10 }}>
            <tbody>
              {data.materials.map((m) => (
                <tr key={m.type_id}>
                  <td>{m.name}</td>
                  <td className="mono num">{m.quantity.toLocaleString()}</td>
                  <td className="mono num neg">{ISK.format(m.value)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="cashflow-totals" style={{ marginTop: 8 }}>
            <span className="neg">Cost {ISK.format(data.material_cost)}</span>
            <span>Sells {ISK.format(data.product_value)}</span>
            <span className={data.profit >= 0 ? "pos" : "neg"}>
              {data.profit >= 0 ? "Profit" : "Loss"} {ISK.format(data.profit)} (
              {(data.margin_pct * 100).toFixed(1)}%)
            </span>
            {data.probability != null && (
              <span>Invention odds {(data.probability * 100).toFixed(0)}%</span>
            )}
          </div>
        </>
      )}
    </div>
  );
}

function EconomyHub({ character }: { character: Character | null }): ReactNode {
  const [jobs, setJobs] = useState<IndustryJobView[]>([]);
  const [market, setMarket] = useState<MarketView | null>(null);
  const [contracts, setContracts] = useState<Contract[]>([]);
  const [loadedAt, setLoadedAt] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [, forceTick] = useState(0);

  useEffect(() => {
    setJobs([]);
    setMarket(null);
    setContracts([]);
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
    api
      .getContracts(character.id, 15)
      .then(setContracts)
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
      <MarketBrowser />
      <StationScanner />
      <ReprocessCalc />
      <BuildPlanner />
      {error && <div className="card" style={{ marginTop: 16 }}><p style={{ color: "var(--text-dim)" }}>{error}</p></div>}
      <div className="card" style={{ marginTop: 16 }}>
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
      {contracts.length > 0 && (
        <div className="card" style={{ marginTop: 16 }}>
          <h3>Contracts <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {contracts.filter((c) => c.status === "outstanding" || c.status === "in_progress").length} active</span></h3>
          <table className="holdings">
            <tbody>
              {contracts.map((c) => {
                const amount = c.reward > 0 ? c.reward : c.price;
                return (
                  <tr key={c.contract_id}>
                    <td>{c.title || prettyRefType(c.type)}</td>
                    <td className="loc">{c.status.replace(/_/g, " ")}</td>
                    <td className={`mono num ${c.reward > 0 ? "pos" : ""}`}>{amount > 0 ? ISK.format(amount) : "—"}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}

// Character groups ("stables"/"hats") — create sets of characters for
// cross-character views. The model/DB shipped in Phase 0; this is its UI.
function CorpHub(): ReactNode {
  const [groups, setGroups] = useState<CharacterGroup[]>([]);
  const [roster, setRoster] = useState<Character[]>([]);
  const [newName, setNewName] = useState("");

  function reload() {
    api.listGroups().then(setGroups).catch(() => undefined);
  }
  useEffect(() => {
    if (!isTauri()) return;
    reload();
    api.listCharacters().then(setRoster).catch(() => undefined);
  }, []);

  function create() {
    const name = newName.trim();
    if (!name) return;
    api.createGroup(name).then(() => {
      setNewName("");
      reload();
    }).catch(() => undefined);
  }
  function toggleMember(group: CharacterGroup, characterId: number) {
    const inGroup = group.members.includes(characterId);
    const op = inGroup
      ? api.removeGroupMember(group.id, characterId)
      : api.addGroupMember(group.id, characterId);
    op.then(reload).catch(() => undefined);
  }

  if (!isTauri()) {
    return (
      <>
        <h1>Corp &amp; Fleet</h1>
        <div className="sub">Design preview — groups load in the desktop shell.</div>
      </>
    );
  }

  return (
    <>
      <h1>Groups</h1>
      <div className="sub">Organize your characters into sets ("stables") for combined views.</div>
      <div className="card" style={{ maxWidth: 560 }}>
        <div className="group-new">
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && create()}
            placeholder="New group name (e.g. Indy Alts)"
          />
          <button className="primary" onClick={create}>Create</button>
        </div>
      </div>
      {groups.length === 0 ? (
        <div className="sub" style={{ marginTop: 14 }}>No groups yet.</div>
      ) : (
        groups.map((g) => (
          <div className="card" key={g.id} style={{ marginTop: 14, maxWidth: 560 }}>
            <div className="group-head">
              <h3 style={{ margin: 0 }}>{g.name} <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {g.members.length}</span></h3>
              <button onClick={() => api.deleteGroup(g.id).then(reload)}>Delete</button>
            </div>
            <ul className="group-members">
              {roster.map((c) => (
                <li key={c.id}>
                  <label>
                    <input
                      type="checkbox"
                      checked={g.members.includes(c.id)}
                      onChange={() => toggleMember(g, c.id)}
                    />
                    <img className="avatar" src={portraitUrl(c.id, 32)} alt="" width={20} height={20} />
                    {c.name}
                  </label>
                </li>
              ))}
            </ul>
          </div>
        ))
      )}
    </>
  );
}

function ToolsHub(): ReactNode {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    api.getSettings().then(setSettings).catch(() => undefined);
  }, []);

  function update(patch: Partial<AppSettings>) {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    api
      .setSettings(next.intensity, next.notify_min)
      .then(() => {
        setSaved(true);
        window.setTimeout(() => setSaved(false), 1500);
      })
      .catch(() => undefined);
  }

  return (
    <>
      <h1>Tools &amp; Settings</h1>
      <div className="sub">Data-freshness and notification preferences.</div>
      {!isTauri() ? (
        <div className="card"><p style={{ color: "var(--text-dim)" }}>Design preview — settings load in the desktop shell.</p></div>
      ) : !settings ? (
        <div className="card"><p style={{ color: "var(--text-dim)" }}>Loading…</p></div>
      ) : (
        <div className="card settings-card">
          <label className="setting-row">
            <div>
              <div className="setting-name">Data freshness</div>
              <div className="setting-help">How aggressively background polling refreshes. Never beats ESI cache timers.</div>
            </div>
            <select value={settings.intensity} onChange={(e) => update({ intensity: e.target.value })}>
              <option value="Light">Light</option>
              <option value="Balanced">Balanced</option>
              <option value="Aggressive">Aggressive</option>
            </select>
          </label>
          <label className="setting-row">
            <div>
              <div className="setting-name">Notify me at</div>
              <div className="setting-help">Minimum severity that raises an OS notification. Lower events still collect in Alerts.</div>
            </div>
            <select value={settings.notify_min} onChange={(e) => update({ notify_min: e.target.value })}>
              <option value="Info">Info &amp; up</option>
              <option value="Warning">Warning &amp; up</option>
              <option value="Critical">Critical only</option>
            </select>
          </label>
          <div className="setting-saved" style={{ opacity: saved ? 1 : 0 }}>Saved ✓</div>
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
      return <CorpHub />;
    case "tools":
      return <ToolsHub />;
    default:
      return <Placeholder title="Unknown" blurb="" />;
  }
}
