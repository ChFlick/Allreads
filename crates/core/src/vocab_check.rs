//! The vocabulary check: a short yes/no test that finds the common words a
//! reader already knows, so a first book does not show half its words as new
//! (plan: "Vocabulary check" in `IMPLEMENTATION_PLAN.md`).
//!
//! The test asks about dictionary forms from frequency bands, plus made-up
//! words that catch over-claiming. It then fits how likely the reader knows a
//! word at each frequency rank and offers to mark the words up to where that
//! stays high, with the accuracy it expects. It aims at few wrong marks
//! rather than many marks: words answered "no" are never marked.
//!
//! The check only needs dictionary forms ordered by frequency; where they
//! come from (Lexique, `FrequencyWords`, …) is the dictionary builder's
//! business.

/// Frequency bands by rank (0 is the most common word), from the very common
/// to the rare. Narrow at the top, where a reader's knowledge changes fastest.
pub const BANDS: [(usize, usize); 11] = [
    (0, 250),
    (250, 500),
    (500, 1_000),
    (1_000, 2_000),
    (2_000, 3_000),
    (3_000, 4_000),
    (4_000, 6_000),
    (6_000, 8_000),
    (8_000, 12_000),
    (12_000, 16_000),
    (16_000, 24_000),
];

/// Words asked per band in the first round.
const PER_BAND: usize = 6;
/// Words asked in the second round, around where the reader's knowledge fades.
const FOLLOW_UP: usize = 12;
/// Made-up words mixed into the first round.
const MADE_UP: usize = 10;
/// The first round stops after this many bands in a row with fewer than half "yes".
const LOW_BANDS_TO_STOP: usize = 2;
/// With more "yes" than this on made-up words, the answers are not trusted.
const MAX_FALSE_YES: f64 = 0.3;
/// The safe choice marks words up to where the chance of knowing them stays above this.
const SAFE_LEVEL: f64 = 0.95;
/// The larger choice goes down to this chance.
const MORE_LEVEL: f64 = 0.85;
/// A "yes" beyond the cut-off is only marked when it is at least this likely to be real.
const YES_TRUSTED: f64 = 0.9;
/// How often a reader is taken to say "no" to a word they know (a slip).
const SLIP: f64 = 0.03;

/// A word to ask about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckWord {
    pub word: String,
    /// Its frequency rank, or `None` for a made-up word.
    pub rank: Option<usize>,
}

/// One way to apply the result: mark the words up to a rank.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkChoice {
    /// Words more common than this rank are marked known (except those
    /// answered "no"), plus the words answered "yes" beyond it.
    pub up_to_rank: usize,
    /// How many dictionary forms that marks.
    pub words: usize,
    /// The share of them the reader is expected to know, from their answers (0 to 1).
    pub expected_accuracy: f64,
}

/// What the test found.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    /// False when the reader said "yes" to too many made-up words; then only
    /// the words they said "yes" to are offered.
    pub reliable: bool,
    /// The share of made-up words answered "yes" (0 to 1).
    pub false_yes: f64,
    /// Roughly how many of the listed words the reader knows, for display.
    pub estimated_known: usize,
    /// The safe choice first, then a larger one when it marks more.
    pub choices: Vec<MarkChoice>,
}

/// A vocabulary check in progress. Ask the words of [`VocabCheck::next_words`]
/// in order, report each with [`VocabCheck::answer`], and call `next_words`
/// again until it returns nothing; then read [`VocabCheck::result`].
pub struct VocabCheck {
    /// Dictionary forms, most common first.
    ranked: Vec<String>,
    rng: Rng,
    /// Made-up words still to mix into the first round.
    made_up: Vec<String>,
    /// The next band of the first round, or `BANDS.len()` once it is over.
    next_band: usize,
    followed_up: bool,
    /// Answers to real words: rank and "yes".
    answers: Vec<(usize, bool)>,
    /// Answers to made-up words.
    made_up_answers: Vec<bool>,
}

impl VocabCheck {
    /// Starts a check over `ranked`, dictionary forms most common first.
    /// `is_word` says whether a string is a real word (from the dictionary),
    /// so made-up words never turn out to exist. `seed` picks the words; a
    /// new check should get a new seed.
    pub fn new(ranked: Vec<String>, is_word: &dyn Fn(&str) -> bool, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let made_up = make_up_words(&ranked, is_word, &mut rng, MADE_UP);
        Self {
            ranked,
            rng,
            made_up,
            next_band: 0,
            followed_up: false,
            answers: Vec::new(),
            made_up_answers: Vec::new(),
        }
    }

