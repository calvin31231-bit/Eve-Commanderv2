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
