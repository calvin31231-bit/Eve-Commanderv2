# EVE Commander v2 — The Master EVE Online Companion

## Context

EVE Online's third-party ecosystem is powerful but **fragmented**: a serious player juggles 10+ tools — Adam4EVE/Fuzzwork (market), Ravworks (industry), PYFA (fitting), zKillboard (PvP), Pathfinder/Dotlan (mapping), EVEMon/EveLens (skills), Vintel (intel), SeAT/AllianceAuth (corp), plus PI/LP/abyssal/wallet utilities. Each is excellent in its lane; none unify the data, and constant tab-switching is the norm.

This project builds **one cross-platform desktop app** that consolidates the best ideas of every category *and* fills the gaps no single tool can — because only an all-in-one desktop app has your live ESI data for every character, your local game logs, the offline SDE, and third-party feeds in the same place at the same time.

The repo `/home/user/Eve-Commanderv2` is currently **empty** (clean git init on branch `claude/eloquent-ride-k5jp4v`). This is a greenfield build.

**Decisions locked with the user:** Desktop app · all playstyles · full vision + phased roadmap (start by implementing the Phase 0 foundation).

**Ecosystem coverage** was cross-checked against the EVE University wiki Third-party tools page, the `devfleet/awesome-eve` index (now archived June 2025; CCP has a semi-official successor), and `evecourier.com`. That pass added the modules flagged ★ below (Mining, PvE content, SRP, Recruitment/HR, Courier/Hauling logistics, Insurance/Reprocessing, Fit-sharing & fleet-sim) which the first survey under-weighted.

### Hard constraint — strict EULA / Third-Party Developer License compliance
This is a **read / display / analysis layer only**. Allowed and built-in: ESI REST API via EVE SSO OAuth2, passive *reading* of local EVE log files, clipboard D-scan paste, ESI route/waypoint set. **Forbidden and never implemented:** input automation/macros, memory reading, disallowed cache scraping, any in-game command injection or overlay-on-game, market-order automation, anything that accelerates progression beyond normal play. The compliance line is documented in `docs/EULA-notes.md` and every feature is gated against it in review.

---

## Tech Stack

- **Shell:** Tauri 2.x (Rust backend + WebView). Chosen over Electron for tiny footprint (matters running beside a heavy game client), a real multi-threaded Rust backend ideal for concurrent multi-character ESI polling + continuous log tailing + SDE joins, and clean OS-keychain token storage.
- **Frontend:** React 18 + TypeScript + Vite; Tailwind + shadcn/ui (Radix) for a dense dark EVE-style UI; TanStack Table + TanStack Virtual (asset/market/kill lists are tens of thousands of rows); ECharts + visx for charts; TanStack Router.
- **State:** TanStack Query as the backbone (every ESI dataset is a cached query with `staleTime` tied to ESI cache headers); Zustand for UI/active-character state; Tauri events (`emit`/`listen`) for backend→frontend push (live log lines, poll completions, alerts).
- **Backend language:** Rust for everything — `reqwest` (HTTP), `oauth2` (PKCE), `governor` (rate limiter), `keyring` (token store), `notify` (log watcher), `tokio` (scheduler), `sqlx` (DB). All business logic (optimizers, route solver, dogma/fitting math, risk scoring) lives in testable Rust `domain/` modules.
- **Local DB:** SQLite (WAL mode, FTS5 for name search), three logical stores: `sde.sqlite` (static, prebuilt, hot-swappable), `cache.sqlite` (ESI responses + ETag/expiry, disposable), `app.sqlite` (characters, settings, watchlists, fits, plans, historical snapshots — the durable store).
- **Type safety:** `ts-rs` generates TypeScript types from Rust structs so the IPC boundary is end-to-end typed.
- **AI (optional, local-first):** an OpenAI-compatible LLM client (works with Ollama / LM Studio / llama.cpp / vLLM), `sqlite-vec` for local RAG + a tiered AI **memory store** — durable memory as an **Obsidian-compatible Markdown vault** (graph + vector), high-churn data in the relational DB — with time-series rollups, plus an MCP server/client bridge. Off by default; see the AI Layer + Memory sections.

---

## Architecture

Single OS process; concurrency via tokio tasks (no Electron-style main/renderer/worker split). Rust core domains: **Auth** (SSO/PKCE + keychain), **ESI Service** (per-character poll scheduler), **Rate Limiter** (error-budget circuit breaker), **Cache** (ETag/expiry-aware), **SDE** (ingest + query), **Log Tailer** (fs-watch per char log dir), **Notifier** (tray + Discord webhook + rule engine), **Analytics** (optimizers/solvers/scoring), **DB** (sqlx pools).

