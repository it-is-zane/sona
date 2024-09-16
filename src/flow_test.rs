#[derive(Debug)]
enum Action {
    Char(char),
    Backspace,
    Goto(Page),
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

trait View {}

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
        match action {
            Action::Exit => *self = true,
            _ => (),
        }
    }
}

fn main() {
    let mut dis = Dispatcher::new();

    let mut should_exit = dis.register(false);
    let mut page = dis.register(Page::Game);

    loop {
        dis.update();
        
        dis.action(Action::Goto(Page::Results));
        dis.action(Action::Exit);

        match *page.borrow_mut() {
            Page::Game => println!("game"),       // draw game
            Page::Results => println!("results"), // draw results
        }

        if *should_exit.borrow_mut() {
            break;
        }
    }
}
