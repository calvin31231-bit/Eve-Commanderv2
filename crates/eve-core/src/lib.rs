//! # eve-core
//!
//! Core, GUI-independent logic for EVE Commander — the master EVE Online companion.
//!
//! This crate deliberately has **no Tauri / WebView dependency** so it can be
//! compiled and unit-tested in headless environments. The desktop shell
//! (`src-tauri`) depends on this crate and exposes its functionality over IPC.
//!
//! Module map (see the project plan for the full architecture):
//! - [`auth`]   — EVE SSO OAuth2 + PKCE and token storage.
//! - [`esi`]    — the ESI client: cache-first requests, error-budget rate
//!   limiting, and the tiered background poll scheduler.
//! - [`model`]  — account / character / group data model.
//! - [`config`] — application configuration and on-disk paths.
//! - [`error`]  — the crate-wide error type.
//!
//! ## EULA note
//! Everything here is a **read / display / analysis** layer over the public
//! ESI API. There is no input automation, no game-memory reading, and no
//! disallowed cache scraping. See `docs/EULA-notes.md`.

pub mod abyss;
pub mod account;
pub mod ai;
pub mod ai_memory;
pub mod assets;
pub mod auth;
pub mod calendar;
pub mod character;
pub mod clones;
pub mod config;
pub mod contacts;
pub mod contracts;
pub mod corp;
pub mod courier;
pub mod dashboard;
pub mod db;
pub mod discord;
pub mod dogma;
pub mod dscan;
pub mod error;
pub mod esi;
pub mod esi_notifications;
pub mod eve_scout;
pub mod fitsync;
pub mod fitting;
pub mod fleet;
pub mod implants;
pub mod income;
pub mod industry;
pub mod industry_plan;
pub mod intel;
pub mod insurance;
pub mod killmail;
pub mod logs;
pub mod loot;
pub mod loyalty;
pub mod lp;
pub mod mail;
pub mod market;
pub mod market_universe;
pub mod marketdata;
pub mod mcp;
pub mod mining;
pub mod model;
pub mod names;
pub mod navigation;
pub mod notify;
pub mod planets;
pub mod plugin;
pub mod prices;
pub mod pve;
pub mod recruit;
pub mod redisq;
pub mod remap;
pub mod reprocess;
pub mod research;
pub mod sde;
pub mod share;
pub mod shopping;
pub mod signatures;
pub mod skillplan;
pub mod skillplan_import;
pub mod srp;
pub mod telemetry;
pub mod trading;
pub mod universe;
pub mod update;
pub mod wallet;
pub mod wormhole;

pub use error::{Error, Result};
