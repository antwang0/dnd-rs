use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::actions::action_template::Action;
use crate::engine::encounter::{EncounterInstance, StackState};
use crate::engine::side_effects::Resource;
use crate::engine::terrain::TerrainType;
use crate::engine::types::Coordinate;
use crate::engine::util::get_colored_span;

/// Push a `"<label>: type, type, ..."` line into `stats_lines` for a
/// non-empty damage-type set. Sorted by debug name so the output is
/// stable across runs (HashSet iteration order is non-deterministic).
/// Empty set is a no-op so we don't pad the panel with "Resistant: -".
fn push_damage_set_line(
    stats_lines: &mut Vec<Line<'static>>,
    label: &str,
    set: &std::collections::HashSet<crate::engine::types::DamageType>,
    color: Color,
) {
    if set.is_empty() {
        return;
    }
    let mut names: Vec<String> = set.iter().map(|d| format!("{:?}", d)).collect();
    names.sort();
    stats_lines.push(Line::from(Span::styled(
        format!("{}: {}", label, names.join(", ")),
        Style::default().fg(color),
    )));
}

/// Heuristic classifier that styles a log line based on its contents.
/// Cheap pattern-matching against the message strings the engine emits
/// today; centralized here so the engine can keep emitting plain strings.
pub fn style_log_line(msg: &str) -> Line<'static> {
    let style = if msg.starts_with("[reaction]") {
        Style::default().fg(Color::Cyan)
    } else if msg.contains("Conscious at 1 HP") {
        Style::default().fg(Color::LightGreen)
    } else if msg.contains("falls unconscious")
        || msg.contains(" dies.")
        || msg.contains("stabilized")
    {
        Style::default().fg(Color::Yellow)
    } else if msg.contains(" damage") || msg.ends_with("— hit") {
        Style::default().fg(Color::LightRed)
    } else if msg.contains("death save") {
        if msg.contains("success") {
            Style::default().fg(Color::LightGreen)
        } else if msg.contains("failure") || msg.contains("dies!") {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default()
        }
    } else if msg.contains("— miss") {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };
    Line::from(Span::styled(msg.to_string(), style))
}

/// Compact tag for an action's resource cost — fits in the action panel
/// before the name. Multi-resource costs (e.g. leveled spells with both
/// Action and SpellSlot) join with `+`. Empty cost falls back to "[-]"
/// except for Move which special-cases to "[M]" — its actual cost is
/// path-dependent and only resolves at confirm time.
fn cost_label(costs: &[Resource], action_name: &str) -> String {
    if costs.is_empty() {
        return if action_name == "move" {
            "[M]".to_string()
        } else {
            "[-]".to_string()
        };
    }
    let parts: Vec<String> = costs
        .iter()
        .map(|r| match r {
            Resource::Action => "A".to_string(),
            Resource::BonusAction => "BA".to_string(),
            Resource::Reaction => "R".to_string(),
            Resource::LegendaryAction => "Lg".to_string(),
            Resource::Movement(_) => "M".to_string(),
            Resource::SpellSlot(n) => format!("S{}", n),
        })
        .collect();
    format!("[{}]", parts.join("+"))
}

/// True/false toggle that flips every ~500ms based on wall-clock time.
/// Used to manually blink UI elements; ANSI SLOW_BLINK is unreliable on
/// many terminals (notably Windows Terminal).
pub fn blink_on() -> bool {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    (millis / 500).is_multiple_of(2)
}

