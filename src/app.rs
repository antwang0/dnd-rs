use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io;
use std::time::Duration;

use crate::actions::action_template::{ActionExecutionInfo, TargetingSchema};
use crate::actors::actor_template::{ActorInstance, HpState};
use crate::ai::{Controller, ControllerDecision, PlayerController};
use crate::engine::actor_gen::ActorGenParams;
use crate::engine::encounter::EncounterInstance;
use crate::engine::terrain_gen::TerrainGenParams;
use crate::engine::types::Coordinate;

const POLL_INTERVAL: Duration = Duration::from_millis(200);
/// Cap how many AI decisions we resolve per frame so a runaway controller
/// can't lock the UI thread.
const MAX_AI_STEPS_PER_TICK: usize = 64;

/// Outcome of handling a single key press.
pub enum Tick {
    Continue,
    Quit,
}

/// Holds the loop's mutable UI/input state so main.rs can stay slim.
pub struct App {
    pub encounter: EncounterInstance,
    /// Per-team turn driver. Teams without an entry default to the player
    /// controller (i.e. the App pumps the keyboard for them).
    controllers: HashMap<usize, Box<dyn Controller>>,
    default_controller: Box<dyn Controller>,
    /// Saved generator params so we can spin up the next encounter when the
    /// player long-rests after a victory.
    terrain_params: TerrainGenParams,
    actor_params: ActorGenParams,
    encounter_number: u32,
    map_width: u16,
    map_height: u16,
    input_str: String,
    tmp_message: String,
    /// Why the last "R: long rest & continue" press did nothing, or
    /// `None` if it hasn't failed.
    ///
    /// The keypress used to discard `start_next_encounter`'s return
    /// value, so a generation failure left the victory banner up, the
    /// board unchanged and the player pressing R at a screen that
    /// silently refused. Generation *can* fail — the difficulty ramp is
    /// unbounded and the map is a fixed size, so a deep enough run
    /// eventually asks for more creatures than the board has anchors
    /// for — and "nothing happened, with no explanation" is the worst
    /// available way to say so.
    continue_error: Option<String>,
    selected_action_idx: usize,
    /// Index into the cached `valid_targets` list. Only meaningful when the
    /// selected action's schema is `SingleActor`.
    selected_target_idx: usize,
    /// Recomputed each refresh from the active prompt + selected action.
    /// Holds actor ids of every enemy the selected action validates against.
    valid_targets: Vec<usize>,
    last_actor_id: Option<usize>,
    last_action_idx: Option<usize>,
}

impl App {
    pub fn new(
        encounter: EncounterInstance,
        terrain_params: TerrainGenParams,
        actor_params: ActorGenParams,
    ) -> Self {
        let map_width = terrain_params.width;
        let map_height = terrain_params.height;
        Self {
            encounter,
            controllers: HashMap::new(),
            default_controller: Box::new(PlayerController),
            terrain_params,
            actor_params,
            encounter_number: 1,
            map_width: u16::try_from(map_width).unwrap_or(u16::MAX),
            map_height: u16::try_from(map_height).unwrap_or(u16::MAX),
            input_str: String::new(),
            tmp_message: String::new(),
            continue_error: None,
            selected_action_idx: 0,
            selected_target_idx: 0,
            valid_targets: Vec::new(),
            last_actor_id: None,
            last_action_idx: None,
        }
    }

    pub fn encounter_number(&self) -> u32 {
        self.encounter_number
    }

    /// CR budget for the *next* encounter: linear ramp on `encounter_number`.
    /// The first fight uses `actor_params.cr_target` as-is; each subsequent
    /// encounter adds half of that base on top, so encounter 2 is 1.5×,
    /// encounter 3 is 2×, etc. Linear keeps the curve readable; aggressive
    /// enough that fights get noticeably tougher within a session, gentle
    /// enough that the player isn't insta-overrun by encounter 4.
    fn scaled_cr_target(&self) -> f32 {
        let next_n = self.encounter_number + 1;
        self.actor_params.cr_target * (1.0 + 0.5 * (next_n - 1) as f32)
    }

    /// Advance to the next encounter: take surviving team-0 actors out of
    /// the current fight, long-rest them, and spawn them into a freshly
    /// generated map alongside new enemies. Resets per-encounter UI state.
    /// Returns false if no team-0 survivors exist (game over) or generation
    /// fails — in either case the existing encounter is left untouched.
    ///
    /// The two false cases are not the same thing and no longer read the
    /// same. A generation failure records its reason in `continue_error`,
    /// which the banner prints, so the player is told the board could
    /// not be built rather than being left to conclude the key is
    /// broken.
    pub fn start_next_encounter(&mut self) -> bool {
        let pcs: Vec<ActorInstance> = self
            .encounter
            .actors
            .values()
            .filter(|a| a.team() == self.actor_params.start_team && a.hp_state() != HpState::Dead)
            .cloned()
            .collect();
        if pcs.is_empty() {
            return false;
        }
        let mut rested: Vec<ActorInstance> = pcs;
        // The full rest, off the outgoing encounter's seeded roller —
        // hit points and slots back, staves topped up, and the XP the
        // party banked in the last room cashed in for levels. This used
        // to be a bare `ActorInstance::long_rest` per PC, which is the
        // first of those three and neither of the others: a run could
        // bank every point of XP the dungeon paid out and never level,
        // because the only long rest a playthrough reaches is this one.
        // See `EncounterInstance::long_rest_party`.
        let rest_lines = self.encounter.long_rest_party(&mut rested);
        // Bump the CR budget for this fight only; keep `actor_params` as the
        // base so future scaling stays anchored to the original difficulty.
        let mut scaled_params = self.actor_params.clone();
        scaled_params.cr_target = self.scaled_cr_target();
        // The lights carry over. The ambient light belongs to the
        // *game* the player started, not to one encounter of it, so a
        // `--dark` run stays dark across the whole dungeon rather than
        // walking into daylight the moment the party takes a rest.
        // Read off the encounter that is ending, which is the only
        // place it has been recorded since `main` set it.
        let ambient = self.encounter.ambient_light();
        match EncounterInstance::with_pcs(&self.terrain_params, &scaled_params, None, rested) {
            Ok(mut next) => {
                next.set_ambient_light(ambient);
                // Into the *new* log, not the old one. The rest happens
                // between two maps and the encounter it happened in is
                // about to be dropped, so a level-up announced there is
                // one the player never sees.
                for line in rest_lines {
                    next.log(line);
                }
                self.encounter = next;
                self.encounter_number += 1;
                self.input_str.clear();
                self.tmp_message.clear();
                self.selected_action_idx = 0;
                self.selected_target_idx = 0;
                self.valid_targets.clear();
                self.last_actor_id = None;
                self.last_action_idx = None;
                self.continue_error = None;
                true
            }
            Err(err) => {
                self.continue_error = Some(err.to_string());
                false
            }
        }
    }

    /// Assigns a controller to a team. Replaces any existing one.
    pub fn set_controller(&mut self, team_id: usize, controller: Box<dyn Controller>) {
        self.controllers.insert(team_id, controller);
    }

    fn controller_for(&self, team_id: usize) -> &dyn Controller {
        self.controllers
            .get(&team_id)
            .map_or(self.default_controller.as_ref(), |c| c.as_ref())
    }

    /// Resync derived UI state with the engine — call once per loop iteration
    /// before drawing. Drives AI controllers to completion so the player only
    /// sees the engine when it's their turn (or when the encounter ends).
    pub fn refresh(&mut self) {
        for _ in 0..MAX_AI_STEPS_PER_TICK {
            self.encounter.process_stack();
            if self.encounter.is_complete() {
                break;
            }

            let Some(prompt) = self.encounter.peek_prompt() else {
                break;
            };
            let actor_id = prompt.actor_id();
            let Some(team) = self.encounter.actors.get(&actor_id).map(|a| a.team()) else {
                break;
            };

            match self.controller_for(team).decide(&self.encounter, actor_id) {
                ControllerDecision::AwaitInput => break,
                ControllerDecision::Act(aei) => {
                    self.encounter.pop_prompt();
                    self.encounter.push_action(aei);
                }
            }
        }

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

        // Recompute valid targets when the active actor or action changes,
        // and clamp the target cursor when the list shrinks.
        if Some(self.selected_action_idx) != self.last_action_idx
            || curr_actor_id != self.last_actor_id
        {
            self.selected_target_idx = 0;
        }
        self.valid_targets = self.compute_valid_targets();
        if !self.valid_targets.is_empty() {
            self.selected_target_idx %= self.valid_targets.len();
        } else {
            self.selected_target_idx = 0;
        }
        self.last_action_idx = Some(self.selected_action_idx);
    }

