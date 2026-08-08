use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ERINYES_LONGSWORD, ERINYES_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Erinyes — CR 12 fallen-angel devil. Flying STR-based melee boss with
/// a poisoned longsword (2d8+4 slashing + 3d8 poison rider) and the
/// standard devil immunity envelope (fire / poison; can't be poisoned
/// or charmed). At CR 12, the multiattack burst can land 6d8+12 slashing
/// + 9d8 poison on a full connect — a true threat to a mid-tier party.
///
/// Stats target the MM Erinyes block: 153 HP, AC 18, STR 18 / DEX 16 /
/// CON 18 / WIS 14 / CHA 18. Proficient DEX / CON / WIS / CHA saves —
/// devils are tough across the mental save lane, and the high DEX save
/// is the defining "flying angel" survivability.
pub static ERINYES_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ERINYES_LONGSWORD);
    actions.push(&*ERINYES_MULTI);
    CreatureTemplate {
        name: "Erinyes",
        // 'E' — distinct from Imp (capital 'i' is taken by Illithid /
        // Mind Flayer, lowercase 'i' is Imp).
        glyph: 'E',
        ac: 18,
        // 18d8+72 ≈ 153 average per the MM Erinyes stat block.
        hitpoints: "18d8+72".parse().unwrap(),
        // flying speed 60 RAW — we model as ground speed for grid mobility
        strength: 18,
        dexterity: 16,
        constitution: 18,
        intelligence: 14,
        wisdom: 14,
        charisma: 18, // CHA-anchor for the devil's spell DC
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        languages: HashSet::from([Language::Infernal, Language::Common]),
        cr: 12.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        // MM Erinyes: immune to fire + poison; resistant to cold +
        // non-magical bludgeoning / piercing / slashing. We omit the
        // magical-vs-mundane resistance distinction.
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
        ]),
        // MM Erinyes proficient saves: DEX / CON / WIS / CHA — the
        // devil's "untouchable" save profile.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Devil envelope: can't be poisoned or charmed. We add
        // Frightened-immunity too since the MM Erinyes lists it.
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
        ]),
        has_magic_resistance: true,
        // 5e **Devil's Sight** — "magical darkness doesn't impede this
        // devil's darkvision." Carried by every devil in the bestiary,
        // and the one thing in the game that sees through the Darkness
        // spell. Before the lighting layer existed the trait was
        // approximated as a generous darkvision radius, which was the
        // closest the engine could get to it and got the crucial half
        // exactly backwards: RAW darkvision is precisely what magical
        // darkness defeats.
        features: HashSet::from([crate::actions::class_features::DEVILS_SIGHT_TAG]),
        ..CreatureTemplate::defaults()
    }
});