Key mechanics:
- **ESI polling is cache-first.** Check `cache.sqlite` before any request; if fresh, serve it; if stale-with-ETag, send `If-None-Match` (a `304` is cheap and costs no error budget); never poll faster than the endpoint's `Expires`. ESI is pull-only (no webhooks), so "real-time" = poll at the tightest allowed cadence. See **Data Freshness vs. Resource Budget** for the tiered cadence, idle scaling, and local-countdown strategy that keeps this light.
- **Rate limiting tracks the error budget**, not RPS: watch `X-Esi-Error-Limit-Remain`/`-Reset`, trip a circuit breaker when low, cap global in-flight (~20–30) with jitter, always send a descriptive `User-Agent` (CCP requirement).
- **Auth:** OAuth2 Authorization Code + **PKCE** (no embedded secret), loopback redirect captured by Tauri's local server; store **refresh tokens only** in the OS keychain, access tokens in memory; **incremental per-feature scope consent**.
- **SDE:** ship a prebuilt version-pinned `sde.sqlite` in the installer (converted from CCP's YAML/CSV in CI via `sde-tools/`); check a manifest on launch and hot-swap deltas out-of-band; never block startup.
- **Log tailing (desktop-only moat):** watch `~/Documents/EVE/logs/Chatlogs` & `/Gamelogs` (user-overridable path), parse incrementally → Chatlogs feed intel/Local tracking, Gamelogs feed DPS/AAR. Passive reads only.
- **Multi-character model:** Character is first-class (own tokens/scopes/schedule); every view supports single-char vs all-chars; user-defined **Groups/"Hats"** (e.g. "industry stable", "scout fleet") drive cross-character aggregations.

---

## Data Freshness vs. Resource Budget (fresh data without bricking the machine)

The hard rule everything else follows: **ESI publishes a cache timer per endpoint, and polling faster than it returns identical data for wasted CPU/network.** So the maximum *useful* refresh rate is already defined by CCP — we never beat it; we just schedule intelligently underneath it. Representative timers: location/ship ~5s, online 60s, skills/skillqueue & wallet balance ~120s, industry jobs ~300s, region market orders ~300s, character market orders ~1200s, assets/journal/transactions/contracts/clones/structures ~3600s, market history ~86400s.

**1. Tiered poll classes (each endpoint placed by its cache timer):**
- *On-demand only* (don't background-poll): location/ship/online — fetched only while a view that needs them is open.
- *Fast (~1–2 min):* skills, wallet balance, industry jobs, mail headers.
- *Medium (~5–20 min):* market orders, contracts.
- *Slow (~1 hr):* assets, journal, transactions, clones, structures, sov.
- *Daily:* market history, SDE manifest check.

**2. Derive live state locally instead of polling.** This is the biggest win: most "live" numbers are deterministic once fetched. Fetch the skill-queue *end time*, structure *fuel-expiry*, job *completion time*, or order *expiry* **once**, then **count down client-side** with zero further requests until the cache window actually rolls. A dashboard full of ticking timers can run on almost no network.

**3. Push, not poll, where a stream exists.** The local-system killfeed/Threat-Scanner uses zKillboard **RedisQ/websocket** (an event stream) — no ESI hammering for the always-on safety data.

**4. Active vs. idle scaling.** The foreground/selected character polls at full cadence; background alts run slower (×2–4) on a round-robin. When the **app is minimized or the machine idle**, drop to "essentials only" (safety, alerts, training countdowns); on battery, throttle further. Stop all per-character polling for **offline** characters.

**5. Chunking & smoothing the work.** A single global scheduler queue drains *due* jobs through a concurrency cap (~8–16 in-flight) with jitter, **round-robin across characters**, so no tick is a spike — instead of firing 12 alts × 15 endpoints simultaneously. Paginated datasets (assets, market) fetch pages with bounded concurrency and process incrementally. Heavy parsing/analytics run on Rust background threads (tokio blocking pool / rayon), streaming results to a virtualized UI; DB writes are batched in WAL transactions.

**6. Coalesce shared data across alts.** Public/shared data (region market orders, structure info, SDE, name resolution) is fetched **once and shared** by all characters, not re-fetched per alt.

**7. Conditional requests everywhere.** ETag `If-None-Match` → `304` keeps "refreshes" nearly free and off the error budget.

**8. User-facing intensity profile.** A **Light / Balanced / Aggressive** data-freshness setting (auto-tunable by machine/battery): Light polls only foreground essentials and leans on on-demand fetches; Aggressive keeps everything near cache cadence. A small local meter shows current request rate, error-budget headroom, and resource use so power users can tune.

Net effect: continuous, *honest* freshness (never staler than CCP's own data) at a tiny, smoothed resource footprint — the Tauri/Rust stack and local-countdown derivation are what make this feasible where an Electron poller would struggle.

---

## UI / UX Strategy (taming 25+ modules without overwhelming anyone)

**Guiding principle: progressive disclosure + role-based surfaces.** The app never shows everything at once. Depth is always *reachable* but never *forced*. A casual player sees a calm, guided surface; a hardcore player unlocks a dense, keyboard-driven cockpit — same app, same data, different density.

**1. Information architecture — group by player intent, not by ESI scope.** The 25+ modules collapse into ~7 top-level **hubs** on a slim, collapsible left icon-rail: **Home/Overview · Character · Economy** (market, industry, reactions, PI, mining) · **Combat & Intel** (killboard, intel map, D-scan, AAR, fitting) · **Navigation & Logistics** (routes, capital/JF, courier, bookmarks) · **Corp & Fleet** (members, SRP, structures, fleet boss, recruitment) · **Tools** (calculators: reprocessing, insurance, LP, abyssal). Max two nav levels — no deep trees.

**2. Adaptive home dashboard — configurable widget cards.** A drag-grid of cards (à la a trading/DevOps dashboard) surfacing *what matters now*: training queue, wallet/net-worth trend, active jobs, intel alerts, structure-fuel runways, the "what should I do?" optimizer summary. Widgets are add/remove/rearrange; defaults vary by playstyle.

**3. Role presets + a Simple↔Advanced toggle — the casual/hardcore solution.** First-run picks a profile (Explorer / Industrialist / Trader / PvPer / FC-Director / Everything) that sets prominent hubs, default widgets, and density. A global **complexity toggle** hides advanced columns/settings in Simple and unlocks all in Advanced. Nothing is ever mandatory; everything is discoverable.

**4. Command palette (Ctrl/⌘-K) + universal search.** Fuzzy-jump to any module, run any calc, look up any item/system/character/corp. Power users bypass the chrome entirely; casual users get one obvious search bar. Keeps the UI minimal for both.

**5. Visual language — "sci-fi cockpit meets Bloomberg terminal."** Dark, high-contrast HUD aesthetic that *nods* to EVE's blue-cyan neocom UI without skinning it — restrained, not gamer-RGB. Built on a strict design system (Tailwind tokens + shadcn/Radix): one tight palette (neutrals + 1–2 accents + semantic status colors), consistent type/spacing scale, unified iconography. Data density is tamed with strong typographic hierarchy, virtualized/zebra tables, inline sparklines, and a user **density choice** (comfortable/compact). Light theme + colorblind-safe palettes + scalable fonts for accessibility.

**6. Ruthless consistency & shared primitives — every module feels identical.** One reusable data-table (sort/filter/column-pick/export), entity **hover-cards** everywhere (hover any item→price/attrs, any system→sec/kills/route, any character→killboard/standings), standard value formatters (ISK/time/volume), a universal detail **drawer**, and a fixed action vocabulary (Open in game · Add to plan · Pin · Copy · Export).

**7. Deep cross-linking — the payoff of one app.** Everything is clickable into everything: assets→market→what-it-builds; map system→intel+your-assets-there+route; killmail→fit→counter-fit. A persistent **context bar** shows the active character (location, online, wallet). Power users **pin/split panes** (market + industry side-by-side) and **pop panels into separate OS windows** (Tauri multiwindow — e.g. the intel map on a second monitor).

**8. Onboarding & calm guidance.** First-run wizard (log in → pick profile → optional tour), teaching **empty states** ("Connect a character to see assets"), inline tooltips, and a feature-discovery surface. Casual users are guided; experts skip it all.

**9. Feels-fast + calm notifications.** Skeleton loaders, optimistic UI, subtle "updated 2m ago" freshness chips (honest about ESI cache timing) instead of blocking spinners. A single notification center + tray with severity tiers that *collect* rather than interrupt — calm technology, not alert spam.

### Lessons from the in-game UI (mirror what players already trust)
EVE's own client already solves "lots of info, must stay glanceable," and players have deep muscle memory for it — we echo its patterns rather than reinvent:
- **Neocom** (left rail launcher with the portrait + an always-visible skill-training progress bar) → our left hub-rail with a persistent training indicator.
- **Local chat** (member count + standing-colored skulls; a fast-rising count = "spiking" = the #1 danger signal) → a permanent safety surface; "spike" detection when logs are available.
- **Overview** (sortable object list where danger is shown via full-row **background color + blink**, not subtle icons) → our threat tables use status-color rows and blink for hostiles, copying a convention players already read instinctively.
- **Watchlist** (fleet/friends online + location/distance) → a docked fleet-proximity panel.
- **HUD / D-scan / Selected Item** (capacitor + on-demand directional scan + current-target context) → D-scan paste and combat panels are quick-access but on-demand.

### Competitor UI — strengths to steal, weaknesses to avoid
| Tool | Strength to adopt | Weakness to avoid |
|---|---|---|
| Pathfinder | Clean, modern, **real-time collaborative** map; in-app manual | Single-purpose (WH only); web-only (no logs); setup friction |
| zKillboard | Unmatched **data depth**; canonical PvP intel | Utilitarian/cluttered; intimidating; not task-oriented |
| Dotlan | **Authoritative** maps/routing/stats | Dated, static, dense UI |
| PYFA | Deep sim; **all fits saved & browsable**; graphs | Aging wx-widgets desktop look; steep first-use curve |
| EVEMon | Thorough long-term planning | Legacy Windows look; aging |
| jEveAssets | Powerful filters across all assets | **Spreadsheet overload**; overwhelming; Java-desktop feel |
| EVE OS / CapsuleerKit | Modern, consolidated, clean | Web (can't read logs); breadth sometimes shallow |
| Vintel / IntelPy | **Live chat-log intel → map highlight** | Single-purpose; utilitarian |

Pattern: keep their depth, but reject dated desktop toolkits, spreadsheet dumps, single-purpose silos, and intimidating density with no progressive disclosure. Our desktop+log advantage directly answers the biggest shared weakness of the modern web tools.

### Foreground vs. tabbed — the data taxonomy (the heart of this answer)
Two persistent, always-on surfaces frame every view (modeled on the game's ever-present Local + Neocom + Watchlist), so safety- and time-critical data never hides behind a tab:

**1. Persistent top context bar (every screen):** active character · current system + security · live **system safety score** (kills in system + adjacent systems last hour, known gate-camp flag, local-spike indicator from logs) · skill-training countdown · ESI freshness/connection dot.

**2. Collapsible right-hand "Situational Awareness" rail (docked, present in all hubs, pop-out to a 2nd monitor):**
- **Local-system killfeed / safety tracker** — live kills in your system and neighbors, color-coded by recency/proximity (the example you raised; this is the always-valuable safety data).
- **Local Threat Scanner** — copy Local (or paste names) → per-pilot Safe/Pirate/Danger flags from zKillboard (see Flagship Feature section).
- **Fleet proximity panel** — fleet members, their system, **jumps-from-you**, ship, online status (game-watchlist parity).
- **Critical alerts** — structure low-fuel countdowns, job completions, wallet thresholds, intel pings — collected, severity-tiered, non-interrupting.

**Always foreground:** active char/location, system safety + killfeed, fleet proximity, training progress, critical alerts, connection/freshness.
**Tabbed / on-demand (rich but not constantly needed):** market browser, industry/BOM planner, fitting, killboard deep-dives, full maps, corp management, PI, LP/abyssal/reprocessing calculators, accounting, AAR archive.

This gives a casual ratter the same at-a-glance safety net the game provides, while an industrialist three hubs deep still sees a hostile spike or a fuel timer the instant it matters.

**Frontend libraries supporting this:** shadcn/ui + Radix + Tailwind (design system), TanStack Table + Virtual (grids), `cmdk` (command palette), `dnd-kit` (dashboard grid), Tauri multiwindow (pop-outs), Framer Motion (subtle transitions), ECharts/visx (viz). These were already chosen in the Tech Stack for exactly this reason.

---

## Feature Modules (consolidating the ecosystem)

Each maps to a `src/features/<name>` folder + Rust `domain`/`endpoints`. Source legend: ESI (live per-char) · SDE (static) · Logs (local) · 3P (third-party API).

| Module | Matches / beats | Sources |
|---|---|---|
| Auth & Characters | — | ESI |
| Skills & Training | EVEMon / EveLens / SkillQ | ESI + SDE |
| Wallet & Accounting | EWA / ECT | ESI |
| Assets & Inventory | ECT / jEveAssets | ESI + SDE + market |
| Market & Trading | Adam4EVE, Fuzzwork, evetrade, EVE Tycoon | ESI + SDE + 3P |
| Industry / Reactions / Invention | Ravworks / ISK Per Hour / Eve Cost | ESI + SDE + market |
| ★ Mining (ledger, yield, fleet, moons) | jEveAssets ledger | ESI + SDE |
| ★ Reprocessing / Refining / Insurance calc | Adam4EVE, Insurance Fraud | SDE + market |
| Fitting (dogma engine, EFT import/export) | PYFA | SDE + ESI skills |
| ★ Fit-sharing & Fleet-composition simulator | EVE Workbench, EVEShip.fit, Eve Fleet Simulator | SDE + community |
| PI Planning | EVE-HUB / EvePiTool | ESI + SDE |
| Killboards / PvP Analytics | zKillboard / EVE-KILL | 3P + ESI + SDE |
| Mapping & Navigation (route, sov/ADM, WH chains+mass, Thera) | Pathfinder, Dotlan, Eveeye, EVE-Scout, anoik.is | ESI + SDE + 3P |
| ★ Courier / Hauling logistics (route, gatecamp check, reward/collateral calc, marketplaces) | evecourier, Red Frog/PushX/GHSOL | ESI + SDE + 3P |
| Intel / Local / D-scan / gate-camp / char background-check | Vintel, EveWho, Gate Camp Check, PySpy | Logs + clipboard + SDE + 3P |
| Combat / DPS / AAR | PyEveLiveDPS | Logs + SDE |
| ★ PvE content (missions/agent finder, incursions, faction warfare, abyssal run stats) | EVEMissioneer, Abyss Tracker | ESI + SDE + 3P |
| Corp / Alliance (roles, structures+timerboards, moons, standings) | SeAT / AllianceAuth / Upwell.gg | ESI corp endpoints |
| ★ SRP (ship replacement) & ★ Recruitment/HR | EVE-SRP, Eve-HR | ESI + app |
| LP Store Optimizer | Fuzzwork LP | ESI + SDE + market |
| Abyssal / Mutaplasmid valuation | MutaMarket / Abyssal Market | 3P + ESI |
| Contracts | — | ESI + SDE |
| Calendar / events / standings-grinding | JitaCalendar, USIA | ESI + SDE |
| Notifications & Rules (tray + Discord webhook) | Reconbot / ThunderED | ESI + Logs |

### ESI scope completeness — additional scope-backed features (☆)
A pass over the full `esi-*` scope list surfaced these capabilities not yet represented above; each is added to the relevant module/phase:
- ☆ **Fleet boss management** (`esi-fleets` read/**write**) — live fleet/wing/squad composition, member positions & ships, MOTD, move members, broadcasts. (Phase 5; complements the log-based AAR in Phase 4.) *write_fleet is ESI-sanctioned, not input automation.*
- ☆ **EVEmail client** (`esi-mail` read/send/organize) — full in-app mail, not just notification reads. (Phase 1.)
- ☆ **In-game UI bridge** (`esi-ui` open_window / write_waypoint) — "open in game" for market/info/contract windows and set autopilot waypoint. The one EULA-sanctioned write path to the client. (Phase 5, woven through all modules as action buttons.)
- ☆ **In-game fitting sync** (`esi-fittings` read/write) — pull/push saved fits to the game fitting window. (Phase 3.)
- ☆ **Bookmark manager** (`esi-bookmarks`) — personal/corp bookmarks for nav/exploration/safespots. (Phase 5.)
- ☆ **Jump fatigue tracker** (`read_fatigue`) — capital-pilot fatigue/timers feeding navigation. (Phase 5.)
- ☆ **R&D agents / datacores** (`read_agents_research`) — passive-income datacore accumulation. (Phase 2.)
- ☆ **Contacts & standings editor** (`read/write_contacts`, char/corp/alliance) — manage contacts, not just display intel. (Phase 4.)
- ☆ **Corp security & vetting** (`membertracking`, `container_logs`) — last login/location/ship per member, theft detection from audit logs. (Phase 5, Corp module.)
- ☆ Minor: medals (`read_medals`), calendar event responses (`respond_calendar_events`), POCO/customs offices (`read_customs_offices`), opportunities — folded into Corp/PI/Calendar modules.

Coverage check: every authenticated `esi-*` scope now maps to at least one module. Scopes are still requested **incrementally per feature** (never all at once).

## The "10x" differentiators (Phase 6 — the reason this beats everything)

Only possible because one app holds all characters' live data + logs + SDE + 3P together (**all shipped** — pure logic in `eve-core`, commands in `src-tauri`, UI cards in `src/hubs.tsx`):
1. ✅ **Cross-content income optimizer** — ranks every income activity by realistic ISK/hr *for your specific* skills/assets/location/standings/time. (`income::rank_income`, Tools→Income Optimizer)
2. ✅ **Unified risk/intel score** — one live threat number per system fusing Local reds (logs) + their zKill history + standings + recent kills + your route/hull. (`intel::score_system_risk`, Intel→Map gauge)
3. ✅ **Hauling profit optimizer** — multi-hub arbitrage solved with your real cargo/collateral and route gate-camp risk (#2). (`marketdata::best_arbitrage`, Economy→Market)
4. ✅ **Portfolio analytics over time** — true net-worth time-series and P&L attribution, built from persisted local snapshots (ESI has no history). (`db::snapshots` + `get_portfolio_history`)
5. ✅ **Real-time fleet AAR / multi-box console** — combined live DPS + logi view from multiple Gamelogs, auto after-action report when a fight ends. (`gamelog::merge_fleet_aar`, Combat→AAR)
6. ✅ **"Can-I / Should-I" fit gatekeeper** — for any EFT fit: who can fly it now, train time, do I own it, acquisition cost, risk-adjusted value. (`fit_gatekeeper` command)
7. ✅ **Skill-plan ROI** — rank skill plans by ISK-impact, not just training time. (`skillplan::rank_roi`, Skills→Plan ROI)

**Phase 6 complete.** Next: Phase 6.5 (AI Jarvis layer) or Phase 7 (polish/plugins/localization).

---

## Expansion & Fortification Backlog (to be the most comprehensive tool, period)

A living backlog beyond the core modules. Items are tagged for where they slot in. Sourced from a second tool sweep (Compass, EveReactor/`rr.harkayn.ovh`-class reaction calculators, EVE Workbench, Eve Fleet Simulator, Nakamura-labs fatigue, jambeeno) and the documented player wishlist.

**A. Tools to fully absorb / level up**
- **Capital & JF logistics (Compass-class)** — Dijkstra jump routing across cyno-able systems, cyno-alt assignment per midpoint, multi-freighter fleet optimizer, live fuel + fatigue. (Elevate Courier/Nav into a full *Capital Logistics* module.)
- **Reaction-chain optimizer (EveReactor / `rr.harkayn.ovh` / maullerz-class)** — composite/hybrid/biochem reaction trees with live prices, rigs, structure & system-cost-index bonuses, build-vs-buy per tier. (Deepen Industry/Reactions.)
- **Full BOM tree** — capital/supercapital component planner that fuses manufacturing + reactions + PI + invention into one multi-level shopping list with per-alt job-slot/time allocation.
- **Doctrine & fleet-sim (Eve Fleet Simulator / Workbench)** — whole-fleet engagement sim, doctrine library + compliance checker (who in corp can fly the doctrine, who's missing what).

**B. Player-wishlist features (things players asked for that don't exist well)**
- **Corp loyalty-point engine** — assign internal points for tax/donations/PvP participation, redeemable against a corp-run store. (Corp engagement module.)
- **Alliance Tournament / competitive manager** — pilots register skills/ships/prefs; auto-allocate to team comps (replaces the Google-Sheets workflow alliances complain about).
- **Attribute remap optimizer** — paste a skill queue/plan → optimal neural remaps + implant advice for fastest training.
- **"Amazon for EVE" buy-&-deliver flow** — item search → best-price source → auto-generate buy list + courier contract to your doorstep (market + hauling + contracts fused; manual confirm, no automation).

**C. Platform-level fortifications (turn the app into a platform)**
- **Encrypted cloud sync & backup** (opt-in) of local data across machines; **importers** from EVEMon, PYFA, jEveAssets, EFT.
- **Mobile companion** (read-only push: skill done, low fuel, intel spike, wallet, structure timer) paired to the desktop core.
- **Plugin/extension API + user scripting** (custom columns, formulas, community modules) and a **public read API / outbound webhooks** so other tools integrate with *us*.
- **AI/LLM assistant layer** — natural-language queries over your own data ("most profitable build right now?", "is this route safe?"), fit suggestions, market-anomaly and theft detection. Major differentiator.
- **Streamer/OBS browser-source widgets** (your app's data, not a game overlay — EULA-safe) and a **read-only Discord slash-command bot**.
- **Multi-channel notifications** (tray, Discord, Telegram, ntfy, email, push) with an **anomaly/rules engine**; **shared self-hosted backend** mode for corps.
- **Historical data warehouse** — because we persist snapshots, offer trend analytics no web tool can (net-worth velocity, market forecasting, MER-style personal economy dashboard).
- **Theming, full localization, accessibility.**

**D. Per-module depth (fortify what exists)**
- *Market:* all-hub arbitrage, broker/tax modeling, FIFO profit accounting, price-spike & manipulation alerts, history forecasting.
- *Fitting:* full dogma (links/boosts/drugs/implants/WH effects), cap stability, resist/EHP-by-damage-type, DPS-vs-range/transversal graphs, **killmail→enemy-fit reconstruction + counter-fit suggestions**.
- *Intel/Nav:* kill-density heatmap routing, gank-hotspot avoidance (Uedama-class chokepoints), blops/hunter & cyno detection from logs, hostile-fleet composition estimation.
- *Wormhole:* mass/EOL tracking, rolling calculator, signature/wandering-connection management.
- *PvE/territory:* FW frontline & advantage tracker, incursion live-spawn tracker w/ community ISK/hr, sov/ADM campaign aggregation across alliances.
- *PI:* extractor-cycle reminders, profit-ranked planet/heatmap finder.

These feed Phases 6–7 and post-1.0; none are required for the MVP but each moves the needle toward "most comprehensive."

---

## AI Layer — "The Jarvis" (optional for players, local-first agent mesh for power users)

**Design tension solved:** AI is **off by default and the app is 100% functional without it** (no forced model downloads, no friction for casual players), while power users can wire **locally-run models into every aspect**. The architecture makes this natural: the Rust backend already exposes every capability as typed commands over a local data warehouse, so those commands *become the tool surface* an LLM drives. **The AI orchestrates and explains; deterministic Rust does the math** — the model is never asked to compute ISK, it calls our optimizer/pricer/router and narrates the result.

### Three setup tiers (a friendly wizard, all skippable)
1. **None (default)** — full app, no AI.
2. **Local AI (your path)** — point at any **OpenAI-compatible endpoint**: Ollama, LM Studio, llama.cpp/`llama-server`, vLLM, Jan, text-generation-webui. The app **auto-detects** a running Ollama/LM Studio and lists local models; one setting = base URL + model. An optional one-click helper installs Ollama + pulls a recommended model for non-technical users — entirely opt-in. **Privacy is the headline: with a local model, your wallet/assets/intel never leave the machine** (matches our "all data local by default" stance).
3. **Cloud (BYO key)** — Anthropic/OpenAI/etc. for users who prefer it.

Model **routing by task**: a small fast local model for classification/summaries, a larger local model for reasoning; user-configurable. Token use stays low because agents receive **structured tool results, not raw tables**.

### Agent architecture
- **AI gateway** in Rust exposes a single **tool registry** (our existing Tauri commands + `domain/` functions: query assets, run income optimizer, price a fit, check route safety, draft a mail, set an ESI waypoint, …). One registry, shared by all agents, via standard function-calling.
- **Two-way MCP:** the app ships as an **MCP server** so the user's *own* local agents (e.g. Claude Desktop, a local agent runtime) can drive EVE Commander's tools — and as an **MCP client** so it can consume the user's other tools. This is how it becomes *your* Jarvis, not a walled garden.
- **Local RAG** over `sde.sqlite` + game knowledge + the user's own history using `sqlite-vec` (already noted in Tech Stack) + a local embeddings model — grounded answers without huge context.
- **Optional voice** for the true Jarvis feel: `whisper.cpp` (STT) + Piper (TTS), fully local. Opt-in.

### The agent mesh — AI on every aspect
A single **Commander (orchestrator)** is the natural-language front door; it routes to domain specialists, each scoped to its module's tools:
- **Market Analyst** — price checks, arbitrage, spike/manipulation detection, "buy now or wait?"
- **Production Advisor** — build-vs-buy, optimal jobs across alts, reaction chains, "what's profitable with my BPOs right now?"
- **Fitting & Doctrine Coach** — suggest/critique fits, reconstruct a counter-fit from a killmail, who-can-fly, EFT in/out.
- **Threat/Intel Analyst** — fuse local+D-scan+killboard+standings into "is it safe to undock?", profile a hostile, estimate gang intent.
- **Navigation & Logistics Planner** — safe-route rationale, capital/JF planning, hauling manifests.
- **Skills & Career Mentor** — plan toward a goal ("I want to fly a carrier"), remap advice, ISK-ROI ranking.
- **PvE Guide** — mission/abyss/incursion advice and ISK/hr comparisons.
- **Corp & Fleet Ops** — member-vetting summaries, SRP triage, fleet-comp suggestions, ping/mail drafting, fuel triage.
- **Accounting Analyst** — P&L narratives, anomaly/theft detection ("where did my ISK go?").
- **Scribe** — draft EVEmails, recruitment replies, AARs, summarize long notifications/mail.
- **Daily Briefing / Proactive agent** — the real Jarvis behavior: an event-driven agent watching the **Situational Awareness rail** that speaks up unprompted ("Local spiked +8 reds, 3 jumps out, you're in a ratting Ishtar — recommend docking") and gives a morning "state of your empire" brief. Consent-gated and rate-limited.

### Guardrails (EULA-safe)
AI is **advisory/analytical only**. It may read your data, recommend, and draft text. Any **state-changing or in-game action** (ESI waypoint/open-window, sending a mail, pushing a fit) requires **explicit user confirmation**; there is **no input automation and no market-order automation** — the model cannot drive the client or bot. Deterministic math always lives in Rust.

### AI Memory, Compression & Recall (a companion that grows with the player, without eating the disk)
The dual problem: an LLM's context window can't hold everything, and naively logging "everything the AI saw" would balloon storage — yet the player wants memory that *accumulates over months*. Solved with a layered memory architecture where **the AI never hoards its own copy of game data.**

- **Single source of truth — retrieve, don't restate.** Game state (assets, wallet, history, killmails) already lives once in `app.sqlite`/`sde.sqlite`. The agent reads it **on demand via the tool registry**, so nothing is duplicated into AI storage. This alone removes the biggest storage risk: the model is fed small, relevant query *results*, not gigabytes of context.
- **Tiered memory:**
  - *Working memory* — the live conversation, trimmed to a token budget.
  - *Episodic memory* — rolling **summarize-then-discard**: long chats/days compact into short notes; raw turns are pruned. Recursive digests (session → weekly → monthly milestone).
  - *Semantic / long-term memory* — a small, curated set of durable facts about *you*: goals ("training toward a carrier"), preferences ("avoids lowsec"), key relationships, learned playstyle. Stored as an **Obsidian-compatible Markdown vault** (see below).
  - *Retrieval (RAG)* — big data stays in the DB / `sqlite-vec` vector store and is fetched **by relevance per query**, never all loaded.

- **Obsidian-style Markdown vault for durable memory (recommended substrate).** The AI's semantic + episodic memory is written as **plain-text Markdown notes with `[[wiki-links]]` and YAML frontmatter** in an Obsidian-compatible vault — one note per entity (character, corp, system, doctrine, ongoing goal, milestone) plus dated journal notes. This is a proven pattern (Basic Memory, Obsidian Memory MCP, Memory Vault). Why it's a strong fit here:
  - *Transparency & control* — the player can open the vault in **Obsidian itself**, read/edit/delete exactly what the AI "knows," and *see the graph of their EVE journey*. Satisfies the inspect/edit/forget + "grows with me" goals natively.
  - *Portability & longevity* — plain-text files, not opaque DB blobs; export is just copying a folder.
  - *Linked recall* — backlinks form a knowledge graph the AI traverses, **complementing** vector search (graph + vector = sharper recall).
  - *Compression-friendly* — notes **are** the compressed summaries: summarize-then-discard writes/edits a note; rollups refine a note in place — bounded, not append-only logs.
  - *Boundary that keeps it tiny* — the vault holds only **distilled, durable** memory (insights, preferences, narrative, decisions, relationships). High-churn game data (market/assets/wallet) stays in the relational DB under "retrieve-don't-restate" — we never dump bulk data into Markdown, so the vault stays small and Obsidian-fast.
  - *Implementation* — `sqlite-vec` indexes the notes for semantic lookup; the existing file-watcher keeps index ↔ vault in sync and treats the user's manual edits as authoritative; the vault is exposed over our **MCP server** so the player's own Obsidian/agent tooling can share the same memory. Optionally point at an existing vault or use a dedicated managed one. Off by default like the rest of the AI layer.
- **Compression that bounds growth:**
  - **Time-series rollups/downsampling** for history (portfolio, activity): per-minute → hourly → daily → weekly as data ages — a year of net-worth becomes kilobytes, not gigabytes.
  - **Embeddings + short snippets** instead of full text for recall; **dedup by referencing SDE IDs** rather than re-storing names/stats.
  - **Raw logs are transient**: chat/game logs are parsed into compact events, then rotated/discarded — we never keep gigabytes of log files.
- **Recall quality (so it doesn't "go dumb"):** hybrid retrieval (vector similarity + structured filters + **recency × importance** weighting); a **salience score on write** (goals/decisions/corrections = high, chit-chat = low); periodic **consolidation** that promotes frequently-used facts to durable memory and lets unused ones decay. And because SDE + current game state are always reachable via tools, the AI is **never blind** even with empty memory — it can re-derive facts at any time.
- **Player control & budget:** a configurable storage cap with age/importance-based eviction (LRU) and **pinned memories** that never evict; a memory viewer to **inspect, edit, or forget** anything (transparency + privacy); all local, with the export/wipe controls from the privacy rec.

The growth story falls out of this: the durable semantic memory + rolled-up history *is* the player's accumulating journey (milestones, doctrines flown, net-worth arc) — the AI gets richer month over month while the footprint stays bounded and predictable.

---

## Flagship Feature: Local Threat Scanner ("are these pilots safe or pirates?")

A standout safety feature (the PySpy / Pirate's Little Helper / EVE OS Local Intel pattern), surfaced permanently in the **Situational Awareness rail** and narrated by the AI **Threat Analyst**. Two modes:

**1. Manual paste.** Paste a list of pilot names (in-game, select-all in the Local member list → Ctrl/⌘-C copies the names). The tool resolves names → character IDs via ESI `universe/ids`, then for each pilot pulls: zKillboard stats (kills/losses, ISK destroyed, danger ratio, solo vs gang, recent activity, kills in *this* system/region), affiliation + corp age + security status (ESI), and the ship types they fly. It returns a sorted threat table with per-pilot badges (**Safe / Neutral / Caution / Danger**) plus a one-line summary ("3 known hunters, 1 likely cyno alt, gang of 5 from <alliance> active 2 jumps out").

**2. "Automatic" = clipboard-watch (the EULA-safe equivalent of auto-tracking).** Important constraint surfaced in research: **ESI has no "who is in local" endpoint, and reading game memory is a bannable offense** — so genuine zero-touch detection of *silent* pilots is impossible by any legal means. PySpy's solution, which we adopt, is a background **clipboard monitor**: the user copies Local (a single keystroke they already use), and the tool instantly re-scans with no further interaction — effectively automatic. As a complement, the **passive chat-log parser already in Phase 4** flags anyone who *speaks* in Local in real time without any copy. Together: copy-to-refresh for the full roster, always-on for talkers.

**Threat scoring inputs & flags:** danger ratio & recent kill volume (hunter), low security status + heavy killboard (pirate), young character + cyno-capable hull (cyno/hot-drop risk), gank hull in high-sec (Catalyst/Taloses), alliance owns supers/blops (escalation risk), recent kills near your location, and your own standings/known-hostile lists (red/neutral/blue). The deterministic score is computed in Rust; the AI layer only narrates it.

**Performance/compliance:** cache per-character zKill stats (they change slowly), batch ESI ID resolution, respect zKill API rate limits + descriptive User-Agent. Lives in the **Intel module (Phase 4)**, pinned to the Situational Awareness rail so it's always-foreground exactly like the in-game Local window.

---

## Phased Roadmap (each phase independently shippable)

- **Phase 0 — Foundation (first implementation target):** Tauri+React shell, routing, theme, tray; EVE SSO PKCE login + OS-keychain tokens; ESI client (cache-first + ETag + error-budget limiter) + background scheduler; SDE ingestion + query API + `sde-tools` converter; multi-character + Groups model; settings. *Ship: log in, see your characters & public info.*
- **Phase 1 — Character Core (ESI, high value/low effort):** Skills & training, Wallet & accounting, Assets & valuation, Clones/implants, **Mining ledger & yield**, **EVEmail client**. *Ship: a solid character monitor.* Establishes the polling/cache patterns reused everywhere.
- **Phase 2 — Markets & Industry:** market browser + regional compare + station-trade scanner; industry build planner (ME/TE, job tracking, reactions, invention odds); **reprocessing/refining + insurance calculators**; contracts. *Ship: the trader/industrialist app.*
- **Phase 3 — Fitting & Skills integration:** PYFA-class dogma fitting + EFT import/export + skill-aware sim; **fit-sharing + fleet-composition simulator**; skill-plan editor; "can-I-fly-this-fit"; PI planner. *Ship: the planner app.*
- **Phase 4 — Desktop-only live layer (the moat):** log tailer; Vintel-class intel map; D-scan paste; **Local Threat Scanner (clipboard-watch + manual paste → zKill safe/pirate flags)**; **gate-camp check + character background-check**; live combat/DPS + AAR; zKillboard integration; notification rule engine (tray + Discord). *Ship: the PvP/intel app web tools can't build.*
- **Phase 5 — Navigation, Corp & Content:** route planner + avoidance + ESI waypoint set; **courier/hauling logistics (reward/collateral calc, marketplaces)**; Dotlan-style sov/ADM overlay; WH chain mapper (+mass) + EVE-Scout/Thera; **live fleet boss management (ESI fleets)**; **in-game UI bridge (open-window / waypoint) woven through every module**; **bookmark manager**; **jump-fatigue tracker**; corp/alliance management incl. **SRP, recruitment/HR + corp-security vetting, timerboards, moon scheduling**; **PvE content suite (missions/agent finder, incursions, faction warfare, abyssal run stats)**; LP optimizer; calendar/events. *Ship: the FC/director/PvE app.*
- **Phase 6 — The 10x differentiators** (list above), each built on data layers already shipped.
- **Phase 6.5 — AI foundation (the Jarvis):** AI gateway + tool registry over existing commands, OpenAI-compatible local-LLM client with auto-detect, MCP server/client, local RAG (`sqlite-vec`), the **tiered memory store + compression/consolidation jobs**, the Commander orchestrator, and the first domain agents (Market, Threat/Intel, Fitting). Per-domain agents then light up alongside their modules; proactive Briefing agent wires into the Situational Awareness rail. Optional voice (whisper.cpp/Piper). *Ship: optional, off-by-default; local-first.*
  - ✅ **OpenAI-compatible client** (`ai::AiClient`) — Ollama/LM Studio/llama.cpp/cloud, pure tested wire-format, endpoint auto-detect.
  - ✅ **Tool registry + Commander orchestrator** — 10 read-only tools (item/market/arbitrage/risk/account/portfolio/income/skill-ROI) with a bounded tool-call loop; cloud key in OS keychain.
  - ✅ **Proactive briefing** — "state of your empire" one-shot via the tools.
  - ✅ **Durable memory** — pure salience scoring + importance/recency eviction (pinned-spared), `ai_memory` store, recall injected into chat + briefing, inspect/pin/forget UI.
  - ⏳ **Remaining:** MCP server/client (two-way), `sqlite-vec` RAG over SDE + notes, per-domain specialist agents, optional voice.
- **Phase 7 — Polish & scale:** plugin/extension API, layout customization, perf pass (DuckDB for heavy analytics if needed), localization, auto-update, opt-in telemetry, community fit/plan sharing.

---

## Build status (audit)

| Phase | Status | Notes |
|---|---|---|
| 0 — Foundation | ✅ Complete | Shell/routing/theme/tray, SSO PKCE + keychain, cache-first ESI (ETag/304, error-budget breaker), poll scheduler, SDE + `sde-tools` converter, multi-char + Groups, settings. |
| 1 — Character Core | ✅ Complete | Skills/training, wallet/accounting, assets/valuation, clones/implants, mining ledger, EVEmail. |
| 2 — Markets & Industry | ✅ Complete | Market browser + hub compare + station-trade scanner, industry planner (ME/TE, jobs, reactions, invention odds), reprocess + insurance calcs, contracts. |
| 3 — Fitting & Skills | ✅ Complete (exceeds) | Dogma EHP/DPS/cap with resists + active tank + character damage skills + hull-bonus display; EFT in/out; fit library + doctrine sim; skill editor + EVEmon import + ROI + **remap optimizer**; can-I-fly + gatekeeper; PI planner. Only **esi-fittings game-sync** unbuilt. |
| 4 — Live layer | ✅ Complete | Log tailer, intel map + unified system risk, D-scan, threat scanner, gate-camp + background check, combat AAR + multibox fleet AAR, zKill, notify rules + Discord. |
| 5 — Nav/Corp/Content | ✅ Complete | Built: routing + avoidance + waypoint, courier, sov-owner + **ADM overlay**, **WH mass roller** + **signature/chain tracker** + Thera/EVE-Scout, fleet read + **MOTD/free-move write** + **member kick/move (wing/squad)**, in-game UI bridge, jump-fatigue, corp members/structures/**SRP**/**recruitment**/**timerboard**/**moon-extraction scheduler**/**container-log theft vetting**, PvE incursions/FW/**abyss tracker**/**agent-mission finder**, LP, calendar. (Bookmark mgr removed — CCP retired the ESI scope. Agent finder + ADM need an SDE rebuild / live sov data to populate.) |
| 6 — 10x differentiators | ✅ Complete | Income optimizer, unified risk, hub arbitrage (ISK/m³), portfolio analytics, multibox fleet AAR, fit gatekeeper, skill-plan ROI. |
| 6.5 — AI (Jarvis) | ✅ Complete | Built: OpenAI-compatible local client + auto-detect, tool registry + orchestrator, **per-domain specialist agents**, proactive briefing, durable memory + **hybrid vector+keyword RAG recall** (embeddings client + cosine, side-table vector store, reindex), **MCP server core** + **stdio transport bridge** (`--mcp-stdio` headless mode). Optional extra (off-by-default, needs bundled native binaries): **voice** (whisper.cpp STT / Piper TTS) — the one deferred nicety; everything else in the phase is shipped. |
| 7 — Polish & scale | ✅ Complete | Built: data export/wipe, local libraries (skill plans / fits / implant loadouts) + EVEmon/EFT import, appraisal, **configurable home dashboard** (show/hide/reorder widgets), **localization framework** (i18n + language picker, en + starter de, English fallback), **update check** (notify-only semver vs. releases), **opt-in telemetry** (off by default, name+counts only, transparency view), **backend-free community sharing** (EVECMDR1 share codes for fits/plans + importer), **read-only plugin API** (JSON-manifest panels gated to the read-only tool registry). Ops-bound remainders (need release/hosting infra, not app code): **signed auto-update channel** (the check is built; signing+download is release ops), **hosted cloud sync**, full per-string translation coverage. |

**Remaining gaps split two ways:**
- *Buildable & self-contained* (no live ESI needed): WH signature/chain tracker, structure reinforcement timerboard, opt-in telemetry flag, localization scaffolding, configurable dashboard widgets.
- *Needs a live client / 3P / infra* (validate on the user's machine): fleet write-ops, contacts/standings editor, esi-fittings sync, container-log theft detection, MCP server, sqlite-vec RAG, voice, auto-update + code-signing, cloud sync.

---

## Repository structure (created in Phase 0)

```
Eve-Commanderv2/
├─ Cargo.toml · package.json · tauri.conf.json
├─ src-tauri/src/
│  ├─ main.rs                 # Tauri builder, plugins, tray
│  ├─ commands/               # thin #[tauri::command] IPC handlers
│  ├─ esi/                    # client.rs ratelimit.rs cache.rs scheduler.rs endpoints/
│  ├─ auth/                   # sso.rs pkce.rs token_store.rs
│  ├─ sde/                    # ingest.rs query.rs schema.sql
│  ├─ logs/                   # watcher.rs chatlog.rs gamelog.rs dscan.rs
│  ├─ notify/                 # rules.rs discord.rs tray.rs
│  ├─ db/                     # pool.rs migrations/ models/
│  ├─ domain/                 # fitting/ industry/ market/ navigation/ intel/ optimizer/ portfolio/
│  └─ thirdparty/             # zkillboard, eve-scout, fuzzwork, dotlan
├─ src/                       # React: ipc/ stores/ queries/ components/ features/ lib/ hooks/ types/ theme/
├─ shared/                    # ts-rs generated types
├─ sde-tools/                 # build-time SDE→SQLite converter (CI)
├─ scripts/ · tests/ · docs/  # docs incl. architecture, ESI scope matrix, EULA-notes.md
```

### Critical files (Phase 0 backbone)
- `src-tauri/src/esi/client.rs` — cache-first ESI client + ETag + error-budget limiting (every data feature depends on it).
- `src-tauri/src/auth/sso.rs` — EVE SSO PKCE flow + keychain token storage (gates all authenticated data).
- `src-tauri/src/esi/scheduler.rs` — per-character poll scheduler & rate-budget coordination.
- `src-tauri/src/sde/ingest.rs` — SDE ingestion/query layer underpinning industry, fitting, market, navigation.
- `src-tauri/src/logs/watcher.rs` — local log tailer (Phase 4, the desktop differentiator).

---

## Risks & mitigations
- **ESI error budget across many alts** → cache-first + 304s; circuit breaker on `X-Esi-Error-Limit-Remain`; cadence tied to cache headers; concurrency cap + jitter; prioritized jobs.
- **Resource footprint (CPU/network/battery)** → tiered poll classes, local-countdown derivation, active/idle + minimized throttling, coalesced shared fetches, RedisQ push for killfeed, smoothed scheduler queue, user intensity profile (see Data Freshness section). Goal: idle background use stays negligible beside the running game client.
- **SDE cadence/format** → CI converter + version-pinned prebuilt SQLite in installer; out-of-band hot-swap; never block startup.
- **EULA gray areas** → passive log reads only; no memory/automation/overlay; ESI/clipboard only for any "write"; documented + review-gated.
- **Token security** → PKCE (no secret); refresh tokens in keychain, access tokens memory-only; minimal incremental scopes; clean revoke.
- **3P API reliability (zKill/EVE-Scout/Fuzzwork/Dotlan)** → optional enrichment behind `thirdparty/`, cache + degrade gracefully, never a hard core dependency.
- **Cross-OS WebView differences** → conservative CSS; test WebView2/WKWebView/WebKitGTK in CI.
- **Scope creep** → strict phase gates, each phase shippable; later phases cheaper via reused data layers.
- **AI correctness & EULA** → LLM never computes numbers (tools/Rust do); AI is advisory, all in-game/ESI actions user-confirmed; no automation/botting. AI fully optional and off by default, so it can never block core use.
- **Calculation trust** → players abandon a tool the moment its numbers are wrong; validate dogma/fitting math against PYFA + in-game, and industry/market math against known references, with a regression test suite of "golden" values. This is a first-class quality gate, not an afterthought.
- **AI storage growth & memory drift** → retrieve-don't-restate (no duplicated game data), summarize-then-discard + time-series rollups, configurable storage cap with importance/age eviction + pinned memories; consolidation keeps recall sharp so the AI doesn't "forget." All bounded and user-inspectable (see AI Memory section).
- **Multiboxing / input broadcasting** → explicitly never implemented (ISBoxer-style input multiplexing is a ban); reinforces the no-automation line even though we support viewing many alts.
- **Desktop distribution** → unsigned desktop apps trip Windows SmartScreen / macOS Gatekeeper; budget for code-signing + macOS notarization and a **signed** auto-update channel from the start.

---

## Project, Distribution & Sustainability (recommended additions)
- **ESI app registration is a Phase 0 prerequisite** — register the application at developers.eveonline.com (public client, PKCE, loopback redirect, full scope list) before auth work; the `client_id` ships with the app.
- **Live killfeed mechanism** — use zKillboard's **RedisQ** (pull queue) / websocket for the real-time local killfeed rather than polling, with ESI for killmail detail. Concrete backing for the Situational Awareness rail + Threat Scanner.
- **Branding & IP** — follow CCP's IP/third-party guidelines: a clear "not affiliated with / not endorsed by CCP" disclaimer, no implication of official status. (CCP permits community tools using EVE IP within those bounds.)
- **Sustainability model** — recommend **open-core + donations/optional managed sync** (the ecosystem norm; SeAT/PYFA/zKill are free/donation). Avoid anything that looks like selling ESI data, which the developer license restricts.
- **Resilience** — handle daily downtime (~11:00) and ESI outages gracefully; an **offline mode** keeps SDE-backed features (fitting, BOM, reference, calculators) fully usable when ESI is unreachable.
- **Data portability & privacy** — one-click **export / wipe all local data**; no telemetry without explicit opt-in. Reinforces the local-first trust story (especially with local AI).
- **Suggested v1.0 scope = Phases 0–4** (foundation → character core → economy → fitting/skills → desktop live layer incl. Threat Scanner). Phases 5–7 + AI are post-1.0. Naming a crisp 1.0 is the single best guard against boil-the-ocean.

---

## Verification (Phase 0)
1. **Build & run:** `npm install` + `cargo build`; `npm run tauri dev` launches the app shell with tray and routing.
2. **Auth:** register an ESI app at developers.eveonline.com (loopback redirect), log in via system browser; confirm a refresh token lands in the OS keychain and access token is in memory only.
3. **ESI client:** fetch a public endpoint (e.g. server status) then an authenticated one (character info); verify cache hit on repeat, `304` on ETag revalidation, and that the rate limiter backs off when the simulated error budget drops.
4. **SDE:** confirm `sde.sqlite` ships, loads, and a type/system name lookup (FTS5) returns results offline.
5. **Multi-char:** add two characters, switch active character, create a Group, confirm independent poll schedules.
6. **Tests:** `cargo test` for domain/ESI modules; a Playwright/tauri-driver smoke test for login→character list.
7. **EULA self-check:** confirm no code path sends input to the game, reads game memory, or scrapes cache — only ESI + (later) passive log reads.
8. **Resource budget:** with several characters added, confirm the scheduler respects per-endpoint cache timers (no early re-polls), idle/minimized state drops to essentials, countdown timers tick with no network, and idle CPU/network stay negligible — measured against a target budget.

Work proceeds on branch `claude/eloquent-ride-k5jp4v`; commit per phase, push when each phase is complete.
