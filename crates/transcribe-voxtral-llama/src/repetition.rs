/// Shortest run of back-to-back repeats treated as a decoding loop. Real speech
/// repeats a word or phrase two or three times ("no, no, no"); greedy decoding
/// that loops on a short or near-silent chunk repeats it until the token cap.
const MIN_LOOP_REPEATS: usize = 4;

/// Longest phrase, in words, checked for a loop.
const MAX_LOOP_PHRASE_WORDS: usize = 8;

/// Collapse a decoding loop at the end of the text to its first occurrence.
///
/// A greedy-decoding loop never stops by itself: it runs to the token cap, so
/// it always ends the text, usually with the last repeat cut off. Only such a
/// trailing run is collapsed: a phrase of up to [`MAX_LOOP_PHRASE_WORDS`] words
/// repeated [`MIN_LOOP_REPEATS`]+ times with nothing after it but a partial
/// repeat. Repetition followed by other words is speech ("no no no no and he
/// left") and is left alone. Everything kept is the original text, whitespace
/// included.
pub(crate) fn collapse_repetition_loops(text: &str) -> String {
    let words: Vec<(usize, &str)> = word_spans(text);
    let keys: Vec<String> = words.iter().map(|(_, word)| comparison_key(word)).collect();

    for start in 0..keys.len() {
        if let Some(phrase_len) = trailing_loop_at(&keys, start) {
            let (offset, word) = words[start + phrase_len - 1];
            return text[..offset + word.len()].to_string();
        }
    }
    text.to_string()
}

/// Byte offset and text of every whitespace-separated word.
fn word_spans(text: &str) -> Vec<(usize, &str)> {
    let base = text.as_ptr() as usize;
    text.split_whitespace()
        .map(|word| (word.as_ptr() as usize - base, word))
        .collect()
}

/// The shortest phrase starting at `start` that repeats [`MIN_LOOP_REPEATS`]+
/// times and then runs to the end of the text, allowing a cut-off final repeat.
fn trailing_loop_at(keys: &[String], start: usize) -> Option<usize> {
    for len in 1..=MAX_LOOP_PHRASE_WORDS {
        let Some(phrase) = keys.get(start..start + len) else {
            break;
        };
        if phrase.iter().any(String::is_empty) {
            continue;
        }

        let mut end = start + len;
        while keys.get(end..end + len) == Some(phrase) {
            end += len;
        }
        let tail = &keys[end..];
        let tail_is_partial_repeat =
            tail.len() < len && tail.iter().zip(phrase).all(|(a, b)| a == b);
        if (end - start) / len >= MIN_LOOP_REPEATS && tail_is_partial_repeat {
            return Some(len);
        }
    }
    None
}

/// Case- and punctuation-insensitive form of a word. Only punctuation is
/// trimmed: combining marks (Devanagari vowel signs, for one) are part of the
/// word, so "का" and "की" stay different.
fn comparison_key(word: &str) -> String {
    word.trim_matches(|c: char| c.is_ascii_punctuation() || "।॥…“”‘’«»¿¡、。，！？".contains(c))
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_a_looping_sentence() {
        assert_eq!(
            collapse_repetition_loops(
                "We should ship it. Thank you. Thank you. Thank you. Thank you. Thank you."
            ),
            "We should ship it. Thank you."
        );
    }

    #[test]
    fn drops_the_repeat_the_token_cap_cut_off() {
        assert_eq!(
            collapse_repetition_loops(
                "go to the store go to the store go to the store go to the store go to"
            ),
            "go to the store"
        );
    }

    #[test]
    fn keeps_repetition_that_speech_continues_after() {
        for text in [
            "no, no, no, that is wrong",
            "very very good",
            "I told him no no no no and he left",
            "go to the store go to the store go to the store go to the store go to the park",
            "",
        ] {
            assert_eq!(collapse_repetition_loops(text), text);
        }
    }

    #[test]
    fn keeps_the_original_whitespace_before_the_loop() {
        assert_eq!(
            collapse_repetition_loops("line one\nline  two\tok ok ok ok ok"),
            "line one\nline  two\tok"
        );
        let text = "line one\nline two  with  spaces";
        assert_eq!(collapse_repetition_loops(text), text);
    }

    #[test]
    fn single_word_loops_collapse_with_case_and_punctuation_ignored() {
        assert_eq!(
            collapse_repetition_loops("so okay Okay, okay. OKAY okay"),
            "so okay"
        );
    }

    #[test]
    fn devanagari_words_differing_by_a_vowel_sign_are_not_a_loop() {
        let text = "का की के को";
        assert_eq!(collapse_repetition_loops(text), text);
        assert_eq!(collapse_repetition_loops("है। है। है। है।"), "है।");
    }
}
