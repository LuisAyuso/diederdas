mod deck;
mod srs;

use deck::Deck;
use dfui::taffy::{AlignItems, Dimension, JustifyContent, LengthPercentageAuto};
use dfui::widgets::{Snapshot, Styled, button, column, container, future_builder, row, scroll, spacer, text, watch};
use dfui::anim::Lerp;
use dfui::{App, Color, Element, EventCx, Font, Notifier, Shadow, sleep};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

struct Palette {
    bg: Color,
    surface: Color,
    ink: Color,
    muted: Color,
    line: Color,
    ok: Color,
    bad: Color,
    shadow: f32,
    /// Text and fill colour per article: der, die, das, den, dem.
    articles: [(u32, u32); 5],
}

const LIGHT: Palette = Palette {
    bg: Color::hex(0xeef1f5),
    surface: Color::WHITE,
    ink: Color::hex(0x18202b),
    muted: Color::hex(0x5d6878),
    line: Color::hex(0xd5dbe3),
    ok: Color::hex(0x238a4f),
    bad: Color::hex(0xc8372f),
    shadow: 0.08,
    articles: [(0x2a5db0, 0xc9dcf6), (0xa2368f, 0xf2cdea), (0x8a6100, 0xf6e2b0), (0x6040b0, 0xdccff5), (0x1f7486, 0xc4e8ef)],
};

const DARK: Palette = Palette {
    bg: Color::hex(0x11151b),
    surface: Color::hex(0x1b222c),
    ink: Color::hex(0xe8edf3),
    muted: Color::hex(0x97a3b3),
    line: Color::hex(0x2c3542),
    ok: Color::hex(0x4fc07f),
    bad: Color::hex(0xf07068),
    shadow: 0.3,
    articles: [(0x8fb4f0, 0x24395c), (0xe591d6, 0x4a2444), (0xe6c066, 0x4a3a14), (0xb4a0f0, 0x352a5c), (0x7dcfe0, 0x173f48)],
};

static IS_DARK: AtomicBool = AtomicBool::new(false);

fn pal() -> &'static Palette {
    if IS_DARK.load(Ordering::Relaxed) { &DARK } else { &LIGHT }
}

const BODY: Font = Font(include_bytes!("../assets/atkinson-400.ttf"));
const BOLD: Font = Font(include_bytes!("../assets/atkinson-700.ttf"));
const DISPLAY: Font = Font(include_bytes!("../assets/bricolage-800.ttf"));

const GAMES: [(&str, &str, &str); 2] = [
    ("Artikel", "Wörter", "Welcher Artikel passt zum Nomen? Fehler kommen schnell wieder, sichere Wörter seltener."),
    ("Fälle", "Sätze", "Welcher Artikel fehlt im Satz? Akkusativ oder Dativ – das Objekt verrät es."),
];
const OPTIONS: [&[&str]; 2] = [&["der", "die", "das"], &["der", "die", "das", "den", "dem"]];

/// Text and pastel fill colour of an article. Hues stay clear of red and
/// green, which mean wrong and right.
fn tint(article: &str) -> (Color, Color) {
    let p = pal();
    let i = match article {
        "der" => 0,
        "die" => 1,
        "das" => 2,
        "den" => 3,
        "dem" => 4,
        _ => return (p.ink, p.line),
    };
    let (fg, bg) = p.articles[i];
    (Color::hex(fg), Color::hex(bg))
}

struct Model {
    decks: [Deck; 2],
    screen: Screen,
    confirm_reset: bool,
}

enum Screen {
    Menu,
    Game(Game),
    Progress(usize),
}

struct Game {
    deck: usize,
    card: usize,
    ok: u32,
    all: u32,
    reveal: Option<Reveal>,
}

struct Reveal {
    chosen: &'static str,
    correct: bool,
    error: Option<String>,
}

impl Model {
    fn open(dir: PathBuf) -> Model {
        let _ = std::fs::create_dir_all(&dir);
        Model {
            decks: [
                Deck::open(deck::words(), dir.join("progress.json")),
                Deck::open(deck::sentences(), dir.join("progress-faelle.json")),
            ],
            screen: Screen::Menu,
            confirm_reset: false,
        }
    }

