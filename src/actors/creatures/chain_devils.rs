use crate::actions::class_features::{DEVILS_SIGHT_TAG, MAGICAL_ATTACKS_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CHAIN_DEVIL_CHAIN, CHAIN_DEVIL_MULTI};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Chain Devil (Kyton) — CR 8 medium fiend. The Hells' torturer, and the
/// bestiary's most single-minded control creature: two reach-10 chains a
/// round, each one a DC 15 DEX save against being Restrained.
///
/// Action lanes:
/// - **chain devil multiattack** — 2 chains. Two saves a round against
///   the same condition is what makes the kyton a lockdown creature
///   rather than a damage one: 2d6+4 twice is unremarkable at CR 8, and
///   a party member who fails both saves has stopped moving.
/// - **chain devil chain** (standalone) — the same swing, once.
///
/// **Devil's Sight** and **Magic Resistance** are the family envelope.
/// The kyton also has RAW's **Unnerving Mask** — a reaction-adjacent
/// clause where the devil's face becomes someone the target has lost,
/// forcing a DC 14 WIS save or Frightened. It is not carried: the engine
/// has no lane for a per-target illusion that reshapes itself, and the
/// clause RAW attaches it to (Animate Chains) is a summon of terrain
/// this stat block does not have. What is kept is the half that
/// mattered — the chains, and the fact that they hold.
///
/// The reach is the other half. Ten feet means a kyton restrains from
/// outside the reach of most of what it restrains, and a Restrained
/// creature cannot close the gap.
///
/// Stat shape per the SRD: AC 15 (natural armor), 85 HP (10d8+40), STR
/// 18 / DEX 15 / CON 18 / INT 11 / WIS 12 / CHA 14. Speed 30. Proficient
/// CON / WIS / CHA saves. Darkvision 120. Languages: Infernal. CR 8.
pub static CHAIN_DEVIL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*CHAIN_DEVIL_MULTI);
    actions.push(&CHAIN_DEVIL_CHAIN);
    CreatureTemplate {
        name: "Chain Devil",
        // 'B' (uppercase) — the devil band, shared with the Bearded and
        // Barbed Devils. Named for the family rather than the word, so
        // the map reads "a devil" at a glance and the panel says which.
        glyph: 'B',
        ac: 15,
        // 10d8+40 ≈ 85 average per the SRD (CR 8).
        hitpoints: "10d8+40".parse().unwrap(),
        speed: 30.,
        strength: 18,
        dexterity: 15,
        constitution: 18,
        intelligence: 11,
        wisdom: 12,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Infernal]),
        cr: 8.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: damage_modifiers_from([
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        has_magic_resistance: true,
        features: HashSet::from([DEVILS_SIGHT_TAG, MAGICAL_ATTACKS_TAG]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &CHAIN_DEVIL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn chain_devil_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 8.0);
        assert_eq!(a.creature_type(), CreatureType::Fiend);
        assert!(a.find_action("chain devil multiattack").is_some());
        assert!(a.find_action("chain devil chain").is_some());
    }

    /// The chain reaches ten feet and lands Restrained, and both halves
    /// have to be true for the creature to work: a reach-5 chain is a
    /// devil standing where its victim can hit it back, and a chain
    /// that grappled instead would be a condition the engine reads as
    /// strictly weaker.
    #[test]
    fn the_chain_restrains_from_outside_its_victims_reach() {
        assert_eq!(CHAIN_DEVIL_CHAIN.condition, Condition::Restrained);
        assert_eq!(CHAIN_DEVIL_CHAIN.reach, 2);
        assert_eq!(CHAIN_DEVIL_CHAIN.save_ability, AbilityScoreType::Dexterity);
    }
}
