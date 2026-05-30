use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::VAMPIRIC_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Vampire Spawn — CR 5 undead. Vampiric bite hits hard with piercing +
/// 3d6 necrotic and feeds the spawn back for full necrotic HP — the
/// signature lifesteal pattern. Resistant to necrotic, immune to the
/// usual undead suite (poisoned / charmed). The natural counterpart to
/// the Wraith: instead of draining max-HP, the spawn caps off its own
/// pool and is much harder to chip down across a long encounter.
pub static VAMPIRE_SPAWN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*VAMPIRIC_BITE);
    CreatureTemplate {
        name: "Vampire Spawn",
        // 'V' for vampire — distinct from 'W' (wolf) and 'R' (wraith).
        glyph: 'V',
        ac: 15,
        hitpoints: "11d8+33".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 11,
        dexterity: 16,
        wisdom: 10,
        constitution: 16,
        charisma: 12,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 5.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Necrotic-resistant (5e MM vampires resist necrotic and non-
        // magical physical; we keep necrotic + the physical trio).
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::from([Condition::Poisoned, Condition::Charmed]),
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
    }
});
