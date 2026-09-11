//! **Feats** — SRD 5.2's fourth character-building axis, and the one
//! the engine had exactly one of.
//!
//! A class gives you a chassis, a subclass gives it a direction, a
//! species gives it a body; a feat is the thing a character picks that
//! none of those three decided. SRD 5.2 sorts them into four
//! categories — Origin, General, Fighting Style and Epic Boon — and the
//! engine already carries the whole Fighting Style column (Archery,
//! Defense, Great Weapon Fighting, Two-Weapon Fighting, plus the
//! Protection and Interception styles) on template flags of its own,
//! because those arrived as class features rather than as feats.
//!
//! What was missing is everything else. Entries were chosen on one
//! axis: does the feat change a die the engine already rolls?
//!
//! | feat | category | what it moves |
//! |------|----------|---------------|
//! | Alert | Origin | the initiative roll |
//! | Savage Attacker | Origin | one weapon damage roll a turn |
//! | Tough | Origin | the hit point maximum, by twice the level |
//! | Grappler | General | attack rolls against what you are holding |
//! | Speedy | General | ten feet of walking speed |
//! | Charger | General | 1d8 on the swing at the end of a run |
//! | War Caster | General | the concentration save |
//! | Boon of Combat Prowess | Epic Boon | one miss a turn becomes a hit |
//! | Boon of Dimensional Travel | Epic Boon | thirty feet after the swing |
//! | Boon of Fate | Epic Boon | 2d4 onto a d20 that came up short |
//! | Boon of Irresistible Offense | Epic Boon | resistance to B/P/S, and the nat 20 |
//! | Boon of Spell Recall | Epic Boon | one slot in four comes back |
//! | Boon of the Night Spirit | Epic Boon | what the dark is worth to its holder |
//! | Boon of Truesight | Epic Boon | sixty feet of seeing through |
//!
//! ## The Epic Boon column
//!
//! SRD 5.2 prints seven Epic Boon feats and the engine had a shelf for
//! none of them. They are all here, which makes the Epic Boon category
//! the first feat column the engine carries *whole* — Origin is missing
//! Magic Initiate and Skilled, General is missing Ability Score
//! Improvement, and both absences are the same absence: a level-up lane
//! and an out-of-combat economy the engine does not have.
//!
//! Every boon is a level-19+ feat, so none of them belongs on a chassis
//! by seniority — every playable template in the engine would be too
//! junior for all seven. They are placed instead by *identity*: each
//! boon ships on the one subclass whose own capstone it reads as the
//! epic extension of, and no chassis carries two. A Champion fighter
//! whose whole subclass is the widened crit range is who Peerless Aim
//! belongs to; a Divination wizard who already rewrites one d20 a rest
//! is who Improve Fate belongs to. The placements are named on each
//! constant below, and the sweep in
//! `every_feat_is_carried_by_a_playable_chassis` is what keeps a boon
//! from quietly becoming a constant nobody can take.
//!
//! ## What is still not here
//!
//! The General column is long and this file is not. Every feat above
//! earned its place by landing on a lane that already existed; the ones
//! still missing are missing because they need a lane nobody has built,
//! and the two shapes recur. **A mid-roll choice** — Lucky's luck
//! points, Great Weapon Master's bonus-action follow-up, Sentinel's
//! reaction — needs a channel for a decision taken between a roll and
//! its consequence, which the engine has in exactly one place (the
//! reaction dispatcher) and cannot generalise cheaply. **A swing bound
//! to a weapon** — Polearm Master, Dual Wielder, Crusher/Piercer/Slasher
//! — needs the attack pipeline to know which object made the attack,
//! and it does not: see `conditions::Condition::DragonSlaying` for the
//! same absence viewed from the magic armoury.
//!
//! ## Why tags rather than fields
//!
//! `CreatureTemplate` carries some thirty `has_*` booleans for exactly
//! this kind of passive, and every one of them is a field the struct
//! and its `defaults()` have to know about. A feat is the case that
//! argues hardest against another: feats are *many* and each is small,
//! so they ride `features` — the same `HashSet<&'static str>` the
//! class-feature tags use — and reach the engine through
//! `ActorInstance::has_passive_feature`. Adding a fourth feat is one
//! constant here, one row in a cohort table, and one line on whichever
//! chassis takes it.
//!
//! ## What each feat gives up
//!
//! Almost every feat here has clauses with no engine surface, and they
//! are named per-feat below rather than quietly dropped. The pattern is
//! the same one the rest of the engine follows: the clause that moves a
//! die ships, and the clause that needs a subsystem nobody has built
//! (an out-of-combat economy, a choice prompt mid-roll, a
//! drag-a-body movement lane) is written down as absent.
//!
//! One clause is absent from all seven boons for one reason, so it is
//! written down once here instead of seven times below: **every Epic
//! Boon opens with an Ability Score Increase**, and the engine builds
//! finished stat blocks with no level-up lane to apply one in. The
//! exception is Boon of Irresistible Offense, whose second clause
//! *reads* the score the first one raised — that one is spelled out on
//! its own constant, because there the absence changes a number.

