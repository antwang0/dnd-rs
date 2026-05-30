use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SALAMANDER_MULTI, SALAMANDER_SPEAR, SALAMANDER_TAIL};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Salamander — CR 5 elemental. Fire-attuned serpentine giant from the
/// Elemental Plane of Fire. Action lanes:
/// - **Multiattack** (1 spear + 1 tail) — heterogeneous compound; spear
///   pokes at reach 2, tail whips at reach 3.
/// - **Salamander Spear** (standalone) — 2d6 piercing + 1d6 fire rider.
/// - **Salamander Tail** (standalone) — 2d6 bludgeoning + 1d6 fire rider.
///
/// MM RAW: AC 15, ~90 HP (12d10+24), STR 18, DEX 14, CON 15. **Fire
/// immune**, **cold vulnerable**, **fire shield** style: every melee
/// attacker takes a hit of heat damage. We don't model the Heated Body
/// reflect (it would need its own caster-side rider table — out of scope
/// for this drop); the fire/cold envelope is the load-bearing flavor.
pub static SALAMANDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SALAMANDER_MULTI);
    actions.push(&*SALAMANDER_SPEAR);
    actions.push(&*SALAMANDER_TAIL);
    CreatureTemplate {
        name: "Salamander",
        // 'a' (lowercase) — fire-themed serpentine glyph. Distinct from
        // existing 'A' (Animated Armor) and 's' (Skeleton).
        glyph: 'a',
        ac: 15,
        // 12d10+24 ≈ 90 average per MM (CR 5).
        hitpoints: "12d10+24".parse().unwrap(),
        speed: 30.,
        strength: 18,
        intelligence: 11,
        dexterity: 14,
        wisdom: 10,
        constitution: 15,
        charisma: 12,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            // Fire immunity — they ARE fire.
            (DamageType::Fire, DamageModifier::Immunity),
            // Cold vulnerability — water and ice undo them.
            (DamageType::Cold, DamageModifier::Vulnerability),
            // Mundane B/P/S resistance per MM (we approximate as flat).
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
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
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        sorcery_points: 0,
    }
});
