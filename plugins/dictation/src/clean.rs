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
    clean_transcript_with_dictionary(text, &[])
}

/// A wrong -> right replacement from the custom dictionary, applied to the FINAL
/// transcript text (not just as an STT hint). Mirrors the frontend
/// `DictionaryMapping` (`apps/desktop/src/dictation/dictionary.ts`) so the two
/// stay wire-compatible: this is the Rust side that `apply_dictionary` consumes.
///
/// Deriving `Deserialize`/`specta::Type` so it is ready to be threaded straight
/// from the stored dictionary setting once the command/session layer forwards it
/// (see the note on `clean_transcript_with_dictionary`).
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct DictionaryMapping {
    pub wrong: String,
    pub right: String,
    #[serde(default)]
    pub case_sensitive: bool,
}

/// Clean an accumulated transcript AND apply the custom-dictionary replacements
/// to the final text, for BOTH engines (whisper + parakeet — this layer is
/// engine-agnostic, it only sees text).
///
/// Order: collapse whitespace -> strip trailing partials -> apply dictionary ->
/// capitalize sentence starts. Dictionary runs before capitalization so a
/// replacement landing at a sentence start is still capitalized.
///
/// THREADING NOTE: `ext.rs::clean_text` (and the Tauri command behind it) is the
/// off-limits call site that currently calls `clean_transcript(text)` with no
/// mappings. To activate dictionary-on-output end to end, that command must parse
/// the stored `personalization_dictionary_terms` mapping entries into
/// `Vec<DictionaryMapping>` and call `clean_transcript_with_dictionary` instead.
pub fn clean_transcript_with_dictionary(text: &str, mappings: &[DictionaryMapping]) -> String {
    let collapsed = collapse_whitespace(text);
    let stripped = strip_trailing_partials(&collapsed);
    let mapped = apply_dictionary(&stripped, mappings);
    capitalize_sentence_starts(&mapped)
}