/// **Alert** (Origin feat) — *"When you roll Initiative, you can add
/// your Proficiency Bonus to the roll."*
///
/// One row on `PROFICIENCY_INITIATIVE_BONUSES`, alongside the Watchers
/// Paladin's Aura of the Sentinel, which is the same full-proficiency
/// bump arriving from a different direction. That cohort's own docstring
/// has been listing "a hypothetical Alert-style feat" as its example of
/// a future row since it was written.
///
/// Going earlier in the order is worth more than it looks in this
/// engine: the opening round decides who is standing where when the
/// first area spell lands, and a caster who beats the enemy line to
/// initiative gets their concentration up before anything can break it.
///
/// **Initiative Swap is not modeled.** RAW lets the holder trade
/// initiative with a willing ally immediately after rolling. The engine
/// has no channel for a decision taken between the roll and the order
/// being fixed, and no AI heuristic for whether a swap is worth making;
/// a swap made badly is worse than none.
pub const ALERT_TAG: &str = "feat.alert";

/// **Savage Attacker** (Origin feat) — *"Once per turn when you hit a
/// target with a weapon, you can roll the weapon's damage dice twice
/// and use either roll against the target."*
///
/// Read at the weapon-damage chokepoint in `engine::attack`, through the
/// shared once-per-turn ledger every other "once on your turn" rider in
/// the engine uses (`ONCE_PER_TURN_RIDER_TAGS`).
///
/// "Either roll" is resolved as the better one. RAW leaves the choice
/// to the player and there is no reason a player would ever take the
/// smaller number, so the choice is not a choice — the same reading
/// Portent's substitution and the Halfling's Lucky reroll already use.
///
/// The whole pool is rerolled, crit dice included, because RAW's unit is
/// "the weapon's damage dice" for that attack and a critical hit's
/// doubled dice are those dice. Flat modifiers are not rerolled, which
/// is the same rule the crit doubling itself follows.
///
/// Weapon attacks only. A Fire Bolt is not a weapon, and the spell lane
/// rolls its damage through a different chokepoint anyway.
pub const SAVAGE_ATTACKER_TAG: &str = "feat.savage_attacker";

/// **Grappler** (General feat) — of whose four clauses the engine
/// carries the one that moves a die: *"Attack Advantage. You have
/// Advantage on attack rolls against a creature Grappled by you."*
///
/// Read at `EncounterInstance::attack_mode_tally` off the `Grappled`
/// back-link, which is what makes "by you" enforceable: a creature held
/// by somebody else, or restrained by a Web with no grappler behind it,
/// hands this feat nothing. That link is the same one `GrappleEscape`
/// contests against and the same one Grappled's own
/// disadvantage-against-anyone-else clause reads.
///
/// It is the clause that makes grappling a *plan* rather than a way to
/// spend an Action: hold the ogre, then swing at it with advantage for
/// the rest of the fight, and watch it swing back at disadvantage
/// against everyone but you.
///
/// Three clauses are absent, each for a reason the engine states
/// elsewhere:
///
///   - **Ability Score Increase** — the engine builds finished stat
///     blocks and has no level-up lane to apply one in.
///   - **Punch and Grab** ("use both the Damage and the Grapple option
///     of an Unarmed Strike") — the engine's Grapple is its own Action
///     rather than an option on a strike, so there is no pair to fold
///     together.
///   - **Fast Wrestler** ("you don't have to spend extra movement to
///     move a creature Grappled by you") — an exemption from a cost the
///     engine does not charge. `Condition::Grappled`'s third RAW clause,
///     the one about dragging a captive along at a foot per foot, is
///     itself unmodeled; a grappler walks away and the hold breaks on
///     the range check instead.
pub const GRAPPLER_TAG: &str = "feat.grappler";

