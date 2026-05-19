use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGBOW, SCIMITAR};
use crate::actions::spells::{
    CURE_WOUNDS, FAERIE_FIRE, HAIL_OF_THORNS, HUNTERS_MARK, LESSER_RESTORATION, SPIKE_GROWTH,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Ranger PC template. DEX-primary half-caster martial. Plays as a
/// kiter — longbow as the workhorse attack, Hunter's Mark for the
/// per-hit +1d6 rider, Hail of Thorns for an opening AoE on the lead
/// shot, Cure Wounds + Lesser Restoration for self-sustain. Mid-CR PC
/// with a small but focused spell list that the AI's existing
/// targeting heuristics already exercise (kite + ranged-with-rider).
///
/// Stats target a level-5 ranger: ~32 HP (5d10+5), AC 15 (studded
/// leather + DEX), DEX 16, WIS 14, half-caster slots (4/2). Scimitar
/// as the melee fallback when an enemy closes through the kite.
pub static RANGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LONGBOW);
    actions.push(&SCIMITAR);
    actions.push(&*HUNTERS_MARK);
    actions.push(&*HAIL_OF_THORNS);
    actions.push(&*CURE_WOUNDS);
    actions.push(&*LESSER_RESTORATION);
    actions.push(&*FAERIE_FIRE);
    actions.push(&*SPIKE_GROWTH);
    CreatureTemplate {
        name: "Ranger",
        glyph: 'R',
        ac: 15,
        hitpoints: "5d10+5".parse().unwrap(),
        speed: 30.,
        strength: 12,
        intelligence: 10,
        dexterity: 16,
        wisdom: 14, // spellcasting ability
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Elvish]),
        cr: 1.0,
        size: Size::Medium,
        actions,
        // Half-caster slots: level-5 ranger has 4 level-1 and 2 level-2.
        spell_slots_by_level: vec![4, 2],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Rangers are proficient in STR and DEX saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
    }
});
