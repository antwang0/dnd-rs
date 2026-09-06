//! 5e's **attach** clause — the stat blocks whose signature move is to
//! latch onto a creature and ride it.
//!
//! Three creatures in SRD 5.2 carry one, and all three word it the same
//! way: the attack hits, "the *creature* attaches to the target", and
//! from then until somebody pulls it off the pair are one body on the
//! board. What differs between them is only what the latch *does* while
//! it holds — the stirge drains, the cloaker blinds and splits the
//! damage aimed at it, the darkmantle smothers — which is why the three
//! share one link and one `AttachProfile` rather than three abilities.
//!
//! ## Why this is a link and not a condition
//!
//! The engine already has four ways for one creature to hold another
//! still, and none of them is this:
//!
//!   - `Grappled` / `Restrained` say the *target* cannot move. An
//!     attached creature's target moves perfectly well; it is the
//!     **attacher** that is pinned, to the thing it is riding.
//!   - `Adhered` (the mimic's glue) is the same polarity as Grappled.
//!   - `Charmed`'s back-link is a fact about who did it, not about
//!     where anybody is standing.
//!
//! RAW's sentence is positional — *"it moves with the target"* — so the
//! model has to be positional too, and the engine has exactly one
//! precedent for two creatures sharing a space: `engine::mounts`. This
//! module borrows its chassis wholesale, with the roles swapped.
//!
//! ## The board, while the latch holds
//!
//!   - The **host owns the tiles.** `actor_map` is one id per subtile,
//!     so the attacher comes off the grid the moment it latches and its
//!     `location` is mirrored onto the host's every time either of them
//!     is relocated. Everything else about it stays a first-class
//!     creature: it is still hit by a Fireball, still in an aura, still
//!     a legal target for the ally trying to peel it off, because all
//!     of those read `location()` rather than the grid.
//!   - The **attacher does not walk.** RAW gives the darkmantle
//!     "its Speed becomes 0… and it moves with the target" and leaves
//!     the other two implicit; the engine reads it as universal,
//!     because an off-grid creature that walked away under its own
//!     power would be stamping a footprint onto tiles it does not own.
//!     `ActorInstance::remaining_movement` returns zero while
//!     `attached_to` is set.
//!   - The **attacher can only attack its host.** RAW says so outright
//!     for the darkmantle ("can attack only the target"), and for the
//!     other two by the narrower clause that it cannot repeat the
//!     attach attack. One gate covers both, at the same site the
//!     Charmed hostility gate uses — see `attachment_blocks_hostility`.
//!
//! ## What is deliberately not modeled
//!
//!   - **RAW's price on the release.** All three stat blocks let the
//!     attacher "detach itself by spending 5 feet of its movement",
//!     one sentence after zeroing the movement it would spend. The
//!     engine charges nothing for `Release`, because there is nothing
//!     in the budget to charge and the creature spending it has no
//!     other use for the turn's movement anyway.
//!   - **The darkmantle's suffocation.** The engine has no breath
//!     clock; that half of the cover clause was already dropped where
//!     the attack was declared, and dropping it here keeps it dropped
//!     in one place.
//!   - **The darkmantle's advantage gate on covering.** RAW blinds only
//!     when the attack had advantage, which the creature arranges by
//!     dropping on you from a ceiling the engine has no vertical axis
//!     to hang it from. The size half of the gate is kept
//!     (`host_conditions_max_size`); the advantage half is not, for the
//!     reason `DARKMANTLE_CRUSH` has always given.
//!
//! ## Footprint asymmetry
//!
//! A mount is at least one size larger than its rider, so the rider's
//! box always fits inside the tiles the pair occupies. An attacher has
//! no such guarantee — a Large cloaker latches onto a Medium creature —
//! so while the link is up the cloaker's own 2×2 footprint is measured
//! from the host's anchor and overhangs tiles the host does not own.
//! Nothing collides (it is off the grid), and the only observable
//! effect is that a wrapped cloaker can be reached from a tile further
//! out than a bare Medium creature could. That is the right direction
//! to err: the ally trying to peel it off should find it easier, not
//! harder, than swinging at the person underneath.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::types::{AbilityScoreType, DamageType, Size, Skill};

