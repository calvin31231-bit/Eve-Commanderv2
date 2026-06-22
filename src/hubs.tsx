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
  ColonyView,
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
  ResolvedFit,
  SkillPlanView,
  CanFlyView,
  DoctrineView,
  DscanResult,
  ThreatScanView,
  PilotBackgroundView,
  IncursionView,
  GateCampView,
  SystemSafetyView,
  CombatLogView,
  RouteView,
  CourierView,
  RegionMapView,
  JumpFatigue,
  CorpStructureView,
  CorpMemberView,
  LpStoreView,
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

interface SubTab {
  id: string;
  label: string;
}

/// A horizontal tab strip rendered under a hub title; the side rail keeps the
/// major hubs, these switch the sections within one hub.
function SubTabs({
  tabs,
  active,
  onSelect,
}: {
  tabs: SubTab[];
  active: string;
  onSelect: (id: string) => void;
}): ReactNode {
  return (
    <div className="subtabs" role="tablist">
      {tabs.map((t) => (
        <button
          key={t.id}
          role="tab"
          aria-selected={active === t.id}
          className={`subtab${active === t.id ? " active" : ""}`}
          onClick={() => onSelect(t.id)}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}

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
  const [sub, setSub] = useState("overview");

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
      <SubTabs
        tabs={[
          { id: "overview", label: "Overview" },
          { id: "wallet", label: "Wallet" },
          { id: "assets", label: "Assets" },
          { id: "skills", label: "Skill Plan" },
          { id: "mail", label: "Mail" },
        ]}
        active={sub}
        onSelect={setSub}
      />
      {error && !sheet && <div className="card"><p style={{ color: "var(--text-dim)" }}>{error}</p></div>}
      {loading && <div className="card"><p style={{ color: "var(--text-dim)" }}>Loading…</p></div>}
      {sub === "overview" && sheet && (
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
      {sub === "wallet" && cashflow && cashflow.entry_count > 0 && (
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
      {sub === "wallet" && txns.length > 0 && (
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
      {sub === "assets" && holdings && holdings.groups.length > 0 && (
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
      {sub === "assets" && locations.length > 0 && (
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
      {sub === "assets" && mining && mining.total_units > 0 && (
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
      {sub === "skills" && <SkillPlanner character={character} />}
      {sub === "mail" && <MailCard character={character} />}
    </>
  );
}

function SkillPlanner({ character }: { character: Character }): ReactNode {
  const [targets, setTargets] = useState<
    { skill_type_id: number; name: string; target_level: number }[]
  >([]);
  const [plan, setPlan] = useState<SkillPlanView | null>(null);

  function recost(next: typeof targets) {
    setTargets(next);
    if (!isTauri() || next.length === 0) {
      setPlan(null);
      return;
    }
    api
      .costSkillPlan(
        character.id,
        next.map((t) => ({ skill_type_id: t.skill_type_id, target_level: t.target_level })),
      )
      .then(setPlan)
      .catch(() => setPlan(null));
  }

  return (
    <div className="card skill-planner">
      <h3>Skill Plan</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Add skills and target levels to estimate SP and training time at your attributes.
      </p>
      <ItemPicker
        placeholder="Search a skill…"
        onPick={(h) => {
          if (targets.some((t) => t.skill_type_id === h.type_id)) return;
          recost([...targets, { skill_type_id: h.type_id, name: h.name, target_level: 5 }]);
        }}
      />
      {targets.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <tbody>
            {targets.map((t, i) => {
              const step = plan?.steps[i];
              return (
                <tr key={t.skill_type_id}>
                  <td>{step?.name ?? t.name}</td>
                  <td>
                    <select
                      value={t.target_level}
                      onChange={(e) => {
                        const next = [...targets];
                        next[i] = { ...t, target_level: Number(e.target.value) };
                        recost(next);
                      }}
                    >
                      {[1, 2, 3, 4, 5].map((l) => (
                        <option key={l} value={l}>
                          {step ? `${step.current_level}→${l}` : `L${l}`}
                        </option>
                      ))}
                    </select>
                  </td>
                  <td className="mono num">
                    {step?.known ? formatDuration(step.seconds) : step ? "no SDE" : "…"}
                  </td>
                  <td>
                    <button
                      onClick={() => recost(targets.filter((_, j) => j !== i))}
                      style={{ padding: "1px 8px" }}
                    >
                      ✕
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
      {plan && plan.total_seconds > 0 && (
        <div className="cashflow-totals" style={{ marginTop: 8 }}>
          <span>{(plan.total_sp / 1000).toFixed(0)}k SP</span>
          <span className="pos">{formatDuration(plan.total_seconds)} total</span>
        </div>
      )}
    </div>
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

function PlanetsCard({ character }: { character: Character | null }): ReactNode {
  const [colonies, setColonies] = useState<ColonyView[] | null>(null);
  const [loadedAt, setLoadedAt] = useState(0);
  const [, forceTick] = useState(0);

  useEffect(() => {
    setColonies(null);
    if (!character || !isTauri()) return;
    setLoadedAt(Date.now());
    api.getPlanets(character.id).then(setColonies).catch(() => setColonies([]));
  }, [character]);

  // Tick the extractor countdowns once a second (derived locally, no network).
  useEffect(() => {
    const t = window.setInterval(() => forceTick((n) => n + 1), 1000);
    return () => window.clearInterval(t);
  }, []);

  if (!character || !colonies || colonies.length === 0) return null;
  const elapsed = loadedAt ? Math.floor((Date.now() - loadedAt) / 1000) : 0;

  return (
    <div className="card planets-card" style={{ marginTop: 16 }}>
      <h3>
        Planetary Industry{" "}
        <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {colonies.length} colonies</span>
      </h3>
      <table className="holdings">
        <tbody>
          {colonies.map((c) => {
            const remaining = Math.max(0, c.seconds_remaining - elapsed);
            const expiring = c.soonest_expiry != null;
            return (
              <tr key={c.planet_id}>
                <td>
                  {c.planet_type} · {c.system_name}
                  <span style={{ color: "var(--text-dim)", fontSize: 11 }}>
                    {" "}
                    · L{c.upgrade_level} · {c.num_pins} pins
                  </span>
                  {c.products.length > 0 && (
                    <div style={{ color: "var(--text-dim)", fontSize: 11 }}>{c.products.join(", ")}</div>
                  )}
                </td>
                <td className="mono num">
                  {!expiring ? (
                    <span style={{ color: "var(--text-dim)" }}>idle</span>
                  ) : remaining === 0 ? (
                    <span className="badge caution">expired</span>
                  ) : (
                    formatDuration(remaining)
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function EconomyHub({ character }: { character: Character | null }): ReactNode {
  const [jobs, setJobs] = useState<IndustryJobView[]>([]);
  const [market, setMarket] = useState<MarketView | null>(null);
  const [contracts, setContracts] = useState<Contract[]>([]);
  const [loadedAt, setLoadedAt] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [sub, setSub] = useState("market");
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
      <div className="sub">Markets, industry, contracts, and planets.</div>
      <SubTabs
        tabs={[
          { id: "market", label: "Market" },
          { id: "industry", label: "Industry" },
          { id: "contracts", label: "Contracts" },
          { id: "planets", label: "Planets" },
        ]}
        active={sub}
        onSelect={setSub}
      />
      {error && <div className="card" style={{ marginTop: 16 }}><p style={{ color: "var(--text-dim)" }}>{error}</p></div>}
      {sub === "market" && (
        <>
          <MarketBrowser />
          <StationScanner />
        </>
      )}
      {sub === "industry" && (
        <>
          <ReprocessCalc />
          <BuildPlanner />
        </>
      )}
      {sub === "planets" && <PlanetsCard character={character} />}
      {sub === "industry" && (
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
      )}
      {sub === "market" && market && (
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
      {sub === "contracts" && contracts.length > 0 && (
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
      {sub === "contracts" && contracts.length === 0 && !error && (
        <div className="card" style={{ marginTop: 16 }}>
          <p style={{ color: "var(--text-dim)" }}>No contracts.</p>
        </div>
      )}
      {sub === "planets" && <PlanetsEmpty character={character} />}
    </>
  );
}

/// Fallback note for the Planets tab when the character runs no colonies (the
/// PlanetsCard renders nothing in that case).
function PlanetsEmpty({ character }: { character: Character }): ReactNode {
  const [colonies, setColonies] = useState<ColonyView[] | null>(null);
  useEffect(() => {
    if (!isTauri()) return;
    api.getPlanets(character.id).then(setColonies).catch(() => setColonies([]));
  }, [character.id]);
  if (colonies === null || colonies.length > 0) return null;
  return (
    <div className="card" style={{ marginTop: 16 }}>
      <p style={{ color: "var(--text-dim)" }}>No planetary colonies on this character.</p>
    </div>
  );
}

// Character groups ("stables"/"hats") — create sets of characters for
// cross-character views. The model/DB shipped in Phase 0; this is its UI.
function CorpHub({ character }: { character: Character | null }): ReactNode {
  const [groups, setGroups] = useState<CharacterGroup[]>([]);
  const [roster, setRoster] = useState<Character[]>([]);
  const [newName, setNewName] = useState("");
  const [sub, setSub] = useState("groups");

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
      <h1>Corp &amp; Fleet</h1>
      <div className="sub">Character groups, and corp structure fuel timers.</div>
      <SubTabs
        tabs={[
          { id: "groups", label: "Groups" },
          { id: "structures", label: "Structures" },
          { id: "members", label: "Members" },
        ]}
        active={sub}
        onSelect={setSub}
      />
      {sub === "groups" && (
        <>
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
      )}
      {sub === "structures" && <CorpStructures character={character} />}
      {sub === "members" && <CorpMembers character={character} />}
    </>
  );
}

function CorpMembers({ character }: { character: Character | null }): ReactNode {
  const [rows, setRows] = useState<CorpMemberView[] | null>(null);

  useEffect(() => {
    setRows(null);
    if (!character || !isTauri()) return;
    api.getCorpMembers(character.id).then(setRows).catch(() => setRows([]));
  }, [character]);

  if (!character) return <div className="sub">Select a character with a corp role.</div>;
  if (!rows) return <div className="sub">Loading…</div>;
  if (rows.length === 0) {
    return (
      <div className="card" style={{ maxWidth: 720 }}>
        <p style={{ color: "var(--text-dim)" }}>
          No member data — needs a Director role and the member-tracking scope.
        </p>
      </div>
    );
  }
  return (
    <div className="card" style={{ maxWidth: 720 }}>
      <h3>Members <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {rows.length}</span></h3>
      <table className="holdings">
        <tbody>
          {rows.slice(0, 100).map((m) => (
            <tr key={m.character_id}>
              <td>{m.name}</td>
              <td className="loc">{m.ship_name}{m.location_name ? ` · ${m.location_name}` : ""}</td>
              <td className="mono num">{m.logon_date ? shortDate(m.logon_date) : "never"}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function CorpStructures({ character }: { character: Character | null }): ReactNode {
  const [rows, setRows] = useState<CorpStructureView[] | null>(null);
  const [, forceTick] = useState(0);
  const [loadedAt, setLoadedAt] = useState(0);

  useEffect(() => {
    setRows(null);
    if (!character || !isTauri()) return;
    setLoadedAt(Date.now());
    api.getCorpStructures(character.id).then(setRows).catch(() => setRows([]));
  }, [character]);

  useEffect(() => {
    const t = window.setInterval(() => forceTick((n) => n + 1), 1000);
    return () => window.clearInterval(t);
  }, []);

  if (!character) return <div className="sub">Select a character with a corp role.</div>;
  if (!rows) return <div className="sub">Loading…</div>;
  if (rows.length === 0) {
    return (
      <div className="card" style={{ maxWidth: 640 }}>
        <p style={{ color: "var(--text-dim)" }}>
          No structures — needs a Director / Station Manager role and the structures scope.
        </p>
      </div>
    );
  }
  const elapsed = loadedAt ? Math.floor((Date.now() - loadedAt) / 1000) : 0;
  return (
    <div className="card" style={{ maxWidth: 640 }}>
      <h3>Structures <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {rows.length}</span></h3>
      <table className="holdings">
        <tbody>
          {rows.map((s) => {
            const remaining = Math.max(0, s.fuel_seconds_remaining - elapsed);
            const low = s.has_fuel_timer && remaining < 86400 * 2;
            return (
              <tr key={s.structure_id}>
                <td>
                  {s.name}
                  <div style={{ color: "var(--text-dim)", fontSize: 11 }}>
                    {s.type_name} · {s.system_name} · {s.state}
                  </div>
                </td>
                <td className={`mono num ${low ? "neg" : ""}`}>
                  {!s.has_fuel_timer ? "—" : remaining === 0 ? "OUT OF FUEL" : formatDuration(remaining)}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function ToolsHub(): ReactNode {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saved, setSaved] = useState(false);
  const [sub, setSub] = useState("settings");

  useEffect(() => {
    if (!isTauri()) return;
    api.getSettings().then(setSettings).catch(() => undefined);
  }, []);

  function update(patch: Partial<AppSettings>) {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    api
      .setSettings(next.intensity, next.notify_min, next.discord_webhook)
      .then(() => {
        setSaved(true);
        window.setTimeout(() => setSaved(false), 1500);
      })
      .catch(() => undefined);
  }

  return (
    <>
      <h1>Tools &amp; Settings</h1>
      <div className="sub">Preferences and calculators.</div>
      <SubTabs
        tabs={[
          { id: "settings", label: "Settings" },
          { id: "lp", label: "LP Optimizer" },
        ]}
        active={sub}
        onSelect={setSub}
      />
      {sub === "lp" && <LpOptimizer />}
      {sub === "settings" && (!isTauri() ? (
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
          <label className="setting-row">
            <div>
              <div className="setting-name">Discord webhook</div>
              <div className="setting-help">Mirror interrupting alerts to a Discord channel. Paste an incoming-webhook URL; leave blank to disable.</div>
            </div>
            <input
              type="text"
              value={settings.discord_webhook}
              placeholder="https://discord.com/api/webhooks/…"
              style={{ minWidth: 240 }}
              onChange={(e) => update({ discord_webhook: e.target.value })}
            />
          </label>
          <div className="setting-saved" style={{ opacity: saved ? 1 : 0 }}>Saved ✓</div>
        </div>
      ))}
    </>
  );
}

function LpOptimizer(): ReactNode {
  const [corp, setCorp] = useState("");
  const [store, setStore] = useState<LpStoreView | null>(null);
  const [loading, setLoading] = useState(false);

  function run() {
    if (!isTauri() || !corp.trim()) return;
    setLoading(true);
    setStore(null);
    api
      .lpStore(corp.trim())
      .then(setStore)
      .catch(() => setStore(null))
      .finally(() => setLoading(false));
  }

  return (
    <div className="card lp-optimizer">
      <h3>LP Store Optimizer</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Rank a corp's loyalty-point offers by ISK per LP (output value minus ISK + item cost).
      </p>
      <div className="market-search">
        <input
          value={corp}
          onChange={(e) => setCorp(e.target.value)}
          placeholder="Corporation (e.g. Federation Navy)…"
          onKeyDown={(e) => e.key === "Enter" && run()}
        />
      </div>
      <button onClick={run} disabled={loading || !corp.trim()} style={{ marginTop: 8 }}>
        {loading ? "Loading…" : "Rank offers"}
      </button>
      {store && !store.found && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>{store.message}</p>
      )}
      {store && store.found && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Offer</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>LP</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>ISK/LP</th>
            </tr>
          </thead>
          <tbody>
            {store.offers.slice(0, 40).map((o) => (
              <tr key={o.offer_id}>
                <td>{o.quantity > 1 ? `${o.quantity}× ` : ""}{o.name}</td>
                <td className="mono num">{ISK.format(o.lp_cost)}</td>
                <td className={`mono num ${o.isk_per_lp >= 0 ? "pos" : "neg"}`}>{ISK.format(o.isk_per_lp)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

function FitImporter({ character }: { character: Character | null }): ReactNode {
  const [eft, setEft] = useState("");
  const [fit, setFit] = useState<ResolvedFit | null>(null);
  const [canFly, setCanFly] = useState<CanFlyView | null>(null);
  const [doctrine, setDoctrine] = useState<DoctrineView | null>(null);
  const [error, setError] = useState<string | null>(null);

  function parse() {
    setError(null);
    setFit(null);
    setCanFly(null);
    setDoctrine(null);
    if (!isTauri()) {
      setError("Design preview — connect the desktop shell to parse fits.");
      return;
    }
    api
      .parseFit(eft)
      .then((f) => (f ? setFit(f) : setError("Not a valid EFT fit (check the [Ship, Name] header).")))
      .catch((e) => setError(String(e)));
  }

  function checkCanFly() {
    if (!character || !isTauri()) return;
    setCanFly(null);
    api.canFlyFit(character.id, eft).then(setCanFly).catch(() => setCanFly(null));
  }

  function checkDoctrine() {
    if (!isTauri()) return;
    setDoctrine(null);
    api.doctrineCheck(eft).then(setDoctrine).catch(() => setDoctrine(null));
  }

  return (
    <div className="card fit-importer">
      <h3>Fitting · EFT Import</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Paste an EFT block (from PYFA or the in-game fitting window) to parse and resolve it.
      </p>
      <textarea
        value={eft}
        onChange={(e) => setEft(e.target.value)}
        placeholder={"[Rifter, My Rifter]\nDamage Control II\n200mm AutoCannon II, EMP S\n\nHobgoblin II x5"}
        rows={8}
        style={{ width: "100%", fontFamily: "var(--mono, monospace)", fontSize: 12 }}
      />
      <div style={{ marginTop: 8, display: "flex", gap: 8 }}>
        <button onClick={parse}>Parse fit</button>
        {character && (
          <button onClick={checkCanFly} disabled={!eft.trim()}>
            Can {character.name} fly this?
          </button>
        )}
        <button onClick={checkDoctrine} disabled={!eft.trim()}>
          Check all pilots
        </button>
      </div>
      {error && <p style={{ color: "var(--text-dim)", fontSize: 12 }}>{error}</p>}
      {canFly && canFly.parsed && (
        <div style={{ marginTop: 10 }}>
          {canFly.can_fly ? (
            <p>
              <span className="badge safe">Can fly</span> {canFly.ship} — all required skills trained.
            </p>
          ) : (
            <>
              <p>
                <span className="badge caution">Missing skills</span> {canFly.ship} ·{" "}
                {formatDuration(canFly.total_seconds)} to train
              </p>
              <table className="holdings">
                <tbody>
                  {canFly.missing.map((m) => (
                    <tr key={m.skill_type_id}>
                      <td>{m.name}</td>
                      <td className="mono num">
                        {m.current_level}→{m.required_level}
                      </td>
                      <td className="mono num">
                        {m.seconds > 0 ? formatDuration(m.seconds) : "no SDE"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
          {canFly.unresolved.length > 0 && (
            <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
              {canFly.unresolved.length} item(s) unresolved — skill needs unchecked (needs full SDE).
            </p>
          )}
        </div>
      )}
      {doctrine && doctrine.parsed && (
        <div style={{ marginTop: 10 }}>
          <p>
            <span className={doctrine.can_fly_count > 0 ? "badge safe" : "badge caution"}>
              {doctrine.can_fly_count}/{doctrine.pilots.length} can fly
            </span>{" "}
            {doctrine.ship}
          </p>
          <table className="holdings">
            <tbody>
              {doctrine.pilots.map((p) => (
                <tr key={p.character_id}>
                  <td>{p.name}</td>
                  <td className="mono num">
                    {p.can_fly ? (
                      <span className="badge safe">ready</span>
                    ) : (
                      `${p.missing_count} skill${p.missing_count === 1 ? "" : "s"}`
                    )}
                  </td>
                  <td className="mono num">
                    {p.can_fly ? "" : p.total_seconds > 0 ? formatDuration(p.total_seconds) : "no SDE"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {fit && (
        <div style={{ marginTop: 10 }}>
          <div className="market-name">
            {fit.ship} · <span style={{ color: "var(--text-dim)" }}>{fit.name}</span>
          </div>
          <table className="holdings" style={{ marginTop: 8 }}>
            <tbody>
              {fit.items.map((it, i) => (
                <tr key={i}>
                  <td>
                    {it.name}
                    {it.charge && <span style={{ color: "var(--text-dim)" }}> · {it.charge}</span>}
                  </td>
                  <td className="mono num">{it.quantity > 1 ? `x${it.quantity}` : ""}</td>
                  <td className="mono num">
                    {it.type_id != null ? (
                      <span className="badge safe">#{it.type_id}</span>
                    ) : (
                      <span className="badge caution">?</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {fit.unresolved.length > 0 && (
            <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
              {fit.unresolved.length} name(s) unresolved — needs the full prebuilt SDE.
            </p>
          )}
        </div>
      )}
    </div>
  );
}

function CombatHub({ character }: { character: Character | null }): ReactNode {
  const [sub, setSub] = useState("fitting");
  return (
    <>
      <h1>Combat &amp; Intel</h1>
      <div className="sub">Fitting and D-scan; threat scanner + intel map land next.</div>
      <SubTabs
        tabs={[
          { id: "fitting", label: "Fitting" },
          { id: "dscan", label: "D-Scan" },
          { id: "map", label: "Intel Map" },
          { id: "threat", label: "Threat Scanner" },
          { id: "aar", label: "Combat Log" },
          { id: "pve", label: "PvE" },
        ]}
        active={sub}
        onSelect={setSub}
      />
      {sub === "fitting" && <FitImporter character={character} />}
      {sub === "dscan" && <DscanPanel />}
      {sub === "pve" && <IncursionsPanel />}
      {sub === "map" && (
        <>
          <IntelMap />
          <RegionMap />
        </>
      )}
      {sub === "threat" && (
        <>
          <ThreatScanner />
          <BackgroundCheck />
          <GateCampCheck />
        </>
      )}
      {sub === "aar" && <CombatLogPanel />}
    </>
  );
}

function characterAgeDays(birthday: string | null): number | null {
  if (!birthday) return null;
  const born = new Date(birthday).getTime();
  if (Number.isNaN(born)) return null;
  return Math.floor((Date.now() - born) / 86_400_000);
}

function BackgroundCheck(): ReactNode {
  const [name, setName] = useState("");
  const [result, setResult] = useState<PilotBackgroundView | null>(null);
  const [loading, setLoading] = useState(false);

  function check() {
    if (!isTauri() || !name.trim()) return;
    setLoading(true);
    setResult(null);
    api
      .pilotBackground(name.trim())
      .then(setResult)
      .catch(() => setResult(null))
      .finally(() => setLoading(false));
  }

  const ageDays = result ? characterAgeDays(result.birthday) : null;
  return (
    <div className="card background-check" style={{ marginTop: 16 }}>
      <h3>Background Check</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        One pilot — affiliation, age, sec status, and killboard threat.
      </p>
      <div className="market-search">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Pilot name…"
          onKeyDown={(e) => e.key === "Enter" && check()}
        />
      </div>
      <button onClick={check} disabled={loading || !name.trim()} style={{ marginTop: 8 }}>
        {loading ? "Checking…" : "Check"}
      </button>
      {result && !result.found && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>No character by that name.</p>
      )}
      {result && result.found && (
        <div style={{ marginTop: 10 }}>
          <div className="market-name">
            <span className={THREAT_BADGE[result.level] ?? "badge"}>{result.level}</span> {result.name}
          </div>
          <p style={{ color: "var(--text-dim)", fontSize: 12, margin: "4px 0" }}>
            {result.corporation}
            {result.alliance ? ` · ${result.alliance}` : ""}
            {" · "}sec {result.security_status.toFixed(1)}
            {ageDays != null ? ` · ${ageDays.toLocaleString()}d old` : ""}
          </p>
          <div className="cashflow-totals">
            <span className="pos">{result.ships_destroyed.toLocaleString()} kills</span>
            <span className="neg">{result.ships_lost.toLocaleString()} losses</span>
            <span>{result.danger_ratio}% danger</span>
          </div>
          {result.reasons.length > 0 && (
            <p style={{ color: "var(--text-dim)", fontSize: 12 }}>{result.reasons.join(" · ")}</p>
          )}
        </div>
      )}
    </div>
  );
}

function GateCampCheck(): ReactNode {
  const [system, setSystem] = useState("");
  const [result, setResult] = useState<GateCampView | null>(null);
  const [loading, setLoading] = useState(false);

  function check() {
    if (!isTauri() || !system.trim()) return;
    setLoading(true);
    setResult(null);
    api
      .gateCampCheck(system.trim())
      .then(setResult)
      .catch(() => setResult(null))
      .finally(() => setLoading(false));
  }

  return (
    <div className="card gatecamp-check" style={{ marginTop: 16 }}>
      <h3>Gate-Camp Check</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Recent kill volume in a system (zKillboard, last hour) as a camp signal.
      </p>
      <div className="market-search">
        <input
          value={system}
          onChange={(e) => setSystem(e.target.value)}
          placeholder="System name (e.g. Uedama)…"
          onKeyDown={(e) => e.key === "Enter" && check()}
        />
      </div>
      <button onClick={check} disabled={loading || !system.trim()} style={{ marginTop: 8 }}>
        {loading ? "Checking…" : "Check"}
      </button>
      {result && (
        <p style={{ marginTop: 10 }}>
          {!result.found ? (
            <span style={{ color: "var(--text-dim)" }}>System not found.</span>
          ) : (
            <>
              <span className={THREAT_BADGE[result.level] ?? "badge"}>{result.level}</span>{" "}
              {result.system} — {result.message}
            </>
          )}
        </p>
      )}
    </div>
  );
}

function killColor(kills: number): string {
  if (kills === 0) return "#4ade80";
  if (kills <= 5) return "#fbbf24";
  return "#f87171";
}

function IntelMap(): ReactNode {
  const [safety, setSafety] = useState<SystemSafetyView | null>(null);
  const [loading, setLoading] = useState(false);

  function load() {
    if (!isTauri()) return;
    setLoading(true);
    api
      .getSystemSafety()
      .then(setSafety)
      .catch(() => setSafety(null))
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const W = 380;
  const H = 320;
  const cx = W / 2;
  const cy = H / 2;
  const R = 118;
  const neighbors = safety?.neighbors.slice(0, 12) ?? [];

  return (
    <div className="card intel-map">
      <h3>Intel Map <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· your neighbourhood</span></h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Your current system and adjacent systems, sized/coloured by kills in the last hour.
      </p>
      <button onClick={load} disabled={loading}>{loading ? "Loading…" : "Refresh"}</button>
      {safety && (!safety.found || !safety.current) && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
          Needs an active character in space (location scope).
        </p>
      )}
      {safety && safety.found && safety.current && (
        <svg viewBox={`0 0 ${W} ${H}`} width="100%" style={{ marginTop: 8, maxWidth: W }}>
          {neighbors.map((n, i) => {
            const a = (i / Math.max(1, neighbors.length)) * Math.PI * 2 - Math.PI / 2;
            const x = cx + R * Math.cos(a);
            const y = cy + R * Math.sin(a);
            return <line key={`e${n.system_id}`} x1={cx} y1={cy} x2={x} y2={y} stroke="var(--border)" strokeWidth="1" />;
          })}
          {neighbors.map((n, i) => {
            const a = (i / Math.max(1, neighbors.length)) * Math.PI * 2 - Math.PI / 2;
            const x = cx + R * Math.cos(a);
            const y = cy + R * Math.sin(a);
            const r = 13 + Math.min(n.kills_last_hour, 10);
            return (
              <g key={n.system_id}>
                <circle cx={x} cy={y} r={r} fill={killColor(n.kills_last_hour)} fillOpacity={0.85} />
                <text x={x} y={y + 1} textAnchor="middle" fontSize="9" fill="#0b0f17" fontWeight="700">
                  {n.kills_last_hour > 0 ? n.kills_last_hour : ""}
                </text>
                <text x={x} y={y + r + 11} textAnchor="middle" fontSize="10" fill="var(--text)">
                  {n.name}
                </text>
              </g>
            );
          })}
          <circle cx={cx} cy={cy} r={20 + Math.min(safety.current.kills_last_hour, 12)} fill={killColor(safety.current.kills_last_hour)} stroke="#fff" strokeWidth="1.5" />
          <text x={cx} y={cy + 1} textAnchor="middle" fontSize="11" fill="#0b0f17" fontWeight="700">
            {safety.current.kills_last_hour > 0 ? safety.current.kills_last_hour : "0"}
          </text>
          <text x={cx} y={cy + 34} textAnchor="middle" fontSize="11" fill="var(--text)" fontWeight="600">
            {safety.current.name}
          </text>
        </svg>
      )}
    </div>
  );
}

function RegionMap(): ReactNode {
  const [regions, setRegions] = useState<string[]>([]);
  const [region, setRegion] = useState("");
  const [map, setMap] = useState<RegionMapView | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    api.listMapRegions().then(setRegions).catch(() => undefined);
    // Default: the active character's current region.
    load("");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function load(r: string) {
    if (!isTauri()) return;
    setLoading(true);
    api
      .getRegionMap(r || undefined)
      .then(setMap)
      .catch(() => setMap(null))
      .finally(() => setLoading(false));
  }

  // Project EVE x/z coordinates into the SVG viewport.
  const W = 640;
  const H = 440;
  const PAD = 24;
  const nodes = map?.nodes ?? [];
  const xs = nodes.map((n) => n.x);
  const zs = nodes.map((n) => n.z);
  const minX = Math.min(...xs, 0);
  const maxX = Math.max(...xs, 1);
  const minZ = Math.min(...zs, 0);
  const maxZ = Math.max(...zs, 1);
  const spanX = maxX - minX || 1;
  const spanZ = maxZ - minZ || 1;
  const px = (x: number) => PAD + ((x - minX) / spanX) * (W - 2 * PAD);
  // EVE z grows "south"; flip so north is up.
  const pz = (z: number) => PAD + (1 - (z - minZ) / spanZ) * (H - 2 * PAD);
  const pos = new Map(nodes.map((n) => [n.system_id, { x: px(n.x), y: pz(n.z) }]));

  return (
    <div className="card region-map" style={{ marginTop: 16 }}>
      <h3>
        Region Map{map?.found ? ` · ${map.region_name}` : ""}
      </h3>
      <div className="market-search" style={{ marginBottom: 6 }}>
        <select
          value={region}
          onChange={(e) => {
            setRegion(e.target.value);
            load(e.target.value);
          }}
        >
          <option value="">Current region</option>
          {regions.map((r) => (
            <option key={r} value={r}>{r}</option>
          ))}
        </select>
      </div>
      {loading && <p style={{ color: "var(--text-dim)", fontSize: 12 }}>Loading…</p>}
      {map && !map.found && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>{map.message}</p>
      )}
      {map && map.found && (
        <svg viewBox={`0 0 ${W} ${H}`} width="100%" style={{ background: "rgba(10,14,22,0.5)", borderRadius: 6 }}>
          {map.edges.map(([a, b], i) => {
            const pa = pos.get(a);
            const pb = pos.get(b);
            if (!pa || !pb) return null;
            return <line key={i} x1={pa.x} y1={pa.y} x2={pb.x} y2={pb.y} stroke="var(--border)" strokeWidth="1" />;
          })}
          {nodes.map((n) => {
            const p = pos.get(n.system_id)!;
            const hot = n.kills > 0;
            return (
              <g key={n.system_id}>
                <title>{`${n.name} · sec ${n.security.toFixed(1)}${hot ? ` · ${n.kills} kills/hr` : ""}`}</title>
                {hot && <circle cx={p.x} cy={p.y} r={6 + Math.min(n.kills, 12)} fill="#f87171" fillOpacity={0.25} />}
                <circle cx={p.x} cy={p.y} r={4} fill={secColor(n.security)} stroke={hot ? "#f87171" : "none"} strokeWidth="1.5" />
                {hot && (
                  <text x={p.x} y={p.y - 8} textAnchor="middle" fontSize="9" fill="#f87171" fontWeight="700">
                    {n.kills}
                  </text>
                )}
              </g>
            );
          })}
        </svg>
      )}
      <p style={{ color: "var(--text-dim)", fontSize: 11, marginBottom: 0 }}>
        Node colour = security; red halo = ship kills in the last hour (hover for details).
      </p>
    </div>
  );
}

function IncursionsPanel(): ReactNode {
  const [rows, setRows] = useState<IncursionView[] | null>(null);
  const [loading, setLoading] = useState(false);

  function load() {
    if (!isTauri()) return;
    setLoading(true);
    api
      .getIncursions()
      .then(setRows)
      .catch(() => setRows([]))
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="card incursions-panel">
      <h3>Incursions <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· live</span></h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Active incursions, freshest first. Lower influence = more farmed.
      </p>
      <button onClick={load} disabled={loading}>{loading ? "Loading…" : "Refresh"}</button>
      {rows && rows.length === 0 && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>No active incursions.</p>
      )}
      {rows && rows.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Staging</th>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>State</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Influence</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i}>
                <td>
                  {r.staging_system}
                  <div style={{ color: "var(--text-dim)", fontSize: 11 }}>
                    {r.faction} · {r.system_count} systems{r.has_boss ? " · boss up" : ""}
                  </div>
                </td>
                <td className="loc">{r.state}</td>
                <td className="mono num">{r.influence_pct.toFixed(0)}%</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

function CombatLogPanel(): ReactNode {
  const [data, setData] = useState<CombatLogView | null>(null);
  const [loading, setLoading] = useState(false);

  function load() {
    if (!isTauri()) return;
    setLoading(true);
    api
      .getCombatSummary()
      .then(setData)
      .catch(() => setData(null))
      .finally(() => setLoading(false));
  }

  const s = data?.summary;
  return (
    <div className="card combat-log">
      <h3>Combat Log · After-Action</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Reads your latest Gamelog (Documents/EVE/logs) for DPS and a kill/damage breakdown.
      </p>
      <button onClick={load} disabled={loading}>
        {loading ? "Reading…" : "Analyze latest log"}
      </button>
      {data && !data.found && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
          No Gamelog found — set EVE_COMMANDER_LOG_DIR if your logs aren't under Documents/EVE/logs.
        </p>
      )}
      {s && (
        <div style={{ marginTop: 10 }}>
          <div className="cashflow-totals">
            <span className="pos">{ISK.format(s.damage_dealt)} dealt · {s.dps_dealt.toFixed(0)} dps</span>
            <span className="neg">{ISK.format(s.damage_received)} taken · {s.dps_received.toFixed(0)} dps</span>
            <span style={{ color: "var(--text-dim)" }}>{formatDuration(s.duration_seconds)}</span>
          </div>
          {s.top_targets.length > 0 && (
            <>
              <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "8px 0 2px" }}>Top targets</p>
              <table className="holdings">
                <tbody>
                  {s.top_targets.map((t) => (
                    <tr key={t.entity}>
                      <td>{t.entity}</td>
                      <td className="mono num pos">{ISK.format(t.damage)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
          {s.top_attackers.length > 0 && (
            <>
              <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "8px 0 2px" }}>Top attackers</p>
              <table className="holdings">
                <tbody>
                  {s.top_attackers.map((t) => (
                    <tr key={t.entity}>
                      <td>{t.entity}</td>
                      <td className="mono num neg">{ISK.format(t.damage)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
        </div>
      )}
    </div>
  );
}

const THREAT_BADGE: Record<string, string> = {
  Danger: "badge danger",
  Caution: "badge caution",
  Neutral: "badge",
  Safe: "badge safe",
};

// A clipboard blob "looks like" a copied Local roster when it's several lines
// of plausible character names (letters/digits/space/'-.) and nothing else.
function looksLikeRoster(text: string): boolean {
  const lines = text.split(/[\n\r]+/).map((l) => l.trim()).filter(Boolean);
  if (lines.length < 2) return false;
  const nameLike = lines.filter((l) => l.length <= 37 && /^[A-Za-z0-9'\-. ]+$/.test(l));
  return nameLike.length >= 2 && nameLike.length >= lines.length - 1;
}

function ThreatScanner(): ReactNode {
  const [text, setText] = useState("");
  const [result, setResult] = useState<ThreatScanView | null>(null);
  const [loading, setLoading] = useState(false);
  const [watch, setWatch] = useState(false);

  function scan(input?: string) {
    if (!isTauri()) return;
    const names = (input ?? text)
      .split(/[\n\r]+/)
      .map((n) => n.trim())
      .filter(Boolean);
    if (names.length === 0) return;
    setLoading(true);
    setResult(null);
    api
      .scanPilots(names)
      .then(setResult)
      .catch(() => setResult(null))
      .finally(() => setLoading(false));
  }

  // Clipboard-watch: the EULA-safe "automatic" mode. While on, poll the OS
  // clipboard; when it changes to something that looks like a copied Local
  // roster, drop it into the box and rescan — no further interaction needed.
  useEffect(() => {
    if (!watch || !isTauri()) return;
    let last = "";
    const tick = () => {
      api
        .readClipboard()
        .then((clip) => {
          if (clip && clip !== last && looksLikeRoster(clip)) {
            last = clip;
            setText(clip);
            scan(clip);
          }
        })
        .catch(() => undefined);
    };
    const t = window.setInterval(tick, 2000);
    return () => window.clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [watch]);

  return (
    <div className="card threat-scanner">
      <h3>Local Threat Scanner</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        In space, select all in the Local member list and copy. Paste below, or enable Watch
        clipboard to auto-rescan every time you copy Local.
      </p>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={"One pilot name per line…"}
        rows={6}
        style={{ width: "100%", fontFamily: "var(--mono, monospace)", fontSize: 12 }}
      />
      <div style={{ marginTop: 8, display: "flex", gap: 8, alignItems: "center" }}>
        <button onClick={() => scan()} disabled={loading || !text.trim()}>
          {loading ? "Scanning…" : "Scan pilots"}
        </button>
        <label style={{ fontSize: 12, display: "flex", gap: 6, alignItems: "center" }}>
          <input type="checkbox" checked={watch} onChange={(e) => setWatch(e.target.checked)} />
          Watch clipboard
        </label>
      </div>
      {result && (
        <div style={{ marginTop: 10 }}>
          <p style={{ marginTop: 0 }}>{result.summary}</p>
          <table className="holdings">
            <tbody>
              {result.pilots.map((p) => (
                <tr key={p.name}>
                  <td>
                    <span className={THREAT_BADGE[p.level] ?? "badge"}>{p.level}</span> {p.name}
                    <div style={{ color: "var(--text-dim)", fontSize: 11 }}>
                      {p.reasons.join(" · ")}
                    </div>
                  </td>
                  <td className="mono num">{p.ships_destroyed > 0 ? `${p.ships_destroyed} kills` : ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {result.unresolved.length > 0 && (
            <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
              Unresolved: {result.unresolved.join(", ")}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

function DscanPanel(): ReactNode {
  const [text, setText] = useState("");
  const [result, setResult] = useState<DscanResult | null>(null);

  function scan() {
    if (!isTauri()) return;
    api.parseDscan(text).then(setResult).catch(() => setResult(null));
  }

  return (
    <div className="card dscan-panel">
      <h3>Directional Scan</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        In space, open the D-scan, "Copy to clipboard", and paste here for a grouped readout.
      </p>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Paste D-scan results…"
        rows={6}
        style={{ width: "100%", fontFamily: "var(--mono, monospace)", fontSize: 12 }}
      />
      <button onClick={scan} disabled={!text.trim()} style={{ marginTop: 8 }}>
        Scan
      </button>
      {result && (
        <div style={{ marginTop: 10 }}>
          {result.warnings.map((w, i) => (
            <p key={i} style={{ margin: "2px 0" }}>
              <span className="badge danger">!</span> {w}
            </p>
          ))}
          <p style={{ color: "var(--text-dim)", fontSize: 12 }}>{result.total} contacts</p>
          <table className="holdings">
            <tbody>
              {result.groups.map((g) => (
                <tr key={g.type_name}>
                  <td>{g.type_name}</td>
                  <td className="mono num">×{g.count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function NavigationHub({ character }: { character: Character | null }): ReactNode {
  const [origin, setOrigin] = useState("");
  const [dest, setDest] = useState("");
  const [flag, setFlag] = useState("shortest");
  const [route, setRoute] = useState<RouteView | null>(null);
  const [loading, setLoading] = useState(false);
  const [waypointMsg, setWaypointMsg] = useState<string | null>(null);

  function plan() {
    if (!isTauri() || !origin.trim() || !dest.trim()) return;
    setLoading(true);
    setRoute(null);
    setWaypointMsg(null);
    api
      .planRoute(origin.trim(), dest.trim(), flag)
      .then(setRoute)
      .catch(() => setRoute(null))
      .finally(() => setLoading(false));
  }

  function setWaypoint() {
    if (!character || !dest.trim()) return;
    setWaypointMsg("Setting…");
    api
      .setRouteWaypoint(character.id, dest.trim())
      .then(() => setWaypointMsg("Waypoint set in-game ✓"))
      .catch((e) => setWaypointMsg(`Failed: ${e}`));
  }

  return (
    <>
      <h1>Navigation &amp; Logistics</h1>
      <div className="sub">Route planner with safety, and in-game waypoint set.</div>
      <div className="card route-planner">
        <h3>Route Planner</h3>
        <div className="cashflow-totals" style={{ gap: 8, flexWrap: "wrap" }}>
          <input value={origin} onChange={(e) => setOrigin(e.target.value)} placeholder="Origin system" />
          <input value={dest} onChange={(e) => setDest(e.target.value)} placeholder="Destination system" />
          <select value={flag} onChange={(e) => setFlag(e.target.value)}>
            <option value="shortest">Shortest</option>
            <option value="secure">Prefer high-sec</option>
            <option value="insecure">Prefer low/null</option>
          </select>
        </div>
        <div style={{ marginTop: 8, display: "flex", gap: 8, alignItems: "center" }}>
          <button onClick={plan} disabled={loading || !origin.trim() || !dest.trim()}>
            {loading ? "Planning…" : "Plan route"}
          </button>
          {character && route?.found && (
            <button onClick={setWaypoint}>Set destination in-game</button>
          )}
          {waypointMsg && <span style={{ color: "var(--text-dim)", fontSize: 12 }}>{waypointMsg}</span>}
        </div>
        {route && !route.found && (
          <p style={{ color: "var(--text-dim)", fontSize: 12 }}>{route.message}</p>
        )}
        {route && route.found && (
          <div style={{ marginTop: 10 }}>
            <p style={{ marginTop: 0, color: "var(--text-dim)", fontSize: 12 }}>
              {route.jumps} jump{route.jumps === 1 ? "" : "s"}
            </p>
            <ol className="route-list">
              {route.hops.map((h) => (
                <li key={h.system_id}>
                  <span style={{ color: secColor(h.security) }}>{h.security.toFixed(1)}</span> {h.name}
                </li>
              ))}
            </ol>
          </div>
        )}
        {!character && (
          <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
            Add a character to set the waypoint in your client.
          </p>
        )}
      </div>
      <CourierCalc />
      {character && <JumpFatigueCard character={character} />}
    </>
  );
}

function JumpFatigueCard({ character }: { character: Character }): ReactNode {
  const [fatigue, setFatigue] = useState<JumpFatigue | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [, forceTick] = useState(0);

  useEffect(() => {
    setFatigue(null);
    setLoaded(false);
    if (!isTauri()) return;
    api
      .getJumpFatigue(character.id)
      .then(setFatigue)
      .catch(() => undefined)
      .finally(() => setLoaded(true));
  }, [character.id]);

  useEffect(() => {
    const t = window.setInterval(() => forceTick((n) => n + 1), 1000);
    return () => window.clearInterval(t);
  }, []);

  if (!loaded) return null;
  const expire = fatigue?.jump_fatigue_expire_date
    ? new Date(fatigue.jump_fatigue_expire_date).getTime()
    : 0;
  const remaining = expire ? Math.max(0, Math.floor((expire - Date.now()) / 1000)) : 0;

  return (
    <div className="card jump-fatigue" style={{ marginTop: 16, maxWidth: 420 }}>
      <h3>Jump Fatigue</h3>
      {remaining > 0 ? (
        <>
          <p className="mono" style={{ fontSize: 22, margin: "4px 0" }}>{formatDuration(remaining)}</p>
          <p style={{ color: "var(--text-dim)", fontSize: 12 }}>until fatigue clears (blue timer)</p>
        </>
      ) : (
        <p style={{ color: "var(--text-dim)" }}>
          No active fatigue{fatigue?.last_jump_date ? ` · last jump ${shortDate(fatigue.last_jump_date)}` : ""}.
        </p>
      )}
    </div>
  );
}

function CourierCalc(): ReactNode {
  const [origin, setOrigin] = useState("");
  const [dest, setDest] = useState("");
  const [volume, setVolume] = useState(320000);
  const [collateral, setCollateral] = useState(0);
  const [reward, setReward] = useState(0);
  const [flag, setFlag] = useState("shortest");
  const [result, setResult] = useState<CourierView | null>(null);
  const [loading, setLoading] = useState(false);

  function run() {
    if (!isTauri() || !origin.trim() || !dest.trim()) return;
    setLoading(true);
    setResult(null);
    api
      .courierEstimate(origin.trim(), dest.trim(), volume, collateral, reward, flag)
      .then(setResult)
      .catch(() => setResult(null))
      .finally(() => setLoading(false));
  }

  const numField = (label: string, value: number, set: (n: number) => void) => (
    <label style={{ fontSize: 12 }}>
      {label}{" "}
      <input
        type="number"
        value={value}
        style={{ width: 130 }}
        onChange={(e) => set(Number(e.target.value) || 0)}
      />
    </label>
  );

  return (
    <div className="card courier-calc" style={{ marginTop: 16 }}>
      <h3>Courier / Hauling Estimate</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Reward vs collateral vs route risk (kills on route, low/null hops).
      </p>
      <div className="cashflow-totals" style={{ gap: 8, flexWrap: "wrap" }}>
        <input value={origin} onChange={(e) => setOrigin(e.target.value)} placeholder="Origin system" />
        <input value={dest} onChange={(e) => setDest(e.target.value)} placeholder="Destination system" />
        <select value={flag} onChange={(e) => setFlag(e.target.value)}>
          <option value="shortest">Shortest</option>
          <option value="secure">Prefer high-sec</option>
          <option value="insecure">Prefer low/null</option>
        </select>
      </div>
      <div className="cashflow-totals" style={{ gap: 12, flexWrap: "wrap", marginTop: 8 }}>
        {numField("Volume m³", volume, setVolume)}
        {numField("Collateral", collateral, setCollateral)}
        {numField("Reward", reward, setReward)}
      </div>
      <button onClick={run} disabled={loading || !origin.trim() || !dest.trim()} style={{ marginTop: 8 }}>
        {loading ? "Estimating…" : "Estimate"}
      </button>
      {result && !result.found && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>{result.message}</p>
      )}
      {result && result.found && (
        <div style={{ marginTop: 10 }}>
          <div className="cashflow-totals">
            <span>{result.jumps} jumps</span>
            <span className="pos">{ISK.format(result.reward_per_jump)}/jump</span>
            <span>{ISK.format(result.reward_per_m3)}/m³</span>
            <span className={result.collateral_ratio > 20 ? "neg" : ""}>
              {result.collateral_ratio.toFixed(1)}× collateral
            </span>
          </div>
          <p style={{ marginTop: 6 }}>
            <span className={result.lowsec_hops > 0 || result.kills_on_route > 0 ? "badge caution" : "badge safe"}>
              {result.lowsec_hops > 0 || result.kills_on_route > 0 ? "Risk" : "Clean"}
            </span>{" "}
            {result.verdict}
          </p>
        </div>
      )}
    </div>
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
      return <CombatHub character={activeCharacter} />;
    case "navigation":
      return <NavigationHub character={activeCharacter} />;
    case "corp":
      return <CorpHub character={activeCharacter} />;
    case "tools":
      return <ToolsHub />;
    default:
      return <Placeholder title="Unknown" blurb="" />;
  }
}
