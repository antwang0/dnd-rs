//! 5e's **Swallow** clause — the stat blocks that end a grapple by
//! eating it.
//!
//! Seven creatures in SRD 5.2 carry one: the Behir, the Giant Frog, the
//! Giant Toad, the Kraken, the Purple Worm, the Remorhaz and the
//! Tarrasque. All seven word it the same way. The victim is already
//! Grappled; the swallower spends an action (a Bonus Action, for four of
//! the seven) and the target *stops being grappled and starts being
//! inside*, at which point four things become true at once:
//!
//!   1. it is **Blinded and Restrained** (the kraken, alone, only
//!      Restrains — a creature in its beak can still see out);
//!   2. it has **Total Cover against attacks and other effects outside**
//!      the swallower, and so does everything outside against it;
//!   3. it takes **acid at the start of each of the swallower's turns**;
//!   4. it can hurt the thing around it, and if it hurts it enough in
//!      one turn the swallower **throws it back up**.
//!
//! ## Why this is not the attach chassis, and not `Restrained`
//!
//! It looks like `engine::attachment` and is deliberately its own
//! module. An attach is two creatures sharing a space with both of them
//! still in the fight; a swallow puts one of them *out of reach of the
//! board entirely*. The differences are not shades:
//!
//!   - **Polarity.** An attach pins the attacher to its host. A swallow
//!     pins the victim inside the swallower, which is the opposite
//!     party, and every gate reads the other way round.
//!   - **Total Cover.** Nothing about an attach hides anybody. This is
//!     the whole tactical shape of a swallow: the party cannot reach
//!     their friend, the friend cannot reach them, and a Fireball on the
//!     worm does not cook the person inside it. That is one gate at
//!     `actors_in_burst`, one at `Action::validate`, and it is the
//!     reason a swallowed creature is not merely `Restrained` in the
//!     swallower's square.
//!   - **The way out is damage.** RAW's escape clause is not a check —
//!     it is a damage threshold measured *per turn* and *by source*,
//!     which needs a counter on the swallower that nothing else in the
//!     engine has needed. `ActorInstance::damage_from_inside_this_turn`
//!     is that counter, and `resolve_regurgitation_checks` is the save
//!     it feeds.
//!
//! Layered on the attach chassis instead, every one of those would have
//! been an `if profile.swallows { … }` in somebody else's method.
//!
//! ## The board, while a creature is inside
//!
//!   - The **swallower owns the tiles.** The victim comes off the
//!     occupancy grid the moment it goes in and its `location` is
//!     mirrored onto the swallower's, the same arrangement `mounts` and
//!     `attachment` use and for the same reason: `actor_map` is one id
//!     per subtile. Everything that reads `location()` — auras,
//!     distances, the map layers — keeps working.
//!   - The victim **does not walk**. `Restrained` zeroes its speed, so
//!     this needs no separate rule; the conditions are refreshed on
//!     every digest tick, so a swallowed creature cannot Freedom of
//!     Movement its way out of the stomach for more than a round.
//!   - The victim **can only reach the swallower**, and only the
//!     swallower can reach it. Both halves are `swallow_blocks_targeting`,
//!     which is RAW's Total Cover expressed as the same shape
//!     `attachment_blocks_hostility` already uses at the same three
//!     sites.
//!
//! ## Getting out
//!
//! Three ways, and the engine has all three:
//!
//!   - **Regurgitation.** Hurt it enough from the inside in one turn and
//!     it fails a Constitution save at the end of that turn, bringing
//!     *everybody* back up — RAW says "all swallowed creatures", not the
//!     one that did the damage. See `resolve_regurgitation_checks`.
//!   - **Killing it.** "If the worm dies, any swallowed creature no
//!     longer has the Restrained condition and can escape from the
//!     corpse using 20 feet of movement, exiting Prone." The engine
//!     charges nothing for the crawl out — it has no lane for movement
//!     spent by a creature whose turn is not running — and lands them
//!     Prone beside the corpse, which is where the movement would have
//!     put them.
//!   - **Digestion running out of victim.** A creature that dies inside
//!     is a creature the fight is over for; it leaves through
//!     `sever_swallows` like any other departing body.
//!
//! ## What is deliberately not modeled
//!
//!   - **The exit cost.** RAW prices the crawl out of a corpse at 5–20
//!     feet of movement, per stat block. Nothing here reads it, for the
//!     reason above.
//!   - **The kraken's tick timing.** RAW hangs the kraken's acid on the
//!     *swallowed* creature's turn and the giant frog's and toad's on
//!     the *end* of the swallower's; the other four say "the start of
//!     each of the swallower's turns", which is what every profile here
//!     uses. The three outliers land within one tick of the same place
//!     and the engine has one start-of-turn lane; a second one would buy
//!     an ordering nobody at a table would notice.
//!   - **The tarrasque's "can't teleport"**. The engine's teleports are
//!     all self-targeted spells the swallowed creature cannot cast at
//!     anything outside anyway, so the clause has nothing here to stop.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::types::{AbilityScoreType, DamageType, Size};

