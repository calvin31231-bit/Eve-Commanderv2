# EULA & Third-Party Developer License — compliance notes

EVE Commander must remain strictly within CCP's
[EULA](https://support.eveonline.com/hc/en-us/articles/8414732786972),
[Third-Party Policies](https://support.eveonline.com/hc/en-us/articles/8564030965660),
and [Developer License](https://docs.esi.evetech.net/docs/developer_license.html).
Every feature is reviewed against the lines below.

## Allowed (and used)
- **ESI REST API** via EVE SSO (OAuth2 Authorization Code + PKCE), respecting cache headers,
  the error budget, and a descriptive `User-Agent`.
- **Passive reading** of the player's local EVE log files (Chatlogs / Gamelogs) — display and
  analysis only.
- **Clipboard reading** that the user initiates (e.g. copying the Local member list, a D-scan
  dump) — the basis of the Local Threat Scanner, exactly as PySpy does it.
- **ESI-sanctioned writes**, always behind explicit user confirmation: setting an autopilot
  waypoint, opening an in-game window (`esi-ui`), sending an EVEmail, saving a fitting.
- **Third-party data** (zKillboard, EVE-Scout, Fuzzwork, Dotlan) as optional enrichment.

## Forbidden (never implemented)
- ❌ Input automation / macros / sending keystrokes or clicks to the game client.
- ❌ Reading or writing **game memory**; any client injection or on-game overlay.
- ❌ **Cache scraping** in disallowed ways.
- ❌ **Market-order automation** (auto-placing/adjusting orders), probe/scan automation, or any
  automation that accelerates progression beyond ordinary play.
- ❌ **Input multiplexing / broadcasting** (ISBoxer-style) — even though we support viewing many
  alts, we never broadcast input to them.
- ❌ RMT / ISK selling facilitation.

## AI layer
The optional AI ("Jarvis") layer is **advisory/analytical only**. It may read your data,
recommend, and draft text. It **never computes the numbers itself** (deterministic math lives in
Rust) and **never takes a state-changing or in-game action without explicit user confirmation**.
With a local model, none of your data leaves your machine.

## Implementation guardrail
There is **no code path** in this project that sends input to the game, reads game memory, or
scrapes the cache. Reviewers: reject any change that introduces one.
