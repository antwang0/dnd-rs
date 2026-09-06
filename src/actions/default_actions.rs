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
        // Frightened: can't willingly move closer to any enemy. We don't
        // track per-source fear yet, so any enemy is a "source." A move
        // that strictly decreases the gap to any enemy is forbidden.
        use crate::conditions::Condition;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return true;
        };
        if !actor.has_condition(Condition::Frightened) {
            return true;
        }
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
pub struct StandUp {}

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
            Some(a) => vec![Resource::Movement(a.speed() / 2.0)],
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

/// 5e Grapple Escape — a grappled creature uses its Action to attempt to
/// break free: "a Strength (Athletics) or Dexterity (Acrobatics) check
/// contested by the grappler's Strength (Athletics) check."
///
/// Two shapes, picked by whether the hold names a grappler. A `Grappled`
/// installed by the Grapple action or by a creature's own grab — the
/// Roper's tendril — carries a back-link, so the escape is the RAW
/// contest against that creature. Everything else that pins a target —
/// Evard's Black Tentacles, Maximilian's Earthen Grasp, an ooze's
/// Adhered — installs the flag with nobody on the other end of it, and
/// those keep the flat DC: there is no grappler's Athletics to roll.
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
        use crate::conditions::Condition;
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| {
                a.is_combat_active()
                    && (a.has_condition(Condition::Grappled)
                        || a.has_condition(Condition::Adhered)
                        || a.has_condition(Condition::EarthenGrasped))
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
        // Snapshot which grapple-like conditions are active before we
        // mutably borrow `encounter` for the roll and log calls.
        let active_conditions: Vec<Condition> =
            [Condition::Grappled, Condition::Adhered, Condition::EarthenGrasped]
                .into_iter()
                .filter(|c| actor.has_condition(*c))
                .collect();
        // A linked `Grappled` names the creature holding on, and that
        // turns the escape into the contest RAW asks for. The captive
        // gets the choice of ability (`GRAPPLE_DEFENSE_CONTEST` on the
        // challenging side here — the roles are reversed from Grapple's,
        // because it is the captive straining now).
        let grappler = actor.linked_by(Condition::Grappled);
        let broke_free = match grappler {
            Some(grappler_id) if encounter.actors.contains_key(&grappler_id) => encounter
                .roll_contest(
                    "escape grapple",
                    caster_id,
                    GRAPPLE_DEFENSE_CONTEST,
                    grappler_id,
                    ATHLETICS_CONTEST,
                ),
            // No grappler on the other end (a spell or a monster ability
            // installed the flag directly, or the grappler is gone):
            // fall back to the flat DC, still rolled as a real check so
            // proficiency and roll mode apply.
            _ => {
                let (ability, skill) = encounter
                    .best_check_option(caster_id, GRAPPLE_DEFENSE_CONTEST)
                    .unwrap_or((
                        crate::engine::types::AbilityScoreType::Strength,
                        crate::engine::types::Skill::Athletics,
                    ));
                let total = encounter.roll_ability_check(caster_id, ability, Some(skill));
                encounter.log(format!(
                    "  escape grapple: {} vs DC {}",
                    total, UNANCHORED_ESCAPE_DC
                ));
                total >= UNANCHORED_ESCAPE_DC
            }
        };
        if broke_free {
            encounter.log("  broke free!".to_string());
            let mut effects: Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> =
                Vec::new();
            // Remove whichever grapple-like condition was active
            for condition in active_conditions {
                effects.push(Box::new(crate::engine::side_effects::RemoveCondition {
                    actor_id: caster_id,
                    condition,
                }));
            }
            // …and the restraint that hold was imposing, if it was
            // imposing one. RAW words the clause as a consequence —
            // "until the grapple ends, the target is restrained" — so
            // breaking the grapple has to end it, and a captive who won
            // the contest and stayed at zero movement would have gained
            // nothing from winning.
            //
            // Gated on the two links naming the *same* holder, which is
            // the whole reason `Restrained` carries one. A creature who
            // breaks a roper's tendril while also standing in somebody
            // else's Web is still in the web.
            if let Some(holder) = grappler
                && encounter
                    .actors
                    .get(&caster_id)
                    .and_then(|a| a.linked_by(Condition::Restrained))
                    == Some(holder)
            {
                effects.push(Box::new(crate::engine::side_effects::RemoveCondition {
                    actor_id: caster_id,
                    condition: Condition::Restrained,
                }));
            }
            effects
        } else {
            encounter.log("  failed to break free.".to_string());
            Vec::new()
        }
    }
}

pub static GRAPPLE_ESCAPE: LazyLock<GrappleEscape> = LazyLock::new(|| GrappleEscape {});

