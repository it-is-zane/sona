use rand::seq::index;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct WordData {
    pub id: String,
    pub usage_category: String,
    pub word: String,
    pub deprecated: bool,
    pub ku_data: Option<std::collections::HashMap<String, u16>>,
    pub pu_verbatim: Option<std::collections::HashMap<String, String>>,
    pub commentary: Option<String>,
    pub definitions: Option<String>,
}

static WORDS: std::sync::LazyLock<Vec<WordData>> = std::sync::LazyLock::new(|| {
    #[derive(Deserialize, Serialize, Debug)]
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

#[derive(Debug)]
enum Action {
    Char(char),
    Backspace,
    Goto(Page),
    ApplyGameSettigns(GameSettings),
    Exit,
}

#[derive(Default)]
struct Dispatcher {
    stores: Vec<std::rc::Rc<std::cell::RefCell<dyn Store>>>,
    queue: std::collections::VecDeque<Action>,
}

impl Dispatcher {
    fn new() -> Self {
        Dispatcher::default()
    }
    fn register<T: Store + 'static>(&mut self, store: T) -> std::rc::Rc<std::cell::RefCell<T>> {
        let rc = std::rc::Rc::new(std::cell::RefCell::new(store));
        self.stores.push(rc.clone());
        rc
    }
    fn action(&mut self, action: Action) {
        self.queue.push_back(action);
    }
    fn update(&mut self) {
        let mut new_queue = std::collections::VecDeque::new();

        self.queue.iter().for_each(|action| {
            self.stores
                .iter_mut()
                .for_each(|store| store.borrow_mut().update(action, &mut new_queue))
        });

        self.queue = new_queue;
    }
}

trait Store {
    fn update(&mut self, action: &Action, queue: &mut std::collections::VecDeque<Action>);
}

#[derive(Copy, Clone, Debug)]
enum Page {
    Game,
    Results,
}

impl Store for Page {
    fn update(&mut self, action: &Action, queue: &mut std::collections::VecDeque<Action>) {
        if let Action::Goto(page) = action {
            *self = *page;
        }
    }
}

impl Store for bool {
    fn update(&mut self, action: &Action, queue: &mut std::collections::VecDeque<Action>) {
        if let Action::Exit = action {
            *self = true
        }
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
}

#[derive(Debug)]
struct GameSettings {
    word_count: usize,
}

#[derive(Default)]
struct Game {
    words: Vec<Word>,
    index: usize,
    start: Option<std::time::Instant>,
}

impl Store for Game {
    fn update(&mut self, action: &Action, queue: &mut std::collections::VecDeque<Action>) {
        match action {
            Action::ApplyGameSettigns(settings) => {
                self.words = WORDS
                    .iter()
                    .filter_map(|word| {
                        word.definitions
                            .clone()
                            .map(|def| Word::new(word.word.clone(), def))
                    })
                    .take(settings.word_count)
                    .collect();

                use rand::seq::SliceRandom;
                self.words.shuffle(&mut rand::thread_rng());
            }
            Action::Char(' ') => {
                if let Some(word) = self.words.get_mut(self.index) {
                    if let Some(enter) = word.enter {
                        word.duration += enter.elapsed();
                    }

                    word.enter = None;
                }

                self.index += 1;

                if self.index >= self.words.len() - 1 {
                    queue.push_back(Action::Goto(Page::Results));
                }
            }
            Action::Backspace => {
                if let Some(i) = self.index.checked_sub(1) {
                    if let Some(word) = self.words.get_mut(self.index) {
                        if let Some(enter) = word.enter {
                            word.duration += enter.elapsed();
                        }

                        word.enter = None;
                    }

                    self.index = i;
                }
            }
            Action::Char(c) => {
                if let Some(word) = self.words.get_mut(self.index) {
                    word.input.push(*c);

                    if word.enter.is_none() {
                        word.enter = Some(std::time::Instant::now());
                    }
                }
            }
            _ => (),
        }
    }
}

fn main() {
    let mut terminal = ratatui::init();
    terminal.clear().unwrap();

    let mut dis = Dispatcher::new();

    let should_exit = dis.register(false);
    let page = dis.register(Page::Game);
    let game = dis.register(Game::default());

    dis.action(Action::ApplyGameSettigns(GameSettings { word_count: 10 }));

    loop {
        dis.update();

        if *should_exit.borrow_mut() {
            break;
        }

        let game = game.borrow();
        let index = game.index;

        _ = match *page.borrow_mut() {
            Page::Game => terminal.draw(|_frame| {
                _frame.render_widget(
                    game.words
                        .iter()
                        .map(|word| word.target.clone() + " ")
                        .collect::<String>()
                        + &index.to_string()
                        + " "
                        + &game.words[index].target,
                    _frame.area(),
                )
            }), // draw game
            Page::Results => terminal.draw(|_frame| _frame.render_widget("results", _frame.area())), // draw results
        };

        if let Ok(ratatui::crossterm::event::Event::Key(key)) = ratatui::crossterm::event::read() {
            let ctrl = key
                .modifiers
                .contains(ratatui::crossterm::event::KeyModifiers::CONTROL);

            use ratatui::crossterm::event::KeyCode::*;
            match key.code {
                Backspace => dis.action(Action::Backspace),
                Char('c') if ctrl => dis.action(Action::Exit),
                Char(c) => dis.action(Action::Char(c)),
                _ => (),
            }
        }
    }

    ratatui::restore();
}
