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

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod esi;
pub mod model;
pub mod notify;
pub mod sde;

pub use error::{Error, Result};
