# Running EVE Commander locally

This is the desktop app (Tauri 2 + React). It needs a WebView and an ESI
application registered with EVE.

## 1. Prerequisites

- **Rust** (1.80+) — https://rustup.rs
- **Node 18+** and **pnpm** — `npm i -g pnpm`
- **Platform WebView / system libs:**
  - **Windows:** WebView2 (preinstalled on Win10/11) + the MSVC build tools.
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`). WKWebView is built in.
  - **Linux (Debian/Ubuntu):**
    ```bash
    sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
      libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev \
      build-essential libssl-dev
    ```

## 2. Register an ESI application

1. Go to https://developers.eveonline.com/applications and create a new app.
2. **Connection Type:** *Authentication & API Access* (so it can request scopes).
3. **Callback URL:** exactly
   ```
   http://localhost:8787/callback
   ```
4. **Scopes:** add all of these (they back the Character + Economy hubs):
   ```
   publicData
   esi-skills.read_skills.v1
   esi-skills.read_skillqueue.v1
   esi-wallet.read_character_wallet.v1
   esi-assets.read_assets.v1
   esi-industry.read_character_mining.v1
   esi-industry.read_character_jobs.v1
   esi-markets.read_character_orders.v1
   esi-mail.read_mail.v1
   esi-clones.read_clones.v1
   esi-clones.read_implants.v1
   esi-location.read_location.v1
   esi-location.read_online.v1
   ```
5. Copy the **Client ID**. (PKCE is used, so the secret is not needed.)

## 3. Configure

**Recommended — a `.env` file** (no shell wrangling, survives restarts). Copy
the example and fill in your Client ID:

```bash
cp .env.example .env      # PowerShell: copy .env.example .env
# then edit .env and set EVE_COMMANDER_CLIENT_ID=...
```

`.env` is git-ignored and loaded automatically at startup.

**Or via an environment variable** (must be set in the *same* terminal, *before*
launching, and the app must be restarted to pick up changes):

```bash
export EVE_COMMANDER_CLIENT_ID=your_client_id_here     # macOS/Linux
```
```powershell
$env:EVE_COMMANDER_CLIENT_ID = "your_client_id_here"   # Windows PowerShell
```

A real environment variable takes precedence over `.env`.

## 4. Run

```bash
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` starts Vite (port 5173) and launches the desktop app.

## 5. First-run flow

1. The cockpit window opens on **Home**.
2. Click **Log in with EVE** → your browser opens the EVE SSO page → authorize.
3. The browser shows a "Signed in" page; return to the app. Your character is
   added and made active automatically.
4. Open the **Character** hub (☺) for skills/wallet/cashflow/assets/clones/
   mining/mail, and the **Economy** hub (₿) for industry jobs + market orders.
5. Add more characters from Home ("Add character"); click a character row to
   switch the active one.

## Building the full SDE (unlocks reprocessing, BOM, skill plans, can-I-fly)

The full Static Data Export is too large to ship in the repo, so generate it
once locally. On Windows:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-sde.ps1
```

This downloads CCP's SDE, runs the `sde-convert` tool over the FSD YAML
(`typeIDs`, `typeMaterials`, `blueprints`, `typeDogma`), and writes
`sde.sqlite` into `%APPDATA%\eve-commander\` where the app finds it on the next
launch. To run the converter by hand (any OS):

```
cargo run -p sde-tools --release -- \
  --out <data_dir>/sde.sqlite \
  --types       fsd/typeIDs.yaml \
  --type-materials fsd/typeMaterials.yaml \
  --blueprints  fsd/blueprints.yaml \
  --type-dogma  fsd/typeDogma.yaml
```

`--type-dogma` derives both skill ranks/attributes and per-item required skills
in one pass, so skill plans and the can-I-fly check go live too. The PowerShell
script additionally pulls Fuzzwork's `mapSolarSystems`/`mapSolarSystemJumps`/
`mapRegions` CSVs (`--systems-csv/--jumps-csv/--regions-csv`) so the region map
and routing have universe topology. Until the SDE is present those panels show a
"needs full SDE" note and everything else works.

## Notes

- **Names:** without a full prebuilt `sde.sqlite`, common items (minerals, ores,
  iconic ships, trade hubs) resolve via a built-in seed; everything else shows
  `Type {id}`. Build the full SDE (above) to resolve everything.
- **Tokens:** refresh tokens are stored in the OS keychain (the desktop build
  enables the `keychain` feature); access tokens stay in memory.
- **Data dir:** `~/.local/share/eve-commander` (Linux), `%APPDATA%\eve-commander`
  (Windows) — holds `app.sqlite`, `cache.sqlite`, and the prebuilt `sde.sqlite`.
