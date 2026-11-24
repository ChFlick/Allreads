//! Sentence boundaries, following Lukaisu's `findLatinSentenceEnd` rules.

use std::ops::Range;

use super::LanguageRules;

/// Closing marks that may follow a sentence end and belong to that sentence.
const CLOSERS: &str = "]'`\"”)‘’‹›“„«»』」";

/// Closing marks that may also follow after a space, as French writes `? »`.
/// Only unambiguous closers: `«` and `"` could open the next sentence instead.
const SPACED_CLOSERS: &str = "»›”’)]」』";

/// Splits `text` into sentences, as byte ranges that cover the whole text.
///
/// A sentence ends at one or more end marks (e.g. `.`, `?!`, `...`), plus any
/// closing quotes or brackets, when whitespace or the end of the text follows.
/// The whitespace after a sentence starts the next one. Not an end:
/// - a short number before a dot (`3.` in `am 3. Oktober`);
/// - a dot after an all-consonant word or one capital vowel (`Mr.`, `Dr.`, `A.`);
/// - a `.` or `:` before a lower-case word (`etc. and`);
/// - anything the language's exceptions match (`Mr.`, `[A-Z].`).
#[must_use]
pub fn sentence_ranges(text: &str, rules: &LanguageRules) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut boundaries = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !rules.end_chars.contains(&chars[i].1) {
            i += 1;
            continue;
        }
        let mut mark_end = i;
        while mark_end < chars.len() && rules.end_chars.contains(&chars[mark_end].1) {
            mark_end += 1;
        }
        let mut end = mark_end;
        while end < chars.len() && CLOSERS.contains(chars[end].1) {
            end += 1;
        }
        loop {
            let mut next = end;
            while next < chars.len() && chars[next].1.is_whitespace() {
                next += 1;
            }
            if next > end && next < chars.len() && SPACED_CLOSERS.contains(chars[next].1) {
                end = next + 1;
            } else {
                break;
            }
        }
        let at_text_end = end == chars.len();
        if at_text_end || chars[end].1.is_whitespace() {
            let mark: String = chars[i..mark_end].iter().map(|&(_, c)| c).collect();
            let before = word_before(&chars, i);
            let after = word_after(&chars, end);
            if ends_sentence(&before, &mark, &after, rules) {
                boundaries.push(if at_text_end {
                    text.len()
                } else {
                    chars[end].0
                });
            }
        }
        i = end.max(i + 1);
    }

    let mut ranges = Vec::with_capacity(boundaries.len() + 1);
    let mut start = 0;
    for boundary in boundaries {
        if boundary > start {
            ranges.push(start..boundary);
            start = boundary;
        }
    }
    if start < text.len() {
        ranges.push(start..text.len());
    }
    ranges
}

/// The run of non-whitespace before index `i`, skipping whitespace first, so
/// French spacing (`va ?`) still finds `va`.
fn word_before(chars: &[(usize, char)], i: usize) -> String {
    let mut end = i;
    while end > 0 && chars[end - 1].1.is_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && !chars[start - 1].1.is_whitespace() {
        start -= 1;
    }
    chars[start..end].iter().map(|&(_, c)| c).collect()
}

/// The run of non-whitespace after index `i`, skipping whitespace first.
fn word_after(chars: &[(usize, char)], i: usize) -> String {
    chars[i..]
        .iter()
        .map(|&(_, c)| c)
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| !c.is_whitespace())
        .collect()
}

fn ends_sentence(before: &str, mark: &str, after: &str, rules: &LanguageRules) -> bool {
    if before.is_empty() {
        return false;
    }
    let is_number = before.chars().all(|c| c.is_ascii_digit());
    if is_number && before.len() < 3 {
        return false;
    }
    let is_dot = mark.starts_with('.');
    if !is_number && is_dot && (is_all_consonants(before) || is_capital_vowel(before)) {
        return false;
    }
    let lower_case_follows = after.chars().next().is_some_and(char::is_lowercase);
    if (mark.contains('.') || mark.contains(':')) && lower_case_follows {
        return false;
    }
    if let Some(exceptions) = &rules.exceptions {
        if exceptions.is_match(&format!("{before}{mark}")) {
            return false;
        }
    }
    true
}