    /// The next words to ask, in order: one band of the first round (with a
    /// made-up word or two mixed in), then the follow-up round. Empty when the
    /// check is over.
    pub fn next_words(&mut self) -> Vec<CheckWord> {
        while self.next_band < BANDS.len() && !self.first_round_over() {
            let band = self.next_band;
            self.next_band += 1;
            let mut words = self.sample_band(band, PER_BAND);
            if words.is_empty() {
                continue;
            }
            // Spread the made-up words over the first bands, one or two each.
            let take = self.made_up.len().min(if band < 5 { 2 } else { 1 });
            for word in self.made_up.drain(..take) {
                let at = self.rng.below(words.len() + 1);
                words.insert(at, CheckWord { word, rank: None });
            }
            return words;
        }
        self.next_band = BANDS.len();
        if !self.followed_up {
            self.followed_up = true;
            return self.follow_up_words();
        }
        Vec::new()
    }

    /// Records the answer to a word from [`VocabCheck::next_words`].
    pub fn answer(&mut self, word: &CheckWord, yes: bool) {
        match word.rank {
            Some(rank) => self.answers.push((rank, yes)),
            None => self.made_up_answers.push(yes),
        }
    }

    /// What the answers so far say, with the choices of words to mark.
    #[must_use]
    pub fn result(&self) -> CheckResult {
        let false_yes = self.false_yes();
        let reliable = false_yes <= MAX_FALSE_YES;
        let curve = self.curve(false_yes);
        let estimated_known = BANDS
            .iter()
            .zip(&curve)
            .map(|(&(start, end), &p)| {
                (end.min(self.ranked.len()).saturating_sub(start)) as f64 * p.unwrap_or(0.0)
            })
            .sum::<f64>()
            .round() as usize;
        let mut choices = Vec::new();
        if reliable {
            for level in [SAFE_LEVEL, MORE_LEVEL] {
                let choice = self.choice(&curve, level);
                if choices
                    .last()
                    .is_none_or(|last: &MarkChoice| choice.words > last.words)
                {
                    choices.push(choice);
                }
            }
        } else {
            // No cut-off: only "yes" answers likely to be real (few, with this many guesses).
            choices.push(self.choice(&curve, f64::INFINITY));
        }
        CheckResult {
            reliable,
            false_yes,
            estimated_known,
            choices,
        }
    }

    /// The dictionary forms a choice marks known: those more common than its
    /// rank, except the ones answered "no", plus those answered "yes" beyond
    /// it when such a "yes" is likely real (not a guess, in a band the reader
    /// mostly does not know).
    #[must_use]
    pub fn words_to_mark(&self, choice: &MarkChoice) -> Vec<String> {
        let curve = self.curve(self.false_yes());
        let mut marked: Vec<bool> = (0..self.ranked.len())
            .map(|rank| rank < choice.up_to_rank)
            .collect();
        for &(rank, yes) in &self.answers {
            if let Some(slot) = marked.get_mut(rank) {
                *slot = yes && (*slot || self.yes_is_real(&curve, rank) >= YES_TRUSTED);
            }
        }
        self.ranked
            .iter()
            .zip(marked)
            .filter(|(_, mark)| *mark)
            .map(|(word, _)| word.clone())
            .collect()
    }

    /// The first round ends after enough bands in a row with fewer than half "yes".
    fn first_round_over(&self) -> bool {
        let mut low_in_a_row = 0;
        for band in 0..self.next_band {
            match self.band_share(band) {
                Some(share) if share < 0.5 => low_in_a_row += 1,
                Some(_) => low_in_a_row = 0,
                None => {}
            }
        }
        low_in_a_row >= LOW_BANDS_TO_STOP
    }

    /// More words in the band where the fitted chance falls below the safe
    /// level, and the band before it: that is where the cut-off is decided.
    fn follow_up_words(&mut self) -> Vec<CheckWord> {
        let curve = self.curve(self.false_yes());
        let crossing = curve
            .iter()
            .position(|p| p.is_some_and(|p| p < SAFE_LEVEL))
            .unwrap_or_else(|| curve.iter().rposition(Option::is_some).unwrap_or(0));
        let bands = if crossing == 0 {
            vec![0]
        } else {
            vec![crossing - 1, crossing]
        };
        let per_band = FOLLOW_UP / bands.len();
        let mut words: Vec<CheckWord> = bands
            .into_iter()
            .flat_map(|band| self.sample_band(band, per_band))
            .collect();
        // Made-up words not asked yet (the first round stopped early) come
        // now, so every reader answers all of them.
        for word in std::mem::take(&mut self.made_up) {
            let at = self.rng.below(words.len() + 1);
            words.insert(at, CheckWord { word, rank: None });
        }
        words
    }

