use super::*;

/// The stand-in dictionary form at `rank`: its rank in letters ("bab",
/// "bac", …), so made-up words can be spliced from them as from real ones.
fn word_at(rank: usize) -> String {
    let consonants = ['b', 'd', 'f', 'g', 'l', 'm', 'n', 'p', 'r', 's', 't', 'v'];
    let vowels = ['a', 'e', 'i', 'o', 'u'];
    let mut word = String::new();
    let mut n = rank + 60; // at least two syllables
    while n > 0 {
        word.push(consonants[n % 12]);
        n /= 12;
        word.push(vowels[n % 5]);
        n /= 5;
    }
    // A check letter, so a word spliced from two others is rarely another
    // stand-in, as in a real language.
    word.push(['x', 'y', 'z', 'k', 'h', 'w', 'j'][rank % 7]);
    word
}

/// The rank of a stand-in word.
fn rank_of(word: &str) -> usize {
    let consonants = "bdfglmnprstv";
    let vowels = "aeiou";
    let chars: Vec<char> = word.chars().collect();
    let chars = &chars[..chars.len() - 1];
    let mut n = 0;
    for pair in chars.chunks(2).rev() {
        n = n * 5 + vowels.find(pair[1]).unwrap();
        n = n * 12 + consonants.find(pair[0]).unwrap();
    }
    n - 60
}

/// 30,000 stand-in dictionary forms, most common first.
fn ranked() -> Vec<String> {
    (0..30_000).map(word_at).collect()
}

/// Real-looking words for the made-up word tests: letters only.
fn lettered() -> Vec<String> {
    let syllables = [
        "ma", "ri", "lo", "te", "pu", "san", "vor", "del", "quin", "bre",
    ];
    let mut words = Vec::new();
    for a in syllables {
        for b in syllables {
            for c in syllables {
                words.push(format!("{a}{b}{c}"));
            }
        }
    }
    words
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
fn an_over_claimer_is_caught_and_only_their_yes_words_are_offered() {
    let mut learner = Learner::new(1_000, 0.6, 1);
    let (check, _) = learner.take(VocabCheck::new(ranked(), &|_| false, 1));
    let result = check.result();
    assert!(!result.reliable);
    assert_eq!(result.choices.len(), 1);
    let choice = result.choices[0];
    assert_eq!(choice.up_to_rank, 0);
    // Only "yes" answers, and with this much guessing hardly any of them.
    let yes: std::collections::HashSet<String> = check
        .answers
        .iter()
        .filter(|(_, yes)| *yes)
        .map(|(rank, _)| word_at(*rank))
        .collect();
    let marked = check.words_to_mark(&choice);
    assert!(marked.iter().all(|w| yes.contains(w)));
    assert!(
        marked.len() * 4 < yes.len(),
        "{} of {} yes-words marked",
        marked.len(),
        yes.len()
    );
    assert_eq!(choice.words, marked.len());
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
    let words = lettered();
    let real: std::collections::HashSet<String> = words.iter().cloned().collect();
    let in_dictionary = |w: &str| w.ends_with("ma");
    let mut check = VocabCheck::new(words, &in_dictionary, 3);
    assert_eq!(check.made_up.len(), MADE_UP);
    for word in check.made_up.clone() {
        assert!(!real.contains(&word) && !in_dictionary(&word), "{word}");
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
