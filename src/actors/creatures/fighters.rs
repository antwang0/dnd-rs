use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, INDOMITABLE, INDOMITABLE_TAG, SECOND_WIND, SECOND_WIND_TAG,
    TRIP_ATTACK, TRIP_ATTACK_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCIMITAR;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Fighter — the simplest player class. Heavy armor, decent HP, one
/// martial weapon (scimitar — STR-based slashing) and the standard
/// movement actions. No spells. The headline distinction from monsters
/// is `rolls_death_saves: true` — at 0 HP a Fighter enters the dying
/// state and rolls saves on each of their turns instead of dropping
/// outright.
///
/// Stats are roughly a level-3 fighter: 24 HP (3d10+6), AC 16 from
/// chain mail, STR 16 (the standard "strength build" defaults).
pub static FIGHTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    actions.push(&*INDOMITABLE);
    // Trip Attack — Battle Master maneuver (once per long rest in our
    // model). Bonus action prime; next melee hit forces a STR save vs
    // the fighter's maneuver DC or knocks the target Prone. Sets up
    // the prone-melee-advantage clause for follow-up swings.
    actions.push(&*TRIP_ATTACK);
    CreatureTemplate {
        name: "Fighter",
        glyph: 'F',
        ac: 16,
        hitpoints: "3d10+6".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 10,
        dexterity: 12,
        wisdom: 11,
        constitution: 14,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Fighters are proficient in STR and CON saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Strength, AbilityScoreType::Constitution]),
        condition_immunities: HashSet::new(),
        features: HashSet::from([
            SECOND_WIND_TAG,
            ACTION_SURGE_TAG,
            INDOMITABLE_TAG,
            TRIP_ATTACK_TAG,
        ]),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
    }
});