/// 5e Hide action — Stealth check; on success the actor becomes Hidden
/// (attackers have disadvantage, you have advantage on your next attack).
/// DC is the highest passive Perception (10 + WIS mod) among active
/// enemies, defaulting to 10 if none are present. Cannot be used while
/// any enemy is footprint-adjacent — you can't realistically duck from
/// sight while they're inside arm's reach.
pub struct Hide {}

impl Action for Hide {
    fn name(&self) -> &str {
        "hide"
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
/// One gate for all three printings of the action. RAW's own version of
/// this sentence is a paragraph about obscurement and cover; the
/// engine's is the one clause of it a flat, fully-lit board can
/// enforce — **not while something hostile is standing next to you.**
/// You cannot slip out of sight of a creature that is already inside
/// your reach, whatever you are wearing.
///
/// Shared because it was not. `Hide` carried this check and the two
/// bonus-action printings — the Rogue's Cunning Hide and the Ranger's
/// Vanish — carried none, so a rogue toe-to-toe with an ogre could
/// vanish from it as a bonus action while the ogre's own player could
/// not do it with a whole Action. The docstrings on those printings say
/// they are "the same effect at the cheaper cost", and now they are.
pub fn can_attempt_hide(encounter: &EncounterInstance, caster_id: usize) -> bool {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
    let Some(me) = encounter.actors.get(&caster_id) else {
        return false;
    };
    let my_team = me.team();
    let my_loc = me.location();
    let my_size = get_tiles_from_size(me.size());
    !encounter.actors.iter().any(|(id, other)| {
        *id != caster_id
            && other.team() != my_team
            && other.is_combat_active()
            && footprint_chebyshev(
                other.location(),
                get_tiles_from_size(other.size()),
                my_loc,
                my_size,
            ) == 0
    })
}

/// Roll one Hide attempt and return what it installs — the `Hidden`
/// condition on a pass, and nothing at all on a fail.
///
/// 5e Hide is a Dexterity (Stealth) *check* against the best passive
/// Perception watching. It used to roll a Dexterity *save*, which is a
/// different number on the same die: the save lane collects save
/// proficiency, Aura of Protection, Bless, and the save-mode cohorts,
/// and collects none of the Stealth proficiency the action is named
/// after. A rogue who is proficient in Stealth got nothing for it —
/// while `Search`, on the other side of the same contest, was already
/// adding that very proficiency into the DC it compared against.
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
    // The highest passive Perception among active enemies, via
    // `ActorInstance::passive_perception` (which folds in Perception
    // skill proficiency where applicable).
    let caster_team = encounter
        .actors
        .get(&caster_id)
        .map(|a| a.team())
        .unwrap_or(0);
    let dc = encounter
        .actors
        .iter()
        .filter(|(id, a)| **id != caster_id && a.team() != caster_team && a.is_combat_active())
        .map(|(_, a)| a.passive_perception())
        .max()
        .unwrap_or(10);
    let roll = encounter.roll_ability_check(
        caster_id,
        AbilityScoreType::Dexterity,
        Some(Skill::Stealth),
    );
    if roll < dc {
        encounter.log(format!("  hide: stealth {} fails vs DC {}", roll, dc));
        return Vec::new();
    }
    encounter.log("  hide: succeeds");
    vec![Box::new(crate::engine::side_effects::ApplyCondition {
        actor_id: caster_id,
        condition: crate::conditions::Condition::Hidden,
        timer: crate::conditions::ConditionTimer::Permanent,
    })]
}

pub static HIDE: LazyLock<Hide> = LazyLock::new(|| Hide {});

/// 5e Search action — Wisdom (Perception) check vs the Stealth DC of
/// hidden enemies. Costs an Action. On success, every enemy within the
/// searcher's normal sight range whose Stealth roll the Perception check
/// beats loses their Hidden / Invisible cover. We approximate the
/// "Stealth DC" with `12 + DEX modifier` (the standard passive-Stealth
/// shape) per target, computed at the call site. Range is bounded by
/// footprint-Chebyshev gap of 12 (60 ft) — a reasonable in-combat
/// "scan the room" envelope. The Perception check itself routes through
/// `roll_ability_check` so racial / passive bonuses (Keen Senses, etc.)
/// stack on top cleanly.
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
            let dex_mod = target.ability_modifier(AbilityScoreType::Dexterity);
            let mut dc = 12 + dex_mod;
            if target.has_skill(Skill::Stealth) {
                dc += target.proficiency_bonus();
            }
            if perception < dc {
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
                // grants attack advantage to allies for the round.
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
        if revealed == 0 {
            encounter.log("  search: nothing new spotted.".to_string());
        }
        effects
    }
}

pub static SEARCH: LazyLock<Search> = LazyLock::new(|| Search {});

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
            &*WIPE_ACID,
        ]
    },
);
