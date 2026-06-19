use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DROW_POISONED_CROSSBOW, SCIMITAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Drow — Underdark elf raider. Trained scimitarist + ranged poisoned-bolt
/// pressure via a hand crossbow. The poison rider is the headline (CON
/// save DC 13, fail = +2d4 poison and Poisoned for 2 rounds). Stat shape
/// follows MM Drow at CR 1/4: 14 AC (chain shirt), 13 HP, +4 to hit.
/// Senses: Darkvision (120 ft, doubling a goblin's 60 ft).
pub static DROW_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&*DROW_POISONED_CROSSBOW);
    CreatureTemplate {
        name: "Drow",
        glyph: 'D',
        ac: 15,
        hitpoints: "3d8".parse().unwrap(),
        strength: 10,
        dexterity: 14,
        constitution: 10,
        intelligence: 11,
        wisdom: 11,
        charisma: 12,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Elvish, Language::Undercommon]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        has_magic_resistance: true,
        // 5e Drow Fey Ancestry: advantage on saves vs Charmed; magic
        // can't put a drow to sleep. Approximated as Charmed + Asleep
        // install immunity via `dynamic_immunity_to`. Stacks with the
        // drow's existing Magic Resistance (drow are notoriously
        // anti-enchantment in the SRD).
        has_fey_ancestry: true,
        ..CreatureTemplate::defaults()
    }
});
