// Thin, typed wrappers around the Tauri IPC commands exposed by `src-tauri`.
//
// When the frontend is opened in a plain browser (e.g. `vite dev` without the
// desktop shell), `window.__TAURI_INTERNALS__` is absent. We detect that and
// fail gracefully so the UI can still render for design work.

import { invoke } from "@tauri-apps/api/core";
import type {
  AccountOverview,
  PortfolioHistory,
  AppSettings,
  CalendarEvent,
  CashflowSummary,
  RealizedIncome,
  Character,
  CharacterGroup,
  ColonyView,
  CorpStructureView,
  CorpMemberView,
  FleetView,
  LpStoreView,
  Contract,
  CharacterAttributes,
  CharacterProfile,
  CharacterSheet,
  CharacterStatusView,
  ClonesView,
  QueuedSkillView,
  TransactionView,
  HoldingsView,
  IndustryJobView,
  ItemHit,
  LocationValueView,
  MarketBrowse,
  MailHeader,
  MailView,
  MarketView,
  MiningView,
  ResearchAgentView,
  Notification,
  ServerStatus,
  TradeOpportunity,
  ArbitrageView,
  ReprocessView,
  BuildPlanView,
  ResolvedFit,
  FitStatsView,
  ImplantView,
  ImplantValueView,
  SavedLoadoutView,
  SkillPlanView,
  SkillImportView,
  RemapView,
  RoiPlan,
  RoiResult,
  IncomeActivity,
  IncomeRanking,
  CanFlyView,
  FitGatekeeperView,
  DoctrineView,
  DscanResult,
  ThreatScanView,
  PilotBackgroundView,
  GateCampView,
  SystemSafetyView,
  SystemRiskView,
  PodRiskView,
  CombatLogView,
  FleetAarView,
  IncursionView,
  AbyssTrackerView,
  LootValueView,
  FwSystemView,
  LocalIntel,
  RouteView,
  CourierView,
  RegionMapView,
  JumpFatigue,
  TheraConnection,
  RollPlan,
  SrpBoardView,
  RecruitBoardView,
  AiSettingsView,
  AiEndpointView,
  ChatMessage,
  AiChatView,
  MemoryNoteView,
  SavedPlanView,
  SavedFitView,
} from "./types";

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error(`IPC '${cmd}' unavailable: not running inside the desktop shell`);
  }
  return invoke<T>(cmd, args);
}