/// **Tough** (Origin feat) — *"Your Hit Point maximum increases by an
/// amount equal to twice your character level when you gain this feat.
/// Whenever you gain a level thereafter, your Hit Point maximum
/// increases by an additional 2 Hit Points."*
///
/// The two sentences collapse to one number on a finished stat block:
/// twice the level, whatever the level is now. The engine builds
/// characters at a level rather than walking them up to one, so "when
/// you gain it" and "thereafter" describe the same total.
///
/// Read at `ActorInstance::max_hitpoints`, through
/// `passive_feature_max_hp_bonus` — the lane this feat is the first
/// member of, and the reason it is a lane rather than a term: the
/// accessor already folded in a total from the inventory, and a second
/// hardcoded `if` beside it is how that accessor turns into the
/// thirty-branch pile the rest of the engine keeps tables to avoid.
///
/// Worth more in this engine than the flat number suggests, because the
/// number is not flat against anything: hit points are the one resource
/// no action in the game restores in bulk mid-fight, so twice a level
/// on the sheet is close to a free casting of Cure Wounds that arrives
/// before the fight starts and cannot be dispelled.
pub const TOUGH_TAG: &str = "feat.tough";

/// Hit points **Tough** grants per character level — RAW's *"twice your
/// character level"*, named because the multiplication happens in a
/// different file from the sentence.
pub const TOUGH_HP_PER_LEVEL: u32 = 2;

/// **Speedy** (General feat) — of whose three clauses the engine carries
/// the one that moves a number every turn: *"Your Speed increases by 10
/// feet."*
///
/// One row on `PASSIVE_FEATURE_SPEED_BONUSES`, beside the Barbarian's
/// Fast Movement and the Monk's Unarmored Movement, which are the same
/// bump arriving from a class instead of a choice.
///
/// Ten feet is four tiles on the 2.5-ft grid, and on this engine's
/// boards that is the difference between reaching the caster this turn
/// and reaching them next turn. It is also what lets a skirmisher
/// disengage out of one melee and into another in the same turn, which
/// is the fantasy the feat is sold on.
///
/// **Two clauses are absent.** *"When you take the Dash action,
/// Difficult Terrain doesn't cost you extra movement for the rest of
/// the turn"* would need the Dash action to hand a per-turn exemption
/// to the movement cost table, which has no channel for one; and
/// *"Opportunity Attacks have Disadvantage against you when you Dash"*
/// would need the opportunity-attack lane to ask what the provoker
/// spent their own turn on, which it does not record.
pub const SPEEDY_TAG: &str = "feat.speedy";

/// Feet of walking speed the **Speedy** feat grants its holder.
pub const SPEEDY_SPEED_BONUS: f32 = 10.0;

/// **Charger** (General feat) — *"Charge Attack. If you move at least 10
/// feet in a straight line immediately before hitting with a melee
/// attack as part of the Attack action, choose one: the target takes an
/// extra 1d8 damage of the weapon's type, or you push the target up to
/// 10 feet away from you."*
///
/// A `ChargeRider`, which is the lane the bestiary's boars and the
/// Cavalier's Ferocious Charger already ride — and the reason this feat
/// costs almost nothing to add is that the Cavalier got there first and
/// paid for the two columns a *character's* charge needs: `weapon: None`
/// (a character is carrying whatever the party found, where a boar has
/// tusks) and a once-per-turn ledger key (Extra Attack would otherwise
/// cash one run twice).
///
/// RAW's choice is resolved as the damage die, and this is one of the
/// few places the engine picks for the player where a player might
/// genuinely differ. A push is worth more than 1d8 exactly when the
/// shove buys a turn the party needs and the die does not finish the
/// target — a judgement about the whole board, which is what the AI
/// making the choice would have to model. The die is the answer that is
/// never wrong, only sometimes second-best.
///
/// **Improved Dash** (*"when you take the Dash action, your Speed
/// increases by 10 feet for that action"*) is absent: the engine's Dash
/// is a flat doubling of the turn's movement allowance with no per-
/// action speed to raise.
pub const CHARGER_TAG: &str = "feat.charger";

