#[macro_use]
mod macros;

pub mod actions;
pub mod actors;
pub mod conditions;
pub mod engine;
pub mod items;

use crate::engine::encounter::EncounterInstance;
use crate::engine::terrain_gen::TerrainGenParams;
use crate::engine::{actor_gen::ActorGenParams, terrain};

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
    let mut messages: Vec<String> = Vec::new();

    let terrain_params = TerrainGenParams {
        width: 64,
        height: 32,
        branch_depth: 8,
        branch_prob: 0.5,
    };

    let mut encounter_instance: EncounterInstance = EncounterInstance::from_params(
        &terrain_params,
        &ActorGenParams {
            cr_target: 5.0,
            n_teams: 2,
        },
    );

    while running {
        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(HEIGHT.min(terrain_params.height) as u16 + 2), // Map area
                    Constraint::Length(3),                                            // Input area
                    Constraint::Min(1),                                               // Message log
                ])
                .split(f.area());

            // Map
            encounter_instance.render(f, chunks[0]);

            // Input
            let input_widget = Paragraph::new(input_str.as_str())
                .block(Block::default().borders(Borders::ALL).title("Input"));
            f.render_widget(input_widget, chunks[1]);

            // Messages
            let messages_text: Text = messages
                .iter()
                .rev()
                .take(5)
                .map(|m| Span::raw(m.clone()))
                .collect();
            let messages_widget = Paragraph::new(messages_text)
                .block(Block::default().borders(Borders::ALL).title("Log"));
            f.render_widget(messages_widget, chunks[2]);
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
                            if input_str.trim() == "quit" {
                                running = false;
                            }
                            // game.process_command();
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
