// Thin, typed wrappers around the Tauri IPC commands exposed by `src-tauri`.
//
// When the frontend is opened in a plain browser (e.g. `vite dev` without the
// desktop shell), `window.__TAURI_INTERNALS__` is absent. We detect that and
// fail gracefully so the UI can still render for design work.

import { invoke } from "@tauri-apps/api/core";
import type {
  AccountOverview,
  AppSettings,
  CashflowSummary,
  Character,
  CharacterGroup,
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
  Notification,
  ServerStatus,
  TradeOpportunity,
  ReprocessView,
  BuildPlanView,
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
  getCashflow: (characterId: number) =>
    call<CashflowSummary>("get_cashflow", { characterId }),
  getIndustryJobs: (characterId: number) =>
    call<IndustryJobView[]>("get_industry_jobs", { characterId }),
  getMarketOrders: (characterId: number) =>
    call<MarketView>("get_market_orders", { characterId }),
  getContracts: (characterId: number, limit: number) =>
    call<Contract[]>("get_contracts", { characterId, limit }),
  searchItems: (query: string, limit: number) =>
    call<ItemHit[]>("search_items", { query, limit }),
  getMarketBrowse: (typeId: number) =>
    call<MarketBrowse>("get_market_browse", { typeId }),
  scanStationTrades: (
    typeIds?: number[],
    brokerFee?: number,
    salesTax?: number,
  ) =>
    call<TradeOpportunity[]>("scan_station_trades", { typeIds, brokerFee, salesTax }),
  reprocessItem: (typeId: number, units: number, efficiency?: number) =>
    call<ReprocessView | null>("reprocess_item", { typeId, units, efficiency }),
  planBuild: (productTypeId: number, runs: number, me: number, activity?: string) =>
    call<BuildPlanView | null>("plan_build", { productTypeId, runs, me, activity }),
  getMining: (characterId: number) =>
    call<MiningView>("get_mining", { characterId }),
  getMailHeaders: (characterId: number) =>
    call<MailHeader[]>("get_mail_headers", { characterId }),
  getMail: (characterId: number, mailId: number) =>
    call<MailView>("get_mail", { characterId, mailId }),
  markMailRead: (characterId: number, mailId: number) =>
    call<void>("mark_mail_read", { characterId, mailId }),
  listGroups: () => call<CharacterGroup[]>("list_groups"),
  createGroup: (name: string) => call<CharacterGroup>("create_group", { name }),
  deleteGroup: (groupId: number) => call<void>("delete_group", { groupId }),
  addGroupMember: (groupId: number, characterId: number) =>
    call<void>("add_group_member", { groupId, characterId }),
  removeGroupMember: (groupId: number, characterId: number) =>
    call<void>("remove_group_member", { groupId, characterId }),
  getSettings: () => call<AppSettings>("get_settings"),
  setSettings: (intensity: string, notifyMin: string) =>
    call<void>("set_settings", { intensity, notifyMin }),
  listNotifications: () => call<Notification[]>("list_notifications"),
  unreadNotifications: () => call<number>("unread_notifications"),
  markNotificationsRead: () => call<void>("mark_notifications_read"),
  dismissNotification: (key: string) =>
    call<void>("dismiss_notification", { key }),
};
