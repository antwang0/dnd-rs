use crate::{
    actions::action_template::TargetingSchema,
    engine::{side_effects::GiveResource, types::Coordinate},
};
use std::{collections::HashSet, sync::LazyLock};

use crate::{
    actions::action_template::{Action, first_target_id, first_target_location},
    engine::{
        action_overrides::ActionOverride,
        encounter::EncounterInstance,
        side_effects::{MoveActor, Resource, SkipTurn},
    },
};

/// The 5e contest a shove or a grapple opens with: "a Strength
/// (Athletics) check contested by…". One option, so `best_check_option`
/// has nothing to choose between — the slice shape exists so both sides
/// of `roll_contest` speak the same language.
const ATHLETICS_CONTEST: &[(
    crate::engine::types::AbilityScoreType,
    crate::engine::types::Skill,
)] = &[(
    crate::engine::types::AbilityScoreType::Strength,
    crate::engine::types::Skill::Athletics,
)];

/// The defending half of that sentence: "…contested by the target's
/// Strength (Athletics) or Dexterity (Acrobatics) check (the target
/// chooses the ability to use)." The target's choice is resolved by
/// `best_check_option`, which compares the two pairings *including
/// proficiency* — so an acrobatic rogue answers with Acrobatics even
/// when their raw Strength modifier is the larger of the two.
///
/// Athletics is listed first so a defender equally good at both answers
/// with the same skill the challenger used, which is the version of the
/// tie a table would narrate — `best_check_option` resolves ties to the
/// earlier row for exactly that purpose.
const GRAPPLE_DEFENSE_CONTEST: &[(
    crate::engine::types::AbilityScoreType,
    crate::engine::types::Skill,
)] = &[
    (
        crate::engine::types::AbilityScoreType::Strength,
        crate::engine::types::Skill::Athletics,
    ),
    (
        crate::engine::types::AbilityScoreType::Dexterity,
        crate::engine::types::Skill::Acrobatics,
    ),
];

/// Escape DC for a hold with nobody on the other end of it — an ooze's
/// adhesive, a spell's tentacles, a grappler who has since left the
/// board. 8 + a typical grappler's Strength modifier + proficiency, the
/// number the contest would average to.
const UNANCHORED_ESCAPE_DC: i32 = 13;

/// One hold a creature can spend its Action trying to get out of.
///
/// The cohort behind [`GrappleEscape`], and the reason it is a table is
/// that the list was a hard-coded triple written out twice — once in
/// the validator that decides whether the Action is offered at all, and
/// once in the resolver that decides what a success removes. Two
/// spellings of one list is the shape that goes wrong silently: a
/// fourth hold added to the resolver and not the validator is a hold
/// whose escape never appears on the picker, and the reverse is an
/// Action the picker offers and the resolver frees nothing for.
///
/// It is also the field the triple could not carry. RAW prints a
/// *number* on some of these holds — the Energy Bow's arrow is a flat
/// DC 20 — and the hard-coded list had exactly one DC for all of them.
struct EscapableHold {
    /// The flag that says the holder is in this hold.
    condition: crate::conditions::Condition,
    /// The DC the captive's check has to beat when the hold names
    /// nobody to contest against.
    dc: i32,
    /// Whether a back-link on the condition turns this into RAW's
    /// *contest* rather than a flat check.
    ///
    /// True only for `Grappled`, which is the one hold in the game
    /// whose difficulty is another creature's Athletics — *"contested
    /// by the grappler's Strength (Athletics) check"*. A spell's
    /// tentacles and an arrow stuck through a boot have a printed
    /// number and nobody straining at the other end, so their link (if
    /// any) is there to name the source in the log, not to roll.
    contested: bool,
}

/// Every hold the Escape action answers, in the order it tries them.
///
/// The three that were the hard-coded triple keep the shared
/// unanchored DC they have always used; the fourth is the first row to
/// bring a number of its own.
const ESCAPABLE_HOLDS: &[EscapableHold] = &[
    EscapableHold {
        condition: crate::conditions::Condition::Grappled,
        dc: UNANCHORED_ESCAPE_DC,
        contested: true,
    },
    // An ooze's adhesive. RAW gives it an escape DC off the ooze's own
    // stat block; the engine has always spent the shared number here.
    EscapableHold {
        condition: crate::conditions::Condition::Adhered,
        dc: UNANCHORED_ESCAPE_DC,
        contested: false,
    },
    // Maximilian's Earthen Grasp. RAW's escape is a Strength check
    // against the caster's spell save DC, which the shared number
    // stands in for.
    EscapableHold {
        condition: crate::conditions::Condition::EarthenGrasped,
        dc: UNANCHORED_ESCAPE_DC,
        contested: false,
    },
    // SRD 5.2 **Energy Bow**, Arrow of Restraint: *"As an action, a
    // creature Restrained by an arrow can make a DC 20 Strength
    // (Athletics) check to try to break the restraint."* The first row
    // whose number is printed rather than averaged, and the reason the
    // struct has a `dc` column at all.
    EscapableHold {
        condition: crate::conditions::Condition::ArrowPinned,
        dc: 20,
        contested: false,
    },
];

pub struct Move {}

impl Action for Move {
    fn name(&self) -> &str {
        "move"
    }

    fn is_harmful(&self) -> bool {
        // Nobody is on the receiving end of this. `is_harmful` defaults
        // to true because most actions are attacks, and these four had
        // never said otherwise — which put them in every "walk the
        // actor's harmful actions" scan in the AI, to be rejected a
        // predicate later.
        false
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["mv"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        let Some(dest) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(dist) = encounter.path_cost_to(caster_id, dest) else {
            return Vec::new();
        };
        vec![Resource::Movement(dist)]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(tl) = target_locations else {
            return false;
        };
        let Some(&coord) = tl.first() else {
            return false;
        };
        // path_cost_to verifies destination footprint AND walkable path
        // within remaining movement budget.
        if encounter.path_cost_to(caster_id, coord).is_none() {
            return false;
        }
        // 5e Frightened: "you can't willingly move closer to the source
        // of your fear." A move that strictly decreases the gap to the
        // source is forbidden.
        //
        // *The* source, when the condition records one — the back-link
        // it carries is what turns RAW's sentence into a question the
        // board can answer. A fear installed by something that did not
        // record who caused it falls back to the older reading, in
        // which every enemy is a source, which is the safe direction:
        // it forbids a superset of the steps RAW forbids rather than
        // quietly letting a frightened creature walk at whatever
        // frightened it.
        use crate::conditions::Condition;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return true;
        };
        if !actor.has_condition(Condition::Frightened) {
            return true;
        }
        let fear_source = actor.linked_by(Condition::Frightened);
        // The gaps are measured from the body that is doing the
        // travelling, which for a mounted rider is the horse — the same
        // redirect `path_cost_to` above already made. A Medium knight on
        // a Large warhorse closes on an enemy with the horse's
        // four-tile footprint, and asking with the rider's two would
        // read every gap one tile wider than it is.
        let body_id = encounter.movement_body(caster_id);
        let body = encounter.actors.get(&body_id).unwrap_or(actor);
        let my_size = get_tiles_from_size(body.size());
        let my_loc = body.location();
        let my_team = actor.team();
        for (other_id, other) in encounter.actors.iter() {
            if *other_id == caster_id
                || *other_id == body_id
                || other.team() == my_team
                || !other.is_combat_active()
            {
                continue;
            }
            // With a named source, every other enemy on the board is
            // just an enemy: RAW's clause is about one creature, and a
            // frightened fighter is free to charge the goblin beside
            // the dragon it is running from.
            if fear_source.is_some_and(|src| src != *other_id) {
                continue;
            }
            let o_loc = other.location();
            let o_size = get_tiles_from_size(other.size());
            let cur = footprint_chebyshev(my_loc, my_size, o_loc, o_size);
            let after = footprint_chebyshev(coord, my_size, o_loc, o_size);
            if after < cur {
                return false;
            }
        }
        true
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_location) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Use the same Dijkstra path that `cost` charged for, so OAs fire on
        // every threatened-square exit along the way. Falls back to a
        // single-tile teleport if pathing fails (validate should already
        // have caught this, but defense in depth).
        let path = encounter
            .path_to(caster_id, target_location)
            .unwrap_or_else(|| vec![target_location]);
        vec![Box::new(MoveActor {
            actor_id: caster_id,
            path,
        })]
    }
}

pub static MOVE: LazyLock<Move> = LazyLock::new(|| Move {});

pub struct Skip {}

impl Action for Skip {
    fn name(&self) -> &str {
        "skip"
    }

    fn is_harmful(&self) -> bool {
        // Nobody is on the receiving end of this. `is_harmful` defaults
        // to true because most actions are attacks, and these four had
        // never said otherwise — which put them in every "walk the
        // actor's harmful actions" scan in the AI, to be rejected a
        // predicate later.
        false
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["s"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        Vec::new()
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(SkipTurn {})]
    }
}

pub static SKIP: LazyLock<Skip> = LazyLock::new(|| Skip {});

pub struct Dash {}

impl Action for Dash {
    fn name(&self) -> &str {
        "dash"
    }

    /// One of the three Actions SRD 5.2 names by hand in Haste's
    /// restricted list — the ones the default (`an attack, and only
    /// one`) cannot recognise because they are not attacks at all.
    fn hasted_action_eligible(&self) -> bool {
        true
    }


