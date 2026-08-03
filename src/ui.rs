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

/// Render the actor's damage modifier table as up to three colored
/// lines (Resistant / Immune / Vulnerable). Each line is suppressed if
/// the corresponding bucket is empty so PCs without any modifiers don't
/// show empty rows. Damage types are sorted alphabetically so the output
/// is stable across runs.
fn push_damage_modifier_line(
    stats_lines: &mut Vec<Line<'static>>,
    actor: &crate::actors::actor_template::ActorInstance,
) {
    use crate::engine::types::{DamageModifier, DamageType};
    let all = [
        DamageType::Acid,
        DamageType::Bludgeoning,
        DamageType::Cold,
        DamageType::Fire,
        DamageType::Force,
        DamageType::Lightning,
        DamageType::Necrotic,
        DamageType::Piercing,
        DamageType::Poison,
        DamageType::Psychic,
        DamageType::Radiant,
        DamageType::Slashing,
        DamageType::Thunder,
    ];
    let mut buckets: [Vec<String>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for dt in all {
        match actor.damage_modifier(dt) {
            Some(DamageModifier::Resistance) => buckets[0].push(format!("{:?}", dt)),
            Some(DamageModifier::Immunity) => buckets[1].push(format!("{:?}", dt)),
            Some(DamageModifier::Vulnerability) => buckets[2].push(format!("{:?}", dt)),
            None => {}
        }
    }
    let labeled = [
        ("Resistant", Color::Cyan),
        ("Immune", Color::Green),
        ("Vulnerable", Color::Red),
    ];
    for (i, (label, color)) in labeled.iter().enumerate() {
        if buckets[i].is_empty() {
            continue;
        }
        stats_lines.push(Line::from(Span::styled(
            format!("{}: {}", label, buckets[i].join(", ")),
            Style::default().fg(*color),
        )));
    }
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
            } else if let Some(glyph) = zone_glyph(encounter, coord) {
                // A persistent magical area draws over bare ground —
                // above terrain (the fog is what matters about the tile
                // now) and below actors and loot (a creature standing
                // in the web is still the thing you need to see).
                row.push(Span::styled(
                    glyph.to_string(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                let s = Span::from(
                    match encounter.terrain_at(coord).map(|t| &t.terrain_type) {
                        Some(TerrainType::Floor) => '░',
                        Some(TerrainType::Wall) => '█',
                        Some(TerrainType::DifficultTerrain) => '▒',
                        // Denser than rubble's '▒' and lighter than a
                        // wall's '█', which is what a low wall is: you
                        // can cross it and you can shoot over it, but
                        // both cost you something.
                        Some(TerrainType::LowWall) => '▓',
                        // A pane you can see through and not walk
                        // through. Drawn as an outline rather than a
                        // fill for exactly that reason — what is behind
                        // it is still in play.
                        Some(TerrainType::ForceWall) => '╬',
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

/// The character to draw for a tile under one or more persistent
/// magical areas, or `None` for a tile under none.
///
/// One glyph per kind of clause, ranked by what a player most needs to
/// know before they step there:
///
///   - `☠` — it will hurt you (a cloud of daggers, a moonbeam).
///   - `≈` — it will hold you (a web, a patch of grease).
///   - `▚` — you cannot see through it (a fog cloud).
///
/// A tile carrying more than one shows the most urgent, which is why
/// the checks are in that order rather than in the order the zones were
/// installed: standing in a web inside a fog bank, the knives are still
/// the news.
fn zone_glyph(encounter: &EncounterInstance, coord: Coordinate) -> Option<char> {
    let mut found: Option<char> = None;
    for zone in encounter.zones() {
        if !zone.covers(coord) {
            continue;
        }
        let glyph = if zone.effect.contact.is_some_and(|c| c.damage.is_some()) {
            '☠'
        } else if zone.effect.is_harmful() || zone.effect.difficult {
            '≈'
        } else if zone.effect.obscures {
            '▚'
        } else {
            continue;
        };
        let rank = |g: char| match g {
            '☠' => 2,
            '≈' => 1,
            _ => 0,
        };
        if found.is_none_or(|f| rank(glyph) > rank(f)) {
            found = Some(glyph);
        }
    }
    found
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
    for (position, slot) in encounter.initiative_slots().into_iter().enumerate() {
        let actor_id = slot.actor_id;
        // The list is rotated to start at the active slot, so "current"
        // is a question about position, not about identity. It has to
        // be: an actor can hold more than one slot in the queue (a Thief
        // Rogue's Thief's Reflexes gives them two in round 1), and
        // matching on the id alone would draw the marker on both — the
        // one acting now and the one still waiting ten points down.
        let is_current =
            position == 0 && prompt_info.as_ref().is_some_and(|(id, _)| *id == actor_id);
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
            // A bonus slot (Thief's Reflexes) puts the same name in the
            // list twice. Say which row is the spare, or the panel reads
            // as a rendering fault rather than as the feature.
            if slot.is_extra {
                spans.push(Span::styled(
                    " +turn".to_string(),
                    Style::default().fg(Color::LightMagenta),
                ));
            }
            initiative_lines.push(Line::from(spans));
        }
    }
    // Persistent areas ride under the initiative queue, because they
    // are the other thing on the board with a turn counter on it. The
    // map already shows *where* they are; what the map cannot show is
    // how long they last, which is exactly the number a player needs to
    // decide whether waiting one more round is cheaper than crossing.
    if !encounter.zones().is_empty() {
        initiative_lines.push(Line::from(""));
        for zone in encounter.zones() {
            let glyph = zone_glyph(encounter, zone.origin).unwrap_or('·');
            initiative_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    glyph.to_string(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    zone.name.to_string(),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!(" {} — {}r", zone.origin, zone.rounds_remaining),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }
    // The seed rides the panel title because it is the one number a
    // player wants *after* the fight rather than during it — to replay a
    // good encounter, or to hand over with a bug report. Every encounter
    // has one now, including the ones started without a seed argument.
    let init_title = format!(
        "Initiative — Round {} — seed {}",
        encounter.round(),
        encounter.seed()
    );
    frame.render_widget(
        Paragraph::new(initiative_lines)
            .block(Block::default().borders(Borders::ALL).title(init_title)),
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
    // 5e Abjuration Wizard Arcane Ward — a third HP pool that soaks
    // damage ahead of temp HP. Shown as "(ward N/M)" once woven so the
    // player can see how much absorption is left and whether another
    // abjuration cast would be wasted on a full ward. Deliberately still
    // rendered at 0/M: RAW keeps a drained ward rechargeable, and hiding
    // it there would read as "the feature is gone".
    if curr_actor.arcane_ward_formed() {
        hp_spans.push(Span::styled(
            format!(
                " (ward {}/{})",
                curr_actor.arcane_ward(),
                curr_actor.arcane_ward_max()
            ),
            Style::default().fg(Color::LightBlue),
        ));
    }
    // 5e Divination Wizard Portent — the banked foretold faces, shown
    // as "(portent 19, 3)" so the player can see both how many
    // substitutions are left and, crucially, *which* ones: a 19 and a 3
    // in the bank tell them entirely different things about the next
    // few rounds than a 12 and an 11 would. Rendered only once the
    // forecast exists and only while dice remain — unlike the ward,
    // an empty pool really is gone until the next long rest.
    if !curr_actor.portent_pool().is_empty() {
        hp_spans.push(Span::styled(
            format!(
                " (portent {})",
                curr_actor
                    .portent_pool()
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Style::default().fg(Color::LightMagenta),
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
    // Size is fixed for all but a handful of creatures, so this line
    // appears only when something has moved it — or is trying to. The
    // refused case is the one that most needs saying: a Rune Knight who
    // spends Giant's Might in a corridor has burned the charge, is
    // holding the condition, and has exactly the footprint they started
    // with, which is otherwise invisible. Naming what they are waiting
    // to become tells them to take a step.
    let size = curr_actor.size();
    let wanted = curr_actor.desired_size();
    if size != curr_actor.base_size() || wanted != size {
        let text = if wanted != size {
            format!("Size: {} — no room to become {}", size, wanted)
        } else {
            format!("Size: {}", size)
        };
        stats_lines.push(Line::from(Span::styled(
            text,
            Style::default().fg(Color::LightYellow),
        )));
    }
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
    if !curr_actor.conditions().is_empty() {
        // Format each condition with its remaining duration when timed.
        // Sort alphabetically so HashMap iteration order doesn't leak.
        let mut entries: Vec<(String, &'static str)> = curr_actor
            .conditions()
            .iter()
            .map(|(c, timer)| {
                // Exhaustion is the one condition whose severity isn't
                // carried by its timer — it has six rungs and a
                // permanent timer on every one of them, so a bare
                // "exhausted" would read the same at tier 1 as at the
                // tier that halves your hit points. Show the rung.
                if matches!(c, crate::conditions::Condition::Exhausted) {
                    return (
                        format!("{} {}", c.name(), curr_actor.exhaustion_level()),
                        c.name(),
                    );
                }
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
    // Damage modifier callout — only render if the creature has any.
    push_damage_modifier_line(&mut stats_lines, curr_actor);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
    use crate::actors::creatures::rogues::THIEF_ROGUE_TEMPLATE;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::terrain_gen::TerrainGenParams;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Render `render_sideinfo` into an off-screen buffer and return the
    /// panel as plain text, one line per row.
    ///
    /// The two panels this module draws had no coverage at all, which is
    /// how a duplicated initiative row and a missing seed both went
    /// unnoticed as *presentation* problems rather than as engine ones.
    /// A `TestBackend` render is the cheapest thing that can tell the
    /// difference.
    fn rendered_panel(encounter: &EncounterInstance) -> String {
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).expect("test backend");
        terminal
            .draw(|f| {
                let area = f.area();
                render_sideinfo(encounter, f, area, 0);
            })
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn encounter_with(actors: &[(&'static crate::actors::actor_template::CreatureTemplate, usize)])
    -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(4)).unwrap();
        // Spawned at generator-chosen floor tiles rather than fixed
        // coordinates: this module's tests are about what the panel
        // prints, and where anybody is standing doesn't reach it.
        for (i, (template, team)) in actors.iter().enumerate() {
            let loc = e.get_random_spawn(template.size).expect("a floor tile");
            e.instantiate_creature(template, loc, *team, i)
                .expect("instantiate");
        }
        e
    }

    /// The seed is on screen. Every encounter has one now, and it is
    /// only useful if the player can read it back off the panel after a
    /// fight worth replaying.
    #[test]
    fn the_initiative_panel_shows_the_seed() {
        let e = encounter_with(&[(&GOBLIN_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        let panel = rendered_panel(&e);
        assert!(
            panel.contains("seed 4"),
            "the seed should be readable off the panel:\n{}",
            panel
        );
    }

    /// A persistent area is listed with the rounds it has left. The map
    /// shows where the web is; only the panel can say how much longer
    /// waiting it out would cost.
    #[test]
    fn the_initiative_panel_counts_down_a_persistent_area() {
        use crate::engine::types::AbilityScoreType;
        use crate::engine::zones::{Zone, ZoneContact, ZoneEffect, ZoneMotion};

        let mut e = encounter_with(&[(&GOBLIN_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        assert!(!rendered_panel(&e).contains("web"));
        e.install_zone(Zone {
            id: 0,
            name: "web",
            owner_id: 0,
            origin: Coordinate::new(4, 4),
            radius: 2,
            effect: ZoneEffect::clinging(ZoneContact::save_or(
                AbilityScoreType::Dexterity,
                13,
                crate::conditions::Condition::Restrained,
                crate::conditions::ConditionTimer::Rounds(10),
            )),
            rounds_remaining: 7,
            concentration: true,
            motion: ZoneMotion::Fixed,
        });
        let panel = rendered_panel(&e);
        assert!(
            panel.contains("web") && panel.contains("7r"),
            "the panel should name the area and its remaining rounds:\n{}",
            panel
        );
    }

    /// A Thief's bonus slot is labelled, so the same name appearing
    /// twice in round 1 reads as the feature rather than as a rendering
    /// fault — and the label is gone once the extra turn retires.
    #[test]
    fn the_initiative_panel_labels_a_bonus_turn() {
        let mut e = encounter_with(&[(&THIEF_ROGUE_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        let panel = rendered_panel(&e);
        assert_eq!(
            panel.matches("Thief Rogue").count(),
            2,
            "the Thief holds two slots in round 1:\n{}",
            panel
        );
        assert_eq!(
            panel.matches("+turn").count(),
            1,
            "exactly one of them is marked as the spare:\n{}",
            panel
        );

        // Walk out of round 1; the spare slot and its label go together.
        for _ in 0..e.initiative_slots().len() {
            e.skip_turn();
        }
        let panel = rendered_panel(&e);
        assert_eq!(
            panel.matches("Thief Rogue").count(),
            1,
            "one slot after round 1:\n{}",
            panel
        );
        assert!(!panel.contains("+turn"), "and no label left:\n{}", panel);
    }
}
