#[allow(non_camel_case_types)]
#[derive(
    serde::Deserialize, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum UsageCategory {
    core,
    common,
    uncommon,
    obscure,
    sandbox,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct WordData {
    pub id: String,
    pub usage_category: UsageCategory,
    pub word: String,
    pub deprecated: bool,
    pub ku_data: Option<std::collections::HashMap<String, u16>>,
    pub pu_verbatim: Option<std::collections::HashMap<String, String>>,
    pub commentary: Option<String>,
    pub definitions: Option<String>,
}

static WORDS: std::sync::LazyLock<Vec<WordData>> = std::sync::LazyLock::new(|| {
    #[derive(serde::Deserialize, serde::Serialize, Debug)]
    struct Words {
        words: Vec<WordData>,
    }

    #[cfg(feature = "compressed")]
    let words = {
        let mut toml = String::new();
        std::io::Read::read_to_string(
            &mut bzip2::read::BzDecoder::new(include_bytes!("../res/words.toml.bz2").as_slice()),
            &mut toml,
        )
        .unwrap();
        toml::from_str::<Words>(&toml).unwrap().words
    };

    #[cfg(not(feature = "compressed"))]
    let mut words = {
        toml::from_str::<Words>(include_str!("../res/words.toml"))
            .unwrap()
            .words
    };

    words
});

#[derive(serde::Serialize, serde::Deserialize)]
struct WordErrors {
    words: std::collections::HashMap<String, (i32, i32)>,
}

/// Extends iterators by first wraping its elements with Some and then chains an infinite iterator of None elements.
fn extend<I: Clone, T: Iterator<Item = I>>(
    iter: T,
) -> std::iter::Chain<std::iter::Map<T, impl Fn(I) -> Option<I>>, std::iter::Repeat<Option<I>>> {
    iter.map(Some).chain(std::iter::repeat(None))
}

/// Zips two iterators so that the resulting iterator is the length of the longest iterator.
/// Their items are wraped with Some so that if one itterator runs out it can return None
fn full_zip<IA: Clone, IB: Clone, A: Iterator<Item = IA>, B: Iterator<Item = IB>>(
    a: A,
    b: B,
) -> impl std::iter::Iterator<Item = (Option<IA>, Option<IB>)> {
    extend(a)
        .zip(extend(b))
        .take_while(|(a, b)| a.is_some() || b.is_some())
}

enum TextRenderType<'a> {
    Correct(&'a str),
    Incorrect { target: &'a str, input: &'a str },
    Excess(&'a str),
    NoInput(&'a str),
}

fn color_text<'a>(target: &str, input: &str) -> ratatui::prelude::Text<'a> {
    use ratatui::style::Stylize;

    let default = ratatui::style::Style::new();
    let blank = default;
    let correct = default;
    let error = default.red().underlined();
    let excess = default.light_yellow();

    let mut colored_out = ratatui::text::Text::default();

    full_zip(target.split_terminator(' '), input.split_terminator(' ')).for_each(
        |(target, input)| {
            match (target, input) {
                (Some(target), None) => colored_out
                    .push_span(ratatui::text::Span::raw("_".repeat(target.len())).style(blank)),
                (Some(target), Some(input)) => {
                    full_zip(target.chars(), input.chars()).for_each(|(target, input)| {
                        match (target, input) {
                            (Some(target), Some(input)) if target == input => colored_out
                                .push_span(
                                    ratatui::text::Span::raw(target.to_string()).style(correct),
                                ),
                            (Some(target), Some(input)) if target != input => colored_out
                                .push_span(
                                    ratatui::text::Span::raw(target.to_string()).style(error),
                                ),
                            (Some(_), None) => {
                                colored_out.push_span(ratatui::text::Span::raw("_").style(blank))
                            }
                            (None, Some(input)) => colored_out.push_span(
                                ratatui::text::Span::raw(input.to_string()).style(excess),
                            ),
                            _ => (),
                        }
                    });
                }
                _ => (),
            }
            colored_out.push_span(ratatui::text::Span::raw(" ").style(blank));
        },
    );

    colored_out
}

#[derive(Default, Clone, Copy)]
struct WordReq {
    in_use: bool,
    deprecated: bool,
    core: bool,
    common: bool,
    uncommon: bool,
    obscure: bool,
    sandbox: bool,
    ku: bool,
    pu: bool,
    commentary: bool,
    definitions: bool,
    n: usize,
}

fn get_subset<'a>(settings: WordReq) -> Vec<&'a WordData> {
    use rand::seq::SliceRandom;

    let mut words: Vec<&WordData> = WORDS
        .iter()
        .filter(|data| settings.in_use | data.deprecated)
        .filter(|data| settings.deprecated | !data.deprecated)
        .filter(|data| settings.core | (data.usage_category != UsageCategory::core))
        .filter(|data| settings.common | (data.usage_category != UsageCategory::common))
        .filter(|data| settings.uncommon | (data.usage_category != UsageCategory::uncommon))
        .filter(|data| settings.obscure | (data.usage_category != UsageCategory::obscure))
        .filter(|data| settings.sandbox | (data.usage_category != UsageCategory::sandbox))
        .filter(|data| settings.ku | data.ku_data.is_some())
        .filter(|data| settings.pu | data.pu_verbatim.is_some())
        .filter(|data| settings.commentary | data.commentary.is_some())
        .filter(|data| settings.definitions | data.definitions.is_some())
        .collect();

    words.drain((settings.n)..);

    words.shuffle(&mut rand::thread_rng());

    words
}

enum State {
    Game { settings: WordReq },
    Results {},
    Settings,
    Exit,
}

fn get_char(event: &ratatui::crossterm::event::Event) -> Option<char> {
    if let ratatui::crossterm::event::Event::Key(key) = event {
        if let ratatui::crossterm::event::KeyCode::Char(c) = key.code {
            return Some(c);
        }
    }

    None
}

fn render(
    colored_out: ratatui::text::Text,
    hint: Option<&String>,
    terminal: &mut ratatui::DefaultTerminal,
) {
    terminal
        .draw(|frame| {
            let layout: [_; 2] = ratatui::layout::Layout::new(
                ratatui::layout::Direction::Vertical,
                ratatui::layout::Constraint::from_mins([10, 100]),
            )
            .areas(frame.area());

            let block =
                ratatui::widgets::Block::new().padding(ratatui::widgets::Padding::new(1, 1, 1, 0));

            if let Some(hint) = hint {
                use ratatui::text::ToSpan;

                frame.render_widget(
                    ratatui::widgets::Paragraph::new(hint.to_span()),
                    block.inner(layout[0]),
                );
            }

            frame.render_widget(
                ratatui::widgets::Paragraph::new(colored_out)
                    .wrap(ratatui::widgets::Wrap { trim: false }),
                block.inner(layout[1]),
            );
        })
        .unwrap();
}

fn handle_input(
    index: &mut usize,
    input: &mut String,
    durations: &mut Vec<std::time::Duration>,
    enter: &mut std::time::Instant,
    exit: &mut bool,
) {
    let event = ratatui::crossterm::event::read().unwrap();

    if input.is_empty() {
        *enter = std::time::Instant::now();
        durations.clear();
    }

    match get_char(&event) {
        Some(' ') => {
            match durations.get_mut(*index) {
                Some(duration) => *duration += enter.elapsed(),
                None => durations.push(enter.elapsed()),
            }
            *enter = std::time::Instant::now();

            input.push(' ');
            *index += 1
        }
        Some('q') => *exit = true,
        Some(c) => input.push(c),
        None => {
            if let ratatui::crossterm::event::Event::Key(ratatui::crossterm::event::KeyEvent {
                code: ratatui::crossterm::event::KeyCode::Backspace,
                ..
            }) = event
            {
                if let Some(' ') = input.pop() {
                    match durations.get_mut(*index) {
                        Some(duration) => *duration += enter.elapsed(),
                        None => durations.push(enter.elapsed()),
                    }
                    *enter = std::time::Instant::now();

                    *index -= 1;
                }
            }
        }
    }
}

fn get_word_skills() {}

fn main() {
    let mut terminal = ratatui::init();
    let word_skill: std::collections::HashMap<String, (usize, usize, usize)>;

    let mut sorted_words: Vec<WordData> = WORDS.iter().cloned().collect();
    sorted_words.sort_unstable_by(|a, b| a.usage_category.cmp(&b.usage_category));

    let (ids, words, definitions) = sorted_words
        .iter()
        .map(|word| (&word.id, &word.word, word.usage_category, &word.definitions))
        .filter_map(|(id, word, cat, def)| def.as_ref().map(|d| (id, word, cat, d)))
        .fold(
            (String::new(), String::new(), Vec::<String>::new()),
            |(mut ai, mut aw, mut ad), (id, word, cat, def)| {
                ai.push_str(id);
                ai.push(' ');
                aw.push_str(word);
                aw.push(' ');
                ad.push(format!("{:?}: ", cat) + def);
                (ai, aw, ad)
            },
        );

    let mut index: usize = 0;
    let mut input = String::new();
    let mut durations: Vec<std::time::Duration> = Vec::new();
    let mut enter = std::time::Instant::now();
    let mut exit = false;

    loop {
        let colored_out = color_text(&words, &input);

        render(colored_out, definitions.get(index), &mut terminal);

        handle_input(
            &mut index,
            &mut input,
            &mut durations,
            &mut enter,
            &mut exit,
        );

        if exit {
            break;
        }
    }

    ratatui::restore();
}
