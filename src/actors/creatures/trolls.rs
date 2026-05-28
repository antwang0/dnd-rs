use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BITE, SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Troll — large regenerating brute (CR 5). High HP and two attacks
/// (claw + bite) per turn via the multiattack wrapper. Mechanically the
/// troll exercises the Large footprint and reach-2 adjacency the same
/// way the Ogre does, but with more staying power.
///
/// Regeneration: 3 HP at end-of-round while combat-active, suppressed
/// for one round whenever the troll takes acid or fire damage. The
/// engine reads `regen_per_round` / `regen_suppressors` from the
/// template, and `DealDamage` flips `regen_suppressed` whenever a
/// suppressor type lands.
pub static TROLL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    // Claws (slam) + Bite — two attacks per action, matching 5e multiattack.
    actions.push(&SLAM);
    actions.push(&*BITE);
    CreatureTemplate {
        name: "Troll",
        glyph: 'T',
        ac: 15,
        hitpoints: "8d10+24".parse().unwrap(),
        speed: 30.,
        strength: 18,
        intelligence: 7,
        dexterity: 13,
        wisdom: 9,
        constitution: 20,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Giant]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Giant,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 3,
        regen_suppressors: HashSet::from([DamageType::Acid, DamageType::Fire]),
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
