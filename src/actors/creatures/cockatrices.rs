use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::COCKATRICE_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Cockatrice — CR 1/2 monstrosity. Tiny flier with a petrifying bite:
/// 1d4 piercing on hit, plus a CON save (DC 11) or be Petrified for one
/// round. Petrified locks the target out of their action economy and
/// auto-fails STR/DEX saves, so a single bad save can turn an encounter.
/// AC and HP are tuned low (AC 11, ~3d6 HP) so the cockatrice itself
/// goes down quickly — the threat is the rider, not the body.
pub static COCKATRICE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*COCKATRICE_BITE);
    CreatureTemplate {
        name: "Cockatrice",
        // 'k' is currently free (Kobold uses 'K' uppercase, no others
        // claim lowercase k).
        glyph: 'k',
        ac: 11,
        hitpoints: "5d6".parse().unwrap(),
        speed: 20., // 5e: 20ft walking + 40ft fly — we model walking only.
        strength: 6,
        intelligence: 2,
        dexterity: 12,
        wisdom: 13,
        constitution: 12,
        charisma: 5,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 0.5,
        size: Size::Small,
        creature_type: CreatureType::Monstrosity,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
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
    }
});
