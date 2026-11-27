//! Cloud providers that use the user's own API key (ADR 0015).
//!
//! Anthropic Messages for answers, OpenAI chat completions for answers and
//! OpenAI audio transcriptions for speech. Requests are built by pure
//! functions so they are tested without a network; the HTTP client only
//! talks to the provider's HTTPS host and never follows a redirect.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use super::{Assistant, Question, SpeechToText, shrink_for_model};
use crate::assistant::prompt;
use crate::secrets::KeyProvider;

const ANTHROPIC_URL: &str = "https://api.anthropic.com";
const OPENAI_URL: &str = "https://api.openai.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Reasoning tokens count against the limit on both providers, so leave
/// room beyond the short spoken answer.
const ANSWER_MAX_TOKENS: u32 = 1024;
/// gpt-6-luna defaults to `medium`, which is slow for spoken replies.
const OPENAI_REASONING_EFFORT: &str = "low";
/// Haiku 5.5 uses adaptive thinking at `medium` by default.
const ANTHROPIC_EFFORT: &str = "low";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const ANSWER_TIMEOUT: Duration = Duration::from_secs(60);
const TRANSCRIBE_TIMEOUT: Duration = Duration::from_secs(60);

/// One provider's HTTPS endpoint plus the user's key. The key is never part
/// of `Debug` output or of an error message.
#[derive(Clone)]
pub struct CloudClient {
    provider: KeyProvider,
    key: String,
    base_url: String,
    timeout: Duration,
    allow_loopback_http: bool,
}

impl std::fmt::Debug for CloudClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudClient")
            .field("provider", &self.provider)
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl CloudClient {
    pub fn new(provider: KeyProvider, key: String, timeout: Duration) -> Self {
        let base_url = match provider {
            KeyProvider::Anthropic => ANTHROPIC_URL,
            KeyProvider::Openai => OPENAI_URL,
        };
        Self {
            provider,
            key,
            base_url: base_url.to_string(),
            timeout,
            allow_loopback_http: false,
        }
    }

    /// Points the client at a local mock server. Tests only.
    #[cfg(test)]
    fn for_test(provider: KeyProvider, key: &str, base_url: String) -> Self {
        Self {
            base_url,
            allow_loopback_http: true,
            ..Self::new(provider, key.to_string(), Duration::from_secs(5))
        }
    }

    fn check_url(&self, url: &str) -> Result<(), String> {
        if url.starts_with("https://") {
            return Ok(());
        }
        if self.allow_loopback_http {
            return super::require_loopback(url);
        }
        Err(format!("{} requests must use https", self.provider.label()))
    }

    fn post(&self, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<String, String> {
        let url = format!("{}{path}", self.base_url.trim_end_matches('/'));
        self.check_url(&url)?;
        // Normal proxy settings apply (the user chose to use the cloud), but
        // a redirect could carry the key to another host, so none is followed.
        let config = ureq::Agent::config_builder()
            .max_redirects(0)
            .https_only(!self.allow_loopback_http)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(self.timeout))
            .http_status_as_error(false)
            .build();
        let agent = ureq::Agent::new_with_config(config);
        let mut request = agent.post(&url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let label = self.provider.label();
        let mut response = request
            .send(body)
            .map_err(|error| self.redact(&describe_transport_error(label, &error)))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|_| format!("could not read {label}'s reply"))?;
        if status >= 300 {
            return Err(self.redact(&describe_status(self.provider, status, &text)));
        }
        Ok(text)
    }

    fn redact(&self, text: &str) -> String {
        if self.key.is_empty() {
            return text.to_string();
        }
        text.replace(&self.key, "[key]")
    }
}

fn describe_transport_error(label: &str, error: &ureq::Error) -> String {
    match error {
        ureq::Error::Timeout(_) => format!("{label} took too long to answer; try again"),
        ureq::Error::Io(_) | ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => {
            format!("could not reach {label}; check your internet connection")
        }
        _ => format!("could not talk to {label}"),
    }
}

