use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DJINNI_MULTI, DJINNI_SCIMITAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    ELEMENTAL_CONDITION_IMMUNITIES, elemental_damage_modifiers,
};
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Djinni — CR 11 large elemental (air genie). The noble Lord of the
/// Plane of Air: a turbaned blue-skinned giant wreathed in shrieking
/// winds, wielding a scimitar that crackles with thunder. Slots above
/// the CR 5–6 elemental quartet (Fire / Water / Earth / Air Elemental)
/// and alongside the Nalfeshnee (CR 13 demon), Horned Devil (CR 11),
/// Roper (CR 5), and Behir (CR 11) on the upper-mid extraplanar bench.
/// The genie family also includes the efreeti (fire), marid (water),
/// and dao (earth) — all four CR-11 noble genies now modeled.
///
/// Action lanes:
/// - **djinni multiattack** — 3 scimitar swings per Action via the shared
///   homogeneous `Multiattack` chassis. Heavier than the Bandit Captain's
///   triple-scimitar (1d6+STR slashing + 1d6 thunder rider per swing);
///   each rider routes through `add_flat_damage_rider` so per-target
///   thunder resistance applies independently from the slashing base.
/// - **djinni scimitar** (standalone) — STR-based 1d6+STR slashing with
///   the 1d6 thunder rider for the AI's single-target fallback.
///
/// Defensive identity: AC 17 (natural armor — the air genie's gleaming
/// scaled hide), 161 HP (14d10+84). Standard elemental envelope:
/// non-magical BPS resistance, poison immunity. Lightning + thunder
/// damage resistance (the air genie's affinity for its own element).
/// Magic Resistance gives advantage on every save vs spells. Standard
/// 9-condition elemental immunity envelope (Charmed / Frightened /
/// Paralyzed / Petrified / Poisoned / Asleep / Prone / Grappled /
/// Restrained) via the shared `ELEMENTAL_CONDITION_IMMUNITIES`.
///
/// Stat shape: AC 17, ~161 HP (14d10+84), STR 21, DEX 15, CON 22,
/// INT 15, WIS 16, CHA 20. Speed 30 (RAW also grants fly 90 which we
/// don't model). Senses: Darkvision 120ft. Languages: Auran collapsed
/// to Primordial in this engine. Size Large. CR 11.
pub static DJINNI_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DJINNI_MULTI);
    actions.push(&DJINNI_SCIMITAR);
    CreatureTemplate {
        name: "Djinni",
        // 'D' (uppercase) — distinct from existing 'd' (Drow / Dryad /
        // Drider). 'D' for the towering djinn silhouette.
        glyph: 'D',
        ac: 17,
        // 14d10+84 ≈ 161 average per MM (CR 11).
        hitpoints: "14d10+84".parse().unwrap(),
        speed: 30.,
        strength: 21,
        intelligence: 15,
        dexterity: 15,
        wisdom: 16,
        constitution: 22,
        charisma: 20,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 11.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        // Djinni proficient saves: DEX, WIS, CHA per MM. The agile +
        // willful + force-of-personality saves befitting an air genie's
        // signature traits.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Standard elemental damage envelope (poison immune + BPS
        // resistance) overlaid with thunder + lightning resistance — the
        // air genie's signature elemental affinity. Mirrors the Air
        // Elemental's overlay shape but lifted from "lightning only" to
        // the broader "thunder + lightning" pair.
        damage_modifiers: elemental_damage_modifiers([
            (DamageType::Thunder, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
        ]),
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        // Magic Resistance: advantage on saves vs spells / magical
        // effects. Standard upper-tier genie / fey / fiend trait.
        has_magic_resistance: true,
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn djinni_template_shape() {
        let a = ActorInstance::from_creature_template(
            &DJINNI_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 11.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        // Multi primary + standalone scimitar fallback.
        assert!(a.find_action("djinni multiattack").is_some());
        assert!(a.find_action("djinni scimitar").is_some());
    }

    #[test]
    fn djinni_has_elemental_envelope() {
        let a = ActorInstance::from_creature_template(
            &DJINNI_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Elemental envelope: poison immune, BPS resistant, lightning +
        // thunder resistant from the air-element overlay.
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Thunder),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Lightning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        assert!(a.has_magic_resistance());
        // Full elemental condition envelope.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
    }
}
