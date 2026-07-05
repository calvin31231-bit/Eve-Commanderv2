// The top-level hubs. The 25+ feature modules from the ROADMAP collapse into a
// small set of intent-based hubs on the left rail (see ROADMAP "UI/UX
// Strategy"). Phase 0 ships the shell + Home; other hubs are placeholders that
// later phases fill in.

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { api, isTauri } from "./ipc";
import { t, useLang, setLang, getLang, LANGUAGES } from "./i18n";
import type {
  AccountOverview,
  PortfolioHistory,
  AppSettings,
  CalendarEvent,
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
  ResearchAgentView,
  ServerStatus,
  TradeOpportunity,
  HubBoardView,
  HubTradeView,
  ShoppingPlanView,
  TradingPnlView,
  ReprocessView,
  BuildPlanView,
  ResolvedFit,
  SkillPlanView,
  RemapView,
  RoiPlan,
  RoiResult,
  IncomeActivity,
  IncomeRanking,
  RealizedIncome,
  CanFlyView,
  FitGatekeeperView,
  FitStatsView,
  ImplantView,
  SavedLoadoutView,
  DoctrineView,
  DscanResult,
  DscanDiff,
  DoctrineComplianceView,
  GameFitView,
  ContactView,
  ThreatScanView,
  PilotBackgroundView,
  IncursionView,
  FwSystemView,
  GateCampView,
  SystemSafetyView,
  SystemRiskView,
  CombatLogView,
  FleetAarView,
  AbyssTrackerView,
  LootValueView,
  RollPlan,
  SignatureView,
  TimerView,
  SrpBoardView,
  RecruitBoardView,
  AiSettingsView,
  AiEndpointView,
  AiAgentView,
  WidgetSlot,
  UpdateStatus,
  TelemetryEventView,
  PluginView,
  EsiHealthView,
  MemoryNoteView,
  SavedPlanView,
  SavedFitView,
  RouteView,
  CourierView,
  RegionMapView,
  AgentFinderView,
  JumpFatigue,
  TheraConnection,
  CorpStructureView,
  CorpMemberView,
  ContainerTheftView,
  ExtractionView,
  FleetView,
  FleetWingView,
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

// Every place the command palette can jump to: each hub plus its sub-tabs
// (deep-linked via the "hub/sub" hash format). Kept as one static registry so
// the palette never drifts from the real tab lists by more than a code review.
export const PALETTE_TARGETS: { hub: string; sub?: string; label: string }[] = [
  ...HUBS.map((h) => ({ hub: h.id, label: h.label })),
  { hub: "character", sub: "wallet", label: "Character · Wallet" },
  { hub: "character", sub: "assets", label: "Character · Assets" },
  { hub: "character", sub: "skills", label: "Character · Skill Plan" },
  { hub: "character", sub: "implants", label: "Character · Implants" },
  { hub: "character", sub: "mail", label: "Character · Mail" },
  { hub: "character", sub: "contacts", label: "Character · Contacts & Standings" },
  { hub: "economy", sub: "market", label: "Economy · Market" },
  { hub: "economy", sub: "industry", label: "Economy · Industry" },
  { hub: "economy", sub: "contracts", label: "Economy · Contracts" },
  { hub: "economy", sub: "planets", label: "Economy · Planets" },
  { hub: "combat", sub: "fitting", label: "Combat · Fitting" },
  { hub: "combat", sub: "dscan", label: "Combat · D-Scan" },
  { hub: "combat", sub: "map", label: "Combat · Intel Map" },
  { hub: "combat", sub: "threat", label: "Combat · Threat Scanner" },
  { hub: "combat", sub: "aar", label: "Combat · Combat Log" },
  { hub: "combat", sub: "pve", label: "Combat · PvE / Agent Finder" },
  { hub: "combat", sub: "abyss", label: "Combat · Abyss Tracker" },
  { hub: "corp", sub: "groups", label: "Corp · Groups" },
  { hub: "corp", sub: "structures", label: "Corp · Structures" },
  { hub: "corp", sub: "moons", label: "Corp · Moon Extractions" },
  { hub: "corp", sub: "members", label: "Corp · Members & Vetting" },
  { hub: "corp", sub: "fleet", label: "Corp · Fleet" },
  { hub: "corp", sub: "srp", label: "Corp · SRP" },
  { hub: "corp", sub: "recruit", label: "Corp · Recruitment" },
  { hub: "corp", sub: "timers", label: "Corp · Timerboard" },
  { hub: "tools", sub: "settings", label: "Tools · Settings" },
  { hub: "tools", sub: "lp", label: "Tools · LP Optimizer" },
  { hub: "tools", sub: "income", label: "Tools · Income Optimizer" },
  { hub: "tools", sub: "appraisal", label: "Tools · Appraisal" },
  { hub: "tools", sub: "ai", label: "Tools · AI Assistant" },
  { hub: "tools", sub: "plugins", label: "Tools · Plugins" },
];

/// Navigate to a hub (and optionally a sub-tab) via the hash router.
export function navigateTo(hub: string, sub?: string): void {
  window.location.hash = sub ? `${hub}/${sub}` : hub;
}

// UI density (comfortable/compact), persisted locally and applied as a root
// data-attribute the stylesheet keys off. Applied on module load so there's no
// flash of the wrong density.
const DENSITY_KEY = "eve-commander-density";
export function getDensity(): "comfortable" | "compact" {
  try {
    return localStorage.getItem(DENSITY_KEY) === "compact" ? "compact" : "comfortable";
  } catch {
    return "comfortable";
  }
}
export function setDensity(d: "comfortable" | "compact"): void {
  try {
    localStorage.setItem(DENSITY_KEY, d);
  } catch {
    // best-effort persistence
  }
  document.documentElement.dataset.density = d;
}
setDensity(getDensity());

// Theme (dark default / light / colorblind-safe), persisted like density.
export type Theme = "dark" | "light" | "colorblind";
const THEME_KEY = "eve-commander-theme";
export function getTheme(): Theme {
  try {
    const v = localStorage.getItem(THEME_KEY);
    return v === "light" || v === "colorblind" ? v : "dark";
  } catch {
    return "dark";
  }
}
export function setTheme(t: Theme): void {
  try {
    localStorage.setItem(THEME_KEY, t);
  } catch {
    // best-effort persistence
  }
  if (t === "dark") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = t;
}
setTheme(getTheme());

/// The sub-tab part of the current hash ("corp/srp" → "srp").
function subFromHash(): string | null {
  const h = window.location.hash.replace(/^#/, "");
  const i = h.indexOf("/");
  return i >= 0 && h.length > i + 1 ? h.slice(i + 1) : null;
}

/// Sub-tab state that honors "hub/sub" hash deep links: initializes from the
/// hash and follows hash changes (the command palette's jump path), while plain
/// clicks keep working through the returned setter.
function useSubTab(defaultId: string): [string, (id: string) => void] {
  const [sub, setSub] = useState(subFromHash() ?? defaultId);
  useEffect(() => {
    const onHash = () => {
      const s = subFromHash();
      if (s) setSub(s);
    };
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);
  return [sub, setSub];
}

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
  loginBusy?: boolean;
  loginError?: string | null;
}

// Visible feedback for the SSO flow, shown right under the login buttons so a
// failure (or the "approve in browser" wait) is never silent.
function LoginFeedback({ busy, error }: { busy?: boolean; error?: string | null }): ReactNode {
  if (busy) {
    return (
      <p style={{ fontSize: 12, color: "var(--text-dim)", marginTop: 8 }}>
        Opening your browser… approve the EVE login, then return here. (Waiting up to 2 minutes.)
      </p>
    );
  }
  if (error) {
    return (
      <p style={{ fontSize: 12, color: "var(--danger)", marginTop: 8, whiteSpace: "pre-wrap" }}>
        Login failed: {error}
      </p>
    );
  }
  return null;
}

// Entity hover-card: hover any item name → its Jita split, fetched lazily and
// memoized for the session. Advisory glance data, never blocking the row.
const hoverCache = new Map<string, { sell: number | null; buy: number | null } | null>();
function ItemHover({ name, children }: { name: string; children?: ReactNode }): ReactNode {
  const [card, setCard] = useState<{ sell: number | null; buy: number | null } | null>(null);
  const [show, setShow] = useState(false);

  function enter() {
    setShow(true);
    if (hoverCache.has(name)) {
      setCard(hoverCache.get(name) ?? null);
      return;
    }
    if (!isTauri()) return;
    api
      .searchItems(name, 1)
      .then((hits) => {
        const hit = hits[0];
        if (!hit || hit.name.toLowerCase() !== name.toLowerCase()) throw new Error("no match");
        return api.getMarketBrowse(hit.type_id);
      })
      .then((b) => {
        const v = { sell: b.quote.best_sell, buy: b.quote.best_buy };
        hoverCache.set(name, v);
        setCard(v);
      })
      .catch(() => hoverCache.set(name, null));
  }

  return (
    <span className="hovercard-anchor" onMouseEnter={enter} onMouseLeave={() => setShow(false)}>
      {children ?? name}
      {show && card && (card.sell != null || card.buy != null) && (
        <span className="hovercard mono">
          {card.sell != null && <>sell {ISK.format(card.sell)}</>}
          {card.sell != null && card.buy != null && " · "}
          {card.buy != null && <>buy {ISK.format(card.buy)}</>}
        </span>
      )}
    </span>
  );
}

// First-run role presets: each names the hubs that matter most for a playstyle
// so the picker can point new users somewhere useful immediately.
const ROLE_PRESETS: { id: string; label: string; hubs: { hub: string; sub?: string; label: string }[] }[] = [
  { id: "explorer", label: "Explorer", hubs: [
    { hub: "navigation", label: "Navigation" },
    { hub: "combat", sub: "dscan", label: "D-Scan" },
    { hub: "combat", sub: "map", label: "Intel Map" },
  ] },
  { id: "industrialist", label: "Industrialist", hubs: [
    { hub: "economy", sub: "industry", label: "Industry" },
    { hub: "economy", sub: "market", label: "Market" },
    { hub: "tools", sub: "income", label: "Income Optimizer" },
  ] },
  { id: "trader", label: "Trader", hubs: [
    { hub: "economy", sub: "market", label: "Market" },
    { hub: "character", sub: "wallet", label: "Wallet" },
    { hub: "tools", sub: "appraisal", label: "Appraisal" },
  ] },
  { id: "pvper", label: "PvPer", hubs: [
    { hub: "combat", sub: "threat", label: "Threat Scanner" },
    { hub: "combat", sub: "fitting", label: "Fitting" },
    { hub: "combat", sub: "dscan", label: "D-Scan" },
  ] },
  { id: "director", label: "FC / Director", hubs: [
    { hub: "corp", sub: "fleet", label: "Fleet" },
    { hub: "corp", sub: "timers", label: "Timerboard" },
    { hub: "corp", sub: "srp", label: "SRP" },
  ] },
];
const ROLE_KEY = "eve-commander-role";

// The home dashboard's widgets, in their default order. The saved layout
// (order + visibility) is reconciled against this list on the backend, so
// adding a widget here makes it appear for everyone on next launch.
const HOME_WIDGETS: { id: string; label: string }[] = [
  { id: "networth", label: "Account net worth" },
  { id: "status", label: "Tranquility status" },
  { id: "characters", label: "Characters" },
];

function Home({ status, statusError, characters, onLogin, onSelectCharacter, loginBusy, loginError }: HomeProps): ReactNode {
  useLang();
  const [account, setAccount] = useState<AccountOverview | null>(null);
  const [history, setHistory] = useState<PortfolioHistory | null>(null);
  const [layout, setLayout] = useState<WidgetSlot[]>(
    HOME_WIDGETS.map((w) => ({ id: w.id, visible: true })),
  );
  const [editing, setEditing] = useState(false);
  const [role, setRole] = useState<string>(() => {
    try {
      return localStorage.getItem(ROLE_KEY) ?? "";
    } catch {
      return "";
    }
  });
  function pickRole(id: string) {
    try {
      localStorage.setItem(ROLE_KEY, id);
    } catch {
      // best-effort persistence
    }
    setRole(id);
  }

  useEffect(() => {
    if (!isTauri() || characters.length === 0) {
      setAccount(null);
      setHistory(null);
      return;
    }
    api.getAccountOverview().then(setAccount).catch(() => undefined);
    api.getPortfolioHistory(null, 90).then(setHistory).catch(() => undefined);
  }, [characters.length]);

  useEffect(() => {
    if (!isTauri()) return;
    api.getHomeLayout(HOME_WIDGETS.map((w) => w.id)).then(setLayout).catch(() => undefined);
  }, []);

  function persist(next: WidgetSlot[]) {
    setLayout(next);
    if (isTauri()) api.setHomeLayout(next).catch(() => undefined);
  }
  function toggle(id: string) {
    persist(layout.map((s) => (s.id === id ? { ...s, visible: !s.visible } : s)));
  }
  function move(idx: number, dir: -1 | 1) {
    const j = idx + dir;
    if (j < 0 || j >= layout.length) return;
    const next = [...layout];
    [next[idx], next[j]] = [next[j], next[idx]];
    persist(next);
  }
  const label = (id: string) => HOME_WIDGETS.find((w) => w.id === id)?.label ?? id;

  const networthWidget = account && account.characters.length > 0 && (
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
          {history && history.networth.length >= 2 ? (
            <div className="networth-trend">
              <div className="networth-trend-head">
                <span style={{ color: "var(--text-dim)", fontSize: 12 }}>Net worth · 90 days</span>
                <span className={history.networth_change >= 0 ? "pos" : "neg"}>
                  {history.networth_change >= 0 ? "+" : ""}{ISK.format(history.networth_change)} ISK
                  {" "}({history.networth_change_pct >= 0 ? "+" : ""}{history.networth_change_pct.toFixed(1)}%)
                </span>
              </div>
              <NetWorthChart points={history.networth} />
            </div>
          ) : (
            account && (
              <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 6 }}>
                Net-worth history builds as the app runs — your trend appears here within an hour.
              </p>
            )
          )}
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
      );

  const statusWidget = (
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
  );

  const charactersWidget = (
    <div className="card">
      <h3>Characters</h3>
      {characters.length === 0 ? (
        <>
          <p style={{ color: "var(--text-dim)" }}>No characters yet.</p>
          <button className="primary" onClick={onLogin} disabled={loginBusy}>
            {loginBusy ? "Connecting…" : "Log in with EVE"}
          </button>
          <LoginFeedback busy={loginBusy} error={loginError} />
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
          <button className="primary" style={{ marginTop: 10 }} onClick={onLogin} disabled={loginBusy}>
            {loginBusy ? "Connecting…" : "Add character"}
          </button>
          <LoginFeedback busy={loginBusy} error={loginError} />
        </>
      )}
    </div>
  );

  const widgetNode = (id: string): ReactNode => {
    if (id === "networth") return networthWidget || null;
    if (id === "status") return statusWidget;
    if (id === "characters") return charactersWidget;
    return null;
  };

  // The net-worth widget spans the full width; the rest sit in a grid.
  const visible = layout.filter((s) => s.visible);

  return (
    <>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
        <div>
          <h1>{t("home.welcome")}</h1>
          <div className="sub">{t("home.subtitle")}</div>
        </div>
        <button onClick={() => setEditing((e) => !e)} title="Show, hide, and reorder dashboard widgets">
          {editing ? t("home.done") : t("home.customize")}
        </button>
      </div>

      {!role ? (
        <div className="card" style={{ marginBottom: 12 }}>
          <h3 style={{ marginTop: 0 }}>What kind of capsuleer are you?</h3>
          <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
            Pick a playstyle to get quick links to the hubs that matter for it. Everything stays
            reachable — this only sets your shortcuts.
          </p>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            {ROLE_PRESETS.map((r) => (
              <button key={r.id} onClick={() => pickRole(r.id)}>{r.label}</button>
            ))}
            <button onClick={() => pickRole("everything")} style={{ color: "var(--text-dim)" }}>
              A bit of everything
            </button>
          </div>
        </div>
      ) : (
        (() => {
          const preset = ROLE_PRESETS.find((r) => r.id === role);
          return preset ? (
            <div style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 12, flexWrap: "wrap" }}>
              <span style={{ fontSize: 12, color: "var(--text-dim)" }}>{preset.label} shortcuts:</span>
              {preset.hubs.map((h) => (
                <button key={h.label} style={{ fontSize: 12 }} onClick={() => navigateTo(h.hub, h.sub)}>
                  {h.label}
                </button>
              ))}
              <button style={{ fontSize: 11, color: "var(--text-dim)" }} onClick={() => pickRole("")}>
                change
              </button>
            </div>
          ) : null;
        })()
      )}
      {editing && (
        <div className="card" style={{ marginBottom: 12 }}>
          <h3 style={{ marginTop: 0 }}>Dashboard layout</h3>
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {layout.map((s, i) => (
              <li key={s.id} style={{ display: "flex", alignItems: "center", gap: 8, padding: "4px 0" }}>
                <label style={{ flex: 1, display: "flex", gap: 6, alignItems: "center" }}>
                  <input type="checkbox" checked={s.visible} onChange={() => toggle(s.id)} />
                  {label(s.id)}
                </label>
                <button onClick={() => move(i, -1)} disabled={i === 0} title="Move up">↑</button>
                <button onClick={() => move(i, 1)} disabled={i === layout.length - 1} title="Move down">↓</button>
              </li>
            ))}
          </ul>
        </div>
      )}

      {visible.filter((s) => s.id === "networth").map((s) => (
        <div key={s.id}>{widgetNode(s.id)}</div>
      ))}
      <div className="card-grid">
        {visible
          .filter((s) => s.id !== "networth")
          .map((s) => (
            <div key={s.id} style={{ display: "contents" }}>{widgetNode(s.id)}</div>
          ))}
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