    fn selected_action(&self) -> Option<&'static (dyn crate::actions::action_template::Action + Send + Sync)> {
        self.encounter
            .peek_prompt()?
            .actions()
            .get(self.selected_action_idx)
            .copied()
    }

    fn compute_valid_targets(&self) -> Vec<usize> {
        let Some(prompt) = self.encounter.peek_prompt() else {
            return Vec::new();
        };
        let Some(action) = self.selected_action() else {
            return Vec::new();
        };
        if !matches!(action.targeting_schema(), TargetingSchema::SingleActor) {
            return Vec::new();
        }
        let caster_id = prompt.actor_id();
        let mut targets: Vec<usize> = self
            .encounter
            .actors
            .keys()
            .filter_map(|id| {
                if *id == caster_id {
                    return None;
                }
                let aei = ActionExecutionInfo::new(action, caster_id, Some(vec![*id]), None, None);
                if aei.validate(&self.encounter) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        targets.sort_unstable();
        targets
    }

    fn highlighted_target(&self) -> Option<usize> {
        self.valid_targets.get(self.selected_target_idx).copied()
    }

    pub fn draw(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(self.map_height + 2), // Map area
                Constraint::Length(3),                   // Input area
                Constraint::Length(3),                   // Temp message
                Constraint::Min(1),                      // Message log
                Constraint::Length(1),                   // Help bar
            ])
            .split(f.area());

        let info_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(self.map_width + 2), Constraint::Min(1)])
            .split(chunks[0]);

        let highlighted = self.highlighted_target();
        let preview = self.previewed_area();
        crate::ui::render_map(&self.encounter, f, info_area[0], highlighted, &preview);
        crate::ui::render_sideinfo(
            &self.encounter,
            f,
            info_area[1],
            self.selected_action_idx,
        );

        let input_widget =
            Paragraph::new(self.input_str.as_str())
                .block(Block::default().borders(Borders::ALL).title("Input"));
        f.render_widget(input_widget, chunks[1]);

        let banner = self.completion_banner();
        let target_line = self.target_line();
        let msg_text = banner
            .as_deref()
            .or(target_line.as_deref())
            .unwrap_or(self.tmp_message.as_str());
        let tmp_message_widget = Paragraph::new(msg_text)
            .block(Block::default().borders(Borders::ALL).title("Message"));
        f.render_widget(tmp_message_widget, chunks[2]);

        let messages_lines: Vec<ratatui::text::Line<'static>> = self
            .encounter
            .messages()
            .iter()
            .rev()
            .take(5)
            .map(|m| crate::ui::style_log_line(m))
            .collect();
        let messages_widget = Paragraph::new(messages_lines)
            .block(Block::default().borders(Borders::ALL).title("Log"));
        f.render_widget(messages_widget, chunks[3]);

        let help = self.help_hint();
        let help_widget = Paragraph::new(help)
            .style(ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray));
        f.render_widget(help_widget, chunks[4]);
    }

    /// Context-sensitive footer string. Schema of the currently selected
    /// action drives which keys are mentioned, so the player only sees
    /// hints relevant to what they can do *right now*.
    fn help_hint(&self) -> String {
        let current_cr = self.actor_params.cr_target * (1.0 + 0.5 * (self.encounter_number - 1) as f32);
        let prefix = format!(
            " [Encounter {} | CR {:.1}]",
            self.encounter_number, current_cr
        );
        if self.encounter.is_complete() {
            if self.player_may_continue() {
                return format!("{}  R: long rest & continue  Esc: quit", prefix);
            }
            return format!("{}  Esc: quit", prefix);
        }
        let Some(action) = self.selected_action() else {
            return format!("{}  (waiting for prompt) | Esc: quit", prefix);
        };
        let body = match action.targeting_schema() {
            TargetingSchema::SinglePoint => {
                "\u{2191}\u{2193}\u{2190}\u{2192}: step  Tab: cycle action  End: end turn  Esc: quit"
            }
            TargetingSchema::SingleActor => {
                "\u{2190}\u{2192}: target  \u{2191}\u{2193}: cycle action  Tab: cycle  Enter: confirm  End: end turn  Esc: quit"
            }
            TargetingSchema::Burst { .. }
            | TargetingSchema::Cone { .. }
            | TargetingSchema::Line { .. } => {
                "type 'X,Y' point + Enter  \u{2191}\u{2193}: cycle action  Tab: cycle  End: end turn  Esc: quit"
            }
            TargetingSchema::NoArgs | TargetingSchema::Custom => {
                "\u{2191}\u{2193}: cycle action  Tab: cycle  Enter: confirm  End: end turn  Esc: quit"
            }
        };
        format!("{}  {}", prefix, body)
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
        // Once the encounter is decided, the only keys that matter are
        // quit and — for a player who is still standing, whether they
        // won or the fight was called a draw — "R" to long-rest into
        // the next encounter. Everything else is ignored so stray input doesn't
        // get buffered into the input box behind the banner.
        if self.encounter.is_complete() {
            return match key.code {
                KeyCode::Esc => Tick::Quit,
                KeyCode::Char('r') | KeyCode::Char('R') if self.player_may_continue() => {
                    self.start_next_encounter();
                    Tick::Continue
                }
                _ => Tick::Continue,
            };
        }
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
            KeyCode::End => self.end_turn(),
            KeyCode::Esc => return Tick::Quit,
            _ => {}
        }
        Tick::Continue
    }

    /// Shortcut: invoke the active actor's Skip action. Lets the player end
    /// their turn without navigating to "skip" in the action list. No-op if
    /// there's no prompt or the actor lacks a Skip action.
    fn end_turn(&mut self) {
        let Some(prompt) = self.encounter.peek_prompt() else {
            return;
        };
        let actor_id = prompt.actor_id();
        let Some(skip) = prompt.actions().iter().find(|a| a.name() == "skip").copied() else {
            return;
        };
        let aei = ActionExecutionInfo::new(skip, actor_id, None, None, None);
        if aei.validate(&self.encounter) {
            self.encounter.pop_prompt();
            self.encounter.push_action(aei);
            self.input_str.clear();
            self.tmp_message.clear();
        }
    }

    /// True once the fight is over *and* the player walked out of it —
    /// the condition for offering "R: long rest & continue".
    ///
    /// Three call sites read this: the banner that offers the key, the
    /// status line that advertises it, and the handler that honours it.
    /// All three used to compare `winning_team()` against the player's
    /// team by hand, which agreed with itself only as long as "the
    /// player survived" and "the player won" were the same sentence.
    /// A draw makes them different: the engine can end a fight with
    /// several teams still standing (see
    /// `EncounterInstance::is_stalemate`), and a player who is alive at
    /// the end of one has no more reason to be sent to the quit prompt
    /// than a player who won.
    ///
    /// Asking after the player's own team rather than after the winner
    /// is also the more direct question, and it answers the win case
    /// identically: a victory is by definition the state where the
    /// player's team is the only one left standing.
    fn player_may_continue(&self) -> bool {
        self.encounter.is_complete()
            && self
                .encounter
                .actors
                .values()
                .any(|a| a.team() == self.actor_params.start_team && a.is_combat_active())
    }

    /// What the banner says once the fight is over.
    ///
    /// `winning_team` answers `None` for two completely different
    /// endings, and the banner used to print the same sentence for
    /// both. "No survivors" is right for the one where everybody is
    /// dead and badly wrong for the other — a draw, where the board is
    /// still full of people who simply could not finish each other.
    /// The engine ends those two ways deliberately (see
    /// `EncounterInstance::is_stalemate`, both halves), so the player
    /// deserves to be told which one they are looking at, not least
    /// because the mutual-annihilation ending is genuinely rare and
    /// reads as a bug when it is announced over a board of live
    /// creatures.
    ///
    /// A draw offers the same "long rest and continue" the win does.
    /// The player did not lose — nobody did — and the alternative is
    /// making them quit over a fight the engine called off.
    fn completion_banner(&self) -> Option<String> {
        let banner = self.completion_verdict()?;
        // A failed continue is appended rather than replacing the
        // verdict: the fight is still over and still won, and the player
        // needs both facts to decide whether to press R again or quit.
        match &self.continue_error {
            Some(err) => Some(format!(
                "{} [could not build the next encounter: {}]",
                banner, err
            )),
            None => Some(banner),
        }
    }

    /// The verdict half of the banner — who won, and what the player may
    /// do about it. Split from `completion_banner` so the failed-continue
    /// note has something to hang off without every arm below having to
    /// know about it.
    fn completion_verdict(&self) -> Option<String> {
        if !self.encounter.is_complete() {
            return None;
        }
        let player_team = self.actor_params.start_team;
        match self.encounter.winning_team() {
            Some(team) if team == player_team => Some(format!(
                "Encounter {} cleared! (R: long rest & continue, Esc: quit)",
                self.encounter_number
            )),
            Some(team) => Some(format!(
                "Team {} wins. You fall in encounter {}. (Esc to quit)",
                team, self.encounter_number
            )),
            // Somebody is still standing, so this is the draw rather
            // than the wipe. Read off the board rather than off
            // `is_stalemate`, because the question the sentence answers
            // is "is anyone left?" and that is what to ask.
            None if self
                .encounter
                .actors
                .values()
                .any(|a| a.is_combat_active()) =>
            {
                Some(format!(
                    "Encounter {} ends in a stalemate — nobody could finish it. \
                     (R: long rest & continue, Esc: quit)",
                    self.encounter_number
                ))
            }
            None => Some(format!(
                "No survivors of encounter {}. (Esc to quit)",
                self.encounter_number
            )),
        }
    }

    /// The tiles the action the player has selected would cover, if
    /// they confirmed the coordinate they are currently typing.
    ///
    /// Areas are aimed by typing `X,Y` into a text box, which was
    /// survivable while every area in the engine was a sphere centred
    /// on that tile — the player could read the radius off the prompt
    /// and count squares. It is not survivable for a cone: the shape
    /// depends on where the *caster* is standing as much as on the tile
    /// named, its far end is twenty-four tiles wide, and no amount of
    /// counting tells you whether the fighter in the doorway is inside
    /// it. So the map draws the answer while the player types it.
    ///
    /// Empty whenever there is nothing to preview — no prompt, a
    /// non-area action, an unparseable coordinate — which is the common
    /// case and costs the renderer a `HashSet::is_empty`.
    ///
    /// The coordinate is the *last* whitespace-separated token of the
    /// input that parses as one, because the prompt's own parser
    /// accepts an action name in front of it (`cone of cold 20,14`) and
    /// a cast level on either side of it (`fireball +2 20,14`). Parsed
    /// relative to the actor, so the `r3u2` forms `parse_coord`
    /// supports preview too.
    ///
    /// And when the input names an action, *that* is the action
    /// previewed — resolved through the same `Prompt::resolve_action`
    /// the cast will use, rather than through the arrow-key selection.
    /// The two disagree exactly when the player types a spell's name
    /// while the highlight sits on a different one, and a preview that
    /// read the highlight would shade a sixty-foot cone and then cast a
    /// fifteen-foot one. Falls back to the selection when the input is
    /// a bare coordinate, which is the ordinary case, the one the
    /// selection is for, and the one `bare_point_cast` casts.
    ///
    /// Shades nothing when the cast would be refused, which it asks by
    /// building the cast and validating it rather than by re-deriving
    /// the conditions. The shading is a promise about what the Enter
    /// key will do, so the only way for it to stay honest is to ask the
    /// Enter key's own question: a cone aimed past its own length is
    /// out of reach, an unaffordable slot is unaffordable, and neither
    /// should be drawn as though it were about to happen.
    fn previewed_area(&self) -> std::collections::HashSet<Coordinate> {
        let empty = std::collections::HashSet::new();
        let Some(prompt) = self.encounter.peek_prompt() else {
            return empty;
        };
        let Some(actor) = self.encounter.actors.get(&prompt.actor_id()) else {
            return empty;
        };
        let tokens: Vec<&str> = self.input_str.split_whitespace().collect();
        let Some((index, aim)) = tokens.iter().enumerate().rev().find_map(|(i, t)| {
            crate::engine::util::parse_coord(t, actor.location()).map(|c| (i, c))
        }) else {
            return empty;
        };
        // Everything before the tile is the action, if there is
        // anything — `resolve_action` takes the longest prefix that
        // names one, so a cast level sitting between the name and the
        // tile is simply not part of the prefix that matched. An
        // unresolvable or ambiguous name previews nothing rather than
        // falling back to the selection: the cast is going to be
        // refused, and shading an area for a spell that will not be
        // cast is worse than shading none.
        let named = &tokens[..index];
        let action = if named.is_empty() {
            self.selected_action()
        } else {
            crate::engine::prompt::Prompt::resolve_action(prompt.actions(), named)
                .ok()
                .map(|(_, a)| a)
        };
        let Some(action) = action else {
            return empty;
        };
        let Some(shape) = action.targeting_schema().area_shape() else {
            return empty;
        };
        // The cast level the line asks for, if it asks for one. Threaded
        // into the preview rather than dropped, because the validate
        // below is the promise this shading makes: a caster out of
        // third-level slots typing `fireball +2 20,14` is about to make
        // a legal cast, and shading nothing would say otherwise.
        let base_level = crate::engine::side_effects::spell_slot_level(&action.cost(
            &self.encounter,
            prompt.actor_id(),
            None,
            None,
            None,
        ));
        let overrides = tokens
            .iter()
            .find_map(|t| {
                crate::engine::prompt::Prompt::parse_cast_level(t, base_level.unwrap_or(0))
            })
            .and_then(|parsed| parsed.ok())
            .filter(|lvl| base_level.is_some_and(|base| *lvl >= base))
            .map(|lvl| {
                std::collections::HashSet::from([
                    crate::engine::action_overrides::ActionOverride::CastLevel(lvl),
                ])
            });
        let aei = ActionExecutionInfo::new(
            action,
            prompt.actor_id(),
            None,
            Some(vec![aim]),
            overrides,
        );
        if !aei.validate(&self.encounter) {
            return empty;
        }
        shape
            .tiles(actor.location(), actor.size(), aim)
            .into_iter()
            .collect()
    }

    /// The cast a bare `X,Y` means: the highlighted action, aimed at
    /// that tile.
    ///
    /// The prompt's parser wants an action name in front of the tile,
    /// so `12,5` on its own came back as *unknown or unavailable action
    /// "12,5"* — while the target line above it read `cone of cold:
    /// 60-foot cone. Type 'X,Y' to aim it through a tile, Enter to
    /// cast.` and the map shaded the cone as the digits went in. Three
    /// parts of the interface promised a cast the fourth refused. This
    /// is the one that was wrong: the action is already named, by the
    /// highlight, and making the player retype it is asking them to say
    /// twice what they have said once.
    ///
    /// `None` when the input is not a lone coordinate or the highlight
    /// does not take a point, which leaves the parser's own error to be
    /// shown — a mistyped spell name must still read as a mistyped
    /// spell name and not as a rejected tile.
    fn bare_point_cast(&self, trimmed: &str) -> Option<ActionExecutionInfo> {
        let prompt = self.encounter.peek_prompt()?;
        let actor = self.encounter.actors.get(&prompt.actor_id())?;
        if trimmed.split_whitespace().count() != 1 {
            return None;
        }
        let aim = crate::engine::util::parse_coord(trimmed, actor.location())?;
        let action = self.selected_action()?;
        if !action.targeting_schema().takes_one_point() {
            return None;
        }
        Some(ActionExecutionInfo::new(
            action,
            prompt.actor_id(),
            None,
            Some(vec![aim]),
            None,
        ))
    }

    fn target_line(&self) -> Option<String> {
        let prompt = self.encounter.peek_prompt()?;
        let action = self.selected_action()?;
        if let Some(shape) = action.targeting_schema().area_shape() {
            // A burst is aimed at a centre and a cone or a line is
            // aimed *through* a tile, which is a different instruction
            // to give the player — "where do you want this to land"
            // versus "which way do you want to point".
            let how = match shape {
                crate::engine::areas::AreaShape::Burst { .. } => "target a tile",
                _ => "aim it through a tile",
            };
            return Some(format!(
                "{}: {}. Type 'X,Y' to {}, Enter to cast.",
                action.name(),
                shape.label(),
                how
            ));
        }
        if !matches!(action.targeting_schema(), TargetingSchema::SingleActor) {
            return None;
        }
        if self.valid_targets.is_empty() {
            // Distinguish "I can't afford this" from "no enemy is reachable":
            // both produce an empty target list but the player needs a
            // different next move (switch action vs. close distance / wait).
            let caster_id = prompt.actor_id();
            let costs = action.cost(&self.encounter, caster_id, None, None, None);
            let unaffordable_cost = costs.iter().find(|c| {
                self.encounter
                    .actors
                    .get(&caster_id)
                    .is_none_or(|a| !a.can_consume_resource(**c))
            });
            // 5e Haste's restricted slot. The actor has an Action —
            // `can_consume_resource` says so, because it counts slots
            // and cannot tell one kind from the other — and still
            // cannot spend it on this. Without this arm the player is
            // told "no targets in reach" while looking at a panel that
            // says two Actions and an enemy standing in front of them,
            // which is the exact confusion the affordability branch
            // above exists to prevent.
            let blocked_by_haste = costs.contains(&crate::engine::side_effects::Resource::Action)
                && !action.hasted_action_eligible()
                && self
                    .encounter
                    .actors
                    .get(&caster_id)
                    .is_some_and(|a| a.action_slots() <= a.restricted_action_slots());
            // 5e: a spell can be cast with a slot of its own level or
            // higher. The engine prices a cast at exactly one level, so
            // a wizard out of third-level slots holding a fifth is
            // correctly refused *this* cast — and can make the
            // fifth-level one instead, by typing the cast level. Naming
            // the slot that would work turns a dead end into a
            // keystroke; without it the panel says "no level-3 spell
            // slot" beside a sheet showing two level-5s.
            let upcast_hint = |c: &crate::engine::side_effects::Resource| match c {
                crate::engine::side_effects::Resource::SpellSlot(lvl) => self
                    .encounter
                    .actors
                    .get(&caster_id)
                    .and_then(|a| a.lowest_available_spell_slot_at_least(lvl + 1))
                    .map(|higher| format!(" — type 'lvl:{}' to cast it with one", higher)),
                _ => None,
            };
            let reason = if let Some(c) = unaffordable_cost {
                format!(
                    "{}{}",
                    c.lack_description(),
                    upcast_hint(c).unwrap_or_default()
                )
            } else if blocked_by_haste {
                "only haste's action is left (attack, dash, disengage or hide)".to_string()
            } else {
                "no targets in reach".to_string()
            };
            return Some(format!(
                "{}: {} (Tab to switch action)",
                action.name(),
                reason
            ));
        }
        let id = self.highlighted_target()?;
        let target = self.encounter.actors.get(&id)?;
        Some(format!(
            "{}: target {} (HP {}/{}, team {}) — \u{2190}/\u{2192} cycle, Enter confirm",
            action.name(),
            target.name(),
            target.hitpoints(),
            target.max_hitpoints(),
            target.team()
        ))
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
        let Some(action) = self.selected_action() else {
            return;
        };

        match action.targeting_schema() {
            TargetingSchema::SinglePoint => {
                // Move-style action: arrows step the actor one tile.
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
            }
            TargetingSchema::SingleActor => {
                // Target picker: left/right cycle the highlighted enemy,
                // up/down cycle through actions (so the player can switch
                // off an actor-target action without leaving the keyboard).
                let n = self.valid_targets.len();
                match code {
                    KeyCode::Left if n > 0 => {
                        self.selected_target_idx = (self.selected_target_idx + n - 1) % n;
                    }
                    KeyCode::Right if n > 0 => {
                        self.selected_target_idx = (self.selected_target_idx + 1) % n;
                    }
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
            TargetingSchema::NoArgs
            | TargetingSchema::Custom
            | TargetingSchema::Burst { .. }
            | TargetingSchema::Cone { .. }
            | TargetingSchema::Line { .. } => {
                // Up/Down cycle the action; Left/Right ignored (avoid
                // accidental selection-changes during command typing).
                // Burst falls in here because point-targeting goes through
                // the text input today (e.g. "sacred burst 12,8").
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
        }
    }

    /// `attune <item>` / `unattune <item>` — the player's two levers on
    /// the three-item attunement ceiling.
    ///
    /// **Commands** rather than `Action`s, sitting beside `quit`,
    /// because neither is a thing that belongs on the action picker:
    /// the picker is a list of ways to affect the board, and these
    /// affect the holder's own pack. They still cost what RAW charges,
    /// and the two halves are charged differently because RAW charges
    /// them differently:
    ///
    ///   - **Ending** a bond is free and immediate — *"you can end your
    ///     attunement … by removing the item"*, which is the work of a
    ///     moment. The slot is available the same tick.
    ///   - **Forming** one *"requires a creature to spend a short rest
    ///     focused on only that item"*. This engine's rest is the walk
    ///     between two rooms, where there is no prompt to type at, so
    ///     the compression is the actor's **Action** for the turn:
    ///     the strongest price the engine can charge inside an
    ///     initiative order, and enough that a mid-fight re-equip costs
    ///     a round rather than nothing.
    ///
    /// An `attune` that cannot be paid for still does half its job: it
    /// clears the item out of `attunement_refusals`, so the bond forms
    /// on its own at the next rest. That is the case the message names,
    /// and it is why the command is worth typing when the party is out
    /// of slots and out of Actions both.
    ///
    /// Matching is a case-insensitive substring over the actor's own
    /// pack, so `attune belt` finds the Belt of Giant Strength. An
    /// ambiguous fragment is refused by name rather than resolved, for
    /// the same reason `prompt.rs` refuses an ambiguous action alias:
    /// the wrong ring is a worse outcome than a retyped line.
    fn attunement_command(&mut self, arg: &str, forming: bool) -> Tick {
        use crate::engine::side_effects::Resource;
        self.tmp_message.clear();
        let Some(prompt) = self.encounter.peek_prompt() else {
            self.tmp_message.push_str("there is nobody whose pack to change");
            return Tick::Continue;
        };
        let actor_id = prompt.actor_id();
        let Some(actor) = self.encounter.actors.get(&actor_id) else {
            return Tick::Continue;
        };
        // The candidate set is the half of the pack the verb is about:
        // `unattune` can only reach a bond that exists, `attune` only an
        // item that wants one and has not got it.
        let candidates: Vec<&'static str> = if forming {
            actor
                .items()
                .iter()
                .filter(|i| i.requires_attunement && !actor.is_attuned_to(i.name))
                .map(|i| i.name)
                .collect()
        } else {
            actor.attunements().to_vec()
        };
        let verb = if forming { "attune" } else { "unattune" };
        if arg.is_empty() {
            let _ = write!(
                self.tmp_message,
                "{} what? {}: {}",
                verb,
                if forming { "waiting" } else { "attuned" },
                if candidates.is_empty() {
                    "nothing".to_string()
                } else {
                    candidates.join(", ")
                }
            );
            return Tick::Continue;
        }
        let needle = arg.to_lowercase();
        let mut hits: Vec<&'static str> = candidates
            .iter()
            .copied()
            .filter(|n| n.to_lowercase().contains(&needle))
            .collect();
        // Sorted before the dedupe, because `dedup` only collapses
        // *adjacent* equals and the pack can hold two Rings of
        // Protection with a cloak between them. Two copies of one item
        // are one bond (`attunements` is keyed by name), so leaving them
        // both in would report the ring as ambiguous with itself.
        hits.sort_unstable();
        hits.dedup();
        let name = match hits.as_slice() {
            [] => {
                let _ = write!(
                    self.tmp_message,
                    "nothing to {} matching '{}'",
                    verb, arg
                );
                return Tick::Continue;
            }
            [one] => *one,
            many => {
                let _ = write!(
                    self.tmp_message,
                    "'{}' is ambiguous: {}",
                    arg,
                    many.join(", ")
                );
                return Tick::Continue;
            }
        };

        if !forming {
            if let Some(actor) = self.encounter.actors.get_mut(&actor_id) {
                actor.end_attunement(name);
            }
            // A bond broken is a lamp out: the Mace of Disruption a
            // player sets aside stops lighting the corridor. Both ends
            // of an attunement reconcile the carried lights.
            self.encounter.light_carried_items(actor_id);
            let who = self.encounter.actor_name(actor_id);
            self.encounter
                .log(format!("{} sets aside {}.", who, name));
            self.input_str.clear();
            return Tick::Continue;
        }

        // Forming. Every refusal is checked before anything is spent,
        // so a holder with an Action and no slot keeps the Action.
        let Some(actor) = self.encounter.actors.get_mut(&actor_id) else {
            return Tick::Continue;
        };
        // RAW's *"requires attunement by a Paladin"* first, because it is
        // the one refusal that no amount of rearranging fixes. Reported
        // in the book's own words, and *without* clearing the refusal
        // set: an item this creature can never bond with has no place in
        // the queue for the next freed slot.
        let barred = actor
            .items()
            .iter()
            .find(|i| i.name == name)
            .and_then(|i| {
                i.attunement_restriction
                    .filter(|_| !actor.may_attune_to_item(i))
            });
        if let Some(restriction) = barred {
            let _ = write!(
                self.tmp_message,
                "{} answers only to {}",
                name, restriction.describes
            );
            return Tick::Continue;
        }
        if actor.free_attunement_slots() == 0 {
            // Still worth the keystroke: the refusal is cleared, so the
            // next freed slot goes here. `attune_to` does that much and
            // then declines the bond itself.
            actor.attune_to(name);
            let _ = write!(
                self.tmp_message,
                "no free attunement slot for {} — unattune something first \
                 (it is first in line once one frees)",
                name
            );
            return Tick::Continue;
        }
        if !actor.can_consume_resource(Resource::Action) {
            actor.attune_to(name);
            let _ = write!(
                self.tmp_message,
                "attuning to {} takes an Action — none left this turn, so it \
                 will form at the next rest",
                name
            );
            return Tick::Continue;
        }
        // The bond first and the bill second, rather than the other way
        // round. Nothing above should let a refusal through this far —
        // but "should" is what makes a creature that loses its turn to a
        // refusal nobody logged, and the order costs nothing.
        if !actor.attune_to(name) {
            return Tick::Continue;
        }
        actor.consume_resource(Resource::Action);
        self.encounter.light_carried_items(actor_id);
        let who = self.encounter.actor_name(actor_id);
        self.encounter
            .log(format!("{} attunes to {}.", who, name));
        self.input_str.clear();
        Tick::Continue
    }

    fn handle_enter(&mut self) -> Tick {
        let trimmed = self.input_str.trim();
        if trimmed == "quit" {
            return Tick::Quit;
        }
        // Checked before `attune`, because `strip_prefix("attune")`
        // would never see "unattune" but the reverse is not true of a
        // reader: keeping the longer word first means the pair cannot
        // be reordered into a bug.
        if let Some(arg) = trimmed.strip_prefix("unattune") {
            let arg = arg.trim().to_string();
            return self.attunement_command(&arg, false);
        }
        if let Some(arg) = trimmed.strip_prefix("attune") {
            let arg = arg.trim().to_string();
            return self.attunement_command(&arg, true);
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
                    // A bare tile against the highlighted action, which
                    // is what the target line has been telling the
                    // player to type all along — see `bare_point_cast`.
                    match self.bare_point_cast(trimmed) {
                        Some(aei) if aei.validate(&self.encounter) => {
                            self.encounter.pop_prompt();
                            self.encounter.push_action(aei);
                            self.input_str.clear();
                            self.tmp_message.clear();
                        }
                        Some(aei) => {
                            self.tmp_message.clear();
                            let _ = write!(
                                self.tmp_message,
                                "'{}' cannot be aimed there",
                                aei.action().name()
                            );
                        }
                        None => {
                            self.tmp_message.clear();
                            self.tmp_message.push_str(&e.to_string());
                        }
                    }
                }
            }
            return Tick::Continue;
        }

        // Empty input: confirm the current selection. Behavior depends on
        // the selected action's targeting schema:
        //   - SingleActor: invoke it against the currently highlighted target
        //   - NoArgs:      invoke it as-is (dash, skip)
        //   - SinglePoint: needs an arrow-key destination; tell the player
        //   - Custom:      try invocation with no args and surface any error
        let Some(prompt) = self.encounter.peek_prompt() else {
            return Tick::Continue;
        };
        let actor_id = prompt.actor_id();
        let Some(action) = self.selected_action() else {
            return Tick::Continue;
        };
        let aei = match action.targeting_schema() {
            TargetingSchema::SingleActor => {
                let Some(target_id) = self.highlighted_target() else {
                    self.tmp_message.clear();
                    let _ = write!(self.tmp_message, "'{}' has no valid targets", action.name());
                    return Tick::Continue;
                };
                ActionExecutionInfo::new(action, actor_id, Some(vec![target_id]), None, None)
            }
            TargetingSchema::SinglePoint => {
                self.tmp_message.clear();
                let _ = write!(
                    self.tmp_message,
                    "'{}' needs a destination — use arrow keys",
                    action.name()
                );
                return Tick::Continue;
            }
            TargetingSchema::Burst { .. } => {
                self.tmp_message.clear();
                let _ = write!(
                    self.tmp_message,
                    "'{}' needs a target tile — type 'X,Y' first",
                    action.name()
                );
                return Tick::Continue;
            }
            TargetingSchema::Cone { .. } | TargetingSchema::Line { .. } => {
                self.tmp_message.clear();
                let _ = write!(
                    self.tmp_message,
                    "'{}' needs a direction — type 'X,Y' to aim it through a tile",
                    action.name()
                );
                return Tick::Continue;
            }
            TargetingSchema::NoArgs | TargetingSchema::Custom => {
                ActionExecutionInfo::new(action, actor_id, None, None, None)
            }
        };
        if aei.validate(&self.encounter) {
            self.encounter.pop_prompt();
            self.encounter.push_action(aei);
            self.input_str.clear();
            self.tmp_message.clear();
        } else {
            self.tmp_message.clear();
            let _ = write!(self.tmp_message, "'{}' is not valid right now", action.name());
        }
        Tick::Continue
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::action_template::ActionExecutionInfo;
    use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;

    /// An empty 20×20 arena with the generator's roster switched off,
    /// so the only actors on the board are the ones a test puts there.
    fn app_with_empty_board() -> App {
        let terrain_params = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let actor_params = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut encounter =
            EncounterInstance::from_params(&terrain_params, &actor_params, Some(0))
                .expect("an empty board generates");
        // Dry. "Empty" here means nothing happens on this board unless a
        // test makes it happen, and a generated pool is not nothing: a
        // creature standing in one is on `engine::breath`'s suffocation
        // clock, so the stalemate test below — two goblins who never act,
        // for as many rounds as it takes — would end with a drowned
        // goblin instead of a draw. The tests in this module are about
        // banners and prompts; the water layer has its own.
        for x in 0..terrain_params.width as isize {
            for y in 0..terrain_params.height as isize {
                let c = Coordinate::new(x, y);
                if encounter
                    .terrain_at(c)
                    .is_some_and(|t| t.terrain_type.is_water())
                {
                    encounter.set_terrain_at(c, crate::engine::terrain::TerrainType::Floor);
                }
            }
        }
        App::new(encounter, terrain_params, actor_params)
    }

    fn spawn(app: &mut App, team: usize, at: Coordinate) -> usize {
        app.encounter
            .instantiate_creature(&GOBLIN_TEMPLATE, at, team, team)
            .expect("the goblin fits")
    }

    /// The party that walks into the next room has rested properly:
    /// levelled for the XP it banked, and holding a staff with charges
    /// back on it.
    ///
    /// Both halves are things the between-encounters rest did not do.
    /// `start_next_encounter` called `ActorInstance::long_rest` on each
    /// surviving PC, which restores hit points and slots and nothing
    /// else — so the XP `kill` awards, under a comment reading "leveling
    /// happens on long rest", was banked by every party in every run and
    /// spent by none of them, and a staff emptied in the first room
    /// stayed empty for the dungeon.
    #[test]
    fn the_party_levels_and_recharges_on_the_way_to_the_next_room() {
        use crate::items::item_template::STAFF_OF_FIRE;

        let mut app = app_with_empty_board();
        // A wizard rather than the goblin `spawn` hands out: RAW's staff
        // answers only to a spellcaster, and an unattuned one has no
        // charges anybody can spend.
        let pc = app
            .encounter
            .instantiate_creature(
                &crate::actors::creatures::wizards::WIZARD_TEMPLATE,
                Coordinate::new(5, 5),
                0,
                0,
            )
            .expect("the wizard fits");
        let (level_before, charges_before) = {
            let a = app.encounter.actors.get_mut(&pc).unwrap();
            a.pickup_item(&STAFF_OF_FIRE);
            // Down to two of ten, and enough XP banked for exactly one
            // level (the curve is `level * 300`).
            assert!(a.consume_resource(crate::engine::side_effects::Resource::ItemCharges {
                item: STAFF_OF_FIRE.name,
                count: 8,
            }));
            a.award_xp(a.xp_threshold_for_next_level());
            (a.level(), a.item_charges_remaining(STAFF_OF_FIRE.name))
        };
        assert_eq!(charges_before, 2);

        assert!(app.start_next_encounter(), "the next room generates");

        let pc = *app
            .encounter
            .actors
            .iter()
            .find(|(_, a)| a.team() == 0)
            .expect("the party came through")
            .0;
        let a = &app.encounter.actors[&pc];
        assert_eq!(
            a.level(),
            level_before + 1,
            "the XP the last room paid out should have bought a level"
        );
        let after = a.item_charges_remaining(STAFF_OF_FIRE.name);
        assert!(
            (3..=10).contains(&after),
            "1d6+1 back on top of two, capped at ten (got {after})"
        );
        // And the player is told, in the log they are about to read
        // rather than in the one that just got dropped.
        assert!(
            app.encounter
                .messages()
                .iter()
                .any(|m| m.contains("reaches level")),
            "the level-up should be announced in the new encounter's log"
        );
    }

    /// The player cleared the board: the banner says so and offers the
    /// key that starts the next fight.
    #[test]
    fn a_cleared_board_offers_the_next_encounter() {
        let mut app = app_with_empty_board();
        spawn(&mut app, 0, Coordinate::new(5, 5));
        assert!(app.encounter.is_complete(), "one team left is a win");
        assert!(app.player_may_continue());
        let banner = app.completion_banner().expect("a finished fight has a banner");
        assert!(banner.contains("cleared"), "got: {banner}");
        assert!(banner.contains("long rest"), "got: {banner}");
    }

    /// The player fell: no continue, and the banner names the winner
    /// rather than offering a rest the player cannot take.
    #[test]
    fn a_lost_board_offers_nothing_but_the_exit() {
        let mut app = app_with_empty_board();
        spawn(&mut app, 1, Coordinate::new(5, 5));
        assert!(app.encounter.is_complete());
        assert!(
            !app.player_may_continue(),
            "nobody on the player's team is standing"
        );
        let banner = app.completion_banner().expect("a finished fight has a banner");
        assert!(banner.contains("Team 1 wins"), "got: {banner}");
        assert!(!banner.contains("long rest"), "got: {banner}");
    }

    /// A draw is not a wipe, and the banner has to stop saying it is.
    ///
    /// `winning_team` answers `None` for both endings, and the banner
    /// used to print "No survivors" for each — over a board with two
    /// live goblins standing on it, in this case. The player is alive
    /// and did not lose, so they get the same offer a winner gets.
    #[test]
    fn a_draw_is_reported_as_a_draw_and_the_survivor_may_continue() {
        let mut app = app_with_empty_board();
        let pc = spawn(&mut app, 0, Coordinate::new(5, 5));
        let foe = spawn(&mut app, 1, Coordinate::new(6, 5));
        assert!(!app.encounter.is_complete(), "the fight has not started yet");

        // Neither goblin ever swings, so the fight makes no progress
        // and the engine eventually calls it. See
        // `EncounterInstance::is_stalemate`.
        let mut steps = 0usize;
        while !app.encounter.is_complete() && steps < 200_000 {
            steps += 1;
            app.encounter.process_stack();
            if app.encounter.is_complete() {
                break;
            }
            let Some(prompt) = app.encounter.peek_prompt() else {
                break;
            };
            let actor_id = prompt.actor_id();
            app.encounter.pop_prompt();
            app.encounter.push_action(ActionExecutionInfo::new(
                &*crate::actions::default_actions::SKIP,
                actor_id,
                None,
                None,
                None,
            ));
        }

        assert!(app.encounter.is_complete(), "the draw was called");
        assert_eq!(app.encounter.winning_team(), None, "nobody won");
        assert!(
            app.encounter.actors[&pc].is_combat_active()
                && app.encounter.actors[&foe].is_combat_active(),
            "both are still standing, which is what makes this a draw and not a wipe"
        );
        let banner = app.completion_banner().expect("a finished fight has a banner");
        assert!(banner.contains("stalemate"), "got: {banner}");
        assert!(
            !banner.contains("No survivors"),
            "there are two of them right there: {banner}"
        );
        assert!(
            app.player_may_continue(),
            "a player who walked out of a draw has no more reason to be sent \
             to the quit prompt than one who won"
        );
    }

    /// A continue that cannot build a board says so, instead of doing
    /// nothing and leaving the player to press R at a screen that
    /// refuses without explaining.
    ///
    /// The keypress discarded `start_next_encounter`'s return value, so
    /// the two ways it can answer false — everybody is dead, and the
    /// generator could not place anybody — were indistinguishable from
    /// the outside and one of them was invisible. The failure is forced
    /// here with a map too small to hold what the budget asks for,
    /// which is the same shape the real one takes: an unbounded
    /// difficulty ramp against a fixed-size board.
    #[test]
    fn a_continue_that_cannot_build_a_board_says_so() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        // A one-room map barely wider than the fighter standing in it,
        // asked for a fight it has no anchors left to hold.
        let terrain_params = TerrainGenParams {
            width: 6,
            height: 6,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let actor_params = ActorGenParams {
            cr_target: 40.0,
            n_teams: 2,
            pc_template: Some(&FIGHTER_TEMPLATE),
            start_team: 0,
        };
        // Build the app around a board the generator *can* make (the
        // player alone), then ask it for the impossible next one.
        let mut solo = actor_params.clone();
        solo.n_teams = 1;
        let encounter = EncounterInstance::from_params(&terrain_params, &solo, Some(3))
            .expect("one fighter fits on any board");
        let mut app = App::new(encounter, terrain_params, actor_params);
        assert!(app.encounter.is_complete(), "one team left is a win");
        assert!(app.player_may_continue());

        let advanced = app.start_next_encounter();
        assert!(!advanced, "the board cannot hold that fight");
        assert_eq!(
            app.encounter_number, 1,
            "a failed continue must not advance the counter"
        );
        let banner = app
            .completion_banner()
            .expect("the fight is still over and still won");
        assert!(
            banner.contains("cleared"),
            "the verdict survives the failure: {banner}"
        );
        assert!(
            banner.contains("could not build the next encounter"),
            "and the failure is named: {banner}"
        );
    }

    /// The preview reads the tile off the *tail* of the input, because
    /// the prompt's own parser accepts an action name in front of it.
    /// A player who typed `burning hands 12,5` is aiming at (12,5), and
    /// a preview that parsed the first token would show them nothing.
    #[test]
    fn the_area_preview_reads_the_coordinate_off_the_end_of_the_input() {
        use crate::engine::areas::AreaShape;

        let mut app = app_with_empty_board();
        let wiz = app
            .encounter
            .instantiate_creature(
                &crate::actors::creatures::wizards::WIZARD_TEMPLATE,
                Coordinate::new(4, 4),
                0,
                0,
            )
            .expect("the wizard fits");
        spawn(&mut app, 1, Coordinate::new(12, 5));
        // Park the selection on Burning Hands, which is the cone this
        // is about.
        let idx = app.encounter.actors[&wiz]
            .actions
            .iter()
            .position(|a| a.name() == "burning hands")
            .expect("a wizard carries burning hands");
        app.selected_action_idx = idx;
        app.encounter.process_stack();

        let expected: std::collections::HashSet<Coordinate> = AreaShape::Cone { length: 6 }
            .tiles(
                Coordinate::new(4, 4),
                app.encounter.actors[&wiz].size(),
                Coordinate::new(12, 5),
            )
            .into_iter()
            .collect();
        assert!(!expected.is_empty());

        for typed in ["12,5", "burning hands 12,5"] {
            app.input_str.clear();
            app.input_str.push_str(typed);
            assert_eq!(
                app.previewed_area(),
                expected,
                "typing {typed:?} should preview the same cone"
            );
        }

        // `bh` is one of the collisions `Prompt::resolve_action`
        // refuses outright — a wizard's Burning Hands and Bigby's Hand
        // both answer to it — and the preview refuses it for the same
        // reason: the cast is going to be rejected, so shading either
        // spell's area would be a promise the Enter key does not keep.
        app.input_str.clear();
        app.input_str.push_str("bh 12,5");
        assert!(app.previewed_area().is_empty());

        // A half-typed coordinate previews nothing rather than
        // guessing, which is what keeps the map from flickering through
        // wrong shapes on every keystroke.
        for typed in ["", "12,", "burning", "nonsense"] {
            app.input_str.clear();
            app.input_str.push_str(typed);
            assert!(
                app.previewed_area().is_empty(),
                "typing {typed:?} should preview nothing"
            );
        }

        // And a *named* action wins over the arrow-key selection, which
        // is the case the two can disagree about: the wizard's Cone of
        // Cold is twenty-four tiles long where Burning Hands is six, so
        // a preview that read the highlight would shade one and cast
        // the other.
        let coc = app.encounter.actors[&wiz]
            .actions
            .iter()
            .position(|a| a.name() == "cone of cold")
            .expect("a wizard carries cone of cold");
        app.selected_action_idx = coc;
        app.input_str.clear();
        app.input_str.push_str("burning hands 12,5");
        assert_eq!(
            app.previewed_area(),
            expected,
            "the typed spell is the one being cast, so it is the one drawn"
        );

        // An unresolvable name previews nothing rather than falling
        // back to the highlight — the cast is going to be refused, and
        // shading an area for a spell that will not be cast is worse
        // than shading none.
        app.input_str.clear();
        app.input_str.push_str("wobble 12,5");
        assert!(app.previewed_area().is_empty());
    }

    /// A bare tile casts the highlighted action at it.
    ///
    /// The other half of the preview: the map shades the cone as the
    /// digits go in and the target line says *type 'X,Y' to aim it
    /// through a tile, Enter to cast*, so Enter has to cast it. It used
    /// to answer `unknown or unavailable action "12,5"`, because the
    /// prompt's parser wants the spell named in front of the tile —
    /// which the highlight has already named.
    #[test]
    fn a_bare_tile_casts_the_highlighted_action() {
        let mut app = app_with_empty_board();
        let wiz = app
            .encounter
            .instantiate_creature(
                &crate::actors::creatures::wizards::WIZARD_TEMPLATE,
                Coordinate::new(4, 4),
                0,
                0,
            )
            .expect("the wizard fits");
        let goblin = spawn(&mut app, 1, Coordinate::new(9, 5));
        let idx = app.encounter.actors[&wiz]
            .actions
            .iter()
            .position(|a| a.name() == "burning hands")
            .expect("a wizard carries burning hands");
        app.selected_action_idx = idx;
        app.encounter.process_stack();
        assert!(app.encounter.peek_prompt().is_some(), "the wizard is asked");

        app.input_str.push_str("9,5");
        // The preview and the cast agree on the shape, which is the
        // property the two share a parse for.
        let previewed = app.previewed_area();
        assert!(previewed.contains(&Coordinate::new(9, 5)));
        app.handle_enter();
        assert!(
            app.tmp_message.is_empty(),
            "no complaint: {}",
            app.tmp_message
        );
        assert!(app.input_str.is_empty(), "the input was consumed");
        assert!(
            app.encounter.peek_prompt().is_none(),
            "and the prompt was answered rather than re-asked"
        );
        let before = app.encounter.actors[&goblin].hitpoints();
        app.encounter.process_stack();
        assert!(
            app.encounter
                .messages()
                .iter()
                .any(|m| m.contains("burning hands")),
            "the highlighted spell is the one that went off"
        );
        assert!(
            app.encounter.actors[&goblin].hitpoints() < before,
            "and the goblin standing in the cone felt it"
        );
    }

    /// A mistyped spell name still reads as one.
    ///
    /// The bare-tile path is a fallback for input the parser rejected,
    /// so it has to stay out of the way of every other rejection: a
    /// player who typed `wobble 12,5` has misspelled a spell, not aimed
    /// one, and the message must say so rather than blaming the tile.
    #[test]
    fn a_mistyped_name_is_not_read_as_a_bare_tile() {
        let mut app = app_with_empty_board();
        app.encounter
            .instantiate_creature(
                &crate::actors::creatures::wizards::WIZARD_TEMPLATE,
                Coordinate::new(4, 4),
                0,
                0,
            )
            .expect("the wizard fits");
        spawn(&mut app, 1, Coordinate::new(9, 5));
        app.encounter.process_stack();

        app.input_str.push_str("wobble 9,5");
        app.handle_enter();
        assert!(
            app.tmp_message.contains("wobble"),
            "the name is what was wrong: {}",
            app.tmp_message
        );
        assert!(
            app.encounter.peek_prompt().is_some(),
            "and the prompt is still waiting"
        );
    }

    /// A single-target action previews nothing however valid the tile
    /// the player has typed is — the shading means "this is the area",
    /// and an action with no area has none.
    #[test]
    fn a_non_area_action_previews_nothing() {
        let mut app = app_with_empty_board();
        let pc = spawn(&mut app, 0, Coordinate::new(4, 4));
        spawn(&mut app, 1, Coordinate::new(6, 4));
        app.encounter.process_stack();
        let idx = app.encounter.actors[&pc]
            .actions
            .iter()
            .position(|a| a.targeting_schema().area_shape().is_none())
            .expect("a goblin has a non-area action");
        app.selected_action_idx = idx;
        app.input_str.clear();
        app.input_str.push_str("6,4");
        assert!(app.previewed_area().is_empty());
    }

    /// A caster out of the printed slot is told which one *would* work.
    ///
    /// 5e lets any spell be cast with a slot of its own level or
    /// higher, and the engine prices a cast at exactly one level — so
    /// "no level-3 spell slot" is a correct refusal of the wrong
    /// question, printed beside a sheet that still shows two level-5s.
    /// The hint names the slot the player can actually ask for.
    #[test]
    fn a_spent_slot_names_the_bigger_one_that_would_still_cast_it() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::engine::side_effects::Resource;

        let mut app = app_with_empty_board();
        let wizard = app
            .encounter
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .expect("the wizard fits");
        spawn(&mut app, 1, Coordinate::new(7, 5));
        app.encounter.process_stack();

        // Spend every third-level slot, leaving the higher ones alone.
        while app.encounter.actors[&wizard].can_consume_resource(Resource::SpellSlot(3)) {
            app.encounter
                .actors
                .get_mut(&wizard)
                .unwrap()
                .consume_resource(Resource::SpellSlot(3));
        }
        let higher = app.encounter.actors[&wizard]
            .lowest_available_spell_slot_at_least(4)
            .expect("a level-9 wizard still has something bigger");

        // Point the selection at a level-3 single-target spell the
        // wizard can no longer afford.
        let idx = app
            .encounter
            .peek_prompt()
            .expect("the wizard is up")
            .actions()
            .iter()
            .position(|a| a.name() == "vampiric touch")
            .expect("the wizard carries it");
        app.selected_action_idx = idx;

        let line = app.target_line().expect("an unaffordable spell says why");
        assert!(
            line.contains("no level-3 spell slot"),
            "the refusal should still name the slot it wanted: {line}"
        );
        assert!(
            line.contains(&format!("lvl:{}", higher)),
            "and the slot that would work: {line}"
        );
    }

    /// And it stays quiet when there is nothing bigger to offer — a
    /// caster with no slots at all is not helped by being told to try a
    /// slot they also do not have.
    #[test]
    fn a_caster_with_nothing_left_is_offered_nothing() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::engine::side_effects::Resource;

        let mut app = app_with_empty_board();
        let wizard = app
            .encounter
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .expect("the wizard fits");
        spawn(&mut app, 1, Coordinate::new(7, 5));
        app.encounter.process_stack();

        for lvl in 1..=9 {
            while app.encounter.actors[&wizard].can_consume_resource(Resource::SpellSlot(lvl)) {
                app.encounter
                    .actors
                    .get_mut(&wizard)
                    .unwrap()
                    .consume_resource(Resource::SpellSlot(lvl));
            }
        }
        let idx = app
            .encounter
            .peek_prompt()
            .expect("the wizard is up")
            .actions()
            .iter()
            .position(|a| a.name() == "vampiric touch")
            .expect("the wizard carries it");
        app.selected_action_idx = idx;

        let line = app.target_line().expect("an unaffordable spell says why");
        assert!(line.contains("no level-3 spell slot"), "got: {line}");
        assert!(!line.contains("lvl:"), "nothing to suggest: {line}");
    }

    /// The two attunement commands, from the panel's `*` to the bond.
    ///
    /// The path a player actually walks when the fourth ring drops:
    /// something inert in the pack, a slot freed on purpose, and the
    /// ring taken up in its place for the price of an Action. All three
    /// steps are typed at the same prompt the rest of the game is typed
    /// at, which is the reason they are commands.
    #[test]
    fn a_player_can_trade_one_attunement_for_another_at_the_prompt() {
        use crate::engine::side_effects::Resource;
        use crate::items::item_template::{
            CLOAK_OF_PROTECTION, RING_OF_PROTECTION, SCARAB_OF_PROTECTION, STONE_OF_GOOD_LUCK,
        };
        let mut app = app_with_empty_board();
        let pc = app
            .encounter
            .instantiate_creature(
                &crate::actors::creatures::fighters::FIGHTER_TEMPLATE,
                Coordinate::new(4, 4),
                0,
                0,
            )
            .expect("the fighter fits");
        spawn(&mut app, 1, Coordinate::new(9, 5));
        for item in [
            &RING_OF_PROTECTION,
            &CLOAK_OF_PROTECTION,
            &STONE_OF_GOOD_LUCK,
            &SCARAB_OF_PROTECTION,
        ] {
            app.encounter.actors.get_mut(&pc).unwrap().pickup_item(item);
        }
        app.encounter.process_stack();
        while app
            .encounter
            .peek_prompt()
            .is_some_and(|p| p.actor_id() != pc)
        {
            app.encounter.pop_prompt();
            app.encounter.process_stack();
        }
        assert!(
            app.encounter.peek_prompt().is_some_and(|p| p.actor_id() == pc),
            "the fighter needs to be the one at the prompt"
        );
        let scarab = SCARAB_OF_PROTECTION.name;
        assert!(!app.encounter.actors[&pc].is_attuned_to(scarab));

        // Attuning with every slot full is refused, but the ask is
        // remembered — that is the whole of the "first in line" clause.
        app.input_str.clear();
        app.input_str.push_str("attune scarab");
        app.handle_enter();
        assert!(
            app.tmp_message.contains("no free attunement slot"),
            "expected the ceiling to be reported, got {:?}",
            app.tmp_message
        );
        assert!(!app.encounter.actors[&pc].is_attuned_to(scarab));

        // Free one. Immediate and free, as RAW has it.
        app.input_str.clear();
        app.input_str.push_str("unattune stone");
        app.handle_enter();
        assert!(!app.encounter.actors[&pc].is_attuned_to(STONE_OF_GOOD_LUCK.name));
        assert_eq!(app.encounter.actors[&pc].free_attunement_slots(), 1);

        // And take the scarab up, for an Action.
        assert!(app.encounter.actors[&pc].can_consume_resource(Resource::Action));
        app.input_str.clear();
        app.input_str.push_str("attune scarab");
        app.handle_enter();
        assert!(
            app.encounter.actors[&pc].is_attuned_to(scarab),
            "the bond forms once there is a slot and an Action to pay with"
        );
        assert!(
            !app.encounter.actors[&pc].can_consume_resource(Resource::Action),
            "and the Action is what it cost"
        );
    }

    /// An attunement command that names nothing, or names too much,
    /// changes nothing and says why.
    #[test]
    fn an_ambiguous_attunement_command_is_refused_rather_than_guessed() {
        use crate::items::item_template::{RING_OF_FIRE_RESISTANCE, RING_OF_PROTECTION};
        let mut app = app_with_empty_board();
        let pc = app
            .encounter
            .instantiate_creature(
                &crate::actors::creatures::fighters::FIGHTER_TEMPLATE,
                Coordinate::new(4, 4),
                0,
                0,
            )
            .expect("the fighter fits");
        spawn(&mut app, 1, Coordinate::new(9, 5));
        for item in [&RING_OF_PROTECTION, &RING_OF_FIRE_RESISTANCE] {
            app.encounter.actors.get_mut(&pc).unwrap().pickup_item(item);
        }
        app.encounter.process_stack();
        while app
            .encounter
            .peek_prompt()
            .is_some_and(|p| p.actor_id() != pc)
        {
            app.encounter.pop_prompt();
            app.encounter.process_stack();
        }

        app.input_str.clear();
        app.input_str.push_str("unattune ring");
        app.handle_enter();
        assert!(
            app.tmp_message.contains("ambiguous"),
            "two rings match 'ring'; got {:?}",
            app.tmp_message
        );
        assert_eq!(
            app.encounter.actors[&pc].attunements().len(),
            2,
            "an ambiguous command must not break a bond"
        );

        app.input_str.clear();
        app.input_str.push_str("unattune greatsword");
        app.handle_enter();
        assert!(app.tmp_message.contains("nothing to unattune"));
        assert_eq!(app.encounter.actors[&pc].attunements().len(), 2);
    }
    /// `attune` on something that was made for somebody else says so in
    /// the book's own words, and costs nothing.
    ///
    /// The refusal a player most needs explained: a free slot and a full
    /// Action are both there, and the bond still will not form. Told
    /// "answers only to a spellcaster" they know to stop trying; told
    /// "not valid right now" they would rearrange the whole pack.
    #[test]
    fn attuning_to_somebody_elses_item_is_refused_in_raws_own_words() {
        use crate::engine::side_effects::Resource;
        use crate::items::item_template::WAND_OF_FIREBALLS;
        let mut app = app_with_empty_board();
        let pc = app
            .encounter
            .instantiate_creature(
                &crate::actors::creatures::fighters::FIGHTER_TEMPLATE,
                Coordinate::new(4, 4),
                0,
                0,
            )
            .expect("the fighter fits");
        spawn(&mut app, 1, Coordinate::new(9, 5));
        app.encounter
            .actors
            .get_mut(&pc)
            .unwrap()
            .pickup_item(&WAND_OF_FIREBALLS);
        app.encounter.process_stack();
        while app
            .encounter
            .peek_prompt()
            .is_some_and(|p| p.actor_id() != pc)
        {
            app.encounter.pop_prompt();
            app.encounter.process_stack();
        }
        assert!(app.encounter.actors[&pc].free_attunement_slots() > 0);
        assert!(app.encounter.actors[&pc].can_consume_resource(Resource::Action));

        app.input_str.clear();
        app.input_str.push_str("attune fireballs");
        app.handle_enter();
        assert!(
            app.tmp_message.contains("answers only to a spellcaster"),
            "expected RAW's clause, got {:?}",
            app.tmp_message
        );
        assert!(!app.encounter.actors[&pc].is_attuned_to(WAND_OF_FIREBALLS.name));
        assert!(
            app.encounter.actors[&pc].can_consume_resource(Resource::Action),
            "a refusal that cost the fighter their turn would be worse than no rule"
        );
    }
}