pub fn render_map(
    encounter: &EncounterInstance,
    frame: &mut Frame,
    area: Rect,
    highlighted_target: Option<usize>,
) {
    let mut text: Vec<Line> = Vec::new();

    let active_actor_id = match encounter.stack_state() {
        StackState::AwaitingPrompt(id) => Some(id),
        _ => None,
    };

    for y in (0..encounter.height).rev() {
        let mut row: Vec<Span> = Vec::new();
        for x in 0..encounter.width {
            let coord = Coordinate::new(x as isize, y as isize);
            if let Some(actor_id) = encounter.actor_id_at(coord)
                && let Some(actor) = encounter.actors.get(&actor_id)
            {
                // Stale id (cleanup race between damage tick and frame draw)
                // would otherwise crash the renderer; skip to terrain.
                let (mut s, c, bg): (String, Color, Color) =
                    get_colored_span(actor.glyph(), actor.team());
                let mut style = Style::default().fg(c).bg(bg);
                if Some(actor_id) == active_actor_id && !blink_on() {
                    style = style.add_modifier(Modifier::REVERSED);
                }
                // Prone actors render with a lowercase glyph (Z → z) — a
                // quick at-a-glance signal of "knocked down."
                if actor.has_condition(crate::conditions::Condition::Prone) {
                    s = s.to_lowercase();
                }
                // Downed actors render dimmed (stable as `+`, dying as `x`)
                // so the player can read the battlefield at a glance instead
                // of relying on the side-panel HP bar.
                if !actor.is_combat_active() {
                    s = if actor.is_stable() {
                        "+".to_string()
                    } else {
                        "x".to_string()
                    };
                    style = Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM);
                }
                if Some(actor_id) == highlighted_target {
                    // Bright magenta bg + bold makes the picker target pop
                    // above the team-color background.
                    style = Style::default()
                        .fg(Color::Black)
                        .bg(Color::Magenta)
                        .add_modifier(Modifier::BOLD);
                }
                row.push(Span::styled(s, style));
            } else if let Some(item) = encounter.items_at(coord).last() {
                // Ground loot draws on top of terrain. Render the most
                // recently dropped item's glyph in bright yellow so the
                // player notices pickups at a glance.
                row.push(Span::styled(
                    item.glyph.to_string(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                let s = Span::from(
                    match encounter.terrain_at(coord).map(|t| &t.terrain_type) {
                        Some(TerrainType::Floor) => '░',
                        Some(TerrainType::Wall) => '█',
                        _ => ' ',
                    }
                    .to_string(),
                );
                row.push(s);
            }
        }
        text.push(Line::from(row));
    }
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Map")),
        area,
    );
}

fn hp_bar_spans(current: u32, max: u32, width: usize) -> Vec<Span<'static>> {
    if max == 0 {
        return vec![];
    }
    let ratio = current as f64 / max as f64;
    let filled = ((ratio * width as f64).round() as usize).min(width);
    let empty = width - filled;
    let color = if ratio > 0.5 {
        Color::Green
    } else if ratio > 0.25 {
        Color::Yellow
    } else {
        Color::Red
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    if filled > 0 {
        spans.push(Span::styled(
            "█".repeat(filled),
            Style::default().fg(color),
        ));
    }
    if empty > 0 {
        spans.push(Span::styled(
            "░".repeat(empty),
            Style::default().fg(Color::DarkGray),
        ));
    }
    spans
}

pub fn render_sideinfo(
    encounter: &EncounterInstance,
    frame: &mut Frame,
    area: Rect,
    selected_action_idx: usize,
) {
    let area_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Min(1), Constraint::Min(1)])
        .split(area);

    let stack_state = encounter.stack_state();
    type ActionRef = &'static (dyn Action + Send + Sync);
    let prompt_info: Option<(usize, Vec<ActionRef>)> =
        if let StackState::AwaitingPrompt(actor_id) = stack_state {
            // Pull the prompt's action list. peek_prompt only returns Some
            // on AwaitingPrompt, so this is fine even though stack_state
            // already told us the actor.
            encounter
                .peek_prompt()
                .map(|p| (actor_id, p.actions().clone()))
        } else {
            None
        };

    // Initiative queue: show all actors in turn order, starting from the
    // current actor; highlight + blink-glyph the active one.
    let mut initiative_lines: Vec<Line<'static>> = Vec::new();
    for actor_id in encounter.initiative_actor_ids() {
        let is_current = prompt_info.as_ref().is_some_and(|(id, _)| *id == actor_id);
        if let Some(actor) = encounter.actors.get(&actor_id) {
            let (s, c, bg) = get_colored_span(actor.glyph(), actor.team());
            let prefix = if is_current { "> " } else { "  " };
            let mut glyph_style = Style::default().fg(c).bg(bg);
            let name_style = if is_current {
                if !blink_on() {
                    glyph_style = glyph_style.add_modifier(Modifier::REVERSED);
                }
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let mut spans: Vec<Span<'static>> = vec![
                Span::raw(prefix),
                Span::styled(s, glyph_style),
                Span::raw(" "),
                Span::styled(actor.name().to_string(), name_style),
                Span::raw(" "),
            ];
            spans.extend(hp_bar_spans(actor.hitpoints(), actor.max_hitpoints(), 8));
            initiative_lines.push(Line::from(spans));
        }
    }
    frame.render_widget(
        Paragraph::new(initiative_lines)
            .block(Block::default().borders(Borders::ALL).title("Initiative")),
        area_split[0],
    );

    // Resources / Actions panels need a valid prompt with a known actor.
    let Some((curr_actor_id, actions)) = prompt_info else {
        let msg = match stack_state {
            StackState::Processing => "(processing...)",
            _ => "(no prompt)",
        };
        frame.render_widget(
            Paragraph::new(msg)
                .block(Block::default().borders(Borders::ALL).title("Resources")),
            area_split[1],
        );
        frame.render_widget(
            Paragraph::new("").block(Block::default().borders(Borders::ALL).title("Actions")),
            area_split[2],
        );
        return;
    };

    let Some(curr_actor) = encounter.actors.get(&curr_actor_id) else {
        frame.render_widget(
            Paragraph::new("(missing actor)")
                .block(Block::default().borders(Borders::ALL).title("Resources")),
            area_split[1],
        );
        frame.render_widget(
            Paragraph::new("").block(Block::default().borders(Borders::ALL).title("Actions")),
            area_split[2],
        );
        return;
    };

    let hp = curr_actor.hitpoints();
    let max_hp = curr_actor.max_hitpoints();
    let ac = curr_actor.armor_class();
    let movement = curr_actor.remaining_movement();
    let action_slots = curr_actor.action_slots();
    let bonus_slots = curr_actor.bonus_action_slots();

    let mut hp_spans: Vec<Span<'static>> = vec![Span::raw("HP: ")];
    hp_spans.extend(hp_bar_spans(hp, max_hp, 10));
    hp_spans.push(Span::raw(format!(" {}/{}", hp, max_hp)));
    // Tack on temp-HP as a compact "(+N temp)" suffix when present —
    // pretending it's part of the HP line keeps the panel narrow.
    if curr_actor.temp_hp() > 0 {
        hp_spans.push(Span::styled(
            format!(" (+{} temp)", curr_actor.temp_hp()),
            Style::default().fg(Color::LightCyan),
        ));
    }

    let mut stats_lines: Vec<Line<'static>> = vec![
        Line::from(hp_spans),
        Line::from(Span::raw(format!("AC: {}", ac))),
        Line::from(Span::raw(format!("Movement: {:.0}", movement))),
        Line::from(Span::raw(format!(
            "Actions: {}  Bonus: {}",
            action_slots, bonus_slots
        ))),
    ];
    // Dodge / Disengage are turn-scoped flags — surface them so the
    // player can see they're spending their action on defense rather
    // than offense.
    let mut stance_tags: Vec<&str> = Vec::new();
    if curr_actor.is_dodging() {
        stance_tags.push("Dodging");
    }
    if curr_actor.is_disengaging() {
        stance_tags.push("Disengaging");
    }
    if !stance_tags.is_empty() {
        stats_lines.push(Line::from(Span::styled(
            format!("Stance: {}", stance_tags.join(", ")),
            Style::default().fg(Color::LightCyan),
        )));
    }
    // Concentration target — the spell name is enough; full effect tree
    // already lives in the log.
    if let Some(conc) = curr_actor.concentration() {
        stats_lines.push(Line::from(Span::styled(
            format!("Concentrating: {}", conc.spell_name),
            Style::default().fg(Color::LightMagenta),
        )));
    }
    // Buff totals (Bless, etc.) — only show when nonzero so we don't
    // clutter the panel for everyone else.
    let atk_buff = curr_actor.attack_bonus_buff();
    let save_buff = curr_actor.save_bonus_buff();
    if atk_buff != 0 || save_buff != 0 {
        stats_lines.push(Line::from(Span::styled(
            format!("Buffs: atk{:+}  save{:+}", atk_buff, save_buff),
            Style::default().fg(Color::LightGreen),
        )));
    }
    // Level/XP only shown for PCs (team 0) — monsters have stub values
    // (level=1, xp=0) that would clutter the panel without conveying info.
    if curr_actor.team() == 0 {
        stats_lines.push(Line::from(Span::raw(format!(
            "Level: {}  XP: {}/{}",
            curr_actor.level(),
            curr_actor.xp(),
            curr_actor.xp_threshold_for_next_level()
        ))));
    }
    if !curr_actor.items().is_empty() {
        // Show carried items as a compact comma-joined list. Bonuses are
        // already folded into HP/AC/Movement above, so this is a "what's
        // attributable to gear" callout rather than per-item detail.
        let names: Vec<String> = curr_actor
            .items()
            .iter()
            .map(|i| i.name.to_string())
            .collect();
        stats_lines.push(Line::from(Span::styled(
            format!("Items: {}", names.join(", ")),
            Style::default().fg(Color::Yellow),
        )));
    }
    // Damage modifier callout — only render if the creature has any. Most
    // PCs have nothing here, so the line stays hidden in the common case.
    let immunities = curr_actor.immunities();
    let resistances = curr_actor.resistances();
    let vulnerabilities = curr_actor.vulnerabilities();
    if !immunities.is_empty() || !resistances.is_empty() || !vulnerabilities.is_empty() {
        let fmt = |set: &std::collections::HashSet<crate::engine::types::DamageType>| {
            let mut names: Vec<String> =
                set.iter().map(|d| format!("{:?}", d)).collect();
            names.sort_unstable();
            names.join(",")
        };
        let mut parts: Vec<String> = Vec::new();
        if !immunities.is_empty() {
            parts.push(format!("Imm:{}", fmt(immunities)));
        }
        if !resistances.is_empty() {
            parts.push(format!("Res:{}", fmt(resistances)));
        }
        if !vulnerabilities.is_empty() {
            parts.push(format!("Vul:{}", fmt(vulnerabilities)));
        }
        stats_lines.push(Line::from(Span::styled(
            parts.join(" "),
            Style::default().fg(Color::Cyan),
        )));
    }
    if !curr_actor.conditions().is_empty() {
        // Format each condition with its remaining duration when timed.
        // Sort alphabetically so HashMap iteration order doesn't leak.
        let mut entries: Vec<(String, &'static str)> = curr_actor
            .conditions()
            .iter()
            .map(|(c, timer)| {
                let label = match timer {
                    crate::conditions::ConditionTimer::Permanent => c.name().to_string(),
                    crate::conditions::ConditionTimer::Rounds(n) => format!("{}({})", c.name(), n),
                    crate::conditions::ConditionTimer::UntilStartOfNextTurn => {
                        format!("{}*", c.name())
                    }
                };
                (label, c.name())
            })
            .collect();
        entries.sort_unstable_by(|a, b| a.1.cmp(b.1));
        let labels: Vec<String> = entries.into_iter().map(|(l, _)| l).collect();
        stats_lines.push(Line::from(Span::styled(
            format!("Conditions: {}", labels.join(", ")),
            Style::default().fg(Color::Yellow),
        )));
    }
    push_damage_set_line(
        &mut stats_lines,
        "Resistant",
        curr_actor.damage_resistances(),
        Color::Cyan,
    );
    push_damage_set_line(
        &mut stats_lines,
        "Immune",
        curr_actor.damage_immunities(),
        Color::Green,
    );
    push_damage_set_line(
        &mut stats_lines,
        "Vulnerable",
        curr_actor.damage_vulnerabilities(),
        Color::Red,
    );
    frame.render_widget(
        Paragraph::new(stats_lines)
            .block(Block::default().borders(Borders::ALL).title("Resources")),
        area_split[1],
    );

    let n_actions = actions.len();
    let highlight_idx = if n_actions > 0 {
        selected_action_idx % n_actions
    } else {
        0
    };
    let action_lines: Vec<Line<'static>> = actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let is_selected = i == highlight_idx;
            let prefix = if is_selected { "> " } else { "  " };
            // Look up costs with placeholder args. Static-cost actions
            // return their full cost list here; context-sensitive ones
            // (Move) return an empty list at this stage.
            let costs = action.cost(encounter, curr_actor_id, None, None, None);
            let unaffordable = costs.iter().any(|c| !curr_actor.can_consume_resource(*c));
            let tag = cost_label(&costs, action.name());

            let base_style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if unaffordable {
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::raw(prefix),
                Span::styled(tag, base_style),
                Span::raw(" "),
                Span::styled(action.name().to_string(), base_style),
            ])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(action_lines)
            .block(Block::default().borders(Borders::ALL).title("Actions")),
        area_split[2],
    );
}
