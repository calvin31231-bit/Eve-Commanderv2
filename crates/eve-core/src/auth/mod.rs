//! EVE SSO authentication.
//!
//! EVE Commander is a distributed desktop app, so it uses the OAuth2
//! **Authorization Code flow with PKCE** (no embedded client secret). The flow:
//!
//! 1. Generate a [`pkce::PkcePair`] and a random `state`.
//! 2. Open [`sso::authorize_url`] in the system browser.
//! 3. Capture the `code` on the loopback redirect, verify `state`.
//! 4. Exchange `code` + `verifier` for tokens via [`sso::exchange_code`].
//! 5. Persist only the **refresh token** (in the OS keychain when the
//!    `keychain` feature is enabled); keep access tokens in memory.

pub mod flow;
pub mod pkce;
pub mod sso;
pub mod token;
pub mod token_store;

pub use flow::{CompletedLogin, LoginManager};
pub use pkce::PkcePair;
pub use sso::{SsoClient, TokenResponse};
pub use token::{decode_claims, Claims};
