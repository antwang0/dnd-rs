use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::{Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::fmt::Write as _;
use std::io;
use std::time::Duration;

use crate::actions::action_template::ActionExecutionInfo;
use crate::engine::encounter::EncounterInstance;
use crate::engine::types::Coordinate;

const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Outcome of handling a single key press.
pub enum Tick {
    Continue,
    Quit,
}

/// Holds the loop's mutable UI/input state so main.rs can stay slim.
pub struct App {
    pub encounter: EncounterInstance,
    map_width: u16,
    map_height: u16,
    input_str: String,
    tmp_message: String,
    selected_action_idx: usize,
    last_actor_id: Option<usize>,
}

impl App {
    pub fn new(encounter: EncounterInstance, map_width: usize, map_height: usize) -> Self {
        Self {
            encounter,
            map_width: u16::try_from(map_width).unwrap_or(u16::MAX),
            map_height: u16::try_from(map_height).unwrap_or(u16::MAX),
            input_str: String::new(),
            tmp_message: String::new(),
            selected_action_idx: 0,
            last_actor_id: None,
        }
    }

    /// Resync derived UI state with the engine — call once per loop iteration
    /// before drawing.
    pub fn refresh(&mut self) {
        self.encounter.process_stack();

        let curr_actor_id = self
            .encounter
            .peek_prompt()
            .map(crate::engine::prompt::Prompt::actor_id);
        if curr_actor_id != self.last_actor_id {
            self.selected_action_idx = 0;
            self.last_actor_id = curr_actor_id;
        }
        if let Some(prompt) = self.encounter.peek_prompt() {
            let n = prompt.actions().len();
            if n > 0 {
                self.selected_action_idx %= n;
            } else {
                self.selected_action_idx = 0;
            }
        }
    }

    pub fn draw(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(self.map_height + 2), // Map area
                Constraint::Length(3),                   // Input area
                Constraint::Length(3),                   // Temp message
                Constraint::Min(1),                      // Message log
            ])
            .split(f.area());

        let info_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(self.map_width + 2), Constraint::Min(1)])
            .split(chunks[0]);

        self.encounter.render_map(f, info_area[0]);
        self.encounter
            .render_sideinfo(f, info_area[1], self.selected_action_idx);

        let input_widget =
            Paragraph::new(self.input_str.as_str())
                .block(Block::default().borders(Borders::ALL).title("Input"));
        f.render_widget(input_widget, chunks[1]);

        let tmp_message_widget = Paragraph::new(self.tmp_message.as_str())
            .block(Block::default().borders(Borders::ALL).title("Message"));
        f.render_widget(tmp_message_widget, chunks[2]);

        let messages_text: Text = self
            .encounter
            .messages()
            .iter()
            .rev()
            .take(5)
            .map(|m| Span::raw(m.clone()))
            .collect();
        let messages_widget = Paragraph::new(messages_text)
            .block(Block::default().borders(Borders::ALL).title("Log"));
        f.render_widget(messages_widget, chunks[3]);
    }

    /// Polls a single key event (with timeout) and applies it to app state.
    pub fn pump_input(&mut self) -> io::Result<Tick> {
        if !event::poll(POLL_INTERVAL)? {
            return Ok(Tick::Continue);
        }
        let Event::Key(key) = event::read()? else {
            return Ok(Tick::Continue);
        };
        if key.kind != KeyEventKind::Press {
            return Ok(Tick::Continue);
        }
        Ok(self.handle_key(key))
    }

    fn handle_key(&mut self, key: KeyEvent) -> Tick {
        match key.code {
            KeyCode::Char(c) => self.input_str.push(c),
            KeyCode::Backspace => {
                self.input_str.pop();
            }
            KeyCode::Tab => self.cycle_action(),
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                self.handle_arrow(key.code);
            }
            KeyCode::Enter => return self.handle_enter(),
            KeyCode::Esc => return Tick::Quit,
            _ => {}
        }
        Tick::Continue
    }

    fn cycle_action(&mut self) {
        let Some(prompt) = self.encounter.peek_prompt() else {
            return;
        };
        let n = prompt.actions().len();
        if n > 0 {
            self.selected_action_idx = (self.selected_action_idx + 1) % n;
        }
    }

    fn handle_arrow(&mut self, code: KeyCode) {
        let Some(prompt) = self.encounter.peek_prompt() else {
            return;
        };
        let action_count = prompt.actions().len();
        if action_count == 0 {
            return;
        }
        let actor_id = prompt.actor_id();
        let selected = prompt.actions().get(self.selected_action_idx).copied();
        let selected_is_move = selected.map(|a| a.name()) == Some("move");

        if selected_is_move {
            let Some(action) = selected else { return };
            let Some(actor) = self.encounter.actors.get(&actor_id) else {
                return;
            };
            let dest = match code {
                KeyCode::Up => actor.location() + Coordinate::new(0, 1),
                KeyCode::Down => actor.location() + Coordinate::new(0, -1),
                KeyCode::Left => actor.location() + Coordinate::new(-1, 0),
                KeyCode::Right => actor.location() + Coordinate::new(1, 0),
                _ => return,
            };
            let aei = ActionExecutionInfo::new(action, actor_id, None, Some(vec![dest]), None);
            if aei.validate(&self.encounter) {
                self.encounter.pop_prompt();
                self.encounter.push_action(aei);
                self.input_str.clear();
                self.tmp_message.clear();
            } else {
                self.tmp_message.clear();
                self.tmp_message.push_str("cannot move there");
            }
            return;
        }

        match code {
            KeyCode::Up => {
                self.selected_action_idx =
                    (self.selected_action_idx + action_count - 1) % action_count;
            }
            KeyCode::Down => {
                self.selected_action_idx = (self.selected_action_idx + 1) % action_count;
            }
            _ => {}
        }
    }

    fn handle_enter(&mut self) -> Tick {
        let trimmed = self.input_str.trim();
        if trimmed == "quit" {
            return Tick::Quit;
        }
        if !trimmed.is_empty() {
            let Some(prompt) = self.encounter.peek_prompt() else {
                return Tick::Continue;
            };
            match prompt.process_input(trimmed, &self.encounter) {
                Ok(aei) => {
                    self.encounter.pop_prompt();
                    self.encounter.push_action(aei);
                    self.input_str.clear();
                    self.tmp_message.clear();
                }
                Err(e) => {
                    self.tmp_message.clear();
                    self.tmp_message.push_str(&e.to_string());
                }
            }
            return Tick::Continue;
        }

        // Empty input: execute the currently selected action if it doesn't
        // need targeting (e.g. dash, skip).
        let Some(prompt) = self.encounter.peek_prompt() else {
            return Tick::Continue;
        };
        let actor_id = prompt.actor_id();
        let Some(action) = prompt.actions().get(self.selected_action_idx).copied() else {
            return Tick::Continue;
        };
        let aei = ActionExecutionInfo::new(action, actor_id, None, None, None);
        if aei.validate(&self.encounter) {
            self.encounter.pop_prompt();
            self.encounter.push_action(aei);
            self.input_str.clear();
            self.tmp_message.clear();
        } else {
            self.tmp_message.clear();
            let _ = write!(self.tmp_message, "'{}' needs a target", action.name());
        }
        Tick::Continue
    }
}

