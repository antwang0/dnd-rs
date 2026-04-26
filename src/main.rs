pub mod actions;
pub mod actors;
pub mod ai;
pub mod app;
pub mod conditions;
pub mod engine;
pub mod items;
pub mod ui;

use crate::ai::SimpleAi;
use crate::app::{App, Tick};
use crate::engine::actor_gen::ActorGenParams;
use crate::engine::encounter::EncounterInstance;
use crate::engine::terrain_gen::TerrainGenParams;

use crossterm::{execute, terminal};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

fn main() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), terminal::LeaveAlternateScreen)?;
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let terrain_params = TerrainGenParams {
        width: 40,
        height: 20,
        branch_depth: 8,
        branch_prob: 0.5,
    };
    let actor_params = ActorGenParams {
        cr_target: 1.0,
        n_teams: 2,
    };
    let seed: Option<u64> = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u64>().ok());

    let encounter = EncounterInstance::from_params(&terrain_params, &actor_params, seed)
        .expect("failed to create encounter");

    let mut app = App::new(encounter, terrain_params.width, terrain_params.height);
    // Team 0 is the player by default; everyone else gets the baseline AI.
    // Swap in custom controllers via App::set_controller when you want
    // smarter or bespoke behavior.
    for team_id in 1..actor_params.n_teams {
        app.set_controller(team_id, Box::new(SimpleAi));
    }

    loop {
        app.refresh();
        terminal.draw(|f| app.draw(f))?;
        if matches!(app.pump_input()?, Tick::Quit) {
            return Ok(());
        }
    }
}
