use crate::actions::class_features::{HEALING_HANDS, HEALING_HANDS_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::LONGSWORD;
use crate::actions::spells::{
    BLESS, CURE_WOUNDS, GUIDING_BOLT, HEALING_WORD, LESSER_RESTORATION, SACRED_FLAME,
    SHIELD_OF_FAITH, SPARE_THE_DYING,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Aasimar — celestial-touched humanoid, built on a Cleric-lite chassis.
/// The headline racial trait is **Healing Hands**: a once-per-rest touch
/// heal for `level` HP. RAW also gives them necrotic / radiant
/// resistance from their celestial ancestry — both modeled here.
///
/// Stat shape targets a level-3 build: AC 16 (chain shirt + DEX), 22 HP
/// (3d8+6), CHA 16 (the celestial spellcasting ability). The loadout
/// stays light on slot use: a few healing spells (Cure Wounds, Healing
/// Word, Spare the Dying) plus the racial Healing Hands so the aasimar
/// has a fallback when slots run dry; a couple of low-level offensive
/// picks (Sacred Flame, Guiding Bolt) for damage; Bless / Shield of
/// Faith for the support lane.
pub static AASIMAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LONGSWORD);
    // Cantrips (at-will).
    actions.push(&*SACRED_FLAME);
    actions.push(&*SPARE_THE_DYING);
    // Level 1 — Cure Wounds / Healing Word / Bless / Shield of Faith /
    // Guiding Bolt. Standard cleric chassis with both healing and a
    // ranged single-target attack.
    actions.push(&*CURE_WOUNDS);
    actions.push(&HEALING_WORD);
    actions.push(&*BLESS);
    actions.push(&*SHIELD_OF_FAITH);
    actions.push(&*GUIDING_BOLT);
    // Level 2 — Lesser Restoration (cleanse Poisoned / Paralyzed).
    actions.push(&*LESSER_RESTORATION);
    // Racial: Healing Hands — once-per-rest touch heal for `level` HP.
    actions.push(&*HEALING_HANDS);
    CreatureTemplate {
        name: "Aasimar",
        // 'A' — distinct from 'a' (already used by ankheg), reads as a
        // robed CHA-caster with celestial flair.
        glyph: 'A',
        ac: 16,
        hitpoints: "3d8+6".parse().unwrap(),
        speed: 30.,
        strength: 12,
        intelligence: 10,
        dexterity: 12,
        wisdom: 14,
        constitution: 14,
        charisma: 16, // celestial spellcasting ability
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Celestial]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Cleric-lite slot table — three lv1 slots plus two lv2 slots
        // for Lesser Restoration. Enough to keep the support lane
        // active through a medium-length encounter without dilution.
        spell_slots_by_level: vec![3, 2],
        rolls_death_saves: true,
        // 5e Aasimar Celestial Resistance: resistance to necrotic and
        // radiant damage from the celestial heritage.
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Radiant, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::from([AbilityScoreType::Wisdom, AbilityScoreType::Charisma]),
        condition_immunities: HashSet::new(),
        // Racial Healing Hands flag — once-per-rest gated.
        features: HashSet::from([HEALING_HANDS_TAG]),
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