/// Charger as a `ChargeRider`, read through `ActorInstance::charge` —
/// see `CHARGER_TAG`.
///
/// `knocks_prone: false`, unlike Ferocious Charger beside it: RAW's two
/// options are a die and a push, and neither is a knockdown.
pub const CHARGER: crate::engine::attack::ChargeRider = crate::engine::attack::ChargeRider {
    weapon: None,
    dice: crate::engine::dice::Dice::new(1, 8),
    damage_type: crate::engine::types::DamageType::Bludgeoning,
    run_tiles: crate::engine::attack::charge_run_tiles(10),
    knocks_prone: false,
    label: "charger",
    knockdown_label: "charger",
    once_per_turn_tag: Some(CHARGER_TAG),
    prone_follow_up: None,
    max_target_size: None,
};

/// **War Caster** (General feat) — *"Concentration. You have Advantage
/// on Constitution saving throws that you make to maintain
/// Concentration."*
///
/// One `add_if` in `EncounterInstance::roll_concentration_save`, beside
/// the Warlock's Eldritch Mind invocation, which is the same advantage
/// arriving from a different direction — and this is the direction a
/// caster who has *chosen* to stand in melee comes from.
///
/// It is the feat that makes a front-line caster a real chassis rather
/// than a mistake. Every concentration spell worth a slot — Haste,
/// Spirit Guardians, Greater Invisibility, Hold Monster — is one hit
/// away from being wasted, and advantage on the save roughly halves how
/// often the hit costs the spell. The Bladesinger's Intelligence bonus
/// to the same save is the other half of that answer, and the two
/// compose.
///
/// **Two clauses are absent.** *"Reactive Spell"* — casting a spell in
/// place of an opportunity attack — would need the reaction dispatcher
/// to offer an action menu where it currently offers a swing, and to
/// decide which spell; that is a picker, not a flag. *"Somatic
/// Components"* is an exemption from a hand-occupancy rule the engine
/// does not enforce.
pub const WAR_CASTER_TAG: &str = "feat.war_caster";

/// **Boon of Combat Prowess** (Epic Boon) — *"Peerless Aim. When you
/// miss with an attack roll, you can hit instead. Once you use this
/// benefit, you can't use it again until the start of your next turn."*
///
/// Read at both attack chokepoints — `engine::attack::resolve_attack`
/// for weapons and `spells::spell_attack_roll` for spell attacks —
/// through the shared `EncounterInstance::peerless_aim_rescues` helper,
/// because RAW's trigger is "an attack roll" and a Fire Bolt is one.
///
/// **Placed after Bend Luck and after the underwater clause**, which is
/// the whole of the implementation's subtlety. RAW's benefit answers the
/// *final* verdict on the swing, so it has to be the last thing that
/// speaks: fired any earlier, a Wild Magic sorcerer's 1d4 would un-hit a
/// swing the boon had already paid for, and the holder would have spent
/// their one rescue on nothing.
///
/// Not on `ONCE_PER_TURN_RIDER_TAGS`'s per-*rest* cousin: the recharge
/// is "the start of your next turn", which is exactly the window the
/// shared once-per-turn ledger measures, so the boon is a ledger entry
/// rather than a charge.
///
/// **A natural 1 is rescued too.** RAW's trigger is an unqualified "when
/// you miss with an attack roll" and a fumble is a miss — the boon is
/// the one thing in the engine that reaches past the nat-1 auto-miss,
/// and it is written to. The underwater auto-miss is *not* rescued, and
/// that is a deliberate reading of a different rule: a ranged weapon
/// past its normal range underwater does not miss because the shot was
/// bad, it misses because the water stopped it, and no amount of aim
/// answers that.
///
/// Ships on `fighters::CHAMPION_TEMPLATE` — the subclass whose entire
/// identity is that its attack rolls land more often than anybody
/// else's.
pub const BOON_OF_COMBAT_PROWESS_TAG: &str = "boon.combat_prowess";

