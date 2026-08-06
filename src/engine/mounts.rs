//! 5e **Mounted Combat** (PHB p.198) — the rules for putting one
//! creature on top of another.
//!
//! The board has always been one id per tile: `actor_map` stores a
//! single `Option<usize>` per subtile and every collision check reads
//! it. A rider and a mount share a space, so a naive model would need
//! multi-occupancy, which is a change to the *board* rather than to a
//! creature — the same wall the swarm's "can occupy another creature's
//! space" clause hit.
//!
//! What this module does instead is take the rider off the grid. While
//! the link is up:
//!
//!   - The **mount owns the tiles**. It is the only one of the pair
//!     stamped into `actor_map`, so nothing else can walk into the
//!     square and the pair moves as a single body.
//!   - The **rider's `location` is mirrored** onto the mount's, every
//!     step. That is what keeps the rider a first-class creature
//!     everywhere else: distance, reach, auras, bursts, line of sight
//!     and the initiative queue all read `location()` and `size()`
//!     rather than the grid, so a mounted rider is still hit by a
//!     Fireball, still inside a paladin's aura, still a legal target for
//!     the pike of the creature it just rode up to.
//!   - The **rider spends the mount's legs**. `dijkstra_path` resolves
//!     the geometry of a mounted rider's walk against the mount and
//!     `start_turn_for` fills the rider's budget from the mount's speed,
//!     so a knight on a warhorse moves 60 ft on a 2×2 body's pathing and
//!     the warhorse's own turn is skipped. That is RAW's **controlled
//!     mount** — "it moves as you direct it… a controlled mount can move
//!     and act even on the turn that you mount it" — rather than the
//!     independent variant, because a mount whose movement the rider
//!     cannot spend is not a mount, it is a passenger with hooves.
//!
//! The three ways RAW throws a rider off all land in `unseat`, and all
//! three ask the same DC 10 Dexterity save: the mount is knocked prone,
//! the mount is moved against its will, and the mount drops. The fourth
//! way — the rider getting off on purpose — is `dismount`, and costs the
//! movement `ActorInstance::mount_movement_cost` prices.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::encounter::EncounterInstance;
use crate::engine::saves::SaveOutcome;
use crate::engine::types::{AbilityScoreType, Coordinate};

/// 5e "you must succeed on a DC 10 Dexterity saving throw or fall off
/// the mount, landing prone in a space within 5 feet of it" — the DC is
/// flat, and the same one for all three of RAW's involuntary dismounts.
pub const STAY_IN_SADDLE_DC: i32 = 10;

/// Why a rider is coming off. Only the log line differs; the save, the
/// landing and the prone-on-failure are RAW-identical across all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnseatCause {
    /// "If your mount is knocked prone…"
    MountProne,
    /// "If an effect moves your mount against its will while you're on
    /// it…"
    MountForcedMove,
    /// The mount has dropped out of the fight underneath its rider. RAW
    /// covers this under the same clause as being knocked prone (a
    /// falling horse takes its rider down with it); we keep it separate
    /// so the log says what actually happened.
    MountDropped,
}

impl UnseatCause {
    fn describe(self) -> &'static str {
        match self {
            UnseatCause::MountProne => "goes down underneath",
            UnseatCause::MountForcedMove => "is dragged out from under",
            UnseatCause::MountDropped => "falls beneath",
        }
    }
}

/// Why a `mount` attempt was refused. Returned rather than logged so the
/// action layer can use the same answer for its validator (which runs
/// on every keystroke of the target picker, and must not log) and for
/// its resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountRefusal {
    /// One of the two ids names nobody, or names the same creature
    /// twice.
    NoSuchPair,
    /// RAW: "a willing creature". We read willingness as team
    /// membership — a hostile horse is not lending you its back — and
    /// a creature that is down is not willing either.
    Unwilling,
    /// RAW: "that has an appropriate anatomy". See
    /// `CreatureTemplate::mountable`.
    WrongAnatomy,
    /// RAW: "at least one size larger than you".
    TooSmall,
    /// The mount already has somebody on it, or the rider is already on
    /// something. RAW never says a horse carries two knights.
    AlreadyPaired,
    /// You have to be able to reach the stirrup.
    NotAdjacent,
}

