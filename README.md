# EVE Commander

**The master EVE Online companion** — one cross-platform desktop app that consolidates the
best of the EVE third-party ecosystem (market, industry, fitting, killboards, mapping, skills,
intel, corp/fleet, PI, and more) into a single cohesive tool, and fills the gaps no single tool
can — because only an all-in-one desktop app has your live ESI data for every character, your
local game logs, the offline SDE, and third-party feeds in the same place at the same time.

> **Strictly EULA-compliant.** EVE Commander is a **read / display / analysis** layer over the
> public ESI API and passive reads of your local log files. There is **no input automation, no
> botting, no game-memory reading, and no disallowed cache scraping**. See
> [`docs/EULA-notes.md`](docs/EULA-notes.md). Not affiliated with or endorsed by CCP Games.

The full product vision, module breakdown, UI/UX strategy, resource-budget design, optional
local-AI ("Jarvis") layer, and phased roadmap live in [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Architecture

A Rust + Tauri desktop app, structured so the logic is testable without a GUI:

| Crate | Role | Builds headless? |
|---|---|---|
| [`crates/eve-core`](crates/eve-core) | All logic: SSO/PKCE auth, cache-first ESI client, error-budget rate limiter, tiered poll scheduler, SDE access, domain models. **No GUI deps.** | ✅ Yes — `cargo test -p eve-core` |
| [`src-tauri`](src-tauri) | The desktop shell: window, system tray, IPC commands. Thin; delegates to `eve-core`. | ⚠️ Needs a system WebView (WebView2 / WKWebView / WebKitGTK) |
| `src/` *(planned)* | React + TypeScript + Vite frontend. | — |

This split is deliberate: the heavy, correctness-critical work (ESI scheduling, rate limiting,
fitting/industry math) lives in `eve-core` and is unit-tested anywhere, while `src-tauri` only
owns the window and IPC.

## Phase 0 status (foundation)

Implemented and tested in `eve-core`:

- **EVE SSO** — OAuth2 Authorization Code + **PKCE** (`auth::pkce`, `auth::sso`), with refresh
  tokens stored in the OS keychain (`auth::token_store`, `keychain` feature).
- **Cache-first ESI client** (`esi::client`) — serves fresh cache without network, revalidates
  stale entries with `If-None-Match` (cheap `304`s), parses `Expires`, and always sends a
  descriptive `User-Agent`.
- **Error-budget rate limiter** (`esi::ratelimit`) — tracks `X-Esi-Error-Limit-*` and trips a
  circuit breaker before CCP throttles us.
- **Tiered poll scheduler** (`esi::scheduler`) — poll classes aligned to ESI cache timers, with
  background characters polled less often per the user's intensity profile (the resource-budget
  design).
- **Model** (`model`) — first-class characters + user-defined groups ("hats").

## Building

### Prerequisites
- Rust (stable, 1.80+) and Cargo
- Node 18+ and a package manager (pnpm recommended)
- A system WebView for the desktop shell:
  - **Windows:** WebView2 (preinstalled on Win11)
  - **macOS:** WKWebView (built in)
  - **Linux:** `webkit2gtk-4.1` and `libayatana-appindicator3` (install via your package manager)

### Run the headless logic tests (no WebView required)
```bash
cargo test -p eve-core
```

### Run the desktop app (requires a WebView + the frontend)
```bash
# once the frontend lands:
pnpm install
pnpm tauri dev
```

You will need a registered ESI application (https://developers.eveonline.com) — a public PKCE
client with a loopback redirect URI — and to provide its `client_id`:

```bash
export EVE_COMMANDER_CLIENT_ID=your_client_id
export EVE_COMMANDER_REDIRECT_URI=http://localhost:8787/callback
```

## License

MIT. EVE Online and all related trademarks are property of CCP Games. This is an unofficial,
community tool.