/// **Boon of Dimensional Travel** (Epic Boon) — *"Blink Steps.
/// Immediately after you take the Attack action or the Magic action, you
/// can teleport up to 30 feet to an unoccupied space you can see."*
///
/// Fires from the end-of-action hook in
/// `EncounterInstance::dispatch_blink_steps`, on any action whose cost
/// included an Action slot — which is the engine's spelling of "the
/// Attack action or the Magic action", and is slightly *narrower* than
/// RAW rather than wider: an action the engine bills as neither (a Dash,
/// a Dodge) is not a swing or a spell either, and RAW would not have
/// offered the step after it.
///
/// **Where to.** RAW asks the holder to pick a space; the engine has no
/// channel for that mid-action, so it answers the question the way the
/// AI's own repositioning rungs answer it — a holder whose reach is
/// melee steps *toward* its nearest enemy, one who fights at range steps
/// *away* from it, and one already where it wants to be does not step at
/// all. The last clause is what keeps the boon from being a twitch: a
/// creature that teleports every single turn regardless of the board is
/// noise in the log rather than a feature.
///
/// **The step is a teleport, not a walk.** It provokes nothing, ignores
/// difficult terrain, and breaks the holder's straight-line run (so it
/// cannot be laundered into a charge rider) — see `ActorInstance::
/// break_run`. Range is thirty feet, which is twelve tiles on the
/// 2.5-ft grid.
///
/// Ships on `rangers::HORIZON_WALKER_RANGER_TEMPLATE`, whose subclass is
/// planar travel and whose own capstone (Distant Strike) is already a
/// teleport between swings.
pub const BOON_OF_DIMENSIONAL_TRAVEL_TAG: &str = "boon.dimensional_travel";

/// **Boon of Fate** (Epic Boon) — *"Improve Fate. When you or another
/// creature within 60 feet of you succeeds on or fails a D20 Test, you
/// can roll 2d4 and apply the total rolled as a bonus or penalty to the
/// d20 roll. Once you use this benefit, you can't use it again until you
/// roll Initiative or finish a Short or Long Rest."*
///
/// One row on `FAILED_SAVE_ADD_DIE_SOURCES` and one on
/// `MISSED_ATTACK_BOOSTS`, sharing this tag and therefore sharing the
/// single charge: the two cohorts were built for exactly this shape —
/// "spend a charge, add a die pool to a d20 that came up short" — on the
/// save lane and the attack lane respectively, and the add-die cohort's
/// own docstring has been naming *"a hypothetical `Reroll +Nd4` feat"*
/// as its example of a future row since it was written.
///
/// **Two of RAW's four directions are absent, and they are the two
/// nobody would use.** The clause is symmetric — the holder may improve
/// or worsen, their own roll or a nearby creature's — and the engine
/// ships the two halves that a holder actually reaches for: rescue your
/// own failed save, rescue your own missed swing. Worsening a *success*
/// is a choice with no beneficiary, and applying the pool to another
/// creature's roll needs a decision channel mid-roll that the engine has
/// nowhere to open. Both are named here rather than dropped.
///
/// The recharge — "until you roll Initiative or finish a Short or Long
/// Rest" — is once per encounter, which is what a plain one-charge
/// feature already is on a board that starts every fight fresh.
///
/// Ships on `wizards::DIVINATION_WIZARD_TEMPLATE`, the subclass that
/// already spends its whole identity rewriting one d20 a rest.
pub const BOON_OF_FATE_TAG: &str = "boon.fate";

