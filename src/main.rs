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
        self.queue.drain(..).for_each(|action| {
            self.stores
                .iter_mut()
                .for_each(|store| store.borrow_mut().update(&action))
        });
    }
}
trait Store {
    fn update(&mut self, action: &Action);
}

#[derive(Copy, Clone, Debug)]
enum Page {
    Game,
    Results,
}

impl Store for Page {
    fn update(&mut self, action: &Action) {
        if let Action::Goto(page) = action {
            *self = *page;
        }
    }
}

impl Store for bool {
    fn update(&mut self, action: &Action) {
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
    start: Option<std::time::Instant>,
    end: Option<std::time::Instant>,
}

impl Store for Game {
    fn update(&mut self, action: &Action) {
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
                    .collect()
            }
            Action::Char(c) => (),
            Action::Backspace => (),
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

    loop {
        dis.update();

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

        match *page.borrow_mut() {
            Page::Game => println!("game"),       // draw game
            Page::Results => println!("results"), // draw results
        }

        if *should_exit.borrow_mut() {
            break;
        }
    }

    ratatui::restore();
}