    fn pick(&self, deck: usize) -> usize {
        self.decks[deck].progress.pick(fastrand::f64())
    }

    fn start(&mut self, deck: usize) {
        let card = self.pick(deck);
        self.screen = Screen::Game(Game { deck, card, ok: 0, all: 0, reveal: None });
    }

    fn answer(&mut self, chosen: &'static str) {
        let Screen::Game(g) = &mut self.screen else { return };
        if g.reveal.is_some() {
            return;
        }
        let d = &mut self.decks[g.deck];
        let correct = chosen == d.items[g.card].answer;
        d.progress.answer(g.card, correct);
        let error = d.save().err().map(|e| e.to_string());
        g.all += 1;
        g.ok += correct as u32;
        g.reveal = Some(Reveal { chosen, correct, error });
    }

    fn next_card(&mut self) {
        let Screen::Game(g) = &self.screen else { return };
        let card = self.pick(g.deck);
        if let Screen::Game(g) = &mut self.screen {
            g.card = card;
            g.reveal = None;
        }
    }

    fn reset(&mut self) {
        for d in &mut self.decks {
            d.progress = srs::Progress::new(d.items.len());
            let _ = d.save();
        }
        self.confirm_reset = false;
    }
}

/// Shared handle to the model; handlers change it and notify the view.
#[derive(Clone)]
struct Cx {
    m: Rc<RefCell<Model>>,
    n: Notifier,
}

impl Cx {
    fn on(&self, f: impl Fn(&mut Model) + 'static) -> impl Fn(&mut EventCx) + 'static {
        let cx = self.clone();
        move |_| {
            f(&mut cx.m.borrow_mut());
            cx.n.notify();
        }
    }
}

pub struct Artikel(Cx);

impl Artikel {
    pub fn new(data_dir: PathBuf, dark: bool) -> Self {
        IS_DARK.store(dark, Ordering::Relaxed);
        Artikel(Cx { m: Rc::new(RefCell::new(Model::open(data_dir))), n: Notifier::new() })
    }
}

impl App for Artikel {
    fn view(&self) -> Element {
        let cx = self.0.clone();
        watch(&self.0.n, move || {
            let m = cx.m.borrow();
            let mut page = match &m.screen {
                Screen::Menu => column([scroll(menu(&cx, &m).gap(20.0)).fill().into()]),
                Screen::Game(g) => game(&cx, &m, g),
                Screen::Progress(d) => progress(&cx, &m.decks[*d]),
            }
            .padding_xy(16.0, 20.0)
            .gap(20.0)
            .fill()
            .fill_width();
            page.style_mut().max_size.width = LengthPercentageAuto::length(440.0);
            column([page.into()]).align(AlignItems::CENTER).fill()
        })
        .fill()
        .into()
    }