/// The check a bystander makes to pull a latched creature off, for the
/// two stat blocks that ask for one: "a creature within 5 feet of it
/// can take an action to try to detach the cloaker, doing so by
/// succeeding on a DC 14 Strength (Athletics) check."
pub const PRY_CHECK: (AbilityScoreType, Skill) =
    (AbilityScoreType::Strength, Skill::Athletics);

/// How long a host keeps the conditions its passenger imposes, before
/// the passenger's next turn refreshes them.
///
/// Two rounds rather than one, and the number is the same one
/// `DARKMANTLE_CRUSH` has always installed its blindness for: a timer
/// installed on the attacher's turn in round *N* has to survive until
/// the attacher's turn in round *N+1* to be refreshed there, and a
/// one-round timer expires at the end of round *N*.
///
/// Refreshing rather than removing on release is deliberate. `Blinded`
/// is not a linked condition and the engine does not refcount
/// conditions, so a release that stripped it would also strip a
/// blindness the host had picked up from somebody else's Color Spray.
/// A short refreshed timer cannot make that mistake; the cost is that a
/// host stays blind for up to a round after the thing on its face lets
/// go, which is a smaller error in a direction nobody has to adjudicate.
pub const HOST_CONDITION_TIMER: ConditionTimer = ConditionTimer::Rounds(2);

/// Everything that differs between one stat block's attach clause and
/// another's. Declared per creature on `CreatureTemplate::attach` and
/// copied onto the instance, the same way `charge` is: the clause
/// belongs to the creature, not to the swing that opens it, and the
/// engine asks about it from five sites that never see the attack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttachProfile {
    /// Verb for the log line when the latch takes hold ("attaches to",
    /// "wraps around"), so three creatures with three very different
    /// silhouettes do not all read the same.
    pub verb: &'static str,
    /// The largest host this creature can latch onto, or `None` when
    /// RAW puts no size gate on the clause.
    ///
    /// `Some(Size::Large)` is the cloaker's "if the target is a Large
    /// or smaller creature"; the stirge and the darkmantle attach to
    /// anything they can hit.
    pub max_host_size: Option<Size>,
    /// Damage the host takes at the start of each of the *attacher's*
    /// turns, with its type — the stirge's "the target takes 5 (2d4)
    /// Necrotic damage at the start of each of the stirge's turns".
    ///
    /// `None` for the two that hold on without feeding.
    pub drain: Option<(Dice, DamageType)>,
    /// What the host suffers while the latch holds — the cloaker's and
    /// the darkmantle's `Blinded`, and nothing for the stirge.
    ///
    /// Installed on attach and refreshed at the start of each of the
    /// attacher's turns; see `HOST_CONDITION_TIMER`.
    pub host_conditions: &'static [Condition],
    /// The largest host that suffers `host_conditions`, or `None` when
    /// the clause has no size gate.
    ///
    /// The darkmantle's "if the target is a Medium or smaller
    /// creature… it covers the target". The cloaker's blindness has no
    /// such gate of its own — the attach clause above it already
    /// stopped at Large — so it leaves this `None` rather than
    /// restating the same number twice.
    pub host_conditions_max_size: Option<Size>,
    /// The cloaker's "the cloaker halves the damage it takes (round
    /// down), and the target takes the same amount of damage": every
    /// blow aimed at the passenger is split down the middle and both
    /// halves land, one on each of them.
    pub shares_damage: bool,
    /// DC of the Strength (Athletics) check a bystander makes to pull
    /// this creature off, or `None` when it simply comes away — the
    /// stirge's "the target or a creature within 5 feet of it can
    /// detach the stirge as an action", with no check named.
    pub pry_dc: Option<i32>,
    /// The darkmantle's "…but has Advantage on its attack rolls",
    /// which it gets for being wrapped around its victim's head.
    pub advantage_on_host: bool,
}

