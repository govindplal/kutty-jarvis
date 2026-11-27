//! Client for OpenAI-compatible `/v1/chat/completions` servers.
//!
//! `llama-server` speaks this protocol, and so do most cloud providers, so
//! the same client serves both. Requests are plain JSON built by pure
//! functions so they can be unit-tested without a server.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use super::{Assistant, Question, SpeechToText};
use crate::assistant::prompt;

const ASR_PROMPT: &str = "Perform ASR in English.";
const ASR_MAX_TOKENS: u32 = 256;
const ANSWER_MAX_TOKENS: u32 = 200;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Where and how to reach one chat-completions endpoint.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub base_url: String,
    pub api_key: Option<String>,
    /// Sent as `model`; llama-server ignores it, cloud servers need it.
    pub model: Option<String>,
    pub timeout: Duration,
}

impl Endpoint {
    pub fn local(base_url: String, timeout: Duration) -> Self {
        Self {
            base_url,
            api_key: None,
            model: None,
            timeout,
        }
    }

    fn completions_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        if base.ends_with("/v1") {
            format!("{base}/chat/completions")
        } else {
            format!("{base}/v1/chat/completions")
        }
    }
}

fn base_body(endpoint: &Endpoint, temperature: f64, max_tokens: u32, messages: Value) -> Value {
    let mut body = json!({
        "temperature": temperature,
        "max_tokens": max_tokens,
        "messages": messages,
    });
    if let Some(model) = &endpoint.model {
        body["model"] = json!(model);
    }
    body
}

pub fn transcription_request(endpoint: &Endpoint, wav: &[u8]) -> Value {
    base_body(
        endpoint,
        0.0,
        ASR_MAX_TOKENS,
        json!([
            { "role": "system", "content": ASR_PROMPT },
            { "role": "user", "content": [{
                "type": "input_audio",
                "input_audio": { "data": STANDARD.encode(wav), "format": "wav" },
            }] },
        ]),
    )
}

pub fn answer_request(endpoint: &Endpoint, question: &Question) -> Value {
    let mut content = Vec::new();
    if let Some(png) = &question.screenshot_png {
        content.push(json!({
            "type": "image_url",
            "image_url": { "url": format!("data:image/png;base64,{}", STANDARD.encode(png)) },
        }));
    }
    content.push(json!({ "type": "text", "text": prompt::user_message(question) }));
    base_body(
        endpoint,
        0.3,
        ANSWER_MAX_TOKENS,
        json!([
            { "role": "system", "content": prompt::SYSTEM },
            { "role": "user", "content": content },
        ]),
    )
}

/// Pulls the reply text out of a chat-completions response body.
pub fn parse_response(body: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| "the model server sent an unreadable reply".to_string())?;
    if let Some(message) = value.pointer("/error/message").and_then(Value::as_str) {
        return Err(format!("the model server reported an error: {message}"));
    }
    match value.pointer("/choices/0/message/content") {
        Some(Value::String(text)) => Ok(text.trim().to_string()),
        // Some servers return content as typed parts.
        Some(Value::Array(parts)) => Ok(parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string()),
        _ => Err("the model server's reply had no text in it".to_string()),
    }
}

fn post(endpoint: &Endpoint, body: &Value) -> Result<String, String> {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(endpoint.timeout))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let url = endpoint.completions_url();
    let mut request = agent.post(&url).header("content-type", "application/json");
    if let Some(key) = &endpoint.api_key {
        request = request.header("authorization", format!("Bearer {key}"));
    }
    let mut response = request
        .send(body.to_string())
        .map_err(|error| describe_transport_error(&endpoint.base_url, &error))?;
    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|error| format!("could not read the model server's reply: {error}"))?;
    if status >= 400 {
        let detail = parse_response(&text).err().unwrap_or(text);
        return Err(format!("the model server answered HTTP {status}: {detail}"));
    }
    parse_response(&text)
}

fn describe_transport_error(base_url: &str, error: &ureq::Error) -> String {
    match error {
        ureq::Error::Timeout(_) => format!(
            "the model server at {base_url} took too long to answer; a slow computer may need a smaller model"
        ),
        ureq::Error::Io(_) | ureq::Error::ConnectionFailed => {
            format!("could not reach the model server at {base_url}; is it running?")
        }
        other => format!("could not talk to the model server at {base_url}: {other}"),
    }
}

