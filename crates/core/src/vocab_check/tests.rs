use super::*;

/// 30,000 stand-in dictionary forms, most common first: words of two to
/// four random syllables, a sparse sample of millions of possible ones, so
/// made-up words spliced from them are as rarely real (or one letter from a
/// real one) as in a real language.
fn ranked() -> Vec<String> {
    words().to_vec()
}

/// The stand-in words, made once.
fn words() -> &'static [String] {
    use std::sync::OnceLock;
    static WORDS: OnceLock<Vec<String>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let onsets = [
            "b", "d", "f", "g", "l", "m", "n", "p", "r", "s", "t", "v", "ch", "br", "tr", "pl",
        ];
        let vowels = ["a", "e", "i", "o", "u", "ou", "ai", "é"];
        let codas = ["", "", "", "r", "n", "s", "l"];
        let mut rng = Rng::new(2026);
        let mut seen = std::collections::HashSet::new();
        let mut words = Vec::new();
        while words.len() < 30_000 {
            let syllables = 2 + rng.below(3);
            let word: String = (0..syllables)
                .map(|_| {
                    format!(
                        "{}{}{}",
                        onsets[rng.below(onsets.len())],
                        vowels[rng.below(vowels.len())],
                        codas[rng.below(codas.len())]
                    )
                })
                .collect();
            if seen.insert(word.clone()) {
                words.push(word);
            }
        }
        words
    })
}

/// The stand-in word at `rank`.
fn word_at(rank: usize) -> String {
    words()[rank].clone()
}

/// The rank of a stand-in word.
fn rank_of(word: &str) -> usize {
    use std::sync::OnceLock;
    static RANKS: OnceLock<std::collections::HashMap<String, usize>> = OnceLock::new();
    RANKS.get_or_init(|| {
        ranked()
            .into_iter()
            .enumerate()
            .map(|(rank, w)| (w, rank))
            .collect()
    })[word]
}

/// A simulated reader: knows the word at rank r with a chance that falls
/// from near 1 to near 0 around `known` (as real readers do, gradually),
/// slips on 3 % of known words, and says "yes" to `over_claim` of the words
/// (and made-up words) they do not know.
struct Learner {
    known: f64,
    over_claim: f64,
    rng: Rng,
    knows: std::collections::HashMap<usize, bool>,
}

impl Learner {
    fn new(known: usize, over_claim: f64, seed: u64) -> Self {
        Self {
            known: known as f64,
            over_claim,
            rng: Rng::new(seed.wrapping_mul(7919)),
            knows: std::collections::HashMap::new(),
        }
    }

    fn knows(&mut self, rank: usize) -> bool {
        let chance = 1.0 / (1.0 + ((rank as f64 - self.known) / (0.2 * self.known)).exp());
        let roll = self.rng.chance(chance);
        *self.knows.entry(rank).or_insert(roll)
    }

    fn answer(&mut self, word: &CheckWord) -> bool {
        match word.rank {
            Some(rank) if self.knows(rank) => !self.rng.chance(0.03),
            _ => self.rng.chance(self.over_claim),
        }
    }

    /// Takes the whole check and returns it finished.
    fn take(&mut self, mut check: VocabCheck) -> (VocabCheck, usize) {
        let mut asked = 0;
        loop {
            let words = check.next_words();
            if words.is_empty() {
                return (check, asked);
            }
            for word in &words {
                asked += 1;
                let yes = self.answer(word);
                check.answer(word, yes);
            }
        }
    }
}

/// The share of marked words the learner really knows.
fn true_accuracy(check: &VocabCheck, choice: &MarkChoice, learner: &mut Learner) -> f64 {
    let words = check.words_to_mark(choice);
    if words.is_empty() {
        return 1.0;
    }
    let known = words.iter().filter(|w| learner.knows(rank_of(w))).count();
    known as f64 / words.len() as f64
}

#[test]
fn stand_in_words_are_unique_and_read_back() {
    let words = ranked();
    let unique: std::collections::HashSet<&String> = words.iter().collect();
    assert_eq!(unique.len(), words.len());
    for rank in [0, 1, 59, 60, 12_345, 29_999] {
        assert_eq!(rank_of(&word_at(rank)), rank);
    }
}

