use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::MIMIC_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Mimic — CR 2 shape-shifting monstrosity. Disguises itself as inert
/// scenery, then opens up with an adhesive bite that sticks the victim
/// in place. We don't model the disguise (no perception system yet),
/// but the Adhesive bite rider applies the `Adhered` condition for two
/// rounds — long enough to make the encounter feel like a trap, short
/// enough that a single victim won't be perma-locked. Immune to
/// acid (its own goo doesn't hurt it) and to prone (already amorphous).
pub static MIMIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MIMIC_BITE);
    CreatureTemplate {
        name: "Mimic",
        // 'm' (lowercase) — distinct from 'M' (Mage / Wizard).
        glyph: 'm',
        ac: 12,
        hitpoints: "9d8+9".parse().unwrap(),
        speed: 15.,
        strength: 17,
        intelligence: 5,
        dexterity: 12,
        wisdom: 13,
        constitution: 12,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([(DamageType::Acid, DamageModifier::Immunity)]),
        proficient_saves: HashSet::new(),
        // Amorphous: can't be knocked prone.
        condition_immunities: HashSet::from([Condition::Prone]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
    }
});