/// **Boon of Irresistible Offense** (Epic Boon), both of whose combat
/// clauses ship:
///
///   - *"Overcome Defenses. The Bludgeoning, Piercing, and Slashing
///     damage you deal always ignores Resistance."*
///   - *"Overwhelming Strike. When you roll a 20 on the d20 for an
///     attack roll, you can deal extra damage to the target equal to the
///     ability score increased by this feat."*
///
/// **Overcome Defenses** resolves at the attack chokepoints as a sibling
/// of `attack::apply_nonmagical_resistance` — the engine's other
/// attacker-aware adjustment to a queued damage payload, and the reason
/// the shape was already there to reuse. It pre-doubles a physical
/// payload aimed at a creature that would halve it, so the halving the
/// target's own sheet applies a moment later nets back to the whole
/// number. The arithmetic is exact rather than approximate: resistance
/// is a floored halving, and `2n / 2 == n` for every n.
///
/// Only resistance is bypassed. Immunity is not resistance and RAW does
/// not name it; a creature that takes no slashing damage still takes
/// none.
///
/// **Overwhelming Strike** is gated on the *natural* 20 rather than on
/// the swing being a critical hit, because RAW names the die face and
/// those are different questions in this engine: a Champion crits on a
/// 19, a Paralyzed target's every hit crits, and neither is "you rolled
/// a 20 on the d20". Its damage is flat and therefore not doubled by the
/// crit that the same face produced — the same rule the crit doubling
/// itself follows.
///
/// **The extra damage is the Strength or Dexterity *score*, whichever is
/// higher**, which is the one place the missing Ability Score Increase
/// changes a number: RAW's boon would have raised one of the two by 1
/// first, so the engine's holder deals exactly 1 less than a
/// character-sheet holder would. Naming the better of the two matches
/// what the feat's own prerequisite ("Increase your Strength or
/// Dexterity") lets its holder choose, and picks the one a
/// finished stat block would have picked.
///
/// Ships on `barbarians::BERSERKER_BARBARIAN_TEMPLATE` — the chassis
/// whose whole subclass is refusing to be stopped.
pub const BOON_OF_IRRESISTIBLE_OFFENSE_TAG: &str = "boon.irresistible_offense";

/// **Boon of Spell Recall** (Epic Boon) — *"Free Casting. Whenever you
/// cast a spell with a level 1–4 spell slot, roll 1d4. If the number you
/// roll is the same as the slot's level, the slot isn't expended."*
///
/// Read at `ConsumeResource::apply`, the one place a
/// `Resource::SpellSlot` spend becomes a spend: every cast in the engine
/// declares its slot as a cost and the cost tail in `Action::execute`
/// bills it there, so a single gate covers the whole spell list without
/// touching a single spell.
///
/// A quarter of the caster's first four slot levels come back, and the
/// odds are flat across them — a level-1 slot survives on a 1 and a
/// level-4 slot on a 4, both one time in four. Slots of level 5 and up
/// are outside the clause and are always spent, which is what makes the
/// boon a reason to cast *down* rather than an unconditional discount.
///
/// Ships on `sorcerers::SORCERER_TEMPLATE` — the class whose entire
/// resource system is the exchange rate between slots and something
/// else, and for whom a slot that does not go away is the purest
/// possible epic reward.
pub const BOON_OF_SPELL_RECALL_TAG: &str = "boon.spell_recall";

/// **Boon of the Night Spirit** (Epic Boon), whose two clauses are two
/// readings of the same sentence — *while you are in Dim Light or
/// Darkness*:
///
///   - *"Merge with Shadows. …you can give yourself the Invisible
///     condition as a Bonus Action. The condition ends on you
///     immediately after you take an action, a Bonus Action, or a
///     Reaction."*
///   - *"Shadowy Form. …you have Resistance to all damage except Psychic
///     and Radiant."*
///
/// The light gate is live rather than latched, for both clauses: the
/// engine's lighting is per-tile and a creature that steps out of the
/// shadow into a torch's circle is out of the shadow. Shadowy Form is
/// therefore read at the damage chokepoint against wherever the holder
/// is *standing when the blow lands*, not where they were when the fight
/// started, and Merge with Shadows' Invisible is stripped by the same
/// check.
///
/// **Merge with Shadows ships as a Bonus Action** (`class_features::
/// MergeWithShadows`) rather than as a passive, because RAW makes it one
/// and because the cost is the whole balance of the clause: a holder who
/// hides this turn has not attacked this turn. RAW's "ends immediately
/// after you take an action, a Bonus Action, or a Reaction" is enforced
/// at the same three spend sites the engine already tracks, so the
/// invisibility buys exactly what RAW sells — the gap between one turn
/// and the next, and the advantage on the swing that ends it.
///
/// Ships on `monks::SHADOW_MONK_TEMPLATE`, whose subclass already spends
/// its bonus actions on the dark.
pub const BOON_OF_THE_NIGHT_SPIRIT_TAG: &str = "boon.night_spirit";

