//! Secure storage for **refresh tokens** only.
//!
//! Access tokens are short-lived and kept in memory by the caller. Refresh
//! tokens are long-lived secrets and go in the OS keychain (macOS Keychain,
//! Windows Credential Manager, Linux Secret Service) when the `keychain`
//! feature is enabled. A volatile in-memory fallback is used otherwise (e.g.
//! in headless tests) so the rest of the system can be exercised.

use crate::error::{Error, Result};

const SERVICE: &str = "eve-commander";

/// Abstraction over a refresh-token store, keyed by character id.
pub trait TokenStore: Send + Sync {
    fn save_refresh_token(&self, character_id: i64, token: &str) -> Result<()>;
    fn load_refresh_token(&self, character_id: i64) -> Result<Option<String>>;
    fn delete_refresh_token(&self, character_id: i64) -> Result<()>;
}

/// OS-keychain backed store (only available with the `keychain` feature).
#[cfg(feature = "keychain")]
pub struct KeychainTokenStore;

#[cfg(feature = "keychain")]
impl KeychainTokenStore {
    fn entry(character_id: i64) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, &format!("refresh:{character_id}"))
            .map_err(|e| Error::Auth(format!("keychain entry error: {e}")))
    }
}

#[cfg(feature = "keychain")]
impl TokenStore for KeychainTokenStore {
    fn save_refresh_token(&self, character_id: i64, token: &str) -> Result<()> {
        Self::entry(character_id)?
            .set_password(token)
            .map_err(|e| Error::Auth(format!("keychain save error: {e}")))
    }

    fn load_refresh_token(&self, character_id: i64) -> Result<Option<String>> {
        match Self::entry(character_id)?.get_password() {
            Ok(t) => Ok(Some(t)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Auth(format!("keychain load error: {e}"))),
        }
    }

    fn delete_refresh_token(&self, character_id: i64) -> Result<()> {
        match Self::entry(character_id)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::Auth(format!("keychain delete error: {e}"))),
        }
    }
}

/// Volatile in-memory store. Used as a fallback and in tests. Never persists
/// secrets to disk.
#[derive(Default)]
pub struct MemoryTokenStore {
    inner: std::sync::Mutex<std::collections::HashMap<i64, String>>,
}

impl TokenStore for MemoryTokenStore {
    fn save_refresh_token(&self, character_id: i64, token: &str) -> Result<()> {
        self.inner
            .lock()
            .map_err(|_| Error::other("token store poisoned"))?
            .insert(character_id, token.to_string());
        Ok(())
    }

    fn load_refresh_token(&self, character_id: i64) -> Result<Option<String>> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| Error::other("token store poisoned"))?
            .get(&character_id)
            .cloned())
    }

    fn delete_refresh_token(&self, character_id: i64) -> Result<()> {
        self.inner
            .lock()
            .map_err(|_| Error::other("token store poisoned"))?
            .remove(&character_id);
        Ok(())
    }
}

/// Returns the best available token store: the OS keychain when compiled with
/// the `keychain` feature, otherwise a volatile in-memory store.
pub fn default_store() -> Box<dyn TokenStore> {
    #[cfg(feature = "keychain")]
    {
        Box::new(KeychainTokenStore)
    }
    #[cfg(not(feature = "keychain"))]
    {
        Box::new(MemoryTokenStore::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrip() {
        let store = MemoryTokenStore::default();
        assert_eq!(store.load_refresh_token(42).unwrap(), None);
        store.save_refresh_token(42, "secret").unwrap();
        assert_eq!(store.load_refresh_token(42).unwrap().as_deref(), Some("secret"));
        store.delete_refresh_token(42).unwrap();
        assert_eq!(store.load_refresh_token(42).unwrap(), None);
    }
}