#[test]
fn simulated_readers_get_at_least_97_percent_right_with_the_safe_choice() {
    for known in [300, 1_000, 3_000, 8_000] {
        for over_claim in [0.0, 0.1] {
            let (mut accuracy, mut predicted, mut marked, mut asked) = (0.0, 0.0, 0.0, 0.0);
            let mut untrusted = 0;
            let runs: u32 = 40;
            for seed in 0..u64::from(runs) {
                let mut learner = Learner::new(known, over_claim, seed);
                let (check, questions) = learner.take(VocabCheck::new(ranked(), &|_| false, seed));
                let result = check.result();
                if !result.reliable {
                    untrusted += 1;
                    continue;
                }
                let safe = result.choices[0];
                accuracy += true_accuracy(&check, &safe, &mut learner);
                predicted += safe.expected_accuracy;
                marked += safe.words as f64;
                asked += questions as f64;
            }
            // A careful reader is nearly always trusted.
            assert!(
                untrusted <= runs / 10,
                "knows {known}: {untrusted} of {runs} runs not trusted"
            );
            let trusted = f64::from(runs - untrusted);
            let (accuracy, predicted, marked, asked) = (
                accuracy / trusted,
                predicted / trusted,
                marked / trusted,
                asked / trusted,
            );
            assert!(
                accuracy >= 0.97,
                "knows {known}, over-claims {over_claim}: {accuracy:.3} right"
            );
            // The accuracy shown to the reader is close to the real one.
            assert!(
                (predicted - accuracy).abs() < 0.03,
                "predicted {predicted:.3}, really {accuracy:.3}"
            );
            // It still marks a useful share of what they know, in a short test.
            assert!(
                marked >= known as f64 * 0.15,
                "knows {known}: marks only {marked:.0}"
            );
            assert!(asked <= 100.0, "{asked:.0} questions");
        }
    }
}

#[test]
fn the_larger_choice_marks_more_and_stays_mostly_right() {
    for known in [1_000, 3_000, 8_000] {
        let runs: usize = 30;
        let (mut accuracy, mut more_than_safe) = (0.0, 0);
        for seed in 0..runs as u64 {
            let mut learner = Learner::new(known, 0.0, seed);
            let (check, _) = learner.take(VocabCheck::new(ranked(), &|_| false, seed));
            let result = check.result();
            let more = *result.choices.last().unwrap();
            if result.choices.len() > 1 {
                more_than_safe += usize::from(more.words > result.choices[0].words);
            }
            accuracy += true_accuracy(&check, &more, &mut learner);
        }
        let accuracy = accuracy / runs as f64;
        assert!(accuracy >= 0.93, "knows {known}: {accuracy:.3}");
        assert!(
            more_than_safe > runs / 2,
            "knows {known}: the larger choice rarely adds words"
        );
    }
}

#[test]
fn an_over_claimer_is_caught_and_offered_nothing() {
    for seed in 0..5 {
        let mut learner = Learner::new(1_000, 0.6, seed);
        let (check, _) = learner.take(VocabCheck::new(ranked(), &|_| false, seed));
        let result = check.result();
        assert!(!result.reliable, "seed {seed}");
        assert!(result.choices.is_empty());
    }
}

#[test]
fn words_answered_no_are_never_marked() {
    let mut learner = Learner::new(3_000, 0.0, 5);
    let (check, _) = learner.take(VocabCheck::new(ranked(), &|_| false, 5));
    let result = check.result();
    for choice in &result.choices {
        let marked: std::collections::HashSet<String> =
            check.words_to_mark(choice).into_iter().collect();
        for (rank, yes) in &check.answers {
            let is_marked = marked.contains(&word_at(*rank));
            if !*yes {
                assert!(!is_marked, "rank {rank} answered no");
            } else if *rank < choice.up_to_rank {
                assert!(is_marked, "rank {rank} answered yes within the cut-off");
            }
        }
    }
}

#[test]
fn a_beginner_stops_early_and_an_advanced_reader_goes_through_every_band() {
    let mut beginner = Learner::new(150, 0.0, 2);
    let (_, asked) = beginner.take(VocabCheck::new(ranked(), &|_| false, 2));
    assert!(asked <= 45, "{asked}");
    let mut advanced = Learner::new(30_000, 0.0, 2);
    let (check, asked) = advanced.take(VocabCheck::new(ranked(), &|_| false, 2));
    assert!(asked > 70, "{asked}");
    assert!(check.result().estimated_known > 15_000);
}

