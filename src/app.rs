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
        for pc in &mut rested {
            pc.long_rest();
        }
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
        crate::ui::render_map(&self.encounter, f, info_area[0], highlighted);
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
            let reason = if let Some(c) = unaffordable_cost {
                c.lack_description()
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
        let encounter = EncounterInstance::from_params(&terrain_params, &actor_params, Some(0))
            .expect("an empty board generates");
        App::new(encounter, terrain_params, actor_params)
    }

    fn spawn(app: &mut App, team: usize, at: Coordinate) -> usize {
        app.encounter
            .instantiate_creature(&GOBLIN_TEMPLATE, at, team, team)
            .expect("the goblin fits")
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
}
