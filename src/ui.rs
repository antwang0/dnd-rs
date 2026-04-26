use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::encounter::{EncounterInstance, StackState};
use crate::engine::terrain::TerrainType;
use crate::engine::types::Coordinate;
use crate::engine::util::get_colored_span;

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
                    get_colored_span(actor_id, actor.team());
                let mut style = Style::default().fg(c).bg(bg);
                if Some(actor_id) == active_actor_id && !blink_on() {
                    style = style.add_modifier(Modifier::REVERSED);
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
    let prompt_info: Option<(usize, Vec<String>)> = if let StackState::AwaitingPrompt(actor_id) =
        stack_state
    {
        // Pull the prompt's action list. peek_prompt only returns Some on
        // AwaitingPrompt, so this is fine even though stack_state already
        // told us the actor.
        encounter.peek_prompt().map(|p| {
            (
                actor_id,
                p.actions().iter().map(|a| a.name().to_string()).collect(),
            )
        })
    } else {
        None
    };

    // Initiative queue: show all actors in turn order, starting from the
    // current actor; highlight + blink-glyph the active one.
    let mut initiative_lines: Vec<Line<'static>> = Vec::new();
    for actor_id in encounter.initiative_actor_ids() {
        let is_current = prompt_info.as_ref().is_some_and(|(id, _)| *id == actor_id);
        if let Some(actor) = encounter.actors.get(&actor_id) {
            let (s, c, bg) = get_colored_span(actor_id, actor.team());
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
    let Some((curr_actor_id, action_names)) = prompt_info else {
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

    let stats_lines: Vec<Line<'static>> = vec![
        Line::from(hp_spans),
        Line::from(Span::raw(format!("AC: {}", ac))),
        Line::from(Span::raw(format!("Movement: {:.0}", movement))),
        Line::from(Span::raw(format!(
            "Actions: {}  Bonus: {}",
            action_slots, bonus_slots
        ))),
    ];
    frame.render_widget(
        Paragraph::new(stats_lines)
            .block(Block::default().borders(Borders::ALL).title("Resources")),
        area_split[1],
    );

    let n_actions = action_names.len();
    let highlight_idx = if n_actions > 0 {
        selected_action_idx % n_actions
    } else {
        0
    };
    let action_lines: Vec<Line<'static>> = action_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let is_selected = i == highlight_idx && n_actions > 0;
            let prefix = if is_selected { "> " } else { "  " };
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::from(vec![Span::raw(prefix), Span::styled(name.clone(), style)])
        })
        .collect();
    frame.render_widget(
        Paragraph::new(action_lines)
            .block(Block::default().borders(Borders::ALL).title("Actions")),
        area_split[2],
    );
}
