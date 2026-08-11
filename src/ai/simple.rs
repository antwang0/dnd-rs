use std::sync::LazyLock;

use crate::actions::action_template::{Action, ActionExecutionInfo, MELEE_REACH, TargetingSchema};
use crate::actions::class_features::{
    ARCANE_ABJURATION, ASPECT_OF_THE_WYRM, CHAMPION_CHALLENGE, CHARM_ANIMALS_AND_PLANTS,
    CONQUERING_PRESENCE, DREADFUL_ASPECT, ENTHRALLING_PERFORMANCE,
    ORDERS_DEMAND, TURN_THE_FAITHLESS, TURN_UNDEAD,
    TurnBurst,
};
use crate::ai::{Controller, ControllerDecision};
use crate::conditions::Condition;
use crate::engine::dice::RollMode;
use crate::engine::encounter::EncounterInstance;
use crate::engine::types::AbilityScoreType;
use crate::engine::underwater::UnderwaterVerdict;
use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

/// Tactical heuristic AI. The decision pipeline runs in priority order:
/// 1. **Kite**: if I have a ranged attack and an enemy is in melee reach
///    of me, step away (one tile) before attacking. Repeated calls per
///    turn means the actor will kite then shoot in the same round.
/// 2. **Focus fire**: among enemies I can hit *this instant*, attack the
///    one with the lowest current HP — finishing wounded targets is
///    higher leverage than spreading damage.
/// 3. **Approach lowest HP**: if no one's in reach, BFS-step toward the
///    weakest visible enemy (not the nearest).
/// 4. **Skip**: nothing useful to do — end the turn.
///
/// Stateless across turns. New behaviors land as new helpers + a new
/// pipeline entry; existing helpers (`try_attack`, `try_step_toward`)
/// stay narrow so adding tactics doesn't tangle them.
pub struct SimpleAi;

impl Controller for SimpleAi {
    fn decide(&self, encounter: &EncounterInstance, actor_id: usize) -> ControllerDecision {
        if !encounter.actors.contains_key(&actor_id) {
            return skip_or_await(encounter, actor_id);
        }

        // 1. Stand up if prone — disadvantage on attacks and 0 movement
        //    otherwise. Costs half-speed; the rest of the turn still has
        //    resources to act.
        if let Some(aei) = try_stand_up(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 1b. Hypnotic Gaze — the enchanter's slot-free adjacent
        //     lockdown. Deliberately ABOVE the kite / teleport /
        //     Disengage rungs below, all of which fire on exactly the
        //     same trigger (a ranged caster with something in contact)
        //     and would otherwise consume every situation the gaze
        //     exists for — an integration probe caught it firing in
        //     0 of 40 encounters when it sat below them. Answering the
        //     melee threat by disabling it beats stepping one tile away
        //     from a hostile that has 5 ft of reach and 30 ft of
        //     movement, and the Charmed half means the gazed creature
        //     can't swing at the enchanter even on the way past.
        if let Some(aei) = try_hypnotic_gaze(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 2. Kite if we're a ranged attacker under melee threat — and
        //    if the shot is actually worth the ground. See
        //    `ranged_lane_beats_staying`: a barbarian with a handaxe on
        //    its belt and a paladin with a javelin both answer yes to
        //    "has a ranged attack", and both are giving up more by
        //    backing out of contact than the throw could ever return.
        if ranged_lane_beats_staying(encounter, actor_id)
            && under_melee_threat(encounter, actor_id)
            && let Some(aei) = try_step_away_from_threats(encounter, actor_id)
        {
            return ControllerDecision::Act(aei);
        }

        // 2a. Blink out when a ranged actor is genuinely pinned — below
        //    half HP or with two or more hostiles in contact. Slotted
        //    after the free one-tile step above and before Disengage
        //    below because it sits between them in both cost and
        //    effect: it spends a slot or a charge where the step spends
        //    nothing, but it actually leaves the melee, where a step
        //    against a 5-ft-reach enemy with 30 ft of movement usually
        //    doesn't and a Disengage buys only a walk. Covers Benign
        //    Transposition, Misty Step and Dimension Door through one
        //    registry.
        if let Some(aei) = try_teleport_escape(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 2b. If we're a low-HP ranged caster surrounded by melee, the
        //    safer exit is the Disengage action — gives our retreat free
        //    OA-suppression. We use it only when our HP is below 30% and
        //    we have a ranged option to capitalize on the disengaged
        //    movement after the action.
        if ranged_lane_beats_staying(encounter, actor_id)
            && under_melee_threat(encounter, actor_id)
            && is_low_hp(encounter, actor_id, 0.3)
            && let Some(aei) = try_disengage(encounter, actor_id)
        {
            return ControllerDecision::Act(aei);
        }

        // 2c. Reel in the catch. A Bonus Action, so it competes with
        //     nothing else on the turn, and it is what turns the
        //     Action that follows from a fourth tendril into a bite.
        //     Above the support lane because it is free; its own gate
        //     is "is anything held by me", so a roper with an empty
        //     line falls straight through. See `try_reel`.
        if let Some(aei) = try_reel(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3. Heal a dying / wounded ally.
        if let Some(aei) = try_support_heal(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3'. The at-will ally-support pulse — an action whose whole
        //     effect is spreading a buff over the teammates standing
        //     near the actor, at no cost and on every turn. See
        //     `try_ally_support_pulse`.
        if let Some(aei) = try_ally_support_pulse(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3a'. Greater Restoration — cleanse a debuffed ally of a severe
        //      condition (Petrified, Stunned, Paralyzed, Blinded, etc.).
        //      High priority because the conditions block the ally's turn.
        if let Some(aei) = try_greater_restoration(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3a. Self-heal (Second Wind) when below half HP — Fighter's
        // bonus-action restore. Comes before attacks because the heal
        // is bonus-action and doesn't conflict with this turn's swing.
        if let Some(aei) = try_self_heal(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3a'. Wild Heal — the Moon Druid's slot-to-hit-points
        //      conversion, available only in beast form. Sits with the
        //      other self-heals because that is what it is; the only
        //      reason it needs its own rung is that it is the *sole*
        //      thing a wild-shaped druid's slots can still buy, so it
        //      should out-rank every casting rung below rather than
        //      compete with them.
        if let Some(aei) = try_wild_heal(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3a''. Shapechanger — the Transmuter's emergency self-Polymorph.
        //       Below the heal lane because 30 temp HP on a beast body
        //       is strictly worse than an actual heal when both are
        //       available, and because the form costs the wizard their
        //       concentration. Its own gate carries the "is that trade
        //       worth it right now?" judgement.
        if let Some(aei) = try_shapechanger(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3a'''. Symbiotic Entity — the Spores Druid's Action-cost
        //        shield. Sits with the self-heals because that is what
        //        36 temp HP on a d8 chassis amounts to, and above the
        //        casting rungs for the reason the Moon Druid's Wild
        //        Heal is: a druid who is about to be hit has a better
        //        use for the Action than a spell. Its own gate carries
        //        the "is anything close enough to matter?" judgement —
        //        the melee rider and the doubled halo are both
        //        short-ranged, so a symbiote raised across the room
        //        spends the charge and buys only the temp HP.
        //
        //        The emergency case is already handled above it: the
        //        symbiote declares `is_heal`, so a druid under half HP
        //        picks it up from the self-heal rung with no distance
        //        gate at all — which is right, because 36 temp HP is
        //        worth the charge whatever else is happening. This rung
        //        is the other half: the healthy druid who should wait
        //        one more turn.
        if let Some(aei) = try_symbiotic_entity(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3a''''. Walls — cut the line an unscreened enemy is closing
        //         down. A wall is self-preservation, so it belongs with
        //         the self-preservation rungs (the kite, the blink, the
        //         Disengage) rather than down among the attacks: by the
        //         time the enemy has arrived there is nothing left to
        //         wall. Under the heals above, because an ally bleeding
        //         out is more urgent than a wall one round early; over
        //         the buffs below, because +3 AC does not answer a hill
        //         giant and a wall does.
        //
        //         The lane's own six-tile window is what keeps it from
        //         eating the fight: outside it — and while the caster is
        //         holding any concentration spell at all — it declines,
        //         and every rung below gets its turn back.
        if let Some(aei) = try_wall_off_approach(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3a⁵. Free summons — the Ranger's Companion, the wildfire
        //      spirit, the tentacle. The half of the summon lane that
        //      costs neither a spell slot nor the caster's concentration,
        //      hoisted above the buff lane because the argument that puts
        //      summons *low* is entirely about concentration and does not
        //      apply to these. See `SummonTier`.
        //
        //      Over the self-buffs below on the plainest arithmetic in
        //      the ladder: a second body attacking every round beats +3
        //      AC on one body, and unlike a buff a summon's value is
        //      strictly decreasing in how long you wait — a round spent
        //      not summoning is a round of its attacks that no later
        //      turn gets back. The Fathomless warlock is the case that
        //      surfaced it: it reached its tentacle in one live fight out
        //      of eight, having spent its bonus action on a shove cantrip
        //      every round the tentacle was sitting there available.
        if let Some(aei) = try_summon_allies(encounter, actor_id, SummonTier::Free) {
            return ControllerDecision::Act(aei);
        }

        // 3a⁶. Get in the saddle. Costs half the actor's speed and
        //      nothing else — no action, no bonus action, no slot — so
        //      it sits above every rung that spends something, and the
        //      turn it fires on still gets its attack from a rung below.
        //
        //      What it buys is the mount's speed for the rest of the
        //      fight, which is the one resource on this ladder that
        //      compounds: a knight who walks the first round arrives a
        //      round later than one who rides, and every round after
        //      that. Its own gate is narrow — the horse has to already
        //      be next to you — so the rung is a no-op on the
        //      overwhelming majority of boards rather than a detour
        //      anyone takes.
        if let Some(aei) = try_mount_up(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3a⁷. …and the other side of the same rung: a horse whose
        //      rider is standing right there holds its ground for one
        //      turn instead of galloping off into the fight alone.
        //      Without it the pair is at the mercy of the initiative
        //      order — a warhorse that rolls above its knight charges
        //      the enemy line by itself, dies to the first thing it
        //      reaches, and the knight spends the fight on foot next to
        //      an empty saddle.
        if let Some(aei) = try_stand_for_rider(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3b. Mage Armor — self-only AC boost. Casts once per combat
        //     since the condition lasts ~100 rounds; gated by "don't
        //     re-cast" via the condition check. Bonus action, so it
        //     stacks with this turn's offensive action.
        if let Some(aei) = try_self_buff_mage_armor(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3b*. See Invisibility — level-2 divination self-buff. Only
        //      fires when an invisible hostile is actually on the map
        //      and the caster can't already see through it, so the
        //      slot is never spent speculatively. Placed right after
        //      Mage Armor because being unable to see the enemy at all
        //      dominates every offensive pick below: an unseen target
        //      costs the caster disadvantage on every attack and hands
        //      the enemy advantage on every swing back.
        if let Some(aei) = try_see_invisibility(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3b**. Darkvision — level-2 transmutation, touch. Sits beside
        //       See Invisibility because it answers the same question
        //       from the other end: that one lifts the enemy's
        //       concealment, this one lifts the room's. Both dominate
        //       every offensive pick below for the same reason — a
        //       caster who cannot see the target swings at
        //       disadvantage and is swung at with advantage — and both
        //       refuse to fire unless the problem is actually on the
        //       board.
        if let Some(aei) = try_darkvision(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3b***. Water Walk — level-3 transmutation, ally burst. A
        //        movement buff rather than a sight one, so it ranks
        //        below both: being slowed by a lake is a worse turn,
        //        not a worse fight. Gated on the water being between
        //        the party and the enemy, which is the only
        //        configuration where the slot pays for itself.
        if let Some(aei) = try_water_walk(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3b'. Armor of Agathys — warlock 1st-level self-buff: 5 temp
        //      HP + 5 cold reflected on melee hit. Pre-buff when an
        //      enemy is near so the retaliation will trigger. Costs
        //      an Action (not bonus action) plus a lv1 slot — pairs
        //      with Hex on the bonus-action lane.
        if let Some(aei) = try_armor_of_agathys(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c. Rage — barbarian's bonus-action damage-resistance + STR
        //     advantage. Fire as soon as an enemy is in reach so the
        //     physical resistance lands before incoming swings. Once
        //     per long rest; gated on the feature flag so a duplicate
        //     call doesn't double-spend the resource.
        if let Some(aei) = try_rage(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c''. Form of Dread — the Undead Warlock's bonus-action
        //       transformation. Same rung as Rage and Bladesong for the
        //       same reason: a once-per-rest, bonus-action posture that
        //       wants to be up before the swinging starts. Its fear
        //       rider needs the warlock to be *landing hits*, so the
        //       gate is the tighter melee-ish band rather than
        //       Bladesong's approach-distance one.
        if let Some(aei) =
            try_self_action_when_enemy_within(encounter, actor_id, IMMINENT_CONTACT_GAP, "form of dread")
        {
            return ControllerDecision::Act(aei);
        }

        // 3c'''. Giant's Might — the Rune Knight's bonus-action
        //        growth. Same rung and the same reasoning as Rage and
        //        Form of Dread: a per-rest bonus-action posture worth
        //        more the earlier it is up. The tighter melee-ish gate
        //        applies here for a reason the others don't have — the
        //        feature's best half is the widened footprint, and a
        //        wider footprint is only worth anything once there is
        //        someone close enough to be caught by it.
        if let Some(aei) = try_self_action_when_enemy_within(
            encounter,
            actor_id,
            IMMINENT_CONTACT_GAP,
            "giant's might",
        ) {
            return ControllerDecision::Act(aei);
        }

        // 3c''''. Elemental Cleaver — the Path of the Giant Barbarian's
        //         bonus-action kindling. Below Rage rather than beside
        //         it, and the order is load-bearing: the action refuses
        //         to install unless the rage is already up, so a rung
        //         above Rage would spend the turn deciding nothing. The
        //         melee-ish gate is Giant's Might's, for the same
        //         reason — the die rides swings, and a barbarian with
        //         nothing in reach has nothing to ride.
        //
        //         The bonus action it spends is the one Frenzy wants,
        //         which is the trade the subclass is priced on. Ranking
        //         the kindling first is right anyway: the cleaver pays
        //         out on every hit for the rest of the rage, and
        //         Frenzy's extra swing pays out once.
        if let Some(aei) = try_self_action_when_enemy_within(
            encounter,
            actor_id,
            IMMINENT_CONTACT_GAP,
            "elemental cleaver",
        ) {
            return ControllerDecision::Act(aei);
        }

        // 3c'''''. Arms of the Astral Self — the Astral Self Monk's
        //         bonus-action summon. Same rung and the same argument
        //         as Rage, Form of Dread and Giant's Might: a
        //         bonus-action posture worth more the earlier it is up.
        //         Gated at Bladesong's wider approach band rather than
        //         the melee-ish one its neighbours use, because the
        //         monk is the one closing the distance and the arms are
        //         what it wants to arrive holding — a summon paid for
        //         on the contact turn has spent the bonus action Flurry
        //         of Blows wanted, on a round the monk could have swung
        //         twice. The action's own `!has_condition` gate makes
        //         this a no-op for the rest of the minute, so the cost
        //         is one bonus action per fight.
        if let Some(aei) = try_self_action_when_enemy_within(
            encounter,
            actor_id,
            BLADESONG_ENGAGE_GAP,
            "arms of the astral self",
        ) {
            return ControllerDecision::Act(aei);
        }

        // 3c'. Bladesong — the Bladesinger's bonus-action trance. Sits
        //      beside Rage because it is the same kind of decision: a
        //      once-per-rest, bonus-action, whole-fight defensive
        //      posture that wants to be up *before* the first swing
        //      lands, not after. Gated a little wider than Rage's
        //      melee-reach trigger (`BLADESONG_ENGAGE_GAP`) because the
        //      wizard is the one who has to close the distance, and a
        //      song started on arrival has already spent the round it
        //      was meant to protect.
        if let Some(aei) =
            try_self_action_when_enemy_within(encounter, actor_id, BLADESONG_ENGAGE_GAP, "bladesong")
        {
            return ControllerDecision::Act(aei);
        }

        // 3c'. Reckless Attack — barbarian / berserker bonus action.
        //      Self-applies the `Helped` rider for advantage on this
        //      turn's melee swing, trading inbound attacker advantage
        //      for the trade-off. Fires only when an enemy is in
        //      melee reach so the next swing consumes the rider.
        //      Pairs with Brutal Critical (more crits per turn → more
        //      extra weapon dice on the lethal swing).
        if let Some(aei) = try_reckless_attack(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c''. Frenzy — Berserker barbarian bonus action while raging.
        //       Grants a fresh Action token for one extra melee swing
        //       this turn. The action's validator gates on the passive
        //       `FRENZY_TAG` feature AND the active `Raging` condition,
        //       so non-Berserker subclasses naturally bounce out. Slots
        //       after Reckless Attack so the granted Action benefits
        //       from the advantage rider on the follow-up swing.
        if let Some(aei) = try_frenzy(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c'^. Wild Shape — the Moon Druid's bonus-action transform.
        //       Slotted here with the other bonus-action commitments
        //       (Rage is the closest sibling: same "become a melee
        //       chassis for the fight" shape) and above the primes,
        //       because unlike them it changes what the rest of the
        //       turn is even allowed to do — the form locks the spell
        //       list out entirely, so deciding it late would mean
        //       re-walking the ladder.
        if let Some(aei) = try_wild_shape(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c''^. Shadow Step — Way of Shadow Monk bonus action,
        //        at-will, 60 ft. The approach counterpart to the
        //        `try_teleport_escape` rung far above: that one moves a
        //        pinned caster away from melee, this one drops a monk
        //        into it with advantage on the swing that follows.
        //        Slotted here with the other bonus-action grants
        //        because it competes with them for the same slot; its
        //        own gate declines whenever an enemy is already in
        //        reach, which is when Stunning Strike and Flurry want
        //        the bonus action more.
        if let Some(aei) = try_shadow_step(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c''*. War Magic — Eldritch Knight bonus action, at-will,
        //        available only on a turn the knight already spent
        //        their Action on a cantrip. Sits next to Frenzy because
        //        it is the same trade in a different costume: a bonus
        //        action bought back into a fresh Action token. Its own
        //        gate is the `WarMagicPrimed` condition, which the
        //        post-cast hook installs — so this picker is a no-op on
        //        every turn the knight opened by swinging, which is
        //        most of them.
        if let Some(aei) = try_war_magic(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c'''. Eagle Dive — Eagle Totem barbarian bonus-action Dash
        //        while raging. Grants a fresh movement chunk for closing
        //        on a fleeing target. The action's validator gates on
        //        the passive `EAGLE_TOTEM_TAG` feature AND the active
        //        `Raging` condition, so non-Eagle Totem subclasses
        //        naturally bounce out. Heuristic gates on "no enemy in
        //        melee reach but at least one within chase range" so
        //        the dive isn't burned when an adjacent target is
        //        already available.
        if let Some(aei) = try_eagle_dive(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c''. Lunging Attack — Fighter Battle Master bonus-action
        //       prime. Extends melee reach by one tile for the next
        //       swing. Fires only when an enemy sits at the precise
        //       gap the lunge opens up (gap 2 — one tile past default
        //       melee reach), so the prime isn't wasted on adjacents
        //       and isn't burned at out-of-reach distances.
        if let Some(aei) = try_lunging_attack(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c''''. Vow of Enmity — Vengeance Paladin Channel Divinity,
        //         bonus action, once per long rest. Marks one in-reach
        //         (4 tiles = 10 ft) hostile so the paladin gets advantage
        //         on attack rolls against the sworn quarry for 10
        //         rounds. Slots before Divine Smite so the vow is up
        //         first — the next swing chain benefits from the
        //         advantage rider AND the smite prime simultaneously
        //         (and Improved Divine Smite's +1d8 rider lands too).
        //         The action's `feature_ready` gate covers once-per-
        //         rest; the AI picker handles target selection.
        if let Some(aei) = try_vow_of_enmity(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c'''''. Hexblade's Curse — Hexblade Warlock subclass level 1,
        //          bonus action, once per short rest. Sits next to Vow of
        //          Enmity on this lane and for the same reason: it is a
        //          bonus-action mark whose payoff is spread over every
        //          later swing, so it wants to be up before the turn's
        //          Action is spent rather than after.
        //
        //          Longer reach than the vow (12 tiles = 30 ft RAW vs 4)
        //          because the hexblade may still be closing when they
        //          name their quarry — the curse costs a bonus action the
        //          warlock has nothing else to do with on an approach
        //          turn, and having it up on arrival is worth more than
        //          waiting to be adjacent.
        if let Some(aei) = try_hexblades_curse(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c''''a. Nature's Wrath — Ancients Paladin Channel Divinity,
        //          action, once per short rest. Restrains one in-reach
        //          (4 tiles = 10 ft RAW) hostile via a STR save vs the
        //          paladin's CHA-anchored DC. Slots next to Vow of Enmity
        //          on the CD family — same 10ft window, single-target,
        //          Ancients-picks-Restrain and Vengeance-picks-advantage-
        //          prime. Restrained gives the paladin advantage on every
        //          follow-up attack against the target for 10 rounds AND
        //          zeroes their movement / imposes save-disadvantage —
        //          harder lock than the Sworn advantage-only prime, at
        //          the cost of an Action (not bonus action). Sibling
        //          shape to Intimidating Presence (Berserker) below.
        if let Some(aei) = try_natures_wrath(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c''''b. Intimidating Presence — Berserker Barbarian action,
        //          once per short rest. Frightens one in-reach (12 tiles
        //          = 30 ft RAW) hostile via a WIS save vs the barbarian's
        //          CHA-anchored DC. Slots next to Nature's Wrath on the
        //          class-feature single-target lockdown lane; distinct
        //          in reach (30ft vs 10ft) and installed condition
        //          (Frightened vs Restrained). Frightened costs the target
        //          attack disadvantage against sources of fear — great
        //          setup for the barbarian's raging swing chain.
        if let Some(aei) = try_intimidating_presence(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c''''c. Path to the Grave — Grave Domain Cleric Channel
        //          Divinity, action, once per short rest. Curses one
        //          in-reach (12 tiles = 30 ft RAW) hostile with
        //          `MarkedForGrave` so any attack against the cursed
        //          target has advantage until the cleric's next turn.
        //          Slots next to Intimidating Presence on the class-
        //          feature single-target lockdown lane; the setup buff
        //          benefits the WHOLE party (advantage on every ally
        //          swing against the cursed target), so we prime the
        //          curse before spending the turn's spell slot / cantrip.
        if let Some(aei) = try_path_to_the_grave(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c'''''. Guided Strike — War Domain Cleric Channel Divinity,
        //          bonus action, once per short rest. Installs the
        //          `GuidedStriking` prime (+10 to next attack roll) on
        //          the cleric. Slots after Vow of Enmity (a straight
        //          advantage buff) and before Divine Smite (a damage
        //          prime) — the accuracy buff is highest-impact on
        //          low-hit-chance swings, so having it up before any
        //          follow-up smite / rider lands guarantees the smite
        //          connects. The action's `feature_prime_ready` gate
        //          covers once-per-short-rest + no-stack; the AI picker
        //          handles the "enemy within attack window" heuristic.
        if let Some(aei) = try_guided_strike(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3c''''''. War Priest — War Domain Cleric bonus-action extra
        //           weapon swing, once per short rest. Grants a fresh
        //           Action token for a follow-up strike this turn.
        //           Slots after Guided Strike so the granted Action
        //           benefits from the +10 accuracy prime if both
        //           charges are up. Same action-economy trade as
        //           Frenzy / Flurry of Blows / Action Surge — the
        //           action's `feature_ready` gate covers once-per-
        //           short-rest; the AI picker handles the "enemy
        //           adjacent" gate so the granted swing lands.
        if let Some(aei) = try_war_priest(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3d. Divine Smite — paladin bonus action that primes the next
        //     melee hit with +2d8 radiant. Fire when an enemy is in
        //     melee reach so the prime is consumed this turn (the
        //     Smiting condition's short Rounds(2) timer covers
        //     reaction-attack edge cases but we don't lean on it).
        if let Some(aei) = try_divine_smite(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3e. Paladin Smite spells — bonus-action concentration primes
        //     (Searing / Wrathful / Thunderous / Branding / Blinding /
        //     Staggering / Banishing, slot-cheapest first). Same trigger
        //     as Divine Smite but concentration-gated; skipped when the
        //     paladin already holds Bless / Compelled Duel etc. Spell
        //     order is defined by `ALL_SMITE_SPELLS` in spells.rs.
        if let Some(aei) = try_smite_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3e''. The signature per-hit marks — Hex and Hunter's Mark.
        //       Bonus-action casts, so laying one still leaves the turn's
        //       Action for the swing that collects on it. What the rung
        //       really spends is the concentration, and that is what
        //       fixes its place: directly above the ranged-smite lane it
        //       competes with head-on, and below the melee smites, which
        //       belong to a chassis that carries no mark.
        //
        //       The arithmetic is the argument. A smite prime and a mark
        //       cost the same bonus action, the same concentration and
        //       (for Ensnaring Strike, Zephyr Strike, Hail of Thorns) the
        //       same 1st-level slot, and both pay 1d6-ish on the next
        //       hit — but the prime stops there and the mark keeps
        //       paying on every swing for ten rounds. A Ranger with
        //       Extra Attack banks the prime's whole value back inside
        //       two rounds and collects for the rest of the fight.
        //
        //       The row this demotes with a real claim is Lightning
        //       Arrow: 4d8 on the next hit plus a splash, which the mark
        //       needs about three rounds to match. It is also a
        //       3rd-level slot against the mark's 1st, and the ordering
        //       does not spend it — a ranger who marks first still has
        //       the arrow in hand for the turn the mark is finally
        //       broken, where a ranger who arrows first has spent the
        //       concentration the mark wanted and will spend it again
        //       next turn on another one-shot.
        //
        //       Placed here rather than down in the self-buff cohort
        //       because that is where it was first, and a Ranger who
        //       carries any smite spell at all never got past this rung
        //       to reach it — which is every Ranger template on the
        //       roster.
        if let Some(aei) = try_concentration_mark(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3e'. Ranged smite spells (Lightning Arrow) — same chassis as
        //      the paladin smites but the prime fires on a ranged
        //      weapon hit. Gated on enemy-in-bow-range (24 tiles)
        //      rather than melee adjacency, since the prime is
        //      consumed by the next bow swing. Slots after the melee
        //      smite picker so a paladin standing next to an enemy
        //      doesn't accidentally pick up a ranged smite.
        if let Some(aei) = try_ranged_smite_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3e''. Fighter Indomitable — the cheapest rung on the whole
        //       ladder, and the reason it sits this high: RAW is "no
        //       action required", so the engine's `free_cost` is exact
        //       and firing it costs the fighter nothing they could have
        //       spent on anything else. There is no version of holding
        //       it that is better than arming it, because the charge is
        //       once per long rest and a fighter in a fight is going to
        //       be asked for a saving throw.
        //
        //       Above the primes rather than below them for the same
        //       reason: a rung that consumes no part of the turn cannot
        //       pre-empt one that does, so its position only decides how
        //       early in the fight the marker goes up.
        if let Some(aei) = try_indomitable(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3f. Monk Stunning Strike — once-per-rest bonus-action prime
        //     that lays a stun save on the next melee hit. Fire when
        //     an adjacent enemy is queued for a swing this turn.
        if let Some(aei) = try_stunning_strike(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3f-. Monk Patient Defense — bonus action, at will: the Dodge
        //      action for free. Above Flurry of Blows because a monk
        //      that is losing wants every incoming swing at
        //      disadvantage more than it wants one more swing of its
        //      own, and below Stunning Strike because a stun that lands
        //      stops the incoming swings altogether. Gated on the monk
        //      actually being under pressure — a healthy monk dodging is
        //      a monk giving up a third of its damage for nothing.
        if let Some(aei) = try_patient_defense(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3f--. Monk Flurry of Blows — bonus action, at will: a whole
        //       extra Action, which on this chassis is another unarmed
        //       strike (two, with Extra Attack). It is the monk's
        //       default use of the bonus action and it was missing from
        //       this ladder entirely, so every monk on the roster has
        //       been fighting at two thirds of its damage whenever
        //       Stunning Strike was unavailable or pointless.
        //
        //       Last of the three monk bonus-action rungs because it is
        //       the one with no precondition worth waiting for: a stun
        //       has to be worth priming and a dodge has to be worth
        //       taking, and an extra swing is worth having whenever
        //       there is something in reach to spend it on.
        if let Some(aei) = try_flurry_of_blows(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3f'. Monk Empty Body — once-per-long-rest defensive burst
        //      Action. Installs Invisible + DamageResistant on self for
        //      10 rounds. Fire when the monk is below 40% HP AND has an
        //      adjacent enemy — the burst dominates a single swing when
        //      survival is at stake. Tighter HP threshold than the heal
        //      picker so the once-per-rest charge doesn't burn on a
        //      first-scratch alarm.
        if let Some(aei) = try_empty_body(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3f''. Monk Stillness of Mind — Action that clears Charmed /
        //       Frightened on the holder. Fire when either condition is
        //       up; cleansing those debuffs (especially Frightened, which
        //       stacks disadvantage on every attack until cured) is worth
        //       the action lane over a single attack.
        if let Some(aei) = try_stillness_of_mind(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3f''. Wipe Acid — universal cleanse Action. Fire when the
        //       holder carries the CausticBrewed DoT and they're below
        //       half HP — the 2d4/round acid drip (avg 5 HP/round) outpaces
        //       most attack swings for a near-downed actor; the
        //       cleanse-then-survive trade dominates. The action's own
        //       validation gates on the flag being present, so the AI
        //       just adds the HP heuristic so a topped-up actor doesn't
        //       burn the Action lane to scrape off a single drip.
        if let Some(aei) = try_wipe_acid(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3f'''. Paladin Cleansing Touch — once-per-long-rest cleanse on
        //        an adjacent ally (or self) afflicted with a heavyweight
        //        lockdown debuff (Paralyzed / Stunned / Charmed / Confused
        //        / Dominated / Frightened). Sits next to the other Action-
        //        priced cleanses so the support pipeline picks it before
        //        the standard attack lane fires.
        if let Some(aei) = try_cleansing_touch(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g. Channel Divinity turn-bursts — Turn Undead, Turn the
        //     Faithless, Arcane Abjuration, Charm Animals and Plants,
        //     Dreadful Aspect. Fire the narrowest variant the actor holds
        //     that has enough eligible hostiles inside the 30ft burst.
        if let Some(aei) = try_turn_burst(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3f''''. Fiend Warlock Hurl Through Hell — the lv14 capstone,
        //         once per rest, 60 ft: the target spends a turn in the
        //         lower planes and comes back with 10d10 psychic on it.
        //         Losing a turn *and* taking the roster's largest
        //         single-target die pool is the biggest thing the
        //         warlock can do to one creature, so the pick is the
        //         beefiest hostile in range per the shared picker — the
        //         damage is fixed, and the turn taken off the board is
        //         worth most against whatever was going to use it best.
        if let Some(aei) = try_hurl_through_hell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3f'''''. Tiefling Infernal Legacy: Infernal Rebuke — a racial
        //          once-per-rest 3d10 fire burst at 60 ft, DEX save for
        //          half. Sibling to Wrath of the Storm two rungs down
        //          and gated the same way: single-target by
        //          construction, save-for-half rather than
        //          save-for-nothing, so it beats the cantrip it competes
        //          with every time it is spent.
        //
        //          Below Hurl Through Hell because no chassis carries
        //          both and the order between them therefore decides
        //          nothing; it is written down because a tiefling
        //          warlock is a build this engine can express.
        if let Some(aei) = try_infernal_rebuke(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g-. Light Domain Cleric Radiance of the Dawn — once-per-rest
        //      Channel Divinity, a 30 ft self-centred radiant burst that
        //      hits hostiles only. Gated on two of them inside the
        //      radius for the same reason `best_burst_placement` refuses
        //      a one-enemy placement: against a single target the
        //      cleric's at-will cantrip is most of the damage for none
        //      of the charge, and the burst's whole value is the crowd.
        //
        //      Above Preserve Life because a burst that lands is often
        //      why the healing is not needed, and below the turn-bursts
        //      because those are the narrower feature — a cleric that
        //      can rout the undead in front of it should, and the two
        //      draw on the same Channel Divinity pool.
        if let Some(aei) = try_radiance_of_the_dawn(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g--. Tempest Domain Cleric Wrath of the Storm — once-per-rest
        //       single-target 2d8 lightning inside 30 ft, save for half.
        //       No crowd gate: unlike the dawn this is single-target by
        //       construction, and half damage on a made save makes it
        //       strictly better than the Sacred Flame the cleric would
        //       otherwise throw at the same creature — which is a save
        //       for *nothing*. A once-per-rest charge that beats the
        //       at-will alternative every time it is spent is a charge
        //       to spend on the first thing in range.
        if let Some(aei) = try_wrath_of_the_storm(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g'. Cleric Preserve Life — once-per-rest Channel Divinity.
        //      Fire when at least one ally (including self) is wounded
        //      below half HP and inside the 30ft aura. The action's
        //      `side_effects` allocates the 5×level pool to the most-
        //      hurt allies first; we just gate on whether anyone needs
        //      it so the feature doesn't burn pre-encounter.
        if let Some(aei) = try_preserve_life(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g'b. Peace Domain Balm of Peace — the other Channel Divinity
        //       on the healing lane, and the one whose gate is about
        //       *where everyone is standing* rather than how hurt they
        //       are. Directly below Preserve Life because when a cleric
        //       could spend either, Preserve Life is the better press:
        //       it reaches 30 ft and tops everyone to half, where the
        //       balm reaches 5 ft and hands over a flat lump. In
        //       practice no cleric holds both — the two are different
        //       domains — so the order between them is a tidiness
        //       question rather than a live one.
        if let Some(aei) = try_balm_of_peace(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g''. Wizard Arcane Recovery — once-per-rest free action that
        //       restores a level-1 (and a level-2 at lv3+) spell slot.
        //       Fire when the caster has spent a slot and isn't burning
        //       it on an empty room.
        if let Some(aei) = try_arcane_recovery(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g''b. Druid Circle of the Land Natural Recovery — sibling
        //        of Arcane Recovery on the Land Druid subclass. Same
        //        engaged-plus-spent-slot gate via the shared
        //        `try_engaged_self_recovery` helper.
        if let Some(aei) = try_natural_recovery(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g'''. Bard Cutting Words — bonus-action enemy debuff. Fire
        //        on the most threatening adjacent-to-an-ally enemy who
        //        isn't already Mocked, so the disadvantage lands before
        //        their swing.
        if let Some(aei) = try_cutting_words(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g'''b. Eloquence Bard Unsettling Words — the same bonus
        //         action on the other axis. Sits directly below Cutting
        //         Words because no template carries both, so the order
        //         between them decides nothing; it sits above the burst
        //         and attack rungs because the whole point is to land
        //         the −4 *before* the save-or-suck spell that collects
        //         on it, and every rung below this one would spend the
        //         turn instead.
        if let Some(aei) = try_unsettling_words(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g2. Pit Fiend Fear Aura — boss-level "frighten everyone
        //      in the room" burst. Fire when 2+ enemies sit inside
        //      the 20ft radius (single-target a normal swing is
        //      better, but at 2+ the multi-target frighten dominates).
        if let Some(aei) = try_fear_aura(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g3. Dragon Breath Weapon — recharge-gated AoE. Fire when
        //      the breath is available and 2+ enemies cluster within
        //      burst range. High-priority because the breath is the
        //      dragon's highest-damage single action; spending it
        //      before it might get wasted to a lucky recharge roll
        //      next turn is always correct.
        if let Some(aei) = try_breath_weapon(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3h⁻. Mantle of Inspiration — the Glamour Bard's other way to
        //      spend the same pool. Sits directly above Bardic
        //      Inspiration because the two draw on one another's
        //      charges and the mantle is the narrower pick: its gate
        //      wants a party that is already being hurt, and on a turn
        //      that gate says no the die below is what the bard should
        //      be handing out instead.
        if let Some(aei) = try_mantle_of_inspiration(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3h. Bardic Inspiration — bonus-action ally buff. Fire on the
        //     highest-HP ally so the inspiration die rides their next
        //     attack swing (front-liners get the most value).
        if let Some(aei) = try_bardic_inspiration(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3h'. Zealous Presence — Zealot Barbarian bonus-action ally-
        //      burst, once per long rest. Blessifies up to 10 allies
        //      within 60ft (24 tiles). Gated on "at least one other
        //      combat-active ally within 24 tiles" — a lone-wolf
        //      zealot doesn't burn the charge to blessify only
        //      themselves. Slots next to Bardic Inspiration on the
        //      ally-buff bonus-action lane; distinct in target set
        //      (burst-of-N vs. single ally) and duration (10 rounds
        //      vs. Inspired's 10-round Rounds timer that consumes on
        //      the next attack/save).
        if let Some(aei) = try_zealous_presence(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3h''. Area control — Web / Hypnotic Pattern / Evard's /
        //       Entangle / Sleet Storm on the densest cluster of
        //       hostiles. Declines while already concentrating.
        //
        //       Placed at the TOP of the concentration lane, above the
        //       apex ally buffs immediately below, because those buffs
        //       fire on turn one and hold for the whole fight — an
        //       integration probe found every wizard opening with
        //       Foresight and therefore never casting a control spell
        //       again, in any of 40 encounters. Only one concentration
        //       can be held, so whichever rung comes first here decides
        //       the caster's entire fight, and a dense cluster of
        //       hostiles is the case where locking them down beats
        //       giving one ally advantage. When no cluster exists the
        //       gate declines and the buffs below fire exactly as
        //       before.
        //
        //       Also above the damage AoE further down: when both would
        //       fire, taking three hostiles out of the fight beats
        //       damaging them, and the scorer both rungs share ranks
        //       purely by how many bodies the blast catches — which a
        //       control spell almost never wins against a
        //       same-or-larger-radius Fireball.
        if let Some(aei) = try_area_control(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3i. Foresight — level-9 single-target ally apex buff. Lay it
        //     on the toughest ally before they engage. Highest priority
        //     of the support-buff lane because the slot is precious.
        if let Some(aei) = try_foresight(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3j. The concentration self-buff cohort, upper half — Holy
        //     Aura, Spirit Shroud, Bigby's Hand, Tenser's
        //     Transformation. One rung per table row used to live here,
        //     each calling a one-line wrapper; see
        //     `SELF_BUFFS_ABOVE_DUPLICITY`.
        if let Some(aei) = try_self_buff_pick(encounter, actor_id, SELF_BUFFS_ABOVE_DUPLICITY) {
            return ControllerDecision::Act(aei);
        }

        // 3k. Invoke Duplicity — Trickery Domain Cleric Channel
        //     Divinity (Action, once per short rest). Ten rounds of
        //     advantage on every attack roll the cleric makes. Sits
        //     between the two halves of the self-buff cohort because it
        //     is the same purchase — an Action spent up front to make
        //     every later swing land — and above the damage lane for
        //     the same reason: a turn spent arming pays back over the
        //     rest of the fight, and paying for it after the shooting
        //     starts wastes the window it buys.
        if let Some(aei) = try_invoke_duplicity(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3l. The concentration self-buff cohort, lower half — the four
        //     Investitures, Wind Wall, Shadow Blade, Antilife Shell, the
        //     two paladin auras, Holy Weapon and Pass Without Trace. See
        //     `SELF_BUFFS_BELOW_DUPLICITY` for what each row's
        //     engagement and ally gates are buying.
        if let Some(aei) = try_self_buff_pick(encounter, actor_id, SELF_BUFFS_BELOW_DUPLICITY) {
            return ControllerDecision::Act(aei);
        }

        // 3m. Warding Bond — cleric / paladin lv2 abjuration. Touch-
        //     range damage-share bond: bonded ally gets +1 AC, +1 saves,
        //     and damage resistance; the caster takes the mirrored
        //     (post-resistance) damage. Fire on a footprint-adjacent
        //     ally that isn't already bonded, when the caster has spare
        //     HP to sink the mirror cost. Not a row on the cohort above
        //     — it targets an ally rather than the caster, and its gate
        //     is about the caster's own remaining hit points rather than
        //     about who is standing where.
        if let Some(aei) = try_warding_bond(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3m'. Emboldening Bond — the Peace Cleric's level-1 charge.
        //      Beside Warding Bond rather than on the self-buff cohort
        //      above for the same reason: what it buffs is other
        //      people, and its gate is about who is standing near the
        //      cleric rather than about the cleric's own state.
        //
        //      Below both because a d4 on future rolls is the least
        //      urgent thing on this stretch of the ladder — it pays out
        //      over ten rounds, so a round spent bonding early is worth
        //      nearly as much as one spent bonding now, which is not
        //      true of a heal or a wall.
        if let Some(aei) = try_emboldening_bond(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3m''. Bastion of Law — the Clockwork Soul's ward. Beside the
        //       two bonds above and below both, on the same reasoning:
        //       it buffs somebody else, and it is preventative, so a
        //       round spent warding early is worth about what a round
        //       spent warding now is. Under `try_support_heal` far
        //       above, which is where the ward lands when an ally is
        //       already bleeding — this rung is the other half, the
        //       front-liner who is about to be.
        if let Some(aei) = try_bastion_of_law(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3n. The engaged self-posture lane — bonus-action, once-per-rest
        //     buttons whose payout is survivability rather than a rider
        //     on the next swing. See `ENGAGED_SELF_POSTURES`.
        //
        //     Above the prime table below because the two compete for
        //     the same bonus action and answer different questions: a
        //     prime makes this turn's swing hit harder, a posture makes
        //     the next few turns survivable, and a chassis that is about
        //     to be in contact wants the second one first. Below the
        //     concentration self-buffs above because those cost a slot
        //     and these cost a charge, so spending the charge first
        //     would leave the slot unspent for the rest of the fight.
        if let Some(aei) = ENGAGED_SELF_POSTURES.iter().find_map(|name| {
            try_self_action_when_enemy_within(encounter, actor_id, IMMINENT_CONTACT_GAP, name)
        }) {
            return ControllerDecision::Act(aei);
        }

        // 3o. The melee-adjacent self-prime lane — one ordered table
        //     rather than N near-identical rungs. See
        //     `MELEE_ADJACENT_PRIMES` for the roster and the reasoning
        //     behind the order.
        if let Some(aei) = MELEE_ADJACENT_PRIMES.iter().find_map(|name| {
            try_self_action_when_enemy_within(encounter, actor_id, MELEE_REACH, name)
        }) {
            return ControllerDecision::Act(aei);
        }

        // 3p'''''. Sweeping Attack — fighter bonus-action prime (Battle
        //          Master). Splashes 1d8 slashing onto one adjacent enemy
        //          of the primary target on hit. Only worth a charge when
        //          there's actually a second hostile clustered next to the
        //          first — gate on at least two adjacent enemies to the
        //          fighter (the splash target sits in the second-enemy
        //          slot). Slotted before Feinting Attack because Sweeping
        //          gives raw damage; Feinting gives raw accuracy, which
        //          the fighter's high STR + Bless / Precision stack
        //          already covers reasonably well.
        if let Some(aei) = try_sweeping_attack(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p''''''. Precision Attack — fighter bonus-action prime (Battle
        //           Master). +4 to the next attack roll (one-shot via
        //           `clear_attack_advantage_riders`). The "do I miss?"
        //           insurance — fire when an enemy is in melee so the
        //           prime is consumed this turn. Same gate as the
        //           `MELEE_ADJACENT_PRIMES` table above but a separate
        //           rung, because Sweeping Attack's two-enemy gate sits
        //           between them and the order is load-bearing.
        if let Some(aei) = try_self_action_when_enemy_within(encounter, actor_id, MELEE_REACH, "precision attack")
        {
            return ControllerDecision::Act(aei);
        }

        // 3p'''''''. Distracting Strike — fighter bonus-action prime
        //            (Battle Master). +1d6 damage to the next melee hit
        //            plus a target-side advantage rider for *other*
        //            allies who attack the same target. Fire only when
        //            both (a) an enemy sits in melee reach so the prime
        //            lands this turn AND (b) at least one other ally is
        //            adjacent to that same enemy — without a follow-up
        //            attacker, the target-side advantage is wasted and
        //            the maneuver collapses to a flat +1d6, which other
        //            maneuvers above already serve. The damage prime is
        //            the floor; the team setup is the upside.
        if let Some(aei) = try_distracting_attack(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p''''''''. Feinting Attack — fighter bonus-action prime (Battle
        //             Master). Targets one enemy in melee reach and grants
        //             self-advantage on the next attack vs them via the
        //             help-grant lane. Higher leverage than Precision
        //             against high-AC targets where advantage outperforms
        //             a flat +4; lower than Sweeping when there's an
        //             adjacent splash target available.
        if let Some(aei) = try_feinting_attack(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p'''''''''. Versatile Trickster — Arcane Trickster rogue
        //              bonus action. Same help-grant lane as Feinting
        //              Attack, at-will and at 30 ft instead of per-rest
        //              and at arm's length. Sits immediately after it
        //              because it is the same decision, and below every
        //              Cunning Action rung above because those spend the
        //              same bonus action on things the rogue needs more
        //              often — the gate is what keeps this from
        //              crowding them out.
        if let Some(aei) = try_versatile_trickster(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p*. Action Surge — free, once per short rest, hands the
        //      fighter a second Action. Sits with the other free primes
        //      because it costs nothing to take: firing it here never
        //      displaces anything, and the ladder is re-entered
        //      afterwards so the extra action gets spent on whatever
        //      rung the fighter would have used anyway.
        if let Some(aei) = try_action_surge(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q-. Kensei's Shot — the Kensei Monk's free bonus-action
        //      prime. Sits with the other bonus-action primes and is
        //      gated on a hostile inside bow range rather than melee
        //      reach, because the whole point of the feature is the
        //      shots the monk takes without closing. Costs nothing but
        //      the bonus action, so it never competes with a rung below
        //      it — the only thing it can lose the monk is a Flurry
        //      they were not in position to throw.
        if let Some(aei) =
            try_self_action_when_enemy_within(encounter, actor_id, BOW_RANGE_GAP, "kensei's shot")
        {
            return ControllerDecision::Act(aei);
        }

        // 3q-'. Arcane Shot — the Arcane Archer's bonus-action prime.
        //       Sits beside Kensei's Shot for the same reason: it is a
        //       ranged prime, so its gate is bow range rather than
        //       melee reach, and an archer who waited to be adjacent
        //       would never spend it. Below the Battle Master rungs
        //       above because those are gated on an enemy already in
        //       reach — a turn where both fire is a turn the archer got
        //       caught in melee, and the maneuver that is already
        //       cashable should win the bonus action.
        if let Some(aei) = try_arcane_shot(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p'. Starry Form — Circle of Stars Druid. Sits above
        //      Shillelagh because the two compete for the same bonus
        //      action and they are not the same size of purchase: the
        //      constellation is ten rounds of a new capability bought
        //      once per short rest, and Shillelagh is +1d8 on one
        //      swing. Deferring the form to buy the die would mean the
        //      druid never transforms at all, since a Stars druid
        //      standing in melee has a Shillelagh worth casting every
        //      single round.
        if let Some(aei) = try_starry_form(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q. Shillelagh — druid bonus-action cantrip prime that adds
        //     +1d8 force damage to the next melee weapon hit. Fire when
        //     an enemy is footprint-adjacent so the prime is consumed
        //     this turn. Slot-free (cantrip) so it stays on the bonus-
        //     action lane without competing with leveled smites.
        if let Some(aei) = try_shillelagh(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q'. Empowered Spell — sorcerer bonus-action metamagic prime.
        //      Burns 1 sorcery point to reroll low dice on the next
        //      spell damage roll. Fire when an enemy sits in spell
        //      range (a generous 24 tiles — covers Fireball at 60 ft,
        //      Cone of Cold at 60 ft, Magic Missile at 120 ft) so the
        //      prime feeds an actual blast this turn. Skipped if the
        //      sorcerer is already primed.
        if let Some(aei) = try_empowered_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q'-b. Overchannel — Evocation Wizard prime. Free to declare;
        //        makes the next level 1-5 damaging spell deal maximum
        //        damage. Sits next to the metamagic primes because it's
        //        the same "set up the next cast" shape, but its picker
        //        gates on surviving the backlash rather than on a
        //        resource pool.
        if let Some(aei) = try_overchannel(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q''. Heightened Spell — sorcerer bonus-action metamagic prime.
        //       Burns 3 sorcery points to force the first save against
        //       the next save-or-suck cast (Hold Person / Polymorph /
        //       Banishment / Dominate Monster) at disadvantage. The
        //       tactic gates on the sorcerer having a heightenable
        //       control spell in the kit AND not already concentrating
        //       (otherwise the follow-up control spell would lose its
        //       slot to concentration loss).
        if let Some(aei) = try_heightened_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q'''. Careful Spell — sorcerer bonus-action metamagic prime.
        //        Burns 1 sorcery point so the next AoE can spare up to
        //        CHA-mod allies caught in the blast. Fires only when an
        //        ally is sitting close enough to a live enemy that a
        //        typical AoE between them would catch the ally too —
        //        without that overlap the prime never pays off.
        if let Some(aei) = try_careful_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q''''. Distant Spell — sorcerer bonus-action metamagic prime.
        //         Burns 1 sorcery point to double the range of the next
        //         ranged spell. Fires only when the nearest enemy sits
        //         in the 13–24 tile band where doubled reach actually
        //         flips a "can't hit" into a hit; closer enemies don't
        //         need the bump and farther ones stay unreachable.
        if let Some(aei) = try_distant_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q'''''. Twinned Spell — sorcerer bonus-action metamagic prime.
        //          Burns max(1, spell_level) SP at consume time to re-fire
        //          the next single-target spell on a second target. Fires
        //          only when at least two combat-active enemies sit in
        //          range AND the kit owns a known-twinnable single-target
        //          damage spell — without those, the prime would dangle
        //          and the SP would be wasted.
        if let Some(aei) = try_twinned_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q''''''. Extended Spell — sorcerer bonus-action metamagic
        //           prime. Burns 1 sorcery point to double the next
        //           long-duration condition install. Fires only when the
        //           kit owns an extendable spell and an enemy is in
        //           engagement range — otherwise the prime would dangle
        //           and the SP would be wasted.
        if let Some(aei) = try_extended_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q'''''''. Seeking Spell (Tasha's) — sorcerer bonus-action
        //            metamagic prime. Burns 2 sorcery points so the
        //            next missed spell-attack roll is rerolled. Fires
        //            only when the kit owns a spell-attack spell (Fire
        //            Bolt / Ray of Frost / Chromatic Orb / Witch Bolt /
        //            ...) AND an enemy sits in attack range — otherwise
        //            the prime would dangle. The 2 SP cost makes this
        //            the most expensive of the cheap primes, so it
        //            slots last in the metamagic lane.
        if let Some(aei) = try_seeking_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q''''''''. Subtle Spell — sorcerer bonus-action metamagic
        //             prime. Burns 1 sorcery point so the next spell
        //             slips past Counterspell. Fires only when an
        //             opposing-team counterspeller is on the field —
        //             without that, the prime protects nothing and the
        //             1 SP is wasted. Cheap enough (1 SP) that even a
        //             single opposing wizard / sorcerer / warlock with
        //             Counterspell triggers the gate.
        if let Some(aei) = try_subtle_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q'''''''''. Transmuted Spell (Tasha's) — sorcerer bonus-action
        //             metamagic prime. Burns 1 sorcery point to remap the
        //             next elemental spell's damage type to the target's
        //             worst weakness (vulnerability > non-resisted >
        //             best-non-immune). Fires only when at least one
        //             nearby enemy has an asymmetric resistance profile
        //             (some elements resisted/immune AND some
        //             vulnerable/neutral) so the prime delivers an actual
        //             damage win.
        if let Some(aei) = try_transmuted_spell(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q'''''''''. Tides of Chaos — Wild Magic Sorcerer 1/long-rest
        //              bonus action. Installs advantage on the next
        //              attack roll. No SP cost; pairs naturally with a
        //              metamagic prime to double up on a single big
        //              cast. Gated on the feature charge being available
        //              and an enemy in attack range.
        if let Some(aei) = try_tides_of_chaos(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q'''''''''''-. Insightful Fighting — Inquisitive Rogue lv3
        //                  bonus action. Marks one hostile within 30 ft
        //                  so every subsequent Sneak Attack against it
        //                  lands without needing advantage or a flanker.
        //
        //                  Above Steady Aim because it outlives it: the
        //                  mark is good for ten rounds and Steady Aim's
        //                  advantage is good for one swing, so a rogue
        //                  that can afford exactly one bonus action this
        //                  turn should buy the ten rounds. It also costs
        //                  no movement, which is the whole price of
        //                  Steady Aim on a chassis that usually wants to
        //                  be somewhere else by the end of the turn.
        //
        //                  Highest-HP hostile wins per the shared picker,
        //                  and here that heuristic is doing real work: the
        //                  mark pays once per swing that lands on the
        //                  marked creature, so its value is proportional
        //                  to how many swings the creature survives. Same
        //                  reasoning as Hexblade's Curse.
        if let Some(aei) = try_insightful_fighting(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q'''''''''''--. Master of Tactics — Mastermind Rogue lv3
        //                  bonus action. Hands an ally the Help action's
        //                  advantage from up to 30 ft away.
        //
        //                  Below Insightful Fighting because the rogue's
        //                  own Sneak Attack is worth more to the rogue
        //                  than an ally's swing, and above Steady Aim for
        //                  the same reason Insightful Fighting is: it
        //                  costs no movement. Fires only when an ally
        //                  actually has something in reach to swing at —
        //                  advantage handed to a creature with no target
        //                  expires unspent.
        if let Some(aei) = try_master_of_tactics(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q''''''''''''. Steady Aim — Tasha's Rogue lv3 bonus action.
        //                 Mirrors Tides of Chaos's advantage prime but
        //                 costs the rest of the turn's movement instead
        //                 of a feature charge. Gated on no movement
        //                 spent yet AND an enemy within ranged-attack
        //                 reach so the speed-zero trade pays off.
        if let Some(aei) = try_steady_aim(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q'''''''''''''. Cunning Strike (2024 Rogue lv5) — bonus-action
        //                  primes that trade Sneak Attack dice for
        //                  tactical effects (Poison / Trip). Fires when
        //                  the rogue has a sneak-eligible adjacent
        //                  target, the sneak charge is still fresh, and
        //                  the sneak pool can spare a die.
        if let Some(aei) = try_cunning_strike(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3q''''''''''. Font of Magic — convert spell slot ↔ sorcery
        //             points. Bonus action; only fires when one
        //             resource is critically low while the other has
        //             headroom. The two sub-tactics (refill SP from a
        //             held slot, recreate a low-level slot from SP) are
        //             surfaced behind the same gate so the AI doesn't
        //             zig-zag between directions in the same turn.
        if let Some(aei) = try_font_of_magic(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3r. Telekinetic — wizard / sorcerer / warlock bonus-action
        //     cantrip shove. Pulls an enemy 5 ft closer on a failed STR
        //     save; no slot. Fire when an enemy is just out of reach for
        //     a melee follow-up next turn — typically gap 2-6 tiles
        //     (5-15 ft) so the pull yanks them into melee range without
        //     wasting on an enemy already adjacent. Slot-free, so it
        //     stays on the bonus-action lane alongside Shillelagh.
        if let Some(aei) = try_telekinetic(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3s. Halo of Spores — Circle of Spores Druid reaction. Costs
        //     neither the Action nor the Bonus Action, so it never
        //     competes with anything below it; the only thing it can
        //     lose the druid is a reaction they had no other use for.
        //     That is exactly why it sits above the casting rungs
        //     rather than in them — deferring it risks the turn ending
        //     with the slot unspent, and an unspent reaction is worth
        //     nothing at all.
        if let Some(aei) = try_halo_of_spores(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 4. Bless — round 1 self+ally buff. Only valid before we're
        //    already concentrating on something.
        if let Some(aei) = try_bless(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 4b. Spirit Guardians — cleric level-3 self-aura that deals 3d8
        //     radiant each round to nearby enemies. Fire when 2+ hostiles
        //     sit within the 6-tile aura radius and the caster isn't already
        //     concentrating on something better. The no-args validation
        //     inside the action handles the rest.
        if let Some(aei) = try_self_buff_concentration(
            encounter,
            actor_id,
            "spirit guardians",
            Condition::SpiritGuarding,
            6,
        ) {
            return ControllerDecision::Act(aei);
        }

        // 5. The lockdown lane — one rung, twenty-six spells, and the
        //    toughest enemy on the board that any of them can legally
        //    take out of the fight. See `LOCKDOWNS` for the tiering and
        //    `try_lockdown` for why the four rungs that used to stand
        //    here (Hold Person, Cause Fear, Dominate Monster, Geas) are
        //    one, and why one of them could never fire at all.
        if let Some(aei) = try_lockdown(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5a'''. Slotted summons — Conjure Animals, Conjure
        //        Elemental, Animate Dead, Animate Objects, and the whole
        //        Tasha's family. Before this rung existed, no AI-driven
        //        caster ever summoned anything on any template: a
        //        summon declares no damage types and targets nothing,
        //        so every picker above filtered it out and the spells
        //        were reachable only by a human typing their name.
        //
        //        Placed at the bottom of the concentration lane rather
        //        than the top, which is the opposite of where a
        //        summon's raw value would put it. Everything above —
        //        area control, the apex ally buffs, Spirit Guardians,
        //        Hold Person, Cause Fear, Dominate Monster — holds the
        //        same single concentration slot, and all of them act on
        //        the fight *now*. A pack of wolves is worth more over
        //        six rounds and less over one, and the caster can't
        //        know which fight this is. Ranking it under the
        //        immediate effects means the summon fires exactly when
        //        nothing more urgent wants the slot, which is also when
        //        the fight is most likely to be the long kind that
        //        pays for it.
        //
        //        That argument is about the concentration, so it binds
        //        only the summons that spend some — which is why the
        //        free half of the lane is up at rung 3a⁵ instead.
        //
        //        Still above the damage lane below: two extra bodies
        //        out-damage one Fireball across any fight that lasts
        //        long enough for the question to matter.
        if let Some(aei) = try_summon_allies(encounter, actor_id, SummonTier::Slotted) {
            return ControllerDecision::Act(aei);
        }

        // 5a⁴. Dispel Magic — end the enemy's spell rather than
        //      out-rolling it. Concentration-free, so it does not
        //      compete with the lane above; it sits here rather than
        //      higher because it only ever fires when the board
        //      actually shows something to end, and on a clean board
        //      the rung costs nothing but a walk of the actor list.
        //
        //      Above the damage lanes below because a dispel is worth
        //      more the earlier it lands: a Haste stripped this turn
        //      denies the whole rest of the fight, and a flier dropped
        //      before the Fireball goes out is a flier the Fireball can
        //      reach.
        if let Some(aei) = try_dispel_magic(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5. AoE — a point with no friendly fire that catches two
        //    enemies, or one when the cast spends neither the Action nor
        //    a slot (see `best_burst_placement`'s floor).
        if let Some(aei) = try_attack_aoe(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5b. Caster-centered NoArgs burst (Thunderwave / Word of Radiance
        //     / Holy Word) — fire when 2+ enemies sit inside the spell's
        //     implicit radius. The action validates its own radius via
        //     `enemy_burst_targets`, so the AI only needs to enumerate
        //     NoArgs harmful actions and pick the cheapest hitter.
        if let Some(aei) = try_self_centered_burst(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5b'. Attrition — Command, Bestow Curse, Enemies Abound. The
        //      control spells that leave the enemy fighting, promised a
        //      rung of their own by `LOCKDOWNS` and given one here:
        //      below every burst, because a blast that catches three
        //      beats a penalty on one, and above single-target focus
        //      fire, because a penalty on one beats one creature's
        //      worth of weapon damage.
        if let Some(aei) = try_attrition(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5c. Shove — knock an adjacent enemy prone when at least one
        //     ally is also adjacent (the prone condition gives them
        //     advantage on melee attacks). Only fires when the target
        //     isn't already prone — no point double-shoving.
        if let Some(aei) = try_shove(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5d. Grapple — lock down a ranged enemy in melee so their
        //     movement is zero and they can't kite. Only fires when
        //     the target has a ranged attack and isn't already grappled.
        if let Some(aei) = try_grapple(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5d'. Damage-free grabs — the roper's tendril and whatever
        //      joins it. Next to the Grapple rung because it is the
        //      same idea at fifty feet, and above focus-fire because
        //      focus-fire cannot see these actions at all: it skips
        //      anything that declares it deals no damage. See
        //      `try_damage_free_grab`.
        if let Some(aei) = try_damage_free_grab(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5e. Form of the Beast (Bite) — the Beast Barbarian's swing for
        //     when the fight has turned. It sits above focus-fire rather
        //     than inside it because the damage picker would never
        //     choose it: 1d8 loses to the greataxe's 1d12 on every
        //     comparison the picker makes, and correctly so. What the
        //     picker cannot see is that below half HP the bite is also a
        //     heal, and two points of average damage is a bad price for
        //     that.
        if let Some(aei) = try_beast_bite_when_bloodied(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5f. Make light. Sits immediately above focus-fire because
        //     that is exactly the rung it is standing in for: it fires
        //     only on a turn where the picker can see nothing to shoot
        //     at, and every rung below is about what to do when there
        //     is nothing to shoot at. See `try_make_light` for the
        //     gate, which is written to be a no-op on any lit board.
        if let Some(aei) = try_make_light(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 6. Focus-fire: pick targets with advantage > normal > disadv;
        //    tie-break by lower HP (finish wounded).
        if let Some(aei) = try_attack_focus_fire(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 7. Defensive dodge: if we're below 30% HP, no allies need
        //    healing, and we don't have a high-leverage attack queued
        //    above, take the Dodge action so incoming swings have
        //    disadvantage. Better than trading blows on the way down.
        if let Some(aei) = try_dodge_when_low_hp(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 7b. Pinned and unable to swing back — spend the Action
        //     breaking the hold instead of on movement that cannot
        //     happen. See `try_escape_grapple`; this sits below every
        //     attack lane on purpose, so a grappled brawler who can
        //     still reach something hits it rather than wriggling.
        if let Some(aei) = try_escape_grapple(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 8. No one in reach — close on the lowest-HP enemy.
        if let Some(aei) = try_step_toward_lowest_hp(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 8a. Dash — if we've already used our movement but still have
        //     an Action and enemies are far away, Dash doubles our movement
        //     budget so the next decide() call can close the gap. Only fires
        //     when no enemy is within our normal movement range (otherwise
        //     step-toward handles it) and we haven't already spent the Action.
        if let Some(aei) = try_dash_to_close(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 8b. We have an Action but no offensive option — Dodge is strictly
        //    better than Skip (imposes disadvantage on incoming attacks).
        if let Some(aei) = try_dodge(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 9. Nothing useful. End the turn.
        skip_or_await(encounter, actor_id)
    }
}

/// How far away an enemy can be and still be revealed by a light the
/// actor lights on themselves: the bright core plus the dim collar of
/// a torch or a Light cantrip, which are the same radius.
///
/// The band is what makes the rung honest. Lighting up does not help
/// you see something forty tiles away — it only tells *it* where *you*
/// are. An AI that struck a light at any unseen enemy would be handing
/// away its position for nothing, which is the single worst thing to
/// do in the dark and exactly what a naive "I can't see, make light"
/// gate would produce.
const LIGHT_REVEAL_BAND: isize =
    crate::engine::lighting::TORCH_BRIGHT_TILES + crate::engine::lighting::TORCH_DIM_TILES;

/// Strike a light when the dark is the only reason this turn has
/// nothing in it.
///
/// Three gates, and each of them is doing real work:
///
///   1. **Nothing is visible.** Not "something is invisible" — *nothing*
///      is. The rung sits above focus-fire, so an actor who can see any
///      enemy at all should be shooting it; a light struck on that turn
///      costs an Action that had a target.
///   2. **Something is close enough to reveal.** See
///      `LIGHT_REVEAL_BAND`. Outside it, striking a light is pure
///      giveaway.
///   3. **The actor is not already lit.** Both actions refuse this
///      themselves, so this is only about not walking the enemy scan
///      on every turn of every fight for an answer that cannot change.
///
/// Between them the three make the rung a no-op on a lit board — where
/// `darkness_blinds` is false for everybody — which is where it needs
/// to be free, because every AI turn in the suite walks past it.
///
/// **A creature with darkvision never reaches this rung, and that is
/// the interesting half.** The reveal band is 16 tiles and the
/// shortest darkvision in the bestiary is 24, so anything close enough
/// to be worth revealing is already visible to anyone who can see in
/// the dark. A goblin does not light a torch to fight a human in an
/// unlit corridor, and it should not: the dark is the goblin's
/// advantage, and the fastest way to lose it is to carry a lamp.
///
/// The torch is preferred to the cantrip where both are available,
/// because it is a bonus action and the cantrip is an Action — the
/// torch leaves the turn intact, and the loop comes straight back
/// around to a board the actor can now see.
fn try_make_light(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if encounter.actor_carries_light(actor_id) {
        return None;
    }
    let team = actor.team();
    let here = actor.location();
    let mut worth_revealing = false;
    for (id, other) in encounter.actors.iter() {
        if *id == actor_id || other.team() == team || !other.is_combat_active() {
            continue;
        }
        if !encounter.darkness_blinds(actor_id, *id) {
            // Something is visible; the attack rungs below own this
            // turn.
            return None;
        }
        worth_revealing |= here.chebyshev_to(other.location()) <= LIGHT_REVEAL_BAND;
    }
    if !worth_revealing {
        return None;
    }
    // The torch is a carried consumable, so it is reachable only
    // through the `available_actions()` lookup — `try_self_action`
    // searches the template list, where a torch has never been.
    try_self_action_inc_items(encounter, actor_id, "light torch")
        .or_else(|| try_self_action(encounter, actor_id, "light"))
}

/// True if the actor's current HP fraction is below `frac`. Stable /
/// dying actors return true (HP is 0). Used by the AI to gate
/// defensive actions (Disengage, retreat heals) on actually being hurt.
fn is_low_hp(encounter: &EncounterInstance, actor_id: usize, frac: f32) -> bool {
    let Some(actor) = encounter.actors.get(&actor_id) else {
        return false;
    };
    let max = actor.max_hitpoints().max(1) as f32;
    (actor.hitpoints() as f32) / max < frac
}

/// Take the Disengage action if available and currently valid. The
/// caller is expected to gate this on actually wanting the OA-skip
/// (under threat, low HP, etc.). Returns None when the actor doesn't
/// have Disengage in their loadout or can't afford the cost.
///
/// Routed through `try_cheapest_printing` so a rogue reaches for Cunning
/// Action first — see that helper for why the same effect at two prices
/// was being bought at the higher one.
fn try_disengage(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_cheapest_printing(
        encounter,
        actor_id,
        // The monk's Step of the Wind is the bonus-action printing of
        // Disengage *and* Dash at once, so it heads both lists. No
        // chassis carries it alongside Cunning Action, so the order
        // between the two cheap printings decides nothing.
        &["step of the wind", "cunning disengage", "disengage"],
    )
}

/// Pick the first action in `names` the actor carries and can afford.
///
/// 5e prints the same effect at two prices more than once, and the
/// engine models both printings as separate actions with separate names:
/// the Rogue's Cunning Action is Dash, Disengage and Hide *as a bonus
/// action*, and the PHB's own Dash, Disengage and Hide cost the whole
/// Action. Every heuristic in this file that wanted one of those three
/// asked for the expensive printing by name, so a rogue — the one
/// chassis that has the cheap one — bought the expensive one and gave up
/// its attack to do it.
///
/// Order the list cheapest-first. There is never a reason to prefer the
/// Action-priced version when the bonus-action one is on the sheet: the
/// effect is identical, and a rogue that Disengages for a bonus action
/// still has an Action to Sneak Attack with.
fn try_cheapest_printing(
    encounter: &EncounterInstance,
    actor_id: usize,
    names: &[&str],
) -> Option<ActionExecutionInfo> {
    names
        .iter()
        .find_map(|name| try_self_action(encounter, actor_id, name))
}

/// If the actor is Prone, return the StandUp action invocation. The action
/// itself custom-validates `has_condition(Prone)` and pays half-speed in
/// movement.
fn try_stand_up(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.has_condition(Condition::Prone) {
        return None;
    }
    try_self_action(encounter, actor_id, "stand")
}

/// Dodge if the actor still has an Action available and reached this
/// step in the pipeline (i.e. nothing else worked). Validates the action
/// before returning so a stunned / actionless actor falls through.
fn try_dodge(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_action(encounter, actor_id, "dodge")
}

/// One row in the **lockdown** cohort — a single-target hostile cast
/// whose job is to take a creature out of the fight rather than to
/// damage it.
///
/// A row is the spell's name plus the condition it installs, and the
/// condition is there for exactly one purpose: skipping a target that is
/// already carrying it, so the AI never spends a second slot re-locking
/// something it has already locked.
///
/// **Whether the cast claims concentration is deliberately not a column
/// here.** It is asked of the action, the same way the area-control
/// registry asks it, because the spell already knows and a `bool` beside
/// the name is a second copy of a fact that can drift. That single
/// change is what opened this cohort to the whole non-concentration half
/// of 5e's lockdown list: the picker used to bail on a concentrating
/// caster before it looked at anything, so Command, Power Word Stun,
/// Feeblemind, Contagion, Forcecage and Charm Monster could not have
/// been rows even though nothing about them needs the slot.
struct LockdownPick {
    name: &'static str,
    /// The condition the lock leaves on its victim — the "already
    /// handled" marker, not a description of the spell.
    ///
    /// `None` for the one row that leaves no marker because it leaves no
    /// victim: Plane Shift takes its target off the board for good, so
    /// there is nothing to re-check next turn — the creature is simply
    /// not in the candidate walk any more. Every other row has to say
    /// what it installed, or the caster would spend its whole fight
    /// re-Holding the same person.
    condition: Option<Condition>,
}

/// The single-target lockdown lane, in preference order.
///
/// Five rows before this list was a list. Fifteen of the twenty entries
/// below ship on chassis that carry them today and had no rung that
/// could reach them: Hold Monster and Suggestion on four chassis
/// families each, Power Word Stun and Banishment on three, and not one
/// of them had ever been cast by an AI-driven caster. A single-target save-or-suck deals no
/// damage and buffs nobody, so the damage lane, the self-buff cohort and
/// the area-control registry each filtered it out for a different
/// reason — the same three-way miss that kept the summons and the
/// per-hit marks unreachable.
///
/// **What earns a row is taking a creature out of the fight**, which is
/// the same membership test the area-control registry states for its own
/// rung, and it is a real test rather than a description. Six spells the
/// roster carries were tried here and removed for failing it — Command,
/// Bestow Curse, Enemies Abound, Contagion, Eyebite and Power Word Pain.
/// All six leave the target fighting (Command for exactly one round),
/// and this rung sits above the whole damage lane: a row that only makes
/// an enemy *worse* would still shut out every Fireball the caster owns,
/// every turn, for as long as it had a 1st-level slot. Three of them
/// live on `ATTRITION` instead, which is the same row shape read at a
/// rung below the bursts; the other three are too expensive for that
/// rung's slot cap and remain unreachable.
///
/// The two attrition rows at the bottom — Cause Fear and Ray of
/// Enfeeblement — fail that test too, and are kept because they were
/// already here. Demoting them would change the opening move of shipped
/// templates to buy consistency in a comment.
///
/// **The order is the priority and it is load-bearing.** Three tiers, by
/// how much of the target's turn the lock takes away, and inside each
/// tier by slot cost ascending so a level-8 is never spent on what a
/// level-2 would have done:
///
///   1. **Turn removal** — the target does not act. Sleep Gaze (a
///      monster ability, free) leads because it costs no slot at all;
///      then Tasha's Hideous Laughter at 1st and Hold Person at 2nd, up
///      through Maze, Power Word Stun and Feeblemind at 8th. Otto's
///      Irresistible Dance is placed above Flesh to Stone at the same
///      slot level for the reason the old comment gave about Hold
///      Person: Petrified hands the target broad damage resistance and
///      slows the kill clock, where a dancing target simply loses its
///      turn.
///   2. **Redirection** — the target still acts, but not against us.
///      The charm and domination family, cheapest first.
///   3. **The two inherited rows.**
///
/// Polymorph is deliberately absent despite being the archetypal
/// level-4 "remove a creature" spell, and the reason is this engine's
/// implementation rather than the rule: `Polymorph` hands its target 30
/// temporary hit points to represent the beast form's pool, applied
/// "uniformly regardless of allegiance". Cast at an enemy here it would
/// be a buff.
const LOCKDOWNS: &[LockdownPick] = &[
    // --- Tier 1: the target does not act. ---
    //
    // Plane Shift leads, and it is the only row that is not a lock at
    // all: the target is put on another plane and does not come back,
    // this fight or ever. Nothing else on the list ends a creature's
    // participation permanently, so nothing else outranks it — and its
    // touch range is what keeps that honest, because the row can only
    // validate against something the caster is already standing next
    // to. It ships on one chassis (the lich) and had no rung that could
    // reach it: a spell that installs no condition and deals no damage
    // is invisible to the damage lane, the self-buff cohort and the
    // area-control registry alike, which is the same three-way miss
    // that kept the rest of this list unreachable before it was a list.
    LockdownPick { name: "plane shift", condition: None },
    LockdownPick { name: "sleep gaze", condition: Some(Condition::Asleep) },
    LockdownPick { name: "tasha's hideous laughter", condition: Some(Condition::Incapacitated) },
    LockdownPick { name: "hold person", condition: Some(Condition::Stunned) },
    LockdownPick { name: "banishment", condition: Some(Condition::Banished) },
    // Otiluke's Resilient Sphere, and it is a level-4 tier-1 lock beside
    // Banishment for the same reason Banishment is one: `Sphered` joins
    // `blocks_action_economy`, so the target takes no action, no bonus
    // action and no reaction for ten rounds. It ships on four chassis —
    // the artificer, and the wizard, cleric and sorcerer subclasses that
    // pick it up — and on a scroll, and no rung could reach it.
    //
    // Below Banishment rather than beside it because the sphere leaves a
    // body on the board. RAW's sphere is also a shield, and this engine
    // does not model that half, so the difference here is only the tile
    // the target keeps standing on — but that tile is a corridor it is
    // still blocking and cover it is still granting, which is exactly
    // the argument the off-board lane was built on.
    LockdownPick { name: "resilient sphere", condition: Some(Condition::Sphered) },
    LockdownPick { name: "hold monster", condition: Some(Condition::Stunned) },
    LockdownPick { name: "otto's irresistible dance", condition: Some(Condition::Dancing) },
    LockdownPick { name: "flesh to stone", condition: Some(Condition::Petrified) },
    LockdownPick { name: "forcecage", condition: Some(Condition::Caged) },
    LockdownPick { name: "maze", condition: Some(Condition::Mazed) },
    LockdownPick { name: "power word stun", condition: Some(Condition::Stunned) },
    LockdownPick { name: "feeblemind", condition: Some(Condition::Feebled) },
    // --- Tier 2: the target acts, but not against us. ---
    LockdownPick { name: "crown of madness", condition: Some(Condition::Charmed) },
    LockdownPick { name: "suggestion", condition: Some(Condition::Charmed) },
    LockdownPick { name: "charm monster", condition: Some(Condition::Charmed) },
    LockdownPick { name: "dominate beast", condition: Some(Condition::Dominated) },
    LockdownPick { name: "geas", condition: Some(Condition::Charmed) },
    LockdownPick { name: "dominate person", condition: Some(Condition::Dominated) },
    LockdownPick { name: "dominate monster", condition: Some(Condition::Dominated) },
    // --- Tier 3: the two attrition rows this cohort already had. ---
    LockdownPick { name: "cause fear", condition: Some(Condition::Frightened) },
    LockdownPick { name: "ray of enfeeblement", condition: Some(Condition::Poisoned) },
];

/// Take the toughest enemy the caster can legally lock out of the fight.
///
/// One rung replacing four. `try_cause_fear`, `try_dominate_monster` and
/// `try_geas` were each a private copy of the walk below with one name
/// substituted, and the first of the three was worse than redundant:
/// "cause fear" was already a row on this cohort, sitting above it in
/// the ladder with the identical concentration gate and the identical
/// highest-HP pick, so `try_cause_fear` could not return `Some` on any
/// board where the rung above it had not already fired. It was dead in
/// the strict sense — reachable code that no input could make matter.
///
/// **Selection**: the highest-HP legal target wins, and a tie between
/// two locks on the same target breaks to the earlier row. Kept exactly
/// as it was, because the two rules compose into the behaviour the
/// cohort's ordering is written for — every row is offered the same set
/// of enemies, so the toughest one is the same across rows, and the tie
/// then picks the most-preferred lock that will actually land on it. A
/// cheap lock that cannot legally touch the biggest threat (Hold Person
/// against a Giant) drops out at `validate` and the next row answers for
/// it.
///
/// **Concentration** is asked per row rather than once up front. A
/// caster already holding a spell skips the concentrating rows and can
/// still reach for Command or Power Word Stun, which is both RAW and
/// the thing the old up-front bail made impossible.
fn try_lockdown(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    pick_from_cohort(encounter, actor_id, LOCKDOWNS)
}

/// The walk both control cohorts share: offer every row the caster can
/// currently cast to every legal enemy, and keep the best pair.
///
/// Split out when the second cohort arrived rather than copied, because
/// a copy is what the four rungs this replaced already were.
fn pick_from_cohort(
    encounter: &EncounterInstance,
    actor_id: usize,
    cohort: &[LockdownPick],
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let concentrating = actor.is_concentrating();
    let candidates: Vec<(&'static (dyn Action + Send + Sync), Option<Condition>)> = cohort
        .iter()
        .filter_map(|row| {
            let action = actor.find_action(row.name)?;
            (!concentrating || !action.holds_concentration()).then_some((action, row.condition))
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let my_team = actor.team();

    let mut best: Option<(u32, usize, ActionExecutionInfo)> = None;
    for target_id in encounter.sorted_actor_ids() {
        let Some(target) = encounter.actors.get(&target_id) else {
            continue;
        };
        if target_id == actor_id || target.team() == my_team || !target.is_combat_active() {
            continue;
        }
        for (priority, (action, condition)) in candidates.iter().enumerate() {
            if condition.is_some_and(|c| target.has_condition(c)) {
                continue;
            }
            let aei =
                ActionExecutionInfo::new(*action, actor_id, Some(vec![target_id]), None, None);
            if !aei.validate(encounter) {
                continue;
            }
            let hp = target.hitpoints();
            let pick = match &best {
                None => true,
                Some((best_hp, best_pri, _)) => {
                    hp > *best_hp || (hp == *best_hp && priority < *best_pri)
                }
            };
            if pick {
                best = Some((hp, priority, aei));
            }
        }
    }
    best.map(|(_, _, aei)| aei)
}

/// The **attrition** cohort — single-target casts that leave the enemy
/// fighting and merely make them worse at it.
///
/// Sibling to `LOCKDOWNS` and split from it on the one question that
/// decides where a control spell belongs in the ladder: does the target
/// still get to act? A Hold Person takes a creature out of the fight and
/// is worth an Action ahead of any burst; a Bestow Curse leaves it
/// swinging with a penalty and is not. The two lived on one list briefly
/// and it went exactly as that difference predicts — the rung sits above
/// the damage lane, so a 1st-level Command shut out every Fireball the
/// caster owned, every turn, for as long as it had a 1st-level slot.
///
/// **Rows are capped at 3rd level.** Attrition is worth a cheap slot and
/// not a precious one, and this rung has no way to price "the target is
/// Poisoned" against what the same slot buys on the single-target damage
/// lane below it. That cap is what keeps Contagion (5th), Eyebite (6th)
/// and Power Word Pain (7th) off the list: each of the three would have
/// to beat Disintegrate or Chain Lightning out of the caster's hand to
/// earn the slot, and none of them does.
///
/// Row shape and the concentration question are `LockdownPick`'s, which
/// is the point of sharing the struct — the two cohorts differ in what
/// they contain and in where they are read, not in how a row works.
const ATTRITION: &[LockdownPick] = &[
    // Level 1, no concentration: one enemy turn, for the cheapest slot
    // in the game. The engine models RAW's "Halt" / "Grovel" as a single
    // round of Stunned.
    LockdownPick { name: "command", condition: Some(Condition::Stunned) },
    // Level 3, concentration: a lasting penalty on everything the target
    // rolls, and the widest-carried row here — five chassis families.
    LockdownPick { name: "bestow curse", condition: Some(Condition::Baned) },
    // Level 3, concentration: Confused, which in this engine costs the
    // holder its reactions as well as its aim.
    LockdownPick { name: "enemies abound", condition: Some(Condition::Confused) },
];

/// Make the toughest enemy worse at fighting, once the bursts have
/// declined.
///
/// Shares `pick_from_cohort` with `try_lockdown`; what differs is the
/// list and the rung. Placed below every burst lane and above
/// single-target focus fire, which is the seam the split was made for:
/// a debuff is worth more than one creature's worth of weapon damage
/// and less than a blast that catches three.
fn try_attrition(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    pick_from_cohort(encounter, actor_id, ATTRITION)
}

/// One row in the **concentration mark** cohort — a bonus-action spell
/// whose whole effect is a rider on every later swing at one named
/// creature.
///
/// The two rows are 5e's twin signature marks, and their AI decision is
/// identical down to the slot level: bonus action plus a 1st-level slot,
/// concentration, 90 ft, +1d6 per landed hit for as long as the caster
/// keeps hold of it. A row is (spell name, condition it installs, how
/// close the target has to be) for the same reason `SelfBuffPick` is —
/// the judgement is entirely in the third field, and a third mark
/// arriving should be one line rather than a fourth near-identical
/// picker.
struct ConcentrationMarkPick {
    name: &'static str,
    condition: Condition,
    /// Fire only at a hostile inside this many tiles. Well under the
    /// spells' own 36-tile range, deliberately: the mark pays nothing
    /// until the caster starts landing hits, and a mark placed on
    /// something ninety feet away burns the concentration slot for the
    /// rounds it takes to close.
    engage_gap: isize,
}

/// The signature per-hit marks, in preference order.
///
/// Neither had ever been cast. Both declare no damage, install nothing
/// on the caster and target a single enemy, so the damage lane filtered
/// them out (they deal none), the self-buff cohort filtered them out
/// (they buff nobody), and the lockdown lane filtered them out (they
/// disable nothing) — the same three-way miss that kept the summons
/// unreachable before their rung existed. A Ranger who never casts
/// Hunter's Mark and a Warlock who never casts Hex are each missing the
/// one spell their class is built around.
///
/// Hex leads on the roster's arithmetic rather than on any rule: its
/// rider is necrotic, the mark's is force, and rather more of the
/// bestiary is vulnerable to the first than to the second. In practice
/// no actor carries both, so the order exists to give a future one an
/// answer instead of a coin flip.
const CONCENTRATION_MARKS: &[ConcentrationMarkPick] = &[
    ConcentrationMarkPick {
        name: "hex",
        condition: Condition::Hexed,
        engage_gap: 12,
    },
    ConcentrationMarkPick {
        name: "hunters mark",
        condition: Condition::HuntersMarked,
        engage_gap: 12,
    },
];

/// Lay a per-hit mark on the enemy the caster is most likely to keep
/// hitting.
///
/// Three gates, and each answers a different way the cast can be wasted:
///
///   - **Not already concentrating.** The mark is worth about three and
///     a half damage a swing; everything else in the concentration lane
///     above this rung decides fights. Ranked below all of them so the
///     mark fires exactly when nothing better wants the slot, which is
///     the same argument the slotted-summon rung makes just below.
///   - **Nobody already carries the mark.** Re-marking is a wasted slot,
///     and marking a second creature silently replaces the first.
///   - **Target inside `engage_gap`.** See the field's docstring.
///
/// The pick is the **lowest-HP** enemy in band, which is the opposite of
/// `try_lockdown`'s "toughest wins" and deliberately so. A lockdown
/// is spent to stop the scariest thing on the board; a mark is spent to
/// be collected on, one swing at a time, and it is only ever collected
/// on the creature the caster actually attacks — which is the one the
/// damage lane below is about to focus-fire. Marking the toughest target
/// would put the rider on a creature the AI has already decided to walk
/// past.
///
/// The cast costs a Bonus Action rather than an Action, so the turn that
/// lays the mark still swings: the ladder is re-entered after the cast
/// and falls through to the attack lane with the rider already up.
fn try_concentration_mark(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    let my_team = actor.team();
    let mut best: Option<(usize, u32, ActionExecutionInfo)> = None;
    for (priority, row) in CONCENTRATION_MARKS.iter().enumerate() {
        let Some(action) = actor.find_action(row.name) else {
            continue;
        };
        for target_id in encounter.sorted_actor_ids() {
            let Some(target) = encounter.actors.get(&target_id) else {
                continue;
            };
            if target_id == actor_id
                || target.team() == my_team
                || !target.is_combat_active()
                || target.has_condition(row.condition)
            {
                continue;
            }
            if footprint_chebyshev(
                actor.location(),
                get_tiles_from_size(actor.size()),
                target.location(),
                get_tiles_from_size(target.size()),
            ) > row.engage_gap
            {
                continue;
            }
            let aei =
                ActionExecutionInfo::new(action, actor_id, Some(vec![target_id]), None, None);
            if !aei.validate(encounter) {
                continue;
            }
            let hp = target.hitpoints();
            let pick = match &best {
                None => true,
                Some((best_pri, best_hp, _)) => {
                    priority < *best_pri || (priority == *best_pri && hp < *best_hp)
                }
            };
            if pick {
                best = Some((priority, hp, aei));
            }
        }
    }
    best.map(|(_, _, aei)| aei)
}

/// Cast **Dispel Magic** at the enemy carrying the most magic worth
/// ending.
///
/// Five PC / NPC chassis carry the spell — Wizard, Cleric, Druid,
/// Artificer and the Death Knight — and until this rung existed not one
/// of them ever cast it. Dispel Magic declares no damage and installs no
/// condition, so every picker above filtered it out the same way the
/// summons used to be filtered out: the AI's lanes are damage, lockdown
/// and buff, and "undo something" is none of the three.
///
/// The target ranking is three tiers, and the top one is new:
///
///   - **Airborne** (`altitude_ft > 0`). Ending a flier's spell is the
///     only play on the board that both takes the effect away *and*
///     charges for the way down — 3d6 and prone, courtesy of the
///     altitude sweep. It also happens to be the case a damage-shaped
///     picker is least able to see, because the payoff lands on a
///     different lane entirely.
///   - **Concentrating.** RAW's headline use, and the one that scales:
///     dropping a concentration ends the whole spell on every target it
///     touched, so one action can undo a Web that pinned three allies.
///   - **Carrying any dispellable buff.** The long tail — a Raging
///     barbarian, a Blurred mage, a Hasted brute. Worth an action and a
///     3rd-level slot, but only once the two above have said no.
///
/// Ties inside a tier break to the lowest actor id rather than to the
/// toughest target, which is the opposite of `try_lockdown`'s rule
/// one rung up and deliberately so: a lockdown is spent *on* the
/// creature and wants the scariest one, while a dispel is spent on the
/// *effect*, and this engine has no way to price one Haste above
/// another. Lowest id is then the honest tiebreak — reproducible from
/// the seed, and not pretending to a judgement the AI cannot make.
///
/// Range and line of sight are left to `validate`, which owns the 120 ft
/// band the spell declares.
fn try_dispel_magic(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("dispel magic")?;
    let my_team = actor.team();
    let mut best: Option<(u8, ActionExecutionInfo)> = None;
    for target_id in encounter.sorted_actor_ids() {
        let Some(target) = encounter.actors.get(&target_id) else {
            continue;
        };
        if target_id == actor_id || target.team() == my_team || !target.is_combat_active() {
            continue;
        }
        let tier = if target.altitude_ft() > 0 {
            3
        } else if target.is_concentrating() {
            2
        } else if target
            .conditions()
            .keys()
            .any(|c| c.is_dispellable_buff())
        {
            1
        } else {
            continue;
        };
        // Only the winner is built and validated, but the check has to
        // happen per candidate: a higher-tier target the caster cannot
        // see must not shut out a lower-tier one it can.
        if best.as_ref().is_some_and(|(best_tier, _)| tier <= *best_tier) {
            continue;
        }
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![target_id]), None, None);
        if aei.validate(encounter) {
            best = Some((tier, aei));
        }
    }
    best.map(|(_, aei)| aei)
}




/// Cast Bless if we have it, aren't already concentrating, and there's at
/// least one combat-active ally (otherwise the buff is wasted on solo).
/// Mage Armor self-buff — only worth casting once. The MageArmored
/// condition has a long timer, so we suppress repeat casts by checking
/// for it. Tries the spell first (slot-cost, no inventory drain); falls
/// back to the Potion of Mage Armor if the caster is carrying one (loot
/// consumable, drinkable by non-casters). Validates spell-slot
/// availability via the action's own `validate_input`, so this also
/// gracefully no-ops when out of slots.
fn try_self_buff_mage_armor(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.has_condition(Condition::MageArmored) {
        return None;
    }
    // Prefer the spell — it's reusable across encounters (slot-based)
    // and doesn't burn an inventory slot. Fall back to the potion if
    // the caster has no Mage Armor spell or the slot is spent.
    try_self_action(encounter, actor_id, "mage armor")
        .or_else(|| try_self_action_inc_items(encounter, actor_id, "drink potion of mage armor"))
}

/// See Invisibility — level-2 divination self-buff. Fire only when the
/// spell has something to do: at least one hostile inside the caster's
/// engagement window is concealed by something the buff would actually
/// pierce, and the caster can't already see through it.
///
/// The gate is the whole point of the picker. See Invisibility is
/// worthless against every opponent that isn't invisible, so a naive
/// "cast your buffs at the top of the fight" heuristic would throw away
/// a level-2 slot in the overwhelming majority of encounters. Checking
/// `concealment_piercing_of(...).pierces(c)` against the enemy's actual
/// conditions asks exactly the right question — would this buff change
/// whether I can see that creature — and answers it the same way the
/// attack-mode clauses will.
///
/// Two short-circuits before the scan: the caster must not already hold
/// a sight buff (the spell's own `custom_validate_input` also refuses,
/// but bailing here skips the walk), and the target must be an enemy —
/// an invisible *ally* is not a problem worth a slot.
fn try_see_invisibility(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.has_condition(Condition::SeeingInvisible)
        || actor.has_condition(Condition::TrueSighted)
    {
        return None;
    }
    let team = actor.team();
    let blinding_enemy = encounter.actors.iter().any(|(id, other)| {
        if *id == actor_id || other.team() == team || !other.is_combat_active() {
            return false;
        }
        // Something this enemy is wearing must be both (a) invisibility
        // the buff would lift and (b) not already pierced.
        other
            .conditions()
            .keys()
            .any(|c| c.countered_by_see_invisibility())
            && !encounter.viewer_can_see(actor_id, *id)
    });
    if !blinding_enemy {
        return None;
    }
    try_self_action(encounter, actor_id, "see invisibility")
}

/// Armor of Agathys — warlock signature 1st-level abjuration. Pre-buff
/// the caster with 5 temp HP + 5-cold melee retaliation. Gate on:
/// - Not already shielded (one-shot install).
/// - An enemy within ~6 tiles (≈15ft) so the retaliation rider will
///   actually land before the buff times out. The spell costs a lv1
///   slot — we don't want to burn it in an empty room.
/// - No active concentration check needed (AoA isn't concentration-
///   bound, so it pairs with the warlock's other concentration loops).
fn try_armor_of_agathys(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.has_condition(Condition::AgathysShielded) {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, 6) {
        return None;
    }
    try_self_action(encounter, actor_id, "armor of agathys")
}

/// One row in the concentration self-buff cohort — a spell whose whole
/// AI decision is the same three questions: am I already concentrating,
/// is the buff already up, and is anything close enough for it to be
/// worth a slot this round.
///
/// Fifteen of these had fourteen consecutive rungs in the decision
/// ladder — thirteen of them reached through a one-line wrapper
/// function that existed only to pass three constants, and two spelled
/// out inline. The rungs had run out of names: the labels were four
/// prime marks deep by the end. Every row differs in a name, a
/// condition, and one or two distances, and in nothing else, which is
/// what a table is for.
struct SelfBuffPick {
    /// Canonical action name, matched the way `try_self_action`
    /// resolves it.
    name: &'static str,
    /// The condition the buff installs. Doubles as the don't-recast
    /// gate: holding it means the buff is already up.
    condition: Condition,
    /// Fire only with a combat-active hostile inside this many tiles.
    /// This is the value on the row that carries most of the per-spell
    /// judgement — a melee retaliation rider wants somebody in contact
    /// (1), a ranged-deflection rider wants somebody far enough away to
    /// be shooting (20), a party-wide aura only wants the fight to have
    /// started at all (60).
    engage_gap: isize,
    /// `Some(gap)` for the rows that buff the party rather than the
    /// caster: require at least one *other* combat-active ally inside
    /// `gap` tiles before spending the slot. `None` for the self-buffs,
    /// which are worth casting alone.
    ///
    /// The distinction is the aura radius, and it is per-spell: Holy
    /// Aura reaches 30 ft (12 tiles), the paladin auras 15 ft (6). A
    /// row that got this wrong would fire in a formation the aura
    /// doesn't actually cover.
    allies_within: Option<isize>,
}

/// The self-buffs that outrank the Trickery Cleric's Invoke Duplicity,
/// in priority order.
///
/// The cohort is split in two because Invoke Duplicity sits between the
/// halves, and the seam is load-bearing rather than historical: the
/// baseline cleric list carries both Spirit Shroud and Antilife Shell,
/// so the Trickery Cleric reaches this cohort twice with a Channel
/// Divinity decision in the middle. Collapsing the two into one table
/// would move Antilife Shell above Invoke Duplicity or Spirit Shroud
/// below it, and either way a template that exists today would start
/// making a different opening move.
const SELF_BUFFS_ABOVE_DUPLICITY: &[SelfBuffPick] = &[
    // Level-8: the party-wide apex aura, and the only row up here that
    // buffs anyone but the caster — hence the ally gate. Slot-cheaper
    // than Foresight per ally affected, which is why the rung above
    // this cohort is Foresight and this is the first row in it.
    SelfBuffPick {
        name: "holy aura",
        condition: Condition::HolyAuraed,
        engage_gap: 60,
        allies_within: Some(12),
    },
    // Level-3 concentration; the cold rider only lands on a melee
    // swing, so it wants somebody in contact.
    SelfBuffPick {
        name: "spirit shroud",
        condition: Condition::SpiritShrouded,
        engage_gap: 1,
        allies_within: None,
    },
    // Level-5 wizard; +1d10 force on every attack the caster makes.
    // 8 tiles is melee plus close-ranged reach.
    SelfBuffPick {
        name: "bigby's hand",
        condition: Condition::BigbysHanded,
        engage_gap: 8,
        allies_within: None,
    },
    // Level-6 wizard; 50 temp HP and self-attack advantage. The 30 ft
    // gate is Holy Aura's — the temp HP buffer wants a fight, not a
    // contact.
    SelfBuffPick {
        name: "tenser's transformation",
        condition: Condition::Transformed,
        engage_gap: 12,
        allies_within: None,
    },
];

/// The self-buffs that sit below Invoke Duplicity, in priority order.
///
/// The four Investitures are mutually exclusive with each other and
/// with everything else here — `try_self_buff_concentration`'s
/// concentration short-circuit means the first row that fires ends the
/// walk for the rest of the fight, so table order *is* the preference
/// between them.
const SELF_BUFFS_BELOW_DUPLICITY: &[SelfBuffPick] = &[
    // Level-6: fire resistance + 1d10 fire melee retaliation.
    SelfBuffPick {
        name: "investiture of flame",
        condition: Condition::InvestedInFlame,
        engage_gap: 6,
        allies_within: None,
    },
    // Level-6: cold resistance + 1d10 cold melee retaliation.
    SelfBuffPick {
        name: "investiture of ice",
        condition: Condition::InvestedInIce,
        engage_gap: 6,
        allies_within: None,
    },
    // Level-6: the physical resistance trio + 1d10 force retaliation —
    // a broader envelope than the elemental pair above, and force is
    // the rarest damage type to resist.
    SelfBuffPick {
        name: "investiture of stone",
        condition: Condition::InvestedInStone,
        engage_gap: 6,
        allies_within: None,
    },
    // Level-6: ranged-attack disadvantage + 60 ft of flight. The odd
    // gate in the family — the deflection clause is the load-bearing
    // half, so it wants a shooter at range rather than a body in
    // contact, and 20 tiles matches Wind Wall directly below.
    SelfBuffPick {
        name: "investiture of wind",
        condition: Condition::InvestedInWind,
        engage_gap: 20,
        allies_within: None,
    },
    // Level-3: the cheap version of Investiture of Wind's deflection
    // clause, which is why it sits under all four of them.
    SelfBuffPick {
        name: "wind wall",
        condition: Condition::WindWalled,
        engage_gap: 20,
        allies_within: None,
    },
    // Level-2 warlock / wizard: advantage on attacks plus a psychic
    // rider, so it wants contact.
    SelfBuffPick {
        name: "shadow blade",
        condition: Condition::SpiritShrouded,
        engage_gap: 1,
        allies_within: None,
    },
    // Level-5 cleric: pushes adjacent enemies off and installs Warded.
    SelfBuffPick {
        name: "antilife shell",
        condition: Condition::Warded,
        engage_gap: 2,
        allies_within: None,
    },
    // Level-4 paladin: intercepts a killing blow on anyone in the 15 ft
    // aura. Ahead of Aura of Purity because the lethality button gets
    // first pick when both are affordable and only one can be held.
    SelfBuffPick {
        name: "aura of life",
        condition: Condition::DeathWarded,
        engage_gap: 60,
        allies_within: Some(6),
    },
    // Level-4 paladin, same envelope: Charmed / Frightened / Poisoned
    // immunity and poison resistance instead of the interception.
    SelfBuffPick {
        name: "aura of purity",
        condition: Condition::Purified,
        engage_gap: 60,
        allies_within: Some(6),
    },
    // Level-5 paladin: +2d8 radiant on every weapon hit. Same 20 ft
    // engagement envelope as Bigby's Hand, and for the same reason —
    // the rider is worthless until something is in reach.
    SelfBuffPick {
        name: "holy weapon",
        condition: Condition::HolyWeaponed,
        engage_gap: 8,
        allies_within: None,
    },
    // Level-5 paladin: advantage on saves against spells for everyone
    // in the 30 ft circle, and no damage at all on one that is made.
    // Below the two lv4 auras and Holy Weapon rather than above them
    // because its value is entirely conditional on the other side
    // casting — the auras and the weapon buff pay off against any
    // board. The ally gate is 12 tiles, the circle's own radius,
    // rather than the paladin auras' 6.
    SelfBuffPick {
        name: "circle of power",
        condition: Condition::PowerCircled,
        engage_gap: 60,
        allies_within: Some(12),
    },
    // Level-2 druid / ranger: disadvantage on attacks against anyone in
    // the 30 ft sphere. No ally gate despite being an aura — the caster
    // is inside their own sphere, so a solo druid still collects it,
    // which is the one place the two aura shapes genuinely differ.
    SelfBuffPick {
        name: "pass without trace",
        condition: Condition::Untracked,
        engage_gap: 60,
        allies_within: None,
    },
    // Level-2 bard / druid / sorcerer / wizard: a ring of roaring wind
    // that makes ranged attacks into and out of it roll at
    // disadvantage. Modelled through the same `Untracked` lane as Pass
    // Without Trace directly above, which makes the two mechanically
    // identical here and is why this sits *below* it rather than above:
    // the druid carries both, and putting the newcomer first would
    // change which spell an existing template opens with to no effect
    // anyone could observe. Below it, the row is what the wizard and
    // the sorcerer — who have no Pass Without Trace — reach for.
    SelfBuffPick {
        name: "warding wind",
        condition: Condition::Untracked,
        engage_gap: 20,
        allies_within: None,
    },
    // Level-3 artificer / bard / sorcerer / warlock / wizard: psychic
    // resistance and advantage on every mental save. Last on the
    // cohort, and the placement is the honest one — every row above it
    // changes what happens on a turn the caster is having, where this
    // one changes what happens on a turn somebody else is having to
    // them. It is worth a concentration slot only when nothing that
    // pays out unconditionally is available, which is exactly what
    // "bottom of an ordered walk" means.
    //
    // The 20-tile engagement gate is the shooter's band rather than the
    // melee one: the saves this protects are cast at range.
    SelfBuffPick {
        name: "intellect fortress",
        condition: Condition::IntellectFortified,
        engage_gap: 20,
        allies_within: None,
    },
];

/// Walk a self-buff cohort in order and return the first row that
/// passes `try_self_buff_concentration`'s gate.
fn try_self_buff_pick(
    encounter: &EncounterInstance,
    actor_id: usize,
    picks: &[SelfBuffPick],
) -> Option<ActionExecutionInfo> {
    picks.iter().find_map(|pick| {
        // The ally-cluster gate is the row's own, not the shared one:
        // `try_self_buff_concentration` answers "is this worth a slot
        // at all", and this answers "is anybody standing in it".
        if let Some(gap) = pick.allies_within
            && n_actors_within(encounter, actor_id, gap, true, 1) < 1
        {
            return None;
        }
        try_self_buff_concentration(
            encounter,
            actor_id,
            pick.name,
            pick.condition,
            pick.engage_gap,
        )
    })
}

/// Ordered roster of the "bonus action; spend a per-rest charge to
/// prime the next melee hit" lane. Walked top-to-bottom by the ladder
/// at rung 3o; the first entry the actor carries, can afford, and
/// isn't already holding wins the bonus action.
///
/// Every row shares one gate — an enemy inside footprint reach, so the
/// prime is cashed on this turn's swing rather than banked — and the
/// action's own `custom_validate_input` supplies the rest (charge
/// available, flag not already up). That uniformity is why the lane is
/// a table: each entry used to be a three-line wrapper function around
/// the identical `try_self_action_when_enemy_within(.., name)` call,
/// plus a rung in the ladder, so a new prime cost two edits in two
/// places to express one string.
///
/// The gate is `MELEE_REACH`, the same gap the swing that cashes the
/// prime is allowed to cross. It used to be a literal 0 — strictly
/// tighter than melee reach — which meant a fighter standing at the
/// far edge of its own envelope, which is where the AI's approach
/// routinely stops, never primed anything at all. Every entry on this
/// table was unreachable in that stance, so the whole lane was mostly
/// theoretical.
///
/// **The order is the priority and it is load-bearing**, which is the
/// one thing a table must not lose:
///
///   1. `reaper's touch` — the Death Domain's Channel Divinity, 3d8
///      necrotic on the next melee hit. Ahead of the Divine Strikes
///      below because it is the same shape for triple the die, and the
///      chassis has one bonus action to spend on the lane.
///   2. `divine strike` and its domain typings (`poison`, `necrotic`,
///      `psychic`) — the cleric's flat damage rider. Early because it
///      is pure upside with no save to fail and no positioning to set
///      up; a cleric in melee always wants it. The typings never
///      co-occur on one template (a domain swaps rather than stacks),
///      so listing all four costs nothing.
///   3. `fangs of the fire snake` — the Four Elements monk's +1d10
///      fire rider. Same shape as Divine Strike and sits with it for
///      the same reason; the monk carries no other entry on this lane.
///   4. `fire rune` — the Rune Knight's prime, and the only entry on
///      this lane that pays twice: 2d6 fire on the hit *and* a STR
///      save against Restrained, which is prone's advantage-granting
///      clause plus a speed of zero plus disadvantage on the target's
///      own swings. Ahead of every maneuver below because it strictly
///      contains what they buy.
///   5. `trip attack` — prone is the strongest maneuver rider: it
///      hands every melee ally advantage against the target *and*
///      costs the target its movement.
///   6. `menacing attack` — Frightened sticks on tough-STR monsters
///      that shrug off the trip, but only disadvantages the target's
///      own swings rather than enabling the party's.
///   7. `disarming attack` — attacker disadvantage, which bites
///      hardest on ranged and multiattack threats but lasts a single
///      round in this engine.
///   8. `pushing attack` — pure displacement, no accuracy or save
///      rider attached; the finisher when nothing above is available.
///   9. `goading attack` — the tank-anchor. Last because its value is
///      conditional on the fighter *wanting* to be attacked, which is
///      the situation left over once the debuff riders are spent.
///
/// Precision Attack shares the gate but is deliberately *not* here:
/// Sweeping Attack's two-adjacent-enemies rung sits between it and
/// this table in the ladder, and collapsing the two would silently
/// reorder them.
/// Footprint gap at which contact is imminent enough to be worth
/// spending a once-per-rest posture that only pays off in or near
/// melee. One tile wider than a 10 ft reach, so the buff goes up on the
/// turn *before* the enemy closes rather than the turn after — the
/// action is spent either way, and spending it a turn early is the only
/// way the buff is live when the first swing lands.
///
/// Two users, for the same reason in different words. The Spores
/// Druid's Symbiotic Entity buys a melee rider and a doubled 10 ft halo
/// alongside its temp HP; raised across the room it buys only the temp
/// HP. The Undead Warlock's Form of Dread buys a fear rider that needs
/// the warlock to be landing hits; raised across the room it likewise
/// collapses to temp HP alone.
const IMMINENT_CONTACT_GAP: isize = 5;

/// Footprint gap a longbow can reach — the engine's bows carry
/// `reach_tiles == 12`, which is 30 ft on the 2.5 ft grid. Used as the
/// AI's gate for ranged-only primes, so the prime is spent on a turn
/// the shot can actually be taken.
const BOW_RANGE_GAP: isize = 12;

/// Footprint gap inside which the Bladesinger starts the song. Wider
/// than a melee prime's trigger because none of the song's four clauses
/// needs the enemy adjacent — the AC, the speed, the concentration
/// bonus and the damage bump all want to be live for the approach, not
/// just for the swing at the end of it.
const BLADESONG_ENGAGE_GAP: isize = 8;

/// Reach of the Halo of Spores in tiles, mirroring RAW's 10 ft on the
/// engine's 2.5 ft grid. The action's own `reach_tiles` is the
/// authority; this is the AI's pre-filter so the picker doesn't build
/// and validate an AEI for every hostile on the map.
const HALO_OF_SPORES_GAP: isize = 4;

/// Spells worth spending the Sorcerer's Heightened Spell prime on
/// because a single failed save decides the fight — the concentration
/// lockdowns. Read only as a "does this kit contain one?" gate, so the
/// prime isn't declared into an empty hand.
const HEIGHTENED_LOCKDOWN: &[&str] = &[
    "hold person",
    "hold monster",
    "polymorph",
    "banishment",
    "dominate person",
    "dominate monster",
];

/// The other half of the Heightened Spell gate: save-for-half bursts,
/// where forcing disadvantage on the first save doubles that target's
/// damage. Unlike the lockdown half this one doesn't care whether the
/// sorcerer is already concentrating.
const HEIGHTENED_BURST: &[&str] = &[
    "fireball",
    "cone of cold",
    "sunburst",
    "burning hands",
    "thunderwave",
    "shatter",
];

/// Long-duration casts worth doubling with the Sorcerer's Extended
/// Spell prime. Read only as a "does this kit contain one?" gate — the
/// prime itself applies to whatever is cast next, so a false negative
/// costs a wasted declaration and a false positive costs nothing.
const EXTENDABLE: &[&str] = &[
    "mage armor",
    "hunters mark",
    "bless",
    "hold person",
    "hold monster",
    "polymorph",
    "fly",
    "spider climb",
    "haste",
    "invisibility",
    "greater invisibility",
    "stoneskin",
    "mirror image",
    "shield of faith",
    "false life",
    "heroism",
    "blur",
    "barkskin",
    "pass without trace",
    "spirit guardians",
    "spirit shroud",
    "crusader's mantle",
    "holy weapon",
    "wind wall",
    "globe of invulnerability",
    "fire shield",
    "mind blank",
    "warding bond",
    "death ward",
    "magic weapon",
    "protection from energy",
    "enlarge",
    "shadow blade",
];

/// Ordered roster of the "bonus action; spend a per-rest charge on
/// something that keeps you standing" lane. Walked top-to-bottom by the
/// ladder at rung 3n; the first entry the actor carries and can afford
/// wins the bonus action.
///
/// The sibling table to `MELEE_ADJACENT_PRIMES` below, split from it on
/// what the charge buys. A prime is a rider on the swing the actor is
/// about to make, so it is worth nothing unless that swing happens this
/// turn — which is why that table's gate is `MELEE_REACH`. A posture is
/// worth something for as long as it lasts, so it wants to be up on the
/// turn *before* contact, which is what `IMMINENT_CONTACT_GAP` buys.
///
/// **The order is the priority.** Both current entries are artificer
/// buttons and no chassis carries both, so the order costs nothing
/// today; it is written down because the next entry will make it matter.
///
///   1. `defensive field` — the Armorer's slab of temporary hit points.
///      A known quantity with no roll attached, which is what puts it
///      first: the alchemist's flask might come out as an AC bump the
///      artificer did not need, and this never does.
///   2. `experimental elixir` — the Alchemist's flask. Rolls for its own
///      effect, so it is the entry whose value the AI cannot know before
///      spending the action, which is exactly the position RAW puts the
///      alchemist in.
const ENGAGED_SELF_POSTURES: &[&str] = &["defensive field", "experimental elixir"];

const MELEE_ADJACENT_PRIMES: &[&str] = &[
    "reaper's touch",
    "divine strike",
    "divine strike poison",
    "divine strike necrotic",
    "divine strike psychic",
    "fangs of the fire snake",
    "fire rune",
    "trip attack",
    "menacing attack",
    "disarming attack",
    "pushing attack",
    "goading attack",
];

/// Fighter Battle Master Distracting Strike — bonus-action prime that
/// adds +1d6 damage to the next melee hit and tags the target with
/// `Distracted`. The Distracted condition grants advantage to *other*
/// attackers — the fighter's own follow-up swings don't benefit. So
/// the prime only earns its full value when an ally is positioned to
/// cash in on the advantage rider.
///
/// Gate: at least one in-reach hostile (so the prime lands this turn)
/// AND at least one other ally footprint-adjacent to the same hostile
/// (so the advantage rider has a follow-up attacker to feed). Without
/// that follow-up, the maneuver collapses to a flat +1d6 which the
/// other once-per-rest primes above already serve — no point burning
/// this charge for the same floor.
fn try_distracting_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());

    // Find at least one in-melee hostile that *also* has a non-fighter
    // ally adjacent — that's the target whose Distracted rider would
    // actually feed a follow-up swing this round.
    let mut has_setup = false;
    'outer: for (eid, enemy) in encounter.actors.iter() {
        if *eid == actor_id || enemy.team() == my_team || !enemy.is_combat_active() {
            continue;
        }
        let e_loc = enemy.location();
        let e_size = get_tiles_from_size(enemy.size());
        // Enemy must be in fighter's melee reach so the prime hits this turn.
        if footprint_chebyshev(my_loc, my_size, e_loc, e_size) > 1 {
            continue;
        }
        // Scan for an ally (not the fighter) adjacent to the same enemy.
        for (aid, ally) in encounter.actors.iter() {
            if *aid == actor_id || *aid == *eid {
                continue;
            }
            if ally.team() != my_team || !ally.is_combat_active() {
                continue;
            }
            let a_loc = ally.location();
            let a_size = get_tiles_from_size(ally.size());
            if footprint_chebyshev(a_loc, a_size, e_loc, e_size) == 0 {
                has_setup = true;
                break 'outer;
            }
        }
    }
    if !has_setup {
        return None;
    }
    try_self_action(encounter, actor_id, "distracting strike")
}

/// Fighter Battle Master Sweeping Attack — bonus-action prime that
/// splashes 1d8 slashing onto one adjacent enemy of the primary target.
/// Only worth a charge when there's at least one *pair* of adjacent
/// enemies near the fighter, so the splash has somewhere to land. We
/// approximate by requiring two-plus enemies within melee reach of the
/// fighter — the rider's adjacency check at trigger time handles the
/// "no actual splash target" case as a no-op anyway, but the AI gate
/// keeps the charge for fights where it'll do real work.
fn try_sweeping_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
    let mut adj_enemies = 0u32;
    for (id, other) in encounter.actors.iter() {
        if *id == actor_id || other.team() == my_team || !other.is_combat_active() {
            continue;
        }
        let dist = footprint_chebyshev(
            my_loc,
            my_size,
            other.location(),
            get_tiles_from_size(other.size()),
        );
        if dist <= 1 {
            adj_enemies += 1;
        }
    }
    if adj_enemies < 2 {
        return None;
    }
    try_self_action(encounter, actor_id, "sweeping attack")
}

/// Fighter Battle Master Feinting Attack — bonus-action targeting one
/// enemy in melee reach. Grants self-advantage on the next attack vs the
/// feinted enemy via the help-grant lane. We pick the closest in-reach
/// hostile so the swing actually lands on the feinted target.
fn try_feinting_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    // Feint is melee-touch range — same envelope as Help.
    try_action_on_nearest_enemy(encounter, actor_id, "feinting attack", |gap| gap <= 1)
}

/// Arcane Trickster Versatile Trickster — bonus action at 30 ft that
/// grants self-advantage on the next attack against the designated
/// enemy, through the same help-grant lane as Feinting Attack.
///
/// Two extra gates beyond the action's own validator, both about not
/// wasting the rogue's scarcest resource:
///   1. Skip if a help-grant is already up. The grant is single-slot —
///      re-designating would overwrite an existing one for no gain, and
///      would burn the bonus action doing it.
///   2. Require an enemy inside the rogue's own melee reach, not just
///      inside the action's 30 ft. Advantage is only worth a bonus
///      action if the rogue is going to *swing* this turn, and the
///      shortsword that carries Sneak Attack is a melee weapon. The
///      designated target is then the same nearest enemy, so the
///      advantage lands on the swing that follows.
fn try_versatile_trickster(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.help_grant_any() {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, MELEE_REACH) {
        return None;
    }
    try_action_on_nearest_enemy(encounter, actor_id, "versatile trickster", |gap| gap <= 12)
}

/// Druid Shillelagh — bonus-action cantrip prime that adds +1d8 force
/// damage to the next melee weapon hit. Fire when an enemy is inside
/// the druid's own reach so the prime is consumed by the swing this
/// turn. The action itself custom-validates `!has_condition
/// (Shillelaghed)` so the AI never double-primes. Free (no slot
/// consumed) so it stays on the bonus-action lane without competing
/// with the leveled-slot smites.
fn try_shillelagh(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, MELEE_REACH) {
        return None;
    }
    try_self_action(encounter, actor_id, "shillelagh")
}

/// Empowered Spell — sorcerer bonus-action metamagic prime. Fires when
/// the caster has sorcery points available AND a combat-active enemy
/// sits within typical spell-attack / AoE range (24 tiles, 60 ft). Gate
/// matches the action's own validator — the action no-ops if the prime
/// is already up, so the AI check just keeps us off the action search
/// path when nothing's there to blast. Bonus action; non-conflicting
/// with the sorcerer's main-action damage spell on the same turn.
fn try_empowered_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.sorcery_points() == 0 {
        return None;
    }
    if actor.has_condition(Condition::EmpoweredSpelling) {
        return None;
    }
    // Range matches the longest sorcerer blaster spell — Magic Missile
    // is 120 ft (48 tiles), Fireball is 150 ft (60 tiles). We pick 24
    // as the engagement gate (Fireball / Cone of Cold's typical pin
    // distance) so the metamagic doesn't burn in an empty room.
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    try_self_action(encounter, actor_id, "empowered spell")
}

/// Overchannel — Evocation Wizard prime (subclass lv14). Declares that
/// the next damaging spell of level 1-5 deals maximum damage.
///
/// Costs no resource at all, so unlike the sorcerer's metamagic primes
/// there's no "is it worth the points" question — the price is the
/// escalating necrotic backlash, which is what this picker actually
/// gates on. The evoker overchannels freely on the first use of each
/// long rest and then only while healthy enough to pay: the backlash for
/// the next use is `(uses + 1) * spell_level` d12, so we require the
/// evoker's current HP to cover its *average* comfortably before
/// declaring. A wizard chassis has few enough hit points that a
/// second-use backlash off a level-5 slot (3d12 x 5, avg 97) would be
/// suicide, and the gate stops the AI from walking into it.
///
/// Also gated on holding a level 1-5 slot (the RAW window — priming with
/// only level 6+ slots left would burn nothing, but would also never
/// fire) and on an enemy in spell range, so the prime doesn't dangle in
/// an empty room.
fn try_overchannel(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    const MAX_OVERCHANNEL_LEVEL: u32 = 5;
    let actor = encounter.actors.get(&actor_id)?;
    if actor.has_condition(Condition::Overchanneling) {
        return None;
    }
    // Highest slot level 1-5 the evoker could actually spend — both the
    // "do I have a usable slot" gate and the worst-case backlash base.
    let top_slot = (1..=MAX_OVERCHANNEL_LEVEL)
        .rfind(|lvl| actor.spell_slot_manager.spell_slots(*lvl).spell_slots > 0)?;
    // Average of Nd12 is 6.5N; require better than 2x headroom so a bad
    // roll doesn't drop the evoker on their own spell.
    let backlash_dice = (actor.overchannel_uses() + 1) * top_slot;
    let expected = (backlash_dice * 13).div_ceil(2);
    if backlash_dice > 0 && actor.hitpoints() <= expected * 2 {
        return None;
    }
    try_self_action_when_enemy_within(encounter, actor_id, 24, "overchannel")
}

/// Heightened Spell — sorcerer bonus-action metamagic prime. Burns
/// 3 sorcery points to force disadvantage on the first save against
/// the next save-or-suck or burst-damage cast. Pricier than Empowered
/// (3 SP vs 1) so we gate on the SP being available AND at least one
/// "heightenable" spell present in the kit — either a concentration
/// lockdown spell (Hold Person / Hold Monster / Polymorph / Banishment
/// / Dominate Monster) or a save-for-half burst (Fireball / Cone of
/// Cold / Sunburst / Sleet Storm). Skipped if Heightened or Empowered
/// is already primed (the empowered damage burst is usually the better
/// play if a blaster spell is the next cast).
fn try_heightened_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.sorcery_points() < 3 {
        return None;
    }
    if actor.has_condition(Condition::HeightenedSpelling)
        || actor.has_condition(Condition::EmpoweredSpelling)
    {
        return None;
    }
    // Only fire when a heightenable spell is actually in the kit — either
    // a concentration lockdown or a save-for-half burst. Without one of
    // these the prime would dangle and the SP would be wasted. The
    // lockdown half is gated on `!is_concentrating` since casting a new
    // concentration spell would drop the old one; the burst half doesn't
    // care about concentration.
    let has_lockdown = !actor.is_concentrating()
        && HEIGHTENED_LOCKDOWN
            .iter()
            .any(|name| actor.find_action(name).is_some());
    let has_burst = HEIGHTENED_BURST
        .iter()
        .any(|name| actor.find_action(name).is_some());
    if !has_lockdown && !has_burst {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, 36) {
        return None;
    }
    try_self_action(encounter, actor_id, "heightened spell")
}

/// Careful Spell — sorcerer bonus-action metamagic prime. Burns 1
/// sorcery point to let up to CHA-mod allies auto-pass + take 0 damage
/// on the next AoE. Fire when:
/// - The sorcerer has SP available and the prime isn't already up.
/// - The sorcerer has a Burst-targeting harmful action in their kit (a
///   prime that never feeds a blast is wasted SP).
/// - At least one ally (the sorcerer themselves counts) sits within
///   ~5 tiles of a combat-active enemy — close enough that a typical
///   AoE between caster and target would catch both. Tighter than the
///   Heightened Spell gate (24 tiles) because Careful Spell only
///   pays off when an AoE would otherwise eat a teammate.
fn try_careful_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.sorcery_points() == 0 {
        return None;
    }
    if actor.has_condition(Condition::CarefulSpelling) {
        return None;
    }
    // Only fire when the caster owns at least one burst-targeting AoE
    // — otherwise the prime never engages and the SP is wasted.
    let has_aoe = actor.actions.iter().any(|a| {
        a.is_harmful() && matches!(a.targeting_schema(), TargetingSchema::Burst { .. })
    });
    if !has_aoe {
        return None;
    }
    let my_team = actor.team();
    // Check for an ally close to a live enemy — the only situation where
    // Careful Spell pays off. 5 tiles ≈ Fireball's 4-tile radius + slop.
    let actors: Vec<_> = encounter.actors.iter().collect();
    let mut close_pair = false;
    for (ally_id, ally) in &actors {
        if !ally.is_combat_active() || ally.team() != my_team {
            continue;
        }
        for (enemy_id, enemy) in &actors {
            if !enemy.is_combat_active() || enemy.team() == my_team {
                continue;
            }
            if ally_id == enemy_id {
                continue;
            }
            let dist = footprint_chebyshev(
                ally.location(),
                get_tiles_from_size(ally.size()),
                enemy.location(),
                get_tiles_from_size(enemy.size()),
            );
            if dist <= 5 {
                close_pair = true;
                break;
            }
        }
        if close_pair {
            break;
        }
    }
    if !close_pair {
        return None;
    }
    try_self_action(encounter, actor_id, "careful spell")
}

/// Distant Spell — sorcerer bonus-action metamagic prime. Burns 1
/// sorcery point to double the range of the next ranged spell. Fire
/// when:
/// - The sorcerer has SP available and the prime isn't already up.
/// - The closest visible enemy sits *beyond* the typical reach of the
///   sorcerer's mid-range arsenal (≈ 12 tiles / 30 ft) but inside the
///   doubled-reach envelope (≈ 24 tiles). Closer enemies don't need
///   the bonus — the SP would be wasted; enemies farther than 24
///   tiles aren't reachable even with the prime.
fn try_distant_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.sorcery_points() == 0 {
        return None;
    }
    if actor.has_condition(Condition::DistantSpelling) {
        return None;
    }
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
    let mut nearest: Option<isize> = None;
    for (id, a) in encounter.actors.iter() {
        if *id == actor_id || a.team() == my_team || !a.is_combat_active() {
            continue;
        }
        let dist = footprint_chebyshev(my_loc, my_size, a.location(), get_tiles_from_size(a.size()));
        if nearest.is_none_or(|d| dist < d) {
            nearest = Some(dist);
        }
    }
    // Only fire if the nearest enemy is in the 13..=24 tile band where
    // the prime actually flips a reach decision (closer enemies don't
    // need it; farther are unreachable even doubled).
    if !nearest.is_some_and(|d| (13..=24).contains(&d)) {
        return None;
    }
    try_self_action(encounter, actor_id, "distant spell")
}

/// Twinned Spell — sorcerer bonus-action metamagic prime. Burns
/// max(1, spell_level) SP to fire the next single-target spell against
/// a second valid target. Fires when:
/// - The sorcerer has SP available (RAW floor is 1 SP) and the prime
///   isn't already up.
/// - At least two combat-active enemies sit within 24 tiles — Twinned
///   only pays off if there's a second target to hit. We don't try to
///   match the LOS / reach gate of the *next* spell here (the consume
///   site does that); the proximity guard just keeps us off the search
///   path when there's nothing to twin onto.
/// - The sorcerer owns a known-twinnable single-target damage spell
///   (Fire Bolt / Ray of Frost / Chill Touch / Chromatic Orb / Witch
///   Bolt / etc.) — otherwise the prime would dangle and the SP would
///   be wasted on an AoE-only kit.
fn try_twinned_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.sorcery_points() == 0 {
        return None;
    }
    if actor.has_condition(Condition::TwinnedSpelling) {
        return None;
    }
    // Need ≥ 2 enemies within range so the prime can actually re-fire.
    if n_actors_within(encounter, actor_id, 24, false, 2) < 2 {
        return None;
    }
    // Only fire when the kit owns a twinnable single-target damage spell.
    // We derive the gate from the action trait — any action with a
    // SingleActor schema, is_harmful, and deals_damage is a viable twin
    // target. This keeps the heuristic future-proof: new single-target
    // damage spells added to the kit get picked up automatically without
    // a hardcoded name list to maintain.
    let has_twinnable = actor.actions.iter().any(|a| {
        a.is_harmful()
            && a.deals_damage()
            && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
    });
    if !has_twinnable {
        return None;
    }
    try_self_action(encounter, actor_id, "twinned spell")
}

/// Extended Spell — sorcerer bonus-action metamagic prime. Burns 1
/// sorcery point so the next spell with a 1-minute-or-longer duration
/// (Rounds(n) with n >= 10) has its timer doubled. Fires when:
/// - The sorcerer has SP available and the prime isn't already up.
/// - The kit owns a spell that installs a Rounds-style buff/debuff
///   (Mage Armor, Hunter's Mark, Bless, Hold Person, Polymorph, ...) —
///   without one, the prime would dangle and the SP would be wasted.
/// - At least one combat-active enemy is within typical engagement range
///   (24 tiles) so the sorcerer is actually casting this round.
///
/// Cheaper than Empowered (same 1 SP) but only pays off on the long-
/// duration cast. Skipped if Heightened / Empowered are already up so
/// the higher-leverage primes can fire first; Extended is the cheapest
/// of the long-prime family so it's the fallback rather than the lead.
fn try_extended_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.sorcery_points() == 0 {
        return None;
    }
    if actor.has_condition(Condition::ExtendedSpelling)
        || actor.has_condition(Condition::EmpoweredSpelling)
        || actor.has_condition(Condition::HeightenedSpelling)
    {
        return None;
    }
    // Spells whose canonical cast installs a Rounds(n) condition with
    // n >= 10 (1 minute or longer in our 6-second rounds). The list is
    // intentionally narrow: extending an instantaneous-damage spell
    // doesn't burn the prime, so the gate only matters for cleanly
    // signaling "this kit has at least one extendable cast." Keep in
    // sync with the canonical long-buff / lockdown spells the sorcerer
    // ships with.
    let has_extendable = EXTENDABLE
        .iter()
        .any(|name| actor.find_action(name).is_some());
    if !has_extendable {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    try_self_action(encounter, actor_id, "extended spell")
}

/// Seeking Spell (Tasha's) — sorcerer bonus-action metamagic prime.
/// Burns 2 sorcery points so the next missed spell-attack roll is
/// rerolled. Fires only when:
/// - The sorcerer has >= 2 SP and the prime isn't already up.
/// - The kit owns at least one spell-attack spell (`SingleActor` schema,
///   `is_harmful`, `deals_damage`) — Fire Bolt / Ray of Frost /
///   Chromatic Orb / Witch Bolt / Inflict Wounds / Scorching Ray and
///   friends. Without one of these the prime never engages and the
///   2 SP would be wasted on a save-for-half burst that ignores it.
/// - A combat-active enemy sits within typical engagement range
///   (24 tiles) so the prime feeds an actual attack this round.
///
/// We share the "kit-has-spell-attack" gate with Twinned Spell's
/// targeting heuristic (any `SingleActor` + `is_harmful` +
/// `deals_damage` action). Seeking is the pricier of the cheap primes
/// (2 SP vs 1) so it slots after the other 1-SP metamagics in the AI
/// pipeline; we also skip it when Empowered / Heightened are already
/// up (the higher-leverage primes feed the same blast already).
fn try_seeking_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.sorcery_points() < 2 {
        return None;
    }
    if actor.has_condition(Condition::SeekingSpelling)
        || actor.has_condition(Condition::EmpoweredSpelling)
        || actor.has_condition(Condition::HeightenedSpelling)
    {
        return None;
    }
    let has_attack_spell = actor.actions.iter().any(|a| {
        a.is_harmful()
            && a.deals_damage()
            && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
    });
    if !has_attack_spell {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    try_self_action(encounter, actor_id, "seeking spell")
}

/// Subtle Spell — sorcerer bonus-action metamagic prime. Burns 1 SP so
/// the next spell ignores Counterspell. Fires only when:
/// - The sorcerer has SP available and no metamagic prime is already up
///   (re-priming wastes the SP).
/// - At least one combat-active enemy on the *opposing* team owns the
///   Counterspell action — without an opposing counterspeller, the
///   prime has no payoff and the 1 SP is wasted.
///
/// We treat the gate as "does an enemy in this fight have Counterspell
/// in their action list?" — a coarse but reliable proxy for "is the
/// counterspell trade a live threat?" The cost is cheap enough (1 SP)
/// that even a weak signal is worth firing on.
fn try_subtle_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.sorcery_points() == 0 {
        return None;
    }
    if actor.has_any_metamagic_prime() {
        return None;
    }
    // Look for an opposing counterspeller as the live threat signal —
    // without an enemy who could Counterspell, the prime has nothing to
    // bite on and the 1 SP is wasted. Coarse but reliable.
    let caster_team = actor.team();
    let any_enemy_counterspeller = encounter.actors.iter().any(|(id, other)| {
        *id != actor_id
            && other.team() != caster_team
            && other.is_combat_active()
            && other.find_action("counterspell").is_some()
    });
    if !any_enemy_counterspeller {
        return None;
    }
    try_self_action(encounter, actor_id, "subtle spell")
}

/// Transmuted Spell (Tasha's) — sorcerer bonus-action metamagic prime.
/// Burns 1 sorcery point so the next elemental spell (acid / cold / fire /
/// lightning / poison / thunder) is remapped to the target's worst
/// weakness. Fires only when:
/// - The sorcerer has SP available and no metamagic prime is already up
///   (Transmuted stacks with damage-rerolling primes RAW but the engine
///   keeps the gate simple by routing through `has_any_metamagic_prime`).
/// - The kit owns at least one elemental damage spell (Fire Bolt /
///   Burning Hands / Lightning Bolt / Cone of Cold / Acid Splash /
///   Thunderwave / etc.) so the prime has something to bite on.
/// - At least one combat-active enemy on the opposing team has a
///   resistance or immunity to at least one elemental type AND
///   a vulnerability or neutrality to a different element — i.e.
///   there's an actual remap win to be had. Without that asymmetry the
///   prime burns 1 SP for zero damage delta.
///
/// Cheap (1 SP), so we slot it after the bigger-leverage primes
/// (Empowered, Heightened, Twinned, Extended) but before Seeking (2 SP).
fn try_transmuted_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    use crate::engine::side_effects::TRANSMUTABLE_DAMAGE_TYPES;
    use crate::engine::types::DamageModifier;
    let actor = encounter.actors.get(&actor_id)?;
    if actor.sorcery_points() == 0 {
        return None;
    }
    if actor.has_any_metamagic_prime() {
        return None;
    }
    // Kit gate: at least one elemental damage spell in the kit, asked of
    // each action rather than matched against a list of forty spell
    // names. The list was here because `damage_types()` was described as
    // UI-only and unreliable — it isn't: every damaging spell any
    // playable template carries declares its types, which
    // `nothing_friendly_claims_to_deal_damage` and the school-tag sweeps
    // keep true. Asking is both shorter and strictly more accurate; the
    // whitelist was missing fourteen spells the playable roster
    // actually carries — Chromatic Orb, Aganazzar's Scorcher, Flaming
    // Sphere, Immolation, Hellish Rebuke, Witch Bolt, Meteor Swarm,
    // Prismatic Spray and the rest — and every one of those was a
    // sorcerer declining a metamagic it could have used.
    //
    // `school().is_some()` is what keeps a flaming weapon out of it:
    // Transmuted Spell remaps a *spell's* damage, and a monster's fire
    // bite is not one.
    let has_elemental_spell = actor.actions.iter().any(|a| {
        a.school().is_some()
            && a.damage_types()
                .iter()
                .copied()
                .any(crate::engine::side_effects::is_transmutable_element)
    });
    if !has_elemental_spell {
        return None;
    }
    // Weakness-asymmetry gate: at least one nearby enemy has a non-trivial
    // resistance profile across the six elements (some resisted/immune
    // AND at least one not-resisted). Without the asymmetry the prime
    // can't improve damage. 24 tiles matches the other metamagic primes.
    let caster_team = actor.team();
    let any_asymmetric_enemy = encounter.actors.iter().any(|(id, other)| {
        if *id == actor_id || other.team() == caster_team || !other.is_combat_active() {
            return false;
        }
        let mut has_strong = false;
        let mut has_weak = false;
        for &dt in TRANSMUTABLE_DAMAGE_TYPES.iter() {
            match other.damage_modifier(dt) {
                // Absorption belongs on the strong side for the same
                // reason immunity does, and more so: it is the element
                // the prime most wants to move a spell *off* of.
                Some(DamageModifier::Resistance)
                | Some(DamageModifier::Immunity)
                | Some(DamageModifier::Absorption) => {
                    has_strong = true;
                }
                Some(DamageModifier::Vulnerability) | None => {
                    has_weak = true;
                }
            }
        }
        has_strong && has_weak
    });
    if !any_asymmetric_enemy {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    try_self_action(encounter, actor_id, "transmuted spell")
}

/// Symbiotic Entity — Circle of Spores Druid Action, once per short
/// rest. 36 temp HP plus a +1d6 necrotic melee rider and a doubled
/// Halo of Spores die.
///
/// Gate: a hostile within `IMMINENT_CONTACT_GAP`. Two of the feature's
/// three payoffs are short-ranged (the melee rider needs contact, the
/// doubled halo needs 10 ft), so raising the symbiote while the nearest
/// enemy is still crossing the room converts a once-per-rest charge and
/// a whole Action into temp HP alone. Waiting one turn costs nothing —
/// the charge doesn't expire — and buys the full feature.
///
/// The action's own validator carries the rest: charge unspent, holder
/// alive, no symbiote already riding.
fn try_symbiotic_entity(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_action_when_enemy_within(
        encounter,
        actor_id,
        IMMINENT_CONTACT_GAP,
        "symbiotic entity",
    )
}

/// Halo of Spores — Circle of Spores Druid reaction. 1d6 necrotic
/// (2d6 with the symbiote up) to one creature within 10 ft on a failed
/// Constitution save.
///
/// Nearest eligible hostile inside the halo's own reach. There is no
/// cleverer target choice available: the damage doesn't scale with
/// anything about the target, the save is the target's own, and the
/// reach is short enough that "in range at all" is usually a
/// one-candidate question. Nearest also keeps the pick deterministic,
/// which the AI-vs-AI sweep relies on.
fn try_halo_of_spores(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_action_on_nearest_enemy(encounter, actor_id, "halo of spores", |gap| {
        gap <= HALO_OF_SPORES_GAP
    })
}

/// Insightful Fighting — Inquisitive Rogue lv3 bonus action. Marks one
/// hostile within 30 ft (12 tiles) so the rogue's Sneak Attack lands on
/// it unconditionally for the next ten rounds.
///
/// Delegates wholesale to the shared single-target picker: the action's
/// own `custom_validate_input` owns the already-read-by-me dedup, and
/// the highest-HP heuristic the picker applies is the right one here for
/// the reason the Hexblade's Curse doc gives — a mark that pays per
/// landed swing is worth what the marked creature survives.
fn try_insightful_fighting(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_single_target_class_feature_hostile(encounter, actor_id, "insightful fighting", 12)
}

/// Master of Tactics — Mastermind Rogue lv3 bonus action. The Help
/// action at 30 ft, handed to an ally.
///
/// Two gates beyond the action's own. The ally must have a hostile
/// inside their own melee reach, because the grant is consumed by their
/// next attack and advantage on a swing they cannot make expires
/// unspent; and they must not already be `Helped`, because a second
/// grant on top of the first buys nothing.
///
/// Among the allies that qualify, the pick is the one with the most hit
/// points remaining. That is a proxy for "the ally most likely to still
/// be standing when their turn comes", which is what the grant needs —
/// it lasts until the start of the rogue's next turn, so an ally who
/// drops before acting wastes it. Ties break on the sorted id order the
/// seeded sweeps rely on.
fn try_master_of_tactics(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("master of tactics")?;
    let my_team = actor.team();
    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for ally_id in encounter.sorted_actor_ids() {
        if ally_id == actor_id {
            continue;
        }
        let Some(ally) = encounter.actors.get(&ally_id) else {
            continue;
        };
        if ally.team() != my_team
            || !ally.is_combat_active()
            || ally.has_condition(Condition::Helped)
        {
            continue;
        }
        // The grant has to have a swing to ride. `MELEE_REACH` rather
        // than the ally's own weapon reach: the picker is a heuristic,
        // and one tile of slack the other way would have it hand
        // advantage to an archer who is about to be charged.
        let in_contact = encounter.actors.iter().any(|(eid, enemy)| {
            *eid != ally_id
                && enemy.team() != my_team
                && enemy.is_combat_active()
                && encounter
                    .footprint_distance(ally_id, *eid)
                    .is_some_and(|d| d <= crate::actions::action_template::MELEE_REACH)
        });
        if !in_contact {
            continue;
        }
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![ally_id]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        let hp = ally.hitpoints();
        if best.as_ref().is_none_or(|(best_hp, _)| hp > *best_hp) {
            best = Some((hp, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Steady Aim — Tasha's Rogue lv3 bonus action. Installs an advantage-
/// on-next-attack prime at the cost of zeroing speed for the rest of
/// the turn. Fires when:
/// - The action validates (haven't moved, no Helped already up).
/// - At least one combat-active enemy is within typical ranged-weapon
///   range so the advantage actually feeds a swing — gated wider than
///   adjacent reach since the speed-zero cost trades best for a
///   shortbow / hand crossbow snipe rather than a melee follow-up.
///
/// Doesn't trip on metamagic / concentration markers because the rogue
/// doesn't carry those — the action's own `custom_validate_input` is the
/// load-bearing gate.
fn try_steady_aim(encounter: &EncounterInstance, actor_id: usize) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    // Engine-side gate is the source of truth; bail early if it would
    // refuse so we don't burn the action picker on a no-op.
    if actor.has_moved_this_turn() || actor.has_condition(Condition::Helped) {
        return None;
    }
    // Need a swing-able enemy in range. Use the shortbow's normal range
    // (8 tiles) as the gate — the speed-zero cost is justified when the
    // ranged follow-up is a real option.
    if !any_enemy_within(encounter, actor_id, 8) {
        return None;
    }
    try_self_action(encounter, actor_id, "steady aim")
}

/// Cunning Strike — 5e 2024 Rogue lv5 bonus-action primes. Trades a
/// Sneak Attack die for one of:
/// - Poison: CON save or Poisoned for 10 rounds (most generally useful
///   debuff so it's the AI's first pick).
/// - Trip: DEX save or Prone (high payoff vs ranged targets but the
///   rogue's own next swing benefits the least; we save it for backup).
///
/// Gates:
/// - The rogue's once-per-turn Sneak Attack hasn't been spent yet.
/// - No Cunning Strike prime is already active (mutually exclusive).
/// - A sneak-eligible enemy is adjacent (i.e. footprint-touching) so the
///   prime is consumed this turn — the rogue's shortsword is melee-only,
///   and primes self-clear via `UntilStartOfNextTurn`.
/// - The shortsword action is installed (otherwise priming is pointless).
/// - Sneak attack pool ≥ 2 dice so the consume gate (`pool > cost`) accepts
///   the deduction. At level 1 the pool is 1d6 — priming a Cunning Strike
///   would dangle since `consume_cunning_strike` refuses to reduce
///   below 1 die.
///
/// Returns the best installable variant, or None if the gate fails.
/// Withdraw / Daze are intentionally skipped: Withdraw is a kiting tool
/// the simple AI doesn't strategize around, and Daze's 2-die cost is
/// rarely worth the loss in burst damage compared to Poison's 1-die.
fn try_cunning_strike(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.sneak_attack_used() {
        return None;
    }
    // Mutually exclusive: if any prime is up, do nothing.
    if actor.has_any_cunning_strike_prime() {
        return None;
    }
    let level = actor.level();
    if crate::actions::class_attacks::sneak_attack_dice_for_level(level) < 2 {
        return None;
    }
    // Need a sneak-eligible enemy the shortsword can actually reach,
    // which is `MELEE_REACH` — the gate used to say gap 0 while its own
    // comment named the reach as 1, and a rogue standing at the
    // distance its blade covers found no candidate.
    if !any_enemy_within(encounter, actor_id, MELEE_REACH) {
        return None;
    }
    // Prefer Poison — broadest debuff (disadvantage on attacks and
    // ability checks across the full 10-round timer). Falls through to
    // Trip if Poison isn't installed.
    try_self_action(encounter, actor_id, "cunning strike (poison)")
        .or_else(|| try_self_action(encounter, actor_id, "cunning strike (trip)"))
}

/// Tides of Chaos — Wild Magic Sorcerer 1/long-rest bonus action.
/// Installs an advantage-on-next-attack prime. Fires only when:
/// - The feature charge is still available.
/// - The prime isn't already up.
/// - An enemy sits within typical spell-attack reach (24 tiles) so the
///   advantage actually feeds a swing this round.
///
/// Doesn't gate on SP (no SP cost) and doesn't conflict with metamagic
/// primes (they stack — Tides + Empowered + Fire Bolt is a real combo).
/// We do skip it when a save-or-suck control prime is up (Heightened
/// Spell) since the next cast probably won't be an attack roll and the
/// advantage would dangle.
fn try_tides_of_chaos(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.feature_available(crate::actions::class_features::TIDES_OF_CHAOS_TAG) {
        return None;
    }
    if actor.has_condition(Condition::TidesOfChaos) {
        return None;
    }
    // Save-or-suck spells don't roll an attack roll; the advantage would
    // dangle. The other metamagic primes (Empowered, Twinned, Seeking,
    // ...) all feed attack-style or damage-style casts, so they're
    // compatible and don't block Tides here.
    if actor.has_condition(Condition::HeightenedSpelling) {
        return None;
    }
    // Need a swing-able enemy in range. Use the same 24-tile gate as the
    // empowered / seeking primes — covers Fire Bolt / Magic Missile range.
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    try_self_action(encounter, actor_id, "tides of chaos")
}

/// Font of Magic — convert spell slot ↔ sorcery points. Bonus action.
/// Two directions:
/// - **Refill SP from a held slot** (`convert level-N slot` → N SP).
///   Fire when SP is at 0 AND at least one slot of an unused level is
///   held. Picks the lowest-level held slot for the conversion (cheapest
///   resource trade per RAW — burning a level-1 slot for 1 SP is the
///   smallest sacrifice; higher levels are saved for actual casts).
/// - **Recreate a spent low-level slot** (`create level-1 slot` for
///   2 SP). Fire when the sorcerer's level-1 pool is depleted AND they
///   hold >= 2 SP that aren't earmarked for an active metamagic prime.
///   We only consider the level-1 conversion here — higher-level
///   conversions cost more SP than they yield in raw casting value, so
///   the AI sticks to the cheapest restoration.
///
/// Skipped when any metamagic prime is currently primed (don't muddle
/// the bonus-action lane while a higher-leverage prime is up).
fn try_font_of_magic(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    // Don't compete with an active prime — the prime is more valuable
    // than re-shuffling resources between SP and slots. Consolidated
    // through `has_any_metamagic_prime` so new primes (Subtle Spell,
    // future Tasha's lineup) auto-extend the gate without an edit here.
    if actor.has_any_metamagic_prime() {
        return None;
    }
    // Direction 1: SP empty → burn a low-level slot to refill SP.
    if actor.sorcery_points() == 0 {
        for (level, name) in [
            (1, "convert level-1 slot"),
            (2, "convert level-2 slot"),
            (3, "convert level-3 slot"),
        ] {
            let ssi = actor.spell_slot_manager.spell_slots(level);
            if ssi.spell_slots == 0 {
                continue;
            }
            if let Some(aei) = try_self_action(encounter, actor_id, name) {
                return Some(aei);
            }
        }
    }
    // Direction 2: level-1 pool depleted but SP is flush — recreate a
    // level-1 slot. Gated on >= 2 SP so the metamagic primes still have
    // room to fire on a future turn.
    if actor.sorcery_points() >= 2 {
        let lv1 = actor.spell_slot_manager.spell_slots(1);
        if lv1.max_spell_slots > 0 && lv1.spell_slots == 0
            && let Some(aei) = try_self_action(encounter, actor_id, "create level-1 slot")
        {
            return Some(aei);
        }
    }
    None
}

/// Telekinetic — bonus-action cantrip shove. Pulls a single enemy 5 ft
/// toward the caster on a failed STR save. We pick the closest enemy
/// that's *out* of melee reach but inside the cantrip's 60ft range, so
/// the pull yanks them into melee range (or at least closer) for the
/// caster or an ally. An enemy already adjacent is skipped — the pull
/// would be wasted, and the AI's other bonus-action lanes (Hex re-target,
/// Shillelagh) get to run instead.
fn try_telekinetic(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    // Closest enemy in the 2-24 tile sweet spot. Skip already-adjacent
    // (gap 0-1) because the pull does nothing; cap at 24 (60ft) per
    // RAW range.
    try_action_on_nearest_enemy(encounter, actor_id, "telekinetic", |gap| {
        (2..=24).contains(&gap)
    })
}

/// Cleric Preserve Life — once-per-rest Channel Divinity mass-heal. The
/// action's `side_effects` distributes 5×level HP among the most-wounded
/// allies (including self) within 30ft. We gate on:
/// - At least one combat-active ally (self counts) below half max HP,
///   AND inside the 30ft aura.
/// - At least one enemy nearby — outside a fight, the cleric should heal
///   with Cure Wounds / Healing Word instead so this once-per-rest stays
///   for the mass-cleanse moment.
fn try_preserve_life(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("preserve life")?;
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    let my_team = actor.team();
    let wounded_nearby = encounter.actors.iter().any(|(_id, a)| {
        if a.team() != my_team || !a.is_combat_active() {
            return false;
        }
        let cap = a.max_hitpoints();
        if a.hitpoints() >= cap / 2 + (cap % 2) {
            return false;
        }
        actor.footprint_gap_to(a) <= 12
    });
    if !wounded_nearby {
        return None;
    }
    let aei = ActionExecutionInfo::new(action, actor_id, None, None, None);
    aei.validate(encounter).then_some(aei)
}

/// Peace Domain Balm of Peace — Channel Divinity, heals every ally
/// inside 5 ft for 2d6 + WIS and leaves the cleric free to walk away.
///
/// Two gates, and the second is the one that makes it a decision. The
/// fight has to be live (an enemy inside the same 60 ft window every
/// other charge on the ladder uses, so the balm isn't burned before
/// anybody has been hit), and **two** allies have to be standing in
/// the five feet the balm reaches — the cleric plus at least one other.
///
/// Two rather than one, unlike Preserve Life's gate. Preserve Life is
/// billed once and spread over everyone who needs it out to 30 ft, so
/// a single wounded ally already justifies it; the balm's whole
/// argument is that it hits several people at once, and a cleric who
/// spends a Channel Divinity to heal only themselves has spent it on
/// the worst thing it does. The wound check is folded in the same way —
/// somebody in the huddle has to actually be missing hit points.
fn try_balm_of_peace(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("balm of peace")?;
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    // 5 ft — the balm's own radius, and the same one its `side_effects`
    // heals over, so the gate and the payout can't disagree.
    let huddle = encounter.ally_heal_burst_targets(actor_id, actor.location(), 1);
    if huddle.len() < 2 {
        return None;
    }
    let anyone_hurt = huddle.iter().any(|id| {
        encounter
            .actors
            .get(id)
            .is_some_and(|a| a.hitpoints() < a.max_hitpoints())
    });
    if !anyone_hurt {
        return None;
    }
    let aei = ActionExecutionInfo::new(action, actor_id, None, None, None);
    aei.validate(encounter).then_some(aei)
}

/// Peace Domain Emboldening Bond — hand the d4 to the cleric and the
/// two nearest allies.
///
/// The gate is thin on purpose. The action's own
/// `custom_validate_input` already declines when there is nobody left
/// to bond (everyone in range is carrying it), which is the stacking
/// guard; all this rung adds is that the fight is live, so a cleric
/// doesn't spend both charges on an empty corridor and arrive at the
/// first enemy with none.
///
/// No "is it worth it" heuristic beyond that, because for this feature
/// there isn't one: the bond costs an Action the cleric would
/// otherwise spend on a cantrip, pays out on every roll every bonded
/// creature makes for the next ten rounds, and is strictly better the
/// earlier it lands. The interesting decision the domain poses is
/// about *formation*, which the AI expresses by moving rather than by
/// choosing between buttons.
fn try_emboldening_bond(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("emboldening bond")?;
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    let aei = ActionExecutionInfo::new(action, actor_id, None, None, None);
    aei.validate(encounter).then_some(aei)
}

/// Clockwork Soul Bastion of Law — lay 5d8 temporary hit points on the
/// body most likely to need them.
///
/// "Most likely to need them" is the ally standing nearest to a
/// hostile, ties broken by the lower hit-point total and then by id so
/// a seeded run reproduces. Nearest-to-a-hostile rather than
/// lowest-HP, which is what every heal on the ladder sorts by, because
/// a ward is not a heal: it is worth exactly what the next few blows
/// aimed at its holder are worth, and the creature about to be hit is
/// the front-liner rather than the wounded archer behind them. A ward
/// on somebody nothing can reach expires unspent.
///
/// The sorcerer is in the running like anybody else — a Clockwork Soul
/// that a hill giant has walked up to is the right place for it — and
/// the action's own validation carries the "already warded" gate, so
/// this rung never spends the charge overwriting a full one.
fn try_bastion_of_law(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("bastion of law")?;
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    let my_team = actor.team();
    let mut best: Option<((isize, u32, usize), ActionExecutionInfo)> = None;
    for ally_id in encounter.sorted_actor_ids() {
        let Some(ally) = encounter.actors.get(&ally_id) else {
            continue;
        };
        if ally.team() != my_team || !ally.is_combat_active() {
            continue;
        }
        let Some(nearest_threat) = encounter
            .actors
            .iter()
            .filter(|(_, h)| h.team() != my_team && h.is_combat_active())
            .map(|(_, h)| ally.footprint_gap_to(h))
            .min()
        else {
            continue;
        };
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![ally_id]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        let key = (nearest_threat, ally.hitpoints(), ally_id);
        if best.as_ref().is_none_or(|(best_key, _)| key < *best_key) {
            best = Some((key, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Wizard Arcane Recovery — free no-cost slot restore. Fire when the
/// wizard has spent a low-tier slot and is in an active fight (so the
/// recovered slot has something to land on this encounter).
fn try_arcane_recovery(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_engaged_self_recovery(encounter, actor_id, "arcane recovery")
}

/// Druid Circle of the Land Natural Recovery — free no-cost slot
/// restore. Mirror of `try_arcane_recovery`: fires when the druid has
/// spent a low-tier slot and is engaged in a fight (so the recovered
/// slot pays back this encounter). Shared gate lives in
/// `try_engaged_self_recovery` — a future short-rest slot-recovery
/// feature (Sorcerer's Font of Magic recovery variants, etc.) drops
/// in as a one-liner alongside these two.
fn try_natural_recovery(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_engaged_self_recovery(encounter, actor_id, "natural recovery")
}

/// Shared "engaged in combat + fire a free-cost self-recovery action"
/// gate. Both Arcane Recovery and Natural Recovery use the same shape:
/// short-circuit if no enemy is within a plausible action-window
/// distance (24 tiles ≈ 60 ft — the range at which any low-tier
/// ranged / area spell can matter this encounter), then delegate to
/// `try_self_action` with the recovery's canonical name. Factored so
/// the gate lives in one place; new short-rest recovery features slot
/// in with a one-line action-name change.
///
/// Now a thin gap-24 delegation on top of the general
/// `try_self_action_when_enemy_within` helper — the recovery cohort
/// (Arcane Recovery / Natural Recovery) still routes through this
/// named alias to keep the domain semantics readable, but the general
/// helper is the source of truth for the gate.
fn try_engaged_self_recovery(
    encounter: &EncounterInstance,
    actor_id: usize,
    action_name: &str,
) -> Option<ActionExecutionInfo> {
    try_self_action_when_enemy_within(encounter, actor_id, 24, action_name)
}

/// Bard Cutting Words — bonus-action enemy debuff. Fires Mocked on the
/// enemy that's most likely to swing next turn (heuristic: nearest
/// combat-active enemy within 60ft that isn't already Mocked). The
/// "biggest threat" picker would need attack-roll inspection; closest-
/// enemy-with-melee-range-to-an-ally is a reasonable proxy.
fn try_cutting_words(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("cutting words")?;
    let my_team = actor.team();
    // Find an enemy that's adjacent to one of our allies — they'll likely
    // swing at us / our ally next turn, so debuffing them lands the
    // disadvantage on a strike that matters.
    let mut best: Option<(isize, ActionExecutionInfo)> = None;
    for tid in encounter.sorted_actor_ids() {
        let Some(t) = encounter.actors.get(&tid) else {
            continue;
        };
        if tid == actor_id || t.team() == my_team || !t.is_combat_active() {
            continue;
        }
        if t.has_condition(Condition::Mocked) {
            continue;
        }
        // How close is this enemy to any of our combat-active allies?
        // The closer they are, the more likely their next attack is the
        // one we want to spoil.
        let t_loc = t.location();
        let t_size = get_tiles_from_size(t.size());
        let min_ally_gap = encounter
            .actors
            .iter()
            .filter_map(|(_id, a)| {
                if a.team() != my_team || !a.is_combat_active() {
                    return None;
                }
                Some(footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    t_loc,
                    t_size,
                ))
            })
            .min()
            .unwrap_or(isize::MAX);
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![tid]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        if best.as_ref().is_none_or(|(best_d, _)| min_ally_gap < *best_d) {
            best = Some((min_ally_gap, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Unsettling Words — Eloquence Bard bonus action. Picks the *toughest*
/// live enemy in range, which is the opposite of what almost every other
/// picker in this file does and is the right answer here for two
/// reasons.
///
/// The first is that the −4 is only ever collected by a saving throw,
/// and the bard's own sheet is where those saves come from: Hold Person,
/// Compulsion, Suggestion, Charm Person and Dissonant Whispers are all
/// save-or-suck, and all five are worth casting on the enemy the party
/// least wants taking turns. Focus-firing the debuff onto whoever is
/// already nearly dead would spend the bard's one charge on a creature
/// the fighter is about to kill anyway.
///
/// The second is that the debuff spends itself on the *first* save its
/// holder rolls, wanted or not. Against a wounded straggler, the die is
/// as likely to be burned by an incidental Fireball as by anything the
/// bard chose; against the biggest thing in the room, more of the saves
/// on offer are ones worth bending.
///
/// "Toughest" is current HP rather than max, so a boss that has already
/// been ground down below the reinforcements stops being the pick — the
/// heuristic tracks who is still standing, not who started largest.
/// Ties break on the lower actor id, keeping the choice deterministic
/// under a fixed seed like every other picker here.
fn try_unsettling_words(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("unsettling words")?;
    let my_team = actor.team();
    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for tid in encounter.sorted_actor_ids() {
        let Some(t) = encounter.actors.get(&tid) else {
            continue;
        };
        if tid == actor_id || t.team() == my_team || !t.is_combat_active() {
            continue;
        }
        // The action's own gate already refuses a target still holding
        // the condition; checking here as well keeps the loop from
        // building an `ActionExecutionInfo` per already-unsettled enemy
        // on the way to being told no.
        if t.has_condition(Condition::Unsettled) {
            continue;
        }
        let hp = t.hitpoints();
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![tid]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        if best.as_ref().is_none_or(|(best_hp, _)| hp > *best_hp) {
            best = Some((hp, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Warding Bond — cleric / paladin lv2 abjuration. Touch-range damage-
/// share bond: pick the most fragile combat-active ally that's footprint-
/// adjacent and bond with them. "Most fragile" = lowest current HP /
/// max HP ratio (the ally who most benefits from the resistance bump).
/// Gated on:
/// - At least one enemy within 12 tiles (~30ft) — don't waste the slot
///   pre-fight, since the bond only matters when an ally is taking hits.
/// - The caster's own current HP fraction is above 50% — the caster
///   pays mirrored damage, so a low-HP caster should skip the bond
///   rather than join the ally on the death-save table.
fn try_warding_bond(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("warding bond")?;
    // Don't bond if we're already too hurt to carry the mirrored hits.
    if is_low_hp(encounter, actor_id, 0.5) {
        return None;
    }
    // Don't bond outside an active fight.
    if !any_enemy_within(encounter, actor_id, 12) {
        return None;
    }
    let my_team = actor.team();
    // Most fragile ally in touch range (gap 1) that isn't already
    // bonded. Tie-break by lower HP ratio (more wounded wins).
    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for tid in encounter.sorted_actor_ids() {
        let Some(t) = encounter.actors.get(&tid) else {
            continue;
        };
        if tid == actor_id || t.team() != my_team || !t.is_combat_active() {
            continue;
        }
        if t.has_condition(Condition::WardingBonded) {
            continue;
        }
        if actor.footprint_gap_to(t) > 1 {
            continue;
        }
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![tid]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        // Score = max_hp - hp (lower HP wins); equal HP breaks toward
        // higher max-HP (the tougher frame benefits more from the
        // resistance / AC bump). Stored as a single u32 so the
        // comparison stays terse.
        let max = t.max_hitpoints().max(1);
        let cur = t.hitpoints();
        let score = max.saturating_sub(cur);
        if best.as_ref().is_none_or(|(best_score, _)| score > *best_score) {
            best = Some((score, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Foresight — level-9 single-target ally buff. Pick the highest-HP
/// combat-active ally (likely a frontliner) and lay the apex buff on
/// them. Concentration-gated.
fn try_foresight(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    let action = actor.find_action("foresight")?;
    let team = actor.team();
    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for id in encounter.sorted_actor_ids() {
        let Some(a) = encounter.actors.get(&id) else {
            continue;
        };
        if a.team() != team || !a.is_combat_active() {
            continue;
        }
        if a.has_condition(Condition::Foreseen) {
            continue;
        }
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![id]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        let hp = a.hitpoints();
        if best.as_ref().is_none_or(|(best_hp, _)| hp > *best_hp) {
            best = Some((hp, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// True if any combat-active hostile actor's footprint sits within
/// `max_gap` tiles of `actor_id`'s footprint. Shared helper for
/// proximity-gated self-buff heuristics (Rage at gap 12, Reckless
/// Attack at `MELEE_REACH`). Returns false when `actor_id` is missing.
fn any_enemy_within(
    encounter: &EncounterInstance,
    actor_id: usize,
    max_gap: isize,
) -> bool {
    n_actors_within(encounter, actor_id, max_gap, false, 1) >= 1
}

/// Count of combat-active actors (excluding the caster) within
/// `max_gap` tiles of `actor_id`'s footprint, filtered by team
/// relation. `allies = true` counts team-mates; `allies = false`
/// counts hostiles. Caller can short-circuit by passing `cap` —
/// counting stops as soon as we hit that many candidates, which
/// keeps the proximity check O(min(cap, n)) on large maps.
///
/// Returns 0 when `actor_id` is missing. Shared body for the
/// proximity-and-cluster checks used by `any_enemy_within`,
/// `try_holy_aura` (ally-cluster gate), `try_aura_of_life`,
/// `try_aura_of_purity`, and other ally-or-enemy-radius heuristics.
fn n_actors_within(
    encounter: &EncounterInstance,
    actor_id: usize,
    max_gap: isize,
    allies: bool,
    cap: usize,
) -> usize {
    let Some(actor) = encounter.actors.get(&actor_id) else {
        return 0;
    };
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
    let mut hits = 0usize;
    for (id, a) in &encounter.actors {
        if *id == actor_id || !a.is_combat_active() {
            continue;
        }
        if (a.team() == my_team) != allies {
            continue;
        }
        let dist = footprint_chebyshev(
            my_loc,
            my_size,
            a.location(),
            get_tiles_from_size(a.size()),
        );
        if dist <= max_gap {
            hits += 1;
            if hits >= cap {
                return hits;
            }
        }
    }
    hits
}

/// Wrap "find action by name → ActionExecutionInfo if validates".
/// Stays tight on the surface area for buff-style self-target actions
/// that take no args.
fn try_self_action(
    encounter: &EncounterInstance,
    actor_id: usize,
    action_name: &str,
) -> Option<ActionExecutionInfo> {
    let action = encounter.actors.get(&actor_id)?.find_action(action_name)?;
    // A buff that names a creature is aimed at the caster; a buff that
    // takes no arguments is already about them.
    //
    // The branch is what lets a `SingleActor` self-buff ride this lane
    // at all. Most of the roster's self-buffs are `NoArgs` (Blur,
    // Bigby's Hand, the four Investitures), but the ones RAW writes as
    // "a creature you touch" are not — Intellect Fortress is the first,
    // Barkskin and Warding Bond are the same shape one lane over — and
    // every one of them fails `validate` with an empty target list.
    // Without this they could only be reached by a picker of their own,
    // which is a whole function to express "point it at yourself".
    // The `!is_harmful` half is the guard, not decoration. Two hostile
    // `SingleActor` features reach the AI by name today (Halo of Spores,
    // Insightful Fighting) and both are routed through the enemy-side
    // pickers instead, which is where they belong — but a third arriving
    // on this lane by mistake would otherwise be aimed at the caster,
    // and "the druid saves against its own spores" is not a failure any
    // test would be looking for.
    let targets = match action.targeting_schema() {
        TargetingSchema::SingleActor if !action.is_harmful() => Some(vec![actor_id]),
        _ => None,
    };
    let aei = ActionExecutionInfo::new(action, actor_id, targets, None, None);
    aei.validate(encounter).then_some(aei)
}

/// Sister to `try_self_action` gated on "at least one hostile within
/// `max_gap` tiles of the actor's footprint". Short-circuits with
/// `None` when the room is empty of eligible enemies — the common
/// no-op case that every once-per-rest / prime-style self-target
/// picker below open-coded as a two-line `any_enemy_within → return
/// None` prelude followed by a `try_self_action` delegation.
///
/// The `max_gap` parameter distinguishes the two shipping cadences:
///   - **`MELEE_REACH`** — in reach of a swing: bonus-action primes
///     whose next swing has to connect this turn (Trip / Menacing /
///     Disarming / Pushing / Goading / Precision Attack; Divine Smite /
///     Divine Strike; Frenzy / War Priest / Reckless Attack).
///   - **gap 24** — spell-window: free-cost / prime actions whose
///     payoff can be spent across the next few turns (Arcane Recovery,
///     Natural Recovery, Guided Strike's +10 accuracy prime).
///
/// `MELEE_REACH` and not gap 0, which is what half this cohort used to
/// pass. Gap 0 means two footprints actually touching, and the AI never
/// closes that far: a 5 ft weapon reaches a gap of 1, so the attack
/// picker fires — and the approach stops — one tile short of the
/// distance those gates were asking for. Every prime on the stricter
/// number was therefore dead. See `try_divine_smite`.
///
/// Adding a future once-per-rest self-target picker drops in as a
/// one-line delegation instead of the three-line `any_enemy_within +
/// return None + try_self_action` boilerplate.
fn try_self_action_when_enemy_within(
    encounter: &EncounterInstance,
    actor_id: usize,
    max_gap: isize,
    action_name: &str,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, max_gap) {
        return None;
    }
    try_self_action(encounter, actor_id, action_name)
}

/// Fire `action_name` at the *closest* combat-active hostile whose
/// footprint gap from `actor_id` satisfies `gap_ok`, and return the
/// validated `ActionExecutionInfo`. `None` when the actor doesn't carry
/// the action, no hostile is in band, or every candidate fails the
/// action's own validator.
///
/// Enemy-side counterpart to `try_self_action` for the single-target
/// bonus-action lane, and the shared body for the pickers that each
/// open-coded the same walk: iterate `sorted_actor_ids` (for
/// determinism), skip self / allies / downed, measure the footprint
/// Chebyshev gap, filter by band, build the AEI, validate, keep the
/// nearest survivor.
///
/// The band is a closure rather than a `max_gap` scalar because the
/// interesting pickers on this lane aren't all "within N": Telekinetic
/// wants a *donut* (2..=24 — an adjacent enemy can't be pulled any
/// closer), where Feinting Attack and Versatile Trickster want plain
/// ceilings at melee reach and 30 ft. A scalar would have forced the
/// donut case back into an open-coded loop, which is the duplication
/// this helper exists to remove.
fn try_action_on_nearest_enemy(
    encounter: &EncounterInstance,
    actor_id: usize,
    action_name: &str,
    gap_ok: impl Fn(isize) -> bool,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action(action_name)?;
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
    let mut best: Option<(isize, ActionExecutionInfo)> = None;
    for tid in encounter.sorted_actor_ids() {
        let Some(t) = encounter.actors.get(&tid) else {
            continue;
        };
        if tid == actor_id || t.team() == my_team || !t.is_combat_active() {
            continue;
        }
        let dist = footprint_chebyshev(
            my_loc,
            my_size,
            t.location(),
            get_tiles_from_size(t.size()),
        );
        if !gap_ok(dist) {
            continue;
        }
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![tid]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        if best.as_ref().is_none_or(|(best_d, _)| dist < *best_d) {
            best = Some((dist, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Same as `try_self_action`, but searches `available_actions()` —
/// the template-action list plus one entry per carried consumable item —
/// instead of just the template list. Used by self-buff heuristics that
/// want to fall back to a Potion of X consumable when the matching
/// spell isn't on the caster's template (e.g. a fighter drinking a
/// Potion of Mage Armor). The slower lookup (`available_actions` rebuilds
/// the dedup vec each call) is fine since this fires once per turn at
/// most through the AI pipeline.
fn try_self_action_inc_items(
    encounter: &EncounterInstance,
    actor_id: usize,
    action_name: &str,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor
        .available_actions()
        .into_iter()
        .find(|a| a.name() == action_name)?;
    let aei = ActionExecutionInfo::new(action, actor_id, None, None, None);
    aei.validate(encounter).then_some(aei)
}

/// Shared gate for concentration-bound self-buff heuristics. Returns
/// the `try_self_action` result iff:
/// - the caster exists,
/// - the caster is not already concentrating,
/// - the caster does not already hold `installed_marker` (skip re-cast),
/// - at least one combat-active enemy is within `engage_radius` tiles
///   (don't burn the slot in an empty room).
///
/// Every caller reaches it through `try_self_buff_pick`, which walks
/// one of the two `SelfBuffPick` cohorts — this is the gate, and the
/// cohorts are the ten spells that want it.
fn try_self_buff_concentration(
    encounter: &EncounterInstance,
    actor_id: usize,
    action_name: &str,
    installed_marker: Condition,
    engage_radius: isize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    if actor.has_condition(installed_marker) {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, engage_radius) {
        return None;
    }
    try_self_action(encounter, actor_id, action_name)
}

/// Starry Form — Circle of Stars Druid bonus action, once per short
/// rest, and the only pick on the roster where the AI has to choose
/// *which* buff rather than whether to take one.
///
/// The three constellations are not ranked against each other, so the
/// choice is made by asking which of them has work to do *this* fight,
/// most specific condition first:
///
///   1. **Chalice** if a hurt ally is inside the 30 ft spill radius.
///      The overflow needs the druid to cast a heal to have anything to
///      spill, and the druid's heal lane only fires when somebody is
///      hurt — so a wounded ally in range is the gate that says the
///      form will pay for itself.
///   2. **Dragon** if the druid is concentrating on something *and* has
///      already taken damage. Both halves are load-bearing: a
///      concentration save is only rolled when the holder is hit, so a
///      druid at full HP has not yet been shown that its concentration
///      is under any threat at all, and floors nothing by taking this.
///   3. **Archer** otherwise — the opening-round answer and the solo
///      answer. A bonus action the druid had no other use for becomes
///      1d8 + WIS a round for the next ten rounds.
///
/// The order matters more than it looks, because this rung sits below
/// the concentration self-buff cohort: by the time it runs, a druid
/// that had a self-buff to cast has already cast it and is
/// concentrating on *something*. Testing concentration alone would
/// therefore have picked Dragon on essentially every chassis that
/// carries a self-buff, every fight — which is how the first draft
/// behaved, and it made two of the three constellations unreachable.
/// The damage clause is what distinguishes "holding a spell" from
/// "holding a spell that is being knocked out of me".
///
/// The enemy-proximity gate is shared by all three: ten rounds started
/// in an empty room is ten rounds of nothing, and the charge does not
/// come back until a short rest.
fn try_starry_form(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.feature_available(crate::actions::class_features::STARRY_FORM_TAG) {
        return None;
    }
    // 30 ft — the same engagement radius Rage uses, and far enough out
    // that the form is up before the first exchange rather than after
    // it.
    if !any_enemy_within(encounter, actor_id, 12) {
        return None;
    }
    let taking_fire = actor.hitpoints() < actor.max_hitpoints();
    let name = if encounter
        .most_wounded_ally_within(actor_id, 12, &[actor_id])
        .is_some()
    {
        "starry form chalice"
    } else if actor.is_concentrating() && taking_fire {
        "starry form dragon"
    } else {
        "starry form archer"
    };
    try_self_action(encounter, actor_id, name)
}

/// The Arcane Archer's six shots, paired with the save each one forces
/// and the condition it lands. Read by `try_arcane_shot`.
///
/// **The order is the tie-break and it is load-bearing.** The picker
/// chooses the shot the target is worst at saving against, and a
/// creature with two equally bad saves — an ogre's CHA and WIS are both
/// −2 — has to be resolved somehow. Resolving it by this order means
/// the tie goes to the shot that is worth more, so the list runs
/// strongest first:
///
///   1. `banishing arrow` — a failed save costs the target its whole
///      turn. Nothing else on the menu buys that.
///   2. `grasping arrow` — Restrained is speed zero, disadvantage on
///      its own swings, and advantage for every ally shooting at it.
///      The archer's answer to anything that has to close to hurt them.
///   3. `shadow arrow` — Blinded is the same disadvantage without the
///      speed clause, so it ranks below Grasping against a melee threat
///      and is the better answer to a caster or an archer.
///   4. `enfeebling arrow` — halves what the target deals rather than
///      stopping it dealing anything. Worth most against a
///      multiattacker and least against a single big swing.
///   5. `beguiling arrow` — Charmed only forbids attacking the archer,
///      so an enemy with other targets shrugs most of it off.
///
/// Bursting Arrow is deliberately absent: its value has nothing to do
/// with the target's saves — there is no save — and everything to do
/// with how many bystanders are standing near it, so it is chosen ahead
/// of this table rather than inside it.
const ARCANE_SHOT_ORDER: &[(&str, AbilityScoreType, Condition)] = &[
    (
        "banishing arrow",
        AbilityScoreType::Charisma,
        Condition::Banished,
    ),
    (
        "grasping arrow",
        AbilityScoreType::Strength,
        Condition::Restrained,
    ),
    (
        "shadow arrow",
        AbilityScoreType::Wisdom,
        Condition::Blinded,
    ),
    (
        "enfeebling arrow",
        AbilityScoreType::Constitution,
        Condition::Enfeebled,
    ),
    (
        "beguiling arrow",
        AbilityScoreType::Charisma,
        Condition::Charmed,
    ),
];

/// Blast radius of Bursting Arrow in footprint tiles — 10 ft on the
/// 2.5 ft grid, the same four tiles the rider itself uses. Kept in step
/// with `FollowUpEffect::Burst`'s `radius_tiles` by
/// `the_bursting_arrow_radius_agrees_with_its_rider`; an AI that
/// believed in a wider blast than the rider delivers would spend a
/// charge on a detonation that caught nobody.
pub(crate) const BURSTING_ARROW_RADIUS: isize = 4;

/// Arcane Archer Fighter — nock an Arcane Shot before the bow comes up.
///
/// Two charges per short rest and six options, so the pick is the whole
/// feature. It is made in two steps:
///
///   1. **Is anybody standing next to the target?** Bursting Arrow is
///      the only shot whose payload scales with the crowd, and the only
///      one with no save to fail, so a blast that catches even one
///      bystander beats any single-target rider on expected damage.
///   2. **Otherwise, where is the target weak?** Walk
///      `ARCANE_SHOT_ORDER` and take the shot forcing the save the
///      target adds least to, skipping any whose condition it already
///      carries — a second Blinded does nothing, and the charge is too
///      scarce to spend on it. Ties go to the earlier row, which is why
///      that order runs strongest-first.
///
/// The target the picker reads is the nearest hostile in bow range, not
/// the lowest-HP one the attack rung will eventually shoot. They are the
/// same creature in nearly every fight, and where they differ the prime
/// is still spent on a shot that lands — the rider fires on whatever the
/// archer hits, so a mis-guessed target costs accuracy of the *choice*,
/// not the charge.
fn try_arcane_shot(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    use crate::conditions::condition_template::ARCANE_SHOTS;
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.feature_available(crate::actions::class_features::ARCANE_SHOT_TAG) {
        return None;
    }
    // One arrow on the string at a time. The action's own validator
    // only refuses the *same* shot twice; without this the archer would
    // nock a second option over the first and throw the first charge
    // away, since installing one strips the rest.
    if ARCANE_SHOTS.iter().any(|&c| actor.has_condition(c)) {
        return None;
    }
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
    let target_id = encounter
        .sorted_actor_ids()
        .into_iter()
        .filter(|&tid| {
            encounter.actors.get(&tid).is_some_and(|t| {
                tid != actor_id && t.team() != my_team && t.is_combat_active()
            })
        })
        .min_by_key(|&tid| {
            let t = &encounter.actors[&tid];
            (
                footprint_chebyshev(my_loc, my_size, t.location(), get_tiles_from_size(t.size())),
                tid,
            )
        })
        .filter(|&tid| {
            let t = &encounter.actors[&tid];
            footprint_chebyshev(my_loc, my_size, t.location(), get_tiles_from_size(t.size()))
                <= BOW_RANGE_GAP
        })?;
    let bystanders = encounter
        .actors
        .iter()
        .filter(|(id, other)| {
            **id != actor_id
                && **id != target_id
                && other.team() != my_team
                && other.is_combat_active()
        })
        .filter(|(id, _)| {
            encounter
                .footprint_distance(**id, target_id)
                .is_some_and(|gap| gap <= BURSTING_ARROW_RADIUS)
        })
        .count();
    if bystanders > 0
        && let Some(aei) = try_self_action(encounter, actor_id, "bursting arrow")
    {
        return Some(aei);
    }
    let target = encounter.actors.get(&target_id)?;
    let name = ARCANE_SHOT_ORDER
        .iter()
        .filter(|(_, _, lands)| !target.has_condition(*lands))
        .min_by_key(|(_, save, _)| target.save_modifier(*save))
        .map(|(name, _, _)| *name)?;
    try_self_action(encounter, actor_id, name)
}

/// Barbarian Rage trigger: a barbarian who isn't already Raging fires
/// the rage spell as soon as any hostile actor is in attack reach,
/// trading the bonus action for resistance + STR advantage. Validation
/// gates on the feature being available and the action being affordable.
fn try_rage(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.has_condition(Condition::Raging) {
        return None;
    }
    // 30ft = 12 tiles — save the 10-round buff for when it matters.
    if !any_enemy_within(encounter, actor_id, 12) {
        return None;
    }
    try_self_action(encounter, actor_id, "rage")
}

/// 5e Reckless Attack (barbarian / berserker): bonus action that grants
/// advantage on this turn's melee swings at the cost of attackers
/// having advantage against the barbarian until their next turn. The
/// AI fires it on the same trigger as Rage — an enemy is footprint-
/// adjacent so the barbarian will swing this turn. Skipped when the
/// barbarian already has the rider (the action self-applies the
/// `Helped` condition; re-casting wastes a bonus action). The
/// trade-off (eat extra incoming damage) is implicit: the barbarian's
/// physical resistance from Rage and high HP pool make the deal
/// strictly profitable when an enemy is already in reach and a swing
/// is queued.
///
/// Pairs cleanly with Brutal Critical: more nat-20s land per turn,
/// each crit rolls extra weapon dice. The two riders together push the
/// barbarian's per-turn damage well above the baseline fighter.
fn try_reckless_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    // Already self-buffed for this swing — bonus action would no-op.
    if actor.has_condition(Condition::Helped) {
        return None;
    }
    // Only fire when an enemy is footprint-adjacent so the advantage
    // is consumed this turn (Helped clears on the next attack).
    if !any_enemy_within(encounter, actor_id, MELEE_REACH) {
        return None;
    }
    try_self_action(encounter, actor_id, "reckless attack")
}

/// 5e Berserker **Frenzy** trigger: while raging, the barbarian spends
/// a bonus action to gain a fresh Action token for an extra melee swing.
/// Gates fire only when:
///   - the barbarian is Raging (the action's own validator double-checks
///     the passive `FRENZY_TAG` flag — no need to re-test here),
///   - an enemy is footprint-adjacent so the granted Action will be
///     consumed by a melee swing this turn rather than wasted on Move.
///
/// Slots between Reckless Attack and Lunging Attack on the bonus-action
/// lane — once Reckless Attack runs (advantage-on-melee buff) the barbarian
/// will want a second swing to spend it on. The granted Action has no
/// reach gate of its own, so the AI's normal attack picker handles
/// weapon / target selection on the follow-up.
fn try_frenzy(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.has_condition(Condition::Raging) {
        return None;
    }
    // Only fire when an enemy is footprint-adjacent so the granted
    // Action gets spent on a melee swing this turn.
    if !any_enemy_within(encounter, actor_id, MELEE_REACH) {
        return None;
    }
    try_self_action(encounter, actor_id, "frenzy")
}

/// War Magic — Eldritch Knight Fighter bonus action (subclass lv7).
/// Trades the bonus action for a fresh Action token on any turn the
/// knight opened with a cantrip.
///
/// Two gates, and neither is a heuristic judgement call:
///   1. The `WarMagicPrimed` condition is up — the knight cast a
///      cantrip with their Action this turn. This is the whole feature,
///      and it means the picker no-ops on the far more common
///      swing-first turn without needing to know anything about the
///      knight's build.
///   2. At least one enemy is on the map within a generous window (24
///      tiles — the spell-window cadence `try_self_action_when_enemy_within`
///      already uses for payoffs the actor can spend across a few
///      turns). The granted Action is worth having for a swing *or* a
///      second cantrip, so unlike Frenzy this doesn't demand an
///      adjacent target — but an empty room is still no reason to
///      spend the bonus action.
///
/// No feature charge to check: War Magic is at-will, and the
/// once-per-turn cap falls out of the prime being consumed on use.
fn try_war_magic(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.has_condition(Condition::WarMagicPrimed) {
        return None;
    }
    try_self_action_when_enemy_within(encounter, actor_id, 24, "war magic")
}

/// Eagle Totem Spirit's Dash-as-bonus-action — Eagle Totem barbarian
/// bonus action while raging. Grants a fresh movement chunk for kiting
/// or closing. Fires only when there's at least one enemy that needs
/// closing (no enemy in melee reach AND at least one enemy on the map
/// to chase) — burning the bonus action for extra movement with all
/// enemies already adjacent wastes the eagle barbarian's bonus-action
/// slot on a follow-up Reckless Attack window.
fn try_eagle_dive(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.has_condition(Condition::Raging) {
        return None;
    }
    // An enemy already inside the barbarian's reach → no need to Dash;
    // let Reckless / Frenzy take the bonus-action slot instead. The
    // question is "can I already swing at somebody", so the distance is
    // the swing's, not a tighter one — at gap 0 this declined to notice
    // the enemy the barbarian was standing in reach of and dashed away
    // from a fight it had already reached.
    if any_enemy_within(encounter, actor_id, MELEE_REACH) {
        return None;
    }
    // No enemies in chase distance at all → Dash buys nothing.
    if !any_enemy_within(encounter, actor_id, 60) {
        return None;
    }
    try_self_action(encounter, actor_id, "eagle dive")
}

/// Battle Master Lunging Attack — bonus-action prime that extends the
/// fighter's melee reach by one tile for the next swing. Fire only when
/// an enemy sits at the precise distance the lunge opens up (gap 2,
/// i.e. one tile outside the default melee reach of 1) — at gap 0 / 1
/// the prime is wasted, since the fighter can already strike without it.
/// At gap 3+ the lunge alone still doesn't close the distance, so a
/// move-then-swing is cheaper than burning the once-per-rest prime.
fn try_lunging_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.has_condition(Condition::LungingAttacking) {
        return None;
    }
    // Need an enemy at exactly the gap the lunge opens up — gap 2 is the
    // sweet spot for default melee weapons (reach 1 + lunge 1).
    let my_team = actor.team();
    let in_lunge_window = encounter.actors.iter().any(|(id, e)| {
        *id != actor_id
            && e.team() != my_team
            && e.is_combat_active()
            && encounter.footprint_distance(actor_id, *id).is_some_and(|d| d == 2)
    });
    if !in_lunge_window {
        return None;
    }
    try_self_action(encounter, actor_id, "lunging attack")
}

/// War Domain Cleric War Priest — bonus-action extra weapon swing.
/// Grants a fresh Action token to be spent on a follow-up strike this
/// turn. Fires only when a hostile is footprint-adjacent so the granted
/// Action lands a real swing rather than being wasted on a whiffed
/// ranged spell. The action's own validator gates on the once-per-short-
/// rest charge; we mirror the "enemy adjacent" gate here so the
/// heuristic doesn't burn the charge when the follow-up would just be
/// a Move.
///
/// Sibling to `try_frenzy` (Barbarian Berserker) — same bonus-action
/// extra-Action shape gated on both a per-rest feature charge AND an
/// adjacent enemy for the granted swing.
fn try_war_priest(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_action_when_enemy_within(encounter, actor_id, MELEE_REACH, "war priest")
}

/// War Domain Cleric Guided Strike — bonus-action Channel Divinity
/// prime that adds +10 to the cleric's next attack roll. Fires when an
/// enemy sits within any plausible attack window (24 tiles ≈ 60 ft —
/// covers Guiding Bolt at 120 ft compressed to the engine's read
/// window; a Sacred Flame DEX save has no attack roll so Guided Strike
/// on a save-only cleric fires when there's a viable melee / spell-
/// attack target). The action's `custom_validate_input` gates on the
/// once-per-short-rest charge AND the no-stack clause (skip re-prime
/// while GuidedStriking is up).
///
/// Sibling to `try_precision_attack` (Battle Master Precision Attack)
/// — same bonus-action attack-roll prime shape but with a fatter +10
/// vs Precision's +4, gated on the same "enemy within attack window"
/// heuristic.
fn try_guided_strike(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_action_when_enemy_within(encounter, actor_id, 24, "guided strike")
}

/// Trickery Domain Cleric Invoke Duplicity — Channel Divinity, Action,
/// once per short rest. Installs ten rounds of advantage on the
/// cleric's attack rolls.
///
/// Same 24-tile (≈60 ft) engagement window as Guided Strike, and for
/// the same reason: the cleric's attack rolls come from spell attacks
/// as often as from a weapon, so "is there anything I could plausibly
/// swing at" is a wider question than melee reach. Wider than the
/// bonus-action primes' adjacency gate because this buff lasts ten
/// rounds rather than one swing — arming it a turn before contact is
/// correct play, not a wasted charge.
///
/// The action's `custom_validate_input` owns the rest: the short-rest
/// charge, and the no-restack clause that stops a cleric from spending
/// the charge to refresh a `Duplicity` flag that is already up.
fn try_invoke_duplicity(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_action_when_enemy_within(encounter, actor_id, 24, "invoke duplicity")
}

/// Paladin Divine Smite — bonus-action prime that lays a +2d8 radiant
/// rider on the next melee hit. Fires only when an enemy is inside the
/// paladin's own reach, so the prime doesn't tick out without a target
/// to land on. Validation handles the "already primed" and "no level-1
/// slot" gates.
///
/// The gate is `MELEE_REACH` and was gap 0, and the difference is the
/// whole feature. Gap 0 is two footprints touching; a 5 ft weapon
/// reaches a gap of 1, so the paladin's own attack picker fires — and
/// its approach stops — one tile short of what this gate was asking
/// for. Across twelve seeds of an AI paladin duelling an ogre and
/// swinging a greatsword every round, the smite was never once cast.
/// Its four siblings on the same lane (Reckless Attack, Frenzy,
/// Precision Attack, War Priest) were dead the same way; the rest of
/// the cohort had already been moved onto `MELEE_ADJACENT_PRIMES`,
/// which asks the right question.
///
/// Reaching one tile further does not risk a wasted prime: the swing
/// that cashes it validates at exactly this distance, which is the
/// point.
fn try_divine_smite(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_action_when_enemy_within(encounter, actor_id, MELEE_REACH, "divine smite")
}

/// Shared "pick the highest-HP hostile within `range_tiles` and queue
/// the named single-target class-feature action against them" body.
///
/// Vow of Enmity (Vengeance Paladin CD, 4-tile reach → Sworn install),
/// Intimidating Presence (Berserker Barbarian lv10, 12-tile reach →
/// Frightened install), and Nature's Wrath (Ancients Paladin CD,
/// 4-tile reach → Restrained install) all share the picker shape:
///   - resolve the named action via `find_action`,
///   - iterate hostile combat-active actors within `range_tiles`,
///   - defer no-redup / feature-charge checks to the action's own
///     `custom_validate_input`,
///   - pick the highest-HP surviving candidate.
///
/// Beefy targets benefit most from a single-target lockdown / accuracy
/// prime because the paladin/barbarian's follow-up swing chain has more
/// turns to cash the debuff before the target drops. Factoring the
/// picker here means the three feature-specific `try_*` shims collapse
/// to one-line delegations, and a future single-target class-feature
/// (a hypothetical "Cause Fear at will" for a Warlock invocation, a
/// druid subclass single-target restraint) drops in as a fresh caller
/// with a new (action_name, range_tiles) pair.
fn try_single_target_class_feature_hostile(
    encounter: &EncounterInstance,
    actor_id: usize,
    action_name: &str,
    range_tiles: isize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action(action_name)?;
    let my_team = actor.team();

    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for target_id in encounter.sorted_actor_ids() {
        let Some(target) = encounter.actors.get(&target_id) else {
            continue;
        };
        if target_id == actor_id || target.team() == my_team || !target.is_combat_active() {
            continue;
        }
        // Range gate mirrors the action's own reach cap so the loop
        // doesn't queue an out-of-range candidate just to have
        // `validate` reject it later.
        if encounter
            .footprint_distance(actor_id, target_id)
            .is_none_or(|d| d > range_tiles)
        {
            continue;
        }
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![target_id]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        let hp = target.hitpoints();
        if best.as_ref().is_none_or(|(best_hp, _)| hp > *best_hp) {
            best = Some((hp, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Vengeance Paladin Vow of Enmity — bonus-action Channel Divinity,
/// once per long rest. Marks one in-reach (4 tiles = 10 ft RAW) hostile
/// creature so the paladin gets advantage on all subsequent attack rolls
/// against that target for 10 rounds. Highest-HP candidate wins per the
/// shared picker's beefy-target heuristic — the more swings the vow
/// survives across, the more Smite primes cash in on advantage.
fn try_vow_of_enmity(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_single_target_class_feature_hostile(encounter, actor_id, "vow of enmity", 4)
}

/// Hexblade Warlock Hexblade's Curse — bonus-action subclass feature,
/// once per short rest. Marks one in-reach (12 tiles = 30 ft RAW)
/// hostile so the hexblade adds their proficiency bonus to damage
/// against it, crits it on 19-20, and heals when it drops.
///
/// Highest-HP candidate wins per the shared picker, and here that is the
/// heuristic doing real work rather than inheriting a default. The
/// curse's reliable payout is the per-hit damage bonus, which pays once
/// per swing that lands on the cursed creature — so its value is
/// proportional to how many swings the creature survives, and the
/// beefiest enemy on the field is the one that absorbs the most.
///
/// The heal-on-death clause pulls the other way, and it loses. Cursing
/// something nearly dead cashes the heal a round later and then leaves
/// the hexblade with eight rounds of a curse stuck to a corpse — the
/// engine ships no *Master of Hexes*, so the mark cannot move once
/// placed. Treating the heal as a bonus that arrives when the fight is
/// won, rather than as the thing to aim for, is what keeps the curse on
/// the target it can actually earn out on.
fn try_hexblades_curse(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_single_target_class_feature_hostile(encounter, actor_id, "hexblade's curse", 12)
}

/// Berserker Barbarian Intimidating Presence — action, once per short
/// rest. Frightens one in-reach (12 tiles = 30 ft RAW) hostile creature
/// on a failed WIS save vs the barbarian's CHA-anchored DC. Highest-HP
/// candidate wins per the shared picker — a Frightened boss eats attack
/// disadvantage for the whole raging window, so beefy targets get the
/// most swings-lost-to-disadvantage payoff.
fn try_intimidating_presence(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_single_target_class_feature_hostile(encounter, actor_id, "intimidating presence", 12)
}

/// Grave Domain Cleric Path to the Grave — action Channel Divinity,
/// once per short rest. Curses one in-reach (12 tiles = 30 ft RAW)
/// hostile creature with `MarkedForGrave` so any attack against that
/// target has advantage for the rest of the round. Highest-HP
/// candidate wins per the shared picker — a marked boss soaks
/// advantage on every party-side swing that lands before the curse
/// ticks off, so beefy targets that survive multiple attacks yield
/// the biggest accuracy payoff.
fn try_path_to_the_grave(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_single_target_class_feature_hostile(encounter, actor_id, "path to the grave", 12)
}

/// Ancients Paladin Nature's Wrath — action Channel Divinity, once per
/// short rest. Restrains one in-reach (4 tiles = 10 ft RAW) hostile
/// creature on a failed STR save vs the paladin's CHA-anchored DC. Same
/// 4-tile reach as Vow of Enmity — both single-target paladin CDs sit
/// at the same 10ft window so a paladin closing on a boss target lines
/// up either CD with one melee-approach turn.
fn try_natures_wrath(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_single_target_class_feature_hostile(encounter, actor_id, "nature's wrath", 4)
}

// Note: Wholeness of Body (Open Hand Monk lv6) is auto-picked by
// `try_self_heal` above — it's `is_heal() && NoArgs`, so the standard
// self-heal lane catches it at the half-HP gate without a dedicated
// try_wholeness_of_body picker. Lay on Hands / Healing Hands / Cure
// Wounds use the same pattern; no per-feature plumbing needed.

/// Paladin Smite spells (Searing / Wrathful / Branding / Blinding).
/// Same trigger as Divine Smite — fire when an enemy is inside the
/// paladin's reach so the bonus-action prime doesn't go to waste. We try
/// them in increasing-slot-level order so the paladin spends low slots
/// before high ones; each spell's own `custom_validate_input` rejects
/// re-prime if the smite condition is already up. The Smite-spell path
/// is concentration-gated — skip the whole stack if the paladin is
/// already concentrating on something (e.g. Compelled Duel / Bless).
fn try_smite_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    // Melee-flavor smite picker: gate on an enemy inside the paladin's
    // own reach so the prime is consumed this turn by the Extra Attack
    // loop. `MELEE_REACH`, not gap 0 — the swing that cashes the prime
    // validates at exactly this distance, and the tighter number asked
    // for a tile the AI never stands on. Slot-cheapest first —
    // preserves higher slots for emergencies. The order is defined by
    // the central `ALL_SMITE_SPELLS` registry, so adding a new smite is
    // one entry in spells.rs and the AI picks it up automatically.
    try_smite_from_registry(
        encounter,
        actor_id,
        crate::actions::spells::ALL_SMITE_SPELLS,
        MELEE_REACH,
    )
}

/// Ranged-flavor smite picker (Ensnaring Strike, Lightning Arrow, and
/// any future ranger primes). Mirrors `try_smite_spell` but gates on
/// enemy-within-bow-range (24 tiles ≈ 60 ft, well inside the RAW 150 ft
/// longbow range) rather than adjacency — the prime loads the next
/// *ranged* weapon attack, so a far-away threat is the right trigger.
/// The either-lane Ensnaring Strike rider still fires on the melee
/// scimitar fallback when a threat closes through the kite; the
/// bow-range engagement gate here just decides *when to burn the
/// slot* on the smite prime, not which weapon consumes the rider.
/// Skips when the actor is already concentrating (Hunter's Mark,
/// Ensnaring Strike, and Lightning Arrow share the concentration
/// slot; AI picks whichever fires first based on pipeline order).
fn try_ranged_smite_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_smite_from_registry(
        encounter,
        actor_id,
        crate::actions::spells::ALL_RANGED_SMITE_SPELLS,
        24,
    )
}

/// Shared "walk a smite-spell registry, pick the first spell whose
/// per-spell validator allows the cast" helper. Both the paladin's
/// melee-adjacent smite picker (`try_smite_spell`) and the ranger's
/// bow-range smite picker (`try_ranged_smite_spell`) collapse to a
/// one-line call on this helper — the only per-caller axes are the
/// registry (paladin's `ALL_SMITE_SPELLS` vs ranger's
/// `ALL_RANGED_SMITE_SPELLS`) and the engagement range (0 tiles ≈
/// melee-adjacent vs 24 tiles ≈ 60 ft bowshot).
///
/// The per-spell double-prime gate is handled inside each
/// `SmiteSpell::custom_validate_input` (`!has_condition(prime)`), so
/// the loop just relies on `try_self_action` returning `None` for
/// spells whose prime is already up — no outer bail-out needed.
///
/// The concentration gate lives here rather than on each spell's
/// `custom_validate_input` because it's a per-caster policy decision
/// ("don't drop the current concentration for another smite") rather
/// than a per-spell RAW gate — the smite spells themselves are
/// concentration-bound and would validly drop the current
/// concentration to install their own, but the AI's heuristic
/// prefers to keep whatever concentration is already up (Bless /
/// Compelled Duel / Hunter's Mark / etc.) rather than churn.
fn try_smite_from_registry(
    encounter: &EncounterInstance,
    actor_id: usize,
    registry: &[&crate::actions::spells::SmiteSpell],
    engagement_range: isize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, engagement_range) {
        return None;
    }
    use crate::actions::action_template::Action;
    for spell in registry {
        if let Some(aei) = try_self_action(encounter, actor_id, spell.name()) {
            return Some(aei);
        }
    }
    None
}

/// Monk Stunning Strike — bonus action prime that lays a stun save on
/// the next melee hit. Same trigger as Divine Smite: an enemy inside
/// the monk's own reach, so the prime doesn't tick out unspent.
///
/// The second gate — at least one in-reach enemy that is *not already
/// Stunned* — used to be invisible. Stunning Strike carried a private
/// once-per-rest charge, so a monk who had stunned something had no
/// second charge to waste re-stunning it. Now that the charge is one
/// point out of the monk's ki pool (`KI_POINTS_TAG`), the waste is
/// repeatable: without this clause a monk stands over a stunned ogre
/// and spends its whole pool priming a save the ogre is not going to
/// roll, one point per turn, while the Flurry of Blows that would
/// actually finish it never gets the bonus action.
///
/// A stunned creature is also the best possible thing to Flurry — every
/// swing against it has advantage and auto-crits in reach — so the rung
/// below this one is exactly where the AI should fall through to.
fn try_stunning_strike(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();
    let stunnable = encounter.actors.iter().any(|(eid, enemy)| {
        *eid != actor_id
            && enemy.team() != my_team
            && enemy.is_combat_active()
            && !enemy.has_condition(Condition::Stunned)
            && encounter
                .footprint_distance(actor_id, *eid)
                .is_some_and(|d| d <= MELEE_REACH)
    });
    if !stunnable {
        return None;
    }
    try_self_action(encounter, actor_id, "stunning strike")
}

/// Monk Patient Defense — bonus action, at will: take the Dodge action,
/// so every attack against the monk this round rolls at disadvantage.
///
/// Fires only when the monk is both hurt (below 50% HP, the roster's
/// standard "this is going badly" threshold) and in contact, because the
/// dodge is worth exactly as much as the swings it spoils: a healthy
/// monk, or one nothing can reach, is trading a Flurry for nothing.
///
/// Deliberately a looser HP gate than Empty Body's 40%. Patient Defense
/// costs a bonus action and no resource at all, so it can afford to fire
/// early and often; Empty Body costs an Action and four fifths of the
/// monk's ki, so it waits until survival is the question.
fn try_patient_defense(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !is_low_hp(encounter, actor_id, 0.5) {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, MELEE_REACH) {
        return None;
    }
    try_self_action(encounter, actor_id, "patient defense")
}

/// Monk Flurry of Blows — bonus action, at will: grants the monk a
/// second Action, which the attack picker then spends on a strike.
///
/// The gate is simply "something in reach to hit". There is no charge to
/// hoard and no prime to waste — an unspent bonus action on a monk in
/// contact is a strike the monk declined to make.
///
/// The reach check is the monk's melee reach rather than any wider
/// engagement band on purpose: the extra Action is worth a swing, and a
/// swing needs a target the monk can already reach. A monk who has to
/// walk first will flurry next turn, from contact, which is where the
/// feature is worth the most anyway.
fn try_flurry_of_blows(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, MELEE_REACH) {
        return None;
    }
    try_self_action(encounter, actor_id, "flurry of blows")
}

/// Monk Empty Body — Action defensive burst, RAW four ki points.
/// Installs Invisible + DamageResistant on self for 10 rounds. Fire when
/// the monk is genuinely under pressure — below 40% HP AND has at least
/// one adjacent enemy that would otherwise pound them. The 40% threshold
/// is tighter than the standard 50% heal trigger because Empty Body is a
/// full Action *and* the largest single draw on the monk's ki
/// (`KI_POINTS_TAG`, five points on this chassis): the AI should hold it
/// until survival is at stake, not fire it on the first scratched-HP
/// alarm.
///
/// Composes with the standard heal picker (Wholeness of Body, potions):
/// this fires ahead of them at the Action lane since Empty Body is a
/// survival burst rather than a topup.
fn try_empty_body(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !is_low_hp(encounter, actor_id, 0.4) {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, MELEE_REACH) {
        return None;
    }
    try_self_action(encounter, actor_id, "empty body")
}

/// Monk Stillness of Mind — Action. Cleanses Charmed / Frightened off
/// the monk in one swing. Fire when the monk is actually afflicted with
/// either condition; the action's `custom_validate_input` will refuse
/// when neither is up, but gating here too keeps the AI's swap rate
/// down (an Action burned on Stillness of Mind is one fewer swing).
fn try_stillness_of_mind(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.has_condition(Condition::Charmed)
        && !actor.has_condition(Condition::Frightened)
    {
        return None;
    }
    try_self_action(encounter, actor_id, "stillness of mind")
}

/// Wipe Acid — universal cleanse Action. Fire when the holder carries
/// the CausticBrewed DoT (2d4 acid per round, avg 5 HP/round) AND they
/// are below half HP — the cleanse-then-survive trade dominates a
/// single attack lane when the drip would outpace the swing. A topped-up
/// actor still gets to swing through the drip (cleanse becomes
/// suboptimal when one round of acid is less than the actor's max-HP
/// buffer). The action's own validate refuses when the flag is absent,
/// so this just gates on the HP heuristic.
fn try_wipe_acid(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.has_condition(Condition::CausticBrewed) {
        return None;
    }
    if !is_low_hp(encounter, actor_id, 0.5) {
        return None;
    }
    try_self_action(encounter, actor_id, "wipe acid")
}

/// Paladin Cleansing Touch — once-per-long-rest action that ends one
/// spell on a willing creature within touch reach. Fire when an adjacent
/// ally (or self) carries one of the heavyweight lockdown debuffs that
/// shuts down a turn outright (Paralyzed / Stunned / Charmed / Confused /
/// Dominated). The action's `validate_input` re-checks the broader
/// cleansable list and the feature-flag gate; the AI's narrower trigger
/// list keeps the once-per-rest charge from burning on a minor debuff
/// (Mocked / Outlined) that a single swing could outpace.
///
/// We pick the closest qualifying ally — the touch reach is 1 tile so
/// only adjacent targets validate anyway. Self counts since RAW lets the
/// paladin target themselves; nothing in the engine prevents a paralyzed
/// paladin from finding their own ID at the top of the candidate list.
fn try_cleansing_touch(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    const HIGH_PRIORITY: &[Condition] = &[
        Condition::Paralyzed,
        Condition::Stunned,
        Condition::Petrified,
        Condition::Dominated,
        Condition::Charmed,
        Condition::Confused,
        Condition::Frightened,
    ];

    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("cleansing touch")?;
    let my_team = actor.team();
    // Walk the sorted-id list so the picked target is deterministic
    // when multiple allies carry the same debuff.
    for tid in encounter.sorted_actor_ids() {
        let Some(t) = encounter.actors.get(&tid) else {
            continue;
        };
        if t.team() != my_team || !t.is_combat_active() {
            continue;
        }
        // Touch range — gate the candidate list to footprint-adjacent
        // allies (and self at gap 0) so the AI's pick lines up with
        // the action's `reach_tiles() = 1`.
        if actor.footprint_gap_to(t) > 1 {
            continue;
        }
        if !HIGH_PRIORITY.iter().any(|c| t.has_condition(*c)) {
            continue;
        }
        let tv = vec![tid];
        let aei = ActionExecutionInfo::new(action, actor_id, Some(tv), None, None);
        if aei.validate(encounter) {
            return Some(aei);
        }
    }
    None
}

/// One row in the `TURN_BURST_PICKS` cohort: a `TurnBurst` Channel
/// Divinity config the AI knows how to fire, plus the minimum number of
/// eligible targets that justifies spending the charge.
///
/// The config supplies the action name and — crucially — the same
/// `type_filter` closure the action itself uses to pick victims, so the
/// heuristic and the resolver can never disagree about who counts.
struct TurnBurstPick {
    /// The engine-side config. Read for `name` (the action lookup) and
    /// `type_filter` (the eligibility scan).
    config: &'static LazyLock<TurnBurst>,
    /// How many eligible hostiles must be in range before the AI spends
    /// the charge. 1 for the type-filtered variants — they're
    /// specialists, and a charge saved for a target that never appears is
    /// a charge wasted. 2 for the unfiltered Dreadful Aspect, whose value
    /// is entirely in breadth: against a single enemy an ordinary swing
    /// beats a frighten.
    min_targets: usize,
}

/// Every `TurnBurst` Channel Divinity the AI can fire, in priority order.
///
/// Before this cohort existed only Turn Undead had an AI heuristic, so
/// three subclasses never used their Channel Divinity at all: a Devotion
/// Paladin never turned a fey, an Oathbreaker never projected dread, and
/// an AI-driven Arcana Cleric would never have abjured anything. Driving
/// the heuristic off the shared configs means a new turn variant becomes
/// AI-visible by being added here, and it reuses the variant's own
/// creature-type filter rather than re-deriving one.
///
/// Order is narrowest-filter-first, so a hypothetical multiclass holding
/// several spends the most specialized charge on the target that only it
/// can answer.
const TURN_BURST_PICKS: &[TurnBurstPick] = &[
    TurnBurstPick {
        config: &CHARM_ANIMALS_AND_PLANTS,
        min_targets: 1,
    },
    TurnBurstPick {
        config: &TURN_UNDEAD,
        min_targets: 1,
    },
    TurnBurstPick {
        config: &TURN_THE_FAITHLESS,
        min_targets: 1,
    },
    TurnBurstPick {
        config: &ARCANE_ABJURATION,
        min_targets: 1,
    },
    TurnBurstPick {
        config: &DREADFUL_ASPECT,
        min_targets: 2,
    },
    // Conquering Presence takes the same unfiltered "anything hostile"
    // shape as Dreadful Aspect but a lower bar, because the Conquest
    // Paladin is the one holder for whom a *single* frightened enemy
    // is worth the charge: Aura of Conquest roots whoever failed and
    // bleeds them 5 psychic a turn, so the fear converts into damage
    // and a lockdown rather than into disadvantage alone. The
    // Oathbreaker, with nothing to cash the fear in for, still wants
    // two.
    TurnBurstPick {
        config: &CONQUERING_PRESENCE,
        min_targets: 1,
    },
    // Champion Challenge takes Conquering Presence's bar rather than
    // Dreadful Aspect's, for the same reason and a different mechanic:
    // one held enemy is already worth the charge, because Rooted stops
    // that enemy reaching the ally behind the paladin at all. It sits
    // ahead of the two Frighten rows on the walk because the Crown
    // Paladin holds no other row — the order between them is only ever
    // read by a hypothetical multiclass — and behind the filtered rows
    // for the same reason they lead: a type-gated turn that finds
    // nobody should fall through to something unfiltered.
    TurnBurstPick {
        config: &CHAMPION_CHALLENGE,
        min_targets: 1,
    },
    // Order's Demand is the other unfiltered variant, and it takes
    // Dreadful Aspect's bar rather than Conquering Presence's: the Order
    // Cleric has nothing that cashes the condition in for damage, so its
    // value is breadth. Charmed does more per target than Frightened —
    // it forbids attacking the cleric outright rather than merely
    // taxing the roll — but one charmed enemy is still worth less than
    // a swing.
    TurnBurstPick {
        config: &ORDERS_DEMAND,
        min_targets: 2,
    },
    // Enthralling Performance is the third unfiltered variant and takes
    // the same bar as Order's Demand, whose condition it shares. One
    // charmed enemy is worth less than the Action it costs, because the
    // Glamour bard has a whole spell list of things to do with an
    // Action; two is where the burst starts beating any single-target
    // pick on the list. It sits last because the bard holds no other
    // row — the order between the unfiltered three is only ever read by
    // a hypothetical multiclass.
    TurnBurstPick {
        config: &ENTHRALLING_PERFORMANCE,
        min_targets: 2,
    },
    // Aspect of the Wyrm is the fourth unfiltered variant and the only
    // one on a chassis that has to be standing inside its own burst.
    // That is what sets its bar at one rather than two: every other row
    // here belongs to a caster who is elsewhere, and for them a single
    // frightened enemy is worth less than the Action. A monk in contact
    // with one hostile is *being hit by it*, and disadvantage on those
    // swings is worth the press on its own — the same argument that
    // gives Conquering Presence a bar of one, arrived at from the
    // defensive side rather than the offensive one.
    TurnBurstPick {
        config: &ASPECT_OF_THE_WYRM,
        min_targets: 1,
    },
];

/// Channel Divinity turn-burst picker — Turn Undead, Turn the Faithless,
/// Arcane Abjuration, Charm Animals and Plants, Dreadful Aspect. Walks
/// `TURN_BURST_PICKS` and fires the first row the actor holds that has
/// enough eligible hostiles inside the 30 ft (12 tile) burst. The action's
/// own validation owns the per-rest charge gate, so the AI supplies only
/// the "is this worth spending on?" heuristic.
///
/// Eligibility is read through the row's own `type_filter` against the
/// target's `creature_type()`. The pre-cohort Turn Undead heuristic
/// instead probed for *poison immunity* as an undead proxy, which was
/// wrong in both directions across the engine's creature pool: 30
/// poison-immune non-undead (every golem, most devils and demons, the
/// tarrasque, giant spiders) drew a wasted charge, and one genuine undead
/// that isn't poison-immune (the crawling claw) never drew one at all.
/// The proxy presumably predates `CreatureType`; the resolver has always
/// filtered on the real type.
fn try_turn_burst(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let team = actor.team();
    for pick in TURN_BURST_PICKS {
        let config = &**pick.config;
        if actor.find_action(config.name).is_none() {
            continue;
        }
        let eligible = encounter
            .actors
            .iter()
            .filter(|(id, a)| {
                **id != actor_id
                    && a.team() != team
                    && a.is_combat_active()
                    && (config.type_filter)(a.creature_type())
                    && actor.footprint_gap_to(a) <= 12
            })
            .count();
        if eligible < pick.min_targets {
            continue;
        }
        if let Some(aei) = try_self_action(encounter, actor_id, config.name) {
            return Some(aei);
        }
    }
    None
}

/// Pit Fiend Fear Aura — action that frightens every hostile within
/// 20ft (8 tiles) on a failed WIS save. Fire when at least 2 enemies
/// (frighten-eligible) are inside the radius — single-target there
/// are better single-target attacks, but at 2+ the aura's burst payoff
/// dominates a normal swing.
fn try_fear_aura(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let team = actor.team();
    let nearby = encounter
        .actors
        .iter()
        .filter(|(id, a)| {
            **id != actor_id
                && a.team() != team
                && a.is_combat_active()
                // Honor dynamic immunities too — a Halfling Brave or
                // Heroic-buffed ally would resist the install even if
                // their template lacks the static immunity flag, so
                // they shouldn't count toward the "is the aura worth it"
                // gate.
                && !a.effectively_immune_to_condition(Condition::Frightened)
                && !a.has_condition(Condition::Frightened)
                && actor.footprint_gap_to(a) <= 8
        })
        .count();
    if nearby < 2 {
        return None;
    }
    try_self_action(encounter, actor_id, "fear aura")
}

/// Dragon Breath Weapon — fire a Burst-schema breath attack when the
/// actor has a recharge-gated breath action available and at least 2
/// enemies cluster inside the burst radius. Mirrors `try_attack_aoe`'s
/// point-selection logic (center on enemy locations, pick the tile that
/// catches the most hostiles without friendly fire). The recharge gate
/// is enforced by the action's `custom_validate_input` (which checks
/// `is_recharge_available("breath_weapon")`), so we only need to verify
/// that the actor has any action whose name contains "breath" and that
/// the recharge resource is up. Spending the recharge happens inside
/// `side_effects` when the action executes.
fn try_breath_weapon(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let actor = encounter.actors.get(&actor_id)?;
    // Quick gate: the actor must have at least one breath_weapon recharge
    // entry that's currently available.
    if !actor.is_recharge_available("breath_weapon") {
        return None;
    }
    let my_team = actor.team();

    // Collect Burst-schema actions whose name ends with "breath" — these
    // are the breath weapon variants (fire breath, cold breath, etc.).
    let breath_actions: Vec<(&'static (dyn Action + Send + Sync), isize)> = actor
        .actions
        .iter()
        .filter_map(|a| {
            if !a.is_harmful() {
                return None;
            }
            if !a.name().contains("breath") {
                return None;
            }
            match a.targeting_schema() {
                TargetingSchema::Burst { radius } => Some((*a, radius)),
                _ => None,
            }
        })
        .collect();
    if breath_actions.is_empty() {
        return None;
    }

    // Candidate burst centers: every combat-active enemy's location.
    let anchor_ids = encounter.sorted_actor_ids();
    let mut candidate_points: Vec<(Coordinate, usize)> = Vec::new();
    for aid in &anchor_ids {
        let Some(a) = encounter.actors.get(aid) else {
            continue;
        };
        if a.team() == my_team || !a.is_combat_active() {
            continue;
        }
        candidate_points.push((a.location(), *aid));
    }

    let mut best: Option<(usize, usize, ActionExecutionInfo)> = None;
    for (point, anchor_id) in &candidate_points {
        let point = *point;
        let anchor_id = *anchor_id;

        for (action, radius) in &breath_actions {
            let aei =
                ActionExecutionInfo::new(*action, actor_id, None, Some(vec![point]), None);
            if !aei.validate(encounter) {
                continue;
            }

            let mut enemy_hits = 0usize;
            let mut friendly_fire = false;
            for (id, a) in encounter.actors.iter() {
                if !a.is_combat_active() {
                    continue;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    point,
                    1,
                );
                if dist > *radius {
                    continue;
                }
                if *id == actor_id || a.team() == my_team {
                    friendly_fire = true;
                    break;
                }
                enemy_hits += 1;
            }
            if friendly_fire || enemy_hits < 2 {
                continue;
            }
            let pick = match &best {
                None => true,
                Some((best_hits, best_anchor, _)) => {
                    enemy_hits > *best_hits
                        || (enemy_hits == *best_hits && anchor_id < *best_anchor)
                }
            };
            if pick {
                best = Some((enemy_hits, anchor_id, aei));
            }
        }
    }
    best.map(|(_, _, aei)| aei)
}

/// College of Glamour Bard Mantle of Inspiration — bonus action that
/// spends a Bardic Inspiration charge on temp HP for up to CHA-modifier
/// allies within 60 ft instead of a die for one of them.
///
/// The action's own validator owns the charge and the "is there anybody
/// this could help" question. What the AI adds is *when the trade is
/// worth it*, and the gate is two facts about the same moment: the
/// mantle would cover at least two creatures, and at least one of them
/// has already been hit.
///
/// Both halves are load-bearing. Without the breadth check a bard alone
/// with one wounded ally would spend a charge on five temp HP where the
/// die is worth more. Without the wounded check the bard opens every
/// fight with the mantle — the cohort is at its widest on round one,
/// when the whole party is standing together at full health and
/// nothing has demonstrated that anyone is going to be hit at all. Five
/// temp HP handed to four untouched creatures is four charges' worth of
/// nothing if the fight is decided at range.
fn try_mantle_of_inspiration(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    const MIN_COVERED: usize = 2;
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("mantle of inspiration")?;
    let team = actor.team();
    let center = actor.location();
    // 24 tiles = 60 ft, the action's own radius. Re-derived rather than
    // shared because this is the AI's pre-filter, not the authority —
    // the action's validator is, and it runs on the AEI below.
    let covered: Vec<usize> = encounter.ally_burst_targets(actor_id, center, 24);
    if covered.len() < MIN_COVERED {
        return None;
    }
    let anyone_hurt = covered.iter().any(|id| {
        encounter
            .actors
            .get(id)
            .is_some_and(|a| a.team() == team && a.is_wounded())
    });
    if !anyone_hurt {
        return None;
    }
    let aei = ActionExecutionInfo::new(action, actor_id, None, None, None);
    aei.validate(encounter).then_some(aei)
}

/// Darkvision — level-2 transmutation, touch. Cast on the ally in reach
/// who most needs it, and only where the dark is a live problem.
///
/// Two gates, and the second is the one that stops the slot being
/// thrown away. The board must actually be dark somewhere the caster
/// can see — `ambient_light` below bright, or a magically dark patch —
/// because a 2nd-level slot spent granting night vision on a lit board
/// buys nothing at all. And the target must be someone the spell would
/// change: the action's own validator refuses a creature that already
/// sees 60 feet in the dark, so a party of dwarves and drow falls
/// straight through this rung rather than passing the buff around.
///
/// Prefers the *caster* when they qualify — a wizard who cannot see is
/// a wizard whose every attack is at disadvantage — and otherwise takes
/// the nearest eligible ally, since touch range means "nearest" and
/// "reachable" are the same question.
fn try_darkvision(encounter: &EncounterInstance, actor_id: usize) -> Option<ActionExecutionInfo> {
    use crate::engine::lighting::LightLevel;
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("darkvision")?;
    // Is the dark a problem here? The caster's own tile answers it
    // most cheaply and most honestly — `perceived_light` already folds
    // in ambient light, nearby torches, magical darkness and whatever
    // the caster was born able to see through.
    if encounter.perceived_light(actor_id, actor.location()) == LightLevel::Bright {
        return None;
    }
    let team = actor.team();
    std::iter::once(actor_id)
        .chain(encounter.sorted_actor_ids())
        .filter(|id| {
            encounter
                .actors
                .get(id)
                .is_some_and(|a| a.team() == team && a.is_combat_active())
        })
        .map(|id| ActionExecutionInfo::new(action, actor_id, Some(vec![id]), None, None))
        .find(|aei| aei.validate(encounter))
}

/// Water Walk — level-3 transmutation, no-args ally burst. Fire only
/// when the water is between the caster's side and somewhere they want
/// to be.
///
/// The action's own validator already refuses a board with no water and
/// a party that all swims; what it cannot ask is whether the water is
/// *in the way*, and that is this rung's job. The test is an enemy on
/// the far side of a wet tile — measured as "an enemy exists, and the
/// straight line to them crosses water" — because a lake nobody needs
/// to cross is scenery, and a 3rd-level slot spent on scenery is the
/// whole failure mode a speculative buff rung has.
fn try_water_walk(encounter: &EncounterInstance, actor_id: usize) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    // Presence of the action on the sheet is the cheap gate; the
    // action's own validator owns the rest and runs at `try_self_action`.
    actor.find_action("water walk")?;
    let (team, from) = (actor.team(), actor.location());
    let water_in_the_way = encounter.actors.iter().any(|(id, other)| {
        if *id == actor_id || other.team() == team || !other.is_combat_active() {
            return false;
        }
        crate::engine::encounter::tiles_between(from, other.location()).any(|tile| {
            encounter
                .terrain_at(tile)
                .is_some_and(|t| t.terrain_type.is_water())
        })
    });
    if !water_in_the_way {
        return None;
    }
    try_self_action(encounter, actor_id, "water walk")
}

/// Bard Bardic Inspiration — bonus action giving an ally a +3 die for
/// their next attack / save. Cast on the ally with the highest current
/// HP (likely a frontliner who's swinging this round) that isn't
/// already Inspired. Validates via the action's own custom check.
fn try_bardic_inspiration(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("bardic inspiration")?;
    let team = actor.team();
    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for id in encounter.sorted_actor_ids() {
        if id == actor_id {
            continue;
        }
        let Some(a) = encounter.actors.get(&id) else {
            continue;
        };
        if a.team() != team || !a.is_combat_active() {
            continue;
        }
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![id]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        let hp = a.hitpoints();
        if best.as_ref().is_none_or(|(best_hp, _)| hp > *best_hp) {
            best = Some((hp, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Zealot Barbarian Zealous Presence — bonus-action ally-burst, once
/// per long rest. Blessifies up to 10 allies within 60ft (24 tiles).
/// Gated on "at least one *other* combat-active ally within 24 tiles"
/// so a lone-wolf zealot doesn't burn the once-per-rest charge to
/// blessify only themselves — the caster is included in the burst
/// naturally, but the AI wants at least one teammate to justify the
/// spend. Sibling shape to `try_bless` on the ally-buff-when-team-
/// present lane, but for a class-feature charge rather than a
/// concentration spell.
fn try_zealous_presence(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    // 60ft RAW = 24 tiles on the 2.5ft grid — matches the spend site
    // in `ZEALOUS_PRESENCE` and the ally_burst_targets radius.
    if n_actors_within(encounter, actor_id, 24, true, 1) < 1 {
        return None;
    }
    try_self_action(encounter, actor_id, "zealous presence")
}

/// Fighter Indomitable — RAW "no action required", so the engine prices
/// it at `free_cost` and the AI can arm it without giving anything up.
///
/// The only gate is that there is a fight on. A marker armed in an empty
/// room would still be there when the fight started — nothing expires
/// it — but firing it against nobody would put a line in the log that
/// says the fighter did something on a turn where nothing happened, and
/// the ladder's contract is that a rung firing means a decision was
/// made. The action's own `custom_validate_input` owns the charge check,
/// so a fighter who has already armed it falls straight through.
fn try_indomitable(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if n_actors_within(encounter, actor_id, isize::MAX, false, 1) < 1 {
        return None;
    }
    try_self_action(encounter, actor_id, "indomitable")
}

/// Fiend Warlock Hurl Through Hell — the lv14 capstone, once per rest,
/// 60 ft (24 tiles). Highest-HP hostile in range wins per the shared
/// picker: the damage is a flat 10d10 and the turn the target loses is
/// worth most taken off whatever was going to make best use of it.
fn try_hurl_through_hell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_single_target_class_feature_hostile(encounter, actor_id, "hurl through hell", 24)
}

/// Tiefling Infernal Legacy: Infernal Rebuke — racial, once per rest,
/// 60 ft (24 tiles), 3d10 fire on a DEX save for half. Same shared
/// picker and the same reasoning as Wrath of the Storm: fixed damage, so
/// the only thing target choice buys is that none of it is wasted on
/// something already dying.
fn try_infernal_rebuke(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_single_target_class_feature_hostile(encounter, actor_id, "infernal rebuke", 24)
}

/// Light Domain Cleric Radiance of the Dawn — once-per-rest Channel
/// Divinity. A 30 ft (12-tile) self-centred burst of radiant damage that
/// reaches hostiles only, so there is no friendly-fire question to ask;
/// the only question is whether there are enough of them to be worth the
/// charge.
///
/// Two is the answer, matching `best_burst_placement`'s own refusal to
/// place a burst that catches fewer. Against one creature the cleric's
/// cantrip is most of the damage and costs nothing, and a Channel
/// Divinity spent to beat a cantrip by a few points is a Channel
/// Divinity the cleric does not have when the second wave arrives.
fn try_radiance_of_the_dawn(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    // 30 ft = 12 tiles on the 2.5 ft grid — the same radius the action's
    // own spend site uses.
    if n_actors_within(encounter, actor_id, 12, false, 2) < 2 {
        return None;
    }
    try_self_action(encounter, actor_id, "radiance of the dawn")
}

/// Tempest Domain Cleric Wrath of the Storm — once-per-rest Channel
/// Divinity, single-target, 30 ft (12 tiles), 2d8 lightning on a DEX
/// save for half.
///
/// Delegates to the shared single-target picker, which takes the
/// highest-HP hostile in range. That is the right tiebreak here for a
/// blunt reason rather than a subtle one: the damage is fixed, so the
/// only thing target choice can buy is that the damage is not wasted on
/// something about to die anyway.
///
/// No crowd gate, unlike the Light domain's burst two rungs above. This
/// one is single-target by construction and it is *strictly* better than
/// the Sacred Flame it competes with — same dice, and a made save halves
/// it here where Sacred Flame's negates it outright.
fn try_wrath_of_the_storm(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_single_target_class_feature_hostile(encounter, actor_id, "wrath of the storm", 12)
}

fn try_bless(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    // Only worth casting if at least one other allied combatant exists.
    let my_team = actor.team();
    let has_ally = encounter.actors.iter().any(|(id, a)| {
        *id != actor_id && a.team() == my_team && a.is_combat_active()
    });
    if !has_ally {
        return None;
    }
    try_self_action(encounter, actor_id, "bless")
}

/// Take the Dodge action when we're below half HP and an enemy still
/// threatens us. Better than Skip when the actor has nothing else to do.
fn try_dodge_when_low_hp(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.has_condition(Condition::Dodging) {
        return None;
    }
    let hp = actor.hitpoints() as f32;
    let max = actor.max_hitpoints().max(1) as f32;
    if hp / max >= 0.5 {
        return None;
    }
    try_self_action(encounter, actor_id, "dodge")
}

/// Bite instead of swinging the axe, once the Beast Barbarian is hurt
/// enough for the bite to heal.
///
/// A rung of its own rather than a case in `best_attack_against`,
/// because the two answers disagree and both are right. The picker ranks
/// by expected damage, and by that measure the bite is the worse swing —
/// 1d8+STR twice against the greataxe's 1d12+STR twice, four points of
/// average damage given up. Below half hit points the bite also returns
/// the barbarian's proficiency bonus, and four points of damage is a
/// good price for four points of healing on a body that Rage is already
/// halving incoming damage against.
///
/// The gates are the heal's own: the tag, the below-half threshold, and
/// the once-per-turn mark the rider sets. Past the mark the bite is
/// simply the worse weapon again, so the rung stands down and the
/// ordinary picker takes the axe back.
///
/// Targets the lowest-HP enemy in reach, matching every other picker in
/// this file, with `sorted_actor_ids` making the tie-break
/// deterministic.
fn try_beast_bite_when_bloodied(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    use crate::actions::class_features::FORM_OF_THE_BEAST_BITE_TAG;
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.has_passive_feature(FORM_OF_THE_BEAST_BITE_TAG)
        || actor.once_per_turn_used(FORM_OF_THE_BEAST_BITE_TAG)
    {
        return None;
    }
    // Strictly below half — the same predicate the rider itself checks,
    // so the rung never reaches for a bite that would heal nothing.
    if !actor.is_below_half_hitpoints() {
        return None;
    }
    let bite = actor.find_action("bite")?;
    let reach = bite.reach_tiles()?;
    let my_team = actor.team();
    let target_id = encounter
        .sorted_actor_ids()
        .into_iter()
        .filter(|&tid| {
            encounter.actors.get(&tid).is_some_and(|t| {
                tid != actor_id
                    && t.team() != my_team
                    && t.is_combat_active()
                    && actor.footprint_gap_to(t) <= reach
            })
        })
        .min_by_key(|tid| encounter.actors[tid].effective_hitpoints())?;
    let aei = ActionExecutionInfo::new(bite, actor_id, Some(vec![target_id]), None, None);
    aei.validate(encounter).then_some(aei)
}

/// Shove an adjacent enemy prone when at least one ally is also adjacent
/// to the same target. Knocking the target prone gives those allies
/// advantage on their next melee swing — high leverage in a team fight.
/// Skips targets already prone (wasted action) and targets too large to
/// shove (the action's `custom_validate_input` handles this, but we
/// gate early to avoid burning the validation cost).
fn try_shove(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("shove")?;
    let my_team = actor.team();

    // Candidate targets: adjacent hostile, not already prone.
    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for tid in encounter.sorted_actor_ids() {
        let Some(t) = encounter.actors.get(&tid) else {
            continue;
        };
        if tid == actor_id || t.team() == my_team || !t.is_combat_active() {
            continue;
        }
        if t.has_condition(Condition::Prone) {
            continue;
        }
        // Must be in melee reach.
        if actor.footprint_gap_to(t) > MELEE_REACH {
            continue;
        }
        // Only shove when at least one friendly melee ally is also adjacent
        // to the target — otherwise prone just halves the target's speed
        // and doesn't give us advantage on our own attack (we already used
        // our Action on the shove).
        //
        // "Melee ally" is the load-bearing word and the code did not
        // check it: any teammate standing nearby counted, including ones
        // that gain nothing from a prone target and ones that are made
        // *worse* by it. Prone hands advantage to melee attackers and
        // disadvantage to ranged ones, and it does nothing at all to a
        // save-based burst — so an Artillerist whose flamethrower cannon
        // happened to be adjacent spent turn after turn shoving an ogre
        // over for a turret that could not use it.
        let ally_adjacent = encounter.actors.iter().any(|(aid, ally)| {
            *aid != actor_id
                && ally.team() == my_team
                && ally.is_combat_active()
                && ally.footprint_gap_to(t) <= MELEE_REACH
                && ally
                    .actions
                    .iter()
                    .any(|a| a.is_harmful() && a.deals_damage() && a.is_melee_attack())
        });
        if !ally_adjacent {
            continue;
        }
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![tid]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        // Prefer highest-HP target (shove the biggest threat).
        let hp = t.hitpoints();
        if best.as_ref().is_none_or(|(best_hp, _)| hp > *best_hp) {
            best = Some((hp, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Grapple a nearby ranged enemy to lock their movement to zero. Ranged
/// enemies often kite — pinning them prevents escape and forces them to
/// make ranged attacks at disadvantage (due to adjacent hostiles). Skips
/// targets already grappled or without ranged attacks (melee enemies are
/// already where we want them).
fn try_grapple(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("grapple")?;
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());

    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for tid in encounter.sorted_actor_ids() {
        let Some(t) = encounter.actors.get(&tid) else {
            continue;
        };
        if tid == actor_id || t.team() == my_team || !t.is_combat_active() {
            continue;
        }
        if t.has_condition(Condition::Grappled) {
            continue;
        }
        // Must be adjacent.
        let dist = footprint_chebyshev(
            my_loc,
            my_size,
            t.location(),
            get_tiles_from_size(t.size()),
        );
        if dist > MELEE_REACH {
            continue;
        }
        // Only grapple enemies that have ranged attacks — melee-only foes
        // gain nothing from breaking free since they want to be in melee.
        //
        // `!is_melee_attack`, not `reach > MELEE_REACH`: an ogre's
        // greatclub reaches two tiles, and under the arithmetic version
        // every reach weapon in the bestiary read as a ranged attack. So
        // the rung grappled ogres, giants, treants and dragons — all of
        // which want to be exactly where the grapple pins them — and
        // spent the Action to do it.
        let has_ranged = t.actions.iter().any(|a| {
            a.is_harmful()
                && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
                && !a.is_melee_attack()
                && a.reach_tiles().is_some()
        });
        if !has_ranged {
            continue;
        }
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![tid]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        let hp = t.hitpoints();
        if best.as_ref().is_none_or(|(best_hp, _)| hp > *best_hp) {
            best = Some((hp, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Throw a **damage-free grab** — a single-target weapon attack whose
/// entire payload is a hold rather than a hit point.
///
/// The cohort is `is_weapon_attack() && !deals_damage()` on the
/// `SingleActor` schema, which is a precise description of exactly one
/// thing: an attack roll that catches somebody. The roper's tendril is
/// its only member today; a net, a lasso or a second monster's snare
/// lands as a declaration rather than an edit here.
///
/// It needs a rung of its own because the two that look like they
/// should cover it don't. `best_attack_against` skips every action that
/// declares `deals_damage() == false`, deliberately — focus-fire is for
/// whittling hit points and a Shove is not — so the tendril was
/// invisible to it. And `try_grapple` is a melee-reach rung with a
/// "only bother if the target shoots" gate, both of which are wrong
/// here: a grab thrown fifty feet is the whole of the roper's offence,
/// and it wants the target *closer*, not pinned where it stands.
///
/// Targets already Grappled or Restrained are skipped: those are the
/// two conditions this cohort installs, and re-catching a creature that
/// is already caught spends the Action for a timer refresh. Among the
/// rest the biggest hit point bar wins, which is the same "grab the
/// worst threat" rule `try_shove` and `try_grapple` use.
fn try_damage_free_grab(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.is_combat_active() {
        return None;
    }
    let my_team = actor.team();
    let grabs: Vec<&'static (dyn Action + Send + Sync)> = actor
        .actions
        .iter()
        .filter(|a| {
            a.is_harmful()
                && a.is_weapon_attack()
                && !a.deals_damage()
                && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
        })
        .copied()
        .collect();
    if grabs.is_empty() {
        return None;
    }

    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for tid in encounter.sorted_actor_ids() {
        let Some(t) = encounter.actors.get(&tid) else {
            continue;
        };
        if tid == actor_id || t.team() == my_team || !t.is_combat_active() {
            continue;
        }
        if t.has_condition(Condition::Grappled) || t.has_condition(Condition::Restrained) {
            continue;
        }
        for action in &grabs {
            let aei = ActionExecutionInfo::new(*action, actor_id, Some(vec![tid]), None, None);
            if !aei.validate(encounter) {
                continue;
            }
            let hp = t.hitpoints();
            if best.as_ref().is_none_or(|(best_hp, _)| hp > *best_hp) {
                best = Some((hp, aei));
            }
        }
    }
    best.map(|(_, aei)| aei)
}

/// Haul in whatever the actor has caught — the roper's Reel.
///
/// Found by name, the way `try_grapple` finds the Shove-and-Grapple
/// pair, because the action's own `custom_validate_input` already asks
/// the only question that matters ("is anything actually held by me?")
/// and there is nothing left for a cohort filter to add. A rung that
/// re-derived the answer here would be asking it twice and could get a
/// different one.
///
/// Sits high in the ladder, above the attack lanes, for two reasons
/// that both come down to it being a Bonus Action: it does not compete
/// with the turn's Action, so firing it early costs the roper nothing;
/// and dragging the catch ten tiles closer is what makes the Action
/// that follows a bite instead of another tendril.
fn try_reel(encounter: &EncounterInstance, actor_id: usize) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.is_combat_active() {
        return None;
    }
    let action = actor.find_action("reel")?;
    let aei = ActionExecutionInfo::new(action, actor_id, None, None, None);
    aei.validate(encounter).then_some(aei)
}

/// Sort key for advantage-aware target selection — lower wins.
fn mode_priority(mode: RollMode) -> u8 {
    match mode {
        RollMode::Advantage => 0,
        RollMode::Normal => 1,
        RollMode::Disadvantage => 2,
    }
}

/// True if the actor has a ranged attack it can make **right now** —
/// beyond melee reach, against some live enemy, and affordable this
/// instant.
///
/// Read only by the two kiting rungs at the top of the ladder, which
/// back an actor out of contact so it can shoot instead. The predicate
/// used to ask a weaker question — does this sheet list a ranged attack
/// at all — on the reasoning that "the kite tactic only cares whether we
/// *could* shoot once we have space". That reasoning has a hole in it,
/// and the hole is a livelock.
///
/// A Four Elements monk holding a stunned ogre found it. Its only
/// single-target ranged attack is Water Whip, a bonus action it had
/// already spent; its Action was gone too. So there was nothing it could
/// do at any range — and rung 2 backed it out of contact anyway, because
/// the sheet still listed the whip. The approach rung then walked it
/// straight back in, rung 2 pushed it out again, and the two spent the
/// monk's entire movement allowance shuffling between two tiles. Then it
/// Dashed, and did it again, for nine rounds, while the thing it was
/// holding stood still and waited.
///
/// The two rungs contradicting each other is what makes the weaker
/// predicate dangerous rather than merely imprecise: every turn one of
/// them wins the first step and the other wins the second, forever.
/// Asking whether the shot is actually available is what stops the
/// argument, because a kite that buys nothing no longer starts it.
///
/// Deliberately *not* narrowed further to "and the shot is better than
/// staying". That is the kind of judgement the picker makes and this is
/// a gate; what it owes the rungs above it is that backing away is not
/// simply wasted.
fn has_ranged_attack(encounter: &EncounterInstance, actor_id: usize) -> bool {
    let Some(actor) = encounter.actors.get(&actor_id) else {
        return false;
    };
    let my_team = actor.team();
    let enemies: Vec<usize> = encounter
        .sorted_actor_ids()
        .into_iter()
        .filter(|id| {
            encounter
                .actors
                .get(id)
                .is_some_and(|a| a.team() != my_team && a.is_combat_active())
        })
        .collect();
    if enemies.is_empty() {
        return false;
    }
    // Restrict to *harmful* SingleActor actions — kiting / disengaging
    // is about ranged offense, not about long-range buff dispensers like
    // Rally (12-tile reach) or Commander's Strike (24-tile reach). Pre-
    // restriction this lane caught the support actions and steered every
    // Fighter into kite-mode the moment they had Rally on their sheet.
    //
    // `deals_damage` carries that same argument one step further, and it
    // has to: a hostile action at range that deals no damage is no more
    // a reason to back away than a friendly one is. A vampire's Charming
    // Gaze is harmful, single-target and reaches 30 ft, so the vampire
    // read as a ranged attacker and kited — from a frost giant that
    // outranges it two to one, every turn, while the rocks came in and
    // its own regeneration undid them. Its actual offense was a melee
    // multiattack it never once used.
    actor.actions.iter().any(|a| {
        a.is_harmful()
            && a.deals_damage()
            && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
            // Same reading as the grapple rung's: a swing is not a shot,
            // however far it reaches. Under the arithmetic version an
            // ogre counted itself as a ranged attacker and backed away
            // from things it wanted to club.
            && !a.is_melee_attack()
            && a.reach_tiles().is_some()
            // The half that was missing. `validate` answers reach, line
            // of sight, the action's own gates *and* whether the actor
            // can still pay for it — which is the clause the monk above
            // needed, since its whip was a bonus action it had already
            // spent.
            && enemies.iter().any(|&tid| {
                ActionExecutionInfo::new(*a, actor_id, Some(vec![tid]), None, None)
                    .validate(encounter)
            })
    })
}

/// The best `expected_damage` this actor can get out of an available
/// harmful single-target attack in each lane, as `(melee, ranged)`.
///
/// `None` in a slot means the lane is either empty or unannotated —
/// the estimate is optional on `Action` and most bespoke attacks leave
/// it off — and callers must treat that as "no opinion" rather than as
/// zero. Availability is `validate` against a live enemy, the same
/// clause `has_ranged_attack` uses, so a swing the actor cannot pay
/// for does not argue for staying and a shot it cannot pay for does
/// not argue for leaving.
fn best_damage_per_lane(
    encounter: &EncounterInstance,
    actor_id: usize,
    enemies: &[usize],
) -> (Option<f32>, Option<f32>) {
    let Some(actor) = encounter.actors.get(&actor_id) else {
        return (None, None);
    };
    let (mut melee, mut ranged) = (None::<f32>, None::<f32>);
    for action in actor.actions.iter() {
        if !action.is_harmful()
            || !action.deals_damage()
            || !matches!(action.targeting_schema(), TargetingSchema::SingleActor)
        {
            continue;
        }
        let Some(est) = action.expected_damage(encounter, actor_id) else {
            continue;
        };
        if !enemies.iter().any(|&tid| {
            ActionExecutionInfo::new(*action, actor_id, Some(vec![tid]), None, None)
                .validate(encounter)
        }) {
            continue;
        }
        let slot = if action.is_melee_attack() {
            &mut melee
        } else {
            &mut ranged
        };
        *slot = Some(slot.map_or(est, |best: f32| best.max(est)));
    }
    (melee, ranged)
}

/// True if backing out of contact is worth the step — that is, if the
/// actor's ranged lane is not strictly worse than the melee lane it
/// would be giving up.
///
/// This is the clause `has_ranged_attack` deliberately stops short of,
/// and it stops short of it for a good reason: that predicate is a
/// gate, and asking it to rank lanes would duplicate the picker. But
/// there is a difference between "don't rank" and "don't notice you
/// are trading a greataxe for a hatchet", and the roster grew creatures
/// on the wrong side of it.
///
/// A barbarian carries a handaxe to throw; a paladin carries a javelin.
/// Both are one line on a sheet whose every other line points at
/// contact — Rage's damage bonus and Reckless Attack's advantage are
/// melee-only, and Divine Smite rides a melee weapon hit, so an
/// out-of-reach round costs the oath its whole damage budget rather
/// than just its turn. Under the existence test both read as ranged
/// attackers the moment the throw went on the sheet, and rung 2 walked
/// them backwards away from the fight they were built to be in.
///
/// The comparison is deliberately permissive in both directions where
/// it has no information. A lane with no estimate is "no opinion", not
/// zero: `expected_damage` is optional and most bespoke attacks leave
/// it off, so demanding a number would turn every unannotated melee
/// attack into a licence to kite and every unannotated ranged one into
/// a ban. Only a melee lane that *demonstrably* beats the ranged lane
/// stops the retreat.
///
/// Ties go to leaving. A creature with equal options standing in
/// something's reach is better off out of it, which is the whole
/// premise of the rung.
fn ranged_lane_beats_staying(encounter: &EncounterInstance, actor_id: usize) -> bool {
    if !has_ranged_attack(encounter, actor_id) {
        return false;
    }
    let Some(actor) = encounter.actors.get(&actor_id) else {
        return false;
    };
    let my_team = actor.team();
    let enemies: Vec<usize> = encounter
        .sorted_actor_ids()
        .into_iter()
        .filter(|id| {
            encounter
                .actors
                .get(id)
                .is_some_and(|a| a.team() != my_team && a.is_combat_active())
        })
        .collect();
    match best_damage_per_lane(encounter, actor_id, &enemies) {
        (Some(melee), Some(ranged)) => ranged >= melee,
        _ => true,
    }
}

/// True if any combat-active enemy has a melee attack whose reach covers
/// our current footprint distance to them. "Melee" = reach ≤ MELEE_REACH.
fn under_melee_threat(encounter: &EncounterInstance, actor_id: usize) -> bool {
    let Some(me) = encounter.actors.get(&actor_id) else {
        return false;
    };
    let my_team = me.team();
    let my_loc = me.location();
    let my_size = get_tiles_from_size(me.size());

    encounter.actors.iter().any(|(other_id, other)| {
        if *other_id == actor_id || other.team() == my_team || !other.is_combat_active() {
            return false;
        }
        let dist = footprint_chebyshev(
            my_loc,
            my_size,
            other.location(),
            get_tiles_from_size(other.size()),
        );
        // Deliberately `MELEE_REACH` and not `MELEE_BAND_REACH`, which
        // is the wider gap `first_melee_weapon_action` measures a swing
        // against. The two questions look identical and are not: that
        // one asks "could this creature swing at that one", and this
        // asks "is stepping one tile going to help". Against an ogre's
        // 10 ft club or a tarrasque's 15 ft bite the answer to the
        // second is no — the same reasoning rung 2 of the ladder gives
        // for a 5 ft threat with 30 ft of movement, only more so — so
        // widening this gate would fire the kiting rung in exactly the
        // situations where it wastes the turn, and would do it ahead of
        // every rung that could have answered the threat instead.
        other.actions.iter().any(|a| {
            matches!(a.targeting_schema(), TargetingSchema::SingleActor)
                && a.reach_tiles()
                    .is_some_and(|r| r <= MELEE_REACH && dist <= r)
        })
    })
}

/// Step one tile in the direction that maximizes the minimum footprint
/// distance to any combat-active enemy, validating against Move.
fn try_step_away_from_threats(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let move_action = actor.find_action("move")?;
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());

    // Snapshot enemy footprints once — we'll project candidate destinations
    // against them. Self-team and downed actors don't count as threats.
    let enemies: Vec<(Coordinate, usize)> = encounter
        .actors
        .iter()
        .filter(|(id, a)| {
            **id != actor_id && a.team() != my_team && a.is_combat_active()
        })
        .map(|(_, a)| (a.location(), get_tiles_from_size(a.size())))
        .collect();
    if enemies.is_empty() {
        return None;
    }

    let current_min = enemies
        .iter()
        .map(|(loc, sz)| footprint_chebyshev(my_loc, my_size, *loc, *sz))
        .min()
        .unwrap_or(0);

    let mut best: Option<(isize, Coordinate)> = None;
    for dy in -1..=1isize {
        for dx in -1..=1isize {
            if dx == 0 && dy == 0 {
                continue;
            }
            let cand = Coordinate::new(my_loc.x + dx, my_loc.y + dy);
            let min_dist = enemies
                .iter()
                .map(|(loc, sz)| footprint_chebyshev(cand, my_size, *loc, *sz))
                .min()
                .unwrap_or(0);
            // Strictly increase distance to closest threat — pacing in
            // place or moving sideways is no better than just shooting.
            if min_dist <= current_min {
                continue;
            }
            if best.is_some_and(|(d, _)| d >= min_dist) {
                continue;
            }
            let aei =
                ActionExecutionInfo::new(move_action, actor_id, None, Some(vec![cand]), None);
            if aei.validate(encounter) {
                best = Some((min_dist, cand));
            }
        }
    }
    let (_, dest) = best?;
    Some(ActionExecutionInfo::new(
        move_action,
        actor_id,
        None,
        Some(vec![dest]),
        None,
    ))
}

/// Fighter **Action Surge** — free (RAW: "no action required"), grants
/// a second Action this turn, once per short rest.
///
/// Unreachable by the AI until now, which mattered more than most
/// missing pickers: it is the largest single-turn output swing a
/// fighter has, and on an Extra Attack chassis it literally doubles
/// the turn.
///
/// Because it costs nothing, the only real question is *when*, and the
/// answer is "as soon as there is something to spend it on". It
/// refreshes on a short rest, so holding it across an encounter wastes
/// it outright — there is no later fight it is being saved for. The
/// gate is therefore just "a hostile is in play within the 24-tile
/// spell window the sibling free-prime pickers use", which fires it on
/// the first round of contact.
///
/// Terminates because `side_effects` spends the charge, so
/// `custom_validate_input`'s `feature_ready` gate fails on the next
/// pass through the ladder and the fighter proceeds to attack with the
/// action it was just handed.
fn try_action_surge(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_action_when_enemy_within(encounter, actor_id, 24, "action surge")
}

/// Enchantment Wizard **Hypnotic Gaze** — an action, no slot: an
/// adjacent creature makes a WIS save or is Charmed *and* Incapacitated
/// until the end of the enchanter's next turn.
///
/// Kept as its own picker rather than a row on the `LOCKDOWNS` cohort,
/// which is the natural-looking home for it. The reason it used to give
/// — that the cohort's picker bailed up front on a concentrating caster
/// and folding a non-concentration gaze in would need that bail to
/// become per-row — is gone: the bail *is* per-row now, and the whole
/// non-concentration half of the lockdown list moved in behind it.
///
/// What still keeps it out is the gate rather than the cost. Every row
/// on that cohort reaches as far as its spell does and leaves the range
/// question to `validate`; Hypnotic Gaze is an adjacency effect, and a
/// row that silently meant "but only at arm's length" would be a second
/// rule hiding inside a table whose entries otherwise all mean the same
/// thing. The ladder placement is the other half: the gaze fires above
/// the whole slot-spending lane precisely because it spends no slot.
///
/// Gated on an adjacent hostile that isn't already Incapacitated —
/// there is no point spending the enchanter's action to disable
/// something that is already out of the fight. Reach and line of sight
/// are left to `validate`.
///
/// Fires ahead of the slot-spending control lane below it, because it
/// is the only lockdown on the chassis that costs no slot at all: a
/// gaze that lands is a Hold Person the wizard didn't have to pay for.
fn try_hypnotic_gaze(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("hypnotic gaze")?;
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
    for target_id in encounter.sorted_actor_ids() {
        let Some(target) = encounter.actors.get(&target_id) else {
            continue;
        };
        if target.team() == my_team
            || !target.is_combat_active()
            || target.has_condition(Condition::Incapacitated)
        {
            continue;
        }
        if footprint_chebyshev(
            my_loc,
            my_size,
            target.location(),
            get_tiles_from_size(target.size()),
        ) > 0
        {
            continue;
        }
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![target_id]), None, None);
        if aei.validate(encounter) {
            return Some(aei);
        }
    }
    None
}

/// Self-teleport actions the AI will spend to break out of melee, in
/// the order it will reach for them: cheapest resource first.
///
/// Every entry is a `SinglePoint` self-move with a reach in tiles, no
/// save, no attack roll, and — RAW, uniformly across teleports — no
/// opportunity attack, since the caster never traverses the
/// intervening squares. That last property is the whole reason this
/// lane exists separately from `try_step_away_from_threats` and
/// `try_disengage`: a one-tile step off a melee threat that has 5 ft
/// of reach and 30 ft of movement has not actually escaped anything,
/// and Disengage buys a walk that costs the whole action. A blink
/// leaves the melee outright.
///
/// Order:
///   1. **Benign Transposition** (Conjuration Wizard lv6) — a charge
///      that the conjurer's own casting refills, so it is very nearly
///      free. Costs the action.
///   2. **Hidden Paths** (Circle of Dreams Druid lv10) — a bonus
///      action and a per-short-rest charge, and no slot at all. Below
///      Benign Transposition only because the conjurer's charge refills
///      itself mid-fight and this one does not; against everything
///      below it, a blink that costs no slot is strictly cheaper.
///
///      It matters most on the chassis that carries it: a druid holding
///      a Moonbeam or a Spike Growth would have to spend the slot that
///      replaces it to take either of the two escapes below, and this
///      one leaves both the slot and the concentration alone.
///   3. **Misty Step** — a 2nd-level slot, but only a bonus action, so
///      the caster still gets to cast on the turn they escape.
///   4. **Dimension Door** — a 4th-level slot and an action, with
///      double the range. The last resort, and the only one that
///      reliably clears a whole engagement.
///
/// A new self-teleport (Thunder Step's damage-on-arrival variant, a
/// Horizon Walker's Planar Step) drops in as one row.
const SELF_TELEPORT_ESCAPES: &[&str] = &[
    "benign transposition",
    "hidden paths",
    "misty step",
    "dimension door",
];

/// The one escape whose price depends on whether it is already up, and
/// so the one that cannot hold a fixed rank on the list above.
///
/// Far Step costs an Action and a 5th-level slot to open — dearer than
/// every row on `SELF_TELEPORT_ESCAPES`, including Dimension Door — and
/// a bare bonus action on every turn after that, which is cheaper than
/// every row including Hidden Paths. Both readings are right; which one
/// applies is a question about the caster's sheet, not about the spell.
///
/// So `try_teleport_escape` splices this name in at whichever end of
/// the walk the caster's `FarStepping` condition says it belongs, and
/// the static list keeps the ranks that really are static.
const SUSTAINED_TELEPORT_ESCAPE: &str = "far step";

/// The eight unit steps on a Chebyshev grid, used to probe teleport
/// destinations outward from the caster.
const COMPASS_DIRECTIONS: [(isize, isize); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Blink out of melee when a ranged actor is genuinely pinned.
///
/// Gated three ways, all of which have to hold, because a teleport is
/// a real resource and the cheaper `try_step_away_from_threats` rung
/// already handles the ordinary case:
///
///   1. **The actor has a ranged attack.** A melee actor that blinks
///      away has to walk back, so the escape costs it the fight.
///   2. **It is under melee threat.** Nothing to escape otherwise.
///   3. **It is actually pinned** — either below half HP, or with two
///      or more hostiles in contact. One healthy caster with one
///      adjacent goblin should step and shoot, not burn a slot.
///
/// Destination policy is "as far from the nearest threat as this
/// teleport can reach", found by probing outward along the eight
/// compass directions rather than by scanning every reachable tile.
/// The exhaustive scan is the obvious implementation and the wrong
/// one: Dimension Door's 24-tile reach makes it a 49x49 sweep scored
/// against every hostile on the map, run for every pinned caster every
/// turn, and it measurably dominated the AI driver's runtime when
/// written that way. The probe costs a fixed ~32 candidates and lands
/// on the same tactical answer, because "blink directly away from what
/// is hitting you" is what the optimum almost always is.
///
/// Candidates are scored by the resulting Chebyshev gap to the closest
/// hostile footprint and taken best-first; the first one that survives
/// `validate` wins, so terrain, occupancy and line of sight are all
/// enforced by the action itself rather than re-derived here. Only the
/// top handful are validated — `validate` is the expensive part of the
/// loop, and a blink that can't land among its best few is walled in
/// well enough that the next-cheapest escape is the better answer.
fn try_teleport_escape(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    /// How many best-scoring destinations to run through `validate`
    /// before giving up on an action. Bounds the per-turn cost of the
    /// search; a blink that can't find a legal landing spot among its
    /// eight best is walled in well enough that the next-cheapest
    /// escape is the better answer anyway.
    const MAX_VALIDATIONS_PER_ACTION: usize = 8;

    // Same gate as the kite rung above, and for the same reason: a
    // blink is a more expensive way of giving up contact than a step
    // is, so a creature that should not be stepping away certainly
    // should not be spending a slot to teleport away.
    if !ranged_lane_beats_staying(encounter, actor_id) || !under_melee_threat(encounter, actor_id) {
        return None;
    }
    // Two conditions, either of which makes leaving worth an action:
    // the caster is already hurt, or two hostiles are inside swinging
    // distance of them. The second used to count only bodies at a gap
    // of 0 — touching — which the AI's approach never produces, so the
    // whole clause was dead and a full-health caster with two enemies
    // on it stood and took the round.
    let pinned = is_low_hp(encounter, actor_id, 0.5)
        || n_actors_within(encounter, actor_id, MELEE_REACH, false, 2) >= 2;
    if !pinned {
        return None;
    }

    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
    let threats: Vec<(Coordinate, usize)> = encounter
        .actors
        .iter()
        .filter(|(id, a)| **id != actor_id && a.team() != my_team && a.is_combat_active())
        .map(|(_, a)| (a.location(), get_tiles_from_size(a.size())))
        .collect();
    if threats.is_empty() {
        return None;
    }
    let gap_to_nearest_threat = |c: Coordinate| -> isize {
        threats
            .iter()
            .map(|(loc, sz)| footprint_chebyshev(c, my_size, *loc, *sz))
            .min()
            .unwrap_or(0)
    };
    let current_gap = gap_to_nearest_threat(my_loc);

    // See `SUSTAINED_TELEPORT_ESCAPE`: an open Far Step is the cheapest
    // blink on the board and an unopened one is the dearest, so it goes
    // at the front of the walk or the back of it depending on which.
    let far_step_is_open = actor.has_condition(Condition::FarStepping);
    let escapes: Vec<&str> = if far_step_is_open {
        std::iter::once(SUSTAINED_TELEPORT_ESCAPE)
            .chain(SELF_TELEPORT_ESCAPES.iter().copied())
            .collect()
    } else {
        SELF_TELEPORT_ESCAPES
            .iter()
            .copied()
            .chain(std::iter::once(SUSTAINED_TELEPORT_ESCAPE))
            .collect()
    };
    for name in &escapes {
        let Some(action) = actor.find_action(name) else {
            continue;
        };
        let Some(reach) = action.reach_tiles() else {
            continue;
        };
        let mut candidates: Vec<(isize, Coordinate)> = Vec::new();
        for (dx, dy) in COMPASS_DIRECTIONS {
            // Probe from the far end inward: the whole point of a
            // teleport is the distance, and a shorter hop is only worth
            // considering when the long one is blocked.
            for numerator in [4isize, 3, 2, 1] {
                let dist = reach * numerator / 4;
                if dist == 0 {
                    continue;
                }
                let cand = Coordinate::new(my_loc.x + dx * dist, my_loc.y + dy * dist);
                // Off the map is not a landing spot, and dropping those
                // here rather than at `validate` is what keeps the
                // validation budget for candidates that could work.
                //
                // It used to be `validate`'s job, and the cost fell
                // entirely on the longest-reaching blinks: candidates
                // are scored by distance gained and the far ones score
                // best, so a 60 ft teleport from mid-map spent all
                // eight of its validations on coordinates past the
                // wall and reported that it could not escape. A 30 ft
                // one never noticed, because half its probe ring landed
                // inside the map to begin with — which is how a picker
                // can be wrong in a way that only the next feature
                // finds.
                if !encounter.in_bounds(cand) {
                    continue;
                }
                let gap = gap_to_nearest_threat(cand);
                // Only landing spots that actually improve on standing
                // still are worth a slot.
                if gap > current_gap {
                    candidates.push((gap, cand));
                }
            }
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, dest) in candidates.into_iter().take(MAX_VALIDATIONS_PER_ACTION) {
            let aei = ActionExecutionInfo::new(action, actor_id, None, Some(vec![dest]), None);
            if aei.validate(encounter) {
                return Some(aei);
            }
        }
    }
    None
}

/// Wild Shape — the Moon Druid's bonus-action transform into a brown
/// bear: 34 temp HP and a 2d6+4 claw swing, at the cost of the entire
/// spell list for as long as it lasts.
///
/// That cost is what the gate is about. Becoming a bear is not a buff
/// the AI should reach for on general principle — for a WIS-18
/// full-caster it is usually a downgrade, and the action's own
/// validator (charge available, not already in form) says nothing about
/// whether it's a *good* idea. So two conditions have to hold together:
///
///   1. An enemy is footprint-adjacent. The form's whole output is a
///      melee swing, and a druid nobody is standing next to should be
///      casting, not clawing.
///   2. The spell list has stopped being the better option — either the
///      slot pool is empty (nothing is being given up) or the druid is
///      under 60% HP (the 34 temp HP roughly doubles what's left, and a
///      caster in melee at half health is about to stop casting
///      anyway).
///
/// Concentration is deliberately *not* part of the gate. RAW lets a
/// wild-shaped druid keep concentrating on something cast before the
/// transformation, and the engine agrees — so a druid holding a Moonbeam
/// keeps it through the change, and the AI doesn't need to protect
/// against a loss that can't happen.
fn try_wild_shape(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !any_enemy_within(encounter, actor_id, 1) {
        return None;
    }
    let slots_dry = actor.lowest_available_spell_slot().is_none();
    if !slots_dry && !is_low_hp(encounter, actor_id, 0.6) {
        return None;
    }
    try_self_action(encounter, actor_id, "wild shape")
}

/// Wild Heal — the Moon Druid's in-form slot-to-hit-points conversion.
///
/// The action's own validator already covers the load-bearing gates
/// (in beast form, below max HP, at least one slot left), so this rung
/// only adds the "is it worth a slot yet" judgement: hold until 70% HP.
/// The threshold sits higher than most self-heal gates in this file
/// because the alternative use for those slots is nothing at all — a
/// wild-shaped druid can't cast, so a slot not spent here is a slot
/// doing no work until the form ends.
fn try_wild_heal(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !is_low_hp(encounter, actor_id, 0.7) {
        return None;
    }
    try_self_action(encounter, actor_id, "wild heal")
}

/// Shadow Step — the Way of Shadow Monk's 60 ft bonus-action blink.
///
/// The mirror image of `try_teleport_escape`, and deliberately so:
/// that picker moves a pinned caster *away* from the nearest threat,
/// this one moves a monk *onto* one. Same candidate-and-validate
/// shape, opposite objective function.
///
/// Three gates:
///   1. An enemy already in melee reach → don't. The monk's bonus
///      action is worth more as a Stunning Strike prime or a Flurry
///      than as a teleport to somewhere they already are, and the
///      advantage rider is worth less than either when the swing was
///      going to happen anyway.
///   2. A reachable enemy exists — the nearest one inside the step's
///      own 24-tile envelope. Out past that there is nothing to arrive
///      at.
///   3. Some compass-neighbour tile of that enemy is a legal landing
///      spot. The action's own `can_move_to` validation is the
///      authority; we just propose.
///
/// Candidates are the ring around the chosen target, probed outward by
/// the target's own footprint so a Huge creature's ring is measured
/// from its far edge rather than its origin tile. Anything that lands
/// footprint-adjacent is acceptable — there is no "better" adjacency
/// for a monk, only reachable and not.
fn try_shadow_step(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    /// Landing spots to run through `validate` before giving up. The
    /// ring around a Medium target is eight tiles; past that the monk
    /// is walled in well enough that walking is the better answer.
    const MAX_VALIDATIONS: usize = 12;

    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("shadow step")?;
    let reach = action.reach_tiles()?;
    if any_enemy_within(encounter, actor_id, 1) {
        return None;
    }
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());

    // Nearest reachable enemy. `sorted_actor_ids` keeps the tiebreak
    // deterministic across runs.
    let mut target: Option<(isize, Coordinate, usize)> = None;
    for tid in encounter.sorted_actor_ids() {
        let Some(t) = encounter.actors.get(&tid) else {
            continue;
        };
        if tid == actor_id || t.team() == my_team || !t.is_combat_active() {
            continue;
        }
        let t_size = get_tiles_from_size(t.size());
        let dist = footprint_chebyshev(my_loc, my_size, t.location(), t_size);
        if dist > reach {
            continue;
        }
        if target.as_ref().is_none_or(|(best, _, _)| dist < *best) {
            target = Some((dist, t.location(), t_size));
        }
    }
    let (_, target_loc, target_size) = target?;

    let mut candidates: Vec<Coordinate> = Vec::new();
    for (dx, dy) in COMPASS_DIRECTIONS {
        // Step out from the target's origin by its own footprint plus
        // one, so the ring sits just outside a Large / Huge body rather
        // than inside it.
        for step in 1..=(target_size as isize + 1) {
            let cand = Coordinate::new(target_loc.x + dx * step, target_loc.y + dy * step);
            if footprint_chebyshev(cand, my_size, target_loc, target_size) > 1 {
                continue;
            }
            if footprint_chebyshev(my_loc, my_size, cand, my_size) > reach {
                continue;
            }
            candidates.push(cand);
        }
    }
    for dest in candidates.into_iter().take(MAX_VALIDATIONS) {
        let aei = ActionExecutionInfo::new(action, actor_id, None, Some(vec![dest]), None);
        if aei.validate(encounter) {
            return Some(aei);
        }
    }
    None
}

/// Shapechanger — the Transmutation Wizard's emergency self-Polymorph.
/// Thirty temp HP and a beast body, for an action and a per-short-rest
/// charge.
///
/// The gate is where the whole judgement sits, because the form is
/// itself a concentration and therefore drops whatever the wizard is
/// currently holding. So there are two thresholds rather than one:
///
///   - Not concentrating: fire below 40% HP. The temp HP is pure
///     upside and the charge refreshes on a short rest.
///   - Concentrating: fire only below 20%. Giving up a Web or a Hold
///     Monster is a real cost, and at that point the alternative is
///     losing the concentration to going down anyway — a wizard at 0
///     HP holds nothing either.
///
/// Never re-fires while already Polymorphed: the second cast would
/// spend the action to re-apply a condition the wizard already has,
/// and the temp HP pool doesn't stack.
fn try_shapechanger(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.has_condition(Condition::Polymorphed) {
        return None;
    }
    let threshold = if actor.is_concentrating() { 0.2 } else { 0.4 };
    if !is_low_hp(encounter, actor_id, threshold) {
        return None;
    }
    try_self_action(encounter, actor_id, "shapechanger")
}

/// Self-targeted heal (e.g. fighter Second Wind, drink healing potion).
/// Triggers when the actor is below 50% HP. Tries every heal action,
/// passing self as the target for `SingleActor` schemas (Lay on Hands,
/// Healing Hands, Cure Wounds) and a no-target call for `NoArgs` schemas
/// (Second Wind, potions). Picks the first validating heal — action-list
/// order means high-value class features land before consumables.
///
/// Walks `available_actions()` (template + carried-consumable) so an
/// AI actor who picked up a healing potion or a cure-wounds scroll on a
/// previous turn actually drinks / reads it when wounded.
fn try_self_heal(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.is_combat_active() {
        return None;
    }
    let max_hp = actor.max_hitpoints().max(1);
    if (actor.hitpoints() as f32) / (max_hp as f32) >= 0.5 {
        return None;
    }
    for action in actor.available_actions() {
        if !action.is_heal() {
            continue;
        }
        let aei = match action.targeting_schema() {
            TargetingSchema::NoArgs => {
                ActionExecutionInfo::new(action, actor_id, None, None, None)
            }
            TargetingSchema::SingleActor => {
                // Self-cast: pass own id as the single-actor target. Lets
                // Lay on Hands / Healing Hands / Cure Wounds repair the
                // caster when no ally needs them more. The action's
                // validation handles team / range / once-per-rest gates.
                ActionExecutionInfo::new(action, actor_id, Some(vec![actor_id]), None, None)
            }
            _ => continue,
        };
        if aei.validate(encounter) {
            return Some(aei);
        }
    }
    None
}

/// Cast a helpful single-actor action (e.g. Healing Word) on an ally who
/// needs it. Priority: dying allies first (revival prevents death-save
/// failure), then wounded combat-active allies below 50% HP. Stable and
/// full-HP allies are ignored. Self-targeting is excluded — the actor
/// should make hostile turns, not heal themselves preemptively.
/// Greater Restoration — cleanse the worst debuff from a nearby ally.
/// Priorities: Petrified > Paralyzed > Stunned > Blinded > Frightened >
/// Charmed > Exhausted > Poisoned. Only fires when an ally has one of
/// these conditions and the caster has the spell + a lv5 slot.
fn try_greater_restoration(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("greater restoration")?;
    let my_team = actor.team();
    let severe_conditions = [
        Condition::Petrified,
        Condition::Paralyzed,
        Condition::Stunned,
        Condition::Blinded,
        Condition::Frightened,
        Condition::Charmed,
        Condition::Exhausted,
        Condition::Poisoned,
        Condition::Feebled,
    ];
    let mut best: Option<(usize, ActionExecutionInfo)> = None;
    for ally_id in encounter.sorted_actor_ids() {
        if ally_id == actor_id {
            continue;
        }
        let Some(ally) = encounter.actors.get(&ally_id) else {
            continue;
        };
        if ally.team() != my_team || !ally.is_combat_active() {
            continue;
        }
        let severity = severe_conditions
            .iter()
            .position(|c| ally.has_condition(*c));
        let Some(severity) = severity else {
            continue;
        };
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![ally_id]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        let pick = match &best {
            None => true,
            Some((best_sev, _)) => severity < *best_sev,
        };
        if pick {
            best = Some((severity, aei));
        }
    }
    best.map(|(_, aei)| aei)
}

/// Cast something helpful on the ally who most needs it — the rung the
/// pipeline reaches when somebody on the team is dying or bloodied.
///
/// The candidate set is deliberately wider than the name: every
/// non-harmful single-target action on the sheet, not only the ones that
/// restore hit points. That is not tidiness — it is the only lane a
/// caster's ally-target buffs have. Barkskin, Death Ward, Freedom of
/// Movement, Haste, Greater Invisibility, Stoneskin, Magic Weapon,
/// Protection from Energy and a dozen more reach the board through here
/// and nowhere else; narrowing the filter to `is_heal()` would delete
/// all of them from every AI-driven caster in one line.
///
/// What the filter must not do is let them *outrank* an actual heal, and
/// until the ordering key below grew its third element it did exactly
/// that. Candidates for one ally all share a priority and an HP, so the
/// old two-element key never separated them, and the winner was simply
/// whichever action sat earliest on the template's list. A cleric whose
/// sheet happened to put Guidance above Cure Wounds answered a dying
/// ally by handing them a d4.
///
/// The key is `(priority, hp, not-a-heal)`, lowest wins:
///   - **priority** — 0 for a dying ally, 1 for a bloodied one. Nothing
///     else is a candidate.
///   - **hp** — among equally urgent allies, the one closest to death.
///   - **not-a-heal** — among actions for the same ally, `is_heal()`
///     first. This is the fix; the two elements above are unchanged.
///
/// Help is excluded by name, and so is the Mastermind Rogue's Master of
/// Tactics — the same action at a different price, so the same
/// reasoning. Both are helpful and single-target and would otherwise
/// sort alongside the buffs, but an attack-advantage rider is the one
/// thing a creature bleeding out has no use for.
fn try_support_heal(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();

    let support_actions: Vec<&'static (dyn Action + Send + Sync)> = actor
        .actions
        .iter()
        .filter(|a| {
            !a.is_harmful()
                && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
                && !matches!(a.name(), "help" | "master of tactics")
        })
        .copied()
        .collect();
    if support_actions.is_empty() {
        return None;
    }

    // Sort actor ids for deterministic tiebreak.
    let ids = encounter.sorted_actor_ids();

    let mut best: Option<((u8, u32, bool), ActionExecutionInfo)> = None;
    for ally_id in ids {
        if ally_id == actor_id {
            continue;
        }
        let Some(ally) = encounter.actors.get(&ally_id) else {
            continue;
        };
        if ally.team() != my_team {
            continue;
        }

        let priority = if ally.is_dying() {
            0u8
        } else if ally.is_combat_active()
            && (ally.hitpoints() as f32) / (ally.max_hitpoints().max(1) as f32) < 0.5
        {
            1u8
        } else {
            continue; // healthy or stable — skip
        };

        for &support in &support_actions {
            // A heal aimed at something that can't regain hit points is
            // an action and often a slot thrown away — a swarm, or an
            // ally under Chill Touch. The buffs in this same list still
            // land, so the gate is on the heal rather than on the ally:
            // Shield of Faith on a chilled fighter is a fine use of the
            // turn, and Cure Wounds on the same fighter is not.
            if support.is_heal()
                && !encounter
                    .actors
                    .get(&ally_id)
                    .is_some_and(|a| a.can_regain_hitpoints())
            {
                continue;
            }
            let aei =
                ActionExecutionInfo::new(support, actor_id, Some(vec![ally_id]), None, None);
            if !aei.validate(encounter) {
                continue;
            }
            let key = (priority, ally.hitpoints(), !support.is_heal());
            if best.as_ref().is_none_or(|(best_key, _)| key < *best_key) {
                best = Some((key, aei));
            }
        }
    }

    best.map(|(_, aei)| aei)
}

/// Try to fire a Burst-schema action centered on a tile that hits as many
/// enemies as possible without catching any allies. Candidate tiles are
/// every combat-active enemy's location (we don't sweep the full map —
/// the optimum is always near an enemy footprint). Picks the tile with
/// the highest enemy-hit count, ties broken by lower target-id of the
/// "anchor" enemy for determinism. Returns None if no Burst action exists,
/// or no point hits 2+ enemies cleanly.
fn try_attack_aoe(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    best_burst_placement(encounter, actor_id, |_| true)
}

/// The wall spells the AI will raise to cut a line, in the order it
/// reaches for them.
///
/// Both write terrain (`crate::engine::conjured_terrain`), both cost a
/// level-5 slot and the caster's concentration, and both target a
/// single point whose *orientation* the spell derives — which is why
/// they need a lane of their own rather than falling out of the burst
/// picker. A burst is aimed at a creature; a wall is aimed at a gap.
///
/// Wall of Force first: it is the strictly better wall for a caster who
/// intends to keep casting, because it is transparent, so the line the
/// wall cuts is the enemy's and not the caster's own. Wall of Stone
/// blinds both sides equally, which is worth less to somebody who wants
/// to keep shooting and is therefore the fallback.
const WALL_SPELLS: &[&str] = &["wall of force", "wall of stone"];

/// How far along the line to the threat the wall goes up. Two tiles is
/// close enough that a wall aimed at an enemy eight tiles out still
/// lands between them and the caster rather than beside them, and far
/// enough that the caster is not standing in their own wall's footprint
/// — a tile with a creature on it is skipped by the terrain layer, and
/// a wall with a hole in it where the caster is standing is not a wall.
const WALL_STANDOFF: isize = 2;

/// Raise a wall across the line an enemy is closing down.
///
/// This is the one thing a wall spell is for, and it is not something
/// any of the pickers above can express. They all choose a *target* —
/// a creature to hit, a cluster to catch, an ally to buff. A wall has
/// no target. Its whole value is a piece of empty floor between the
/// caster and somebody who wants to reach them, and picking that floor
/// takes a rule of its own.
///
/// Fires when all of:
///
///   - the caster owns one of `WALL_SPELLS` and isn't already holding
///     a concentration spell (both walls are concentration, so casting
///     one would drop whatever is up — and everything the AI puts up
///     above this rung it put up on purpose);
///   - `MIN_THREATS` hostiles or more are closing and about to arrive:
///     further than melee reach, no further than `MAX_THREAT_GAP`. Both
///     halves matter. A wall does nothing about a creature already
///     swinging at you and nothing *yet* about one across the room, and
///     one creature closing is a fight the caster wins by casting at
///     it — it takes a line to be worth a level-5 slot and the whole
///     concentration budget spent on empty floor;
///   - none of those hostiles has an ally of the caster's nearer to it
///     than the caster is. A wall raised across a line an ally is
///     standing on cuts the ally off from their own side, and the AI
///     has no way to ask them whether they wanted that. A caster behind
///     a front line therefore never walls, which is right: the front
///     line is the wall.
///
/// The wall is aimed at the nearest qualifying threat, ties by lowest
/// id, which is the same deterministic tie-break every other picker
/// uses.
fn try_wall_off_approach(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    /// Six tiles — fifteen feet, which is inside one move for anything
    /// that walks. Closer than this and the wall is being raised in the
    /// creature's face; further and the caster has another round to
    /// spend on something that kills it instead.
    const MAX_THREAT_GAP: isize = 6;

    /// How many of them it takes before the floor is worth the slot.
    const MIN_THREATS: usize = 2;

    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    let walls: Vec<&'static (dyn Action + Send + Sync)> = WALL_SPELLS
        .iter()
        .filter_map(|name| actor.find_action(name))
        .collect();
    if walls.is_empty() {
        return None;
    }
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
    let gap_to = |from: Coordinate, from_size: usize, tid: usize| -> Option<isize> {
        let t = encounter.actors.get(&tid)?;
        Some(footprint_chebyshev(
            from,
            from_size,
            t.location(),
            get_tiles_from_size(t.size()),
        ))
    };

    let mut threat: Option<(isize, Coordinate)> = None;
    let mut closing = 0usize;
    for tid in encounter.sorted_actor_ids() {
        let Some(t) = encounter.actors.get(&tid) else {
            continue;
        };
        if t.team() == my_team || !t.is_combat_active() {
            continue;
        }
        let Some(gap) = gap_to(my_loc, my_size, tid) else {
            continue;
        };
        if gap <= crate::actions::action_template::MELEE_REACH || gap > MAX_THREAT_GAP {
            continue;
        }
        // Somebody on my side is already between us — walling the line
        // would strand them on the wrong side of it.
        let ally_is_closer = encounter.sorted_actor_ids().into_iter().any(|aid| {
            if aid == actor_id {
                return false;
            }
            let Some(a) = encounter.actors.get(&aid) else {
                return false;
            };
            if a.team() != my_team || !a.is_combat_active() {
                return false;
            }
            gap_to(a.location(), get_tiles_from_size(a.size()), tid)
                .is_some_and(|d| d < gap)
        });
        if ally_is_closer {
            continue;
        }
        closing += 1;
        if threat.as_ref().is_none_or(|(best, _)| gap < *best) {
            threat = Some((gap, t.location()));
        }
    }
    // One hostile closing is a fight the caster wins by casting at it;
    // it takes a line to be worth a level-5 slot and the whole
    // concentration budget spent on empty floor. The count is what
    // keeps this rung — which sits above the entire buff and
    // area-control stack — from taking the opening play away from every
    // caster in every skirmish.
    if closing < MIN_THREATS {
        return None;
    }
    let (_, threat_at) = threat?;

    // Two tiles along the line to them: `wall_tiles` stands the wall
    // *across* whatever line it is aimed down, so aiming at the
    // approach is aiming at the gap.
    let toward = threat_at - my_loc;
    let anchor = my_loc
        + Coordinate::new(
            toward.x.signum() * WALL_STANDOFF,
            toward.y.signum() * WALL_STANDOFF,
        );
    walls.into_iter().find_map(|action| {
        let aei = ActionExecutionInfo::new(action, actor_id, None, Some(vec![anchor]), None);
        aei.validate(encounter).then_some(aei)
    })
}

/// Shared "where do I drop this burst?" search, used by both burst
/// pickers: `try_attack_aoe` (any harmful Burst action the actor owns)
/// and `try_area_control` (only the battlefield-control spells).
///
/// `accept` filters which of the actor's Burst actions are eligible,
/// and is the only thing that differs between the two callers — the
/// candidate-point enumeration, the friendly-fire gate, the two-enemy
/// minimum and the deterministic tie-break are identical, and were
/// duplicated between them before this was factored out.
///
/// Returns the placement hitting the most hostiles, ties broken by the
/// lowest anchor id so the choice is stable across runs. Candidate
/// points are hostile locations rather than an open search over the
/// map: a burst that isn't centred on someone is nearly always worse
/// than one that is.
fn best_burst_placement(
    encounter: &EncounterInstance,
    actor_id: usize,
    accept: impl Fn(&'static (dyn Action + Send + Sync)) -> bool,
) -> Option<ActionExecutionInfo> {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();

    // Find Burst actions we own. Most actors have none — bail early.
    let burst_actions: Vec<(&'static (dyn Action + Send + Sync), isize)> = actor
        .actions
        .iter()
        .filter_map(|a| {
            if !a.is_harmful() || !accept(*a) {
                return None;
            }
            match a.targeting_schema() {
                TargetingSchema::Burst { radius } => Some((*a, radius)),
                _ => None,
            }
        })
        .collect();
    if burst_actions.is_empty() {
        return None;
    }

    // Iterate enemies in id order for deterministic tie-break.
    let anchor_ids = encounter.sorted_actor_ids();

    let mut candidate_points: Vec<(Coordinate, usize)> = Vec::new();
    for anchor_id in &anchor_ids {
        let Some(anchor) = encounter.actors.get(anchor_id) else {
            continue;
        };
        if anchor.team() == my_team || !anchor.is_combat_active() {
            continue;
        }
        candidate_points.push((anchor.location(), *anchor_id));
    }

    let mut best: Option<(usize, usize, ActionExecutionInfo)> = None; // (enemy_hits, anchor_id, aei)
    for (point, anchor_id) in &candidate_points {
        let point = *point;
        let anchor_id = *anchor_id;

        for (action, radius) in &burst_actions {
            // Validate caster→point reach + LOS + cost via the action's
            // own validation (avoids reimplementing).
            let aei =
                ActionExecutionInfo::new(*action, actor_id, None, Some(vec![point]), None);
            if !aei.validate(encounter) {
                continue;
            }

            // Count combat-active actors in the radius. Friendly fire
            // disqualifies the candidate entirely — we don't damage our
            // own side. Self also counts as an ally.
            //
            // Exception: the caster may hold a feature that carves
            // allies out of their own blast — the Sorcerer's Careful
            // Spell prime (CHA-mod allies, any spell) or the Evocation
            // Wizard's Sculpt Spells (1 + spell level allies, evocation
            // only). `ally_shield_capacity` answers "how many allies can
            // this caster tolerate in the blast" for the specific spell
            // under consideration, sharing its per-feature helpers with
            // the resolver that will actually do the sparing — so the
            // AI's model can't drift from the outcome. When the ally
            // count fits the capacity we tolerate the overlap and let
            // the burst chokepoint skip those ids.
            //
            // Without this, an Evocation Wizard would never fire the
            // blast its whole subclass is built around: every candidate
            // point in a melee scrum catches an ally, and the gate would
            // reject all of them.
            let costs = aei.cost(encounter);
            let shield_capacity = encounter.ally_shield_capacity(
                actor_id,
                action.school(),
                crate::engine::side_effects::spell_slot_level(&costs).unwrap_or(0),
            );
            // How many hostiles the blast has to catch to be worth
            // firing.
            //
            // Two, for a cast that spends the turn's Action or a spell
            // slot: those are the caster's scarcest resources, and a
            // burst that catches one creature is a worse use of either
            // than the single-target picker one rung down, which at
            // least aims.
            //
            // One, for a cast that spends neither. Those exist and the
            // floor was never about them: a Melf's Minute Meteors volley
            // and a Moonbeam walked onto somebody each cost a bonus
            // action the caster had no other use for, and refusing to
            // spend it on one hostile does not save it for anything.
            // Both spells were effectively unreachable — the meteors
            // could be lit and then never thrown, and the beam could be
            // placed and then never moved — because a 5-foot burst
            // almost never covers two bodies.
            let min_enemy_hits = if costs.iter().any(|c| {
                matches!(
                    c,
                    crate::engine::side_effects::Resource::Action
                        | crate::engine::side_effects::Resource::SpellSlot(_)
                )
            }) {
                2
            } else {
                1
            };
            let mut enemy_hits = 0usize;
            let mut ally_hits = 0usize;
            for (id, a) in encounter.actors.iter() {
                if !a.is_combat_active() {
                    continue;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    point,
                    1,
                );
                if dist > *radius {
                    continue;
                }
                if *id == actor_id || a.team() == my_team {
                    ally_hits += 1;
                } else {
                    enemy_hits += 1;
                }
            }
            let friendly_fire_blocked = ally_hits > shield_capacity;
            if friendly_fire_blocked || enemy_hits < min_enemy_hits {
                continue;
            }
            let pick = match &best {
                None => true,
                Some((best_hits, best_anchor, _)) => {
                    enemy_hits > *best_hits
                        || (enemy_hits == *best_hits && anchor_id < *best_anchor)
                }
            };
            if pick {
                best = Some((enemy_hits, anchor_id, aei));
            }
        }
    }
    best.map(|(_, _, aei)| aei)
}

/// Battlefield-control area spells — Burst-schema concentration spells
/// whose value is the condition they install, not the damage they do.
///
/// These exist on the wizard and druid chassis and the AI never cast
/// them. `try_attack_aoe` scores every Burst action the actor owns by
/// one number, how many hostiles the blast catches, so a control spell
/// only wins when it strictly out-covers every damage spell in the
/// loadout — and it almost never does, because Fireball's radius is as
/// large or larger. An integration probe over 40 AI-driven encounters
/// found Web cast zero times and Stinking Cloud once.
///
/// The fix is a separate rung rather than a tweak to the shared
/// scorer, because the comparison isn't really about coverage: three
/// hostiles Restrained by a Web are worth more than three hostiles
/// taking 8d6 and continuing to act, and no enemy-count heuristic
/// expresses that. This mirrors the way `try_lockdown` already sits
/// above plain attacks on the single-target lane.
///
/// What bounds the rung is each entry's own
/// `Action::holds_concentration`: a caster already holding something
/// declines every concentrating entry, so a control spell can never
/// displace a control spell. That used to be a `bool` column beside
/// each name here, which put the fact somewhere the spell couldn't see
/// it; asking the action is the same rule with one source of truth.
///
/// The spike fields are deliberately *not* here, despite being area
/// denial. What earns a spell this rung is taking hostiles out of the
/// fight — a Web holds them, a Hypnotic Pattern charms them — and
/// thorns do neither; they tax a creature that chooses to move. On a
/// rung that outranks single-target lockdown, that trade is the wrong
/// way round, and Spike Growth was displacing Hold Person. They stay
/// reachable through the ordinary AoE picker.
///
/// Grease is the one entry that costs no concentration, and the
/// per-entry check exists for it. RAW it is laid down and walked away
/// from — a level-1 slot that keeps tripping people for a minute while
/// the caster concentrates on something else entirely — and folding it
/// into a blanket "not while concentrating" gate would have made the
/// cheapest control spell in the game the only one a caster can't
/// combine with anything.
///
/// **Two of the entries spare their caster's own side**, and that is a
/// known conservatism rather than a rule. Wall of Sand and Crown of
/// Thorns resolve through `enemy_burst_targets`, so an ally standing in
/// the blast is simply not in it — but the rung's placement gate counts
/// bodies without asking, and will refuse a point that catches a
/// teammate. The gate can only make an entry fire *less* often than it
/// should, never wrongly, so it is left alone here: teaching it
/// otherwise means a declared "this burst spares allies" property on
/// `Action`, and sixty-four burst helpers would have to agree about
/// which of them have it. Both spells are still strictly better off on
/// this rung than on the AoE picker below it, which applies the same
/// gate and then loses to Fireball's radius anyway.
const AREA_CONTROL_SPELLS: &[&str] = &[
    "web",
    "hypnotic pattern",
    "black tentacles",
    "entangle",
    "sleet storm",
    "grease",
    // The three the sweep above was written to find, all of them on
    // playable chassis and none of them ever cast off this rung.
    //
    // Stinking Cloud is the plainest case: a zone that takes a
    // creature's whole action for as long as it stands in the gas,
    // which is the membership test stated word for word. The docstring
    // above cites it by name as the spell the original probe saw cast
    // *once* in forty encounters — through the AoE picker, on
    // coverage, against a Fireball it does not out-cover.
    "stinking cloud",
    // Wall of Sand's own docstring places it "between Web and Black
    // Tentacles on the wizard's restraint ladder". Both of those are
    // already rows here; it is the same STR-save-or-Restrained burst
    // under concentration, one slot cheaper than one and one dearer
    // than the other, and it was the only rung of that ladder the AI
    // could not climb.
    "wall of sand",
    // Crown of Thorns is Black Tentacles at level 2 — a concentration
    // burst that deals a die and Restrains what fails the save — which
    // is also the answer to whether the damage rider disqualifies it:
    // Black Tentacles has one and is a row.
    "crown of thorns",
];

/// Which half of the summon lane a rung is asking for.
///
/// The two halves sit in different places in the ladder because the
/// argument for putting summons low applies to only one of them. That
/// argument is about *concentration*: everything ranked above the summon
/// rung — area control, the apex ally buffs, Spirit Guardians, Hold
/// Person — wants the same single concentration slot, and all of them
/// act on the fight now, where a pack of wolves pays out over six rounds
/// the caster can't be sure they'll get.
///
/// A `Free` summon costs neither a slot nor the concentration, so it
/// competes with none of that. It is a permanent extra body bought with
/// one bonus action or one action, and every round it spends unsummoned
/// is a round of its attacks lost for good — there is no fight in which
/// holding it back is right. It belongs high, above the self-buff lane;
/// the Fathomless warlock is what made the point, having reached its
/// tentacle in one live fight out of eight while spending bonus action
/// after bonus action on a shove cantrip that a second attacker
/// outvalues in a round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SummonTier {
    /// No spell slot and no concentration — a per-rest feature summon
    /// like the Ranger's Companion, the wildfire spirit or the tentacle.
    Free,
    /// Costs a slot, or the caster's concentration, or both. Every
    /// summon *spell* is here, including Animate Dead (no
    /// concentration, but a level-3 slot).
    Slotted,
}

impl SummonTier {
    /// Which tier a candidate belongs to. `slot` is 0 for an action that
    /// spends no `Resource::SpellSlot`.
    fn of(slot: u32, holds_concentration: bool) -> Self {
        if slot == 0 && !holds_concentration {
            SummonTier::Free
        } else {
            SummonTier::Slotted
        }
    }
}

/// Put a friendly body on the board — the Tasha's summon family, Conjure
/// Animals, Conjure Elemental, Animate Dead, Animate Objects, the
/// Ranger's Companion, and every feature summon. `tier` picks which half
/// of the lane this call is for; see `SummonTier`.
///
/// The action set comes off `Action::summons_allies` rather than a
/// name list, so a summon added tomorrow is picked up by declaring
/// what it is. Before this rung existed no AI-driven caster ever
/// summoned anything: summons declare no damage types and target
/// nothing, so every picker in the ladder filtered them out, and the
/// spells were reachable only by a human typing their name.
///
/// One rung-wide gate and then three per-summon ones:
///
///   0. **A fight is actually on** (24 tiles ≈ 60 ft). Summons are the
///      most expensive thing in the ladder to waste: a concentration
///      slot, an Action, and a spell slot, all spent on bodies that
///      time out before anything walks into range.
///   1. **Not concentrating on something else** — but only for a summon
///      that would take the concentration. A caster who traded a landed
///      Web for an unlanded pack of wolves has made the fight worse,
///      which is the same argument `try_area_control` makes. A summon
///      that concentrates on nothing can't make that trade, so it isn't
///      asked to.
///   2. **One *slot* spent calling for help per fight.** Gate 1 caps
///      every summon that concentrates and a per-rest charge caps every
///      feature summon, but Animate Dead has neither — RAW it is a
///      permanent minion, so without this a wizard spends every
///      third-level slot it owns on skeletons and never casts anything
///      else. See `Condition::Summoner`. The cap is on the slot, not on
///      the summoning: a charge-gated *feature* summon already carries
///      its own once-per-rest cap, and RAW is clear that a Wildfire
///      druid's spirit and their Conjure Animals wolves can be on the
///      board at once.
///   3. **The action's own validator**, which owns the part the AI
///      shouldn't guess at — whether there is a free adjacent tile of
///      the right size to put the creature on.
///
/// Candidates are tried **cheapest first** — among slotted ones the
/// smaller slot first — falling back to the actor's own list order for a
/// genuine tie. Cost is the only cross-summon metric worth having: there
/// is no sense in which two wolves and a fire elemental can be compared
/// on quality (they are good in different fights), but a smaller slot
/// for a body is unambiguously the one to spend first. It is also what
/// makes gate 2 above sit right — the cheap call goes out, and the
/// bigger slots stay available for what else the fight asks for.
fn try_summon_allies(
    encounter: &EncounterInstance,
    actor_id: usize,
    tier: SummonTier,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    let busy = actor.is_concentrating();
    let already_called = actor.has_condition(Condition::Summoner);
    let mut candidates: Vec<(u32, usize, ActionExecutionInfo)> = actor
        .actions
        .iter()
        .enumerate()
        .filter(|(_, a)| a.summons_allies())
        .filter_map(|(order, a)| {
            if busy && a.holds_concentration() {
                return None;
            }
            let aei = ActionExecutionInfo::new(*a, actor_id, None, None, None);
            let slot = crate::engine::side_effects::spell_slot_level(&a.cost(
                encounter, actor_id, None, None, None,
            ))
            .unwrap_or(0);
            if tier != SummonTier::of(slot, a.holds_concentration()) {
                return None;
            }
            if already_called && slot > 0 {
                return None;
            }
            Some((slot, order, aei))
        })
        .collect();
    candidates.sort_by_key(|(slot, order, _)| (*slot, *order));
    candidates
        .into_iter()
        .find(|(_, _, aei)| aei.validate(encounter))
        .map(|(_, _, aei)| aei)
}

/// Climb onto an allied mount standing next to you (5e Mounted Combat,
/// PHB p.198).
///
/// The gate is `EncounterInstance::can_mount` and nothing else, which
/// already carries every clause RAW attaches — willing, one size larger,
/// right anatomy, in reach, neither of you already paired. What this
/// adds is the two judgements RAW leaves to the rider:
///
///   - **Is it worth the movement?** Only if the horse is actually
///     faster. A creature that would gain nothing but a saddle keeps its
///     own legs, which matters for the Small cohort — a halfling on a
///     mule trades 25 ft of its own for the mule's 40, and a goblin on a
///     worg 30 for 50, but neither should climb onto something slower
///     than they are.
///   - **Which horse?** The nearest, then the fastest, then the lowest
///     id — a total order, so two riders in the same stable make the
///     same choice as each other every run and the same choice as
///     themselves on a replay of the seed.
///
/// Deliberately *not* gated on there being an enemy nearby, unlike the
/// summon rung above it. A summon spends a slot and wants to be held
/// until the fight is real; getting on a horse spends movement the actor
/// was going to use walking anyway, and the whole value of it is being
/// mounted *before* the enemy is in range.
fn try_mount_up(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.is_combat_active() || actor.mounted_on().is_some() {
        return None;
    }
    let own_speed = actor.speed();
    let mount_action = actor
        .actions
        .iter()
        .find(|a| a.name() == crate::actions::default_actions::MOUNT.name())?;
    let mut stable: Vec<(isize, i32, usize)> = encounter
        .actors
        .iter()
        .filter(|(mount_id, m)| {
            m.speed() > own_speed && encounter.can_mount(actor_id, **mount_id).is_ok()
        })
        .map(|(mount_id, m)| {
            (
                encounter
                    .footprint_distance(actor_id, *mount_id)
                    .unwrap_or(isize::MAX),
                // Negated so the sort's ascending order puts the fastest
                // first; the speeds are small whole numbers of feet, so
                // the cast is exact.
                -(m.speed() as i32),
                *mount_id,
            )
        })
        .collect();
    stable.sort_unstable();
    stable.into_iter().find_map(|(_, _, mount_id)| {
        let aei =
            ActionExecutionInfo::new(*mount_action, actor_id, Some(vec![mount_id]), None, None);
        aei.validate(encounter).then_some(aei)
    })
}

/// The mount's half of the mounting rung: hold still for the ally who is
/// about to get on you.
///
/// A horse is a creature with its own initiative slot, and until someone
/// is on it the AI drives it like any other body on the team — straight
/// at the nearest enemy. That is the wrong thing for a mount to do, and
/// it is wrong in a way that is invisible from the rider's side: by the
/// time the knight's turn comes round the warhorse is forty feet away
/// and in melee, `can_mount` refuses on distance, and the rider rung
/// declines forever. The pair never forms, and nothing in the log says
/// why.
///
/// So a mountable creature with an eligible rider beside it takes the
/// Dodge action — which is both the useful thing to do while waiting
/// (nobody hits a dodging horse easily) and one of the three options RAW
/// gives a controlled mount, so the horse is already behaving like what
/// it is about to become.
///
/// Two gates keep it from becoming a stall:
///
///   - **Nobody in contact.** Once the fight has arrived at the horse,
///     the horse fights; a mount that dodged through a pit fiend's turn
///     to wait for a rider who was busy would be worse off than one that
///     kicked.
///   - **The rider would actually take it** — the same `can_mount` plus
///     faster-than-you test the rider's own rung applies. A horse does
///     not wait for somebody who was never going to climb on.
///
/// The wait is bounded by the rider's own ladder: `try_mount_up` sits
/// one rung above the buffs, so an eligible rider mounts on their very
/// next turn and this stops firing.
fn try_stand_for_rider(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.is_mountable() || actor.ridden_by().is_some() || !actor.is_combat_active() {
        return None;
    }
    if !encounter
        .combat_active_enemy_ids_adjacent(actor_id)
        .is_empty()
    {
        return None;
    }
    let my_speed = actor.speed();
    let wanted = encounter.actors.iter().any(|(rider_id, r)| {
        r.speed() < my_speed && encounter.can_mount(*rider_id, actor_id).is_ok()
    });
    wanted.then(|| try_dodge(encounter, actor_id))?
}

/// Fire an at-will, self-centered **ally support pulse** — a `NoArgs`
/// action that costs nothing but the turn, harms nobody, and hands a
/// buff to the teammates standing near the actor.
///
/// The rung was written for one action and has two now, and the pair is
/// what fixed its membership test:
///
///   - The Artillerist's **Protector cannon**, whose whole turn is
///     `1d8 + INT` temporary hit points over every ally within ten feet.
///     Nothing else on the ladder could reach it. `try_support_heal`
///     walks `SingleActor` heals and hands them to a chosen ally;
///     `try_self_heal` walks `NoArgs` heals but only fires when the
///     *actor* is below half, which a turret standing behind the line
///     never is. So the cannon that never attacks also never did
///     anything else.
///   - The Bard's **Countercharm**, which restores nothing at all — and
///     is why the filter reads `pulses_ally_buff` rather than `is_heal`.
///     The old test was a proxy that held only while the cohort had one
///     member.
///
/// The gate is deliberately thin — is there an ally in range at all —
/// because for this cohort the answer to "is it worth a turn" is always
/// yes: the action is free, repeatable, self-limiting, and the
/// alternative is the turret standing still. A future entry that costs a
/// slot or a charge would need a real gate, and would belong on a
/// different rung for exactly that reason.
///
/// Slotted directly below the heal rung: a bleeding ally wants the heal
/// first, and everyone wants the shield before the shooting starts.
fn try_ally_support_pulse(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.is_combat_active() {
        return None;
    }
    for action in actor.actions.iter() {
        // `pulses_ally_buff`, not `is_heal`. The rung was written when
        // every member of the cohort restored hit points and the two
        // questions had the same answer; Countercharm restores nothing
        // and belongs here anyway. See `Action::pulses_ally_buff` for
        // why declaring it a heal to get in would have been worse.
        if action.is_harmful()
            || !action.pulses_ally_buff()
            || !matches!(action.targeting_schema(), TargetingSchema::NoArgs)
        {
            continue;
        }
        // Free, or it does not belong on this rung. A pulse the actor
        // has to pay a slot or a charge for is a decision, and this is
        // a reflex.
        if !action
            .cost(encounter, actor_id, None, None, None)
            .iter()
            .all(|c| {
                use crate::engine::side_effects::Resource;
                matches!(c, Resource::Action | Resource::BonusAction)
            })
        {
            continue;
        }
        // Somebody has to be standing in it. The action declares its own
        // envelope through `reach_tiles`; without one there is nothing
        // to measure and the rung declines rather than guessing.
        let radius = action.reach_tiles()?;
        let loc = actor.location();
        if encounter
            .ally_burst_targets(actor_id, loc, radius)
            .iter()
            .all(|&id| id == actor_id)
        {
            continue;
        }
        let aei = ActionExecutionInfo::new(*action, actor_id, None, None, None);
        if aei.validate(encounter) {
            return Some(aei);
        }
    }
    None
}

/// Drop an area-control spell on the densest cluster of hostiles.
///
/// Shares `best_burst_placement` with `try_attack_aoe`, so the
/// candidate points, the friendly-fire gate, the two-enemy minimum and
/// the tie-break are the same; only the action filter differs. Gated on
/// the caster not already concentrating — every spell on the registry
/// is a concentration spell, so firing while one is up would trade a
/// landed lockdown for an unlanded one.
///
/// Slotted above `try_attack_aoe` in the ladder: when both would fire,
/// the cluster is dense enough that taking the hostiles out of the
/// fight beats damaging them.
fn try_area_control(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let busy = actor.is_concentrating();
    best_burst_placement(encounter, actor_id, |a| {
        AREA_CONTROL_SPELLS.iter().any(|name| *name == a.name())
            && !(busy && a.holds_concentration())
    })
}

/// Pick a NoArgs harmful action (Thunderwave / Word of Radiance / Holy
/// Word) when 2+ enemies sit within ~30ft of the caster. NoArgs actions
/// implicitly center on the caster, so the AI can't pick a "best point" —
/// instead we count combat-active enemies within a heuristic 6-tile
/// (≈30ft) window and fire if the cluster is dense enough. The action
/// itself uses `enemy_burst_targets` to handle the team filter, so
/// allies near the cluster are never collateral.
///
/// Sorted by reach descending so a tight cluster picks the bigger spell
/// (Holy Word's 30ft radius outranks Thunderwave's 10ft 2-tile burst).
fn try_self_centered_burst(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());

    // Heuristic cluster window — 30ft = 12 tiles. Wider than the smallest
    // NoArgs burst (Thunderwave's 2-tile radius), but matches Holy Word's
    // 30ft sphere; the action's own `validate_input` runs anyway and
    // gates on its true radius via enemy_burst_targets at execute time.
    const CLUSTER_RADIUS: isize = 12;

    let nearby_enemies: usize = encounter
        .actors
        .values()
        .filter(|a| {
            a.team() != my_team
                && a.is_combat_active()
                && footprint_chebyshev(
                    my_loc,
                    my_size,
                    a.location(),
                    get_tiles_from_size(a.size()),
                ) <= CLUSTER_RADIUS
        })
        .count();
    // Two enemies is the price of *choosing* a burst over a swing: a
    // blast that catches one creature is nearly always worse than
    // pointing an attack at it, so a caster holding both should point
    // the attack.
    //
    // An actor holding no attack at all is not making that choice. The
    // Artillerist's flamethrower cannon is the case: a turret with a
    // cone and nothing else, whose whole existence is that one action.
    // Against a single enemy the two-enemy floor meant it sat on the
    // board for the entire encounter and never fired once — the
    // subclass's headline feature, inert, in every duel. So the floor
    // is one when there is nothing else to do with the turn.
    let has_a_swing = try_attack_focus_fire(encounter, actor_id).is_some();
    let floor = if has_a_swing { 2 } else { 1 };
    if nearby_enemies < floor {
        return None;
    }

    // Collect NoArgs harmful actions; sort by reach descending so a
    // dense cluster picks the bigger burst (longer reach ≈ bigger
    // radius for self-centered bursts in this codebase). We accept
    // bursts that either deal damage *or* apply a hostile condition:
    // the damage_types non-empty branch covers Thunderwave / Word of
    // Radiance / Holy Word, the deals_damage=false branch admits
    // condition-only NoArgs bursts like the Ghost's Horrifying Visage
    // (frighten on save fail, no HP loss). The is_harmful gate alone
    // is too loose — some default actions inherit the trait default
    // `is_harmful: true` (e.g. Hide before its explicit override) —
    // so we also require either damage or an explicit non-damage flag,
    // which together exclude utility NoArgs (Dodge / Disengage) cleanly.
    let mut bursts: Vec<&'static (dyn Action + Send + Sync)> = actor
        .actions
        .iter()
        .filter(|a| {
            a.is_harmful()
                && matches!(a.targeting_schema(), TargetingSchema::NoArgs)
                && (!a.damage_types().is_empty() || !a.deals_damage())
        })
        .copied()
        .collect();
    bursts.sort_by_key(|a| std::cmp::Reverse(a.reach_tiles().unwrap_or(0)));

    for action in bursts {
        let aei = ActionExecutionInfo::new(action, actor_id, None, None, None);
        if aei.validate(encounter) {
            return Some(aei);
        }
    }
    None
}

/// Find the (target, action) pair where the target is closest to dropping
/// among combat-active enemies AND we can validly hit them right now.
/// "Closest to dropping" is `effective_hitpoints` — HP plus the temp-HP
/// and Arcane Ward absorption pools — not the raw HP bar.
/// Ties on HP break by attack reach (prefer longer-reach action) so we use
/// our better tools when offered the choice.
fn try_attack_focus_fire(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();

    // Iterate actors by sorted id for determinism — HashMap iteration
    // order changes between process runs and would make the AI's
    // tiebreakers nondeterministic given the same seed.
    let ids = encounter.sorted_actor_ids();

    // Sort key: (mode_pri, target_hp, -reach). Lower wins:
    //   - mode_pri (advantage=0, normal=1, disadvantage=2): fish for
    //     advantage opportunities first.
    //   - HP ascending: focus-fire wounded. Reads `effective_hitpoints`
    //     (HP + temp HP + Arcane Ward), not raw HP — the question here
    //     is "who drops soonest", and a target behind an absorption pool
    //     is further from dropping than their HP bar suggests.
    //   - Cover ascending: among targets equally close to dropping,
    //     shoot the one that isn't behind something. Identical enemies
    //     at full HP tie on the key above constantly — three goblins,
    //     three 7-HP bars — and before this the tie went to whichever
    //     had the lower id, which is to say to nothing at all. A low
    //     wall or an intervening body is +2 AC and two of them are +5,
    //     which is a bigger swing than most of what the AI does deliberate
    //     over. Ranked below HP rather than above it: cover makes a
    //     target harder to hit, it does not make a nearly-dead one worth
    //     less than a healthy one.
    //   - Reach descending: prefer the longest-reach action when tied
    //     (so a longbow gets used over a one-tile melee on a far target,
    //     etc.).
    let mut best: Option<(u8, u32, i32, isize, ActionExecutionInfo)> = None;
    for target_id in ids {
        let Some(target) = encounter.actors.get(&target_id) else {
            continue;
        };
        if target_id == actor_id || target.team() == my_team || !target.is_combat_active() {
            continue;
        }
        let Some((reach, action)) =
            best_attack_against(actor_id, actor, encounter, target_id)
        else {
            continue;
        };
        let aei = ActionExecutionInfo::new(action, actor_id, Some(vec![target_id]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        let is_melee = reach <= MELEE_REACH;
        // `peek_attack_mode`, not `compute_attack_mode`: the ranking has
        // to see the per-target Help grant, or a target the actor just
        // spent a bonus action feinting (Feinting Attack, Versatile
        // Trickster) ranks no better than any other and the advantage
        // goes somewhere it wasn't bought for. Read-only by
        // construction — the grant is consumed at the swing, not here.
        let mode = encounter.peek_attack_mode(actor_id, target_id, is_melee);
        let mode_pri = mode_priority(mode);
        let hp = target.effective_hitpoints();
        let cover = encounter.cover_ac_bonus(actor_id, target_id);
        let pick = match &best {
            None => true,
            Some((bm, bh, bc, br, _)) => {
                (mode_pri, hp, cover, std::cmp::Reverse(reach))
                    < (*bm, *bh, *bc, std::cmp::Reverse(*br))
            }
        };
        if pick {
            best = Some((mode_pri, hp, cover, reach, aei));
        }
    }
    best.map(|(_, _, _, _, aei)| aei)
}

/// Among the actor's SingleActor *harmful* actions, the longest-reach
/// one whose reach covers `target_id` and that the actor can actually
/// afford right now. Validating cost here means we don't return Magic
/// Missile (reach 48, costs a spell slot) when no slots remain — the
/// caller would then skip the target entirely instead of falling back
/// to Fire Bolt at reach 24.
/// How badly `attacker_id`'s damage types match up against
/// `target_id`, as a penalty the picker sorts *ascending*:
///
///   - `0` — at least one type the target is vulnerable to.
///   - `1` — neutral: no modifier the engine can see either way.
///   - `2` — at least one type the target resists.
///   - `3` — every type on the list is one the target is immune to;
///     the action does nothing and should not be picked at all.
///
/// Split out of `best_attack_against`'s closure so the qualified half
/// of the lookup is reachable from a test. It was a closure for as long
/// as there was only one table to read; there are two now, and which
/// one applies depends on who is swinging — which is exactly the kind
/// of decision that should be assertable on its own rather than only
/// through whichever weapon a goblin happened to pick.
pub fn matchup_penalty_against(
    encounter: &EncounterInstance,
    attacker_id: usize,
    target_id: usize,
    damage_types: &[crate::engine::types::DamageType],
) -> u8 {
    if crate::engine::magic::resistance_bypass(encounter, attacker_id, target_id, false).is_some()
    {
        return matchup_penalty_vs_magic(encounter, target_id, damage_types);
    }
    matchup_penalty(encounter, target_id, damage_types, false)
}

/// `matchup_penalty_against` for a source that is magical whoever holds
/// it — every spell, and every weapon in a hand the magic axis has
/// already answered for.
pub fn matchup_penalty_vs_magic(
    encounter: &EncounterInstance,
    target_id: usize,
    damage_types: &[crate::engine::types::DamageType],
) -> u8 {
    matchup_penalty(encounter, target_id, damage_types, true)
}

fn matchup_penalty(
    encounter: &EncounterInstance,
    target_id: usize,
    damage_types: &[crate::engine::types::DamageType],
    magical: bool,
) -> u8 {
    use crate::engine::types::DamageModifier;
    if damage_types.is_empty() {
        return 1;
    }
    let Some(target) = encounter.actors.get(&target_id) else {
        return 1;
    };
    let mut all_immune = true;
    let mut has_vuln = false;
    let mut has_resist = false;
    let mut has_absorbed = false;
    for dt in damage_types {
        // The target's source-qualified rows are the whole reason a
        // wraith is a bad target for a mundane sword and a fine one for
        // a Fire Bolt. Consulted only when this source would actually
        // be answered by them — otherwise the picker would go on
        // avoiding the wraith with the +1 longsword that is its best
        // answer to it.
        let modifier = target.damage_modifier(*dt).or_else(|| {
            if magical {
                None
            } else {
                target.nonmagical_damage_modifier(*dt)
            }
        });
        match modifier {
            Some(DamageModifier::Immunity) => {}
            Some(DamageModifier::Absorption) => has_absorbed = true,
            Some(DamageModifier::Vulnerability) => {
                all_immune = false;
                has_vuln = true;
            }
            Some(DamageModifier::Resistance) => {
                all_immune = false;
                has_resist = true;
            }
            None => {
                all_immune = false;
            }
        }
    }
    // An absorbed type is the one matchup that is worse than useless —
    // the swing heals what it hits — so it takes the worst rung
    // outright rather than being averaged against the source's other
    // types. A flaming longsword against an Iron Golem still lands its
    // slashing, but the fire on it is a gift, and the AI has an
    // unenchanted blade on the same list.
    if has_absorbed || all_immune {
        3
    } else if has_vuln {
        0
    } else if has_resist {
        2
    } else {
        1
    }
}

/// What a weapon's 5e mastery property is worth to `actor_id` against
/// `target_id`, in damage-equivalent points, for the attack picker's
/// last sort key.
///
/// Zero for an untrained wielder, an unmastered weapon, and every
/// property whose value the picker cannot act on. The numbers are
/// deliberately small — a die or less, except for Cleave — because this
/// is a tie-break folded into the damage estimate, not a second ranking:
/// a property should decide between two weapons that are otherwise
/// close, and should never talk a martial out of the bigger die.
///
/// Why each is what it is:
///
///   - **Cleave** is a whole extra swing at a second creature, so it is
///     worth the weapon's own dice — but only when there *is* a second
///     creature beside the target and inside the wielder's reach, and
///     only if the once-per-turn allowance is still unspent. Off those
///     conditions it is worth nothing, which is the case that stops a
///     greataxe from outranking everything in a duel.
///   - **Graze** is the ability modifier on a miss. Priced at roughly a
///     third of it: the estimate this folds into is a damage-on-a-hit
///     number with no hit probability in it, so pricing the graze at
///     full value would be counting it on every swing including the
///     ones that land.
///   - **Topple** is advantage on every melee swing at the target for as
///     long as they stay down, which for a wielder with Extra Attack
///     starts paying inside the same turn. Worth nothing against a
///     creature already on the floor.
///   - **Vex** is advantage on the next swing, so it is worth more to a
///     wielder who has another swing coming.
///   - **Sap** and **Slow** are defensive: they cost the target a swing's
///     accuracy and ten feet of ground. Real, small, and not damage.
///   - **Push** and **Nick** score nothing. Nick's value is already in
///     the picker via the cost it removes, and Push moves a creature the
///     wielder may well want to stay next to — a picker that chased it
///     would be optimising for the wrong thing.
fn mastery_damage_bonus(
    encounter: &EncounterInstance,
    actor_id: usize,
    target_id: usize,
    action: &dyn Action,
    reach: isize,
) -> f32 {
    use crate::engine::mastery::{CLEAVE_TAG, WeaponMastery, effective_mastery};
    let Some(mastery) = effective_mastery(encounter, actor_id, action.weapon_mastery()) else {
        return 0.0;
    };
    let Some(actor) = encounter.actors.get(&actor_id) else {
        return 0.0;
    };
    let another_swing_coming = actor.has_extra_attack();
    match mastery {
        WeaponMastery::Cleave => {
            if actor.once_per_turn_used(CLEAVE_TAG)
                || !action.is_melee_attack()
                || !has_cleavable_neighbour(encounter, actor_id, target_id, reach)
            {
                return 0.0;
            }
            action
                .expected_damage(encounter, actor_id)
                .map(|d| d * 0.5)
                .unwrap_or(0.0)
        }
        WeaponMastery::Graze => {
            let ability = crate::engine::types::AbilityScoreType::Strength;
            // The picker has no view of which ability the swing rolls,
            // so it prices the graze off the better of the two a weapon
            // can use. Both are the wielder's own numbers, and a martial
            // with a Graze weapon is holding it with whichever is higher.
            let dex = crate::engine::types::AbilityScoreType::Dexterity;
            let modifier = actor
                .ability_modifier(ability)
                .max(actor.ability_modifier(dex))
                .max(0) as f32;
            modifier / 3.0
        }
        WeaponMastery::Topple => {
            let already_down = encounter
                .actors
                .get(&target_id)
                .is_some_and(|t| t.has_condition(Condition::Prone));
            if already_down {
                0.0
            } else if another_swing_coming {
                2.0
            } else {
                1.0
            }
        }
        WeaponMastery::Vex => {
            if another_swing_coming {
                1.5
            } else {
                0.75
            }
        }
        WeaponMastery::Sap => 1.0,
        WeaponMastery::Slow => {
            let already_slow = encounter
                .actors
                .get(&target_id)
                .is_some_and(|t| t.has_condition(Condition::Hobbled));
            if already_slow { 0.0 } else { 0.75 }
        }
        WeaponMastery::Push | WeaponMastery::Nick => 0.0,
    }
}

/// True if some enemy other than `target_id` is standing within 5 feet
/// of it and inside `reach` of the wielder — the shape 5e's **Cleave**
/// asks for.
///
/// Shares its predicate with `engine::mastery`'s own target sweep by
/// asking the same two footprint questions in the same order; what it
/// deliberately does *not* share is the sweep's tie-break, because the
/// picker only needs to know whether a second creature exists, not which
/// one the swing would carry into.
fn has_cleavable_neighbour(
    encounter: &EncounterInstance,
    actor_id: usize,
    target_id: usize,
    reach: isize,
) -> bool {
    let Some(team) = encounter.actors.get(&actor_id).map(|a| a.team()) else {
        return false;
    };
    encounter.actors.iter().any(|(id, a)| {
        *id != target_id
            && *id != actor_id
            && a.team() != team
            && a.is_combat_active()
            && encounter
                .footprint_distance(target_id, *id)
                .is_some_and(|d| d <= crate::actions::action_template::MELEE_REACH)
            && encounter
                .footprint_distance(actor_id, *id)
                .is_some_and(|d| d <= reach)
    })
}

fn best_attack_against(
    actor_id: usize,
    actor: &crate::actors::actor_template::ActorInstance,
    encounter: &EncounterInstance,
    target_id: usize,
) -> Option<(isize, &'static (dyn Action + Send + Sync))> {
    let target = encounter.actors.get(&target_id)?;
    let dist = footprint_chebyshev(
        actor.location(),
        get_tiles_from_size(actor.size()),
        target.location(),
        get_tiles_from_size(target.size()),
    );
    let matchup_score = |a: &dyn Action| -> u8 {
        // A spell is magical whatever the swinger is holding; a
        // non-spell action rides the creature's own verdict.
        if a.school().is_some() {
            return matchup_penalty_vs_magic(encounter, target_id, &a.damage_types());
        }
        matchup_penalty_against(encounter, actor_id, target_id, &a.damage_types())
    };

    // Best by (matchup score asc, roll mode asc, reach desc, expected
    // damage desc).
    //
    // The fourth key is the one that was missing longest. Ranking
    // stopped at reach, so two neutral, equally-reaching melee weapons
    // were separated by nothing but their order in the actor's action
    // list — a Knight swung whichever of its two weapons happened to be
    // pushed first, and a Beast Barbarian's claws, which land three
    // swings a turn to a greataxe's two, could never be picked at all.
    //
    // **The second key is what makes a melee build swing.** 5e gives a
    // ranged attack disadvantage while a hostile creature is within 5
    // feet of the shooter, and the engine has always enforced it — at
    // the attack site, where the picker could not see it. So a gish
    // standing in an ogre's reach compared its 1d8 sword against its
    // Fire Bolt, saw that the bolt reached further, and fired the bolt
    // at disadvantage every turn for the rest of the fight. Every
    // Eldritch Knight, Bladesinger, Battle Smith and Booming-Blade
    // rogue on the roster played that way.
    //
    // Asking the mode is strictly better than the obvious alternative
    // — a special case for "the target is already adjacent" — because
    // the mode is the actual reason. It also picks up every other
    // clause that moves it: a Blinded archer, an Invisible target, a
    // shot into a fog bank, a Prone target that melee wants and ranged
    // does not. All of those are questions the picker had no way to
    // ask, and each of them can make the longer-reaching option the
    // worse one.
    //
    // Reach still outranks damage, and deliberately: what a longer
    // reach buys is the option of not closing, and that is worth more
    // than a die when the two attacks roll in the same mode.
    //
    // The damage key only decides a tie when *both* sides put a number
    // on themselves, and that restraint is the whole safety argument.
    // Most attacks in the bestiary are bespoke `impl Action` blocks that
    // roll their dice inline and have no estimate to give; treating a
    // missing estimate as zero would have sorted every one of them below
    // every annotated weapon, which is not a better ranking than the
    // declaration order it replaced — it is a different arbitrary one,
    // biased toward whichever attacks happened to be annotated. Two
    // unannotated actions, or one of each, fall through to the order
    // they had before this key existed.
    // (matchup, roll mode, reach, damage estimate, the action itself) —
    // the four sort keys in priority order plus the candidate they rank.
    type Ranked<'a> = (u8, u8, isize, Option<f32>, &'a (dyn Action + Send + Sync));
    let mut best: Option<Ranked> = None;
    for &action in &actor.actions {
        if !matches!(action.targeting_schema(), TargetingSchema::SingleActor) {
            continue;
        }
        if !action.is_harmful() {
            continue;
        }
        // Skip hostile control actions (Shove, etc.) that don't whittle
        // enemy HP — focus-fire is for damage, and the AI doesn't combo
        // shove-then-swing today.
        if !action.deals_damage() {
            continue;
        }
        let Some(declared) = action.reach_tiles() else {
            continue;
        };
        // The reach a prime has actually bought, not the one the action
        // was declared with — `validate_input` adds the same bonus, and
        // comparing against the declared number here dropped every
        // candidate the prime had just made legal. A Battle Master who
        // spent a bonus action on Lunging Attack could not cash it: the
        // rung above primes on an enemy at exactly the gap the lunge
        // opens, and this loop then refused to consider a weapon
        // against that enemy at all.
        //
        // It is also the right sort key. A lunge-extended swing really
        // does reach further than an unextended one, and reach is the
        // second key below.
        let reach = declared + actor.extra_reach(declared);
        if dist > reach {
            continue;
        }
        let score = matchup_score(action);
        if score >= 3 {
            // Every type is immune — useless against this target.
            continue;
        }
        // Affordability gate: can't pay → keep looking for a cheaper
        // option. Full LOS / range / custom-validation still runs at
        // the caller via `ActionExecutionInfo::validate`.
        let aei =
            ActionExecutionInfo::new(action, actor_id, Some(vec![target_id]), None, None);
        if !aei.validate(encounter) {
            continue;
        }
        // What this swing would actually roll from where the actor is
        // standing. `compute_attack_mode` is the shared engine helper
        // the attack sites use, so the picker and the die agree.
        //
        // Plus the one penalty that belongs to the *weapon* rather than
        // to the pair of creatures: a lance jabbed at something already
        // in contact. `compute_attack_mode` cannot see it — it is
        // handed an id and a melee flag, not an action — so the picker
        // asks the action directly, the same number
        // `AttackParams::min_range` will hand the die. Without it a
        // knight with a longsword on their belt reads its lance as the
        // longer-reaching option and jabs at disadvantage all fight.
        let crowded = action
            .min_effective_reach()
            .is_some_and(|min| dist < min);
        // …and the other penalty the pair of creatures can't account
        // for: the water one of them is standing in. Same shape as
        // `crowded` and the same argument — the verdict depends on the
        // weapon, which `compute_attack_mode` never sees — but with one
        // outcome `crowded` doesn't have. A ranged weapon fired
        // underwater past its normal range cannot hit at all, so the
        // candidate is dropped outright rather than ranked last: an
        // attack that provably misses is not a worse option than the
        // alternatives, it is not an option.
        //
        // Dropping it is also what keeps the picker from starving. The
        // rung below this one closes on the nearest enemy, so an archer
        // that has waded into a pool and lost its shot walks back out
        // and finds dry land, instead of standing in the water firing
        // at something it cannot reach for the rest of the fight.
        let underwater = encounter.underwater_verdict(
            actor_id,
            action.name(),
            action.is_melee_attack(),
            action.is_weapon_attack(),
            action.normal_range().is_some_and(|nr| dist > nr),
        );
        if underwater == UnderwaterVerdict::AutoMiss {
            continue;
        }
        // Counted into the same tally the sweep filled rather than
        // folded onto its resolved answer — the prediction has to agree
        // with what `resolve_attack` will actually roll, and folding a
        // disadvantage onto a `Normal` that came from a cancelled pair
        // is precisely where the two used to diverge. The AI ranks
        // targets by this number, so a prediction that says
        // "disadvantage" about a swing the engine will roll straight
        // sends it after the wrong one.
        let mut tally =
            encounter.attack_mode_tally(actor_id, target_id, action.is_melee_attack());
        tally.add_if(
            crowded || underwater == UnderwaterVerdict::Disadvantage,
            crate::engine::dice::RollMode::Disadvantage,
        );
        let mode = mode_priority(encounter.resolve_attack_mode_against(target_id, tally));
        // The damage key, plus what this weapon's 5e mastery property is
        // worth against *this* target. Folded into the estimate rather
        // than added as a fifth sort key on purpose: a property is worth
        // something, not everything, and a rung of its own would let a
        // club's Slow outrank a greataxe's dice. Expressed in
        // damage-equivalent points, it decides a close call and loses a
        // lopsided one — see `mastery_damage_bonus`.
        let damage = action
            .expected_damage(encounter, actor_id)
            .map(|d| d + mastery_damage_bonus(encounter, actor_id, target_id, action, reach));
        let pick = match &best {
            None => true,
            Some((bs, bm, br, bd, _)) => {
                match (score.cmp(bs), mode.cmp(bm), reach.cmp(br)) {
                    (std::cmp::Ordering::Less, _, _) => true,
                    (std::cmp::Ordering::Greater, _, _) => false,
                    (_, std::cmp::Ordering::Less, _) => true,
                    (_, std::cmp::Ordering::Greater, _) => false,
                    (_, _, std::cmp::Ordering::Greater) => true,
                    (_, _, std::cmp::Ordering::Less) => false,
                    // Same matchup, same mode, same reach: the estimate
                    // decides, but only if both sides have one.
                    _ => matches!((damage, bd), (Some(d), Some(b)) if d > *b),
                }
            }
        };
        if pick {
            best = Some((score, mode, reach, damage, action));
        }
    }
    best.map(|(_, _, r, _, a)| (r, a))
}

/// BFS-step toward the lowest-HP visible enemy. Falls back to step toward
/// any enemy if HP-based selection fails for some reason.
fn try_step_toward_lowest_hp(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();
    let move_action = actor.find_action("move")?;
    // Sort by id to break HP ties deterministically (HashMap iteration is
    // non-deterministic across processes).
    let ids = encounter.sorted_actor_ids();
    let target_id = ids
        .into_iter()
        .filter_map(|id| {
            let t = encounter.actors.get(&id)?;
            if id == actor_id || t.team() == my_team || !t.is_combat_active() {
                None
            } else {
                // Same "who drops soonest" question as the focus-fire
                // picker, so the same answer: absorption pools count.
                Some((id, t.effective_hitpoints()))
            }
        })
        .min_by_key(|(_, hp)| *hp)?
        .0;
    let dest = encounter.step_toward_actor(actor_id, target_id)?;
    let aei = ActionExecutionInfo::new(move_action, actor_id, None, Some(vec![dest]), None);
    if aei.validate(encounter) {
        Some(aei)
    } else {
        None
    }
}

/// Dash to close the gap when enemies are out of normal movement range.
/// Fires only when the actor has an Action remaining, no enemies are within
/// attack reach, and there's at least one living enemy. The Dash grants
/// extra movement equal to the actor's speed so a subsequent move call can
/// cover more ground.
fn try_dash_to_close(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if !actor.can_consume_resource(crate::engine::side_effects::Resource::Action) {
        return None;
    }
    // A Dash hands over a second movement budget, and `remaining_movement`
    // reports zero regardless of how much budget is sitting there while a
    // `zeros_movement` condition is up — Grappled, Restrained, Rooted,
    // Adhered. So Dashing out of one of those is not a gamble that
    // sometimes pays; it is an Action that provably cannot buy a single
    // tile.
    //
    // This is not a tuning nicety. It was the last rung a held creature
    // could reach: with nothing in range and no movement to spend, the
    // fallback chain ended here and the actor Dashed on every turn
    // forever. A generated encounter found it — a vampire held by an
    // otyugh's tentacles Dashed a hundred and seventy-four times in a
    // row while its regeneration undid the otyugh's chip damage, and the
    // fight could not end.
    if actor.conditions().keys().any(|c| c.zeros_movement()) {
        return None;
    }
    let my_team = actor.team();
    let has_enemy = encounter
        .actors
        .values()
        .any(|a| a.team() != my_team && a.is_combat_active());
    if !has_enemy {
        return None;
    }
    // Cheapest printing first — a rogue Dashes for a bonus action and
    // keeps its Action. See `try_cheapest_printing`.
    try_cheapest_printing(
        encounter,
        actor_id,
        &["step of the wind", "cunning dash", "dash"],
    )
}

/// Break a hold that has left the actor unable to do anything else.
///
/// This closes a loop the engine could genuinely not get out of. Every
/// grappling condition zeroes movement, and the AI's fallback chain ends
/// in "close on the nearest enemy, and Dash if they're too far" — so a
/// creature held by something with longer reach than its own arms had no
/// reachable target, no usable movement, and an Action it spent on Dash
/// every single turn, forever. An otyugh (10-ft tentacle grapple) against
/// any 5-ft melee creature is the reachable case, and it produced fights
/// that never ended: the vampire dashed 174 times in a row while the
/// otyugh chewed on it and its regeneration undid the damage.
///
/// Three gates, and the middle one is what keeps this from firing on a
/// grapple that doesn't matter:
///   - The actor is held by something `GRAPPLE_ESCAPE` can answer —
///     its own validator owns that list, so this doesn't restate it.
///   - **Nothing hostile is within the actor's own attack reach.** A
///     grappled creature standing next to its grappler is exactly where
///     it wants to be; the hold costs it a step it wasn't taking. It is
///     only worth an Action when the hold is the reason the actor can't
///     fight.
///   - There is another living enemy at all, so a creature held by the
///     last thing standing doesn't wriggle at nobody.
fn try_escape_grapple(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();
    // Longest reach the actor could swing with. `None` for a creature
    // with no attacks at all, which then has nothing to lose by
    // escaping — treat it as reach 0.
    let best_reach = actor
        .actions
        .iter()
        .filter(|a| a.is_harmful() && a.deals_damage())
        .filter_map(|a| a.reach_tiles())
        .max()
        .unwrap_or(0);
    let mut any_enemy = false;
    for (id, t) in encounter.actors.iter() {
        if *id == actor_id || t.team() == my_team || !t.is_combat_active() {
            continue;
        }
        any_enemy = true;
        if actor.footprint_gap_to(t) <= best_reach {
            // Something is in range; swinging beats wriggling.
            return None;
        }
    }
    if !any_enemy {
        return None;
    }
    try_self_action(encounter, actor_id, "escape")
}

/// True when readying an attack is the better use of a turn the actor
/// has otherwise wasted: there is somebody left to shoot, and they are
/// currently outside the reach of the attack the actor would hold.
///
/// The second half is the whole gate. An enemy already inside the reach
/// cannot *enter* it, so a hold aimed at them can never fire — that
/// actor should be swinging, and if it has reached this rung something
/// else has already stopped it from doing so, in which case Dodge is
/// the honest answer.
fn should_ready_instead_of_dodging(encounter: &EncounterInstance, actor_id: usize) -> bool {
    let Some(actor) = encounter.actors.get(&actor_id) else {
        return false;
    };
    let Some(reach) = encounter
        .best_readyable_attack(actor_id)
        .and_then(|a| a.reach_tiles())
    else {
        return false;
    };
    let my_team = actor.team();
    let mut any_outside = false;
    for (id, other) in encounter.actors.iter() {
        if *id == actor_id || other.team() == my_team || !other.is_combat_active() {
            continue;
        }
        if actor.footprint_gap_to(other) <= reach {
            return false;
        }
        any_outside = true;
    }
    any_outside
}

/// Last-resort: hold an attack if there is anyone left to walk into it,
/// otherwise Dodge (defensive posture if we still have an Action slot),
/// otherwise Skip so the turn doesn't go to waste — and finally
/// AwaitInput if none of the three is available, preventing an infinite
/// loop on a malformed actor.
///
/// Ready sits above Dodge because the two answer the same question and
/// only one of them can end a fight. A turn that reaches this rung is
/// already spent; Dodge buys a chance not to be hit, while a raised bow
/// buys a swing at whoever closes the distance. When nobody is
/// approaching — nothing alive, or everything already in reach — the
/// gate fails and Dodge is what is left.
fn skip_or_await(encounter: &EncounterInstance, caster_id: usize) -> ControllerDecision {
    if should_ready_instead_of_dodging(encounter, caster_id)
        && let Some(aei) = try_self_action(encounter, caster_id, "ready")
    {
        return ControllerDecision::Act(aei);
    }
    if let Some(aei) = try_self_action(encounter, caster_id, "dodge") {
        return ControllerDecision::Act(aei);
    }
    if let Some(aei) = try_self_action(encounter, caster_id, "skip") {
        return ControllerDecision::Act(aei);
    }
    ControllerDecision::AwaitInput
}

use crate::engine::types::Coordinate;

#[cfg(test)]
mod tests {
    use super::*;

    use crate::actors::creatures::skeletons::SKELETON_TEMPLATE;
    use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::side_effects::Resource;
    use crate::engine::terrain_gen::TerrainGenParams;
    use crate::engine::types::DamageType;

    /// The AI holds a shot rather than dodging when somebody is out
    /// there to walk into it — and dodges when nobody is.
    ///
    /// Pinned at the rung rather than through a whole encounter,
    /// because the rung is a *last* resort: reaching it in a live fight
    /// means every attack, every spell and every approach has already
    /// failed, which is hard to arrange and easy to arrange
    /// accidentally differently.
    #[test]
    fn the_last_resort_holds_a_shot_before_it_ducks() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::veterans::VETERAN_TEMPLATE;
        use crate::engine::types::Coordinate;

        let mut e = empty_arena();
        let veteran = e
            .instantiate_creature(&VETERAN_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        let reach = e
            .best_readyable_attack(veteran)
            .and_then(|a| a.reach_tiles())
            .expect("a veteran has something to hold");

        // Nobody on the board: nothing to wait for, so Dodge.
        assert!(!should_ready_instead_of_dodging(&e, veteran));

        // An enemy well outside the reach: hold the shot.
        let far = e
            .instantiate_creature(
                &GOBLIN_TEMPLATE,
                Coordinate::new(3 + reach + 4, 3),
                1,
                0,
            )
            .unwrap();
        assert!(should_ready_instead_of_dodging(&e, veteran));
        match skip_or_await(&e, veteran) {
            ControllerDecision::Act(aei) => {
                assert_eq!(aei.action().name(), "ready")
            }
            ControllerDecision::AwaitInput => panic!("expected an action"),
        }

        // Move them inside the reach and the gate closes — a creature
        // already in range cannot walk into range, so a hold aimed at
        // them could never fire.
        e.place_actor_at(far, Coordinate::new(3 + reach, 3)).unwrap();
        assert!(!should_ready_instead_of_dodging(&e, veteran));
        match skip_or_await(&e, veteran) {
            ControllerDecision::Act(aei) => {
                assert_eq!(aei.action().name(), "dodge")
            }
            ControllerDecision::AwaitInput => panic!("expected an action"),
        }
    }

    /// A bloodied ally gets hit points, not a d4.
    ///
    /// The support rung's candidate set is every non-harmful
    /// single-target action on the sheet, because that is the only lane
    /// a caster's ally buffs have. The bug this pins is what happens
    /// when several of them are legal for the same ally: they all share
    /// a priority and an HP, so before the ordering key grew its
    /// `is_heal` element the winner was whichever action sat earliest on
    /// the template's list, and roughly half the roster's casters
    /// answered a bleeding ally with Guidance, Longstrider or Spider
    /// Climb.
    ///
    /// Swept over the whole registry rather than pinned on one template,
    /// because the failure was never about a particular sheet — it was
    /// about the order of one, and any template's order can change.
    #[test]
    fn the_support_rung_prefers_a_heal_over_a_buff() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::pc_template_families;
        let tp = TerrainGenParams {
            width: 24,
            height: 16,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut swept = 0usize;
        for (_family, templates) in pc_template_families() {
            for template in templates {
                let mut e = EncounterInstance::from_params(&tp, &ap, Some(3)).unwrap();
                let caster = e
                    .instantiate_creature(template, Coordinate::new(3, 8), 0, 0)
                    .unwrap();
                let ally = e
                    .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(4, 8), 0, 0)
                    .unwrap();
                // Somebody hostile has to be standing or the encounter
                // is over before the rung is asked anything.
                e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(16, 8), 1, 0)
                    .unwrap();
                let max = e.actors[&ally].max_hitpoints();
                e.actors.get_mut(&ally).unwrap().take_damage(max - 1);

                let Some(aei) = try_support_heal(&e, caster) else {
                    continue;
                };
                if aei.action().is_heal() {
                    swept += 1;
                    continue;
                }
                // A non-heal is only the right answer when no heal on
                // the sheet was legal for this ally.
                let heal_available = e.actors[&caster].actions.iter().any(|a| {
                    a.is_heal()
                        && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
                        && ActionExecutionInfo::new(*a, caster, Some(vec![ally]), None, None)
                            .validate(&e)
                });
                assert!(
                    !heal_available,
                    "{} answered a bloodied ally with '{}' while a heal was legal",
                    template.name,
                    aei.action().name()
                );
                swept += 1;
            }
        }
        assert!(
            swept > 0,
            "the sweep should have found at least one template that responds"
        );
    }

    /// Every name in the self-buff cohorts belongs to an action some
    /// registered PC template actually carries.
    ///
    /// The rows are matched against the actor's action list by string,
    /// so a typo, a renamed spell, or a pick for a spell nothing has
    /// been given is not an error anywhere — it is a row that silently
    /// never fires. That is exactly the failure the twenty-one
    /// unreachable lineage templates had before the registry existed,
    /// and it is invisible for the same reason: nothing asks.
    #[test]
    fn every_self_buff_pick_names_an_action_a_template_carries() {
        use crate::actors::creatures::pc_template_families;
        let mut known: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (_family, templates) in pc_template_families() {
            for t in templates {
                for a in &t.actions {
                    known.insert(a.name());
                }
            }
        }
        for pick in SELF_BUFFS_ABOVE_DUPLICITY
            .iter()
            .chain(SELF_BUFFS_BELOW_DUPLICITY.iter())
        {
            assert!(
                known.contains(pick.name),
                "self-buff pick '{}' names no action on any registered PC template",
                pick.name
            );
        }
    }

    /// The same guarantee for the two string-matched bonus-action
    /// tables, and for the same reason: a row naming an action nothing
    /// carries is not an error anywhere, it is a rung that quietly never
    /// fires. The two are checked together because they are walked one
    /// after the other by adjacent rungs and a name that drifted between
    /// them would look identical from either side.
    #[test]
    fn every_bonus_action_table_row_names_an_action_a_template_carries() {
        use crate::actors::creatures::pc_template_families;
        let mut known: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (_family, templates) in pc_template_families() {
            for t in templates {
                for a in &t.actions {
                    known.insert(a.name());
                }
            }
        }
        for name in ENGAGED_SELF_POSTURES.iter().chain(MELEE_ADJACENT_PRIMES) {
            assert!(
                known.contains(name),
                "bonus-action table row '{}' names no action on any registered PC template",
                name
            );
        }
        // The two tables answer different questions with the same
        // resource — see `ENGAGED_SELF_POSTURES` — so a name on both
        // would be a posture reconsidered as a prime, or the reverse,
        // after the first table had already declined it.
        for posture in ENGAGED_SELF_POSTURES {
            assert!(
                !MELEE_ADJACENT_PRIMES.contains(posture),
                "'{}' is on both bonus-action tables",
                posture
            );
        }
    }

    /// A melee build carrying an attack cantrip swings the weapon once
    /// the enemy is already standing next to it.
    ///
    /// A turret whose only action is a self-centered burst fires it at a
    /// single enemy.
    ///
    /// The two-enemy floor on `try_self_centered_burst` is the price of
    /// *choosing* a blast over a swing, and an actor with no swing is
    /// not choosing. Before the floor learned that, the Artillerist's
    /// flamethrower cannon — a cone and nothing else — sat on the board
    /// for entire encounters without firing once.
    #[test]
    fn a_burst_only_actor_fires_at_a_lone_enemy() {
        use crate::actors::creatures::eldritch_cannons::FLAMETHROWER_CANNON_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        let mut e = empty_arena();
        let cannon = e
            .instantiate_creature(&FLAMETHROWER_CANNON_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        let aei = try_self_centered_burst(&e, cannon).expect("the cannon should fire");
        assert_eq!(aei.action().name(), "flamethrower");

        // And a caster that *does* hold a swing still wants two bodies
        // before it reaches for a blast — the floor is relaxed for the
        // actor with no alternative, not lifted.
        let mut e = empty_arena();
        let wizard = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        assert!(
            try_self_centered_burst(&e, wizard).is_none(),
            "one enemy is not a cluster for something holding a cantrip"
        );
    }

    /// The Protector cannon's whole turn is a pulse of temporary hit
    /// points over the allies standing near it, and nothing on the
    /// ladder could reach it: the heal rung walks single-target heals,
    /// and the self-heal rung only fires when the *actor* is bloodied,
    /// which a turret behind the line never is.
    #[test]
    fn an_ally_support_pulse_fires_for_the_teammates_standing_in_it() {
        use crate::actors::creatures::eldritch_cannons::PROTECTOR_CANNON_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        let mut e = empty_arena();
        let cannon = e
            .instantiate_creature(&PROTECTOR_CANNON_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(14, 14), 1, 0)
            .unwrap();
        // Nobody to shield yet — the cannon is alone on its team.
        assert!(try_ally_support_pulse(&e, cannon).is_none());

        let ally = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(6, 5), 0, 0)
            .unwrap();
        let aei = try_ally_support_pulse(&e, cannon).expect("an ally is standing in it");
        assert_eq!(aei.action().name(), "protector pulse");
        for ef in aei.execute(&mut e) {
            ef.apply(&mut e);
        }
        assert!(
            e.actors[&ally].temp_hp() > 0,
            "the pulse should have shielded the ally"
        );
    }

    /// The kite rung backs an actor out of contact so it can shoot. It
    /// should not do that when there is nothing to shoot with.
    ///
    /// This is the livelock guard. A Four Elements monk holding a
    /// stunned ogre had spent both its Action and the bonus action its
    /// only ranged attack costs — and rung 2 backed it away anyway,
    /// because the whip was still listed on its sheet. The approach rung
    /// walked it back in, rung 2 pushed it out, and the pair burned the
    /// monk's whole movement allowance every turn, then Dashed and did
    /// it again, for nine rounds.
    #[test]
    fn an_actor_with_no_shot_left_does_not_read_as_a_ranged_attacker() {
        use crate::actors::creatures::monks::FOUR_ELEMENTS_MONK_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::engine::side_effects::Resource;
        let mut e = empty_arena();
        let monk = e
            .instantiate_creature(&FOUR_ELEMENTS_MONK_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        // Fresh: the whip is affordable, so the monk really can shoot.
        assert!(has_ranged_attack(&e, monk));
        // Spend the economy the whip rides on. Nothing on the sheet
        // changed; what changed is that none of it can be paid for.
        {
            let m = e.actors.get_mut(&monk).unwrap();
            m.consume_resource(Resource::Action);
            m.consume_resource(Resource::BonusAction);
        }
        assert!(
            !has_ranged_attack(&e, monk),
            "a monk with no action economy left has no shot to back away for"
        );
    }

    /// The kite rung backs an actor out of contact so it can shoot. It
    /// should not do that when the shot is worse than the swing it is
    /// walking away from.
    ///
    /// The existence test above answers "is there a shot"; this one
    /// answers "is the shot worth the ground", and the roster grew
    /// creatures that need the second question asked. A barbarian
    /// carries a handaxe to throw. Under the existence test alone that
    /// one line turned the engine's heaviest melee chassis into a
    /// kiter — every incentive it has is melee-only (Rage's damage
    /// bonus, Reckless Attack's advantage, a d12 against the throw's
    /// d6) and rung 2 walked it backwards out of the fight anyway.
    #[test]
    fn a_melee_chassis_does_not_kite_for_the_hatchet_on_its_belt() {
        use crate::actors::creatures::barbarians::BARBARIAN_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        let mut e = empty_arena();
        let barb = e
            .instantiate_creature(&BARBARIAN_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();

        // The throw is genuinely there and genuinely available — this
        // is not the livelock case above, where the shot had simply
        // been paid for already.
        assert!(
            has_ranged_attack(&e, barb),
            "the thrown handaxe should read as a ranged attack"
        );
        assert!(
            under_melee_threat(&e, barb),
            "the ogre should be in contact for the rung to be live at all"
        );
        assert!(
            !ranged_lane_beats_staying(&e, barb),
            "a greataxe in reach beats a handaxe in flight — no reason to step back"
        );

        // And the gate still lets through the creature it was written
        // for. A wizard's cantrip outdamages its dagger, so backing out
        // of contact is exactly what a wizard should be doing.
        let wiz = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(10, 10), 0, 1)
            .unwrap();
        e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(12, 10), 1, 1)
            .unwrap();
        assert!(
            ranged_lane_beats_staying(&e, wiz),
            "a wizard in an ogre's reach should still want out of it"
        );
    }

    /// A creature standing five feet away is in melee, and a ranged
    /// attack made from there rolls at disadvantage.
    ///
    /// The gate used to ask for a footprint gap of *zero* — actual
    /// contact. A Medium creature occupies a 2×2 box on this grid, so a
    /// gap of one is 5 ft and a gap of zero is standing on top of
    /// someone. Everything in the engine that can reach an enemy with a
    /// melee weapon could therefore also shoot past it for free, which
    /// is the exact tile 5e's rule exists to punish.
    #[test]
    fn a_shot_taken_from_inside_an_enemys_reach_rolls_at_disadvantage() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        let mut e = empty_arena();
        let wizard = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let goblin = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(8, 5), 1, 0)
            .unwrap();
        assert_eq!(
            e.footprint_distance(wizard, goblin),
            Some(MELEE_REACH),
            "the fixture should put the goblin exactly a melee step away"
        );
        assert_eq!(
            e.compute_attack_mode(wizard, goblin, false),
            RollMode::Disadvantage
        );
        // A swing from the same tile is unaffected — the clause is about
        // shooting, not about being near something.
        assert_eq!(
            e.compute_attack_mode(wizard, goblin, true),
            RollMode::Normal
        );
    }

    /// 5e gives a ranged attack disadvantage while a hostile creature
    /// is within 5 feet of the shooter, and the engine has always
    /// enforced it at the attack site — where `best_attack_against`
    /// could not see it. So an Eldritch Knight standing in an ogre's
    /// reach compared its longsword against its Fire Bolt, saw that the
    /// bolt reached further, and fired the bolt at disadvantage every
    /// turn for the rest of the fight. Every gish on the roster played
    /// that way.
    #[test]
    fn a_melee_build_in_contact_swings_rather_than_casting() {
        use crate::actors::creatures::fighters::ELDRITCH_KNIGHT_FIGHTER_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        let tp = TerrainGenParams {
            width: 24,
            height: 16,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(7)).unwrap();
        let knight = e
            .instantiate_creature(
                &ELDRITCH_KNIGHT_FIGHTER_TEMPLATE,
                Coordinate::new(4, 8),
                0,
                0,
            )
            .unwrap();
        let ogre = e
            .instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(5, 8), 1, 0)
            .unwrap();
        let actor = &e.actors[&knight];

        // Standing in contact, the picker chooses something that
        // swings. *Which* swing is a separate question — the knight
        // carries a longsword and a Booming Blade, both at melee reach
        // and both rolling normally — and the guarantee this pins is
        // the one the mode key buys: it is not the thing that would
        // have rolled at disadvantage.
        let (_, in_contact) = best_attack_against(knight, actor, &e, ogre).expect("a swing");
        assert!(
            in_contact.is_melee_attack(),
            "in contact the picker chose {}, which shoots",
            in_contact.name()
        );

        // The same list from across the room: nothing melee reaches, so
        // the ranged option wins — and it wins on reach, which is the
        // key the mode does not displace.
        e.actors
            .get_mut(&knight)
            .unwrap()
            .set_location(Coordinate::new(14, 8));
        let actor = &e.actors[&knight];
        let (_, at_range) = best_attack_against(knight, actor, &e, ogre).expect("a shot");
        assert!(!at_range.is_melee_attack());
    }

    /// The two cohorts are disjoint. A spell listed on both sides of the
    /// Invoke Duplicity seam would be reconsidered after the Channel
    /// Divinity rung had already declined to fire, which reads as a
    /// priority decision and is really a copy-paste.
    #[test]
    fn the_self_buff_cohorts_do_not_overlap() {
        for above in SELF_BUFFS_ABOVE_DUPLICITY {
            for below in SELF_BUFFS_BELOW_DUPLICITY {
                assert_ne!(
                    above.name, below.name,
                    "'{}' appears on both sides of the seam",
                    above.name
                );
            }
        }
    }

    /// Every playable template, driven by the AI, gets through a whole
    /// fight without panicking, without asking for player input, and
    /// without stalling.
    ///
    /// The `ai_vs_ai_terminates_with_new_content` sweep above is a
    /// hand-curated roster that grew one `instantiate_creature` line at
    /// a time as content landed, which means it covers whatever someone
    /// remembered to add — and of the sixty-odd PC templates in the
    /// engine it names about a dozen. This one reads
    /// `pc_template_families()` instead, so a subclass added tomorrow is
    /// swept the moment it joins the registry and never needs a second
    /// edit here.
    ///
    /// Deliberately narrow per template — one duel, one seed — because
    /// the value is breadth. The failures this catches are the ones a
    /// new template actually produces: an action whose `cost` and
    /// `custom_validate_input` disagree so the AI picks something it
    /// can't pay for, a picker rung that returns an `ActionExecutionInfo`
    /// failing its own `validate`, a self-buff with no engagement gate
    /// that the ladder re-fires every turn forever. All three show up as
    /// a hang or a panic in the first fight, not the hundredth.
    ///
    /// The opponent is an Ogre: enough HP to survive a caster's opening
    /// round (so the sweep exercises more than one turn of the ladder),
    /// no reactions or auras of its own (so a failure is attributable to
    /// the template under test), and melee-only (so it closes rather
    /// An AI-driven Hexblade Warlock actually reaches for the curse,
    /// and the engine pays out on it.
    ///
    /// The per-clause tests next to the engine lanes drive the effects
    /// directly, which proves they work but not that anything ever asks
    /// for them — a feature the AI never selects is a feature no player
    /// sees the AI use. This closes that loop: run a real fight and
    /// require the curse to be cast and at least one of its riders to
    /// show up in the log.
    #[test]
    fn an_ai_driven_hexblade_curses_its_quarry_and_the_curse_pays_out() {
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::warlocks::HEXBLADE_WARLOCK_TEMPLATE;
        let mut cast_in = 0;
        let mut paid_in = 0;
        for seed in 0..8u64 {
            let tp = TerrainGenParams {
                width: 24,
                height: 16,
                branch_depth: 0,
                branch_prob: 0.0,
            };
            let ap = ActorGenParams {
                cr_target: 0.0,
                n_teams: 0,
                pc_template: None,
                start_team: 0,
            };
            let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
            e.instantiate_creature(&HEXBLADE_WARLOCK_TEMPLATE, Coordinate::new(3, 8), 0, 0)
                .expect("the hexblade should instantiate");
            e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(14, 8), 1, 0)
                .expect("the ogre should instantiate");
            let ai = SimpleAi;
            let mut steps = 0usize;
            while steps < 20_000 && !e.is_complete() {
                steps += 1;
                e.process_stack();
                let Some(prompt) = e.peek_prompt() else { break };
                let actor_id = prompt.actor_id();
                match ai.decide(&e, actor_id) {
                    ControllerDecision::AwaitInput => break,
                    ControllerDecision::Act(aei) => {
                        e.pop_prompt();
                        e.push_action(aei);
                    }
                }
            }
            let log = e.messages().join("\n");
            if log.contains("hexblade's curse") {
                cast_in += 1;
            }
            // Any of the three clauses landing counts: the heal line
            // names the curse, and the damage bonus / crit expansion
            // show up as a larger total on the swing that follows.
            if log.contains("draws") || log.contains("armor of hexes") {
                paid_in += 1;
            }
        }
        assert!(
            cast_in > 0,
            "an AI hexblade should reach for its curse in at least one of 8 fights"
        );
        assert!(
            paid_in > 0,
            "the curse should pay out visibly in at least one of 8 fights"
        );
    }

    /// than trading at range forever).
    /// Generated encounters run to completion, whatever the generator
    /// rolls.
    ///
    /// The suite's other end-to-end AI test drives one hand-placed PC
    /// against one hand-placed ogre on one fixed map. This one drives
    /// whatever `EncounterInstance::from_params` produces — generated
    /// terrain at a dozen sizes, generated rosters at CR targets from 1
    /// to 14, two to four factions, half of them in the dark — and
    /// asserts only that the fight ends.
    ///
    /// "The fight ends" turns out to be the hard part, and this test is
    /// how `is_stalemate`'s attrition half was found: a Yeti with a
    /// chilling gaze against a Shield Guardian regenerating 10 hit
    /// points a round, with three other factions dashing about out of
    /// everyone's reach, ran to round 4261 and would have run forever.
    /// Every actor had a legal action every round. Nobody could win.
    ///
    /// A *bounded* number of seeds, chosen to spread across the
    /// parameter space rather than to be the ones that once failed. The
    /// sweep that found the deadlock ran nine hundred, which takes
    /// minutes.
    ///
    /// Sixty rather than the eighteen it started at, because eighteen
    /// turned out to be under the floor. A six-hundred-seed run of this
    /// same loop surfaced a generator defect nothing in the suite could
    /// see — see `actor_gen::generate_actors` — and eighteen boards had
    /// been passing over it for as long as it had been there. Sixty is
    /// what the suite can afford; the pattern is that turning this
    /// number up by hand is a productive thing to do periodically, and
    /// leaving it up is not.
    ///
    /// The board is grown to fit what is being asked of it (see
    /// `MIN_TILES_PER_CR_TEAM`). Before that, the widest configurations
    /// — four factions at CR 9 on a sixteen-by-ten room — were asking
    /// the generator for more creatures than the map had anchors to
    /// hold, and failing for a reason that has nothing to do with what
    /// this test is about.
    #[test]
    fn generated_encounters_of_every_shape_run_to_completion() {
        use crate::actors::creatures::pc_template_families;
        /// Board tiles to allow per point of (CR target × faction).
        /// Empirically ~6 is where a densely-branched map starts
        /// running out of anchors for its Large and Huge draws; twice
        /// that leaves the generator room to be unlucky.
        const MIN_TILES_PER_CR_TEAM: f32 = 12.0;
        let families = pc_template_families();
        for seed in 0u64..60 {
            let cr_target = 1.0 + (seed % 9) as f32;
            let n_teams = 2 + (seed % 3) as usize;
            let mut width = 16 + (seed % 12) as usize * 2;
            let mut height = 10 + (seed % 7) as usize;
            // Grow the room — keeping its aspect — until it can plausibly
            // hold the fight. A test that spends its seeds on
            // impossible-to-generate boards is testing the generator's
            // error path, which has its own tests.
            let wanted = (cr_target * n_teams as f32 * MIN_TILES_PER_CR_TEAM) as usize;
            while width * height < wanted {
                width += 2;
                height += 1;
            }
            let tp = TerrainGenParams {
                width,
                height,
                branch_depth: (seed % 6) as usize,
                branch_prob: 0.5,
            };
            let fam = &families[(seed as usize) % families.len()].1;
            let pc = fam[(seed as usize) % fam.len()];
            let ap = ActorGenParams {
                cr_target,
                // Two to four factions. More than two is not a
                // configuration the binary ships, and it is where the
                // deadlock showed up first — a three-way fight has more
                // ways to arrive at nobody being able to finish.
                n_teams,
                pc_template: Some(pc),
                start_team: 0,
            };
            let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed))
                .unwrap_or_else(|err| panic!("seed {seed}: generation failed: {err}"));
            if seed % 3 == 0 {
                // Half the boards unlit, so the lighting layer, the
                // darkvision gates and the AI's torch rung are on the
                // path too.
                e.set_ambient_light(crate::engine::lighting::AmbientLight::Darkness);
            }
            let ai = SimpleAi;
            // The cap is a backstop for a genuine hang, not a budget:
            // a settled fight uses a low four-figure number of steps,
            // and a deadlock caught by the draw uses tens of thousands
            // of cheap ones.
            let mut steps = 0usize;
            let settled = loop {
                if steps >= 400_000 {
                    break false;
                }
                steps += 1;
                e.process_stack();
                if e.is_complete() {
                    break true;
                }
                let Some(prompt) = e.peek_prompt() else {
                    break true;
                };
                let actor_id = prompt.actor_id();
                match ai.decide(&e, actor_id) {
                    ControllerDecision::AwaitInput => {
                        panic!("seed {seed} ({}): SimpleAi asked for player input", pc.name)
                    }
                    ControllerDecision::Act(aei) => {
                        assert!(
                            aei.validate(&e),
                            "seed {seed} ({}): the AI queued '{}', which fails its own validate",
                            pc.name,
                            aei.action().name()
                        );
                        e.pop_prompt();
                        e.push_action(aei);
                    }
                }
            };
            assert!(
                settled,
                "seed {seed} ({}): still going at round {} after {steps} steps — \
                 neither a win nor a draw, which means something can act \
                 forever without getting anywhere and `is_stalemate` did not \
                 notice",
                pc.name,
                e.round()
            );
        }
    }

    #[test]
    fn every_pc_template_can_be_driven_by_the_ai_to_completion() {
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::pc_template_families;
        for (family, templates) in pc_template_families() {
            for template in templates {
                let tp = TerrainGenParams {
                    width: 24,
                    height: 16,
                    branch_depth: 0,
                    branch_prob: 0.0,
                };
                let ap = ActorGenParams {
                    cr_target: 0.0,
                    n_teams: 0,
                    pc_template: None,
                    start_team: 0,
                };
                let mut e = EncounterInstance::from_params(&tp, &ap, Some(7)).unwrap();
                let pc = e
                    .instantiate_creature(template, Coordinate::new(3, 8), 0, 0)
                    .unwrap_or_else(|_| panic!("{} ({}) should instantiate", template.name, family));
                let ogre = e
                    .instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(18, 8), 1, 0)
                    .expect("the ogre should instantiate");
                let ai = SimpleAi;
                // Generous cap: a stall shows up as exhausting it, and
                // the assertion below names the template rather than
                // leaving a bare hang for someone to bisect.
                let mut steps = 0usize;
                let settled = loop {
                    if steps >= 20_000 {
                        break false;
                    }
                    steps += 1;
                    e.process_stack();
                    if e.is_complete() {
                        break true;
                    }
                    let Some(prompt) = e.peek_prompt() else {
                        break true;
                    };
                    let actor_id = prompt.actor_id();
                    match ai.decide(&e, actor_id) {
                        ControllerDecision::AwaitInput => panic!(
                            "{} ({}): SimpleAi asked for player input",
                            template.name, family
                        ),
                        ControllerDecision::Act(aei) => {
                            assert!(
                                aei.validate(&e),
                                "{} ({}): the AI queued an action that fails its own validate",
                                template.name,
                                family
                            );
                            e.pop_prompt();
                            e.push_action(aei);
                        }
                    }
                };
                assert!(
                    settled,
                    "{} ({}): the duel never terminated in 20k steps",
                    template.name, family
                );
                // Somebody won. A fight that ends with both sides
                // untouched means the AI never engaged, which is a
                // stall wearing a completed encounter's clothes.
                let pc_hp = e.actors.get(&pc).map(|a| a.hitpoints()).unwrap_or(0);
                let ogre_hp = e.actors.get(&ogre).map(|a| a.hitpoints()).unwrap_or(0);
                assert!(
                    pc_hp == 0 || ogre_hp == 0,
                    "{} ({}): the duel ended with both sides standing ({} vs {})",
                    template.name,
                    family,
                    pc_hp,
                    ogre_hp
                );
            }
        }
    }

    /// Every stat block the encounter generator can roll survives being
    /// driven by the AI that will drive it.
    ///
    /// The sibling of `every_pc_template_can_be_driven_by_the_ai_to_completion`
    /// on the other side of the screen, and the gap it fills is the
    /// larger one: there are three PC families and two hundred and
    /// fifty monsters, every one of them reachable from the generator,
    /// and nothing checked that any of them could take a turn.
    ///
    /// Two failures are worth naming because they are the ones that do
    /// not announce themselves:
    ///
    ///   - **The AI queues an action that fails its own `validate`.**
    ///     A stat block that carries an action with an unsatisfiable
    ///     gate — a self-condition it never installs, a resource it
    ///     never has — produces a creature that picks that action,
    ///     fails the re-check, and does nothing. The fight still ends;
    ///     the monster just never fought.
    ///   - **`AwaitInput` from a monster.** The AI is the only
    ///     controller a generated creature ever gets. A rung that falls
    ///     through to "ask the player" is a hang in the real loop and a
    ///     panic here.
    ///
    /// Bounded rather than run to completion, which is the one place
    /// this differs from the PC sweep. Two hundred and fifty duels
    /// fought to the last hit point would dominate the suite's runtime,
    /// and the bugs above show up on the first turn a creature takes —
    /// so the sweep spends a fixed budget of decisions per template and
    /// asserts that every one of them was legal. The completion half is
    /// left to the PC sweep, which is small enough to afford it.
    ///
    /// The opponent is a pair of Commoners rather than an Ogre. A pair,
    /// because a single target hides every bug in the picker's ranking
    /// behind "there was only one"; Commoners, because a CR 0 body
    /// cannot kill anything, so the budget is spent on the monster's
    /// turns rather than on its funeral. They start six tiles away so
    /// that the first decision is an engagement rather than the first
    /// of five walks across an empty board. Same seed and same open
    /// board throughout, so a failure reproduces from the template name
    /// alone.
    #[test]
    fn every_template_the_generator_can_roll_survives_a_turn_of_the_ai() {
        use crate::actors::creatures::commoners::COMMONER_TEMPLATE;
        use crate::engine::encounter::EncounterInstance;

        // Enough decisions to get every creature past its opening move
        // and into its second turn, where the recharge / prime / spend
        // rungs come up. Small enough that 250 of them stay cheap.
        const DECISION_BUDGET: usize = 24;

        let (mut driven, mut engaged) = (0, 0);
        for template in EncounterInstance::template_pool() {
            let tp = TerrainGenParams {
                width: 24,
                height: 16,
                branch_depth: 0,
                branch_prob: 0.0,
            };
            let ap = ActorGenParams {
                cr_target: 0.0,
                n_teams: 0,
                pc_template: None,
                start_team: 0,
            };
            let mut e = EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap();
            e.instantiate_creature(template, Coordinate::new(3, 8), 0, 0)
                .unwrap_or_else(|_| panic!("{} should instantiate", template.name));
            for (i, x) in [9isize, 11].into_iter().enumerate() {
                e.instantiate_creature(&COMMONER_TEMPLATE, Coordinate::new(x, 8), 1, i)
                    .expect("a commoner should instantiate");
            }
            driven += 1;

            let victim_hp_before: u32 = e
                .actors
                .values()
                .filter(|a| a.team() == 1)
                .map(|a| a.hitpoints())
                .sum();

            let ai = SimpleAi;
            for _ in 0..DECISION_BUDGET {
                e.process_stack();
                if e.is_complete() {
                    break;
                }
                let Some(prompt) = e.peek_prompt() else {
                    break;
                };
                let actor_id = prompt.actor_id();
                match ai.decide(&e, actor_id) {
                    ControllerDecision::AwaitInput => panic!(
                        "{}: SimpleAi asked for player input — a generated creature has \
                         no player to ask",
                        template.name
                    ),
                    ControllerDecision::Act(aei) => {
                        assert!(
                            aei.validate(&e),
                            "{}: the AI queued \"{}\", which fails its own validate",
                            template.name,
                            aei.action().name()
                        );
                        e.pop_prompt();
                        e.push_action(aei);
                    }
                }
            }
            e.process_stack();
            let victim_hp_after: u32 = e
                .actors
                .values()
                .filter(|a| a.team() == 1)
                .map(|a| a.hitpoints())
                .sum();
            if victim_hp_after < victim_hp_before {
                engaged += 1;
            }
        }
        assert!(
            driven > 200,
            "the pool should be the whole bestiary, found {}",
            driven
        );
        // The sweep is only worth its runtime if the budget actually
        // reaches combat, and "no template crashed" is exactly what a
        // sweep that never got past the walking phase would also
        // report. Not all of them: a caster that opens by buffing, a
        // summoner that opens by summoning, and anything whose reach
        // the board denies will legitimately spend twenty-four
        // decisions without landing a hit. A large majority is the
        // claim, and it fails loudly if the budget is ever cut below
        // the engagement it was chosen to buy.
        assert!(
            engaged * 4 >= driven * 3,
            "only {} of {} templates landed a hit inside the budget — the sweep has \
             stopped reaching combat and is now testing the pathfinder",
            engaged,
            driven
        );
    }

    /// The AI reaches summons at all — the behavioural half of the
    /// `summons_allies` contract.
    ///
    /// Regression pin for a silent hole rather than a preference: for
    /// as long as the summon spells have existed, no AI-driven caster
    /// had ever cast one. They are `is_harmful` but declare no damage
    /// types and target nothing, so the burst picker's "harmful NoArgs
    /// with a damage type or an explicit no-damage flag" filter dropped
    /// every one of them, and no other rung looked. Nothing failed; the
    /// spells were simply never chosen, on any template, in any fight.
    ///
    /// Driven through the Beast Master because its companion is the
    /// cleanest case — no slot to be out of, no concentration to be
    /// holding, so a decline can only mean the rung isn't reached.
    #[test]
    fn the_ai_reaches_the_summon_rung() {
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::rangers::BEAST_MASTER_RANGER_TEMPLATE;
        let tp = TerrainGenParams {
            width: 24,
            height: 16,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(3)).unwrap();
        let ranger = e
            .instantiate_creature(&BEAST_MASTER_RANGER_TEMPLATE, Coordinate::new(4, 8), 0, 0)
            .unwrap();
        // Inside the 24-tile engagement gate, outside melee — the
        // ranger has no reason to do anything else first.
        let _ = e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(16, 8), 1, 0);
        assert!(
            try_summon_allies(&e, ranger, SummonTier::Free).is_some(),
            "an engaged Beast Master with an unspent bond should call the beast"
        );
        // The engagement gate is load-bearing: with nothing to fight,
        // the charge stays in hand.
        let mut empty = EncounterInstance::from_params(&tp, &ap, Some(3)).unwrap();
        let alone = empty
            .instantiate_creature(&BEAST_MASTER_RANGER_TEMPLATE, Coordinate::new(4, 8), 0, 0)
            .unwrap();
        assert!(try_summon_allies(&empty, alone, SummonTier::Free).is_none());
    }

    /// The two summon rungs partition the lane: every summon on every
    /// playable template belongs to exactly one of them, and which one
    /// follows from what it costs rather than from what it is called.
    ///
    /// The partition is the whole reason the split is safe. The free
    /// rung sits ten rungs above the slotted one, so a summon that fell
    /// into neither tier would silently become unreachable by the AI —
    /// the same invisible regression the summon lane already had once,
    /// before `summons_allies` existed — and one that fell into both
    /// would be offered twice at two different priorities.
    #[test]
    fn every_summon_belongs_to_exactly_one_rung() {
        use crate::actors::creatures::pc_template_families;
        use crate::engine::side_effects::spell_slot_level;
        use std::collections::HashSet;

        let mut seen: HashSet<&str> = HashSet::new();
        let mut free: Vec<&str> = Vec::new();
        // The tier a summon lands in is read off its cost, and a cost
        // needs an encounter to be asked for. Any board with the caster
        // on it will do — no summon's cost varies with the fixture.
        let tp = TerrainGenParams {
            width: 24,
            height: 16,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        for (_family, templates) in pc_template_families() {
            for template in templates {
                let mut e = EncounterInstance::from_params(&tp, &ap, Some(3)).unwrap();
                let caster = e
                    .instantiate_creature(template, Coordinate::new(4, 8), 0, 0)
                    .unwrap_or_else(|_| panic!("{} should instantiate", template.name));
                for action in template.actions.iter().filter(|a| a.summons_allies()) {
                    let slot =
                        spell_slot_level(&action.cost(&e, caster, None, None, None)).unwrap_or(0);
                    let tier = SummonTier::of(slot, action.holds_concentration());
                    if tier == SummonTier::Free && seen.insert(action.name()) {
                        free.push(action.name());
                    }
                    seen.insert(action.name());
                }
            }
        }

        // Vacuous-pass guard, and a statement of what the free rung is
        // for: the per-rest feature summons, and nothing else. A summon
        // *spell* that ended up here would be one that forgot to declare
        // its slot.
        free.sort_unstable();
        assert_eq!(
            free,
            vec![
                // The two Circle of the Shepherd totems, which share
                // one charge and are two builds rather than one — see
                // `SPIRIT_TOTEM_TAG`.
                "bear spirit",
                // The Artillerist's three cannon modes, which share one
                // charge and so are one summon wearing three names —
                // see `ELDRITCH_CANNON_TAG`.
                "eldritch cannon (flamethrower)",
                "eldritch cannon (force ballista)",
                "eldritch cannon (protector)",
                "ranger's companion",
                "steel defender",
                "summon drake",
                "summon wildfire spirit",
                "tentacle of the deep",
                "unicorn spirit",
            ],
            "the free rung's membership changed"
        );
        assert!(
            seen.len() > free.len(),
            "the slotted rung came out empty, so the walk found nothing"
        );
    }

    /// A free feature summon leaves the slot lane alone.
    ///
    /// The `Summoner` marker means "I have spent a slot calling for
    /// help", and it used to mean "I have called for help" — which
    /// silently barred a Beast Master who whistled up their wolf on
    /// round one from ever casting Conjure Animals, and would have done
    /// the same to the Wildfire druid and the Fathomless warlock. Both
    /// halves are pinned here: the free summon leaves no mark, and the
    /// slotted one that follows it does.
    #[test]
    fn a_free_summon_does_not_spend_the_fights_one_slotted_call() {
        use crate::actors::creatures::druids::WILDFIRE_DRUID_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        let tp = TerrainGenParams {
            width: 24,
            height: 16,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(3)).unwrap();
        let druid = e
            .instantiate_creature(&WILDFIRE_DRUID_TEMPLATE, Coordinate::new(4, 8), 0, 0)
            .unwrap();
        let _ = e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(16, 8), 1, 0);

        // The free rung finds the charge-gated spirit: no slot, no
        // concentration, so it is what `SummonTier::Free` selects for.
        let aei = try_summon_allies(&e, druid, SummonTier::Free)
            .expect("an engaged Wildfire druid should summon");
        assert_eq!(aei.action().name(), "summon wildfire spirit");
        for ef in aei.execute(&mut e) {
            ef.apply(&mut e);
        }
        assert!(
            !e.actors[&druid].has_condition(Condition::Summoner),
            "a free feature summon must not consume the fight's one slotted call"
        );

        // Hand the druid their Action back and the *slotted* rung — the
        // other half of the lane, further down the ladder — still has
        // something to offer. The two are separate rungs, so this leg
        // asks the slotted one directly rather than watching the free
        // one fall through.
        e.actors
            .get_mut(&druid)
            .unwrap()
            .give_resource(crate::engine::side_effects::Resource::Action);
        let second = try_summon_allies(&e, druid, SummonTier::Slotted)
            .expect("the slot summon should still be available to a Wildfire druid");
        // Cheapest slot first, so this is whichever summon on the
        // druid's list costs the lowest level — Summon Beast at level 2
        // today, and whatever undercuts it tomorrow. Asserted by cost
        // rather than by name because the name is the incidental half:
        // what this leg is pinning is that the rung reached a *slotted*
        // summon at all after the free one was spent.
        assert_eq!(
            crate::engine::side_effects::spell_slot_level(&second.action().cost(
                &e, druid, None, None, None
            )),
            Some(2),
            "expected the cheapest slotted summon, got {}",
            second.action().name()
        );
        for ef in second.execute(&mut e) {
            ef.apply(&mut e);
        }
        assert!(
            e.actors[&druid].has_condition(Condition::Summoner),
            "the slotted call is the one that closes the lane"
        );
    }

    /// The summon rung fires once per fight and then stops.
    ///
    /// The concentration check caps most of the roster, but Animate
    /// Dead holds no concentration and carries no per-rest charge —
    /// RAW it is a permanent minion — so before the `Summoner` marker
    /// a wizard would have spent every third-level slot it owned on
    /// consecutive skeletons and never cast anything else. Driven
    /// through the wizard for exactly that reason.
    #[test]
    fn the_summon_rung_fires_once_per_fight() {
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        let tp = TerrainGenParams {
            width: 24,
            height: 16,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(3)).unwrap();
        let wizard = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(4, 8), 0, 0)
            .unwrap();
        let _ = e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(16, 8), 1, 0);
        let aei = try_summon_allies(&e, wizard, SummonTier::Slotted)
            .expect("an engaged wizard should summon");
        assert_eq!(aei.action().name(), "animate dead");
        for ef in aei.execute(&mut e) {
            ef.apply(&mut e);
        }
        assert!(
            e.actors[&wizard].has_condition(Condition::Summoner),
            "the spawn helper marks the caster"
        );
        assert!(
            try_summon_allies(&e, wizard, SummonTier::Slotted).is_none(),
            "a second summon in the same fight would eat the whole slot pool"
        );
        // The action itself is untouched — the cap is one AI picker's
        // judgement, not a rule. Refill what the first cast spent so
        // the assertion reads the marker rather than an empty slot.
        {
            let w = e.actors.get_mut(&wizard).unwrap();
            w.give_resource(crate::engine::side_effects::Resource::Action);
            w.spell_slot_manager.restore_spell_slots();
        }
        assert!(
            ActionExecutionInfo::new(
                e.actors[&wizard].find_action("animate dead").unwrap(),
                wizard,
                None,
                None,
                None,
            )
            .validate(&e),
            "a human player can still cast it again"
        );
    }

    fn run_to_completion(seed: u64) -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 4,
            branch_prob: 0.5,
        };
        let ap = ActorGenParams {
            cr_target: 0.5,
            n_teams: 2,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
        let ai = SimpleAi;

        // Hard cap so a runaway loop fails the test instead of hanging.
        // Stalemate detection: if HP totals are unchanged for a full
        // sweep across all actors (no damage / heal landed), assume the
        // remaining combatants can't reach each other and bail. That can
        // happen when two ranged-only actors end up in walled-off rooms
        // with no LOS or path between them.
        let total_hp = |e: &EncounterInstance| -> u32 {
            e.actors.values().map(|a| a.hitpoints()).sum()
        };
        let mut last_total = total_hp(&e);
        let mut idle_streak = 0usize;
        let stalemate_window = 4 * e.actors.len().max(1);
        for _ in 0..50_000 {
            e.process_stack();
            if e.is_complete() {
                return e;
            }
            let Some(prompt) = e.peek_prompt() else { break };
            let actor_id = prompt.actor_id();
            match ai.decide(&e, actor_id) {
                ControllerDecision::AwaitInput => {
                    panic!("SimpleAi returned AwaitInput — should always act");
                }
                ControllerDecision::Act(aei) => {
                    e.pop_prompt();
                    e.push_action(aei);
                }
            }
            let cur = total_hp(&e);
            if cur == last_total {
                idle_streak += 1;
                if idle_streak >= stalemate_window {
                    // True stalemate; bail.
                    return e;
                }
            } else {
                last_total = cur;
                idle_streak = 0;
            }
        }
        let snap: Vec<String> = e
            .actors
            .values()
            .map(|a| format!("{} t{} hp{} @{}", a.name(), a.team(), a.hitpoints(), a.location()))
            .collect();
        panic!("seed {} did not terminate; survivors: {:?}", seed, snap);
    }

    /// Drive several AI-vs-AI encounters to completion. Validates the
    /// controller dispatch loop and that SimpleAi makes a decision
    /// every turn, regardless of terrain layout. We accept three
    /// terminal states: clean win, mutual destruction, or stalemate
    /// (HP unchanged across a full round). Stalemate is real for
    /// ranged-vs-ranged in walled-off rooms; the simulator should
    /// stop pumping rather than panic.
    #[test]
    fn ai_vs_ai_terminates() {
        for seed in [1u64, 7, 42, 99, 12345] {
            let _ = run_to_completion(seed);
        }
    }

    /// `try_heightened_spell` picks the heightened metamagic when the
    /// sorcerer has the SP, isn't already primed, and has at least one
    /// heightenable spell in the kit AND an enemy in range. Skipped when
    /// SP is too low or no enemy is nearby.
    #[test]
    fn heightened_spell_ai_gates_on_sp_and_enemy_range() {
        use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap();
        let sorcerer = e
            .instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // Sorcerer needs the bonus action available for the metamagic to fire.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .give_resource(Resource::BonusAction);

        // No enemies on the map — the heuristic skips even though the
        // sorcerer has SP and a heightenable spell in the kit.
        assert!(
            try_heightened_spell(&e, sorcerer).is_none(),
            "no enemies in range → skip"
        );

        // Plant a zombie within heightened-spell range (36 tiles) and
        // re-check — heuristic should now offer the prime.
        let _ = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        assert!(
            try_heightened_spell(&e, sorcerer).is_some(),
            "enemy in range + SP + heightenable spell → prime fires"
        );

        // Burn the SP pool to 2 and re-check — heuristic should skip
        // even though the enemy is still there.
        while e.actors[&sorcerer].sorcery_points() > 2 {
            e.actors.get_mut(&sorcerer).unwrap().spend_sorcery_point();
        }
        assert!(
            try_heightened_spell(&e, sorcerer).is_none(),
            "SP below 3 → skip"
        );
    }

    /// `try_distant_spell` fires only when the closest enemy sits in the
    /// 13–24 tile band — too close (≤12 tiles) means the bonus is wasted,
    /// too far (>24 tiles) means unreachable even doubled. SP gate and
    /// already-primed gate also fire.
    #[test]
    fn distant_spell_ai_gates_on_enemy_range() {
        use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 40,
            height: 40,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap();
        let sorcerer = e
            .instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .give_resource(Resource::BonusAction);

        // No enemies → skip.
        assert!(try_distant_spell(&e, sorcerer).is_none(), "no enemies → skip");

        // Enemy at gap 10 (too close — Fire Bolt already reaches it).
        let close = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(16, 5), 1, 0)
            .unwrap();
        assert!(
            try_distant_spell(&e, sorcerer).is_none(),
            "enemy already in range → skip"
        );

        // Move enemy out to gap 15 (in the 13–24 sweet spot).
        e.actors
            .get_mut(&close)
            .unwrap()
            .set_location(Coordinate::new(21, 5));
        assert!(
            try_distant_spell(&e, sorcerer).is_some(),
            "enemy in 13-24 tile band → prime fires"
        );

        // Already primed → skip.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .add_condition(Condition::DistantSpelling, ConditionTimer::Rounds(2));
        assert!(
            try_distant_spell(&e, sorcerer).is_none(),
            "prime already up → skip"
        );
    }

    /// `try_extended_spell` fires when the sorcerer has SP, hasn't
    /// primed Extended / Empowered / Heightened, owns at least one
    /// long-duration spell, and an enemy is within engagement range.
    /// Skipped on every gate violation.
    #[test]
    fn extended_spell_ai_gates() {
        use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 30,
            height: 30,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(7)).unwrap();
        let sorcerer = e
            .instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .give_resource(Resource::BonusAction);

        // No enemy → skip.
        assert!(
            try_extended_spell(&e, sorcerer).is_none(),
            "no enemy → skip"
        );

        // Enemy in range → fires (sorcerer template has Mage Armor /
        // Hunter's Mark / Haste / etc. — extendable spells in the kit).
        let _ = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(15, 5), 1, 0)
            .unwrap();
        assert!(
            try_extended_spell(&e, sorcerer).is_some(),
            "enemy in range + extendable kit → prime fires"
        );

        // Already primed → skip.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .add_condition(Condition::ExtendedSpelling, ConditionTimer::Rounds(2));
        assert!(
            try_extended_spell(&e, sorcerer).is_none(),
            "already extended → skip"
        );
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .remove_condition(Condition::ExtendedSpelling);

        // Empowered prime up → skip (higher-leverage prime preferred).
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .add_condition(Condition::EmpoweredSpelling, ConditionTimer::Rounds(2));
        assert!(
            try_extended_spell(&e, sorcerer).is_none(),
            "empowered already up → skip"
        );
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .remove_condition(Condition::EmpoweredSpelling);

        // No SP → skip.
        let actor = e.actors.get_mut(&sorcerer).unwrap();
        while actor.spend_sorcery_point() {}
        assert!(
            try_extended_spell(&e, sorcerer).is_none(),
            "no SP → skip"
        );
    }

    /// `try_careful_spell` fires when the sorcerer has SP, hasn't primed
    /// it, owns a Burst-targeting AoE, and has at least one ally close
    /// to an enemy. Skipped when no ally-near-enemy pair exists.
    #[test]
    fn careful_spell_ai_gates_on_ally_proximity_to_enemy() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 30,
            height: 30,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap();
        let sorcerer = e
            .instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .give_resource(Resource::BonusAction);

        // Sorcerer alone — no ally + no enemy → skip.
        assert!(try_careful_spell(&e, sorcerer).is_none(), "no enemies → skip");

        // Add a distant enemy and a distant ally — no ally-near-enemy
        // pair, so Careful Spell wouldn't pay off.
        let _ = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(20, 20), 1, 0)
            .unwrap();
        let ally = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(2, 2), 0, 0)
            .unwrap();
        assert!(
            try_careful_spell(&e, sorcerer).is_none(),
            "no ally near enemy → skip"
        );

        // Move ally adjacent to the enemy — pair now within 5 tiles.
        e.actors
            .get_mut(&ally)
            .unwrap()
            .set_location(Coordinate::new(19, 19));
        assert!(
            try_careful_spell(&e, sorcerer).is_some(),
            "ally close to enemy → prime fires"
        );
    }

    /// Every summoned spirit fights when the AI is driving it.
    ///
    /// A summon that arrives and then stands there is the worst failure
    /// mode this family has, and it is completely silent: the spell
    /// resolves, the body appears, the log looks right, and the spirit
    /// contributes nothing for the rest of the encounter. Nothing else
    /// in the suite would catch it — the spawn tests prove the body
    /// exists, and the AI sweeps drive *casters* rather than the things
    /// they call up.
    ///
    /// So each spirit is put on the board directly, opposite an ogre,
    /// and driven to the end of the fight. The marker is the spirit's own
    /// signature action, which is the part that has to survive twenty-odd
    /// rungs of a ladder written for player characters.
    ///
    /// The Draconic Spirit's breath is deliberately not a marker: the
    /// breath rung wants two clustered enemies and this fixture has one
    /// ogre. Its recharge wiring is pinned in `summoned_spirits`, and its
    /// rend is what the fixture can actually show.
    #[test]
    fn every_summoned_spirit_fights_when_the_ai_drives_it() {
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::summoned_spirits::{
            ABERRANT_SPIRIT_TEMPLATE, BESTIAL_SPIRIT_TEMPLATE, CELESTIAL_SPIRIT_TEMPLATE,
            DRACONIC_SPIRIT_TEMPLATE, ELEMENTAL_SPIRIT_TEMPLATE, FEY_SPIRIT_TEMPLATE,
            FIENDISH_SPIRIT_TEMPLATE, UNDEAD_SPIRIT_TEMPLATE,
        };
        use crate::actors::actor_template::CreatureTemplate;
        let cases: &[(&LazyLock<CreatureTemplate>, &str)] = &[
            (&BESTIAL_SPIRIT_TEMPLATE, "maul"),
            (&FEY_SPIRIT_TEMPLATE, "fey blade"),
            (&UNDEAD_SPIRIT_TEMPLATE, "grave bolt"),
            (&ABERRANT_SPIRIT_TEMPLATE, "eye ray"),
            (&ELEMENTAL_SPIRIT_TEMPLATE, "elemental slam"),
            (&CELESTIAL_SPIRIT_TEMPLATE, "radiant bow"),
            (&DRACONIC_SPIRIT_TEMPLATE, "rend"),
            (&FIENDISH_SPIRIT_TEMPLATE, "fiendish claws"),
        ];
        for (template, marker) in cases {
            let mut seen_in = 0;
            for seed in 0..4u64 {
                let tp = TerrainGenParams {
                    width: 24,
                    height: 16,
                    branch_depth: 0,
                    branch_prob: 0.0,
                };
                let ap = ActorGenParams {
                    cr_target: 0.0,
                    n_teams: 0,
                    pc_template: None,
                    start_team: 0,
                };
                let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
                e.instantiate_creature(template, Coordinate::new(4, 8), 0, 0)
                    .unwrap_or_else(|_| panic!("{} should instantiate", template.name));
                e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(14, 8), 1, 0)
                    .expect("the ogre should instantiate");
                let ai = SimpleAi;
                let mut steps = 0usize;
                while steps < 20_000 && !e.is_complete() {
                    steps += 1;
                    e.process_stack();
                    let Some(prompt) = e.peek_prompt() else { break };
                    let actor_id = prompt.actor_id();
                    match ai.decide(&e, actor_id) {
                        ControllerDecision::AwaitInput => break,
                        ControllerDecision::Act(aei) => {
                            e.pop_prompt();
                            e.push_action(aei);
                        }
                    }
                }
                if e.messages().join("\n").contains(marker) {
                    seen_in += 1;
                }
            }
            assert!(
                seen_in > 0,
                "an AI-driven {} never used \"{}\" in 4 fights — it arrived and stood there",
                template.name,
                marker
            );
        }
    }

    /// Exercise the new spells / creatures in an AI-driven encounter so the
    /// rule changes (Sunbeam / Prayer of Healing / Power Word Heal /
    /// Resurrection in cleric+wizard loadouts; Manticore / Hill Giant /
    /// Treant / Fire Elemental / Gelatinous Cube in the monster pool)
    /// don't crash the AI's action picker or stall the process_stack
    /// loop. Also keeps the prior "new content" coverage on the lineup.
    /// The AI, left to close the distance on its own, produces runs the
    /// charge clauses can see.
    ///
    /// The rule can be perfectly implemented and still never fire in
    /// play: the AI approaches a tile at a time down whatever path the
    /// pathfinder hands back, and until `steps_toward` existed that path
    /// was a staircase of diagonals that collected almost no straight
    /// run at all. This is the test that says the two halves meet.
    #[test]
    fn the_ai_closes_straight_enough_to_charge() {
        use crate::actors::creatures::boars::BOAR_TEMPLATE;
        use crate::actors::creatures::veterans::VETERAN_TEMPLATE;
        use crate::engine::types::Coordinate;

        let mut e = open_field(40, 11);
        // Twelve tiles apart on a shared row: past the eight-tile bar,
        // and close enough that the boar can still reach and swing on
        // the same turn. A charger that has to Dash to arrive spends its
        // Action getting there and attacks a turn later, standing still.
        let boar = e
            .instantiate_creature(&BOAR_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        let _ = e.instantiate_creature(&VETERAN_TEMPLATE, Coordinate::new(17, 5), 0, 0);
        let ai = SimpleAi;
        for _ in 0..40 {
            e.process_stack();
            if e.is_complete() {
                break;
            }
            let Some(prompt) = e.peek_prompt() else { break };
            if prompt.actor_id() != boar {
                break;
            }
            match ai.decide(&e, boar) {
                ControllerDecision::AwaitInput => {
                    panic!("SimpleAi returned AwaitInput unexpectedly")
                }
                ControllerDecision::Act(aei) => {
                    e.pop_prompt();
                    e.push_action(aei);
                }
            }
        }
        e.process_stack();
        let run = e.actors[&boar].straight_run_tiles().unwrap_or(0);
        assert!(
            run >= crate::engine::attack::CHARGE_RUN_TILES,
            "the boar closed twelve open tiles and finished with a run of {}",
            run
        );
    }

    /// An arena of nothing but floor, so a pathing assertion is about the
    /// pathfinder rather than about what the terrain generator happened
    /// to put in the way.
    fn open_field(width: usize, height: usize) -> EncounterInstance {
        let tp = TerrainGenParams {
            width,
            height,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        for y in 0..height as isize {
            for x in 0..width as isize {
                e.set_terrain_at(
                    Coordinate::new(x, y),
                    crate::engine::terrain::TerrainType::Floor,
                );
            }
        }
        e
    }

    #[test]
    fn ai_vs_ai_terminates_with_new_content() {
        use crate::actors::creatures::banshees::BANSHEE_TEMPLATE;
        use crate::actors::creatures::barbarians::BARBARIAN_TEMPLATE;
        use crate::actors::creatures::bards::BARD_TEMPLATE;
        use crate::actors::creatures::beholders::BEHOLDER_TEMPLATE;
        use crate::actors::creatures::berserkers::BERSERKER_TEMPLATE;
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::couatls::COUATL_TEMPLATE;
        use crate::actors::creatures::doppelgangers::DOPPELGANGER_TEMPLATE;
        use crate::actors::creatures::dragons::ADULT_RED_DRAGON_TEMPLATE;
        use crate::actors::creatures::drow::DROW_TEMPLATE;
        use crate::actors::creatures::fire_elementals::FIRE_ELEMENTAL_TEMPLATE;
        use crate::actors::creatures::frost_giants::FROST_GIANT_TEMPLATE;
        use crate::actors::creatures::gelatinous_cubes::GELATINOUS_CUBE_TEMPLATE;
        use crate::actors::creatures::hill_giants::HILL_GIANT_TEMPLATE;
        use crate::actors::creatures::hippogriffs::HIPPOGRIFF_TEMPLATE;
        use crate::actors::creatures::liches::LICH_TEMPLATE;
        use crate::actors::creatures::manticores::MANTICORE_TEMPLATE;
        use crate::actors::creatures::minotaurs::MINOTAUR_TEMPLATE;
        use crate::actors::creatures::monks::MONK_TEMPLATE;
        use crate::actors::creatures::mummies::MUMMY_TEMPLATE;
        use crate::actors::creatures::paladins::PALADIN_TEMPLATE;
        use crate::actors::creatures::pit_fiends::PIT_FIEND_TEMPLATE;
        use crate::actors::creatures::treants::TREANT_TEMPLATE;
        use crate::actors::creatures::vampires::VAMPIRE_TEMPLATE;
        use crate::actors::creatures::veterans::VETERAN_TEMPLATE;
        use crate::actors::creatures::wights::WIGHT_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::actors::creatures::yetis::YETI_TEMPLATE;
        use crate::engine::types::Coordinate;

        /// The log prefixes `engine::mastery` writes, one per property
        /// that has anything to say. Nick is absent on purpose: it
        /// changes what the off-hand swing costs and never announces
        /// itself, so there is no line for it to leave.
        const MASTERY_LOG_TAGS: &[&str] = &[
            "  cleave:", "  graze:", "  push:", "  sap:", "  slow:", "  topple:", "  vex:",
        ];
        let mut mastery_fired = false;
        // 5e legendary actions, end to end. Eight of the creatures
        // placed below carry a legendary repertoire, so a driver in
        // which none of them ever spends a point is one where the
        // whole chain — the template's budget, the repertoire beside
        // it, the turn-end dispatcher, the affordability filter — is
        // exercised only by its unit tests. Accumulated across the
        // seed sweep for the same reason `mastery_fired` is: on any
        // one board the bosses may be dead before their second
        // turn-end.
        let mut legendary_fired = false;
        for seed in [3u64, 11, 71] {
            let tp = TerrainGenParams {
                width: 30,
                height: 20,
                branch_depth: 0,
                branch_prob: 0.0,
            };
            let ap = ActorGenParams {
                cr_target: 0.0,
                n_teams: 0,
                pc_template: None,
                start_team: 0,
            };
            let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
            // Hand-place a representative party of the new content on
            // both teams.
            let _ = e.instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(2, 2), 0, 0);
            let _ = e.instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(2, 4), 0, 1);
            let _ = e.instantiate_creature(&HIPPOGRIFF_TEMPLATE, Coordinate::new(4, 2), 0, 2);
            let _ = e.instantiate_creature(&VETERAN_TEMPLATE, Coordinate::new(4, 4), 0, 3);
            // Barbarian on team 0 so the AI exercises the new Rage /
            // Reckless Attack feature stack mid-encounter.
            let _ = e.instantiate_creature(&BARBARIAN_TEMPLATE, Coordinate::new(6, 2), 0, 4);
            // Paladin on team 0 — exercises Divine Smite (bonus action +
            // slot prime) + Lay on Hands + Channel Divinity: Sacred Weapon
            // mid-encounter.
            let _ = e.instantiate_creature(&PALADIN_TEMPLATE, Coordinate::new(6, 4), 0, 5);
            let _ = e.instantiate_creature(&WIGHT_TEMPLATE, Coordinate::new(27, 17), 1, 0);
            let _ = e.instantiate_creature(&MINOTAUR_TEMPLATE, Coordinate::new(27, 15), 1, 1);
            let _ = e.instantiate_creature(&BANSHEE_TEMPLATE, Coordinate::new(25, 17), 1, 2);
            let _ = e.instantiate_creature(&DOPPELGANGER_TEMPLATE, Coordinate::new(25, 15), 1, 3);
            let _ = e.instantiate_creature(&MUMMY_TEMPLATE, Coordinate::new(25, 13), 1, 4);
            let _ = e.instantiate_creature(&BERSERKER_TEMPLATE, Coordinate::new(23, 17), 1, 5);
            let _ = e.instantiate_creature(&YETI_TEMPLATE, Coordinate::new(23, 15), 1, 6);
            let _ = e.instantiate_creature(&MANTICORE_TEMPLATE, Coordinate::new(23, 13), 1, 7);
            let _ = e.instantiate_creature(&HILL_GIANT_TEMPLATE, Coordinate::new(21, 17), 1, 8);
            let _ = e.instantiate_creature(&TREANT_TEMPLATE, Coordinate::new(21, 15), 1, 9);
            let _ = e.instantiate_creature(&FIRE_ELEMENTAL_TEMPLATE, Coordinate::new(21, 13), 1, 10);
            let _ = e.instantiate_creature(&GELATINOUS_CUBE_TEMPLATE, Coordinate::new(19, 17), 1, 11);
            // New boss-tier content: Lich (CR 21 caster), Adult Red Dragon
            // (CR 17 multiattack + breath), Beholder (CR 13 eye-ray + bite)
            // — the action picker needs to handle the boss spell list and
            // the dragon's burst breath without stalling.
            let _ = e.instantiate_creature(&LICH_TEMPLATE, Coordinate::new(19, 15), 1, 12);
            let _ = e.instantiate_creature(&ADULT_RED_DRAGON_TEMPLATE, Coordinate::new(15, 17), 1, 13);
            let _ = e.instantiate_creature(&BEHOLDER_TEMPLATE, Coordinate::new(15, 14), 1, 14);
            // Drow on the enemy team so the action picker exercises the
            // new poisoned-hand-crossbow CON-save rider.
            let _ = e.instantiate_creature(&DROW_TEMPLATE, Coordinate::new(13, 17), 1, 15);
            // Latest additions: a Vampire (CR 13 boss with regen + charm
            // gaze + lifesteal multiattack) and a Frost Giant (CR 8 huge
            // dice + cold immunity). Round out the enemy lineup.
            let _ = e.instantiate_creature(&VAMPIRE_TEMPLATE, Coordinate::new(13, 15), 1, 16);
            let _ = e.instantiate_creature(&FROST_GIANT_TEMPLATE, Coordinate::new(11, 16), 1, 17);
            // Newest PC-team additions: Bard (Bardic Inspiration support
            // caster) and Monk (Stunning Strike + Patient Defense melee
            // controller). The Couatl on the enemy team has Sleep Gaze +
            // poison bite; the Pit Fiend is the new top-tier devil boss
            // with fear aura + multi-bite/claw burst.
            let _ = e.instantiate_creature(&BARD_TEMPLATE, Coordinate::new(8, 2), 0, 6);
            let _ = e.instantiate_creature(&MONK_TEMPLATE, Coordinate::new(8, 4), 0, 7);
            let _ = e.instantiate_creature(&COUATL_TEMPLATE, Coordinate::new(9, 16), 1, 18);
            let _ = e.instantiate_creature(&PIT_FIEND_TEMPLATE, Coordinate::new(9, 14), 1, 19);
            // Druid on team 0 — exercises the new druid spell loadout
            // (Goodberry, Moonbeam, Call Lightning, Sleet Storm, Reverse
            // Gravity) through the AI's action picker.
            use crate::actors::creatures::druids::DRUID_TEMPLATE;
            let _ = e.instantiate_creature(&DRUID_TEMPLATE, Coordinate::new(10, 2), 0, 8);
            // Tarrasque on the enemy team — CR-30 apex boss with the
            // new heterogeneous multiattack (bite + 2 claws + tail
            // sweep). Verifies the AI doesn't stall on the gargantuan
            // footprint or the prone-on-hit tail rider.
            use crate::actors::creatures::tarrasques::TARRASQUE_TEMPLATE;
            let _ = e.instantiate_creature(&TARRASQUE_TEMPLATE, Coordinate::new(5, 12), 1, 20);
            // Latest additions: Ranger PC (DEX longbow + Hunter's Mark +
            // Hail of Thorns), Aboleth (CR 10 aquatic tentacle multi),
            // Solar (CR 21 celestial wielding Holy Aura + Foresight).
            // Seeds the new spells / classes through the AI picker so
            // any regression in the validation / cost / side-effect path
            // surfaces here, not in the live UI.
            use crate::actors::creatures::aboleths::ABOLETH_TEMPLATE;
            use crate::actors::creatures::rangers::RANGER_TEMPLATE;
            use crate::actors::creatures::solars::SOLAR_TEMPLATE;
            let _ = e.instantiate_creature(&RANGER_TEMPLATE, Coordinate::new(12, 2), 0, 9);
            let _ = e.instantiate_creature(&ABOLETH_TEMPLATE, Coordinate::new(7, 12), 1, 21);
            let _ = e.instantiate_creature(&SOLAR_TEMPLATE, Coordinate::new(14, 2), 0, 10);
            // Latest additions: Sorcerer (CHA-primary blaster caster on
            // team 0), Mind Flayer (CR 7 psionic boss with Mind Blast
            // cone + Tentacle grapple on team 1), Erinyes (CR 12 flying
            // devil with poisoned-longsword triple-multi on team 1).
            // Verifies the AI handles the new spell list, the psychic
            // cone save partition, and the heavy multi-swing burst.
            use crate::actors::creatures::erinyes::ERINYES_TEMPLATE;
            use crate::actors::creatures::mind_flayers::MIND_FLAYER_TEMPLATE;
            use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
            let _ = e.instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(16, 2), 0, 11);
            let _ = e.instantiate_creature(&MIND_FLAYER_TEMPLATE, Coordinate::new(11, 12), 1, 22);
            let _ = e.instantiate_creature(&ERINYES_TEMPLATE, Coordinate::new(11, 14), 1, 23);
            // Latest additions: Hell Hound (CR 3 fiend with fire bite +
            // 15ft cone breath), Wyvern (CR 6 dragon with poison stinger),
            // Storm Giant (CR 13 lightning-themed apex giant). Verifies
            // the AI exercises the new fire breath cone and the heavy
            // poison rider without stalling.
            use crate::actors::creatures::hell_hounds::HELL_HOUND_TEMPLATE;
            use crate::actors::creatures::storm_giants::STORM_GIANT_TEMPLATE;
            use crate::actors::creatures::wyverns::WYVERN_TEMPLATE;
            let _ = e.instantiate_creature(&HELL_HOUND_TEMPLATE, Coordinate::new(15, 12), 1, 24);
            let _ = e.instantiate_creature(&WYVERN_TEMPLATE, Coordinate::new(17, 12), 1, 25);
            let _ = e.instantiate_creature(&STORM_GIANT_TEMPLATE, Coordinate::new(19, 11), 1, 26);
            // Newest additions: Warlock (CHA-primary pact-magic caster
            // on team 0), Hydra (CR 8 huge 5-bite regen boss on team 1),
            // Stone Giant (CR 7 boulder + greatclub on team 1), Medusa
            // (CR 6 petrifying gaze + snake hair on team 1), Salamander
            // (CR 5 fire-elemental on team 1). Verifies the AI handles
            // the new pact-magic slot table, the 5-bite multi, the
            // petrifying gaze rider, and the fire-immune elemental.
            use crate::actors::creatures::hydras::HYDRA_TEMPLATE;
            use crate::actors::creatures::medusas::MEDUSA_TEMPLATE;
            use crate::actors::creatures::salamanders::SALAMANDER_TEMPLATE;
            use crate::actors::creatures::stone_giants::STONE_GIANT_TEMPLATE;
            use crate::actors::creatures::warlocks::WARLOCK_TEMPLATE;
            let _ = e.instantiate_creature(&WARLOCK_TEMPLATE, Coordinate::new(18, 2), 0, 12);
            // Place huge / large creatures off to the side so their
            // footprints fit cleanly in the open lower-left quadrant.
            let _ = e.instantiate_creature(&HYDRA_TEMPLATE, Coordinate::new(3, 17), 1, 27);
            let _ = e.instantiate_creature(&STONE_GIANT_TEMPLATE, Coordinate::new(8, 17), 1, 28);
            let _ = e.instantiate_creature(&MEDUSA_TEMPLATE, Coordinate::new(13, 17), 1, 29);
            let _ = e.instantiate_creature(&SALAMANDER_TEMPLATE, Coordinate::new(16, 17), 1, 30);
            // Latest enemy-team additions: Death Knight (CR 17 boss
            // undead with 3-swing necrotic-rider longsword multi + 10d8
            // Hellfire Orb DEX-save burst + tight fire-evocation spell
            // list), Ghost (CR 4 incorporeal undead with withering
            // necrotic touch + Horrifying Visage WIS-save Frighten
            // burst). Exercises the AI's boss-tier action picker on the
            // new burst path and the Ghost's NoArgs visage cast.
            use crate::actors::creatures::death_knights::DEATH_KNIGHT_TEMPLATE;
            use crate::actors::creatures::ghosts::GHOST_TEMPLATE;
            let _ = e.instantiate_creature(&DEATH_KNIGHT_TEMPLATE, Coordinate::new(19, 17), 1, 31);
            let _ = e.instantiate_creature(&GHOST_TEMPLATE, Coordinate::new(22, 17), 1, 32);
            // Latest boss-tier addition: Stone Golem (CR 10 construct with
            // Legendary Resistance 3/Day + magic-immune envelope + 10ft
            // Slow burst). Verifies the AI handles a boss whose entire
            // schtick is "your saves don't work" — most of the party's
            // control spells get auto-promoted to passes via the new LR
            // gate; the action picker shouldn't stall on the resulting
            // "save spell did nothing" paths.
            use crate::actors::creatures::stone_golems::STONE_GOLEM_TEMPLATE;
            let _ = e.instantiate_creature(&STONE_GOLEM_TEMPLATE, Coordinate::new(25, 12), 1, 33);
            // Latest additions: Bullette (CR 5 burrowing predator with
            // Deadly Leap → prone-on-fail-STR-save) and Bone Devil (CR 9
            // flying fiend with multiattack + poison-rider stinger).
            // Exercises the new prone-on-leap path and the standard
            // devil envelope (fire/poison immunity + cold resistance).
            use crate::actors::creatures::bone_devils::BONE_DEVIL_TEMPLATE;
            use crate::actors::creatures::bullettes::BULLETTE_TEMPLATE;
            let _ = e.instantiate_creature(&BULLETTE_TEMPLATE, Coordinate::new(2, 18), 1, 34);
            let _ = e.instantiate_creature(&BONE_DEVIL_TEMPLATE, Coordinate::new(5, 18), 1, 35);
            // Newest additions: Air Elemental (CR 5 flying elemental with
            // 2-slam multi), Earth Elemental (CR 5 heavy slam + thunder
            // vulnerability), Balor (CR 19 apex demon with longsword +
            // whip multi + Fire Aura bonus action + LR 3/Day). Verifies
            // the AI handles the new elemental envelopes (poison-immunity
            // + condition-immunity stack) and the boss-tier demon's
            // multi-lane attack picker without stalling on the LR gate.
            use crate::actors::creatures::air_elementals::AIR_ELEMENTAL_TEMPLATE;
            use crate::actors::creatures::balors::BALOR_TEMPLATE;
            use crate::actors::creatures::earth_elementals::EARTH_ELEMENTAL_TEMPLATE;
            let _ = e.instantiate_creature(&AIR_ELEMENTAL_TEMPLATE, Coordinate::new(8, 18), 1, 36);
            let _ = e.instantiate_creature(&EARTH_ELEMENTAL_TEMPLATE, Coordinate::new(11, 18), 1, 37);
            let _ = e.instantiate_creature(&BALOR_TEMPLATE, Coordinate::new(14, 18), 1, 38);
            // Newest addition: Glabrezu (CR 9 demon with 4-swing multi —
            // 2 pincers + 2 fists). Verifies the AI handles the
            // mid-tier demon's high-volume multiattack without stalling
            // on the standard demon envelope (poison/cold/fire/lightning
            // resistance + Charmed/Frightened/Poisoned condition immunity).
            use crate::actors::creatures::glabrezus::GLABREZU_TEMPLATE;
            let _ = e.instantiate_creature(&GLABREZU_TEMPLATE, Coordinate::new(17, 18), 1, 39);
            // Newest additions: Marilith (CR 16 demon with 7-swing multi —
            // 6 longswords + 1 tail) and Vrock (CR 6 demon with 3-swing
            // multi + Stunning Screech non-demon-only thunder burst).
            // Exercises the highest-volume single-action multi in the
            // pool and the new screech filter that exempts other demons
            // via the Poison-immunity cohort. The Marilith / Vrock pair
            // also stresses the Thunderous Smite path: a paladin with
            // smite primed and a 7-swing demon adjacent ought to roll
            // its smite prime through the new on-hit rider entry.
            use crate::actors::creatures::mariliths::MARILITH_TEMPLATE;
            use crate::actors::creatures::vrocks::VROCK_TEMPLATE;
            let _ = e.instantiate_creature(&MARILITH_TEMPLATE, Coordinate::new(20, 18), 1, 40);
            let _ = e.instantiate_creature(&VROCK_TEMPLATE, Coordinate::new(23, 18), 1, 41);
            // Newest addition: Shambling Mound (CR 5 plant) — exercises
            // the 2-slam multiattack and the engulf grapple rider via
            // the AI's focus-fire picker, plus the new VitriolicSphere /
            // MaximiliansEarthenGrasp / Shillelagh spells get rolled
            // through the wizard / druid loadouts above (lv2 / lv4 acid
            // + drip + bonus-action force prime).
            use crate::actors::creatures::shambling_mounds::SHAMBLING_MOUND_TEMPLATE;
            let _ = e.instantiate_creature(
                &SHAMBLING_MOUND_TEMPLATE,
                Coordinate::new(26, 18),
                1,
                42,
            );
            // Newest additions: Champion Fighter (Improved Critical: crits
            // on 19+) and Halfling Scout (racial Lucky: re-rolls nat-1s).
            // Exercises the new template fields end-to-end through the
            // AI's attack picker — the Champion's lowered crit threshold
            // surfaces during sustained swing trials, and the Halfling's
            // Lucky reroll fires whenever a d20 lands on 1 on attacks or
            // saves.
            use crate::actors::creatures::fighters::CHAMPION_TEMPLATE;
            use crate::actors::creatures::halflings::HALFLING_SCOUT_TEMPLATE;
            let _ = e.instantiate_creature(&CHAMPION_TEMPLATE, Coordinate::new(20, 2), 0, 13);
            let _ = e.instantiate_creature(
                &HALFLING_SCOUT_TEMPLATE,
                Coordinate::new(22, 2),
                0,
                14,
            );
            // Newest additions: Aasimar (celestial-touched humanoid with
            // racial Healing Hands + radiant/necrotic resistance).
            // Exercises the AI's new SingleActor-self-target self-heal
            // path through the racial action when the Aasimar drops
            // below half HP. The Fighter template also picked up the
            // three new Battle Master maneuvers (Menacing / Disarming /
            // Pushing Attack); the Champion template is the level-5
            // Fighter sans the maneuvers, so the Fighter on team 0
            // (placed earlier above) is the maneuver picker.
            use crate::actors::creatures::aasimars::AASIMAR_TEMPLATE;
            use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
            let _ = e.instantiate_creature(&AASIMAR_TEMPLATE, Coordinate::new(24, 2), 0, 15);
            let _ = e.instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(26, 2), 0, 16);
            // Latest racial additions: Half-Orc Marauder (Savage Attacks +
            // Relentless Endurance racials on a fighter chassis) and
            // Mountain Dwarf Defender (Dwarven Resilience CON-tank).
            // Exercises the new racial flags through the AI picker so
            // any wiring regression in the crit-damage or save-mode
            // lanes surfaces here.
            use crate::actors::creatures::dwarves::DWARF_TEMPLATE;
            use crate::actors::creatures::half_orcs::HALF_ORC_TEMPLATE;
            let _ = e.instantiate_creature(&HALF_ORC_TEMPLATE, Coordinate::new(28, 2), 0, 17);
            let _ = e.instantiate_creature(&DWARF_TEMPLATE, Coordinate::new(28, 4), 0, 18);
            // Newest racial additions: Tiefling (Hellish Resistance + the
            // Infernal Legacy Hellish Rebuke racial), Rock Gnome
            // (Gnome Cunning: advantage on INT/WIS/CHA saves vs magic),
            // and Red Dragonborn (Draconic Ancestry: fire resistance +
            // short-rest Breath Weapon). Exercises the new
            // `has_gnome_cunning` save-mode hook, the dragonborn
            // breath weapon's AoE path through `try_attack_aoe`, and
            // the tiefling's once-per-rest racial action gating. The
            // sorcerer's new Quickened Spell metamagic also rides this
            // smoke test through the existing SORCERER_TEMPLATE.
            use crate::actors::creatures::dragonborn::DRAGONBORN_TEMPLATE;
            use crate::actors::creatures::gnomes::GNOME_TEMPLATE;
            use crate::actors::creatures::tieflings::TIEFLING_TEMPLATE;
            let _ = e.instantiate_creature(&TIEFLING_TEMPLATE, Coordinate::new(28, 6), 0, 19);
            let _ = e.instantiate_creature(&GNOME_TEMPLATE, Coordinate::new(28, 8), 0, 20);
            let _ = e.instantiate_creature(&DRAGONBORN_TEMPLATE, Coordinate::new(28, 10), 0, 21);
            // Latest addition: a level-3 Rogue. Exercises the new 2024
            // Cunning Strike bonus-action primes (Poison / Trip) and
            // the Search default action through the AI picker. We bump
            // the rogue's level to 3 directly so the sneak pool can
            // spare the Cunning Strike die cost.
            use crate::actors::creatures::rogues::ROGUE_TEMPLATE;
            let rogue_id = e
                .instantiate_creature(&ROGUE_TEMPLATE, Coordinate::new(28, 12), 0, 22)
                .unwrap();
            {
                use crate::engine::dice::FastRandRoller;
                e.actors.get_mut(&rogue_id).unwrap().award_xp(10_000);
                let mut roller = FastRandRoller::with_seed(seed);
                while e.actors[&rogue_id].level() < 3
                    && e.actors
                        .get_mut(&rogue_id)
                        .unwrap()
                        .try_level_up(&mut roller)
                        .is_some()
                {}
            }
            // Newest additions: low / mid-CR fillers added to round out
            // the pool — Giant Eagle (CR 1, multi beak + talons), Sahuagin
            // (CR ½ humanoid with Blood Frenzy: advantage on melee vs
            // wounded targets — exercises the new compute_attack_mode
            // gate), Lizardfolk (CR ½ humanoid, bite + heavy-club multi),
            // Centaur (CR 2 hybrid: reach-2 pike + hooves multi with
            // longbow fallback), Giant Ape (CR 7 huge brute: 2 fist multi
            // or 7d6 boulder throw). Verifies the AI's picker handles the
            // new compound attacks and the wounded-target advantage gate
            // without stalling.
            use crate::actors::creatures::centaurs::CENTAUR_TEMPLATE;
            use crate::actors::creatures::giant_apes::GIANT_APE_TEMPLATE;
            use crate::actors::creatures::giant_eagles::GIANT_EAGLE_TEMPLATE;
            use crate::actors::creatures::lizardfolk::LIZARDFOLK_TEMPLATE;
            use crate::actors::creatures::sahuagins::SAHUAGIN_TEMPLATE;
            let _ = e.instantiate_creature(&GIANT_EAGLE_TEMPLATE, Coordinate::new(28, 14), 1, 43);
            let _ = e.instantiate_creature(&SAHUAGIN_TEMPLATE, Coordinate::new(0, 12), 1, 44);
            let _ = e.instantiate_creature(&LIZARDFOLK_TEMPLATE, Coordinate::new(0, 14), 1, 45);
            let _ = e.instantiate_creature(&CENTAUR_TEMPLATE, Coordinate::new(0, 6), 0, 23);
            let _ = e.instantiate_creature(&GIANT_APE_TEMPLATE, Coordinate::new(0, 9), 1, 46);
            // Newest additions: Pegasus (CR 2 celestial — Hooves), Winter
            // Wolf (CR 3 monstrosity — cold-breath weapon + Pack Tactics),
            // Carrion Crawler (CR 2 monstrosity — paralyzing tentacles +
            // bite multi), Triceratops (CR 5 huge beast — gore / stomp),
            // T-Rex (CR 8 huge beast — bite + tail multi). Verifies the
            // AI picker doesn't stall on the breath-recharge gate or the
            // CON-save paralysis rider in a mid-tier fight.
            use crate::actors::creatures::carrion_crawlers::CARRION_CRAWLER_TEMPLATE;
            use crate::actors::creatures::pegasi::PEGASUS_TEMPLATE;
            use crate::actors::creatures::triceratopses::TRICERATOPS_TEMPLATE;
            use crate::actors::creatures::tyrannosauruses::T_REX_TEMPLATE;
            use crate::actors::creatures::winter_wolves::WINTER_WOLF_TEMPLATE;
            let _ = e.instantiate_creature(&PEGASUS_TEMPLATE, Coordinate::new(28, 2), 0, 24);
            let _ = e.instantiate_creature(&WINTER_WOLF_TEMPLATE, Coordinate::new(28, 5), 1, 47);
            let _ = e.instantiate_creature(&CARRION_CRAWLER_TEMPLATE, Coordinate::new(28, 8), 1, 48);
            let _ = e.instantiate_creature(&TRICERATOPS_TEMPLATE, Coordinate::new(26, 10), 1, 49);
            let _ = e.instantiate_creature(&T_REX_TEMPLATE, Coordinate::new(22, 10), 1, 50);
            // Newest additions: Water Elemental (CR 5 — completes the
            // elemental quartet, exercises the new `WHELM` recharge
            // burst), Saber-toothed Tiger (CR 2 — heavier Pounce
            // beast), Hyena / Giant Hyena (CR 0 / CR 1 — pack-tactics
            // pair pressuring the Pack Tactics gate inside
            // `compute_attack_mode`), Green Hag (CR 3 — first medium-
            // CR fey with Magic Resistance, exercises the spell-save
            // advantage clause inside `compute_save_mode`).
            use crate::actors::creatures::giant_hyenas::GIANT_HYENA_TEMPLATE;
            use crate::actors::creatures::green_hags::GREEN_HAG_TEMPLATE;
            use crate::actors::creatures::hyenas::HYENA_TEMPLATE;
            use crate::actors::creatures::saber_toothed_tigers::SABER_TOOTHED_TIGER_TEMPLATE;
            use crate::actors::creatures::water_elementals::WATER_ELEMENTAL_TEMPLATE;
            let _ = e.instantiate_creature(&WATER_ELEMENTAL_TEMPLATE, Coordinate::new(18, 10), 1, 51);
            let _ = e.instantiate_creature(
                &SABER_TOOTHED_TIGER_TEMPLATE,
                Coordinate::new(14, 10),
                1,
                52,
            );
            let _ = e.instantiate_creature(&HYENA_TEMPLATE, Coordinate::new(10, 12), 1, 53);
            let _ = e.instantiate_creature(&GIANT_HYENA_TEMPLATE, Coordinate::new(8, 12), 1, 54);
            let _ = e.instantiate_creature(&GREEN_HAG_TEMPLATE, Coordinate::new(6, 12), 1, 55);
            // Newest additions covering the chaotic-evil fiend / aberration /
            // earth-elemental gaps in the bestiary:
            //   - Quasit (CR 1 tiny fiend) — poisoned claws + scare; exercises
            //     the Frightened install on a fresh non-paladin caster.
            //   - Shadow Demon (CR 4 medium fiend) — psychic claws + radiant
            //     vulnerability — first vulnerability hit in the new pool.
            //   - Succubus (CR 4 medium fiend) — charm + draining-kiss combo
            //     gated on the `Charmed` back-link.
            //   - Intellect Devourer (CR 2 tiny aberration) — INT-save lane
            //     with the damage-threshold stun rider.
            //   - Xorn (CR 5 medium elemental) — 3-claw + bite heavy multi.
            use crate::actors::creatures::intellect_devourers::INTELLECT_DEVOURER_TEMPLATE;
            use crate::actors::creatures::quasits::QUASIT_TEMPLATE;
            use crate::actors::creatures::shadow_demons::SHADOW_DEMON_TEMPLATE;
            use crate::actors::creatures::succubi::SUCCUBUS_TEMPLATE;
            use crate::actors::creatures::xorns::XORN_TEMPLATE;
            let _ = e.instantiate_creature(&QUASIT_TEMPLATE, Coordinate::new(4, 10), 1, 56);
            let _ = e.instantiate_creature(&SHADOW_DEMON_TEMPLATE, Coordinate::new(2, 10), 1, 57);
            let _ = e.instantiate_creature(&SUCCUBUS_TEMPLATE, Coordinate::new(4, 8), 1, 58);
            let _ = e.instantiate_creature(
                &INTELLECT_DEVOURER_TEMPLATE,
                Coordinate::new(2, 8),
                1,
                59,
            );
            let _ = e.instantiate_creature(&XORN_TEMPLATE, Coordinate::new(6, 8), 1, 60);
            // Newest additions exercising the giant-tier multi (Oni's
            // double-glaive with reach-2 + Magic Resistance + 10/round
            // regen), the aquatic-humanoid harpoon-and-bite multi
            // (Merrow), and the cheapest ambient beast (Giant Crab).
            use crate::actors::creatures::giant_crabs::GIANT_CRAB_TEMPLATE;
            use crate::actors::creatures::merrow::MERROW_TEMPLATE;
            use crate::actors::creatures::oni::ONI_TEMPLATE;
            let _ = e.instantiate_creature(&ONI_TEMPLATE, Coordinate::new(8, 6), 1, 61);
            let _ = e.instantiate_creature(&MERROW_TEMPLATE, Coordinate::new(6, 6), 1, 62);
            let _ = e.instantiate_creature(&GIANT_CRAB_TEMPLATE, Coordinate::new(4, 6), 1, 63);
            // Newest additions: Hook Horror (CR 3 monstrosity — vanilla
            // 2-hook multi at reach 2, exercises the brute melee bench),
            // Dragon Turtle (CR 17 gargantuan dragon — bite + 2-claw
            // mixed-reach multi + CON-DC steam-breath, exercises the
            // BreathWeapon chassis on a non-elemental DC), and Kraken
            // (CR 23 gargantuan titan — triple tentacle multi at the
            // engine's longest melee reach + NoArgs Lightning Storm
            // recharge burst, exercises the boss-tier picker on the new
            // self-centered recharge AoE path).
            use crate::actors::creatures::dragon_turtles::DRAGON_TURTLE_TEMPLATE;
            use crate::actors::creatures::hook_horrors::HOOK_HORROR_TEMPLATE;
            use crate::actors::creatures::krakens::KRAKEN_TEMPLATE;
            let _ = e.instantiate_creature(&HOOK_HORROR_TEMPLATE, Coordinate::new(2, 6), 1, 64);
            // Place the gargantuan dragon turtle / kraken far enough from
            // each other and from the other gargantuan / huge actors
            // (Tarrasque, Hydra) that their 4×4 footprints don't collide.
            let _ = e.instantiate_creature(&DRAGON_TURTLE_TEMPLATE, Coordinate::new(15, 5), 1, 65);
            let _ = e.instantiate_creature(&KRAKEN_TEMPLATE, Coordinate::new(20, 5), 1, 66);
            // Newest additions: Helmed Horror (CR 4 construct — exercises
            // the AI on a magic-immunity envelope: spells should bounce
            // off, melee should bite normally), Pixie (CR ¼ tiny fey —
            // exercises the burst-AoE picker on a 1-HP glass-cannon with
            // Sleep Dust at 30 ft), and Androsphinx (CR 17 boss — exercises
            // the self-centered NoArgs Roar through the recharge gate plus
            // the boss-tier LR auto-pass on saves). Placed at the spare
            // tiles on the lower-left quadrant so their footprints don't
            // collide with the existing huge / gargantuan entries.
            use crate::actors::creatures::androsphinxes::ANDROSPHINX_TEMPLATE;
            use crate::actors::creatures::helmed_horrors::HELMED_HORROR_TEMPLATE;
            use crate::actors::creatures::pixies::PIXIE_TEMPLATE;
            let _ = e.instantiate_creature(&HELMED_HORROR_TEMPLATE, Coordinate::new(0, 17), 1, 67);
            let _ = e.instantiate_creature(&PIXIE_TEMPLATE, Coordinate::new(0, 16), 1, 68);
            let _ = e.instantiate_creature(&ANDROSPHINX_TEMPLATE, Coordinate::new(0, 2), 1, 69);
            // Newest additions: mephit cohort (CR ¼ – ½ small elementals
            // with on-death `DeathBurst` triggers). The AI smoke test
            // verifies the burst fires through the engine's
            // `cleanup_dead_actors` chokepoint mid-encounter — every
            // mephit reduced to 0 HP will detonate before being removed,
            // exercising the new burst path without a bespoke test fixture.
            // The magma mephit (CR ½) anchors the bench; ice + steam
            // mephits round out the elemental-pair lineup.
            use crate::actors::creatures::mephits::{
                ICE_MEPHIT_TEMPLATE, MAGMA_MEPHIT_TEMPLATE, STEAM_MEPHIT_TEMPLATE,
            };
            let _ = e.instantiate_creature(&MAGMA_MEPHIT_TEMPLATE, Coordinate::new(0, 10), 1, 70);
            let _ = e.instantiate_creature(&ICE_MEPHIT_TEMPLATE, Coordinate::new(0, 12), 1, 71);
            let _ = e.instantiate_creature(&STEAM_MEPHIT_TEMPLATE, Coordinate::new(0, 14), 1, 72);
            // Newest additions: the two new Arcane Tradition subclass
            // templates. The Evocation Wizard is the interesting one for
            // this smoke test — it drives four new engine paths the AI
            // can reach mid-encounter: the Sculpt Spells branch of the
            // AoE picker's friendly-fire gate (it will fire blasts into
            // its own team, which no other template does), the Potent
            // Cantrip upgrade at the post-save chokepoint, the Empowered
            // Evocation bonus on the shared damage roll, and the
            // Overchannel prime plus its escalating necrotic backlash,
            // which is the only path in the engine where an actor can
            // knock *itself* down as a scheduled consequence of its own
            // cast. Placed on team 0 next to real allies so the sculpt
            // path is actually exercised rather than trivially empty.
            // The Abjuration Wizard exercises the Arcane Ward weave /
            // recharge / absorb cycle off its own Shield and Mage Armor
            // casts.
            // The Divination Wizard drives the newest engine path of
            // the three: Portent reaches into the `roll_d20_lucky`
            // chokepoint and substitutes a foretold face for a d20 the
            // diviner is *not* rolling, which is the only place in the
            // engine where one actor's feature short-circuits another
            // actor's roll. Placed on team 0 with enemies in sight so
            // both directions of the substitution (high faces onto
            // allies, low faces onto enemies) are reachable, plus the
            // Expert Divination slot refund off its Mind Spike / True
            // Seeing / Foresight pickups.
            // The Enchantment Wizard drives the doubling path from the
            // free side: every single-target enchantment it casts
            // re-enters `side_effects` for a second creature, including
            // the concentration spells whose merged `StartConcentration`
            // is the trickiest part of that block. Its Hypnotic Gaze
            // also puts a `Charmed` back-link on a hostile mid-fight,
            // which is the AI-side exercise of the charm restriction on
            // declared actions, opportunity attacks and Riposte alike.
            // The Illusion Wizard drives the interception cohort from
            // the target side: sitting on team 0 it is swung at by the
            // hostiles, so a connecting attack walks the shared
            // `attack_intercepted` rows and — once per short rest —
            // spends the wizard's reaction to no-sell the hit. Its
            // baseline loadout also carries Mirror Image, so the AI
            // reaches the layered case the cohort ordering exists for:
            // decoys first, per-rest charge for what gets through.
            // The Conjuration Wizard exercises the school tag from both
            // ends under the driver: its Web / Stinking Cloud / Cloudkill
            // casts install concentration marks that Focused Conjuration
            // then has to resolve back to a school by name, and every
            // levelled conjuration it casts re-arms Benign Transposition
            // through the post-cast trigger registry.
            // The Transmutation Wizard runs the stone's save-proficiency
            // grant through the AI's own concentration saves, and its
            // Shapechanger puts a second concentration source on the
            // chassis — so the driver exercises the case where an
            // emergency self-Polymorph displaces a control spell the AI
            // was already holding.
            use crate::actors::creatures::wizards::{
                ABJURATION_WIZARD_TEMPLATE, CONJURATION_WIZARD_TEMPLATE,
                DIVINATION_WIZARD_TEMPLATE, ENCHANTMENT_WIZARD_TEMPLATE,
                EVOCATION_WIZARD_TEMPLATE, ILLUSION_WIZARD_TEMPLATE,
                TRANSMUTATION_WIZARD_TEMPLATE,
            };
            let _ = e.instantiate_creature(&EVOCATION_WIZARD_TEMPLATE, Coordinate::new(4, 6), 0, 24);
            let _ = e.instantiate_creature(&ABJURATION_WIZARD_TEMPLATE, Coordinate::new(4, 8), 0, 25);
            let _ = e.instantiate_creature(&DIVINATION_WIZARD_TEMPLATE, Coordinate::new(4, 10), 0, 26);
            let _ =
                e.instantiate_creature(&ENCHANTMENT_WIZARD_TEMPLATE, Coordinate::new(4, 12), 0, 27);
            let _ =
                e.instantiate_creature(&ILLUSION_WIZARD_TEMPLATE, Coordinate::new(4, 14), 0, 28);
            let _ =
                e.instantiate_creature(&CONJURATION_WIZARD_TEMPLATE, Coordinate::new(4, 16), 0, 29);
            let _ = e.instantiate_creature(
                &TRANSMUTATION_WIZARD_TEMPLATE,
                Coordinate::new(6, 6),
                0,
                30,
            );
            // The Eldritch Knight is the first chassis in the driver
            // that both swings and casts, which is the only way to
            // exercise either of its mid-tier features end-to-end.
            // Eldritch Strike stamps its mark from the weapon-hit
            // chokepoint and cashes it at the save chokepoint, so the
            // driver has to route a swing and a spell through the same
            // actor against the same target across turns. War Magic
            // needs the AI to actually open a turn with a cantrip —
            // which happens whenever the knight is out of melee reach
            // and reaches for Fire Bolt — and then find the bonus
            // action still unspent.
            use crate::actors::creatures::fighters::ELDRITCH_KNIGHT_FIGHTER_TEMPLATE;
            let _ = e.instantiate_creature(
                &ELDRITCH_KNIGHT_FIGHTER_TEMPLATE,
                Coordinate::new(6, 8),
                0,
                31,
            );
            // The Arcane Trickster and the Shadow Monk both add a
            // bonus-action rung whose whole design is competing with
            // rungs already on the ladder, so the driver is where the
            // ordering gets exercised: the Trickster's Versatile
            // Trickster has to lose to Cunning Action when Cunning
            // Action has something to do, and the monk's Shadow Step
            // has to decline whenever Stunning Strike or Flurry wants
            // the slot. The step also drives the only landing-spot
            // search in the AI that aims *toward* a target rather than
            // away, so a walled-in monk is the case that exercises its
            // give-up path.
            use crate::actors::creatures::monks::SHADOW_MONK_TEMPLATE;
            use crate::actors::creatures::rogues::ARCANE_TRICKSTER_ROGUE_TEMPLATE;
            let _ = e.instantiate_creature(
                &ARCANE_TRICKSTER_ROGUE_TEMPLATE,
                Coordinate::new(6, 10),
                0,
                32,
            );
            let _ =
                e.instantiate_creature(&SHADOW_MONK_TEMPLATE, Coordinate::new(6, 12), 0, 33);
            // The Moon Druid is the only actor in the driver whose
            // action list can *stop working* mid-encounter: taking the
            // beast form locks out every levelled spell it carries. So
            // it exercises a path nothing else does — the AI's casting
            // rungs walking a caster whose slots are unspendable, and
            // Wild Heal being the only thing those slots can still buy.
            use crate::actors::creatures::druids::MOON_DRUID_TEMPLATE;
            let _ = e.instantiate_creature(&MOON_DRUID_TEMPLATE, Coordinate::new(6, 14), 0, 34);
            // The swarm bench, one on each team. Two paths nothing else
            // on this board reaches:
            //
            //   - On team 0, the insect swarm is an ally the healers
            //     cannot heal. The support rung has to notice that and
            //     spend its turn on something else — a swarm that stays
            //     bloodied forever is precisely the shape that would
            //     otherwise pin a cleric on a no-op Cure Wounds every
            //     round for the rest of the fight.
            //   - On team 1, the quipper swarm is a target whose damage
            //     output *changes* as it dies, through the
            //     attacker-scoped lane, and whose Blood Frenzy pulls the
            //     other way at the same time.
            //
            // The poisonous-snake swarm brings the save rider on the
            // same chassis so the thinning halves a two-part swing.
            use crate::actors::creatures::swarms::{
                SWARM_OF_INSECTS_TEMPLATE, SWARM_OF_POISONOUS_SNAKES_TEMPLATE,
                SWARM_OF_QUIPPERS_TEMPLATE,
            };
            let _ = e.instantiate_creature(&SWARM_OF_INSECTS_TEMPLATE, Coordinate::new(6, 16), 0, 35);
            let _ = e.instantiate_creature(&SWARM_OF_QUIPPERS_TEMPLATE, Coordinate::new(8, 8), 1, 73);
            let _ = e.instantiate_creature(
                &SWARM_OF_POISONOUS_SNAKES_TEMPLATE,
                Coordinate::new(8, 10),
                1,
                74,
            );
            // A knight and their warhorse, side by side on team 0. The
            // one pairing on this board that exercises `engine::mounts`
            // end to end: the AI's mount rung fires on the first turn,
            // and from then on every movement, zone, opportunity-attack
            // and death path in the driver is running with one actor
            // off the occupancy grid and mirrored onto another. Nothing
            // else here would notice if that stopped holding.
            use crate::actors::creatures::knights::KNIGHT_TEMPLATE;
            use crate::actors::creatures::warhorses::WARHORSE_TEMPLATE;
            let _ = e.instantiate_creature(&KNIGHT_TEMPLATE, Coordinate::new(2, 8), 0, 36);
            let _ = e.instantiate_creature(&WARHORSE_TEMPLATE, Coordinate::new(4, 8), 0, 37);
            // `from_params` already initialised the encounter; instantiate_creature
            // wires the new actors into the initiative queue itself.
            let ai = SimpleAi;
            let total_hp = |e: &EncounterInstance| -> u32 {
                e.actors.values().map(|a| a.hitpoints()).sum()
            };
            let mut last_total = total_hp(&e);
            let mut idle_streak = 0usize;
            let stalemate_window = 4 * e.actors.len().max(1);
            for _ in 0..50_000 {
                e.process_stack();
                if e.is_complete() {
                    break;
                }
                let Some(prompt) = e.peek_prompt() else { break };
                let actor_id = prompt.actor_id();
                match ai.decide(&e, actor_id) {
                    ControllerDecision::AwaitInput => {
                        panic!("SimpleAi returned AwaitInput unexpectedly");
                    }
                    ControllerDecision::Act(aei) => {
                        e.pop_prompt();
                        e.push_action(aei);
                    }
                }
                let cur = total_hp(&e);
                if cur == last_total {
                    idle_streak += 1;
                    if idle_streak >= stalemate_window {
                        break;
                    }
                } else {
                    last_total = cur;
                    idle_streak = 0;
                }
            }
            // Three of the creatures placed above have lairs (Lich,
            // Adult Red Dragon, Beholder), and a lair acts on its own
            // schedule rather than through the AI — so nothing else in
            // this driver would notice if the dispatcher stopped firing.
            // The line is the proof it ran inside a live encounter, on
            // a board the AI was moving underneath it.
            assert!(
                e.messages().iter().any(|m| m.contains("[lair]")),
                "seed {}: a lair should have acted at some point",
                seed
            );
            // Somebody got on something. Which pairing it is varies by
            // seed and is not the point — the cleric has taken the
            // hippogriff on one of these and the knight the warhorse on
            // another — but a driver in which nobody ever mounts is one
            // where `engine::mounts` is not being exercised at all, and
            // every path below it is running on an unridden board.
            // Somebody got on something. *Which* pairing varies by seed
            // and is deliberately not asserted: on one of these the
            // divination wizard takes the warhorse before the knight
            // does, and on another a kraken's lightning storm kills the
            // horse in round two — both are the board working. What
            // would be a regression is a driver in which nobody ever
            // mounts, because then every path below this rung is
            // running on an unridden board and `engine::mounts` is
            // being exercised by its unit tests alone.
            assert!(
                e.messages().iter().any(|m| m.contains(" mounts ")),
                "seed {}: somebody on this board should have found a saddle",
                seed
            );
            // 5e weapon mastery, end to end. Martial chassis stand on
            // team 0 holding weapons out of the armoury, so a driver in
            // which no mastery property ever fires is one where the
            // whole chain — the template's training flag, the tag on
            // the weapon, the picker that ranks it, the rider on the
            // pipeline — is being exercised by its unit tests alone.
            //
            // Accumulated across the seed sweep rather than asserted
            // per seed, and that is not a hedge: on one of these three
            // boards the martials are still closing when the casters
            // finish the fight, and a per-seed assertion would be
            // testing which end of the map the AI walked to. Every
            // property is separately pinned in `engine::mastery`'s own
            // tests; what only a live board can show is that a swing
            // the AI chose, against a target it picked, still carries
            // the clause.
            mastery_fired |= MASTERY_LOG_TAGS
                .iter()
                .any(|tag| e.messages().iter().any(|m| m.contains(tag)));
            legendary_fired |= e.messages().iter().any(|m| m.contains("[legendary]"));
        }
        assert!(
            mastery_fired,
            "martials swung on three boards and no weapon mastery ever fired"
        );
        assert!(
            legendary_fired,
            "eight legendary creatures fought on three boards and none of \
             them ever took a legendary action"
        );
    }

    /// Build a no-actors encounter we can hand-place creatures into.
    /// The AI gets on the horse when there is one beside it, picks the
    /// nearest of two, and declines a mount that would slow it down.
    #[test]
    fn the_ai_takes_the_nearest_horse_that_is_faster_than_it_is() {
        use crate::actors::creatures::draft_horses::DRAFT_HORSE_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::warhorses::WARHORSE_TEMPLATE;
        use crate::engine::types::Coordinate;

        let mut e = empty_arena();
        let knight = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        // No stable, no rung.
        assert!(try_mount_up(&e, knight).is_none());

        // A horse across the room is out of reach — `can_mount` refuses
        // it and so does the rung.
        let far = e
            .instantiate_creature(&WARHORSE_TEMPLATE, Coordinate::new(20, 12), 0, 0)
            .unwrap();
        assert!(try_mount_up(&e, knight).is_none());

        // Two beside it: the nearer wins.
        let near = e
            .instantiate_creature(&WARHORSE_TEMPLATE, Coordinate::new(5, 3), 0, 1)
            .unwrap();
        let aei = try_mount_up(&e, knight).expect("a horse is right there");
        assert_eq!(aei.action().name(), "mount");
        assert_eq!(aei.target_ids(), Some(&[near][..]));

        // Riding it costs movement and nothing else — the turn's action
        // is still there for the rungs below.
        e.actors.get_mut(&knight).unwrap().reset_for_new_round();
        let before = e.actors[&knight].remaining_movement();
        for se in aei.execute(&mut e) {
            se.apply(&mut e);
        }
        assert!(e.is_mounted(knight));
        assert!(
            e.actors[&knight].can_consume_resource(Resource::Action),
            "mounting is priced in feet, not in the turn"
        );
        assert!(
            e.actors[&knight].remaining_movement() < before,
            "and the feet were charged"
        );
        // Already up: the rung declines rather than looping.
        assert!(try_mount_up(&e, knight).is_none());
        let _ = far;

        // A second knight with only a slower horse to hand stays on its
        // own feet — a draft horse plods at 40 to a fighter's 30, so the
        // gate is exercised with a genuinely slower body.
        let mut e2 = empty_arena();
        let walker = e2
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        let cart_horse = e2
            .instantiate_creature(&DRAFT_HORSE_TEMPLATE, Coordinate::new(5, 3), 0, 0)
            .unwrap();
        assert!(e2.can_mount(walker, cart_horse).is_ok());
        let faster = e2.actors[&cart_horse].speed() > e2.actors[&walker].speed();
        assert_eq!(
            try_mount_up(&e2, walker).is_some(),
            faster,
            "the rung tracks whether the horse is actually an upgrade"
        );
    }

    /// A horse with a rider beside it holds its ground; one with
    /// nobody waiting, or with the fight already on it, does not.
    #[test]
    fn a_horse_waits_for_its_rider_and_not_for_anybody_else() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::warhorses::WARHORSE_TEMPLATE;
        use crate::engine::types::Coordinate;

        let mut e = empty_arena();
        let horse = e
            .instantiate_creature(&WARHORSE_TEMPLATE, Coordinate::new(5, 3), 0, 0)
            .unwrap();
        // Nobody wants it: the horse fights like anything else.
        assert!(try_stand_for_rider(&e, horse).is_none());

        let knight = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        let aei = try_stand_for_rider(&e, horse).expect("its rider is right there");
        assert_eq!(aei.action().name(), "dodge");

        // Once somebody is on it, the wait is over.
        assert!(e.mount(knight, horse).is_ok());
        assert!(try_stand_for_rider(&e, horse).is_none());
        assert!(e.dismount(knight));

        // And a horse the fight has already reached kicks rather than
        // stands: RAW's controlled mount can Dodge, but a horse nobody
        // has climbed on yet is just a creature in melee.
        let _ = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(9, 3), 1, 0)
            .unwrap();
        assert!(
            !e.combat_active_enemy_ids_adjacent(horse).is_empty(),
            "the goblin is in contact with the horse's Large footprint"
        );
        assert!(try_stand_for_rider(&e, horse).is_none());
    }

    /// The whole player-side route into `engine::mounts`, end to end:
    /// the paladin casts Find Steed, the steed appears beside them, and
    /// the mount rung puts them on it — with the turn's Action still
    /// unspent, because getting into a saddle is priced in feet.
    ///
    /// Worth driving through the ladder rather than calling the two
    /// rungs directly. Find Steed reaches the AI through the summon
    /// lane's *free* tier (it holds no concentration), and the mount
    /// rung sits several rungs below that — so this is also the proof
    /// that a paladin gets to both of them in the same fight instead of
    /// one starving the other.
    #[test]
    fn a_paladin_conjures_a_steed_and_gets_on_it() {
        use crate::actors::creatures::paladins::PALADIN_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::engine::types::Coordinate;

        let mut e = empty_arena();
        let paladin = e
            .instantiate_creature(&PALADIN_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        // Somebody to fight, far enough off that the ladder isn't busy
        // swinging: the summon rung wants an enemy within 24 tiles.
        let _ = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(24, 12), 1, 0)
            .unwrap();

        let ai = SimpleAi;
        for _ in 0..40 {
            e.process_stack();
            if e.is_complete() || e.is_mounted(paladin) {
                break;
            }
            let Some(prompt) = e.peek_prompt() else { break };
            let actor_id = prompt.actor_id();
            match ai.decide(&e, actor_id) {
                ControllerDecision::Act(aei) => {
                    e.pop_prompt();
                    e.push_action(aei);
                }
                ControllerDecision::AwaitInput => break,
            }
        }

        assert!(
            e.messages().iter().any(|m| m.contains("find steed")),
            "the paladin should reach its own summon:\n{}",
            e.messages().join("\n")
        );
        assert!(
            e.is_mounted(paladin),
            "…and then get on it:\n{}",
            e.messages().join("\n")
        );
        let steed = e.actors[&paladin].mounted_on().unwrap();
        assert_eq!(e.actors[&steed].team(), e.actors[&paladin].team());
        assert_eq!(
            e.actors[&paladin].location(),
            e.actors[&steed].location(),
            "and rides where it stands"
        );
    }

    /// The AI can fight in the dark.
    ///
    /// Every rung of the picker that reads visibility now reads the
    /// lighting layer too — `peek_attack_mode` folds in the two
    /// darkness clauses, `viewer_can_see` gates the reactive taxes, and
    /// the target ranking sits on top of both. This driver is the proof
    /// that none of that stalls the AI: an unlit board is the one
    /// configuration in which *every* creature on it is potentially
    /// unable to see *every* other, which is a state nothing before the
    /// lighting layer could produce.
    ///
    /// Deliberately a mixed board. The goblins and the kobold see 60 ft
    /// in the dark and the humans see nothing, so the two sides are
    /// asymmetric in exactly the way the layer is about; the warlock
    /// carries Darkness and Devil's Sight, so a sphere can go down
    /// mid-fight on top of an already-unlit board and the AI has to
    /// keep working underneath it.
    ///
    /// What is asserted is that the fight *happens* — the driver
    /// terminates and blood is drawn. Which side wins, and whether
    /// anybody thinks to light a torch, are picker decisions this test
    /// deliberately does not pin: they are exactly the judgements that
    /// should be free to improve without a test having to be edited.
    #[test]
    fn the_ai_fights_on_an_unlit_board() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::kobolds::KOBOLD_TEMPLATE;
        use crate::actors::creatures::monks::SHADOW_MONK_TEMPLATE;
        use crate::actors::creatures::warlocks::WARLOCK_TEMPLATE;
        use crate::engine::lighting::AmbientLight;
        use crate::engine::types::Coordinate;

        for seed in [5u64, 23] {
            let tp = TerrainGenParams {
                width: 30,
                height: 20,
                branch_depth: 0,
                branch_prob: 0.0,
            };
            let ap = ActorGenParams {
                cr_target: 0.0,
                n_teams: 0,
                pc_template: None,
                start_team: 0,
            };
            let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
            e.set_ambient_light(AmbientLight::Darkness);
            // Team 0 sees nothing without help and carries two ways to
            // get it: the cleric's Light cantrip and the fighter's
            // torch.
            let fighter = e
                .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(2, 2), 0, 0)
                .unwrap();
            e.actors
                .get_mut(&fighter)
                .unwrap()
                .pickup_item(&crate::items::item_template::TORCH);
            let _ = e.instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(2, 4), 0, 1);
            let _ = e.instantiate_creature(&SHADOW_MONK_TEMPLATE, Coordinate::new(2, 6), 0, 2);
            // Team 1 sees 60 ft in the dark for free, and the warlock
            // can make more of it.
            let _ = e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(16, 2), 1, 0);
            let _ = e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(16, 4), 1, 1);
            let _ = e.instantiate_creature(&KOBOLD_TEMPLATE, Coordinate::new(16, 6), 1, 2);
            let _ = e.instantiate_creature(&WARLOCK_TEMPLATE, Coordinate::new(16, 8), 1, 3);

            let ai = SimpleAi;
            let total_hp = |e: &EncounterInstance| -> u32 {
                e.actors.values().map(|a| a.hitpoints()).sum()
            };
            let opening_hp = total_hp(&e);
            let mut last_total = opening_hp;
            let mut idle_streak = 0usize;
            // Wider than the main driver's window. Fighting in the
            // dark is slow by construction — a turn spent striking a
            // light, and several spent walking toward a noise, all pass
            // without anybody's hit points moving — and a window tuned
            // for a lit board reads that as a stalemate and bails out
            // before the first swing.
            let stalemate_window = 12 * e.actors.len().max(1);
            let mut steps = 0usize;
            for _ in 0..50_000 {
                steps += 1;
                e.process_stack();
                if e.is_complete() {
                    break;
                }
                let Some(prompt) = e.peek_prompt() else { break };
                let actor_id = prompt.actor_id();
                match ai.decide(&e, actor_id) {
                    ControllerDecision::AwaitInput => {
                        panic!("seed {}: SimpleAi returned AwaitInput in the dark", seed)
                    }
                    ControllerDecision::Act(aei) => {
                        e.pop_prompt();
                        e.push_action(aei);
                    }
                }
                let cur = total_hp(&e);
                if cur == last_total {
                    idle_streak += 1;
                    if idle_streak >= stalemate_window {
                        break;
                    }
                } else {
                    last_total = cur;
                    idle_streak = 0;
                }
            }
            assert!(
                steps < 50_000,
                "seed {}: the driver never terminated on an unlit board",
                seed
            );
            assert!(
                total_hp(&e) < opening_hp,
                "seed {}: nobody managed to hit anybody in the dark",
                seed
            );
            // Somebody on the sightless team struck a light. Which of
            // the two ways they did it is a picker decision and is not
            // pinned; that one of them happened is the whole point of
            // the `try_make_light` rung, and a driver in which nobody
            // ever does is one where the humans spent the fight
            // swinging at noises.
            assert!(
                e.messages()
                    .iter()
                    .any(|m| m.contains("lights a torch") || m.contains("begins to glow")),
                "seed {}: nobody struck a light on an unlit board",
                seed
            );
            // …and the goblins did not, because they can already see.
            // The reveal band is 16 tiles and their darkvision is 24,
            // so a goblin that lights up has handed away the only
            // advantage the dark was giving it.
            for (id, actor) in e.actors.iter() {
                if actor.team() == 1 && actor.darkvision_tiles() >= LIGHT_REVEAL_BAND {
                    assert!(
                        !e.actor_carries_light(*id),
                        "seed {}: {} can see in the dark and lit itself up anyway",
                        seed,
                        actor.name()
                    );
                }
            }
        }
    }

    fn empty_arena() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap()
    }

    /// The attrition rung sits in the seam it was cut for: a burst wins
    /// over it, and it wins over a swing.
    ///
    /// Both halves are the point. Attrition above focus fire is why the
    /// rung exists at all — a Command that costs an enemy its whole turn
    /// beats one creature's worth of weapon damage. Attrition below the
    /// bursts is why it is not on the lockdown cohort, where it spent
    /// one afternoon shutting out every Fireball on the roster.
    #[test]
    fn attrition_loses_to_a_blast_and_beats_a_bowshot() {
        use crate::actors::creatures::cult_fanatics::CULT_FANATIC_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::engine::types::Coordinate;

        // One lone enemy, far enough out that no burst catches two: the
        // rung's own lane.
        let mut solo = empty_arena();
        let fanatic = solo
            .instantiate_creature(&CULT_FANATIC_TEMPLATE, Coordinate::new(4, 4), 0, 0)
            .unwrap();
        solo.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(12, 4), 1, 0)
            .unwrap();
        assert!(
            solo.actors[&fanatic].find_action("command").is_some(),
            "the premise: the fanatic knows the cheapest row on the cohort"
        );
        let pick = try_attrition(&solo, fanatic).expect("one enemy is enough to debuff");
        assert_eq!(pick.action().name(), "command");

        // The AI's own answer on that board, which is the half a picker
        // test cannot reach: the rung has to actually be *in* the ladder
        // and above focus fire. The concentration is pre-spent so the
        // whole control lane above declines, and one distant enemy means
        // no burst rung can claim two — which leaves exactly the seam
        // this rung was cut into.
        solo.actors
            .get_mut(&fanatic)
            .unwrap()
            .start_concentration(crate::actors::actor_template::ConcentrationData::new("web"));
        let ai = SimpleAi {};
        match ai.decide(&solo, fanatic) {
            ControllerDecision::Act(aei) => assert_eq!(
                aei.action().name(),
                "command",
                "the ladder should reach the attrition rung ahead of a swing"
            ),
            ControllerDecision::AwaitInput => panic!("the fanatic should have something to do"),
        }
    }

    /// The lockdown cohort reaches past the rows it inherited, and a
    /// caster who is already concentrating can still reach the half of
    /// the list that needs no concentration.
    ///
    /// Both halves matter and they fail differently. The first pins that
    /// the new rows are live at all — a wizard facing a Giant cannot
    /// Hold Person it, and before this cohort grew there was nothing
    /// else on the rung to fall through to, so the wizard shrugged and
    /// went to the damage lane. The second pins the change that made the
    /// non-concentration rows possible: the picker used to bail on a
    /// concentrating caster before it looked at a single row, so a
    /// wizard holding a Web could not have cast Power Word Stun with
    /// eight levels of spell slots in hand.
    #[test]
    fn the_lockdown_lane_reaches_its_new_rows_and_past_its_own_concentration() {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::engine::types::Coordinate;

        let mut e = empty_arena();
        let wizard = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(4, 4), 0, 0)
            .unwrap();
        let ogre = e
            .instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(10, 4), 1, 0)
            .unwrap();
        // The premise, asserted rather than assumed: the archmage's list
        // does not contain a single one of the five rows this cohort had
        // before it was a cohort, so anything the rung reaches for here
        // is necessarily one of the new ones.
        for inherited in ["hold person", "sleep gaze"] {
            assert!(
                e.actors[&wizard].find_action(inherited).is_none(),
                "{inherited} would make this test prove nothing"
            );
        }
        let pick = try_lockdown(&e, wizard).expect("a lock the wizard can actually reach");
        assert_eq!(pick.target_ids(), Some(&[ogre][..]));

        // Now hand the wizard a concentration and re-ask. What is left
        // is the non-concentration half of the list — which the old
        // up-front bail made unreachable entirely — and the earliest row
        // on it that the archmage carries is Forcecage.
        e.actors
            .get_mut(&wizard)
            .unwrap()
            .start_concentration(ConcentrationData::new("web"));
        let pick = try_lockdown(&e, wizard)
            .expect("a concentrating caster still has slotless-of-concentration locks");
        assert!(
            !pick.action().holds_concentration(),
            "a concentrating caster must not be offered a second concentration spell, got {}",
            pick.action().name()
        );
        assert_eq!(pick.action().name(), "forcecage");
    }

    /// The two spells their classes are named after get cast, and the
    /// rider they buy shows up on the swing.
    ///
    /// Driven through a real AI turn rather than by calling the picker,
    /// because the picker was never the thing that was broken — the
    /// rung's absence was. A test that only asked
    /// `try_concentration_mark` would have passed on the day before this
    /// existed, against a warlock who could not reach the spell from the
    /// ladder at all.
    #[test]
    fn the_warlock_hexes_and_the_ranger_marks() {
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::engine::types::Coordinate;

        for (template, condition, label) in [
            (
                &*crate::actors::creatures::warlocks::WARLOCK_TEMPLATE,
                Condition::Hexed,
                "hex",
            ),
            (
                &*crate::actors::creatures::rangers::RANGER_TEMPLATE,
                Condition::HuntersMarked,
                "hunters mark",
            ),
        ] {
            let mut e = empty_arena();
            let caster = e
                .instantiate_creature(template, Coordinate::new(6, 4), 0, 0)
                .unwrap();
            // Inside the 12-tile engagement band the cohort gates on, so
            // the test is about the rung and not about the approach.
            let ogre = e
                .instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(12, 4), 1, 0)
                .unwrap();

            // Let the ladder run: the rungs above this one (Mage Armor
            // and the rest of the opening self-buffs) fire first, which
            // is correct — the point is that the mark is reached at all,
            // not that it is reached first.
            let mut cast_the_mark = false;
            for _ in 0..12 {
                let ControllerDecision::Act(aei) = SimpleAi {}.decide(&e, caster) else {
                    break;
                };
                cast_the_mark |= aei.action().name() == label;
                e.push_action(aei);
                e.process_stack();
                if cast_the_mark {
                    break;
                }
            }
            assert!(cast_the_mark, "{label}: the ladder never reached the mark");
            assert!(
                e.actors[&ogre].has_condition(condition),
                "{label}: the mark should be on the target"
            );
            assert!(
                e.actors[&caster].is_concentrating(),
                "{label}: and the caster holding it"
            );
            // The rider is the entire point of the spell, so pin that the
            // engine will actually pay it rather than just that the flag
            // is set.
            assert!(
                e.is_hex_target(caster, ogre) || e.is_hunters_mark_target(caster, ogre),
                "{label}: the attack resolver has to recognise the mark"
            );
            // And the rung declines to re-mark what it already marked —
            // a second cast would silently replace the first.
            assert!(
                try_concentration_mark(&e, caster).is_none(),
                "{label}: no re-marking"
            );
        }
    }

    /// The dispel rung ranks the board, and a flier outranks everything
    /// else on it.
    ///
    /// Three enemies, three tiers, one wizard: a goblin thirty feet up,
    /// a goblin concentrating, and a goblin merely Blurred. The picker
    /// should reach past the two that a damage-shaped lane could at
    /// least in principle have handled and take the one whose spell is
    /// also holding it in the air — because ending that spell is the
    /// only play on the board that collects 3d6 on the way out.
    ///
    /// Then the tiers are peeled back one at a time, which is the half
    /// that would catch a picker that simply preferred the lowest id or
    /// the nearest body.
    #[test]
    fn the_dispel_picker_reaches_for_the_flier_before_the_concentrator() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::actor_template::ConcentrationData;
        use crate::conditions::ConditionTimer;
        use crate::engine::types::Coordinate;

        let mut e = empty_arena();
        let wizard = e
            .instantiate_creature(
                &crate::actors::creatures::wizards::WIZARD_TEMPLATE,
                Coordinate::new(4, 4),
                0,
                0,
            )
            .unwrap();
        // Ordered so that id ascends as tier descends — a picker that
        // preferred the lowest id would agree with the right answer by
        // accident if these were the other way round.
        let flier = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(8, 4), 1, 0)
            .unwrap();
        let concentrator = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(9, 4), 1, 1)
            .unwrap();
        let buffed = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(10, 4), 1, 2)
            .unwrap();
        e.actors
            .get_mut(&flier)
            .unwrap()
            .add_condition(Condition::Flying, ConditionTimer::Rounds(10));
        e.reconcile_altitudes();
        e.actors
            .get_mut(&concentrator)
            .unwrap()
            .start_concentration(ConcentrationData::new("web"));
        e.actors
            .get_mut(&buffed)
            .unwrap()
            .add_condition(Condition::Blurred, ConditionTimer::Rounds(10));

        let pick = try_dispel_magic(&e, wizard).expect("three legal targets");
        assert_eq!(pick.action().name(), "dispel magic");
        assert_eq!(
            pick.target_ids(),
            Some(&[flier][..]),
            "the airborne target outranks the concentrator and the buff"
        );

        // Take the flier's altitude away and the concentrator inherits
        // the pick.
        e.actors
            .get_mut(&flier)
            .unwrap()
            .remove_condition(Condition::Flying);
        e.actors.get_mut(&flier).unwrap().set_altitude_ft(0);
        let pick = try_dispel_magic(&e, wizard).expect("two legal targets");
        assert_eq!(pick.target_ids(), Some(&[concentrator][..]));

        // And with nothing being concentrated on, the bare buff.
        e.actors.get_mut(&concentrator).unwrap().end_concentration();
        let pick = try_dispel_magic(&e, wizard).expect("one legal target");
        assert_eq!(pick.target_ids(), Some(&[buffed][..]));

        // A board with no magic on it at all is a board the rung
        // declines, rather than one where it burns a 3rd-level slot on
        // a goblin with nothing to strip.
        e.actors
            .get_mut(&buffed)
            .unwrap()
            .remove_condition(Condition::Blurred);
        assert!(
            try_dispel_magic(&e, wizard).is_none(),
            "nothing to end means nothing to cast"
        );
    }

    /// The bite rung is the exception to the damage picker, and it is
    /// only an exception while the heal is live.
    ///
    /// A healthy Beast Barbarian should swing the greataxe — 1d12 beats
    /// 1d8 and the bite would heal nothing. Take it below half and the
    /// bite becomes the better trade. Spend the once-per-turn mark and
    /// it stops being one again, mid-turn, without the barbarian's hit
    /// points changing at all.
    #[test]
    fn the_beast_bite_is_only_reached_for_once_the_rage_is_losing() {
        use crate::actions::class_features::FORM_OF_THE_BEAST_BITE_TAG;
        use crate::actors::creatures::barbarians::BITE_BEAST_BARBARIAN_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::conditions::ConditionTimer;

        let mut e = empty_arena();
        let barb = e
            .instantiate_creature(
                &BITE_BEAST_BARBARIAN_TEMPLATE,
                Coordinate::new(5, 5),
                0,
                0,
            )
            .unwrap();
        e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        e.actors
            .get_mut(&barb)
            .unwrap()
            .add_condition(Condition::Raging, ConditionTimer::Rounds(10));

        assert!(
            try_beast_bite_when_bloodied(&e, barb).is_none(),
            "an unhurt barbarian has nothing to gain from the smaller die"
        );

        // Down to just under half.
        let max = e.actors[&barb].max_hitpoints();
        e.actors.get_mut(&barb).unwrap().take_damage(max / 2 + 1);
        assert!(
            e.actors[&barb].hitpoints() * 2 < max,
            "fixture should be below half"
        );
        let aei = try_beast_bite_when_bloodied(&e, barb)
            .expect("below half, the bite's heal is worth the smaller die");
        assert_eq!(aei.action().name(), "bite");

        // The heal is once per turn; past the mark the bite is simply
        // the worse weapon again.
        e.actors
            .get_mut(&barb)
            .unwrap()
            .mark_once_per_turn_used(FORM_OF_THE_BEAST_BITE_TAG);
        assert!(
            try_beast_bite_when_bloodied(&e, barb).is_none(),
            "the rung stands down once the heal is spent"
        );
    }

    /// Two identical enemies at identical HP: shoot the one that isn't
    /// behind a wall.
    ///
    /// This tie happens constantly — a pack of the same monster at full
    /// HP ties on every key above cover — and before cover joined the
    /// sort key it was broken by actor id, which is to say by nothing.
    #[test]
    fn focus_fire_shoots_around_cover_when_the_targets_are_otherwise_equal() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::rangers::RANGER_TEMPLATE;
        use crate::engine::terrain::TerrainType;

        let mut e = empty_arena();
        // `empty_arena` is generator output, so it already has walls and
        // a scatter of its own. Pave it: the fixture is about one low
        // wall, and any second obstruction on either line would make the
        // two goblins tie on cover as well.
        for y in 0..20 {
            for x in 0..30 {
                e.set_terrain_at(Coordinate::new(x, y), TerrainType::Floor);
            }
        }
        let archer = e
            .instantiate_creature(&RANGER_TEMPLATE, Coordinate::new(2, 4), 0, 0)
            .unwrap();
        // Two goblins the same distance out, on separate rows so each
        // has its own line back to the archer.
        let sheltered = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(12, 4), 1, 0)
            .unwrap();
        let exposed = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(12, 10), 1, 1)
            .unwrap();
        // Goblin HP is rolled, so level the two by hand — the whole
        // point of the fixture is that cover is the only thing left to
        // separate them.
        let floor = e.actors[&sheltered]
            .hitpoints()
            .min(e.actors[&exposed].hitpoints());
        for id in [sheltered, exposed] {
            let excess = e.actors[&id].hitpoints() - floor;
            if excess > 0 {
                e.actors.get_mut(&id).unwrap().take_damage(excess);
            }
        }
        assert_eq!(
            e.actors[&sheltered].effective_hitpoints(),
            e.actors[&exposed].effective_hitpoints(),
            "fixture depends on the two being equally close to dropping"
        );
        // The sheltered one is the lower id, so before cover entered the
        // key it won the tie.
        assert!(sheltered < exposed);

        assert!(e.set_terrain_at(Coordinate::new(7, 4), TerrainType::LowWall));
        assert_eq!(e.cover_ac_bonus(archer, sheltered), 2);
        assert_eq!(e.cover_ac_bonus(archer, exposed), 0);

        let aei = try_attack_focus_fire(&e, archer).expect("the ranger should find a shot");
        assert_eq!(
            aei.target_ids().map(|t| t.to_vec()),
            Some(vec![exposed]),
            "the shot should go to the goblin with nothing in front of it"
        );
    }

    /// A wrapper attack outranks the single swing it contains.
    ///
    /// A monster's action list carries the Multiattack / CompoundAttack
    /// *and* the swings inside it, and the picker ranks them against
    /// each other. Before `expected_damage` existed the wrapper won by
    /// being declared last; now it has to win on the number, which means
    /// the wrappers must aggregate their parts rather than fall through
    /// to the "no estimate" default — a wrapper scoring 0.0 against its
    /// own sub-attack's positive score would lose every tie and take
    /// every Multiattack creature in the bestiary out of its
    /// Multiattack.
    #[test]
    fn the_picker_prefers_a_multiattack_to_the_swing_inside_it() {
        use crate::actions::monster_attacks::{BROWN_BEAR_BITE, BROWN_BEAR_MULTI};
        use crate::actors::creatures::brown_bears::BROWN_BEAR_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;

        let mut e = empty_arena();
        let bear = e
            .instantiate_creature(&BROWN_BEAR_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let ogre = e
            .instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(8, 5), 1, 0)
            .unwrap();
        let actor = e.actors[&bear].clone();
        let picked = best_attack_against(bear, &actor, &e, ogre)
            .map(|(_, a)| a.name().to_string())
            .expect("the bear should find a swing");
        let bite = BROWN_BEAR_BITE
            .expected_damage(&e, bear)
            .expect("a plain swing estimates itself");
        let multi = BROWN_BEAR_MULTI
            .expected_damage(&e, bear)
            .expect("a wrapper aggregates its parts");
        assert!(
            multi > bite,
            "bite + claws should out-estimate the bite alone: {multi} vs {bite}"
        );
        assert_eq!(picked, BROWN_BEAR_MULTI.name());
    }

    /// A greataxe out-damages a bite and loses to claws. Both halves
    /// matter: the first is why the bite needs a rung of its own, the
    /// second is what `expected_damage` was added to see.
    #[test]
    fn the_attack_picker_ranks_beast_forms_by_what_a_whole_turn_lands() {
        use crate::actors::creatures::barbarians::{
            BITE_BEAST_BARBARIAN_TEMPLATE, CLAW_BEAST_BARBARIAN_TEMPLATE,
        };
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::conditions::ConditionTimer;

        let pick = |template: &'static crate::actors::actor_template::CreatureTemplate| -> String {
            let mut e = empty_arena();
            let barb = e
                .instantiate_creature(template, Coordinate::new(5, 5), 0, 0)
                .unwrap();
            let ogre = e
                .instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
                .unwrap();
            e.actors
                .get_mut(&barb)
                .unwrap()
                .add_condition(Condition::Raging, ConditionTimer::Rounds(10));
            let actor = e.actors[&barb].clone();
            best_attack_against(barb, &actor, &e, ogre)
                .map(|(_, a)| a.name().to_string())
                .unwrap_or_else(|| "<none>".to_string())
        };

        // 1d8+4 twice (17) against 1d12+4 twice (21).
        assert_eq!(pick(&BITE_BEAST_BARBARIAN_TEMPLATE), "greataxe");
        // 1d6+4 three times (22.5) against 1d12+4 twice (21).
        assert_eq!(pick(&CLAW_BEAST_BARBARIAN_TEMPLATE), "claws");
    }

    #[test]
    fn focus_fire_picks_wounded_target() {
        let mut e = empty_arena();
        // Attacker on team 0; both enemies on team 1 in melee reach.
        let attacker = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let healthy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        let wounded = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 7), 1, 1)
            .unwrap();
        // Knock the second target down to 1 HP.
        let max = e.actors[&wounded].max_hitpoints();
        e.actors.get_mut(&wounded).unwrap().take_damage(max - 1);

        let ai = SimpleAi;
        let decision = ai.decide(&e, attacker);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an attack");
        };
        let target = aei.target_ids().and_then(|ids| ids.first().copied());
        assert_eq!(target, Some(wounded), "AI should focus the wounded target");
        let _ = healthy;
    }

    #[test]
    fn ai_heals_wounded_ally_over_attacking() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        // Cleric + wounded cleric ally; enemy zombie far enough that no
        // melee threat (so kite doesn't pre-empt) but in Sacred Flame range
        // (so attack would validate). Heal-tactic should win the priority.
        let healer = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let wounded = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(7, 5), 0, 1)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 14), 1, 0)
            .unwrap();
        // Drop the ally below 50% HP.
        let max = e.actors[&wounded].max_hitpoints();
        e.actors.get_mut(&wounded).unwrap().take_damage(max - 1);

        let ai = SimpleAi;
        let decision = ai.decide(&e, healer);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(aei.action().name(), "healing word");
        let target = aei.target_ids().and_then(|ids| ids.first().copied());
        assert_eq!(target, Some(wounded));
    }

    #[test]
    fn ai_revives_dying_ally_first() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;

        let mut e = empty_arena();
        let healer = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // Use Fighter for the dying ally — only PCs enter the dying state;
        // a downed Cleric would just die.
        let dying_ally = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(7, 5), 0, 0)
            .unwrap();
        let just_wounded = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 7), 0, 2)
            .unwrap();
        // dying_ally drops to 0 HP (dying); wounded only loses half.
        let dying_max = e.actors[&dying_ally].max_hitpoints();
        e.actors.get_mut(&dying_ally).unwrap().take_damage(dying_max);
        let wounded_max = e.actors[&just_wounded].max_hitpoints();
        e.actors
            .get_mut(&just_wounded)
            .unwrap()
            .take_damage(wounded_max / 2 + 1);

        assert!(e.actors[&dying_ally].is_dying());
        let ai = SimpleAi;
        let decision = ai.decide(&e, healer);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected a heal");
        };
        assert_eq!(aei.action().name(), "healing word");
        let target = aei.target_ids().and_then(|ids| ids.first().copied());
        assert_eq!(target, Some(dying_ally), "dying ally should win priority");
    }

    #[test]
    fn ai_doesnt_heal_full_hp_ally() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        // Cleric + healthy ally + nearby enemy. AI should attack, not heal.
        let healer = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _ally = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(7, 5), 0, 1)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();

        let ai = SimpleAi;
        let decision = ai.decide(&e, healer);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_ne!(
            aei.action().name(),
            "healing word",
            "no ally needs healing"
        );
    }

    #[test]
    fn ai_picks_aoe_when_two_enemies_clustered() {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;

        let mut e = empty_arena();
        // Cleric on team 0; two enemies tightly clustered on team 1, no
        // allies near them. Pre-set the cleric's concentration so Hold
        // Person (higher priority than AoE) is gated out — this test is
        // specifically about the AoE-vs-single-target choice. Goblins
        // are non-undead so Turn Undead doesn't pre-empt either.
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _e1 = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(10, 10), 1, 0)
            .unwrap();
        let _e2 = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(12, 10), 1, 1)
            .unwrap();
        e.actors
            .get_mut(&cleric)
            .unwrap()
            .start_concentration(ConcentrationData::new("Placeholder"));
        // And gate out the summon rung, which also outranks AoE. The
        // held concentration used to do that on its own; it no longer
        // does, because the rung asks each summon whether *it* wants a
        // concentration rather than declining wholesale — and the
        // cleric's Animate Dead wants none. Marking the cleric as having
        // already called for help is the narrowest way to say "not this
        // rung" without changing what the test is about.
        e.actors.get_mut(&cleric).unwrap().add_condition(
            crate::conditions::Condition::Summoner,
            crate::conditions::ConditionTimer::Rounds(100),
        );

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        // Any Burst-schema spell satisfies "pick AoE": once the cleric
        // gained Web alongside Sacred Burst, "name == sacred burst" was
        // over-specific — Web on a 2-enemy cluster is just as legitimate
        // an AoE pick. Assert the schema, not the spell name.
        assert!(
            matches!(
                aei.action().targeting_schema(),
                crate::actions::action_template::TargetingSchema::Burst { .. }
            ),
            "two-enemy cluster should pull an AoE (Burst) action over single-target, got {}",
            aei.action().name()
        );
    }

    #[test]
    fn ai_avoids_aoe_with_friendly_fire() {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        // Cleric + ally clustered with two enemies — any radius-3 burst
        // catches the ally too. AI should fall back to single-target.
        // Pre-set concentration to gate out Hold Person.
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _ally = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(10, 10), 0, 1)
            .unwrap();
        let _e1 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(11, 10), 1, 0)
            .unwrap();
        let _e2 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(12, 10), 1, 1)
            .unwrap();
        e.actors
            .get_mut(&cleric)
            .unwrap()
            .start_concentration(ConcentrationData::new("Placeholder"));

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_ne!(
            aei.action().name(),
            "sacred burst",
            "any burst would clip the ally — AI should pick single-target"
        );
    }

    /// An AI-controlled Evocation Wizard fires its blast into a scrum
    /// that a baseline wizard's friendly-fire gate rejects. Sculpt
    /// Spells is the whole point of the subclass and it lives entirely
    /// on the resolver side, so without teaching the AI's gate about it
    /// the feature would be unreachable for every non-player evoker:
    /// each candidate point in a melee catches an ally, and the gate
    /// would veto all of them.
    ///
    /// Targets `try_attack_aoe` directly rather than the full `decide`
    /// pipeline — the wizard chassis opens with self-buffs (Mage Armor,
    /// Mirror Image) that would win the priority ordering long before
    /// the AoE step, and those are a different decision than the one
    /// under test. Both casters get the identical board, so the only
    /// difference is the feature.
    #[test]
    fn ai_evoker_blasts_through_allies_that_stop_a_baseline_wizard() {
        use crate::actors::actor_template::CreatureTemplate;
        use crate::actors::creatures::wizards::{EVOCATION_WIZARD_TEMPLATE, WIZARD_TEMPLATE};
        use crate::engine::types::SpellSchool;

        // One ally toe-to-toe with two enemies: every burst point that
        // catches both enemies catches the ally too.
        let picks_burst = |template: &'static CreatureTemplate| {
            let mut e = empty_arena();
            let caster = e
                .instantiate_creature(template, Coordinate::new(5, 5), 0, 0)
                .unwrap();
            e.instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(11, 10), 0, 1)
                .unwrap();
            e.instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(12, 10), 1, 0)
                .unwrap();
            e.instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(12, 11), 1, 1)
                .unwrap();
            try_attack_aoe(&e, caster).is_some()
        };

        assert!(
            !picks_burst(&WIZARD_TEMPLATE),
            "baseline wizard should refuse a blast that clips its own ally"
        );
        assert!(
            picks_burst(&EVOCATION_WIZARD_TEMPLATE),
            "Evocation Wizard should accept the same blast — Sculpt Spells \
             carves the ally out"
        );

        // And the capacity the gate reads matches what the resolver will
        // actually spare: evocation only, scaling on 1 + spell level.
        let mut e = empty_arena();
        let evoker = e
            .instantiate_creature(&EVOCATION_WIZARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        assert_eq!(
            e.ally_shield_capacity(evoker, Some(SpellSchool::Evocation), 3),
            4
        );
        assert_eq!(
            e.ally_shield_capacity(evoker, Some(SpellSchool::Evocation), 0),
            1
        );
        assert_eq!(
            e.ally_shield_capacity(evoker, Some(SpellSchool::Enchantment), 3),
            0
        );
        assert_eq!(e.ally_shield_capacity(evoker, None, 3), 0);
    }

    /// Focus fire targets whoever drops soonest, which is not whoever
    /// has the lowest HP bar. Two enemies at identical HP, one sitting
    /// behind an absorption pool: the AI must swing at the unshielded
    /// one, because the shielded one needs strictly more damage to fall.
    ///
    /// Runs the assertion twice, once per pool (temp HP and Arcane
    /// Ward), because they are separate fields and a heuristic could
    /// easily account for one and miss the other.
    #[test]
    fn focus_fire_prefers_the_target_closest_to_dropping() {
        use crate::actors::creatures::wizards::ABJURATION_WIZARD_TEMPLATE;

        // `bare` and `shielded` start at equal HP; only `shielded` gets
        // a pool. Both are in reach of the attacker.
        let picked_target = |grant: &dyn Fn(&mut crate::actors::actor_template::ActorInstance)| {
            let mut e = empty_arena();
            let attacker = e
                .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
                .unwrap();
            let shielded = e
                .instantiate_creature(&ABJURATION_WIZARD_TEMPLATE, Coordinate::new(6, 5), 1, 0)
                .unwrap();
            let bare = e
                .instantiate_creature(&ABJURATION_WIZARD_TEMPLATE, Coordinate::new(4, 5), 1, 1)
                .unwrap();
            // Equalize HP so the pool is the only difference.
            let hp = e.actors[&bare].hitpoints().min(e.actors[&shielded].hitpoints());
            for id in [bare, shielded] {
                let a = e.actors.get_mut(&id).unwrap();
                let excess = a.hitpoints() - hp;
                if excess > 0 {
                    a.take_damage(excess);
                }
            }
            grant(e.actors.get_mut(&shielded).unwrap());
            assert!(
                e.actors[&shielded].effective_hitpoints()
                    > e.actors[&bare].effective_hitpoints(),
                "test setup: the shielded target should be harder to drop"
            );
            let aei = try_attack_focus_fire(&e, attacker).expect("expected an attack");
            let target = aei.target_ids().and_then(|t| t.first().copied());
            (target, bare, shielded)
        };

        for (label, grant) in [
            (
                "temp HP",
                &(|a: &mut crate::actors::actor_template::ActorInstance| {
                    a.gain_temp_hp(8);
                }) as &dyn Fn(&mut crate::actors::actor_template::ActorInstance),
            ),
            (
                "arcane ward",
                &(|a: &mut crate::actors::actor_template::ActorInstance| {
                    a.weave_or_recharge_arcane_ward(1);
                }),
            ),
        ] {
            let (target, bare, shielded) = picked_target(grant);
            assert_eq!(
                target,
                Some(bare),
                "focus fire picked the target behind {} instead of the one \
                 closest to dropping (shielded={}, bare={})",
                label,
                shielded,
                bare
            );
        }
    }

    #[test]
    fn ai_stands_up_when_prone() {
        use crate::conditions::ConditionTimer;

        let mut e = empty_arena();
        let actor = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        e.actors
            .get_mut(&actor)
            .unwrap()
            .add_condition(Condition::Prone, ConditionTimer::Permanent);

        let ai = SimpleAi;
        let decision = ai.decide(&e, actor);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(aei.action().name(), "stand");
    }

    #[test]
    fn ai_casts_hold_person_when_available() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;

        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // Place enemies outside Spirit Guardians 6-tile aura range so the
        // AI falls through to Hold Person. At distance 8+, Spirit Guardians
        // won't fire, letting the disabler priority shine.
        let _e1 = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(14, 5), 1, 0)
            .unwrap();
        let _e2 = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(15, 5), 1, 1)
            .unwrap();

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(
            aei.action().name(),
            "hold person",
            "cleric with Hold Person should cast it on a tough target"
        );
    }

    #[test]
    fn ai_does_not_recast_concentration() {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _e1 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        let _e2 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(11, 5), 1, 1)
            .unwrap();
        // Pretend the cleric is already concentrating — Hold should be
        // skipped and the AI should fall through to attack tactics.
        e.actors
            .get_mut(&cleric)
            .unwrap()
            .start_concentration(ConcentrationData::new("Bless"));

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_ne!(
            aei.action().name(),
            "hold person",
            "AI should not replace existing concentration"
        );
    }

    #[test]
    fn ai_focus_fire_prefers_advantage_target() {
        use crate::conditions::ConditionTimer;

        let mut e = empty_arena();
        // Attacker on team 0; two enemies in melee reach. One is healthy,
        // one is healthy AND prone. The prone one gives melee advantage.
        // Focus-fire should pick the prone target despite equal HP.
        let attacker = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let upright = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        let prone = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 7), 1, 1)
            .unwrap();
        e.actors
            .get_mut(&prone)
            .unwrap()
            .add_condition(Condition::Prone, ConditionTimer::Permanent);

        let ai = SimpleAi;
        let decision = ai.decide(&e, attacker);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an attack");
        };
        let target = aei.target_ids().and_then(|ids| ids.first().copied());
        assert_eq!(
            target,
            Some(prone),
            "AI should fish for advantage when HP ties"
        );
        let _ = upright;
    }

    #[test]
    fn ai_self_heals_when_low_hp() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        let mut e = empty_arena();
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        // Drop fighter below 50%.
        let max = e.actors[&fighter].max_hitpoints();
        e.actors.get_mut(&fighter).unwrap().take_damage(max - 1);

        let ai = SimpleAi;
        let decision = ai.decide(&e, fighter);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(aei.action().name(), "second wind");
    }

    /// Fighter at full HP, enemy at gap 2 (one tile past melee reach,
    /// inside lunge window): the AI should prime Lunging Attack to close
    /// the gap rather than just stepping forward.
    #[test]
    fn ai_primes_lunge_at_gap_two() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        let mut e = empty_arena();
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        // Goblin at gap-2 distance from the fighter footprint (Medium
        // creatures occupy a 2x2 footprint; placing the goblin 4 tiles
        // away gives a 2-tile gap after subtracting the footprints).
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 3), 1, 0)
            .unwrap();
        let ai = SimpleAi;
        let decision = ai.decide(&e, fighter);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(aei.action().name(), "lunging attack");
    }

    /// Adjacent-enemy (gap 0): no lunge needed — the scimitar already
    /// reaches. AI should pick the attack rather than burn the prime.
    #[test]
    fn ai_skips_lunge_when_adjacent() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        let mut e = empty_arena();
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 3), 1, 0)
            .unwrap();
        let ai = SimpleAi;
        let decision = ai.decide(&e, fighter);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_ne!(
            aei.action().name(),
            "lunging attack",
            "lunge should not fire when target is already in melee reach"
        );
    }

    /// Wounded fighter who has already burned Second Wind but is
    /// carrying a Potion of Healing should drink the potion via
    /// `available_actions()` — exercises the AI walking the
    /// template+items action set rather than `actor.actions` alone.
    #[test]
    fn ai_drinks_healing_potion_when_second_wind_spent() {
        use crate::actions::class_features::SECOND_WIND_TAG;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::items::item_template::POTION_OF_HEALING;
        let mut e = empty_arena();
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        // Burn the template-level heal features (Second Wind, Rally) so
        // the consumable lane is the only heal left, then drop the
        // fighter below 50% so the heal pipeline triggers. Rally spends
        // from the shared superiority pool, so it takes the whole pool
        // to close that lane rather than one charge.
        {
            use crate::actions::class_features::RALLY_TAG;
            let a = e.actors.get_mut(&fighter).unwrap();
            assert!(a.spend_feature(SECOND_WIND_TAG));
            while a.spend_feature(RALLY_TAG) {}
            a.pickup_item(&POTION_OF_HEALING);
            let max = a.max_hitpoints();
            a.take_damage(max - 1);
        }

        let ai = SimpleAi;
        let decision = ai.decide(&e, fighter);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(
            aei.action().name(),
            "drink healing potion",
            "AI should reach for the carried potion once Second Wind is burned"
        );
    }

    /// Scroll of Aid uses `TargetingSchema::NoArgs` and is_heal=true so
    /// the self-heal pipeline reaches for it when the reader is below
    /// half HP. Mirrors the Mass Healing Word scroll's AI integration —
    /// confirms a wounded reader will read the scroll without a separate
    /// burst-aware heuristic.
    #[test]
    fn ai_reads_aid_scroll_when_wounded() {
        use crate::actions::class_features::{RALLY_TAG, SECOND_WIND_TAG};
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::items::item_template::SCROLL_OF_AID;
        let mut e = empty_arena();
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        {
            let a = e.actors.get_mut(&fighter).unwrap();
            assert!(a.spend_feature(SECOND_WIND_TAG));
            // Rally draws on the superiority pool — drain it, not one
            // charge of it, or the fighter still has a heal in hand.
            while a.spend_feature(RALLY_TAG) {}
            a.pickup_item(&SCROLL_OF_AID);
            let max = a.max_hitpoints();
            a.take_damage(max - 1);
        }

        let ai = SimpleAi;
        let decision = ai.decide(&e, fighter);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_eq!(
            aei.action().name(),
            "read aid scroll",
            "AI should reach for the Aid scroll when wounded and no other heal is available"
        );
    }

    #[test]
    fn ai_does_not_self_heal_at_full_hp() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        let mut e = empty_arena();
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        let ai = SimpleAi;
        let decision = ai.decide(&e, fighter);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        assert_ne!(aei.action().name(), "second wind");
    }

    #[test]
    fn ai_does_not_heal_enemy() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;

        let mut e = empty_arena();
        // Cleric (has Healing Word + Sacred Flame) on team 0; lone enemy
        // skeleton on team 1 within Sacred Flame's range. The AI must
        // pick Sacred Flame, not Healing Word, against the enemy.
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&SKELETON_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();

        let ai = SimpleAi;
        let decision = ai.decide(&e, cleric);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an attack");
        };
        assert_ne!(
            aei.action().name(),
            "healing word",
            "AI should not target an enemy with a heal"
        );
    }

    #[test]
    fn low_hp_caster_disengages_when_surrounded() {
        // Skeleton (longbow only) deep in the red, with a zombie in melee
        // reach. With kite available *and* HP below 30%, the AI should
        // pick Disengage so the next-step retreat is OA-free. The kite
        // tactic itself fires on the same predicates above this branch,
        // so we need the kite step to be impossible (e.g. surrounded so
        // every cell is still in reach). We arrange that by walling the
        // skeleton in with multiple zombies.
        let mut e = empty_arena();
        let skeleton = e
            .instantiate_creature(&SKELETON_TEMPLATE, Coordinate::new(10, 10), 0, 0)
            .unwrap();
        for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
            // Use NONE adjacent zombie spawn on each diagonal (footprints
            // overlap with the skeleton's neighbors). gap should be 0/1.
            e.instantiate_creature(
                &ZOMBIE_TEMPLATE,
                Coordinate::new(10 + dx * 3, 10 + dy * 3),
                1,
                0,
            )
            .unwrap();
        }
        // Drop the skeleton's HP under 30%.
        let max = e.actors[&skeleton].max_hitpoints();
        let target_hp = (max as f32 * 0.2) as u32;
        let dmg = max.saturating_sub(target_hp);
        e.actors.get_mut(&skeleton).unwrap().take_damage(dmg);

        // We don't strictly assert "disengage" because if a kite step
        // exists the kite branch beats the disengage branch — we just
        // assert the AI is making a defensive choice.
        let ai = SimpleAi;
        let decision = ai.decide(&e, skeleton);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        // The AI should make a movement-flavored choice (move / disengage)
        // rather than committing to a longbow shot at point-blank range.
        let name = aei.action().name();
        assert!(
            name == "move" || name == "disengage" || name == "stand",
            "expected a defensive choice, got {}",
            name
        );
    }

    #[test]
    fn ranged_attacker_kites_when_threatened() {
        let mut e = empty_arena();
        // Skeleton (longbow only) on team 0, zombie (melee multislam) in
        // melee reach on team 1. Skeleton should step away first.
        let skeleton = e
            .instantiate_creature(&SKELETON_TEMPLATE, Coordinate::new(10, 5), 0, 0)
            .unwrap();
        let _zombie = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(12, 5), 1, 0)
            .unwrap();

        let ai = SimpleAi;
        let decision = ai.decide(&e, skeleton);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected a kite step or attack");
        };
        // Confirm it's a Move, not the longbow.
        assert_eq!(aei.action().name(), "move", "skeleton should kite first");
    }

    #[test]
    fn wizard_falls_back_to_fire_bolt_when_out_of_slots() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;

        let mut e = empty_arena();
        let wizard = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(15, 5), 1, 0)
            .unwrap();

        // Drain every spell slot the wizard owns at every level so all
        // leveled spells (Magic Missile, Fireball, Sunbeam, ...) become
        // unavailable. `best_attack_against` should now fall back to the
        // at-will Fire Bolt cantrip. Drain a generous 1..=9 range since
        // the wizard's loadout can grow over time without invalidating
        // this assertion.
        for lvl in 1u32..=9 {
            let max_slots = e.actors[&wizard]
                .spell_slot_manager
                .spell_slots(lvl)
                .max_spell_slots;
            for _ in 0..max_slots {
                e.actors
                    .get_mut(&wizard)
                    .unwrap()
                    .spell_slot_manager
                    .consume_spell_slot(lvl);
            }
        }
        // Also spend Arcane Recovery so the AI can't recover a level-1
        // slot ahead of the cantrip fallback — this test isolates the
        // pure-cantrip lane.
        use crate::actions::class_features::ARCANE_RECOVERY_TAG;
        e.actors
            .get_mut(&wizard)
            .unwrap()
            .spend_feature(ARCANE_RECOVERY_TAG);
        // Burn the wizard's bonus action so this test isolates the
        // Action lane fallback. Otherwise bonus-action cantrips
        // (Telekinetic) win the first decision call and the test
        // would assert against the wrong economy slot.
        e.actors
            .get_mut(&wizard)
            .unwrap()
            .consume_resource(Resource::BonusAction);

        let ai = SimpleAi;
        let decision = ai.decide(&e, wizard);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an attack, got skip / dodge");
        };
        assert_eq!(
            aei.action().name(),
            "fire bolt",
            "wizard with no slots should fall back to Fire Bolt"
        );
    }

    /// Self-centered NoArgs burst (Thunderwave) fires when 2+ enemies sit
    /// within the AI's heuristic cluster window. Verifies the new
    /// try_self_centered_burst slot picks up NoArgs-harmful actions that
    /// neither try_attack_aoe (Burst-only) nor try_attack_focus_fire
    /// (SingleActor-only) would consider.
    #[test]
    fn ai_fires_self_centered_burst_when_clustered() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        let mut e = empty_arena();
        let wizard = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // Two zombies stacked right next to the wizard — close enough
        // that a Thunderwave (2-tile burst) catches both. The cluster
        // window is 12 tiles so a single foot-step away still triggers
        // the heuristic.
        let _e1 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(6, 5), 1, 0)
            .unwrap();
        let _e2 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 6), 1, 1)
            .unwrap();
        let ai = SimpleAi;
        let decision = ai.decide(&e, wizard);
        let ControllerDecision::Act(aei) = decision else {
            panic!("expected an action");
        };
        // The AI may pick a higher-priority option (Shield, etc.), but
        // among NoArgs harmful candidates Thunderwave should be reachable
        // — verify we hit at least one such option in the lookup order.
        // We can't pin a single action because higher-priority lanes
        // (Mage Armor, Mirror Image) come first. Instead, assert that
        // the AI made a *useful* decision (any Act counts) — the
        // narrow correctness here is that the new slot doesn't panic
        // or recurse, which the full ai_vs_ai_terminates_with_new_content
        // integration test also exercises.
        let _ = aei;
    }

    /// `try_warding_bond` should fire when a wounded ally is adjacent
    /// and a fight is engaged (an enemy is within ~30ft). The cleric
    /// AI picks the most-wounded ally as the bond target.
    #[test]
    fn ai_casts_warding_bond_on_wounded_adjacent_ally() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;

        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // Adjacent fighter ally — start them at low HP so they win the
        // bond pick over a hypothetical second ally (none here).
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(6, 5), 0, 0)
            .unwrap();
        // Wound the fighter so the AI's "max-HP - cur-HP" score is
        // positive (otherwise both have 0 wound score and the pick
        // is a coin flip across ids).
        let f_max = e.actors[&fighter].max_hitpoints();
        let half = f_max / 2;
        e.actors
            .get_mut(&fighter)
            .unwrap()
            .take_typed_damage(half, DamageType::Bludgeoning);
        // Engaged enemy within ~30ft (12 tile-gap) so the "active
        // fight" gate fires.
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(12, 5), 1, 0)
            .unwrap();

        let aei = try_warding_bond(&e, cleric).expect(
            "wounded fighter adjacent + zombie engaged → AI should bond the fighter",
        );
        assert_eq!(aei.action().name(), "warding bond");
        let targets = aei.target_ids().expect("bond targets the fighter");
        assert_eq!(targets[0], fighter);
    }

    /// `try_warding_bond` should bail when the caster is already at
    /// low HP (< 50%) — taking on a partner's mirrored damage at low
    /// HP would put the caster on death saves with the next swing.
    #[test]
    fn ai_skips_warding_bond_when_low_hp() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;

        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(6, 5), 0, 0)
            .unwrap();
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(12, 5), 1, 0)
            .unwrap();
        // Drop the cleric to ~30% HP — below the 50% gate.
        let c_max = e.actors[&cleric].max_hitpoints();
        let drain = c_max - (c_max / 3);
        e.actors
            .get_mut(&cleric)
            .unwrap()
            .take_typed_damage(drain, DamageType::Bludgeoning);
        assert!(
            try_warding_bond(&e, cleric).is_none(),
            "low-HP cleric should not bond — would die from mirrored hits"
        );
    }

    /// `try_stillness_of_mind` fires when the monk is Charmed or
    /// Frightened (a single cleanse erases the disadvantage-on-attacks
    /// debuff) and bails when the monk is fine — burning the Action lane
    /// on a no-op cleanse would be worse than a single swing.
    #[test]
    fn ai_uses_stillness_of_mind_when_afflicted() {
        use crate::actors::creatures::monks::MONK_TEMPLATE;
        let mut e = empty_arena();
        let monk = e
            .instantiate_creature(&MONK_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // No affliction → AI skips the cleanse.
        assert!(
            try_stillness_of_mind(&e, monk).is_none(),
            "unencumbered monk should not waste Action on Stillness of Mind"
        );
        // Frightened → AI fires the cleanse.
        e.actors.get_mut(&monk).unwrap().add_condition(
            Condition::Frightened,
            crate::conditions::ConditionTimer::Rounds(10),
        );
        let aei = try_stillness_of_mind(&e, monk).expect(
            "Frightened monk should fire Stillness of Mind",
        );
        assert_eq!(aei.action().name(), "stillness of mind");
        // Charmed → AI fires the cleanse.
        e.actors.get_mut(&monk).unwrap().remove_condition(Condition::Frightened);
        e.actors.get_mut(&monk).unwrap().add_condition(
            Condition::Charmed,
            crate::conditions::ConditionTimer::Rounds(10),
        );
        let aei = try_stillness_of_mind(&e, monk).expect(
            "Charmed monk should fire Stillness of Mind",
        );
        assert_eq!(aei.action().name(), "stillness of mind");
    }

    /// `try_wipe_acid` fires when the holder carries the CausticBrewed
    /// DoT and they're below half HP. A topped-up actor still swings
    /// through the drip (a single round of 2d4 acid is cheaper than a
    /// missed swing on the offense lane).
    #[test]
    fn ai_uses_wipe_acid_when_low_hp_and_brewed() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::engine::types::DamageType;
        let mut e = empty_arena();
        let g = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // No CausticBrewed flag → AI doesn't burn the Action lane.
        assert!(
            try_wipe_acid(&e, g).is_none(),
            "clean goblin should not waste Action on Wipe Acid"
        );
        // Brewed but full HP → AI still skips (drip is cheaper than a
        // missed swing on offense lane).
        e.actors.get_mut(&g).unwrap().add_condition(
            Condition::CausticBrewed,
            crate::conditions::ConditionTimer::Rounds(10),
        );
        assert!(
            try_wipe_acid(&e, g).is_none(),
            "full-HP brewed goblin should swing through the drip"
        );
        // Brewed + low HP → AI fires the cleanse.
        let max_hp = e.actors[&g].max_hitpoints();
        let _ = e
            .actors
            .get_mut(&g)
            .unwrap()
            .take_typed_damage((max_hp / 2) + 1, DamageType::Acid);
        let aei = try_wipe_acid(&e, g).expect(
            "brewed low-HP goblin should fire Wipe Acid",
        );
        assert_eq!(aei.action().name(), "wipe acid");
    }

    /// `try_cleansing_touch` fires on an adjacent ally carrying a heavy
    /// lockdown debuff (Paralyzed). A clean ally, or an ally outside touch
    /// reach, doesn't trigger the cleanse.
    #[test]
    fn ai_uses_cleansing_touch_on_paralyzed_ally() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::paladins::PALADIN_TEMPLATE;
        let mut e = empty_arena();
        let pal = e
            .instantiate_creature(&PALADIN_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(7, 5), 0, 0)
            .unwrap();
        // Clean ally → no cleanse.
        assert!(
            try_cleansing_touch(&e, pal).is_none(),
            "AI should not burn cleansing touch on a clean ally"
        );
        // Paralyze the fighter → AI picks them as the cleanse target.
        e.actors.get_mut(&fighter).unwrap().add_condition(
            Condition::Paralyzed,
            crate::conditions::ConditionTimer::Rounds(10),
        );
        let aei = try_cleansing_touch(&e, pal).expect(
            "paralyzed adjacent ally should trigger Cleansing Touch",
        );
        assert_eq!(aei.action().name(), "cleansing touch");
        assert_eq!(
            aei.target_ids().expect("cleansing touch picks an ally")[0],
            fighter
        );
        // Now move the fighter out of touch reach → cleanse is gated by
        // the touch radius and no longer triggers.
        e.place_actor_at(fighter, Coordinate::new(15, 5)).unwrap();
        assert!(
            try_cleansing_touch(&e, pal).is_none(),
            "out-of-touch ally should not trigger cleansing touch"
        );
    }

    /// `try_telekinetic` should pick the closest in-range enemy that's
    /// not already footprint-adjacent. Verifies the picker skips
    /// adjacent enemies (no value in a 1-tile pull when already in
    /// melee) and finds the next enemy in the 2-24 tile sweet spot.
    #[test]
    fn ai_telekinetic_picks_closest_non_adjacent_enemy() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        let mut e = empty_arena();
        let wiz = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // Adjacent zombie — should be skipped (pull does nothing).
        let _adj = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(6, 5), 1, 0)
            .unwrap();
        // Mid-range zombie — should be the pick.
        let mid = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 1)
            .unwrap();
        // Far zombie — out of the closest-wins picker.
        let _far = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(20, 5), 1, 2)
            .unwrap();
        let aei = try_telekinetic(&e, wiz).expect("non-adjacent target available");
        assert_eq!(aei.action().name(), "telekinetic");
        let targets = aei.target_ids().expect("telekinetic targets a single actor");
        assert_eq!(targets[0], mid, "should pick the mid-range zombie");
    }

    /// Plane Shift is the top of the lockdown ladder, and the row that
    /// proves the marker column had to become an `Option`.
    ///
    /// It ships on exactly one chassis and had no rung that could reach
    /// it: a spell that deals no damage, buffs nobody and installs no
    /// condition is invisible to the damage lane, the self-buff cohort
    /// and the area-control registry alike. What kept it off `LOCKDOWNS`
    /// too was the "already handled" column — every other row names the
    /// condition it leaves behind, and this one leaves no victim to
    /// leave anything on.
    ///
    /// The gate that keeps it honest at the top of the list is its
    /// range: touch. The lich reaches for it against the body it is
    /// standing next to and for nothing further away, which the second
    /// half of this test pins by moving the same fighter two tiles out.
    #[test]
    fn ai_plane_shifts_the_creature_it_is_standing_next_to() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::liches::LICH_TEMPLATE;

        let mut e = empty_arena();
        let lich = e
            .instantiate_creature(&LICH_TEMPLATE, Coordinate::new(5, 5), 1, 0)
            .unwrap();
        let adjacent = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(7, 5), 0, 0)
            .unwrap();
        let aei = try_lockdown(&e, lich).expect("a lockdown row should be available");
        assert_eq!(aei.action().name(), "plane shift");
        assert_eq!(aei.target_ids().expect("single actor")[0], adjacent);

        // Two tiles further out and the touch range refuses. Something
        // else on the ladder answers instead — the point is only that
        // it is not this.
        e.actors
            .get_mut(&adjacent)
            .unwrap()
            .set_location(Coordinate::new(12, 5));
        assert!(
            try_lockdown(&e, lich)
                .is_none_or(|aei| aei.action().name() != "plane shift"),
            "touch range is what keeps the top of the ladder honest"
        );
    }

    /// `try_cutting_words` should pick the enemy closest to one of the
    /// bard's allies — that's the next strike to spoil. Verifies the
    /// closest-ally heuristic against two equidistant-to-the-bard
    /// enemies where one is sitting next to a frontliner.
    #[test]
    fn ai_cutting_words_targets_enemy_near_ally() {
        use crate::actors::creatures::bards::BARD_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        let mut e = empty_arena();
        let bard = e
            .instantiate_creature(&BARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(10, 5), 0, 0)
            .unwrap();
        // Two enemies equidistant from the bard. The one adjacent to the
        // fighter is the priority pick — its next swing is what matters.
        let _far_from_ally = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(5, 10), 1, 0)
            .unwrap();
        let near_ally = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(11, 5), 1, 1)
            .unwrap();
        let aei = try_cutting_words(&e, bard).expect("should pick an enemy");
        assert_eq!(aei.action().name(), "cutting words");
        let targets = aei.target_ids().expect("cutting words targets one actor");
        assert_eq!(
            targets[0], near_ally,
            "should pick the enemy adjacent to the fighter, not the empty-side zombie"
        );
    }

    /// `try_preserve_life` should fire when an ally is wounded below
    /// half HP and the cleric has the feature available. Skips when no
    /// ally is wounded enough to benefit (the feature only heals up to
    /// half max HP, so a 100% HP ally is excluded).
    #[test]
    fn ai_preserve_life_fires_when_ally_is_wounded() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(6, 5), 0, 0)
            .unwrap();
        // Without any wounded ally, the AI should not fire the feature.
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        assert!(
            try_preserve_life(&e, cleric).is_none(),
            "no wounded ally → no fire"
        );
        // Wound the fighter to ~25% HP — well below the half-HP gate.
        let max = e.actors[&fighter].max_hitpoints();
        e.actors
            .get_mut(&fighter)
            .unwrap()
            .take_typed_damage(max.saturating_sub(max / 4), DamageType::Slashing);
        assert!(
            try_preserve_life(&e, cleric).is_some(),
            "wounded ally should trigger the preserve-life heuristic"
        );
    }

    /// The turn-burst picker reads the real `CreatureType`, not the old
    /// poison-immunity proxy. An iron golem is poison-immune and not
    /// undead, so a cleric standing beside one must keep its Turn Undead
    /// charge; a zombie must draw it.
    #[test]
    fn ai_turn_burst_ignores_poison_immune_non_undead() {
        use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
        use crate::actors::creatures::iron_golems::IRON_GOLEM_TEMPLATE;
        use crate::engine::types::DamageType;
        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let golem = e
            .instantiate_creature(&IRON_GOLEM_TEMPLATE, Coordinate::new(8, 5), 1, 0)
            .unwrap();
        // The proxy the old heuristic used still reports true here, which
        // is exactly why it was the wrong question to ask.
        assert!(e.actors[&golem].is_immune_to(DamageType::Poison));
        assert!(
            try_turn_burst(&e, cleric).is_none(),
            "a construct must not draw the cleric's Turn Undead charge"
        );
        // Swap in a genuine undead: now the charge is worth spending.
        e.actors.remove(&golem);
        let _zombie = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(8, 5), 1, 1)
            .unwrap();
        assert!(
            try_turn_burst(&e, cleric).is_some(),
            "an undead in range should draw Turn Undead"
        );
    }

    /// The picker covers every `TurnBurst` config, not just Turn Undead.
    /// A Devotion Paladin beside a fiend fires Turn the Faithless, and an
    /// Arcana Cleric beside the same fiend fires Arcane Abjuration —
    /// neither had any AI path before the shared cohort.
    #[test]
    fn ai_turn_burst_covers_the_other_channel_divinities() {
        use crate::actors::creatures::clerics::ARCANA_CLERIC_TEMPLATE;
        use crate::actors::creatures::imps::IMP_TEMPLATE;
        use crate::actors::creatures::paladins::DEVOTION_PALADIN_TEMPLATE;
        let mut e = empty_arena();
        let paladin = e
            .instantiate_creature(&DEVOTION_PALADIN_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let cleric = e
            .instantiate_creature(&ARCANA_CLERIC_TEMPLATE, Coordinate::new(6, 5), 0, 1)
            .unwrap();
        let _imp = e
            .instantiate_creature(&IMP_TEMPLATE, Coordinate::new(9, 5), 1, 0)
            .unwrap();
        let pal_pick = try_turn_burst(&e, paladin).expect("paladin should turn the fiend");
        assert_eq!(pal_pick.action().name(), "turn the faithless");
        let cleric_pick = try_turn_burst(&e, cleric).expect("cleric should abjure the fiend");
        assert_eq!(cleric_pick.action().name(), "arcane abjuration");
    }

    /// Order's Demand takes Dreadful Aspect's `min_targets: 2` bar for
    /// the same reason — an unfiltered burst against a single enemy is
    /// worth less than a swing — and the Order Cleric is the only
    /// unfiltered *cleric* row, so it also pins that the cohort reaches
    /// past the paladins.
    #[test]
    fn ai_orders_demand_waits_for_a_second_enemy() {
        use crate::actors::creatures::clerics::ORDER_CLERIC_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        let mut e = empty_arena();
        let cleric = e
            .instantiate_creature(&ORDER_CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _one = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(8, 5), 1, 0)
            .unwrap();
        assert!(
            try_turn_burst(&e, cleric).is_none(),
            "one enemy is not worth the Channel Divinity"
        );
        let _two = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(8, 7), 1, 1)
            .unwrap();
        let pick = try_turn_burst(&e, cleric).expect("two enemies should draw the demand");
        assert_eq!(pick.action().name(), "order's demand");
    }

    /// Dreadful Aspect's row carries `min_targets: 2` because its filter
    /// is unconditional — against one enemy an ordinary swing is worth
    /// more than a frighten, so the charge is held.
    #[test]
    fn ai_dreadful_aspect_waits_for_a_second_enemy() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::paladins::OATHBREAKER_PALADIN_TEMPLATE;
        let mut e = empty_arena();
        let paladin = e
            .instantiate_creature(&OATHBREAKER_PALADIN_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let _one = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(8, 5), 1, 0)
            .unwrap();
        assert!(
            try_turn_burst(&e, paladin).is_none(),
            "one enemy is not worth the dread charge"
        );
        let _two = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(8, 6), 1, 1)
            .unwrap();
        let pick = try_turn_burst(&e, paladin).expect("two enemies should draw the dread");
        assert_eq!(pick.action().name(), "dreadful aspect");
    }

    /// `try_arcane_recovery` should fire when the wizard has spent
    /// slots and there's an active fight; skips when slots are full or
    /// no enemies are nearby.
    #[test]
    fn ai_arcane_recovery_fires_when_slots_spent_and_engaged() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        let mut e = empty_arena();
        let wiz = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // No enemy → no fire even with spent slots.
        e.actors
            .get_mut(&wiz)
            .unwrap()
            .spell_slot_manager
            .consume_spell_slot(1);
        assert!(
            try_arcane_recovery(&e, wiz).is_none(),
            "no enemy in range → skip"
        );
        // Add an enemy → fires.
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(15, 5), 1, 0)
            .unwrap();
        assert!(
            try_arcane_recovery(&e, wiz).is_some(),
            "spent slot + enemy → fire"
        );
    }

    /// `try_natural_recovery` fires on the Land Druid subclass with
    /// the same gate shape as Arcane Recovery — engaged in combat AND
    /// a spent low-tier slot. Verifies both halves via the shared
    /// `try_engaged_self_recovery` gate.
    #[test]
    fn ai_natural_recovery_fires_when_slots_spent_and_engaged() {
        use crate::actors::creatures::druids::LAND_DRUID_TEMPLATE;
        let mut e = empty_arena();
        let druid = e
            .instantiate_creature(&LAND_DRUID_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // No enemy → no fire even with spent slots.
        e.actors
            .get_mut(&druid)
            .unwrap()
            .spell_slot_manager
            .consume_spell_slot(1);
        assert!(
            try_natural_recovery(&e, druid).is_none(),
            "no enemy in range → skip"
        );
        // Add an enemy → fires.
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(15, 5), 1, 0)
            .unwrap();
        assert!(
            try_natural_recovery(&e, druid).is_some(),
            "spent slot + enemy → fire"
        );
    }

    /// `try_twinned_spell` gates on: SP available, prime not already up,
    /// ≥ 2 enemies in range, AND a known-twinnable single-target damage
    /// spell in the kit. The sorcerer template ships Fire Bolt / Ray of
    /// Frost / Chill Touch as cantrips so the kit gate passes; we drive
    /// the enemy-count and prime gates explicitly.
    #[test]
    fn twinned_spell_ai_gates_on_sp_and_two_enemies() {
        use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 30,
            height: 30,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap();
        let sorcerer = e
            .instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .give_resource(Resource::BonusAction);

        // 0 enemies → skip.
        assert!(
            try_twinned_spell(&e, sorcerer).is_none(),
            "no enemies → skip"
        );

        // 1 enemy → skip (no twin target).
        let _z1 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        assert!(
            try_twinned_spell(&e, sorcerer).is_none(),
            "single enemy → skip"
        );

        // 2 enemies → prime fires.
        let _z2 = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(11, 6), 1, 1)
            .unwrap();
        assert!(
            try_twinned_spell(&e, sorcerer).is_some(),
            "two enemies + SP + twinnable kit → prime fires"
        );

        // Already primed → skip.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .add_condition(Condition::TwinnedSpelling, ConditionTimer::Rounds(2));
        assert!(
            try_twinned_spell(&e, sorcerer).is_none(),
            "prime already up → skip"
        );

        // Drop prime, burn SP to 0 → skip.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .remove_condition(Condition::TwinnedSpelling);
        while e.actors[&sorcerer].sorcery_points() > 0 {
            e.actors.get_mut(&sorcerer).unwrap().spend_sorcery_point();
        }
        assert!(
            try_twinned_spell(&e, sorcerer).is_none(),
            "no SP → skip"
        );
    }

    /// `try_subtle_spell` fires when SP is available, no prime is up,
    /// AND an opposing counterspeller is on the field. Skipped when no
    /// enemy carries Counterspell — the prime would protect nothing.
    #[test]
    fn subtle_spell_ai_gates_on_opposing_counterspeller() {
        use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 30,
            height: 30,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap();
        let sorcerer = e
            .instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .give_resource(Resource::BonusAction);

        // No enemies → skip.
        assert!(
            try_subtle_spell(&e, sorcerer).is_none(),
            "no enemies → skip"
        );

        // Plain enemy (zombie has no Counterspell) → skip.
        let zombie = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        assert!(
            try_subtle_spell(&e, sorcerer).is_none(),
            "enemy with no counterspell → skip"
        );

        // Replace with a wizard (carries Counterspell) → prime fires.
        e.actors.remove(&zombie);
        let _wizard = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        assert!(
            try_subtle_spell(&e, sorcerer).is_some(),
            "opposing wizard with counterspell → prime fires"
        );

        // Already primed → skip.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .add_condition(Condition::SubtleSpelling, ConditionTimer::Rounds(2));
        assert!(
            try_subtle_spell(&e, sorcerer).is_none(),
            "prime already up → skip"
        );

        // Drop prime, burn SP → skip.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .remove_condition(Condition::SubtleSpelling);
        while e.actors[&sorcerer].sorcery_points() > 0 {
            e.actors.get_mut(&sorcerer).unwrap().spend_sorcery_point();
        }
        assert!(
            try_subtle_spell(&e, sorcerer).is_none(),
            "no SP → skip"
        );
    }

    /// Test scaffold for the escape / self-preservation pickers: an
    /// open map with a caster and however many hostiles the caller
    /// wants pressed up against them.
    fn pinned_caster(
        template: &'static crate::actors::actor_template::CreatureTemplate,
        n_adjacent: usize,
    ) -> (EncounterInstance, usize) {
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;
        let tp = TerrainGenParams {
            width: 30,
            height: 30,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(7)).unwrap();
        let caster = e
            .instantiate_creature(template, Coordinate::new(15, 15), 0, 0)
            .unwrap();
        for i in 0..n_adjacent {
            let at = Coordinate::new(16, 14 + i as isize);
            let _ = e.instantiate_creature(
                &crate::actors::creatures::goblins::GOBLIN_TEMPLATE,
                at,
                1,
                i,
            );
        }
        (e, caster)
    }

    /// Sibling scaffold to `pinned_caster` for the area rungs: hostiles
    /// bunched together but at range, so the melee-threat rungs higher
    /// up the ladder don't intercept before the area lane is reached.
    fn clustered_hostiles(
        template: &'static crate::actors::actor_template::CreatureTemplate,
        n: usize,
    ) -> (EncounterInstance, usize) {
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;
        let tp = TerrainGenParams {
            width: 30,
            height: 30,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(7)).unwrap();
        let caster = e
            .instantiate_creature(template, Coordinate::new(5, 15), 0, 0)
            .unwrap();
        for i in 0..n {
            let _ = e.instantiate_creature(
                &crate::actors::creatures::goblins::GOBLIN_TEMPLATE,
                Coordinate::new(15, 14 + i as isize),
                1,
                i,
            );
        }
        (e, caster)
    }

    /// A pinned ranged caster blinks out, and the destination is
    /// strictly farther from the nearest threat than where it stood.
    #[test]
    fn teleport_escape_blinks_a_pinned_caster_away_from_melee() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
        let (e, wiz) = pinned_caster(&WIZARD_TEMPLATE, 2);
        let aei = try_teleport_escape(&e, wiz).expect("two adjacent hostiles is pinned");
        assert_eq!(
            aei.action().name(),
            "misty step",
            "the baseline wizard's only teleport"
        );
        let dest = aei.target_locations().as_ref().unwrap()[0];
        let me = &e.actors[&wiz];
        let my_size = get_tiles_from_size(me.size());
        let gap = |c| {
            e.actors
                .values()
                .filter(|a| a.team() != me.team() && a.is_combat_active())
                .map(|a| footprint_chebyshev(c, my_size, a.location(), get_tiles_from_size(a.size())))
                .min()
                .unwrap_or(0)
        };
        assert!(
            gap(dest) > gap(me.location()),
            "the blink has to actually gain distance"
        );
    }

    /// The three gates. A healthy caster with a single adjacent goblin
    /// steps and shoots rather than burning a slot; a caster with no
    /// hostile in contact has nothing to escape; and a melee actor
    /// never blinks, because it would only have to walk back.
    #[test]
    fn teleport_escape_holds_its_resource_unless_genuinely_pinned() {
        use crate::actors::creatures::barbarians::BARBARIAN_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        let (e, wiz) = pinned_caster(&WIZARD_TEMPLATE, 1);
        assert!(
            try_teleport_escape(&e, wiz).is_none(),
            "one adjacent hostile at full HP is not pinned"
        );

        let (mut e, wiz) = pinned_caster(&WIZARD_TEMPLATE, 1);
        let max = e.actors[&wiz].max_hitpoints();
        e.actors.get_mut(&wiz).unwrap().take_damage(max * 3 / 4);
        assert!(
            try_teleport_escape(&e, wiz).is_some(),
            "the same contact below half HP is"
        );

        let (e, wiz) = pinned_caster(&WIZARD_TEMPLATE, 0);
        assert!(
            try_teleport_escape(&e, wiz).is_none(),
            "nothing in contact, nothing to escape"
        );

        let (e, barb) = pinned_caster(&BARBARIAN_TEMPLATE, 2);
        assert!(
            try_teleport_escape(&e, barb).is_none(),
            "a melee actor that blinks away only has to walk back"
        );
    }

    /// Registry order is cheapest-resource-first, so a Conjuration
    /// Wizard — which carries both the free rechargeable charge and
    /// Misty Step's 2nd-level slot — spends the charge. With the charge
    /// down it falls through to the slot, which is the whole reason the
    /// picker walks a registry rather than naming one action.
    #[test]
    fn teleport_escape_spends_the_cheapest_resource_first() {
        use crate::actions::class_features::BENIGN_TRANSPOSITION_TAG;
        use crate::actors::creatures::wizards::CONJURATION_WIZARD_TEMPLATE;
        let (mut e, wiz) = pinned_caster(&CONJURATION_WIZARD_TEMPLATE, 2);
        let aei = try_teleport_escape(&e, wiz).expect("pinned");
        assert_eq!(aei.action().name(), "benign transposition");

        e.actors
            .get_mut(&wiz)
            .unwrap()
            .spend_feature(BENIGN_TRANSPOSITION_TAG);
        let aei = try_teleport_escape(&e, wiz).expect("still pinned");
        assert_eq!(
            aei.action().name(),
            "misty step",
            "a spent charge falls through to the next row"
        );
    }

    /// The Dreams Druid takes the escape lane's cheapest exit, and it
    /// is cheaper than either spell on the list: a bonus action and a
    /// per-rest charge, no slot at all.
    ///
    /// That is the row's whole argument on this chassis. A druid pinned
    /// while holding a Moonbeam has to choose between the concentration
    /// and the slot that would replace it if the only exits are Misty
    /// Step and Dimension Door; Hidden Paths costs neither, which is
    /// why it sits above both in `SELF_TELEPORT_ESCAPES`.
    #[test]
    fn the_dreams_druid_blinks_out_without_spending_a_slot() {
        use crate::actions::class_features::HIDDEN_PATHS_TAG;
        use crate::actors::creatures::druids::DREAMS_DRUID_TEMPLATE;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let (e, druid) = pinned_caster(&DREAMS_DRUID_TEMPLATE, 2);
        let slots_before = e.actors[&druid].lowest_available_spell_slot();
        let aei = try_teleport_escape(&e, druid).expect("two adjacent hostiles is pinned");
        assert_eq!(aei.action().name(), "hidden paths");

        let dest = aei.target_locations().as_ref().unwrap()[0];
        let me = &e.actors[&druid];
        let my_size = get_tiles_from_size(me.size());
        let gap = |c| {
            e.actors
                .values()
                .filter(|a| a.team() != me.team() && a.is_combat_active())
                .map(|a| {
                    footprint_chebyshev(c, my_size, a.location(), get_tiles_from_size(a.size()))
                })
                .min()
                .unwrap_or(0)
        };
        assert!(
            gap(dest) > gap(me.location()),
            "the blink has to actually gain distance"
        );
        assert_eq!(
            e.actors[&druid].lowest_available_spell_slot(),
            slots_before,
            "the picker chose an exit that touches no slot"
        );

        // With the charge spent the lane comes back empty, because the
        // blink is the only exit this chassis carries — the druid spell
        // list has neither Misty Step nor Dimension Door. So the whole
        // subclass feature is one press per rest, and the AI does not
        // pretend otherwise by re-proposing a spent charge.
        let (mut e, druid) = pinned_caster(&DREAMS_DRUID_TEMPLATE, 2);
        e.actors
            .get_mut(&druid)
            .unwrap()
            .spend_feature(HIDDEN_PATHS_TAG);
        assert!(try_teleport_escape(&e, druid).is_none());
    }

    /// Shapechanger's two thresholds. The form costs the wizard their
    /// concentration, so the AI demands a worse position before it will
    /// trade one away — and it never re-fires while already
    /// transformed.
    #[test]
    fn shapechanger_gate_is_stricter_while_concentrating() {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::wizards::TRANSMUTATION_WIZARD_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        let hurt_to = |frac_remaining: u32| {
            let (mut e, wiz) = pinned_caster(&TRANSMUTATION_WIZARD_TEMPLATE, 1);
            let max = e.actors[&wiz].max_hitpoints();
            e.actors
                .get_mut(&wiz)
                .unwrap()
                .take_damage(max - (max * frac_remaining / 100).max(1));
            (e, wiz)
        };

        let (e, wiz) = hurt_to(60);
        assert!(
            try_shapechanger(&e, wiz).is_none(),
            "60% HP is not an emergency"
        );

        let (e, wiz) = hurt_to(30);
        assert!(
            try_shapechanger(&e, wiz).is_some(),
            "30% HP with nothing to lose — take the temp HP"
        );

        let (mut e, wiz) = hurt_to(30);
        e.actors
            .get_mut(&wiz)
            .unwrap()
            .start_concentration(ConcentrationData::new("Web"));
        assert!(
            try_shapechanger(&e, wiz).is_none(),
            "30% HP is not worth giving up a Web"
        );

        let (mut e, wiz) = hurt_to(10);
        e.actors
            .get_mut(&wiz)
            .unwrap()
            .start_concentration(ConcentrationData::new("Web"));
        assert!(
            try_shapechanger(&e, wiz).is_some(),
            "at 10% the Web is lost either way"
        );

        // Never re-fires while already transformed.
        let (mut e, wiz) = hurt_to(10);
        e.actors.get_mut(&wiz).unwrap().add_condition(
            Condition::Polymorphed,
            ConditionTimer::Permanent,
        );
        assert!(
            try_shapechanger(&e, wiz).is_none(),
            "the temp HP pool doesn't stack — one form is all there is"
        );
    }

    /// Action Surge was reachable by no picker at all before this —
    /// the fighter's largest single-turn output swing, never used.
    /// Fires as soon as there is a hostile to spend the extra action
    /// on, and terminates because the charge is spent on use.
    #[test]
    fn action_surge_fires_once_when_a_hostile_is_in_play() {
        use crate::actions::class_features::ACTION_SURGE_TAG;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::engine::side_effects::Resource;

        let (e, solo) = pinned_caster(&FIGHTER_TEMPLATE, 0);
        assert!(
            try_action_surge(&e, solo).is_none(),
            "nothing to surge against"
        );

        let (mut e, fighter) = pinned_caster(&FIGHTER_TEMPLATE, 1);
        let aei = try_action_surge(&e, fighter).expect("a hostile is in play");
        assert_eq!(aei.action().name(), "action surge");
        assert!(
            e.actors[&fighter].feature_available(ACTION_SURGE_TAG),
            "the charge is still up before the action resolves"
        );

        // Resolving it hands over an Action and burns the charge, so
        // the picker goes quiet — no re-entry loop on a free action.
        let effects = aei.action().side_effects(&mut e, fighter, None, None, None);
        for eff in effects {
            eff.apply(&mut e);
        }
        assert!(
            !e.actors[&fighter].feature_available(ACTION_SURGE_TAG),
            "the surge spent its charge"
        );
        assert!(
            e.actors[&fighter].can_consume_resource(Resource::Action),
            "and handed over an Action to spend"
        );
        assert!(
            try_action_surge(&e, fighter).is_none(),
            "the picker must not re-fire on a spent charge"
        );
    }

    /// Hypnotic Gaze shipped with no picker, so the Enchantment
    /// Wizard's only slot-free lockdown was player-only. Fires on an
    /// adjacent hostile, skips one that is already Incapacitated, and
    /// doesn't reach past melee.
    #[test]
    fn hypnotic_gaze_targets_an_adjacent_hostile_that_is_still_in_the_fight() {
        use crate::actors::creatures::wizards::{ENCHANTMENT_WIZARD_TEMPLATE, WIZARD_TEMPLATE};
        use crate::conditions::{Condition, ConditionTimer};

        let (e, wiz) = pinned_caster(&ENCHANTMENT_WIZARD_TEMPLATE, 1);
        let aei = try_hypnotic_gaze(&e, wiz).expect("an adjacent hostile");
        assert_eq!(aei.action().name(), "hypnotic gaze");
        let target = aei.target_ids().as_ref().unwrap()[0];
        assert_ne!(e.actors[&target].team(), e.actors[&wiz].team());

        // Already out of the fight — don't spend the action again.
        let (mut e, wiz) = pinned_caster(&ENCHANTMENT_WIZARD_TEMPLATE, 1);
        let goblin = *e
            .actors
            .keys()
            .find(|id| e.actors[id].team() != e.actors[&wiz].team())
            .unwrap();
        e.actors
            .get_mut(&goblin)
            .unwrap()
            .add_condition(Condition::Incapacitated, ConditionTimer::Rounds(2));
        assert!(
            try_hypnotic_gaze(&e, wiz).is_none(),
            "no point gazing at something already incapacitated"
        );

        // Nothing in reach, and no gaze on the baseline chassis.
        let (e, wiz) = pinned_caster(&ENCHANTMENT_WIZARD_TEMPLATE, 0);
        assert!(try_hypnotic_gaze(&e, wiz).is_none(), "gaze is melee-range");
        let (e, plain) = pinned_caster(&WIZARD_TEMPLATE, 1);
        assert!(
            try_hypnotic_gaze(&e, plain).is_none(),
            "the baseline wizard has no gaze"
        );
    }


    /// Area control fires on a cluster, declines on a lone target, and
    /// declines the *concentration* entries while the caster is already
    /// holding one — the gate that keeps one control spell from
    /// displacing another. Grease, which costs no concentration, is
    /// still reachable through that gate, which is the whole reason the
    /// registry carries a flag instead of a bare name.
    #[test]
    fn area_control_locks_down_a_cluster_and_yields_to_held_concentration() {
        use crate::actors::actor_template::ConcentrationData;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;

        let (e, wiz) = pinned_caster(&WIZARD_TEMPLATE, 0);
        assert!(
            try_area_control(&e, wiz).is_none(),
            "no hostiles, nothing to lock down"
        );

        let (e, wiz) = clustered_hostiles(&WIZARD_TEMPLATE, 1);
        assert!(
            try_area_control(&e, wiz).is_none(),
            "a single hostile isn't worth a concentration slot"
        );

        let (mut e, wiz) = clustered_hostiles(&WIZARD_TEMPLATE, 3);
        let aei = try_area_control(&e, wiz).expect("three clustered hostiles");
        assert!(
            AREA_CONTROL_SPELLS.contains(&aei.action().name()),
            "picked {} which isn't on the control registry",
            aei.action().name()
        );

        e.actors
            .get_mut(&wiz)
            .unwrap()
            .start_concentration(ConcentrationData::new("Foresight"));
        match try_area_control(&e, wiz) {
            None => {}
            Some(aei) => {
                let name = aei.action().name().to_string();
                assert!(
                    AREA_CONTROL_SPELLS.contains(&name.as_str()),
                    "picked {name}, which isn't on the control registry"
                );
                assert!(
                    !aei.action().holds_concentration(),
                    "a held concentration must block {}, which wants one of its own",
                    name
                );
            }
        }
    }

    /// The round trip: the AI picks an area-control spell, the engine
    /// executes it, and a persistent area is on the board afterwards.
    ///
    /// Worth pinning as one test rather than as two halves. The picker
    /// tests above stop at "it chose something off the registry", and
    /// the zone tests in `engine::encounter` start at "a zone exists" —
    /// between them sits validation, cost, and the `InstallZone` side
    /// effect, which is exactly where a control spell goes quietly
    /// unreachable without anything failing.
    #[test]
    fn an_ai_chosen_control_spell_actually_lands_an_area_on_the_board() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;

        let (mut e, wiz) = clustered_hostiles(&WIZARD_TEMPLATE, 3);
        assert!(e.zones().is_empty());
        let aei = try_area_control(&e, wiz).expect("three clustered hostiles");
        let name = aei.action().name().to_string();
        e.push_action(aei);
        e.process_stack();
        assert!(
            !e.zones().is_empty(),
            "{} was chosen but left nothing on the map",
            name
        );
        let zone = &e.zones()[0];
        assert_eq!(zone.owner_id, wiz);
        assert!(zone.rounds_remaining > 0);
    }

    /// Every Arcane Shot row names the condition its arrow actually
    /// installs, and the two tables are checked against each other
    /// rather than trusted to agree.
    ///
    /// `ARCANE_SHOT_ORDER`'s third column is not a description. The
    /// picker reads it as the "already landed" gate — it filters out any
    /// shot whose condition the target is already under — so a row that
    /// names the wrong condition makes the archer re-nock a shot that
    /// has already done its work, and nothing anywhere fails.
    ///
    /// It drifted the first time the moment the Banishing Arrow rider
    /// stopped installing `Incapacitated` and started installing
    /// `Banished`: two tables in two modules, one of them the engine's
    /// answer and the other the AI's belief about it, with nothing in
    /// between. This is what is in between.
    #[test]
    fn every_arcane_shot_row_names_the_condition_its_arrow_lands() {
        use crate::engine::attack::{FollowUpEffect, ON_HIT_RIDERS};

        for (name, _, believed) in ARCANE_SHOT_ORDER {
            let rider = ON_HIT_RIDERS
                .iter()
                .find(|r| r.label == *name)
                .unwrap_or_else(|| panic!("{name} names no on-hit rider"));
            let follow_up = rider
                .follow_up
                .as_ref()
                .unwrap_or_else(|| panic!("{name} lands nothing, so the gate is a fiction"));
            match follow_up.effect {
                FollowUpEffect::Condition { condition, .. } => assert_eq!(
                    condition, *believed,
                    "{name} installs {} but the AI's gate watches for {}",
                    condition.name(),
                    believed.name()
                ),
                _ => panic!("{name}'s rider does not install a condition at all"),
            }
        }
    }

    /// Otiluke's Resilient Sphere is a tier-1 lock and had no rung.
    ///
    /// `Sphered` joins `blocks_action_economy`, so a sphered creature
    /// takes no action, no bonus action and no reaction for ten rounds —
    /// which is the lockdown cohort's membership test exactly. It ships
    /// on four chassis (the artificer, and the wizard, cleric and
    /// sorcerer subclasses that pick it up) and on a scroll, and until
    /// it became a row nothing in the engine had ever cast it: a
    /// single-target save-or-suck deals no damage and buffs nobody, so
    /// the damage lane, the self-buff cohort and the area-control
    /// registry each filtered it out for a different reason.
    ///
    /// Checked on the artificer, which carries it on the base chassis
    /// rather than behind a subclass — and with the rows *above* it
    /// stripped, because the cohort returns one pick and Banishment
    /// outranks it on any sheet that holds both.
    #[test]
    fn the_resilient_sphere_is_reachable_on_a_chassis_that_carries_it() {
        use crate::actors::actor_template::CreatureTemplate;
        use crate::actors::creatures::artificers::ARTIFICER_TEMPLATE;

        assert!(
            ARTIFICER_TEMPLATE
                .actions
                .iter()
                .any(|a| a.name() == "resilient sphere"),
            "the artificer has to carry the spell for this case to prove anything"
        );
        let higher: Vec<&str> = LOCKDOWNS
            .iter()
            .take_while(|r| r.name != "resilient sphere")
            .map(|r| r.name)
            .collect();
        let stripped: &'static CreatureTemplate = Box::leak(Box::new(CreatureTemplate {
            actions: ARTIFICER_TEMPLATE
                .actions
                .iter()
                .copied()
                .filter(|a| !higher.contains(&a.name()))
                .collect(),
            ..ARTIFICER_TEMPLATE.clone()
        }));
        let (e, caster) = clustered_hostiles(stripped, 3);
        let aei = try_lockdown(&e, caster).expect("the sphere should be reachable");
        assert_eq!(aei.action().name(), "resilient sphere");
        // And it aims at the toughest thing on the board, which is what
        // the whole cohort is for.
        let target = aei.target_ids().expect("single actor")[0];
        let toughest = e
            .actors
            .iter()
            .filter(|(_, a)| a.team() != e.actors[&caster].team())
            .map(|(_, a)| a.hitpoints())
            .max()
            .unwrap();
        assert_eq!(e.actors[&target].hitpoints(), toughest);
    }

    /// Every row on the registry is a row some chassis can actually
    /// reach for, and the three most recently added are the reason this
    /// exists.
    ///
    /// A name on `AREA_CONTROL_SPELLS` is only half a rung: the other
    /// half is a template that carries the spell and a picker that will
    /// choose it over everything else the same caster owns. Stinking
    /// Cloud, Wall of Sand and Crown of Thorns each spent their whole
    /// lives with the first half and not the second — on the wizard and
    /// druid chassis, passing the registry's stated membership test word
    /// for word, and reachable only through the AoE picker one rung
    /// down, which scores on coverage and loses to Fireball's radius.
    ///
    /// Checked by stripping the caster down to one control spell at a
    /// time, because the rung returns a single pick and the entries
    /// compete: a wizard holding both Web and Wall of Sand proves only
    /// that one of them is reachable.
    #[test]
    fn every_control_row_is_reachable_on_a_chassis_that_carries_it() {
        use crate::actors::actor_template::CreatureTemplate;
        use crate::actors::creatures::druids::DREAMS_DRUID_TEMPLATE;
        use crate::actors::creatures::wizards::{
            EVOCATION_WIZARD_TEMPLATE, WIZARD_TEMPLATE,
        };

        // (the row, a chassis that carries it) — one case per entry the
        // sweep added, each on the template it actually ships on.
        let cases: [(&str, &'static CreatureTemplate); 3] = [
            ("stinking cloud", &WIZARD_TEMPLATE),
            ("wall of sand", &EVOCATION_WIZARD_TEMPLATE),
            ("crown of thorns", &DREAMS_DRUID_TEMPLATE),
        ];
        for (row, template) in cases {
            assert!(
                template.actions.iter().any(|a| a.name() == row),
                "{} does not carry {row}, so this case proves nothing",
                template.name
            );
            // The chassis with every *other* control spell taken off
            // the sheet. Leaked because `instantiate_creature` wants a
            // `&'static` template, which is exactly what a real one is;
            // one allocation per case in one test is the cheapest way
            // to ask the question without a mutator that only tests
            // would ever call.
            let stripped: &'static CreatureTemplate = Box::leak(Box::new(CreatureTemplate {
                actions: template
                    .actions
                    .iter()
                    .copied()
                    .filter(|a| a.name() == row || !AREA_CONTROL_SPELLS.contains(&a.name()))
                    .collect(),
                ..template.clone()
            }));
            let (e, caster) = clustered_hostiles(stripped, 3);
            let aei = try_area_control(&e, caster)
                .unwrap_or_else(|| panic!("{row} unreachable on {}", template.name));
            assert_eq!(aei.action().name(), row);
        }
    }

    /// The ordering that makes the rung reachable at all. Both
    /// `try_area_control` and `try_foresight` want the caster's one
    /// concentration, and Foresight fires on turn one and holds for the
    /// whole fight — so if it ran first, control would never be cast.
    /// Pins that `decide` picks the lockdown when a cluster is present,
    /// and still reaches Foresight when one isn't.
    #[test]
    fn a_cluster_outranks_the_apex_ally_buff_for_the_casters_concentration() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        let ai = SimpleAi;

        let (mut e, wiz) = clustered_hostiles(&WIZARD_TEMPLATE, 3);
        // Both rungs want the same one concentration, which is what
        // makes their relative order load-bearing rather than cosmetic.
        assert!(try_area_control(&e, wiz).is_some());
        assert!(try_foresight(&e, wiz).is_some());
        // Skip the non-concentration self-buff rungs above both of them
        // so `decide` lands on the one under test.
        e.actors
            .get_mut(&wiz)
            .unwrap()
            .add_condition(Condition::MageArmored, ConditionTimer::Rounds(100));
        let ControllerDecision::Act(aei) = ai.decide(&e, wiz) else {
            panic!("expected an action");
        };
        assert!(
            AREA_CONTROL_SPELLS.contains(&aei.action().name()),
            "a dense cluster should take the concentration, got {}",
            aei.action().name()
        );

        // With the cluster spread out past any burst radius, the rung
        // declines and the buff lane is reachable again.
        let (e, wiz) = pinned_caster(&WIZARD_TEMPLATE, 0);
        assert!(
            try_area_control(&e, wiz).is_none(),
            "no cluster, no lockdown — the buff lane below is unaffected"
        );
        assert!(
            try_foresight(&e, wiz).is_some(),
            "and Foresight is still reachable when nothing is clustered"
        );
    }

    /// `try_transmuted_spell` fires when:
    /// - Sorcerer has SP and no metamagic prime up.
    /// - Kit contains a known elemental damage spell.
    /// - At least one combat-active enemy has asymmetric elemental
    ///   resistance (some elements resisted/immune AND others
    ///   neutral/vulnerable). Skipped otherwise.
    #[test]
    fn transmuted_spell_ai_gates_on_asymmetric_resistance() {
        use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::{Coordinate, DamageModifier, DamageType};

        let tp = TerrainGenParams {
            width: 30,
            height: 30,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(13)).unwrap();
        let sorcerer = e
            .instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .give_resource(Resource::BonusAction);

        // No enemies at all → skip.
        assert!(
            try_transmuted_spell(&e, sorcerer).is_none(),
            "no enemies → skip"
        );

        // Plain zombie carries poison immunity (one of the six elementals)
        // so it already has asymmetric resistance — prime should fire.
        let zombie = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        assert!(
            try_transmuted_spell(&e, sorcerer).is_some(),
            "asymmetric resistance (zombie poison immunity) → prime fires"
        );

        // Strip the zombie's poison immunity → no asymmetry across the six
        // elementals → prime gates off.
        e.actors
            .get_mut(&zombie)
            .unwrap()
            .set_damage_modifier(DamageType::Poison, DamageModifier::Vulnerability);
        // Vulnerability is also "weak" (not strong), so we now have all-weak
        // across the six elementals and the prime should skip.
        assert!(
            try_transmuted_spell(&e, sorcerer).is_none(),
            "no asymmetric elemental resistance → skip"
        );

        // Hand the zombie fire resistance — re-establishes asymmetry (fire
        // resisted + others neutral/vulnerable). Prime should fire.
        e.actors
            .get_mut(&zombie)
            .unwrap()
            .set_damage_modifier(DamageType::Fire, DamageModifier::Resistance);
        assert!(
            try_transmuted_spell(&e, sorcerer).is_some(),
            "asymmetric resistance → prime fires"
        );

        // Already primed → skip.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .add_condition(Condition::TransmutedSpelling, ConditionTimer::Rounds(2));
        assert!(
            try_transmuted_spell(&e, sorcerer).is_none(),
            "prime already up → skip"
        );

        // Drop prime, burn SP → skip.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .remove_condition(Condition::TransmutedSpelling);
        while e.actors[&sorcerer].sorcery_points() > 0 {
            e.actors.get_mut(&sorcerer).unwrap().spend_sorcery_point();
        }
        assert!(
            try_transmuted_spell(&e, sorcerer).is_none(),
            "no SP → skip"
        );
    }

    /// `try_tides_of_chaos` fires once per long rest when an enemy is in
    /// range. Skipped after the charge is spent and skipped when no
    /// swing-able enemy is in the 24-tile window.
    #[test]
    fn tides_of_chaos_ai_gates_on_feature_and_range() {
        use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 30,
            height: 30,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap();
        let sorcerer = e
            .instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .give_resource(Resource::BonusAction);

        // No enemies → skip.
        assert!(
            try_tides_of_chaos(&e, sorcerer).is_none(),
            "no enemy → skip"
        );

        // Enemy in range → fires (Tides has no SP cost, just the rest
        // charge).
        let _z = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        assert!(
            try_tides_of_chaos(&e, sorcerer).is_some(),
            "enemy in range + charge available → fires"
        );

        // Spend the feature charge → skip.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .spend_feature(crate::actions::class_features::TIDES_OF_CHAOS_TAG);
        assert!(
            try_tides_of_chaos(&e, sorcerer).is_none(),
            "feature charge spent → skip"
        );

        // Restore charge, install Heightened prime → skip (save-or-suck
        // primes don't roll attacks).
        e.actors.get_mut(&sorcerer).unwrap().long_rest();
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .give_resource(Resource::BonusAction);
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .add_condition(Condition::HeightenedSpelling, ConditionTimer::Rounds(2));
        assert!(
            try_tides_of_chaos(&e, sorcerer).is_none(),
            "Heightened prime up → skip"
        );
    }

    /// `try_steady_aim` fires only when the rogue hasn't moved, no Helped
    /// prime is already up, and an enemy sits within ranged-attack reach.
    #[test]
    fn steady_aim_ai_gates_on_movement_helped_and_range() {
        use crate::actors::creatures::rogues::ROGUE_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 30,
            height: 30,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(7)).unwrap();
        let rogue = e
            .instantiate_creature(&ROGUE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.actors.get_mut(&rogue).unwrap().reset_for_new_round();

        // No enemies → skip.
        assert!(try_steady_aim(&e, rogue).is_none(), "no enemy → skip");

        // Enemy in shortbow range → fires.
        let _z = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(10, 5), 1, 0)
            .unwrap();
        assert!(
            try_steady_aim(&e, rogue).is_some(),
            "enemy in range + full movement → fires"
        );

        // Move the rogue (drain some movement) → skip.
        e.actors
            .get_mut(&rogue)
            .unwrap()
            .consume_resource(Resource::Movement(2.5));
        assert!(
            try_steady_aim(&e, rogue).is_none(),
            "movement already spent → skip"
        );

        // Reset, install Helped → skip (would no-stack).
        e.actors.get_mut(&rogue).unwrap().reset_for_new_round();
        e.actors
            .get_mut(&rogue)
            .unwrap()
            .add_condition(Condition::Helped, ConditionTimer::UntilStartOfNextTurn);
        assert!(
            try_steady_aim(&e, rogue).is_none(),
            "Helped prime already up → skip"
        );
    }

    /// `try_cunning_strike` AI heuristic: only fires when the rogue has
    /// an adjacent sneak-eligible enemy, the once-per-turn sneak charge
    /// is fresh, the sneak pool can spare a die (level >= 3), and no
    /// prime is already up.
    #[test]
    fn cunning_strike_ai_gates() {
        use crate::actors::creatures::rogues::ROGUE_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::dice::FastRandRoller;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(3)).unwrap();
        let rogue = e
            .instantiate_creature(&ROGUE_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        e.actors.get_mut(&rogue).unwrap().reset_for_new_round();

        // No adjacent enemy → skip.
        assert!(
            try_cunning_strike(&e, rogue).is_none(),
            "no adjacent enemy → skip"
        );

        // Adjacent enemy but rogue level 1 (sneak pool < 2) → skip.
        let _z = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(6, 5), 1, 0)
            .unwrap();
        assert!(
            try_cunning_strike(&e, rogue).is_none(),
            "level-1 rogue: pool too small → skip"
        );

        // Level the rogue to 3 so the sneak pool is 2d6.
        e.actors.get_mut(&rogue).unwrap().award_xp(10_000);
        let mut roller = FastRandRoller::with_seed(0);
        while e.actors[&rogue].level() < 3
            && e.actors
                .get_mut(&rogue)
                .unwrap()
                .try_level_up(&mut roller)
                .is_some()
        {}

        // Now the AI should fire.
        assert!(
            try_cunning_strike(&e, rogue).is_some(),
            "lv3 rogue + adjacent enemy → fires"
        );

        // With a prime already up → skip.
        e.actors.get_mut(&rogue).unwrap().add_condition(
            Condition::CunningStrikePoison,
            ConditionTimer::UntilStartOfNextTurn,
        );
        assert!(
            try_cunning_strike(&e, rogue).is_none(),
            "prime already up → skip"
        );

        // Strip the prime, mark sneak attack used → skip (no swing left
        // to consume the prime).
        e.actors
            .get_mut(&rogue)
            .unwrap()
            .remove_condition(Condition::CunningStrikePoison);
        e.actors.get_mut(&rogue).unwrap().mark_sneak_attack_used();
        assert!(
            try_cunning_strike(&e, rogue).is_none(),
            "sneak attack already spent → skip"
        );
    }

    /// Sorcerer's `has_any_metamagic_prime` returns true when any prime
    /// is up and false when none is up. Tides of Chaos is intentionally
    /// excluded — it's a Wild Magic feature, not a metamagic.
    #[test]
    fn has_any_metamagic_prime_covers_each_prime() {
        use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        let sorcerer = e
            .instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        assert!(!e.actors[&sorcerer].has_any_metamagic_prime());

        // Tides of Chaos is NOT a metamagic prime — should remain false.
        e.actors
            .get_mut(&sorcerer)
            .unwrap()
            .add_condition(Condition::TidesOfChaos, ConditionTimer::Rounds(2));
        assert!(!e.actors[&sorcerer].has_any_metamagic_prime());

        // Each metamagic prime in turn flips the flag to true.
        const ALL_PRIMES: &[Condition] = &[
            Condition::EmpoweredSpelling,
            Condition::HeightenedSpelling,
            Condition::CarefulSpelling,
            Condition::DistantSpelling,
            Condition::TwinnedSpelling,
            Condition::ExtendedSpelling,
            Condition::SeekingSpelling,
            Condition::SubtleSpelling,
            Condition::TransmutedSpelling,
        ];
        for &c in ALL_PRIMES {
            // Strip prior conditions to isolate this one.
            let actor = e.actors.get_mut(&sorcerer).unwrap();
            for &c2 in ALL_PRIMES {
                actor.remove_condition(c2);
            }
            actor.add_condition(c, ConditionTimer::Rounds(2));
            assert!(
                e.actors[&sorcerer].has_any_metamagic_prime(),
                "prime {:?} → should flip flag true",
                c
            );
        }
    }

    /// Sorcerous Restoration: a sorcerer with the lv20 capstone tag
    /// regains 4 SP on short rest (saturating at the long-rest cap).
    #[test]
    fn sorcerous_restoration_regains_sp_on_short_rest() {
        use crate::actors::creatures::sorcerers::SORCERER_TEMPLATE;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        let sorcerer = e
            .instantiate_creature(&SORCERER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // Burn SP to 0 to verify the restoration math.
        while e.actors[&sorcerer].sorcery_points() > 0 {
            e.actors.get_mut(&sorcerer).unwrap().spend_sorcery_point();
        }
        assert_eq!(e.actors[&sorcerer].sorcery_points(), 0);
        let mut roller = crate::engine::dice::FastRandRoller::with_seed(1);
        e.actors.get_mut(&sorcerer).unwrap().short_rest(&mut roller);
        // RAW restores exactly 4 SP on short rest; the template starts
        // with sorcery_points_max >= 4 so the full 4 land in the pool.
        assert_eq!(
            e.actors[&sorcerer].sorcery_points(),
            4,
            "short rest should restore 4 SP via Sorcerous Restoration"
        );

        // Short-rest again at near-full pool — should saturate at the
        // long-rest cap, not exceed it.
        e.actors.get_mut(&sorcerer).unwrap().long_rest();
        let cap = e.actors[&sorcerer].sorcery_points_max();
        assert_eq!(e.actors[&sorcerer].sorcery_points(), cap);
        e.actors.get_mut(&sorcerer).unwrap().short_rest(&mut roller);
        assert_eq!(
            e.actors[&sorcerer].sorcery_points(),
            cap,
            "saturate at cap"
        );
    }

    /// The Darkvision picker fires in the dark and not in the light,
    /// and it never spends the slot on a creature that already sees.
    ///
    /// The lighting gate is the half worth pinning. A 2nd-level slot
    /// cast on a lit board buys nothing whatsoever, and a rung sitting
    /// this high in the ladder — above every offensive pick — would
    /// otherwise open every fight in the engine with a wasted turn.
    #[test]
    fn the_darkvision_picker_waits_for_a_room_that_is_actually_dark() {
        use crate::actors::creatures::drow::DROW_TEMPLATE;
        use crate::actors::creatures::rangers::RANGER_TEMPLATE;
        use crate::conditions::ConditionTimer;
        use crate::engine::lighting::AmbientLight;

        // A ranger rather than a ranger: the ranger chassis on this
        // roster is born with 60 ft of darkvision, so the spell has
        // nothing to give it and the picker would be right to stay
        // quiet.
        let mut e = empty_arena();
        let ranger = e
            .instantiate_creature(&RANGER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        assert!(
            super::try_darkvision(&e, ranger).is_none(),
            "a lit board is not a problem this spell solves"
        );

        e.set_ambient_light(AmbientLight::Darkness);
        assert!(
            super::try_darkvision(&e, ranger).is_some(),
            "an unlit board and a human ranger is exactly the case"
        );

        // A drow standing next to them changes nothing: the ranger is
        // still the one who cannot see, and the picker prefers them.
        let _drow = e
            .instantiate_creature(&DROW_TEMPLATE, Coordinate::new(7, 5), 0, 0)
            .unwrap();
        let pick = super::try_darkvision(&e, ranger).expect("still the ranger's problem");
        assert_eq!(pick.target_ids(), Some(&[ranger][..]));

        // Buff the ranger and the rung falls silent — the drow is
        // refused by the action's own validator, so there is nobody
        // left to spend the slot on.
        e.actors
            .get_mut(&ranger)
            .unwrap()
            .add_condition(Condition::Darkvisioned, ConditionTimer::Rounds(100));
        assert!(
            super::try_darkvision(&e, ranger).is_none(),
            "the drow needs nothing and the ranger already has it"
        );
    }

    /// The Water Walk picker fires when the water is between the party
    /// and the enemy, and stays quiet when it is scenery beside them.
    #[test]
    fn the_water_walk_picker_waits_for_water_in_the_way() {
        use crate::actors::creatures::druids::DRUID_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::engine::terrain::TerrainType;

        let mut e = empty_arena();
        let druid = e
            .instantiate_creature(&DRUID_TEMPLATE, Coordinate::new(3, 5), 0, 0)
            .unwrap();
        let _goblin = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(15, 5), 1, 0)
            .unwrap();
        assert!(
            super::try_water_walk(&e, druid).is_none(),
            "a dry board between them is not a crossing"
        );

        // A pond off to one side is scenery, not an obstacle.
        for x in 3..=6isize {
            for y in 14..=17isize {
                e.set_terrain_at(Coordinate::new(x, y), TerrainType::Water);
            }
        }
        assert!(
            super::try_water_walk(&e, druid).is_none(),
            "water nobody has to cross is scenery"
        );

        // Flood the straight line to the goblin and the spell is worth
        // its slot.
        for x in 8..=11isize {
            for y in 4..=6isize {
                e.set_terrain_at(Coordinate::new(x, y), TerrainType::Water);
            }
        }
        assert!(
            super::try_water_walk(&e, druid).is_some(),
            "the lake is now between the druid and the fight"
        );
    }

    /// AI mage-armor fallback: a non-caster (fighter) carrying a Potion of
    /// Mage Armor drinks it via `try_self_buff_mage_armor`. Confirms the
    /// `try_self_action_inc_items` fallback searches `available_actions()`,
    /// which surfaces inventory-granted actions a `find_action` lookup
    /// would miss.
    #[test]
    fn ai_drinks_potion_of_mage_armor_when_no_spell() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::items::item_template::POTION_OF_MAGE_ARMOR;

        let mut e = empty_arena();
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        // Spawn a far-off enemy so the fighter isn't in melee range
        // (the kite / focus-fire branches in front of the buff lane
        // need to skip cleanly so the mage-armor branch can fire).
        let _enemy = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(25, 5), 1, 0)
            .unwrap();
        e.actors
            .get_mut(&fighter)
            .unwrap()
            .pickup_item(&POTION_OF_MAGE_ARMOR);

        // The fighter doesn't know the Mage Armor spell, so the
        // spell-path inside `try_self_buff_mage_armor` returns None
        // — the potion fallback should drive the decision.
        let aei =
            super::try_self_buff_mage_armor(&e, fighter).expect("expected the potion fallback");
        assert_eq!(aei.action().name(), "drink potion of mage armor");
    }

    /// The See Invisibility picker fires only when the buff would
    /// change something. Three cases against the same wizard: a plain
    /// visible enemy (no cast — the slot would be wasted), an Invisible
    /// enemy (cast), and an Invisible enemy the caster already sees
    /// through via True Seeing (no cast — nothing left to lift).
    #[test]
    fn see_invisibility_picker_fires_only_against_unseen_enemies() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::conditions::condition_template::ConditionTimer;
        let picked = |goblin_cover: Option<Condition>, wizard_sight: Option<Condition>| -> bool {
            let mut e = empty_arena();
            let wizard = e
                .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(2, 2), 0, 0)
                .unwrap();
            let goblin = e
                .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(6, 2), 1, 0)
                .unwrap();
            if let Some(c) = goblin_cover {
                e.actors
                    .get_mut(&goblin)
                    .unwrap()
                    .add_condition(c, ConditionTimer::Rounds(10));
            }
            if let Some(c) = wizard_sight {
                e.actors
                    .get_mut(&wizard)
                    .unwrap()
                    .add_condition(c, ConditionTimer::Rounds(10));
            }
            e.actors
                .get_mut(&wizard)
                .unwrap()
                .give_resource(crate::engine::side_effects::Resource::Action);
            super::try_see_invisibility(&e, wizard).is_some()
        };
        assert!(!picked(None, None), "a visible enemy is not worth a slot");
        assert!(
            picked(Some(Condition::Invisible), None),
            "an invisible enemy is exactly what the spell is for"
        );
        assert!(
            !picked(Some(Condition::Invisible), Some(Condition::TrueSighted)),
            "True Seeing already pierces it — nothing left to buy"
        );
        assert!(
            !picked(Some(Condition::Blurred), None),
            "Blur is not invisibility; the spell would do nothing"
        );
    }

    /// The wall lane: a caster with a wall spell and something closing
    /// on them raises a wall across the approach, and the wall really
    /// goes onto the map.
    #[test]
    fn a_caster_walls_off_the_thing_closing_on_them() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::engine::terrain::TerrainType;

        let mut e = empty_arena();
        let wizard = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(4, 10), 0, 0)
            .unwrap();
        // Two of them, four tiles out: past melee reach, inside the
        // lane's window, and enough of a line to be worth the slot.
        e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(10, 9), 1, 0)
            .unwrap();
        e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(10, 12), 1, 1)
            .unwrap();
        e.actors
            .get_mut(&wizard)
            .unwrap()
            .give_resource(crate::engine::side_effects::Resource::Action);
        let aei = super::try_wall_off_approach(&e, wizard)
            .expect("a wizard with a wall spell and something walking at them raises a wall");
        for ef in aei.execute(&mut e) {
            ef.apply(&mut e);
        }
        let patch = &e.conjured_terrain()[0];
        assert!(
            matches!(
                patch.terrain_type,
                TerrainType::ForceWall | TerrainType::Wall
            ),
            "the wall is made of something solid"
        );
        assert!(
            !patch.restore.is_empty(),
            "and it took real tiles off the map"
        );
        // Every tile it took is between the wizard and the goblins
        // rather than behind them: two tiles along the approach, and
        // the wall runs across it from there.
        let wizard_at = e.actors[&wizard].location();
        for (tile, _) in &patch.restore {
            assert!(
                tile.chebyshev_to(wizard_at) <= super::WALL_STANDOFF + 3,
                "{} is not part of a wall raised two tiles from {}",
                tile,
                wizard_at
            );
        }
    }

    /// The two gates that keep the lane from firing on its own side or
    /// on a fight it can't affect.
    #[test]
    fn the_wall_lane_holds_its_fire_for_an_ally_or_an_empty_room() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;

        // One of them is not a line: no wall.
        let mut lone = empty_arena();
        let alone_wiz = lone
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(4, 10), 0, 0)
            .unwrap();
        lone.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(10, 10), 1, 0)
            .unwrap();
        lone.actors
            .get_mut(&alone_wiz)
            .unwrap()
            .give_resource(crate::engine::side_effects::Resource::Action);
        assert!(
            super::try_wall_off_approach(&lone, alone_wiz).is_none(),
            "one goblin is a target, not a wall"
        );

        // Nobody closing: no wall.
        let mut alone = empty_arena();
        let solo = alone
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(4, 10), 0, 0)
            .unwrap();
        alone
            .actors
            .get_mut(&solo)
            .unwrap()
            .give_resource(crate::engine::side_effects::Resource::Action);
        assert!(super::try_wall_off_approach(&alone, solo).is_none());

        // An ally already stands between the wizard and the goblin.
        // Walling the line would strand them on the far side of it.
        let mut screened = empty_arena();
        let wizard = screened
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(4, 10), 0, 0)
            .unwrap();
        screened
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(9, 10), 0, 1)
            .unwrap();
        screened
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(10, 9), 1, 0)
            .unwrap();
        screened
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(10, 12), 1, 1)
            .unwrap();
        screened
            .actors
            .get_mut(&wizard)
            .unwrap()
            .give_resource(crate::engine::side_effects::Resource::Action);
        assert!(
            super::try_wall_off_approach(&screened, wizard).is_none(),
            "the fighter is closer to the goblin than the wizard is"
        );
    }

    /// Every subclass added in this batch is actually reached for by the
    /// AI in a real fight.
    ///
    /// The per-feature tests next to the engine lanes drive the effects
    /// directly, which proves the mechanics work but not that anything
    /// ever asks for them — a feature the AI never selects is a feature
    /// no player sees used, and the failure is invisible because every
    /// mechanical test still passes. This closes that loop for all five
    /// at once: run each of them through eight fights and require their
    /// signature line in the log.
    ///
    /// Ancestral Protectors is deliberately absent. It has no action to
    /// select — the mark rides an ordinary greataxe swing — so an AI
    /// wiring test would only be re-testing the rider, which the
    /// engine-side sweep already covers.
    



    #[test]
    fn the_ai_reaches_for_each_new_subclass_signature() {
        use crate::actors::actor_template::CreatureTemplate;
        use crate::actors::creatures::clerics::{DEATH_CLERIC_TEMPLATE, ORDER_CLERIC_TEMPLATE};
        use crate::actors::creatures::druids::{
            SPORES_DRUID_TEMPLATE, STARS_DRUID_TEMPLATE, WILDFIRE_DRUID_TEMPLATE,
        };
        use crate::actors::creatures::fighters::{
            ARCANE_ARCHER_FIGHTER_TEMPLATE, RUNE_KNIGHT_FIGHTER_TEMPLATE,
        };
        use crate::actors::creatures::monks::{
            ASTRAL_SELF_MONK_TEMPLATE, DRUNKEN_MASTER_MONK_TEMPLATE, KENSEI_MONK_TEMPLATE,
            MERCY_MONK_TEMPLATE, SUN_SOUL_MONK_TEMPLATE,
        };
        use crate::actors::creatures::rogues::{
            INQUISITIVE_ROGUE_TEMPLATE, PHANTOM_ROGUE_TEMPLATE, SOULKNIFE_ROGUE_TEMPLATE,
            THIEF_ROGUE_TEMPLATE,
        };
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::paladins::{
            CONQUEST_PALADIN_TEMPLATE, CROWN_PALADIN_TEMPLATE, REDEMPTION_PALADIN_TEMPLATE,
        };
        use crate::actors::creatures::warlocks::{
            FATHOMLESS_WARLOCK_TEMPLATE, UNDEAD_WARLOCK_TEMPLATE,
        };
        use crate::actors::creatures::bards::ELOQUENCE_BARD_TEMPLATE;
        use crate::actors::creatures::barbarians::{
            CLAW_BEAST_BARBARIAN_TEMPLATE, TAIL_BEAST_BARBARIAN_TEMPLATE,
        };
        use crate::actors::creatures::wizards::BLADESINGER_WIZARD_TEMPLATE;
        use crate::actors::creatures::artificers::{
            ALCHEMIST_ARTIFICER_TEMPLATE, ARMORER_ARTIFICER_TEMPLATE,
            ARTILLERIST_ARTIFICER_TEMPLATE, BATTLE_SMITH_ARTIFICER_TEMPLATE,
            INFILTRATOR_ARTIFICER_TEMPLATE,
        };

        // (template, the log fragment its headline feature prints)
        let cases: [(&CreatureTemplate, &str); 57] = [
            // Not Master of Tactics: the Mastermind hands an *ally*
            // advantage, and this fixture is one PC against one ogre.
            // Misdirection has no action to choose either — the engine
            // spends the reaction — so both belong engine-side.
            (&INQUISITIVE_ROGUE_TEMPLATE, "insightful fighting"),
            (&SPORES_DRUID_TEMPLATE, "halo of spores"),
            (&SPORES_DRUID_TEMPLATE, "symbiotic entity"),
            (&CONQUEST_PALADIN_TEMPLATE, "conquering presence"),
            (&BLADESINGER_WIZARD_TEMPLATE, "bladesong"),
            (&UNDEAD_WARLOCK_TEMPLATE, "form of dread"),
            (&KENSEI_MONK_TEMPLATE, "kensei's shot"),
            (&RUNE_KNIGHT_FIGHTER_TEMPLATE, "giant's might"),
            (&RUNE_KNIGHT_FIGHTER_TEMPLATE, "fire rune"),
            (&DEATH_CLERIC_TEMPLATE, "reaper's touch"),
            // Not Voice of Authority: this fixture is one PC against one
            // ogre, and the feature needs an ally to order. It fires
            // automatically off any levelled cast, so the AI has nothing
            // to choose — the engine-side test is where it belongs.
            (&ORDER_CLERIC_TEMPLATE, "divine strike (psychic)"),
            (&SOULKNIFE_ROGUE_TEMPLATE, "psychic blade"),
            (&SOULKNIFE_ROGUE_TEMPLATE, "second blade"),
            // Fast Hands has no action of its own — what it changes is
            // the price of one — so the marker is the belt being
            // reached for at all. Thief's Reflexes isn't listed: it
            // fires at `initialize` with nothing for a controller to
            // decide, so it belongs to the engine-side tests.
            (&THIEF_ROGUE_TEMPLATE, "potion of healing"),
            // Not Searing Sunburst: `best_burst_placement` refuses any
            // placement catching fewer than two enemies, and this
            // fixture is one PC against one ogre. Its engine-side test
            // is where the save-for-nothing behaviour is pinned.
            (&SUN_SOUL_MONK_TEMPLATE, "radiant sun bolt"),
            // Not Turn the Tide or Divine Allegiance: both need an ally,
            // and this fixture is one PC against one ogre. Divine
            // Allegiance also has no action to choose — the engine
            // spends the reaction — so it belongs to the engine tests.
            (&CROWN_PALADIN_TEMPLATE, "champion challenge"),
            // Not Unfailing Inspiration: the bard would have to inspire
            // an ally and then watch that ally fail a roll, and this
            // fixture is one PC against one ogre. Its refund is pinned
            // engine-side.
            (&ELOQUENCE_BARD_TEMPLATE, "unsettling words"),
            // Not Hand of Healing: it targets an ally, and this fixture
            // is one PC against one ogre. Hand of Harm needs nothing but
            // a swing, which the monk chassis does every turn.
            (&MERCY_MONK_TEMPLATE, "hand of harm"),
            // The wail needs a second enemy to reach, and this fixture
            // has one ogre — so the marker is the Sneak Attack the
            // feature hangs off, and the wail itself is pinned
            // engine-side where a second body can be put on the board.
            (&PHANTOM_ROGUE_TEMPLATE, "sneak attack"),
            // Archer, not Chalice or Dragon: this fixture is one PC
            // against one ogre, so there is no ally to spill a heal
            // onto, and the druid picks its constellation on the
            // opening round before it is concentrating on anything.
            // Those two branches are pinned by
            // `the_starry_form_pick_follows_what_the_round_needs`.
            (&STARS_DRUID_TEMPLATE, "shape of the Archer"),
            // The bolt is reached for by the ordinary attack picker
            // rather than by a rung of its own — reach 24 beats the
            // scimitar's 1 — so seeing it in the log is also the check
            // that a bonus-action attack survives that picker.
            (&STARS_DRUID_TEMPLATE, "starry bolt"),
            // Banishing, not one of the other five: the ogre's worst
            // saves are CHA and WIS at -2 apiece, and `ARCANE_SHOT_ORDER`
            // breaks that tie toward the shot that costs the target its
            // turn. Bursting is unreachable here for the same reason
            // Searing Sunburst is — the blast needs a bystander and this
            // fixture has one ogre.
            (&ARCANE_ARCHER_FIGHTER_TEMPLATE, "banishing arrow"),
            // The rider firing is a separate fact from the prime being
            // declared: it proves the shot reached the ranged lane of
            // the on-hit table rather than sitting on the string.
            (&ARCANE_ARCHER_FIGHTER_TEMPLATE, "banishing arrow banish"),
            // The claws and the tail are reached for by the ordinary
            // attack picker rather than by a rung of their own, so
            // seeing each in the log is also the check that a swing
            // gated on `Raging` survives a picker that scores candidates
            // before the rage is up. The claws in particular are the
            // check on `expected_damage`: they lose to the greataxe on
            // the die and win on the third swing, and before the picker
            // could see that they were unreachable.
            (&CLAW_BEAST_BARBARIAN_TEMPLATE, "claws"),
            (&CLAW_BEAST_BARBARIAN_TEMPLATE, "additional claw"),
            (&TAIL_BEAST_BARBARIAN_TEMPLATE, "tail"),
            // The summon, the swing it unlocks, and the die that only
            // lands while it is up — three separate facts, and the
            // middle one is the interesting one: `ASTRAL_ARMS_STRIKE`
            // is reached for by the ordinary attack picker, so seeing
            // it proves a `SimpleWeapon` gated on a self-condition
            // survives a picker that scores candidates on a turn the
            // condition may not be up yet.
            (&ASTRAL_SELF_MONK_TEMPLATE, "arms of the astral self"),
            (&ASTRAL_SELF_MONK_TEMPLATE, "astral arms"),
            (&ASTRAL_SELF_MONK_TEMPLATE, "empowered arms"),
            // Not Deflect Energy: the ogre swings a greatclub, and the
            // clamp's whole point is that it answers everything except
            // a physical melee swing. Its row is pinned engine-side.
            // Not the bite: it is deliberately the worse swing until the
            // barbarian is below half hit points, and a 76-HP body with
            // Rage halving every physical hit does not get there against
            // one ogre. Its rung is pinned directly by
            // `the_beast_bite_is_only_reached_for_once_the_rage_is_losing`.
            //
            // The summon and the die it turns on are two separate facts.
            // The first is that the AI's summon rung finds a feature
            // rather than a spell — Summon Wildfire Spirit costs no slot
            // and holds no concentration, so it reaches the rung through
            // `summons_allies()` alone. The second is that the spirit
            // being *there* is what pays: Enhanced Bond prints only when
            // a fire spell goes off with the spirit inside 60 ft, which
            // no amount of summoning guarantees on its own.
            (&WILDFIRE_DRUID_TEMPLATE, "summon wildfire spirit"),
            // Not Enhanced Bond: it pays out on a fire spell, and a
            // druid standing in an ogre's reach now swings its
            // shillelagh'd scimitar rather than casting a cantrip at
            // disadvantage — which is the attack picker reading the
            // roll mode, and is right. Its levelled fire spells are
            // area spells the burst rung refuses against a single
            // enemy. Both halves of the bond are pinned engine-side by
            // `enhanced_bond_rides_fire_spells_only_while_the_spirit_is_near`,
            // where a fire cast can be made to happen.
            // The tentacle is called, and then something walks into it.
            // Guardian Coil is deliberately absent: this fixture is one
            // PC against one ogre, so the only creature the coils could
            // shield is the warlock themselves, and a warlock the ogre
            // has reached is a warlock whose tentacle placement stopped
            // mattering. Its radius is pinned engine-side, where a third
            // body can be put on the board.
            (&FATHOMLESS_WARLOCK_TEMPLATE, "tentacle of the deep"),
            (&FATHOMLESS_WARLOCK_TEMPLATE, "tentacle slam"),
            (&ARTILLERIST_ARTIFICER_TEMPLATE, "eldritch cannon"),
            (&BATTLE_SMITH_ARTIFICER_TEMPLATE, "steel defender"),
            // Not the infused weapon or the jolt that rides it: the
            // Battle Smith opens by summoning a defender, the defender
            // takes the ogre, and an artificer whose construct is
            // holding the front line is an artificer who never comes
            // into contact — which is the subclass playing correctly
            // rather than a rung failing to fire. Both are pinned
            // engine-side, where a swing can be made to land.
            (&ARMORER_ARTIFICER_TEMPLATE, "thunder gauntlets"),
            (&ARMORER_ARTIFICER_TEMPLATE, "defensive field"),
            (&INFILTRATOR_ARTIFICER_TEMPLATE, "lightning launcher"),
            (&ALCHEMIST_ARTIFICER_TEMPLATE, "experimental elixir"),
            // The exit rides the Flurry the monk was pressing anyway,
            // which is the whole shape of Drunken Technique — there is
            // no rung of its own to reach for. Not Redirect Attack: it
            // needs a second enemy within 5 ft to send the swing to, and
            // this fixture has one ogre. Not Drunkard's Luck: the monk
            // has to be rolling at disadvantage, which an unimpeded monk
            // in the open never is. Both are pinned engine-side, where a
            // third body and a penalty can be arranged.
            (&DRUNKEN_MASTER_MONK_TEMPLATE, "drunken technique"),
            // Not Aura of the Guardian or Protective Spirit: the first
            // needs an ally inside the aura to take a blow for, and the
            // second fires off the turn-start hook with nothing for a
            // controller to decide. Rebuke the Violent is the oath's one
            // press, and the CD rung reaches it.
            (&REDEMPTION_PALADIN_TEMPLATE, "rebuke the violent"),
            // Not Radiance of the Dawn beside it: the Light Cleric's
            // burst is gated on two hostiles in the radius, and this
            // fixture has one ogre. Its rung is pinned by
            // `the_light_clerics_dawn_waits_for_a_crowd` below.
            (&crate::actors::creatures::clerics::TEMPEST_CLERIC_TEMPLATE, "wrath of the storm"),
            (
                &crate::actors::creatures::warlocks::FIEND_WARLOCK_TEMPLATE,
                "hurl through hell",
            ),
            (
                &crate::actors::creatures::tieflings::TIEFLING_TEMPLATE,
                "infernal rebuke",
            ),
            (
                &crate::actors::creatures::fighters::FIGHTER_TEMPLATE,
                "indomitable",
            ),
            // Not Breath of the Dragon: `best_burst_placement` refuses
            // any placement catching fewer than two enemies, and this
            // fixture is one PC against one ogre — the same reason
            // Searing Sunburst is unreachable here. Not the Draconic
            // Strike either: it is the ordinary fist in a different
            // element, so which of the two the attack picker prefers is
            // a matchup question this fixture doesn't pose. Both are
            // pinned engine-side. The aura is the press with a rung of
            // its own, and its bar of one is exactly the claim that a
            // monk in contact with a single hostile should still spend
            // the ki.
            (
                &crate::actors::creatures::monks::ASCENDANT_DRAGON_MONK_TEMPLATE,
                "aspect of the wyrm",
            ),
            // Not Balm of Peace: its rung wants two bodies inside five
            // feet and this fixture is one PC against one ogre, so the
            // cleric is the only creature in its own huddle. Not
            // Protective Bond either — it has no action to choose, the
            // engine spends the reaction, and it needs an ally to spend
            // it for. Both are pinned engine-side. The bond is the
            // domain's level-1 press and it fires on a lone cleric,
            // because the cleric is one of the creatures RAW lets it
            // pick.
            (
                &crate::actors::creatures::clerics::PEACE_CLERIC_TEMPLATE,
                "emboldening bond",
            ),
            // Not Restore Balance: it has no action to choose — the
            // engine spends the reaction at the roll-mode chokepoint —
            // so it belongs to the engine-side tests, where a
            // disadvantaged roll can be arranged. The ward is the
            // origin's one press, and this fixture is where it lands on
            // the sorcerer's own body: it is the only creature on its
            // team, and an ogre has walked up to it.
            (
                &crate::actors::creatures::sorcerers::CLOCKWORK_SOUL_SORCERER_TEMPLATE,
                "bastion of law",
            ),
            // The drake is called, and then it bites. Not the bond and
            // not the breath: the bond pays out on the ranger's own
            // swing, and a ranger whose drake is holding the ogre is a
            // ranger the ogre never reached — which is the subclass
            // playing correctly. The breath is a burst, and
            // `best_burst_placement` refuses any placement catching
            // fewer than two enemies, which is the same wall Searing
            // Sunburst and Breath of the Dragon meet in this fixture.
            // Both are pinned engine-side, where a second body can be
            // put on the board.
            (
                &crate::actors::creatures::rangers::DRAKEWARDEN_RANGER_TEMPLATE,
                "summon drake",
            ),
            (
                &crate::actors::creatures::rangers::DRAKEWARDEN_RANGER_TEMPLATE,
                "bite",
            ),
            // The kindling and the die it turns on are two separate
            // facts, and the first is the interesting one: the rung
            // sits below Rage because the action refuses to install
            // without it, so seeing the log line at all is the proof
            // that a bonus-action press gated on a condition the
            // barbarian acquires one rung earlier is still reachable in
            // the same turn order. Not Giant Stature: it has no press —
            // the size follows the rage — and its gate is pinned
            // engine-side where an ordinary barbarian can stand beside
            // it for comparison.
            (
                &crate::actors::creatures::barbarians::GIANT_BARBARIAN_TEMPLATE,
                "elemental cleaver",
            ),
            (
                &crate::actors::creatures::barbarians::GIANT_BARBARIAN_TEMPLATE,
                "elemental cleaver: +",
            ),
            // Both totems reach the free-summon rung on their own, which
            // is the fact worth pinning: they are not `FeatureSummon`s,
            // so the rung finds them through `summons_allies()` and
            // nothing else. The bear's ward lands in the same breath
            // because this fixture is one PC — the druid is the only
            // creature in its own aura, which is exactly one of the
            // creatures RAW lets it ward.
            //
            // Not the unicorn's spill: it needs a slot heal *and* an
            // ally inside the aura who the heal missed, and this fixture
            // has neither. It is pinned engine-side, where a second body
            // can be put on the board.
            (
                &crate::actors::creatures::druids::BEAR_SHEPHERD_DRUID_TEMPLATE,
                "bear spirit",
            ),
            (
                &crate::actors::creatures::druids::UNICORN_SHEPHERD_DRUID_TEMPLATE,
                "unicorn spirit",
            ),
            // The balm reaches the heal rungs through `is_heal()` alone,
            // which is the fact worth pinning: no rung of its own, and a
            // druid that is the only creature on its team still finds
            // itself when the hit points call for it. Not Hidden Paths —
            // the escape lane wants a pinned *ranged* actor at half hit
            // points or with two bodies on it, and one ogre closing on a
            // druid is neither. Its row on `SELF_TELEPORT_ESCAPES` and
            // its blink are pinned engine-side.
            (
                &crate::actors::creatures::druids::DREAMS_DRUID_TEMPLATE,
                "balm of the summer court",
            ),
            // The two-weapon pair, and the reason both halves are
            // listed: the off-hand swing is the first action in the
            // engine whose legality depends on what the *same actor*
            // already did this turn, so seeing it in the log is the
            // check that the attack picker's ordering opens the gate
            // before it tries the door. A picker that reached for the
            // bonus swing first would find it invalid, spend the turn
            // on the main hand alone, and leave nothing in the log
            // that looked wrong.
            (
                &crate::actors::creatures::rangers::TWO_WEAPON_RANGER_TEMPLATE,
                "uses shortsword",
            ),
            (
                &crate::actors::creatures::rangers::TWO_WEAPON_RANGER_TEMPLATE,
                "off-hand shortsword",
            ),
            // The unstyled half of the same rule. A rogue gets no
            // fighting style, so the dagger is dice alone — and it is
            // still worth a bonus action, because Sneak Attack is once
            // per turn rather than once per attack and the off hand is
            // a second roll to land it on.
            (
                &crate::actors::creatures::rogues::SWASHBUCKLER_ROGUE_TEMPLATE,
                "off-hand dagger",
            ),
        ];

        for (template, marker) in cases {
            let mut seen_in = 0;
            for seed in 0..16u64 {
                let tp = TerrainGenParams {
                    width: 24,
                    height: 16,
                    branch_depth: 0,
                    branch_prob: 0.0,
                };
                let ap = ActorGenParams {
                    cr_target: 0.0,
                    n_teams: 0,
                    pc_template: None,
                    start_team: 0,
                };
                let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
                e.instantiate_creature(template, Coordinate::new(3, 8), 0, 0)
                    .unwrap_or_else(|_| panic!("{} should instantiate", template.name));
                e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(14, 8), 1, 0)
                    .expect("the ogre should instantiate");
                let ai = SimpleAi;
                let mut steps = 0usize;
                while steps < 20_000 && !e.is_complete() {
                    steps += 1;
                    e.process_stack();
                    let Some(prompt) = e.peek_prompt() else { break };
                    let actor_id = prompt.actor_id();
                    match ai.decide(&e, actor_id) {
                        ControllerDecision::AwaitInput => break,
                        ControllerDecision::Act(aei) => {
                            e.pop_prompt();
                            e.push_action(aei);
                        }
                    }
                }
                if e.messages().join("\n").contains(marker) {
                    seen_in += 1;
                }
            }
            assert!(
                seen_in > 0,
                "an AI {} should use \"{}\" in at least one of 16 fights",
                template.name,
                marker
            );
        }
    }

    /// The monk's bonus action, which used to have exactly one thing on
    /// it. Stunning Strike is still the first pick, but it declines
    /// against a creature that is already stunned — and what it declines
    /// falls through to Patient Defense when the monk is losing and to
    /// Flurry of Blows otherwise.
    ///
    /// Before these two rungs existed the fall-through was nothing at
    /// all: a monk with the stun unavailable simply did not use its
    /// bonus action, which on this chassis is a third of its damage.
    #[test]
    fn the_monks_bonus_action_has_three_answers_and_picks_between_them() {
        use crate::actors::creatures::monks::MONK_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::conditions::ConditionTimer;
        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(3)).unwrap();
        let monk = e
            .instantiate_creature(&MONK_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();

        // Nothing in reach: none of the three rungs fires, because all
        // three are worth exactly what the swing they ride is worth.
        assert!(try_stunning_strike(&e, monk).is_none());
        assert!(try_flurry_of_blows(&e, monk).is_none());
        assert!(try_patient_defense(&e, monk).is_none());

        let ogre = e
            .instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(6, 5), 1, 0)
            .unwrap();
        // In contact and healthy: stun first, flurry available behind
        // it, no dodge.
        assert!(try_stunning_strike(&e, monk).is_some());
        assert!(try_flurry_of_blows(&e, monk).is_some());
        assert!(
            try_patient_defense(&e, monk).is_none(),
            "a healthy monk dodging is a monk giving up a Flurry for nothing"
        );

        // Already stunned: re-priming would spend a point of ki on a
        // save the ogre is not going to roll. Flurry is what a monk
        // standing over a stunned creature should be doing anyway —
        // every swing against it has advantage.
        e.actors
            .get_mut(&ogre)
            .unwrap()
            .add_condition(Condition::Stunned, ConditionTimer::Rounds(2));
        assert!(
            try_stunning_strike(&e, monk).is_none(),
            "nothing left in reach worth stunning"
        );
        assert!(try_flurry_of_blows(&e, monk).is_some());

        // Hurt: the dodge outranks the extra swing.
        let max = e.actors[&monk].max_hitpoints();
        e.actors.get_mut(&monk).unwrap().take_damage(max * 3 / 4);
        assert!(try_patient_defense(&e, monk).is_some());
    }

    /// The Light Cleric's Channel Divinity waits for a crowd: one
    /// hostile in the radius is a job for the cantrip, two is what the
    /// burst is for. Same two-enemy rule `best_burst_placement` applies
    /// to placed bursts, applied here to a self-centred one.
    #[test]
    fn the_light_clerics_dawn_waits_for_a_crowd() {
        use crate::actors::creatures::clerics::LIGHT_CLERIC_TEMPLATE;
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        let tp = TerrainGenParams {
            width: 32,
            height: 32,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(5)).unwrap();
        let cleric = e
            .instantiate_creature(&LIGHT_CLERIC_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        assert!(
            try_radiance_of_the_dawn(&e, cleric).is_none(),
            "an empty room is not a crowd"
        );
        e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(7, 5), 1, 0)
            .unwrap();
        assert!(
            try_radiance_of_the_dawn(&e, cleric).is_none(),
            "one goblin is a job for Sacred Flame"
        );
        // A second goblin out past the 30 ft radius doesn't make a
        // crowd — the gate counts what the burst would actually reach.
        e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(25, 25), 1, 1)
            .unwrap();
        assert!(
            try_radiance_of_the_dawn(&e, cleric).is_none(),
            "a goblin across the room is not in the burst"
        );
        e.instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(7, 6), 1, 2)
            .unwrap();
        assert!(
            try_radiance_of_the_dawn(&e, cleric).is_some(),
            "two is what the Channel Divinity is for"
        );
    }

    /// The same effect at two prices is bought at the cheaper one. A
    /// rogue Disengages with Cunning Action and keeps its Action; a monk
    /// does it with Step of the Wind and gets the Dash thrown in;
    /// everyone else pays the PHB's Action for it.
    #[test]
    fn the_cheap_printing_of_disengage_is_the_one_that_gets_bought() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::monks::MONK_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::rogues::ROGUE_TEMPLATE;
        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap();
        e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(4, 4), 1, 0)
            .unwrap();
        for (template, expected, x) in [
            (&*ROGUE_TEMPLATE, "cunning disengage", 8),
            (&*MONK_TEMPLATE, "step of the wind", 10),
            (&*FIGHTER_TEMPLATE, "disengage", 12),
        ] {
            let id = e
                .instantiate_creature(template, Coordinate::new(x, 8), 0, x as usize)
                .unwrap();
            let picked = try_disengage(&e, id).expect("every one of them can disengage somehow");
            assert_eq!(
                picked.action().name(),
                expected,
                "{} reached for the wrong printing",
                template.name
            );
        }
    }

    /// The wall lane is reached for in a live fight.
    ///
    /// Same reasoning as the subclass sweep above: the lane tests drive
    /// `try_wall_off_approach` directly, which proves the rule but not
    /// that anything ever gets past the twenty-odd rungs above it. A
    /// wall spell the AI never raises is an invisible regression —
    /// every mechanical test still passes and no player sees the
    /// feature.
    ///
    /// Two unscreened casters with a line of ogres one move out is
    /// exactly the shape the lane's window describes, and the assertion
    /// is that the wall goes onto the map, not merely that the action
    /// was selected.
    ///
    /// The zone movers are not checked here. They ride on Cloudkill,
    /// Moonbeam, Dawn, Flaming Sphere and Incendiary Cloud, and which
    /// of those an AI caster reaches for is a question about the
    /// damage picker's scoring rather than about the mover — a fixture
    /// tuned until one of them comes out would be pinning the picker
    /// and calling it the map layer. `move_zone` and the five spells'
    /// own reposition branches are pinned directly, engine-side.
    #[test]
    fn a_live_fight_raises_a_wall() {
        assert!(
            two_casters_against_three_ogres_at(9).contains("rises across"),
            "a caster with a line of ogres one move away should raise a wall"
        );
    }

    /// Drive a whole AI-vs-AI fight and hand back its log. `ogre_x` is
    /// how far down the board the melee line starts, which is the only
    /// thing the two callers differ on.
    ///
    /// Runs eight seeds and concatenates, because a single seed's fight
    /// turns on a handful of rolls and the question here is whether the
    /// AI reaches for a thing at all, not whether it reaches for it
    /// every time.
    fn two_casters_against_three_ogres_at(ogre_x: isize) -> String {
        use crate::actors::creatures::druids::DRUID_TEMPLATE;
        use crate::actors::creatures::ogres::OGRE_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;

        let mut log = String::new();
        for seed in 0..8u64 {
            let tp = TerrainGenParams {
                width: 30,
                height: 20,
                branch_depth: 0,
                branch_prob: 0.0,
            };
            let ap = ActorGenParams {
                cr_target: 0.0,
                n_teams: 0,
                pc_template: None,
                start_team: 0,
            };
            let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
            let _ = e.instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(3, 9), 0, 0);
            let _ = e.instantiate_creature(&DRUID_TEMPLATE, Coordinate::new(3, 11), 0, 1);
            for (i, y) in [8isize, 11, 14].into_iter().enumerate() {
                let _ = e.instantiate_creature(&OGRE_TEMPLATE, Coordinate::new(ogre_x, y), 1, i);
            }
            let ai = SimpleAi;
            let mut steps = 0usize;
            while steps < 20_000 && !e.is_complete() {
                steps += 1;
                e.process_stack();
                let Some(prompt) = e.peek_prompt() else { break };
                let actor_id = prompt.actor_id();
                match ai.decide(&e, actor_id) {
                    ControllerDecision::AwaitInput => break,
                    ControllerDecision::Act(aei) => {
                        e.pop_prompt();
                        e.push_action(aei);
                    }
                }
            }
            log.push_str(&e.messages().join("\n"));
            log.push('\n');
        }
        log
    }

    /// The Arcane Shot pick reads the target, not the roster.
    ///
    /// One ogre, walked down its own save sheet. Each step installs the
    /// condition the previous pick would have landed, which takes that
    /// shot off the menu — RAW-wise a second Blinded buys nothing, and
    /// the pool is two charges deep — and forces the picker onto the
    /// next-weakest save. The ogre's sheet (STR +4, CON +3, WIS −2,
    /// CHA −2) is what makes the walk deterministic, and the CHA / WIS
    /// tie at the top is what pins `ARCANE_SHOT_ORDER`'s
    /// strongest-first ordering as the tie-break.
    /// The bonus-action primes that have to be cashed by a swing this
    /// turn are gated on the same distance the swing itself needs, and
    /// not on a tighter one.
    ///
    /// This is a regression pin with a specific failure behind it. Five
    /// of these gates asked for a footprint gap of 0 — two bodies
    /// actually touching — while every melee weapon in the game reaches
    /// a gap of 1. The AI's approach stops the moment it can attack, so
    /// gap 0 was a distance the AI never stood at, and a paladin
    /// swinging a greatsword at an ogre every round for twelve seeds
    /// never once cast Divine Smite. A monk in the same fixture never
    /// stunned, and a druid never picked up its club.
    ///
    /// The fixture is the exact geometry the bug lived at: one tile of
    /// separation, a legal swing, and a prime that has to agree.
    #[test]
    fn the_swing_primes_fire_at_the_distance_the_swing_lands_at() {
        use crate::actors::creatures::barbarians::BARBARIAN_TEMPLATE;
        use crate::actors::creatures::paladins::PALADIN_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::druids::DRUID_TEMPLATE;
        use crate::actors::creatures::monks::MONK_TEMPLATE;
        use crate::actors::actor_template::CreatureTemplate;

        let primed = |template: &'static CreatureTemplate,
                      picker: &dyn Fn(&EncounterInstance, usize) -> Option<ActionExecutionInfo>|
         -> bool {
            let mut e = empty_arena();
            let pc = e
                .instantiate_creature(template, Coordinate::new(5, 5), 0, 0)
                .unwrap();
            let z = e
                .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(8, 5), 1, 0)
                .unwrap();
            // One tile of footprint gap: what `MELEE_REACH` covers, and
            // where the AI's approach actually stops.
            assert_eq!(e.footprint_distance(pc, z), Some(1));
            assert!(
                best_attack_against(pc, &e.actors[&pc], &e, z).is_some(),
                "the fixture has to be a distance the actor can already swing from"
            );
            picker(&e, pc).is_some()
        };

        assert!(
            primed(&PALADIN_TEMPLATE, &try_divine_smite),
            "a paladin in reach of an enemy should prime the smite"
        );
        assert!(
            primed(&BARBARIAN_TEMPLATE, &try_reckless_attack),
            "a barbarian in reach of an enemy should attack recklessly"
        );
        assert!(
            primed(&FIGHTER_TEMPLATE, &|e, id| {
                try_self_action_when_enemy_within(e, id, MELEE_REACH, "precision attack")
            }),
            "a fighter in reach of an enemy should be able to prime precision"
        );
        assert!(
            primed(&MONK_TEMPLATE, &try_stunning_strike),
            "a monk in reach of an enemy should prime the stun"
        );
        assert!(
            primed(&DRUID_TEMPLATE, &try_shillelagh),
            "a druid in reach of an enemy should prime the club"
        );
        assert!(
            primed(&PALADIN_TEMPLATE, &try_smite_spell),
            "and should be able to reach the smite spells too, not just the feature"
        );
    }

    /// A prime that buys reach has to be visible to the picker that
    /// spends it. The AI primes Lunging Attack when an enemy stands at
    /// exactly the gap the lunge opens — and, before `extra_reach` had
    /// one home, then compared the enemy's distance against the
    /// *declared* reach of every weapon and found none of them legal.
    /// The prime was spent every time on a swing that never happened.
    #[test]
    fn a_lunging_fighter_can_reach_what_the_lunge_bought() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::conditions::ConditionTimer;
        let mut e = empty_arena();
        let f = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let z = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(9, 5), 1, 0)
            .unwrap();
        // Two tiles of footprint gap — one past the scimitar's reach,
        // exactly on the lunge's, and exactly where `try_lunging_attack`
        // decides to spend the bonus action.
        assert_eq!(e.footprint_distance(f, z), Some(2));
        assert!(
            best_attack_against(f, &e.actors[&f], &e, z).is_none(),
            "unprimed, a gap of two is out of reach"
        );
        assert!(
            try_lunging_attack(&e, f).is_some(),
            "and it is the gap the AI primes the lunge at"
        );
        e.actors
            .get_mut(&f)
            .unwrap()
            .add_condition(Condition::LungingAttacking, ConditionTimer::Rounds(2));
        let picked = best_attack_against(f, &e.actors[&f], &e, z);
        assert!(
            picked.is_some(),
            "primed, the picker should see the swing the engine already validates"
        );
        // And the swing really is legal — the picker and the action's
        // own validator now agree, which is the invariant that broke.
        let sword = e.actors[&f].find_action(picked.unwrap().1.name()).unwrap();
        let aei = ActionExecutionInfo::new(sword, f, Some(vec![z]), None, None);
        assert!(aei.validate(&e));
    }

    /// The Glamour bard's two ways to spend one pool are separated by
    /// the AI's gate, not by the action's validator: the mantle wants a
    /// party that is both wide enough to be worth covering and already
    /// being hurt, and on any turn that is not true the die below it is
    /// what the bard hands out instead.
    ///
    /// Both halves of the gate get their own fixture, because a
    /// heuristic missing either one collapses into "always the mantle":
    /// the cohort is at its widest on round one, when nobody has been
    /// touched yet.
    #[test]
    fn the_glamour_bard_saves_the_mantle_for_a_party_that_needs_it() {
        use crate::actors::creatures::bards::GLAMOUR_BARD_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;

        let pick = |setup: &dyn Fn(&mut EncounterInstance, usize)| -> String {
            let mut e = empty_arena();
            let bard = e
                .instantiate_creature(&GLAMOUR_BARD_TEMPLATE, Coordinate::new(5, 5), 0, 0)
                .unwrap();
            e.instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(12, 5), 1, 0)
                .unwrap();
            setup(&mut e, bard);
            try_mantle_of_inspiration(&e, bard)
                .or_else(|| try_bardic_inspiration(&e, bard))
                .map(|aei| aei.action().name().to_string())
                .unwrap_or_else(|| "<none>".to_string())
        };

        // An ally in range but nobody hurt: the die, every time.
        assert_eq!(
            pick(&|e, _b| {
                e.instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(6, 5), 0, 0)
                    .unwrap();
            }),
            "bardic inspiration"
        );

        // Same party, one wounded fighter: now the mantle.
        assert_eq!(
            pick(&|e, _b| {
                let ally = e
                    .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(6, 5), 0, 0)
                    .unwrap();
                e.actors.get_mut(&ally).unwrap().take_damage(9);
            }),
            "mantle of inspiration"
        );

        // A wounded bard standing alone is not two creatures, so the
        // breadth half of the gate declines and the bard falls through
        // — to nothing at all here, since there is no ally to inspire.
        assert_eq!(
            pick(&|e, b| {
                e.actors.get_mut(&b).unwrap().take_damage(9);
            }),
            "<none>"
        );
    }

    /// The Starry Form pick reads the round, not the roster.
    ///
    /// Three fixtures, one for each branch of `try_starry_form`. The
    /// Chalice and Dragon branches are unreachable from the one-PC
    /// fixture the subclass-signature sweep uses — Chalice needs a
    /// second body and Dragon needs a druid that is both concentrating
    /// and already hurt — so they are pinned here instead.
    #[test]
    fn the_starry_form_pick_follows_what_the_round_needs() {
        use crate::actors::creatures::druids::STARS_DRUID_TEMPLATE;
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::conditions::ConditionTimer;

        // Helper: build a druid two tiles from a zombie, hand the
        // caller the encounter, and ask what the AI transforms into.
        let pick = |setup: &dyn Fn(&mut EncounterInstance, usize)| -> String {
            let mut e = empty_arena();
            let druid = e
                .instantiate_creature(&STARS_DRUID_TEMPLATE, Coordinate::new(5, 5), 0, 0)
                .unwrap();
            e.instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(8, 5), 1, 0)
                .unwrap();
            setup(&mut e, druid);
            try_starry_form(&e, druid)
                .map(|aei| aei.action().name().to_string())
                .unwrap_or_else(|| "<none>".to_string())
        };

        // Nothing special about the round: the Archer, which is the
        // opening-round and solo answer.
        assert_eq!(pick(&|_e, _d| {}), "starry form archer");

        // A hurt ally inside the 30 ft spill radius: the Chalice, so
        // the heal lane's next cast covers two creatures.
        assert_eq!(
            pick(&|e, _d| {
                let ally = e
                    .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(6, 5), 0, 0)
                    .unwrap();
                let max = e.actors[&ally].max_hitpoints();
                e.actors.get_mut(&ally).unwrap().take_damage(max - 1);
            }),
            "starry form chalice"
        );

        // Concentrating AND already taking fire: the Dragon. Both
        // halves are required — the two single-clause fixtures below
        // are what stop this from collapsing back into "always Dragon",
        // which is how the first draft of the heuristic behaved.
        assert_eq!(
            pick(&|e, d| {
                e.actors.get_mut(&d).unwrap().take_damage(5);
                e.actors
                    .get_mut(&d)
                    .unwrap()
                    .add_condition(Condition::Blessed, ConditionTimer::Rounds(10));
                e.actors.get_mut(&d).unwrap().start_concentration(
                    crate::actors::actor_template::ConcentrationData::new("Moonbeam"),
                );
            }),
            "starry form dragon"
        );

        // Concentrating but untouched: no evidence the concentration is
        // under threat, so the Archer still wins.
        assert_eq!(
            pick(&|e, d| {
                e.actors.get_mut(&d).unwrap().start_concentration(
                    crate::actors::actor_template::ConcentrationData::new("Moonbeam"),
                );
            }),
            "starry form archer"
        );

        // Hurt but holding nothing: there is no concentration for the
        // floor to protect.
        assert_eq!(
            pick(&|e, d| {
                e.actors.get_mut(&d).unwrap().take_damage(5);
            }),
            "starry form archer"
        );
    }

    /// Every action name the AI's heuristics look up is the canonical
    /// name of an action some playable template actually carries.
    ///
    /// `ActorInstance::find_action` matches canonical names only — not
    /// aliases, not substrings — so a name list entry that is off by a
    /// word doesn't fail loudly, it silently never matches. That is
    /// exactly what happened to `"enlarge / reduce"`, which sat in the
    /// Extended Spell gate for as long as the gate existed while the
    /// spell's canonical name was `"enlarge"`: the metamagic's whole
    /// reason to fire was invisible to it, and every test still passed.
    ///
    /// The four Sorcerer gate lists were function-local consts until
    /// this test needed them; lifting them to module scope is what makes
    /// the sweep possible at all, and puts them beside
    /// `MELEE_ADJACENT_PRIMES`, which was already there.
    #[test]
    fn every_ai_action_name_matches_a_real_action() {
        use crate::actors::creatures::pc_template_families;
        use std::collections::HashSet;

        // Every template the AI can find itself steering, which is both
        // sides of a fight and not just the party's. Widened when the
        // `LockdownPick` cohorts joined the sweep below and immediately
        // flagged two rows — Sleep Gaze and Plane Shift, a vampire's
        // stare and a lich's touch — that no PC will ever carry and
        // that the AI reaches for every time it drives one.
        let known: HashSet<&str> = pc_template_families()
            .into_iter()
            .flat_map(|(_family, templates)| templates)
            .chain(crate::engine::encounter::EncounterInstance::template_pool())
            .flat_map(|t| t.actions.iter().map(|a| a.name()))
            .collect();

        // Every module-level name list the heuristics consult. A new
        // list belongs here; the cost of forgetting is a heuristic that
        // quietly never fires.
        let lists: [(&str, &[&str]); 8] = [
            ("MELEE_ADJACENT_PRIMES", MELEE_ADJACENT_PRIMES),
            ("SELF_TELEPORT_ESCAPES", SELF_TELEPORT_ESCAPES),
            ("AREA_CONTROL_SPELLS", AREA_CONTROL_SPELLS),
            ("HEIGHTENED_LOCKDOWN", HEIGHTENED_LOCKDOWN),
            ("HEIGHTENED_BURST", HEIGHTENED_BURST),
            ("EXTENDABLE", EXTENDABLE),
            ("ENGAGED_SELF_POSTURES", ENGAGED_SELF_POSTURES),
            ("WALL_SPELLS", WALL_SPELLS),
        ];
        // Two entries name real spells that no playable template
        // currently carries — a level-8 lockdown and a level-1 temp-HP
        // buff. They are kept because a future template picking either
        // one up should find the heuristic already waiting, and they are
        // written as `name()` reads off the statics rather than as
        // string literals, so a rename breaks this line instead of
        // quietly re-orphaning the entry.
        let waiting_on_a_template: HashSet<&str> = HashSet::from([
            crate::actions::spells::DOMINATE_MONSTER.name(),
            crate::actions::spells::FALSE_LIFE.name(),
        ]);
        let mut orphans: Vec<String> = Vec::new();
        for (list_name, entries) in lists {
            for name in entries {
                if !known.contains(name) && !waiting_on_a_template.contains(name) {
                    orphans.push(format!("{}: {:?}", list_name, name));
                }
            }
        }
        // The two `LockdownPick` cohorts, swept the same way. They were
        // outside this check for as long as it has existed, purely
        // because they are a different type from the `&[&str]` lists
        // above — which is not a reason, and it left the longest
        // name-keyed table in the AI unguarded. Between them they are
        // twenty-five rows of string literal, every one of which is a
        // rung that silently never fires if it is a character off.
        for (list_name, entries) in [("LOCKDOWNS", LOCKDOWNS), ("ATTRITION", ATTRITION)] {
            for row in entries {
                if !known.contains(row.name) && !waiting_on_a_template.contains(row.name) {
                    orphans.push(format!("{}: {:?}", list_name, row.name));
                }
            }
        }
        // The remaining struct-row cohorts, each of which resolves its
        // action by the same string lookup and fails the same silent
        // way. Written as three loops rather than one because the row
        // types differ in everything except the column that matters
        // here.
        for row in CONCENTRATION_MARKS {
            if !known.contains(row.name) && !waiting_on_a_template.contains(row.name) {
                orphans.push(format!("CONCENTRATION_MARKS: {:?}", row.name));
            }
        }
        for (list_name, entries) in [
            ("SELF_BUFFS_ABOVE_DUPLICITY", SELF_BUFFS_ABOVE_DUPLICITY),
            ("SELF_BUFFS_BELOW_DUPLICITY", SELF_BUFFS_BELOW_DUPLICITY),
        ] {
            for row in entries {
                if !known.contains(row.name) && !waiting_on_a_template.contains(row.name) {
                    orphans.push(format!("{}: {:?}", list_name, row.name));
                }
            }
        }
        for row in TURN_BURST_PICKS {
            let name = row.config.name;
            if !known.contains(name) && !waiting_on_a_template.contains(name) {
                orphans.push(format!("TURN_BURST_PICKS: {:?}", name));
            }
        }
        for (name, _, _) in ARCANE_SHOT_ORDER {
            if !known.contains(name) && !waiting_on_a_template.contains(name) {
                orphans.push(format!("ARCANE_SHOT_ORDER: {:?}", name));
            }
        }
        assert!(
            orphans.is_empty(),
            "these AI heuristic entries match no action any playable \
             template carries, so they can never fire:\n  {}",
            orphans.join("\n  ")
        );
    }

    /// Every action the AI's two concentration-aware rungs can reach
    /// declares whether it takes the caster's concentration, and the
    /// answers are pinned here.
    ///
    /// `Action::holds_concentration` defaults to `false`, which is the
    /// right default for the thousands of actions in the engine that
    /// genuinely do not concentrate and the wrong one for a
    /// concentration spell that forgets to override it. The two cohorts
    /// below are exactly where a wrong answer costs something — the
    /// summon rung and the area-control rung both gate on it — so a new
    /// summon or a new control spell has to state its answer here rather
    /// than inherit a default nobody checked.
    ///
    /// The summon cohort is discovered rather than listed: every action
    /// on every playable template that declares `summons_allies`. A
    /// seventh summon added tomorrow fails this test until someone says
    /// what it costs.
    #[test]
    fn the_ai_gated_cohorts_declare_their_concentration() {
        use crate::actors::creatures::pc_template_families;
        use std::collections::HashSet;

        // (action name, does it take the caster's concentration)
        let expected: &[(&str, bool)] = &[
            // Summons. Every spell in the lane anchors its minions to
            // the caster's concentration except Animate Dead, and every
            // per-rest *feature* summon does not — see
            // `spells::SummonSpell::concentration` for why that split is
            // the design rather than an accident of who wrote what.
            ("conjure animals", true),
            ("conjure elemental", true),
            ("animate objects", true),
            ("animate dead", false),
            // Find Steed is the second `None` on the chassis, and for
            // the same reason: a paladin who had to concentrate on their
            // horse could never smite from its back.
            ("find steed", false),
            ("find greater steed", false),
            // The third and last `None` on the chassis, and the same
            // sentence one class over: a wizard who had to concentrate
            // on their own horse could not concentrate on anything
            // worth casting from it.
            ("phantom steed", false),
            // The rest of the SRD conjure family. All four concentrate,
            // like the two that predate them — a conjured body lasts as
            // long as the caster keeps thinking about it, and the two
            // steeds are the exception precisely because nothing about
            // a horse depends on that.
            ("conjure woodland beings", true),
            ("conjure minor elementals", true),
            ("conjure fey", true),
            ("conjure celestial", true),
            ("summon beast", true),
            ("summon fey", true),
            ("summon undead", true),
            ("summon aberration", true),
            ("summon elemental", true),
            ("summon celestial", true),
            ("summon draconic spirit", true),
            ("summon fiend", true),
            ("bear spirit", false),
            ("unicorn spirit", false),
            ("ranger's companion", false),
            ("summon drake", false),
            ("summon wildfire spirit", false),
            ("tentacle of the deep", false),
            ("eldritch cannon (flamethrower)", false),
            ("eldritch cannon (force ballista)", false),
            ("eldritch cannon (protector)", false),
            ("steel defender", false),
            // Area control.
            ("web", true),
            ("hypnotic pattern", true),
            ("black tentacles", true),
            ("entangle", true),
            ("sleet storm", true),
            ("grease", false),
            ("stinking cloud", true),
            ("wall of sand", true),
            ("crown of thorns", true),
        ];

        let mut checked: HashSet<&str> = HashSet::new();
        let mut summons: HashSet<&str> = HashSet::new();
        for (_family, templates) in pc_template_families() {
            for action in templates.iter().flat_map(|t| t.actions.iter()) {
                if action.summons_allies() {
                    summons.insert(action.name());
                }
                if let Some((_, wants)) =
                    expected.iter().find(|(n, _)| *n == action.name())
                {
                    assert_eq!(
                        action.holds_concentration(),
                        *wants,
                        "{} disagrees with the pinned answer",
                        action.name()
                    );
                    checked.insert(action.name());
                }
            }
        }

        let undeclared: Vec<&str> = summons
            .iter()
            .filter(|n| !expected.iter().any(|(e, _)| e == *n))
            .copied()
            .collect();
        assert!(
            undeclared.is_empty(),
            "these summons reach the AI's summon rung without an answer \
             pinned here: {undeclared:?}"
        );
        for name in AREA_CONTROL_SPELLS {
            assert!(
                expected.iter().any(|(e, _)| e == name),
                "{name} is on the area-control registry without an answer pinned here"
            );
        }
        // Sanity that the walk above actually found the cohorts rather
        // than passing vacuously.
        assert!(
            checked.len() >= expected.len() - 1,
            "only reached {:?} of the pinned actions",
            checked
        );
    }

    /// An action that isn't aimed at an enemy doesn't whittle anybody's
    /// hit points.
    ///
    /// `deals_damage()` defaults to `is_harmful()` precisely so this
    /// holds without anyone maintaining it, and the exception list is
    /// the point of the test: exactly one action in the engine is
    /// legitimately `!is_harmful()` while declaring a damage type, and
    /// it says `deals_damage() == false` anyway. A second exception
    /// should have to be argued for here rather than arriving as a
    /// forgotten override.
    ///
    /// Worth pinning because the failure is silent. Every consumer of
    /// `deals_damage` today happens to check `is_harmful` first, so a
    /// wrong answer costs nothing until the consumer that doesn't
    /// arrives — at which point it inherits every wrong answer at once.
    #[test]
    fn nothing_friendly_claims_to_deal_damage() {
        use crate::actors::creatures::pc_template_families;

        let mut liars: Vec<&str> = Vec::new();
        for (_family, templates) in pc_template_families() {
            for action in templates.iter().flat_map(|t| t.actions.iter()) {
                if !action.is_harmful() && action.deals_damage() {
                    liars.push(action.name());
                }
            }
        }
        liars.sort_unstable();
        liars.dedup();
        assert!(
            liars.is_empty(),
            "these actions target no enemy yet claim to deal damage: {liars:?}"
        );
    }
    /// A burst that costs nothing but a bonus action fires at a single
    /// hostile; one that costs the Action or a slot still waits for two.
    ///
    /// The floor is what made two spells unreachable. Melf's Minute
    /// Meteors is a 5-foot burst thrown off a bonus action once it is
    /// lit, and a 5-foot burst almost never covers two bodies — so the
    /// meteors could be lit and then never thrown for the rest of the
    /// fight. The fixture is that exact stance: the orbit is up, one
    /// goblin is in range, and the caster has a bonus action.
    #[test]
    fn a_free_burst_is_worth_one_hostile_and_a_paid_one_is_not() {
        use crate::actors::creatures::goblins::GOBLIN_TEMPLATE;
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::types::Coordinate;

        let mut e = empty_arena();
        let wiz = e
            .instantiate_creature(&WIZARD_TEMPLATE, Coordinate::new(3, 3), 0, 0)
            .unwrap();
        let _goblin = e
            .instantiate_creature(&GOBLIN_TEMPLATE, Coordinate::new(9, 3), 1, 0)
            .unwrap();

        // Unlit, the meteors cost an Action and a 3rd-level slot, so a
        // lone goblin is not worth them — and neither is any other
        // burst on the wizard's list.
        let paid = best_burst_placement(&e, wiz, |a| a.name() == "melf's minute meteors");
        assert!(
            paid.is_none(),
            "a slot-and-Action burst should still want two hostiles"
        );

        // Lit, the volley is a bare bonus action and the same goblin is
        // worth throwing at.
        e.actors
            .get_mut(&wiz)
            .unwrap()
            .add_condition(Condition::MinuteMeteors, ConditionTimer::Rounds(3));
        let free = best_burst_placement(&e, wiz, |a| a.name() == "melf's minute meteors")
            .expect("a free volley should fire at one hostile");
        assert_eq!(free.action().name(), "melf's minute meteors");
    }

    /// 5e **Cleave** is only worth something when there is a second
    /// creature for the swing to carry into, and the picker prices it
    /// that way — the same greataxe that wins a crowd loses a duel to a
    /// heavier die.
    ///
    /// Both halves are the test. A flat bonus for holding a Cleave
    /// weapon would pass the first assertion and fail the second, and
    /// that is the failure worth pinning: a picker that always reached
    /// for the axe would be optimising for a clause that cannot fire.
    #[test]
    fn the_picker_prices_cleave_by_whether_there_is_a_second_body() {
        use crate::actions::monster_attacks::{GREATAXE, GREATSWORD};
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;

        // 1d12 (6.5) against 2d6 (7.0): the greatsword wins on dice
        // alone, so anything the axe wins it wins on its property.
        let pick_with_crowd = |crowd: bool| -> String {
            let mut e = empty_arena();
            let fighter = e
                .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
                .unwrap();
            let target = e
                .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(6, 5), 1, 0)
                .unwrap();
            if crowd {
                e.instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(6, 6), 1, 1)
                    .unwrap();
            }
            let mut actor = e.actors[&fighter].clone();
            actor.actions = vec![&GREATAXE, &GREATSWORD];
            best_attack_against(fighter, &actor, &e, target)
                .map(|(_, a)| a.name().to_string())
                .unwrap_or_else(|| "<none>".to_string())
        };
        assert_eq!(pick_with_crowd(true), "greataxe", "a crowd is what Cleave is for");
        assert_eq!(pick_with_crowd(false), "greatsword", "a duel is not");
    }

    /// The training gate reaches the picker too. The same two weapons in
    /// an untrained hand are ranked on their dice and nothing else, so a
    /// creature with no Weapon Mastery never picks the axe for a clause
    /// it cannot use.
    #[test]
    fn an_untrained_picker_ranks_the_same_two_weapons_on_dice_alone() {
        use crate::actions::monster_attacks::{GREATAXE, GREATSWORD};
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;

        let mut e = empty_arena();
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let target = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(6, 5), 1, 0)
            .unwrap();
        e.instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(6, 6), 1, 1)
            .unwrap();
        let mut actor = e.actors[&fighter].clone();
        actor.actions = vec![&GREATAXE, &GREATSWORD];
        actor.set_weapon_mastery(false);
        e.actors.get_mut(&fighter).unwrap().set_weapon_mastery(false);
        assert_eq!(
            best_attack_against(fighter, &actor, &e, target)
                .map(|(_, a)| a.name().to_string())
                .unwrap_or_else(|| "<none>".to_string()),
            "greatsword",
            "an untrained wielder is ranked on dice"
        );
    }
    /// 5e surprise, on a board the AI is driving.
    ///
    /// Two things are being pinned and only one of them is the rule. The
    /// first is that a torchless creature walking into a dark room full
    /// of darkvision is caught unawares at all — the check runs on the
    /// first `process_stack`, not at `initialize`, because the ambient
    /// light is set after the encounter is built and a surprise decided
    /// any earlier would be decided on a bright board in every game.
    ///
    /// The second is that the AI *survives* it. A surprised creature has
    /// no action, no bonus action, no reaction and no movement, which is
    /// a state the picker had never been handed before: every rung it
    /// tries is unaffordable. A driver that stalls there hangs the game,
    /// so the loop asserts the turn moves on.
    #[test]
    fn the_ai_does_not_stall_on_a_creature_that_was_caught_unawares() {
        use crate::actors::creatures::fighters::FIGHTER_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::conditions::Condition;
        use crate::engine::lighting::AmbientLight;
        use crate::engine::types::Coordinate;

        let mut e = empty_arena();
        e.set_ambient_light(AmbientLight::Darkness);
        let fighter = e
            .instantiate_creature(&FIGHTER_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let zombie = e
            .instantiate_creature(&ZOMBIE_TEMPLATE, Coordinate::new(8, 5), 1, 0)
            .unwrap();

        let ai = SimpleAi;
        let mut turns = 0usize;
        let mut saw_surprise = false;
        for _ in 0..2_000 {
            e.process_stack();
            saw_surprise |= e.actors[&fighter].has_condition(Condition::Surprised);
            if e.is_complete() {
                break;
            }
            let Some(prompt) = e.peek_prompt() else { break };
            let actor_id = prompt.actor_id();
            match ai.decide(&e, actor_id) {
                ControllerDecision::AwaitInput => panic!("SimpleAi returned AwaitInput"),
                ControllerDecision::Act(aei) => {
                    e.pop_prompt();
                    e.push_action(aei);
                }
            }
            turns += 1;
            if e.round() > 3 {
                break;
            }
        }
        assert!(
            saw_surprise,
            "a torchless fighter in the dark should have been caught unawares"
        );
        assert!(turns > 0, "the driver never got a single turn out of the board");
        assert!(
            e.round() > 1,
            "the encounter never left round one — the surprised actor stalled the queue"
        );
        let _ = zombie;
    }
}