/// **Boon of Truesight** (Epic Boon) — *"Truesight. You have Truesight
/// with a range of 60 feet."*
///
/// The whole feat, and the only one here that needed no new machinery at
/// all: the engine has carried `SpecialSense::Truesight(feet)` since the
/// bestiary's devas and pit fiends needed it, and every reader of it —
/// the illusion-piercing gate at `pierces_illusion_of`, the attack-mode
/// sweep, the radius arithmetic in `sense_tiles` — asks the senses set
/// rather than the template.
///
/// So the boon is a *grant into that set*, applied at instantiation by
/// `senses_with_feature_grants`. Doing it there rather than by widening
/// `has_truesight` to also consult the tag is what keeps the radius
/// real: a boolean accessor would have given the holder truesight of
/// unbounded range, and sixty feet is a number the feat prints.
///
/// Ships on `warlocks::GREAT_OLD_ONE_WARLOCK_TEMPLATE` — the pact whose
/// every other feature is about perceiving what is not meant to be
/// perceived.
pub const BOON_OF_TRUESIGHT_TAG: &str = "boon.truesight";

/// How far Boon of Truesight sees, in feet. RAW's number, kept beside
/// the tag rather than inline at the grant site so the feat's text and
/// the engine's constant sit in one place.
pub const BOON_OF_TRUESIGHT_FEET: u32 = 60;

/// SRD 5.2's Epic Boon category, whole.
///
/// A sub-list of `FEAT_TAGS` rather than a second registry: the sweep
/// that matters (every feat is carried by something a player can be)
/// runs over the whole set, and this exists so the *completeness* of the
/// column is itself checkable — see
/// `the_epic_boon_column_is_carried_whole`. A boon added to the module
/// and forgotten here is a boon missing from the category it belongs to,
/// which is the one thing a flat list of every feat cannot say.
pub const EPIC_BOON_TAGS: &[&str] = &[
    BOON_OF_COMBAT_PROWESS_TAG,
    BOON_OF_DIMENSIONAL_TRAVEL_TAG,
    BOON_OF_FATE_TAG,
    BOON_OF_IRRESISTIBLE_OFFENSE_TAG,
    BOON_OF_SPELL_RECALL_TAG,
    BOON_OF_THE_NIGHT_SPIRIT_TAG,
    BOON_OF_TRUESIGHT_TAG,
];

/// Every feat tag in the engine.
///
/// One list, for the reason `pc_template_families` and
/// `all_summon_spells` are each one list: the invariant worth checking
/// across feats is that each of them is actually carried by something a
/// player can be. A feat nobody can take is a passive that never fires,
/// and it is invisible by construction — no test fails, no encounter
/// behaves differently, and the constant sits here reading as
/// implemented. See `every_feat_is_carried_by_a_playable_chassis`.
pub const FEAT_TAGS: &[&str] = &[
    ALERT_TAG,
    SAVAGE_ATTACKER_TAG,
    TOUGH_TAG,
    GRAPPLER_TAG,
    SPEEDY_TAG,
    CHARGER_TAG,
    WAR_CASTER_TAG,
    BOON_OF_COMBAT_PROWESS_TAG,
    BOON_OF_DIMENSIONAL_TRAVEL_TAG,
    BOON_OF_FATE_TAG,
    BOON_OF_IRRESISTIBLE_OFFENSE_TAG,
    BOON_OF_SPELL_RECALL_TAG,
    BOON_OF_THE_NIGHT_SPIRIT_TAG,
    BOON_OF_TRUESIGHT_TAG,
];