/// A word char for boundary detection — Unicode-aware like the frontend's
/// `\p{L}\p{N}\p{M}_`, using std only (no new dep). `char::is_alphanumeric`
/// covers letters + numbers; `_` is included explicitly.
///
/// Divergence from the frontend: std has no `\p{M}` (combining-mark) test, so a
/// lone combining mark at a boundary reads as a NON-word char here. In practice
/// dictionary terms are whole words, so this only matters for a term edged
/// exactly at a matra/virama seam — acceptable and documented.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Case-insensitive single-code-point comparison (used when a mapping is not
/// case-sensitive).
fn chars_eq_ci(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

struct PreparedRule {
    wrong: Vec<char>,
    right: String,
    case_sensitive: bool,
    first_is_word: bool,
    last_is_word: bool,
}

fn prepare_rules(mappings: &[DictionaryMapping]) -> Vec<PreparedRule> {
    let mut rules: Vec<PreparedRule> = mappings
        .iter()
        // Drop empty `wrong` and no-op mappings (`wrong == right`): a no-op can
        // never usefully fire and guarantees no self-replacement loop.
        .filter(|m| !m.wrong.is_empty() && m.wrong != m.right)
        .map(|m| {
            let wrong: Vec<char> = m.wrong.chars().collect();
            let first_is_word = wrong.first().copied().map(is_word_char).unwrap_or(false);
            let last_is_word = wrong.last().copied().map(is_word_char).unwrap_or(false);
            PreparedRule {
                wrong,
                right: m.right.clone(),
                case_sensitive: m.case_sensitive,
                first_is_word,
                last_is_word,
            }
        })
        .collect();

    // Longest `wrong` first so overlapping terms ("notare" vs "note") resolve to
    // the most specific match at any given position.
    rules.sort_by(|a, b| b.wrong.len().cmp(&a.wrong.len()));
    rules
}

/// Apply the dictionary's replacement mappings to `text` in a single
/// left-to-right pass. Port of the frontend `applyDictionary`. Guarantees:
///
/// - **surrogate/astral safe** — operates on Rust `char`s (full code points), so
///   there are no UTF-16 surrogate seams to worry about;
/// - **word-boundary safe** — a mapping whose `wrong` starts/ends with a word
///   char won't match mid-word; a `wrong` edged by punctuation (e.g. `C++`)
///   relaxes the boundary on that side;
/// - **literal** — `wrong` is matched as a plain string, no regex;
/// - **longest-wrong-first** precedence for overlaps;
/// - **per-mapping case sensitivity** (case-insensitive by default);
/// - **single pass, no cascading** — an emitted `right` is never re-scanned.
///
/// Deterministic and synchronous. Flat-string dictionary terms (STT hints) are
/// not represented here; only replacement mappings are passed in.
pub fn apply_dictionary(text: &str, mappings: &[DictionaryMapping]) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    let rules = prepare_rules(mappings);
    if rules.is_empty() {
        return text.to_string();
    }

    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;

    while i < n {
        let mut matched = false;

        for rule in &rules {
            let len = rule.wrong.len();
            let j = i + len;
            if j > n {
                continue;
            }

            let hit = if rule.case_sensitive {
                (0..len).all(|k| chars[i + k] == rule.wrong[k])
            } else {
                (0..len).all(|k| chars_eq_ci(chars[i + k], rule.wrong[k]))
            };
            if !hit {
                continue;
            }

            // Boundary check — only enforced on a side whose edge char is a word
            // char, so punctuation-edged terms still match adjacent letters.
            if rule.first_is_word && i > 0 && is_word_char(chars[i - 1]) {
                continue;
            }
            if rule.last_is_word && j < n && is_word_char(chars[j]) {
                continue;
            }

            out.push_str(&rule.right);
            i = j;
            matched = true;
            break;
        }

        if !matched {
            out.push(chars[i]);
            i += 1;
        }
    }

    out
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
    use super::{
        DictionaryMapping, apply_dictionary, clean_transcript, clean_transcript_with_dictionary,
    };

    fn m(wrong: &str, right: &str, cs: bool) -> DictionaryMapping {
        DictionaryMapping {
            wrong: wrong.to_string(),
            right: right.to_string(),
            case_sensitive: cs,
        }
    }

    #[test]
    fn dictionary_basic_replacement_case_insensitive_by_default() {
        let rules = [m("notare", "Notare", false)];
        assert_eq!(
            apply_dictionary("open notare and Notare", &rules),
            "open Notare and Notare"
        );
    }

    #[test]
    fn dictionary_respects_case_sensitivity() {
        let rules = [m("ios", "iOS", true)];
        // Exact case matches; other casings are left untouched.
        assert_eq!(
            apply_dictionary("ship ios not IOS", &rules),
            "ship iOS not IOS"
        );
    }

    #[test]
    fn dictionary_is_word_boundary_safe() {
        let rules = [m("note", "NOTE", false)];
        // "notare" must NOT be rewritten to "NOTEare".
        assert_eq!(
            apply_dictionary("a note in notare", &rules),
            "a NOTE in notare"
        );
    }

    #[test]
    fn dictionary_punctuation_edged_term_relaxes_boundary() {
        let rules = [m("c++", "cpp", false)];
        // Trailing '+' is a non-word char, so the term matches against an
        // adjacent letter/space on that side.
        assert_eq!(
            apply_dictionary("i love c++ code", &rules),
            "i love cpp code"
        );
    }

    #[test]
    fn dictionary_longest_wrong_wins() {
        let rules = [m("new york", "NYC", false), m("new", "NEW", false)];
        assert_eq!(apply_dictionary("new york new", &rules), "NYC NEW");
    }

    #[test]
    fn dictionary_single_pass_no_cascade() {
        // "a" -> "b" and "b" -> "c": an emitted "b" must NOT be rewritten to "c".
        let rules = [m("a", "b", false), m("b", "c", false)];
        assert_eq!(apply_dictionary("a b", &rules), "b c");
    }

    #[test]
    fn dictionary_ignores_empty_and_noop_mappings() {
        let rules = [m("", "x", false), m("same", "same", false)];
        assert_eq!(apply_dictionary("same and empty", &rules), "same and empty");
    }

    #[test]
    fn dictionary_astral_and_cjk_safe() {
        // Emoji (astral plane) and CJK are single Rust chars; matching a term
        // next to them must not corrupt the code points.
        let rules = [m("hi", "HELLO", false)];
        assert_eq!(
            apply_dictionary("hi 😀 hi 世界", &rules),
            "HELLO 😀 HELLO 世界"
        );
    }

    #[test]
    fn dictionary_empty_inputs() {
        assert_eq!(apply_dictionary("", &[m("a", "b", false)]), "");
        assert_eq!(apply_dictionary("unchanged", &[]), "unchanged");
    }

    #[test]
    fn clean_with_dictionary_applies_then_capitalizes() {
        let rules = [m("notare", "Notare", false)];
        // Whitespace collapsed, mapping applied, sentence start capitalized.
        assert_eq!(
            clean_transcript_with_dictionary("  open   notare now  ", &rules),
            "Open Notare now"
        );
    }

    #[test]
    fn clean_transcript_still_equals_no_dictionary_path() {
        assert_eq!(
            clean_transcript("hello there. this works!"),
            clean_transcript_with_dictionary("hello there. this works!", &[])
        );
    }

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
        assert_eq!(
            clean_transcript("run the tests [BLANK_AUDIO]"),
            "Run the tests"
        );
        assert_eq!(
            clean_transcript("run the tests (inaudible)"),
            "Run the tests"
        );
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
