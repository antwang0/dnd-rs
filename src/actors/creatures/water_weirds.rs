use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::WATER_WEIRD_CONSTRICT;
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Water Weird — CR 3 large elemental. A serpent of animate water bound
/// to a fountain or a cistern, invisible until it strikes.
///
/// Action lane: **water weird constrict**, 3d6+STR bludgeoning at reach
/// 10 ft. that Restrains on hit with no save. That last part is RAW —
/// the weird's grab is not contested when it lands; the escape DC is
/// what the victim rolls on their own turn, which the engine spells as
/// a timer.
///
/// **Invisible in Water** is carried through `innate_conditions`, the
/// same lane the Invisible Stalker uses, and it is the reason a CR 3
/// creature opens the fight on its own terms: the first constrict comes
/// from something nobody could target. The condition is permanent here
/// rather than water-gated, which is a deliberate over-reach in the
/// weird's favour — the engine has no "is the creature in water"
/// predicate on the invisibility lane, and a water weird is generated
/// in the pool of things that guard water.
///
/// It also breaks the ordinary way: attacking does not reveal a water
/// weird, because RAW's invisibility is a property of the medium rather
/// than a spell it is maintaining. The engine's `Invisible` condition
/// does not clear on attack either, so the two agree by construction.
///
/// The rest of the envelope is the elemental one — poison immune,
/// nonmagical physical resistance, and the condition list of a thing
/// made of water — plus **cold vulnerability**, which is the weird's
/// own and the answer a party is supposed to find: you do not stab
/// water, you freeze it.
///
/// Stat shape per the SRD: AC 13, 58 HP (9d10+9), STR 17 / DEX 16 / CON
/// 13 / INT 11 / WIS 10 / CHA 10. Speed 30 with a swim speed.
/// Blindsight 30. Languages: Primordial (Aquan). CR 3.
pub static WATER_WEIRD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WATER_WEIRD_CONSTRICT);
    CreatureTemplate {
        name: "Water Weird",
        // 'W' (uppercase) — the water band, next to the Water Elemental
        // it most often shares a cistern with.
        glyph: 'W',
        ac: 13,
        // 9d10+9 ≈ 58 average per the SRD (CR 3).
        hitpoints: "9d10+9".parse().unwrap(),
        speed: 30.,
        strength: 17,
        dexterity: 16,
        constitution: 13,
        intelligence: 11,
        wisdom: 10,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Blindsight(30)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        damage_modifiers: damage_modifiers_from([
            (DamageType::Poison, DamageModifier::Immunity),
            // The answer the party is supposed to find.
            (DamageType::Cold, DamageModifier::Vulnerability),
        ]),
        condition_immunities: HashSet::from([
            Condition::Exhausted,
            Condition::Grappled,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Prone,
            Condition::Restrained,
            Condition::Unconscious,
        ]),
        // Invisible in Water — see the type docs for why it is
        // unconditional here.
        innate_conditions: vec![(Condition::Invisible, ConditionTimer::Permanent)],
        // RAW's swim speed: what makes water free to cross and lifts
        // the underwater melee penalty on the constrict.
        features: HashSet::from([SWIM_SPEED_TAG]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
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
            &WATER_WEIRD_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn water_weird_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 3.0);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        assert!(a.find_action("water weird constrict").is_some());
    }

    /// The weird arrives invisible rather than turning invisible, which
    /// is the difference between an ambusher and a creature with a
    /// spell.
    ///
    /// Driven through `instantiate_creature` rather than through
    /// `from_creature_template`, and that is the whole assertion: the
    /// `innate_conditions` lane is applied by the *encounter* when a
    /// creature reaches the board, not by the constructor. A test that
    /// built the actor directly would read an empty condition set on a
    /// template whose list is correct, and a test that read the
    /// template's list would pass even if nothing ever applied it.
    #[test]
    fn the_weird_is_already_invisible_when_it_reaches_the_board() {
        use crate::engine::actor_gen::ActorGenParams;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;

        let mut e = EncounterInstance::from_params(
            &TerrainGenParams {
                width: 20,
                height: 20,
                branch_depth: 0,
                branch_prob: 0.0,
            },
            &ActorGenParams {
                cr_target: 0.0,
                n_teams: 0,
                pc_template: None,
                start_team: 0,
            },
            Some(3),
        )
        .unwrap();
        let id = e
            .instantiate_creature(&WATER_WEIRD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        assert!(e.actors[&id].has_condition(Condition::Invisible));
    }

    /// You do not stab water. Cold vulnerability against nonmagical
    /// physical resistance is the whole shape of the fight, and the two
    /// have to be on opposite sides of the ledger for it to work.
    #[test]
    fn steel_glances_off_and_cold_bites_double() {
        let a = make();
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Vulnerability)
        );
        assert_eq!(
            a.nonmagical_damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
    }
}
