//! Prompt and reply parsing for the buddy. Written for this project; the
//! pointing tag is ours (`[POINT:12]`), chosen because small models copy a
//! simple fixed format reliably.

use crate::models::Question;

/// Small local models follow the two examples far better than the rules
/// alone (measured: without them the 1.6B model often echoed the list).
pub const SYSTEM: &str = "You are kutty-jarvis, a friendly buddy on the user's PC. \
You can see their screen. The user asks a question out loud and your reply is spoken aloud, \
so write the way people talk: one to three short sentences, no lists, no markdown.\n\n\
You are given a screenshot and a numbered list of UI elements. If your answer is about one \
element, name it in words and finish with its number as [POINT:n]. If no single element fits, \
finish with [POINT:none]. Put the tag only at the very end, use only numbers from the list, \
and never repeat the list.\n\n\
Examples of good replies:\n\
User said: \"how do I export this?\" -> The Export button is in the top bar, I'll point at it. [POINT:7]\n\
User said: \"what is this program?\" -> This looks like a spreadsheet with a table of sales figures. [POINT:none]";

/// The text part of the user turn: what was said plus the element list.
pub fn user_message(question: &Question) -> String {
    let elements = if question.elements.trim().is_empty() {
        "No element list is available, so end with [POINT:none].".to_string()
    } else {
        question.elements.trim_end().to_string()
    };
    format!(
        "The user said: \"{}\"\n\n{}",
        question.transcript.trim(),
        elements
    )
}

/// A reply split into what to say and what to point at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedReply {
    pub text: String,
    /// Element number from the last well-formed tag that passed validation.
    pub point: Option<usize>,
}

const TAG_START: &str = "point";

/// Splits the raw model reply into spoken text and an optional element id.
///
/// Tolerates `[POINT:12]`, `[point: 12]`, `(POINT:12)`, a tag cut off by the
/// token limit, and several tags (the last valid one wins). Tags with an
/// unknown id, `none` or garbage are removed and point at nothing.
pub fn parse_reply(raw: &str, is_valid: impl Fn(usize) -> bool) -> ParsedReply {
    let lower = raw.to_ascii_lowercase();
    let mut text = String::new();
    let mut point = None;
    let mut cursor = 0;

    while let Some(offset) = lower[cursor..].find(TAG_START) {
        let keyword = cursor + offset;
        // Needs an opening bracket right before the keyword (spaces allowed).
        let before = raw[..keyword].trim_end();
        let opened = before.ends_with('[') || before.ends_with('(');
        let keyword_end = keyword + TAG_START.len();
        let rest = raw[keyword_end..].trim_start();
        let after = raw.len() - rest.len();
        if !opened || !rest.starts_with(':') {
            text.push_str(&raw[cursor..keyword_end]);
            cursor = keyword_end;
            continue;
        }
        text.push_str(&raw[cursor..(before.len() - 1).max(cursor)]);
        let after = after + 1;
        let rest = &rest[1..];
        let end = rest.find([']', ')']).map_or(rest.len(), |close| close + 1);
        let body = rest[..end].trim_end_matches([']', ')']);
        if let Some(id) = parse_id(body)
            && is_valid(id)
        {
            point = Some(id);
        }
        cursor = after + end;
    }
    text.push_str(&raw[cursor..]);

    ParsedReply {
        text: tidy(&text),
        point,
    }
}

/// Reads `12`, `#12` or `id 12`; anything else (including `none`) is no id.
fn parse_id(body: &str) -> Option<usize> {
    let body = body.trim().to_ascii_lowercase();
    let body = body.strip_prefix("id").unwrap_or(&body).trim_start();
    let body = body.strip_prefix('#').unwrap_or(body).trim();
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    body.parse().ok()
}

