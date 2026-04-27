use ratatui::style::Color;
use regex::Regex;
use std::sync::LazyLock;

use crate::engine::types::{Coordinate, Size};

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
    let bg = if team_u8 < 1 {
        Color::LightCyan
    } else {
        Color::Black
    };
    (glyph.to_string(), color, bg)
}

pub fn modifier_from_score(score: u32) -> i32 {
    (score as i32 / 2) - 5
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
static RE_REL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(r|l)(\d+),?(u|d)(\d+)$").unwrap());

pub fn parse_coord(input: &str, base_coord: Coordinate) -> Option<Coordinate> {
    if let Some(caps) = RE_ABS.captures(input) {
        let x = caps[1].parse::<isize>().ok()?;
        let y = caps[2].parse::<isize>().ok()?;
        return Some(Coordinate::new(x, y));
    }

    if let Some(caps) = RE_REL.captures(input) {
        let pos_x: bool = &caps[1] == "r";
        let pos_y: bool = &caps[3] == "u";
        let x_off = caps[2].parse::<isize>().ok()? * if pos_x { 1 } else { -1 };
        let y_off = caps[4].parse::<isize>().ok()? * if pos_y { 1 } else { -1 };
        return Some(base_coord + Coordinate::new(x_off, y_off));
    }

    None
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
    fn parse_coord_invalid() {
        assert_eq!(parse_coord("garbage", Coordinate::new(0, 0)), None);
        assert_eq!(parse_coord("1,", Coordinate::new(0, 0)), None);
    }

    #[test]
    fn modifier_from_score_examples() {
        assert_eq!(modifier_from_score(10), 0);
        assert_eq!(modifier_from_score(8), -1);
        assert_eq!(modifier_from_score(20), 5);
    }
}
