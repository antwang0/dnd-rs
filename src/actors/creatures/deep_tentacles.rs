use crate::actions::action_template::{Action, TargetingSchema, action_only, first_target_id};
use crate::actions::class_features::{SWIM_SPEED_TAG, TENTACLE_OF_THE_DEEP_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::simple_weapon_attack;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::{ApplicableSideEffect, ApplyCondition, Resource};
use crate::engine::types::{
    AbilityScoreType, Coordinate, CreatureType, DamageModifier, DamageType, Size,
    SpecialSense,
};
use crate::engine::action_overrides::ActionOverride;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// **Tentacle Slam** — the deep tentacle's only attack. RAW: the
/// warlock spends a bonus action to have the tentacle lash a creature
/// within 10 ft of it for `1d8` cold, and that creature's "speed is
/// reduced by 10 feet until the start of your next turn."
///
/// Both halves ship. The cold die is the small part; the ten feet is
/// the reason the subclass has a tentacle at all — a Fathomless warlock
/// parks it in a doorway and every creature that comes through arrives
/// a round later than it meant to.
///
/// Reach 4 tiles is RAW's 10 ft on the 2.5 ft grid, measured from the
/// tentacle rather than from the warlock, which is what putting the
/// attack on the tentacle's own stat block buys.
pub struct TentacleSlam {}

impl Action for TentacleSlam {
    fn name(&self) -> &str {
        "tentacle slam"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["slam", "lash"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(4)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
    }
    fn expected_damage(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
    ) -> Option<f32> {
        // Flat dice — the tentacle adds no ability modifier to damage,
        // so the estimate is the die average and nothing else.
        Some(Dice::new(1, 8).average_roll())
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_only()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Dexterity,
            // RAW's `1d8 cold` carries no ability modifier.
            None,
            Dice::new(1, 8),
            DamageType::Cold,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        encounter.log("  the coils drag at the target's legs (speed -10 ft)");
        effects.push(Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Coiled,
            // RAW ends it at the start of the warlock's next turn, which
            // is one round on this engine's turn model.
            timer: ConditionTimer::Rounds(1),
        }));
        effects
    }
}

pub static TENTACLE_SLAM: LazyLock<TentacleSlam> = LazyLock::new(|| TentacleSlam {});

/// Tentacle of the Deep — the spectral limb a Fathomless Warlock calls
/// up out of nothing, and the second creature in the bestiary (after
/// the wildfire spirit) that exists to be somebody's subclass.
///
/// It is barely a combatant: 10 hit points, AC 13, one 1d8 slam. What
/// it does instead is occupy a square and make everything within ten
/// feet of that square slower — and, once Guardian Coil is online, make
/// everything within ten feet of that square harder to hurt. So the
/// tentacle is a *place*, and playing the subclass is deciding which
/// place matters.
///
/// That reading is why the slam lives on this stat block rather than as
/// a warlock action with a long reach: RAW measures the lash's 10 ft
/// from the tentacle, and an action on the warlock's sheet would have
/// had to measure it from the warlock. The same goes for Guardian
/// Coil's radius, which reads the tentacle's position off the board
/// through `TENTACLE_OF_THE_DEEP_TAG`.
///
/// The departure from RAW is the turn economy. RAW's tentacle acts on
/// the warlock's bonus action and never on its own initiative; here it
/// is an ordinary summoned ally that rolls initiative and takes turns,
/// the same simplification `RANGERS_COMPANION_TEMPLATE` and the
/// wildfire spirit already make. The engine has no lane for a creature
/// that occupies a square but is driven entirely from another
/// creature's action economy, and inventing one for a single consumer
/// would be a larger change than the fidelity it buys.
///
/// Cold immunity and the standard elemental condition envelope: the
/// tentacle has no metabolism, no joints and no mind, and it is made of
/// the same freezing dark it throws.
///
/// Glyph 't' (lowercase) — for **t**entacle, paired with the Fathomless
/// warlock's 'T' the way the wildfire spirit's 'w' pairs with its
/// druid's 'W'.
pub static TENTACLE_OF_THE_DEEP_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*TENTACLE_SLAM);
    CreatureTemplate {
        name: "Tentacle of the Deep",
        glyph: 't',
        ac: 13,
        // RAW: 10 hit points, flat. A tentacle that survived a swing
        // would be a pet; one that doesn't is a position the enemy can
        // take away, which is the interesting version.
        hitpoints: "3d6".parse().unwrap(),
        // RAW: speed 0 — the tentacle is rooted where it was called. The
        // engine's mover reads `speed()` directly, so a zero here is the
        // whole implementation of "it does not move".
        speed: 0.,
        strength: 10,
        dexterity: 14,
        constitution: 10,
        intelligence: 1,
        wisdom: 10,
        charisma: 1,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        // Understands its summoner and speaks to nobody.
        languages: HashSet::new(),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Aberration,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Cold, DamageModifier::Immunity)]),
        condition_immunities:
            crate::actors::creatures::fire_elementals::ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        // The beacon Guardian Coil's clamp row searches the board for.
        // Carried by the tentacle rather than back-linked from the
        // warlock, for the reasons `WILDFIRE_SPIRIT_TAG` gives.
        // The second tag is the "of the Deep" half taken literally: the
        // thing the invocation calls up is a tentacle out of the ocean,
        // and a pool on the board is the only water it will ever see.
        features: HashSet::from([TENTACLE_OF_THE_DEEP_TAG, SWIM_SPEED_TAG]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;

    fn tentacle() -> ActorInstance {
        ActorInstance::from_creature_template(
            &TENTACLE_OF_THE_DEEP_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn the_tentacle_carries_the_beacon_guardian_coil_searches_for() {
        assert!(tentacle().has_passive_feature(TENTACLE_OF_THE_DEEP_TAG));
    }

    /// RAW's tentacle has speed 0 — it is a place, not a pet. The whole
    /// implementation of that is the template field, so it is worth a
    /// line here: a tentacle that could walk would be a different
    /// subclass.
    #[test]
    fn the_tentacle_stays_where_it_was_called() {
        assert_eq!(tentacle().speed(), 0.0);
    }

    #[test]
    fn the_slam_reaches_ten_feet_and_freezes() {
        let a = tentacle();
        assert!(a.find_action("tentacle slam").is_some());
        assert_eq!(TENTACLE_SLAM.reach_tiles(), Some(4));
        assert_eq!(TENTACLE_SLAM.damage_types(), vec![DamageType::Cold]);
    }

    /// The coil takes ten feet off, and the shared speed cohort is
    /// where it does it — so it composes with a buff rather than
    /// overriding one.
    #[test]
    fn coiling_a_creature_costs_it_ten_feet_and_composes() {
        let mut victim = ActorInstance::from_creature_template(
            &crate::actors::creatures::goblins::GOBLIN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        let base = victim.speed();
        victim.add_condition(Condition::Coiled, ConditionTimer::Rounds(1));
        assert_eq!(victim.speed(), base - 10.0);
        // Longstrider's +10 cancels it exactly, which is the point of
        // routing both through one signed sum.
        victim.add_condition(Condition::Longstriding, ConditionTimer::Rounds(10));
        assert_eq!(victim.speed(), base);
    }
}
