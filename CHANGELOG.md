# Changelog

All notable changes to EVE Commander are documented here. This project follows
[Semantic Versioning](https://semver.org/).

## [0.1.0] — First public release

The first public build: a cross-platform desktop companion (Tauri 2 + Rust +
React) that unifies the EVE third-party ecosystem over your live ESI data, the
offline SDE, local game logs, and third-party feeds — read/display/analysis
only, strictly EULA-compliant.

### Foundation
- EVE SSO login (OAuth2 + **PKCE**, no client secret); refresh tokens stored in
  the OS keychain, access tokens in memory only.
- Cache-first ESI client with ETag revalidation and an error-budget rate
  limiter; a tiered, per-character background poll scheduler.
- Multi-character model with groups, and a hot-swappable prebuilt SDE.

### Character & economy
- Skills & training, wallet & cashflow, assets & valuation, clones/implants,
  mining ledger, EVEmail.
- Market browser with cross-hub comparison, price-anomaly flags, and a per-item
  **price trend + forecast**; station-trade and multi-hub trade scanners.
- Industry build planner with ME/job profit and a **multi-level BOM tree**
  (recursive build-vs-buy); reprocessing calculator with an **ore ISK/m³
  ranking**; reactions, contracts, insurance.

### Fitting, intel & navigation
- PYFA-class dogma fitting (EHP, DPS-vs-range, cap stability), EFT import,
  can-I-fly, killmail→fit reconstruction, and a **doctrine fleet-readiness
  matrix**.
- Local threat scanner, gate-camp check, combat AAR with tackle/EWAR detection,
  live killfeed, and route planning with a **kill-heatmap + gank-chokepoint
  danger overlay**.
- Wormhole rolling calculator and **chain mapper**; signatures; courier logistics
  with a suggested-reward calculator.

### Corp, PvE & tools
- Corp structures/fuel, SRP, recruitment, loyalty-point engine, timerboard, and
  an **alliance-tournament comp builder**.
- Incursions with a community **ISK/hr estimate**, faction warfare, PI colonies
  with an **ISK/hr profit ranking**, LP optimizer, abyssal valuation.
- Net-worth history with **velocity + forecast**, income optimizer, skill-plan
  ROI, attribute remap.

### AI (optional, off by default)
- Local-first "Jarvis": OpenAI-compatible client (Ollama/LM Studio/…), tool
  registry over existing commands, an MCP server/client bridge, local RAG, a
  tiered memory store, and a proactive daily-briefing agent.

### Quality
- 349 unit tests plus a golden-value regression suite pinning the fitting,
  industry, market, and skill math against known EVE reference values.
- CI runs the full gate (tests + zero-warning clippy + typecheck + build) on
  every change.

[0.1.0]: https://github.com/calvin31231-bit/eve-commanderv2/releases/tag/v0.1.0
