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

export interface ValuedAssetGroup {
  type_id: number;
  name: string;
  quantity: number;
  locations: number;
  value: number;
}

export interface HoldingsView {
  total_value: number;
  groups: ValuedAssetGroup[];
}

export interface NamedType {
  type_id: number;
  name: string;
}

export interface ClonesView {
  jump_clone_count: number;
  active_implant_count: number;
  implants: NamedType[];
}

export interface RefTypeTotal {
  ref_type: string;
  total: number;
  count: number;
}

export interface CashflowSummary {
  income: number;
  expenses: number;
  net: number;
  entry_count: number;
  by_ref_type: RefTypeTotal[];
}

export interface IndustryJobView {
  job_id: number;
  activity: string;
  item_name: string;
  runs: number;
  status: string;
  end_date: string;
  seconds_remaining: number;
}

export interface MarketOrderView {
  order_id: number;
  item_name: string;
  is_buy_order: boolean;
  price: number;
  volume_remain: number;
  volume_total: number;
  seconds_remaining: number;
}

export interface MarketView {
  buy_count: number;
  sell_count: number;
  total_escrow: number;
  sell_value: number;
  orders: MarketOrderView[];
}

export interface NamedOre {
  type_id: number;
  name: string;
  quantity: number;
  value: number;
}

export interface MiningView {
  total_units: number;
  day_count: number;
  total_value: number;
  ores: NamedOre[];
}

export interface MailHeader {
  mail_id: number;
  subject: string;
  from_name: string;
  timestamp: string;
  is_read: boolean;
}

export interface MailView {
  subject: string;
  from: number;
  body: string;
  timestamp: string;
  read: boolean;
}

export interface CharacterWorth {
  character_id: number;
  name: string;
  wallet_balance: number;
  asset_value: number;
  total_sp: number;
  net_worth: number;
}

export interface AccountOverview {
  characters: CharacterWorth[];
  total_net_worth: number;
  total_wallet: number;
  total_asset_value: number;
  total_sp: number;
}

export interface CharacterProfile {
  name: string;
  corporation: string;
  alliance: string | null;
  security_status: number;
}

export interface CharacterStatusView {
  online: boolean;
  system_name: string;
  ship_name: string;
  ship_type_name: string;
  training: string | null;
  training_seconds_remaining: number | null;
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
