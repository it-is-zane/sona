use ratatui::layout;

#[allow(non_camel_case_types)]
#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsageCategory {
    core,
    common,
    uncommon,
    obscure,
    sandbox,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
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
    {
        let mut toml = String::new();
        std::io::Read::read_to_string(
            &mut bzip2::read::BzDecoder::new(include_bytes!("../res/words.toml.bz2").as_slice()),
            &mut toml,
        )
        .unwrap();
        toml::from_str::<Words>(&toml).unwrap().words
    }

    #[cfg(not(feature = "compressed"))]
    {
        toml::from_str::<Words>(include_str!("../res/words.toml"))
            .unwrap()
            .words
    }
});

enum TextRenderType {
    Correct(String),
    Incorrect { target: String, input: String },
    Excess(String),
    NoInput(String),
}

struct Word<'a> {
    data: &'a WordData,
    time: std::time::Duration,
    errors: usize,
}

struct Game<'a> {
    words: Vec<Word<'a>>,
    input: String,
    time: std::time::Duration,
}

#[derive(Default, Clone, Copy)]
struct GameSettings {
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

impl Game<'_> {
    fn new(settings: GameSettings) -> Self {
        use rand::seq::SliceRandom;

        let word_iter = WORDS
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
            .filter(|data| settings.definitions | data.definitions.is_some());

        let mut words: Vec<Word> = word_iter
            .map(|data| Word {
                data,
                time: std::time::Duration::default(),
                errors: 0,
            })
            .collect();

        words.drain((settings.n)..);

        words.shuffle(&mut rand::thread_rng());

        Game {
            words,
            input: String::new(),
            time: std::time::Duration::default(),
        }
    }
    fn render(&self) -> Vec<TextRenderType> {
        use TextRenderType::*;
        self.words
            .iter()
            .map(|word| &word.data.word)
            .map(Some)
            .zip(
                self.input
                    .split(' ')
                    .map(Some)
                    .chain(std::iter::once(None).cycle()),
            )
            .fold(Vec::new(), |mut acc, vals| {
                match vals {
                    (Some(target), None) => {
                        if let Some(NoInput(str)) = acc.last_mut() {
                            str.push_str(target);
                        } else {
                            acc.push(NoInput(target.clone()));
                        }
                    }
                    (Some(target), Some(input)) => {
                        target
                            .chars()
                            .map(Some)
                            .chain(std::iter::once(None).cycle())
                            .zip(input.chars().map(Some).chain(std::iter::once(None).cycle()))
                            .take(target.len().max(input.len()))
                            .for_each(|char_pair| match char_pair {
                                (None, None) => (),
                                (None, Some(c)) => {
                                    if let Some(Excess(str)) = acc.last_mut() {
                                        str.push(c);
                                    } else {
                                        acc.push(Excess(c.to_string()));
                                    }
                                }
                                (Some(c), None) => {
                                    if let Some(NoInput(str)) = acc.last_mut() {
                                        str.push(c);
                                    } else {
                                        acc.push(NoInput(c.to_string()));
                                    }
                                }
                                (Some(t), Some(c)) if t == c => {
                                    if let Some(Correct(str)) = acc.last_mut() {
                                        str.push(c);
                                    } else {
                                        acc.push(Correct(c.to_string()));
                                    }
                                }
                                (Some(t), Some(c)) => {
                                    if let Some(Incorrect { target, input }) = acc.last_mut() {
                                        target.push(t);
                                        input.push(c);
                                    } else {
                                        acc.push(Incorrect {
                                            target: t.to_string(),
                                            input: c.to_string(),
                                        });
                                    }
                                }
                            });
                    }
                    _ => (),
                };

                if let Some(item) = acc.last_mut() {
                    match item {
                        Correct(str) => str.push(' '),
                        Incorrect {
                            target: _,
                            input: _,
                        }
                        | Excess(_)
                        | NoInput(_) => acc.push(Correct(" ".to_string())),
                    }
                };

                acc
            })
    }
}

enum Page {}
enum State {
    Game { settings: GameSettings },
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

fn main() {
    let mut terminal = ratatui::init();

    let mut state = State::Settings;

    loop {
        match state {
            State::Game { settings } => {
                let mut game = Game::new(settings);

                let layout = ratatui::layout::Layout::vertical([
                    ratatui::layout::Constraint::Min(4),
                    ratatui::layout::Constraint::Fill(1),
                    ratatui::layout::Constraint::Length(1),
                ]);

                let padding =
                    ratatui::widgets::Block::new().padding(ratatui::widgets::Padding::uniform(1));

                loop {
                    use ratatui::style::Stylize;
                    terminal
                        .draw(|frame| {
                            frame.render_widget(&padding, layout.split(frame.area())[0]);

                            frame.render_widget(
                                ratatui::widgets::Block::new()
                                    .title("white")
                                    .title_alignment(ratatui::layout::Alignment::Center)
                                    .on_white()
                                    .black(),
                                padding.inner(layout.split(frame.area())[0]),
                            );

                            frame.render_widget(&padding, layout.split(frame.area())[1]);

                            use ratatui::text::ToSpan;
                            frame.render_widget(
                                ratatui::widgets::Paragraph::new(
                                    game.render()
                                        .iter()
                                        .map(|item| -> ratatui::prelude::Span {
                                            match item {
                                                TextRenderType::Correct(str) => {
                                                    str.to_span().white()
                                                }
                                                TextRenderType::Incorrect { target, input: _ } => {
                                                    target.to_span().red().underlined()
                                                }
                                                TextRenderType::Excess(str) => {
                                                    str.to_span().yellow()
                                                }
                                                TextRenderType::NoInput(str) => {
                                                    str.to_span().dark_gray()
                                                }
                                            }
                                        })
                                        .collect::<ratatui::text::Line>(),
                                )
                                .wrap(ratatui::widgets::Wrap { trim: false }),
                                padding.inner(layout.split(frame.area())[1]),
                            );

                            frame.render_widget(
                                ratatui::widgets::Block::new()
                                    .title("Black")
                                    .title_alignment(ratatui::layout::Alignment::Center),
                                layout.split(frame.area())[2],
                            );
                        })
                        .unwrap();

                    let event = ratatui::crossterm::event::read().unwrap();

                    if let Some('q') = get_char(&event) {
                        break;
                    }

                    if let Some(c) = get_char(&event) {
                        game.input.push(c);
                    }

                    if let ratatui::crossterm::event::Event::Key(event) = event {
                        if let ratatui::crossterm::event::KeyCode::Backspace = event.code {
                            _ = game.input.pop();
                        }
                    }
                }

                state = State::Results {};
            }
            State::Results {} => todo!(),
            State::Settings => {
                state = State::Game {
                    settings: GameSettings {
                        n: 100,
                        in_use: true,
                        definitions: true,
                        core: true,
                        ..Default::default()
                    },
                };
            }
            State::Exit => break,
        }
    }

    ratatui::restore();
}