impl MountRefusal {
    /// A player-facing sentence for the refusal, used by the action's
    /// log line when a mount attempt that passed the picker still can't
    /// resolve.
    pub fn describe(self) -> &'static str {
        match self {
            MountRefusal::NoSuchPair => "there is nothing there to ride",
            MountRefusal::Unwilling => "it will not carry you",
            MountRefusal::WrongAnatomy => "there is no riding that",
            MountRefusal::TooSmall => "it is not big enough to carry you",
            MountRefusal::AlreadyPaired => "one of you is already in a saddle",
            MountRefusal::NotAdjacent => "it is out of reach",
        }
    }
}

impl EncounterInstance {
    /// True while this actor is sitting on something.
    pub fn is_mounted(&self, actor_id: usize) -> bool {
        self.actors
            .get(&actor_id)
            .is_some_and(|a| a.mounted_on().is_some())
    }

    /// The body whose legs carry `actor_id` across the board: the mount
    /// if it is riding one, itself otherwise.
    ///
    /// The single redirect every movement-shaped query routes through —
    /// pathing geometry, the footprint the walk has to fit, and the tile
    /// writes `relocate_actor` performs. Total and cheap for the
    /// overwhelming majority of actors, who are standing on their own
    /// feet and get their own id straight back.
    pub fn movement_body(&self, actor_id: usize) -> usize {
        self.actors
            .get(&actor_id)
            .and_then(|a| a.mounted_on())
            .unwrap_or(actor_id)
    }

    /// Both halves of a rider/mount pair, or just the one creature when
    /// it isn't half of anything.
    ///
    /// Read by the per-tile triggers a walk fires — `touch_zones` and
    /// `charge_zone_movement` — because a horse that gallops through a
    /// Web has carried its rider into the Web too, and each of them
    /// saves for themselves. The order is always mount-then-rider so a
    /// zone that drops the mount resolves the mount's death before the
    /// rider is asked anything.
    pub fn ride_pair(&self, actor_id: usize) -> Vec<usize> {
        let Some(actor) = self.actors.get(&actor_id) else {
            return vec![actor_id];
        };
        match (actor.mounted_on(), actor.ridden_by()) {
            (Some(mount_id), _) => vec![mount_id, actor_id],
            (None, Some(rider_id)) => vec![actor_id, rider_id],
            (None, None) => vec![actor_id],
        }
    }

    /// The speed the board actually moves this actor at: its mount's if
    /// it is riding one, its own otherwise.
    ///
    /// The distinction `ActorInstance::speed()` cannot make, because a
    /// creature cannot see the thing it is sitting on. Every rule that
    /// converts "your speed" into *feet of travel* reads this instead —
    /// the turn's opening budget, and each of the four Dash-shaped
    /// grants (Dash, Cunning Action's, Step of the Wind, the Eagle
    /// totem's dive). A knight who Dashes on a warhorse covers sixty
    /// more feet, not thirty: RAW's controlled mount is the thing taking
    /// the Dash, and it is the horse that runs.
    ///
    /// Deliberately *not* read by the rules that convert "your speed"
    /// into something other than travel. `mount_movement_cost` prices
    /// getting into the saddle off the rider's own legs, which is the
    /// only speed they have at that moment; and a Longstrider on the
    /// knight still does nothing for the horse, which is RAW — your
    /// speed is not what is carrying you.
    pub fn travel_speed(&self, actor_id: usize) -> f32 {
        self.actors
            .get(&self.movement_body(actor_id))
            .map_or(0.0, |a| a.speed())
    }

    /// Hand a mounted rider their mount's speed as this turn's movement
    /// budget, replacing their own.
    ///
    /// 5e's controlled mount "moves as you direct it" — the rider spends
    /// the horse's legs, and the horse's own turn is skipped for the
    /// same reason (see `process_stack`). Doing it as a budget swap at
    /// the top of the rider's turn, rather than by charging the mount's
    /// pool from inside the movement path, keeps the whole redirect to
    /// two places: this, and the geometry swap in `dijkstra_path`.
    ///
    /// A no-op for everyone on their own feet.
    pub(crate) fn grant_mounted_movement(&mut self, actor_id: usize) {
        if !self.is_mounted(actor_id) {
            return;
        }
        let speed = self.travel_speed(actor_id);
        if let Some(rider) = self.actors.get_mut(&actor_id) {
            rider.set_movement_budget(speed);
        }
    }