/// True for abbreviations like `Mr`, `Dr` or `St`: Latin consonants only.
fn is_all_consonants(word: &str) -> bool {
    word.chars()
        .all(|c| (c.is_ascii_alphabetic() && !"aeiouyAEIOUY".contains(c)) || c == 'ñ')
}

/// True for an initial like `A` or `E`.
fn is_capital_vowel(word: &str) -> bool {
    matches!(word, "A" | "E" | "I" | "O" | "U" | "Y")
}

#[cfg(test)]
mod tests {
    use super::super::test_rules::builtin;
    use super::*;

    fn sentences<'a>(text: &'a str, code: &str) -> Vec<&'a str> {
        sentence_ranges(text, &builtin(code))
            .into_iter()
            .map(|range| &text[range])
            .collect()
    }

    #[test]
    fn whitespace_after_an_end_starts_the_next_sentence() {
        assert_eq!(
            sentences("The fox jumps. The dog sleeps!", "en"),
            ["The fox jumps.", " The dog sleeps!"]
        );
    }

    #[test]
    fn closing_quotes_stay_with_their_sentence() {
        assert_eq!(
            sentences("She said, \"Hello there.\" Then she left.", "en"),
            ["She said, \"Hello there.\"", " Then she left."]
        );
    }

    #[test]
    fn abbreviations_do_not_end_a_sentence() {
        assert_eq!(
            sentences("Mr. Smith met Dr. Jones. They left!", "en"),
            ["Mr. Smith met Dr. Jones.", " They left!"]
        );
    }

    #[test]
    fn german_ordinals_do_not_end_a_sentence() {
        assert_eq!(
            sentences("Er kam am 3. Oktober. Dann ging er.", "de"),
            ["Er kam am 3. Oktober.", " Dann ging er."]
        );
    }

    #[test]
    fn french_spacing_before_a_question_mark_still_ends_it() {
        assert_eq!(
            sentences("Comment ça va ? Très bien.", "fr"),
            ["Comment ça va ?", " Très bien."]
        );
    }

    #[test]
    fn a_colon_before_lower_case_continues_the_sentence() {
        assert_eq!(
            sentences("Il dit : oui. Fin.", "fr"),
            ["Il dit : oui.", " Fin."]
        );
    }

    #[test]
    fn decimals_and_initialisms_without_spaces_are_not_ends() {
        assert_eq!(
            sentences("It costs 3.14 in the U.S.A today.", "en"),
            ["It costs 3.14 in the U.S.A today."]
        );
    }

    #[test]
    fn ellipsis_before_lower_case_continues() {
        assert_eq!(
            sentences("Et puis... rien. Fin.", "fr"),
            ["Et puis... rien.", " Fin."]
        );
    }

    #[test]
    fn a_spaced_closing_guillemet_stays_with_its_sentence() {
        assert_eq!(
            sentences("« Vraiment ? » Il rit.", "fr"),
            ["« Vraiment ? »", " Il rit."]
        );
    }

    #[test]
    fn an_opening_guillemet_after_a_space_starts_the_next_sentence() {
        assert_eq!(
            sentences("Il dit. « Oui. »", "fr"),
            ["Il dit.", " « Oui. »"]
        );
    }

    #[test]
    fn several_marks_end_once() {
        assert_eq!(sentences("Quoi ?! Non.", "fr"), ["Quoi ?!", " Non."]);
    }

    #[test]
    fn text_without_an_end_mark_is_one_sentence() {
        assert_eq!(sentences("Pas de point", "fr"), ["Pas de point"]);
        assert!(sentences("", "fr").is_empty());
    }
}
