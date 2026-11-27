//! Commands for the Models settings. Keys come in through `set_api_key`
//! and never go back out: only presence and the last four characters do.

use tauri::State;

use crate::assistant::runtime::Providers;
use crate::secrets::{self, KeyProvider, KeyStatus, KeyStatuses};
use crate::settings::{self, ModelSettings, SettingsPatch};

#[tauri::command]
pub fn get_model_settings(providers: State<Providers>) -> ModelSettings {
    settings::load(&providers.config_dir)
}

#[tauri::command]
pub fn set_model_settings(
    providers: State<Providers>,
    settings: ModelSettings,
) -> Result<ModelSettings, String> {
    settings::save(&providers.config_dir, settings)
}

/// Changes only the named choices, applied to the file as it is now.
#[tauri::command]
pub fn patch_model_settings(
    providers: State<Providers>,
    patch: SettingsPatch,
) -> Result<ModelSettings, String> {
    settings::patch(&providers.config_dir, patch)
}

/// Returns only the saved provider's status, so an unrelated status read
/// cannot make a successful write look like a failure.
#[tauri::command]
pub fn set_api_key(
    providers: State<Providers>,
    provider: KeyProvider,
    key: String,
) -> Result<KeyStatus, String> {
    save_key(providers.store.as_ref(), provider, &key)
}

#[tauri::command]
pub fn clear_api_key(
    providers: State<Providers>,
    provider: KeyProvider,
) -> Result<KeyStatus, String> {
    clear_key(providers.store.as_ref(), provider)
}

fn save_key(
    store: &dyn secrets::KeyStore,
    provider: KeyProvider,
    key: &str,
) -> Result<KeyStatus, String> {
    secrets::save(store, provider, key)?;
    secrets::status(store, provider)
}

fn clear_key(store: &dyn secrets::KeyStore, provider: KeyProvider) -> Result<KeyStatus, String> {
    store.delete(provider)?;
    secrets::status(store, provider)
}

#[tauri::command]
pub fn api_key_status(providers: State<Providers>) -> Result<KeyStatuses, String> {
    secrets::statuses(providers.store.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::KeyStore;
    use crate::secrets::fake::MemoryStore;

    /// Writes fine but cannot read the OpenAI entry back.
    struct FlakyStore(MemoryStore);

    impl KeyStore for FlakyStore {
        fn get(&self, provider: KeyProvider) -> Result<Option<String>, String> {
            match provider {
                KeyProvider::Openai => Err("credential store unavailable".into()),
                _ => self.0.get(provider),
            }
        }
        fn set(&self, provider: KeyProvider, key: &str) -> Result<(), String> {
            self.0.set(provider, key)
        }
        fn delete(&self, provider: KeyProvider) -> Result<(), String> {
            self.0.delete(provider)
        }
    }

    #[test]
    fn saving_returns_only_that_providers_status() {
        let store = FlakyStore(MemoryStore::default());
        let status = save_key(&store, KeyProvider::Anthropic, "sk-ant-abcdefgh1234").unwrap();
        assert!(status.configured);
        assert_eq!(status.last4.as_deref(), Some("1234"));
        assert!(secrets::statuses(&store).is_err());
        let cleared = clear_key(&store, KeyProvider::Anthropic).unwrap();
        assert!(!cleared.configured);
    }
}
