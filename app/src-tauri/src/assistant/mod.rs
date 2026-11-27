//! The model loop: recorded question in, spoken-style reply and pointing
//! target out (ADR 0012).
//!
//! [`run`] is the whole pipeline and knows nothing about Tauri or about which
//! provider is behind the traits, so it is tested with fakes. [`runtime`]
//! connects it to the hotkey, the overlay and the frontend.

pub mod prompt;
pub mod runtime;

use std::time::{Duration, Instant};

use serde::Serialize;

use crate::models::{Assistant, Question, SpeechToText};
use crate::uia::{ElementList, Rect, render_for_prompt};

/// Which step failed, shown to the user and sent to the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Recording,
    Busy,
    Transcribe,
    Answer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub stage: Stage,
    pub message: String,
}

/// What one question has to work with. Held in memory only.
pub struct Input {
    pub wav: Vec<u8>,
    pub screenshot_png: Option<Vec<u8>>,
    pub elements: Option<ElementList>,
}

/// The element the reply points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub id: usize,
    pub role: String,
    pub name: String,
    /// Physical screen pixels, for `overlay::point_at`.
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub transcript: String,
    pub text: String,
    pub target: Option<Target>,
    pub transcribe_time: Duration,
    pub answer_time: Duration,
}

/// Transcribes the recording, asks the model and resolves the pointing tag.
/// `on_transcript` runs between the two model calls so the UI can show what
/// was heard while the answer is still being written.
pub fn run(
    stt: &dyn SpeechToText,
    assistant: &dyn Assistant,
    input: Input,
    on_transcript: impl FnOnce(&str),
) -> Result<Reply, Failure> {
    let started = Instant::now();
    let transcript = stt.transcribe(&input.wav).map_err(|message| Failure {
        stage: Stage::Transcribe,
        message,
    })?;
    let transcript = transcript.trim().to_string();
    if transcript.is_empty() {
        return Err(Failure {
            stage: Stage::Transcribe,
            message: "I did not catch any words; try again a little closer to the microphone"
                .into(),
        });
    }
    let transcribe_time = started.elapsed();
    on_transcript(&transcript);

    let question = Question {
        transcript: transcript.clone(),
        screenshot_png: input.screenshot_png,
        elements: input
            .elements
            .as_ref()
            .map(render_for_prompt)
            .unwrap_or_default(),
    };
    let started = Instant::now();
    let raw = assistant.answer(&question).map_err(|message| Failure {
        stage: Stage::Answer,
        message,
    })?;
    let answer_time = started.elapsed();

    let elements = input.elements.as_ref();
    let parsed = prompt::parse_reply(&raw, |id| {
        elements.is_some_and(|list| list.get(id).is_some())
    });
    let target = parsed
        .point
        .and_then(|id| elements.and_then(|list| list.get(id)))
        .map(|element| Target {
            id: element.id,
            role: element.role.as_str().to_string(),
            name: element.name.clone(),
            rect: element.rect,
        });
    let text = match (parsed.text.is_empty(), &target) {
        (false, _) => parsed.text,
        (true, Some(_)) => "It's right here.".to_string(),
        (true, None) => {
            return Err(Failure {
                stage: Stage::Answer,
                message: "the model gave an empty reply".into(),
            });
        }
    };
    Ok(Reply {
        transcript,
        text,
        target,
        transcribe_time,
        answer_time,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::uia::{Element, Role};

    struct FakeStt(Result<String, String>);
    impl SpeechToText for FakeStt {
        fn transcribe(&self, _wav: &[u8]) -> Result<String, String> {
            self.0.clone()
        }
    }

    struct FakeAssistant {
        reply: Result<String, String>,
        seen: std::sync::Mutex<Option<Question>>,
    }
    impl FakeAssistant {
        fn replying(reply: &str) -> Self {
            Self {
                reply: Ok(reply.into()),
                seen: Default::default(),
            }
        }
    }
    impl Assistant for FakeAssistant {
        fn answer(&self, question: &Question) -> Result<String, String> {
            *self.seen.lock().unwrap() = Some(question.clone());
            self.reply.clone()
        }
    }

    fn list() -> ElementList {
        let element = |id, name: &str, x| Element {
            id,
            role: Role::Button,
            name: name.into(),
            rect: Rect::new(x, 10, 80, 30),
            automation_id: None,
        };
        ElementList {
            window_title: "Editor".into(),
            process_id: 1,
            window_rect: Rect::new(0, 0, 800, 600),
            elements: vec![element(1, "Open", 10), element(2, "Save", 100)],
            candidates: 2,
            visited: 2,
            pruned_offscreen: 0,
            elapsed_ms: 1,
            capped: false,
            partial: false,
            stop_reason: None,
        }
    }

    fn input(elements: Option<ElementList>) -> Input {
        Input {
            wav: vec![1, 2, 3],
            screenshot_png: Some(vec![9]),
            elements,
        }
    }

    fn ask(
        heard: &str,
        reply: &str,
        elements: Option<ElementList>,
    ) -> (Result<Reply, Failure>, FakeAssistant) {
        let assistant = FakeAssistant::replying(reply);
        let result = run(
            &FakeStt(Ok(heard.into())),
            &assistant,
            input(elements),
            |_| {},
        );
        (result, assistant)
    }

    #[test]
    fn points_at_the_element_the_model_names() {
        let (result, _) = ask("where do I save", "Click Save. [POINT:2]", Some(list()));
        let reply = result.unwrap();
        assert_eq!(reply.text, "Click Save.");
        assert_eq!(reply.transcript, "where do I save");
        let target = reply.target.unwrap();
        assert_eq!((target.id, target.name.as_str()), (2, "Save"));
        assert_eq!(target.role, "button");
        assert_eq!(target.rect, Rect::new(100, 10, 80, 30));
    }

    #[test]
    fn model_sees_transcript_screenshot_and_numbered_elements() {
        let (_, assistant) = ask("what is this", "A window. [POINT:none]", Some(list()));
        let seen = assistant.seen.lock().unwrap().clone().unwrap();
        assert_eq!(seen.transcript, "what is this");
        assert_eq!(seen.screenshot_png, Some(vec![9]));
        assert!(
            seen.elements.contains("2 button \"Save\""),
            "{}",
            seen.elements
        );
    }

    #[test]
    fn an_id_not_in_the_list_points_nowhere_but_still_replies() {
        let (result, _) = ask("hi", "Over there. [POINT:7]", Some(list()));
        let reply = result.unwrap();
        assert_eq!(reply.text, "Over there.");
        assert!(reply.target.is_none());
    }

    #[test]
    fn without_an_element_list_it_answers_without_pointing() {
        let (result, assistant) = ask("hi", "Hello! [POINT:1]", None);
        let reply = result.unwrap();
        assert!(reply.target.is_none());
        let seen = assistant.seen.lock().unwrap().clone().unwrap();
        assert!(seen.elements.is_empty());
    }

    #[test]
    fn transcript_is_reported_before_the_answer() {
        let heard = RefCell::new(None);
        let assistant = FakeAssistant::replying("ok [POINT:none]");
        run(
            &FakeStt(Ok(" hello ".into())),
            &assistant,
            input(None),
            |t| {
                *heard.borrow_mut() =
                    Some((t.to_string(), assistant.seen.lock().unwrap().is_none()));
            },
        )
        .unwrap();
        assert_eq!(heard.into_inner(), Some(("hello".to_string(), true)));
    }

    #[test]
    fn empty_transcript_stops_before_the_model() {
        let assistant = FakeAssistant::replying("never");
        let failure = run(&FakeStt(Ok("  ".into())), &assistant, input(None), |_| {}).unwrap_err();
        assert_eq!(failure.stage, Stage::Transcribe);
        assert!(assistant.seen.lock().unwrap().is_none());
    }

    #[test]
    fn failures_name_their_stage() {
        let assistant = FakeAssistant::replying("x");
        let failure = run(
            &FakeStt(Err("asr down".into())),
            &assistant,
            input(None),
            |_| {},
        )
        .unwrap_err();
        assert_eq!(
            failure,
            Failure {
                stage: Stage::Transcribe,
                message: "asr down".into()
            }
        );

        let assistant = FakeAssistant {
            reply: Err("vlm down".into()),
            seen: Default::default(),
        };
        let failure = run(&FakeStt(Ok("hi".into())), &assistant, input(None), |_| {}).unwrap_err();
        assert_eq!(
            failure,
            Failure {
                stage: Stage::Answer,
                message: "vlm down".into()
            }
        );
    }

    #[test]
    fn an_empty_reply_is_an_error_unless_it_points() {
        let (result, _) = ask("hi", "[POINT:none]", Some(list()));
        assert_eq!(result.unwrap_err().stage, Stage::Answer);
        let (result, _) = ask("hi", "[POINT:1]", Some(list()));
        let reply = result.unwrap();
        assert!(!reply.text.is_empty());
        assert_eq!(reply.target.unwrap().id, 1);
    }
}