/// Turns an HTTP error into something the user can act on. 401 text from
/// providers can quote part of the key, so it is never passed on.
fn describe_status(provider: KeyProvider, status: u16, body: &str) -> String {
    let label = provider.label();
    match status {
        401 | 403 => format!(
            "{label} did not accept your API key (HTTP {status}); check or replace it in the Models settings"
        ),
        429 => format!(
            "{label} says you are over a rate limit or out of credit (HTTP 429); wait a bit or check your plan"
        ),
        500..=599 => format!("{label} is having trouble right now (HTTP {status}); try again soon"),
        _ => match error_code(body) {
            Some(code) => format!("{label} refused the request (HTTP {status}, {code})"),
            None => format!("{label} refused the request (HTTP {status})"),
        },
    }
}

/// A short machine-style error type or code from an error body, if it looks
/// like one. Provider free text is never passed on: it could echo the key.
fn error_code(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    ["/error/code", "/error/type"]
        .iter()
        .filter_map(|path| value.pointer(path).and_then(Value::as_str))
        .find(|code| {
            !code.is_empty()
                && code.len() <= 40
                && code
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
        .map(str::to_string)
}

fn png_data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(png))
}

pub fn anthropic_request(model: &str, question: &Question) -> Value {
    let mut content = Vec::new();
    if let Some(png) = &question.screenshot_png {
        content.push(json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/png",
                "data": STANDARD.encode(png),
            },
        }));
    }
    content.push(json!({ "type": "text", "text": prompt::user_message(question) }));
    json!({
        "model": model,
        "max_tokens": ANSWER_MAX_TOKENS,
        "output_config": { "effort": ANTHROPIC_EFFORT },
        "system": prompt::SYSTEM,
        "messages": [{ "role": "user", "content": content }],
    })
}

/// Joins the text blocks of a Messages API reply.
pub fn parse_anthropic(body: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| "Anthropic sent an unreadable reply".to_string())?;
    let text = value
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    let text = text.trim();
    if text.is_empty() {
        if value.get("stop_reason").and_then(Value::as_str) == Some("max_tokens") {
            return Err("Anthropic ran out of room before it wrote an answer; try again".into());
        }
        return Err("Anthropic's reply had no text in it".into());
    }
    Ok(text.to_string())
}

/// Reads a chat-completions reply. Error bodies are never quoted, since a
/// 2xx envelope could still echo the key.
pub fn parse_openai_chat(body: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| "OpenAI sent an unreadable reply".to_string())?;
    if value.get("error").is_some_and(|error| !error.is_null()) {
        return Err(match error_code(body) {
            Some(code) => format!("OpenAI reported an error ({code})"),
            None => "OpenAI reported an error".to_string(),
        });
    }
    let choice = value.pointer("/choices/0");
    let text = match choice.and_then(|c| c.pointer("/message/content")) {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string(),
        _ => String::new(),
    };
    if !text.is_empty() {
        return Ok(text);
    }
    match choice
        .and_then(|c| c.get("finish_reason"))
        .and_then(Value::as_str)
    {
        Some("length") => Err("OpenAI ran out of room before it wrote an answer; try again".into()),
        Some("content_filter") => Err("OpenAI declined to answer that".into()),
        _ => Err("OpenAI's reply had no text in it".into()),
    }
}

pub fn openai_chat_request(model: &str, question: &Question) -> Value {
    let mut content = Vec::new();
    if let Some(png) = &question.screenshot_png {
        content.push(json!({
            "type": "image_url",
            "image_url": { "url": png_data_url(png) },
        }));
    }
    content.push(json!({ "type": "text", "text": prompt::user_message(question) }));
    json!({
        "model": model,
        "max_completion_tokens": ANSWER_MAX_TOKENS,
        "reasoning_effort": OPENAI_REASONING_EFFORT,
        "messages": [
            { "role": "system", "content": prompt::SYSTEM },
            { "role": "user", "content": content },
        ],
    })
}

/// `multipart/form-data` body for `/v1/audio/transcriptions`.
pub fn transcription_form(boundary: &str, model: &str, wav: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(wav.len() + 512);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{model}\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"response_format\"\r\n\r\njson\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"question.wav\"\r\n\
             Content-Type: audio/wav\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(wav);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

fn parse_transcription(body: &str) -> Result<String, String> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .ok_or_else(|| "OpenAI's transcription reply had no text in it".to_string())
}

pub struct AnthropicAssistant {
    pub client: CloudClient,
    pub model: String,
}

impl Assistant for AnthropicAssistant {
    fn answer(&self, question: &Question) -> Result<String, String> {
        let body = anthropic_request(&self.model, &shrink_for_model(question));
        let reply = self.client.post(
            "/v1/messages",
            &[
                ("x-api-key", &self.client.key),
                ("anthropic-version", ANTHROPIC_VERSION),
                ("content-type", "application/json"),
            ],
            body.to_string().as_bytes(),
        )?;
        parse_anthropic(&reply)
    }
}

