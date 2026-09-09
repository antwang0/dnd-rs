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
use crate::engine::lighting::AmbientLight;
use crate::engine::weather::Weather;
use crate::engine::terrain_gen::TerrainGenParams;

use crossterm::{execute, terminal};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

/// Command-line configuration: which class the human plays, which seed
/// the encounter is generated from, and what the lights are doing. All
/// optional, all positional, and order-independent — the first argument
/// that parses as a number is the seed, a `--flag` naming an ambient
/// light sets the lighting, and everything else is the class name.
///
/// Order-independence is worth the few lines it costs because the
/// arguments have no natural order to remember. `dnd-rs 42 four
/// elements monk` and `dnd-rs four elements monk 42` do the same thing,
/// and neither requires the player to look up which slot is which.
struct Cli {
    seed: Option<u64>,
    pc_template: &'static CreatureTemplate,
    /// What the sky is doing — see `crate::engine::lighting`. Defaults
    /// to the lit board the game has always been played on; `dark`
    /// turns the lights out and makes darkvision, torches and the Light
    /// cantrip matter, and `daylight` puts the fight under an open sun
    /// where the kobolds and the drow flinch.
    ambient: AmbientLight,
    /// What the sky is *doing* — see `crate::engine::weather`. Its own
    /// axis rather than a fifth light level, because the two compose: a
    /// rainy noon is `daylight` and `--rain` at once. Defaults to the
    /// still air every fight was fought in before the weather layer.
    weather: Weather,
}

