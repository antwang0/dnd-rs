use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DAGGER, SHORTBOW, THROWN_DAGGER};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Kobold — small, fragile DEX-based skirmisher. Dagger melee + shortbow
/// bonus action lets them hit-and-run. Pack tactics (RAW: advantage when
/// an ally is near) is approximated through the existing Help action,
/// which kobolds have access to via DEFAULT_ACTIONS.
pub static KOBOLD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DAGGER);
    // RAW's kobold stat block reads "Dagger. Melee or Ranged Weapon
    // Attack: +4 to hit, reach 5 ft. or range 20/60 ft." — one line
    // that is two attacks, and the second half is the one a kobold
    // actually wants. The shortbow beside it is a bonus action, so a
    // kobold that opens by throwing its dagger and then looses an
    // arrow spends its whole turn without ever coming into reach,
    // which is the creature playing to its 5 average hit points.
    actions.push(&THROWN_DAGGER);
    actions.push(&SHORTBOW);
    CreatureTemplate {
        name: "Kobold",
        glyph: 'K',
        ac: 12,
        hitpoints: "2d6".parse().unwrap(),
        strength: 7,
        dexterity: 15,
        constitution: 9,
        intelligence: 8,
        wisdom: 9,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 0.125,
        size: Size::Small,
        creature_type: CreatureType::Humanoid,
        actions,
        has_pack_tactics: true,
        ..CreatureTemplate::defaults()
    }
});
