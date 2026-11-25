/// Shortest run of back-to-back repeats treated as a decoding loop. Real speech
/// repeats a word or phrase two or three times ("no, no, no"); greedy decoding
/// that loops on a short or near-silent chunk repeats it until the token cap.
const MIN_LOOP_REPEATS: usize = 4;

/// Longest phrase, in words, checked for a loop.
const MAX_LOOP_PHRASE_WORDS: usize = 8;

/// Collapse every phrase that repeats back-to-back [`MIN_LOOP_REPEATS`] or more
/// times into a single occurrence, along with a truncated repeat that the
/// token cap cut off right after it. Text without such a loop comes back
/// unchanged.
pub(crate) fn collapse_repetition_loops(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let keys: Vec<String> = words.iter().map(|word| comparison_key(word)).collect();

    let mut kept: Vec<&str> = Vec::with_capacity(words.len());
    let mut collapsed_any = false;
    let mut i = 0;
    while i < words.len() {
        match loop_at(&keys, i) {
            Some((phrase_len, end)) => {
                kept.extend_from_slice(&words[i..i + phrase_len]);
                collapsed_any = true;
                i = end;
            }
            None => {
                kept.push(words[i]);
                i += 1;
            }
        }
    }

    if collapsed_any {
        kept.join(" ")
    } else {
        text.to_string()
    }
}

/// The shortest phrase starting at `start` that repeats at least
/// [`MIN_LOOP_REPEATS`] times, as `(phrase length, index after the loop)`.
/// The loop also swallows a partial repeat that follows it.
fn loop_at(keys: &[String], start: usize) -> Option<(usize, usize)> {
    for len in 1..=MAX_LOOP_PHRASE_WORDS {
        let phrase = keys.get(start..start + len)?;
        if phrase.iter().any(String::is_empty) {
            continue;
        }

        let mut end = start + len;
        while keys.get(end..end + len) == Some(phrase) {
            end += len;
        }
        if (end - start) / len < MIN_LOOP_REPEATS {
            continue;
        }

        let partial = keys[end..]
            .iter()
            .zip(phrase)
            .take(len - 1)
            .take_while(|(word, expected)| word == expected)
            .count();
        return Some((len, end + partial));
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
    fn leaves_ordinary_repetition_alone() {
        for text in [
            "no, no, no, that is wrong",
            "very very good",
            "I think, I think we should wait",
            "",
        ] {
            assert_eq!(collapse_repetition_loops(text), text);
        }
    }

    #[test]
    fn keeps_whitespace_when_nothing_loops() {
        let text = "line one\nline two  with  spaces";
        assert_eq!(collapse_repetition_loops(text), text);
    }

    #[test]
    fn single_word_loops_collapse_with_case_and_punctuation_ignored() {
        assert_eq!(
            collapse_repetition_loops("okay Okay, okay. OKAY okay so"),
            "okay so"
        );
    }

    #[test]
    fn devanagari_words_differing_by_a_vowel_sign_are_not_a_loop() {
        let text = "का की के को";
        assert_eq!(collapse_repetition_loops(text), text);
        assert_eq!(collapse_repetition_loops("है। है। है। है।"), "है।");
    }
}