#[test]
fn made_up_words_look_real_but_are_not_words() {
    let words = ranked();
    let real: std::collections::HashSet<String> = words.iter().cloned().collect();
    // Some other words the dictionary knows.
    let in_dictionary = |w: &str| w.ends_with("ra");
    let mut check = VocabCheck::new(words.clone(), &in_dictionary, 3);
    assert_eq!(check.made_up.len(), MADE_UP);
    let spelling = Spelling::learn(&words[..COMMON_WORDS]);
    for word in check.made_up.clone() {
        assert!(!real.contains(&word) && !in_dictionary(&word), "{word}");
        assert!(
            spelling.looks_real(&word) && !spelling.one_letter_from_a_word(&word),
            "{word}"
        );
        assert!(
            word.chars().all(char::is_alphabetic) && (5..=11).contains(&word.chars().count()),
            "{word}"
        );
    }
    // They are mixed into the first round and reported apart from real words.
    let first = check.next_words();
    assert_eq!(first.iter().filter(|w| w.rank.is_none()).count(), 2);
    for word in &first {
        check.answer(word, word.rank.is_some());
    }
    assert_eq!(check.false_yes(), 0.0);
}

#[test]
fn the_same_seed_asks_the_same_words() {
    let first = VocabCheck::new(ranked(), &|_| false, 9).next_words();
    let again = VocabCheck::new(ranked(), &|_| false, 9).next_words();
    let other = VocabCheck::new(ranked(), &|_| false, 10).next_words();
    assert_eq!(first, again);
    assert_ne!(first, other);
}

#[test]
fn a_falling_curve_pools_values_out_of_order() {
    let close = |a: &[f64], b: &[f64]| {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
    };
    assert!(close(
        &falling(&[1.0, 0.8, 0.9, 0.2], &[1.0; 4]),
        &[1.0, 0.85, 0.85, 0.2]
    ));
    assert!(close(&falling(&[0.5, 0.7], &[1.0, 3.0]), &[0.65, 0.65]));
    assert!(falling(&[], &[]).is_empty());
}

#[test]
fn single_letters_and_elided_bits_are_not_asked() {
    assert!(askable("table") && askable("homme-grenouille") && askable("oh"));
    assert!(!askable("l") && !askable("l'un") && !askable("d"));
    let mut words = vec!["l".to_string(), "d".to_string(), "l'un".to_string()];
    words.extend(ranked());
    let mut check = VocabCheck::new(words, &|_| false, 4);
    let mut asked = Vec::new();
    loop {
        let batch = check.next_words();
        if batch.is_empty() {
            break;
        }
        for word in &batch {
            asked.push(word.word.clone());
            check.answer(word, true);
        }
    }
    assert!(!asked.iter().any(|w| w == "l" || w == "d" || w == "l'un"));
}

#[test]
fn marking_includes_every_form_of_each_marked_word() {
    let words: Vec<String> = ["être", "avoir", "lire"]
        .iter()
        .map(|w| (*w).to_string())
        .chain(ranked())
        .collect();
    let mut forms: Vec<Vec<String>> = vec![Vec::new(); words.len()];
    forms[0] = vec!["est".into(), "était".into(), "être".into(), "été".into()];
    forms[2] = vec!["lire".into(), "lisait".into()];
    // "été" is also its own word further down: listed once.
    forms[3] = vec!["été".into()];
    let check = VocabCheck::new(words, &|_| false, 6);
    let choice = MarkChoice {
        up_to_rank: 4,
        words: 0,
        expected_accuracy: 1.0,
    };
    assert_eq!(
        check.forms_to_mark(&choice, &forms),
        ["est", "était", "être", "été", "avoir", "lire", "lisait"]
    );
}

#[test]
fn made_up_words_use_only_common_letter_sequences_and_are_not_near_a_word() {
    let words = ranked();
    let spelling = Spelling::learn(&words[..COMMON_WORDS]);
    // Real words use common sequences; letters no word puts together do not.
    // Nearly all real words: any word may hold one rare sequence.
    let real = words[..1_000]
        .iter()
        .filter(|w| spelling.looks_real(w))
        .count();
    assert!(real >= 900, "{real} of 1,000");
    assert!(!spelling.looks_real("bxqzé") && !spelling.looks_real("aaaaa"));
    // A word with one letter left out, changed or added is too close.
    let word: Vec<char> = words[7].chars().collect();
    let without: String = word[1..].iter().collect();
    let changed: String = std::iter::once('z')
        .chain(word[1..].iter().copied())
        .collect();
    let added: String = std::iter::once('z').chain(word.iter().copied()).collect();
    for near in [without, changed, added] {
        assert!(spelling.one_letter_from_a_word(&near), "{near}");
    }
    assert!(!spelling.one_letter_from_a_word("zzzzzzzz"));
}