impl AttachProfile {
    /// The all-`None` profile the three real ones are written as
    /// deltas from, so a stat block declares the two or three clauses
    /// it actually has instead of eight fields of which five are empty.
    pub const fn defaults() -> Self {
        Self {
            verb: "attaches to",
            max_host_size: None,
            drain: None,
            host_conditions: &[],
            host_conditions_max_size: None,
            shares_damage: false,
            pry_dc: None,
            advantage_on_host: false,
        }
    }

    /// Whether a host of this size suffers `host_conditions`.
    fn conditions_reach(&self, host_size: Size) -> bool {
        self.host_conditions_max_size
            .is_none_or(|max| host_size.ordinal() <= max.ordinal())
    }
}

/// Why an attach attempt was refused.
///
/// Returned rather than logged, for the same reason `MountRefusal` is:
/// the attack's on-hit rider asks before it latches and wants a
/// sentence for the log, and nothing else should have to guess which of
/// the five gates it tripped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachRefusal {
    /// One of the two ids names nobody, names the same creature twice,
    /// or names a creature that is not on the board to be latched onto.
    NoSuchPair,
    /// The attacker has no attach clause on its sheet.
    NoClause,
    /// RAW's size gate — the cloaker's "if the target is a Large or
    /// smaller creature".
    TooBig,
    /// One of the two is already half of an attachment, in either
    /// direction. RAW never stacks the tower, and a latch onto a
    /// creature that is itself latched onto somebody else would put two
    /// mirrors in series.
    AlreadyPaired,
    /// The attacher is in a saddle, or has somebody in its own. A
    /// creature cannot ride two things at once, and the two links write
    /// the same field on the board.
    Mounted,
}

impl AttachRefusal {
    /// A player-facing sentence for the refusal, for the log line an
    /// attack writes when its hit lands but its rider cannot.
    pub fn describe(self) -> &'static str {
        match self {
            AttachRefusal::NoSuchPair => "there is nothing there to hold onto",
            AttachRefusal::NoClause => "it has no way to hold on",
            AttachRefusal::TooBig => "it is too big to wrap",
            AttachRefusal::AlreadyPaired => "one of them is already held",
            AttachRefusal::Mounted => "it cannot let go of what it is riding",
        }
    }
}

/// Why a latch is coming off. Only the log line differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachCause {
    /// The attacher let go on its own turn.
    Voluntary,
    /// Somebody pulled it off — the host, or an ally beside them.
    Pried,
    /// The host is leaving the board and cannot take a passenger with
    /// it. No rule, just teardown.
    HostGone,
}

impl DetachCause {
    fn describe(self) -> &'static str {
        match self {
            DetachCause::Voluntary => "lets go of",
            DetachCause::Pried => "is pulled off",
            DetachCause::HostGone => "loses its grip on",
        }
    }
}

