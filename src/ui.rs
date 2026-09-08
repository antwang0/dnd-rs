use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::actions::action_template::Action;
use crate::engine::encounter::{EncounterInstance, StackState};
use crate::engine::lighting::LightLevel;
use crate::engine::side_effects::Resource;
use crate::engine::terrain::TerrainType;
use crate::engine::types::Coordinate;
use crate::engine::util::get_colored_span;

/// Render the actor's damage modifier table as up to five colored
/// lines — Resistant / Immune / Vulnerable / Healed by, plus the
/// source-qualified "Resistant vs mundane" row. Each line is suppressed
/// if the corresponding bucket is empty so PCs without any modifiers
/// don't show empty rows. Damage types are sorted alphabetically so the
/// output is stable across runs.
///
/// "Healed by" gets its own row rather than sharing "Immune" for the
/// same reason "Resistant vs mundane" does: it is the difference
/// between a wasted Fireball and one that undoes the party's last two
/// rounds, and a player reading "Immune: Fire" off an Iron Golem would
/// have no way to know which they were looking at.
fn push_damage_modifier_line(
    stats_lines: &mut Vec<Line<'static>>,
    actor: &crate::actors::actor_template::ActorInstance,
) {
    use crate::engine::types::{DamageModifier, DamageType};
    let mut buckets: [Vec<String>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    // `DamageType::ALL` rather than a copy of the enum: this panel's
    // whole job is to be exhaustive, and a hand-written list is
    // exhaustive only until somebody adds a fourteenth damage type and
    // forgets this file. The shared array is the same one
    // `DamageTypeSet` assigns bits from, so the two can't disagree.
    for dt in DamageType::ALL {
        match actor.damage_modifier(dt) {
            Some(DamageModifier::Resistance) => buckets[0].push(format!("{:?}", dt)),
            Some(DamageModifier::Immunity) => buckets[1].push(format!("{:?}", dt)),
            Some(DamageModifier::Vulnerability) => buckets[2].push(format!("{:?}", dt)),
            Some(DamageModifier::Absorption) => buckets[3].push(format!("{:?}", dt)),
            None => {}
        }
    }
    let labeled = [
        ("Resistant", Color::Cyan),
        ("Immune", Color::Green),
        ("Vulnerable", Color::Red),
        ("Healed by", Color::Magenta),
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
    // The source-qualified rows, on their own line and named for the
    // qualifier. Forty stat blocks on the roster carry
    // "resistance to bludgeoning, piercing, and slashing damage from
    // nonmagical attacks" and *only* that, so without this line the
    // panel whose whole job is to be exhaustive shows a wraith as
    // having no damage modifiers at all.
    //
    // Worth a separate line rather than folding into "Resistant"
    // above, because the qualifier is the single most actionable fact
    // on this panel: it is the difference between "do not bother
    // swinging" and "swing, but draw the +1 first".
    let qualified: Vec<String> = DamageType::ALL
        .into_iter()
        .filter(|dt| actor.nonmagical_damage_modifier(*dt) == Some(DamageModifier::Resistance))
        .map(|dt| format!("{:?}", dt))
        .collect();
    if !qualified.is_empty() {
        stats_lines.push(Line::from(Span::styled(
            format!("Resistant vs mundane: {}", qualified.join(", ")),
            Style::default().fg(Color::Cyan),
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

/// The background a tile gets when it is inside the area the player is
/// currently aiming — see `render_map`'s `preview` argument.
///
/// A background rather than a glyph, so the preview composes with
/// everything already on the tile instead of replacing it: the player
/// needs to see *which creatures* are in the cone, and a shaded tile
/// with a goblin still on it says that where a shaded tile that erased
/// the goblin would not.
///
/// It does overwrite the one thing the tile background carries on its
/// own — team 0's light-cyan silhouette — which is an acceptable trade
/// because the team is still on the glyph's foreground colour, and
/// because a player aiming a cone is asking a question the shading
/// answers and the silhouette does not.
const AREA_PREVIEW_BG: Color = Color::DarkGray;

pub fn render_map(
    encounter: &EncounterInstance,
    frame: &mut Frame,
    area: Rect,
    highlighted_target: Option<usize>,
    preview: &std::collections::HashSet<Coordinate>,
) {
    let mut text: Vec<Line> = Vec::new();

    let active_actor_id = match encounter.stack_state() {
        StackState::AwaitingPrompt(id) => Some(id),
        _ => None,
    };

    // Whether the map needs a lighting pass at all. On the lit-board
    // default nothing below can change any tile's shade, and the check
    // is cheaper than asking `light_at` once per tile for the answer
    // "bright" forty times a row.
    let lit_board = encounter.ambient_light().level() == LightLevel::Bright
        && encounter.light_sources().is_empty();

    for y in (0..encounter.height).rev() {
        let mut row: Vec<Span> = Vec::new();
        for x in 0..encounter.width {
            let coord = Coordinate::new(x as isize, y as isize);
            // The light this tile sits in, from the active actor's own
            // eyes — darkvision included, which is the whole point. A
            // goblin player sees sixty feet of grey corridor where a
            // human player sees the reach of their torch and nothing
            // else, and the map is the only place that difference can
            // be shown.
            //
            // Falls back to the objective light when nobody is being
            // prompted (an AI turn resolving, the encounter over), so
            // the map never goes blank between frames.
            let light = if lit_board {
                LightLevel::Bright
            } else {
                match active_actor_id {
                    Some(viewer) => encounter.perceived_light(viewer, coord),
                    None => encounter.light_at(coord),
                }
            };
            if let Some(occupant_id) = encounter.actor_id_at(coord)
                && let Some(occupant) = encounter.actors.get(&occupant_id)
            {
                // A mounted rider has no stamp on the grid — the mount
                // owns the pair's tiles (see `engine::mounts`) — so the
                // map would draw a horse and no knight at all. Draw the
                // rider on the mount's *anchor* tile and the mount
                // everywhere else: the pair is one body to walk into and
                // two creatures to shoot at, and the map has room to say
                // both.
                //
                // Falls back to the mount if the link points at somebody
                // who is no longer in the table, for the same reason the
                // outer `if let` exists: a frame can land between a
                // death and its cleanup, and a renderer is the wrong
                // place to find out.
                //
                // The attach link (`engine::attachment`) takes its
                // passenger off the grid the same way and is
                // deliberately *not* given the same treatment: a
                // mounted pair is two creatures the player chose to
                // stack, where a latched one is a monster on a party
                // member's face, and drawing the monster would erase
                // the character. The panel says "gripped by" instead —
                // which is also the row that carries the numbers a
                // player needs (whose turn drains them, how long).
                let (actor_id, actor) = match occupant.ridden_by() {
                    Some(rider_id) if occupant.location() == coord => encounter
                        .actors
                        .get(&rider_id)
                        .map_or((occupant_id, occupant), |r| (rider_id, r)),
                    _ => (occupant_id, occupant),
                };
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
                // Terrain draws in the shade ramp — the denser the
                // block, the more the tile costs you — with one
                // exception. Water is the only tile whose rules are not
                // about how hard it is to cross (it charges the same
                // double every scatter tile does), so a place on the
                // ramp would say the wrong thing about it: a player
                // reading '▒' has learned the tile is slow, and needs
                // to have learned that their bow does not work there.
                // It gets a colour instead, which is the only channel
                // the map has left that the ramp isn't already using.
                let (glyph, color) = match encounter.terrain_at(coord).map(|t| t.terrain_type) {
                    Some(TerrainType::Floor) => ('░', None),
                    Some(TerrainType::Wall) => ('█', None),
                    Some(TerrainType::DifficultTerrain) => ('▒', None),
                    // Denser than rubble's '▒' and lighter than a
                    // wall's '█', which is what a low wall is: you
                    // can cross it and you can shoot over it, but
                    // both cost you something.
                    Some(TerrainType::LowWall) => ('▓', None),
                    // A pane you can see through and not walk
                    // through. Drawn as an outline rather than a
                    // fill for exactly that reason — what is behind
                    // it is still in play.
                    Some(TerrainType::ForceWall) => ('╬', None),
                    Some(TerrainType::Water) => ('≈', Some(Color::Blue)),
                    _ => (' ', None),
                };
                row.push(match color {
                    Some(c) => Span::styled(glyph.to_string(), Style::default().fg(c)),
                    None => Span::from(glyph.to_string()),
                });
            }
            // The aiming preview, applied to whatever was drawn above
            // for the same reason the lighting pass below is: a new
            // kind of tile should not be able to fall out of it.
            //
            // Before the lighting pass rather than after, so a tile the
            // viewer cannot see stays blank. A cone drawn through the
            // dark would otherwise tell the player exactly where the
            // walls are.
            if !preview.is_empty()
                && preview.contains(&coord)
                && let Some(span) = row.last_mut()
            {
                span.style = span.style.bg(AREA_PREVIEW_BG);
            }
            // The lighting pass, applied to whatever was drawn above —
            // terrain, zone, loot or creature alike — rather than to
            // each branch, so a new kind of tile cannot be added
            // without it.
            //
            // Two rungs and not three. `Dim` is greyed, which says "you
            // can see this and not well"; `Dark` is blanked to a space,
            // which says the only honest thing a map can say about a
            // tile its viewer cannot see. Blanking rather than dimming
            // matters: a dimmed monster glyph is still a monster the
            // player has been told about, and the whole tactical
            // content of fighting in the dark is not knowing.
            if let Some(span) = row.last_mut() {
                match light {
                    LightLevel::Bright => {}
                    LightLevel::Dim => {
                        span.style = span.style.add_modifier(Modifier::DIM);
                    }
                    LightLevel::Dark => {
                        *span = Span::from(" ");
                    }
                }
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
        // A set ward outranks everything, including a skull it is
        // sitting under. The map is the player's-eye view of the board
        // and a ward they laid is the one area whose *position* they
        // most need back — a Glyph of Warding is invisible to the
        // pathfinder (see `ZoneEffect::ward`), so the map is the only
        // record of where it is. Its own glyph rather than the skull,
        // because "armed and waiting" and "burning right now" are the
        // two facts a player standing next to one has to tell apart.
        let glyph = if zone.effect.ward.is_some() {
            '◈'
        } else if zone.effect.contact.is_some_and(|c| c.damage.is_some()) {
            '☠'
        } else if zone.effect.deters_walkers() || zone.effect.difficult {
            '≈'
        } else if zone.effect.obscures {
            '▚'
        } else {
            continue;
        };
        let rank = |g: char| match g {
            '◈' => 3,
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
            // A banished creature, said on the row for the same reason
            // the mounted pair below is: its slot passes straight
            // through, and it is not on the map to be looked for. The
            // countdown is the whole reason its row is still here —
            // the party's remaining rounds before an enemy comes back,
            // or before their own ally does.
            if actor.is_off_board() {
                spans.push(Span::styled(
                    match actor.off_board_rounds_left() {
                        Some(n) => format!(" banished ({}r)", n),
                        None => " banished".to_string(),
                    },
                    Style::default().fg(Color::LightMagenta),
                ));
            }
            // A boss's remaining legendary actions, which is the one
            // number in the fight the player has to plan around and
            // cannot see anywhere else. It is not a resource the action
            // panel can show — a legendary action is never on anybody's
            // action list, because it is spent between turns rather
            // than on one — so without this row the party's only
            // evidence that a dragon has two tail swipes left is the
            // log line from the last one it took.
            //
            // Drawn only while the creature can still afford something:
            // a spent boss is back to being an ordinary row, and a
            // permanent "Lg 0/3" on the queue is noise for the whole
            // second half of every round.
            if actor.legendary_actions_per_round() > 0 && actor.legendary_action_slots() > 0 {
                spans.push(Span::styled(
                    format!(
                        " Lg {}/{}",
                        actor.legendary_action_slots(),
                        actor.legendary_actions_per_round()
                    ),
                    Style::default().fg(Color::LightRed),
                ));
            }
            // The two halves of a mounted pair, said on the row rather
            // than left to the map. A ridden mount's slot passes
            // straight through — it acts on its rider's turn — so
            // without this the panel shows a warhorse that is never
            // reached and no reason why.
            if let Some(rider_id) = actor.ridden_by() {
                spans.push(Span::styled(
                    format!(" ridden by {}", encounter.actor_name(rider_id)),
                    Style::default().fg(Color::LightGreen),
                ));
            } else if let Some(mount_id) = actor.mounted_on() {
                spans.push(Span::styled(
                    format!(" riding {}", encounter.actor_name(mount_id)),
                    Style::default().fg(Color::LightGreen),
                ));
            }
            // The attach link, both ends, on the same row and for the
            // same reason the mount link is: an attached creature has
            // no stamp of its own on the grid, so the map can only draw
            // one of the pair, and the half it cannot draw is the one
            // that explains why the other is Blinded and losing hit
            // points every round.
            //
            // Red rather than green, which is the one thing this says
            // that the mounted line does not: nobody up there chose it.
            if let Some(host_id) = actor.attached_to() {
                spans.push(Span::styled(
                    format!(" on {}", encounter.actor_name(host_id)),
                    Style::default().fg(Color::LightRed),
                ));
            } else {
                let latched = encounter.attachers_on(actor_id);
                if !latched.is_empty() {
                    let names: Vec<String> = latched
                        .iter()
                        .map(|&a| encounter.actor_name(a))
                        .collect();
                    spans.push(Span::styled(
                        format!(" gripped by {}", names.join(", ")),
                        Style::default().fg(Color::LightRed),
                    ));
                }
            }
            // The swallow link, both ends, for exactly the reason the
            // attach link above it is here: a swallowed creature has no
            // stamp on the grid, so the map draws only the thing that
            // ate it. Without this row a player watching their fighter
            // vanish off the map has no line anywhere in the interface
            // telling them where they went — and Total Cover means they
            // cannot find out by trying to target them either.
            //
            // The count on the swallower's side is the number the
            // regurgitation clause is worth reading: everybody comes
            // back up together, so four names is four rescues off one
            // failed save.
            if let Some(swallower_id) = actor.swallowed_by() {
                spans.push(Span::styled(
                    format!(" inside {}", encounter.actor_name(swallower_id)),
                    Style::default().fg(Color::LightRed),
                ));
            } else {
                let eaten = encounter.swallowed_in(actor_id);
                if !eaten.is_empty() {
                    let names: Vec<String> =
                        eaten.iter().map(|&a| encounter.actor_name(a)).collect();
                    spans.push(Span::styled(
                        format!(" has swallowed {}", names.join(", ")),
                        Style::default().fg(Color::LightRed),
                    ));
                }
            }
            initiative_lines.push(Line::from(spans));
        }
    }
    // Persistent areas ride under the initiative queue, because they
    // are the other thing on the board with a turn counter on it. The
    // map already shows *where* they are; what the map cannot show is
    // how long they last, which is exactly the number a player needs to
    // decide whether waiting one more round is cheaper than crossing.
    //
    // Conjured terrain rides the same block for the same reason. Its
    // glyph is already on the map — a wall of stone looks exactly like
    // a wall, which is the point — so the map is the one thing that
    // *cannot* tell a player that the corridor they are looking at is
    // going to reopen in four rounds.
    if !encounter.zones().is_empty() || !encounter.conjured_terrain().is_empty() {
        initiative_lines.push(Line::from(""));
        let mut layer_line = |glyph: char, name: &str, detail: String| {
            initiative_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    glyph.to_string(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(name.to_string(), Style::default().fg(Color::Cyan)),
                Span::styled(detail, Style::default().fg(Color::DarkGray)),
            ]));
        };
        for zone in encounter.zones() {
            let glyph = zone_glyph(encounter, zone.origin).unwrap_or('·');
            layer_line(
                glyph,
                zone.name,
                format!(" {} — {}r", zone.origin, zone.rounds_remaining),
            );
        }
        for patch in encounter.conjured_terrain() {
            layer_line(
                '▚',
                patch.name,
                format!(
                    " {} tiles — {}r",
                    patch.restore.len(),
                    patch.rounds_remaining
                ),
            );
        }
    }
    // The seed rides the panel title because it is the one number a
    // player wants *after* the fight rather than during it — to replay a
    // good encounter, or to hand over with a bug report. Every encounter
    // has one now, including the ones started without a seed argument.
    // The ambient light rides in the title beside the seed, and only
    // when it isn't the lit-board default. A player fighting in the
    // dark needs to know that is *why* everything is missing; a player
    // on an ordinary board does not need a line telling them the lights
    // are on.
    let ambient = encounter.ambient_light();
    let lighting = if ambient == crate::engine::lighting::AmbientLight::default() {
        String::new()
    } else {
        format!(" — {}", ambient.label())
    };
    let init_title = format!(
        "Initiative — Round {} — seed {}{}",
        encounter.round(),
        encounter.seed(),
        lighting
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
            "Actions: {}{}  Bonus: {}",
            action_slots,
            // 5e Haste. A bare "Actions: 2" on a hasted creature is a
            // promise the action list will not keep: one of those two
            // buys only an attack, a Dash, a Disengage or a Hide, and
            // the player needs to know that before they plan a turn
            // around a second Fireball. Absent — as it is for every
            // creature that is not hasted — the line reads exactly as
            // it always did.
            match curr_actor.restricted_action_slots() {
                0 => String::new(),
                n => format!(" ({} haste)", n),
            },
            bonus_slots
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
    // Height above the floor, and what it will cost to lose it. Only
    // rendered for an actor actually in the air — 0 ft is every creature
    // almost always, and a line saying so on every panel would be noise.
    //
    // The die count is the point of showing it at all: "Airborne: 30 ft"
    // is trivia, "Airborne: 30 ft (3d6 on landing)" is the reason to
    // think twice before letting the concentration go. Read straight off
    // `fall_damage_dice` rather than divided out here, so the panel can
    // never disagree with what `resolve_fall` will roll.
    let altitude = curr_actor.altitude_ft();
    if altitude > 0 {
        let dice = crate::engine::falling::fall_damage_dice(altitude);
        let cushioned = curr_actor.has_condition(crate::conditions::Condition::Feathered);
        stats_lines.push(Line::from(Span::styled(
            if cushioned {
                format!("Airborne: {} ft (feather fall)", altitude)
            } else {
                format!("Airborne: {} ft ({} on landing)", altitude, dice)
            },
            Style::default().fg(Color::LightBlue),
        )));
    } else if curr_actor.base_fly_speed() > 0.0 {
        // A creature with wings that is not using them. The line exists
        // because the Movement number one row up has quietly changed
        // meaning: a wyvern in the air is moving 80 and the same wyvern
        // on the floor is moving 20, and without this the panel offers
        // no account of the difference.
        //
        // Which of the two rules put it there is worth naming, because
        // the counterplay differs. `Earthbound` is somebody's
        // concentration and ends when that does; the general flying rule
        // is the creature's own condition and ends when it stands up.
        // Nothing is rendered for a creature that simply has no wings,
        // which is almost everything almost always.
        let reason = if curr_actor.has_condition(crate::conditions::Condition::Earthbound) {
            "earthbound"
        } else {
            "cannot fly while downed"
        };
        stats_lines.push(Line::from(Span::styled(
            format!(
                "Grounded: fly {:.0} ft suppressed ({})",
                curr_actor.base_fly_speed(),
                reason
            ),
            Style::default().fg(Color::LightBlue),
        )));
    }
    // 5e Suffocation — the breath clock, rendered only when it is
    // running. Every creature spends nearly every round of nearly every
    // fight pinned at full breath, and a line saying so on every panel
    // would be noise of exactly the kind the Airborne row above avoids.
    //
    // Two shapes, because the two states need different numbers. While
    // the creature still has breath, what matters is how many rounds of
    // it are left — that is the window to get out of the water, and it
    // is the only place the number is visible at all. Once it is out,
    // the countdown is over and the number that matters is the ladder:
    // one rung per round, six is death, and `Exhaustion` is already on
    // the conditions row below with its tier. So the second line says
    // what is happening rather than restating that.
    //
    // Read `can_breathe` rather than a bare immersion check so the
    // panel and the round-end tick can never disagree about whether the
    // clock is running — it is the same predicate, asked once here.
    if !encounter.can_breathe(curr_actor_id) {
        let held = curr_actor.breath_rounds();
        stats_lines.push(Line::from(Span::styled(
            if held > 0 {
                format!(
                    "Breath: {} round{} held",
                    held,
                    if held == 1 { "" } else { "s" }
                )
            } else {
                "Breath: none \u{2014} 1 exhaustion per round".to_string()
            },
            Style::default().fg(Color::LightRed),
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
                // carried by its timer — it has six levels and a
                // permanent timer on every one of them, so a bare
                // "exhausted" would read the same at level 1, which
                // costs two points and five feet, as at level 5, which
                // costs ten and twenty-five. Show the level.
                if matches!(c, crate::conditions::Condition::Exhausted) {
                    return (
                        format!("{} {}", c.name(), curr_actor.exhaustion_level()),
                        c.name(),
                    );
                }
                // The wards whose whole content is *which* damage type
                // they were chosen against — Resistance, Protection
                // from Energy, Absorb Elements. "braced(9)" says
                // nothing a player can act on; "braced vs Fire(9)" is
                // the entire spell. Anything carrying no choice answers
                // `None` and reads exactly as it did. Folded in before
                // the timer suffix rather than after, so the duration
                // stays where the eye expects it.
                let stem = match curr_actor.damage_type_of(*c) {
                    Some(dt) => format!("{} vs {:?}", c.name(), dt),
                    None => c.name().to_string(),
                };
                let label = match timer {
                    crate::conditions::ConditionTimer::Permanent => stem,
                    crate::conditions::ConditionTimer::Rounds(n) => format!("{}({})", stem, n),
                    crate::conditions::ConditionTimer::UntilStartOfNextTurn => {
                        format!("{}*", stem)
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
    // The pane's interior, in rows. Two of the pane's rows are its own
    // border, and what is left is all the list gets.
    let action_rows = area_split[2].height.saturating_sub(2) as usize;
    let window = action_window(n_actions, highlight_idx, action_rows);
    let mut action_lines: Vec<Line<'static>> = actions
        .iter()
        .enumerate()
        .skip(window.start)
        .take(window.end.saturating_sub(window.start))
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
            // 5e weapon mastery: name the property beside the weapon
            // that carries it, but only for a wielder who is trained to
            // use it. An untrained hand gets nothing from the tag, and
            // printing it anyway would advertise a clause that will not
            // fire — which is worse than saying nothing, because the
            // player would plan around it.
            let mastery = crate::engine::mastery::effective_mastery(
                encounter,
                curr_actor_id,
                action.weapon_mastery(),
            );
            let mut spans = vec![
                Span::raw(prefix),
                Span::styled(tag, base_style),
                Span::raw(" "),
                Span::styled(action.name().to_string(), base_style),
            ];
            if let Some(m) = mastery {
                spans.push(Span::styled(
                    format!(" \u{00b7}{}", m.name()),
                    if is_selected {
                        base_style
                    } else {
                        Style::default().fg(Color::Cyan)
                    },
                ));
            }
            Line::from(spans)
        })
        .collect();
    // The two ends of the list that did not fit, named rather than
    // silently cut off. A player who cannot see that there is more below
    // has no reason to press the key that would show it.
    if window.more_above {
        action_lines.insert(
            0,
            Line::from(Span::styled(
                format!("  \u{2191} {} more", window.start),
                Style::default().fg(Color::DarkGray),
            )),
        );
    }
    if window.more_below {
        action_lines.push(Line::from(Span::styled(
            format!("  \u{2193} {} more", n_actions - window.end),
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(action_lines)
            .block(Block::default().borders(Borders::ALL).title("Actions")),
        area_split[2],
    );
}

/// What the action pane draws: a half-open slice of the action list, and
/// whether either end of the list continues past it.
///
/// The two flags are computed here rather than derived from the slice at
/// the draw site, because "there are more entries above" and "there is a
/// row to say so in" are different questions, and only this function
/// knows the answer to the second.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ActionWindow {
    start: usize,
    end: usize,
    more_above: bool,
    more_below: bool,
}

/// The half-open slice of the action list that fits in `rows` interior
/// rows with the selected entry inside it.
///
/// The action pane used to render every action a creature had and let
/// the terminal clip whatever ran past the border. That was survivable
/// when a goblin had six; it is not now. A Battle Master carries every
/// default action plus a weapon plus fourteen maneuvers, a wizard
/// carries sixty spells, and the pane is a third of the sidebar — so on
/// a 24-row terminal the list is six lines long and everything else
/// simply was not on the screen. The selection wrapped past the fold and
/// the player was choosing blind: the highlight bar was somewhere in the
/// clipped remainder, and the panel looked identical whichever of the
/// invisible fifty entries was live.
///
/// Windowing rather than paging, because the selection moves one step at
/// a time: a page that flipped every `rows` entries would put the
/// highlight at the top or the bottom edge on every flip, where a window
/// that follows the cursor keeps its neighbours in view. Centred where
/// there is room, flush at either end where there is not, which is what
/// makes the top and bottom of a list feel like the top and bottom of a
/// list rather than like the middle of one.
///
/// A marker row is reserved for each end that has entries beyond it, and
/// only for those ends — a window flush against the top of the list
/// spends the row it saves on one more action instead. That is the whole
/// reason this returns a slice rather than a scroll offset: how many
/// rows the content gets depends on which markers are showing, which
/// depends on where the window is.
fn action_window(total: usize, highlight: usize, rows: usize) -> ActionWindow {
    if rows == 0 || total == 0 {
        return ActionWindow::default();
    }
    if total <= rows {
        return ActionWindow {
            start: 0,
            end: total,
            more_above: false,
            more_below: false,
        };
    }
    // More entries than rows, so at least one marker wants a row. Start
    // from a content height that assumes both, then grow it into
    // whatever the markers turn out not to need — a window flush against
    // an end pays for one marker, not two, and a pane too short to
    // afford a marker at all spends every row on the list.
    let mut content = rows.saturating_sub(2).max(1);
    loop {
        let start = window_start(total, highlight, content);
        let end = start + content;
        let used = content + usize::from(start > 0) + usize::from(end < total);
        if used >= rows || content >= rows {
            // The selection is what the pane is for, so it keeps its row
            // and a marker that cannot be afforded is simply not drawn.
            let spare = rows.saturating_sub(content);
            let more_above = start > 0;
            let more_below = end < total;
            return ActionWindow {
                start,
                end,
                more_above: more_above && spare > usize::from(more_below),
                more_below: more_below && spare >= 1,
            };
        }
        content += 1;
    }
}

/// Where a window of `content` entries sits when it is centred on
/// `highlight` and clamped to the list. Split out because
/// `action_window` walks it once per candidate height.
fn window_start(total: usize, highlight: usize, content: usize) -> usize {
    let start = highlight.saturating_sub(content / 2);
    if start + content > total {
        total.saturating_sub(content)
    } else {
        start
    }
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
        rendered_panel_tall(encounter, 24)
    }

    /// `rendered_panel` with the terminal height as a parameter, for the
    /// tests that read the *action* list. The panel splits its height
    /// three ways, so at the default 24 rows the action pane is six
    /// lines and a martial's weapon sits well below the fold — a test
    /// that asserted on it there would be asserting about the fold.
    fn rendered_panel_tall(encounter: &EncounterInstance, height: u16) -> String {
        rendered_panel_at(encounter, height, 0)
    }

    /// `rendered_panel_tall` with the selected action index as a
    /// parameter, for the tests that read where the action pane's window
    /// has scrolled to.
    fn rendered_panel_at(
        encounter: &EncounterInstance,
        height: u16,
        selected_action_idx: usize,
    ) -> String {
        let mut terminal = Terminal::new(TestBackend::new(60, height)).expect("test backend");
        terminal
            .draw(|f| {
                let area = f.area();
                render_sideinfo(encounter, f, area, selected_action_idx);
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

    /// The map, flattened to one string. Sibling of `rendered_panel`;
    /// the map is the other half of what a player reads off the screen
    /// and nothing here was testing it.
    fn rendered_map(encounter: &EncounterInstance) -> String {
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).expect("test backend");
        terminal
            .draw(|f| {
                let area = f.area();
                render_map(encounter, f, area, None, &std::collections::HashSet::new());
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

    /// Which *board* tiles the map drew with the aiming shade on them.
    ///
    /// Returned in board coordinates rather than screen ones, so a test
    /// can compare them against what `AreaShape::tiles` says without
    /// re-deriving the renderer's y-flip and one-cell border offset.
    fn shaded_tiles(
        encounter: &EncounterInstance,
        preview: &std::collections::HashSet<Coordinate>,
    ) -> std::collections::HashSet<Coordinate> {
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).expect("test backend");
        terminal
            .draw(|f| {
                let area = f.area();
                render_map(encounter, f, area, None, preview);
            })
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let mut out = std::collections::HashSet::new();
        for row in 0..buffer.area.height {
            for col in 0..buffer.area.width {
                if buffer[(col, row)].style().bg != Some(AREA_PREVIEW_BG) {
                    continue;
                }
                // The map draws inside a one-cell border, top row first
                // and highest y first — undo both to get back to the
                // board's own coordinates.
                let x = col as isize - 1;
                let y = encounter.height as isize - (row as isize - 1) - 1;
                out.insert(Coordinate::new(x, y));
            }
        }
        out
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

    /// The two halves of a mounted pair both reach the screen: the
    /// rider's glyph on the map, and the pairing on the panel.
    ///
    /// This is the one place a rider can go missing. `engine::mounts`
    /// takes the rider off the occupancy grid so the pair can share a
    /// space, and the map is drawn straight off that grid — so a knight
    /// on a horse renders as a horse, and the player loses track of
    /// their own character. The panel matters for the same reason from
    /// the other side: a ridden mount's initiative slot passes straight
    /// through, and a warhorse that is never reached needs to say why.
    #[test]
    fn a_mounted_pair_is_visible_on_the_map_and_named_on_the_panel() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::warhorses::WARHORSE_TEMPLATE;

        let mut e = encounter_with(&[(&FIGHTER_TEMPLATE, 0), (&WARHORSE_TEMPLATE, 0)]);
        let rider = *e
            .actors
            .iter()
            .find(|(_, a)| a.name().starts_with("Fighter"))
            .map(|(id, _)| id)
            .expect("a fighter");
        let mount = *e
            .actors
            .iter()
            .find(|(_, a)| a.name().starts_with("Warhorse"))
            .map(|(id, _)| id)
            .expect("a warhorse");
        // The generator scatters them, so walk the horse over rather
        // than depending on where either landed.
        let beside = e.actors[&rider].location() + Coordinate::new(2, 0);
        e.place_actor_at(mount, beside).expect("room beside the rider");
        assert!(e.mount(rider, mount).is_ok());

        // Distinct glyphs, or "both are drawn" is not a question the
        // map can answer.
        assert_ne!(e.actors[&rider].glyph(), e.actors[&mount].glyph());
        let map = rendered_map(&e);
        let (rider_glyph, mount_glyph) = (
            e.actors[&rider].glyph(),
            e.actors[&mount].glyph(),
        );
        assert!(
            map.contains(rider_glyph),
            "the rider is still on the board and must still be drawn:\n{}",
            map
        );
        assert!(
            map.contains(mount_glyph),
            "and so is the horse under them:\n{}",
            map
        );

        let panel = rendered_panel(&e);
        assert!(
            panel.contains("riding") && panel.contains("ridden by"),
            "the panel names the pairing from both ends:\n{}",
            panel
        );
    }

    /// An unlit tile is drawn as nothing at all, and the goblin
    /// standing on it disappears with it.
    ///
    /// Blanking rather than dimming is the decision worth pinning. A
    /// dimmed monster glyph is still a monster the player has been
    /// told about, and the entire tactical content of fighting in the
    /// dark is not knowing where anything is — so a map that greys the
    /// creature out has given the game away while looking like it
    /// hasn't.
    #[test]
    fn a_creature_standing_in_the_dark_is_not_drawn_on_the_map() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;

        let mut e = encounter_with(&[(&GOBLIN_TEMPLATE, 0)]);
        let goblin_glyph = GOBLIN_TEMPLATE.glyph;
        assert!(
            rendered_map(&e).contains(goblin_glyph),
            "the goblin is on a lit board to start with"
        );
        e.set_ambient_light(crate::engine::lighting::AmbientLight::Darkness);
        assert!(
            !rendered_map(&e).contains(goblin_glyph),
            "and vanishes when the lights go out"
        );
    }

    /// A light source carves a visible island out of the dark — the
    /// other half of the test above, and the one that says the map is
    /// reading the light layer rather than simply blanking on a dark
    /// ambient.
    #[test]
    fn a_torch_carves_a_visible_island_out_of_the_dark() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::engine::lighting::{AmbientLight, LightAnchor, LightSource};

        let mut e = encounter_with(&[(&GOBLIN_TEMPLATE, 0)]);
        let goblin = *e.actors.keys().next().expect("a goblin");
        let at = e.actors[&goblin].location();
        e.set_ambient_light(AmbientLight::Darkness);
        e.add_light_source(LightSource {
            id: 0,
            name: "torch",
            anchor: LightAnchor::Fixed(at),
            bright_tiles: 3,
            dim_tiles: 0,
            rounds_remaining: None,
            spell_level: 0,
            innate: false,
        });
        assert!(
            rendered_map(&e).contains(GOBLIN_TEMPLATE.glyph),
            "the torch is standing on the goblin"
        );
    }

    /// The lighting shows up in the panel title when it is worth
    /// mentioning, and stays out of it when it is not. A player
    /// fighting blind needs to be told why; a player on an ordinary
    /// board does not need a line saying the lights are on.
    #[test]
    fn the_panel_names_the_lighting_only_when_it_is_not_the_default() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::engine::lighting::AmbientLight;

        let mut e = encounter_with(&[(&GOBLIN_TEMPLATE, 0)]);
        assert!(
            !rendered_panel(&e).contains("darkness"),
            "a lit board says nothing about lighting"
        );
        e.set_ambient_light(AmbientLight::Darkness);
        let panel = rendered_panel(&e);
        assert!(
            panel.contains("darkness"),
            "an unlit board says so on the panel:\n{}",
            panel
        );
    }

    /// The panel says how high the current actor is and what the
    /// landing will cost, and says nothing at all for everybody standing
    /// on the floor.
    ///
    /// The die count is the half worth pinning. "Airborne: 30 ft" is
    /// trivia; the number of d6 waiting at the bottom is the reason a
    /// player would think twice about letting the concentration go, and
    /// it is read straight off the same `fall_damage_dice` the fall
    /// itself rolls so the panel can never promise a softer landing than
    /// the engine delivers.
    /// A ward against one damage type says which one.
    ///
    /// The three conditions that carry a chosen element — Resistance's
    /// `Braced`, Protection from Energy's `EnergyWarded`, Absorb
    /// Elements' — are the only conditions in the game whose entire
    /// content is a word that is not their name. "braced(10)" tells a
    /// player nothing they can act on; "braced vs Fire(10)" is the
    /// spell. Everything else on the line reads exactly as it did.
    #[test]
    fn a_ward_against_one_element_names_it_on_the_panel() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::types::DamageType;

        let mut e = encounter_with(&[(&GOBLIN_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        e.process_stack();
        let id = e.current_turn_actor_id().expect("somebody is up");
        for eff in crate::engine::side_effects::install_condition_with_damage_type(
            Condition::Braced,
            id,
            DamageType::Fire,
            ConditionTimer::Rounds(10),
        ) {
            eff.apply(&mut e);
        }
        // A second condition with no choice, to pin that the ordinary
        // shape is untouched.
        e.actors
            .get_mut(&id)
            .unwrap()
            .add_condition(Condition::Blessed, ConditionTimer::Rounds(10));

        let panel = rendered_panel_tall(&e, 90);
        assert!(
            panel.contains("braced vs Fire(10)"),
            "the ward should name the element it is against:\n{}",
            panel
        );
        assert!(
            panel.contains("blessed(10)"),
            "a condition carrying no choice reads exactly as before:\n{}",
            panel
        );
    }

    #[test]
    fn the_panel_counts_the_dice_waiting_under_a_flier() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};

        // The stats pane only renders while somebody is being prompted,
        // so the fixture needs two teams and a `process_stack` to open
        // the first slot.
        let mut e = encounter_with(&[(&GOBLIN_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        e.process_stack();
        assert!(
            !rendered_panel_tall(&e, 90).contains("Airborne"),
            "a creature on the floor gets no altitude line"
        );
        let id = e.current_turn_actor_id().expect("somebody is up");
        e.actors
            .get_mut(&id)
            .unwrap()
            .add_condition(Condition::Flying, ConditionTimer::Rounds(10));
        e.reconcile_altitudes();
        let panel = rendered_panel_tall(&e, 90);
        assert!(
            panel.contains("Airborne: 30 ft") && panel.contains("3d6"),
            "an airborne creature should be told what it owes:\n{}",
            panel
        );

        // Caught: the line stops quoting dice and names the spell that
        // means they will not be rolled.
        e.actors
            .get_mut(&id)
            .unwrap()
            .add_condition(Condition::Feathered, ConditionTimer::Rounds(10));
        let panel = rendered_panel_tall(&e, 90);
        assert!(
            panel.contains("feather fall") && !panel.contains("3d6"),
            "a feathered flier owes nothing:\n{}",
            panel
        );
    }

    /// The panel counts down the breath of a creature that has none
    /// coming, and says nothing at all for everybody standing in air.
    ///
    /// The countdown is the half worth pinning. A player whose fighter
    /// has waded into a pool has one decision to make and one number to
    /// make it on — how many rounds are left before the exhaustion
    /// starts — and that number lives nowhere else: the conditions row
    /// below cannot show it, because until the clock runs out there is
    /// no condition to show.
    #[test]
    fn the_panel_counts_down_the_breath_of_a_creature_under_water() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::engine::terrain::TerrainType;

        let mut e = encounter_with(&[(&GOBLIN_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        e.process_stack();
        assert!(
            !rendered_panel_tall(&e, 90).contains("Breath"),
            "a creature with air around it gets no breath line"
        );

        // Flood the whole of the current actor's footprint — RAW's
        // "fully immersed" is all of it or none of it.
        let id = e.current_turn_actor_id().expect("somebody is up");
        let anchor = e.actors[&id].location();
        for dx in -1..=2isize {
            for dy in -1..=2isize {
                e.set_terrain_at(
                    Coordinate::new(anchor.x + dx, anchor.y + dy),
                    TerrainType::Water,
                );
            }
        }
        let panel = rendered_panel_tall(&e, 90);
        assert!(
            panel.contains("Breath:") && panel.contains("held"),
            "a submerged creature should be told how long it has:\n{}",
            panel
        );

        // Once the clock is spent the countdown is over, and the line
        // stops quoting a number that would only ever read zero.
        while e.actors[&id].breath_rounds() > 0 {
            e.actors.get_mut(&id).unwrap().spend_breath(false);
        }
        let panel = rendered_panel_tall(&e, 90);
        assert!(
            panel.contains("1 exhaustion per round") && !panel.contains("held"),
            "an out-of-breath creature should be told what it is paying:\n{}",
            panel
        );

        // And a creature that can breathe the water it is standing in
        // has no clock at all.
        e.actors
            .get_mut(&id)
            .unwrap()
            .grant_feature_for_test(crate::actions::class_features::UNDERWATER_BREATHING_TAG);
        assert!(
            !rendered_panel_tall(&e, 90).contains("Breath"),
            "gills are not a status effect"
        );
    }

    /// A flier that has been put on the floor says so, and says which of
    /// the two rules did it.
    ///
    /// The line exists because the Movement row above it changes meaning
    /// without changing shape: a wyvern in the air moves 80 and the same
    /// wyvern on the floor moves 20, and a panel that showed only the
    /// smaller number would leave the player with no account of where
    /// the other sixty feet went.
    ///
    /// Naming the cause is the second half. The two groundings end
    /// differently — Earthbind is somebody's concentration and the
    /// general flying rule is the creature's own condition — so the
    /// counterplay differs and the panel should not make the reader
    /// guess which one they are looking at.
    #[test]
    fn a_grounded_flier_is_told_what_it_is_missing() {
        use crate::actors::creatures::wyverns::WYVERN_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};

        let mut e = encounter_with(&[(&WYVERN_TEMPLATE, 0), (&WYVERN_TEMPLATE, 1)]);
        e.process_stack();
        e.reconcile_altitudes();
        let panel = rendered_panel_tall(&e, 90);
        assert!(
            panel.contains("Airborne") && !panel.contains("Grounded"),
            "a wyvern starts in the air:\n{}",
            panel
        );

        // The general flying rule: knocked out of the sky by a condition
        // it holds itself.
        let id = e.current_turn_actor_id().expect("somebody is up");
        e.actors
            .get_mut(&id)
            .unwrap()
            .add_condition(Condition::Incapacitated, ConditionTimer::Rounds(10));
        e.reconcile_altitudes();
        let panel = rendered_panel_tall(&e, 90);
        assert!(
            panel.contains("Grounded: fly 80 ft suppressed (cannot fly while downed)"),
            "a downed wyvern should be told which sixty feet it lost:\n{}",
            panel
        );

        // Earthbind: the same suppression, a different owner, a
        // different line.
        e.actors
            .get_mut(&id)
            .unwrap()
            .remove_condition(Condition::Incapacitated);
        e.actors
            .get_mut(&id)
            .unwrap()
            .add_condition(Condition::Earthbound, ConditionTimer::Rounds(10));
        e.reconcile_altitudes();
        let panel = rendered_panel_tall(&e, 90);
        assert!(
            panel.contains("Grounded: fly 80 ft suppressed (earthbound)"),
            "and a spell-grounded one should name the spell:\n{}",
            panel
        );

        // Nothing at all for a creature that never had wings — the line
        // is not a permanent fixture of the panel.
        let spawn = e
            .get_random_spawn(crate::engine::types::Size::Large)
            .expect("a floor tile");
        let walker = e
            .instantiate_creature(
                &crate::actors::creatures::ogres::OGRE_TEMPLATE,
                spawn,
                1,
                9,
            )
            .unwrap();
        assert_eq!(
            e.actors[&walker].base_fly_speed(),
            0.0,
            "an ogre is the control: no wings, and so no line to print"
        );
    }

    /// A creature whose only damage modifier is source-qualified still
    /// gets a line on the panel, and the line names the qualifier.
    ///
    /// The panel reads `damage_modifier`, which by construction answers
    /// `None` for every qualified row — so the forty stat blocks that
    /// carry "resistance to bludgeoning, piercing, and slashing damage
    /// from nonmagical attacks" and nothing else would display as
    /// having no damage modifiers whatsoever. That is the exact failure
    /// this panel's docstring says it exists to prevent, arriving from
    /// the one direction a "did somebody add a fourteenth damage type"
    /// sweep could not see.
    #[test]
    fn a_creature_that_only_resists_mundane_steel_still_says_so_on_the_panel() {
        use crate::actors::actor_template::ActorInstance;
        use crate::actors::creatures::wraiths::WRAITH_TEMPLATE;
        use crate::engine::dice::FastRandRoller;

        let wraith = ActorInstance::from_creature_template(
            &WRAITH_TEMPLATE,
            Coordinate::new(0, 0),
            0,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        let mut lines: Vec<Line<'static>> = Vec::new();
        push_damage_modifier_line(&mut lines, &wraith);
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.to_string())
                    .collect::<String>()
            })
            .collect();
        assert!(
            rendered
                .iter()
                .any(|l| l.starts_with("Resistant vs mundane:")
                    && l.contains("Bludgeoning")
                    && l.contains("Piercing")
                    && l.contains("Slashing")),
            "the wraith's whole physical defence is missing from the panel: {:?}",
            rendered
        );
        // And the unqualified rows keep their own lines rather than
        // being folded in with it.
        assert!(
            rendered.iter().any(|l| l.starts_with("Immune:")),
            "the wraith's necrotic and poison immunity should still be there: {:?}",
            rendered
        );
        assert!(
            rendered
                .iter()
                .any(|l| l.starts_with("Resistant:") && !l.contains("Bludgeoning")),
            "the unqualified line must not claim the qualified types: {:?}",
            rendered
        );
    }

    /// 5e weapon mastery is annotated on the action list, and only for
    /// the wielder who can use it.
    ///
    /// The gate is the point of the test. The tag lives on shared weapon
    /// statics — the goblin's scimitar *is* the fighter's — so a panel
    /// that printed it off the weapon alone would advertise
    /// "scimitar ·nick" to every goblin on the roster: a clause the
    /// The action window keeps the selection on screen, never overruns
    /// the pane, and always reserves a row for each marker it is about
    /// to draw.
    ///
    /// Swept exhaustively over every (list length, selection, pane
    /// height) the pane can plausibly be handed, because the arithmetic
    /// has four branches and the interesting ones are the boundaries —
    /// a window flush against either end, a pane one row tall, a list
    /// exactly as long as the pane.
    #[test]
    fn the_action_window_always_holds_the_selection_and_fits_the_pane() {
        for total in 0..40usize {
            for rows in 0..16usize {
                for highlight in 0..total.max(1) {
                    let w = super::action_window(total, highlight, rows);
                    let (start, end) = (w.start, w.end);
                    assert!(start <= end, "{total}/{highlight}/{rows}: inverted window");
                    assert!(end <= total, "{total}/{highlight}/{rows}: window past the end");
                    if rows == 0 || total == 0 {
                        assert_eq!((start, end), (0, 0));
                        continue;
                    }
                    assert!(
                        start <= highlight && highlight < end,
                        "{total}/{highlight}/{rows}: the selection is off screen \
                         ({start}..{end})"
                    );
                    // A marker is only ever claimed when there is more
                    // list on that side to point at.
                    assert!(!w.more_above || start > 0, "{total}/{highlight}/{rows}");
                    assert!(!w.more_below || end < total, "{total}/{highlight}/{rows}");
                    let markers = usize::from(w.more_above) + usize::from(w.more_below);
                    assert!(
                        (end - start) + markers <= rows,
                        "{total}/{highlight}/{rows}: {} rows of content plus {markers} \
                         markers overruns a {rows}-row pane",
                        end - start
                    );
                    // Every row the pane has is used unless the whole
                    // list fits — a window that left a row blank while
                    // hiding an entry would be wasting the screen.
                    if total > rows {
                        assert_eq!(
                            (end - start) + markers,
                            rows,
                            "{total}/{highlight}/{rows}: a row was left empty with \
                             entries still hidden"
                        );
                    }
                }
            }
        }
    }

    /// The pane follows the selection rather than clipping it. A fighter
    /// carries far more actions than a short terminal can show, and the
    /// entry the highlight is on has to be one of the ones drawn.
    ///
    /// Read off the rendered panel rather than off `action_window`,
    /// because what is under test is the wiring: the window is computed
    /// from the pane's own height, and the pane's height is a third of
    /// whatever the sidebar was given.
    #[test]
    fn a_long_action_list_scrolls_to_the_selection_rather_than_clipping_it() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;

        let mut e = encounter_with(&[(&FIGHTER_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        e.process_stack();
        let n = e
            .peek_prompt()
            .map(|p| p.actions().len())
            .expect("the fighter should be holding the prompt");
        assert!(
            n > 20,
            "this test needs a chassis whose list overruns a short pane (saw {n})"
        );

        // The first entry is on screen at selection 0, and the last
        // entry is on screen when the selection is on it — which is the
        // whole of what windowing buys and exactly what clipping lost.
        let first = e.peek_prompt().unwrap().actions()[0].name().to_string();
        let last = e.peek_prompt().unwrap().actions()[n - 1].name().to_string();
        let top = rendered_panel_at(&e, 24, 0);
        assert!(
            top.contains(&first),
            "the first action should be visible at selection 0:\n{top}"
        );
        let bottom = rendered_panel_at(&e, 24, n - 1);
        assert!(
            bottom.contains(&last),
            "the last action should be visible when it is selected:\n{bottom}"
        );
        // …and the reader is told which way the rest of the list went.
        assert!(
            top.contains('\u{2193}'),
            "a clipped tail should be announced:\n{top}"
        );
        assert!(
            bottom.contains('\u{2191}'),
            "a clipped head should be announced:\n{bottom}"
        );
    }

    /// player would plan around and that would never fire.
    #[test]
    fn the_action_list_names_a_mastery_property_only_for_a_trained_wielder() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;

        // The action list only renders while somebody is being prompted,
        // so each fixture needs a live turn — hence the second actor and
        // the `process_stack` that opens the first slot.
        let mut trained = encounter_with(&[(&FIGHTER_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        trained.process_stack();
        let panel = rendered_panel_tall(&trained, 90);
        assert!(
            panel.contains("\u{00b7}nick"),
            "a fighter's scimitar should name what it does:\n{panel}"
        );

        let mut untrained = encounter_with(&[(&GOBLIN_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        untrained.process_stack();
        let panel = rendered_panel_tall(&untrained, 90);
        assert!(
            panel.contains("scimitar"),
            "the goblin is holding the same weapon:\n{panel}"
        );
        assert!(
            !panel.contains("\u{00b7}nick"),
            "an untrained wielder should be promised nothing:\n{panel}"
        );
    }

    /// Water is drawn, and drawn as something other than the shade ramp
    /// the other five terrain types share.
    ///
    /// The glyph matters more here than it does for rubble. Every other
    /// tile on the map costs a creature movement and nothing else, so a
    /// player who misreads one loses a step; a pool switches off a
    /// build's ranged offense entirely, and a player who cannot see
    /// where it is has no way to make the decision the tile exists to
    /// pose. The '≈' is deliberately not on the '░▒▓█' ladder — a tile
    /// that reads as "slower floor" would be the wrong lesson.
    #[test]
    fn water_is_drawn_as_water_and_not_as_another_rung_of_the_shade_ramp() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::engine::terrain::TerrainType;

        let mut e = encounter_with(&[(&FIGHTER_TEMPLATE, 0)]);
        // Counted rather than merely present: the generator lays pools
        // of its own, so a bare `contains` would pass without the tiles
        // this test flooded ever being drawn.
        let before = rendered_map(&e).matches('≈').count();

        let mut flooded = 0;
        for x in 3..=6isize {
            for y in 3..=6isize {
                if e.terrain_at(Coordinate::new(x, y)).map(|t| t.terrain_type)
                    == Some(TerrainType::Floor)
                {
                    assert!(e.set_terrain_at(Coordinate::new(x, y), TerrainType::Water));
                    flooded += 1;
                }
            }
        }
        assert!(flooded > 0, "the fixture map has open floor to flood");
        let map = rendered_map(&e);
        assert_eq!(
            map.matches('≈').count(),
            before + flooded,
            "every flooded tile is drawn as water:\n{}",
            map
        );
        for ramp in ['░', '▒', '▓', '█'] {
            assert_ne!(
                '≈', ramp,
                "water must not borrow a glyph from the movement-cost ramp"
            );
        }
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

    /// A set ward is drawn with its own glyph, not the hazard skull.
    ///
    /// The map is the only record of where a ward is: `ZoneEffect::ward`
    /// makes it invisible to the pathfinder, so nothing else on screen
    /// can tell a player which tile they armed three rounds ago. Its own
    /// glyph rather than the skull, because "armed and waiting" and
    /// "burning right now" are the two facts a player standing beside
    /// one has to tell apart — and the ward outranks the skull so a
    /// glyph laid under a cloud is still findable.
    #[test]
    fn a_set_ward_is_drawn_as_a_ward_and_not_as_a_live_hazard() {
        use crate::engine::dice::Dice;
        use crate::engine::types::{AbilityScoreType, DamageType};
        use crate::engine::zones::{Zone, ZoneContact, ZoneEffect, ZoneMotion};

        let mut e = encounter_with(&[(&GOBLIN_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        let bite = || {
            ZoneContact::save_for_half(
                AbilityScoreType::Dexterity,
                15,
                Dice::new(5, 8),
                DamageType::Fire,
            )
        };
        let lay = |e: &mut EncounterInstance, origin: Coordinate, effect: ZoneEffect| {
            e.install_zone(Zone {
                id: 0,
                name: "test area",
                owner_id: 0,
                origin,
                radius: 1,
                effect,
                rounds_remaining: 10,
                concentration: false,
                motion: ZoneMotion::Fixed,
            });
        };

        // A live damaging hazard is a skull; the same clause set as a
        // ward is not.
        lay(&mut e, Coordinate::new(4, 4), ZoneEffect::hazard(bite()));
        let map = rendered_map(&e);
        assert!(map.contains('☠'), "a live hazard is a skull:\n{}", map);
        assert!(!map.contains('◈'), "and is not a ward:\n{}", map);

        // Laid over the top of it, the ward wins the tile.
        lay(&mut e, Coordinate::new(4, 4), ZoneEffect::ward(bite(), 0));
        let map = rendered_map(&e);
        assert!(
            map.contains('◈'),
            "a ward is drawn, and outranks the skull under it:\n{}",
            map
        );
    }

    /// A boss's remaining legendary actions are on the panel, and they
    /// leave it once they are spent.
    ///
    /// The one number in a boss fight the player has to plan around and
    /// cannot read anywhere else: a legendary action never appears on
    /// an action list, because it is spent between turns rather than on
    /// one. Both halves are pinned — the row appears while there is
    /// something left to spend, and stops once there is not, because a
    /// permanent "Lg 0/3" would be noise for the second half of every
    /// round.
    #[test]
    fn the_initiative_panel_counts_a_bosss_remaining_legendary_actions() {
        use crate::actors::creatures::dragons::ADULT_RED_DRAGON_TEMPLATE;
        use crate::engine::side_effects::Resource;

        let mut e = encounter_with(&[(&ADULT_RED_DRAGON_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        let dragon = *e
            .actors
            .iter()
            .find(|(_, a)| a.legendary_actions_per_round() > 0)
            .map(|(id, _)| id)
            .expect("the fixture has a legendary creature");
        assert!(
            rendered_panel(&e).contains("Lg 3/3"),
            "a fresh boss shows a full pool:\n{}",
            rendered_panel(&e)
        );

        for _ in 0..2 {
            e.actors
                .get_mut(&dragon)
                .unwrap()
                .consume_resource(Resource::LegendaryAction);
        }
        assert!(
            rendered_panel(&e).contains("Lg 1/3"),
            "a partly spent pool shows what is left:\n{}",
            rendered_panel(&e)
        );

        e.actors
            .get_mut(&dragon)
            .unwrap()
            .consume_resource(Resource::LegendaryAction);
        assert!(
            !rendered_panel(&e).contains("Lg "),
            "a spent boss is an ordinary row again:\n{}",
            rendered_panel(&e)
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

    /// Conjured terrain is listed too, and the panel is the only thing
    /// that can list it: a wall of stone draws as a wall, which is the
    /// point, so nothing on the map says the corridor reopens in four
    /// rounds.
    #[test]
    fn the_initiative_panel_counts_down_a_conjured_wall() {
        use crate::engine::conjured_terrain::ConjuredTerrain;
        use crate::engine::terrain::TerrainType;

        let mut e = encounter_with(&[(&GOBLIN_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        assert!(!rendered_panel(&e).contains("wall of stone"));
        e.conjure_terrain(ConjuredTerrain::new(
            "wall of stone",
            0,
            TerrainType::Wall,
            vec![Coordinate::new(4, 4), Coordinate::new(4, 5)],
            4,
            true,
        ));
        let panel = rendered_panel(&e);
        assert!(
            panel.contains("wall of stone") && panel.contains("4r"),
            "the panel should name the wall and its remaining rounds:\n{}",
            panel
        );
    }

    /// A banished creature is off the map and still in the queue, and
    /// the panel is the only thing that can say so.
    ///
    /// The map draws off the actor grid, so a body on a demiplane
    /// genuinely is not on it — which is the feature working, and also a
    /// creature that has apparently ceased to exist while its initiative
    /// slot keeps coming round and doing nothing. The same problem the
    /// ridden-mount label below solves, and the countdown is the part a
    /// player actually needs: how many rounds until the ogre is back.
    #[test]
    fn the_initiative_panel_counts_down_a_banished_creature() {
        use crate::conditions::{Condition, ConditionTimer};

        let mut e = encounter_with(&[(&GOBLIN_TEMPLATE, 0), (&GOBLIN_TEMPLATE, 1)]);
        let victim = *e.actors.keys().max().expect("two goblins");
        let tile = e.actors[&victim].location();
        assert!(!rendered_panel(&e).contains("banished"));
        assert!(rendered_map(&e).contains(GOBLIN_TEMPLATE.glyph));

        e.actors
            .get_mut(&victim)
            .unwrap()
            .add_condition(Condition::Banished, ConditionTimer::Rounds(6));
        e.reconcile_board_presence();

        let panel = rendered_panel(&e);
        assert!(
            panel.contains("banished") && panel.contains("6r"),
            "the panel should name the state and the rounds left:\n{}",
            panel
        );
        assert_eq!(
            e.actor_id_at(tile),
            None,
            "…precisely because there is nothing on the map to look at"
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

    /// The map shades the tiles an area would cover, which is the only
    /// way a cone is aimable at all: it is typed as an `X,Y` pair, it
    /// depends on where the caster is standing as much as on the tile
    /// named, and at sixty feet its far end is twenty-five tiles wide.
    /// Nobody counts that in their head.
    #[test]
    fn the_map_shades_the_area_the_player_is_aiming() {
        use crate::engine::areas::AreaShape;

        let mut e = encounter_with(&[]);
        let caster = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(4, 4), 0, 0)
            .expect("instantiate");
        let cone = AreaShape::Cone { length: 6 };
        let aim = Coordinate::new(12, 5);
        let loc = e.actors[&caster].location();
        let size = e.actors[&caster].size();
        let want: std::collections::HashSet<Coordinate> =
            cone.tiles(loc, size, aim).into_iter().collect();
        assert!(!want.is_empty(), "the fixture aims a real cone");

        let shaded = shaded_tiles(&e, &want);
        // Everything the shape covers *and fits on the drawn map* is
        // shaded; the map is 20×20 and the terminal is wider, so the
        // whole cone is on screen here.
        assert_eq!(shaded, want);
    }

    /// And it shades nothing when there is nothing being aimed, which is
    /// every frame of an ordinary turn.
    #[test]
    fn the_map_shades_nothing_when_no_area_is_being_aimed() {
        let mut e = encounter_with(&[]);
        e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(4, 4), 0, 0)
            .expect("instantiate");
        assert!(shaded_tiles(&e, &std::collections::HashSet::new()).is_empty());
    }

    /// A cone drawn through the dark stays dark. The preview is applied
    /// before the lighting pass for exactly this reason: shading a tile
    /// the viewer cannot see would tell the player where the walls are
    /// by drawing the shape that stops at them.
    #[test]
    fn the_aiming_shade_does_not_light_up_tiles_the_viewer_cannot_see() {
        use crate::engine::areas::AreaShape;
        use crate::engine::lighting::AmbientLight;

        let mut e = encounter_with(&[]);
        e.set_ambient_light(AmbientLight::Darkness);
        let caster = e
            .instantiate_creature(&crate::actors::creatures::commoners::COMMONER_TEMPLATE,
                Coordinate::new(4, 4), 0, 0)
            .expect("instantiate");
        let cone = AreaShape::Cone { length: 6 };
        let loc = e.actors[&caster].location();
        let size = e.actors[&caster].size();
        let want: std::collections::HashSet<Coordinate> = cone
            .tiles(loc, size, Coordinate::new(12, 5))
            .into_iter()
            .collect();
        // A commoner has no darkvision, so on an unlit board every tile
        // of the cone is blanked and none of them carries the shade.
        assert!(
            shaded_tiles(&e, &want).is_empty(),
            "an unlit board should not be mapped by aiming at it"
        );
    }

}
