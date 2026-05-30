use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DAGGER;
use crate::actions::spells::{
    BESTOW_CURSE, BLESS, COMMAND, HEX, HOLD_PERSON, INFLICT_WOUNDS, SACRED_FLAME, SHIELD_OF_FAITH,
    SPIRITUAL_WEAPON,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Cult Fanatic — CR 2 humanoid spellcaster. WIS-primary like the
/// cleric, but darker spell list: Inflict Wounds (necrotic touch),
/// Hold Person, and Command for control. Carries a dagger for melee
/// fallback. Slots are tuned for a single fight: 4×L1 / 3×L2. The
/// fanatic is a mid-tier caster threat (more dangerous than the cleric
/// thanks to Hold Person being on-tier with the encounter difficulty).
pub static CULT_FANATIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DAGGER);
    actions.push(&*SACRED_FLAME);
    actions.push(&*INFLICT_WOUNDS);
    actions.push(&*COMMAND);
    actions.push(&*BLESS);
    actions.push(&*SHIELD_OF_FAITH);
    actions.push(&*HOLD_PERSON);
    actions.push(&*SPIRITUAL_WEAPON);
    actions.push(&*HEX);
    actions.push(&*BESTOW_CURSE);
    CreatureTemplate {
        name: "Cult Fanatic",
        // 'V' for villain — distinct from 'C' (Cleric).
        glyph: 'V',
        ac: 13,
        hitpoints: "6d8+6".parse().unwrap(),
        speed: 30.,
        strength: 11,
        intelligence: 10,
        dexterity: 14,
        wisdom: 13,
        constitution: 12,
        charisma: 14,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // 4 level-1 + 3 level-2 slots — typical level-4 spellcaster.
        spell_slots_by_level: vec![4, 3],
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        // No save proficiencies in the MM stat block (NPC, not a class
        // proper). Leave empty so this matches RAW.
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
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});