/// Collapses whitespace so a stripped tag leaves no gap.
fn tidy(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(id: usize) -> bool {
        (1..=20).contains(&id)
    }

    fn parse(raw: &str) -> ParsedReply {
        parse_reply(raw, valid)
    }

    #[test]
    fn strips_a_trailing_tag_and_returns_the_id() {
        let parsed = parse("That's the Save button. [POINT:12]");
        assert_eq!(parsed.text, "That's the Save button.");
        assert_eq!(parsed.point, Some(12));
    }

    #[test]
    fn none_means_no_pointing() {
        let parsed = parse("It looks like a text editor. [POINT:none]");
        assert_eq!(parsed.text, "It looks like a text editor.");
        assert_eq!(parsed.point, None);
    }

    #[test]
    fn tolerates_case_spacing_and_brackets() {
        assert_eq!(parse("Here. [point: 7]").point, Some(7));
        assert_eq!(parse("Here. [ POINT : 7 ]").point, Some(7));
        assert_eq!(parse("Here. (POINT:7)").point, Some(7));
        assert_eq!(parse("Here. [POINT:#7]").point, Some(7));
        assert_eq!(parse("Here. [POINT:id 7]").point, Some(7));
        assert_eq!(parse("Here.\n\n[POINT:7]\n").text, "Here.");
    }

    #[test]
    fn tag_in_the_middle_leaves_clean_text() {
        let parsed = parse("Click Save [POINT:3] and then close it.");
        assert_eq!(parsed.text, "Click Save and then close it.");
        assert_eq!(parsed.point, Some(3));
    }

    #[test]
    fn unknown_ids_are_dropped_but_still_stripped() {
        let parsed = parse("Over there. [POINT:99]");
        assert_eq!(parsed.text, "Over there.");
        assert_eq!(parsed.point, None);
        assert_eq!(parse("Zero. [POINT:0]").point, None);
    }

    #[test]
    fn garbage_in_the_tag_points_nowhere() {
        for raw in [
            "x [POINT:]",
            "x [POINT:abc]",
            "x [POINT:-3]",
            "x [POINT:3.5]",
            "x [POINT:12abc]",
        ] {
            let parsed = parse(raw);
            assert_eq!(parsed.point, None, "{raw}");
            assert_eq!(parsed.text, "x", "{raw}");
        }
    }

    #[test]
    fn a_tag_cut_off_by_the_token_limit_still_works() {
        let parsed = parse("The menu is up there. [POINT:5");
        assert_eq!(parsed.text, "The menu is up there.");
        assert_eq!(parsed.point, Some(5));
        assert_eq!(parse("Cut off [POINT:").point, None);
    }

    #[test]
    fn the_last_valid_tag_wins() {
        let parsed = parse("A [POINT:2] B [POINT:none] C [POINT:4]");
        assert_eq!(parsed.point, Some(4));
        assert_eq!(parsed.text, "A B C");
    }

    #[test]
    fn plain_text_without_a_tag_is_kept() {
        let parsed = parse("I am not sure what that is.");
        assert_eq!(parsed.text, "I am not sure what that is.");
        assert_eq!(parsed.point, None);
    }

    #[test]
    fn the_word_point_in_prose_is_not_a_tag() {
        let parsed = parse("The point: it saves your work. [POINT:1]");
        assert_eq!(parsed.text, "The point: it saves your work.");
        assert_eq!(parsed.point, Some(1));
    }

    #[test]
    fn handles_non_ascii_text_around_the_tag() {
        let parsed = parse("Voilà, l'icône « Enregistrer » [POINT:2] ✓");
        assert_eq!(parsed.text, "Voilà, l'icône « Enregistrer » ✓");
        assert_eq!(parsed.point, Some(2));
    }

    #[test]
    fn empty_and_tag_only_replies_give_empty_text() {
        assert_eq!(parse("").text, "");
        let parsed = parse("[POINT:3]");
        assert_eq!(parsed.text, "");
        assert_eq!(parsed.point, Some(3));
    }

    fn question(transcript: &str, elements: &str) -> Question {
        Question {
            transcript: transcript.into(),
            screenshot_png: None,
            elements: elements.into(),
        }
    }

    #[test]
    fn user_message_contains_the_transcript_and_elements() {
        let message = user_message(&question("  where is save? ", "1 button \"Save\"\n"));
        assert!(message.starts_with("The user said: \"where is save?\""));
        assert!(message.ends_with("1 button \"Save\""));
    }

    #[test]
    fn user_message_says_when_there_is_no_element_list() {
        let message = user_message(&question("hello", "  \n"));
        assert!(message.contains("No element list is available"));
    }

    #[test]
    fn system_prompt_documents_the_tag_format() {
        assert!(SYSTEM.contains("[POINT:n]"));
        assert!(SYSTEM.contains("[POINT:none]"));
    }
}