/// How long the conditions a swallow imposes last before the next
/// digest tick refreshes them.
///
/// Two rounds, and the number is `attachment::HOST_CONDITION_TIMER`'s
/// for exactly the same arithmetic: a timer installed on the
/// swallower's turn in round *N* has to survive to its turn in round
/// *N+1* to be refreshed there, and a one-round timer does not.
///
/// Refreshed rather than removed on release, for the same reason the
/// attach chassis refreshes: `Blinded` is not a linked condition and the
/// engine does not refcount conditions, so stripping it on the way out
/// would also strip a blindness the victim had picked up from somebody
/// else's Color Spray. The cost is that a regurgitated creature stays
/// blind for up to a round after it lands, which is a smaller error in
/// a direction nobody has to adjudicate.
pub const SWALLOWED_CONDITION_TIMER: ConditionTimer = ConditionTimer::Rounds(2);

/// RAW's Constitution save the swallower makes when something inside it
/// has hurt it enough. Named because the profile stores only the DC and
/// the threshold; the ability is the same on all seven stat blocks.
pub const REGURGITATION_SAVE: AbilityScoreType = AbilityScoreType::Constitution;

/// The damage threshold and save DC of one stat block's regurgitation
/// clause.
///
/// ```text
/// If the worm takes 30 damage or more on a single turn from a creature
/// inside it, the worm must succeed on a DC 21 Constitution saving throw
/// at the end of that turn or regurgitate all swallowed creatures.
/// ```
///
/// Both numbers are per-creature and neither is derivable from the
/// other: the tarrasque wants sixty damage against a DC 20, and the
/// remorhaz wants thirty against a DC 15. A creature four times as hard
/// to hurt is not four times as hard to make sick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegurgitationClause {
    /// Damage from inside, in one turn, that forces the save.
    pub threshold: u32,
    /// DC of the Constitution save that avoids it.
    pub dc: i32,
}

