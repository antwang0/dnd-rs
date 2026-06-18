use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DISPLACER_BEAST_MULTI, TENTACLE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Displacer Beast — CR 3 monstrosity. Six-legged panther with two
/// barbed tentacles sprouting from its shoulders. Attacks with a
/// multiattack of two tentacle strikes at 10ft reach. Its signature
/// displacement trait gives disadvantage on attacks against it; the
/// displacement flickers off when the beast takes damage and restores
/// at the start of its next turn.
pub static DISPLACER_BEAST_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&TENTACLE);
    actions.push(&*DISPLACER_BEAST_MULTI);
    CreatureTemplate {
        name: "Displacer Beast",
        glyph: 'D',
        ac: 13,
        hitpoints: "10d10+30".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 6,
        dexterity: 15,
        wisdom: 12,
        constitution: 16,
        charisma: 8,
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        has_displacement: true,
        ..CreatureTemplate::defaults()
    }
});
