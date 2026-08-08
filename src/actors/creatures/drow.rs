use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DROW_POISONED_CROSSBOW, SCIMITAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::lighting::SunlightFrailty;
use crate::engine::types::{CreatureType, Language, Size, Skill, SpecialSense};
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
    actions.push(&DROW_POISONED_CROSSBOW);
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
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        // 5e Drow **Sunlight Sensitivity**: disadvantage on attack
        // rolls in sunlight (the Perception half has no surface here).
        // The drow's answer to it is on their own spell list in RAW —
        // and the engine now has the spell that provides it, since a
        // Darkness sphere drives its tiles to `LightLevel::Dark` and
        // `is_sunlit` reports false inside one.
        sunlight_frailty: Some(SunlightFrailty::Sensitivity),
        ..CreatureTemplate::defaults()
    }
});