/// Everything that differs between one stat block's Swallow clause and
/// another's.
///
/// Declared per creature on `CreatureTemplate::swallow` and copied onto
/// the instance, the same way `charge` and `attach` are: the clause
/// belongs to the creature rather than to the action that opens it, and
/// four of the sites that ask about it — the digest tick, the
/// regurgitation check, the targeting gate, the teardown on death —
/// never see the action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwallowProfile {
    /// Verb for the log line ("swallows", "engulfs"), so a frog's gulp
    /// and a tarrasque's do not read identically.
    pub verb: &'static str,
    /// The largest creature this one can get down, or `None` for a
    /// clause RAW leaves ungated.
    ///
    /// `Some(Size::Small)` is the giant frog's "a Small or smaller
    /// target it is grappling"; the toad stops at Medium and the other
    /// five at Large.
    pub max_target_size: Option<Size>,
    /// How many creatures fit at once. One for the behir, the frog and
    /// the toad; six for the tarrasque.
    ///
    /// A hard cap rather than a soft one: RAW parenthesises it into the
    /// targeting line ("it can have up to three creatures swallowed at a
    /// time"), so a full swallower simply cannot take the action.
    pub capacity: usize,
    /// The saving throw that avoids being swallowed, or `None` for the
    /// two stat blocks that offer none.
    ///
    /// The frog and the toad just do it — *"The frog swallows a Small or
    /// smaller target it is grappling"*, no save, no attack roll —
    /// which is the trade for the sizes they are limited to. Everything
    /// bigger asks for a Strength or Dexterity save first.
    pub save: Option<(AbilityScoreType, i32)>,
    /// Damage the swallowed creature takes at the start of each of the
    /// swallower's turns, with its type.
    ///
    /// A slice because the remorhaz's stomach does two things at once —
    /// *"10 (3d6) Acid damage plus 10 (3d6) Fire damage"* — and a
    /// single `(Dice, DamageType)` would have forced it to pick which
    /// half of its own stat line to be.
    pub digest: &'static [(Dice, DamageType)],
    /// What a swallowed creature suffers. `Blinded` and `Restrained` for
    /// six of the seven; the kraken Restrains without blinding.
    pub swallowed_conditions: &'static [Condition],
    /// RAW's damage-from-inside escape clause, or `None` for the two
    /// small stat blocks that print none.
    ///
    /// The frog and the toad have no threshold at all — the frog
    /// disgorges on a timer instead ("if that damage doesn't kill it,
    /// the frog disgorges it") and the toad simply holds on. Neither
    /// clause survives a creature that can hurt them from inside for
    /// more than a round anyway: they have 18 and 39 hit points.
    pub regurgitate: Option<RegurgitationClause>,
    /// `Action::name()` of an attack this creature cannot use while it
    /// has something inside it, or `None`.
    ///
    /// The frog's and the toad's *"can't use Bite while it has a
    /// swallowed target"* — a real cost, and the reason swallowing is a
    /// choice for them rather than a free upgrade: a frog with somebody
    /// inside has given up its only attack.
    pub blocked_while_full: Option<&'static str>,
}

impl SwallowProfile {
    /// The all-empty profile the seven real ones are written as deltas
    /// from, so a stat block declares the clauses it has rather than
    /// nine fields of which six are blank.
    pub const fn defaults() -> Self {
        Self {
            verb: "swallows",
            max_target_size: None,
            capacity: 1,
            save: None,
            digest: &[],
            swallowed_conditions: &[Condition::Blinded, Condition::Restrained],
            regurgitate: None,
            blocked_while_full: None,
        }
    }
}

/// Why a swallow attempt was refused.
///
/// Returned rather than logged, the same shape `AttachRefusal` has and
/// for the same reason: the action asks before it opens its mouth, and
/// wants a sentence for the log rather than a bare `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwallowRefusal {
    /// One of the two ids names nobody, names the same creature twice,
    /// or names a creature that is not on the board.
    NoSuchPair,
    /// The swallower has no Swallow clause on its sheet.
    NoClause,
    /// RAW's size gate — "one Large or smaller creature".
    TooBig,
    /// RAW's parenthesised cap — "it can have up to three creatures
    /// swallowed at a time".
    Full,
    /// The target is not Grappled by this creature. Every one of the
    /// seven clauses names a creature *"Grappled by"* the swallower;
    /// none of them can eat something it is not already holding.
    NotHeld,
    /// The target is already inside something — this creature or
    /// another one. RAW never nests the doll.
    AlreadyInside,
    /// One of the two is half of some *other* link that owns a
    /// footprint: a saddle, or an attach. RAW has nothing to say about
    /// it because nothing in the book stacks them; the engine has to,
    /// because all three mechanisms write the same occupancy grid and a
    /// body cannot be off it twice.
    ///
    /// Refused rather than silently untangled. Swallowing a knight off
    /// his horse would leave the horse's saddle pointing at somebody who
    /// is inside a worm, and the honest answer is that the worm has to
    /// eat one of them.
    Entangled,
}

impl SwallowRefusal {
    /// A player-facing sentence for the refusal, for the log line the
    /// action writes when it cannot go through.
    pub fn describe(self) -> &'static str {
        match self {
            SwallowRefusal::NoSuchPair => "there is nothing there to swallow",
            SwallowRefusal::NoClause => "it has no way to swallow anything",
            SwallowRefusal::TooBig => "it will not fit",
            SwallowRefusal::Full => "it has no room left inside",
            SwallowRefusal::NotHeld => "it has not got hold of them",
            SwallowRefusal::AlreadyInside => "they are already inside something",
            SwallowRefusal::Entangled => "it cannot get a clean hold on them",
        }
    }
}

