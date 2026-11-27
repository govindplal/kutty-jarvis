//! Commands for the Models settings. Keys come in through `set_api_key`
//! and never go back out: only presence and the last four characters do.

use tauri::State;

use crate::assistant::runtime::Providers;
use crate::secrets::{self, KeyProvider, KeyStatuses};
use crate::settings::{self, ModelSettings};

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

#[tauri::command]
pub fn set_api_key(
    providers: State<Providers>,
    provider: KeyProvider,
    key: String,
) -> Result<KeyStatuses, String> {
    secrets::save(providers.store.as_ref(), provider, &key)?;
    secrets::statuses(providers.store.as_ref())
}

#[tauri::command]
pub fn clear_api_key(
    providers: State<Providers>,
    provider: KeyProvider,
) -> Result<KeyStatuses, String> {
    providers.store.delete(provider)?;
    secrets::statuses(providers.store.as_ref())
}

#[tauri::command]
pub fn api_key_status(providers: State<Providers>) -> Result<KeyStatuses, String> {
    secrets::statuses(providers.store.as_ref())
}
