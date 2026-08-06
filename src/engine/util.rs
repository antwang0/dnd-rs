use ratatui::style::Color;
use regex::Regex;
use std::sync::LazyLock;

use crate::engine::types::{Coordinate, Size};

/// Feet of board covered by one grid tile.
///
/// 5e's grid is 5 ft to the square and this engine's is 2.5, because a
/// Medium creature occupies a 2×2 block rather than a single tile — see
/// `get_tiles_from_size`. Every conversion between a rule written in feet
/// ("half your speed", "within 10 feet") and a count of tiles goes
/// through this number, and it was a bare `2.5` at each of those sites.
pub const TILE_FEET: f32 = 2.5;

pub fn get_tiles_from_size(size: Size) -> usize {
    match size {
        Size::Tiny => 1,
        Size::Small | Size::Medium => 2,
        Size::Large => 4,
        Size::Huge => 6,
        Size::Gargantuan => 8,
    }
}

/// Map glyph + team-keyed (fg, bg) for an actor. The glyph comes from the
/// creature template (capital letter per species); the team color uses the
/// 16-color ANSI palette. Team 0 gets a lighter bg so it stands out as
/// "the player's side" by convention.
pub fn get_colored_span(glyph: char, team: usize) -> (String, Color, Color) {
    let team_u8: u8 = (team % 16) as u8;
    let color = Color::Indexed(team_u8);
    // Team 0 (the player's side by convention) gets a contrasting bg so
    // the PC silhouette stands out from the rest of the board. Every
    // other team falls through to the default Black bg.
    let bg = if team_u8 == 0 {
        Color::LightCyan
    } else {
        Color::Black
    };
    (glyph.to_string(), color, bg)
}

pub fn modifier_from_score(score: u32) -> i32 {
    (score as i32 / 2) - 5
}

/// 5e proficiency bonus by character level. PHB Table:
/// levels 1-4 → +2, 5-8 → +3, 9-12 → +4, 13-16 → +5, 17-20 → +6.
/// Monsters use their CR-derived bonus in the books, but for our
/// engine all non-PCs stay at level 1 so they default to +2.
pub fn proficiency_bonus_for_level(level: u32) -> i32 {
    match level {
        0..=4 => 2,
        5..=8 => 3,
        9..=12 => 4,
        13..=16 => 5,
        _ => 6,
    }
}

/// 5e cantrip damage scaling: cantrips gain extra damage dice at caster
/// levels 5, 11, and 17. Returns the number of base damage dice the
/// cantrip should roll at a given caster level.
pub fn cantrip_dice_count(caster_level: u32) -> u32 {
    match caster_level {
        0..=4 => 1,
        5..=10 => 2,
        11..=16 => 3,
        _ => 4,
    }
}

/// Min Chebyshev gap (in tiles) between two square footprints. 0 means
/// touching/overlapping; 1 means one tile of clear space between them, etc.
/// Used everywhere "is X next to Y" matters — origin-to-origin distance gives
/// the wrong answer for non-Tiny creatures since Medium occupies a 2×2 block
/// in this 2.5ft-tile grid.
pub fn footprint_chebyshev(
    a_loc: Coordinate,
    a_size: usize,
    b_loc: Coordinate,
    b_size: usize,
) -> isize {
    let a_size = a_size as isize;
    let b_size = b_size as isize;
    let a_right = a_loc.x + a_size - 1;
    let a_top = a_loc.y + a_size - 1;
    let b_right = b_loc.x + b_size - 1;
    let b_top = b_loc.y + b_size - 1;
    let gap_x = (a_loc.x.max(b_loc.x) - a_right.min(b_right) - 1).max(0);
    let gap_y = (a_loc.y.max(b_loc.y) - a_top.min(b_top) - 1).max(0);
    gap_x.max(gap_y)
}

static RE_ABS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+),(\d+)$").unwrap());