/// Why a swallowed creature is coming back out. Only the log line and
/// the landing differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisgorgeCause {
    /// The swallower failed its Constitution save after being hurt from
    /// the inside.
    Regurgitated,
    /// The swallower died and the creature crawled out of the corpse.
    CorpseEscape,
    /// The swallower is leaving the board and cannot take passengers
    /// with it. No rule, just teardown.
    SwallowerGone,
}

impl DisgorgeCause {
    fn describe(self) -> &'static str {
        match self {
            DisgorgeCause::Regurgitated => "is thrown back up by",
            DisgorgeCause::CorpseEscape => "crawls out of the corpse of",
            DisgorgeCause::SwallowerGone => "spills out of",
        }
    }
}

impl EncounterInstance {
    /// This actor's Swallow clause, if its stat block has one.
    pub fn swallow_profile(&self, actor_id: usize) -> Option<&'static SwallowProfile> {
        self.actors.get(&actor_id).and_then(|a| a.swallow_profile())
    }

    /// The creature `actor_id` is currently inside, if any.
    pub fn swallowed_by(&self, actor_id: usize) -> Option<usize> {
        self.actors.get(&actor_id).and_then(|a| a.swallowed_by())
    }

    /// True while this actor is somebody's lunch.
    pub fn is_swallowed(&self, actor_id: usize) -> bool {
        self.swallowed_by(actor_id).is_some()
    }

    /// Everything currently inside `swallower_id`, in ascending id
    /// order.
    ///
    /// A scan rather than a stored `Vec` on the swallower, for the
    /// reason `attachers_on` gives: the link is one-to-many and a list
    /// on the other side would be a second copy of the same truth to
    /// keep in step. Sorted so a kraken with four people in it digests
    /// them in the same order on a replay of the same seed.
    pub fn swallowed_in(&self, swallower_id: usize) -> Vec<usize> {
        let mut ids: Vec<usize> = self
            .actors
            .iter()
            .filter(|(_, a)| a.swallowed_by() == Some(swallower_id))
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Whether `swallower_id` may swallow `target_id` right now, and why
    /// not when it may not.
    ///
    /// Every gate here is RAW's, in the order the stat block prints
    /// them: a clause, a creature it is holding, of a size it can get
    /// down, with room left inside.
    pub fn can_swallow(
        &self,
        swallower_id: usize,
        target_id: usize,
    ) -> Result<&'static SwallowProfile, SwallowRefusal> {
        if swallower_id == target_id {
            return Err(SwallowRefusal::NoSuchPair);
        }
        let (Some(swallower), Some(target)) =
            (self.actors.get(&swallower_id), self.actors.get(&target_id))
        else {
            return Err(SwallowRefusal::NoSuchPair);
        };
        if swallower.is_off_board() || target.is_off_board() {
            return Err(SwallowRefusal::NoSuchPair);
        }
        // A dying creature is still swallowable — a purple worm
        // finishing off somebody who has just gone down is the shape of
        // the encounter it belongs to, and the same allowance the attach
        // chassis makes for a stirge.
        if !(target.is_combat_active() || target.is_dying()) {
            return Err(SwallowRefusal::NoSuchPair);
        }
        if !swallower.is_combat_active() {
            return Err(SwallowRefusal::NoSuchPair);
        }
        let Some(profile) = swallower.swallow_profile() else {
            return Err(SwallowRefusal::NoClause);
        };
        if target.swallowed_by().is_some() {
            return Err(SwallowRefusal::AlreadyInside);
        }
        // Nothing nests. A swallower that is itself inside something is
        // sharing a stomach rather than holding a fight, and a target
        // that is half of a saddle or an attach has a footprint that
        // belongs to a third body — see `SwallowRefusal::Entangled`.
        if swallower.swallowed_by().is_some() {
            return Err(SwallowRefusal::AlreadyInside);
        }
        if target.mounted_on().is_some()
            || target.ridden_by().is_some()
            || target.attached_to().is_some()
            || !self.attachers_on(target_id).is_empty()
        {
            return Err(SwallowRefusal::Entangled);
        }
        // RAW: "one Large or smaller creature **Grappled by** the worm".
        // Read off the grapple's own back-link rather than off distance,
        // so the creature has to be held by *this* swallower — a target
        // in somebody else's coils is not on the menu.
        if target.linked_by(Condition::Grappled) != Some(swallower_id) {
            return Err(SwallowRefusal::NotHeld);
        }
        if !target.size().clears_gate(profile.max_target_size) {
            return Err(SwallowRefusal::TooBig);
        }
        if self.swallowed_in(swallower_id).len() >= profile.capacity {
            return Err(SwallowRefusal::Full);
        }
        Ok(profile)
    }

    /// Put `target_id` inside `swallower_id`. The only place a
    /// creature's footprint comes off the board for this link; the
    /// counterparts that put it back are `disgorge` and
    /// `sever_swallows`.
    ///
    /// Charges nothing and rolls nothing — the save and the action cost
    /// belong to the `SwallowAttack` that calls this — so the engine op
    /// stays usable by anything that wants to set the link without an
    /// action behind it.
    pub fn swallow(&mut self, swallower_id: usize, target_id: usize) -> Result<(), SwallowRefusal> {
        let profile = self.can_swallow(swallower_id, target_id)?;
        let (loc, size, swallower_loc) = {
            let target = &self.actors[&target_id];
            let swallower = &self.actors[&swallower_id];
            (target.location(), target.size(), swallower.location())
        };
        // Off the grid first, then onto the swallower — the same
        // ordering `attach` and `mount` need, and for the same reason:
        // clearing after the location has been mirrored would erase the
        // *swallower's* stamp.
        self.clear_footprint_of(target_id, loc, size);
        if let Some(t) = self.actors.get_mut(&target_id) {
            t.set_swallowed_by(Some(swallower_id));
            t.set_location(swallower_loc);
            // "…and the Grappled condition ends." The hold is over
            // because something better has replaced it, which is the one
            // sentence every one of the seven stat blocks agrees on
            // word for word.
            t.remove_condition(Condition::Grappled);
        }
        let (swallower_name, target_name) =
            (self.actor_name(swallower_id), self.actor_name(target_id));
        self.log(format!(
            "  {} {} {}.",
            swallower_name, profile.verb, target_name
        ));
        self.impose_swallowed_conditions(target_id);
        Ok(())
    }

    /// Install (or refresh) what being inside imposes.
    ///
    /// Called on the way in and again on every digest tick; see
    /// `SWALLOWED_CONDITION_TIMER` for why a refresh rather than an
    /// install-and-remove pair.
    fn impose_swallowed_conditions(&mut self, target_id: usize) {
        let Some(swallower_id) = self.swallowed_by(target_id) else {
            return;
        };
        let Some(profile) = self.swallow_profile(swallower_id) else {
            return;
        };
        let target_name = self.actor_name(target_id);
        let mut newly: Vec<Condition> = Vec::new();
        if let Some(target) = self.actors.get_mut(&target_id) {
            for &condition in profile.swallowed_conditions {
                if !target.has_condition(condition) {
                    newly.push(condition);
                }
                target.add_condition(condition, SWALLOWED_CONDITION_TIMER);
            }
        }
        for condition in newly {
            self.log(format!("  {} is {}.", target_name, condition.name()));
        }
    }

    /// Bring one creature back out and stand it beside the swallower,
    /// Prone.
    ///
    /// Returns false — and changes nothing — when there is nowhere to
    /// put it, which is the honest answer for somebody inside a kraken
    /// wedged in a sealed corridor: they stay in. The exception is
    /// `DisgorgeCause::SwallowerGone`, where the swallower's own tiles
    /// are about to be released and `sever_swallows` owns the landing
    /// instead.
    ///
    /// Prone on the way out is RAW and unanimous — *"each of which falls
    /// in a space within 10 feet of the kraken with the Prone
    /// condition"* — and it is what stops regurgitation from being a
    /// pure gift to the party: a creature that has just been thrown up
    /// is on the floor, in reach, with its turn's movement about to be
    /// spent standing.
    pub fn disgorge(&mut self, target_id: usize, cause: DisgorgeCause) -> bool {
        let Some(swallower_id) = self.swallowed_by(target_id) else {
            return false;
        };
        let Some(landing) = self.find_adjacent_teleport_anchor(swallower_id, target_id) else {
            let name = self.actor_name(target_id);
            self.log(format!("  {} has nowhere to land, and stays inside.", name));
            return false;
        };
        self.unlink_swallow(target_id, landing);
        let (target_name, swallower_name) =
            (self.actor_name(target_id), self.actor_name(swallower_id));
        self.log(format!(
            "{} {} {}, and lands prone.",
            target_name,
            cause.describe(),
            swallower_name
        ));
        // Through `add_condition` rather than an `ApplyCondition`
        // side-effect, because this is not a rider on anybody's attack:
        // `disgorge` is called from a save resolution and from a
        // teardown, neither of which has an effects vec to push onto.
        // The condition chokepoint still honours Prone-immunity, which
        // is what an ooze crawling out of a corpse needs.
        if let Some(t) = self.actors.get_mut(&target_id) {
            t.add_condition(Condition::Prone, ConditionTimer::Permanent);
        }
        // The swallower's square is a new space as far as the map
        // layers are concerned: somebody thrown up into a Web has been
        // thrown into a Web.
        self.touch_zones(target_id);
        true
    }

    /// Break the link and put the creature back on the board at
    /// `landing`. The shared tail of `disgorge`; it does not log,
    /// because its callers are narrating different events.
    fn unlink_swallow(&mut self, target_id: usize, landing: crate::engine::types::Coordinate) {
        if let Some(t) = self.actors.get_mut(&target_id) {
            t.set_swallowed_by(None);
            t.set_location(landing);
        }
        let size = self.actors[&target_id].size();
        self.stamp_footprint_of(target_id, landing, size);
    }

    /// Cut whatever swallow link `id` is half of, for a creature that is
    /// leaving the board entirely.
    ///
    /// Returns **true when the caller must not clear `id`'s footprint**
    /// — the same contract `sever_attachments` and `sever_ride_links`
    /// have, and for the same reason: a swallowed creature's remembered
    /// footprint belongs to the swallower, and stamping `None` over it
    /// would erase a living body.
    ///
    /// The swallower side is RAW's death clause: *"If the worm dies, any
    /// swallowed creature no longer has the Restrained condition and can
    /// escape from the corpse using 20 feet of movement, exiting
    /// Prone."* They come out onto the tiles the corpse is about to
    /// vacate, which is where they already are, and the Restrained the
    /// corpse was imposing is lifted explicitly — nothing else would,
    /// since it is a refreshed timer rather than a linked condition.
    pub(crate) fn sever_swallows(&mut self, id: usize) -> bool {
        if self.swallowed_by(id).is_some() {
            if let Some(a) = self.actors.get_mut(&id) {
                a.set_swallowed_by(None);
            }
            return true;
        }
        let eaten = self.swallowed_in(id);
        if eaten.is_empty() {
            return false;
        }
        let (loc, size) = match self.actors.get(&id) {
            Some(swallower) => (swallower.location(), swallower.size()),
            None => return false,
        };
        let owns_tiles = self.actor_id_at(loc) == Some(id);
        if owns_tiles {
            self.clear_footprint_of(id, loc, size);
        }
        let corpse_name = self.actor_name(id);
        for target_id in eaten {
            if let Some(t) = self.actors.get_mut(&target_id) {
                t.set_swallowed_by(None);
                // RAW lifts the Restrained by name; the Blinded that
                // came with it is on the same refreshed timer and lapses
                // on its own, which is the same one-round lag every
                // condition the attach chassis imposes carries.
                t.remove_condition(Condition::Restrained);
                t.add_condition(Condition::Prone, ConditionTimer::Permanent);
            }
            // The vacated box first — it is where they already are —
            // then anywhere beside it. `can_move_to` is still asked
            // rather than assumed: a Large creature crawling out of a
            // Gargantuan corpse may not fit in the anchor tile.
            let landing = if self.can_move_to(target_id, loc) {
                Some(loc)
            } else {
                self.find_adjacent_teleport_anchor(id, target_id)
            };
            let name = self.actor_name(target_id);
            match landing {
                Some(landing) => {
                    let target_size = self.actors[&target_id].size();
                    if let Some(t) = self.actors.get_mut(&target_id) {
                        t.set_location(landing);
                    }
                    self.stamp_footprint_of(target_id, landing, target_size);
                    self.log(format!(
                        "{} {} {}.",
                        name,
                        DisgorgeCause::CorpseEscape.describe(),
                        corpse_name
                    ));
                }
                None => {
                    // Unreachable in practice, and an unstamped actor is
                    // a far cheaper bug than two bodies on one tile:
                    // every distance and burst query still finds it, and
                    // the next thing that moves it stamps it back.
                    self.log(format!("{} is left with nowhere to land.", name));
                }
            }
        }
        owns_tiles
    }

    /// Keep every swallowed creature's `location` mirrored onto the body
    /// carrying it.
    ///
    /// Called from `relocate_actor` after the moving body has taken its
    /// new tile, beside the attach chassis's mirror and for the same
    /// reason: a creature inside a kraken that swims thirty feet has
    /// been moved thirty feet.
    pub(crate) fn mirror_swallowed_onto(
        &mut self,
        swallower_id: usize,
        coord: crate::engine::types::Coordinate,
    ) {
        for target_id in self.swallowed_in(swallower_id) {
            if let Some(t) = self.get_actor(target_id) {
                // Always `set_location`, never `walk_to`: a swallowed
                // creature is not walking anywhere, and crediting it
                // with the swallower's run would let somebody inside a
                // charging behir count the charge as their own.
                t.set_location(coord);
            }
        }
    }

    /// The start-of-turn half of the clause: refresh what being inside
    /// imposes, then digest.
    ///
    /// RAW hangs the acid on the *swallower's* turn — "takes 17 (5d6)
    /// Acid damage at the start of each of the worm's turns" — which is
    /// what makes a swallow a clock the party can stop by killing the
    /// thing rather than a lump the victim eats on its own initiative.
    ///
    /// Routed through `DealDamage` rather than a bare HP subtraction, so
    /// the victim's resistances, concentration save and death handling
    /// all fire exactly as they would for a swing. A creature with acid
    /// resistance really does last twice as long inside a purple worm.
    pub(crate) fn digest_swallowed(&mut self, swallower_id: usize) {
        use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};
        let Some(profile) = self.swallow_profile(swallower_id) else {
            return;
        };
        for target_id in self.swallowed_in(swallower_id) {
            // A body that has stopped being a creature digests into
            // nothing. The link itself is cut by `sever_swallows` when
            // it actually leaves.
            if !self
                .actors
                .get(&target_id)
                .is_some_and(|t| t.is_combat_active() || t.is_dying())
            {
                continue;
            }
            self.impose_swallowed_conditions(target_id);
            for &(dice, damage_type) in profile.digest {
                let amount = self.roll(&dice);
                let (swallower_name, target_name) =
                    (self.actor_name(swallower_id), self.actor_name(target_id));
                self.log(format!(
                    "  {} digests {} for {} {}.",
                    swallower_name, target_name, amount, damage_type
                ));
                DealDamage {
                    actor_id: target_id,
                    amount,
                    damage_type,
                }
                .apply(self);
            }
        }
    }

    /// RAW's escape clause, resolved at the end of every turn: *"If the
    /// worm takes 30 damage or more on a single turn from a creature
    /// inside it, the worm must succeed on a DC 21 Constitution saving
    /// throw at the end of that turn or regurgitate all swallowed
    /// creatures."*
    ///
    /// Swept across every swallower on the board rather than aimed at
    /// the creature whose turn just ended, because the damage need not
    /// have been dealt on the swallower's own turn or the victim's: a
    /// swallowed rogue's readied attack, or a reaction, lands on
    /// somebody else's. RAW says "on a single turn", not "on its turn".
    ///
    /// The counter is cleared for everybody afterwards, which is what
    /// makes the threshold per-turn rather than cumulative — the rule
    /// that keeps a creature nibbling from inside a tarrasque from
    /// eventually adding up to sixty.
    ///
    /// "All swallowed creatures" is RAW and is not a rounding: a kraken
    /// that a single swallowed wizard makes sick brings up all four
    /// people in it. That is the whole reason the clause is worth
    /// aiming for.
    pub(crate) fn resolve_regurgitation_checks(&mut self) {
        let sick: Vec<(usize, RegurgitationClause, u32)> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                let clause = a.swallow_profile()?.regurgitate?;
                let taken = a.damage_from_inside_this_turn();
                (taken >= clause.threshold && taken > 0).then_some((*id, clause, taken))
            })
            .collect();
        for (swallower_id, clause, taken) in sick {
            // Sorted by the same scan `swallowed_in` uses, so the sweep
            // is order-stable; and re-read here because the damage that
            // triggered the check could have emptied the stomach.
            if self.swallowed_in(swallower_id).is_empty() {
                continue;
            }
            let name = self.actor_name(swallower_id);
            self.log(format!(
                "{} has taken {} from inside — DC {} {} or bring it up.",
                name, taken, clause.dc, REGURGITATION_SAVE
            ));
            if self
                .roll_save(swallower_id, REGURGITATION_SAVE, clause.dc)
                .passed()
            {
                self.log(format!("  {} keeps it down.", name));
                continue;
            }
            for target_id in self.swallowed_in(swallower_id) {
                self.disgorge(target_id, DisgorgeCause::Regurgitated);
            }
        }
        for actor in self.actors.values_mut() {
            actor.clear_damage_from_inside();
        }
    }

    /// Record a blow struck from inside, for the regurgitation clause to
    /// read at the end of the turn.
    ///
    /// Called from the `DealDamage` chokepoint, which is the one place
    /// that sees every point of damage after mitigation — and which
    /// carries no attacker, so the source is attributed the way every
    /// other "on your turn" lane in the engine attributes one: through
    /// `current_turn_actor_id`. That is right for the case the clause
    /// exists for (a swallowed creature hacking its way out on its own
    /// turn) and for the readied and reaction cases too, since RAW's
    /// "on a single turn" is a window rather than an owner.
    ///
    /// What it cannot see is a swallowed creature's *ally* hurting the
    /// swallower on the same turn — but that ally is outside, so its
    /// damage was never eligible, and the check reads the sum this
    /// method built rather than the swallower's own HP loss.
    pub(crate) fn note_damage_from_inside(&mut self, victim_id: usize, amount: u32) {
        if amount == 0 {
            return;
        }
        let Some(source_id) = self.current_turn_actor_id() else {
            return;
        };
        if self.swallowed_by(source_id) != Some(victim_id) {
            return;
        }
        if let Some(v) = self.actors.get_mut(&victim_id) {
            v.add_damage_from_inside(amount);
        }
    }

    /// RAW's **Total Cover**, in both directions: *"has Total Cover
    /// against attacks and other effects outside the worm"*.
    ///
    /// True when `actor_id` and `target_id` are on opposite sides of a
    /// stomach wall — one of them inside a creature the other is not
    /// inside. The swallower itself is deliberately *not* blocked, in
    /// either direction: RAW's cover is against what is outside it, and
    /// a creature carving its way out from inside is the entire escape
    /// route the clause is built around.
    ///
    /// Shaped as a predicate rather than wired into the targeting code,
    /// and named to sit beside `charm_blocks_hostility` and
    /// `attachment_blocks_hostility` at the three sites that reach
    /// hostility — declared actions, opportunity attacks and Riposte.
    /// Unlike those two it is not gated on `is_harmful`: cover stops a
    /// Cure Wounds from reaching somebody inside a kraken exactly as
    /// firmly as it stops an arrow.
    pub fn swallow_blocks_targeting(&self, actor_id: usize, target_id: usize) -> bool {
        let actor_in = self.swallowed_by(actor_id);
        let target_in = self.swallowed_by(target_id);
        if actor_in == target_in {
            return false;
        }
        // Either party being the other's swallower is the one pairing
        // cover does not separate.
        actor_in != Some(target_id) && target_in != Some(actor_id)
    }

    /// The frog's and the toad's *"can't use Bite while it has a
    /// swallowed target"*.
    ///
    /// Asked by the named attack's own validation rather than by a
    /// general "what can a full creature do" rule, because RAW names one
    /// attack per stat block and leaves everything else alone: a
    /// tarrasque with six people inside it is not slowed down at all.
    pub fn swallow_blocks_action(&self, actor_id: usize, action_name: &str) -> bool {
        self.swallow_profile(actor_id)
            .and_then(|p| p.blocked_while_full)
            .is_some_and(|blocked| {
                blocked == action_name && !self.swallowed_in(actor_id).is_empty()
            })
    }
}