/// What `Cli::parse` decided the arguments meant.
enum Invocation {
    /// Start a game with this configuration.
    Play(Cli),
    /// The player asked what the arguments are. Print `text` and exit 0
    /// — a help request is a thing the program was asked to do and did,
    /// not a mistake, so it doesn't belong on stderr behind an exit 2.
    Help(String),
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
    fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Invocation, String> {
        let mut seed = None;
        let mut ambient = AmbientLight::default();
        let mut weather = Weather::default();
        let mut name_parts: Vec<String> = Vec::new();
        for arg in args {
            // Checked before the number parse and before the name
            // collection, so `--help` doesn't end up joined into a class
            // name and answered with "unknown class \"--help\"" — which
            // is what happened before, on stderr, behind exit code 2.
            if matches!(arg.as_str(), "-h" | "--help" | "help") {
                return Ok(Invocation::Help(Self::help_message()));
            }
            // Checked before the class-name collection for the same
            // reason `--help` is: a lighting request is not a class
            // name, and letting it fall through would answer it with
            // "unknown class".
            //
            // The `--` is *required*, unlike the seed and the class
            // name, and that is not an inconsistency — it is what keeps
            // the three from colliding. The class list already contains
            // a Sun Soul Monk and a Gloom Stalker Ranger, so a parser
            // that accepted bare `sun` or `gloom` as a light level
            // would quietly eat the first word of a class the player
            // asked for and then refuse the rest of it. A prefix that
            // no class name can start with is the cheapest way to keep
            // the namespaces apart.
            if let Some(flagless) = arg.strip_prefix("--")
                && let Some(a) = AmbientLight::parse(flagless)
            {
                ambient = a;
                continue;
            }
            // The weather, on the same `--` lane and for the same reason:
            // `rain` and `wind` are not class names, and the prefix is
            // what keeps them from being read as one. A second lane
            // rather than a second value on the first, because the two
            // axes compose — `--daylight --rain` is a legal and
            // meaningful pair, and a single `ambient` slot could only
            // hold whichever came last.
            if let Some(flagless) = arg.strip_prefix("--")
                && let Some(w) = Weather::parse(flagless)
            {
                weather = w;
                continue;
            }
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
        Ok(Invocation::Play(Self {
            seed,
            pc_template,
            ambient,
            weather,
        }))
    }

    /// Usage plus the full class listing. Shares
    /// `class_listing` with the unknown-class error so the two can't
    /// disagree about what is playable.
    fn help_message() -> String {
        let mut msg =
            String::from("usage: dnd-rs [seed] [class name] [--light-level] [--weather]\n\n");
        msg.push_str("Every argument is optional and order-independent: the first\n");
        msg.push_str("argument that parses as a number is the seed, a --flag sets\n");
        msg.push_str("the light level or the weather, and everything else is the\n");
        msg.push_str("class name. With no seed, one is drawn and printed in the\n");
        msg.push_str("initiative panel so the encounter can be replayed.\n\n");
        msg.push_str(&Self::lighting_listing());
        msg.push('\n');
        msg.push_str(&Self::weather_listing());
        msg.push('\n');
        msg.push_str("Classes:\n");
        msg.push_str(&Self::class_listing());
        msg
    }

    /// The ambient-light options, read off `AmbientLight::NAMES` rather
    /// than written out here so the help text cannot advertise a
    /// spelling the parser refuses — the same single-source discipline
    /// `class_listing` keeps for the class names.
    fn lighting_listing() -> String {
        let flags: Vec<String> = AmbientLight::NAMES
            .iter()
            .map(|n| format!("--{}", n))
            .collect();
        format!(
            "Light levels ({} is the default):\n  {}\n",
            AmbientLight::default().label(),
            flags.join(", ")
        )
    }

    /// The weather options, read off `Weather::NAMES` for the reason
    /// `lighting_listing` reads off `AmbientLight::NAMES`: the help text
    /// must not be able to advertise a spelling the parser refuses.
    fn weather_listing() -> String {
        let flags: Vec<String> = Weather::NAMES.iter().map(|n| format!("--{}", n)).collect();
        format!(
            "Weather ({} is the default):\n  {}\n",
            Weather::default().label(),
            flags.join(", ")
        )
    }

    /// Every playable template, one line per class family. The shared
    /// half of the help text and the unknown-class error.
    fn class_listing() -> String {
        let mut out = String::new();
        for (family, templates) in pc_template_families() {
            let names: Vec<&str> = templates.iter().map(|t| t.name).collect();
            out.push_str(&format!("  {}: {}\n", family, names.join(", ")));
        }
        out
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
        format!("unknown class {:?}. Available:\n{}", wanted, Self::class_listing())
    }
}

fn main() -> io::Result<()> {
    let cli = match Cli::parse(std::env::args().skip(1)) {
        Ok(Invocation::Play(cli)) => cli,
        Ok(Invocation::Help(text)) => {
            print!("{}", text);
            return Ok(());
        }
        Err(msg) => {
            // Printed before the alternate screen is entered, so the
            // listing survives on the terminal instead of being wiped
            // by the TUI teardown.
            eprintln!("{}", msg);
            eprintln!("usage: dnd-rs [seed] [class name] [--light-level] [--weather]");
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

    let mut encounter = EncounterInstance::from_params(&terrain_params, &actor_params, cli.seed)
        .expect("failed to create encounter");
    encounter.set_ambient_light(cli.ambient);
    encounter.set_weather(cli.weather);

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
    use super::{AmbientLight, Cli, Invocation, Weather};

    /// Parse and unwrap to a playable configuration. Every test below
    /// but the help ones expects `Play`, so the unwrap is the assertion.
    fn parse(args: &[&str]) -> Result<Cli, String> {
        match Cli::parse(args.iter().map(|s| s.to_string()))? {
            Invocation::Play(cli) => Ok(cli),
            Invocation::Help(_) => panic!("{:?} is not a help request", args),
        }
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

    /// A help flag is answered with help, on stdout, at exit 0.
    ///
    /// It used to fall through to the class matcher: `--help` collected
    /// into `name_parts`, missed every template, and came back as
    /// `unknown class "--help"` on stderr behind exit code 2. The
    /// listing was right there in the error, which is why it went
    /// unnoticed — the output was useful and the framing was wrong.
    #[test]
    fn a_help_flag_is_answered_with_help_and_not_with_an_error() {
        for flag in ["-h", "--help", "help"] {
            let text = match Cli::parse([flag.to_string()]) {
                Ok(Invocation::Help(t)) => t,
                Ok(Invocation::Play(cli)) => {
                    panic!("{} started a {} game", flag, cli.pc_template.name)
                }
                Err(e) => panic!("{} was refused: {}", flag, e),
            };
            assert!(text.contains("usage:"), "{}: {}", flag, text);
            assert!(text.contains("Trickery Cleric"), "{}: {}", flag, text);
        }
    }

    /// Help wins over the rest of the line rather than being shadowed by
    /// it — asking for help while also naming a class is still asking
    /// for help, and a valid class name would otherwise have started a
    /// game the player didn't ask to play.
    #[test]
    fn help_wins_over_the_arguments_beside_it() {
        assert!(matches!(
            Cli::parse(["7".to_string(), "--help".to_string(), "Champion".to_string()]),
            Ok(Invocation::Help(_))
        ));
    }

    /// The light-level flags are recognised, from either side of the
    /// other arguments, and the default stands when none is given.
    #[test]
    fn a_light_level_flag_sets_the_ambient_and_leaves_the_rest_alone() {
        let cli = parse(&[]).expect("no args is always valid");
        assert_eq!(cli.ambient, AmbientLight::default());
        for args in [
            &["--dark", "7", "Champion"][..],
            &["7", "--dark", "Champion"][..],
            &["7", "Champion", "--dark"][..],
        ] {
            let cli = parse(args).unwrap_or_else(|e| panic!("{:?}: {}", args, e));
            assert_eq!(cli.ambient, AmbientLight::Darkness, "{:?}", args);
            assert_eq!(cli.seed, Some(7), "{:?}", args);
            assert_eq!(cli.pc_template.name, "Champion", "{:?}", args);
        }
        assert_eq!(
            parse(&["--daylight"]).expect("a bare flag").ambient,
            AmbientLight::Daylight
        );
    }

    /// The flag requires its `--`, and this is the test that says why:
    /// the class list contains a **Sun** Soul Monk and a **Gloom**
    /// Stalker Ranger, and a parser that took bare light-level words
    /// would eat the first word of each and then refuse the rest.
    ///
    /// Pinned as a pair — the class still resolves, and the ambient is
    /// untouched — because either half failing on its own is the bug.
    #[test]
    fn a_class_whose_name_starts_with_a_light_word_still_resolves() {
        for (args, name) in [
            (&["Sun", "Soul", "Monk"][..], "Sun Soul Monk"),
            (&["Gloom", "Stalker", "Ranger"][..], "Gloom Stalker Ranger"),
            (&["Light", "Cleric"][..], "Light Cleric"),
        ] {
            let cli = parse(args).unwrap_or_else(|e| panic!("{:?}: {}", args, e));
            assert_eq!(cli.pc_template.name, name);
            assert_eq!(cli.ambient, AmbientLight::default(), "{:?}", args);
        }
    }

    /// The weather flags are recognised on the same lane as the light
    /// levels, and the two **compose** — which is the whole reason they
    /// are separate axes rather than four more values on one enum. A
    /// rainy noon is a real sky.
    #[test]
    fn a_weather_flag_sets_the_sky_and_composes_with_the_light() {
        assert_eq!(parse(&[]).expect("no args is always valid").weather, Weather::Calm);
        for args in [
            &["--wind", "7", "Champion"][..],
            &["7", "--wind", "Champion"][..],
            &["7", "Champion", "--wind"][..],
        ] {
            let cli = parse(args).unwrap_or_else(|e| panic!("{:?}: {}", args, e));
            assert_eq!(cli.weather, Weather::StrongWind, "{:?}", args);
            assert_eq!(cli.seed, Some(7), "{:?}", args);
            assert_eq!(cli.pc_template.name, "Champion", "{:?}", args);
        }
        let both = parse(&["--daylight", "--rain"]).expect("two axes, two flags");
        assert_eq!(both.ambient, AmbientLight::Daylight);
        assert_eq!(both.weather, Weather::HeavyPrecipitation);
        // …and the light flag alone leaves the sky still, so neither
        // axis can quietly set the other.
        assert_eq!(
            parse(&["--dark"]).expect("a bare flag").weather,
            Weather::Calm
        );
    }

    /// An unrecognized `--flag` is still refused as a class name rather
    /// than silently ignored — the same bargain the unknown-class error
    /// makes, and the reason a typo'd `--darkk` does not quietly start
    /// a lit game.
    #[test]
    fn an_unrecognized_flag_is_refused_rather_than_ignored() {
        assert!(parse(&["--darkk"]).is_err());
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
