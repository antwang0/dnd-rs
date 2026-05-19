use crate::actions::class_features::{RAGE, RAGE_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GREATAXE, RECKLESS_ATTACK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Barbarian PC template. The classic STR-melee bruiser: heavy HP from
/// the 1d12 hit die, modest AC (unarmored — relies on the rage damage
/// reduction instead), STR-primary. Headline mechanics:
/// - **Rage** (bonus action, 1/long rest): resistance to bludgeoning /
///   piercing / slashing, advantage on STR checks/saves.
/// - **Reckless Attack** (bonus action): grants advantage on the next
///   melee swing this turn at the cost of attackers having advantage
///   against the barbarian until their next turn.
/// - Greataxe (1d12 slashing) as the signature damage weapon.
///
/// Stats target a level-3 barbarian: 32 HP (3d12+9), AC 13 (unarmored
/// defense baseline ≈ 10 + DEX(+1) + CON(+2)), STR 16, CON 14.
pub static BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GREATAXE);
    actions.push(&*RAGE);
    actions.push(&*RECKLESS_ATTACK);
    CreatureTemplate {
        name: "Barbarian",
        glyph: 'B',
        ac: 13,
        hitpoints: "3d12+9".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 8,
        dexterity: 12,
        wisdom: 12,
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
        // Barbarians are proficient in STR and CON saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Strength, AbilityScoreType::Constitution]),
        condition_immunities: HashSet::new(),
        features: HashSet::from([RAGE_TAG]),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
    }
});
