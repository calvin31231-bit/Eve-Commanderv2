//! The ESI client and supporting machinery.
//!
//! - [`ratelimit`] — the error-budget circuit breaker that keeps us within
//!   CCP's limits across many characters.
//! - [`cache`]     — cache freshness (Expires / ETag) so we never poll faster
//!   than an endpoint allows and so "refreshes" are nearly free.
//! - [`scheduler`] — tiered poll classes + active/idle cadence scaling that
//!   keep the resource footprint tiny.
//! - [`client`]    — the cache-first HTTP client tying it together.

pub mod cache;
pub mod cache_store;
pub mod client;
pub mod endpoints;
pub mod ratelimit;
pub mod scheduler;

pub use cache_store::SqliteCacheStore;
pub use client::{CacheStore, EsiClient, MemoryCacheStore};
pub use endpoints::{all_jobs, Endpoint, CATALOG};
pub use ratelimit::ErrorBudget;
pub use scheduler::{PollClass, PollJob, Scheduler};
