use std::num::Wrapping;

use ratatui::text::ToText;

mod data {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};
    #[derive(Deserialize, Serialize, Debug)]
    pub struct Word {
        pub id: String,
        pub usage_category: String,
        pub word: String,
        pub deprecated: bool,
        pub ku_data: Option<HashMap<String, u16>>,
        pub pu_verbatim: Option<HashMap<String, String>>,
        pub commentary: Option<String>,
        pub definitions: Option<String>,
    }

    #[derive(Deserialize, Serialize, Debug)]
    struct Words {
        words: Vec<Word>,
    }

    pub fn get_compressed() -> Vec<Word> {
        let mut toml = String::new();
        std::io::Read::read_to_string(
            &mut bzip2::read::BzDecoder::new(include_bytes!("../res/words.toml.bz2").as_slice()),
            &mut toml,
        )
        .unwrap();
        toml::from_str::<Words>(&toml).unwrap().words
    }

    pub fn get() -> Vec<Word> {
        toml::from_str::<Words>(include_str!("../res/words.toml"))
            .unwrap()
            .words
    }
}

struct Word {
    target: String,
    input: String,
    info: String,
    enter: Option<std::time::Instant>,
    duration: std::time::Duration,
}
impl Word {
    fn new(target: String, info: String) -> Self {
        Self {
            target,
            input: String::new(),
            info,
            enter: None,
            duration: std::time::Duration::default(),
        }
    }
    fn handle_input(&mut self, event: ratatui::crossterm::event::KeyEvent, index: &mut usize) {
        match (self.enter, event.code) {
            (Some(enter), ratatui::crossterm::event::KeyCode::Char(' ')) => {
                self.duration += enter.elapsed();
                *index += 1;
            }
            (Some(enter), ratatui::crossterm::event::KeyCode::Backspace) => {
                if self.input.pop().is_none() {
                    self.duration += enter.elapsed();
                    self.enter = None;
                    *index -= 1;
                }
            }
            (None, ratatui::crossterm::event::KeyCode::Char(c)) => {
                self.enter = Some(std::time::Instant::now());
                self.input.push(c);
            }
            (_, ratatui::crossterm::event::KeyCode::Char(c)) => {
                self.input.push(c);
            }
            _ => (),
        }
    }
    fn get_widget(&self) -> Vec<ratatui::text::Span> {
        let mut target = self.target.chars();
        let mut input = self.input.chars();

        let mut spans = Vec::new();

        use ratatui::style::Stylize;
        use ratatui::text::ToSpan;
        loop {
            match (target.next(), input.next()) {
                (None, None) => break,
                (None, Some(i)) => spans.push(ratatui::text::Span::raw(i.to_string()).light_red()),
                (Some(_), None) => spans.push('_'.to_span()),
                (Some(t), Some(i)) if t == i => spans.push(ratatui::text::Span::raw(i.to_string())),
                (Some(t), Some(_)) => spans.push(ratatui::text::Span::raw(t.to_string()).red()),
            }
        }
        spans.push(' '.to_span());
        spans
    }
}

fn game(words: Vec<data::Word>, mut terminal: ratatui::DefaultTerminal) {
    let mut test: Vec<Word> = words
        .iter()
        .filter(|word| word.usage_category == "core")
        .filter(|word| word.definitions.is_some())
        .map(|word| Word::new(word.word.clone(), word.definitions.clone().unwrap()))
        .collect();

    use rand::seq::SliceRandom;
    test.shuffle(&mut rand::thread_rng());
    test.truncate(40);

    let mut index = 0;

    loop {
        terminal
            .draw(|frame| {
                let mut text = test[index].info.to_text();
                text.push_line(
                    test.iter()
                        .flat_map(|word| word.get_widget())
                        .collect::<ratatui::text::Line>(),
                );

                frame.render_widget(
                    ratatui::widgets::Paragraph::new(text)
                        .wrap(ratatui::widgets::Wrap { trim: false }),
                    frame.area(),
                );
            })
            .unwrap();

        if let Ok(ratatui::crossterm::event::Event::Key(key)) = ratatui::crossterm::event::read() {
            test[index].handle_input(key, &mut index);
        }

        if index >= test.len() {
            break;
        }
    }
}

fn main() {
    let mut terminal = ratatui::init();
    terminal.clear().unwrap();

    let words = std::thread::spawn(data::get_compressed);

    let words = words.join().unwrap();

    game(words, terminal);

    ratatui::restore();
}