    fn is_harmful(&self) -> bool {
        // Nobody is on the receiving end of this. `is_harmful` defaults
        // to true because most actions are attacks, and these four had
        // never said otherwise — which put them in every "walk the
        // actor's harmful actions" scan in the AI, to be rejected a
        // predicate later.
        false
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["dsh"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        // `travel_speed`, not the actor's own: a Dash is a second
        // helping of whatever is carrying you, and for a mounted rider
        // that is the horse. See `engine::mounts::travel_speed`.
        let speed = encounter.travel_speed(caster_id);
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Movement(speed),
        })]
    }
}

pub static DASH: LazyLock<Dash> = LazyLock::new(|| Dash {});

/// Stand up from being prone. 5e: standing up costs half your speed in
/// movement. Only valid while the caster has the Prone condition.
///
/// The **Athlete** feat replaces the halving with a flat five feet —
/// *"When you have the Prone condition, you can right yourself with only
/// 5 feet of movement"* — which is the one clause in the game that
/// changes this price. See `ATHLETE_STAND_UP_FEET` and
/// `crate::actions::feats::ATHLETE_TAG`.
pub struct StandUp {}

/// What standing up costs a holder of the **Athlete** feat, in feet.
///
/// A constant rather than a literal in the `cost` body because it is
/// RAW's own number and not a tuning dial, and because the halving it
/// replaces is derived from the stander's speed while this is not: on a
/// 30-ft chassis it saves ten feet, on a 40-ft monk fifteen, and on a
/// creature slowed to 10 ft it saves nothing at all. Floored against the
/// ordinary price so the feat can never make standing up *more*
/// expensive for a creature whose speed has been cut below it.
pub const ATHLETE_STAND_UP_FEET: f32 = 5.0;

impl Action for StandUp {
    fn name(&self) -> &str {
        "stand"
    }

    fn is_harmful(&self) -> bool {
        // Nobody is on the receiving end of this. `is_harmful` defaults
        // to true because most actions are attacks, and these four had
        // never said otherwise — which put them in every "walk the
        // actor's harmful actions" scan in the AI, to be rejected a
        // predicate later.
        false
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["standup", "su"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        match encounter.actors.get(&caster_id) {
            Some(a) => {
                let ordinary = a.speed() / 2.0;
                // The Athlete feat's flat price, taken only where it is
                // actually cheaper: a creature whose speed has been cut
                // to 8 ft already stands for 4, and RAW's "only 5 feet"
                // is a discount rather than a floor to be raised to.
                let price = if a.has_passive_feature(crate::actions::feats::ATHLETE_TAG) {
                    ordinary.min(ATHLETE_STAND_UP_FEET)
                } else {
                    ordinary
                };
                vec![Resource::Movement(price)]
            }
            None => Vec::new(),
        }
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_condition(crate::conditions::Condition::Prone))
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::RemoveCondition {
            actor_id: caster_id,
            condition: crate::conditions::Condition::Prone,
        })]
    }
}

pub static STAND_UP: LazyLock<StandUp> = LazyLock::new(|| StandUp {});

/// Drop Prone — voluntarily lie down. 5e RAW: dropping prone costs no
/// movement and isn't an action — it's a free "part of movement." We
/// model it as a free no-cost action (empty `cost` vec) so the picker
/// can surface it without nudging the action economy. The reverse
/// move (standing back up) routes through `StandUp` and costs half
/// the actor's speed in movement per RAW.
///
/// Tactical use cases:
/// - Soak a ranged volley: prone grants ranged attackers disadvantage
///   (engine's `compute_attack_mode` reads the Prone condition).
/// - Pre-position for an Uncanny Dodge / Shield reaction trade.
/// - Bait melee attackers into closing (melee attacks against prone
///   targets have advantage RAW — symmetric trade with the ranged
///   disadvantage).
///
/// Validates: caster exists, is not already prone, is not unconscious
/// / petrified / paralyzed (those install Prone via their own state
/// machinery — dropping prone on top of an auto-prone condition is
/// a no-op refresh that the engine's `add_condition` would collapse,
/// but rejecting early keeps the picker UI clean).
pub struct DropProne {}

impl Action for DropProne {
    fn name(&self) -> &str {
        "drop prone"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["dp", "lay", "prone"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // RAW: dropping prone is free — no Action / Movement spend.
        Vec::new()
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        use crate::conditions::Condition;
        let Some(a) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if a.has_condition(Condition::Prone) {
            return false;
        }
        // Auto-prone conditions already install Prone; the picker
        // shouldn't surface a redundant "lay" line. `add_condition`
        // would no-op via the timer-longer collapse, but rejecting at
        // the validate gate keeps the action picker tidy.
        if a.has_condition(Condition::Unconscious)
            || a.has_condition(Condition::Petrified)
            || a.has_condition(Condition::Paralyzed)
            || a.has_condition(Condition::Asleep)
        {
            return false;
        }
        true
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::ApplyCondition {
            actor_id: caster_id,
            condition: crate::conditions::Condition::Prone,
            timer: crate::conditions::ConditionTimer::Permanent,
        })]
    }
}

pub static DROP_PRONE: LazyLock<DropProne> = LazyLock::new(|| DropProne {});

/// Dodge action — attacks against you have disadvantage and you have
/// advantage on DEX saves until the start of your next turn (5e). Costs
/// an Action; harmless flag-flip side effect.
pub struct Dodge {}

impl Action for Dodge {
    fn name(&self) -> &str {
        "dodge"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["dg"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::SetDodging {
            actor_id: caster_id,
            dodging: true,
        })]
    }
}

pub static DODGE: LazyLock<Dodge> = LazyLock::new(|| Dodge {});

/// Disengage action — your movement this turn doesn't provoke
/// opportunity attacks (5e). Costs an Action.
pub struct Disengage {}

impl Action for Disengage {
    fn name(&self) -> &str {
        "disengage"
    }

    /// One of the three Actions SRD 5.2 names by hand in Haste's
    /// restricted list — the ones the default (`an attack, and only
    /// one`) cannot recognise because they are not attacks at all.
    fn hasted_action_eligible(&self) -> bool {
        true
    }


    fn aliases(&self) -> Vec<&str> {
        vec!["de", "dis"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::SetDisengaging {
            actor_id: caster_id,
            disengaging: true,
        })]
    }
}

pub static DISENGAGE: LazyLock<Disengage> = LazyLock::new(|| Disengage {});

/// 5e **Ready** — spend your Action to hold an attack, and swing as a
/// reaction the moment an enemy walks into its reach.
///
/// The last of the PHB's core actions to exist here. Dodge, Disengage,
/// Help, Hide, Search, Grapple and Shove were all present; the one that
/// lets a creature *wait* was not, so a bow-armed defender covering a
/// doorway had nothing to do but fire at a wall or Dash into the open.
///
/// **What is narrowed, and why.** RAW readies any action against any
/// trigger the player can describe ("when the cultist finishes his
/// chant", "if anyone opens that door"). A trigger like that is a
/// sentence, and the prompt takes one action name per line — there is
/// nowhere to put it and nothing to parse it with. So this readies one
/// thing against one trigger: the holder's longest-reaching attack, and
/// "an enemy comes within its reach". That is the shape the action is
/// nearly always used in at a table, and the one the engine's existing
/// movement-trigger dispatcher can enforce exactly.
///
/// The reaction is *not* spent here — RAW spends it when the readied
/// action fires, and a readier whose trigger never comes keeps theirs.
/// What they lose is the Action, the same as at a table.
pub struct Ready {}

