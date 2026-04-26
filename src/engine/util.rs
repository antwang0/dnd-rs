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

const ACTOR_CHARS: [char; 28] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', '*', '!',
];

pub fn get_colored_span(n: usize, team: usize) -> (String, Color, Color) {
    let s = ACTOR_CHARS[n % ACTOR_CHARS.len()].to_string();

    let team_u8: u8 = (team % 16) as u8;
    let color = Color::Indexed(team_u8);
    let bg = if team_u8 < 1 {
        Color::LightCyan
    } else {
        Color::Black
    };

    (s, color, bg)
}

pub fn modifier_from_score(score: u32) -> i32 {
    (score as i32 / 2) - 5
}

pub fn tile_center_dist(c1: Coordinate, c2: Coordinate) -> f32 {
    let diff = c1 - c2;
    2.5 * ((diff.x.pow(2) + diff.y.pow(2)) as f32).sqrt()
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
