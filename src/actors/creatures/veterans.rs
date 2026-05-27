use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEAVY_CROSSBOW, VETERAN_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Veteran — CR 3 humanoid soldier. Plate-armored melee with a 2x
/// longsword multiattack plus a heavy crossbow for ranged finishers.
/// Higher AC than the berserker but lower HP — the trade-off
/// between staying power and stack.
pub static VETERAN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*VETERAN_MULTI);
    actions.push(&*HEAVY_CROSSBOW);
    CreatureTemplate {
        name: "Veteran",
        // 'v' — distinct from existing 'V' (Vampire Spawn).
        glyph: 'v',
        ac: 17,
        // 9d8+18 = 58 average per MM.
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 10,
        dexterity: 13,
        wisdom: 11,
        constitution: 14,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        // 5e MM Veteran: proficient in athletics + perception (skills),
        // no save proficiencies. We mirror that.
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
        has_extra_attack: true,
    }
});
