use crate::srs::{LEARNED, Progress};
use std::{fs, io, path::PathBuf};

/// One card: `text` is a noun, or a sentence with `___` where `answer` goes.
pub struct Item {
    pub answer: &'static str,
    pub text: &'static str,
    pub note: String,
}

pub struct Deck {
    pub items: Vec<Item>,
    pub progress: Progress,
    path: PathBuf,
}

pub struct Stats {
    pub seen: usize,
    pub learned: usize,
    pub total: usize,
}

pub struct Row<'a> {
    pub item: &'a Item,
    pub streak: u32,
    pub correct: u32,
    pub wrong: u32,
    pub due_in: i64,
}

impl Deck {
    pub fn open(items: Vec<Item>, path: PathBuf) -> Deck {
        let mut progress = fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Progress>(&b).ok())
            .unwrap_or_default();
        // ponytail: progress is keyed by line position; key by text if a list ever changes after release.
        progress.cards.resize(items.len(), Default::default());
        Deck { items, progress, path }
    }

    pub fn save(&self) -> io::Result<()> {
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec(&self.progress)?)?;
        fs::rename(&tmp, &self.path)
    }

    pub fn stats(&self) -> Stats {
        let c = &self.progress.cards;
        Stats {
            seen: c.iter().filter(|c| c.last_seen.is_some()).count(),
            learned: c.iter().filter(|c| c.streak >= LEARNED).count(),
            total: self.items.len(),
        }
    }

    /// Seen cards, soonest due first.
    pub fn rows(&self) -> Vec<Row<'_>> {
        let p = &self.progress;
        let mut rows: Vec<Row> = (0..p.cards.len())
            .filter_map(|i| {
                let gap = p.gap(i)?;
                let c = &p.cards[i];
                Some(Row {
                    item: &self.items[i],
                    streak: c.streak,
                    correct: c.correct,
                    wrong: c.wrong,
                    due_in: c.interval() as i64 - gap as i64,
                })
            })
            .collect();
        rows.sort_by_key(|r| r.due_in);
        rows
    }
}

pub fn words() -> Vec<Item> {
    include_str!("../data/words.tsv")
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(article, noun)| Item { answer: article, text: noun, note: String::new() })
        .collect()
}

pub fn sentences() -> Vec<Item> {
    include_str!("../data/sentences.tsv")
        .lines()
        .map(|l| {
            let mut f = l.split('\t');
            let (text, case, gender) = (f.next().unwrap(), f.next().unwrap_or(""), f.next().unwrap_or(""));
            let answer = match (case, gender) {
                ("akk", "m") => "den",
                ("akk", "f" | "pl") => "die",
                ("akk", "n") => "das",
                ("dat", "m" | "n") => "dem",
                ("dat", "f") => "der",
                ("dat", "pl") => "den",
                _ => panic!("bad sentence line: {l}"),
            };
            let g = match gender {
                "m" => "maskulin",
                "f" => "feminin",
                "n" => "neutral",
                _ => "Plural",
            };
            let c = if case == "akk" { "Akkusativ" } else { "Dativ" };
            Item { answer, text, note: format!("{c} · {g}") }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentences_parse_with_one_gap() {
        let s = sentences();
        assert_eq!(s.len(), 200);
        for i in &s {
            assert_eq!(i.text.matches("___").count(), 1, "{}", i.text);
        }
    }
}