/// Parse a coordinate from `input`, interpreted relative to `base_coord`
/// when the form is relative. Supported syntaxes:
///
/// - **Absolute** — `x,y` (e.g. `3,5`) maps to `Coordinate { x, y }`.
/// - **Relative single-axis** — `r3`, `l4`, `u2`, `d1` shift only one
///   axis from `base_coord` (the other stays put). Lets the user type
///   `r3` to nudge their picker one tile east of the previous pick
///   without having to specify the vertical at 0.
/// - **Relative both-axis** — `r3u2`, `l4d1`, `u2r3`, `d1l4` shift both
///   axes from `base_coord`. The horizontal-first and vertical-first
///   orderings are both accepted, and an optional comma may separate
///   the two segments (`r3,u2` reads the same as `r3u2`).
///
/// Case-insensitive: `R3U2` matches as `r3u2`. Repeating the same axis
/// (e.g. `r3r2`) is rejected — the user almost certainly meant `r5`.
pub fn parse_coord(input: &str, base_coord: Coordinate) -> Option<Coordinate> {
    if let Some(caps) = RE_ABS.captures(input) {
        let x = caps[1].parse::<isize>().ok()?;
        let y = caps[2].parse::<isize>().ok()?;
        return Some(Coordinate::new(x, y));
    }

    // Lowercase once so `R3U2` and `r3u2` parse identically; the
    // segment regex is lowercase-only to keep its branch count small.
    let lowered = input.to_ascii_lowercase();
    if lowered.is_empty() {
        return None;
    }
    // Walk the input looking for `<dir><N>[<dir><N>]` with an optional
    // comma between segments. We require exactly one or two segments,
    // each on a distinct axis (one horizontal + one vertical); repeats
    // like `r3l2` are rejected as ambiguous.
    let mut bytes = lowered.as_bytes();
    let (mut dx, mut dy) = (0isize, 0isize);
    let (mut saw_x, mut saw_y) = (false, false);
    let mut segments = 0;
    while !bytes.is_empty() {
        let dir = bytes[0];
        if !matches!(dir, b'r' | b'l' | b'u' | b'd') {
            return None;
        }
        // Consume the magnitude digits.
        let mag_start = 1;
        let mut mag_end = mag_start;
        while mag_end < bytes.len() && bytes[mag_end].is_ascii_digit() {
            mag_end += 1;
        }
        if mag_end == mag_start {
            return None;
        }
        let mag: isize = std::str::from_utf8(&bytes[mag_start..mag_end])
            .ok()?
            .parse()
            .ok()?;
        match dir {
            b'r' if !saw_x => {
                dx = mag;
                saw_x = true;
            }
            b'l' if !saw_x => {
                dx = -mag;
                saw_x = true;
            }
            b'u' if !saw_y => {
                dy = mag;
                saw_y = true;
            }
            b'd' if !saw_y => {
                dy = -mag;
                saw_y = true;
            }
            // Repeat axis — `r3r2` / `u1u2` etc. are ambiguous; bail.
            _ => return None,
        }
        segments += 1;
        if segments > 2 {
            return None;
        }
        bytes = &bytes[mag_end..];
        // Optional comma between segments. Trailing comma (or two
        // commas) is a syntax error — bail back to the outer loop
        // which will reject on empty / non-direction bytes.
        if bytes.first() == Some(&b',') {
            bytes = &bytes[1..];
            if bytes.is_empty() {
                return None;
            }
        }
    }
    // Guard against `0`-magnitude no-op forms slipping through as a
    // valid relative coordinate when they didn't actually parse — at
    // least one segment must have been consumed.
    if !saw_x && !saw_y {
        return None;
    }
    Some(base_coord + Coordinate::new(dx, dy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_coord_absolute() {
        let base = Coordinate::new(0, 0);
        assert_eq!(parse_coord("3,5", base), Some(Coordinate::new(3, 5)));
    }

    #[test]
    fn parse_coord_relative() {
        let base = Coordinate::new(10, 10);
        assert_eq!(parse_coord("r2u3", base), Some(Coordinate::new(12, 13)));
        assert_eq!(parse_coord("l4d1", base), Some(Coordinate::new(6, 9)));
    }

    #[test]
    fn parse_coord_relative_single_axis() {
        // Pin the load-bearing single-axis lane: a user can type just
        // `r3` to nudge the picker east without spelling out a vertical
        // offset of 0. Previously the parser required both axes and
        // `r3` returned None.
        let base = Coordinate::new(5, 5);
        assert_eq!(parse_coord("r3", base), Some(Coordinate::new(8, 5)));
        assert_eq!(parse_coord("l2", base), Some(Coordinate::new(3, 5)));
        assert_eq!(parse_coord("u4", base), Some(Coordinate::new(5, 9)));
        assert_eq!(parse_coord("d1", base), Some(Coordinate::new(5, 4)));
    }

    #[test]
    fn parse_coord_relative_vertical_first_order() {
        // Either ordering (horizontal-first or vertical-first) parses.
        let base = Coordinate::new(10, 10);
        assert_eq!(parse_coord("u3r2", base), Some(Coordinate::new(12, 13)));
        assert_eq!(parse_coord("d1l4", base), Some(Coordinate::new(6, 9)));
    }

    #[test]
    fn parse_coord_relative_case_insensitive() {
        let base = Coordinate::new(0, 0);
        assert_eq!(parse_coord("R3U2", base), Some(Coordinate::new(3, 2)));
        assert_eq!(parse_coord("r3U2", base), Some(Coordinate::new(3, 2)));
    }

    #[test]
    fn parse_coord_relative_comma_separator() {
        let base = Coordinate::new(0, 0);
        assert_eq!(parse_coord("r3,u2", base), Some(Coordinate::new(3, 2)));
        assert_eq!(parse_coord("u2,l4", base), Some(Coordinate::new(-4, 2)));
    }

    #[test]
    fn parse_coord_invalid() {
        assert_eq!(parse_coord("garbage", Coordinate::new(0, 0)), None);
        assert_eq!(parse_coord("1,", Coordinate::new(0, 0)), None);
    }

    #[test]
    fn parse_coord_rejects_repeated_axis() {
        // Repeated axis (`r3r2`, `u1u2`) is ambiguous — the user
        // almost certainly meant `r5` / `u3`. Pin the rejection so a
        // typo doesn't silently parse as just the *second* segment
        // (which a naive walker would let through).
        let base = Coordinate::new(0, 0);
        assert_eq!(parse_coord("r3r2", base), None);
        assert_eq!(parse_coord("u1u2", base), None);
        assert_eq!(parse_coord("l1r1", base), None);
    }

    #[test]
    fn parse_coord_rejects_trailing_comma() {
        let base = Coordinate::new(0, 0);
        assert_eq!(parse_coord("r3,", base), None);
        assert_eq!(parse_coord(",r3", base), None);
    }

    #[test]
    fn parse_coord_rejects_missing_magnitude() {
        let base = Coordinate::new(0, 0);
        assert_eq!(parse_coord("r", base), None);
        assert_eq!(parse_coord("ru3", base), None);
    }

    #[test]
    fn modifier_from_score_examples() {
        assert_eq!(modifier_from_score(10), 0);
        assert_eq!(modifier_from_score(8), -1);
        assert_eq!(modifier_from_score(20), 5);
    }

    #[test]
    fn proficiency_bonus_steps() {
        assert_eq!(proficiency_bonus_for_level(1), 2);
        assert_eq!(proficiency_bonus_for_level(4), 2);
        assert_eq!(proficiency_bonus_for_level(5), 3);
        assert_eq!(proficiency_bonus_for_level(9), 4);
        assert_eq!(proficiency_bonus_for_level(17), 6);
        assert_eq!(proficiency_bonus_for_level(50), 6);
    }
}