    /// 5e **Mounted Combatant**, first clause: whether `attacker_id` is
    /// swinging down from a saddle at something small enough and
    /// footbound enough for RAW to hand them advantage — "an unmounted
    /// creature that is smaller than your mount".
    ///
    /// Three conditions, and each of them is doing work:
    ///
    ///   - The attacker has the feat *and* is currently mounted. The
    ///     feat is dead weight on foot, which is RAW and is also the
    ///     reason this is a predicate on the encounter rather than a
    ///     flag on the actor.
    ///   - The target is not itself mounted. Two riders meeting is a
    ///     fair fight; the clause is about reach and footing, and a
    ///     mounted opponent has both.
    ///   - The target is smaller than the *mount*, not than the rider.
    ///     A Medium knight rides down anything Medium or below because
    ///     the thing bearing down on it is a Large horse.
    pub fn rides_down(&self, attacker_id: usize, target_id: usize) -> bool {
        let Some(attacker) = self.actors.get(&attacker_id) else {
            return false;
        };
        if !attacker.has_mounted_combatant() {
            return false;
        }
        let Some(mount) = attacker.mounted_on().and_then(|id| self.actors.get(&id)) else {
            return false;
        };
        let Some(target) = self.actors.get(&target_id) else {
            return false;
        };
        target.mounted_on().is_none() && target.size().ordinal() < mount.size().ordinal()
    }

    /// 5e **Mounted Combatant**, second clause: "you can force an attack
    /// that targets your mount to target you instead."
    ///
    /// Returns the rider who takes the blow, or `None` when the damage
    /// lands where it was aimed. Resolved as a *damage* redirect rather
    /// than as a retarget before the roll, which is the shape the engine
    /// already has — the Crown Paladin's Divine Allegiance shares this
    /// lane, and `DealDamage` asks both questions in the same breath.
    /// The divergence from RAW is that the attack is rolled against the
    /// horse's AC rather than the knight's; a Knight and a Warhorse
    /// differ by exactly nothing there (AC 18 plate against AC 11
    /// hide — so the divergence is real, and it favours the attacker,
    /// which is the direction to err in for a feat).
    ///
    /// Costs no reaction, unlike its lane-mate, because RAW attaches
    /// none — the feat's second clause has no rider on how often it
    /// fires. What it does share is `redirect_depth`: a blow already
    /// being carried for somebody cannot be handed on again, which is
    /// what stops a rider and an adjacent Crown Paladin from volleying
    /// one hit between them.
    pub fn claim_rider_interposition(&self, mount_id: usize, amount: u32) -> Option<usize> {
        if amount == 0 || self.in_damage_redirect() {
            return None;
        }
        let rider_id = self.actors.get(&mount_id)?.ridden_by()?;
        let rider = self.actors.get(&rider_id)?;
        (rider.has_mounted_combatant() && rider.is_combat_active()).then_some(rider_id)
    }

    /// Whether `rider_id` may climb onto `mount_id` right now, and why
    /// not when it may not.
    ///
    /// The whole of RAW's gate: "a willing creature that is at least one
    /// size larger than you and that has an appropriate anatomy may
    /// serve as a mount", plus the two things the sentence takes for
    /// granted — that neither of you is already paired off, and that you
    /// are standing next to it.
    pub fn can_mount(&self, rider_id: usize, mount_id: usize) -> Result<(), MountRefusal> {
        if rider_id == mount_id {
            return Err(MountRefusal::NoSuchPair);
        }
        let (Some(rider), Some(mount)) =
            (self.actors.get(&rider_id), self.actors.get(&mount_id))
        else {
            return Err(MountRefusal::NoSuchPair);
        };
        if rider.mounted_on().is_some() || mount.ridden_by().is_some() {
            return Err(MountRefusal::AlreadyPaired);
        }
        // A mount that is itself riding something is not available to be
        // ridden — RAW never stacks the tower, and the link is
        // single-valued on both sides.
        if mount.mounted_on().is_some() || rider.ridden_by().is_some() {
            return Err(MountRefusal::AlreadyPaired);
        }
        if !mount.is_mountable() {
            return Err(MountRefusal::WrongAnatomy);
        }
        if mount.team() != rider.team() || !mount.is_combat_active() {
            return Err(MountRefusal::Unwilling);
        }
        if mount.size().ordinal() < rider.size().ordinal() + 1 {
            return Err(MountRefusal::TooSmall);
        }
        if self.footprint_distance(rider_id, mount_id).unwrap_or(isize::MAX) > 0 {
            return Err(MountRefusal::NotAdjacent);
        }
        Ok(())
    }