/// **Merge with Shadows** — the Bonus Action half of `BOON_OF_THE_NIGHT_
/// SPIRIT_TAG`: *"While within Dim Light or Darkness, you can give
/// yourself the Invisible condition as a Bonus Action. The condition
/// ends on you immediately after you take an action, a Bonus Action, or
/// a Reaction."*
///
/// The first `Action` in this module, and the reason feats stopped being
/// only constants: every feat before it changed a die somebody else was
/// already rolling, and this one is a thing its holder *does*. It lives
/// here rather than in `class_features` because it is not a class
/// feature — a chassis gets it by taking the boon, and the gate below is
/// the tag rather than the template.
///
/// **Two gates, both RAW.** The holder must be standing in Dim Light or
/// Darkness (`EncounterInstance::is_shrouded_in_shadow`, the same
/// predicate Shadowy Form reads), and must not already be Invisible —
/// re-merging would spend a Bonus Action to refresh a timer.
///
/// **The timer is `UntilStartOfNextTurn`, and that is an approximation
/// of RAW's end clause rather than a transcription of it.** RAW ends the
/// condition the moment the holder next takes an action, a Bonus Action
/// or a Reaction; the engine ends it at the start of their next turn,
/// which is where all three of those roads lead for a creature that has
/// finished its turn in the dark. Two differences follow and both are
/// small:
///
///   - A holder who merges *before* spending their Action keeps the
///     invisibility through that Action, where RAW would have ended it.
///     The AI does not do this (`SimpleAi` reaches for a bonus action
///     after its action, not before), so the divergence is a
///     human-player one, and it costs the holder their Action's
///     ordering rather than winning them a second effect.
///   - A holder who merges and then does nothing loses the condition at
///     the top of their next turn instead of at their first act on it,
///     so the swing that RAW would have let them open with at advantage
///     lands at normal. This is the direction the engine should err in.
///
/// The Reaction clause *is* enforced exactly, in
/// `EncounterInstance::end_shadow_merge_on_reaction`, because a Reaction
/// is spent between turns where no timer can stand in for it.
pub struct MergeWithShadows {}

impl crate::actions::action_template::Action for MergeWithShadows {
    fn name(&self) -> &str {
        "merge with shadows"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["merge", "mws"]
    }

    fn targeting_schema(&self) -> crate::actions::action_template::TargetingSchema {
        crate::actions::action_template::TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn deals_damage(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _e: &crate::engine::encounter::EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<crate::engine::types::Coordinate>>,
        _o: Option<&std::collections::HashSet<crate::engine::action_overrides::ActionOverride>>,
    ) -> Vec<crate::engine::side_effects::Resource> {
        crate::actions::action_template::bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &crate::engine::encounter::EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<crate::engine::types::Coordinate>>,
        _overrides: Option<&std::collections::HashSet<crate::engine::action_overrides::ActionOverride>>,
    ) -> bool {
        // The tag gate rides `is_shrouded_in_shadow`, which asks for both
        // halves of RAW's condition at once — the boon, and the dark.
        encounter.is_shrouded_in_shadow(caster_id)
            && encounter.actors.get(&caster_id).is_some_and(|a| {
                a.is_combat_active()
                    && !a.has_condition(crate::conditions::Condition::Invisible)
            })
    }

    fn side_effects(
        &self,
        encounter: &mut crate::engine::encounter::EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<crate::engine::types::Coordinate>>,
        _o: Option<&std::collections::HashSet<crate::engine::action_overrides::ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let name = encounter.actor_name(caster_id);
        encounter.log(format!("{} merges with the shadows and is gone.", name));
        vec![Box::new(crate::engine::side_effects::ApplyCondition {
            actor_id: caster_id,
            condition: crate::conditions::Condition::Invisible,
            timer: crate::conditions::ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static MERGE_WITH_SHADOWS: std::sync::LazyLock<MergeWithShadows> =
    std::sync::LazyLock::new(|| MergeWithShadows {});
