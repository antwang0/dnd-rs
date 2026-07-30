pub mod actions;
pub mod actors;
pub mod ai;
pub mod app;
pub mod conditions;
pub mod engine;
pub mod items;
pub mod ui;

use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
use crate::actors::creatures::pc_template_families;
use crate::ai::SimpleAi;
use crate::app::{App, Tick};
use crate::engine::actor_gen::ActorGenParams;
use crate::engine::encounter::EncounterInstance;
use crate::engine::terrain_gen::TerrainGenParams;

use crossterm::{execute, terminal};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

/// Command-line configuration: which class the human plays and which
/// seed the encounter is generated from. Both optional, both
/// positional, and order-independent — the first argument that parses
/// as a number is the seed and everything else is the class name.
///
/// Order-independence is worth the few lines it costs because the two
/// arguments have no natural order to remember. `dnd-rs 42 four
/// elements monk` and `dnd-rs four elements monk 42` do the same thing,
/// and neither requires the player to look up which slot is which.
struct Cli {
    seed: Option<u64>,
    pc_template: &'static CreatureTemplate,
}

impl Cli {
    /// Resolve `args` (the raw argv tail, without the program name).
    ///
    /// A class name is matched case-insensitively against the names in
    /// `pc_template_families()`, joined across arguments so both
    /// `"four elements monk"` and `four elements monk` work — a shell
    /// user shouldn't have to quote a name that has spaces in it
    /// because the engine happened to store it that way.
    ///
    /// Returns `Err` with a listing rather than falling back silently:
    /// a typo'd class name that quietly started a Fighter game is worse
    /// than a refusal, because the player finds out several turns in.
    fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Self, String> {
        let mut seed = None;
        let mut name_parts: Vec<String> = Vec::new();
        for arg in args {
            match arg.parse::<u64>() {
                Ok(n) if seed.is_none() => seed = Some(n),
                _ => name_parts.push(arg),
            }
        }
        let pc_template = if name_parts.is_empty() {
            &*FIGHTER_TEMPLATE
        } else {
            let wanted = name_parts.join(" ");
            Self::find_template(&wanted).ok_or_else(|| Self::unknown_class_message(&wanted))?
        };
        Ok(Self { seed, pc_template })
    }

    /// Case-insensitive exact match on a template's display name.
    ///
    /// Exact rather than fuzzy on purpose: the names are close
    /// neighbours ("Cleric", "War Cleric", "Knowledge Cleric"), so a
    /// prefix or substring match would have to pick between them, and
    /// picking wrong hands the player a different character than the
    /// one they asked for without saying so.
    fn find_template(wanted: &str) -> Option<&'static CreatureTemplate> {
        pc_template_families()
            .into_iter()
            .flat_map(|(_family, templates)| templates)
            .find(|t| t.name.eq_ignore_ascii_case(wanted))
    }

    /// The error text for an unrecognized class name: what was asked
    /// for, then every option grouped by family so the reader can scan
    /// for the one they meant.
    fn unknown_class_message(wanted: &str) -> String {
        let mut msg = format!("unknown class {:?}. Available:\n", wanted);
        for (family, templates) in pc_template_families() {
            msg.push_str(&format!("  {}: ", family));
            let names: Vec<&str> = templates.iter().map(|t| t.name).collect();
            msg.push_str(&names.join(", "));
            msg.push('\n');
        }
        msg
    }
}