impl EncounterInstance {
    /// This actor's attach clause, if its stat block has one.
    pub fn attach_profile(&self, actor_id: usize) -> Option<&'static AttachProfile> {
        self.actors.get(&actor_id).and_then(|a| a.attach_profile())
    }

    /// The creature `actor_id` is currently latched onto, if any.
    pub fn attached_host(&self, actor_id: usize) -> Option<usize> {
        self.actors.get(&actor_id).and_then(|a| a.attached_to())
    }

    /// True while this actor is riding somebody it bit.
    pub fn is_attached(&self, actor_id: usize) -> bool {
        self.attached_host(actor_id).is_some()
    }

    /// Everything currently latched onto `host_id`, in ascending id
    /// order.
    ///
    /// A scan rather than a stored back-link, which is the one place
    /// this diverges from the mount chassis and it is deliberate: the
    /// link is one-to-many here (RAW never says a creature can carry
    /// only one stirge, and a cloud of them is the entire point of the
    /// creature), and a `Vec` field on the host would be a second copy
    /// of the truth to keep in step. Rosters are tens of actors, and
    /// every caller either already holds `&mut self` or is on a
    /// per-turn path.
    ///
    /// Sorted so a board with two stirges on one victim resolves them
    /// in the same order on a replay of the same seed.
    pub fn attachers_on(&self, host_id: usize) -> Vec<usize> {
        let mut ids: Vec<usize> = self
            .actors
            .iter()
            .filter(|(_, a)| a.attached_to() == Some(host_id))
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Whether `attacher_id` may latch onto `host_id` right now, and
    /// why not when it may not.
    pub fn can_attach(
        &self,
        attacher_id: usize,
        host_id: usize,
    ) -> Result<&'static AttachProfile, AttachRefusal> {
        if attacher_id == host_id {
            return Err(AttachRefusal::NoSuchPair);
        }
        let (Some(attacher), Some(host)) =
            (self.actors.get(&attacher_id), self.actors.get(&host_id))
        else {
            return Err(AttachRefusal::NoSuchPair);
        };
        // A host that is off the board has no tiles to lend and no
        // location worth mirroring; a banished creature cannot be
        // ridden back. A host that has stopped being a creature has
        // none either, and would be swept off the grid a moment later —
        // taking its passenger's landing with it. `is_dying` is
        // deliberately still a host: a stirge feeding on somebody who
        // has just gone down is the whole shape of the encounter it
        // belongs to.
        if host.is_off_board() || attacher.is_off_board() {
            return Err(AttachRefusal::NoSuchPair);
        }
        if !(host.is_combat_active() || host.is_dying()) {
            return Err(AttachRefusal::NoSuchPair);
        }
        let Some(profile) = attacher.attach_profile() else {
            return Err(AttachRefusal::NoClause);
        };
        if attacher.attached_to().is_some() || host.attached_to().is_some() {
            return Err(AttachRefusal::AlreadyPaired);
        }
        // The other direction of the same "no chains" rule: nothing may
        // latch onto a creature that is itself already carrying
        // somebody's weight *as a passenger*. Two stirges on one victim
        // is fine — that is the host side, and it is a scan, not a
        // link.
        if !self.attachers_on(attacher_id).is_empty() {
            return Err(AttachRefusal::AlreadyPaired);
        }
        if attacher.mounted_on().is_some() || attacher.ridden_by().is_some() {
            return Err(AttachRefusal::Mounted);
        }
        if profile
            .max_host_size
            .is_some_and(|max| host.size().ordinal() > max.ordinal())
        {
            return Err(AttachRefusal::TooBig);
        }
        Ok(profile)
    }

    /// Latch `attacher_id` onto `host_id`. One of the two writers of
    /// the link (the other is `detach`), and the only place the
    /// attacher's footprint comes off the board.
    ///
    /// Charges nothing — the price is the attack that opened it — so
    /// the engine op stays usable by anything that wants to set a latch
    /// without a swing behind it.
    pub fn attach(
        &mut self,
        attacher_id: usize,
        host_id: usize,
    ) -> Result<(), AttachRefusal> {
        let profile = self.can_attach(attacher_id, host_id)?;
        let (loc, size, host_loc) = {
            let attacher = &self.actors[&attacher_id];
            let host = &self.actors[&host_id];
            (attacher.location(), attacher.size(), host.location())
        };
        // Off the grid first, then onto the host — the same ordering
        // `mount` needs and for the same reason: clearing after the
        // location has been mirrored would erase the *host's* stamp.
        self.clear_footprint_of(attacher_id, loc, size);
        if let Some(a) = self.actors.get_mut(&attacher_id) {
            a.set_attached_to(Some(host_id));
            a.set_location(host_loc);
        }
        let (attacher_name, host_name) =
            (self.actor_name(attacher_id), self.actor_name(host_id));
        self.log(format!(
            "  {} {} {}.",
            attacher_name, profile.verb, host_name
        ));
        self.impose_host_conditions(attacher_id);
        // The host's square is a new space as far as the map layers are
        // concerned: a stirge that latches onto somebody standing in a
        // Web has been dragged into the Web.
        self.touch_zones(attacher_id);
        Ok(())
    }

    /// Install (or refresh) whatever the latch imposes on its host.
    ///
    /// Called on attach and again at the start of each of the
    /// attacher's turns; see `HOST_CONDITION_TIMER` for why a refresh
    /// rather than an install-and-remove pair.
    fn impose_host_conditions(&mut self, attacher_id: usize) {
        let Some(profile) = self.attach_profile(attacher_id) else {
            return;
        };
        if profile.host_conditions.is_empty() {
            return;
        }
        let Some(host_id) = self.attached_host(attacher_id) else {
            return;
        };
        let Some(host_size) = self.actors.get(&host_id).map(|a| a.size()) else {
            return;
        };
        if !profile.conditions_reach(host_size) {
            return;
        }
        let host_name = self.actor_name(host_id);
        let mut newly: Vec<Condition> = Vec::new();
        if let Some(host) = self.actors.get_mut(&host_id) {
            for &condition in profile.host_conditions {
                if !host.has_condition(condition) {
                    newly.push(condition);
                }
                host.add_condition(condition, HOST_CONDITION_TIMER);
            }
        }
        for condition in newly {
            self.log(format!("  {} is {}.", host_name, condition.name()));
        }
    }

    /// Break the latch and put the attacher back on the board beside
    /// its host.
    ///
    /// Returns false — and changes nothing — when there is nowhere to
    /// put it, which is the honest answer for a creature wrapped around
    /// somebody in a sealed corridor: it stays on. The exception is
    /// `DetachCause::HostGone`, where the host's own tiles are about to
    /// be released and `sever_attachments` owns the landing instead.
    pub fn detach(&mut self, attacher_id: usize, cause: DetachCause) -> bool {
        let Some(host_id) = self.attached_host(attacher_id) else {
            return false;
        };
        let Some(landing) = self.find_adjacent_teleport_anchor(host_id, attacher_id) else {
            let name = self.actor_name(attacher_id);
            self.log(format!("  {} has nowhere to drop to, and holds on.", name));
            return false;
        };
        self.unlink_attachment(attacher_id, landing);
        let (attacher_name, host_name) =
            (self.actor_name(attacher_id), self.actor_name(host_id));
        self.log(format!(
            "{} {} {}.",
            attacher_name,
            cause.describe(),
            host_name
        ));
        self.touch_zones(attacher_id);
        true
    }

    /// Cut whatever attachment `id` is half of, for a creature that is
    /// leaving the board entirely.
    ///
    /// Returns **true when the caller must not clear `id`'s
    /// footprint** — the same contract `sever_ride_links` has, and for
    /// the same reason: an attached creature's remembered footprint
    /// belongs to the host it is riding, and stamping `None` over it
    /// would erase a living body.
    ///
    /// The host side stands its passengers back up, preferring the tiles
    /// the host is about to vacate — which is exactly where they
    /// already are — and reports true only when it actually released
    /// them.
    ///
    /// It cannot always release them, and the case that proves it is a
    /// stirge on a mounted knight: the knight's remembered footprint
    /// belongs to the horse. So the host branch asks the grid who owns
    /// the tiles before clearing any, and a host that does not own them
    /// leaves them alone and lands its passengers beside instead.
    pub(crate) fn sever_attachments(&mut self, id: usize) -> bool {
        if self.attached_host(id).is_some() {
            if let Some(a) = self.actors.get_mut(&id) {
                a.set_attached_to(None);
            }
            return true;
        }
        let riders = self.attachers_on(id);
        if riders.is_empty() {
            return false;
        }
        let (loc, size) = match self.actors.get(&id) {
            Some(host) => (host.location(), host.size()),
            None => return false,
        };
        let owns_tiles = self.actor_id_at(loc) == Some(id);
        if owns_tiles {
            self.clear_footprint_of(id, loc, size);
        }
        for rider_id in riders {
            if let Some(r) = self.actors.get_mut(&rider_id) {
                r.set_attached_to(None);
            }
            // The vacated box first — it is where the passenger already
            // is — then anywhere beside it. `can_move_to` is still
            // asked rather than assumed: an attacher can be larger than
            // the host it was wrapped around, so the box it is standing
            // in may not fit it once it lets go.
            let landing = if self.can_move_to(rider_id, loc) {
                Some(loc)
            } else {
                self.find_adjacent_teleport_anchor(id, rider_id)
            };
            let name = self.actor_name(rider_id);
            match landing {
                Some(landing) => {
                    let rider_size = self.actors[&rider_id].size();
                    if let Some(r) = self.actors.get_mut(&rider_id) {
                        r.set_location(landing);
                    }
                    self.stamp_footprint_of(rider_id, landing, rider_size);
                    self.log(format!("{} drops where its host fell.", name));
                }
                None => {
                    // Unreachable in practice, and an unstamped actor
                    // is a far cheaper bug than two bodies on one tile:
                    // every distance and burst query still finds it,
                    // and the next thing that moves it stamps it back.
                    self.log(format!("{} is left with nowhere to land.", name));
                }
            }
        }
        owns_tiles
    }

    /// Break the link and put the attacher back on the board at
    /// `landing`. The shared tail of `detach`; it does not log, because
    /// its callers are narrating different events.
    fn unlink_attachment(&mut self, attacher_id: usize, landing: crate::engine::types::Coordinate) {
        if let Some(a) = self.actors.get_mut(&attacher_id) {
            a.set_attached_to(None);
            a.set_location(landing);
        }
        let size = self.actors[&attacher_id].size();
        self.stamp_footprint_of(attacher_id, landing, size);
    }

    /// Keep every passenger's `location` mirrored onto the body they
    /// are riding.
    ///
    /// Called from `relocate_actor` after the moving body has taken its
    /// new tile, for both halves of a rider/mount pair — a stirge on a
    /// knight rides the horse the knight is riding, and the knight's
    /// own mirror has already run by the time this does.
    pub(crate) fn mirror_attachers_onto(
        &mut self,
        host_id: usize,
        coord: crate::engine::types::Coordinate,
        walked: bool,
    ) {
        for rider_id in self.attachers_on(host_id) {
            if let Some(r) = self.get_actor(rider_id) {
                // Mirrored with the same verb the host used, so a
                // passenger on a charging boar is charging and one on a
                // shoved body has been shoved.
                if walked {
                    r.walk_to(coord);
                } else {
                    r.set_location(coord);
                }
            }
        }
    }

    /// The start-of-turn half of the clause: refresh what the host is
    /// suffering, then feed.
    ///
    /// RAW hangs the drain on the *attacher's* turn — "the target takes
    /// 5 (2d4) Necrotic damage at the start of each of the stirge's
    /// turns" — which is what makes a cloud of stirges a damage-over-
    /// time clock the party can stop by killing one of them, rather
    /// than a lump the victim eats on its own turn.
    ///
    /// Routed through `DealDamage` rather than a bare HP subtraction so
    /// the host's resistances, concentration save and death handling
    /// all fire exactly as they would for a swing.
    pub(crate) fn drain_attached_host(&mut self, attacher_id: usize) {
        use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};
        let Some(host_id) = self.attached_host(attacher_id) else {
            return;
        };
        // A latch on a creature that has stopped being a creature — a
        // corpse still on the board, a banished body — feeds on
        // nothing. The link itself is cut by `sever_attachments` when
        // the host actually leaves.
        if !self
            .actors
            .get(&host_id)
            .is_some_and(|h| h.is_combat_active() || h.is_dying())
        {
            return;
        }
        self.impose_host_conditions(attacher_id);
        let Some((dice, damage_type)) = self.attach_profile(attacher_id).and_then(|p| p.drain)
        else {
            return;
        };
        let amount = self.roll(&dice);
        let (attacher_name, host_name) =
            (self.actor_name(attacher_id), self.actor_name(host_id));
        self.log(format!(
            "  {} drains {} for {} {}.",
            attacher_name, host_name, amount, damage_type
        ));
        DealDamage {
            actor_id: host_id,
            amount,
            damage_type,
        }
        .apply(self);
    }

    /// 5e Cloaker: "the cloaker halves the damage it takes (round
    /// down), and the target takes the same amount of damage."
    ///
    /// Returns the host and the halved figure both of them take, or
    /// `None` when this blow is not being split. Shaped as a claim —
    /// the same shape as the two interposition lanes it sits beside in
    /// `DealDamage::apply` — so the damage chokepoint reads as three
    /// questions in a row rather than as one lane with a special case
    /// wired into it.
    ///
    /// Guarded on `in_damage_redirect`, which does two jobs at once:
    /// it stops the re-issued half from splitting again (the recursion
    /// that would otherwise halve a blow down to nothing), and it keeps
    /// a chain of interposing paladins from volleying the same hit.
    pub(crate) fn claim_attachment_damage_share(
        &self,
        victim_id: usize,
        amount: u32,
    ) -> Option<(usize, u32)> {
        if amount == 0 || self.in_damage_redirect() {
            return None;
        }
        if !self.attach_profile(victim_id)?.shares_damage {
            return None;
        }
        let host_id = self.attached_host(victim_id)?;
        Some((host_id, amount / 2))
    }

    /// 5e's "while attached… the *creature* can attack only the
    /// target", and the narrower "can't make Attach attacks against
    /// other targets" the other two stat blocks word it as.
    ///
    /// One gate for both, and the same shape as
    /// `charm_blocks_hostility` so it can sit beside it at the three
    /// sites that reach hostility — declared actions, opportunity
    /// attacks and Riposte. A latched creature swinging at anybody but
    /// the thing it is wrapped around is refused before the die is
    /// rolled.
    pub fn attachment_blocks_hostility(&self, actor_id: usize, target_id: usize) -> bool {
        self.attached_host(actor_id)
            .is_some_and(|host| host != target_id)
    }

    /// 5e Darkmantle: "…but has Advantage on its attack rolls."
    ///
    /// Scoped to the host, which is the only thing it can swing at
    /// anyway — see `attachment_blocks_hostility` — so the gate is
    /// belt-and-braces rather than a second rule.
    pub fn attachment_grants_advantage(&self, actor_id: usize, target_id: usize) -> bool {
        self.attach_profile(actor_id)
            .is_some_and(|p| p.advantage_on_host)
            && self.attached_host(actor_id) == Some(target_id)
    }

    /// The bystander's half of the clause: "the target or a creature
    /// within 5 feet of it can take an action to try to detach the
    /// cloaker, doing so by succeeding on a DC 14 Strength (Athletics)
    /// check."
    ///
    /// Returns whether the latch came off. A profile with no `pry_dc`
    /// comes off without a roll — the stirge's sentence names no check,
    /// and inventing one would make the cheapest creature in the SRD
    /// harder to shift than the CR 8 one.
    ///
    /// The roll goes through `roll_ability_check` rather than a bare
    /// d20 so proficiency, Guidance, Bardic Inspiration, Portent and
    /// the Lucky nat-1 reroll all reach it — every one of which RAW
    /// says applies to an ability check, and every one of which a
    /// hand-rolled d20 here would silently skip.
    pub fn pry_attachment(&mut self, prier_id: usize, attacher_id: usize) -> bool {
        let Some(profile) = self.attach_profile(attacher_id) else {
            return false;
        };
        if self.attached_host(attacher_id).is_none() {
            return false;
        }
        let (prier_name, attacher_name) =
            (self.actor_name(prier_id), self.actor_name(attacher_id));
        let Some(dc) = profile.pry_dc else {
            self.log(format!("  {} pulls {} away.", prier_name, attacher_name));
            return self.detach(attacher_id, DetachCause::Pried);
        };
        let (ability, skill) = PRY_CHECK;
        let total = self.roll_ability_check(prier_id, ability, Some(skill));
        self.log(format!(
            "  {} tries to pull {} loose: {} vs DC {}",
            prier_name, attacher_name, total, dc
        ));
        if total < dc {
            self.log("  it holds on.".to_string());
            return false;
        }
        self.detach(attacher_id, DetachCause::Pried)
    }
}