    /// `count` words of a band not asked yet, in random order.
    fn sample_band(&mut self, band: usize, count: usize) -> Vec<CheckWord> {
        let (start, end) = BANDS[band];
        let end = end.min(self.ranked.len());
        let asked: Vec<usize> = self.answers.iter().map(|(rank, _)| *rank).collect();
        let mut pool: Vec<usize> = (start..end).filter(|rank| !asked.contains(rank)).collect();
        let mut words = Vec::new();
        while words.len() < count && !pool.is_empty() {
            let rank = pool.swap_remove(self.rng.below(pool.len()));
            words.push(CheckWord {
                word: self.ranked[rank].clone(),
                rank: Some(rank),
            });
        }
        words
    }

    fn false_yes(&self) -> f64 {
        if self.made_up_answers.is_empty() {
            return 0.0;
        }
        self.made_up_answers.iter().filter(|yes| **yes).count() as f64
            / self.made_up_answers.len() as f64
    }

    /// The share of "yes" in a band, or `None` if none of its words were asked.
    fn band_share(&self, band: usize) -> Option<f64> {
        let (yes, asked) = self.band_counts(band);
        (asked > 0).then(|| yes as f64 / asked as f64)
    }

    fn band_counts(&self, band: usize) -> (usize, usize) {
        let (start, end) = BANDS[band];
        let in_band = self
            .answers
            .iter()
            .filter(|(rank, _)| (start..end).contains(rank));
        let (mut yes, mut asked) = (0, 0);
        for (_, answer) in in_band {
            asked += 1;
            yes += usize::from(*answer);
        }
        (yes, asked)
    }

    /// The chance of knowing a word in each band, corrected for false "yes"
    /// and made to fall with rarity (a band cannot be better known than a
    /// more common one). `None` for bands not asked.
    fn curve(&self, false_yes: f64) -> Vec<Option<f64>> {
        let mut shares = Vec::new();
        let mut weights = Vec::new();
        let mut asked_bands = Vec::new();
        for band in 0..BANDS.len() {
            let (yes, asked) = self.band_counts(band);
            if asked == 0 {
                continue;
            }
            let hit = yes as f64 / asked as f64;
            // The usual correction for guessing: what is left of the hits once
            // the false-yes rate is taken out.
            let corrected = if false_yes < 1.0 {
                ((hit - false_yes) / (1.0 - false_yes)).max(0.0)
            } else {
                0.0
            };
            shares.push(corrected);
            weights.push(asked as f64);
            asked_bands.push(band);
        }
        let fitted = falling(&shares, &weights);
        let mut curve = vec![None; BANDS.len()];
        for (band, p) in asked_bands.into_iter().zip(fitted) {
            curve[band] = Some(p);
        }
        curve
    }

    /// The chance of knowing the word at `rank`, read off the curve: straight
    /// lines between the bands' middles on a log scale, starting from certain
    /// at the very top. Ranks past the last asked band count as unknown.
    fn chance_at(curve: &[Option<f64>], rank: usize) -> f64 {
        let x = ((rank + 1) as f64).ln();
        let mut previous = (0.0_f64, 1.0_f64);
        for (&(start, end), p) in BANDS.iter().zip(curve) {
            let Some(p) = *p else { continue };
            let middle = (((start + end) / 2) as f64).ln();
            if x <= middle {
                let (x0, p0) = previous;
                return if middle > x0 {
                    p0 + (p - p0) * (x - x0) / (middle - x0)
                } else {
                    p
                };
            }
            previous = (middle, p);
        }
        0.0
    }

    /// The chance that a "yes" at `rank` means the reader knows the word,
    /// rather than a guess: from how likely they know words there (the curve)
    /// and how often they guess (made-up words, never taken as zero).
    fn yes_is_real(&self, curve: &[Option<f64>], rank: usize) -> f64 {
        let known = Self::chance_at(curve, rank);
        let made_up_yes = self.made_up_answers.iter().filter(|yes| **yes).count() as f64;
        // A little caution for the few made-up words asked: 0 of 10 is not "never guesses".
        let guess = (made_up_yes + 0.5) / (self.made_up_answers.len() as f64 + 1.0);
        let real = known * (1.0 - SLIP);
        let guessed = (1.0 - known) * guess;
        if real + guessed > 0.0 {
            real / (real + guessed)
        } else {
            0.0
        }
    }

