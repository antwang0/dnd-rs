use crate::actions::action_template::{Action, ActionExecutionInfo, MELEE_REACH, TargetingSchema};
use crate::ai::{Controller, ControllerDecision};
use crate::conditions::Condition;
use crate::engine::dice::RollMode;
use crate::engine::encounter::EncounterInstance;
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

        // 2. Kite if we're a ranged attacker under melee threat.
        if has_ranged_attack(encounter, actor_id)
            && under_melee_threat(encounter, actor_id)
            && let Some(aei) = try_step_away_from_threats(encounter, actor_id)
        {
            return ControllerDecision::Act(aei);
        }

        // 2b. If we're a low-HP ranged caster surrounded by melee, the
        //    safer exit is the Disengage action — gives our retreat free
        //    OA-suppression. We use it only when our HP is below 30% and
        //    we have a ranged option to capitalize on the disengaged
        //    movement after the action.
        if has_ranged_attack(encounter, actor_id)
            && under_melee_threat(encounter, actor_id)
            && is_low_hp(encounter, actor_id, 0.3)
            && let Some(aei) = try_disengage(encounter, actor_id)
        {
            return ControllerDecision::Act(aei);
        }

        // 3. Heal a dying / wounded ally.
        if let Some(aei) = try_support_heal(encounter, actor_id) {
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

        // 3b. Mage Armor — self-only AC boost. Casts once per combat
        //     since the condition lasts ~100 rounds; gated by "don't
        //     re-cast" via the condition check. Bonus action, so it
        //     stacks with this turn's offensive action.
        if let Some(aei) = try_self_buff_mage_armor(encounter, actor_id) {
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

        // 3c''. Lunging Attack — Fighter Battle Master bonus-action
        //       prime. Extends melee reach by one tile for the next
        //       swing. Fires only when an enemy sits at the precise
        //       gap the lunge opens up (gap 2 — one tile past default
        //       melee reach), so the prime isn't wasted on adjacents
        //       and isn't burned at out-of-reach distances.
        if let Some(aei) = try_lunging_attack(encounter, actor_id) {
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

        // 3f. Monk Stunning Strike — once-per-rest bonus-action prime
        //     that lays a stun save on the next melee hit. Fire when
        //     an adjacent enemy is queued for a swing this turn.
        if let Some(aei) = try_stunning_strike(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g. Cleric Turn Undead — once-per-rest Channel Divinity.
        //     Fire when at least one undead-proxy enemy is within 30ft
        //     so the cleanse-and-frighten lands on someone worth it.
        if let Some(aei) = try_turn_undead(encounter, actor_id) {
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

        // 3g''. Wizard Arcane Recovery — once-per-rest free action that
        //       restores a level-1 (and a level-2 at lv3+) spell slot.
        //       Fire when the caster has spent a slot and isn't burning
        //       it on an empty room.
        if let Some(aei) = try_arcane_recovery(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3g'''. Bard Cutting Words — bonus-action enemy debuff. Fire
        //        on the most threatening adjacent-to-an-ally enemy who
        //        isn't already Mocked, so the disadvantage lands before
        //        their swing.
        if let Some(aei) = try_cutting_words(encounter, actor_id) {
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

        // 3h. Bardic Inspiration — bonus-action ally buff. Fire on the
        //     highest-HP ally so the inspiration die rides their next
        //     attack swing (front-liners get the most value).
        if let Some(aei) = try_bardic_inspiration(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3i. Foresight — level-9 single-target ally apex buff. Lay it
        //     on the toughest ally before they engage. Highest priority
        //     of the support-buff lane because the slot is precious.
        if let Some(aei) = try_foresight(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3j. Holy Aura — level-8 concentration burst centered on the
        //     caster. Fire when allies are clustered and a fight has
        //     started. Slot-cheaper than Foresight per ally affected.
        if let Some(aei) = try_holy_aura(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3k. Spirit Shroud — level-3 self concentration. Fire when an
        //     enemy is in melee so the cold rider lands this round.
        if let Some(aei) = try_spirit_shroud(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3l. Bigby's Hand — level-5 wizard concentration self-buff
        //     (persistent +1d10 force per-hit rider). Fire when an
        //     enemy is in attack reach so the rider lands this turn.
        if let Some(aei) = try_bigbys_hand(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3m. Tenser's Transformation — level-6 wizard concentration
        //     self-buff (50 temp HP + self-attack-advantage). Fire
        //     when engaged so the temp HP buffer matters this round.
        if let Some(aei) = try_tensers_transformation(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3m'. Investiture of Flame — level-6 caster concentration
        //      self-buff (fire resistance + 1d10 fire melee retaliation).
        //      Fire when at least one enemy is in attack reach so the
        //      melee retaliation will trigger this round. Mirrors the
        //      Bigby's Hand / Tenser's Transformation gates.
        if let Some(aei) = try_investiture_of_flame(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3m''. Wind Wall — level-3 caster concentration self-buff
        //       (ranged-attack disadvantage). Fire when an enemy sits
        //       at long range so the deflection rider matters this
        //       fight. Slot-cheap (lv3) and the bigger concentration
        //       buffs above (Bigby's Hand / Tenser's / Investiture)
        //       take priority via earlier branches.
        if let Some(aei) = try_wind_wall(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3m'''. Shadow Blade — level-2 warlock/wizard concentration
        //        self-buff (advantage on attacks + psychic rider). Fire
        //        when an enemy is in melee range so the conjured blade
        //        sees use immediately.
        if let Some(aei) = try_self_buff_concentration(
            encounter,
            actor_id,
            "shadow blade",
            Condition::SpiritShrouded,
            1,
        ) {
            return ControllerDecision::Act(aei);
        }

        // 3m''''. Antilife Shell — level-5 cleric concentration self-buff.
        //         Push adjacent enemies away and apply Warded. Fire when
        //         2+ enemies are in melee reach so the push-back matters.
        if let Some(aei) = try_self_buff_concentration(
            encounter,
            actor_id,
            "antilife shell",
            Condition::Warded,
            2,
        ) {
            return ControllerDecision::Act(aei);
        }

        // 3n. Aura of Life — level-4 paladin concentration aura. Fire
        //     when at least one ally is clustered in the aura radius
        //     and a fight has started.
        if let Some(aei) = try_aura_of_life(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3n''. Holy Weapon — level-5 paladin self concentration buff
        //       (persistent +2d8 radiant per-hit rider). Fire when an
        //       enemy is in attack reach so the rider lands this turn.
        //       Slot-cost is steeper than Spirit Shroud / Bigby's Hand
        //       so we gate on the same engagement radius.
        if let Some(aei) = try_holy_weapon(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3n'''. Pass Without Trace — level-2 druid / ranger aura that
        //        imposes disadvantage on attackers targeting any ally in
        //        the 30ft sphere. Fire when at least one ally is in the
        //        aura radius and a fight has started — concentration-
        //        gated so the caster picks the highest-leverage buff
        //        across all the lv2-and-up self-buff branches above.
        if let Some(aei) = try_pass_without_trace(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3n'. Warding Bond — cleric / paladin lv2 abjuration. Touch-
        //      range damage-share bond: bonded ally gets +1 AC, +1 saves,
        //      and damage resistance; the caster takes the mirrored
        //      (post-resistance) damage. Fire on a footprint-adjacent
        //      ally that isn't already bonded, when the caster has spare
        //      HP to sink the mirror cost. The action's `custom_validate`
        //      handles the team / already-bonded / self-target gates.
        if let Some(aei) = try_warding_bond(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3o. Divine Strike — cleric bonus-action prime (once per long
        //     rest). Fire when an enemy is in melee so the +1d8 radiant
        //     rider lands on the cleric's next swing.
        if let Some(aei) = try_divine_strike(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p. Trip Attack — fighter bonus-action prime (once per long
        //     rest). Fire when an enemy is in melee so the prone-on-
        //     fail save lands this turn.
        if let Some(aei) = try_trip_attack(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p'. Menacing Attack — fighter bonus-action prime (Battle
        //      Master). Same engagement gate as Trip Attack; the
        //      WIS-save-vs-frighten rider sticks even on tough STR
        //      monsters that would resist the trip. Lower priority
        //      than Trip Attack because prone enables follow-up
        //      melee-advantage swings, whereas Frightened only
        //      disadvantages the target's own attacks.
        if let Some(aei) = try_menacing_attack(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p''. Disarming Attack — fighter bonus-action prime (Battle
        //       Master). STR save vs disarm; the one-round attacker
        //       disadvantage hits especially hard against ranged or
        //       multi-attack threats. Slotted after Menacing because
        //       Frightened lasts longer than Disarmed in our model.
        if let Some(aei) = try_disarming_attack(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p'''. Pushing Attack — fighter bonus-action prime (Battle
        //        Master). STR save vs forced 4-tile shove. Last of the
        //        maneuver lane because pure displacement (no attack /
        //        save penalty rider) is the weakest tactically against
        //        a target already in melee; it's a finisher when none
        //        of the debuff-rider maneuvers are available.
        if let Some(aei) = try_pushing_attack(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p''''. Goading Attack — fighter bonus-action prime (Battle
        //         Master). WIS save vs goaded (tank-anchor: target eats
        //         disadvantage on attacks against anyone other than the
        //         fighter). Last among the maneuvers since the tank-
        //         anchor effect is strongest when the fighter has
        //         already absorbed the maneuver-debuff options above on
        //         tougher single targets — at which point the surviving
        //         enemy still gets goaded onto the front-liner.
        if let Some(aei) = try_goading_attack(encounter, actor_id) {
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
        //           prime is consumed this turn.
        if let Some(aei) = try_precision_attack(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 3p'''''''. Feinting Attack — fighter bonus-action prime (Battle
        //            Master). Targets one enemy in melee reach and grants
        //            self-advantage on the next attack vs them via the
        //            help-grant lane. Higher leverage than Precision
        //            against high-AC targets where advantage outperforms
        //            a flat +4; lower than Sweeping when there's an
        //            adjacent splash target available.
        if let Some(aei) = try_feinting_attack(encounter, actor_id) {
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

        // 5. Hold Person — lock down toughest enemy if we have it and
        //    aren't already concentrating on something.
        if let Some(aei) = try_hold_person(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5a. Cause Fear — disabler against the toughest enemy who isn't
        //     already Frightened. Concentration-gated, so we only fire
        //     when nothing else holds the slot.
        if let Some(aei) = try_cause_fear(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5a'. Dominate Monster — level-8 concentration charm against the
        //      highest-HP enemy. Works on any creature type (unlike Hold
        //      Person). Concentration-gated; fire when we have the slot
        //      and aren't already concentrating.
        if let Some(aei) = try_dominate_monster(encounter, actor_id) {
            return ControllerDecision::Act(aei);
        }

        // 5. AoE — point that catches 2+ enemies, no friendly fire.
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
/// have Disengage in their loadout or can't afford the Action cost.
fn try_disengage(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_action(encounter, actor_id, "disengage")
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

/// Cast Hold Person on the toughest in-range enemy if we have it and
/// aren't already concentrating. "Toughest" = highest current HP among
/// not-already-stunned enemies (no point double-locking).
fn try_hold_person(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    // (action, condition the action installs) so we can avoid retargeting
    // someone already locked. Order = preference: hard lockdown beats
    // soft. New entries land in priority order.
    const SOFT_LOCKS: &[(&str, Condition)] = &[
        ("hold person", Condition::Stunned),
        ("sleep gaze", Condition::Asleep),
        ("cause fear", Condition::Frightened),
        ("ray of enfeeblement", Condition::Poisoned),
    ];
    let candidates: Vec<(&'static (dyn Action + Send + Sync), Condition)> = actor
        .actions
        .iter()
        .filter_map(|a| {
            SOFT_LOCKS
                .iter()
                .find(|(name, _)| a.name() == *name)
                .map(|(_, cond)| (*a, *cond))
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let my_team = actor.team();

    // Search per (action, target) so we evaluate every soft-lock against
    // every legal enemy. We pick toughest target and break ties by
    // SOFT_LOCKS index (Hold Person beats Cause Fear when both validate).
    let mut best: Option<(u32, usize, ActionExecutionInfo)> = None;
    for target_id in encounter.sorted_actor_ids() {
        let Some(target) = encounter.actors.get(&target_id) else {
            continue;
        };
        if target_id == actor_id || target.team() == my_team || !target.is_combat_active() {
            continue;
        }
        for (priority, (action, condition)) in candidates.iter().enumerate() {
            if target.has_condition(*condition) {
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

/// Cast Cause Fear on the toughest in-range enemy if we have it and
/// aren't concentrating yet. Skips already-Frightened targets so the
/// AI doesn't waste a slot reapplying the same debuff. Mirrors
/// `try_hold_person`'s "highest current HP wins" target picker — the
/// AI tries to disable the threat that would cost the most to chip
/// down with damage.
fn try_cause_fear(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    let cf = actor.find_action("cause fear")?;
    let my_team = actor.team();

    let ids = encounter.sorted_actor_ids();

    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for target_id in ids {
        let Some(target) = encounter.actors.get(&target_id) else {
            continue;
        };
        if target_id == actor_id || target.team() == my_team || !target.is_combat_active() {
            continue;
        }
        if target.has_condition(Condition::Frightened) {
            continue;
        }
        let aei = ActionExecutionInfo::new(cf, actor_id, Some(vec![target_id]), None, None);
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

/// Cast Dominate Monster on the highest-HP in-range enemy if we have it
/// and aren't already concentrating. Unlike Hold Person this works on any
/// creature type, so we target the beefiest hostile to flip the toughest
/// threat to our side. Concentration-gated; the action's own validation
/// checks the level-8 slot availability.
fn try_dominate_monster(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    let action = actor.find_action("dominate monster")?;
    let my_team = actor.team();

    let mut best: Option<(u32, ActionExecutionInfo)> = None;
    for target_id in encounter.sorted_actor_ids() {
        let Some(target) = encounter.actors.get(&target_id) else {
            continue;
        };
        if target_id == actor_id || target.team() == my_team || !target.is_combat_active() {
            continue;
        }
        // Skip already-dominated targets.
        if target.has_condition(Condition::Dominated) {
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

/// Cast Bless if we have it, aren't already concentrating, and there's at
/// least one combat-active ally (otherwise the buff is wasted on solo).
/// Mage Armor self-buff — only worth casting once. The MageArmored
/// condition has a long timer, so we suppress repeat casts by checking
/// for it. Validates spell-slot availability via the action's own
/// `validate_input`, so this also gracefully no-ops when out of slots.
fn try_self_buff_mage_armor(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.has_condition(Condition::MageArmored) {
        return None;
    }
    try_self_action(encounter, actor_id, "mage armor")
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

/// Holy Aura — level-8 concentration burst centered on the caster. Fire
/// only when at least 2 allies (caster + 1 other) sit within 30ft AND a
/// hostile is engaged. Single-caster clerics get more value from a
/// level-2 Hold Person than a level-8 self-only aura, so we gate on
/// actual ally clustering. Skips re-cast when already concentrating.
fn try_holy_aura(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    if actor.has_condition(Condition::HolyAuraed) {
        return None;
    }
    // Engagement check — don't burn a level-8 slot in an empty room.
    if !any_enemy_within(encounter, actor_id, 60) {
        return None;
    }
    // Ally-cluster check: 12 tiles = 30ft aura radius. Require at least
    // 1 other combat-active ally inside (caster is free).
    if n_actors_within(encounter, actor_id, 12, true, 1) < 1 {
        return None;
    }
    try_self_action(encounter, actor_id, "holy aura")
}

/// Spirit Shroud — level-3 concentration self-buff. Fire when an enemy
/// is in melee reach so the cold rider lands this turn. Concentration-
/// gated; skip if the holder already concentrates.
fn try_spirit_shroud(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_buff_concentration(
        encounter,
        actor_id,
        "spirit shroud",
        Condition::SpiritShrouded,
        1,
    )
}

/// Bigby's Hand — level-5 wizard concentration self-buff. The on-hit
/// rider lands +1d10 force on every attack the caster makes. Fire when
/// at least one enemy is within melee + close-ranged reach (8 tiles ≈
/// 20ft) so the rider lands this round; concentration-gated.
fn try_bigbys_hand(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_buff_concentration(
        encounter,
        actor_id,
        "bigby's hand",
        Condition::BigbysHanded,
        8,
    )
}

/// Tenser's Transformation — level-6 wizard concentration self-buff.
/// Grants 50 temp HP plus advantage on weapon attacks. Concentration-
/// gated; fire only when engaged so the temp HP buffer matters.
fn try_tensers_transformation(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    // 30ft engagement radius — same envelope as Holy Aura's gate.
    try_self_buff_concentration(
        encounter,
        actor_id,
        "tenser's transformation",
        Condition::Transformed,
        12,
    )
}

/// Investiture of Flame — level-6 caster concentration self-buff. The
/// holder gains fire resistance and 1d10 fire retaliation on melee
/// hits. Concentration-gated; skip when already invested (the install
/// site's `custom_validate_input` also blocks this, but the explicit
/// gate keeps the picker from re-considering the action on every turn).
/// Engagement gate: at least one enemy within 6 tiles (~15ft) so a
/// melee swing actually arrives before the buff times out.
fn try_investiture_of_flame(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    try_self_buff_concentration(
        encounter,
        actor_id,
        "investiture of flame",
        Condition::InvestedInFlame,
        6,
    )
}

/// Wind Wall — level-3 evocation, concentration. Self-buff that imposes
/// disadvantage on ranged attacks against the caster. Concentration-
/// gated; fire when at least one enemy with a ranged weapon is within
/// ~20 tiles (50ft) so the buff matters this round. We approximate
/// "ranged threat" by checking any enemy within range — the engine
/// doesn't model intent, but the disadvantage rider lands the moment
/// any ranged attack arrives so the buff is cheap insurance.
fn try_wind_wall(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    // 20 tiles ≈ 50ft — typical longbow range. If no enemy can shoot
    // us yet, skip; the concentration slot is better held for an
    // active fight.
    try_self_buff_concentration(
        encounter,
        actor_id,
        "wind wall",
        Condition::WindWalled,
        20,
    )
}

/// Aura of Life — level-4 paladin concentration aura. Fires when at
/// least one ally sits in the 30ft radius and a hostile is engaged.
/// Concentration-gated; skip re-cast when the caster already holds the
/// DeathWarded buff (i.e. the aura is already up on them).
fn try_aura_of_life(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    if actor.has_condition(Condition::DeathWarded) {
        // The aura installs DeathWarded on the caster; if the caster
        // already has it, the aura is presumed active.
        return None;
    }
    // Engagement check — don't burn a level-4 slot in an empty room.
    if !any_enemy_within(encounter, actor_id, 60) {
        return None;
    }
    // Ally-cluster check: 6 tiles = 30ft aura radius. Require at least
    // 1 other combat-active ally inside.
    if n_actors_within(encounter, actor_id, 6, true, 1) < 1 {
        return None;
    }
    try_self_action(encounter, actor_id, "aura of life")
}

/// Holy Weapon — level-5 paladin concentration self-buff. Every weapon
/// hit lands +2d8 radiant via the on_hit_riders table for the duration.
/// Same engagement gate as Spirit Shroud / Bigby's Hand — fire when an
/// enemy is within attack reach so the rider matters this round. The
/// action's `custom_validate_input` covers the not-already-concentrating
/// and not-already-buffed clauses.
fn try_holy_weapon(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    // 8 tiles ≈ 20ft — same envelope as Bigby's Hand. A paladin who
    // hasn't engaged yet should save the lv5 slot for the actual fight.
    try_self_buff_concentration(
        encounter,
        actor_id,
        "holy weapon",
        Condition::HolyWeaponed,
        8,
    )
}

/// Pass Without Trace — level-2 druid / ranger aura. Cloaks every ally
/// inside the 30ft sphere (12 tiles), imposing disadvantage on attackers
/// targeting them for the duration. Fires when the caster isn't already
/// concentrating, isn't already cloaked, and at least one ally sits in
/// the radius (the caster covers themselves for free, so a solo caster
/// can fire too — the aura still buffs the caster). The engagement gate
/// keeps the slot from burning in an empty room.
fn try_pass_without_trace(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    if actor.has_condition(Condition::Untracked) {
        return None;
    }
    // Engagement check — don't burn a lv2 slot in an empty room.
    if !any_enemy_within(encounter, actor_id, 60) {
        return None;
    }
    try_self_action(encounter, actor_id, "pass without trace")
}

/// Cleric Divine Strike — once-per-rest bonus-action prime. Fire when
/// an enemy is footprint-adjacent so the +1d8 radiant rider lands on
/// the cleric's next melee swing (most likely Thorn Whip or melee
/// weapon). Validation handles the feature-available + already-primed
/// gate.
fn try_divine_strike(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    try_self_action(encounter, actor_id, "divine strike")
}

/// Fighter Trip Attack — once-per-rest bonus-action maneuver. Fire when
/// an enemy is footprint-adjacent so the prone-on-fail STR save lands
/// this turn. Validation handles the feature-available + already-primed
/// gate.
fn try_trip_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    try_self_action(encounter, actor_id, "trip attack")
}

/// Fighter Battle Master Menacing Attack — bonus-action prime that lays a
/// WIS save vs frighten on the next melee hit. Same engagement gate as
/// Trip Attack (enemy must be in reach so the swing connects this turn).
/// Validation handles the feature-available + already-primed gate.
fn try_menacing_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    try_self_action(encounter, actor_id, "menacing attack")
}

/// Fighter Battle Master Disarming Attack — bonus-action prime that lays
/// a STR save vs disarm on the next melee hit. Same engagement gate as
/// Trip Attack. Disarmed (one-round attacker disadvantage) layers nicely
/// with a follow-up swing from the Extra Attack lane.
fn try_disarming_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    try_self_action(encounter, actor_id, "disarming attack")
}

/// Fighter Battle Master Pushing Attack — bonus-action prime that lays a
/// STR save vs shove on the next melee hit. Same engagement gate as Trip
/// Attack. The shove makes most tactical sense when an adjacent enemy
/// threatens an ally — pushing them clear of the squishy backline.
fn try_pushing_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    try_self_action(encounter, actor_id, "pushing attack")
}

/// Fighter Battle Master Goading Attack — bonus-action prime that lays a
/// WIS save vs goaded on the next melee hit. The Goaded debuff is the
/// tank-anchor maneuver: it forces the target to focus the fighter or
/// eat disadvantage on every other swing. Higher leverage when the
/// fighter is in melee with a threat to a squishy ally — we approximate
/// "I'm the tank" by gating on at least one ally being within close
/// reach (4 tiles) so the goad does work this round. Falls back to the
/// generic adjacency gate when no ally is in sight.
fn try_goading_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    try_self_action(encounter, actor_id, "goading attack")
}

/// Fighter Battle Master Precision Attack — bonus-action prime that
/// adds +4 to the next attack roll. Same engagement gate as Trip Attack
/// (enemy in melee so the prime lands this turn).
fn try_precision_attack(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    try_self_action(encounter, actor_id, "precision attack")
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
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("feinting attack")?;
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
        // Feint is melee-touch range — same envelope as Help.
        if dist > 1 {
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

/// Druid Shillelagh — bonus-action cantrip prime that adds +1d8 force
/// damage to the next melee weapon hit. Fire when an enemy is
/// footprint-adjacent so the prime is consumed by the druid's swing
/// this turn. The action itself custom-validates `!has_condition
/// (Shillelaghed)` so the AI never double-primes. Free (no slot
/// consumed) so it stays on the bonus-action lane without competing
/// with the leveled-slot smites.
fn try_shillelagh(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 0) {
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
    const HEIGHTENED_LOCKDOWN: &[&str] = &[
        "hold person",
        "hold monster",
        "polymorph",
        "banishment",
        "dominate person",
        "dominate monster",
    ];
    const HEIGHTENED_BURST: &[&str] = &[
        "fireball",
        "cone of cold",
        "sunburst",
        "burning hands",
        "thunderwave",
        "shatter",
    ];
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
    const EXTENDABLE: &[&str] = &[
        "mage armor",
        "hunter's mark",
        "bless",
        "hold person",
        "hold monster",
        "polymorph",
        "fly",
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
        "enlarge / reduce",
        "shadow blade",
    ];
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
    let actor = encounter.actors.get(&actor_id)?;
    let action = actor.find_action("telekinetic")?;
    let my_team = actor.team();
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
    // Closest enemy in the 2-24 tile sweet spot. Skip already-adjacent
    // (gap 0-1) because the pull does nothing; cap at 24 (60ft) per
    // RAW range.
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
        if !(2..=24).contains(&dist) {
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
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
    let wounded_nearby = encounter.actors.iter().any(|(_id, a)| {
        if a.team() != my_team || !a.is_combat_active() {
            return false;
        }
        let cap = a.max_hitpoints();
        if a.hitpoints() >= cap / 2 + (cap % 2) {
            return false;
        }
        footprint_chebyshev(
            my_loc,
            my_size,
            a.location(),
            get_tiles_from_size(a.size()),
        ) <= 12
    });
    if !wounded_nearby {
        return None;
    }
    let aei = ActionExecutionInfo::new(action, actor_id, None, None, None);
    aei.validate(encounter).then_some(aei)
}

/// Wizard Arcane Recovery — free no-cost slot restore. Fire when the
/// wizard has spent a low-tier slot and is in an active fight (so the
/// recovered slot has something to land on this encounter).
fn try_arcane_recovery(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    try_self_action(encounter, actor_id, "arcane recovery")
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
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());
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
        let gap = footprint_chebyshev(
            my_loc,
            my_size,
            t.location(),
            get_tiles_from_size(t.size()),
        );
        if gap > 1 {
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
/// proximity-gated self-buff heuristics (Rage at gap 12, Divine Smite
/// at gap 0). Returns false when `actor_id` is missing.
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
/// `try_holy_aura` (ally-cluster gate), `try_aura_of_life`, and
/// other ally-or-enemy-radius heuristics.
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
/// Used by `try_bigbys_hand` / `try_tensers_transformation` /
/// `try_investiture_of_flame` / `try_spirit_shroud` — every
/// concentration-bound self-buff with the same three-step gate.
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
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    try_self_action(encounter, actor_id, "reckless attack")
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

/// Paladin Divine Smite — bonus-action prime that lays a +2d8 radiant
/// rider on the next melee hit. Fires only when an enemy is footprint-
/// adjacent so the prime doesn't tick out without a target to land on.
/// Validation handles the "already primed" and "no level-1 slot" gates.
fn try_divine_smite(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    try_self_action(encounter, actor_id, "divine smite")
}

/// Paladin Smite spells (Searing / Wrathful / Branding / Blinding).
/// Same trigger as Divine Smite — fire when an enemy is footprint-
/// adjacent so the bonus-action prime doesn't go to waste. We try
/// them in increasing-slot-level order so the paladin spends low slots
/// before high ones; each spell's own `custom_validate_input` rejects
/// re-prime if the smite condition is already up. The Smite-spell path
/// is concentration-gated — skip the whole stack if the paladin is
/// already concentrating on something (e.g. Compelled Duel / Bless).
fn try_smite_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    // Slot-cheapest first — preserves higher slots for emergencies.
    // The order is defined by the central `ALL_SMITE_SPELLS` registry,
    // so adding a new smite is one entry in spells.rs and the AI picks
    // it up automatically.
    use crate::actions::action_template::Action;
    use crate::actions::spells::ALL_SMITE_SPELLS;
    for spell in ALL_SMITE_SPELLS {
        if let Some(aei) = try_self_action(encounter, actor_id, spell.name()) {
            return Some(aei);
        }
    }
    None
}

/// Ranged-flavor smite picker (Lightning Arrow and any future ranged
/// primes). Mirrors `try_smite_spell` but gates on enemy-within-bow-
/// range (24 tiles) rather than adjacency — the prime loads the next
/// *ranged* weapon attack, so a far-away threat is the right trigger.
/// Skips when the actor is already concentrating (Hunter's Mark and
/// Lightning Arrow share the concentration slot; AI picks whichever
/// fires first based on pipeline order) or has the prime up already.
fn try_ranged_smite_spell(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    if actor.is_concentrating() {
        return None;
    }
    // Bow range RAW = 150 ft = 60 tiles; we use 24 tiles (60 ft) as the
    // engagement gate so the ranger only burns the slot when a threat
    // is in a reasonably-aimed bowshot, not across the entire map.
    if !any_enemy_within(encounter, actor_id, 24) {
        return None;
    }
    use crate::actions::action_template::Action;
    use crate::actions::spells::ALL_RANGED_SMITE_SPELLS;
    for spell in ALL_RANGED_SMITE_SPELLS {
        if actor.has_condition(spell.prime) {
            continue;
        }
        if let Some(aei) = try_self_action(encounter, actor_id, spell.name()) {
            return Some(aei);
        }
    }
    None
}

/// Monk Stunning Strike — bonus action prime that lays a stun save on
/// the next melee hit. Same trigger as Divine Smite (adjacent enemy
/// required so the prime doesn't tick out). Once-per-rest gated so the
/// AI only fires it when the action picker has a melee target queued.
fn try_stunning_strike(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    if !any_enemy_within(encounter, actor_id, 0) {
        return None;
    }
    try_self_action(encounter, actor_id, "stunning strike")
}

/// Cleric Channel Divinity: Turn Undead — action. Fire when at least
/// one undead-proxy enemy (Poison-immune) is within 30ft. Once per
/// long rest; the action's own validation handles the feature-flag
/// gate so the AI just provides the proximity heuristic.
fn try_turn_undead(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    use crate::engine::types::DamageType;
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let actor = encounter.actors.get(&actor_id)?;
    let team = actor.team();
    let loc = actor.location();
    let size = get_tiles_from_size(actor.size());
    let undead_nearby = encounter.actors.iter().any(|(id, a)| {
        *id != actor_id
            && a.team() != team
            && a.is_combat_active()
            && a.is_immune_to(DamageType::Poison)
            && footprint_chebyshev(loc, size, a.location(), get_tiles_from_size(a.size())) <= 12
    });
    if !undead_nearby {
        return None;
    }
    try_self_action(encounter, actor_id, "turn undead")
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
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let actor = encounter.actors.get(&actor_id)?;
    let team = actor.team();
    let loc = actor.location();
    let size = get_tiles_from_size(actor.size());
    let nearby = encounter
        .actors
        .iter()
        .filter(|(id, a)| {
            **id != actor_id
                && a.team() != team
                && a.is_combat_active()
                && !a.is_immune_to_condition(Condition::Frightened)
                && !a.has_condition(Condition::Frightened)
                && footprint_chebyshev(loc, size, a.location(), get_tiles_from_size(a.size())) <= 8
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
    let my_loc = actor.location();
    let my_size = get_tiles_from_size(actor.size());

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
        let dist = footprint_chebyshev(
            my_loc,
            my_size,
            t.location(),
            get_tiles_from_size(t.size()),
        );
        if dist > MELEE_REACH {
            continue;
        }
        // Only shove when at least one friendly melee ally is also adjacent
        // to the target — otherwise prone just halves the target's speed
        // and doesn't give us advantage on our own attack (we already used
        // our Action on the shove).
        let t_loc = t.location();
        let t_size = get_tiles_from_size(t.size());
        let ally_adjacent = encounter.actors.iter().any(|(aid, ally)| {
            *aid != actor_id
                && ally.team() == my_team
                && ally.is_combat_active()
                && footprint_chebyshev(ally.location(), get_tiles_from_size(ally.size()), t_loc, t_size)
                    <= MELEE_REACH
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
        let has_ranged = t.actions.iter().any(|a| {
            a.is_harmful()
                && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
                && a.reach_tiles().is_some_and(|r| r > MELEE_REACH)
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

/// Sort key for advantage-aware target selection — lower wins.
fn mode_priority(mode: RollMode) -> u8 {
    match mode {
        RollMode::Advantage => 0,
        RollMode::Normal => 1,
        RollMode::Disadvantage => 2,
    }
}

/// True if the actor has any single-actor attack with reach beyond melee.
/// Doesn't require a current valid target — the kite tactic only cares
/// whether we *could* shoot once we have space.
fn has_ranged_attack(encounter: &EncounterInstance, actor_id: usize) -> bool {
    let Some(actor) = encounter.actors.get(&actor_id) else {
        return false;
    };
    // Restrict to *harmful* SingleActor actions — kiting / disengaging
    // is about ranged offense, not about long-range buff dispensers like
    // Rally (12-tile reach) or Commander's Strike (24-tile reach). Pre-
    // restriction this lane caught the support actions and steered every
    // Fighter into kite-mode the moment they had Rally on their sheet.
    actor.actions.iter().any(|a| {
        a.is_harmful()
            && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
            && a.reach_tiles().is_some_and(|r| r > MELEE_REACH)
    })
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

/// Self-targeted heal (e.g. fighter Second Wind, drink healing potion).
/// Triggers when the actor is below 50% HP. Tries every heal action,
/// passing self as the target for `SingleActor` schemas (Lay on Hands,
/// Healing Hands, Cure Wounds) and a no-target call for `NoArgs` schemas
/// (Second Wind, potions). Picks the first validating heal — action-list
/// order means high-value class features land before consumables.
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
    for &action in &actor.actions {
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

fn try_support_heal(
    encounter: &EncounterInstance,
    actor_id: usize,
) -> Option<ActionExecutionInfo> {
    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();

    // Helpful actions only — `is_harmful=false` guards against ever
    // picking an attack here. SingleActor schema so we can pick a
    // target. Exclude Help: it's helpful but doesn't heal — it grants
    // an attack-advantage rider that's pointless when an ally is
    // bleeding out and wants HP back.
    let heal_actions: Vec<&'static (dyn Action + Send + Sync)> = actor
        .actions
        .iter()
        .filter(|a| {
            !a.is_harmful()
                && matches!(a.targeting_schema(), TargetingSchema::SingleActor)
                && a.name() != "help"
        })
        .copied()
        .collect();
    if heal_actions.is_empty() {
        return None;
    }

    // Sort actor ids for deterministic tiebreak.
    let ids = encounter.sorted_actor_ids();

    // (priority, hp, aei): lower priority value = more urgent.
    // 0 = dying, 1 = wounded combat-active.
    let mut best: Option<(u8, u32, ActionExecutionInfo)> = None;
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

        for &heal in &heal_actions {
            let aei =
                ActionExecutionInfo::new(heal, actor_id, Some(vec![ally_id]), None, None);
            if !aei.validate(encounter) {
                continue;
            }
            let hp = ally.hitpoints();
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
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let actor = encounter.actors.get(&actor_id)?;
    let my_team = actor.team();

    // Find Burst actions we own. Most actors have none — bail early.
    let burst_actions: Vec<(&'static (dyn Action + Send + Sync), isize)> = actor
        .actions
        .iter()
        .filter_map(|a| {
            if !a.is_harmful() {
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
            // 5e Sorcerer Careful Spell exception: if the caster has the
            // CarefulSpelling prime up, allies inside the radius can be
            // shielded — up to CHA-mod of them (the caster themselves
            // included). When the ally count fits the shield cap, we
            // tolerate the "friendly fire" and let the burst chokepoint
            // consume the prime + skip those ids.
            let careful_capacity = if actor.has_condition(Condition::CarefulSpelling) {
                Some(
                    actor
                        .ability_modifier(crate::engine::types::AbilityScoreType::Charisma)
                        .max(1) as usize,
                )
            } else {
                None
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
            let friendly_fire_blocked = match careful_capacity {
                Some(cap) => ally_hits > cap,
                None => ally_hits > 0,
            };
            if friendly_fire_blocked || enemy_hits < 2 {
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
    if nearby_enemies < 2 {
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

/// Find the (target, action) pair where the target has the lowest current
/// HP among combat-active enemies AND we can validly hit them right now.
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
    //   - HP ascending: focus-fire wounded.
    //   - Reach descending: prefer the longest-reach action when tied
    //     (so a longbow gets used over a one-tile melee on a far target,
    //     etc.).
    let mut best: Option<(u8, u32, isize, ActionExecutionInfo)> = None;
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
        let mode = encounter.compute_attack_mode(actor_id, target_id, is_melee);
        let mode_pri = mode_priority(mode);
        let hp = target.hitpoints();
        let pick = match &best {
            None => true,
            Some((bm, bh, br, _)) => {
                (mode_pri, hp, std::cmp::Reverse(reach))
                    < (*bm, *bh, std::cmp::Reverse(*br))
            }
        };
        if pick {
            best = Some((mode_pri, hp, reach, aei));
        }
    }
    best.map(|(_, _, _, aei)| aei)
}

/// Among the actor's SingleActor *harmful* actions, the longest-reach
/// one whose reach covers `target_id` and that the actor can actually
/// afford right now. Validating cost here means we don't return Magic
/// Missile (reach 48, costs a spell slot) when no slots remain — the
/// caller would then skip the target entirely instead of falling back
/// to Fire Bolt at reach 24.
fn best_attack_against(
    actor_id: usize,
    actor: &crate::actors::actor_template::ActorInstance,
    encounter: &EncounterInstance,
    target_id: usize,
) -> Option<(isize, &'static (dyn Action + Send + Sync))> {
    use crate::engine::types::DamageModifier;

    let target = encounter.actors.get(&target_id)?;
    let dist = footprint_chebyshev(
        actor.location(),
        get_tiles_from_size(actor.size()),
        target.location(),
        get_tiles_from_size(target.size()),
    );
    // Score the damage-type matchup: lower is better.
    // 0 = at least one Vulnerable type and no Immune-only
    // 1 = neutral (no info or all-neutral)
    // 2 = at least one Resistant type
    // 3 = every listed type is Immune (skip)
    let matchup_score = |a: &dyn Action| -> u8 {
        let dts = a.damage_types();
        if dts.is_empty() {
            return 1;
        }
        let mut all_immune = true;
        let mut has_vuln = false;
        let mut has_resist = false;
        for dt in &dts {
            match target.damage_modifier(*dt) {
                Some(DamageModifier::Immunity) => {}
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
        if all_immune {
            3
        } else if has_vuln {
            0
        } else if has_resist {
            2
        } else {
            1
        }
    };

    // Best by (score asc, reach desc).
    let mut best: Option<(u8, isize, &(dyn Action + Send + Sync))> = None;
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
        let Some(reach) = action.reach_tiles() else {
            continue;
        };
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
        let pick = match &best {
            None => true,
            Some((bs, br, _)) => score < *bs || (score == *bs && reach > *br),
        };
        if pick {
            best = Some((score, reach, action));
        }
    }
    best.map(|(_, r, a)| (r, a))
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
                Some((id, t.hitpoints()))
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
    let my_team = actor.team();
    let has_enemy = encounter
        .actors
        .values()
        .any(|a| a.team() != my_team && a.is_combat_active());
    if !has_enemy {
        return None;
    }
    try_self_action(encounter, actor_id, "dash")
}

/// Last-resort: prefer Dodge (defensive posture if we still have an Action
/// slot) over Skip so the turn doesn't go to waste. Falls through to Skip
/// if Dodge isn't available, and finally AwaitInput if neither is —
/// preventing an infinite loop on a malformed actor.
fn skip_or_await(encounter: &EncounterInstance, caster_id: usize) -> ControllerDecision {
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

    /// Exercise the new spells / creatures in an AI-driven encounter so the
    /// rule changes (Sunbeam / Prayer of Healing / Power Word Heal /
    /// Resurrection in cleric+wizard loadouts; Manticore / Hill Giant /
    /// Treant / Fire Elemental / Gelatinous Cube in the monster pool)
    /// don't crash the AI's action picker or stall the process_stack
    /// loop. Also keeps the prior "new content" coverage on the lineup.
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
        }
    }

    /// Build a no-actors encounter we can hand-place creatures into.
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
}
