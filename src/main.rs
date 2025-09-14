#[macro_use]
mod macros;

pub mod engine;
pub mod actions;
pub mod items;
pub mod conditions;
pub mod units;

use crate::engine::encounter::{EncounterInstance, construct_encounter_instance};
use crate::engine::terrain_gen::TerrainGenParams;

use std::io;
use crossterm::{
    event::{self, Event, KeyCode},
    execute, terminal,
};
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{event::{KeyEventKind}},
    Terminal,
    widgets::{Block, Borders, Paragraph},
    layout::{Layout, Constraint, Direction},
    text::{Span, Text},
};

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

    while running {
        let mut encounter_instance: EncounterInstance = construct_encounter_instance(
            &TerrainGenParams {
                width: 256,
                height: 64,
                branch_depth: 8,
                branch_prob: 0.5
            }
        );
        let map_text = Text::from(encounter_instance.ascii());

        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(HEIGHT as u16 + 2), // Map area
                    Constraint::Length(3),                 // Input area
                    Constraint::Min(1),                   // Message log
                ])
                .split(f.area());

            // Map
            let map_widget = Paragraph::new(map_text).block(Block::default().borders(Borders::ALL).title("Map"));
            f.render_widget(map_widget, chunks[0]);

            // Input
            let input_widget = Paragraph::new(input_str.as_str())
                .block(Block::default().borders(Borders::ALL).title("Input"));
            f.render_widget(input_widget, chunks[1]);

            // Messages
            let messages_text: Text = messages.iter().rev().take(5).map(|m| Span::raw(m.clone())).collect();
            let messages_widget = Paragraph::new(messages_text).block(Block::default().borders(Borders::ALL).title("Log"));
            f.render_widget(messages_widget, chunks[2]);
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char(c) => input_str.push(c),
                        KeyCode::Backspace => { input_str.pop(); },
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