    /// Marks words up to where the chance of knowing them falls below `level`.
    fn choice(&self, curve: &[Option<f64>], level: f64) -> MarkChoice {
        let mut up_to_rank = 0;
        while up_to_rank < self.ranked.len() && Self::chance_at(curve, up_to_rank) >= level {
            up_to_rank += 1;
        }
        let choice = MarkChoice {
            up_to_rank,
            words: 0,
            expected_accuracy: 1.0,
        };
        let words = self.words_to_mark(&choice);
        let ranks: std::collections::HashMap<&str, usize> = self
            .ranked
            .iter()
            .enumerate()
            .map(|(rank, word)| (word.as_str(), rank))
            .collect();
        let answered: std::collections::HashSet<usize> =
            self.answers.iter().map(|(rank, _)| *rank).collect();
        // Words answered "yes" count as likely as such a "yes" is real; the others as the curve says.
        let expected: f64 = words
            .iter()
            .filter_map(|word| ranks.get(word.as_str()))
            .map(|&rank| {
                if answered.contains(&rank) {
                    self.yes_is_real(curve, rank)
                } else {
                    Self::chance_at(curve, rank)
                }
            })
            .sum();
        MarkChoice {
            up_to_rank,
            words: words.len(),
            expected_accuracy: if words.is_empty() {
                1.0
            } else {
                expected / words.len() as f64
            },
        }
    }
}

/// The closest falling sequence to `values` (weighted): each value no higher
/// than the one before. Neighbours out of order are pooled into their average.
fn falling(values: &[f64], weights: &[f64]) -> Vec<f64> {
    // Each block: (average, total weight, how many values it covers).
    let mut blocks: Vec<(f64, f64, usize)> = Vec::new();
    for (&value, &weight) in values.iter().zip(weights) {
        blocks.push((value, weight, 1));
        while blocks.len() >= 2 && blocks[blocks.len() - 2].0 < blocks[blocks.len() - 1].0 {
            let (v2, w2, n2) = blocks.pop().unwrap_or_default();
            let (v1, w1, n1) = blocks.pop().unwrap_or_default();
            let weight = w1 + w2;
            let average = if weight > 0.0 {
                (v1 * w1 + v2 * w2) / weight
            } else {
                0.0
            };
            blocks.push((average, weight, n1 + n2));
        }
    }
    blocks
        .into_iter()
        .flat_map(|(value, _, count)| std::iter::repeat_n(value, count))
        .collect()
}

/// Made-up words that look like the language: the start of one real word
/// joined to the end of another, checked against the word list and the
/// dictionary so none of them exists.
fn make_up_words(
    ranked: &[String],
    is_word: &dyn Fn(&str) -> bool,
    rng: &mut Rng,
    count: usize,
) -> Vec<String> {
    let known: std::collections::HashSet<&str> = ranked.iter().map(String::as_str).collect();
    // Middle-frequency words of a reasonable length make the likeliest parts.
    let parts: Vec<&str> = ranked
        .iter()
        .take(8_000)
        .skip(500)
        .map(String::as_str)
        .filter(|w| (5..=10).contains(&w.chars().count()) && w.chars().all(char::is_alphabetic))
        .collect();
    let mut made_up = Vec::new();
    let mut tries = 0;
    while made_up.len() < count && tries < 10_000 && parts.len() >= 2 {
        tries += 1;
        let first: Vec<char> = parts[rng.below(parts.len())].chars().collect();
        let second: Vec<char> = parts[rng.below(parts.len())].chars().collect();
        let word: String = first[..first.len() / 2]
            .iter()
            .chain(&second[second.len() / 2..])
            .collect();
        let length = word.chars().count();
        if (5..=11).contains(&length)
            && !known.contains(word.as_str())
            && !is_word(&word)
            && !made_up.contains(&word)
        {
            made_up.push(word);
        }
    }
    made_up
}

/// A small random number generator (xorshift64*), so the check needs no
/// extra crate and a seed always gives the same words in tests.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Zero would stay zero forever.
        Self(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number from 0 to `n - 1` (`n` must not be 0).
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// True with chance `p`.
    #[cfg(test)]
    fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }
}

#[cfg(test)]
mod tests;
