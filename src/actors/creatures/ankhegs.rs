use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ANKHEG_ACID_SPRAY, ANKHEG_BITE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Ankheg — CR 2 monstrosity. Giant burrowing insect that erupts from
/// the ground. Bite deals 2d6+3 slashing + 1d6 acid. Also has an acid
/// spray (3d6 acid, DEX save DC 13, 30ft line). AC 14 (natural armor, 11
/// when prone/burrowed), ~45 HP (6d10+12). Tremorsense 60ft, darkvision
/// 60ft.
///
/// Speed 30, **burrow 10** — and the gap between those two numbers is
/// the creature. An ankheg that has dug in cannot be reached, cannot
/// be caught in an area, and crosses the room at four tiles a turn
/// with the party's archers watching a patch of churned earth. See
/// `crate::engine::burrowing`; the tremorsense is what it hunts with
/// while it is down there.
pub static ANKHEG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ANKHEG_BITE);
    actions.push(&*ANKHEG_ACID_SPRAY);
    CreatureTemplate {
        name: "Ankheg",
        glyph: 'å',
        ac: 14,
        hitpoints: "6d10+12".parse().unwrap(),
        speed: 30.,
        burrow_speed: 10.,
        strength: 17,
        intelligence: 1,
        dexterity: 11,
        wisdom: 13,
        constitution: 14,
        charisma: 6,
        senses: HashSet::from([
            SpecialSense::Darkvision(60),
            SpecialSense::Tremorsense(60),
        ]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