pub struct OpenAiCompatAsr(pub Endpoint);

impl SpeechToText for OpenAiCompatAsr {
    fn transcribe(&self, wav: &[u8]) -> Result<String, String> {
        post(&self.0, &transcription_request(&self.0, wav))
    }
}

pub struct OpenAiCompatAssistant(pub Endpoint);

impl Assistant for OpenAiCompatAssistant {
    fn answer(&self, question: &Question) -> Result<String, String> {
        post(&self.0, &answer_request(&self.0, question))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint() -> Endpoint {
        Endpoint::local("http://127.0.0.1:9000".into(), Duration::from_secs(1))
    }

    #[test]
    fn url_gets_the_version_prefix_once() {
        assert_eq!(
            endpoint().completions_url(),
            "http://127.0.0.1:9000/v1/chat/completions"
        );
        let cloud = Endpoint::local("https://api.example.com/v1/".into(), Duration::from_secs(1));
        assert_eq!(
            cloud.completions_url(),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn transcription_sends_the_wav_as_input_audio() {
        let body = transcription_request(&endpoint(), &[1, 2, 3]);
        assert_eq!(body["temperature"], 0.0);
        assert_eq!(body["messages"][0]["content"], ASR_PROMPT);
        let audio = &body["messages"][1]["content"][0];
        assert_eq!(audio["type"], "input_audio");
        assert_eq!(audio["input_audio"]["format"], "wav");
        assert_eq!(audio["input_audio"]["data"], "AQID");
        assert!(body.get("model").is_none());
    }

    #[test]
    fn answer_puts_the_image_before_the_text() {
        let question = Question {
            transcript: "where is save".into(),
            screenshot_png: Some(vec![0, 1, 2]),
            elements: "1 button \"Save\"".into(),
        };
        let body = answer_request(&endpoint(), &question);
        assert_eq!(body["messages"][0]["role"], "system");
        let parts = body["messages"][1]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], "image_url");
        assert_eq!(parts[0]["image_url"]["url"], "data:image/png;base64,AAEC");
        assert_eq!(parts[1]["type"], "text");
        let text = parts[1]["text"].as_str().unwrap();
        assert!(text.contains("where is save"));
        assert!(text.contains("1 button \"Save\""));
    }

    #[test]
    fn answer_without_a_screenshot_is_text_only() {
        let question = Question {
            transcript: "hi".into(),
            screenshot_png: None,
            elements: String::new(),
        };
        let body = answer_request(&endpoint(), &question);
        let parts = body["messages"][1]["content"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["type"], "text");
    }

    #[test]
    fn model_name_is_sent_when_configured() {
        let mut cloud = endpoint();
        cloud.model = Some("some-model".into());
        assert_eq!(transcription_request(&cloud, &[])["model"], "some-model");
    }

    #[test]
    fn parses_plain_and_typed_content() {
        let plain = r#"{"choices":[{"message":{"content":"  hello \n"}}]}"#;
        assert_eq!(parse_response(plain).unwrap(), "hello");
        let parts = r#"{"choices":[{"message":{"content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]}}]}"#;
        assert_eq!(parse_response(parts).unwrap(), "ab");
    }

    #[test]
    fn reports_server_errors_and_garbage() {
        let error = r#"{"error":{"message":"context too long"}}"#;
        assert!(
            parse_response(error)
                .unwrap_err()
                .contains("context too long")
        );
        assert!(parse_response("<html>").unwrap_err().contains("unreadable"));
        assert!(
            parse_response(r#"{"choices":[]}"#)
                .unwrap_err()
                .contains("no text")
        );
    }

    #[test]
    fn nothing_listening_gives_an_actionable_message() {
        let dead = Endpoint::local("http://127.0.0.1:1".into(), Duration::from_secs(2));
        let error = OpenAiCompatAsr(dead).transcribe(&[0; 4]).unwrap_err();
        assert!(error.contains("127.0.0.1:1"), "{error}");
    }
}
