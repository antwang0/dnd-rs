use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{
    AID, BANE, BEACON_OF_HOPE, BESTOW_CURSE, BLESS, CALM_EMOTIONS, COMMAND, CURE_WOUNDS,
    DEATH_WARD, DISPEL_MAGIC, DIVINE_FAVOR, FAERIE_FIRE, GUIDING_BOLT, HASTE, HEAL_SPELL_HIGH,
    HEALING_WORD, HEROISM, HOLD_PERSON, INFLICT_WOUNDS, LESSER_RESTORATION, MASS_CURE_WOUNDS,
    MASS_HEAL, MASS_HEALING_WORD, POWER_WORD_HEAL, PRAYER_OF_HEALING,
    PROTECTION_FROM_EVIL_AND_GOOD, RESURRECTION, REVIVIFY, SACRED_BURST,
    SACRED_FLAME, SHIELD_OF_FAITH, SPARE_THE_DYING, SPIRIT_GUARDIANS, SPIRITUAL_WEAPON,
    STONESKIN, SUGGESTION, SUNBEAM, SUNBURST, THORN_WHIP, TOLL_THE_DEAD, WORD_OF_RADIANCE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Acolyte-style spellcaster. WIS-primary; Sacred Flame as the staple
/// damage option, Healing Word and Cure Wounds for support, Bless for
/// pre-buff, Hold Person for lockdown. Modeled to be roughly equivalent
/// to MM Acolyte (CR 1/4) — light HP, medium AC, no melee.
pub static CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SACRED_FLAME);
    actions.push(&*SACRED_BURST);
    actions.push(&HEALING_WORD);
    actions.push(&*CURE_WOUNDS);
    actions.push(&*HOLD_PERSON);
    actions.push(&*SHIELD_OF_FAITH);
    actions.push(&*BLESS);
    actions.push(&*GUIDING_BOLT);
    actions.push(&*FAERIE_FIRE);
    actions.push(&*BANE);
    actions.push(&*SPIRITUAL_WEAPON);
    actions.push(&*AID);
    actions.push(&*INFLICT_WOUNDS);
    actions.push(&*LESSER_RESTORATION);
    actions.push(&*THORN_WHIP);
    actions.push(&*SPARE_THE_DYING);
    actions.push(&*TOLL_THE_DEAD);
    actions.push(&*HEROISM);
    actions.push(&*MASS_HEALING_WORD);
    actions.push(&*PROTECTION_FROM_EVIL_AND_GOOD);
    actions.push(&*COMMAND);
    actions.push(&*DIVINE_FAVOR);
    actions.push(&*SPIRIT_GUARDIANS);
    actions.push(&*BESTOW_CURSE);
    actions.push(&*MASS_CURE_WOUNDS);
    actions.push(&*HASTE);
    actions.push(&*DISPEL_MAGIC);
    actions.push(&*DEATH_WARD);
    actions.push(&*REVIVIFY);
    actions.push(&*BEACON_OF_HOPE);
    actions.push(&*STONESKIN);
    actions.push(&*HEAL_SPELL_HIGH);
    actions.push(&*WORD_OF_RADIANCE);
    actions.push(&*CALM_EMOTIONS);
    actions.push(&*SUGGESTION);
    actions.push(&*SUNBURST);
    actions.push(&*MASS_HEAL);
    actions.push(&*PRAYER_OF_HEALING);
    actions.push(&*SUNBEAM);
    actions.push(&*RESURRECTION);
    actions.push(&*POWER_WORD_HEAL);
    CreatureTemplate {
        name: "Cleric",
        glyph: 'C',
        ac: 13,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 10,
        dexterity: 10,
        wisdom: 14, // primary spellcasting ability
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        actions,
        // 4/3/3/2/2/1/1/1/2 — cleric loadout extended to support the
        // full SRD spell list now in their kit. Level-3 slot covers
        // Mass Healing Word / Spirit Guardians / Beacon of Hope /
        // Haste; level-4 slot covers Stoneskin / Death Ward; level-5
        // covers Mass Cure Wounds; level-6 fuels one Heal or one
        // Sunbeam (concentration — only one at a time anyway); the
        // new level-7 slot powers exactly one Resurrection; the
        // level-8 slot powers a single Sunburst; and the level-9 row
        // jumps to 2 so Mass Heal and Power Word Heal can each fire
        // once per long rest.
        spell_slots_by_level: vec![4, 3, 3, 2, 2, 1, 1, 1, 2],
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        // Clerics are proficient in WIS and CHA saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Wisdom, AbilityScoreType::Charisma]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
    }
});