    /// Put `rider_id` in `mount_id`'s saddle. One of the two writers of
    /// the link (the other is `dismount`), and the only place the
    /// rider's footprint comes off the board.
    ///
    /// Charges nothing: the movement toll is the action's, priced by
    /// `ActorInstance::mount_movement_cost`, so the engine op stays
    /// usable by anything that seats a rider without spending a turn
    /// doing it.
    pub fn mount(&mut self, rider_id: usize, mount_id: usize) -> Result<(), MountRefusal> {
        self.can_mount(rider_id, mount_id)?;
        let (rider_loc, rider_size, mount_loc) = {
            let rider = &self.actors[&rider_id];
            let mount = &self.actors[&mount_id];
            (rider.location(), rider.size(), mount.location())
        };
        // Off the grid first, then into the saddle. The order matters:
        // the tiles have to be released before the rider's location is
        // moved on top of the mount, or the clear would erase the
        // *mount's* stamp instead of the rider's.
        self.clear_footprint_of(rider_id, rider_loc, rider_size);
        if let Some(r) = self.actors.get_mut(&rider_id) {
            r.set_mounted_on(Some(mount_id));
            // A creature that has just been lifted onto a horse is not
            // mid-charge, whatever run it had going.
            r.set_location(mount_loc);
        }
        if let Some(m) = self.actors.get_mut(&mount_id) {
            m.set_ridden_by(Some(rider_id));
        }
        let (rider_name, mount_name) = (self.actor_name(rider_id), self.actor_name(mount_id));
        self.log(format!("{} mounts {}.", rider_name, mount_name));
        // The saddle is a new space as far as the map layers are
        // concerned — a rider hoisted onto a horse standing in a Web has
        // entered the Web.
        self.touch_zones(rider_id);
        Ok(())
    }

    /// Take `rider_id` out of the saddle under their own control,
    /// landing them on a free tile beside the mount.
    ///
    /// Returns false — and changes nothing — when there is nowhere to
    /// put them, which is the honest answer for a rider hemmed in by
    /// walls and bodies: RAW's dismount puts you "in a space within 5
    /// feet", and when no such space exists you stay up. The action
    /// layer refuses the same case in its validator, so this returning
    /// false is the belt to that braces.
    pub fn dismount(&mut self, rider_id: usize) -> bool {
        let Some(mount_id) = self.actors.get(&rider_id).and_then(|a| a.mounted_on()) else {
            return false;
        };
        let Some(landing) = self.find_adjacent_teleport_anchor(mount_id, rider_id) else {
            return false;
        };
        self.unlink_ride(rider_id, mount_id, landing);
        let (rider_name, mount_name) = (self.actor_name(rider_id), self.actor_name(mount_id));
        self.log(format!("{} dismounts from {}.", rider_name, mount_name));
        self.touch_zones(rider_id);
        true
    }

    /// The three involuntary dismounts, and the DC 10 Dexterity save
    /// that answers all of them.
    ///
    /// RAW: "If an effect moves your mount against its will while you're
    /// on it, you must succeed on a DC 10 Dexterity saving throw or fall
    /// off the mount, landing prone in a space within 5 feet of it. If
    /// you're knocked prone while mounted, you must make the same saving
    /// throw." A *passed* save keeps the rider in the saddle entirely —
    /// which is why this returns without touching the link on a pass.
    ///
    /// `MountDropped` is the exception, and the only asymmetry here: a
    /// horse that has fallen out of the fight cannot be stayed on, so
    /// the rider comes off whichever way the die lands and the save
    /// decides only whether they land on their feet.
    pub(crate) fn unseat(&mut self, mount_id: usize, cause: UnseatCause) {
        let Some(rider_id) = self.actors.get(&mount_id).and_then(|a| a.ridden_by()) else {
            return;
        };
        let (rider_name, mount_name) = (self.actor_name(rider_id), self.actor_name(mount_id));
        self.log(format!(
            "  {} {} {} — DC {} DEX to stay seated.",
            mount_name,
            cause.describe(),
            rider_name,
            STAY_IN_SADDLE_DC,
        ));
        let stayed = self.roll_save(rider_id, AbilityScoreType::Dexterity, STAY_IN_SADDLE_DC)
            == SaveOutcome::Pass;
        if stayed && cause != UnseatCause::MountDropped {
            self.log(format!("  {} keeps their seat.", rider_name));
            return;
        }
        // RAW: "landing prone in a space within 5 feet of it." The mount
        // is still standing on its own tiles at this point — a dropped
        // one has not been swept off the board yet — so the rider needs
        // a neighbouring space, not the one underneath them.
        let Some(landing) = self.find_adjacent_teleport_anchor(mount_id, rider_id) else {
            // Nowhere to fall. The rider stays up, which is strictly
            // better than a body written on top of another body — the
            // alternative to this branch is a corrupted occupancy grid.
            // A rider left clinging to a dying horse comes off in
            // `sever_ride_links` when the corpse vacates its tiles.
            self.log(format!("  {} has nowhere to fall, and clings on.", rider_name));
            return;
        };
        self.unlink_ride(rider_id, mount_id, landing);
        if stayed {
            self.log(format!("  {} rolls clear and lands on their feet.", rider_name));
        } else {
            self.log(format!("  {} is thrown, and lands prone.", rider_name));
            if let Some(r) = self.actors.get_mut(&rider_id) {
                r.add_condition(Condition::Prone, ConditionTimer::Permanent);
            }
        }
        self.touch_zones(rider_id);
    }

