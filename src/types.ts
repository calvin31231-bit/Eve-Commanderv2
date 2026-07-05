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
  undercut: boolean;
  best_competing: number | null;
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

export interface HistoryPoint {
  at: number;
  value: number;
}

export interface PortfolioHistory {
  networth: HistoryPoint[];
  sp: HistoryPoint[];
  networth_change: number;
  networth_change_pct: number;
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

export type PriceFlag = "normal" | "cheap" | "spike" | "manipulation";

export interface PriceAnomaly {
  deviation: number;
  median: number;
  flag: PriceFlag;
  note: string;
}

export interface MarketBrowse {
  quote: MarketQuote;
  history: HistoryStats;
  insurance: InsuranceLevel[] | null;
  hubs: HubQuote[];
  anomaly: PriceAnomaly;
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

export interface ArbitrageView {
  type_id: number;
  name: string;
  buy_hub: string;
  sell_hub: string;
  buy_price: number;
  sell_price: number;
  profit_per_unit: number;
  margin_pct: number;
  volume: number;
  profit_per_m3: number;
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

export interface RoiPlan {
  label: string;
  train_seconds: number;
  isk_per_hour: number;
  hours_per_day: number;
  upfront_isk: number;
}

export interface RoiResult {
  label: string;
  train_days: number;
  daily_gain: number;
  payback_days: number;
  roi_score: number;
}

export interface RemapView {
  intelligence: number;
  memory: number;
  perception: number;
  willpower: number;
  charisma: number;
  optimal_seconds: number;
  balanced_seconds: number;
  saved_seconds: number;
}

export interface IncomeActivity {
  name: string;
  isk_per_hour: number;
  risk: number;
  setup_cost: number;
  eligible: boolean;
}

export interface IncomeRanking {
  name: string;
  eligible: boolean;
  effective_isk_per_hour: number;
  session_profit: number;
}

export interface GatekeeperItem {
  type_id: number;
  name: string;
  needed: number;
  owned: number;
  missing: number;
  unit_price: number;
  missing_cost: number;
}

export interface FitGatekeeperView {
  parsed: boolean;
  ship: string;
  can_fly: boolean;
  missing_skills: MissingSkillView[];
  train_seconds: number;
  items: GatekeeperItem[];
  total_value: number;
  acquisition_cost: number;
  owned_fraction: number;
  unresolved: string[];
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

export interface DscanDiff {
  appeared: DscanGroup[];
  disappeared: DscanGroup[];
  warnings: string[];
}

export interface SrpPrefillView {
  pilot: string;
  ship: string;
  loss_value: number;
  location: string;
}

export interface ReconstructedFitView {
  eft: string;
  ship: string;
  pilot: string;
}

export interface ComplianceRow {
  character_id: number;
  character_name: string;
  can_fly: boolean;
  missing_count: number;
  train_seconds: number;
}

export interface DoctrineComplianceView {
  fit_name: string;
  ship: string;
  rows: ComplianceRow[];
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
  composition: string;
  unresolved: string[];
}

export interface PilotBackgroundView {
  found: boolean;
  name: string;
  corporation: string;
  alliance: string | null;
  security_status: number;
  birthday: string | null;
  level: "Safe" | "Neutral" | "Caution" | "Danger";
  reasons: string[];
  danger_ratio: number;
  ships_destroyed: number;
  ships_lost: number;
}

export interface GateCampView {
  system: string;
  found: boolean;
  kills_last_hour: number;
  level: "Safe" | "Neutral" | "Caution" | "Danger";
  message: string;
}

export interface SafetySystemView {
  system_id: number;
  name: string;
  security: number;
  kills_last_hour: number;
  jumps: number;
}

export interface SystemSafetyView {
  found: boolean;
  current: SafetySystemView | null;
  neighbors: SafetySystemView[];
  total_kills: number;
  level: "Safe" | "Neutral" | "Caution" | "Danger";
  message: string;
}

export interface SystemRiskView {
  found: boolean;
  system_id: number;
  system_name: string;
  score: number;
  level: "Safe" | "Neutral" | "Caution" | "Danger";
  reasons: string[];
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

export interface FleetPilot {
  name: string;
  summary: AarSummary;
}

export interface FleetAar {
  pilots: FleetPilot[];
  damage_dealt: number;
  damage_received: number;
  duration_seconds: number;
  dps_dealt: number;
  dps_received: number;
  top_targets: EntityDamage[];
  top_attackers: EntityDamage[];
}

export interface FleetAarView {
  found: boolean;
  fleet: FleetAar | null;
}

export interface LocalIntel {
  system: string | null;
  speakers: string[];
  line_count: number;
}

export interface ResearchAgentView {
  agent_name: string;
  datacore_name: string;
  points_per_day: number;
  remainder_points: number;
  started_at: string;
}

export interface IncursionView {
  staging_system: string;
  faction: string;
  state: string;
  influence_pct: number;
  has_boss: boolean;
  system_count: number;
}

export interface FwSystemView {
  system_name: string;
  owner: string;
  occupier: string;
  contested: string;
  progress_pct: number;
}

export interface MapNode {
  system_id: number;
  name: string;
  security: number;
  x: number;
  z: number;
  kills: number;
  sov_alliance_id: number;
  sov_owner: string;
  adm: number;
}

export interface RegionMapView {
  found: boolean;
  region_id: number;
  region_name: string;
  nodes: MapNode[];
  edges: [number, number][];
  message: string;
}

export interface RouteHop {
  system_id: number;
  name: string;
  security: number;
}

export interface AgentFinderView {
  corporation_name: string;
  division_name: string;
  level: number;
  is_locator: boolean;
  station_name: string;
  system_name: string;
  security: number;
}

export interface CalendarEvent {
  event_id: number;
  title: string;
  event_date: string;
  event_response: string;
  importance: number;
}

export interface TheraConnection {
  hub: string;
  destination: string;
  region: string;
  wh_type: string;
  max_ship_size: string;
  remaining_hours: number;
}

export interface JumpFatigue {
  jump_fatigue_expire_date: string | null;
  last_jump_date: string | null;
  last_update_date: string | null;
}

export interface LpOfferView {
  offer_id: number;
  name: string;
  quantity: number;
  lp_cost: number;
  total_isk_cost: number;
  output_value: number;
  profit: number;
  isk_per_lp: number;
}

export interface LpStoreView {
  found: boolean;
  corporation: string;
  offers: LpOfferView[];
  message: string;
}

export interface FleetMemberView {
  character_id: number;
  name: string;
  ship: string;
  system: string;
  role: string;
  jumps: number | null;
}

export interface HubQuoteView {
  hub: string;
  best_sell: number | null;
  best_buy: number | null;
}

export interface ShoppingLineView {
  name: string;
  quantity: number;
  best_hub: string;
  unit_price: number;
  line_total: number;
  volume: number;
  unresolved: boolean;
}

export interface ShoppingPlanView {
  lines: ShoppingLineView[];
  total_cost: number;
  total_volume: number;
  by_hub: [string, number][];
}

export interface HubTradeView {
  type_id: number;
  name: string;
  buy_hub: string;
  sell_hub: string;
  buy_price: number;
  sell_price: number;
  profit_per_unit: number;
  margin_pct: number;
  volume: number;
  units_per_trip: number;
  trip_profit: number;
  available_volume: number;
  jumps: number;
  route_kills: number;
  statement: string;
}

export interface HubBoardView {
  type_id: number;
  name: string;
  volume: number;
  hubs: HubQuoteView[];
  flip_buy_hub: string | null;
  flip_sell_hub: string | null;
  flip_profit: number | null;
  flip_margin_pct: number | null;
  sell_buy_hub: string | null;
  sell_sell_hub: string | null;
  sell_profit: number | null;
  sell_margin_pct: number | null;
}

export interface ItemPnlView {
  name: string;
  units_sold: number;
  revenue: number;
  cost: number;
  profit: number;
  margin_pct: number | null;
  units_open: number;
  open_cost: number;
}

export interface TradingPnlView {
  total_revenue: number;
  total_cost: number;
  total_profit: number;
  total_net: number;
  items: ItemPnlView[];
}

export interface ContactView {
  contact_id: number;
  name: string;
  contact_type: string;
  standing: number;
  is_blocked: boolean;
  is_watched: boolean;
}

export interface GameFitView {
  fitting_id: number;
  name: string;
  ship: string;
  eft: string;
}

export interface LiveKillView {
  killmail_id: number;
  system_name: string;
  ship_name: string;
  total_value: number;
  age_seconds: number;
}

export interface EsiHealthView {
  budget_remaining: number;
  backoff_seconds: number;
}

export interface FleetView {
  in_fleet: boolean;
  member_count: number;
  members: FleetMemberView[];
}

export interface FleetSquadView {
  id: number;
  name: string;
}

export interface FleetWingView {
  id: number;
  name: string;
  squads: FleetSquadView[];
}

export interface CorpMemberView {
  character_id: number;
  name: string;
  ship_name: string;
  location_name: string;
  logon_date: string | null;
  logoff_date: string | null;
}

export interface ContainerTheftView {
  character_name: string;
  action: string;
  item_name: string;
  quantity: number;
  logged_at: string | null;
  severity: number;
  reason: string;
}

export interface ExtractionView {
  structure_name: string;
  moon_name: string;
  chunk_arrival_time: string | null;
  natural_decay_time: string | null;
  arrival_seconds_remaining: number;
  ready: boolean;
}

export interface CorpStructureView {
  structure_id: number;
  name: string;
  type_name: string;
  system_name: string;
  state: string;
  fuel_seconds_remaining: number;
  has_fuel_timer: boolean;
}

export interface CourierView {
  found: boolean;
  jumps: number;
  reward_per_jump: number;
  reward_per_m3: number;
  collateral_ratio: number;
  lowsec_hops: number;
  kills_on_route: number;
  suggested_reward: number;
  verdict: string;
  hops: RouteHop[];
  message: string;
}

export interface RouteView {
  found: boolean;
  jumps: number;
  hops: RouteHop[];
  message: string;
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

export interface AiSettingsView {
  enabled: boolean;
  base_url: string;
  model: string;
  embed_model: string;
  has_api_key: boolean;
}

export interface AiEndpointView {
  label: string;
  base_url: string;
  models: string[];
}

export interface UpdateStatus {
  current: string;
  latest: string;
  update_available: boolean;
  url: string;
}

export interface TelemetryEventView {
  name: string;
  counts_json: string;
  created_at: number;
}

export interface SharedImportView {
  kind: string;
  name: string;
}

export interface WidgetSlot {
  id: string;
  visible: boolean;
}

export interface PluginPanel {
  title: string;
  command: string;
  args: unknown;
}

export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  panels: PluginPanel[];
}

export interface PluginView {
  manifest: PluginManifest;
  errors: string[];
}

// Mirrors eve_core::ai::ChatMessage (tool fields omitted for the simple UI).
export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string;
}

export interface AiChatView {
  reply: string;
  tools_used: string[];
}

export interface MemoryNoteView {
  id: number;
  kind: string;
  title: string;
  body: string;
  salience: number;
  pinned: boolean;
  created_at: number;
}

export interface ImportedSkillView {
  skill_type_id: number;
  name: string;
  target_level: number;
}

export interface SkillImportView {
  targets: ImportedSkillView[];
  unresolved: string[];
}

export interface SavedPlanView {
  id: number;
  name: string;
  body: string;
  updated_at: number;
}

export interface SavedFitView {
  id: number;
  name: string;
  ship: string;
  eft: string;
  updated_at: number;
}

export interface RealizedIncome {
  net: number;
  active_days: number;
  isk_per_day: number;
}

export interface FitStatsView {
  found: boolean;
  ship: string;
  shield_ehp: number;
  armor_ehp: number;
  hull_ehp: number;
  total_ehp: number;
  dps: number;
  volley: number;
  shield_rps: number;
  armor_rps: number;
  cap_capacity: number;
  cap_peak_recharge: number;
  cap_load: number;
  cap_stable: boolean;
  cap_seconds_to_empty: number | null;
  hull_bonuses: string[];
  dps_curve: DpsPoint[];
  note: string;
}

export interface DpsPoint {
  range_km: number;
  dps: number;
}

export interface ImplantView {
  type_id: number;
  name: string;
  slot: number;
  category: string;
}

export interface SavedLoadoutView {
  id: number;
  name: string;
  implant_ids: number[];
  updated_at: number;
}

export interface ImplantValueView {
  total: number;
  lines: [number, number][];
}

export interface PodRiskView {
  found: boolean;
  system_name: string;
  security: number;
  implant_value: number;
  implant_count: number;
  danger: boolean;
  message: string;
}

export interface AbyssRun {
  id: number;
  ran_at: number;
  tier: number;
  weather: string;
  ship: string;
  fit: string;
  duration_seconds: number;
  loot_value: number;
  survived: boolean;
  notes: string;
}

export interface TierStat {
  tier: number;
  runs: number;
  avg_loot: number;
  avg_seconds: number;
}

export interface AbyssStats {
  runs: number;
  deaths: number;
  survival_rate: number;
  total_loot: number;
  avg_loot: number;
  total_seconds: number;
  avg_seconds: number;
  isk_per_hour: number;
  best_loot: number;
  by_tier: TierStat[];
}

export interface AbyssTrackerView {
  runs: AbyssRun[];
  stats: AbyssStats;
}

export interface LootLineView {
  name: string;
  quantity: number;
  unit_price: number;
  value: number;
}

export interface LootValueView {
  lines: LootLineView[];
  total: number;
  unresolved: string[];
}

export interface RollPlan {
  per_pass_mass: number;
  passes_to_reduced: number;
  passes_to_critical: number;
  safe_passes: number;
  collapse_earliest: number;
  collapse_latest: number;
  warning: string | null;
}

export interface SrpClaim {
  id: number;
  submitted_at: number;
  pilot: string;
  ship: string;
  loss_value: number;
  location: string;
  killmail_url: string;
  notes: string;
  status: string;
  payout: number;
  reviewer_note: string;
  decided_at: number | null;
}

export interface SrpSummary {
  total: number;
  pending: number;
  approved_unpaid: number;
  paid: number;
  rejected: number;
  total_loss: number;
  outstanding: number;
  total_paid: number;
}

export interface SrpBoardView {
  claims: SrpClaim[];
  summary: SrpSummary;
}

export interface Recruit {
  id: number;
  applied_at: number;
  name: string;
  source: string;
  notes: string;
  status: string;
  recruiter: string;
  reviewer_note: string;
  decided_at: number | null;
}

export interface RecruitSummary {
  total: number;
  applied: number;
  interview: number;
  trial: number;
  accepted: number;
  rejected: number;
  active: number;
  acceptance_rate: number;
}

export interface RecruitBoardView {
  recruits: Recruit[];
  summary: RecruitSummary;
}

export interface SignatureView {
  id: number;
  system: string;
  sig_id: string;
  category: string;
  name: string;
  wh_type: string;
  destination: string;
  mass_state: string;
  eol: boolean;
  notes: string;
}

export interface TimerView {
  id: number;
  title: string;
  system: string;
  structure: string;
  timer_type: string;
  side: string;
  exits_at: number;
  seconds_remaining: number;
  notes: string;
}

export interface AiAgentView {
  id: string;
  name: string;
  description: string;
}