export const api = {
  serverStatus: () => call<ServerStatus>("server_status"),
  listCharacters: () => call<Character[]>("list_characters"),
  getAccountOverview: () => call<AccountOverview>("get_account_overview"),
  getPortfolioHistory: (characterId: number | null, days: number) =>
    call<PortfolioHistory>("get_portfolio_history", { characterId, days }),
  // Runs the whole SSO flow on the backend (opens the browser, captures the
  // loopback redirect) and resolves with the newly-added character.
  login: () => call<Character>("login"),
  setActiveCharacter: (characterId: number) =>
    call<void>("set_active_character", { characterId }),
  removeCharacter: (characterId: number) =>
    call<void>("remove_character", { characterId }),
  getCharacterSheet: (characterId: number) =>
    call<CharacterSheet>("get_character_sheet", { characterId }),
  getCharacterStatus: (characterId: number) =>
    call<CharacterStatusView>("get_character_status", { characterId }),
  getCharacterProfile: (characterId: number) =>
    call<CharacterProfile>("get_character_profile", { characterId }),
  getSkillQueue: (characterId: number) =>
    call<QueuedSkillView[]>("get_skill_queue", { characterId }),
  getAttributes: (characterId: number) =>
    call<CharacterAttributes>("get_attributes", { characterId }),
  getTransactions: (characterId: number, limit: number) =>
    call<TransactionView[]>("get_transactions", { characterId, limit }),
  getTopHoldings: (characterId: number, limit: number) =>
    call<HoldingsView>("get_top_holdings", { characterId, limit }),
  getAssetsByLocation: (characterId: number, limit: number) =>
    call<LocationValueView[]>("get_assets_by_location", { characterId, limit }),
  getClones: (characterId: number) =>
    call<ClonesView>("get_clones", { characterId }),
  getCalendar: (characterId: number) =>
    call<CalendarEvent[]>("get_calendar", { characterId }),
  getCashflow: (characterId: number) =>
    call<CashflowSummary>("get_cashflow", { characterId }),
  getRealizedIncome: (characterId: number | null) =>
    call<RealizedIncome>("get_realized_income", { characterId }),
  getIndustryJobs: (characterId: number) =>
    call<IndustryJobView[]>("get_industry_jobs", { characterId }),
  getMarketOrders: (characterId: number) =>
    call<MarketView>("get_market_orders", { characterId }),
  getContracts: (characterId: number, limit: number) =>
    call<Contract[]>("get_contracts", { characterId, limit }),
  searchItems: (query: string, limit: number) =>
    call<ItemHit[]>("search_items", { query, limit }),
  getResearchAgents: (characterId: number) =>
    call<ResearchAgentView[]>("get_research_agents", { characterId }),
  getMarketBrowse: (typeId: number) =>
    call<MarketBrowse>("get_market_browse", { typeId }),
  scanStationTrades: (
    typeIds?: number[],
    brokerFee?: number,
    salesTax?: number,
  ) =>
    call<TradeOpportunity[]>("scan_station_trades", { typeIds, brokerFee, salesTax }),
  scanArbitrage: (typeIds?: number[], salesTax?: number) =>
    call<ArbitrageView[]>("scan_arbitrage", { typeIds, salesTax }),
  reprocessItem: (typeId: number, units: number, efficiency?: number) =>
    call<ReprocessView | null>("reprocess_item", { typeId, units, efficiency }),
  planBuild: (productTypeId: number, runs: number, me: number, activity?: string) =>
    call<BuildPlanView | null>("plan_build", { productTypeId, runs, me, activity }),
  parseFit: (eft: string) => call<ResolvedFit | null>("parse_fit", { eft }),
  fitStats: (eft: string, characterId: number | null) =>
    call<FitStatsView>("fit_stats", { eft, characterId }),
  listImplants: () => call<ImplantView[]>("list_implants"),
  valueImplants: (typeIds: number[]) =>
    call<ImplantValueView>("value_implants", { typeIds }),
  saveImplantLoadout: (name: string, implantIds: number[]) =>
    call<number>("save_implant_loadout", { name, implantIds }),
  listImplantLoadouts: () => call<SavedLoadoutView[]>("list_implant_loadouts"),
  deleteImplantLoadout: (id: number) =>
    call<void>("delete_implant_loadout", { id }),
  canFlyFit: (characterId: number, eft: string) =>
    call<CanFlyView>("can_fly_fit", { characterId, eft }),
  fitGatekeeper: (characterId: number, eft: string) =>
    call<FitGatekeeperView>("fit_gatekeeper", { characterId, eft }),
  doctrineCheck: (eft: string) => call<DoctrineView>("doctrine_check", { eft }),
  parseDscan: (text: string) => call<DscanResult>("parse_dscan", { text }),
  scanPilots: (names: string[]) => call<ThreatScanView>("scan_pilots", { names }),
  pilotBackground: (name: string) =>
    call<PilotBackgroundView>("pilot_background", { name }),
  // Reads the OS clipboard via the clipboard-manager plugin (for the threat
  // scanner's "watch" mode — copy Local in-game to auto-rescan).
  readClipboard: () => call<string>("plugin:clipboard-manager|read_text"),
  gateCampCheck: (system: string) => call<GateCampView>("gate_camp_check", { system }),
  getSystemSafety: () => call<SystemSafetyView>("get_system_safety"),
  getPodRisk: () => call<PodRiskView>("get_pod_risk"),
  getSystemRisk: () => call<SystemRiskView>("get_system_risk"),
  getRegionMap: (region?: string) => call<RegionMapView>("get_region_map", { region }),
  listMapRegions: () => call<string[]>("list_map_regions"),
  planRoute: (origin: string, destination: string, flag: string) =>
    call<RouteView>("plan_route", { origin, destination, flag }),
  courierEstimate: (
    origin: string,
    destination: string,
    volume: number,
    collateral: number,
    reward: number,
    flag: string,
  ) =>
    call<CourierView>("courier_estimate", {
      origin,
      destination,
      volume,
      collateral,
      reward,
      flag,
    }),
  rollWormhole: (totalMass: number, maxJumpMass: number, shipPassMass: number) =>
    call<RollPlan>("roll_wormhole", { totalMass, maxJumpMass, shipPassMass }),
  submitSrpClaim: (claim: {
    pilot: string;
    ship: string;
    loss_value: number;
    location: string;
    killmail_url: string;
    notes: string;
  }) =>
    call<number>("submit_srp_claim", {
      pilot: claim.pilot,
      ship: claim.ship,
      lossValue: claim.loss_value,
      location: claim.location,
      killmailUrl: claim.killmail_url,
      notes: claim.notes,
    }),
  getSrpBoard: () => call<SrpBoardView>("get_srp_board"),
  decideSrpClaim: (id: number, status: string, payout: number, reviewerNote: string) =>
    call<void>("decide_srp_claim", { id, status, payout, reviewerNote }),
  markSrpPaid: (id: number) => call<void>("mark_srp_paid", { id }),
  deleteSrpClaim: (id: number) => call<void>("delete_srp_claim", { id }),
  submitRecruit: (name: string, source: string, notes: string, recruiter: string) =>
    call<number>("submit_recruit", { name, source, notes, recruiter }),
  getRecruitBoard: () => call<RecruitBoardView>("get_recruit_board"),
  setRecruitStatus: (id: number, status: string, reviewerNote: string) =>
    call<void>("set_recruit_status", { id, status, reviewerNote }),
  deleteRecruit: (id: number) => call<void>("delete_recruit", { id }),
  getJumpFatigue: (characterId: number) =>
    call<JumpFatigue | null>("get_jump_fatigue", { characterId }),
  getTheraConnections: () => call<TheraConnection[]>("get_thera_connections"),
  setRouteWaypoint: (characterId: number, system: string) =>
    call<void>("set_route_waypoint", { characterId, system }),
  openMarketWindow: (characterId: number, typeId: number) =>
    call<void>("open_market_window", { characterId, typeId }),
  getCombatSummary: () => call<CombatLogView>("get_combat_summary"),
  getFleetAar: (maxPilots?: number) =>
    call<FleetAarView>("get_fleet_aar", { maxPilots }),
  getLocalIntel: () => call<LocalIntel | null>("get_local_intel"),
  getIncursions: () => call<IncursionView[]>("get_incursions"),
  getAbyssTracker: () => call<AbyssTrackerView>("get_abyss_tracker"),
  logAbyssRun: (run: {
    tier: number;
    weather: string;
    ship: string;
    fit: string;
    duration_seconds: number;
    loot_value: number;
    survived: boolean;
    notes: string;
  }) =>
    call<number>("log_abyss_run", {
      tier: run.tier,
      weather: run.weather,
      ship: run.ship,
      fit: run.fit,
      durationSeconds: run.duration_seconds,
      lootValue: run.loot_value,
      survived: run.survived,
      notes: run.notes,
      ranAt: null,
    }),
  deleteAbyssRun: (id: number) => call<void>("delete_abyss_run", { id }),
  valueLoot: (text: string) => call<LootValueView>("value_loot", { text }),
  getFwSystems: () => call<FwSystemView[]>("get_fw_systems"),
  costSkillPlan: (
    characterId: number,
    targets: { skill_type_id: number; target_level: number }[],
  ) => call<SkillPlanView>("cost_skill_plan", { characterId, targets }),
  optimizeRemap: (
    characterId: number,
    targets: { skill_type_id: number; target_level: number }[],
  ) => call<RemapView>("optimize_remap", { characterId, targets }),
  importSkillPlan: (text: string) =>
    call<SkillImportView>("import_skill_plan", { text }),
  rankSkillRoi: (plans: RoiPlan[]) => call<RoiResult[]>("rank_skill_roi", { plans }),
  rankIncome: (activities: IncomeActivity[], hours: number) =>
    call<IncomeRanking[]>("rank_income", { activities, hours }),
  getMining: (characterId: number) =>
    call<MiningView>("get_mining", { characterId }),
  getPlanets: (characterId: number) =>
    call<ColonyView[]>("get_planets", { characterId }),
  getMailHeaders: (characterId: number) =>
    call<MailHeader[]>("get_mail_headers", { characterId }),
  getMail: (characterId: number, mailId: number) =>
    call<MailView>("get_mail", { characterId, mailId }),
  markMailRead: (characterId: number, mailId: number) =>
    call<void>("mark_mail_read", { characterId, mailId }),
  getCorpStructures: (characterId: number) =>
    call<CorpStructureView[]>("get_corp_structures", { characterId }),
  getCorpMembers: (characterId: number) =>
    call<CorpMemberView[]>("get_corp_members", { characterId }),
  getFleet: (characterId: number) => call<FleetView>("get_fleet", { characterId }),
  lpStore: (corporation: string) => call<LpStoreView>("lp_store", { corporation }),
  listGroups: () => call<CharacterGroup[]>("list_groups"),
  createGroup: (name: string) => call<CharacterGroup>("create_group", { name }),
  deleteGroup: (groupId: number) => call<void>("delete_group", { groupId }),
  addGroupMember: (groupId: number, characterId: number) =>
    call<void>("add_group_member", { groupId, characterId }),
  removeGroupMember: (groupId: number, characterId: number) =>
    call<void>("remove_group_member", { groupId, characterId }),
  getSettings: () => call<AppSettings>("get_settings"),
  setSettings: (intensity: string, notifyMin: string, discordWebhook: string) =>
    call<void>("set_settings", { intensity, notifyMin, discordWebhook }),
  listNotifications: () => call<Notification[]>("list_notifications"),
  unreadNotifications: () => call<number>("unread_notifications"),
  markNotificationsRead: () => call<void>("mark_notifications_read"),
  dismissNotification: (key: string) =>
    call<void>("dismiss_notification", { key }),
  getAiSettings: () => call<AiSettingsView>("get_ai_settings"),
  setAiSettings: (
    enabled: boolean,
    baseUrl: string,
    model: string,
    apiKey: string | null,
  ) => call<void>("set_ai_settings", { enabled, baseUrl, model, apiKey }),
  aiDetectEndpoints: () => call<AiEndpointView[]>("ai_detect_endpoints"),
  aiChat: (messages: ChatMessage[]) =>
    call<AiChatView>("ai_chat", { messages }),
  aiBriefing: () => call<AiChatView>("ai_briefing"),
  addMemory: (kind: string, title: string, body: string) =>
    call<number | null>("add_memory", { kind, title, body }),
  listMemory: () => call<MemoryNoteView[]>("list_memory"),
  forgetMemory: (id: number) => call<void>("forget_memory", { id }),
  pinMemory: (id: number, pinned: boolean) =>
    call<void>("pin_memory", { id, pinned }),
  exportData: () => call<string>("export_data"),
  wipeData: () => call<void>("wipe_data"),
  saveSkillPlan: (name: string, body: string) =>
    call<number>("save_skill_plan", { name, body }),
  listSkillPlans: () => call<SavedPlanView[]>("list_skill_plans"),
  deleteSkillPlan: (id: number) => call<void>("delete_skill_plan", { id }),
  saveFit: (name: string, eft: string) =>
    call<number>("save_fit", { name, eft }),
  listFits: () => call<SavedFitView[]>("list_fits"),
  deleteFit: (id: number) => call<void>("delete_fit", { id }),
};