    fn background(&self) -> Color {
        pal().bg
    }
}

fn label(s: &str) -> Element {
    text(s.to_uppercase()).font(BOLD).size(12.0).color(pal().muted).into()
}

fn link(s: &str, f: impl Fn(&mut EventCx) + 'static) -> Element {
    button(text(s).font(BOLD).size(15.0).color(pal().muted)).color(Color::TRANSPARENT).flat().padding_xy(0.0, 6.0).on_click(f).into()
}

fn small_button(s: &str, f: impl Fn(&mut EventCx) + 'static) -> Element {
    button(text(s).font(BODY).size(15.0).color(pal().ink)).color(pal().surface).flat().radius(8.0).padding_xy(12.0, 4.0).on_click(f).into()
}

fn chip(s: &str, t: &str) -> Element {
    let (fg, bg) = tint(t);
    container(text(s).font(BOLD).size(13.0).color(fg)).bg(bg).radius(99.0).padding_xy(10.0, 3.0).into()
}

fn card_shadow() -> Shadow {
    Shadow { color: Color::BLACK.alpha(pal().shadow), offset: (0.0, 8.0), blur: 24.0 }
}

/// `text` as wrapping words, with `art` in the `___` gap or in front.
fn phrase(text: &str, art: Element, word: impl Fn(&str) -> Element, gap: f32) -> dfui::widgets::Container {
    let mut art = Some(art);
    let mut words: Vec<Element> = text
        .split(' ')
        .map(|w| match w.split_once("___") {
            Some((pre, post)) => {
                let a = art.take().unwrap();
                if pre.is_empty() && post.is_empty() { a } else { row([word(pre), a, word(post)]).into() }
            }
            None => word(w),
        })
        .collect();
    if let Some(a) = art {
        words.insert(0, a);
    }
    row(words).wrap().gap(gap).align(AlignItems::CENTER)
}

fn menu(cx: &Cx, m: &Model) -> dfui::widgets::Container {
    let h1 = |s: &str, c| text(s).font(DISPLAY).size(35.0).color(c).into();
    let header = column([
        row(["der", "die", "das"].map(|a| h1(a, tint(a).0))).gap(10.0).into(),
        h1("Deutsch üben", pal().ink),
        text("Kurze Runden, jeden Tag ein bisschen.").font(BODY).color(pal().muted).into(),
    ])
    .gap(2.0);

    let mut items = vec![header.into()];
    items.extend((0..2).map(|g| tile(cx, g, &m.decks[g])));
    items.push(if m.confirm_reset {
        row([
            text("Wirklich alles löschen?").font(BOLD).color(pal().bad).into(),
            small_button("Ja, löschen", cx.on(Model::reset)),
            small_button("Abbrechen", cx.on(|m| m.confirm_reset = false)),
        ])
        .wrap()
        .gap(10.0)
        .align(AlignItems::CENTER)
        .into()
    } else {
        row([link("Fortschritt zurücksetzen", cx.on(|m| m.confirm_reset = true))]).into()
    });
    items.push(text("„Gelernt“ heißt: dreimal hintereinander richtig.").font(BODY).size(13.0).color(pal().muted).into());
    column(items)
}

fn tile(cx: &Cx, g: usize, d: &Deck) -> Element {
    let (name, unit, sub) = GAMES[g];
    let s = d.stats();
    let chips = if g == 0 {
        row(["der", "die", "das"].map(|a| chip(a, a)))
    } else {
        row(["den", "dem", "der"].map(|a| chip(a, a)))
    };
    let stat = |l: &str, n: usize| {
        column([label(l), text(n.to_string()).font(DISPLAY).size(22.0).color(pal().ink).into()]).gap(2.0).grow(1.0).into()
    };
    let mut fill = spacer().bg(pal().ok).radius(3.0).height(6.0);
    fill.style_mut().size.width = Dimension::percent(s.learned as f32 / s.total.max(1) as f32);
    let play = button(text("Spielen").font(BOLD).color(pal().bg))
        .color(pal().ink)
        .radius(12.0)
        .padding_xy(18.0, 10.0)
        .on_click(cx.on(move |m| m.start(g)));
    container(column([
        label(&format!("Spiel {}", g + 1)),
        text(name).font(DISPLAY).size(26.0).color(pal().ink).into(),
        chips.gap(6.0).into(),
        text(sub).font(BODY).color(pal().muted).into(),
        row([stat("Gesehen", s.seen), stat("Gelernt", s.learned), stat(unit, s.total)]).gap(8.0).into(),
        row([fill.into()]).bg(pal().line).radius(3.0).height(6.0).into(),
        row([play.into(), link("Lernstand ansehen", cx.on(move |m| m.screen = Screen::Progress(g)))])
            .wrap()
            .gap(18.0)
            .align(AlignItems::CENTER)
            .into(),
    ])
    .gap(14.0))
    .bg(pal().surface)
    .radius(18.0)
    .padding(22.0)
    .shadow(card_shadow())
    .into()
}

fn game(cx: &Cx, m: &Model, g: &Game) -> dfui::widgets::Container {
    let item = &m.decks[g.deck].items[g.card];
    let c = &m.decks[g.deck].progress.cards[g.card];
    let sentence = g.deck == 1;
    // ponytail: long nouns shrink by length; measure the text if one still overflows.
    let size = if sentence { 24.0 } else { 46.0f32.min(620.0 / item.text.chars().count() as f32) };
    let word = |w: &str| text(w).font(DISPLAY).size(size).color(pal().ink).into();
    let gap = size * 0.28;
    let art_c = tint(item.answer).0;

    let (noun, verdict, meta) = match &g.reveal {
        None => {
            let noun = if sentence { phrase(item.text, word("___"), word, gap) } else { row([word(item.text)]) };
            let meta = match m.decks[g.deck].progress.gap(g.card) {
                None => "Neu".to_string(),
                Some(ago) => format!("Serie: {} · zuletzt vor {ago} Karten", c.streak),
            };
            (noun, text(""), meta)
        }
        Some(r) => {
            let art = text(item.answer).font(DISPLAY).size(size).color(art_c).into();
            let msg = if let Some(e) = &r.error {
                format!("Speichern fehlgeschlagen: {e}")
            } else if r.correct {
                if item.note.is_empty() { "Richtig!".into() } else { format!("Richtig! {}", item.note) }
            } else if item.note.is_empty() {
                format!("Falsch. Es heißt „{} {}“.", item.answer, item.text)
            } else {
                format!("Falsch. Richtig ist „{}“ – {}.", item.answer, item.note)
            };
            let ok = r.correct && r.error.is_none();
            let verdict = text(msg).font(BOLD).color(if ok { pal().ok } else { pal().bad });
            let meta = format!("Serie: {} · kommt in ca. {} Karten wieder", c.streak, c.interval());
            (phrase(item.text, art, word, gap), verdict, meta)
        }
    };

    let card = container(column([
        noun.justify(JustifyContent::CENTER).into(),
        verdict.min_height(24.0).into(),
        text(meta).font(BODY).size(13.0).color(pal().muted).into(),
    ])
    .gap(10.0)
    .align(AlignItems::CENTER))
    .bg(pal().surface)
    .radius(22.0)
    .padding(24.0)
    .shadow(card_shadow())
    // Zero basis: the card's height comes from the free space, not its text, so revealing doesn't resize it.
    .fill()
    .min_height(130.0)
    .justify(JustifyContent::CENTER);

    let answers = row(OPTIONS[g.deck].iter().map(|&a| {
        let bg = tint(a).1;
        let dim = g.reveal.as_ref().is_some_and(|r| r.chosen != a);
        let b = button(text(a).font(DISPLAY).size(22.0).color(pal().ink))
            .color(if dim { bg.lerp(pal().bg, 0.6) } else { bg })
            .radius(16.0)
            .padding_xy(0.0, 20.0)
            .fill();
        if g.reveal.is_some() { b.into() } else { b.on_click(cx.on(move |m| m.answer(a))).into() }
    }))
    .gap(10.0);

    let mut items = vec![
        row([
            link("← Menü", cx.on(|m| m.screen = Screen::Menu)),
            spacer().grow(1.0).into(),
            text(format!("Richtig: {} / {}", g.ok, g.all)).font(BOLD).color(pal().ink).into(),
        ])
        .align(AlignItems::CENTER)
        .into(),
        card.into(),
        answers.into(),
    ];
    // The timer lives inside the bottom spacer, so the column's children (and gaps) stay the same.
    let mut bottom = spacer().fill();
    if let Some(r) = &g.reveal {
        let wait = Duration::from_millis(if r.correct { 900 } else { 2200 });
        let cx = cx.clone();
        bottom = bottom.child(future_builder(
            g.all,
            move |_| async move {
                sleep(wait).await;
                cx.m.borrow_mut().next_card();
                cx.n.notify();
            },
            |_: &Snapshot<Option<()>>| spacer(),
        ));
    }
    items.push(bottom.into());
    column(items)
}

fn progress(cx: &Cx, d: &Deck) -> dfui::widgets::Container {
    let cell = |s: String, w: f32, c: Color, f: Font| {
        container(text(s).font(f).size(14.0).color(c)).width(w).shrink(0.0).align(AlignItems::END).into()
    };
    let head = |s: &str, w| container(label(s)).width(w).shrink(0.0).align(AlignItems::END).into();
    let line = |first: Element, cols: Vec<Element>| {
        let mut r = vec![container(first).grow(1.0).shrink(1.0).into()];
        r.extend(cols);
        row(r).gap(8.0).padding_xy(12.0, 8.0).bg(pal().surface).align(AlignItems::CENTER).into()
    };
    let rows = d.rows();
    let table: Element = if rows.is_empty() {
        text("Noch nichts gespielt.").font(BODY).color(pal().muted).into()
    } else {
        let mut lines =
            vec![line(label("Karte"), vec![head("Serie", 44.0), head("Richtig", 52.0), head("Falsch", 48.0), head("Fällig in", 72.0)])];
        lines.extend(rows.iter().map(|r| {
            let word = |w: &str| text(w).font(BODY).size(14.0).color(pal().ink).into();
            let (due, due_c, due_f) =
                if r.due_in <= 0 { ("jetzt".into(), pal().bad, BOLD) } else { (format!("{} Karten", r.due_in), pal().ink, BODY) };
            line(
                phrase(r.item.text, chip(r.item.answer, r.item.answer), word, 4.0).into(),
                vec![
                    cell(r.streak.to_string(), 44.0, pal().ink, BODY),
                    cell(r.correct.to_string(), 52.0, pal().ink, BODY),
                    cell(r.wrong.to_string(), 48.0, pal().ink, BODY),
                    cell(due, 72.0, due_c, due_f),
                ],
            )
        }));
        column(lines).gap(1.0).bg(pal().line).into()
    };
    column([
        row([link("← Menü", cx.on(|m| m.screen = Screen::Menu)), spacer().grow(1.0).into(), label("Lernstand")])
            .align(AlignItems::CENTER)
            .into(),
        text("Sortiert nach nächster Wiederholung. Nach einem Fehler kommt das Wort nach etwa 10 Karten wieder, nach n richtigen Antworten in Folge nach etwa 20 × n Karten.")
            .font(BODY)
            .size(15.0)
            .color(pal().muted)
            .into(),
        scroll(container(table).bg(pal().surface).radius(14.0).padding_xy(0.0, 6.0)).fill().into(),
    ])
}

/// Whether GNOME's colour scheme prefers dark; false where gsettings is missing.
pub fn desktop_dark() -> bool {
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("dark"))
}

