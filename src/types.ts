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

export interface LocationValueView {
  location_name: string;
  value: number;
  item_count: number;
}

export interface NamedType {
  type_id: number;
  name: string;
}

export interface JumpCloneView {
  jump_clone_id: number;
  name: string | null;
  location_name: string;
  implants: NamedType[];
}

export interface ClonesView {
  jump_clone_count: number;
  active_implant_count: number;
  implants: NamedType[];
  home_location_name: string | null;
  jump_clones: JumpCloneView[];
  last_jump_date: string | null;
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

export interface ColonyView {
  planet_id: number;
  system_name: string;
  planet_type: string;
  upgrade_level: number;
  num_pins: number;
  extractor_count: number;
  products: string[];
  soonest_expiry: string | null;
  seconds_remaining: number;
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

export interface QueuedSkillView {
  name: string;
  finished_level: number;
  queue_position: number;
  seconds_remaining: number;
}

export interface CharacterAttributes {
  intelligence: number;
  memory: number;
  perception: number;
  willpower: number;
  charisma: number;
  bonus_remaps: number | null;
}

export interface TransactionView {
  item_name: string;
  is_buy: boolean;
  quantity: number;
  unit_price: number;
  total: number;
  date: string;
}

export interface CharacterStatusView {
  online: boolean;
  system_name: string;
  ship_name: string;
  ship_type_name: string;
  training: string | null;
  training_seconds_remaining: number | null;
}

export interface AppSettings {
  intensity: string;
  notify_min: string;
  discord_webhook: string;
}

export interface CharacterGroup {
  id: number;
  name: string;
  members: number[];
}

export interface Contract {
  contract_id: number;
  type: string;
  status: string;
  title: string;
  date_issued: string;
  date_expired: string;
  price: number;
  reward: number;
  collateral: number;
  volume: number;
  for_corporation: boolean;
}

export interface ItemHit {
  type_id: number;
  name: string;
}

export interface MarketQuote {
  best_sell: number | null;
  best_buy: number | null;
  spread: number | null;
  spread_pct: number | null;
  sell_volume: number;
  buy_volume: number;
  sell_orders: number;
  buy_orders: number;
}

export interface HistoryStats {
  last_average: number | null;
  avg_30d: number;
  high_30d: number;
  low_30d: number;
  daily_volume_30d: number;
  recent: number[];
}

export interface InsuranceLevel {
  name: string;
  cost: number;
  payout: number;
}

export interface HubQuote {
  hub: string;
  best_sell: number | null;
  best_buy: number | null;
}

export interface MarketBrowse {
  quote: MarketQuote;
  history: HistoryStats;
  insurance: InsuranceLevel[] | null;
  hubs: HubQuote[];
}

export interface TradeOpportunity {
  type_id: number;
  name: string;
  buy_price: number;
  sell_price: number;
  margin_pct: number;
  profit_per_unit: number;
  daily_volume: number;
  daily_potential: number;
}

export interface RefineYieldView {
  type_id: number;
  name: string;
  quantity: number;
  value: number;
}

export interface ReprocessView {
  portions: number;
  leftover_units: number;
  yields: RefineYieldView[];
  refined_value: number;
  sell_value: number;
  advantage: number;
}

export interface PlanLineView {
  type_id: number;
  name: string;
  quantity: number;
  unit_price: number;
  value: number;
}

export interface BuildPlanView {
  product_type_id: number;
  product_name: string;
  runs: number;
  me: number;
  output_units: number;
  materials: PlanLineView[];
  material_cost: number;
  product_value: number;
  profit: number;
  margin_pct: number;
  probability: number | null;
}

export interface ResolvedItem {
  type_id: number | null;
  name: string;
  charge: string | null;
  quantity: number;
}

export interface ResolvedFit {
  ship_type_id: number | null;
  ship: string;
  name: string;
  items: ResolvedItem[];
  unresolved: string[];
}

export interface SkillStepView {
  skill_type_id: number;
  name: string;
  current_level: number;
  target_level: number;
  sp: number;
  seconds: number;
  known: boolean;
}

export interface SkillPlanView {
  steps: SkillStepView[];
  total_sp: number;
  total_seconds: number;
}

export interface MissingSkillView {
  skill_type_id: number;
  name: string;
  required_level: number;
  current_level: number;
  seconds: number;
}

export interface CanFlyView {
  ship: string;
  can_fly: boolean;
  missing: MissingSkillView[];
  total_seconds: number;
  parsed: boolean;
  unresolved: string[];
}

export interface DoctrinePilotView {
  character_id: number;
  name: string;
  can_fly: boolean;
  missing_count: number;
  total_seconds: number;
}

export interface DoctrineView {
  ship: string;
  parsed: boolean;
  pilots: DoctrinePilotView[];
  can_fly_count: number;
  unresolved: string[];
}

export interface DscanGroup {
  type_name: string;
  count: number;
}

export interface DscanResult {
  total: number;
  groups: DscanGroup[];
  warnings: string[];
}

export interface PilotThreatView {
  name: string;
  level: "Safe" | "Neutral" | "Caution" | "Danger";
  reasons: string[];
  danger_ratio: number;
  ships_destroyed: number;
  sec_status: number;
}

export interface ThreatScanView {
  pilots: PilotThreatView[];
  summary: string;
  unresolved: string[];
}

export interface GateCampView {
  system: string;
  found: boolean;
  kills_last_hour: number;
  level: "Safe" | "Neutral" | "Caution" | "Danger";
  message: string;
}

export interface EntityDamage {
  entity: string;
  damage: number;
}

export interface AarSummary {
  damage_dealt: number;
  damage_received: number;
  duration_seconds: number;
  dps_dealt: number;
  dps_received: number;
  event_count: number;
  top_targets: EntityDamage[];
  top_attackers: EntityDamage[];
}

export interface CombatLogView {
  found: boolean;
  summary: AarSummary | null;
}

export interface LocalIntel {
  system: string | null;
  speakers: string[];
  line_count: number;
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
