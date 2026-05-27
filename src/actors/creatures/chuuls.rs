use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CHUUL_PINCER, CHUUL_TENTACLES};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Chuul — CR 4 aberration. Lobster-like predator with paralyzing
/// tentacles. Two actions: pincer (2d6+4 bludgeoning + grapple on hit)
/// and tentacles (1d6+4 poison + CON save DC 13 or Paralyzed 1 round,
/// only on grappled targets). AC 16, ~93 HP (11d10+33). Immune to
/// poison damage and the Poisoned condition. Amphibious with darkvision
/// 60ft and tremorsense 60ft.
pub static CHUUL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*CHUUL_PINCER);
    actions.push(&*CHUUL_TENTACLES);
    CreatureTemplate {
        name: "Chuul",
        glyph: 'ç',
        ac: 16,
        hitpoints: "11d10+33".parse().unwrap(),
        speed: 30.,
        strength: 19,
        intelligence: 5,
        dexterity: 10,
        wisdom: 11,
        constitution: 16,
        charisma: 5,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Darkvision(60),
            SpecialSense::Tremorsense(60),
        ]),
        languages: HashSet::new(),
        cr: 4.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::from([Condition::Poisoned]),
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
    }
});
