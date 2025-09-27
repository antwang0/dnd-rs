pub mod actions;
pub mod actors;
pub mod conditions;
pub mod engine;
pub mod items;

use crate::engine::actor_gen::ActorGenParams;
use crate::engine::encounter::EncounterInstance;
use crate::engine::terrain_gen::TerrainGenParams;

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

    let terrain_params = TerrainGenParams {
        width: 40,
        height: 20,
        branch_depth: 8,
        branch_prob: 0.5,
    };

    let mut encounter_instance: EncounterInstance = EncounterInstance::from_params(
        &terrain_params,
        &ActorGenParams {
            cr_target: 1.0,
            n_teams: 2,
        },
    );

    while running {
        encounter_instance.process_stack();

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
                    Constraint::Length(terrain_params.width as u16),
                    Constraint::Min(1),
                ])
                .split(chunks[0]);
            encounter_instance.render_map(f, info_area[0]);
            encounter_instance.render_sideinfo(f, info_area[1]);

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
        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char(c) => input_str.push(c),
                        KeyCode::Backspace => {
                            input_str.pop();
                        }
                        KeyCode::Enter => {
                            let trimmed = input_str.trim();
                            if trimmed == "quit" {
                                running = false;
                            }
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
                            // game.process_command(input_str);
                        }
                        KeyCode::Esc => running = false,
                        _ => {}
                    }
                }
            }
        }
    }

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), terminal::LeaveAlternateScreen)?;
    Ok(())
}
