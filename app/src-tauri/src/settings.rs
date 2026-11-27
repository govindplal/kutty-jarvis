//! Non-secret model choices, kept as a small JSON file in the app config
//! folder (ADR 0015). Keys are not stored here.

use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "model-settings.json";
const MAX_MODEL_LEN: usize = 100;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SttProvider {
    #[default]
    Local,
    Openai,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnswerProvider {
    #[default]
    Local,
    Anthropic,
    Openai,
}

/// Model ids sent to the cloud providers. Overridable because providers
/// retire models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CloudModels {
    pub anthropic: String,
    pub openai_chat: String,
    pub openai_transcribe: String,
}

impl Default for CloudModels {
    fn default() -> Self {
        Self {
            anthropic: "claude-haiku-5-5".into(),
            openai_chat: "gpt-6-luna".into(),
            openai_transcribe: "gpt-transcribe".into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelSettings {
    pub speech_to_text: SttProvider,
    pub assistant: AnswerProvider,
    pub models: CloudModels,
}

impl ModelSettings {
    pub fn validate(&self) -> Result<(), String> {
        for (label, id) in [
            ("Anthropic model", &self.models.anthropic),
            ("OpenAI chat model", &self.models.openai_chat),
            ("OpenAI transcription model", &self.models.openai_transcribe),
        ] {
            let id = id.trim();
            if id.is_empty() || id.len() > MAX_MODEL_LEN || id.contains(char::is_whitespace) {
                return Err(format!(
                    "{label} must be a model id with no spaces (up to {MAX_MODEL_LEN} characters)"
                ));
            }
        }
        Ok(())
    }

    fn trimmed(mut self) -> Self {
        self.models.anthropic = self.models.anthropic.trim().to_string();
        self.models.openai_chat = self.models.openai_chat.trim().to_string();
        self.models.openai_transcribe = self.models.openai_transcribe.trim().to_string();
        self
    }
}

/// Reads the settings; a missing or unreadable file means the local defaults.
pub fn load(dir: &Path) -> ModelSettings {
    match std::fs::read_to_string(dir.join(FILE_NAME)) {
        Ok(text) => serde_json::from_str::<ModelSettings>(&text)
            .ok()
            .filter(|settings| settings.validate().is_ok())
            .unwrap_or_else(|| {
                eprintln!("settings: {FILE_NAME} is unreadable; using local defaults");
                ModelSettings::default()
            }),
        Err(_) => ModelSettings::default(),
    }
}

/// Changes to individual choices; absent fields are left as they are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPatch {
    pub speech_to_text: Option<SttProvider>,
    pub assistant: Option<AnswerProvider>,
}

/// Serializes read-modify-write cycles so concurrent saves cannot undo each
/// other.
static WRITE_LOCK: Mutex<()> = Mutex::new(());
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn lock() -> std::sync::MutexGuard<'static, ()> {
    WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Validates and writes the settings atomically (temp file, then rename).
pub fn save(dir: &Path, settings: ModelSettings) -> Result<ModelSettings, String> {
    let _guard = lock();
    write(dir, settings)
}

/// Applies a patch to the settings currently on disk.
pub fn patch(dir: &Path, patch: SettingsPatch) -> Result<ModelSettings, String> {
    let _guard = lock();
    let mut settings = load(dir);
    if let Some(stt) = patch.speech_to_text {
        settings.speech_to_text = stt;
    }
    if let Some(assistant) = patch.assistant {
        settings.assistant = assistant;
    }
    write(dir, settings)
}

fn write(dir: &Path, settings: ModelSettings) -> Result<ModelSettings, String> {
    settings.validate()?;
    let settings = settings.trimmed();
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("could not create the settings folder: {error}"))?;
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("could not write settings: {error}"))?;
    let nonce = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = dir.join(format!("{FILE_NAME}.{}.{nonce}.tmp", std::process::id()));
    let result = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .and_then(|mut file| file.write_all(json.as_bytes()))
        .and_then(|()| std::fs::rename(&temp, dir.join(FILE_NAME)));
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("could not save settings: {error}"));
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("kj-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn defaults_are_local() {
        let settings = ModelSettings::default();
        assert_eq!(settings.speech_to_text, SttProvider::Local);
        assert_eq!(settings.assistant, AnswerProvider::Local);
        assert_eq!(load(&temp_dir("missing")), settings);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = temp_dir("roundtrip");
        let mut settings = ModelSettings {
            speech_to_text: SttProvider::Openai,
            assistant: AnswerProvider::Anthropic,
            ..Default::default()
        };
        settings.models.anthropic = " claude-custom ".into();
        let saved = save(&dir, settings).unwrap();
        assert_eq!(saved.models.anthropic, "claude-custom");
        assert_eq!(load(&dir), saved);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn partial_and_corrupt_files_fall_back() {
        let dir = temp_dir("partial");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE_NAME), r#"{"assistant":"openai"}"#).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.assistant, AnswerProvider::Openai);
        assert_eq!(loaded.speech_to_text, SttProvider::Local);
        std::fs::write(dir.join(FILE_NAME), "{ not json").unwrap();
        assert_eq!(load(&dir), ModelSettings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_model_ids_are_refused() {
        let dir = temp_dir("invalid");
        for bad in ["", "two words", &"x".repeat(101)] {
            let mut settings = ModelSettings::default();
            settings.models.openai_chat = bad.to_string();
            assert!(save(&dir, settings).is_err(), "{bad}");
        }
        assert!(!dir.exists());
    }

    #[test]
    fn patches_change_one_field_of_the_current_file() {
        let dir = temp_dir("patch");
        let both = SettingsPatch {
            speech_to_text: Some(SttProvider::Openai),
            assistant: Some(AnswerProvider::Anthropic),
        };
        patch(&dir, both).unwrap();
        let saved = patch(
            &dir,
            SettingsPatch {
                speech_to_text: Some(SttProvider::Local),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(saved.speech_to_text, SttProvider::Local);
        assert_eq!(saved.assistant, AnswerProvider::Anthropic);
        let saved = patch(
            &dir,
            SettingsPatch {
                assistant: Some(AnswerProvider::Local),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(saved, ModelSettings::default());
        assert_eq!(load(&dir), ModelSettings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn concurrent_writes_all_succeed_and_leave_no_temp_files() {
        let dir = temp_dir("concurrent");
        let cloud = ModelSettings {
            speech_to_text: SttProvider::Openai,
            assistant: AnswerProvider::Openai,
            ..Default::default()
        };
        save(&dir, cloud).unwrap();
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    let change = if i % 2 == 0 {
                        SettingsPatch {
                            speech_to_text: Some(SttProvider::Local),
                            ..Default::default()
                        }
                    } else {
                        SettingsPatch {
                            assistant: Some(AnswerProvider::Local),
                            ..Default::default()
                        }
                    };
                    patch(&dir, change).unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(load(&dir), ModelSettings::default());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
