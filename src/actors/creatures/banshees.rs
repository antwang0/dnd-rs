use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BANSHEE_WAIL, CORRUPTING_TOUCH};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Banshee — CR 4 undead. Frail body (12 AC, 58 average HP) wrapped
/// around a brutal AoE: a once-per-encounter wail that frightens and
/// damages every nearby non-undead. Pair the wail with the corrupting
/// touch (3d6+CHA necrotic) for the finisher — the banshee is a glass
/// cannon, not a brawler. Like other undead it's immune to poison,
/// necrotic, and the standard charm/frighten lockdown.
pub static BANSHEE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CORRUPTING_TOUCH);
    actions.push(&*BANSHEE_WAIL);
    CreatureTemplate {
        name: "Banshee",
        // 'b' was free (B is Bugbear); use 'b' for banshee.
        glyph: 'b',
        ac: 12,
        // 13d8 = 58 average per MM.
        hitpoints: "13d8".parse().unwrap(),
        speed: 30.,
        strength: 1,
        intelligence: 12,
        dexterity: 14,
        wisdom: 11,
        constitution: 10,
        charisma: 17, // primary stat — drives CORRUPTING_TOUCH
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
            // 5e: resistance to non-magical bludgeoning/piercing/slashing.
            // We don't track magical weapon flags so we apply the
            // resistance directly (mirrors the wraith / specter pattern).
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::from([AbilityScoreType::Charisma]),
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Grappled,
            Condition::Prone,
            Condition::Restrained,
        ]),
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
