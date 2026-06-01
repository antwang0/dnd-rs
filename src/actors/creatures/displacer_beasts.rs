use crate::engine::types::CreatureType;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DISPLACER_BEAST_MULTI, TENTACLE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::Size;
use std::collections::{HashMap, HashSet};
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
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::new(),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
        has_displacement: true,
    }
});