    /// Cut whatever rider/mount link `id` is half of, for a creature
    /// that is leaving the board entirely — a corpse, or a summon whose
    /// concentration lapsed. No save and no ceremony: nothing here is a
    /// rule, it is the teardown that keeps a link from outliving one of
    /// its ends.
    ///
    /// Returns **true when the caller must not clear `id`'s footprint**,
    /// which covers both of the cases where the plain
    /// `write_footprint(None, loc, size)` every removal path ends with
    /// would erase the wrong body:
    ///
    ///   - `id` was a **rider**, so those tiles belong to the mount it
    ///     was sitting on and were never `id`'s to release.
    ///   - `id` was a **mount** still carrying somebody, in which case
    ///     this releases the tiles itself and then stands the rider up
    ///     on them — the one moment a rider who had nowhere to fall
    ///     finally does have somewhere.
    ///
    /// The ordinary route off a dying mount is `trigger_creature_dropped`
    /// → `unseat`, which fires at 0 hit points with the horse still
    /// standing and gives the rider their DC 10 save. This runs later
    /// and catches only what that left behind.
    pub(crate) fn sever_ride_links(&mut self, id: usize) -> bool {
        let (mounted_on, ridden_by) = match self.actors.get(&id) {
            Some(a) => (a.mounted_on(), a.ridden_by()),
            None => return false,
        };
        if let Some(mount_id) = mounted_on {
            if let Some(m) = self.actors.get_mut(&mount_id) {
                m.set_ridden_by(None);
            }
            if let Some(r) = self.actors.get_mut(&id) {
                r.set_mounted_on(None);
            }
            return true;
        }
        let Some(rider_id) = ridden_by else {
            return false;
        };
        let (loc, size) = {
            let mount = &self.actors[&id];
            (mount.location(), mount.size())
        };
        self.clear_footprint_of(id, loc, size);
        if let Some(m) = self.actors.get_mut(&id) {
            m.set_ridden_by(None);
        }
        if let Some(r) = self.actors.get_mut(&rider_id) {
            r.set_mounted_on(None);
        }
        // The mount was at least one size larger, so the rider's
        // footprint fits inside the box just vacated, anchored at the
        // same corner. `can_move_to` is still asked rather than assumed:
        // the mount may have shrunk since it was mounted.
        let landing = if self.can_move_to(rider_id, loc) {
            Some(loc)
        } else {
            self.find_adjacent_teleport_anchor(id, rider_id)
        };
        match landing {
            Some(landing) => {
                let rider_size = self.actors[&rider_id].size();
                if let Some(r) = self.actors.get_mut(&rider_id) {
                    r.set_location(landing);
                }
                self.stamp_footprint_of(rider_id, landing, rider_size);
                let rider_name = self.actor_name(rider_id);
                self.log(format!("{} comes down where their mount fell.", rider_name));
            }
            None => {
                // Unreachable in practice — the vacated box is exactly
                // where the rider already is. If the board ever says
                // otherwise, an unstamped actor is a far cheaper bug
                // than two bodies sharing a tile: every distance and
                // burst query still finds them, and the next thing that
                // moves them stamps them back.
                let rider_name = self.actor_name(rider_id);
                self.log(format!("{} is left with nowhere to stand.", rider_name));
            }
        }
        true
    }

    /// Break the link and put the rider back on the board at `landing`.
    /// The shared tail of `dismount` and `unseat`; neither logs from
    /// here, because the two of them are narrating different events.
    fn unlink_ride(&mut self, rider_id: usize, mount_id: usize, landing: Coordinate) {
        if let Some(m) = self.actors.get_mut(&mount_id) {
            m.set_ridden_by(None);
        }
        if let Some(r) = self.actors.get_mut(&rider_id) {
            r.set_mounted_on(None);
            r.set_location(landing);
        }
        let size = self.actors[&rider_id].size();
        self.stamp_footprint_of(rider_id, landing, size);
    }
}
