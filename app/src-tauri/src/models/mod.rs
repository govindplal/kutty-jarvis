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

/// Fails unless `url` points at this machine (127.0.0.0/8, ::1 or
/// `localhost`). Local providers must never send audio or screenshots
/// off-machine, even if a URL override says otherwise.
pub fn require_loopback(url: &str) -> Result<(), String> {
    let refuse = || {
        format!(
            "{url} is not a local address; local model servers must be on localhost, 127.0.0.1 or [::1]"
        )
    };
    let rest = url
        .trim()
        .strip_prefix("http://")
        .or_else(|| url.trim().strip_prefix("https://"))
        .ok_or_else(|| format!("{url} must start with http:// or https://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return Err(refuse());
    }
    let host = match authority.strip_prefix('[') {
        Some(bracketed) => bracketed.split(']').next().unwrap_or_default(),
        None => authority.split(':').next().unwrap_or_default(),
    };
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if loopback { Ok(()) } else { Err(refuse()) }
}

/// An HTTP agent for local servers: no proxy (environment proxies would
/// carry the traffic elsewhere) and no redirects (a redirect could leave
/// the machine).
pub fn local_agent(
    connect: Option<std::time::Duration>,
    global: std::time::Duration,
) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .timeout_connect(connect)
        .timeout_global(Some(global))
        .http_status_as_error(false)
        .build();
    ureq::Agent::new_with_config(config)
}

#[cfg(test)]
mod tests {
    use super::require_loopback;

    #[test]
    fn loopback_addresses_are_accepted() {
        for url in [
            "http://127.0.0.1:8080",
            "http://127.9.9.9",
            "http://localhost:1234/v1",
            "http://LOCALHOST",
            "http://[::1]:8080/v1",
        ] {
            assert!(require_loopback(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn everything_else_is_refused() {
        for url in [
            "http://example.com",
            "http://192.168.1.5:8080",
            "http://0.0.0.0:8080",
            "http://127.0.0.1.evil.com",
            "http://localhost.evil.com",
            "http://127.0.0.1@evil.com",
            "http://evil.com/127.0.0.1",
            "http://[::2]",
            "ftp://127.0.0.1",
            "127.0.0.1:8080",
            "",
        ] {
            assert!(require_loopback(url).is_err(), "{url}");
        }
    }
}