fn main() -> io::Result<()> {
    let cli = match Cli::parse(std::env::args().skip(1)) {
        Ok(cli) => cli,
        Err(msg) => {
            // Printed before the alternate screen is entered, so the
            // listing survives on the terminal instead of being wiped
            // by the TUI teardown.
            eprintln!("{}", msg);
            eprintln!("usage: dnd-rs [seed] [class name]");
            std::process::exit(2);
        }
    };

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, &cli);

    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), terminal::LeaveAlternateScreen)?;
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, cli: &Cli) -> io::Result<()> {
    let terrain_params = TerrainGenParams {
        width: 40,
        height: 20,
        branch_depth: 8,
        branch_prob: 0.5,
    };
    let actor_params = ActorGenParams {
        cr_target: 1.0,
        n_teams: 2,
        pc_template: Some(cli.pc_template),
        start_team: 0,
    };

    let encounter = EncounterInstance::from_params(&terrain_params, &actor_params, cli.seed)
        .expect("failed to create encounter");

    let n_teams = actor_params.n_teams;
    let mut app = App::new(encounter, terrain_params, actor_params);
    // Team 0 is the player by default; everyone else gets the baseline AI.
    // Swap in custom controllers via App::set_controller when you want
    // smarter or bespoke behavior.
    for team_id in 1..n_teams {
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

#[cfg(test)]
mod tests {
    use super::Cli;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        Cli::parse(args.iter().map(|s| s.to_string()))
    }

    /// No arguments keeps the historical behaviour exactly: a Fighter
    /// on a random seed.
    #[test]
    fn empty_args_default_to_a_random_seeded_fighter() {
        let cli = parse(&[]).expect("no args is always valid");
        assert!(cli.seed.is_none());
        assert_eq!(cli.pc_template.name, "Fighter");
    }

    /// The seed alone still works — it was the only argument the binary
    /// took before this, and a player with a seed in their shell
    /// history shouldn't find it broken.
    #[test]
    fn a_bare_number_is_still_the_seed() {
        let cli = parse(&["42"]).expect("a bare seed is valid");
        assert_eq!(cli.seed, Some(42));
        assert_eq!(cli.pc_template.name, "Fighter");
    }

    /// A multi-word class name works unquoted, and the seed can sit on
    /// either side of it.
    #[test]
    fn class_name_and_seed_are_order_independent() {
        for args in [
            &["7", "Four", "Elements", "Monk"][..],
            &["Four", "Elements", "Monk", "7"][..],
            &["Four", "Elements", "7", "Monk"][..],
        ] {
            let cli = parse(args).unwrap_or_else(|e| panic!("{:?}: {}", args, e));
            assert_eq!(cli.seed, Some(7));
            assert_eq!(cli.pc_template.name, "Four Elements Monk");
        }
    }

    /// Matching ignores case, because a shell user types lowercase.
    #[test]
    fn class_matching_is_case_insensitive() {
        let cli = parse(&["trickery", "cleric"]).expect("lowercase should resolve");
        assert_eq!(cli.pc_template.name, "Trickery Cleric");
    }

    /// A typo refuses rather than silently handing back a Fighter, and
    /// the refusal lists what the player could have meant.
    #[test]
    fn an_unknown_class_lists_the_alternatives() {
        let err = match parse(&["Tricky", "Cleric"]) {
            Err(e) => e,
            Ok(cli) => panic!("a typo should not start a {} game", cli.pc_template.name),
        };
        assert!(err.contains("Tricky Cleric"), "{}", err);
        assert!(err.contains("Trickery Cleric"), "{}", err);
        assert!(err.contains("wizard:"), "{}", err);
    }

    /// Every template in the registry is reachable by typing its own
    /// name — the same guarantee the prompt parser makes for actions,
    /// applied to the class picker. A template whose name collided with
    /// another family's would resolve to whichever came first, so the
    /// sweep also pins that no two do.
    #[test]
    fn every_registered_template_is_reachable_by_its_own_name() {
        for (family, templates) in crate::actors::creatures::pc_template_families() {
            for template in templates {
                let cli = parse(&template.name.split(' ').collect::<Vec<_>>())
                    .unwrap_or_else(|e| panic!("{} ({}): {}", template.name, family, e));
                assert_eq!(
                    cli.pc_template.name, template.name,
                    "{} ({}) resolved to the wrong template",
                    template.name, family
                );
            }
        }
    }
}