impl Action for Ready {
    fn name(&self) -> &str {
        "ready"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["rd", "hold"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        // Nothing to hold without an attack to hold, and nothing to
        // spend it with without a reaction — a creature that has
        // already reacted this round would be buying a promise it
        // cannot keep.
        encounter.best_readyable_attack(caster_id).is_some()
            && actor.can_consume_resource(Resource::Reaction)
            && !actor.has_condition(crate::conditions::Condition::Readied)
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::ApplyCondition {
            actor_id: caster_id,
            condition: crate::conditions::Condition::Readied,
            // RAW: "up to the start of your next turn". The engine's
            // start-of-turn sweep expires this timer, so a readier whose
            // trigger never came simply stops holding.
            timer: crate::conditions::ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static READY: LazyLock<Ready> = LazyLock::new(|| Ready {});

/// 5e Help action: target one ally; their next attack against a
/// designated foe before the start of *your* next turn has advantage.
/// We collapse the timing slightly: we record `Helped` against *any*
/// future attack, the helped actor consumes it on their next attack.
///
/// Parameterised over cost and reach because 5e writes the same grant
/// twice. The PHB's Help is an Action given to somebody you can touch;
/// the Mastermind Rogue's **Master of Tactics** (XGtE, subclass level 3)
/// is "you can use the Help action as a bonus action… to aid a friendly
/// creature attacking a creature within 30 feet of you", which is the
/// identical effect at a different price. Two statics off one impl
/// rather than a near-copy, so the "designated foe" heuristic and the
/// `HelpGrant` bookkeeping stay in one place.
pub struct Help {
    /// Log / lookup name, and what a controller types.
    name: &'static str,
    aliases: &'static [&'static str],
    /// `true` for the bonus-action variant (Master of Tactics).
    bonus_action: bool,
    /// Footprint-gap reach, in tiles.
    reach: isize,
}

impl Action for Help {
    fn name(&self) -> &str {
        self.name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(self.reach)
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        if self.bonus_action {
            crate::actions::action_template::bonus_action_only()
        } else {
            crate::actions::action_template::action_only()
        }
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Help requires an *ally* target — never self, never an enemy.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        if target_id == caster_id {
            return false;
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() == caster.team() && target.is_combat_active()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(ally_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Pick the closest hostile to the helped ally as the "designated
        // foe" for the grant. This mirrors RAW where the helper picks a
        // target; using closest enemy is a reasonable default. If there
        // is no enemy in view, we skip the grant and just leave the
        // Helped condition — the engine reads either lane to grant
        // advantage on the next swing.
        let designated = encounter.actors.get(&ally_id).and_then(|a| {
            let my_team = a.team();
            let my_loc = a.location();
            encounter
                .actors
                .iter()
                .filter(|(id, other)| {
                    **id != ally_id && other.team() != my_team && other.is_combat_active()
                })
                .min_by_key(|(_, other)| {
                    let dx = other.location().x - my_loc.x;
                    let dy = other.location().y - my_loc.y;
                    dx.unsigned_abs().max(dy.unsigned_abs())
                })
                .map(|(id, _)| *id)
        });
        if let Some(target) = encounter.actors.get_mut(&ally_id) {
            match designated {
                Some(against) => target.set_help_grant(Some(
                    crate::actors::actor_template::HelpGrant {
                        helper_id: caster_id,
                        against,
                    },
                )),
                None => target.set_help_grant(None),
            }
        }
        vec![Box::new(crate::engine::side_effects::ApplyCondition {
            actor_id: ally_id,
            condition: crate::conditions::Condition::Helped,
            timer: crate::conditions::ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static HELP: LazyLock<Help> = LazyLock::new(|| Help {
    name: "help",
    aliases: &["h", "assist"],
    bonus_action: false,
    reach: crate::actions::action_template::MELEE_REACH,
});

/// **Master of Tactics** — Mastermind Rogue subclass level 3 (XGtE).
/// The Help action, as a bonus action, at 30 ft.
///
/// Both halves matter and they matter together. The bonus action is what
/// makes it free: a rogue's Action is a Sneak Attack and its bonus
/// action is Cunning Action, and Cunning Action is the one a Mastermind
/// standing safely at range has least use for. The thirty feet is what
/// makes it a *rogue* feature rather than a second front-liner's — the
/// Mastermind hands the fighter advantage from wherever the rogue chose
/// to stand, which for a d8 chassis with no armour is a long way from
/// the fighter.
///
/// It is also, on this roster, the only repeatable advantage-granting
/// button that costs nothing and never runs out. The bard's Inspiration
/// is a pool of three, the Battle Master's Commander's Strike spends a
/// superiority die, and Help itself costs the helper their whole turn.
pub static MASTER_OF_TACTICS: LazyLock<Help> = LazyLock::new(|| Help {
    name: "master of tactics",
    aliases: &["mot", "tactics", "bonus-help"],
    bonus_action: true,
    // 30 ft = 12 tiles on the 2.5 ft grid.
    reach: 12,
});

/// 5e **Mount** (PHB p.198): "Once during your move, you can mount a
/// creature that is within 5 feet of you… the cost is movement equal to
/// half your speed."
///
/// Costs movement and nothing else — no action, no bonus action. That is
/// the whole shape of the rule, and it is what makes the mounted turn
/// worth taking: a knight who spends fifteen feet climbing into the
/// saddle still has their whole Attack action and the horse's remaining
/// forty-five feet to spend on it.
///
/// The gate lives in `EncounterInstance::can_mount`, which the validator
/// and the side effect both read, so the target picker offers exactly
/// the creatures that would actually accept a rider.
pub struct Mount {}

impl Action for Mount {
    fn name(&self) -> &str {
        "mount"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ride", "saddle"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        mount_toll(encounter, caster_id)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        first_target_id(target_ids)
            .is_some_and(|mount_id| encounter.can_mount(caster_id, mount_id).is_ok())
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(mount_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(crate::engine::side_effects::MountUp {
            rider_id: caster_id,
            mount_id,
        })]
    }
}

pub static MOUNT: LazyLock<Mount> = LazyLock::new(|| Mount {});

/// 5e **Dismount** (PHB p.198): the other half of the same sentence, at
/// the same price — half your speed, spent to get down.
///
/// `NoArgs`, because there is only one thing you can be sitting on. The
/// landing tile is the engine's to pick (`EncounterInstance::dismount`
/// takes the closest free space beside the mount), which is also why the
/// validator asks for one: a rider walled in on every side stays up, and
/// offering the action would charge them half their speed for nothing.
pub struct Dismount {}

impl Action for Dismount {
    fn name(&self) -> &str {
        "dismount"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["unmount", "getoff"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        mount_toll(encounter, caster_id)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter
            .actors
            .get(&caster_id)
            .and_then(|a| a.mounted_on())
            .is_some_and(|mount_id| {
                encounter
                    .find_adjacent_teleport_anchor(mount_id, caster_id)
                    .is_some()
            })
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::DismountFrom {
            rider_id: caster_id,
        })]
    }
}

pub static DISMOUNT: LazyLock<Dismount> = LazyLock::new(|| Dismount {});

/// The movement RAW charges to get on or off a mount — "half your
/// speed", both ways.
///
/// Shared by both actions rather than written twice because they are one
/// sentence in the book, and because a `Vec::new()` for a missing actor
/// is the wrong answer in a subtly expensive way: an empty cost is a
/// *free* action, so a rider the table has lost track of would be able
/// to mount and dismount without limit.  There is no such rider —
/// `caster_id` always resolves — and the fallback is the toll a 30-ft
/// creature would pay, so the impossible case is priced rather than
/// comped.
fn mount_toll(encounter: &EncounterInstance, caster_id: usize) -> Vec<Resource> {
    let feet = encounter
        .actors
        .get(&caster_id)
        .map_or(DEFAULT_MOUNT_TOLL_FEET, |a| a.mount_movement_cost());
    vec![Resource::Movement(feet)]
}

/// Half the speed of an ordinary 30-ft creature, rounded to the grid —
/// the toll `mount_toll` falls back to. See there.
const DEFAULT_MOUNT_TOLL_FEET: f32 = 15.0;

/// 5e Shove (special melee attack): contested Athletics check — attacker's
/// d20 + STR mod vs target's d20 + max(STR mod, DEX mod). On success the
/// target is knocked prone AND pushed 5 ft away from the attacker.
/// Target must be no more than one size category larger. Costs an Action.
pub struct Shove {}

impl Action for Shove {
    fn name(&self) -> &str {
        "shove"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sh", "push"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        // 5e: target must be no more than one size larger.
        caster.size().can_grapple(target.size())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        if !encounter.actors.contains_key(&target_id) {
            return Vec::new();
        }
        // 5e Shove: "a Strength (Athletics) check contested by the
        // target's Strength (Athletics) or Dexterity (Acrobatics) check."
        // Attacker wins ties. Both halves route through the engine's
        // contest chokepoint, so the skills the rule names are actually
        // rolled — with proficiency, condition modes, and one-shot
        // riders — instead of the bare `d20 + ability modifier` this
        // used to open-code.
        if !encounter.roll_contest(
            "shove",
            caster_id,
            ATHLETICS_CONTEST,
            target_id,
            GRAPPLE_DEFENSE_CONTEST,
        ) {
            encounter.log("  shove: target resists");
            return Vec::new();
        }
        encounter.log("  shove: target knocked prone and pushed");
        vec![
            Box::new(crate::engine::side_effects::ApplyCondition {
                actor_id: target_id,
                condition: crate::conditions::Condition::Prone,
                timer: crate::conditions::ConditionTimer::Permanent,
            }) as Box<dyn crate::engine::side_effects::ApplicableSideEffect>,
            Box::new(crate::engine::side_effects::PushActor {
                actor_id: target_id,
                from: caster_loc,
                max_tiles: crate::engine::util::tiles_from_feet(5),
            }),
        ]
    }
}

pub static SHOVE: LazyLock<Shove> = LazyLock::new(|| Shove {});

/// 5e Grapple (special melee attack): contested Athletics check — attacker's
/// d20 + STR mod vs target's d20 + max(STR mod, DEX mod). On success the
/// target gains the Grappled condition (speed = 0). Target must be no more
/// than one size category larger. Costs an Action.
pub struct Grapple {}

impl Action for Grapple {
    fn name(&self) -> &str {
        "grapple"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gr", "grab"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        // 5e: target must be no more than one size larger.
        caster.size().can_grapple(target.size())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if !encounter.actors.contains_key(&caster_id)
            || !encounter.actors.contains_key(&target_id)
        {
            return Vec::new();
        }
        // Same contest as Shove, same chokepoint — see the note there.
        if !encounter.roll_contest(
            "grapple",
            caster_id,
            ATHLETICS_CONTEST,
            target_id,
            GRAPPLE_DEFENSE_CONTEST,
        ) {
            encounter.log("  grapple: target slips free");
            return Vec::new();
        }
        encounter.log("  grapple: target is grappled");
        // Grappled until the grappler releases or is incapacitated. We
        // don't yet model release as an action — for now we use a long
        // Rounds timer (10 rounds = 1 minute) so it has a definite
        // expiration.
        //
        // The back-link is what makes the escape a contest rather than a
        // flat DC: `GrappleEscape` reads it to find whose Athletics the
        // captive is straining against, and `release_broken_grapples`
        // reads it to end the hold when RAW says it ends (the grappler
        // is incapacitated, or stops existing).
        crate::engine::side_effects::install_condition_with_link(
            crate::conditions::Condition::Grappled,
            target_id,
            caster_id,
            crate::conditions::ConditionTimer::Rounds(10),
        )
    }
}

pub static GRAPPLE: LazyLock<Grapple> = LazyLock::new(|| Grapple {});

/// 5e Grapple Escape — a held creature uses its Action to attempt to
/// break free: "a Strength (Athletics) or Dexterity (Acrobatics) check
/// contested by the grappler's Strength (Athletics) check."
///
/// Two shapes, picked by whether the hold names a grappler. A `Grappled`
/// installed by the Grapple action or by a creature's own grab — the
/// Roper's tendril — carries a back-link, so the escape is the RAW
/// contest against that creature. Everything else that pins a target —
/// Evard's Black Tentacles, Maximilian's Earthen Grasp, an ooze's
/// Adhered, an Energy Bow's arrow — installs the flag with nobody
/// straining at the other end of it, and those roll against a flat DC
/// instead: the shared unanchored number for the ones RAW prices off a
/// stat block the engine has not got, and RAW's own number where it
/// prints one.
///
/// Which holds those are is [`ESCAPABLE_HOLDS`], which is also where a
/// fifth one goes.
///
/// **One Action buys one check, and the check is compared against every
/// hold on the creature.** That is a widening of what the single roll
/// has always meant rather than a new rule: the list used to carry one
/// DC, so freeing all of them together was the same sentence. With a
/// row at DC 20 beside three at 13 it stops being the same sentence,
/// and per-hold is the reading that keeps both the cheap holds cheap
/// and the expensive one expensive — a captive who is both glued to an
/// ooze and pinned by an arrow gets out of the glue and stays pinned.
///
/// A successful escape also ends the *restraint the hold was imposing*,
/// where there is one. Several 5e holds word the restraint as a
/// consequence of the grapple rather than as a second effect, and the
/// engine tells those apart by the `Restrained` back-link naming the
/// same holder — see the removal branch below.
pub struct GrappleEscape {}

impl Action for GrappleEscape {
    fn name(&self) -> &str {
        "escape grapple"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["escape", "break free"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && ESCAPABLE_HOLDS
                    .iter()
                    .any(|hold| a.has_condition(hold.condition))
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::Condition;
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Snapshot which holds are active, and who each one names,
        // before we mutably borrow `encounter` for the roll and log
        // calls. `(hold, holder)` — the holder is `None` for a hold
        // that was installed with nobody on the other end of it.
        let active: Vec<(&'static EscapableHold, Option<usize>)> = ESCAPABLE_HOLDS
            .iter()
            .filter(|hold| actor.has_condition(hold.condition))
            .map(|hold| (hold, actor.linked_by(hold.condition)))
            .collect();
        // The restraint the holds may be imposing, and whose it is.
        // Read once here for the same borrow reason, and used below to
        // decide whether breaking a hold also ends it.
        let restraint_holder = actor.linked_by(Condition::Restrained);
        // SRD 5.2's "advantage on any ability check you make to end the
        // Grappled condition" clauses — the Goliath's Powerful Build
        // today. Scoped to *what the check is for* rather than to who is
        // rolling, so it cannot ride `compute_check_mode` and arrives
        // from here instead. See `ESCAPE_CHECK_ADVANTAGES`.
        let escape_mode = encounter.escape_check_mode(caster_id);
        // One Action, one heave: the captive's own check is rolled at
        // most once and every flat-DC hold is measured against that one
        // total. Rolled lazily so a creature held only by a linked
        // grapple — the common case — spends no die here at all and the
        // contest below is the only roll, which is what the log has
        // always shown.
        let mut flat_total: Option<i32> = None;
        let mut freed: Vec<(Condition, Option<usize>)> = Vec::new();
        for (hold, holder) in &active {
            // A linked `Grappled` names the creature holding on, and
            // that turns the escape into the contest RAW asks for. The
            // captive gets the choice of ability
            // (`GRAPPLE_DEFENSE_CONTEST` on the challenging side here —
            // the roles are reversed from Grapple's, because it is the
            // captive straining now).
            let broke = match holder {
                Some(holder_id)
                    if hold.contested && encounter.actors.contains_key(holder_id) =>
                {
                    encounter.roll_contest_with_challenger_mode(
                        "escape grapple",
                        caster_id,
                        GRAPPLE_DEFENSE_CONTEST,
                        escape_mode,
                        *holder_id,
                        ATHLETICS_CONTEST,
                    )
                }
                // Nobody straining at the other end — a spell, a monster
                // ability or an arrow installed the flag directly, or
                // the grappler is gone. Roll against the flat DC, still
                // as a real check so proficiency and roll mode apply.
                _ => {
                    let total = match flat_total {
                        Some(total) => total,
                        None => {
                            let (ability, skill) = encounter
                                .best_check_option(caster_id, GRAPPLE_DEFENSE_CONTEST)
                                .unwrap_or((
                                    crate::engine::types::AbilityScoreType::Strength,
                                    crate::engine::types::Skill::Athletics,
                                ));
                            let total = encounter.roll_ability_check_with_extra_mode(
                                caster_id,
                                ability,
                                Some(skill),
                                escape_mode,
                            );
                            flat_total = Some(total);
                            total
                        }
                    };
                    encounter
                        .log(format!("  escape grapple: {} vs DC {}", total, hold.dc));
                    total >= hold.dc
                }
            };
            if broke {
                freed.push((hold.condition, *holder));
            }
        }
        if freed.is_empty() {
            encounter.log("  failed to break free.".to_string());
            return Vec::new();
        }
        encounter.log("  broke free!".to_string());
        let mut effects: Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> =
            Vec::new();
        // …and the restraint those holds were imposing, if they were
        // imposing one. RAW words the clause as a consequence — "until
        // the grapple ends, the target is restrained", "have the
        // Restrained condition … until it breaks the restraint" — so
        // ending the hold has to end it, and a captive who won the
        // contest and stayed at zero movement would have gained nothing
        // from winning.
        //
        // Gated on the two links naming the *same* holder, which is the
        // whole reason `Restrained` carries one. A creature who breaks a
        // roper's tendril while also standing in somebody else's Web is
        // still in the web.
        let mut restraint_ends = false;
        for (condition, holder) in freed {
            effects.push(Box::new(crate::engine::side_effects::RemoveCondition {
                actor_id: caster_id,
                condition,
            }));
            restraint_ends |= holder.is_some() && holder == restraint_holder;
        }
        if restraint_ends {
            effects.push(Box::new(crate::engine::side_effects::RemoveCondition {
                actor_id: caster_id,
                condition: Condition::Restrained,
            }));
        }
        effects
    }
}

pub static GRAPPLE_ESCAPE: LazyLock<GrappleEscape> = LazyLock::new(|| GrappleEscape {});

/// **Hide** — SRD 5.2's action, in full:
///
/// > With the Hide action, you try to hide yourself. To do so, you must
/// > succeed on a DC 15 Dexterity (Stealth) check while you're Heavily
/// > Obscured or behind Three-Quarters Cover or Total Cover, and you
/// > must be out of any enemy's line of sight […] On a successful
/// > check, you have the Invisible condition while hidden. Make note of
/// > your check's total, which is the DC for a creature to find you
/// > with a Wisdom (Perception) check.
///
/// Three sentences, and the engine used to carry a different rule for
/// each of them. The check rolled against the best passive Perception
/// on the board rather than a flat 15; the *conditions* for attempting
/// it were "nothing hostile is standing next to you", which is not a
/// rule in any edition and let a creature vanish standing in an open,
/// brightly lit field; and Search looked for the hider against a
/// `12 + DEX` estimate rather than against the number the hider
/// actually rolled.
///
/// All three are the same rule now, and the reason they can be is that
/// the engine grew the layers RAW's paragraph is written about. Heavy
/// obscurement is the lighting layer and the zone layer between them
/// (`viewer_can_see` folds darkness, fog and walls); three-quarters
/// cover is `cover_ac_bonus`'s upper rung; total cover is a blocked
/// line of sight. See `can_attempt_hide` for how the two clauses
/// compose.
///
/// The one deliberate divergence is which condition lands.
/// RAW hands out `Invisible`; the engine installs `Condition::Hidden`,
/// which does the same two things on the d20 and differs in the one
/// place that matters: it burns off when the hider attacks, which is
/// RAW's *"you stop being hidden […] you make an attack roll"* arriving
/// as the condition's own clause rather than as a fourth rule
/// somewhere else. The other three ways RAW ends hiding — a sound
/// louder than a whisper, an enemy finding you, a verbal-component
/// spell — are respectively unmodelled, the Search action, and
/// unmodelled.
pub struct Hide {}

impl Action for Hide {
    fn name(&self) -> &str {
        "hide"
    }
    /// One of the three Actions SRD 5.2 names by hand in Haste's
    /// restricted list — the ones the default (`an attack, and only
    /// one`) cannot recognise because they are not attacks at all.
    fn hasted_action_eligible(&self) -> bool {
        true
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["hd"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        can_attempt_hide(encounter, caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        resolve_hide_attempt(encounter, caster_id)
    }
}

/// Can `caster_id` attempt to hide at all?
///
/// SRD 5.2's condition, asked of every enemy on the board: *"while
/// you're Heavily Obscured or behind Three-Quarters Cover or Total
/// Cover, and you must be out of any enemy's line of sight."*
///
/// RAW writes it as two clauses joined by "and", and on a battle map
/// they collapse into one question asked per watcher: **does this
/// enemy have a clear look at you?** An enemy answers no if it cannot
/// see the hider at all — `viewer_can_see` folds blindness, an unlit
/// tile the watcher has no darkvision for, a fog bank, and a wall,
/// which between them are every way RAW's first clause can be
/// satisfied — or if what it can see is a sliver behind three-quarters
/// cover. Every enemy has to answer no; one clear line of sight is
/// enough to spoil it, which is what "out of *any* enemy's line of
/// sight" says.
///
/// **This replaced "nothing hostile is standing next to you"**, which
/// was the one clause of RAW's paragraph a flat, fully-lit board could
/// enforce back when the engine had neither a lighting layer nor a
/// cover ladder. It has both, and the old gate's practical effect was
/// that anything could vanish in an open field at noon so long as it
/// had taken one step back first. The adjacency clause is gone rather
/// than kept alongside, because the new gate subsumes it: a creature
/// inside your reach is exempted from the cover ladder by
/// `cover_ac_bonus`'s own melee clause, so the only way to hide from
/// one is for it to be unable to see you at all — which is exactly
/// right, and is how you hide from something standing next to you in
/// pitch darkness.
///
/// A board with no enemies on it answers `true` vacuously, which is
/// the honest answer: there is nobody to hide from and nothing to fail.
///
/// Shared by all three printings of the action. `Hide` carried the old
/// check and the two bonus-action printings — the Rogue's Cunning Hide
/// and the Ranger's Vanish — carried none, so a rogue toe-to-toe with
/// an ogre could vanish from it as a bonus action while the ogre's own
/// player could not do it with a whole Action. The docstrings on those
/// printings say they are "the same effect at the cheaper cost", and
/// now they are.
pub fn can_attempt_hide(encounter: &EncounterInstance, caster_id: usize) -> bool {
    let Some(me) = encounter.actors.get(&caster_id) else {
        return false;
    };
    let my_team = me.team();
    encounter.actors.iter().all(|(id, other)| {
        if *id == caster_id || other.team() == my_team || !other.is_combat_active() {
            return true;
        }
        !encounter.viewer_can_see(*id, caster_id)
            || encounter.cover_ac_bonus(*id, caster_id)
                >= EncounterInstance::THREE_QUARTERS_COVER_AC
    })
}

/// SRD 5.2's flat Hide DC: *"you must succeed on a DC 15 Dexterity
/// (Stealth) check."*
///
/// A number rather than a contest, which is the 2024 rule and a real
/// change from the one the engine used to run: rolling against the
/// best passive Perception on the board made hiding harder in
/// proportion to how many creatures were looking, and RAW's contest
/// happens on the *other* side of it — a watcher spends its Search
/// action and rolls Perception against the number the hider actually
/// got. That number is `ActorInstance::hidden_check_total`, and the
/// Search action is where it is spent.
pub const HIDE_DC: i32 = 15;

/// Roll one Hide attempt and return what it installs — the `Hidden`
/// condition on a pass, and nothing at all on a fail.
///
/// 5e Hide is a Dexterity (Stealth) *check* against `HIDE_DC`. It used
/// to roll a Dexterity *save*, which is a different number on the same
/// die: the save lane collects save proficiency, Aura of Protection,
/// Bless, and the save-mode cohorts, and collects none of the Stealth
/// proficiency the action is named after. A rogue who is proficient in
/// Stealth got nothing for it — while `Search`, on the other side of
/// the same contest, was already adding that very proficiency into the
/// DC it compared against.
///
/// The successful total is recorded on the hider, because RAW says to:
/// *"make note of your check's total, which is the DC for a creature to
/// find you with a Wisdom (Perception) check."* A rogue who rolls a 27
/// is harder to find than one who scraped a 15, and before the number
/// was kept, both were found on the same `12 + DEX` estimate.
///
/// Shared by all three printings for the same reason `can_attempt_hide`
/// is: the bonus-action ones used to install `Hidden` outright, with no
/// roll and nothing to beat. A Stealth check the cheap printing never
/// makes is not a cheaper Hide, it is a better one, and the difference
/// was invisible because the two lived in different files.
pub fn resolve_hide_attempt(
    encounter: &mut EncounterInstance,
    caster_id: usize,
) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
    use crate::engine::types::{AbilityScoreType, Skill};
    let roll = encounter.roll_ability_check(
        caster_id,
        AbilityScoreType::Dexterity,
        Some(Skill::Stealth),
    );
    if roll < HIDE_DC {
        encounter.log(format!("  hide: stealth {} fails vs DC {}", roll, HIDE_DC));
        return Vec::new();
    }
    // Written eagerly rather than through a side-effect, for the same
    // reason a light source is: it is a number the *condition* is about,
    // and the condition install queued below is what the stack is for.
    if let Some(me) = encounter.actors.get_mut(&caster_id) {
        me.set_hidden_check_total(roll);
    }
    encounter.log(format!("  hide: succeeds (found only on a DC {})", roll));
    vec![Box::new(crate::engine::side_effects::ApplyCondition {
        actor_id: caster_id,
        condition: crate::conditions::Condition::Hidden,
        timer: crate::conditions::ConditionTimer::Permanent,
    })]
}

pub static HIDE: LazyLock<Hide> = LazyLock::new(|| Hide {});

/// 5e Search action — Wisdom (Perception) check against what a hidden
/// enemy actually rolled to hide. Costs an Action. On success, every
/// enemy within the searcher's sight range whose number the Perception
/// check beats loses their Hidden / Invisible cover.
///
/// The DC is SRD 5.2's: *"make note of your check's total, which is the
/// DC for a creature to find you with a Wisdom (Perception) check"* —
/// read off `ActorInstance::hidden_check_total`, which the Hide action
/// writes. The old `12 + DEX modifier` estimate survives as the
/// fallback for the two cases with no Stealth roll behind them: a
/// creature holding `Hidden` because something installed it directly,
/// and the `Invisible` branch, which is a spell rather than a check.
///
/// Range is bounded by a footprint-Chebyshev gap of 12 (60 ft) — a
/// reasonable in-combat "scan the room" envelope. The Perception check
/// itself routes through `roll_ability_check` so racial / passive
/// bonuses (Keen Senses, etc.) stack on top cleanly.
pub struct Search {}

impl Action for Search {
    fn name(&self) -> &str {
        "search"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sr", "scan", "look"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::Condition;
        use crate::engine::types::{AbilityScoreType, Skill};
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
        const SEARCH_RANGE: isize = 12;
        // The floor is looked at from much closer than a room is
        // scanned. RAW's trap entries say "examine the trapped area" and
        // "a creature within 5 feet of the statue"; four tiles is ten
        // feet on the 2.5-ft grid, which is the tile you are standing on
        // and the ring around it — the ground a creature could actually
        // crouch down and read.
        //
        // Deliberately much shorter than `SEARCH_RANGE`. A search that
        // found every pressure plate in the room from thirty feet away
        // would make the Action the answer to the whole trap layer, and
        // there would be no reason ever to walk anywhere without
        // spending it first.
        const TRAP_SEARCH_RANGE: isize = 4;

        let Some(searcher) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let searcher_team = searcher.team();
        let searcher_loc = searcher.location();
        let searcher_size = get_tiles_from_size(searcher.size());
        // Snapshot blinded — auto-fails the sight-based search.
        let blinded = searcher.has_condition(Condition::Blinded);
        if blinded {
            encounter.log("  search: blinded, fails to spot anything.".to_string());
            return Vec::new();
        }

        // Roll Perception once — the same roll compares against every
        // hidden enemy's Stealth DC. Mirrors 5e Perception scan
        // semantics. `roll_ability_check` logs the roll and its
        // breakdown itself, so there is no second line to print here.
        let perception = encounter.roll_ability_check(
            caster_id,
            AbilityScoreType::Wisdom,
            Some(Skill::Perception),
        );

        // Walk the actor table; for any hidden / invisible enemy in range
        // with LOS, compare the searcher's roll to the target's stealth
        // DC. On a beat, reveal them.
        let candidates: Vec<usize> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if *id == caster_id || a.team() == searcher_team || !a.is_combat_active() {
                    return None;
                }
                if !a.has_condition(Condition::Hidden) && !a.has_condition(Condition::Invisible) {
                    return None;
                }
                let dist = footprint_chebyshev(
                    searcher_loc,
                    searcher_size,
                    a.location(),
                    get_tiles_from_size(a.size()),
                );
                if dist > SEARCH_RANGE {
                    return None;
                }
                Some(*id)
            })
            .collect();

        let mut revealed = 0usize;
        let mut effects: Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> =
            Vec::new();
        for target_id in candidates {
            if !encounter.actor_has_line_of_sight(caster_id, target_id) {
                continue;
            }
            let target = match encounter.actors.get(&target_id) {
                Some(a) => a,
                None => continue,
            };
            // SRD 5.2: the DC to find a hider is *"your check's
            // total"* — the number they actually rolled. See
            // `ActorInstance::hidden_find_dc`, which also carries the
            // estimate used for a creature that is hiding without
            // having rolled and for the `Invisible` branch, which has
            // no Stealth check behind it at all.
            let dc = target.hidden_find_dc();
            // 5e's lightly-obscured tax, converted to a number the same
            // way RAW converts it for a passive check: a creature
            // standing in dim light is five points harder to find. Read
            // per target rather than folded into the roll above,
            // because one Perception check is being compared against
            // several hiders and only some of them are in the gloom.
            let dim = encounter.dim_light_search_penalty(caster_id, target_id);
            if perception - dim < dc {
                continue;
            }
            let target_name = target.name().to_string();
            if target.has_condition(Condition::Hidden) {
                encounter.log(format!(
                    "  search: spotted {} (DC {}) — hidden status broken.",
                    target_name, dc
                ));
                effects.push(Box::new(
                    crate::engine::side_effects::RemoveCondition {
                        actor_id: target_id,
                        condition: Condition::Hidden,
                    },
                ));
                revealed += 1;
            } else if target.has_condition(Condition::Invisible) {
                // 5e: Search reveals an invisible creature's *location*; we
                // model the location-reveal by tagging Outlined for a
                // round. The Invisible condition itself stays (since
                // becoming visible would dispel the spell), but Outlined
                // grants attack advantage to allies for the round — and,
                // since `Condition::suppresses_invisibility` arrived,
                // takes the concealment's disadvantage off the same
                // swings. A pinpointed target is pinpointed: the mark
                // no longer merely cancels out against the invisibility
                // it was painted over.
                encounter.log(format!(
                    "  search: pinpoint {}'s invisible location (DC {}).",
                    target_name, dc
                ));
                effects.push(Box::new(crate::engine::side_effects::ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::Outlined,
                    timer: crate::conditions::ConditionTimer::UntilStartOfNextTurn,
                }));
                revealed += 1;
            }
        }
        // The floor, after the room. A concealed area — a set glyph, a
        // dungeon trap — is found by the same act of looking and the
        // same roll: RAW asks for one check against the thing's own DC,
        // and the searcher who rolled a 19 rolled it once.
        //
        // Resolved here rather than as a queued effect because
        // `reveal_zone` is a change to the board rather than to an
        // actor, the same reason `add_light_source` is called inline by
        // the spells that light one.
        for (zone_id, dc) in encounter.concealed_zones_near(caster_id, TRAP_SEARCH_RANGE) {
            if perception < dc {
                continue;
            }
            if encounter.reveal_zone(zone_id) {
                revealed += 1;
            }
        }
        if revealed == 0 {
            encounter.log("  search: nothing new spotted.".to_string());
        }
        effects
    }
}

pub static SEARCH: LazyLock<Search> = LazyLock::new(|| Search {});

/// SRD 5.2 **Study** action, narrowed to the one thing on this board
/// worth studying: an image that isn't there.
///
/// RAW's Study is the broad "make an Intelligence check to recall or
/// work something out" action, and most of what it covers — a book, a
/// memory, a clue — is outside a fight. What is inside one is the
/// disbelief clause every image spell prints: *"a creature that uses
/// its Study action to examine the image can determine that it is an
/// illusion with a successful Intelligence (Investigation) check
/// against your spell save DC."* That is the whole of what this action
/// does, and it is the counterplay the illusion layer is worth nothing
/// without — see [`crate::engine::illusions`].
///
/// Deliberately the sibling of `Search` rather than a variant of it,
/// and the two are not interchangeable. Search is a Wisdom (Perception)
/// check that finds things which are *there* and hiding; this is an
/// Intelligence (Investigation) check that unfinds things which are
/// not. A creature good at one is routinely bad at the other, which is
/// exactly the choice RAW means a player to be making when a screen
/// goes up across the corridor.
///
/// Refuses to be spent on a board with nothing to study, which is what
/// keeps a whole Action from disappearing into a shrug — see
/// `EncounterInstance::studyable_illusions` for the candidate gate
/// (range, line of sight, and whether this creature is fooled at all).
pub struct Study {}

impl Action for Study {
    fn name(&self) -> &str {
        "study"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["st", "examine", "disbelieve"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        !encounter.studyable_illusions(caster_id).is_empty()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        // Resolved inline rather than as a queued effect, for the
        // reason `Search`'s zone reveal is: seeing through an image is
        // a change to a board layer rather than to an actor, and the
        // side-effect stack is the actors' queue.
        if encounter.study_illusions(caster_id) == 0 {
            encounter.log("  study: whatever is over there looks solid enough.".to_string());
        }
        Vec::new()
    }
}

pub static STUDY: LazyLock<Study> = LazyLock::new(|| Study {});

/// Wipe Acid — universal cleanse action. Spends an Action to scrape off
/// the lingering acid from 5e Tasha's Caustic Brew (the `CausticBrewed`
/// condition). RAW: "as an action, a creature can use a wet rag or a
/// similar absorbent item to wipe off the acid, ending the effect."
/// Self-target only — we surface the action universally rather than as
/// a class feature so any splashed creature can scrape off their own
/// drip, mirroring how `StandUp` lives on the default action list.
/// Custom-validates the condition presence so a misclick doesn't burn
/// the Action lane on a no-op.
pub struct WipeAcid {}

impl Action for WipeAcid {
    fn name(&self) -> &str {
        "wipe acid"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wa", "wipe", "scrape"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && a.has_condition(crate::conditions::Condition::CausticBrewed)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        encounter.log("  wipe acid: the lingering brew is scraped clean.".to_string());
        vec![Box::new(RemoveCondition {
            actor_id: caster_id,
            condition: crate::conditions::Condition::CausticBrewed,
        })]
    }
}

pub static WIPE_ACID: LazyLock<WipeAcid> = LazyLock::new(|| WipeAcid {});

/// **Drop and Roll** — the escape clause SRD 5.2 prints inside the
/// Burning hazard itself, and the half the engine never had.
///
/// > A burning creature or object takes 1d4 Fire damage at the start of
/// > each of its turns. **As an action, you can extinguish fire on
/// > yourself by giving yourself the Prone condition and rolling on the
/// > ground.** The fire also goes out if it is doused, submerged, or
/// > suffocated.
///
/// The engine had the first sentence — `Condition::Burning` is a row on
/// `ROUND_END_DOTS` — and neither of the other two. So a creature
/// caught by a Searing Smite or a Fire Bolt's rider burned for the full
/// timer with nothing at all it could do about it, which is not a
/// hazard, it is a countdown. Both halves land here: this action is the
/// deliberate one, and
/// `EncounterInstance::douse_burning_in_the_water` is "submerged".
///
/// **The Prone is the price, and it is the whole of the decision.** RAW
/// does not offer a save or a check; it offers a trade — an Action and
/// your feet against 1d4 a round — and the trade is a bad one for a
/// creature at full health and a good one for a creature two rounds
/// from dying with a caster still concentrating on the flames. Modeled
/// exactly as written: no roll, an unconditional Prone, and the fire
/// out.
///
/// Universal rather than a class feature, for `WipeAcid`'s reason
/// directly above and `StandUp`'s before it: anything that can catch
/// fire can put itself out, and the validator is the gate.
///
/// One divergence from the letter, and it is in the engine's favour: a
/// creature that is already Prone still pays the Action. RAW's sentence
/// is "by giving yourself the Prone condition", which a creature
/// already on the ground has already done — but rolling on the ground
/// is the thing being paid for, and the alternative reading makes the
/// fire free to put out for anyone who fell over first.
pub struct DropAndRoll {}

impl Action for DropAndRoll {
    fn name(&self) -> &str {
        "drop and roll"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["roll", "smother", "put out"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        use crate::conditions::Condition;
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.has_condition(Condition::Burning))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::Condition;
        use crate::engine::side_effects::{ApplyCondition, RemoveCondition};
        encounter.log(format!(
            "  drop and roll: {} beats out the flames.",
            encounter.actor_name(caster_id)
        ));
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Prone,
                timer: crate::conditions::ConditionTimer::Permanent,
            }),
            Box::new(RemoveCondition {
                actor_id: caster_id,
                condition: Condition::Burning,
            }),
        ]
    }
}

pub static DROP_AND_ROLL: LazyLock<DropAndRoll> = LazyLock::new(|| DropAndRoll {});

/// 5e's attach clause, the attacher's half: "the stirge can detach
/// itself by spending 5 feet of its movement."
///
/// `NoArgs`, because there is only one thing you can be wrapped around,
/// and free, because the sentence before it set the creature's Speed to
/// 0 — see `crate::engine::attachment`'s "what is deliberately not
/// modeled" for the whole of that argument. The landing tile is the
/// engine's to pick, which is why the validator asks for one: a
/// darkmantle wrapped around somebody in a sealed corridor stays on
/// rather than spending a turn failing to get off.
///
/// Lives on the default list rather than on the three stat blocks that
/// can use it, for the same reason `StandUp` does: the validator is the
/// gate, and three copies of one sentence is three places for it to
/// drift.
pub struct Release {}

impl Action for Release {
    fn name(&self) -> &str {
        "release"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["letgo", "unlatch"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        Vec::new()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter
            .attached_host(caster_id)
            .is_some_and(|host_id| {
                encounter
                    .find_adjacent_teleport_anchor(host_id, caster_id)
                    .is_some()
            })
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::ReleaseAttachment {
            attacher_id: caster_id,
        })]
    }
}

pub static RELEASE: LazyLock<Release> = LazyLock::new(|| Release {});

/// 5e's attach clause, everybody else's half: "the target or a creature
/// within 5 feet of it can take an action to try to detach the cloaker,
/// doing so by succeeding on a DC 14 Strength (Athletics) check."
///
/// Targets the *passenger*, not the person wearing it, which is the
/// reading that makes the two halves of RAW's sentence one action: a
/// victim prying the thing off its own face and an ally pulling it off
/// theirs are the same swing at the same creature, and the only
/// difference is where the prier is standing. Both are covered by the
/// ordinary melee reach check, because an attached creature's location
/// is mirrored onto its host's — so the host is at gap 0 from it and a
/// neighbour is at gap 1.
///
/// Harmful, so the AI's hostile-action scan finds it and so the Charmed
/// gate keeps a charmed victim from peeling their charmer's pet off.
/// Deals no damage, so the attack-ranking lanes leave it alone: this
/// action is chosen by `ai::simple::try_pry_attachment`, which knows
/// what it is worth, rather than by the damage estimator, which would
/// price it at zero.
pub struct PryLoose {}

impl Action for PryLoose {
    fn name(&self) -> &str {
        "pry loose"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pry", "peel", "detach"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        if target_id == caster_id {
            return false;
        }
        if !encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active())
        {
            return false;
        }
        // RAW's "the target or a creature within 5 feet of it": the
        // prier has to be the host or standing beside them. The reach
        // check in `Action::validate` measures to the *passenger*,
        // whose location is the host's — so it answers both cases at
        // once and this only has to confirm there is a latch to pull.
        encounter.attached_host(target_id).is_some()
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(attacher_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(crate::engine::side_effects::PryAttachment {
            prier_id: caster_id,
            attacher_id,
        })]
    }
}

pub static PRY_LOOSE: LazyLock<PryLoose> = LazyLock::new(|| PryLoose {});

/// **Burrow** — dig into the ground, and out of the fight.
///
/// Offered only to the ten stat blocks that print a burrow speed,
/// and only while they are standing on earth with room to go down. See
/// `crate::engine::burrowing` for what being under the floor costs and
/// buys, and for why the price is half a move rather than nothing.
///
/// Priced in movement rather than in an action, which is the choice
/// that makes the lane worth having: a bulette that spent its Action
/// digging in would be a bulette that never bit anything, and RAW is
/// unambiguous that burrowing is movement. Half a move is what the
/// engine charges for the other posture change it prices this way —
/// see `StandUp` — and it leaves a creature that dives in able to
/// tunnel the rest of its round.
pub struct Burrow {}

impl Action for Burrow {
    /// Only the ten stat blocks that print a burrow speed, which is
    /// not a thing that appears mid-fight. See `Action::possible_for`.
    fn possible_for(&self, actor: &crate::actors::actor_template::ActorInstance) -> bool {
        actor.can_burrow()
    }

    fn name(&self) -> &str {
        "burrow"
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["dig", "submerge"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        match encounter.actors.get(&caster_id) {
            // The *walking* speed, because that is what `speed()`
            // reports for a creature still on the surface — which is
            // the only creature this action is ever offered to. Digging
            // out costs half the burrowing speed instead, and the
            // asymmetry is the right way round: coming up through ten
            // feet of earth is the slow half.
            Some(a) => vec![Resource::Movement(
                a.speed() * crate::engine::burrowing::BURROW_TRANSIT_FRACTION,
            )],
            None => Vec::new(),
        }
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.can_submerge(caster_id)
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::Submerge {
            actor_id: caster_id,
        })]
    }
}

pub static BURROW: LazyLock<Burrow> = LazyLock::new(|| Burrow {});

/// **Surface** — come back up. The inverse of `Burrow`, and the only
/// way out of `Condition::Burrowed`.
///
/// Priced off the speed the actor has *right now*, which underground is
/// the burrow speed — so an ankheg pays five feet to come up and
/// fifteen to go down, and a bulette pays twenty either way.
pub struct Surface {}

impl Action for Surface {
    /// The inverse of `Burrow`'s row and gated on the same thing rather
    /// than on being underground: a burrower that is on the surface
    /// should see the pair, greyed out, the way `Stand` and `Drop Prone`
    /// sit beside each other.
    fn possible_for(&self, actor: &crate::actors::actor_template::ActorInstance) -> bool {
        actor.can_burrow()
    }

    fn name(&self) -> &str {
        "surface"
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["emerge", "erupt"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        match encounter.actors.get(&caster_id) {
            Some(a) => vec![Resource::Movement(
                a.speed() * crate::engine::burrowing::BURROW_TRANSIT_FRACTION,
            )],
            None => Vec::new(),
        }
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.is_burrowed(caster_id)
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::Surface {
            actor_id: caster_id,
        })]
    }
}

pub static SURFACE: LazyLock<Surface> = LazyLock::new(|| Surface {});

/// **Sunder** — SRD 5.2 *Breaking Objects*, swung by hand.
///
/// > Objects can be harmed by attacks and by some spells … **Armor
/// > Class.** The Object Armor Class table suggests ACs for various
/// > substances. **Hit Points.** An object is destroyed when it has 0
/// > Hit Points.
///
/// One Action, one adjacent tile of something breakable, one swing of
/// whatever the swinger is already holding. What it is for is the
/// party that meets a Wall of Stone and has nothing that explodes: the
/// spell half of this rule (`EncounterInstance::damage_objects_in_area`)
/// belongs to casters, and without this one a fighter's answer to a
/// wall is to walk around it or to stand there.
///
/// ## Why a separate action rather than the attack lane
///
/// Because the attack lane is built out of target *ids*. Every step of
/// it — the roll-mode tally, cover, the resistance table, the damage
/// payload, the drop-to-zero cohort — reads an `ActorInstance`, and a
/// wall of ice does not have one. Giving it one would mean a creature
/// on the board with no turn, no team and no initiative slot, which is
/// a much larger change than this and would have to be excluded by hand
/// from every sweep in the engine.
///
/// So the swing is taken apart instead: the weapon is asked what it
/// would have rolled ([`Action::melee_swing_profile`]), the roll is made
/// against the object's own AC, and the damage goes to
/// `EncounterInstance::damage_object_at`, which owns the object side of
/// the pipeline the way `DealDamage` owns the creature side.
///
/// ## What it swings
///
/// The best melee weapon in the swinger's repertoire, by the average
/// the profile itself describes — so ranking and resolution cannot
/// disagree, which is the failure `weapon_expected_damage_named`'s
/// docstring describes from the other side. A creature whose whole kit
/// is a save-or-suck (a gelatinous cube's engulf, a roper's tendril)
/// has no profile to offer and cannot sunder, which is the right
/// answer: half of what those attacks do is a rule about a creature.
///
/// ## What is not modelled
///
/// **A critical hit.** RAW allows one against an object and the engine
/// does not roll for it here, because the crit lane is
/// `engine::criticals` and every entry point it has takes an
/// `AttackParams`. The cost is a small underestimate of how fast a
/// wall comes down, in the same direction as every other simplification
/// on this action.
pub struct Sunder {}

impl Sunder {
    /// The swinger's best melee swing, and what it is worth on average.
    ///
    /// `attack_repertoire` rather than `available_actions`, for the
    /// reason `best_melee_damage_if_closed` gives: the question is
    /// "what could I *swing*", which is the stat block plus the one
    /// thing the loot table hands out that is a swing.
    fn best_swing(
        encounter: &EncounterInstance,
        actor_id: usize,
    ) -> Option<crate::actions::action_template::MeleeSwingProfile> {
        let actor = encounter.actors.get(&actor_id)?;
        let mut best: Option<(f32, crate::actions::action_template::MeleeSwingProfile)> = None;
        for action in actor.attack_repertoire() {
            let Some(profile) = action.melee_swing_profile() else {
                continue;
            };
            let worth = profile.dice.average_roll()
                + profile.flat_bonus as f32
                + profile
                    .damage_ability
                    .map(|a| actor.ability_modifier(a) as f32)
                    .unwrap_or(0.0);
            if best.as_ref().is_none_or(|(b, _)| worth > *b) {
                best = Some((worth, profile));
            }
        }
        best.map(|(_, p)| p)
    }
}

impl Action for Sunder {
    fn name(&self) -> &str {
        "sunder"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["smash", "breach"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // A tile, not a creature — which is the whole reason this action
        // exists. See the type docs.
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn deals_damage(&self) -> bool {
        // No creature loses hit points, which is what this answer is
        // about: the AI's focus-fire pipeline prices actions by what
        // they take off an enemy, and a wall is not an enemy.
        false
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        crate::actions::action_template::action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(point) = first_target_location(target_locations) else {
            return false;
        };
        encounter.breakable_at(point).is_some()
            && Self::best_swing(encounter, caster_id).is_some()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some((_, profile)) = encounter.breakable_at(point) else {
            return Vec::new();
        };
        let Some(swing) = Self::best_swing(encounter, caster_id) else {
            return Vec::new();
        };
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let name = actor.name().to_string();
        let ability_to_hit = actor.spell_attack_modifier(swing.attack_ability);
        let damage_bonus = swing
            .damage_ability
            .map(|a| actor.ability_modifier(a))
            .unwrap_or(0)
            + swing.flat_bonus
            + actor.item_damage_bonus()
            + actor.damage_bonus_buff();
        // Whatever the pack and the party's buffs are worth to a swing.
        // `caster_attack_buffs` with `is_spell: false` is the same term
        // the attack pipeline folds in for an ordinary hit — a fighter
        // with a `+3` sword and a Bless on them should chop at a wall
        // the way they chop at anything else, and a Sunder that read
        // only the ability modifier would be the one swing in the
        // engine their gear did not reach.
        let (flat_buff, condition_buff) = encounter.caster_attack_buffs(caster_id, false);
        let to_hit = ability_to_hit + flat_buff + condition_buff;
        // A plain d20 against a number, with no roll-mode tally: an
        // object is not Prone, not Invisible, not flanked and not
        // dodging, so every input the shared tally reads is absent by
        // construction. See the type docs for the one thing that *is*
        // absent and should not be — the crit.
        let roll = encounter.roll(&crate::engine::dice::Dice::new(1, 20)) as i32;
        let total = roll + to_hit;
        if total < profile.ac as i32 {
            encounter.log(format!(
                "  {} swings at the {} and misses (d20 {} {:+} = {} vs AC {}).",
                name, profile.label, roll, to_hit, total, profile.ac
            ));
            return Vec::new();
        }
        let rolled = encounter.roll(&swing.dice) as i32;
        let amount = (rolled + damage_bonus).max(0) as u32;
        encounter.log(format!(
            "  {} strikes the {} (d20 {} {:+} = {} vs AC {}).",
            name, profile.label, roll, to_hit, total, profile.ac
        ));
        vec![Box::new(crate::engine::side_effects::DamageObjectAt {
            coord: point,
            amount,
            damage_type: swing.damage_type,
        })]
    }
}

pub static SUNDER: LazyLock<Sunder> = LazyLock::new(|| Sunder {});

/// **Cut Free** — SRD 5.2 *Breaking Objects*, swung at something stuck
/// to a friend.
///
/// The Giant Spider's web is *"AC 10; HP 5; Vulnerability to Fire
/// damage"* and holds its victim *"until the web is destroyed"*. There
/// is no escape check in that sentence — no Athletics, no repeated save
/// — so without an action that attacks the web, a webbed creature stays
/// webbed for the rest of the fight and RAW's five hit points are a
/// number nobody can spend.
///
/// [`Sunder`]'s sibling, and separate from it for the reason that
/// action's own docstring gives from the other side: Sunder's schema is
/// `SinglePoint` because a wall of ice has no actor id, and a web has
/// nothing *but* an actor id. One of the two has to take a creature and
/// one has to take a tile, and an action cannot do both.
///
/// Everything else is shared. The same [`Sunder::best_swing`] picks the
/// weapon, the same buffs and item bonuses fold into the same numbers,
/// the same plain d20 goes against the object's armour class with no
/// roll-mode tally — a web is not Prone or Invisible or dodging either
/// — and the damage lands through `EncounterInstance::damage_web_on`,
/// which shares `ObjectProfile::effective_damage` with the walls. A
/// torch-wielding fighter doubles their die against silk for the same
/// line of code that doubles a Fireball against ice.
///
/// **The victim may cut themselves free**, and there is no clause
/// stopping them: `Restrained` costs its holder their movement and
/// taxes their attack rolls, and takes nothing off their Action. That
/// reads correctly — RAW's web is an object in reach of the creature
/// wearing it — and it is the difference between a hold that ends a
/// character and one that costs them a turn.
pub struct CutFree {}

impl Action for CutFree {
    fn name(&self) -> &str {
        "cut free"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cut", "free", "burn web"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // A creature, not a tile — the whole reason this is not Sunder.
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn deals_damage(&self) -> bool {
        // The web loses hit points and the creature wearing it does not,
        // which is what this answer is about: the AI prices actions by
        // what they take off a target, and the target here keeps every
        // point they have.
        false
    }
    fn is_harmful(&self) -> bool {
        // Aimed at a friend, almost always. Answering `true` would put
        // this on every hostile-action scan in the AI and would break
        // the swinger's own Sanctuary for cutting an ally loose.
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        crate::actions::action_template::action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter.web_on(target_id).is_some()
            && Sunder::best_swing(encounter, caster_id).is_some()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if encounter.web_on(target_id).is_none() {
            return Vec::new();
        }
        let Some(swing) = Sunder::best_swing(encounter, caster_id) else {
            return Vec::new();
        };
        let profile = &crate::engine::objects::SPIDER_WEB_PROFILE;
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let name = actor.name().to_string();
        let ability_to_hit = actor.spell_attack_modifier(swing.attack_ability);
        let damage_bonus = swing
            .damage_ability
            .map(|a| actor.ability_modifier(a))
            .unwrap_or(0)
            + swing.flat_bonus
            + actor.item_damage_bonus()
            + actor.damage_bonus_buff();
        let (flat_buff, condition_buff) = encounter.caster_attack_buffs(caster_id, false);
        let to_hit = ability_to_hit + flat_buff + condition_buff;
        let roll = encounter.roll(&crate::engine::dice::Dice::new(1, 20)) as i32;
        let total = roll + to_hit;
        let victim = encounter.actor_name(target_id);
        if total < profile.ac as i32 {
            encounter.log(format!(
                "  {} cuts at the web on {} and misses (d20 {} {:+} = {} vs AC {}).",
                name, victim, roll, to_hit, total, profile.ac
            ));
            return Vec::new();
        }
        let rolled = encounter.roll(&swing.dice) as i32;
        let amount = (rolled + damage_bonus).max(0) as u32;
        encounter.log(format!(
            "  {} cuts the web on {} (d20 {} {:+} = {} vs AC {}).",
            name, victim, roll, to_hit, total, profile.ac
        ));
        vec![Box::new(crate::engine::side_effects::DamageWebOn {
            actor_id: target_id,
            amount,
            damage_type: swing.damage_type,
        })]
    }
}

pub static CUT_FREE: LazyLock<CutFree> = LazyLock::new(|| CutFree {});

/// **First Aid** — the other half of SRD 5.2's *Knocking Out a
/// Creature*.
///
/// > The creature remains Unconscious until it regains any Hit Points
/// > or until someone uses an action to administer first aid to it,
/// > which requires a successful DC 10 Wisdom (Medicine) check.
///
/// The knockout is a choice somebody made about a creature they could
/// have killed, and this is the sentence that makes it a *choice* — a
/// captive who cannot be woken is a corpse that takes up a tile. It
/// also gives the party's medic something to do with a turn on the
/// round after a fight has been decided, which nothing else in the
/// engine does.
///
/// **Not a heal**, which is the difference from every other lane that
/// touches an Unconscious creature. RAW hands back consciousness and no
/// hit points: whoever is woken this way stands up on the one hit point
/// the pulled punch left them, which is a real decision for whoever is
/// waking them.
///
/// **Not `stabilize` either.** The engine's `ActorInstance::stabilize`
/// is the *Dying* lane's — a creature at 0 hit points that stops
/// rolling death saves — and a knocked-out creature is neither at 0 nor
/// dying. Two sentences in the book, two lanes here.
///
/// `WIS` and `Skill::Medicine`, which is RAW and is why the cleric's
/// acolyte and the priest carry the proficiency: a chassis that trained
/// for this rolls it better.
pub struct FirstAid {}

impl FirstAid {
    /// RAW's DC, flat. Nothing scales it and nobody sets it — the same
    /// shape as the slippery-ice save, and named here for the same
    /// reason: a number quoted in a docstring and compared against in a
    /// body is a number that drifts.
    pub const DC: i32 = 10;
}

impl Action for FirstAid {
    fn name(&self) -> &str {
        "first aid"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["aid", "wake", "revive"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        crate::actions::action_template::action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        if target_id == caster_id {
            return false;
        }
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        // Unconscious *and still standing on its own hit points*, which
        // is the state a pulled punch leaves and the state RAW's
        // sentence is about. A creature at 0 is Dying, and what that one
        // wants is a heal or a stabilize — two different sentences, two
        // different lanes.
        target.has_condition(crate::conditions::Condition::Unconscious)
            && target.hitpoints() > 0
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::engine::types::{AbilityScoreType, Skill};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let total = encounter.roll_ability_check(
            caster_id,
            AbilityScoreType::Wisdom,
            Some(Skill::Medicine),
        );
        let (medic, patient) = (encounter.actor_name(caster_id), encounter.actor_name(target_id));
        if total < Self::DC {
            encounter.log(format!(
                "  first aid: {} cannot rouse {} ({} vs DC {}).",
                medic, patient, total, Self::DC
            ));
            return Vec::new();
        }
        encounter.log(format!(
            "  first aid: {} brings {} round ({} vs DC {}).",
            medic, patient, total, Self::DC
        ));
        // Consciousness and nothing else — RAW hands back no hit points.
        // The Prone that came with the knockout stays, because standing
        // up is the patient's own action and the book never says
        // otherwise.
        vec![Box::new(crate::engine::side_effects::RemoveCondition {
            actor_id: target_id,
            condition: crate::conditions::Condition::Unconscious,
        })]
    }
}

pub static FIRST_AID: LazyLock<FirstAid> = LazyLock::new(|| FirstAid {});

pub static DEFAULT_ACTIONS: LazyLock<Vec<&'static (dyn Action + Send + Sync)>> = LazyLock::new(
    || {
        vec![
            &*MOVE,
            &*DASH,
            &*SKIP,
            &*STAND_UP,
            &*DROP_PRONE,
            &*DODGE,
            &*DISENGAGE,
            &*READY,
            &*HELP,
            &*MOUNT,
            &*DISMOUNT,
            &*RELEASE,
            &*PRY_LOOSE,
            &*SHOVE,
            &*GRAPPLE,
            &*GRAPPLE_ESCAPE,
            &*HIDE,
            &*SEARCH,
            &*STUDY,
            &*WIPE_ACID,
            &*DROP_AND_ROLL,
            &*SUNDER,
            &*CUT_FREE,
            &*FIRST_AID,
            &*BURROW,
            &*SURFACE,
        ]
    },
);
