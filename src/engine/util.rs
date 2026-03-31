use ratatui::style::Color;

use crate::engine::types::Size;

pub fn get_tiles_from_size(size: Size) -> usize {
    match size {
        Size::Tiny => 1,
        Size::Small => 2,
        Size::Medium => 2,
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
