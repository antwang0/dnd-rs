use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCIMITAR;
use crate::actions::spells::{
    CALL_LIGHTNING, CONFUSION, CURE_WOUNDS, DAYLIGHT, DISPEL_MAGIC, FAERIE_FIRE, FLY, GOODBERRY,
    HEALING_WORD, HEAT_METAL, HEROES_FEAST, ICE_STORM, LESSER_RESTORATION, LEVITATE, MAGIC_STONE,
    MASS_CURE_WOUNDS, MOONBEAM, PLANT_GROWTH, POISON_SPRAY, POLYMORPH, REVERSE_GRAVITY,
    SLEET_STORM, SPIKE_GROWTH, SPIKE_STONES, STORM_OF_VENGEANCE, THORN_WHIP, THUNDERWAVE,
    WALL_OF_FIRE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Druid PC template. WIS-primary full-caster with a nature-flavored
/// spell list. Plays as a midline support / control caster — strong
/// AoE (Moonbeam, Call Lightning, Sleet Storm), solid heals (Cure
/// Wounds, Healing Word, Goodberry, Mass Cure Wounds), terrain control
/// (Spike Growth, Wall of Fire), and the level-7 nuke Reverse Gravity
/// for boss fights.
///
/// Loadout: Poison Spray + Thorn Whip cantrips for at-will damage,
/// Scimitar as the weapon fallback. Healing line spans 1→5
/// (Goodberry / Healing Word / Cure Wounds / Lesser Restoration /
/// Mass Cure Wounds). Control line spans 1→9 (Faerie Fire / Spike
/// Growth / Sleet Storm / Polymorph / Reverse Gravity).
///
/// Stats target a level-9 druid: 58 HP (9d8+18), AC 14 (leather + DEX),
/// WIS 18 (spell save DC 8+4+4 = 16). Full-caster slot table mirrors
/// the wizard / cleric loadout in this engine: 4/3/3/2/2/1/1/1/1.
/// PC flag flips on so the druid rolls death saves at 0 HP.
pub static DRUID_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    // Cantrips
    actions.push(&*POISON_SPRAY);
    actions.push(&*THORN_WHIP);
    actions.push(&*MAGIC_STONE);
    // Level 1
    actions.push(&*GOODBERRY);
    actions.push(&HEALING_WORD);
    actions.push(&*CURE_WOUNDS);
    actions.push(&*FAERIE_FIRE);
    actions.push(&*THUNDERWAVE);
    // Level 2
    actions.push(&*MOONBEAM);
    actions.push(&*SPIKE_GROWTH);
    actions.push(&*HEAT_METAL);
    actions.push(&*LESSER_RESTORATION);
    actions.push(&*LEVITATE);
    // Level 3
    actions.push(&*CALL_LIGHTNING);
    actions.push(&*SLEET_STORM);
    actions.push(&*DAYLIGHT);
    actions.push(&*DISPEL_MAGIC);
    actions.push(&*PLANT_GROWTH);
    actions.push(&*FLY);
    // Level 4
    actions.push(&*ICE_STORM);
    actions.push(&*POLYMORPH);
    actions.push(&*WALL_OF_FIRE);
    actions.push(&*CONFUSION);
    actions.push(&*SPIKE_STONES);
    // Level 5
    actions.push(&*MASS_CURE_WOUNDS);
    // Level 6 — apex pre-fight buff: ally temp-HP + heal + Heroic.
    actions.push(&*HEROES_FEAST);
    // Level 7
    actions.push(&*REVERSE_GRAVITY);
    // Level 9 — the apex druid spell.
    actions.push(&*STORM_OF_VENGEANCE);
    CreatureTemplate {
        name: "Druid",
        glyph: 'D',
        ac: 14, // leather armor (11) + DEX(+2) + Wis-adjacent shield use; rounded.
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 12,
        dexterity: 14,
        wisdom: 18,     // primary spellcasting ability
        constitution: 14,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        // 5e druids speak Druidic in addition to their starting language.
        languages: HashSet::from([Language::Common, Language::Druidic]),
        cr: 4.0,
        size: Size::Medium,
        actions,
        // Level-9 full-caster loadout — mirrors wizard / cleric.
        spell_slots_by_level: vec![4, 3, 3, 2, 2, 1, 1, 1, 1],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // 5e druids are proficient in INT and WIS saves.
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
    }
});
