use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{INTELLECT_DEVOURER_CLAWS, INTELLECT_DEVOURER_DEVOUR};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Intellect Devourer — CR 2 tiny aberration. The mind-flayer's brain-on-
/// legs servitor: low HP, low STR, but the signature `INTELLECT_DEVOURER_
/// DEVOUR` INT-save attack is one of the rare INT-save lanes in the
/// engine, and the stun rider triggers when the psychic damage knocks
/// the target below half HP. Fills the "save against your mental stat"
/// niche between the Mind Flayer (Mind Blast: INT-save burst) and the
/// Nothic (WIS-save rotting gaze).
///
/// RAW resistances: bludgeoning / piercing / slashing from non-magical
/// non-adamantine weapons. We approximate with a flat physical
/// resistance envelope (same shape as the quasit / succubus). Immune to
/// blinded — has no eyes; reads its environment via Blindsight.
///
/// Stats roughly track MM Intellect Devourer at CR 2 — high DEX (its
/// scrabbling movement), modest CON (2d6+2 = 21 HP), INT 12 for
/// telepathy / mental probing flavor.
pub static INTELLECT_DEVOURER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&INTELLECT_DEVOURER_CLAWS);
    actions.push(&*INTELLECT_DEVOURER_DEVOUR);
    CreatureTemplate {
        name: "Intellect Devourer",
        // 'Ψ' (Greek capital psi) — distinct from 'Z' (Zombie family) and
        // 'M' (Mind Flayer). The trident-like shape evokes the
        // brain-on-legs silhouette of a scurrying psionic aberration.
        glyph: 'Ψ',
        ac: 12,
        hitpoints: "6d4+6".parse().unwrap(),
        speed: 40.,
        strength: 6,
        intelligence: 12,
        dexterity: 14,
        wisdom: 11,
        constitution: 13,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        languages: HashSet::from([Language::DeepSpeech, Language::Undercommon]),
        cr: 2.0,
        size: Size::Tiny,
        creature_type: CreatureType::Aberration,
        actions,
        damage_modifiers: non_magical_physical_resistances([]),
        // Immune to Blinded — the devourer has no eyes to lose.
        condition_immunities: HashSet::from([crate::conditions::Condition::Blinded]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    #[test]
    fn devourer_is_blinded_immune() {
        let a = ActorInstance::from_creature_template(
            &INTELLECT_DEVOURER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert!(a.effectively_immune_to_condition(crate::conditions::Condition::Blinded));
    }

    #[test]
    fn devourer_resists_physical() {
        let a = ActorInstance::from_creature_template(
            &INTELLECT_DEVOURER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
    }
}
