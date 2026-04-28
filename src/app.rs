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
        match EncounterInstance::with_pcs(&self.terrain_params, &scaled_params, None, rested) {
            Ok(next) => {
                self.encounter = next;
                self.encounter_number += 1;
                self.input_str.clear();
                self.tmp_message.clear();
                self.selected_action_idx = 0;
                self.selected_target_idx = 0;
                self.valid_targets.clear();
                self.last_actor_id = None;
                self.last_action_idx = None;
                true
            }
            Err(_) => false,
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
            if self.encounter.winning_team() == Some(self.actor_params.start_team) {
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
            TargetingSchema::Burst { .. } => {
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
        // quit and (on player victory) "R" to long-rest into the next
        // encounter. Everything else is ignored so stray input doesn't
        // get buffered into the input box behind the banner.
        if self.encounter.is_complete() {
            return match key.code {
                KeyCode::Esc => Tick::Quit,
                KeyCode::Char('r') | KeyCode::Char('R')
                    if self.encounter.winning_team() == Some(self.actor_params.start_team) =>
                {
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

    fn completion_banner(&self) -> Option<String> {
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
            None => Some(format!(
                "No survivors of encounter {}. (Esc to quit)",
                self.encounter_number
            )),
        }
    }

    fn target_line(&self) -> Option<String> {
        let prompt = self.encounter.peek_prompt()?;
        let action = self.selected_action()?;
        if let TargetingSchema::Burst { radius } = action.targeting_schema() {
            return Some(format!(
                "{}: AoE radius {}. Type 'X,Y' to target a tile, Enter to cast.",
                action.name(),
                radius
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
            let reason = if let Some(c) = unaffordable_cost {
                c.lack_description()
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
            TargetingSchema::NoArgs | TargetingSchema::Custom | TargetingSchema::Burst { .. } => {
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

