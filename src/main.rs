pub mod actions;
pub mod actors;
pub mod conditions;
pub mod engine;
pub mod items;

use crate::actions::action_template::ActionExecutionInfo;
use crate::engine::actor_gen::ActorGenParams;
use crate::engine::encounter::EncounterInstance;
use crate::engine::terrain_gen::TerrainGenParams;
use crate::engine::types::Coordinate;

use crossterm::{
    event::{self, Event, KeyCode},
    execute, terminal,
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::event::KeyEventKind,
    layout::{Constraint, Direction, Layout},
    text::{Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use std::io;

const HEIGHT: usize = 64;

fn main() -> io::Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut running = true;

    // TODO: move to handler obj
    let mut input_str: String = String::new();
    let mut tmp_message: String = String::new();
    let mut selected_action_idx: usize = 0;
    let mut last_actor_id: Option<usize> = None;

    let terrain_params = TerrainGenParams {
        width: 40,
        height: 20,
        branch_depth: 8,
        branch_prob: 0.5,
    };

    let seed: Option<u64> = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u64>().ok());

    let mut encounter_instance: EncounterInstance = EncounterInstance::from_params(
        &terrain_params,
        &ActorGenParams {
            cr_target: 1.0,
            n_teams: 2,
        },
        seed,
    )
    .expect("failed to create encounter");

    while running {
        encounter_instance.process_stack();

        // Reset action selection when the active actor changes (new turn).
        let curr_actor_id = encounter_instance.peek_prompt().map(|p| p.actor_id());
        if curr_actor_id != last_actor_id {
            selected_action_idx = 0;
            last_actor_id = curr_actor_id;
        }
        // Keep the selection in bounds in case the action list shrank.
        if let Some(prompt) = encounter_instance.peek_prompt() {
            let n = prompt.actions().len();
            if n > 0 {
                selected_action_idx %= n;
            } else {
                selected_action_idx = 0;
            }
        }

        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(HEIGHT.min(terrain_params.height) as u16 + 2), // Map area
                    Constraint::Length(3),                                            // Input area
                    Constraint::Length(3), // Temp message
                    Constraint::Min(1),    // Message log
                ])
                .split(f.area());

            // Map and current actions
            let info_area = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(terrain_params.width as u16 + 2),
                    Constraint::Min(1),
                ])
                .split(chunks[0]);
            encounter_instance.render_map(f, info_area[0]);
            encounter_instance.render_sideinfo(f, info_area[1], selected_action_idx);

            // Input
            let input_widget: Paragraph<'_> = Paragraph::new(input_str.as_str())
                .block(Block::default().borders(Borders::ALL).title("Input"));
            f.render_widget(input_widget, chunks[1]);

            let tmp_message_widget: Paragraph<'_> = Paragraph::new(tmp_message.as_str())
                .block(Block::default().borders(Borders::ALL).title("Message"));
            f.render_widget(tmp_message_widget, chunks[2]);

            // Messages
            let messages_text: Text = encounter_instance
                .messages()
                .iter()
                .rev()
                .take(5)
                .map(|m| Span::raw(m.clone()))
                .collect();
            let messages_widget = Paragraph::new(messages_text)
                .block(Block::default().borders(Borders::ALL).title("Log"));
            f.render_widget(messages_widget, chunks[3]);
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char(c) => input_str.push(c),
                KeyCode::Backspace => {
                    input_str.pop();
                }
                KeyCode::Tab => {
                    if let Some(prompt) = encounter_instance.peek_prompt() {
                        let n = prompt.actions().len();
                        if n > 0 {
                            selected_action_idx = (selected_action_idx + 1) % n;
                        }
                    }
                }
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                    let prompt_data = encounter_instance.peek_prompt().map(|p| {
                        let action_count = p.actions().len();
                        let selected = p.actions().get(selected_action_idx).copied();
                        (p.actor_id(), action_count, selected)
                    });
                    let Some((actor_id, action_count, selected)) = prompt_data else {
                        continue;
                    };
                    if action_count == 0 {
                        continue;
                    }
                    let selected_is_move = selected.map(|a| a.name()) == Some("move");

                    if selected_is_move {
                        let action = selected.unwrap();
                        let Some(actor) = encounter_instance.actors.get(&actor_id) else {
                            continue;
                        };
                        let loc = actor.location();
                        let dest = match key.code {
                            KeyCode::Up => loc + Coordinate::new(0, 1),
                            KeyCode::Down => loc + Coordinate::new(0, -1),
                            KeyCode::Left => loc + Coordinate::new(-1, 0),
                            KeyCode::Right => loc + Coordinate::new(1, 0),
                            _ => unreachable!(),
                        };
                        let aei = ActionExecutionInfo::new(
                            action,
                            actor_id,
                            None,
                            Some(vec![dest]),
                            None,
                        );
                        if aei.validate(&encounter_instance) {
                            encounter_instance.pop_prompt();
                            encounter_instance.push_action(aei);
                            input_str.clear();
                            tmp_message.clear();
                        } else {
                            tmp_message.clear();
                            tmp_message.push_str("cannot move there");
                        }
                    } else {
                        match key.code {
                            KeyCode::Up => {
                                selected_action_idx =
                                    (selected_action_idx + action_count - 1) % action_count;
                            }
                            KeyCode::Down => {
                                selected_action_idx = (selected_action_idx + 1) % action_count;
                            }
                            _ => {}
                        }
                    }
                }
                KeyCode::Enter => {
                    let trimmed = input_str.trim();
                    if trimmed == "quit" {
                        running = false;
                    } else if !trimmed.is_empty() {
                        if let Some(prompt) = encounter_instance.peek_prompt() {
                            match prompt.process_input(trimmed, &encounter_instance) {
                                Ok(aei) => {
                                    encounter_instance.pop_prompt();
                                    encounter_instance.push_action(aei);
                                    input_str.clear();
                                    tmp_message.clear();
                                }
                                Err(e) => {
                                    tmp_message.clear();
                                    tmp_message.push_str(&e.to_string());
                                }
                            }
                        }
                    } else {
                        // Empty input: execute the currently selected action if it
                        // doesn't need targeting (e.g. dash, skip).
                        let prompt_data = encounter_instance.peek_prompt().map(|p| {
                            let selected = p.actions().get(selected_action_idx).copied();
                            (p.actor_id(), selected)
                        });
                        if let Some((actor_id, Some(action))) = prompt_data {
                            let aei =
                                ActionExecutionInfo::new(action, actor_id, None, None, None);
                            if aei.validate(&encounter_instance) {
                                encounter_instance.pop_prompt();
                                encounter_instance.push_action(aei);
                                input_str.clear();
                                tmp_message.clear();
                            } else {
                                tmp_message.clear();
                                tmp_message
                                    .push_str(&format!("'{}' needs a target", action.name()));
                            }
                        }
                    }
                }
                KeyCode::Esc => running = false,
                _ => {}
            }
        }
    }

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), terminal::LeaveAlternateScreen)?;
    Ok(())
}
