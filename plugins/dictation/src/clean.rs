//! Deterministic cleanup of an accumulated dictation transcript before it is
//! pasted in batch mode (`DictationOutputMode::BatchPaste`). Pure string
//! processing - deliberately no LLM involved:
//!
//! 1. collapse whitespace runs to single spaces and trim;
//! 2. strip trailing partials: dangling hyphenated word fragments (`transcri-`),
//!    bracketed non-speech artifacts (`[BLANK_AUDIO]`, `(inaudible)`) and lone
//!    punctuation left at the end by the final flush;
//! 3. capitalize sentence starts (the first letter, and the first letter after
//!    `.`, `!` or `?`).

/// Clean an accumulated transcript for pasting. Returns an empty string when
/// nothing usable remains.
pub fn clean_transcript(text: &str) -> String {
    let collapsed = collapse_whitespace(text);
    let stripped = strip_trailing_partials(&collapsed);
    capitalize_sentence_starts(&stripped)
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Drop trailing tokens that are recognizable partials rather than speech:
/// hyphen-dangling word fragments, fully bracketed artifacts, or tokens with
/// no alphanumeric content at all (stray punctuation).
fn strip_trailing_partials(text: &str) -> String {
    let mut tokens: Vec<&str> = text.split(' ').filter(|t| !t.is_empty()).collect();

    while let Some(last) = tokens.last() {
        if is_trailing_partial(last) {
            tokens.pop();
        } else {
            break;
        }
    }

    tokens.join(" ")
}

fn is_trailing_partial(token: &str) -> bool {
    if token.ends_with('-') {
        return true;
    }
    if (token.starts_with('[') && token.ends_with(']'))
        || (token.starts_with('(') && token.ends_with(')'))
    {
        return true;
    }
    !token.chars().any(char::is_alphanumeric)
}

fn capitalize_sentence_starts(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut capitalize_next = true;

    for ch in text.chars() {
        if capitalize_next && ch.is_alphabetic() {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
            continue;
        }

        if matches!(ch, '.' | '!' | '?') {
            capitalize_next = true;
        } else if !ch.is_whitespace() && !ch.is_alphabetic() {
            // Digits, quotes, etc. start the sentence without being
            // capitalizable themselves ("42 is the answer").
            if ch.is_alphanumeric() {
                capitalize_next = false;
            }
        } else if ch.is_alphabetic() {
            capitalize_next = false;
        }

        result.push(ch);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::clean_transcript;

    #[test]
    fn empty_and_whitespace_only_become_empty() {
        assert_eq!(clean_transcript(""), "");
        assert_eq!(clean_transcript("   \n\t  "), "");
    }

    #[test]
    fn collapses_whitespace_and_trims() {
        assert_eq!(
            clean_transcript("  hello   world \n this  is\tnotare  "),
            "Hello world this is notare"
        );
    }

    #[test]
    fn capitalizes_sentence_starts() {
        assert_eq!(
            clean_transcript("hello there. this works! does it? yes"),
            "Hello there. This works! Does it? Yes"
        );
    }

    #[test]
    fn does_not_touch_existing_capitals_mid_sentence() {
        assert_eq!(
            clean_transcript("open the README in VS Code"),
            "Open the README in VS Code"
        );
    }

    #[test]
    fn capitalizes_after_digits_only_at_sentence_starts() {
        assert_eq!(
            clean_transcript("42 is the answer. it really is"),
            "42 is the answer. It really is"
        );
    }

    #[test]
    fn strips_trailing_hyphen_fragment() {
        assert_eq!(clean_transcript("so i was transcri-"), "So i was");
    }

    #[test]
    fn strips_trailing_bracketed_artifacts_and_stray_punctuation() {
        assert_eq!(clean_transcript("run the tests [BLANK_AUDIO]"), "Run the tests");
        assert_eq!(clean_transcript("run the tests (inaudible)"), "Run the tests");
        assert_eq!(clean_transcript("run the tests ,"), "Run the tests");
    }

    #[test]
    fn strips_stacked_trailing_partials() {
        assert_eq!(
            clean_transcript("ship it now [BLANK_AUDIO] transcri- ."),
            "Ship it now"
        );
    }

    #[test]
    fn keeps_brackets_and_hyphens_mid_text() {
        assert_eq!(
            clean_transcript("use the [debug] build for a dry-run today"),
            "Use the [debug] build for a dry-run today"
        );
    }

    #[test]
    fn only_partials_becomes_empty() {
        assert_eq!(clean_transcript("[BLANK_AUDIO] -"), "");
    }
}