/// `$XDG_DATA_HOME/de.luisayuso.artikel`, falling back to `~/.local/share`.
pub fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_default()
        .join("de.luisayuso.artikel")
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: dfui::AndroidApp) {
    // The data dir root, not files/: progress written by the earlier Tauri build lives there.
    let dir = app.internal_data_path().and_then(|d| d.parent().map(PathBuf::from)).expect("app data dir");
    let dark = dfui::android::with_activity(&app, night_mode).unwrap_or(false);
    dfui::run_android(Artikel::new(dir, dark), app);
}

/// Whether the system is in dark mode: `Configuration.uiMode & UI_MODE_NIGHT_MASK == UI_MODE_NIGHT_YES`.
#[cfg(target_os = "android")]
fn night_mode(
    env: &mut jni::Env,
    activity: &jni::objects::JObject,
) -> jni::errors::Result<bool> {
    use jni::{jni_sig, jni_str};
    let res = env.call_method(activity, jni_str!("getResources"), jni_sig!("()Landroid/content/res/Resources;"), &[])?.l()?;
    let conf = env
        .call_method(&res, jni_str!("getConfiguration"), jni_sig!("()Landroid/content/res/Configuration;"), &[])?
        .l()?;
    let mode = env.get_field(&conf, jni_str!("uiMode"), jni_sig!("I"))?.i()?;
    Ok(mode & 0x30 == 0x20)
}
