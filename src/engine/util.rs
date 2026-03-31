use ratatui::{
    style::{Color, Stylize},
    text::Span,
};

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
    // pick from the Basic Multilingual Plane (BMP) for simplicity
    // let code = (n % 0xD7FF) as u32; // avoid surrogate range
    // let s = std::char::from_u32(code).unwrap_or('?').to_string();
    let s = ACTOR_CHARS[n % ACTOR_CHARS.len()].to_string();

    let team_u8: u8 = (team % 16) as u8;
    let color = Color::Indexed(team_u8);
    let bg = if team_u8 < 1 {
        Color::DarkGray
    } else {
        Color::Black
    };

    (s, color, bg)
}