// Sovereignty ring colour by Activity Defense Multiplier: dim purple when no
// structure data, green at the 6.0 cap (well-defended), amber when weak.
function admColor(adm: number): string {
  if (adm <= 0) return "#a78bfa";
  if (adm >= 5) return "#4ade80";
  if (adm >= 3) return "#facc15";
  return "#fb923c";
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

// Contacts & standings editor: view/red/blue/remove contacts (ESI writes,
// user-initiated). Standings feed the threat scanner's friend-or-foe logic.
function ContactsPanel({ character }: { character: Character | null }): ReactNode {
  const [rows, setRows] = useState<ContactView[] | null>(null);
  const [name, setName] = useState("");
  const [standing, setStanding] = useState("-10");
  const [msg, setMsg] = useState("");

  function load() {
    if (!character || !isTauri()) return;
    api.getContacts(character.id).then(setRows).catch(() => setRows([]));
  }
  useEffect(() => {
    setRows(null);
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [character?.id]);

  function add() {
    if (!character || !name.trim()) return;
    setMsg("Adding…");
    api
      .addContact(character.id, name.trim(), Number(standing))
      .then(() => { setMsg(`Added ${name.trim()}.`); setName(""); load(); })
      .catch((e) => setMsg(String(e)));
  }
  function setStandingFor(id: number, s: number) {
    if (!character) return;
    api.setContactStanding(character.id, id, s).then(load).catch((e) => setMsg(String(e)));
  }
  function remove(id: number) {
    if (!character) return;
    api.deleteContact(character.id, id).then(load).catch((e) => setMsg(String(e)));
  }
  const standingColor = (s: number) =>
    s < 0 ? "var(--danger)" : s > 0 ? "var(--accent)" : "var(--text-dim)";

  if (!character) return <div className="sub">Select a character to manage contacts.</div>;
  return (
    <div className="card" style={{ maxWidth: 720 }}>
      <h3>Contacts {rows && <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {rows.length}</span>}</h3>
      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Pilot name…" style={{ flex: 1, minWidth: 160 }} />
        <select value={standing} onChange={(e) => setStanding(e.target.value)}>
          <option value="-10">−10 (red)</option>
          <option value="-5">−5</option>
          <option value="0">0 (neutral)</option>
          <option value="5">+5</option>
          <option value="10">+10 (blue)</option>
        </select>
        <button onClick={add} disabled={!name.trim()}>Add contact</button>
        {msg && <span style={{ fontSize: 12, color: "var(--text-dim)" }}>{msg}</span>}
      </div>
      {!rows ? (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>Loading…</p>
      ) : rows.length === 0 ? (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
          No contacts (or the contacts scope isn't granted yet — re-login to add it).
        </p>
      ) : (
        <table className="holdings" style={{ marginTop: 10 }}>
          <tbody>
            {rows.slice(0, 200).map((c) => (
              <tr key={c.contact_id}>
                <td>
                  {c.name}
                  <span style={{ fontSize: 11, color: "var(--text-dim)" }}> · {c.contact_type}</span>
                  {c.is_watched && <span className="badge safe" style={{ marginLeft: 6 }}>watched</span>}
                  {c.is_blocked && <span className="badge danger" style={{ marginLeft: 6 }}>blocked</span>}
                </td>
                <td className="mono num" style={{ color: standingColor(c.standing) }}>
                  {c.standing > 0 ? "+" : ""}{c.standing.toFixed(1)}
                </td>
                <td style={{ textAlign: "right", whiteSpace: "nowrap" }}>
                  <select
                    value=""
                    onChange={(e) => e.target.value && setStandingFor(c.contact_id, Number(e.target.value))}
                    style={{ fontSize: 11 }}
                    title="Change standing"
                  >
                    <option value="">Set…</option>
                    {[-10, -5, 0, 5, 10].map((s) => (
                      <option key={s} value={s}>{s > 0 ? `+${s}` : s}</option>
                    ))}
                  </select>
                  <button style={{ fontSize: 11, marginLeft: 4 }} onClick={() => remove(c.contact_id)}>
                    Remove
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
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
  const [calendar, setCalendar] = useState<CalendarEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [sub, setSub] = useSubTab("overview");

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
    setCalendar([]);
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
    api.getCalendar(character.id).then(setCalendar).catch(() => undefined);
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
          { id: "implants", label: "Implants" },
          { id: "mail", label: "Mail" },
          { id: "contacts", label: "Contacts" },
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
              {clones.last_jump_date && (() => {
                // Clone jump cooldown: 24h from the last jump (skills can lower
                // it — this shows the conservative base timer).
                const readyAt = new Date(clones.last_jump_date).getTime() + 24 * 3600 * 1000;
                const left = Math.floor((readyAt - Date.now()) / 1000);
                return (
                  <p style={{ fontSize: 12, margin: "2px 0 0" }}>
                    {left <= 0 ? (
                      <span className="badge safe">clone jump ready</span>
                    ) : (
                      <span style={{ color: "var(--caution)" }}>next clone jump in {formatDuration(left)}</span>
                    )}
                  </p>
                );
              })()}
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
      {sub === "overview" && calendar.length > 0 && (
        <div className="card" style={{ marginTop: 16 }}>
          <h3>Calendar <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {calendar.length} upcoming</span></h3>
          <table className="holdings">
            <tbody>
              {calendar.slice(0, 8).map((e) => {
                const secs = Math.floor((new Date(e.event_date).getTime() - Date.now()) / 1000);
                return (
                  <tr key={e.event_id}>
                    <td>{e.title}</td>
                    <td className="loc">{e.event_response.replace(/_/g, " ")}</td>
                    <td className="mono num">{secs > 0 ? formatDuration(secs) : shortDate(e.event_date)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
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
      {sub === "wallet" && character && <TradingPnlCard character={character} />}
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
      {sub === "skills" && (
        <>
          <SkillPlanner character={character} />
          <SkillRoiPlanner />
        </>
      )}
      {sub === "implants" && <ImplantFitter />}
      {sub === "mail" && <MailCard character={character} />}
      {sub === "contacts" && <ContactsPanel character={character} />}
    </>
  );
}

// RPG-style implant fitter: a 10-slot rack you equip implants into, with a
// browsable catalogue filtered by slot, boost category, and name search.
function ImplantFitter(): ReactNode {
  const [all, setAll] = useState<ImplantView[] | null>(null);
  const [rack, setRack] = useState<Record<number, ImplantView>>({});
  const [slotFilter, setSlotFilter] = useState("0"); // 0 = any
  const [catFilter, setCatFilter] = useState("Any");
  const [q, setQ] = useState("");
  const [loadouts, setLoadouts] = useState<SavedLoadoutView[] | null>(null);
  const [rackValue, setRackValue] = useState<number | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    api.listImplants().then(setAll).catch(() => setAll([]));
    api.listImplantLoadouts().then(setLoadouts).catch(() => setLoadouts([]));
  }, []);

  const equipped = useMemo(() => Object.values(rack), [rack]);

  // Price the rack whenever it changes.
  useEffect(() => {
    if (!isTauri() || equipped.length === 0) {
      setRackValue(null);
      return;
    }
    api.valueImplants(equipped.map((i) => i.type_id)).then((v) => setRackValue(v.total)).catch(() => setRackValue(null));
  }, [equipped]);

  // Count equipped implants per boost category for the summary.
  const rackCategories = useMemo(() => {
    const counts: Record<string, number> = {};
    equipped.forEach((i) => { counts[i.category] = (counts[i.category] ?? 0) + 1; });
    return Object.entries(counts).sort((a, b) => b[1] - a[1]);
  }, [equipped]);

  function refreshLoadouts() {
    api.listImplantLoadouts().then(setLoadouts).catch(() => undefined);
  }

  function saveLoadout() {
    const ids = Object.values(rack).map((i) => i.type_id);
    if (ids.length === 0) return;
    const name = window.prompt("Save implant loadout as:");
    if (!name) return;
    api.saveImplantLoadout(name, ids).then(refreshLoadouts).catch(() => undefined);
  }

  function loadLoadout(l: SavedLoadoutView) {
    if (!all) return;
    const byId = new Map(all.map((i) => [i.type_id, i]));
    const next: Record<number, ImplantView> = {};
    for (const id of l.implant_ids) {
      const imp = byId.get(id);
      if (imp) next[imp.slot] = imp;
    }
    setRack(next);
  }

  const categories = useMemo(() => {
    const set = new Set<string>();
    (all ?? []).forEach((i) => set.add(i.category));
    return ["Any", ...Array.from(set).sort()];
  }, [all]);

  const filtered = useMemo(() => {
    const slot = Number(slotFilter);
    const needle = q.trim().toLowerCase();
    return (all ?? [])
      .filter((i) => (slot === 0 || i.slot === slot))
      .filter((i) => (catFilter === "Any" || i.category === catFilter))
      .filter((i) => (needle === "" || i.name.toLowerCase().includes(needle)))
      .slice(0, 200);
  }, [all, slotFilter, catFilter, q]);

  function equip(i: ImplantView) {
    setRack((r) => ({ ...r, [i.slot]: i }));
  }
  function unequip(slot: number) {
    setRack((r) => {
      const next = { ...r };
      delete next[slot];
      return next;
    });
  }

  if (!isTauri()) {
    return <div className="card"><p style={{ color: "var(--text-dim)" }}>The implant fitter runs in the desktop shell.</p></div>;
  }
  if (all && all.length === 0) {
    return (
      <div className="card">
        <h3>Implant Fitter</h3>
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
          No implants in the SDE yet — rebuild sde.sqlite with the latest converter (implant slot attribute).
        </p>
      </div>
    );
  }

  return (
    <>
      <div className="card">
        <h3>
          Implant Rack <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· slots 1–10</span>
          <button
            style={{ float: "right", fontSize: 11 }}
            onClick={saveLoadout}
            disabled={Object.keys(rack).length === 0}
          >
            Save loadout
          </button>
          {Object.keys(rack).length > 0 && (
            <button style={{ float: "right", fontSize: 11, marginRight: 6 }} onClick={() => setRack({})}>
              Clear
            </button>
          )}
        </h3>
        <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
          Build a clone loadout: pick an implant from the catalogue to slot it. One implant per slot.
        </p>
        {equipped.length > 0 && (
          <div className="cashflow-totals" style={{ marginBottom: 8 }}>
            <span>{equipped.length}/10 slots</span>
            {rackValue !== null && <span className="neg">{ISK.format(rackValue)} at risk</span>}
            {rackCategories.map(([cat, n]) => (
              <span key={cat} style={{ color: "var(--text-dim)" }}>{cat} ×{n}</span>
            ))}
          </div>
        )}
        {loadouts && loadouts.length > 0 && (
          <div style={{ marginBottom: 8 }}>
            <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "0 0 4px" }}>Saved loadouts</p>
            {loadouts.map((l) => (
              <div key={l.id} style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 3 }}>
                <button style={{ fontSize: 11 }} onClick={() => loadLoadout(l)}>Load</button>
                <span style={{ fontSize: 13 }}>{l.name}</span>
                <span style={{ fontSize: 11, color: "var(--text-dim)" }}>· {l.implant_ids.length} implant(s)</span>
                <button
                  style={{ fontSize: 11, marginLeft: "auto" }}
                  onClick={() => api.deleteImplantLoadout(l.id).then(refreshLoadouts)}
                >
                  Delete
                </button>
              </div>
            ))}
          </div>
        )}
        <table className="holdings">
          <tbody>
            {Array.from({ length: 10 }, (_, k) => k + 1).map((slot) => {
              const eq = rack[slot];
              return (
                <tr key={slot}>
                  <td style={{ width: 56, color: "var(--text-dim)" }}>Slot {slot}</td>
                  <td>{eq ? eq.name : <span style={{ color: "var(--text-dim)" }}>— empty —</span>}</td>
                  <td style={{ fontSize: 11, color: "var(--text-dim)" }}>{eq?.category ?? ""}</td>
                  <td style={{ textAlign: "right" }}>
                    {eq && <button style={{ fontSize: 11 }} onClick={() => unequip(slot)}>Remove</button>}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <div className="card">
        <h3>Catalogue</h3>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "end" }}>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Slot
            <select value={slotFilter} onChange={(e) => setSlotFilter(e.target.value)}>
              <option value="0">Any</option>
              {Array.from({ length: 10 }, (_, k) => k + 1).map((s) => (
                <option key={s} value={String(s)}>{s}</option>
              ))}
            </select>
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Boost type
            <select value={catFilter} onChange={(e) => setCatFilter(e.target.value)}>
              {categories.map((c) => <option key={c} value={c}>{c}</option>)}
            </select>
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)", flex: 1 }}>
            Search
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="implant name…" style={{ width: "100%" }} />
          </label>
        </div>
        {!all && <p style={{ color: "var(--text-dim)", fontSize: 12 }}>Loading…</p>}
        {all && (
          <table className="holdings" style={{ marginTop: 8 }}>
            <tbody>
              {filtered.map((i) => (
                <tr key={i.type_id}>
                  <td style={{ width: 40, color: "var(--text-dim)" }}>{i.slot}</td>
                  <td>{i.name}</td>
                  <td style={{ fontSize: 11, color: "var(--text-dim)" }}>{i.category}</td>
                  <td style={{ textAlign: "right" }}>
                    <button style={{ fontSize: 11 }} onClick={() => equip(i)}>Slot it</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {all && filtered.length === 0 && (
          <p style={{ color: "var(--text-dim)", fontSize: 12 }}>No implants match.</p>
        )}
      </div>
    </>
  );
}

function SkillPlanner({ character }: { character: Character }): ReactNode {
  const [targets, setTargets] = useState<
    { skill_type_id: number; name: string; target_level: number }[]
  >([]);
  const [plan, setPlan] = useState<SkillPlanView | null>(null);
  const [remap, setRemap] = useState<RemapView | null>(null);
  const [showImport, setShowImport] = useState(false);
  const [importText, setImportText] = useState("");
  const [importMsg, setImportMsg] = useState("");
  const [saved, setSaved] = useState<SavedPlanView[] | null>(null);

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

  function applyText(text: string, clearInput: boolean) {
    if (!isTauri() || !text.trim()) return;
    api
      .importSkillPlan(text)
      .then((res) => {
        const merged = [...targets];
        for (const t of res.targets) {
          const existing = merged.find((m) => m.skill_type_id === t.skill_type_id);
          if (existing) existing.target_level = Math.max(existing.target_level, t.target_level);
          else merged.push({ skill_type_id: t.skill_type_id, name: t.name, target_level: t.target_level });
        }
        recost(merged);
        setImportMsg(
          `Loaded ${res.targets.length} skill(s)` +
            (res.unresolved.length ? `, ${res.unresolved.length} unresolved: ${res.unresolved.slice(0, 5).join(", ")}` : ""),
        );
        if (clearInput) setImportText("");
      })
      .catch((e) => setImportMsg(String(e)));
  }

  function loadLibrary() {
    if (!isTauri()) return;
    api.listSkillPlans().then(setSaved).catch(() => setSaved([]));
  }

  function savePlan() {
    if (!isTauri() || targets.length === 0) return;
    const name = window.prompt("Save plan as:");
    if (!name) return;
    const body = targets.map((t) => `${t.name} ${t.target_level}`).join("\n");
    api.saveSkillPlan(name, body).then(() => { setImportMsg(`Saved "${name}".`); if (saved) loadLibrary(); }).catch((e) => setImportMsg(String(e)));
  }

  return (
    <div className="card skill-planner">
      <h3>
        Skill Plan
        <button style={{ float: "right", fontSize: 11 }} onClick={() => setShowImport((v) => !v)}>
          Import
        </button>
        <button
          style={{ float: "right", fontSize: 11, marginRight: 6 }}
          onClick={() => { setShowImport(true); loadLibrary(); }}
        >
          Library
        </button>
        <button
          style={{ float: "right", fontSize: 11, marginRight: 6 }}
          onClick={savePlan}
          disabled={targets.length === 0}
        >
          Save
        </button>
      </h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Add skills and target levels to estimate SP and training time at your attributes. Save a plan to
        reuse it on a new character.
      </p>
      {showImport && (
        <div style={{ marginBottom: 10 }}>
          {saved && (
            <div style={{ marginBottom: 8 }}>
              {saved.length === 0 && <span style={{ fontSize: 12, color: "var(--text-dim)" }}>No saved plans yet.</span>}
              {saved.map((p) => (
                <div key={p.id} style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 3 }}>
                  <button style={{ fontSize: 11 }} onClick={() => applyText(p.body, false)}>Load</button>
                  <span style={{ fontSize: 13 }}>{p.name}</span>
                  <button
                    style={{ fontSize: 11, marginLeft: "auto" }}
                    onClick={() =>
                      api.shareArtifact("plan", p.name, p.body).then((codeStr) => {
                        navigator.clipboard?.writeText(codeStr);
                        setImportMsg(`Share code for "${p.name}" copied to clipboard.`);
                      }).catch((e) => setImportMsg(String(e)))
                    }
                  >
                    Share
                  </button>
                  <button
                    style={{ fontSize: 11 }}
                    onClick={() => api.deleteSkillPlan(p.id).then(loadLibrary)}
                  >
                    Delete
                  </button>
                </div>
              ))}
            </div>
          )}
          <textarea
            value={importText}
            onChange={(e) => setImportText(e.target.value)}
            placeholder={"Paste an EVEMon plan export or a list like:\nCaldari Cruiser V\nShield Management IV"}
            style={{ width: "100%", minHeight: 80, fontFamily: "monospace", fontSize: 12 }}
          />
          <div style={{ display: "flex", gap: 8, marginTop: 4, alignItems: "center" }}>
            <button onClick={() => applyText(importText, true)}>Import plan</button>
            {importMsg && <span style={{ fontSize: 12, color: "var(--text-dim)" }}>{importMsg}</span>}
          </div>
        </div>
      )}
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
        <>
          <div className="cashflow-totals" style={{ marginTop: 8 }}>
            <span>{(plan.total_sp / 1000).toFixed(0)}k SP</span>
            <span className="pos">{formatDuration(plan.total_seconds)} total</span>
            <button
              style={{ fontSize: 11 }}
              onClick={() =>
                api
                  .optimizeRemap(
                    character.id,
                    targets.map((t) => ({ skill_type_id: t.skill_type_id, target_level: t.target_level })),
                  )
                  .then(setRemap)
                  .catch(() => setRemap(null))
              }
            >
              Suggest remap
            </button>
          </div>
          {remap && (
            <div style={{ marginTop: 8, fontSize: 12 }}>
              <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
                <span>Int {remap.intelligence}</span>
                <span>Mem {remap.memory}</span>
                <span>Per {remap.perception}</span>
                <span>Wil {remap.willpower}</span>
                <span>Cha {remap.charisma}</span>
              </div>
              <div style={{ color: "var(--accent)", marginTop: 4 }}>
                {remap.saved_seconds > 0
                  ? `Saves ${formatDuration(remap.saved_seconds)} vs a balanced map.`
                  : "Already near-optimal for this plan."}
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

// Ranks candidate skill plans by ISK return on training time. The player
// describes each plan in ISK terms (income unlocked, days to train); the
// backend does the deterministic ranking.
function SkillRoiPlanner(): ReactNode {
  const [plans, setPlans] = useState<RoiPlan[]>([]);
  const [results, setResults] = useState<RoiResult[] | null>(null);
  const [label, setLabel] = useState("");
  const [trainDays, setTrainDays] = useState("30");
  const [iskHr, setIskHr] = useState("50");
  const [hoursDay, setHoursDay] = useState("2");
  const [upfront, setUpfront] = useState("0");

  function add() {
    if (!label.trim()) return;
    const plan: RoiPlan = {
      label: label.trim(),
      train_seconds: Math.round((parseFloat(trainDays) || 0) * 86400),
      isk_per_hour: (parseFloat(iskHr) || 0) * 1_000_000,
      hours_per_day: parseFloat(hoursDay) || 0,
      upfront_isk: (parseFloat(upfront) || 0) * 1_000_000,
    };
    setPlans((p) => [...p, plan]);
    setResults(null);
    setLabel("");
  }

  function rank() {
    if (!isTauri() || plans.length === 0) return;
    api.rankSkillRoi(plans).then(setResults).catch(() => setResults([]));
  }

  return (
    <div className="card">
      <h3>
        Skill-Plan ROI{" "}
        <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· train by return</span>
      </h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Rank training plans by ISK unlocked per day of training — not just time. Enter the income
        each plan unlocks; values in millions of ISK.
      </p>
      <div style={{ display: "grid", gridTemplateColumns: "1.4fr repeat(4, 1fr) auto", gap: 6, alignItems: "end" }}>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Plan
          <input value={label} onChange={(e) => setLabel(e.target.value)} placeholder="e.g. Marauder" />
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Train days
          <input value={trainDays} onChange={(e) => setTrainDays(e.target.value)} />
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          ISK/hr (M)
          <input value={iskHr} onChange={(e) => setIskHr(e.target.value)} />
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Hrs/day
          <input value={hoursDay} onChange={(e) => setHoursDay(e.target.value)} />
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Upfront (M)
          <input value={upfront} onChange={(e) => setUpfront(e.target.value)} />
        </label>
        <button onClick={add}>Add</button>
      </div>
      {plans.length > 0 && (
        <div style={{ marginTop: 8, display: "flex", gap: 8, alignItems: "center" }}>
          <button onClick={rank}>Rank {plans.length} plan(s)</button>
          <button onClick={() => { setPlans([]); setResults(null); }}>Clear</button>
        </div>
      )}
      {results && results.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Plan</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Train</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>ISK/day</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Payback</th>
            </tr>
          </thead>
          <tbody>
            {results.map((r, i) => (
              <tr key={r.label + i}>
                <td>{i === 0 ? "★ " : ""}{r.label}</td>
                <td className="mono num">{r.train_days.toFixed(0)}d</td>
                <td className="mono num pos">{ISK.format(r.daily_gain)}</td>
                <td className="mono num">{r.payback_days < 1e6 ? `${r.payback_days.toFixed(0)}d` : "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
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
          <div className="market-name">
            {sel.name}
            {data.anomaly.flag !== "normal" && (
              <span
                className={`badge ${data.anomaly.flag === "cheap" ? "safe" : data.anomaly.flag === "spike" ? "caution" : "danger"}`}
                style={{ marginLeft: 8, fontWeight: 400 }}
                title={data.anomaly.note}
              >
                {data.anomaly.flag === "cheap" ? "below normal" : data.anomaly.flag === "spike" ? "price spike" : "anomaly"}
              </span>
            )}
          </div>
          {data.anomaly.flag !== "normal" && (
            <p style={{ fontSize: 12, margin: "2px 0 0", color: data.anomaly.flag === "cheap" ? "var(--safe)" : "var(--caution)" }}>
              {data.anomaly.note}
            </p>
          )}
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

function NetWorthChart({ points }: { points: { at: number; value: number }[] }): ReactNode {
  const w = 520;
  const h = 90;
  const vals = points.map((p) => p.value);
  const min = Math.min(...vals);
  const max = Math.max(...vals);
  const span = max - min || 1;
  const n = points.length;
  const x = (i: number) => (i / (n - 1)) * w;
  const y = (v: number) => h - ((v - min) / span) * (h - 8) - 4;
  const line = points.map((p, i) => `${x(i)},${y(p.value)}`).join(" ");
  const area = `0,${h} ${line} ${w},${h}`;
  const up = vals[n - 1] >= vals[0];
  const stroke = up ? "#4ade80" : "#f87171";
  return (
    <svg className="networth-chart" viewBox={`0 0 ${w} ${h}`} width="100%" height={h} preserveAspectRatio="none">
      <polygon points={area} fill={stroke} fillOpacity={0.12} />
      <polyline points={line} fill="none" stroke={stroke} strokeWidth="1.5" />
    </svg>
  );
}

// DPS-vs-range line chart (turret falloff applied). Range on x, DPS on y.
function DpsRangeChart({ curve }: { curve: { range_km: number; dps: number }[] }): ReactNode {
  const W = 260;
  const H = 70;
  const pad = 4;
  const maxDps = Math.max(...curve.map((p) => p.dps), 1);
  const maxRange = Math.max(...curve.map((p) => p.range_km), 1);
  const pts = curve
    .map((p) => {
      const x = pad + (p.range_km / maxRange) * (W - 2 * pad);
      const y = H - pad - (p.dps / maxDps) * (H - 2 * pad);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <div style={{ marginTop: 8 }}>
      <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "0 0 2px" }}>
        DPS vs range · peak {Math.round(maxDps)} dps · to {Math.round(maxRange)} km
      </p>
      <svg viewBox={`0 0 ${W} ${H}`} width="100%" style={{ background: "rgba(10,14,22,0.5)", borderRadius: 6 }}>
        <polyline points={pts} fill="none" stroke="var(--accent)" strokeWidth="1.5" />
      </svg>
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

// FIFO realized trading P&L: on demand (a full transaction pull), matches each
// sale against the oldest un-sold buys to show realized profit per item.
function TradingPnlCard({ character }: { character: Character }): ReactNode {
  const [pnl, setPnl] = useState<TradingPnlView | null>(null);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState("");

  function load() {
    if (!isTauri()) return;
    setLoading(true);
    setErr("");
    api
      .getTradingPnl(character.id)
      .then((p) => { setPnl(p); setLoading(false); })
      .catch((e) => { setErr(String(e)); setLoading(false); });
  }

  return (
    <div className="card" style={{ marginTop: 16 }}>
      <h3>
        Trading P&amp;L <span style={{ color: "var(--text-dim)", fontWeight: 400, fontSize: 12 }}>· FIFO realized</span>
        <button style={{ float: "right", fontSize: 11 }} onClick={load} disabled={loading}>
          {loading ? "Matching…" : pnl ? "Refresh" : "Compute"}
        </button>
      </h3>
      {err && <p style={{ color: "var(--danger)", fontSize: 12 }}>{err}</p>}
      {!pnl ? (
        <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
          Reconstructs realized profit from your transaction history (fees excluded). Sales of items
          bought before the history window show cost-free.
        </p>
      ) : (
        <>
          <div className="cashflow-totals">
            <span className="pos">{ISK.format(pnl.total_revenue)} revenue</span>
            <span className="neg">{ISK.format(pnl.total_cost)} cost</span>
            <span className={pnl.total_profit >= 0 ? "pos" : "neg"}>
              {ISK.format(pnl.total_profit)} gross
            </span>
            <span className={pnl.total_net >= 0 ? "pos" : "neg"} title="After 3% broker + 4.5% sales tax">
              {ISK.format(pnl.total_net)} net
            </span>
          </div>
          <table className="holdings" style={{ marginTop: 8 }}>
            <tbody>
              {pnl.items.slice(0, 40).map((i, idx) => (
                <tr key={idx}>
                  <td>
                    {i.name}
                    {i.units_open > 0 && (
                      <span style={{ color: "var(--text-dim)", fontSize: 11 }}> · {ISK.format(i.units_open)} held</span>
                    )}
                  </td>
                  <td className={`mono num ${i.profit >= 0 ? "pos" : "neg"}`}>{ISK.format(i.profit)}</td>
                  <td className="mono num" style={{ color: "var(--text-dim)" }}>
                    {i.margin_pct != null ? `${i.margin_pct.toFixed(0)}%` : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}

// Buy-and-deliver: paste a shopping list (in-game multibuy), get the cheapest
// hub per item, totals, and cargo volume — the "Amazon for EVE" flow.
function ShoppingList(): ReactNode {
  const [text, setText] = useState("");
  const [plan, setPlan] = useState<ShoppingPlanView | null>(null);
  const [loading, setLoading] = useState(false);
  const [dest, setDest] = useState("");
  const [delivery, setDelivery] = useState<CourierView | null>(null);

  function price() {
    if (!isTauri() || !text.trim()) return;
    setLoading(true);
    setDelivery(null);
    api.shoppingPlan(text).then((p) => { setPlan(p); setLoading(false); }).catch(() => setLoading(false));
  }

  function estimateDelivery() {
    // Origin = the hub with the most spend; collateral = total cost, cargo =
    // total volume. Reward 0 so the estimate returns a *suggested* fair reward.
    if (!isTauri() || !plan || !dest.trim() || plan.by_hub.length === 0) return;
    const origin = plan.by_hub[0][0];
    api
      .courierEstimate(origin, dest.trim(), plan.total_volume, plan.total_cost, 0, "secure")
      .then(setDelivery)
      .catch(() => setDelivery(null));
  }

  return (
    <div className="card">
      <h3>Shopping List <span style={{ color: "var(--text-dim)", fontWeight: 400, fontSize: 12 }}>· buy & deliver</span></h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Paste an in-game multibuy (or "Item xN" lines). Finds the cheapest hub per item and totals
        the cost + cargo volume so you know where to shop and what to haul.
      </p>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={"Damage Control II x5\n200mm AutoCannon II x8\nWarrior II x10"}
        rows={5}
        style={{ width: "100%", fontFamily: "var(--mono, monospace)", fontSize: 12 }}
      />
      <button onClick={price} disabled={loading || !text.trim()} style={{ marginTop: 8 }}>
        {loading ? "Pricing…" : "Price list"}
      </button>
      {plan && (
        <div style={{ marginTop: 10 }}>
          <div className="cashflow-totals">
            <span className="neg">{ISK.format(plan.total_cost)} ISK</span>
            <span>{ISK.format(Math.round(plan.total_volume))} m³</span>
          </div>
          {plan.by_hub.length > 0 && (
            <p style={{ fontSize: 12, color: "var(--text-dim)", margin: "4px 0" }}>
              Buy at: {plan.by_hub.map(([hub, cost]) => `${hub} (${ISK.format(cost)})`).join(" · ")}
            </p>
          )}
          <table className="holdings" style={{ marginTop: 6 }}>
            <tbody>
              {plan.lines.map((l, i) => (
                <tr key={i}>
                  <td>
                    {l.unresolved ? <span style={{ color: "var(--caution)" }}>{l.name} (unknown)</span> : l.name}
                    <span style={{ color: "var(--text-dim)", fontSize: 11 }}> ×{ISK.format(l.quantity)}</span>
                  </td>
                  <td className="loc">{l.best_hub}</td>
                  <td className="mono num">{l.line_total > 0 ? ISK.format(l.line_total) : "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {plan.by_hub.length > 0 && (
            <div style={{ marginTop: 10, borderTop: "1px solid var(--border)", paddingTop: 8 }}>
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <span style={{ fontSize: 12, color: "var(--text-dim)" }}>Deliver from {plan.by_hub[0][0]} to</span>
                <input
                  value={dest}
                  onChange={(e) => setDest(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && estimateDelivery()}
                  placeholder="destination system…"
                  style={{ flex: 1 }}
                />
                <button onClick={estimateDelivery} disabled={!dest.trim()}>Estimate haul</button>
              </div>
              {delivery && delivery.found && (
                <p style={{ fontSize: 12, marginTop: 6 }}>
                  {delivery.jumps} jumps · fair reward{" "}
                  <strong className="pos">{ISK.format(delivery.suggested_reward)} ISK</strong>
                  {delivery.kills_on_route > 0 && (
                    <span style={{ color: "var(--danger)" }}> · ⚠{delivery.kills_on_route} kills on route</span>
                  )}
                  {delivery.lowsec_hops > 0 && (
                    <span style={{ color: "var(--caution)" }}> · {delivery.lowsec_hops} low/null hop(s)</span>
                  )}
                </p>
              )}
              {delivery && !delivery.found && (
                <p style={{ fontSize: 12, marginTop: 6, color: "var(--text-dim)" }}>{delivery.message}</p>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// Hub trade analyzer: the 5-hub board for one item (best buy/sell at Jita,
// Amarr, Dodixie, Rens, Hek) with the best immediate flip and sell-to-sell
// relist. Live-ESI equivalent of a hub market-analysis spreadsheet.
function HubAnalyzer(): ReactNode {
  const [query, setQuery] = useState("");
  const [board, setBoard] = useState<HubBoardView | null>(null);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState("");

  function analyze() {
    if (!isTauri() || !query.trim()) return;
    setLoading(true);
    setErr("");
    api
      .getHubBoard(query.trim())
      .then((b) => { setBoard(b); setLoading(false); })
      .catch((e) => { setErr(String(e)); setLoading(false); });
  }
  const price = (p: number | null) => (p != null ? ISK.format(p) : "—");
  const pct = (p: number | null) => (p != null ? `${(p * 100).toFixed(1)}%` : "");

  return (
    <div className="card">
      <h3>Hub Trade Analyzer</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        The 5-hub board for one item — best buy/sell at each hub, plus the best flip (sell into buy
        orders) and best sell-to-sell relist (fees applied).
      </p>
      <div style={{ display: "flex", gap: 8 }}>
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && analyze()}
          placeholder="Item name (e.g. Damage Control II)…"
          style={{ flex: 1 }}
        />
        <button onClick={analyze} disabled={loading || !query.trim()}>
          {loading ? "Reading hubs…" : "Analyze"}
        </button>
      </div>
      {err && <p style={{ color: "var(--danger)", fontSize: 12 }}>{err}</p>}
      {board && (
        <div style={{ marginTop: 10 }}>
          <p style={{ fontSize: 13, margin: "0 0 6px" }}>
            <strong>{board.name}</strong>
            {board.volume > 0 && <span style={{ color: "var(--text-dim)", fontSize: 11 }}> · {board.volume} m³</span>}
          </p>
          <table className="holdings">
            <thead>
              <tr>
                <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Hub</th>
                <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Best sell</th>
                <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Best buy</th>
              </tr>
            </thead>
            <tbody>
              {board.hubs.map((h) => (
                <tr key={h.hub}>
                  <td>{h.hub}</td>
                  <td className="mono num">{price(h.best_sell)}</td>
                  <td className="mono num">{price(h.best_buy)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div style={{ marginTop: 8, display: "grid", gap: 4, fontSize: 12 }}>
            {board.flip_profit != null ? (
              <div>
                <span className="badge safe">Flip</span> buy <strong>{board.flip_buy_hub}</strong> → sell into buy orders at{" "}
                <strong>{board.flip_sell_hub}</strong>:{" "}
                <span className="pos mono">{ISK.format(board.flip_profit)}/unit</span>{" "}
                <span style={{ color: "var(--text-dim)" }}>({pct(board.flip_margin_pct)})</span>
                {board.volume > 0 && (
                  <span style={{ color: "var(--text-dim)" }}> · {ISK.format(board.flip_profit / board.volume)}/m³</span>
                )}
              </div>
            ) : (
              <div style={{ color: "var(--text-dim)" }}>No profitable immediate flip.</div>
            )}
            {board.sell_profit != null ? (
              <div>
                <span className="badge caution">Relist</span> buy <strong>{board.sell_buy_hub}</strong> → list a sell order at{" "}
                <strong>{board.sell_sell_hub}</strong>:{" "}
                <span className="pos mono">{ISK.format(board.sell_profit)}/unit</span>{" "}
                <span style={{ color: "var(--text-dim)" }}>({pct(board.sell_margin_pct)})</span>
              </div>
            ) : (
              <div style={{ color: "var(--text-dim)" }}>No profitable sell-to-sell relist.</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// The spreadsheet's headline: rank the best hub trades right now across the
// curated universe, in flip or relist mode, cargo-aware (units + trip profit).
function HubTradeScanner(): ReactNode {
  const [rows, setRows] = useState<HubTradeView[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [relist, setRelist] = useState(false);
  const [scope, setScope] = useState("curated");
  const [cargo, setCargo] = useState("");
  const [minProfitM, setMinProfitM] = useState("1"); // millions ISK/unit
  const [minVol, setMinVol] = useState("100");
  const cargoM3 = parseFloat(cargo) || 0;

  function scan() {
    if (!isTauri()) return;
    setLoading(true);
    api
      .scanHubTrades({
        scope,
        relist,
        cargoM3: cargoM3 > 0 ? cargoM3 : undefined,
        // Filters mainly matter for "all" (curated/minerals are pre-vetted).
        minProfit: scope === "all" ? (parseFloat(minProfitM) || 0) * 1_000_000 : undefined,
        minVolume: scope === "all" ? parseInt(minVol) || 0 : undefined,
        topN: scope === "all" ? 50 : 25,
      })
      .then((r) => { setRows(r); setLoading(false); })
      .catch(() => { setRows([]); setLoading(false); });
  }

  return (
    <div className="card">
      <h3>Top Hub Trades</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Ranks the best cross-hub trades right now. Enter a cargo size to rank by total per-trip
        profit and see how many units fit. "All items" pulls each hub's full order book — slow the
        first time, then cached.
      </p>
      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <select value={scope} onChange={(e) => setScope(e.target.value)} style={{ fontSize: 12 }}>
          <option value="curated">Curated list</option>
          <option value="minerals">Minerals only</option>
          <option value="all">All items (slow)</option>
        </select>
        <label style={{ fontSize: 12, display: "flex", gap: 4, alignItems: "center" }}>
          <input type="checkbox" checked={relist} onChange={(e) => setRelist(e.target.checked)} />
          Sell-to-sell (relist)
        </label>
        <input
          value={cargo}
          onChange={(e) => setCargo(e.target.value)}
          placeholder="cargo m³ (optional)"
          style={{ width: 130 }}
        />
        <button onClick={scan} disabled={loading}>
          {loading ? (scope === "all" ? "Reading full order books…" : "Scanning hubs…") : "Scan"}
        </button>
      </div>
      {scope === "all" && (
        <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap", marginTop: 6 }}>
          <label style={{ fontSize: 11, color: "var(--text-dim)", display: "flex", gap: 4, alignItems: "center" }}>
            Min profit/unit (M ISK)
            <input value={minProfitM} onChange={(e) => setMinProfitM(e.target.value)} style={{ width: 60 }} />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)", display: "flex", gap: 4, alignItems: "center" }}>
            Min depth (units)
            <input value={minVol} onChange={(e) => setMinVol(e.target.value)} style={{ width: 70 }} />
          </label>
        </div>
      )}
      {rows && rows.length === 0 && !loading && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>No profitable trades found right now.</p>
      )}
      {rows && rows.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Item</th>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Route</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>{cargoM3 > 0 ? "Trip profit" : "ISK/unit"}</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Margin</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((t) => (
              <tr key={t.type_id}>
                <td>
                  <ItemHover name={t.name} />
                  {cargoM3 > 0 && t.units_per_trip > 0 && (
                    <span style={{ color: "var(--text-dim)", fontSize: 11 }}> · {ISK.format(t.units_per_trip)}u</span>
                  )}
                </td>
                <td className="loc">
                  {t.buy_hub} → {t.sell_hub}
                  {t.jumps > 0 && <span style={{ color: "var(--text-dim)", fontSize: 11 }}> · {t.jumps}j</span>}
                  {t.route_kills > 0 && (
                    <span
                      style={{ color: t.route_kills > 20 ? "var(--danger)" : "var(--caution)", fontSize: 11 }}
                      title="Recent ship+pod kills along the secure route"
                    >
                      {" "}⚠{t.route_kills}
                    </span>
                  )}
                  {t.available_volume > 0 && (
                    <span style={{ color: "var(--text-dim)", fontSize: 11 }}> · {ISK.format(t.available_volume)}u</span>
                  )}
                </td>
                <td className="mono num pos">
                  {cargoM3 > 0 && t.trip_profit > 0 ? ISK.format(t.trip_profit) : ISK.format(t.profit_per_unit)}
                </td>
                <td className="mono num" style={{ color: "var(--text-dim)" }}>{(t.margin_pct * 100).toFixed(0)}%</td>
              </tr>
            ))}
          </tbody>
        </table>
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

function ResearchAgentsCard({ character }: { character: Character }): ReactNode {
  const [rows, setRows] = useState<ResearchAgentView[] | null>(null);

  useEffect(() => {
    setRows(null);
    if (!isTauri()) return;
    api.getResearchAgents(character.id).then(setRows).catch(() => setRows([]));
  }, [character.id]);

  if (!rows || rows.length === 0) return null;
  const accrued = (r: ResearchAgentView) => {
    const days = (Date.now() - new Date(r.started_at).getTime()) / 86_400_000;
    return r.remainder_points + r.points_per_day * Math.max(0, days);
  };
  return (
    <div className="card research-agents" style={{ marginTop: 16 }}>
      <h3>R&amp;D Agents <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {rows.length}</span></h3>
      <table className="holdings">
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td>
                {r.datacore_name}
                <div style={{ color: "var(--text-dim)", fontSize: 11 }}>{r.agent_name}</div>
              </td>
              <td className="mono num">{r.points_per_day.toFixed(0)} RP/day</td>
              <td className="mono num pos">{ISK.format(accrued(r))} RP</td>
            </tr>
          ))}
        </tbody>
      </table>
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
  const [sub, setSub] = useSubTab("market");
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
          <ShoppingList />
          <HubAnalyzer />
          <HubTradeScanner />
          <StationScanner />
        </>
      )}
      {sub === "industry" && (
        <>
          <ReprocessCalc />
          <BuildPlanner />
          {character && <ResearchAgentsCard character={character} />}
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
                      <span className={o.is_buy_order ? "neg" : "pos"}>{o.is_buy_order ? "BUY" : "SELL"}</span>{" "}
                      <ItemHover name={o.item_name} />
                      {o.undercut && (
                        <span
                          className="badge caution"
                          style={{ marginLeft: 6 }}
                          title={o.best_competing != null ? `Best competing: ${ISK.format(o.best_competing)} ISK` : undefined}
                        >
                          {o.is_buy_order ? "outbid" : "undercut"}
                        </span>
                      )}
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
// Ship Replacement Program board: submit losses, review (approve/reject with a
// payout), and track what's owed and paid. Local — no ESI writes.
function SrpBoard(): ReactNode {
  const [data, setData] = useState<SrpBoardView | null>(null);
  const [pilot, setPilot] = useState("");
  const [ship, setShip] = useState("");
  const [loss, setLoss] = useState("");
  const [location, setLocation] = useState("");
  const [km, setKm] = useState("");
  const [notes, setNotes] = useState("");
  const [filter, setFilter] = useState("pending");

  function load() {
    if (!isTauri()) return;
    api.getSrpBoard().then(setData).catch(() => setData(null));
  }
  useEffect(() => { load(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);

  function submit() {
    if (!isTauri() || !pilot.trim() || !ship.trim()) return;
    api
      .submitSrpClaim({
        pilot: pilot.trim(),
        ship: ship.trim(),
        loss_value: (parseFloat(loss) || 0) * 1_000_000,
        location: location.trim(),
        killmail_url: km.trim(),
        notes: notes.trim(),
      })
      .then(() => { setPilot(""); setShip(""); setLoss(""); setLocation(""); setKm(""); setNotes(""); load(); })
      .catch(() => undefined);
  }

  const [prefillMsg, setPrefillMsg] = useState("");
  function prefill() {
    if (!isTauri() || !km.trim()) return;
    setPrefillMsg("Looking up killmail…");
    api
      .srpPrefill(km.trim())
      .then((p) => {
        setPilot(p.pilot);
        setShip(p.ship);
        setLoss((p.loss_value / 1_000_000).toFixed(1));
        setLocation(p.location);
        setPrefillMsg("Filled from killmail.");
      })
      .catch((e) => setPrefillMsg(String(e)));
  }

  function approve(c: SrpBoardView["claims"][number]) {
    const v = window.prompt("Payout (millions of ISK):", (c.loss_value / 1_000_000).toFixed(0));
    if (v === null) return;
    const note = window.prompt("Reviewer note (optional):", "") ?? "";
    api.decideSrpClaim(c.id, "approved", (parseFloat(v) || 0) * 1_000_000, note).then(load);
  }
  function reject(c: SrpBoardView["claims"][number]) {
    const note = window.prompt("Reason for rejection:", "") ?? "";
    api.decideSrpClaim(c.id, "rejected", 0, note).then(load);
  }

  if (!isTauri()) {
    return <div className="card"><p style={{ color: "var(--text-dim)" }}>The SRP board runs in the desktop shell.</p></div>;
  }

  const s = data?.summary;
  const claims = (data?.claims ?? []).filter((c) => filter === "all" || c.status === filter);

  return (
    <>
      <div className="card">
        <h3>SRP · Submit a claim</h3>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 0.8fr 1fr", gap: 6, alignItems: "end" }}>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Pilot
            <input value={pilot} onChange={(e) => setPilot(e.target.value)} />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Ship
            <input value={ship} onChange={(e) => setShip(e.target.value)} />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Loss (M)
            <input value={loss} onChange={(e) => setLoss(e.target.value)} />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Location
            <input value={location} onChange={(e) => setLocation(e.target.value)} placeholder="J123456 / system" />
          </label>
        </div>
        <div style={{ display: "flex", gap: 8, marginTop: 6 }}>
          <input value={km} onChange={(e) => setKm(e.target.value)} placeholder="zKillboard link (optional)" style={{ flex: 1 }} />
          <button onClick={prefill} disabled={!km.trim()} title="Fill pilot/ship/value/system from the killmail">
            Autofill
          </button>
          <input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="notes" style={{ flex: 1 }} />
          <button onClick={submit}>Submit</button>
        </div>
        {prefillMsg && <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "4px 0 0" }}>{prefillMsg}</p>}
      </div>

      {s && (
        <div className="card">
          <h3>Board</h3>
          <div className="cashflow-totals">
            <span>{s.pending} pending</span>
            <span className="neg">{ISK.format(s.outstanding)} owed</span>
            <span className="pos">{ISK.format(s.total_paid)} paid</span>
            <span style={{ color: "var(--text-dim)" }}>{s.rejected} rejected</span>
          </div>
          <div style={{ marginTop: 8 }}>
            <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
              Show{" "}
              <select value={filter} onChange={(e) => setFilter(e.target.value)}>
                <option value="pending">Pending</option>
                <option value="approved">Approved (unpaid)</option>
                <option value="paid">Paid</option>
                <option value="rejected">Rejected</option>
                <option value="all">All</option>
              </select>
            </label>
          </div>
          {claims.length > 0 ? (
            <table className="holdings" style={{ marginTop: 8 }}>
              <tbody>
                {claims.map((c) => (
                  <tr key={c.id}>
                    <td>
                      <strong>{c.pilot}</strong> · {c.ship}
                      <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
                        {c.location}{c.notes ? ` · ${c.notes}` : ""}
                        {c.reviewer_note ? ` · review: ${c.reviewer_note}` : ""}
                      </div>
                    </td>
                    <td className="mono num">{ISK.format(c.loss_value)}</td>
                    <td className="mono num pos">{c.payout > 0 ? ISK.format(c.payout) : ""}</td>
                    <td style={{ textAlign: "right", whiteSpace: "nowrap" }}>
                      {c.status === "pending" && (
                        <>
                          <button style={{ fontSize: 11 }} onClick={() => approve(c)}>Approve</button>{" "}
                          <button style={{ fontSize: 11 }} onClick={() => reject(c)}>Reject</button>
                        </>
                      )}
                      {c.status === "approved" && (
                        <button style={{ fontSize: 11 }} onClick={() => api.markSrpPaid(c.id).then(load)}>Mark paid</button>
                      )}
                      {(c.status === "paid" || c.status === "rejected") && (
                        <span style={{ fontSize: 11, color: "var(--text-dim)" }}>{c.status}</span>
                      )}{" "}
                      <button style={{ fontSize: 11 }} onClick={() => api.deleteSrpClaim(c.id).then(load)}>✕</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 8 }}>No {filter} claims.</p>
          )}
        </div>
      )}
    </>
  );
}

// Structure reinforcement timerboard: track armor/hull/anchor exit times with a
// live countdown. Local — no ESI writes.
function Timerboard(): ReactNode {
  const [timers, setTimers] = useState<TimerView[] | null>(null);
  const [title, setTitle] = useState("");
  const [system, setSystem] = useState("");
  const [structure, setStructure] = useState("Astrahus");
  const [type, setType] = useState("armor");
  const [side, setSide] = useState("hostile");
  const [when, setWhen] = useState("");
  const [, tick] = useState(0);

  function load() {
    if (!isTauri()) return;
    api.listTimers().then(setTimers).catch(() => setTimers([]));
  }
  useEffect(() => {
    load();
    const t = window.setInterval(() => tick((n) => n + 1), 1000);
    return () => window.clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function add() {
    if (!isTauri() || !title.trim() || !when) return;
    // datetime-local is wall-clock; EVE timers are UTC, so read the input as UTC.
    const exits = Math.round(new Date(when + "Z").getTime() / 1000);
    if (Number.isNaN(exits)) return;
    api.addTimer({ title: title.trim(), system: system.trim(), structure: structure.trim(), timer_type: type, side, exits_at: exits, notes: "" })
      .then(() => { setTitle(""); setSystem(""); setWhen(""); load(); })
      .catch(() => undefined);
  }

  function countdown(secs: number): string {
    if (secs <= 0) return "EXITED";
    const d = Math.floor(secs / 86400), h = Math.floor((secs % 86400) / 3600), m = Math.floor((secs % 3600) / 60), s = secs % 60;
    if (d > 0) return `${d}d ${h}h ${m}m`;
    if (h > 0) return `${h}h ${m}m ${s}s`;
    return `${m}m ${s}s`;
  }

  if (!isTauri()) {
    return <div className="card"><p style={{ color: "var(--text-dim)" }}>The timerboard runs in the desktop shell.</p></div>;
  }

  return (
    <>
      <div className="card">
        <h3>Timerboard · Add timer <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· times in EVE (UTC)</span></h3>
        <div style={{ display: "grid", gridTemplateColumns: "1.2fr 0.9fr 1fr auto auto", gap: 6, alignItems: "end" }}>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Title
            <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Astrahus armor" />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            System
            <input value={system} onChange={(e) => setSystem(e.target.value)} />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Structure
            <input value={structure} onChange={(e) => setStructure(e.target.value)} />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Type
            <select value={type} onChange={(e) => setType(e.target.value)}>
              {["armor", "hull", "anchor", "moon", "other"].map((t) => <option key={t} value={t}>{t}</option>)}
            </select>
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Side
            <select value={side} onChange={(e) => setSide(e.target.value)}>
              {["hostile", "friendly", "neutral"].map((sd) => <option key={sd} value={sd}>{sd}</option>)}
            </select>
          </label>
        </div>
        <div style={{ display: "flex", gap: 8, marginTop: 6, alignItems: "center" }}>
          <input type="datetime-local" value={when} onChange={(e) => setWhen(e.target.value)} />
          <button onClick={add} disabled={!title.trim() || !when}>Add timer</button>
        </div>
      </div>

      {timers && timers.length > 0 && (
        <div className="card">
          <h3>Upcoming</h3>
          <table className="holdings">
            <tbody>
              {timers.map((t) => (
                <tr key={t.id} style={{ opacity: t.seconds_remaining <= 0 ? 0.5 : 1 }}>
                  <td>
                    <strong>{t.title}</strong>
                    <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
                      {t.system}{t.structure ? ` · ${t.structure}` : ""} · {t.timer_type}
                    </div>
                  </td>
                  <td style={{ fontSize: 11 }} className={t.side === "friendly" ? "pos" : t.side === "hostile" ? "neg" : ""}>{t.side}</td>
                  <td className={"mono num" + (t.seconds_remaining > 0 && t.seconds_remaining < 3600 ? " neg" : "")}>
                    {countdown(t.seconds_remaining)}
                  </td>
                  <td style={{ textAlign: "right" }}>
                    <button style={{ fontSize: 11 }} onClick={() => api.deleteTimer(t.id).then(load)}>✕</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}

// Recruitment / HR pipeline: applicants move applied → interview → trial →
// accepted/rejected. Local — no ESI writes.
const RECRUIT_STAGES = ["applied", "interview", "trial", "accepted", "rejected"];

function RecruitBoard(): ReactNode {
  const [data, setData] = useState<RecruitBoardView | null>(null);
  const [name, setName] = useState("");
  const [source, setSource] = useState("forum");
  const [recruiter, setRecruiter] = useState("");
  const [notes, setNotes] = useState("");
  const [filter, setFilter] = useState("active");

  function load() {
    if (!isTauri()) return;
    api.getRecruitBoard().then(setData).catch(() => setData(null));
  }
  useEffect(() => { load(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);

  function submit() {
    if (!isTauri() || !name.trim()) return;
    api.submitRecruit(name.trim(), source.trim(), notes.trim(), recruiter.trim())
      .then(() => { setName(""); setNotes(""); load(); })
      .catch(() => undefined);
  }

  function advance(id: number, status: string) {
    const note = window.prompt(`Note for "${status}" (optional):`, "") ?? "";
    api.setRecruitStatus(id, status, note).then(load);
  }

  if (!isTauri()) {
    return <div className="card"><p style={{ color: "var(--text-dim)" }}>The recruitment board runs in the desktop shell.</p></div>;
  }

  const s = data?.summary;
  const recruits = (data?.recruits ?? []).filter((r) =>
    filter === "all" ? true : filter === "active" ? !["accepted", "rejected"].includes(r.status) : r.status === filter,
  );

  return (
    <>
      <div className="card">
        <h3>Recruitment · New applicant</h3>
        <div style={{ display: "grid", gridTemplateColumns: "1.2fr 0.9fr 0.9fr", gap: 6, alignItems: "end" }}>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Name
            <input value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Source
            <input value={source} onChange={(e) => setSource(e.target.value)} placeholder="forum / in-game / referral" />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Recruiter
            <input value={recruiter} onChange={(e) => setRecruiter(e.target.value)} />
          </label>
        </div>
        <div style={{ display: "flex", gap: 8, marginTop: 6 }}>
          <input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="notes" style={{ flex: 1 }} />
          <button onClick={submit}>Add</button>
        </div>
      </div>

      {s && (
        <div className="card">
          <h3>Pipeline</h3>
          <div className="cashflow-totals">
            <span>{s.applied} applied</span>
            <span>{s.interview} interview</span>
            <span>{s.trial} trial</span>
            <span className="pos">{s.accepted} accepted</span>
            <span style={{ color: "var(--text-dim)" }}>{s.rejected} rejected</span>
            {s.accepted + s.rejected > 0 && <span>{(s.acceptance_rate * 100).toFixed(0)}% accept</span>}
          </div>
          <div style={{ marginTop: 8 }}>
            <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
              Show{" "}
              <select value={filter} onChange={(e) => setFilter(e.target.value)}>
                <option value="active">In pipeline</option>
                {RECRUIT_STAGES.map((st) => <option key={st} value={st}>{st}</option>)}
                <option value="all">All</option>
              </select>
            </label>
          </div>
          {recruits.length > 0 ? (
            <table className="holdings" style={{ marginTop: 8 }}>
              <tbody>
                {recruits.map((r) => (
                  <tr key={r.id}>
                    <td>
                      <strong>{r.name}</strong>
                      <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
                        {r.source}{r.recruiter ? ` · ${r.recruiter}` : ""}{r.notes ? ` · ${r.notes}` : ""}
                        {r.reviewer_note ? ` · ${r.reviewer_note}` : ""}
                      </div>
                    </td>
                    <td style={{ fontSize: 12, textTransform: "capitalize" }}>{r.status}</td>
                    <td style={{ textAlign: "right", whiteSpace: "nowrap" }}>
                      <select
                        style={{ fontSize: 11 }}
                        value=""
                        onChange={(e) => { if (e.target.value) advance(r.id, e.target.value); }}
                      >
                        <option value="">Move to…</option>
                        {RECRUIT_STAGES.filter((st) => st !== r.status).map((st) => (
                          <option key={st} value={st}>{st}</option>
                        ))}
                      </select>{" "}
                      <button style={{ fontSize: 11 }} onClick={() => api.deleteRecruit(r.id).then(load)}>✕</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 8 }}>No applicants here.</p>
          )}
        </div>
      )}
    </>
  );
}

function CorpHub({ character }: { character: Character | null }): ReactNode {
  const [groups, setGroups] = useState<CharacterGroup[]>([]);
  const [roster, setRoster] = useState<Character[]>([]);
  const [newName, setNewName] = useState("");
  const [sub, setSub] = useSubTab("groups");

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
          { id: "moons", label: "Moons" },
          { id: "members", label: "Members" },
          { id: "fleet", label: "Fleet" },
          { id: "srp", label: "SRP" },
          { id: "recruit", label: "Recruitment" },
          { id: "timers", label: "Timerboard" },
        ]}
        active={sub}
        onSelect={setSub}
      />
      {sub === "srp" && <SrpBoard />}
      {sub === "recruit" && <RecruitBoard />}
      {sub === "timers" && <Timerboard />}
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
      {sub === "moons" && <MoonExtractions character={character} />}
      {sub === "members" && <CorpMembers character={character} />}
      {sub === "fleet" && <FleetView_ character={character} />}
    </>
  );
}

function FleetView_({ character }: { character: Character | null }): ReactNode {
  const [fleet, setFleet] = useState<FleetView | null>(null);
  const [wings, setWings] = useState<FleetWingView[]>([]);
  const [loading, setLoading] = useState(false);
  const [motd, setMotd] = useState("");
  const [freeMove, setFreeMove] = useState(false);
  const [motdMsg, setMotdMsg] = useState("");
  const [memberMsg, setMemberMsg] = useState("");

  function load() {
    if (!character || !isTauri()) return;
    setLoading(true);
    api.getFleet(character.id).then(setFleet).catch(() => setFleet(null)).finally(() => setLoading(false));
    api.getFleetWings(character.id).then(setWings).catch(() => setWings([]));
  }

  function kick(memberId: number, name: string) {
    if (!character || !isTauri()) return;
    setMemberMsg(`Kicking ${name}…`);
    api.kickFleetMember(character.id, memberId)
      .then(() => { setMemberMsg(`${name} kicked.`); load(); })
      .catch((e) => setMemberMsg(String(e)));
  }

  function moveTo(memberId: number, name: string, value: string) {
    if (!character || !isTauri() || !value) return;
    // value is "squad:<wingId>:<squadId>" for a squad, or a bare role string.
    const [role, wingId, squadId] = value.startsWith("squad:")
      ? ["squad_member", Number(value.split(":")[1]), Number(value.split(":")[2])]
      : [value, undefined, undefined];
    setMemberMsg(`Moving ${name}…`);
    api.moveFleetMember(character.id, memberId, role as string, wingId as number | undefined, squadId as number | undefined)
      .then(() => { setMemberMsg(`${name} moved.`); load(); })
      .catch((e) => setMemberMsg(String(e)));
  }

  function saveMotd() {
    if (!character || !isTauri()) return;
    setMotdMsg("Saving…");
    api.setFleetSettings(character.id, motd, freeMove)
      .then(() => setMotdMsg("Fleet MOTD updated."))
      .catch((e) => setMotdMsg(String(e)));
  }

  useEffect(() => {
    setFleet(null);
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [character?.id]);

  if (!character) return <div className="sub">Select a character to read its fleet.</div>;
  return (
    <div className="card fleet-view" style={{ maxWidth: 720 }}>
      <h3>
        Fleet {fleet?.in_fleet && <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {fleet.member_count}</span>}
      </h3>
      <button onClick={load} disabled={loading}>{loading ? "Loading…" : "Refresh"}</button>
      {fleet && !fleet.in_fleet && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>Not currently in a fleet.</p>
      )}
      {fleet && fleet.in_fleet && (
        <>
          <table className="holdings" style={{ marginTop: 10 }}>
            <tbody>
              {fleet.members.map((m, i) => (
                <tr key={i}>
                  <td>{m.name}</td>
                  <td className="loc">{m.ship}{m.system ? ` · ${m.system}` : ""}</td>
                  <td className="mono num">{m.role}</td>
                  <td style={{ textAlign: "right", whiteSpace: "nowrap" }}>
                    <select
                      value=""
                      onChange={(e) => moveTo(m.character_id, m.name, e.target.value)}
                      title="Move member (boss only)"
                      style={{ fontSize: 11, maxWidth: 120 }}
                    >
                      <option value="">Move to…</option>
                      <option value="fleet_commander">Fleet commander</option>
                      {wings.map((w) => (
                        <optgroup key={w.id} label={w.name || `Wing ${w.id}`}>
                          {w.squads.map((s) => (
                            <option key={s.id} value={`squad:${w.id}:${s.id}`}>
                              {s.name || `Squad ${s.id}`}
                            </option>
                          ))}
                        </optgroup>
                      ))}
                    </select>
                    <button
                      onClick={() => kick(m.character_id, m.name)}
                      title="Kick from fleet (boss only)"
                      style={{ fontSize: 11, marginLeft: 4 }}
                    >
                      Kick
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {memberMsg && <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "6px 0 0" }}>{memberMsg}</p>}
          <div style={{ marginTop: 12, borderTop: "1px solid var(--border)", paddingTop: 10 }}>
            <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "0 0 4px" }}>
              Fleet settings (boss only — ESI write):
            </p>
            <textarea
              value={motd}
              onChange={(e) => setMotd(e.target.value)}
              placeholder="Fleet MOTD…"
              style={{ width: "100%", minHeight: 48, fontSize: 12 }}
            />
            <div style={{ display: "flex", gap: 10, alignItems: "center", marginTop: 6 }}>
              <label style={{ fontSize: 12, display: "flex", gap: 4, alignItems: "center" }}>
                <input type="checkbox" checked={freeMove} onChange={(e) => setFreeMove(e.target.checked)} />
                Free-move
              </label>
              <button onClick={saveMotd}>Update fleet</button>
              {motdMsg && <span style={{ fontSize: 12, color: "var(--text-dim)" }}>{motdMsg}</span>}
            </div>
          </div>
        </>
      )}
    </div>
  );
}

function CorpMembers({ character }: { character: Character | null }): ReactNode {
  const [rows, setRows] = useState<CorpMemberView[] | null>(null);
  const [thefts, setThefts] = useState<ContainerTheftView[]>([]);

  useEffect(() => {
    setRows(null);
    setThefts([]);
    if (!character || !isTauri()) return;
    api.getCorpMembers(character.id).then(setRows).catch(() => setRows([]));
    api.getContainerThefts(character.id).then(setThefts).catch(() => setThefts([]));
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
      {thefts.length > 0 && (
        <div style={{ marginTop: 14, borderTop: "1px solid var(--border)", paddingTop: 10 }}>
          <h4 style={{ margin: "0 0 6px", color: "var(--danger)" }}>
            Container-log alerts <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {thefts.length}</span>
          </h4>
          <table className="holdings">
            <tbody>
              {thefts.slice(0, 50).map((t, i) => (
                <tr key={i}>
                  <td>{t.character_name}</td>
                  <td className="loc">
                    {t.action}
                    {t.item_name ? ` · ${t.item_name}${t.quantity ? ` ×${t.quantity}` : ""}` : ""}
                    <span style={{ display: "block", fontSize: 11, color: "var(--text-dim)" }}>{t.reason}</span>
                  </td>
                  <td className="mono num">{t.logged_at ? shortDate(t.logged_at) : ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "6px 0 0" }}>
            Heuristic vetting from corp container audit logs (password tampering, unlocks, large moves).
          </p>
        </div>
      )}
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

// Moon-mining schedule: each extraction's chunk arrives at a known time, so we
// fetch once and count down locally (like structure fuel).
function MoonExtractions({ character }: { character: Character | null }): ReactNode {
  const [rows, setRows] = useState<ExtractionView[] | null>(null);
  const [, forceTick] = useState(0);
  const [loadedAt, setLoadedAt] = useState(0);

  useEffect(() => {
    setRows(null);
    if (!character || !isTauri()) return;
    setLoadedAt(Date.now());
    api.getMoonExtractions(character.id).then(setRows).catch(() => setRows([]));
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
          No extractions — needs a Structure Manager role and the corporation-mining scope.
        </p>
      </div>
    );
  }
  const elapsed = loadedAt ? Math.floor((Date.now() - loadedAt) / 1000) : 0;
  return (
    <div className="card" style={{ maxWidth: 640 }}>
      <h3>Moon extractions <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· {rows.length}</span></h3>
      <table className="holdings">
        <tbody>
          {rows.map((e, i) => {
            const remaining = Math.max(0, e.arrival_seconds_remaining - elapsed);
            const ready = e.ready || remaining === 0;
            const soon = !ready && remaining < 86400;
            return (
              <tr key={i}>
                <td>
                  {e.moon_name || e.structure_name}
                  <div style={{ color: "var(--text-dim)", fontSize: 11 }}>{e.structure_name}</div>
                </td>
                <td className={`mono num ${ready ? "neg" : soon ? "neg" : ""}`}>
                  {ready ? "READY TO FRACTURE" : formatDuration(remaining)}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "6px 0 0" }}>
        Chunk-arrival countdown; fracture on arrival before natural decay scatters the ore.
      </p>
    </div>
  );
}

// Cross-content income optimizer: rank what to do right now by risk-adjusted
// ISK/hr over the time you have. Activities are user-described (gross ISK/hr,
// risk, setup, eligibility); the backend ranks them deterministically.
function IncomeOptimizer(): ReactNode {
  const [acts, setActs] = useState<IncomeActivity[]>([]);
  const [results, setResults] = useState<IncomeRanking[] | null>(null);
  const [hours, setHours] = useState("3");
  const [name, setName] = useState("");
  const [rate, setRate] = useState("60");
  const [risk, setRisk] = useState("0");
  const [setup, setSetup] = useState("0");
  const [eligible, setEligible] = useState(true);
  const [actual, setActual] = useState<RealizedIncome | null>(null);
  const [actualMsg, setActualMsg] = useState("");

  function loadActual() {
    if (!isTauri()) return;
    setActualMsg("Reading wallet…");
    api
      .getRealizedIncome(null)
      .then((r) => {
        setActual(r);
        setActualMsg("");
        // Add the realized rate as a benchmark activity so it ranks alongside
        // the hypotheticals. Spread the day's net across the hours entered.
        const hrs = parseFloat(hours) || 1;
        setActs((a) => [
          ...a.filter((x) => x.name !== "My current (actual)"),
          {
            name: "My current (actual)",
            isk_per_hour: r.isk_per_day / Math.max(hrs, 1),
            risk: 0,
            setup_cost: 0,
            eligible: true,
          },
        ]);
        setResults(null);
      })
      .catch((e) => setActualMsg(String(e)));
  }

  function add() {
    if (!name.trim()) return;
    setActs((a) => [
      ...a,
      {
        name: name.trim(),
        isk_per_hour: (parseFloat(rate) || 0) * 1_000_000,
        risk: (parseFloat(risk) || 0) / 100,
        setup_cost: (parseFloat(setup) || 0) * 1_000_000,
        eligible,
      },
    ]);
    setResults(null);
    setName("");
  }

  function rank() {
    if (!isTauri() || acts.length === 0) return;
    api.rankIncome(acts, parseFloat(hours) || 0).then(setResults).catch(() => setResults([]));
  }

  return (
    <div className="card">
      <h3>
        Income Optimizer{" "}
        <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· what should I do?</span>
      </h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Rank activities by risk-adjusted ISK/hr over your available time. ISK in millions, risk in %.
        Untick "can do" for things you're not yet set up for.
      </p>
      <div style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 8 }}>
        <button onClick={loadActual}>Benchmark my actual income</button>
        {actual && (
          <span style={{ fontSize: 12, color: "var(--accent)" }}>
            Your recent rate: {ISK.format(actual.isk_per_day)} ISK/day over {actual.active_days} active day(s)
          </span>
        )}
        {actualMsg && <span style={{ fontSize: 12, color: "var(--text-dim)" }}>{actualMsg}</span>}
      </div>
      <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
        Hours available{" "}
        <input value={hours} onChange={(e) => setHours(e.target.value)} style={{ width: 60 }} />
      </label>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1.4fr repeat(3, 1fr) auto auto",
          gap: 6,
          alignItems: "end",
          marginTop: 8,
        }}
      >
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Activity
          <input value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Ratting" />
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          ISK/hr (M)
          <input value={rate} onChange={(e) => setRate(e.target.value)} />
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Risk %
          <input value={risk} onChange={(e) => setRisk(e.target.value)} />
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Setup (M)
          <input value={setup} onChange={(e) => setSetup(e.target.value)} />
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)", display: "flex", gap: 4, alignItems: "center" }}>
          <input type="checkbox" checked={eligible} onChange={(e) => setEligible(e.target.checked)} />
          Can do
        </label>
        <button onClick={add}>Add</button>
      </div>
      {acts.length > 0 && (
        <div style={{ marginTop: 8, display: "flex", gap: 8 }}>
          <button onClick={rank}>Rank {acts.length}</button>
          <button onClick={() => { setActs([]); setResults(null); }}>Clear</button>
        </div>
      )}
      {results && results.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Activity</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Eff. ISK/hr</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Session</th>
            </tr>
          </thead>
          <tbody>
            {results.map((r, i) => (
              <tr key={r.name + i} style={{ opacity: r.eligible ? 1 : 0.5 }}>
                <td>{i === 0 && r.eligible ? "★ " : ""}{r.name}{r.eligible ? "" : " (locked)"}</td>
                <td className="mono num pos">{ISK.format(r.effective_isk_per_hour)}</td>
                <td className={"mono num" + (r.session_profit >= 0 ? " pos" : "")}>
                  {ISK.format(r.session_profit)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

// The AI "Jarvis" layer: off-by-default, local-first. Settings (endpoint/model)
// plus a chat that drives the read-only tool registry on the backend.
function AiAssistant(): ReactNode {
  const [settings, setSettings] = useState<AiSettingsView | null>(null);
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [embedModel, setEmbedModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [reindexMsg, setReindexMsg] = useState("");
  const [endpoints, setEndpoints] = useState<AiEndpointView[] | null>(null);
  const [saved, setSaved] = useState(false);
  const [history, setHistory] = useState<{ role: "user" | "assistant"; content: string; tools?: string[] }[]>([]);
  const [input, setInput] = useState("");
  const [thinking, setThinking] = useState(false);
  const [err, setErr] = useState("");
  const [agents, setAgents] = useState<AiAgentView[]>([]);
  const [agent, setAgent] = useState("commander");

  useEffect(() => {
    if (!isTauri()) return;
    api.getAiSettings().then((s) => {
      setSettings(s);
      setBaseUrl(s.base_url);
      setModel(s.model);
      setEmbedModel(s.embed_model);
    }).catch(() => undefined);
    api.listAiAgents().then(setAgents).catch(() => undefined);
  }, []);

  function save(next?: Partial<AiSettingsView>) {
    if (!settings) return;
    const enabled = next?.enabled ?? settings.enabled;
    api
      .setAiSettings(enabled, baseUrl, model, embedModel, apiKey === "" ? null : apiKey)
      .then(() => {
        setSettings({ ...settings, enabled, base_url: baseUrl, model, embed_model: embedModel, has_api_key: settings.has_api_key || apiKey !== "" });
        setApiKey("");
        setSaved(true);
        window.setTimeout(() => setSaved(false), 1500);
      })
      .catch((e) => setErr(String(e)));
  }

  function reindex() {
    setReindexMsg("Indexing…");
    api
      .reindexMemory()
      .then((n) => setReindexMsg(n === 0 ? "Set an embeddings model first." : `Indexed ${n} note(s).`))
      .catch((e) => setReindexMsg(String(e)));
  }

  function detect() {
    api.aiDetectEndpoints().then(setEndpoints).catch(() => setEndpoints([]));
  }

  function send() {
    const text = input.trim();
    if (!text || thinking) return;
    const nextHistory = [...history, { role: "user" as const, content: text }];
    setHistory(nextHistory);
    setInput("");
    setThinking(true);
    setErr("");
    api
      .aiChat(nextHistory.map((m) => ({ role: m.role, content: m.content })), agent)
      .then((r) => setHistory((h) => [...h, { role: "assistant", content: r.reply, tools: r.tools_used }]))
      .catch((e) => setErr(String(e)))
      .finally(() => setThinking(false));
  }

  function briefing() {
    if (thinking) return;
    setHistory((h) => [...h, { role: "user", content: "Daily briefing" }]);
    setThinking(true);
    setErr("");
    api
      .aiBriefing()
      .then((r) => setHistory((h) => [...h, { role: "assistant", content: r.reply, tools: r.tools_used }]))
      .catch((e) => setErr(String(e)))
      .finally(() => setThinking(false));
  }

  if (!isTauri()) {
    return (
      <div className="card"><p style={{ color: "var(--text-dim)" }}>The AI assistant runs in the desktop shell.</p></div>
    );
  }

  return (
    <>
      <div className="card">
        <h3>AI Assistant <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· local-first, off by default</span></h3>
        <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
          Point at any OpenAI-compatible endpoint (Ollama, LM Studio, llama.cpp). With a local model your
          data never leaves the machine. The assistant calls read-only tools for real numbers and only advises —
          it never acts in-game.
        </p>
        <label className="setting-row">
          <div><div className="setting-name">Enable AI</div></div>
          <input
            type="checkbox"
            checked={settings?.enabled ?? false}
            onChange={(e) => save({ enabled: e.target.checked })}
          />
        </label>
        <div style={{ display: "grid", gap: 6, marginTop: 8 }}>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Base URL
            <input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="http://127.0.0.1:11434/v1" />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Model
            <input value={model} onChange={(e) => setModel(e.target.value)} placeholder="e.g. llama3.1" />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Embeddings model (optional — vector memory recall)
            <input value={embedModel} onChange={(e) => setEmbedModel(e.target.value)} placeholder="e.g. nomic-embed-text (blank = keyword recall)" />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            API key {settings?.has_api_key ? "(stored)" : "(optional, for cloud)"}
            <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="leave blank for local" />
          </label>
        </div>
        <div style={{ display: "flex", gap: 8, marginTop: 8, alignItems: "center", flexWrap: "wrap" }}>
          <button onClick={() => save()}>Save</button>
          <button onClick={detect}>Detect local</button>
          <button onClick={reindex} title="Embed existing memory notes for vector recall">Reindex memory</button>
          {saved && <span style={{ color: "var(--accent)", fontSize: 12 }}>Saved</span>}
          {reindexMsg && <span style={{ color: "var(--text-dim)", fontSize: 12 }}>{reindexMsg}</span>}
        </div>
        {endpoints && (
          <div style={{ marginTop: 8, fontSize: 12 }}>
            {endpoints.length === 0 && <span style={{ color: "var(--text-dim)" }}>No local endpoint found.</span>}
            {endpoints.map((ep) => (
              <div key={ep.base_url} style={{ marginTop: 4 }}>
                <button
                  style={{ fontSize: 11 }}
                  onClick={() => { setBaseUrl(ep.base_url); if (ep.models[0]) setModel(ep.models[0]); }}
                >
                  Use {ep.label}
                </button>{" "}
                <span style={{ color: "var(--text-dim)" }}>{ep.models.slice(0, 4).join(", ") || "(no models)"}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="card">
        <h3>
          Chat
          <button
            style={{ float: "right", fontSize: 11 }}
            onClick={briefing}
            disabled={!settings?.enabled || thinking}
          >
            Daily briefing
          </button>
          {agents.length > 0 && (
            <select
              style={{ float: "right", fontSize: 11, marginRight: 6 }}
              value={agent}
              onChange={(e) => setAgent(e.target.value)}
              title={agents.find((a) => a.id === agent)?.description}
            >
              {agents.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
            </select>
          )}
        </h3>
        <div style={{ maxHeight: 360, overflowY: "auto", display: "flex", flexDirection: "column", gap: 8 }}>
          {history.length === 0 && (
            <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
              Ask things like &ldquo;is my current system safe?&rdquo;, &ldquo;price-check Tritanium across hubs&rdquo;,
              or &ldquo;any good hauls right now?&rdquo;
            </p>
          )}
          {history.map((m, i) => (
            <div
              key={i}
              style={{
                alignSelf: m.role === "user" ? "flex-end" : "flex-start",
                maxWidth: "85%",
                background: m.role === "user" ? "var(--accent-dim, #1e3a5f)" : "var(--panel, #161b26)",
                border: "1px solid var(--border)",
                borderRadius: 8,
                padding: "6px 10px",
                fontSize: 13,
                whiteSpace: "pre-wrap",
              }}
            >
              {m.content}
              {m.tools && m.tools.length > 0 && (
                <div style={{ color: "var(--text-dim)", fontSize: 11, marginTop: 4 }}>
                  used: {m.tools.join(", ")}
                </div>
              )}
            </div>
          ))}
          {thinking && <div style={{ color: "var(--text-dim)", fontSize: 12 }}>Thinking…</div>}
        </div>
        {err && <p style={{ color: "var(--danger, #f87171)", fontSize: 12 }}>{err}</p>}
        <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
          <input
            style={{ flex: 1 }}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") send(); }}
            placeholder={settings?.enabled ? "Ask the assistant…" : "Enable AI above first"}
            disabled={!settings?.enabled}
          />
          <button onClick={send} disabled={!settings?.enabled || thinking}>Send</button>
        </div>
      </div>

      <MemoryViewer />
    </>
  );
}

// Inspect / curate the assistant's durable memory: the small set of facts it
// keeps about the player. Transparent and editable per the plan — pin, forget.
function MemoryViewer(): ReactNode {
  const [notes, setNotes] = useState<MemoryNoteView[] | null>(null);
  const [kind, setKind] = useState("goal");
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");

  function load() {
    if (!isTauri()) return;
    api.listMemory().then(setNotes).catch(() => setNotes([]));
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function add() {
    if (!title.trim() || !body.trim()) return;
    api.addMemory(kind, title, body).then(() => {
      setTitle("");
      setBody("");
      load();
    }).catch(() => undefined);
  }

  return (
    <div className="card">
      <h3>Memory <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· what the assistant remembers</span></h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Durable facts about you (goals, preferences, decisions). Stored locally; pin to keep, forget to delete.
      </p>
      <div style={{ display: "grid", gridTemplateColumns: "auto 1fr auto", gap: 6, alignItems: "end" }}>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Kind
          <select value={kind} onChange={(e) => setKind(e.target.value)}>
            <option value="goal">goal</option>
            <option value="decision">decision</option>
            <option value="preference">preference</option>
            <option value="relationship">relationship</option>
            <option value="correction">correction</option>
          </select>
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Title
          <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="e.g. Carrier goal" />
        </label>
        <button onClick={add}>Add</button>
      </div>
      <input
        style={{ marginTop: 6, width: "100%" }}
        value={body}
        onChange={(e) => setBody(e.target.value)}
        placeholder="The fact to remember…"
      />
      {notes && notes.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <tbody>
            {notes.map((n) => (
              <tr key={n.id}>
                <td style={{ fontSize: 11, color: "var(--text-dim)" }}>{n.kind}</td>
                <td>
                  <strong>{n.title}</strong>
                  <div style={{ fontSize: 12, color: "var(--text-dim)" }}>{n.body}</div>
                </td>
                <td style={{ textAlign: "right", whiteSpace: "nowrap" }}>
                  <button
                    style={{ fontSize: 11 }}
                    onClick={() => api.pinMemory(n.id, !n.pinned).then(load)}
                  >
                    {n.pinned ? "Unpin" : "Pin"}
                  </button>{" "}
                  <button style={{ fontSize: 11 }} onClick={() => api.forgetMemory(n.id).then(load)}>
                    Forget
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {notes && notes.length === 0 && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>No memories yet.</p>
      )}
    </div>
  );
}

// Item appraisal: paste any inventory selection (or a typed item list) and get
// a per-item market valuation + total, using the shared price reference.
function AppraisalTool(): ReactNode {
  const [text, setText] = useState("");
  const [result, setResult] = useState<LootValueView | null>(null);
  const [loading, setLoading] = useState(false);

  function appraise() {
    if (!isTauri() || !text.trim()) return;
    setLoading(true);
    api.valueLoot(text).then(setResult).catch(() => setResult(null)).finally(() => setLoading(false));
  }

  return (
    <div className="card">
      <h3>Appraisal <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· what's it worth?</span></h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Paste an inventory selection (in-game: select all → copy) or type items as
        <span className="mono"> Name⇥Qty</span> per line. Valued at the ESI reference price.
      </p>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={"Tritanium\t10000\nPyerite\t5000\nDamage Control II\t3"}
        style={{ width: "100%", minHeight: 100, fontFamily: "monospace", fontSize: 12 }}
      />
      <div style={{ display: "flex", gap: 8, marginTop: 6 }}>
        <button onClick={appraise} disabled={loading || !text.trim()}>{loading ? "Pricing…" : "Appraise"}</button>
        {result && <span style={{ alignSelf: "center", fontSize: 13 }} className="pos">{ISK.format(result.total)} total</span>}
      </div>
      {result && result.lines.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Item</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Qty</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Unit</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Value</th>
            </tr>
          </thead>
          <tbody>
            {result.lines.map((l, i) => (
              <tr key={l.name + i}>
                <td>{l.name}</td>
                <td className="mono num">{l.quantity.toLocaleString()}</td>
                <td className="mono num">{ISK.format(l.unit_price)}</td>
                <td className="mono num pos">{ISK.format(l.value)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {result && result.unresolved.length > 0 && (
        <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 6 }}>
          Unpriced ({result.unresolved.length}): {result.unresolved.slice(0, 8).join(", ")}
          {result.unresolved.length > 8 ? "…" : ""}
        </p>
      )}
    </div>
  );
}

function ToolsHub(): ReactNode {
  useLang();
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saved, setSaved] = useState(false);
  const [sub, setSub] = useSubTab("settings");
  const [density, setDensityState] = useState(getDensity());
  const [theme, setThemeState] = useState<Theme>(getTheme());
  const [esiHealth, setEsiHealth] = useState<EsiHealthView | null>(null);

  // Live ESI health meter while the settings tab is open (cheap local read).
  useEffect(() => {
    if (!isTauri() || sub !== "settings") return;
    const load = () => api.getEsiHealth().then(setEsiHealth).catch(() => undefined);
    load();
    const timer = window.setInterval(load, 10000);
    return () => window.clearInterval(timer);
  }, [sub]);

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
          { id: "income", label: "Income Optimizer" },
          { id: "appraisal", label: "Appraisal" },
          { id: "ai", label: "AI Assistant" },
          { id: "plugins", label: "Plugins" },
        ]}
        active={sub}
        onSelect={setSub}
      />
      {sub === "lp" && <LpOptimizer />}
      {sub === "income" && <IncomeOptimizer />}
      {sub === "appraisal" && <AppraisalTool />}
      {sub === "ai" && <AiAssistant />}
      {sub === "plugins" && <PluginsPanel />}
      {sub === "settings" && (!isTauri() ? (
        <div className="card"><p style={{ color: "var(--text-dim)" }}>Design preview — settings load in the desktop shell.</p></div>
      ) : !settings ? (
        <div className="card"><p style={{ color: "var(--text-dim)" }}>Loading…</p></div>
      ) : (
        <div className="card settings-card">
          <label className="setting-row">
            <div>
              <div className="setting-name">{t("common.language")}</div>
              <div className="setting-help">UI language. Untranslated text falls back to English.</div>
            </div>
            <select value={getLang()} onChange={(e) => setLang(e.target.value as "en" | "de")}>
              {LANGUAGES.map((l) => (
                <option key={l.code} value={l.code}>{l.label}</option>
              ))}
            </select>
          </label>
          <label className="setting-row">
            <div>
              <div className="setting-name">Theme</div>
              <div className="setting-help">Dark cockpit (default), light, or a deuteranopia-safe palette (blue=safe, orange=caution).</div>
            </div>
            <select
              value={theme}
              onChange={(e) => {
                const v = e.target.value as Theme;
                setTheme(v);
                setThemeState(v);
              }}
            >
              <option value="dark">Dark</option>
              <option value="light">Light</option>
              <option value="colorblind">Colorblind-safe</option>
            </select>
          </label>
          <label className="setting-row">
            <div>
              <div className="setting-name">Density</div>
              <div className="setting-help">Compact tightens paddings and type for dense, multi-table workflows.</div>
            </div>
            <select
              value={density}
              onChange={(e) => {
                const d = e.target.value as "comfortable" | "compact";
                setDensity(d);
                setDensityState(d);
              }}
            >
              <option value="comfortable">Comfortable</option>
              <option value="compact">Compact</option>
            </select>
          </label>
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
          {esiHealth && (
            <div className="setting-row" style={{ alignItems: "center" }}>
              <div>
                <div className="setting-name">ESI health</div>
                <div className="setting-help">
                  Error-budget headroom (100 = healthy; polling pauses near 20).
                  {esiHealth.backoff_seconds > 0 ? ` Backing off ${esiHealth.backoff_seconds}s.` : ""}
                </div>
              </div>
              <span
                className="mono"
                style={{
                  color:
                    esiHealth.budget_remaining > 50
                      ? "var(--safe)"
                      : esiHealth.budget_remaining > 20
                        ? "var(--caution)"
                        : "var(--danger)",
                }}
              >
                {esiHealth.budget_remaining}/100
              </span>
            </div>
          )}
          <div className="setting-saved" style={{ opacity: saved ? 1 : 0 }}>Saved ✓</div>
        </div>
      ))}
      {sub === "settings" && <DataPrivacy />}
      {sub === "settings" && <AppMaintenance />}
    </>
  );
}

// Phase 7: update check, opt-in telemetry, and offline share-code import.
function AppMaintenance(): ReactNode {
  const [update, setUpdate] = useState<UpdateStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [consent, setConsent] = useState(false);
  const [events, setEvents] = useState<TelemetryEventView[]>([]);
  const [code, setCode] = useState("");
  const [importMsg, setImportMsg] = useState("");

  useEffect(() => {
    if (!isTauri()) return;
    api.getTelemetryConsent().then(setConsent).catch(() => undefined);
  }, []);

  function refreshEvents() {
    api.listTelemetry().then(setEvents).catch(() => setEvents([]));
  }
  function toggleConsent(next: boolean) {
    setConsent(next);
    api.setTelemetryConsent(next).then(() => { if (next) refreshEvents(); else setEvents([]); }).catch(() => undefined);
  }
  function check() {
    setChecking(true);
    api.checkForUpdate().then(setUpdate).catch(() => undefined).finally(() => setChecking(false));
  }
  function doImport() {
    setImportMsg("Importing…");
    api
      .importShared(code.trim())
      .then((r) => { setImportMsg(`Imported ${r.kind === "plan" ? "skill plan" : "fit"} "${r.name}" to your library.`); setCode(""); })
      .catch((e) => setImportMsg(String(e)));
  }

  if (!isTauri()) return null;

  return (
    <div className="card">
      <h3>App &amp; Community</h3>

      <div style={{ marginBottom: 12 }}>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <button onClick={check} disabled={checking}>{checking ? "Checking…" : "Check for updates"}</button>
          {update && (
            <span style={{ fontSize: 12, color: update.update_available ? "var(--accent)" : "var(--text-dim)" }}>
              {update.update_available
                ? <>Update available: {update.latest} {update.url && <a href={update.url} target="_blank" rel="noreferrer">release notes</a>}</>
                : `Up to date (${update.current}).`}
            </span>
          )}
        </div>
      </div>

      <label className="setting-row">
        <div>
          <div className="setting-name">Anonymous usage telemetry</div>
          <div className="setting-help">Off by default. Sends only feature-usage counts — never ISK, names, ids, or any game data. Everything stays local until you opt in.</div>
        </div>
        <input type="checkbox" checked={consent} onChange={(e) => toggleConsent(e.target.checked)} />
      </label>
      {consent && (
        <div style={{ fontSize: 11, color: "var(--text-dim)", marginBottom: 12 }}>
          <button onClick={refreshEvents} style={{ fontSize: 11 }}>Show buffered events</button>
          {events.length > 0 && (
            <button onClick={() => api.clearTelemetry().then(() => setEvents([]))} style={{ fontSize: 11, marginLeft: 6 }}>Clear</button>
          )}
          {events.map((e, i) => (
            <div key={i} className="mono">{e.name} {e.counts_json !== "{}" ? e.counts_json : ""}</div>
          ))}
        </div>
      )}

      <div style={{ borderTop: "1px solid var(--border)", paddingTop: 10 }}>
        <div className="setting-name">Import a shared fit or skill plan</div>
        <div className="setting-help" style={{ marginBottom: 6 }}>
          Paste an <span className="mono">EVECMDR1:</span> share code from another player — it imports straight into your local library.
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <input
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder="EVECMDR1:…"
            style={{ flex: 1 }}
          />
          <button onClick={doImport} disabled={!code.trim()}>Import</button>
        </div>
        {importMsg && <p style={{ fontSize: 12, color: "var(--text-dim)", marginTop: 6 }}>{importMsg}</p>}
      </div>
    </div>
  );
}

// Data portability + privacy: one-click export of all local data, and a
// confirmed wipe. Reinforces the local-first trust story.
function DataPrivacy(): ReactNode {
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [confirming, setConfirming] = useState(false);

  function exportData() {
    if (!isTauri()) return;
    setBusy(true);
    setMsg("");
    api
      .exportData()
      .then((json) => {
        const blob = new Blob([json], { type: "application/json" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `eve-commander-export-${new Date().toISOString().slice(0, 10)}.json`;
        a.click();
        URL.revokeObjectURL(url);
        setMsg("Exported.");
      })
      .catch((e) => setMsg(String(e)))
      .finally(() => setBusy(false));
  }

  function wipe() {
    if (!isTauri()) return;
    setBusy(true);
    setMsg("");
    api
      .wipeData()
      .then(() => setMsg("All local data wiped. Restart to begin fresh."))
      .catch((e) => setMsg(String(e)))
      .finally(() => { setBusy(false); setConfirming(false); });
  }

  if (!isTauri()) return null;

  return (
    <div className="card">
      <h3>Data &amp; Privacy</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        All your data is local. Export it any time, or wipe everything. Tokens live in the OS keychain and are
        never included in exports.
      </p>
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <button onClick={exportData} disabled={busy}>Export all data</button>
        {!confirming ? (
          <button onClick={() => setConfirming(true)} disabled={busy} style={{ color: "var(--danger, #f87171)" }}>
            Wipe all data…
          </button>
        ) : (
          <>
            <span style={{ fontSize: 12, color: "var(--danger, #f87171)" }}>This cannot be undone.</span>
            <button onClick={wipe} disabled={busy} style={{ color: "var(--danger, #f87171)" }}>Confirm wipe</button>
            <button onClick={() => setConfirming(false)} disabled={busy}>Cancel</button>
          </>
        )}
      </div>
      {msg && <p style={{ fontSize: 12, color: "var(--text-dim)", marginTop: 8 }}>{msg}</p>}
    </div>
  );
}

// Phase 7: read-only plugin panels. Each plugin is a JSON manifest in the
// app's plugins/ dir; panels call a whitelisted read-only tool and show the
// result. A plugin can never do more than the core read-only tools allow.
function PluginsPanel(): ReactNode {
  const [plugins, setPlugins] = useState<PluginView[] | null>(null);
  const [results, setResults] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!isTauri()) return;
    api.listPlugins().then(setPlugins).catch(() => setPlugins([]));
  }, []);

  function run(pluginId: string, idx: number) {
    const key = `${pluginId}:${idx}`;
    setResults((r) => ({ ...r, [key]: "Running…" }));
    api
      .runPluginPanel(pluginId, idx)
      .then((out) => {
        let pretty = out;
        try { pretty = JSON.stringify(JSON.parse(out), null, 2); } catch { /* leave as-is */ }
        setResults((r) => ({ ...r, [key]: pretty }));
      })
      .catch((e) => setResults((r) => ({ ...r, [key]: String(e) })));
  }

  if (!isTauri()) {
    return <div className="card"><p style={{ color: "var(--text-dim)" }}>Plugins load in the desktop shell.</p></div>;
  }
  if (!plugins) return <div className="card"><p style={{ color: "var(--text-dim)" }}>{t("common.loading")}</p></div>;

  return (
    <div className="card">
      <h3>Plugins <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· read-only</span></h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Drop a plugin manifest (<span className="mono">*.json</span>) in the app's <span className="mono">plugins/</span> folder.
        Each panel calls a whitelisted read-only tool — plugins can read and display, never act in-game.
      </p>
      {plugins.length === 0 && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>No plugins installed.</p>
      )}
      {plugins.map((p) => (
        <div key={p.manifest.id} style={{ borderTop: "1px solid var(--border)", paddingTop: 8, marginTop: 8 }}>
          <div style={{ display: "flex", gap: 8, alignItems: "baseline" }}>
            <strong>{p.manifest.name}</strong>
            {p.manifest.version && <span style={{ fontSize: 11, color: "var(--text-dim)" }}>v{p.manifest.version}</span>}
            {p.manifest.author && <span style={{ fontSize: 11, color: "var(--text-dim)" }}>· {p.manifest.author}</span>}
          </div>
          {p.manifest.description && (
            <div style={{ fontSize: 12, color: "var(--text-dim)" }}>{p.manifest.description}</div>
          )}
          {p.errors.length > 0 ? (
            <ul style={{ color: "var(--danger)", fontSize: 12 }}>
              {p.errors.map((e, i) => <li key={i}>{e}</li>)}
            </ul>
          ) : (
            p.manifest.panels.map((panel, idx) => {
              const key = `${p.manifest.id}:${idx}`;
              return (
                <div key={idx} style={{ marginTop: 6 }}>
                  <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                    <button style={{ fontSize: 11 }} onClick={() => run(p.manifest.id, idx)}>Run</button>
                    <span style={{ fontSize: 13 }}>{panel.title}</span>
                    <span className="mono" style={{ fontSize: 10, color: "var(--text-dim)" }}>{panel.command}</span>
                  </div>
                  {results[key] && (
                    <pre style={{ fontSize: 11, background: "rgba(10,14,22,0.5)", padding: 8, borderRadius: 6, overflowX: "auto", marginTop: 4 }}>
                      {results[key]}
                    </pre>
                  )}
                </div>
              );
            })
          )}
        </div>
      ))}
    </div>
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
  const [gate, setGate] = useState<FitGatekeeperView | null>(null);
  const [stats, setStats] = useState<FitStatsView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<SavedFitView[] | null>(null);
  const [kmUrl, setKmUrl] = useState("");
  const [compliance, setCompliance] = useState<DoctrineComplianceView | null>(null);
  const [gameFits, setGameFits] = useState<GameFitView[] | null>(null);

  function loadGameFits() {
    if (!isTauri() || !character) return;
    api.listGameFits(character.id).then(setGameFits).catch(() => setGameFits([]));
  }

  function pushToGame() {
    if (!isTauri() || !character || !eft.trim()) return;
    setError("Pushing fit to the in-game fitting window…");
    api
      .pushFitToGame(character.id, eft)
      .then(() => setError("Saved to your in-game fittings."))
      .catch((e) => setError(String(e)));
  }

  function fromKillmail() {
    if (!isTauri() || !kmUrl.trim()) return;
    setError("Reconstructing fit from killmail…");
    api
      .reconstructFit(kmUrl.trim())
      .then((r) => {
        setEft(r.eft);
        setError(`Reconstructed ${r.ship}${r.pilot ? ` (${r.pilot})` : ""} — parse or run stats to size it up.`);
      })
      .catch((e) => setError(String(e)));
  }

  function checkStats() {
    if (!isTauri() || !eft.trim()) return;
    setStats(null);
    api.fitStats(eft, character?.id ?? null).then(setStats).catch((e) => setError(String(e)));
  }

  function loadLibrary() {
    if (!isTauri()) return;
    api.listFits().then(setSaved).catch(() => setSaved([]));
  }

  function saveFit() {
    if (!isTauri() || !eft.trim()) return;
    const name = window.prompt("Save fit as:");
    if (!name) return;
    api.saveFit(name, eft).then(() => { setError(null); if (saved) loadLibrary(); }).catch((e) => setError(String(e)));
  }

  function parse() {
    setError(null);
    setFit(null);
    setCanFly(null);
    setDoctrine(null);
    setGate(null);
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

  function checkGate() {
    if (!character || !isTauri()) return;
    setGate(null);
    api.fitGatekeeper(character.id, eft).then(setGate).catch(() => setGate(null));
  }

  return (
    <div className="card fit-importer">
      <h3>Fitting · EFT Import</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Paste an EFT block (from PYFA or the in-game fitting window) to parse and resolve it.
      </p>
      <div style={{ display: "flex", gap: 8, marginBottom: 6 }}>
        <input
          value={kmUrl}
          onChange={(e) => setKmUrl(e.target.value)}
          placeholder="…or paste a zKillboard link to reconstruct the victim's fit"
          style={{ flex: 1, fontSize: 12 }}
        />
        <button onClick={fromKillmail} disabled={!kmUrl.trim()}>From killmail</button>
      </div>
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
        {character && (
          <button onClick={checkGate} disabled={!eft.trim()}>
            Own / cost check
          </button>
        )}
        <button onClick={checkDoctrine} disabled={!eft.trim()}>
          Check all pilots
        </button>
        <button onClick={checkStats} disabled={!eft.trim()}>Stats (EHP/DPS)</button>
        <button onClick={saveFit} disabled={!eft.trim()}>Save fit</button>
        <button onClick={() => (saved ? setSaved(null) : loadLibrary())}>
          {saved ? "Hide library" : "Library"}
        </button>
        {character && (
          <button onClick={pushToGame} disabled={!eft.trim()} title="Save this fit to your in-game fitting window (ESI write)">
            Push to game
          </button>
        )}
        {character && (
          <button onClick={() => (gameFits ? setGameFits(null) : loadGameFits())}>
            {gameFits ? "Hide game fits" : "Game fits"}
          </button>
        )}
      </div>
      {gameFits && (
        <div style={{ marginTop: 8 }}>
          {gameFits.length === 0 && (
            <span style={{ fontSize: 12, color: "var(--text-dim)" }}>
              No in-game fittings (or the fittings scope isn't granted yet — re-login to add it).
            </span>
          )}
          {gameFits.map((f) => (
            <div key={f.fitting_id} style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 3 }}>
              <button style={{ fontSize: 11 }} onClick={() => { setEft(f.eft); setGameFits(null); }}>Load</button>
              <span style={{ fontSize: 13 }}>{f.name}</span>
              <span style={{ fontSize: 11, color: "var(--text-dim)" }}>· {f.ship}</span>
              <button
                style={{ fontSize: 11, marginLeft: "auto" }}
                onClick={() => api.saveFit(f.name, f.eft).then(() => setError(`Imported "${f.name}" to your library.`)).catch((e) => setError(String(e)))}
              >
                Import to library
              </button>
            </div>
          ))}
        </div>
      )}
      {saved && (
        <div style={{ marginTop: 8 }}>
          {saved.length === 0 && <span style={{ fontSize: 12, color: "var(--text-dim)" }}>No saved fits yet.</span>}
          {saved.map((f) => (
            <div key={f.id} style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 3 }}>
              <button style={{ fontSize: 11 }} onClick={() => { setEft(f.eft); setSaved(null); }}>Load</button>
              <span style={{ fontSize: 13 }}>{f.name}</span>
              {f.ship && <span style={{ fontSize: 11, color: "var(--text-dim)" }}>· {f.ship}</span>}
              <button
                style={{ fontSize: 11 }}
                title="Which of your characters can fly this doctrine, and the training gap"
                onClick={() => api.doctrineCompliance(f.id).then(setCompliance).catch(() => setCompliance(null))}
              >
                Compliance
              </button>
              <button
                style={{ fontSize: 11, marginLeft: "auto" }}
                onClick={() =>
                  api.shareArtifact("fit", f.name, f.eft).then((codeStr) => {
                    navigator.clipboard?.writeText(codeStr);
                    setError(`Share code for "${f.name}" copied to clipboard.`);
                  }).catch((e) => setError(String(e)))
                }
              >
                Share
              </button>
              <button
                style={{ fontSize: 11 }}
                onClick={() => api.deleteFit(f.id).then(loadLibrary)}
              >
                Delete
              </button>
            </div>
          ))}
        </div>
      )}
      {compliance && (
        <div style={{ marginTop: 10 }}>
          <p style={{ fontSize: 12, margin: "0 0 4px" }}>
            <strong>Doctrine compliance:</strong> {compliance.fit_name}
            {compliance.ship ? ` (${compliance.ship})` : ""}
            <button style={{ fontSize: 11, marginLeft: 8 }} onClick={() => setCompliance(null)}>Hide</button>
          </p>
          <table className="holdings">
            <tbody>
              {compliance.rows.map((r) => (
                <tr key={r.character_id}>
                  <td>{r.character_name}</td>
                  <td>
                    {r.can_fly ? (
                      <span className="badge safe">can fly</span>
                    ) : (
                      <span className="badge caution">{r.missing_count} skill{r.missing_count === 1 ? "" : "s"} short</span>
                    )}
                  </td>
                  <td className="mono num">{r.can_fly ? "—" : formatDuration(r.train_seconds)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {stats && (
        <div style={{ marginTop: 10 }}>
          {!stats.found ? (
            <p style={{ color: "var(--text-dim)", fontSize: 12 }}>{stats.note}</p>
          ) : (
            <>
              <div className="cashflow-totals">
                <span className="pos">{Math.round(stats.total_ehp).toLocaleString()} EHP</span>
                <span>{Math.round(stats.dps).toLocaleString()} DPS</span>
                <span>{Math.round(stats.volley).toLocaleString()} volley</span>
                {(stats.shield_rps > 0 || stats.armor_rps > 0) && (
                  <span className="pos">
                    {Math.round(Math.max(stats.shield_rps, stats.armor_rps)).toLocaleString()} HP/s rep
                  </span>
                )}
                <span>{Math.round(stats.cap_peak_recharge * 10) / 10} GJ/s cap</span>
                {stats.cap_load > 0 && (
                  <span className={stats.cap_stable ? "pos" : "neg"}>
                    {stats.cap_stable
                      ? "cap stable"
                      : `cap unstable${stats.cap_seconds_to_empty ? ` · ${formatDuration(stats.cap_seconds_to_empty)}` : ""}`}
                  </span>
                )}
              </div>
              <table className="holdings" style={{ marginTop: 8 }}>
                <tbody>
                  <tr><td>Shield</td><td className="mono num">{Math.round(stats.shield_ehp).toLocaleString()} EHP</td></tr>
                  <tr><td>Armor</td><td className="mono num">{Math.round(stats.armor_ehp).toLocaleString()} EHP</td></tr>
                  <tr><td>Hull</td><td className="mono num">{Math.round(stats.hull_ehp).toLocaleString()} EHP</td></tr>
                  <tr><td>Capacitor</td><td className="mono num">{Math.round(stats.cap_capacity).toLocaleString()} GJ</td></tr>
                </tbody>
              </table>
              {stats.dps_curve.length > 1 && stats.dps_curve.some((p) => p.dps > 0) && (
                <DpsRangeChart curve={stats.dps_curve} />
              )}
              {stats.hull_bonuses.length > 0 && (
                <>
                  <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "8px 0 2px" }}>Hull bonuses</p>
                  <ul style={{ margin: 0, paddingLeft: 16, fontSize: 12 }}>
                    {stats.hull_bonuses.map((b, i) => (
                      <li key={i}>{b}</li>
                    ))}
                  </ul>
                </>
              )}
              <p style={{ color: "var(--text-dim)", fontSize: 11, marginTop: 6 }}>{stats.note}</p>
            </>
          )}
        </div>
      )}
      {gate && gate.parsed && (
        <div style={{ marginTop: 10 }}>
          <div className="cashflow-totals">
            <span className={gate.can_fly ? "pos" : "neg"}>{gate.can_fly ? "Can fly" : `${gate.missing_skills.length} skills short`}</span>
            <span>{(gate.owned_fraction * 100).toFixed(0)}% owned</span>
            <span className="neg">{ISK.format(gate.acquisition_cost)} to buy</span>
            <span>{ISK.format(gate.total_value)} value</span>
          </div>
          <table className="holdings" style={{ marginTop: 8 }}>
            <tbody>
              {gate.items.map((it) => (
                <tr key={it.type_id}>
                  <td>
                    {it.needed > 1 ? `${it.needed}× ` : ""}{it.name}
                  </td>
                  <td className="mono num">
                    {it.missing === 0 ? (
                      <span className="badge safe">owned</span>
                    ) : (
                      `need ${it.missing}`
                    )}
                  </td>
                  <td className="mono num neg">{it.missing_cost > 0 ? ISK.format(it.missing_cost) : ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
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

// Abyssal Deadspace run tracker: log each run (tier/weather/ship/fit, time,
// loot, survival) and see ISK/hr, survival rate, and a per-tier breakdown.
function AbyssTracker(): ReactNode {
  const [data, setData] = useState<AbyssTrackerView | null>(null);
  const [tier, setTier] = useState("4");
  const [weather, setWeather] = useState("Dark");
  const [ship, setShip] = useState("");
  const [fit, setFit] = useState("");
  const [mins, setMins] = useState("");
  const [loot, setLoot] = useState("");
  const [survived, setSurvived] = useState(true);
  const [notes, setNotes] = useState("");
  const [lootPaste, setLootPaste] = useState("");
  const [lootMsg, setLootMsg] = useState("");

  function valueLoot() {
    if (!isTauri() || !lootPaste.trim()) return;
    api.valueLoot(lootPaste).then((v) => {
      setLoot((v.total / 1_000_000).toFixed(2));
      const unres = v.unresolved.length ? `, ${v.unresolved.length} unpriced` : "";
      setLootMsg(`${v.lines.length} item(s) → ${ISK.format(v.total)}${unres}`);
    }).catch(() => setLootMsg("Could not value loot."));
  }

  function load() {
    if (!isTauri()) return;
    api.getAbyssTracker().then(setData).catch(() => setData(null));
  }
  useEffect(() => { load(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);

  function logRun() {
    if (!isTauri()) return;
    api
      .logAbyssRun({
        tier: Number(tier) || 0,
        weather,
        ship: ship.trim(),
        fit: fit.trim(),
        duration_seconds: Math.round((parseFloat(mins) || 0) * 60),
        loot_value: (parseFloat(loot) || 0) * 1_000_000,
        survived,
        notes: notes.trim(),
      })
      .then(() => { setShip(""); setFit(""); setMins(""); setLoot(""); setNotes(""); setSurvived(true); setLootPaste(""); setLootMsg(""); load(); })
      .catch(() => undefined);
  }

  if (!isTauri()) {
    return <div className="card"><p style={{ color: "var(--text-dim)" }}>The abyss tracker runs in the desktop shell.</p></div>;
  }

  const s = data?.stats;
  return (
    <>
      <div className="card">
        <h3>Abyss Tracker <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· log a run</span></h3>
        <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
          Record each abyssal run. Loot in millions of ISK, time in minutes.
        </p>
        <div style={{ display: "grid", gridTemplateColumns: "auto auto 1.3fr 1.3fr auto auto auto", gap: 6, alignItems: "end" }}>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Tier
            <select value={tier} onChange={(e) => setTier(e.target.value)}>
              {[1, 2, 3, 4, 5, 6].map((t) => <option key={t} value={String(t)}>T{t}</option>)}
            </select>
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Weather
            <select value={weather} onChange={(e) => setWeather(e.target.value)}>
              {["Dark", "Gamma", "Electrical", "Exotic", "Firestorm"].map((w) => <option key={w} value={w}>{w}</option>)}
            </select>
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Ship
            <input value={ship} onChange={(e) => setShip(e.target.value)} placeholder="Gila" />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Fit
            <input value={fit} onChange={(e) => setFit(e.target.value)} placeholder="T4 Gila" />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Min
            <input value={mins} onChange={(e) => setMins(e.target.value)} style={{ width: 50 }} />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
            Loot M
            <input value={loot} onChange={(e) => setLoot(e.target.value)} style={{ width: 60 }} />
          </label>
          <label style={{ fontSize: 11, color: "var(--text-dim)", display: "flex", gap: 4, alignItems: "center" }}>
            <input type="checkbox" checked={survived} onChange={(e) => setSurvived(e.target.checked)} />
            Survived
          </label>
        </div>
        <div style={{ marginTop: 8 }}>
          <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "0 0 2px" }}>
            Loot value — type it above, or paste the loot window (select all → copy) and value it:
          </p>
          <textarea
            value={lootPaste}
            onChange={(e) => setLootPaste(e.target.value)}
            placeholder={"Triglavian Survey Database\t3\nZero-Point Condensate\t12\n…"}
            style={{ width: "100%", minHeight: 56, fontFamily: "monospace", fontSize: 12 }}
          />
          <div style={{ display: "flex", gap: 8, marginTop: 4, alignItems: "center" }}>
            <button onClick={valueLoot} disabled={!lootPaste.trim()}>Value loot</button>
            {lootMsg && <span style={{ fontSize: 12, color: "var(--text-dim)" }}>{lootMsg}</span>}
          </div>
        </div>
        <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
          <input value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="notes (optional)" style={{ flex: 1 }} />
          <button onClick={logRun}>Log run</button>
        </div>
      </div>

      {s && s.runs > 0 && (
        <div className="card">
          <h3>Stats</h3>
          <div className="cashflow-totals">
            <span>{s.runs} runs</span>
            <span className="pos">{ISK.format(s.isk_per_hour)}/hr</span>
            <span>{ISK.format(s.avg_loot)} avg</span>
            <span>{formatDuration(s.avg_seconds)} avg</span>
            <span className={s.deaths > 0 ? "neg" : "pos"}>{(s.survival_rate * 100).toFixed(0)}% survived</span>
          </div>
          {s.by_tier.length > 0 && (
            <table className="holdings" style={{ marginTop: 8 }}>
              <thead>
                <tr>
                  <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Tier</th>
                  <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Runs</th>
                  <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Avg loot</th>
                  <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Avg time</th>
                </tr>
              </thead>
              <tbody>
                {s.by_tier.map((t) => (
                  <tr key={t.tier}>
                    <td>T{t.tier}</td>
                    <td className="mono num">{t.runs}</td>
                    <td className="mono num pos">{ISK.format(t.avg_loot)}</td>
                    <td className="mono num">{formatDuration(t.avg_seconds)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {data && data.runs.length > 0 && (
        <div className="card">
          <h3>Recent runs</h3>
          <table className="holdings">
            <tbody>
              {data.runs.slice(0, 30).map((r) => (
                <tr key={r.id} style={{ opacity: r.survived ? 1 : 0.6 }}>
                  <td>T{r.tier} {r.weather}</td>
                  <td>{r.ship}{r.fit ? ` · ${r.fit}` : ""}</td>
                  <td className="mono num">{formatDuration(r.duration_seconds)}</td>
                  <td className={"mono num" + (r.survived ? " pos" : " neg")}>
                    {r.survived ? ISK.format(r.loot_value) : "lost"}
                  </td>
                  <td style={{ textAlign: "right" }}>
                    <button style={{ fontSize: 11 }} onClick={() => api.deleteAbyssRun(r.id).then(load)}>✕</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}

function CombatHub({ character }: { character: Character | null }): ReactNode {
  const [sub, setSub] = useSubTab("fitting");
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
          { id: "abyss", label: "Abyss" },
        ]}
        active={sub}
        onSelect={setSub}
      />
      {sub === "fitting" && <FitImporter character={character} />}
      {sub === "dscan" && <DscanPanel />}
      {sub === "abyss" && <AbyssTracker />}
      {sub === "pve" && (
        <>
          <AgentFinder />
          <IncursionsPanel />
          <FactionWarfarePanel />
        </>
      )}
      {sub === "map" && (
        <>
          <SystemRiskGauge />
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
      {sub === "aar" && (
        <>
          <CombatLogPanel />
          <FleetAarPanel />
        </>
      )}
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

function riskColor(level: string): string {
  if (level === "Danger") return "#f87171";
  if (level === "Caution") return "#fbbf24";
  if (level === "Neutral") return "#60a5fa";
  return "#4ade80";
}

function SystemRiskGauge(): ReactNode {
  const [risk, setRisk] = useState<SystemRiskView | null>(null);
  const [loading, setLoading] = useState(false);

  function load() {
    if (!isTauri()) return;
    setLoading(true);
    api
      .getSystemRisk()
      .then(setRisk)
      .catch(() => setRisk(null))
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const color = risk ? riskColor(risk.level) : "var(--text-dim)";

  return (
    <div className="card">
      <h3>
        Unified Risk{" "}
        <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· current system</span>
      </h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        One threat number fusing in-system + neighbour kills, security band and gate-camp signal.
      </p>
      <button onClick={load} disabled={loading}>{loading ? "Loading…" : "Refresh"}</button>
      {risk && !risk.found && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
          Needs an active character in space (location scope).
        </p>
      )}
      {risk && risk.found && (
        <div style={{ marginTop: 10 }}>
          <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
            <span className="mono" style={{ fontSize: 34, fontWeight: 700, color }}>
              {risk.score}
            </span>
            <span style={{ fontSize: 13, color }}>{risk.level}</span>
            <span style={{ color: "var(--text-dim)", fontSize: 12 }}>· {risk.system_name}</span>
          </div>
          <div
            style={{
              height: 8,
              borderRadius: 4,
              background: "var(--border)",
              marginTop: 6,
              overflow: "hidden",
            }}
          >
            <div style={{ width: `${risk.score}%`, height: "100%", background: color }} />
          </div>
          <ul style={{ margin: "10px 0 0", paddingLeft: 16, fontSize: 12, color: "var(--text-dim)" }}>
            {risk.reasons.map((r, i) => (
              <li key={i}>{r}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
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
                <title>{`${n.name} · sec ${n.security.toFixed(1)}${n.sov_owner ? ` · ${n.sov_owner}` : ""}${n.adm > 0 ? ` · ADM ${n.adm.toFixed(1)}` : ""}${hot ? ` · ${n.kills} kills/hr` : ""}`}</title>
                {hot && <circle cx={p.x} cy={p.y} r={6 + Math.min(n.kills, 12)} fill="#f87171" fillOpacity={0.25} />}
                {n.sov_alliance_id !== 0 && (
                  <circle cx={p.x} cy={p.y} r={7} fill="none" stroke={admColor(n.adm)} strokeWidth={n.adm > 0 ? 1.5 : 1} strokeOpacity={0.8} />
                )}
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
        Node colour = security; red halo = ship kills/hr; sov ring = held sovereignty, coloured by ADM (green=strong, amber=weak; hover for owner/ADM).
      </p>
    </div>
  );
}

// Agent / mission finder: filter the SDE's mission agents by level, security
// band, and region. Needs an SDE rebuilt with agent data.
function AgentFinder(): ReactNode {
  const [rows, setRows] = useState<AgentFinderView[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [level, setLevel] = useState("4");
  const [sec, setSec] = useState("0.5");
  const [region, setRegion] = useState("");
  const [regions, setRegions] = useState<string[]>([]);

  useEffect(() => {
    if (!isTauri()) return;
    api.listMapRegions().then(setRegions).catch(() => setRegions([]));
  }, []);

  function load() {
    if (!isTauri()) return;
    setLoading(true);
    api
      .findAgents(
        level ? Number(level) : undefined,
        sec ? Number(sec) : undefined,
        region || undefined,
      )
      .then(setRows)
      .catch(() => setRows([]))
      .finally(() => setLoading(false));
  }

  return (
    <div className="card" style={{ maxWidth: 720 }}>
      <h3>Agent finder</h3>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
        <label style={{ fontSize: 12 }}>
          Level{" "}
          <select value={level} onChange={(e) => setLevel(e.target.value)}>
            <option value="">Any</option>
            {[1, 2, 3, 4, 5].map((l) => (
              <option key={l} value={l}>{l}</option>
            ))}
          </select>
        </label>
        <label style={{ fontSize: 12 }}>
          Security{" "}
          <select value={sec} onChange={(e) => setSec(e.target.value)}>
            <option value="">Any</option>
            <option value="0.5">High-sec (≥0.5)</option>
            <option value="0.0">Low/null (≥0.0)</option>
          </select>
        </label>
        <select value={region} onChange={(e) => setRegion(e.target.value)} style={{ fontSize: 12 }}>
          <option value="">Any region</option>
          {regions.map((r) => (
            <option key={r} value={r}>{r}</option>
          ))}
        </select>
        <button onClick={load} disabled={loading}>{loading ? "Searching…" : "Find"}</button>
      </div>
      {rows && rows.length === 0 && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
          No agents — rebuild the SDE with agent data (--agents/--stations-csv/--divisions).
        </p>
      )}
      {rows && rows.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <tbody>
            {rows.slice(0, 100).map((a, i) => (
              <tr key={i}>
                <td>
                  {a.corporation_name}
                  <div style={{ color: "var(--text-dim)", fontSize: 11 }}>
                    L{a.level} {a.division_name}{a.is_locator ? " · locator" : ""}
                  </div>
                </td>
                <td className="loc">
                  {a.system_name} <span style={{ color: secColor(a.security * 10) }}>{a.security.toFixed(1)}</span>
                  <div style={{ color: "var(--text-dim)", fontSize: 11 }}>{a.station_name}</div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
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

function FactionWarfarePanel(): ReactNode {
  const [rows, setRows] = useState<FwSystemView[] | null>(null);
  const [loading, setLoading] = useState(false);

  function load() {
    if (!isTauri()) return;
    setLoading(true);
    api
      .getFwSystems()
      .then(setRows)
      .catch(() => setRows([]))
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="card fw-panel" style={{ marginTop: 16 }}>
      <h3>Faction Warfare <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· contested</span></h3>
      <button onClick={load} disabled={loading}>{loading ? "Loading…" : "Refresh"}</button>
      {rows && rows.length === 0 && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>No contested systems.</p>
      )}
      {rows && rows.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>System</th>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Occupier</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Contested</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i}>
                <td>{r.system_name}</td>
                <td className="loc">{r.occupier}</td>
                <td className="mono num">{r.progress_pct.toFixed(0)}%</td>
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

// Combined multi-box fleet after-action report: merges the most recent Gamelogs
// (one per boxed character) into fleet totals + per-pilot DPS contributions.
function FleetAarPanel(): ReactNode {
  const [data, setData] = useState<FleetAarView | null>(null);
  const [loading, setLoading] = useState(false);

  function load() {
    if (!isTauri()) return;
    setLoading(true);
    api
      .getFleetAar(8)
      .then(setData)
      .catch(() => setData(null))
      .finally(() => setLoading(false));
  }

  const f = data?.fleet;
  return (
    <div className="card combat-log">
      <h3>Fleet AAR · Multi-box</h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Merges your recent Gamelogs into one fleet report — combined DPS and each pilot&apos;s share.
      </p>
      <button onClick={load} disabled={loading}>
        {loading ? "Reading…" : "Analyze fleet logs"}
      </button>
      {data && !data.found && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>
          No Gamelogs found — run multiple clients so each writes its own log.
        </p>
      )}
      {f && (
        <div style={{ marginTop: 10 }}>
          <div className="cashflow-totals">
            <span className="pos">{ISK.format(f.damage_dealt)} dealt · {f.dps_dealt.toFixed(0)} dps</span>
            <span className="neg">{ISK.format(f.damage_received)} taken · {f.dps_received.toFixed(0)} dps</span>
            <span style={{ color: "var(--text-dim)" }}>{formatDuration(f.duration_seconds)}</span>
          </div>
          <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "8px 0 2px" }}>
            Pilots ({f.pilots.length})
          </p>
          <table className="holdings">
            <tbody>
              {f.pilots.map((p, i) => (
                <tr key={p.name + i}>
                  <td>{p.name}</td>
                  <td className="mono num pos">{p.summary.dps_dealt.toFixed(0)} dps</td>
                  <td className="mono num">{ISK.format(p.summary.damage_dealt)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {f.top_targets.length > 0 && (
            <>
              <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "8px 0 2px" }}>Fleet top targets</p>
              <table className="holdings">
                <tbody>
                  {f.top_targets.map((t) => (
                    <tr key={t.entity}>
                      <td>{t.entity}</td>
                      <td className="mono num pos">{ISK.format(t.damage)}</td>
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
          {result.composition && (
            <p style={{ margin: "0 0 8px", fontSize: 13 }}>
              <span className="badge caution">gang</span> likely composition:{" "}
              <strong>{result.composition}</strong>
              <span style={{ color: "var(--text-dim)", fontSize: 11 }}> (from most-flown ships)</span>
            </p>
          )}
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
  const [lastScan, setLastScan] = useState("");
  const [diff, setDiff] = useState<DscanDiff | null>(null);

  function scan() {
    if (!isTauri()) return;
    api.parseDscan(text).then(setResult).catch(() => setResult(null));
    // Diff against the previous paste — the "what just landed" readout.
    if (lastScan && lastScan !== text) {
      api.dscanDiff(lastScan, text).then(setDiff).catch(() => setDiff(null));
    } else {
      setDiff(null);
    }
    setLastScan(text);
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
      {diff && (diff.appeared.length > 0 || diff.disappeared.length > 0) && (
        <div style={{ marginTop: 10, borderLeft: "2px solid var(--accent)", paddingLeft: 8 }}>
          <p style={{ fontSize: 12, margin: "0 0 4px", color: "var(--text-dim)" }}>Since last scan:</p>
          {diff.warnings.map((w, i) => (
            <p key={i} style={{ margin: "2px 0" }}><span className="badge danger">!</span> {w}</p>
          ))}
          {diff.appeared.map((g) => (
            <p key={`a-${g.type_name}`} style={{ margin: "2px 0", fontSize: 12 }}>
              <span className="neg">▲ +{g.count}</span> {g.type_name}
            </p>
          ))}
          {diff.disappeared.map((g) => (
            <p key={`d-${g.type_name}`} style={{ margin: "2px 0", fontSize: 12, color: "var(--text-dim)" }}>
              ▽ −{g.count} {g.type_name}
            </p>
          ))}
        </div>
      )}
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
                  <td><ItemHover name={g.type_name} /></td>
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
      <WormholeRoller />
      <SignatureTracker />
      <TheraCard />
      {character && <JumpFatigueCard character={character} />}
    </>
  );
}

// Cosmic-signature / wormhole chain tracker: paste the probe scanner, annotate
// wormhole connections (destination, mass, EOL), see your chain by system.
const MASS_STATES = ["stable", "destab", "critical"];

function SignatureTracker(): ReactNode {
  const [sigs, setSigs] = useState<SignatureView[] | null>(null);
  const [system, setSystem] = useState("");
  const [paste, setPaste] = useState("");

  function load() {
    if (!isTauri()) return;
    api.listSignatures().then(setSigs).catch(() => setSigs([]));
  }
  useEffect(() => { load(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);

  function doPaste() {
    if (!isTauri() || !system.trim() || !paste.trim()) return;
    api.pasteSignatures(system.trim(), paste).then(() => { setPaste(""); load(); }).catch(() => undefined);
  }

  function annotate(s: SignatureView) {
    const whType = window.prompt("Wormhole type (e.g. K162, C247):", s.wh_type) ?? s.wh_type;
    const dest = window.prompt("Destination (system / class):", s.destination) ?? s.destination;
    const mass = window.prompt("Mass state (stable / destab / critical):", s.mass_state) ?? s.mass_state;
    const eol = window.confirm("End of life (EOL)? OK = yes, Cancel = no");
    api.annotateSignature(s.id, whType, dest, MASS_STATES.includes(mass) ? mass : "stable", eol, s.notes).then(load);
  }

  if (!isTauri()) {
    return <div className="card" style={{ marginTop: 16 }}><p style={{ color: "var(--text-dim)" }}>The signature tracker runs in the desktop shell.</p></div>;
  }

  // Group by system.
  const bySystem: Record<string, SignatureView[]> = {};
  (sigs ?? []).forEach((s) => { (bySystem[s.system] ??= []).push(s); });

  return (
    <div className="card" style={{ marginTop: 16 }}>
      <h3>Signatures <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· chain tracker</span></h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Paste the in-game probe scanner (select all → copy) for a system; annotate wormholes with their
        destination, mass, and EOL.
      </p>
      <div style={{ display: "flex", gap: 8, alignItems: "end" }}>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          System
          <input value={system} onChange={(e) => setSystem(e.target.value)} placeholder="J100001 / Jita" />
        </label>
        <button onClick={doPaste} disabled={!system.trim() || !paste.trim()}>Add scan</button>
      </div>
      <textarea
        value={paste}
        onChange={(e) => setPaste(e.target.value)}
        placeholder={"ABC-123\tCosmic Signature\tWormhole\tUnstable Wormhole\t100%\t1.5 AU"}
        style={{ width: "100%", minHeight: 56, fontFamily: "monospace", fontSize: 12, marginTop: 6 }}
      />
      {Object.keys(bySystem).sort().map((sys) => (
        <div key={sys} style={{ marginTop: 12 }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <strong>{sys}</strong>
            <button style={{ fontSize: 11 }} onClick={() => api.clearSignatures(sys).then(load)}>Clear system</button>
          </div>
          <table className="holdings">
            <tbody>
              {bySystem[sys].map((s) => (
                <tr key={s.id}>
                  <td className="mono">{s.sig_id}</td>
                  <td style={{ fontSize: 12 }}>
                    {s.category || "—"}{s.name ? ` · ${s.name}` : ""}
                    {s.category === "Wormhole" && (s.destination || s.wh_type) && (
                      <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
                        {s.wh_type}{s.destination ? ` → ${s.destination}` : ""}
                        {s.mass_state !== "stable" ? ` · ${s.mass_state}` : ""}{s.eol ? " · EOL" : ""}
                      </div>
                    )}
                  </td>
                  <td style={{ textAlign: "right", whiteSpace: "nowrap" }}>
                    {s.category === "Wormhole" && (
                      <button style={{ fontSize: 11 }} onClick={() => annotate(s)}>Connect</button>
                    )}{" "}
                    <button style={{ fontSize: 11 }} onClick={() => api.deleteSignature(s.id).then(load)}>✕</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
    </div>
  );
}

// Wormhole rolling calculator: how many ship passes to walk a hole through its
// mass stages and collapse it, with the ±10% variance danger zone.
function WormholeRoller(): ReactNode {
  const [total, setTotal] = useState("2000");
  const [jump, setJump] = useState("300");
  const [pass, setPass] = useState("200");
  const [plan, setPlan] = useState<RollPlan | null>(null);

  function compute() {
    if (!isTauri()) return;
    api
      .rollWormhole((parseFloat(total) || 0) * 1e6, (parseFloat(jump) || 0) * 1e6, (parseFloat(pass) || 0) * 1e6)
      .then(setPlan)
      .catch(() => setPlan(null));
  }

  return (
    <div className="card" style={{ marginTop: 16 }}>
      <h3>Wormhole Roller <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· mass planning</span></h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Plan a roll. Masses in millions of kg (Mkg). Nominal hole mass carries ±10% variance.
      </p>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "end" }}>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Hole total
          <input value={total} onChange={(e) => setTotal(e.target.value)} style={{ width: 80 }} />
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Jump limit
          <input value={jump} onChange={(e) => setJump(e.target.value)} style={{ width: 80 }} />
        </label>
        <label style={{ fontSize: 11, color: "var(--text-dim)" }}>
          Ship/pass
          <input value={pass} onChange={(e) => setPass(e.target.value)} style={{ width: 80 }} />
        </label>
        <button onClick={compute}>Plan roll</button>
      </div>
      {plan && plan.warning && (
        <p style={{ color: "var(--danger, #f87171)", fontSize: 12, marginTop: 8 }}>{plan.warning}</p>
      )}
      {plan && !plan.warning && (
        <div style={{ marginTop: 10 }}>
          <div className="cashflow-totals">
            <span className="pos">{plan.safe_passes} safe passes</span>
            <span>reduced at {plan.passes_to_reduced}</span>
            <span className="neg">critical at {plan.passes_to_critical}</span>
          </div>
          <p style={{ fontSize: 12, marginTop: 8 }}>
            Roll freely for <strong>{plan.safe_passes}</strong> pass(es). From pass{" "}
            <strong>{plan.collapse_earliest}</strong> the hole may collapse; by pass{" "}
            <strong>{plan.collapse_latest}</strong> it definitely will. Split your last heavy ship and
            check the hole between passes inside that window so you don&apos;t get stranded.
          </p>
        </div>
      )}
    </div>
  );
}

function TheraCard(): ReactNode {
  const [rows, setRows] = useState<TheraConnection[] | null>(null);
  const [loading, setLoading] = useState(false);

  function load() {
    if (!isTauri()) return;
    setLoading(true);
    api
      .getTheraConnections()
      .then(setRows)
      .catch(() => setRows([]))
      .finally(() => setLoading(false));
  }

  return (
    <div className="card thera-card" style={{ marginTop: 16 }}>
      <h3>Thera / Turnur Connections <span style={{ color: "var(--text-dim)", fontWeight: 400 }}>· EVE-Scout</span></h3>
      <p style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 0 }}>
        Scanned wormhole shortcuts into known space, soonest to collapse first.
      </p>
      <button onClick={load} disabled={loading}>{loading ? "Loading…" : "Load connections"}</button>
      {rows && rows.length === 0 && (
        <p style={{ color: "var(--text-dim)", fontSize: 12 }}>No connections (or EVE-Scout unreachable).</p>
      )}
      {rows && rows.length > 0 && (
        <table className="holdings" style={{ marginTop: 10 }}>
          <thead>
            <tr>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>Destination</th>
              <th style={{ textAlign: "left", fontSize: 11, color: "var(--text-dim)" }}>From</th>
              <th style={{ textAlign: "right", fontSize: 11, color: "var(--text-dim)" }}>Ends</th>
            </tr>
          </thead>
          <tbody>
            {rows.slice(0, 40).map((c, i) => (
              <tr key={i}>
                <td>
                  {c.destination}
                  <div style={{ color: "var(--text-dim)", fontSize: 11 }}>
                    {c.region} · {c.max_ship_size}
                  </div>
                </td>
                <td className="loc">{c.hub}</td>
                <td className="mono num">{c.remaining_hours}h</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
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
          {result.suggested_reward > 0 && (
            <p style={{ margin: "4px 0", fontSize: 13 }}>
              Fair reward for this haul: <strong className="pos">{ISK.format(result.suggested_reward)} ISK</strong>
            </p>
          )}
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
