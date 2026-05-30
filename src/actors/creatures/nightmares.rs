use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{Multiattack, SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static NIGHTMARE_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "flaming hooves",
    sub_attack: &SLAM,
    count: 2,
});

/// Nightmare — fiendish steed wreathed in flame (CR 3, MM p.235). A
/// Large fiend with fire immunity and cold resistance. Two-hoof
/// multiattack. The hooves deal fire damage RAW but we reuse SLAM
/// (bludgeoning) for simplicity; the fire immunity covers the thematic
/// element.
pub static NIGHTMARE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*NIGHTMARE_MULTI);
    CreatureTemplate {
        name: "Nightmare",
        glyph: 'N',
        ac: 13,
        hitpoints: "8d10+16".parse().unwrap(),
        speed: 60.,
        strength: 18,
        intelligence: 10,
        dexterity: 15,
        wisdom: 12,
        constitution: 16,
        charisma: 15,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Abyssal, Language::Infernal]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
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
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
    }
});
