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

pub mod account;
pub mod assets;
pub mod auth;
pub mod bookmarks;
pub mod calendar;
pub mod character;
pub mod clones;
pub mod config;
pub mod contracts;
pub mod corp;
pub mod courier;
pub mod db;
pub mod discord;
pub mod dscan;
pub mod error;
pub mod esi;
pub mod fitting;
pub mod industry;
pub mod industry_plan;
pub mod intel;
pub mod insurance;
pub mod logs;
pub mod lp;
pub mod mail;
pub mod market;
pub mod marketdata;
pub mod mining;
pub mod model;
pub mod names;
pub mod navigation;
pub mod notify;
pub mod planets;
pub mod prices;
pub mod pve;
pub mod reprocess;
pub mod research;
pub mod sde;
pub mod skillplan;
pub mod universe;
pub mod wallet;

pub use error::{Error, Result};
