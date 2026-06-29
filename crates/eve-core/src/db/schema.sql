-- app.sqlite — durable user data. Idempotent; run on every startup.

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS characters (
    id             INTEGER PRIMARY KEY,
    name           TEXT    NOT NULL,
    corporation_id INTEGER,
    alliance_id    INTEGER,
    -- space-delimited ESI scopes granted for this character (incremental).
    scopes         TEXT    NOT NULL DEFAULT '',
    active         INTEGER NOT NULL DEFAULT 0,
    added_at       INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS character_groups (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT    NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS character_group_members (
    group_id     INTEGER NOT NULL REFERENCES character_groups(id) ON DELETE CASCADE,
    character_id INTEGER NOT NULL REFERENCES characters(id)       ON DELETE CASCADE,
    PRIMARY KEY (group_id, character_id)
);

-- Persisted historical snapshots (net worth, etc.) feed the portfolio
-- analytics; downsampled as they age (see ROADMAP "Data Freshness").
CREATE TABLE IF NOT EXISTS snapshots (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL,
    kind         TEXT    NOT NULL,
    taken_at     INTEGER NOT NULL,
    value        REAL    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_snapshots_char_kind_time
    ON snapshots (character_id, kind, taken_at);

-- Resolved id→name cache (types, systems, characters, …) from ESI
-- /universe/names/. Persistent so names are instant after the first lookup.
CREATE TABLE IF NOT EXISTS names (
    id       INTEGER PRIMARY KEY,
    name     TEXT    NOT NULL,
    category TEXT
);

-- Application settings (key/value).
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);


-- Durable AI memory: the small, curated set of facts the assistant keeps about
-- the player (goals, decisions, preferences). Bounded by an importance/recency
-- eviction policy; pinned notes never evict. See `ai_memory` for the policy.
CREATE TABLE IF NOT EXISTS ai_memory (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT    NOT NULL,
    title      TEXT    NOT NULL,
    body       TEXT    NOT NULL,
    salience   REAL    NOT NULL,
    pinned     INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_memory_updated ON ai_memory (updated_at);

-- Reusable skill-plan library: named plans stored independent of any character
-- (body is the importable text form) so they can be loaded onto a new alt.
CREATE TABLE IF NOT EXISTS skill_plans (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL,
    body       TEXT    NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Reusable fit library: named EFT fits, character-independent, to recall/check
-- against any character or export to the in-game fitting window.
CREATE TABLE IF NOT EXISTS saved_fits (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL,
    ship       TEXT    NOT NULL DEFAULT '',
    eft        TEXT    NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Saved implant loadouts: a named clone's implant set (implant type ids stored
-- comma-separated), character-independent so it travels with the user.
CREATE TABLE IF NOT EXISTS implant_loadouts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL,
    implant_ids TEXT    NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
