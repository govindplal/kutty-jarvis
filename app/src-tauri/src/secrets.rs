//! API keys for the optional cloud providers (ADR 0015).
//!
//! Keys live only in the OS credential store, one entry per provider. They
//! go in through [`save`], and nothing but "is one saved" and its last four
//! characters ever comes back out to the frontend.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[cfg(windows)]
const SERVICE: &str = "kutty-jarvis";
const MIN_KEY_LEN: usize = 16;
const MAX_KEY_LEN: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyProvider {
    Anthropic,
    Openai,
}

impl KeyProvider {
    pub fn label(self) -> &'static str {
        match self {
            KeyProvider::Anthropic => "Anthropic",
            KeyProvider::Openai => "OpenAI",
        }
    }

    #[cfg(windows)]
    fn account(self) -> &'static str {
        match self {
            KeyProvider::Anthropic => "anthropic",
            KeyProvider::Openai => "openai",
        }
    }
}

/// Where keys are kept. Errors never contain key material.
pub trait KeyStore: Send + Sync {
    fn get(&self, provider: KeyProvider) -> Result<Option<String>, String>;
    fn set(&self, provider: KeyProvider, key: &str) -> Result<(), String>;
    /// Succeeds when there was nothing to delete.
    fn delete(&self, provider: KeyProvider) -> Result<(), String>;
}

/// Loose sanity check so a pasted sentence or empty field is caught early.
/// Providers decide what is really valid.
pub fn validate_key(key: &str) -> Result<&str, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("paste an API key first".into());
    }
    if key.chars().any(char::is_whitespace) {
        return Err("an API key has no spaces or line breaks in it".into());
    }
    if !key.is_ascii() {
        return Err("an API key only has plain letters, digits and symbols".into());
    }
    if !(MIN_KEY_LEN..=MAX_KEY_LEN).contains(&key.len()) {
        return Err(format!(
            "an API key is {MIN_KEY_LEN} to {MAX_KEY_LEN} characters long"
        ));
    }
    Ok(key)
}

/// What the frontend may know about a saved key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStatus {
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last4: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyStatuses {
    pub anthropic: KeyStatus,
    pub openai: KeyStatus,
}

pub fn status(store: &dyn KeyStore, provider: KeyProvider) -> Result<KeyStatus, String> {
    Ok(match store.get(provider)? {
        Some(key) => {
            let tail: Vec<char> = key.chars().rev().take(4).collect();
            KeyStatus {
                configured: true,
                last4: Some(tail.into_iter().rev().collect()),
            }
        }
        None => KeyStatus {
            configured: false,
            last4: None,
        },
    })
}

pub fn statuses(store: &dyn KeyStore) -> Result<KeyStatuses, String> {
    Ok(KeyStatuses {
        anthropic: status(store, KeyProvider::Anthropic)?,
        openai: status(store, KeyProvider::Openai)?,
    })
}

pub fn save(store: &dyn KeyStore, provider: KeyProvider, key: &str) -> Result<(), String> {
    store.set(provider, validate_key(key)?)
}

#[cfg(windows)]
struct CredentialManager;

#[cfg(windows)]
impl CredentialManager {
    fn entry(provider: KeyProvider) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, provider.account())
            .map_err(|error| format!("could not open the Windows credential store: {error}"))
    }
}

#[cfg(windows)]
impl KeyStore for CredentialManager {
    fn get(&self, provider: KeyProvider) -> Result<Option<String>, String> {
        match Self::entry(provider)?.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(format!("could not read the saved key: {error}")),
        }
    }

    fn set(&self, provider: KeyProvider, key: &str) -> Result<(), String> {
        Self::entry(provider)?
            .set_password(key)
            .map_err(|error| format!("could not save the key: {error}"))
    }

    fn delete(&self, provider: KeyProvider) -> Result<(), String> {
        match Self::entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!("could not remove the key: {error}")),
        }
    }
}

/// Other platforms keep nothing yet: saving is refused rather than writing
/// a key somewhere less safe.
#[cfg(not(windows))]
struct Unsupported;

#[cfg(not(windows))]
impl KeyStore for Unsupported {
    fn get(&self, _: KeyProvider) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn set(&self, _: KeyProvider, _: &str) -> Result<(), String> {
        Err("secure key storage is only implemented for Windows so far".into())
    }

    fn delete(&self, _: KeyProvider) -> Result<(), String> {
        Ok(())
    }
}

pub fn default_store() -> Arc<dyn KeyStore> {
    #[cfg(windows)]
    return Arc::new(CredentialManager);
    #[cfg(not(windows))]
    return Arc::new(Unsupported);
}

#[cfg(test)]
pub use fake::MemoryStore;

#[cfg(test)]
mod fake {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::{KeyProvider, KeyStore};

    #[derive(Default)]
    pub struct MemoryStore(Mutex<HashMap<KeyProvider, String>>);

    impl MemoryStore {
        pub fn with(provider: KeyProvider, key: &str) -> Self {
            let store = Self::default();
            store.0.lock().unwrap().insert(provider, key.to_string());
            store
        }
    }

    impl KeyStore for MemoryStore {
        fn get(&self, provider: KeyProvider) -> Result<Option<String>, String> {
            Ok(self.0.lock().unwrap().get(&provider).cloned())
        }

        fn set(&self, provider: KeyProvider, key: &str) -> Result<(), String> {
            self.0.lock().unwrap().insert(provider, key.to_string());
            Ok(())
        }

        fn delete(&self, provider: KeyProvider) -> Result<(), String> {
            self.0.lock().unwrap().remove(&provider);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "sk-test-1234567890abcdef";

    #[test]
    fn plausible_keys_pass_and_are_trimmed() {
        assert_eq!(validate_key(&format!("  {KEY}\n")), Ok(KEY));
    }

    #[test]
    fn bad_keys_are_refused_without_echoing_them() {
        for bad in [
            "",
            "   ",
            "sk-has space-1234567890",
            "short",
            "sk-ключ-1234567890abc",
        ] {
            let error = validate_key(bad).unwrap_err();
            assert!(bad.is_empty() || !error.contains(bad), "{error}");
        }
        assert!(validate_key(&"a".repeat(MAX_KEY_LEN + 1)).is_err());
    }

    #[test]
    fn status_reports_only_presence_and_last_four() {
        let store = MemoryStore::with(KeyProvider::Openai, KEY);
        let all = statuses(&store).unwrap();
        assert!(!all.anthropic.configured && all.anthropic.last4.is_none());
        assert_eq!(all.openai.last4.as_deref(), Some("cdef"));
        let json = serde_json::to_string(&all).unwrap();
        assert!(!json.contains(KEY) && !json.contains("sk-test"), "{json}");
    }

    #[test]
    fn save_and_clear_round_trip() {
        let store = MemoryStore::default();
        save(&store, KeyProvider::Anthropic, KEY).unwrap();
        assert!(status(&store, KeyProvider::Anthropic).unwrap().configured);
        assert!(save(&store, KeyProvider::Anthropic, "bad key").is_err());
        store.delete(KeyProvider::Anthropic).unwrap();
        assert!(!status(&store, KeyProvider::Anthropic).unwrap().configured);
    }
}
