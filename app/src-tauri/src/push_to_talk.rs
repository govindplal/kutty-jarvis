//! Push-to-talk state machine.
//!
//! The global shortcut fires a `Pressed` event repeatedly while the key is held
//! (keyboard auto-repeat) and a single `Released` event. This type turns that
//! noisy stream into clean `Idle -> Listening -> Idle` transitions, so the rest
//! of the app only reacts once per press.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PushToTalkState {
    Idle,
    Listening,
}

#[derive(Debug)]
pub struct PushToTalk {
    state: PushToTalkState,
}

impl Default for PushToTalk {
    fn default() -> Self {
        Self {
            state: PushToTalkState::Idle,
        }
    }
}

impl PushToTalk {
    #[cfg(test)]
    pub fn state(&self) -> PushToTalkState {
        self.state
    }

    /// Key went down. Returns the new state only if this is a real transition.
    pub fn press(&mut self) -> Option<PushToTalkState> {
        self.transition_to(PushToTalkState::Listening)
    }

    /// Key went up. Returns the new state only if this is a real transition.
    pub fn release(&mut self) -> Option<PushToTalkState> {
        self.transition_to(PushToTalkState::Idle)
    }

    fn transition_to(&mut self, next: PushToTalkState) -> Option<PushToTalkState> {
        if self.state == next {
            return None;
        }
        self.state = next;
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_idle() {
        assert_eq!(PushToTalk::default().state(), PushToTalkState::Idle);
    }

    #[test]
    fn press_then_release_produces_two_transitions() {
        let mut ptt = PushToTalk::default();
        assert_eq!(ptt.press(), Some(PushToTalkState::Listening));
        assert_eq!(ptt.release(), Some(PushToTalkState::Idle));
    }

    #[test]
    fn auto_repeat_presses_are_ignored() {
        let mut ptt = PushToTalk::default();
        assert_eq!(ptt.press(), Some(PushToTalkState::Listening));
        assert_eq!(ptt.press(), None);
        assert_eq!(ptt.press(), None);
        assert_eq!(ptt.state(), PushToTalkState::Listening);
    }

    #[test]
    fn release_without_press_is_ignored() {
        let mut ptt = PushToTalk::default();
        assert_eq!(ptt.release(), None);
        assert_eq!(ptt.state(), PushToTalkState::Idle);
    }

    #[test]
    fn serialises_as_lowercase_for_the_frontend() {
        assert_eq!(
            serde_json::to_string(&PushToTalkState::Listening).unwrap(),
            "\"listening\""
        );
    }
}