pub struct OpenAiAssistant {
    pub client: CloudClient,
    pub model: String,
}

impl Assistant for OpenAiAssistant {
    fn answer(&self, question: &Question) -> Result<String, String> {
        let body = openai_chat_request(&self.model, &shrink_for_model(question));
        let reply = self.client.post(
            "/v1/chat/completions",
            &[
                ("authorization", &format!("Bearer {}", self.client.key)),
                ("content-type", "application/json"),
            ],
            body.to_string().as_bytes(),
        )?;
        parse_openai_chat(&reply).map_err(|error| self.client.redact(&error))
    }
}

pub struct OpenAiTranscriber {
    pub client: CloudClient,
    pub model: String,
}

impl SpeechToText for OpenAiTranscriber {
    fn transcribe(&self, wav: &[u8]) -> Result<String, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let boundary = format!("kuttyjarvis{nanos:x}");
        let form = transcription_form(&boundary, &self.model, wav);
        let reply = self.client.post(
            "/v1/audio/transcriptions",
            &[
                ("authorization", &format!("Bearer {}", self.client.key)),
                (
                    "content-type",
                    &format!("multipart/form-data; boundary={boundary}"),
                ),
            ],
            &form,
        )?;
        parse_transcription(&reply)
    }
}

pub fn anthropic(key: String, model: String) -> AnthropicAssistant {
    AnthropicAssistant {
        client: CloudClient::new(KeyProvider::Anthropic, key, ANSWER_TIMEOUT),
        model,
    }
}

pub fn openai_assistant(key: String, model: String) -> OpenAiAssistant {
    OpenAiAssistant {
        client: CloudClient::new(KeyProvider::Openai, key, ANSWER_TIMEOUT),
        model,
    }
}

