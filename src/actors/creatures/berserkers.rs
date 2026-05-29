use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BERSERKER_GREATAXE, RECKLESS_ATTACK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Berserker — CR 2 humanoid. A high-HP melee bruiser with Reckless
/// Attack: a bonus-action self-buff that grants advantage on the next
/// attack but leaves the berserker easier to hit until the start of
/// their next turn. Pairs with a greataxe for big-die swings; the AI
/// will (eventually) learn the bonus-action / greataxe combo, but
/// even at a flat-line policy the greataxe alone makes them feel
/// dangerous.
pub static BERSERKER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BERSERKER_GREATAXE);
    actions.push(&*RECKLESS_ATTACK);
    CreatureTemplate {
        name: "Berserker",
        // 'b' — distinct from 'B' (Bandit Captain).
        glyph: 'b',
        ac: 13,
        // 9d8+27 = 67 average per MM.
        hitpoints: "9d8+27".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 9,
        dexterity: 12,
        wisdom: 11,
        constitution: 17,
        charisma: 9,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::from([AbilityScoreType::Strength]),
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
    }
});
