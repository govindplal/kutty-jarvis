//! Picks the provider for each step of one question from the saved settings
//! and key presence (ADR 0015).
//!
//! A selected cloud provider without a key is an error, never a quiet switch
//! to another provider: the user asked for that provider and must know where
//! their audio and screen are going.

use std::sync::Arc;

use serde::Serialize;

use super::{Assistant, SpeechToText, cloud};
use crate::secrets::{KeyProvider, KeyStore};
use crate::settings::{AnswerProvider, ModelSettings, SttProvider};

/// Provider names reported to the frontend with each reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Used {
    pub speech_to_text: &'static str,
    pub assistant: &'static str,
}

pub struct Selection {
    pub speech_to_text: Result<Arc<dyn SpeechToText>, String>,
    pub assistant: Result<Arc<dyn Assistant>, String>,
    pub used: Used,
}

fn key_for(store: &dyn KeyStore, provider: KeyProvider, step: &str) -> Result<String, String> {
    match store.get(provider)? {
        Some(key) => Ok(key),
        None => Err(format!(
            "{} is selected for {step} but no API key is saved; add one in the Models settings or switch back to local",
            provider.label()
        )),
    }
}

pub fn choose(
    settings: &ModelSettings,
    store: &dyn KeyStore,
    local_stt: &Arc<dyn SpeechToText>,
    local_assistant: &Arc<dyn Assistant>,
) -> Selection {
    let models = &settings.models;
    let (speech_to_text, stt_name) = match settings.speech_to_text {
        SttProvider::Local => (Ok(Arc::clone(local_stt)), "local"),
        SttProvider::Openai => (
            key_for(store, KeyProvider::Openai, "speech-to-text").map(|key| {
                Arc::new(cloud::openai_transcriber(
                    key,
                    models.openai_transcribe.clone(),
                )) as Arc<dyn SpeechToText>
            }),
            "openai",
        ),
    };
    let (assistant, assistant_name) = match settings.assistant {
        AnswerProvider::Local => (Ok(Arc::clone(local_assistant)), "local"),
        AnswerProvider::Anthropic => (
            key_for(store, KeyProvider::Anthropic, "answers").map(|key| {
                Arc::new(cloud::anthropic(key, models.anthropic.clone())) as Arc<dyn Assistant>
            }),
            "anthropic",
        ),
        AnswerProvider::Openai => (
            key_for(store, KeyProvider::Openai, "answers").map(|key| {
                Arc::new(cloud::openai_assistant(key, models.openai_chat.clone()))
                    as Arc<dyn Assistant>
            }),
            "openai",
        ),
    };
    Selection {
        speech_to_text,
        assistant,
        used: Used {
            speech_to_text: stt_name,
            assistant: assistant_name,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Question;
    use crate::secrets::MemoryStore;

    struct Local;

    impl SpeechToText for Local {
        fn transcribe(&self, _: &[u8]) -> Result<String, String> {
            Ok("local".into())
        }
    }

    impl Assistant for Local {
        fn answer(&self, _: &Question) -> Result<String, String> {
            Ok("local".into())
        }
    }

    fn pick(settings: &ModelSettings, store: &MemoryStore) -> Selection {
        let stt: Arc<dyn SpeechToText> = Arc::new(Local);
        let assistant: Arc<dyn Assistant> = Arc::new(Local);
        choose(settings, store, &stt, &assistant)
    }

    #[test]
    fn defaults_use_the_local_models_even_with_keys_saved() {
        let store = MemoryStore::with(KeyProvider::Openai, "sk-test-1234567890abcdef");
        let chosen = pick(&ModelSettings::default(), &store);
        assert_eq!(
            chosen.speech_to_text.unwrap().transcribe(b"").unwrap(),
            "local"
        );
        assert_eq!(chosen.used.assistant, "local");
    }

    #[test]
    fn a_selected_cloud_provider_without_a_key_is_an_error() {
        let settings = ModelSettings {
            speech_to_text: SttProvider::Openai,
            assistant: AnswerProvider::Anthropic,
            ..Default::default()
        };
        let chosen = pick(&settings, &MemoryStore::default());
        let stt = chosen.speech_to_text.err().unwrap();
        let assistant = chosen.assistant.err().unwrap();
        assert!(
            stt.contains("OpenAI") && stt.contains("no API key"),
            "{stt}"
        );
        assert!(assistant.contains("Anthropic") && assistant.contains("switch back to local"));
    }

    #[test]
    fn a_saved_key_selects_the_cloud_provider() {
        let store = MemoryStore::with(KeyProvider::Anthropic, "sk-ant-1234567890abcdef");
        let settings = ModelSettings {
            assistant: AnswerProvider::Anthropic,
            ..Default::default()
        };
        let chosen = pick(&settings, &store);
        assert!(chosen.assistant.is_ok());
        assert!(chosen.speech_to_text.is_ok());
        assert_eq!(
            chosen.used,
            Used {
                speech_to_text: "local",
                assistant: "anthropic"
            }
        );
    }
}
