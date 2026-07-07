use std::thread;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::queue::{QueueEvent, QueueSender};

#[derive(Debug, Clone)]
pub enum InputEvent {
    Terminal(Event),
    Error(String),
}

impl InputEvent {
    pub fn key_display(&self) -> Option<String> {
        match self {
            Self::Terminal(Event::Key(key)) => Some(key_display(*key)),
            Self::Terminal(Event::Resize(width, height)) => Some(format!("{width}x{height}")),
            Self::Terminal(_) | Self::Error(_) => None,
        }
    }
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
                        tracing::debug!(?event, "terminal event received");
                        if tx
                            .send(QueueEvent::Input(InputEvent::Terminal(event)))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(event) => tracing::debug!(?event, "terminal event ignored"),
                    Err(error) => {
                        tracing::debug!(%error, "terminal input error");
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
    !matches!(event, Event::Key(key) if key.kind != KeyEventKind::Press)
}

fn key_display(key: KeyEvent) -> String {
    let mut text = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        text.push_str("C-");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        text.push_str("M-");
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) && !matches!(key.code, KeyCode::Char(_)) {
        text.push_str("S-");
    }

    match key.code {
        KeyCode::Backspace => text.push_str("BS"),
        KeyCode::Enter => text.push_str("Enter"),
        KeyCode::Left => text.push_str("Left"),
        KeyCode::Right => text.push_str("Right"),
        KeyCode::Up => text.push_str("Up"),
        KeyCode::Down => text.push_str("Down"),
        KeyCode::Home => text.push_str("Home"),
        KeyCode::End => text.push_str("End"),
        KeyCode::PageUp => text.push_str("PageUp"),
        KeyCode::PageDown => text.push_str("PageDown"),
        KeyCode::Tab => text.push_str("Tab"),
        KeyCode::BackTab => text.push_str("S-Tab"),
        KeyCode::Delete => text.push_str("Del"),
        KeyCode::Insert => text.push_str("Ins"),
        KeyCode::F(index) => text.push_str(&format!("F{index}")),
        KeyCode::Char(' ') => text.push_str("Space"),
        KeyCode::Char(ch) => text.push(ch),
        KeyCode::Esc => text.push_str("Esc"),
        KeyCode::Null => text.push_str("Null"),
        KeyCode::CapsLock => text.push_str("CapsLock"),
        KeyCode::ScrollLock => text.push_str("ScrollLock"),
        KeyCode::NumLock => text.push_str("NumLock"),
        KeyCode::PrintScreen => text.push_str("PrintScreen"),
        KeyCode::Pause => text.push_str("Pause"),
        KeyCode::Menu => text.push_str("Menu"),
        KeyCode::KeypadBegin => text.push_str("KeypadBegin"),
        KeyCode::Media(_) => text.push_str("Media"),
        KeyCode::Modifier(_) => text.push_str("Modifier"),
    }

    if key.kind == KeyEventKind::Repeat {
        text.push('*');
    }
    text
}
