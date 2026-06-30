use std::thread;

use crossterm::event::{self, Event, KeyEventKind};

use crate::queue::{QueueEvent, QueueSender};

#[derive(Debug, Clone)]
pub enum InputEvent {
    Terminal(Event),
    Error(String),
}

pub struct InputSource {
    _thread: thread::JoinHandle<()>,
}

impl InputSource {
    pub fn start(tx: QueueSender) -> Self {
        let thread = thread::spawn(move || {
            loop {
                match event::read() {
                    Ok(event) if should_forward(&event) => {
                        if tx
                            .send(QueueEvent::Input(InputEvent::Terminal(event)))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        let _ = tx.send(QueueEvent::Input(InputEvent::Error(error.to_string())));
                        break;
                    }
                }
            }
        });

        Self { _thread: thread }
    }
}

fn should_forward(event: &Event) -> bool {
    !matches!(
        event,
        Event::Key(key) if key.kind != KeyEventKind::Press
    )
}
