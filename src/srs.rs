use serde::{Deserialize, Serialize};

const FAIL_GAP: u64 = 10;
const OK_GAP: u64 = 20;
const MIN_GAP: u64 = 3;
pub const LEARNED: u32 = 3;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Card {
    pub streak: u32,
    pub correct: u32,
    pub wrong: u32,
    pub last_seen: Option<u64>,
    pub failed: bool,
}

impl Card {
    /// Cards to wait before this one is due: 10 after a miss, 20 × streak otherwise.
    pub fn interval(&self) -> u64 {
        if self.failed {
            FAIL_GAP
        } else {
            OK_GAP * u64::from(self.streak.max(1))
        }
    }
}

/// Progress over the word list; `cards[i]` belongs to word `i`, `now` counts answered cards.
#[derive(Default, Serialize, Deserialize)]
pub struct Progress {
    pub now: u64,
    pub cards: Vec<Card>,
}

impl Progress {
    pub fn new(words: usize) -> Self {
        Progress { now: 0, cards: vec![Card::default(); words] }
    }

    pub fn gap(&self, i: usize) -> Option<u64> {
        self.cards[i].last_seen.map(|l| self.now - l)
    }

    /// Picks the next card; `r` is a uniform random number in [0, 1).
    pub fn pick(&self, r: f64) -> usize {
        let seen: Vec<(usize, u64, f64)> = (0..self.cards.len())
            .filter_map(|i| self.gap(i).map(|g| (i, g, g as f64 / self.cards[i].interval() as f64)))
            .collect();
        let due: Vec<_> = seen.iter().filter(|s| s.2 >= 1.0).map(|s| (s.0, s.2)).collect();
        if !due.is_empty() {
            return weighted(&due, r);
        }
        if let Some(i) = self.cards.iter().position(|c| c.last_seen.is_none()) {
            return i;
        }
        let rested: Vec<_> = seen.iter().filter(|s| s.1 >= MIN_GAP).map(|s| (s.0, s.2)).collect();
        if rested.is_empty() {
            weighted(&seen.iter().map(|s| (s.0, s.2)).collect::<Vec<_>>(), r)
        } else {
            weighted(&rested, r)
        }
    }

    pub fn answer(&mut self, i: usize, correct: bool) {
        self.now += 1;
        let c = &mut self.cards[i];
        c.last_seen = Some(self.now);
        c.failed = !correct;
        if correct {
            c.streak += 1;
            c.correct += 1;
        } else {
            c.streak = 0;
            c.wrong += 1;
        }
    }
}

fn weighted(items: &[(usize, f64)], r: f64) -> usize {
    let mut left = r * items.iter().map(|x| x.1).sum::<f64>();
    for &(i, w) in items {
        left -= w;
        if left <= 0.0 {
            return i;
        }
    }
    items[items.len() - 1].0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn introduces_new_words_in_order() {
        let mut p = Progress::new(100);
        for expected in 0..15 {
            let i = p.pick(0.5);
            assert_eq!(i, expected);
            p.answer(i, true);
        }
    }

    #[test]
    fn failed_card_returns_after_about_ten() {
        let mut p = Progress::new(100);
        p.answer(p.pick(0.5), false);
        for n in 1.. {
            let i = p.pick(0.5);
            if i == 0 {
                assert!((10..=12).contains(&n), "returned after {n}");
                break;
            }
            p.answer(i, true);
        }
    }

    #[test]
    fn cards_never_show_before_due_while_new_words_remain() {
        let mut rng = fastrand::Rng::with_seed(7);
        let mut p = Progress::new(2000);
        for _ in 0..3000 {
            let i = p.pick(rng.f64());
            if let Some(g) = p.gap(i) {
                assert!(g >= p.cards[i].interval(), "card {i} shown after {g}");
            }
            p.answer(i, rng.f64() < 0.7);
        }
    }

    #[test]
    fn no_immediate_repeat_when_all_seen() {
        let mut rng = fastrand::Rng::with_seed(1);
        let mut p = Progress::new(6);
        for _ in 0..500 {
            let i = p.pick(rng.f64());
            if let Some(g) = p.gap(i) {
                assert!(g >= MIN_GAP, "card {i} repeated after {g}");
            }
            p.answer(i, true);
        }
    }
}
