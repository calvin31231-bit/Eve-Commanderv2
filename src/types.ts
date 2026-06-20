// Types shared with the Rust backend. (A future build step can generate these
// from Rust structs via `ts-rs`; hand-written for Phase 0.)

export interface Character {
  id: number;
  name: string;
  corporation_id: number | null;
  alliance_id: number | null;
  scopes: string[];
  active: boolean;
}

export interface ServerStatus {
  players: number;
  server_version: string;
  vip: boolean;
}

export interface CharacterSheet {
  wallet_balance: number;
  total_sp: number;
  unallocated_sp: number | null;
  skill_count: number;
  maxed_count: number;
  queue_len: number;
  active_skill_id: number | null;
  queue_finishes_at: string | null;
  queue_seconds_remaining: number | null;
}

export type Severity = "Info" | "Warning" | "Critical";

export interface Notification {
  key: string;
  title: string;
  body: string;
  severity: Severity;
  category: string;
  created_at: number;
  read: boolean;
}
