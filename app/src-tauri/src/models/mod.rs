//! Model providers behind small interfaces (ADR 0004).
//!
//! The assistant only knows [`SpeechToText`] and [`Assistant`]. Local
//! `llama-server` sidecars implement them today (ADR 0012); a cloud provider
//! with the user's own key would be another implementation.

pub mod llama_server;
pub mod openai_compat;

/// Turns a recorded WAV (16-bit PCM) into text.
pub trait SpeechToText: Send + Sync {
    fn transcribe(&self, wav: &[u8]) -> Result<String, String>;
}

/// Answers a spoken question about the screen.
pub trait Assistant: Send + Sync {
    /// Returns the model's raw reply, including any pointing tag.
    fn answer(&self, question: &Question) -> Result<String, String>;
}

/// Everything the model sees for one turn. Held in memory only.
#[derive(Debug, Clone)]
pub struct Question {
    pub transcript: String,
    /// Screenshot taken when the hotkey went down, if it succeeded.
    pub screenshot_png: Option<Vec<u8>>,
    /// Numbered UI elements of the foreground window, as prompt text.
    /// Empty when the list is unavailable.
    pub elements: String,
}