pub fn openai_transcriber(key: String, model: String) -> OpenAiTranscriber {
    OpenAiTranscriber {
        client: CloudClient::new(KeyProvider::Openai, key, TRANSCRIBE_TIMEOUT),
        model,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    use super::*;

    const KEY: &str = "sk-test-1234567890abcdef";

    fn question() -> Question {
        Question {
            transcript: "where is save".into(),
            screenshot_png: Some(vec![1, 2, 3]),
            elements: "1. button \"Save\"".into(),
        }
    }

    /// Serves one canned response and returns what the client sent.
    fn mock(status: u16, body: &str, location: Option<&str>) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let reply = format!(
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n{}connection: close\r\n\r\n{body}",
            body.len(),
            location
                .map(|l| format!("location: {l}\r\n"))
                .unwrap_or_default(),
        );
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut data = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = stream.read(&mut buf).unwrap();
                data.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&data).to_string();
                if let Some(split) = text.find("\r\n\r\n") {
                    let length = text[..split]
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if data.len() >= split + 4 + length || n == 0 {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            stream.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&data).to_string()
        });
        (url, handle)
    }

    #[test]
    fn anthropic_request_has_image_then_text() {
        let body = anthropic_request("claude-x", &question());
        assert_eq!(body["model"], "claude-x");
        assert_eq!(body["system"], prompt::SYSTEM);
        let content = &body["messages"][0]["content"];
        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["source"]["media_type"], "image/png");
        assert_eq!(content[0]["source"]["data"], "AQID");
        assert_eq!(content[1]["type"], "text");
        assert!(
            content[1]["text"]
                .as_str()
                .unwrap()
                .contains("where is save")
        );
    }

    #[test]
    fn requests_without_a_screenshot_have_only_text() {
        let mut q = question();
        q.screenshot_png = None;
        assert_eq!(
            anthropic_request("m", &q)["messages"][0]["content"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            openai_chat_request("m", &q)["messages"][1]["content"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn openai_request_uses_a_data_url_and_the_system_prompt() {
        let body = openai_chat_request("gpt-x", &question());
        assert_eq!(body["model"], "gpt-x");
        assert_eq!(body["messages"][0]["content"], prompt::SYSTEM);
        assert_eq!(
            body["messages"][1]["content"][0]["image_url"]["url"],
            "data:image/png;base64,AQID"
        );
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn transcription_form_is_well_formed_multipart() {
        let form = transcription_form("B", "gpt-transcribe", b"RIFFdata");
        let text = String::from_utf8(form).unwrap();
        assert!(text.starts_with("--B\r\n"));
        assert!(text.contains("name=\"model\"\r\n\r\ngpt-transcribe\r\n"));
        assert!(
            text.contains(
                "filename=\"question.wav\"\r\nContent-Type: audio/wav\r\n\r\nRIFFdata\r\n"
            )
        );
        assert!(text.ends_with("\r\n--B--\r\n"));
    }

    #[test]
    fn anthropic_replies_join_text_blocks() {
        let body = r#"{"content":[{"type":"text","text":"Click "},{"type":"tool_use"},{"type":"text","text":"Save. [POINT:1]"}]}"#;
        assert_eq!(parse_anthropic(body).unwrap(), "Click Save. [POINT:1]");
        assert!(parse_anthropic(r#"{"content":[]}"#).is_err());
        assert!(parse_anthropic("nope").is_err());
    }

    #[test]
    fn status_messages_are_actionable_and_never_quote_the_key() {
        let body = format!(r#"{{"error":{{"message":"Incorrect API key provided: {KEY}"}}}}"#);
        for status in [401, 403, 429, 500, 529] {
            let text = describe_status(KeyProvider::Openai, status, &body);
            assert!(!text.contains(KEY) && !text.contains("sk-test"), "{text}");
            assert!(text.contains("OpenAI"), "{text}");
        }
        assert!(describe_status(KeyProvider::Anthropic, 401, "").contains("Models settings"));
        assert!(describe_status(KeyProvider::Anthropic, 429, "").contains("rate limit"));
        assert!(describe_status(KeyProvider::Anthropic, 529, "").contains("trouble"));
        let other = describe_status(
            KeyProvider::Anthropic,
            400,
            r#"{"error":{"message":"bad model","type":"invalid_request_error"}}"#,
        );
        assert!(other.contains("HTTP 400") && other.contains("invalid_request_error"));
        assert!(!other.contains("bad model"));
    }

    #[test]
    fn plain_http_to_a_real_host_is_refused() {
        let mut client = CloudClient::new(KeyProvider::Openai, KEY.into(), Duration::from_secs(1));
        client.base_url = "http://api.openai.com".into();
        let error = client.post("/v1/x", &[], b"").unwrap_err();
        assert!(error.contains("https"), "{error}");
        assert!(!format!("{client:?}").contains(KEY));
    }

    #[test]
    fn anthropic_client_path_sends_headers_and_parses_the_reply() {
        let (url, server) = mock(
            200,
            r#"{"content":[{"type":"text","text":"Over there."}]}"#,
            None,
        );
        let assistant = AnthropicAssistant {
            client: CloudClient::for_test(KeyProvider::Anthropic, KEY, url),
            model: "claude-x".into(),
        };
        assert_eq!(assistant.answer(&question()).unwrap(), "Over there.");
        let sent = server.join().unwrap();
        assert!(sent.starts_with("POST /v1/messages HTTP/1.1"), "{sent}");
        let lower = sent.to_ascii_lowercase();
        assert!(lower.contains(&format!("x-api-key: {}", KEY.to_ascii_lowercase())));
        assert!(lower.contains("anthropic-version: 2023-06-01"));
        assert!(sent.contains("\"model\":\"claude-x\""));
    }

    #[test]
    fn openai_client_paths_send_a_bearer_key() {
        let (url, server) = mock(
            200,
            r#"{"choices":[{"message":{"content":"Up top."}}]}"#,
            None,
        );
        let assistant = OpenAiAssistant {
            client: CloudClient::for_test(KeyProvider::Openai, KEY, url),
            model: "gpt-x".into(),
        };
        assert_eq!(assistant.answer(&question()).unwrap(), "Up top.");
        let sent = server.join().unwrap();
        assert!(sent.starts_with("POST /v1/chat/completions "), "{sent}");
        assert!(sent.contains(&format!("Bearer {KEY}")));

        let (url, server) = mock(200, r#"{"text":"where is save"}"#, None);
        let stt = OpenAiTranscriber {
            client: CloudClient::for_test(KeyProvider::Openai, KEY, url),
            model: "gpt-transcribe".into(),
        };
        assert_eq!(stt.transcribe(b"RIFFwav").unwrap(), "where is save");
        let sent = server.join().unwrap();
        assert!(sent.starts_with("POST /v1/audio/transcriptions "), "{sent}");
        assert!(sent.contains("multipart/form-data; boundary="));
        assert!(sent.contains("RIFFwav"));
    }

    #[test]
    fn http_errors_are_mapped_without_leaking_the_key() {
        let body = format!(r#"{{"error":{{"message":"bad key {KEY}"}}}}"#);
        let (url, server) = mock(401, &body, None);
        let assistant = OpenAiAssistant {
            client: CloudClient::for_test(KeyProvider::Openai, KEY, url),
            model: "m".into(),
        };
        let error = assistant.answer(&question()).unwrap_err();
        server.join().unwrap();
        assert!(error.contains("did not accept your API key"), "{error}");
        assert!(!error.contains(KEY), "{error}");
    }

    #[test]
    fn redirects_are_not_followed() {
        let (other, other_server) = mock(200, r#"{"text":"leaked"}"#, None);
        let (url, server) = mock(307, "", Some(&other));
        let stt = OpenAiTranscriber {
            client: CloudClient::for_test(KeyProvider::Openai, KEY, url),
            model: "m".into(),
        };
        assert!(stt.transcribe(b"wav").is_err());
        server.join().unwrap();
        // Nothing connected to the redirect target.
        assert!(!other_server.is_finished());
    }

    #[test]
    fn answer_requests_set_effort_and_roomy_limits() {
        let openai = openai_chat_request("m", &question());
        assert_eq!(openai["reasoning_effort"], "low");
        assert_eq!(openai["max_completion_tokens"], 1024);
        assert!(openai.get("max_tokens").is_none());
        let anthropic = anthropic_request("m", &question());
        assert_eq!(anthropic["output_config"]["effort"], "low");
        assert_eq!(anthropic["max_tokens"], 1024);
    }

    #[test]
    fn length_stop_without_text_is_a_clear_error() {
        let body = r#"{"choices":[{"finish_reason":"length","message":{"content":""}}]}"#;
        assert!(
            parse_openai_chat(body)
                .unwrap_err()
                .contains("ran out of room")
        );
        let body = r#"{"content":[],"stop_reason":"max_tokens"}"#;
        assert!(
            parse_anthropic(body)
                .unwrap_err()
                .contains("ran out of room")
        );
        let ok = r#"{"choices":[{"finish_reason":"length","message":{"content":" Hi "}}]}"#;
        assert_eq!(parse_openai_chat(ok).unwrap(), "Hi");
    }

    #[test]
    fn error_envelopes_never_quote_provider_text() {
        let body = format!(
            r#"{{"error":{{"message":"bad key {KEY}","type":"invalid_request_error","code":"has spaces {KEY}"}}}}"#
        );
        let error = parse_openai_chat(&body).unwrap_err();
        assert_eq!(error, "OpenAI reported an error (invalid_request_error)");
        for status in [400, 404, 422] {
            let error = describe_status(KeyProvider::Openai, status, &body);
            assert!(
                !error.contains(KEY) && !error.contains("bad key"),
                "{error}"
            );
        }
    }

    #[test]
    fn a_2xx_error_envelope_over_http_is_redacted() {
        let body = format!(r#"{{"error":{{"message":"bad key {KEY}"}}}}"#);
        let (url, server) = mock(200, &body, None);
        let assistant = OpenAiAssistant {
            client: CloudClient::for_test(KeyProvider::Openai, KEY, url),
            model: "m".into(),
        };
        let error = assistant.answer(&question()).unwrap_err();
        server.join().unwrap();
        assert!(
            !error.contains(KEY) && !error.contains("bad key"),
            "{error}"
        );
    }

    #[test]
    fn a_400_over_http_does_not_leak_the_key() {
        let body =
            format!(r#"{{"error":{{"message":"echo {KEY}","type":"invalid_request_error"}}}}"#);
        let (url, server) = mock(400, &body, None);
        let assistant = AnthropicAssistant {
            client: CloudClient::for_test(KeyProvider::Anthropic, KEY, url),
            model: "m".into(),
        };
        let error = assistant.answer(&question()).unwrap_err();
        server.join().unwrap();
        assert!(error.contains("HTTP 400"), "{error}");
        assert!(!error.contains(KEY) && !error.contains("echo"), "{error}");
    }
}
