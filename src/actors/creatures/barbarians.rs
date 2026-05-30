use crate::actions::class_features::{RAGE, RAGE_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GREATAXE, RECKLESS_ATTACK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
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
/// - **Brutal Critical** (level 9): on a critical melee weapon hit, roll
///   one additional damage die of the weapon's type. Scales to 2 dice
///   at level 13 and 3 at level 17 — we model the level-9 baseline here.
/// - Greataxe (1d12 slashing) as the signature damage weapon.
///
/// Stats target a level-9 barbarian: 76 HP (9d12+18), AC 15 (unarmored
/// defense ≈ 10 + DEX(+1) + CON(+4) at CON 18), STR 18, CON 18 — the
/// classic "rage tank" loadout. Bumped from the prior level-3 build to
/// surface the Brutal Critical rider at the lowest level that grants it.
pub static BARBARIAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GREATAXE);
    actions.push(&*RAGE);
    actions.push(&*RECKLESS_ATTACK);
    CreatureTemplate {
        name: "Barbarian",
        glyph: 'B',
        ac: 15,
        hitpoints: "9d12+18".parse().unwrap(),
        speed: 30.,
        strength: 18,
        intelligence: 8,
        dexterity: 12,
        wisdom: 12,
        constitution: 18,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
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
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: true,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: true,
        // Level 9 Brutal Critical: +1 weapon die on melee crits.
        brutal_critical_dice: 1,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
    }
});
