use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{
        Action, TargetingSchema, action_only, bonus_action_and_slot, bonus_action_only,
        first_target_id, first_target_location, free_cost, reaction_only,
        resolve_enemy_burst_save_damage,
    },
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{
            ApplicableSideEffect, ApplyCondition, DealDamage, GainTempHp, GiveResource, Heal,
            Resource,
        },
        types::{AbilityScoreType, Coordinate, DamageType},
    },
};

/// Class-feature tags that refresh on a 5e short rest. Read by
/// `ActorInstance::short_rest`, which walks this registry chained with
/// `BATTLE_MASTER_MANEUVERS` and repopulates `features_remaining` for
/// every matching tag the actor carries. The remaining tags in this
/// module are long-rest features and only restore via `long_rest`.
///
/// The two registries are kept disjoint: the maneuvers have their own
/// list because `ActorInstance` needs to enumerate them independently,
/// and duplicating them here would mean two places to forget. See
/// `the_short_rest_registry_matches_the_templates_that_use_it`, which
/// pins the disjointness and checks that every row is actually carried
/// by some registered PC template — a registry entry whose feature has
/// moved on is invisible otherwise.
pub const SHORT_REST_FEATURES: &[&str] = &[
    SECOND_WIND_TAG,
    ACTION_SURGE_TAG,
    ARCANE_RECOVERY_TAG,
    NATURAL_RECOVERY_TAG,
    PRESERVE_LIFE_TAG,
    CUTTING_WORDS_TAG,
    BREATH_WEAPON_TAG,
    // 5e War Domain Cleric features — both refresh on a short rest.
    // WAR_PRIEST is the once-per-rest bonus-action extra swing; GUIDED
    // STRIKE is the Channel Divinity +10 accuracy prime.
    WAR_PRIEST_TAG,
    GUIDED_STRIKE_TAG,
    // 5e Light Domain Cleric Channel Divinity — Radiance of the Dawn:
    // once-per-short-rest 30ft radiant burst. Shares the RAW "Channel
    // Divinity" resource lane with Turn Undead / Preserve Life / Guided
    // Strike, but each tag is a distinct per-rest charge in our model.
    RADIANCE_OF_THE_DAWN_TAG,
    // 5e Light Domain Cleric level-1 feature — Warding Flare: passive
    // reaction that imposes disadvantage on an incoming attack roll.
    // RAW: uses per long rest equal to WIS mod (min 1), refreshed on a
    // long rest. We collapse to a once-per-short-rest charge so the
    // Light Cleric doesn't lose the flare between engagements — same
    // gating shape as the other Cleric Channel Divinity charges.
    WARDING_FLARE_TAG,
    // 5e Oathbreaker Paladin level-15 subclass feature — Fanatical
    // Focus. Auto-fire "reroll one failed save per short rest" gate.
    // RAW: refreshes on a short or long rest, matching the Cleric
    // Channel Divinity cadence — registered here so the short rest
    // refresh path picks it up alongside Guided Strike / Radiance of
    // the Dawn / Warding Flare.
    FANATICAL_FOCUS_TAG,
    // 5e Fiend Warlock level-6 subclass feature — Dark One's Own Luck.
    // Auto-fire "add 1d10 to a failed save" gate. RAW: refreshes on a
    // short or long rest — sibling to Fanatical Focus's reroll shape
    // on the failed-save recovery lane, but adds a die to the total
    // rather than re-rolling the d20.
    DARK_ONES_OWN_LUCK_TAG,
    // 5e Hexblade Warlock level-1 subclass feature — Hexblade's Curse.
    // RAW: "you can't use this feature again until you finish a short
    // or long rest", which is the same cadence as Dark One's Own Luck
    // directly above and the Channel Divinity family, so it refreshes
    // here rather than on the warlock's long-rest lane.
    HEXBLADES_CURSE_TAG,
    // 5e Devotion Paladin level-3 subclass Channel Divinity — Turn the
    // Faithless. Fey / fiend within 30ft roll a WIS save vs the
    // paladin's spell save DC; on fail they're Frightened for 10 rounds
    // (1 minute RAW). Sibling to Turn Undead — RAW Channel Divinity
    // refreshes on short rest, same as Guided Strike / Radiance of the
    // Dawn.
    TURN_THE_FAITHLESS_TAG,
    // 5e Berserker Barbarian level-10 subclass feature — Intimidating
    // Presence. Single-target Frighten via WIS save vs the barbarian's
    // CHA-anchored DC. Refreshed on short rest so a raging Berserker
    // gets one intimidation press per engagement, matching the cadence
    // of the paladin CDs above rather than the long-rest Rage clock.
    INTIMIDATING_PRESENCE_TAG,
    // 5e Ancients Paladin level-3 Channel Divinity — Nature's Wrath.
    // Single-target Restrained via STR save vs the paladin's CHA-
    // anchored DC. Refreshed on short rest alongside the other paladin
    // CD family (Turn the Faithless, Guided Strike, Radiance of the
    // Dawn) — RAW Channel Divinity is once per short rest.
    NATURES_WRATH_TAG,
    // 5e Tempest Domain Cleric level-1 subclass feature — Wrath of the
    // Storm. Single-target 2d8 lightning damage burst via DEX save vs
    // the cleric's WIS-anchored DC. Refreshed on short rest alongside
    // the other cleric CD family (Guided Strike / Radiance of the Dawn
    // / Warding Flare) — RAW uses per long rest = WIS mod refreshes
    // on long rest, collapsed to a single once-per-short-rest charge to
    // match the CD gating shape.
    WRATH_OF_THE_STORM_TAG,
    // 5e Bard **Bardic Inspiration** (level 1) refresh — RAW gates on
    // level 5's Font of Inspiration to promote the long-rest refresh
    // to a short-rest one. `ActorInstance::short_rest` walks this
    // registry AND requires the actor holds `has_passive_feature(tag)`
    // — for Bardic Inspiration, both the primary tag AND the Font of
    // Inspiration gate must be on the template. A lv1-4 bard build
    // (without Font of Inspiration) still has to long-rest to reset
    // the die; a lv5+ bard with the Font tag refreshes here.
    BARDIC_INSPIRATION_TAG,
    // 5e Vengeance Paladin level-3 subclass Channel Divinity — Abjure
    // Enemy. Single-target Frighten via WIS save vs the paladin's
    // CHA-anchored DC. Refreshed on short rest alongside the other
    // paladin CD family (Turn the Faithless, Nature's Wrath, Guided
    // Strike, Radiance of the Dawn) — RAW Channel Divinity is once
    // per short rest.
    ABJURE_ENEMY_TAG,
    // 5e Devotion Paladin level-15 subclass feature — Rebuke the
    // Violent. Single-target 4d10 radiant damage burst via WIS save
    // vs the paladin's CHA-anchored DC. Refreshed on short rest to
    // match the CD gating shape — RAW uses per long rest = CHA mod
    // refreshes on long rest, collapsed to a single once-per-short-
    // rest charge to match the sibling Wrath of the Storm / Infernal
    // Rebuke damage-burst refresh cadence.
    REBUKE_THE_VIOLENT_TAG,
    // 5e Fiend Warlock level-14 subclass capstone — Hurl Through Hell.
    // Signature single-target teleport-to-hell burst. RAW: once per
    // long rest; collapsed to a short-rest charge here so it lands on
    // the Warlock's Pact Magic slot-refresh cadence (RAW warlock slots
    // themselves refresh on short rest — the CR-4 Fiend Warlock's
    // capstone-adjacent burst rides the same short-rest lane as the
    // slot pool it competes with for the action budget). Sibling to
    // Wrath of the Storm / Rebuke the Violent / Infernal Rebuke on the
    // single-target burst-save-for-half damage lane, differentiated by
    // the (save-ability, dice, damage_type, spellcasting_ability)
    // tuple.
    HURL_THROUGH_HELL_TAG,
    // 5e Oathbreaker Paladin level-3 subclass Channel Divinity —
    // Dreadful Aspect. 30ft WIS-save burst → Frightened on fail; no
    // creature-type filter (any hostile). Refreshed on short rest to
    // match the paladin CD family (Turn the Faithless / Nature's
    // Wrath / Abjure Enemy / Guided Strike / Radiance of the Dawn) —
    // RAW Channel Divinity is once per short rest.
    DREADFUL_ASPECT_TAG,
    // 5e Divine Soul Sorcerer level-1 subclass feature — Favored by
    // the Gods. Auto-fire "add 2d4 to a failed save" gate. RAW:
    // refreshes on a short or long rest — sibling to Dark One's Own
    // Luck (Fiend Warlock lv6, +1d10) on the same failed-save
    // add-die recovery lane, differentiated by the die pool
    // (2d4 avg +5 vs 1d10 avg +5.5) and the source chassis (Sorcerer
    // subclass vs. Warlock subclass).
    FAVORED_BY_THE_GODS_TAG,
    // 5e Great Old One Warlock level-6 subclass feature — Entropic
    // Ward. Reactive per-rest disadvantage on an incoming attack roll
    // against the warlock. RAW: refreshes on short or long rest —
    // sibling on the shared "reactive per-rest disadvantage on an
    // incoming attack roll" lane with Warding Flare (Light Cleric
    // lv1) but with an unlimited range (no 30ft cap): the warlock's
    // patron reads the attacker's mind anywhere on the battlefield.
    ENTROPIC_WARD_TAG,
    // 5e Grave Domain Cleric level-2 subclass Channel Divinity — Path
    // to the Grave. Single-target curse (`MarkedForGrave`) that grants
    // advantage to the next attack against the cursed target. RAW:
    // once per short rest — sibling cadence to every other Cleric CD
    // (Turn Undead / Preserve Life / Guided Strike / Radiance of the
    // Dawn / Warding Flare / Wrath of the Storm).
    PATH_TO_THE_GRAVE_TAG,
    // 5e Illusion Wizard level-10 subclass feature — Illusory Self.
    // Reactive per-rest auto-miss on an incoming attack roll that
    // would otherwise connect. RAW: refreshes on a short or long rest
    // — the same cadence as the two sibling reactive per-rest
    // defenses (Warding Flare, Entropic Ward), and a strictly stronger
    // one, which is why it sits at subclass level 10 rather than 1/6.
    ILLUSORY_SELF_TAG,
    // 5e Transmutation Wizard level-10 subclass feature — Shapechanger.
    // RAW: "you can cast the polymorph spell without expending a spell
    // slot... Once you do so, you can't use this feature again until
    // you finish a short or long rest."
    SHAPECHANGER_TAG,
    // 5e Circle of the Moon Druid **Combat Wild Shape** — RAW: "you
    // regain [Wild Shape uses] when you finish a short or long rest",
    // the same cadence as the Channel Divinity family above. The
    // engine collapses RAW's two uses per rest to one charge; the
    // cadence stays exact.
    COMBAT_WILD_SHAPE_TAG,
    // 5e Psi Warrior Fighter **Protective Field** — the Psionic Energy
    // pool RAW "regain all expended dice when you finish a short or
    // long rest", the same cadence as the Channel Divinity family
    // above. The engine collapses RAW's four-plus dice to one charge;
    // the cadence stays exact.
    PROTECTIVE_FIELD_TAG,
    // 5e Cavalier Fighter **Warding Maneuver** — RAW grants uses equal
    // to the cavalier's CON modifier, refreshed on a long rest. The
    // engine collapses the pool to one charge and moves the refresh to
    // the short-rest cadence the rest of the per-rest reaction family
    // (Parry, Warding Flare, Protective Field) shares, so a cavalier
    // doesn't lose the maneuver between engagements.
    WARDING_MANEUVER_TAG,
    // 5e Arcana Domain Cleric Channel Divinity — Arcane Abjuration.
    // RAW Channel Divinity is once per short rest, the same cadence as
    // every sibling CD charge above.
    ARCANE_ABJURATION_TAG,
    // 5e Nature Domain Cleric **Dampen Elements** — RAW costs only the
    // reaction with no per-rest cap; the engine's single charge lands on
    // the short-rest cadence the rest of the reactive per-rest family
    // (Parry, Warding Flare, Warding Maneuver, Protective Field) shares.
    DAMPEN_ELEMENTS_TAG,
    // 5e Nature Domain Cleric Channel Divinity — Charm Animals and
    // Plants. RAW Channel Divinity is once per short rest.
    CHARM_ANIMALS_AND_PLANTS_TAG,
    // 5e Trickery Domain Cleric Channel Divinity — Invoke Duplicity.
    // RAW Channel Divinity is once per short rest, the same cadence as
    // every sibling CD charge above.
    INVOKE_DUPLICITY_TAG,
    // 5e Undead Warlock **Form of Dread** — RAW grants proficiency-bonus
    // uses per long rest; the single charge lands on the short-rest
    // lane, matching the warlock's own Pact Magic refresh.
    FORM_OF_DREAD_TAG,
    // 5e Bladesinging Wizard **Bladesong** — RAW: "you can use this
    // feature twice, and you regain all expended uses when you finish a
    // short or long rest." The engine's single charge lands on the same
    // short-rest cadence.
    BLADESONG_TAG,
    // 5e Conquest Paladin level-3 subclass Channel Divinity —
    // Conquering Presence. Refreshed on short rest alongside the rest
    // of the paladin CD family (Dreadful Aspect, Abjure Enemy, Nature's
    // Wrath, Turn the Faithless).
    CONQUERING_PRESENCE_TAG,
    // 5e Circle of Spores Druid **Symbiotic Entity** — RAW spends a
    // Wild Shape use, and Wild Shape itself recharges on a short rest,
    // so the charge lands on the short-rest lane rather than the
    // long-rest default.
    SYMBIOTIC_ENTITY_TAG,
    // 5e Rune Knight Fighter subclass level 3 — Giant's Might and Fire
    // Rune. RAW gives Giant's Might proficiency-bonus uses per long rest
    // and recharges every rune on a short rest; the engine's one-charge-
    // per-tag model lands both on the short-rest lane, which keeps a
    // Rune Knight's per-fight budget in the same shape as the Battle
    // Master's maneuvers on the same chassis.
    GIANTS_MIGHT_TAG,
    FIRE_RUNE_TAG,
    // 5e Death Domain Cleric Channel Divinity — Reaper's Touch (RAW
    // "Touch of Death"). Same short-rest cadence as the rest of the
    // cleric CD family (Turn Undead, Preserve Life, Guided Strike,
    // Radiance of the Dawn).
    REAPERS_TOUCH_TAG,
    // 5e Order Domain Cleric Channel Divinity — Order's Demand. Same
    // short-rest cadence as the rest of the cleric CD family.
    ORDERS_DEMAND_TAG,
    // 5e Soulknife Rogue **Homing Strikes**. RAW recovers Psionic Energy
    // dice on a short rest, which is the cadence this registry carries.
    HOMING_STRIKES_TAG,
];

/// Battle Master maneuver tags. RAW: maneuvers cost superiority dice
/// which refresh on a short or long rest. We collapse the dice pool to
/// per-tag once-per-rest charges so the gating stays uniform with the
/// other class features.
///
/// Deliberately *not* duplicated into `SHORT_REST_FEATURES`:
/// `ActorInstance::short_rest` chains the two registries, so listing a
/// maneuver in both would refresh it twice and give two places to
/// forget it. This list exists separately because the maneuver cohort
/// is enumerated on its own elsewhere.
pub const BATTLE_MASTER_MANEUVERS: &[&str] = &[
    TRIP_ATTACK_TAG,
    MENACING_ATTACK_TAG,
    DISARMING_ATTACK_TAG,
    PUSHING_ATTACK_TAG,
    GOADING_ATTACK_TAG,
    PRECISION_ATTACK_TAG,
    SWEEPING_ATTACK_TAG,
    FEINTING_ATTACK_TAG,
    LUNGING_ATTACK_TAG,
    RALLY_TAG,
    COMMANDERS_STRIKE_TAG,
    DISTRACTING_ATTACK_TAG,
    // Reaction maneuvers — both fire automatically on an incoming melee
    // swing (no active action to spend on the fighter's turn), gated on
    // the per-rest charge below and the target's reaction slot. Listed
    // here so a short rest refreshes them uniformly with the bonus-
    // action primes above.
    PARRY_TAG,
    RIPOSTE_TAG,
];

/// Tags used by `ActorInstance::feature_available` / `spend_feature` to
/// gate once-per-long-rest class features. Stored as `&'static str` so
/// actor state stays a flat HashSet instead of carrying an enum import.
pub const SECOND_WIND_TAG: &str = "fighter.second_wind";
pub const ACTION_SURGE_TAG: &str = "fighter.action_surge";

/// Tag for the Half-Orc Relentless Endurance racial trait. Passive
/// no-action feature: once per long rest, when damage would reduce
/// the holder to 0 HP, they drop to 1 HP instead. We weave the trigger
/// into `ActorInstance::take_damage` next to the Death Ward hook —
/// mirrors the "intercept the 0-HP transition and roll back to 1"
/// shape, but with the once-per-rest feature flag in place of the
/// short-duration condition.
pub const RELENTLESS_ENDURANCE_TAG: &str = "half_orc.relentless_endurance";

/// Tag for the Paladin **Undying Sentinel** feature (Oath of the
/// Ancients level 15 subclass feature). Passive no-action feature:
/// once per long rest, when damage would reduce the holder to 0 HP
/// (and they're not killed outright by Massive Damage), they drop to
/// 1 HP instead. Mechanically identical to Half-Orc Relentless
/// Endurance — both route through the shared
/// `LETHAL_DAMAGE_ABSORBER_FEATURES` cohort in
/// `take_typed_damage` so a hypothetical multiclass (a half-orc
/// Ancients paladin) spends the tags in order rather than double-
/// dipping on the same damage instance.
///
/// Ships on the ANCIENTS_PALADIN_TEMPLATE feature set (Oath of the
/// Ancients level-15 capstone) above its strict RAW level gate for
/// the same reason Nature's Ward (lv15) does — class templates target
/// a balanced playable level, not lockstep PHB progression. Distinct
/// from Nature's Ward (self-immunity to Charmed / Frightened, always
/// on) — Undying Sentinel is a one-shot "cheat death" resource that
/// refreshes on long rest. RAW-flavored as "the paladin refuses to
/// fall while there's still a chance to strike back", it's the
/// signature Ancients tell in low-HP combat rounds.
///
/// Long-rest refresh: the tag lives on `features_max` for holders,
/// so `long_rest` restores the charge (features_remaining =
/// features_max). Not in `SHORT_REST_FEATURES` — RAW gates on the
/// long rest per PHB text.
pub const UNDYING_SENTINEL_TAG: &str = "paladin.undying_sentinel";

/// Tag for the Sorcerer **Strength of the Grave** feature (Shadow
/// Magic Sorcerous Origin level 1, XGtE). Passive no-action feature:
/// once per long rest, when damage would reduce the holder to 0 HP
/// (and they're not killed outright by Massive Damage), they drop to
/// 1 HP instead. Mechanically identical to Half-Orc Relentless
/// Endurance and Ancients Paladin Undying Sentinel — all three route
/// through the shared `LETHAL_DAMAGE_ABSORBER_FEATURES` cohort in
/// `take_typed_damage` so a hypothetical multi-source carrier spends
/// the tags in order rather than double-dipping on the same damage
/// instance.
///
/// RAW's Strength of the Grave gates the drop-to-1 outcome behind a
/// CHA save (DC 5 + damage taken) that fails on radiant damage or if
/// the killing hit came from a Critical Hit. We collapse the save-
/// vs-DC gate to a once-per-long-rest guaranteed proc for uniformity
/// with the sibling Relentless Endurance / Undying Sentinel entries —
/// the save-DC branch would fold cleanly into the cohort loop if a
/// future refactor promotes the per-entry gate into a struct row (see
/// the `LETHAL_DAMAGE_ABSORBER_FEATURES` docstring for the shape).
/// The once-per-rest collapse also matches how Strength of the Grave
/// is played in practice at low CHA-DC seeds — a level-1 Shadow
/// Sorcerer rolling CHA 16 hits DC 5 + typical 10 damage = DC 15 vs
/// +3 CHA, so ~40% pass without a boost; ~1-per-day is a fair floor.
///
/// Ships on the SHADOW_MAGIC_SORCERER_TEMPLATE feature set (Shadow
/// Magic level-1 subclass tell) at its RAW gate. Distinct from Half-
/// Orc Relentless Endurance (racial trait, same mechanic) — the
/// two never legally co-occur on a single build in D&D 5e's core
/// races (a half-orc can't also be a sorcerer's Shadow Magic origin
/// subclass, but a hypothetical mixed-race Shadow Sorcerer could
/// carry both cohort rows; the shared cohort's "first-charge-only"
/// consumption keeps the double-dip locked out).
///
/// Long-rest refresh: the tag lives on `features_max` for holders,
/// so `long_rest` restores the charge (features_remaining =
/// features_max). Not in `SHORT_REST_FEATURES` — RAW gates Strength
/// of the Grave on a long rest per XGtE text, matching Relentless
/// Endurance / Undying Sentinel on the same lane.
pub const STRENGTH_OF_THE_GRAVE_TAG: &str = "sorcerer.strength_of_the_grave";

/// Ordered cohort of feature tags that intercept a lethal HP-to-zero
/// damage instance and convert it to a "drop to 1 HP instead" outcome.
/// Read in order by `ActorInstance::take_typed_damage` at the 0-HP
/// transition site: the *first* tag on the actor's
/// `features_remaining` is spent, the HP is clamped to 1, and the
/// damage outcome downgrades to `Reduced`. Subsequent tags in the
/// list stay unspent — RAW: each feature says "instead of 0 HP" so
/// only one fires per instance.
///
/// Ordering is significant: earlier entries are consumed first, so
/// a hypothetical Half-Orc / Ancients Paladin multiclass would burn
/// Relentless Endurance before Undying Sentinel on a single lethal
/// hit. All entries refresh on long rest (they live on
/// `features_max`); none refresh on short rest (none are in
/// `SHORT_REST_FEATURES`).
///
/// Death Ward is checked separately, *before* this cohort — RAW: the
/// spell is an active resource the caster chose to maintain, so
/// burning the racial / class feature before Death Ward would waste
/// the slot. Massive Damage (overflow ≥ max HP) also short-circuits
/// before this cohort — RAW: "if remaining damage after hitting 0 HP
/// equals or exceeds the creature's max HP, it dies instantly."
pub const LETHAL_DAMAGE_ABSORBER_FEATURES: &[&str] = &[
    RELENTLESS_ENDURANCE_TAG,
    UNDYING_SENTINEL_TAG,
    STRENGTH_OF_THE_GRAVE_TAG,
];

/// Tag for the Sahuagin Blood Frenzy racial trait. Passive always-on
/// feature: the holder rolls melee attacks with advantage against any
/// target that doesn't have all its hit points. We weave the gate into
/// `compute_attack_mode` next to Pack Tactics — when the attacker has the
/// tag, the swing is melee, and the target's `current_hp() <
/// max_hitpoints()`, the swing gets advantage. The tag lives in the
/// passive-feature pool so a long-rest never "spends" it (it's not a
/// per-rest pool — it always fires on a wounded target).
pub const BLOOD_FRENZY_TAG: &str = "sahuagin.blood_frenzy";

/// Common gating shape for once-per-rest class features: the caster must
/// exist, be combat-active, and have an unspent charge of `tag`. Returns
/// true when all three hold. Centralizes the `is_some_and(|a|
/// a.is_combat_active() && a.feature_available(tag))` boilerplate that
/// every `custom_validate_input` previously open-coded — keeps the gate
/// to a one-line call and gives a single chokepoint for future cross-
/// cutting checks (e.g. "feature suppressed while Silenced").
pub fn feature_ready(
    encounter: &EncounterInstance,
    caster_id: usize,
    tag: &'static str,
) -> bool {
    encounter
        .actors
        .get(&caster_id)
        .is_some_and(|a| a.is_combat_active() && a.feature_available(tag))
}

/// Gating shape for "while-raging" passive features whose action half
/// fires only inside an active Rage condition: the holder must exist, be
/// combat-active, carry the `tag` passive feature flag, AND have the
/// `Raging` condition. The classic Berserker Frenzy / Totem Warrior
/// secondary action shape — Frenzy and Eagle Dive (and any future
/// rage-gated bonus action — Reckless Throw, etc.) both route through
/// this one helper instead of re-inlining the three-clause chain.
pub fn raging_feature_ready(
    encounter: &EncounterInstance,
    caster_id: usize,
    tag: &'static str,
) -> bool {
    encounter.actors.get(&caster_id).is_some_and(|a| {
        a.is_combat_active() && a.has_passive_feature(tag) && a.has_condition(Condition::Raging)
    })
}

/// Gating shape for once-per-rest bonus-action primes that install a
/// self-targeted "next swing rider" condition (Trip Attack, Menacing
/// Attack, Disarming Attack, Pushing Attack, Goading Attack, Distracting
/// Attack, Precision Attack, Sweeping Attack, Lunging Attack, Stunning
/// Strike, Divine Strike). Returns true when:
///   - the caster is combat-active,
///   - the once-per-rest `tag` charge is available,
///   - the caster does NOT already carry `prime` (re-priming would
///     just refresh the timer and waste the charge).
///
/// Centralizes the three-clause chain that every one of the ~10
/// bonus-action primes above previously open-coded, and gives a single
/// chokepoint if the prime-install gate ever needs a cross-cutting
/// check (e.g. "no prime while Silenced" for the future Silence-shuts-
/// down-verbal-effects half). Sibling to `feature_ready` (which lacks
/// the prime-condition no-stack clause) — reach for this one whenever
/// the action installs a caster-side condition and reach for
/// `feature_ready` when the action's effect is one-shot damage / heal /
/// resource grant without a lingering flag.
pub fn feature_prime_ready(
    encounter: &EncounterInstance,
    caster_id: usize,
    tag: &'static str,
    prime: Condition,
) -> bool {
    encounter.actors.get(&caster_id).is_some_and(|a| {
        a.is_combat_active() && a.feature_available(tag) && !a.has_condition(prime)
    })
}

/// Gating shape for once-per-rest single-target class-feature actions
/// that install a debuff *on the target* (Intimidating Presence
/// Frightening a foe, Nature's Wrath Restraining a foe, and any future
/// single-target save-vs-condition CD action). Returns true when:
///   - the caster has an unspent `feature_tag` charge (via `feature_ready`),
///   - `target_ids` carries a resolvable target id,
///   - the target is hostile to the caster AND combat-active,
///   - the target does NOT already carry `skip_if_condition` (re-installing
///     would refresh the timer for no mechanical benefit and burn the
///     once-per-rest charge).
///
/// Sibling to `feature_prime_ready` (which handles the CASTER-side prime
/// de-dup for bonus-action next-swing primes) — this one covers the
/// TARGET-side condition de-dup for single-target debuff-installers.
/// Centralizes the ~10-line "get target / get caster / check hostile /
/// check combat_active / check no-redup" chain that both new features
/// (and any future single-target class-feature debuff) share, so
/// adding a new one lands as a `hostile_target_feature_ready(..., TAG,
/// COND)` one-liner. Also gives a single chokepoint for future
/// cross-cutting checks (e.g. "no debuff install through Sanctuary" —
/// the target-side sanctuary gate that would otherwise need to bounce
/// through every debuff's validate open-code).
pub fn hostile_target_feature_ready(
    encounter: &EncounterInstance,
    caster_id: usize,
    target_ids: Option<&Vec<usize>>,
    feature_tag: &'static str,
    skip_if_condition: Condition,
) -> bool {
    let Some(target) = resolve_hostile_target(encounter, caster_id, target_ids, feature_tag)
    else {
        return false;
    };
    !target.has_condition(skip_if_condition)
}

/// Gating shape for once-per-rest single-target class-feature actions
/// that deal damage on the target with a save-for-half fold (Wrath of
/// the Storm, Rebuke the Violent, Hurl Through Hell, and any future
/// single-target save-for-half damage burst). Returns true when:
///   - the caster has an unspent `feature_tag` charge (via `feature_ready`),
///   - `target_ids` carries a resolvable target id,
///   - the target is hostile to the caster AND combat-active.
///
/// Sibling to `hostile_target_feature_ready` — that helper covers the
/// TARGET-side condition de-dup for single-target debuff-installers
/// (Intimidating Presence Frightening / Nature's Wrath Restraining /
/// Abjure Enemy Frightening); this one covers the "no lingering
/// condition, just damage" lane, so the "skip-if-already-holds-
/// condition" clause is dropped. The two helpers share the same 4-clause
/// "check feature ready / resolve target / check hostile / check combat-
/// active" preamble via the shared `resolve_hostile_target` internal
/// helper and differ only by that last dedup clause; keeping them as
/// siblings lets the caller pick the right shape without threading an
/// `Option<Condition>` through both call sites.
///
/// Centralizes the ~10-line "resolve target / check hostile / check
/// combat-active" chain that Wrath of the Storm / Rebuke the Violent /
/// Hurl Through Hell all previously open-coded in `custom_validate_input`.
/// Adding a future single-target save-for-half damage burst class
/// feature lands as a `hostile_target_burst_ready(..., TAG)` one-liner.
pub fn hostile_target_burst_ready(
    encounter: &EncounterInstance,
    caster_id: usize,
    target_ids: Option<&Vec<usize>>,
    feature_tag: &'static str,
) -> bool {
    resolve_hostile_target(encounter, caster_id, target_ids, feature_tag).is_some()
}

/// Shared preamble for the "hostile target" single-target CD helpers
/// above (`hostile_target_feature_ready` / `hostile_target_burst_ready`).
/// Walks the four uniform gates every hostile-single-target CD action
/// shares:
///   1. Caster has an unspent `feature_tag` charge (via `feature_ready`
///      — same `is_combat_active + feature_available` chokepoint every
///      once-per-rest CD action rides).
///   2. `target_ids` carries a resolvable first id (via `first_target_id`
///      — same shape every SingleActor targeting schema uses).
///   3. Caster resolves to an actor in the encounter (guard against a
///      caster who vanished between enqueue and execute).
///   4. Target resolves to a hostile, combat-active actor (team-mismatch
///      + `is_combat_active` — RAW: no CD on your own ally, no CD on a
///        corpse).
///
/// Returns the target's `&ActorInstance` on all-gates-pass so the two
/// public helpers can layer their own trailing gate (de-dup condition
/// check for the debuff-installer variant, no-trailing-check for the
/// damage-burst variant) with a one-line pattern match at the call
/// site. Returns `None` on any gate failure, matching the public
/// helpers' "any gate fail → false" invariant.
///
/// Private to this module — the two public sibling helpers are the
/// intended surface for callers picking between "install a debuff /
/// deal damage" on the hostile-single-target CD lane; the internal
/// helper collapses their shared 4-line preamble body into one call
/// so a future cross-cutting check (e.g. "no CD on a target holding
/// Sanctuary" — the target-side sanctuary gate) lands in one place
/// instead of two.
fn resolve_hostile_target<'e>(
    encounter: &'e EncounterInstance,
    caster_id: usize,
    target_ids: Option<&Vec<usize>>,
    feature_tag: &'static str,
) -> Option<&'e crate::actors::actor_template::ActorInstance> {
    if !feature_ready(encounter, caster_id, feature_tag) {
        return None;
    }
    let target_id = first_target_id(target_ids)?;
    let caster = encounter.actors.get(&caster_id)?;
    let target = encounter.actors.get(&target_id)?;
    if target.team() == caster.team() || !target.is_combat_active() {
        return None;
    }
    Some(target)
}

/// Fighter Second Wind — bonus action; restore 1d10 + level HP. Once per
/// long rest. Self-targeted; only valid while combat-active (no reviving
/// yourself out of dying via this).
pub struct SecondWind {}

impl Action for SecondWind {
    fn name(&self) -> &str {
        "second wind"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["sw", "wind"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
        true
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, SECOND_WIND_TAG)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let raw = encounter.roll(&Dice::new(1, 10));
        let level = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.level())
            .unwrap_or(1);
        let amount = raw + level;
        // Spend the feature now so a duplicate queued use can't slip
        // through — keeps inventory-style consistency with potions.
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(SECOND_WIND_TAG);
        }
        encounter.log(format!(
            "  second wind: 1d10({})+{} = {} HP",
            raw, level, amount
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static SECOND_WIND: LazyLock<SecondWind> = LazyLock::new(|| SecondWind {});

/// Fighter Action Surge — free; gain an extra Action this turn. Once per
/// long rest. Doesn't grant a Bonus Action or Movement (RAW: Action only).
pub struct ActionSurge {}

impl Action for ActionSurge {
    fn name(&self) -> &str {
        "action surge"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["as", "surge"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn is_harmful(&self) -> bool {
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
        // RAW: Action Surge is "no action required" — no cost.
        free_cost()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, ACTION_SURGE_TAG)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        spend_feature_and_grant_extra_action(
            encounter,
            caster_id,
            ACTION_SURGE_TAG,
            "  action surge: extra Action gained.",
        )
    }
}

pub static ACTION_SURGE: LazyLock<ActionSurge> = LazyLock::new(|| ActionSurge {});

/// Tag for Cunning Action — at-will class feature, not consumable, so
/// it never appears in `features_remaining`. Kept as a const for
/// symmetry with the once-per-rest tags above and so creature templates
/// can declare it explicitly.
pub const CUNNING_ACTION_TAG: &str = "rogue.cunning_action";

/// 5e Rogue Assassin **Assassinate** (level 3 subclass) feature tag.
/// Passive once-only rider: the assassin rolls with advantage on any
/// attack against a creature that hasn't taken a turn yet in this
/// combat. RAW also crits on a hit against a *surprised* target, but
/// we don't model surprise as a discrete state — the engine starts
/// every encounter in initiative order with all actors eligible.
///
/// Read at `EncounterInstance::compute_attack_mode` next to the Pack
/// Tactics / Wolf Totem advantage clauses; the target's
/// `has_taken_turn_in_combat` latch is set on the first turn-start
/// (see `start_turn_for`). Stored as a `has_passive_feature` flag so
/// it never consumes a per-rest charge — the gate is purely the
/// target-side latch.
pub const ASSASSINATE_TAG: &str = "rogue.assassinate";

/// Rogue Cunning Action — bonus-action Dash. 5e gives the rogue a choice
/// of Dash, Disengage, or Hide as a bonus action; we expose Dash here
/// (the most universally useful) and leave a follow-up CunningDisengage
/// / CunningHide pair that mirror the same gating. This is the
/// signature once-a-turn rogue mobility tool.
pub struct CunningDash {}

impl Action for CunningDash {
    fn name(&self) -> &str {
        "cunning dash"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["cdash", "ca-dash"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        bonus_action_only()
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let speed = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.speed())
            .unwrap_or(0.0);
        encounter.log("  cunning dash: extra movement gained.".to_string());
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Movement(speed),
        })]
    }
}

pub static CUNNING_DASH: LazyLock<CunningDash> = LazyLock::new(|| CunningDash {});

/// Rogue Cunning Disengage — bonus-action Disengage. Same effect as the
/// regular Disengage action (your movement this turn doesn't provoke
/// OAs), at the cheaper bonus-action cost.
pub struct CunningDisengage {}

impl Action for CunningDisengage {
    fn name(&self) -> &str {
        "cunning disengage"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["cdis", "ca-dis"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        bonus_action_only()
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::SetDisengaging {
            actor_id: caster_id,
            disengaging: true,
        })]
    }
}

pub static CUNNING_DISENGAGE: LazyLock<CunningDisengage> = LazyLock::new(|| CunningDisengage {});

/// Shared side-effects payload for any bonus-action Hide feature —
/// installs the `Hidden` condition on the caster with a Permanent
/// timer (cleared on the caster's next attack via
/// `clear_attack_advantage_riders`). Sibling to Cunning Hide (Rogue
/// Cunning Action) and Vanish (Ranger lv14) — both classes get a
/// "bonus-action Hide" pick that RAW-agnostically drops the same
/// one-shot attack-advantage rider on the holder. Extracted so a new
/// bonus-action Hide feature (Skulker feat, a hypothetical future
/// Shadow Monk pick) lands as a one-line action `side_effects`
/// delegating here rather than another hand-copied `ApplyCondition`
/// literal.
pub fn bonus_action_hide_effects(caster_id: usize) -> Vec<Box<dyn ApplicableSideEffect>> {
    vec![Box::new(ApplyCondition {
        actor_id: caster_id,
        condition: Condition::Hidden,
        timer: ConditionTimer::Permanent,
    })]
}

/// Rogue Cunning Hide — bonus-action Hide. Same condition as the regular
/// Hide action (Hidden flag for one-shot attack-advantage), at the
/// cheaper bonus-action cost. Keeps the rogue's signature cunning-action
/// trio symmetric (Dash / Disengage / Hide).
pub struct CunningHide {}

impl Action for CunningHide {
    fn name(&self) -> &str {
        "cunning hide"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["chide", "ca-hide"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        bonus_action_only()
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Hidden lasts until the rogue's next attack — same one-shot
        // attack-advantage rider as the Hide Action. Tracked via the
        // existing `Hidden` condition. Routed through the shared
        // `bonus_action_hide_effects` helper so any future bonus-
        // action Hide feature (Vanish, Skulker feat, ...) lands as a
        // one-line delegation.
        bonus_action_hide_effects(caster_id)
    }
}

pub static CUNNING_HIDE: LazyLock<CunningHide> = LazyLock::new(|| CunningHide {});

/// 5e Ranger **Vanish** (class feature, level 14) — feature tag. Passive
/// gate on the paired `VANISH` action, which is a bonus-action Hide
/// (same one-shot attack-advantage rider as CunningHide / the baseline
/// Hide action) available to any ranger regardless of subclass. RAW
/// also states "you can't be tracked by nonmagical means" which is a
/// pure narrative clause with no combat surface — no mechanical wiring
/// needed. Ships as a `has_passive_feature` tag rather than an
/// action-list-only pick so future features that check for "does this
/// actor have Vanish?" (e.g. an anti-tracker override in a survival
/// mini-game) can route through one lookup.
///
/// Sibling to `CUNNING_ACTION_TAG` on the tag lane — both are
/// permanent passive class features with no per-rest charge. The
/// action gates on this tag via `feature_available` inside
/// `custom_validate_input` so a template that doesn't carry the tag
/// can't accidentally fire it if the action leaks onto its action list.
pub const VANISH_TAG: &str = "ranger.vanish";

/// Ranger Vanish — bonus-action Hide gated on the `VANISH_TAG` passive
/// feature. Same one-shot attack-advantage rider as Cunning Hide, at
/// the cheaper bonus-action cost. Ships on `RANGER_TEMPLATE` (and by
/// inheritance on `HUNTER_RANGER_TEMPLATE`); the RAW "can't be tracked
/// by nonmagical means" clause is a pure narrative rider with no
/// mechanical surface in the combat engine.
pub struct Vanish {}

impl Action for Vanish {
    fn name(&self) -> &str {
        "vanish"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["vnsh", "rvan"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Gate on the ranger having the VANISH_TAG passive — the
        // action only fires for a template that ships the tag. Actors
        // without the tag never see the action's cost / effect surface.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_passive_feature(VANISH_TAG))
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Routed through the shared `bonus_action_hide_effects` helper
        // so any tweak to the Hide install lands once across every
        // bonus-action-Hide caller (Cunning Hide, Vanish, ...).
        bonus_action_hide_effects(caster_id)
    }
}

pub static VANISH: LazyLock<Vanish> = LazyLock::new(|| Vanish {});

/// 5e Tasha's Rogue **Steady Aim** (level 3 alternate Cunning Action).
/// Bonus action: grant the rogue advantage on their next attack roll this
/// turn at the cost of their speed dropping to 0 until end of turn.
/// Pairs naturally with Sneak Attack (advantage qualifies for the rider)
/// and the rogue's ranged finesse options — the speed-zero cost is
/// minor for a sniping rogue and the advantage gate replaces the
/// ally-adjacency / disadvantage-free condition that Sneak Attack
/// normally checks.
///
/// RAW gate: "only if you haven't moved during this turn". Enforced via
/// `ActorInstance::has_moved_this_turn`, which compares the current
/// movement budget against `speed()`. After use, `zero_movement()`
/// drains the budget so the rest of the turn is locked in place. The
/// advantage rides on the existing `Helped` one-shot condition (cleared
/// on the next swing in `clear_attack_advantage_riders`).
pub struct SteadyAim {}

impl Action for SteadyAim {
    fn name(&self) -> &str {
        "steady aim"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["aim", "sa-rogue"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
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
                && !a.has_moved_this_turn()
                // No-stack: re-priming would just refresh the timer.
                && !a.has_condition(Condition::Helped)
        })
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let line = encounter.actors.get_mut(&caster_id).map(|actor| {
            actor.zero_movement();
            format!(
                "{} takes steady aim — speed drops to 0 for the rest of the turn.",
                actor.name()
            )
        });
        if let Some(line) = line {
            encounter.log(line);
        }
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Helped,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static STEADY_AIM: LazyLock<SteadyAim> = LazyLock::new(|| SteadyAim {});

/// 5e 2024 Rogue **Cunning Strike** flavors. Each variant is a bonus-action
/// prime: the rogue declares which trick they'll layer onto their next
/// Sneak Attack, trading one (or two) sneak-attack dice for a tactical
/// effect. The shortsword's sneak-attack rider walks the active prime
/// table, deducts the dice cost, and applies the effect — see the
/// `consume_cunning_strike_*` chokepoints in `RogueShortsword::side_effects`.
///
/// We model the four 2024 base variants:
/// - **Poison** (1d6 cost): CON save vs rogue's DEX-based DC; on fail the
///   target is `Poisoned` for 10 rounds (1 minute RAW).
/// - **Trip** (1d6 cost): DEX save vs the same DC; on fail the target is
///   knocked Prone (Large or smaller — gated at the consume site).
/// - **Withdraw** (1d6 cost): no save; immediately after the sneak the
///   rogue moves up to half speed without provoking OAs.
/// - **Daze** (2d6 cost): CON save; on fail the target's next turn loses
///   their Action and Reaction (modeled via the existing `MindWhipped`
///   action-clip plus `NoReaction`).
///
/// Each variant is validated mutually-exclusive (no double-priming) and
/// only valid when sneak attack hasn't been used this turn — otherwise
/// the bonus-action would be wasted on a swing that can't carry the
/// rider. Self-clearing via `UntilStartOfNextTurn` if the rogue never
/// connects with a sneak-eligible swing.
///
/// Cunning Strike: Poison variant. 1d6 sneak-attack die cost; on a
/// successful sneak-attack hit, the target makes a CON save vs the
/// rogue's DEX-based DC (8 + prof + DEX). On fail, Poisoned for 10
/// rounds (1 minute RAW). Bonus-action prime.
pub struct CunningStrikePoison {}

impl Action for CunningStrikePoison {
    fn name(&self) -> &str {
        "cunning strike (poison)"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs-poison", "cspoison", "cunningpoison"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        cunning_strike_prime_ok(encounter, caster_id)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::CunningStrikePoison,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CUNNING_STRIKE_POISON: LazyLock<CunningStrikePoison> =
    LazyLock::new(|| CunningStrikePoison {});

/// Cunning Strike: Trip variant. 1d6 sneak-attack die cost; target makes
/// a DEX save (Large or smaller). On fail, knocked Prone. Bonus-action
/// prime.
pub struct CunningStrikeTrip {}

impl Action for CunningStrikeTrip {
    fn name(&self) -> &str {
        "cunning strike (trip)"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs-trip", "cstrip", "cunningtrip"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        cunning_strike_prime_ok(encounter, caster_id)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::CunningStrikeTrip,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CUNNING_STRIKE_TRIP: LazyLock<CunningStrikeTrip> =
    LazyLock::new(|| CunningStrikeTrip {});

/// Cunning Strike: Withdraw variant. 1d6 sneak-attack die cost; on a
/// successful sneak-attack hit, the rogue immediately moves up to half
/// their speed without provoking opportunity attacks. Bonus-action prime.
pub struct CunningStrikeWithdraw {}

impl Action for CunningStrikeWithdraw {
    fn name(&self) -> &str {
        "cunning strike (withdraw)"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs-withdraw", "cswith", "cunningwithdraw"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        cunning_strike_prime_ok(encounter, caster_id)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::CunningStrikeWithdraw,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CUNNING_STRIKE_WITHDRAW: LazyLock<CunningStrikeWithdraw> =
    LazyLock::new(|| CunningStrikeWithdraw {});

/// Cunning Strike: Daze variant. 2d6 sneak-attack die cost (RAW: 2
/// dice — strongest variant); target makes a CON save on a successful
/// sneak hit, and on fail their next turn loses its Action and Reaction
/// (we layer the action clip via `MindWhipped` plus a `NoReaction`).
/// Bonus-action prime.
pub struct CunningStrikeDaze {}

impl Action for CunningStrikeDaze {
    fn name(&self) -> &str {
        "cunning strike (daze)"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs-daze", "csdaze", "cunningdaze"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Daze costs 2d6 — only worth priming when the rogue's sneak
        // attack pool can spare 2 dice (level 3+ = 2d6, level 5+ = 3d6).
        if !cunning_strike_prime_ok(encounter, caster_id) {
            return false;
        }
        encounter.actors.get(&caster_id).is_some_and(|a| {
            crate::actions::class_attacks::sneak_attack_dice_for_level(a.level()) >= 2
        })
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::CunningStrikeDaze,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CUNNING_STRIKE_DAZE: LazyLock<CunningStrikeDaze> =
    LazyLock::new(|| CunningStrikeDaze {});

/// Shared validation for the Cunning Strike bonus-action primes. Returns
/// true when:
/// - the rogue is combat-active,
/// - no other Cunning Strike prime is already up (mutually exclusive — the
///   first prime survives the bonus-action burn, the duplicate is the
///   wasted resource so we reject),
/// - the rogue hasn't already used Sneak Attack this turn (priming after
///   the once-per-turn fuse is spent is just a bonus-action waste).
fn cunning_strike_prime_ok(encounter: &EncounterInstance, caster_id: usize) -> bool {
    let Some(actor) = encounter.actors.get(&caster_id) else {
        return false;
    };
    if !actor.is_combat_active() {
        return false;
    }
    if actor.sneak_attack_used() {
        return false;
    }
    !actor.has_any_cunning_strike_prime()
}

/// Tag for Fighter's Indomitable — once per long rest.
pub const INDOMITABLE_TAG: &str = "fighter.indomitable";

/// Fighter Indomitable — free no-cost self-flag. Sets a one-shot "reroll
/// the next failed save" marker on the actor via `mark_indomitable_pending`.
/// The reroll lives at the save site (`EncounterInstance::roll_save`):
/// if the marker is set and the save fails, the engine re-rolls once and
/// keeps the better result, then clears the marker. Once per long rest
/// (consumed eagerly here so a duplicate queued use can't double-dip).
pub struct Indomitable {}

impl Action for Indomitable {
    fn name(&self) -> &str {
        "indomitable"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["indom", "indo"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        // No action / bonus action cost — RAW: "no action required",
        // just spend the feature.
        free_cost()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, INDOMITABLE_TAG)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(INDOMITABLE_TAG);
            actor.mark_indomitable_pending();
        }
        encounter.log("  indomitable: next failed save will be re-rolled.".to_string());
        Vec::new()
    }
}

pub static INDOMITABLE: LazyLock<Indomitable> = LazyLock::new(|| Indomitable {});

/// Class-feature tag for Barbarian's Rage — once per long rest.
pub const RAGE_TAG: &str = "barbarian.rage";

/// Rage — barbarian feature, bonus action. Applies the `Raging` condition
/// to the caster: resistance to bludgeoning / piercing / slashing damage
/// (folded into `effective_damage`), and advantage on STR checks / saves
/// (read by `compute_save_mode`). Lasts 10 rounds (approximation of the
/// 5e 1-minute duration). Once-per-long-rest gated on the `RAGE_TAG`
/// feature flag.
pub struct Rage {}

impl Action for Rage {
    fn name(&self) -> &str {
        "rage"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["rg", "anger"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, RAGE_TAG)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // 5e Barbarian **Persistent Rage** (lv15 class feature): the
        // barbarian's rage no longer ends prematurely. Our engine's
        // rage never *ends* early — it holds for a fixed timer — so we
        // repurpose the flag as a duration bump, doubling the install
        // window from Rounds(10) to Rounds(20). Matches the RAW
        // "1 minute → practically-encounter-length" intent for any
        // build that ships the flag on its template.
        let has_persistent = encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_persistent_rage());
        let timer = if has_persistent {
            ConditionTimer::Rounds(20)
        } else {
            ConditionTimer::Rounds(10)
        };
        let log_line = if has_persistent {
            "  rage: barbarian enters a battle frenzy (persistent rage — extended duration)."
        } else {
            "  rage: barbarian enters a battle frenzy."
        };
        prime_self_condition(
            encounter,
            caster_id,
            RAGE_TAG,
            Condition::Raging,
            timer,
            log_line,
        )
    }
}

pub static RAGE: LazyLock<Rage> = LazyLock::new(|| Rage {});

/// 5e Barbarian Path of the Berserker — **Frenzy** (level 3). While raging,
/// the barbarian can use a bonus action on each of their turns to make
/// one additional melee weapon attack. Passive subclass tag (no per-rest
/// charge — the rate limit is the once-per-turn bonus action lane plus the
/// Rage duration ceiling). Held in `features_max` so the `Frenzy` action's
/// `custom_validate_input` can gate on `has_passive_feature(FRENZY_TAG)`
/// without re-checking the per-rest pool.
///
/// RAW: after the rage ends, the barbarian suffers one level of exhaustion.
/// We skip the exhaustion rider for now — modeling the post-rage hook
/// requires a timer-expiry callback that doesn't exist yet. The 10-round
/// Rage cap (and the once-per-long-rest gate on Rage itself) keeps the
/// Frenzy uses bounded per encounter regardless.
pub const FRENZY_TAG: &str = "barbarian.frenzy";

/// Berserker Frenzy bonus action: spend a bonus action to gain a fresh
/// Action this turn, used for one extra melee weapon swing. Mirrors the
/// `FlurryOfBlows` shape (BA → +Action token) so the barbarian's existing
/// Greataxe / weapon action consumes the granted Action. Gated on:
///   - holder has the `FRENZY_TAG` passive subclass feature
///   - holder currently has the `Raging` condition (Frenzy is rage-only)
///   - combat-active (no firing during a downed state)
pub struct Frenzy {}

impl Action for Frenzy {
    fn name(&self) -> &str {
        "frenzy"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fr", "frenzied"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        raging_feature_ready(encounter, caster_id, FRENZY_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  frenzy: barbarian gains an extra Action for a follow-up strike.".to_string());
        grant_extra_action(caster_id)
    }
}

pub static FRENZY: LazyLock<Frenzy> = LazyLock::new(|| Frenzy {});

/// 5e Barbarian Path of the Berserker — **Mindless Rage** (level 6) feature
/// tag. Passive subclass feature: while raging, the barbarian is immune to
/// installs of Charmed and Frightened, AND any existing Charmed or
/// Frightened is suppressed for the duration (RAW: "you can't be charmed
/// or frightened while raging"; installs mid-rage bounce, and any
/// pre-existing installs are held at bay). The immunity flips off the
/// moment the `Raging` condition drops.
///
/// Stored as a `has_passive_feature` flag so it composes naturally with
/// the existing `Raging` condition gate. Read by the flag-driven
/// immunity table in `actor_template.rs` next to Nature's Ward
/// (Charmed / Frightened) and Halfling Brave (Frightened) — the row's
/// closure predicate checks both the passive flag AND the Raging
/// condition, so a non-raging Berserker gets no benefit. RAW also
/// suspends any pre-existing Charmed / Frightened while raging and
/// resumes them when rage ends; our engine's `dynamic_immunity_to`
/// short-circuits the "has this condition installed" check for immune
/// creatures at every read site (compute_attack_mode, etc.), so a
/// mid-rage install that later drops-and-resumes matches this
/// simplified read-only suppression.
///
/// Pairs naturally with the Berserker's Frenzy — the Berserker
/// barbarian is meant to bull-rush the enemy caster's opener, and the
/// classic anti-melee-brute answer is a fear / charm lockout. Mindless
/// Rage gives the Berserker the "no, I don't care" no-op that frees
/// them to keep swinging while every teammate eats the same effect.
pub const MINDLESS_RAGE_TAG: &str = "barbarian.mindless_rage";

/// 5e Barbarian Path of the Zealot — **Divine Fury** (level 3) feature
/// tag. Passive subclass feature: while raging, the first creature the
/// zealot hits with a weapon attack on each of their turns takes extra
/// `1d6 + half barbarian level` (min +1) radiant damage. RAW gives the
/// zealot a choice of radiant or necrotic per RAW; we lock the type to
/// radiant to keep the tell "holy warrior" flavor unambiguous.
///
/// Stored as a `has_passive_feature` flag with no per-rest charge — the
/// rate limit is the per-turn `divine_fury_used` ledger on
/// `ActorInstance`, sibling to `sneak_attack_used` / `colossus_slayer_used`
/// / `foe_slayer_used`. Cleared at turn-start by `reset_for_new_round`
/// so the opening swing of every turn re-arms the rider.
///
/// Three gates on the swing site (`engine::attack::resolve_attack_outcome`):
///   1. Caster has the `DIVINE_FURY_TAG` passive feature flag.
///   2. Caster carries the `Raging` condition (RAW: "while you're
///      raging"). The rider vanishes the moment rage drops.
///   3. Caster hasn't already fired Divine Fury this turn.
///
/// Composes cleanly with the barbarian's other on-hit riders: Brutal
/// Critical (extra weapon die on a crit), Rage's flat +2 melee bump
/// through `MELEE_CASTER_BUMPS`, and any smite-lane rider a hypothetical
/// paladin / barbarian multiclass might carry — all fire on the same
/// swing without stepping on each other. Weapon-only (RAW: "with a
/// weapon attack") so a hypothetical spell attack won't consume the
/// prime.
pub const DIVINE_FURY_TAG: &str = "barbarian.divine_fury";

/// 5e Barbarian Path of the Totem Warrior — **Bear Totem Spirit** (level 3).
/// Passive subclass feature: while raging, the holder has resistance to all
/// damage except psychic. Replaces the default Rage's "BPS resistance only"
/// envelope for the lucky few Bear Totem barbarians.
///
/// Stored as a `has_passive_feature` flag rather than a fresh condition so
/// it composes naturally with the existing `Raging` condition gate (only
/// fires *while* Raging). Read at the damage-pipeline chokepoint
/// `has_condition_resistance` next to the `TYPED_RESISTANCE_CONDITIONS`
/// table so the standard 5e "one halving per damage instance" rule still
/// holds — Bear Totem doesn't stack with a separate template resistance
/// (e.g. a dwarven barbarian still only gets the single /2 on poison).
pub const BEAR_TOTEM_TAG: &str = "barbarian.bear_totem";

/// 5e Barbarian Path of the Totem Warrior — **Wolf Totem Spirit** (level 3).
/// Passive subclass feature: while raging, the holder's allies have advantage
/// on melee attack rolls against any creature footprint-adjacent to the
/// barbarian. The pack-hunter flavor — the wolf totem barbarian becomes a
/// melee anchor whose presence sharpens every teammate's swing on the same
/// target. Stored as a `has_passive_feature` flag so it composes with the
/// existing `Raging` gate and only fires while the rage is active.
///
/// Read at `EncounterInstance::compute_attack_mode` next to the Pack Tactics
/// branch (same ally-side advantage shape, just gated on a passive-feature +
/// raging cohort rather than a template trait). Mirrors Pack Tactics' "one
/// halving per damage instance" parallel — multiple wolf totem allies don't
/// double-stack advantage, since Advantage already collapses to the
/// `RollMode::Advantage` lattice point.
pub const WOLF_TOTEM_TAG: &str = "barbarian.wolf_totem";

/// 5e Barbarian Path of the Totem Warrior — **Eagle Totem Spirit** (level 3).
/// Passive subclass feature: while raging, the eagle barbarian can Dash as a
/// bonus action (the kiter/skirmisher flavor — the eagle barbarian closes or
/// re-positions twice in one turn while the wolf totem anchors and the bear
/// totem tanks). Stored as a `has_passive_feature` flag so it composes
/// naturally with the `Raging` condition gate without consuming a per-rest
/// charge.
///
/// The Dash-as-bonus-action half is exposed via the `EAGLE_DIVE` action,
/// which mirrors `CunningDash`'s shape (`GiveResource::Movement(speed)`)
/// gated on raging + Eagle Totem rather than the rogue's Cunning Action.
pub const EAGLE_TOTEM_TAG: &str = "barbarian.eagle_totem";

/// Eagle Totem Spirit's Dash-as-bonus-action: spend a bonus action to gain
/// a fresh chunk of movement equal to the holder's speed. Mirrors the
/// rogue Cunning Dash shape, but gated on the Eagle Totem barbarian's
/// `EAGLE_TOTEM_TAG` passive feature AND the `Raging` condition — outside
/// of rage the eagle barbarian has no extra mobility.
pub struct EagleDive {}

impl Action for EagleDive {
    fn name(&self) -> &str {
        "eagle dive"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ed", "dive"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        raging_feature_ready(encounter, caster_id, EAGLE_TOTEM_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let speed = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.speed())
            .unwrap_or(0.0);
        encounter.log("  eagle dive: barbarian surges forward on totem wings.".to_string());
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Movement(speed),
        })]
    }
}

pub static EAGLE_DIVE: LazyLock<EagleDive> = LazyLock::new(|| EagleDive {});

/// 5e Barbarian Path of the Totem Warrior — **Tiger Totem Spirit** (level 3,
/// 2024 PHB Path of the Wild Heart flavor). Passive subclass feature: while
/// raging, the holder's walking speed increases by 10 ft. Fourth sibling of
/// Bear (damage envelope), Wolf (ally-aura), Eagle (bonus-action Dash) —
/// the Tiger plays the pure-mobility skirmisher who doesn't need to burn
/// their bonus action on the Dash, since the +10 ft speed is always on
/// while the rage holds. Stored as a `has_passive_feature` flag so it
/// composes with the existing `Raging` condition gate without a per-rest
/// charge.
///
/// Read at the `condition_speed_bonus` chokepoint next to the Longstrider /
/// Expeditious Retreat speed buffs, but gated on `has_condition(Raging) &&
/// has_passive_feature(TIGER_TOTEM_TAG)` rather than a condition flag —
/// outside of rage the tiger barbarian has no extra speed.
pub const TIGER_TOTEM_TAG: &str = "barbarian.tiger_totem";

/// Flat walking-speed bonus in feet granted by the Tiger Totem Spirit
/// passive while raging. Pinned to +10 ft per RAW (PHB Path of the Totem
/// Warrior); exposed as a constant so the
/// `PASSIVE_FEATURE_SPEED_BONUSES` table stays declarative rather than
/// scattering magic numbers into the accessor — same shape as the
/// declared `ELK_TOTEM_SPEED_BONUS` (+15 ft), `WOLVERINE_TOTEM_SPEED_BONUS`
/// (+10 ft), `PANTHER_TOTEM_SPEED_BONUS` (+5 ft), and
/// `UNARMORED_MOVEMENT_SPEED_BONUS` (+10 ft) magnitudes for the sibling
/// speed passives. Same magnitude as Wolverine's rage-gated +10 — the
/// load-bearing distinction between the two rows is the tag identity,
/// not the number.
pub const TIGER_TOTEM_SPEED_BONUS: f32 = 10.0;

/// 5e Barbarian **Fast Movement** (level 5) feature tag. Passive: while
/// not wearing heavy armor, the barbarian's walking speed increases by
/// 10 ft. Our engine doesn't model armor tiers so we collapse the
/// "not-wearing-heavy-armor" gate to "always on for a barbarian holding
/// the tag" — the same simplification we use for Monk Unarmored Movement
/// (the monk's flat AC 15 folds in the Wisdom / Dex scaling instead of
/// gating on armor). Read at the `condition_speed_bonus` chokepoint next
/// to the Longstrider / Tiger Totem speed buffs.
///
/// Distinct from Tiger Totem Spirit (also +10 ft speed) in two ways:
///   1. **Always-on** — Fast Movement fires the moment the barbarian is
///      alive; Tiger Totem gates on the `Raging` condition being active.
///   2. **Non-subclass** — Fast Movement is a class feature every
///      barbarian picks up at level 5, so it stacks additively on Bear /
///      Wolf / Eagle / Berserker barbarians. Tiger + Fast Movement while
///      raging is +20 ft in total.
pub const FAST_MOVEMENT_TAG: &str = "barbarian.fast_movement";

/// Flat walking-speed bonus in feet granted by the Fast Movement class
/// passive. Pinned to +10 ft per RAW (PHB Barbarian lv5); exposed as a
/// constant so the `PASSIVE_FEATURE_SPEED_BONUSES` table stays
/// declarative rather than scattering magic numbers into the accessor.
/// Sibling to `TIGER_TOTEM_SPEED_BONUS` / `WOLVERINE_TOTEM_SPEED_BONUS`
/// / `UNARMORED_MOVEMENT_SPEED_BONUS` (all +10 ft) and
/// `ROVING_SPEED_BONUS` (+5 ft) / `PANTHER_TOTEM_SPEED_BONUS` (+5 ft)
/// / `ELK_TOTEM_SPEED_BONUS` (+15 ft) on the "declared magnitude next
/// to the tag definition" lane.
pub const FAST_MOVEMENT_SPEED_BONUS: f32 = 10.0;

/// 5e Monk **Unarmored Movement** (level 2) feature tag. Passive: while
/// not wearing armor and not wielding a shield, the monk's walking speed
/// increases by 10 ft. Our engine doesn't model armor tiers on the monk
/// chassis (`ac` is a flat 15 baked from Unarmored Defense), so the
/// "not wearing armor and no shield" gate collapses to "always on for a
/// monk holding the tag" — the same simplification we already apply to
/// Barbarian Fast Movement's "not wearing heavy armor" clause.
///
/// Read at the shared `passive_feature_speed_bonus` chokepoint next to
/// Fast Movement (Barbarian lv5, +10 ft always-on), Tiger Totem
/// (Barbarian subclass, +10 ft while raging), and Roving (Ranger 2024
/// lv6, +5 ft always-on) — one lookup table, one source of truth.
/// Sibling to Fast Movement on the always-on +10 ft lane and to Roving
/// on the "always-on passive speed passive on a non-barbarian chassis"
/// lane.
///
/// RAW scales the bump with monk level: +10 ft at lv2, +15 ft at lv6,
/// +20 ft at lv10, +25 ft at lv14, +30 ft at lv18. We pin to the +10
/// baseline that a level-2+ monk gets — the CR-1.5 MONK_TEMPLATE
/// targets a balanced playable level rather than lockstep PHB
/// progression, matching how Purity of Body (RAW lv10) and Diamond
/// Soul (RAW lv14) already ride the same template above their strict
/// gate.
///
/// Always-on passive; no per-rest charge and no condition gate. The
/// tag lives in the actor's `features` pool, not in
/// `SHORT_REST_FEATURES` / long-rest tables — nothing consumes it and
/// nothing refreshes it.
pub const UNARMORED_MOVEMENT_TAG: &str = "monk.unarmored_movement";

/// Flat walking-speed bonus in feet granted by the Unarmored Movement
/// passive. Pinned to +10 ft (the level-2 baseline) per RAW; exposed
/// as a constant so the `PASSIVE_FEATURE_SPEED_BONUSES` table stays
/// declarative rather than scattering magic numbers into the accessor.
pub const UNARMORED_MOVEMENT_SPEED_BONUS: f32 = 10.0;

/// 5e Barbarian Path of the Totem Warrior — **Elk Totem Spirit** (RAW
/// XGtE lv3 expansion of the Bear / Wolf / Eagle triad). Passive
/// subclass feature: while raging, the holder's walking speed
/// increases by 15 ft. Fifth sibling of Bear (damage envelope), Wolf
/// (ally-aura), Eagle (bonus-action Dash), Tiger (+10 ft rage
/// mobility) — the Elk is the pure sprint totem, hitting +15 ft on
/// the same rage gate that Tiger uses for its +10.
///
/// Read at the shared `passive_feature_speed_bonus` chokepoint next
/// to Tiger Totem (compound gate: `has_condition(Raging) &&
/// has_passive_feature(...)`) — outside of rage the elk barbarian
/// has no extra speed. Composes additively with Fast Movement's
/// always-on +10 ft: a raging elk barbarian at level 5+ opens at +25
/// ft over the base 30 ft chassis (= 55 ft walking, a full extra
/// move on the opening round vs. a Tiger totem's +20 stack).
///
/// Distinct from Tiger Totem Spirit (also rage-gated) in one way:
/// **magnitude** — Elk is the bigger sprint (+15 vs. Tiger's +10),
/// trading off against the Tiger's kit-shape identity in the
/// codebase's totem lineup. Ships on `ELK_TOTEM_BARBARIAN_TEMPLATE`
/// via the shared `totem_barbarian_template` helper — one-line entry
/// alongside Bear / Wolf / Eagle / Tiger just as the helper's
/// docstring promised.
pub const ELK_TOTEM_TAG: &str = "barbarian.elk_totem";

/// Flat walking-speed bonus in feet granted by the Elk Totem Spirit
/// passive while raging. Pinned to +15 ft per RAW (XGtE); exposed as
/// a constant so the `PASSIVE_FEATURE_SPEED_BONUSES` table stays
/// declarative rather than scattering magic numbers into the accessor.
/// Sibling to `ROVING_SPEED_BONUS` (Ranger +5) and
/// `UNARMORED_MOVEMENT_SPEED_BONUS` (Monk +10) on the "declared
/// magnitude next to the tag definition" lane.
pub const ELK_TOTEM_SPEED_BONUS: f32 = 15.0;

/// 5e Barbarian Path of the Wild Heart — **Wolverine Totem Spirit**
/// (2024 PHB lv3, sixth totem in the Bear / Wolf / Eagle / Tiger / Elk
/// / Wolverine lineup). Passive subclass feature: while raging, the
/// holder's walking speed increases by 10 ft. Fits into the same
/// rage-gated mobility lane the Tiger totem already occupies — same
/// compound gate (`has_condition(Raging) && has_passive_feature(...)`)
/// and same +10 ft magnitude, but a distinct tag / template so a
/// Tiger-vs-Wolverine encounter renders unambiguously and the two
/// flags never legally co-occur on a single PC per RAW (one totem
/// pick per barbarian).
///
/// Read at the shared `PASSIVE_FEATURE_SPEED_BONUSES` chokepoint next
/// to Tiger Totem — the two rows fire independently on their
/// respective tags so a Tiger barbarian doesn't accidentally pick up
/// the Wolverine bonus and vice versa. Composes additively with Fast
/// Movement (Barbarian lv5, +10 ft always-on): a raging Wolverine
/// barbarian at level 5+ opens at 50 ft (30 base + 10 Wolverine + 10
/// Fast Movement), matching the Tiger totem's opening sprint.
///
/// Ships on `WOLVERINE_TOTEM_BARBARIAN_TEMPLATE` via the shared
/// `totem_barbarian_template` helper — one-line entry alongside Bear
/// / Wolf / Eagle / Tiger / Elk, exactly as the helper's docstring
/// promised. Glyph 'V' — distinct from the other totem glyphs
/// (Berserker 'Z', baseline Barbarian 'B', Totem/Bear 'T', Wolf 'W',
/// Eagle 'A', Tiger 'I', Elk 'E', Zealot 'X').
pub const WOLVERINE_TOTEM_TAG: &str = "barbarian.wolverine_totem";

/// Flat walking-speed bonus in feet granted by the Wolverine Totem
/// Spirit passive while raging. Pinned to +10 ft per RAW (2024 Wild
/// Heart Wolverine); exposed as a constant so the
/// `PASSIVE_FEATURE_SPEED_BONUSES` table stays declarative rather than
/// scattering magic numbers into the accessor. Same magnitude as
/// Tiger Totem's rage-gated +10 — the load-bearing distinction
/// between the two rows is the tag identity, not the number, so a
/// Tiger barbarian dialing on `WOLVERINE_TOTEM_TAG` (or vice versa)
/// via `grant_feature_for_test` reads a stacked +20 total, matching
/// the additive semantics of the table.
pub const WOLVERINE_TOTEM_SPEED_BONUS: f32 = 10.0;

/// 5e Barbarian Path of the Totem Warrior — **Panther Totem Spirit**
/// (RAW XGtE lv3 alongside the Elk expansion of the PHB Bear / Wolf /
/// Eagle triad). Passive subclass feature: while raging, the holder's
/// walking speed increases by 5 ft. RAW: "You gain a climbing speed
/// equal to your walking speed." The engine doesn't model climbing as
/// a distinct movement axis — every non-Wall tile is walked over the
/// same 2.5-ft grid regardless of vertical geometry — so the
/// "climbing = walking" clause has no direct combat surface. We
/// approximate the mobility bump by folding a slim +5 ft flat bonus
/// through the shared `PASSIVE_FEATURE_SPEED_BONUSES` table: a raging
/// panther barbarian kites half a step further than a stock rager
/// without the +10 Tiger / +15 Elk sprint magnitudes, which reads on
/// the map as the intended "slinky prowler" identity even without a
/// third-dimensional climb axis to represent.
///
/// Read at the shared `PASSIVE_FEATURE_SPEED_BONUSES` chokepoint —
/// compound gate combining `has_condition(Raging)` AND
/// `has_passive_feature(PANTHER_TOTEM_TAG)` so outside of rage the
/// panther barbarian has no extra speed, matching the Tiger / Elk /
/// Wolverine rage-gate cadence.
///
/// Distinct from every other rage-gated totem row in the cohort by
/// magnitude alone: Panther +5, Tiger +10, Wolverine +10, Elk +15.
/// The four flags never legally co-occur on a single PC per RAW (one
/// totem pick per barbarian). Ships on
/// `PANTHER_TOTEM_BARBARIAN_TEMPLATE` via the shared
/// `totem_barbarian_template` helper — one-line entry alongside the
/// other five totems. Glyph 'P' — distinct from every existing totem
/// glyph (Berserker 'Z', baseline Barbarian 'B', Totem/Bear 'T',
/// Wolf 'W', Eagle 'A', Tiger 'I', Elk 'E', Wolverine 'V', Zealot
/// 'X').
pub const PANTHER_TOTEM_TAG: &str = "barbarian.panther_totem";

/// Flat walking-speed bonus in feet granted by the Panther Totem
/// Spirit passive while raging. Pinned to +5 ft in this engine —
/// smaller than every other rage-gated totem (Tiger / Wolverine +10,
/// Elk +15) to reflect that RAW's XGtE Panther grants climbing speed
/// only, and the engine surfaces only a fraction of that bump through
/// the flat-walking-speed lane since there's no 3D terrain to
/// differentiate. Exposed as a constant so the
/// `PASSIVE_FEATURE_SPEED_BONUSES` table stays declarative rather
/// than scattering magic numbers into the accessor. Sibling to
/// `ROVING_SPEED_BONUS` (Ranger +5) on the +5-ft-bonus lane — same
/// magnitude, different chassis and different gate (Roving is
/// always-on, Panther is rage-gated).
pub const PANTHER_TOTEM_SPEED_BONUS: f32 = 5.0;

/// 5e Monk **Purity of Body** (level 10) feature tag. Passive: the monk
/// gains immunity to disease and poison (RAW: "your mastery of the ki
/// flowing through you makes you immune to disease and poison"). Two
/// mechanically visible effects in our engine:
///   1. Immunity to the `Poisoned` condition — installs bounce at the
///      `dynamic_immunity_to(Poisoned)` chokepoint next to the Purified
///      / Petrified / Fey Ancestry checks.
///   2. Immunity to poison damage — `effective_damage` short-circuits
///      to 0 at the same "typed condition immunity" lane the Petrified
///      poison-immunity rider already uses; here the gate is a passive
///      feature flag rather than a condition. Disease has no mechanical
///      surface in the combat engine, so the disease half of RAW is a
///      no-op we don't need to wire up.
///
/// Sibling to Sahuagin Blood Frenzy (passive combat advantage) and
/// Bear Totem Spirit (raging damage envelope) on the passive-feature-
/// flag lane — no per-rest charge, no condition gate, just an
/// always-on effect while the monk is alive.
pub const PURITY_OF_BODY_TAG: &str = "monk.purity_of_body";

/// 5e Warlock Otherworldly Patron — **The Undying** — **Aspect of the
/// Moon** eldritch invocation (patron-gated Undying pick, SCAG). Passive:
/// the warlock no longer needs to sleep and can't be forced to sleep by
/// any means. In this engine that collapses to a single mechanically
/// visible clause — **immunity to the `Asleep` condition install** —
/// since natural sleep (long rest) sits outside the combat loop entirely
/// and every `Asleep` source in the engine routes through the standard
/// `add_condition(Asleep, …)` chokepoint that `dynamic_immunity_to`
/// gates on. RAW's "no need to sleep" clause has no mechanical surface
/// on the combat side — it just means the warlock stays awake during a
/// party's long rest without spending a hit die, which the engine's
/// rest lane doesn't model per-actor.
///
/// Read at the shared `FLAG_DRIVEN_IMMUNITIES` cohort in `actor_template.rs`
/// next to Fey Ancestry's Asleep row — same suppressed condition, same
/// "flag closure returns true → install bounces" wire, different
/// source (racial Fey Ancestry vs. Undying-patron invocation). The two
/// rows are OR'd so a Fey-Ancestry Elven Warlock with Aspect of the
/// Moon carries redundant Asleep immunity, and either flag alone is
/// sufficient — same OR semantics the Halfling Brave / Nature's Ward /
/// Mindless Rage rows already use for their respective condition
/// bounces.
///
/// Ships on `UNDYING_WARLOCK_TEMPLATE` — the Otherworldly Patron: The
/// Undying subclass template — alongside the baseline Warlock envelope
/// (CHA-primary Pact Magic, Eldritch Blast + Hex + Witch Bolt at will,
/// Agonizing / Repelling / Eldritch Mind invocations). Distinct
/// mechanically from Fey Ancestry (RAW: advantage on Charmed saves +
/// magic can't put you to sleep — Charmed is the load-bearing distinction
/// on the racial trait) since Aspect of the Moon grants only the sleep
/// half; a Warlock chassis without the tag still eats a Sleep spell
/// even if they also happen to be an Elf.
pub const ASPECT_OF_THE_MOON_TAG: &str = "warlock.aspect_of_the_moon";

/// 5e Warlock Otherworldly Patron — **The Archfey** — **Beguiling
/// Defenses** subclass feature tag (level 10, PHB). Passive: the warlock
/// is immune to being **Charmed**. RAW also grants a reaction that lets
/// the warlock charm-back a would-be charmer for 1 minute on a failed
/// WIS save (psychic damage rider), but the reaction half has no clean
/// combat surface without a save-triggered reaction framework; we ship
/// the Charmed install-immunity half, which is the mechanical load-
/// bearing clause on the passive lane.
///
/// Read at the shared `FLAG_DRIVEN_IMMUNITIES` cohort in
/// `actor_template.rs` next to Fey Ancestry's Charmed row — same
/// suppressed condition, same "flag closure returns true → install
/// bounces" wire, different source (Elven / Half-Elven / Drow racial
/// Fey Ancestry vs. Archfey-patron subclass feature). The two rows are
/// OR'd so a Fey-Ancestry Elven Archfey Warlock carries redundant
/// Charmed immunity, and either flag alone is sufficient — same OR
/// semantics the Halfling Brave / Nature's Ward / Aspect of the Moon
/// rows already use for their respective condition bounces.
///
/// Distinct from `PSYCHIC_DEFENSES_TAG` (Aberrant Mind Sorcerer lv14,
/// Charmed + Frightened) on the condition axis: Beguiling Defenses
/// covers only Charmed (RAW-exact — the Archfey warlock's charm-back
/// clause presumes the warlock stays lucid, so the RAW immunity is
/// scoped to Charmed alone) while Psychic Defenses covers both. A
/// hypothetical Archfey Warlock / Aberrant Mind Sorcerer multi-class
/// would carry both rows, but the OR-of-cohort-hits semantics means
/// either alone bounces the Charmed install.
///
/// Ships on `ARCHFEY_WARLOCK_TEMPLATE` — the Otherworldly Patron: The
/// Archfey subclass template — alongside the baseline Warlock envelope
/// (CHA-primary Pact Magic, Eldritch Blast + Hex + Witch Bolt at will,
/// Agonizing / Repelling / Eldritch Mind invocations). Distinct from
/// `FIEND_WARLOCK_TEMPLATE` (Dark One's Blessing + Dark One's Own Luck
/// + Fiendish Resilience — the fiery kill-focused build),
///   `UNDYING_WARLOCK_TEMPLATE` (Aspect of the Moon — the insomniac's
///   build), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` (Entropic Ward — the
///   alien-awareness reactive build), and the baseline
///   `WARLOCK_TEMPLATE` (patron-less baseline).
///
/// Ships on the CR-4 template above the strict RAW lv10 gate for the
/// same reason `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience
/// (RAW lv10) and `GREAT_OLD_ONE_WARLOCK_TEMPLATE` ships Entropic Ward
/// (RAW lv6): class templates target a balanced playable level, not
/// lockstep PHB progression.
pub const BEGUILING_DEFENSES_TAG: &str = "warlock.beguiling_defenses";

/// 5e Warlock Otherworldly Patron — **The Celestial** — **Radiant Soul**
/// subclass feature tag (level 6, XGtE). Passive: the warlock's
/// celestial-linked soul carries two mechanically distinct RAW clauses;
/// only the load-bearing defensive half ships on the CR-4 chassis.
///   1. **Resistance to radiant damage** — folds through the shared
///      `PASSIVE_TYPED_RESISTANCES` cohort in `actor_template.rs` next
///      to Heart of the Storm's lightning + thunder row and Psychic
///      Defenses' psychic row. Single-type slice (Radiant only) —
///      distinct from Heart of the Storm on the multi-type slice
///      shape; the RAW subclass grant is scoped to radiant alone even
///      though the sibling damage-boost clause (see clause 2) covers
///      both fire and radiant.
///   2. **+CHA-mod damage rider on radiant / fire spells** — RAW: when
///      the warlock casts a spell that deals radiant or fire damage
///      they add their CHA mod to one damage roll of that spell. This
///      is a per-cast damage-boost hook that needs a per-spell prime
///      wire; not yet on the CR-4 chassis. Left as future work — the
///      resistance clause is the load-bearing defensive half and rides
///      here alone, matching the way Heart of the Storm ships its
///      passive resistance without the eruption-on-cast clause on
///      earlier chassis iterations.
///
/// Sibling to `HEART_OF_THE_STORM_TAG` / `PSYCHIC_DEFENSES_TAG` on the
/// Otherworldly Patron / Sorcerous Origin subclass passive lane —
/// same "one feature tag drives one cohort row" declarative-table
/// pattern. Distinct on the damage axis (Radiant vs. Lightning +
/// Thunder / Psychic / Fire) and the source chassis (Warlock patron
/// vs. Sorcerer bloodline). Distinct from Fiendish Resilience (Fiend
/// Warlock lv10 fire) on the "typed resistance from an Otherworldly
/// Patron" lane — Fiend covers fire, Celestial covers radiant, so a
/// hypothetical multi-patron carrier stacks the two flag closures
/// cleanly under the "one halving per damage instance" rule since the
/// two rows never overlap on a single damage type.
///
/// Ships on `CELESTIAL_WARLOCK_TEMPLATE` — the Otherworldly Patron:
/// The Celestial subclass template — alongside the baseline Warlock
/// envelope (CHA-primary half-caster with Pact Magic, Eldritch Blast
/// + Hex + Witch Bolt at will, Agonizing / Repelling / Eldritch Mind
///   invocations). Distinct from `FIEND_WARLOCK_TEMPLATE` (Dark One's
///   Blessing + Dark One's Own Luck + Fiendish Resilience — the fiery
///   kill-focused build), `UNDYING_WARLOCK_TEMPLATE` (Aspect of the
///   Moon — the insomniac's build), `GREAT_OLD_ONE_WARLOCK_TEMPLATE`
///   (Entropic Ward — the alien-awareness reactive build),
///   `ARCHFEY_WARLOCK_TEMPLATE` (Beguiling Defenses — the Charmed-
///   bounce build), and the baseline `WARLOCK_TEMPLATE` (patron-less
///   baseline).
///
/// Ships on the CR-4 template above the strict RAW lv6 gate for the
/// same reason `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience
/// (RAW lv10), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` ships Entropic Ward
/// (RAW lv6), and `ARCHFEY_WARLOCK_TEMPLATE` ships Beguiling
/// Defenses (RAW lv10): class templates target a balanced playable
/// level, not lockstep PHB progression.
pub const RADIANT_SOUL_TAG: &str = "warlock.radiant_soul";

/// 5e Warlock Otherworldly Patron — **The Genie (Marid)** — **Elemental
/// Gift** subclass feature tag (level 6, TCE). Passive: the Marid-pact
/// warlock gains **resistance to cold damage** — the marid's water- and
/// ice-flavored patron pact leaks its elemental affinity into the
/// warlock's own resilience.
///
/// RAW's Elemental Gift picks a damage type based on the warlock's
/// chosen genie kind: **Dao** (bludgeoning), **Djinni** (thunder),
/// **Efreeti** (fire), **Marid** (cold). This tag ships the Marid
/// variant alongside the sibling `DAO_ELEMENTAL_GIFT_TAG` (Bludgeoning)
/// and `DJINNI_ELEMENTAL_GIFT_TAG` (Thunder) — three of the four RAW
/// genie kinds land on the passive typed-resistance cohort with their
/// own single-type slice row. The three never legally co-occur on a
/// single build (RAW: one genie kind per warlock), so the split-tag
/// shape is a template-drift lock rather than a stacking concern. The
/// remaining **Efreeti** (Fire) variant is a semantic duplicate of the
/// Fiendish / Draconic Resilience Fire rows and is left as future
/// work in favor of the three genie kinds whose damage axes are
/// otherwise uncovered on the passive-resistance lane.
///
/// RAW's Elemental Gift also grants a per-day 10-minute **flying speed
/// equal to walking speed** clause; the flight half needs a per-cast
/// timer / activated-buff surface not yet wired on this chassis (the
/// engine models flight through condition installs like `Flying` /
/// `InvestedInWind`, both of which need an activated spell or item
/// grant). Left as future work — the resistance clause is the load-
/// bearing defensive half and rides here alone, matching the way
/// Heart of the Storm shipped its passive resistance without the
/// eruption-on-cast clause on earlier chassis iterations and Radiant
/// Soul ships without the +CHA-mod radiant / fire damage rider.
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to Radiant Soul's radiant row, Heart of
/// the Storm's lightning + thunder row, and Psychic Defenses' psychic
/// row. Single-type slice (Cold only) — same shape as the Fiendish /
/// Draconic Resilience Fire row and the Radiant Soul Radiant row on
/// the single-type-per-patron subclass lane. Distinct from Heart of
/// the Storm on the multi-type slice shape; the RAW Marid grant is
/// scoped to cold alone.
///
/// Sibling to `RADIANT_SOUL_TAG` / `HEART_OF_THE_STORM_TAG` /
/// `PSYCHIC_DEFENSES_TAG` on the Otherworldly Patron / Sorcerous Origin
/// subclass passive lane — same "one feature tag drives one cohort
/// row" declarative-table pattern. Distinct on the damage axis (Cold
/// vs. Radiant / Lightning + Thunder / Psychic / Fire) and the source
/// chassis (Warlock Genie patron vs. Warlock Celestial patron /
/// Sorcerer Storm bloodline / Sorcerer Aberrant Mind bloodline /
/// Warlock Fiend patron).
///
/// Ships on `MARID_WARLOCK_TEMPLATE` — the Otherworldly Patron: The
/// Genie (Marid) subclass template — alongside the baseline Warlock
/// envelope (CHA-primary half-caster with Pact Magic, Eldritch Blast +
/// Hex + Witch Bolt at will, Agonizing / Repelling / Eldritch Mind
/// invocations). Distinct from `FIEND_WARLOCK_TEMPLATE` (Dark One's
/// Blessing + Dark One's Own Luck + Fiendish Resilience — the fiery
/// kill-focused build), `UNDYING_WARLOCK_TEMPLATE` (Aspect of the
/// Moon — the insomniac's build), `GREAT_OLD_ONE_WARLOCK_TEMPLATE`
/// (Entropic Ward — the alien-awareness reactive build),
/// `ARCHFEY_WARLOCK_TEMPLATE` (Beguiling Defenses — the Charmed-
/// bounce build), `CELESTIAL_WARLOCK_TEMPLATE` (Radiant Soul — the
/// radiant-resistance build), and the baseline `WARLOCK_TEMPLATE`
/// (patron-less baseline).
///
/// Ships on the CR-4 template above the strict RAW lv6 gate for the
/// same reason `CELESTIAL_WARLOCK_TEMPLATE` ships Radiant Soul (RAW
/// lv6), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` ships Entropic Ward (RAW
/// lv6), and `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience (RAW
/// lv10): class templates target a balanced playable level, not
/// lockstep PHB progression.
pub const MARID_ELEMENTAL_GIFT_TAG: &str = "warlock.marid_elemental_gift";

/// 5e Warlock Otherworldly Patron — **The Genie (Dao)** — **Elemental
/// Gift** subclass feature tag (level 6, TCE). Passive: the Dao-pact
/// warlock gains **resistance to bludgeoning damage** — the dao's earth-
/// flavored patron pact hardens the warlock's body against blunt-force
/// hits. First user of the Bludgeoning slot on the passive typed-
/// resistance lane — Cold is owned by Marid's `MARID_ELEMENTAL_GIFT_TAG`,
/// Fire by Fiendish / Draconic Resilience, Lightning + Thunder by Heart
/// of the Storm, Psychic by Psychic Defenses, and Radiant by Radiant
/// Soul, so the Bludgeoning axis was uncovered on the passive-typed-
/// resistance cohort until this row lands. Bludgeoning is one of the
/// three most-common weapon damage types in RAW (alongside Piercing /
/// Slashing) and the Dao's grant is the first passive lane pattern
/// carrier for any physical damage type — a Dao warlock halves incoming
/// morningstar / maul / greatclub / warhammer damage where a baseline
/// Warlock eats the hit clean.
///
/// RAW's Elemental Gift picks a damage type based on the warlock's
/// chosen genie kind: **Dao** (bludgeoning), **Djinni** (thunder),
/// **Efreeti** (fire), **Marid** (cold). This tag ships the Dao variant
/// alongside the pre-existing Marid variant (`MARID_ELEMENTAL_GIFT_TAG` above)
/// — the two never legally co-occur on a single build (RAW: one genie
/// kind per warlock), so the split-tag shape is a template-drift lock
/// rather than a stacking concern. Future Djinni / Efreeti variants
/// drop in as sibling tags with their own single-type slice entry in
/// `PASSIVE_TYPED_RESISTANCES` — same "one feature tag drives one
/// cohort row" pattern this tag and its Marid sibling already use.
///
/// RAW's Elemental Gift also grants a per-day 10-minute **flying speed
/// equal to walking speed** clause; the flight half needs a per-cast
/// timer / activated-buff surface not yet wired on this chassis. Left
/// as future work — the resistance clause is the load-bearing defensive
/// half and rides here alone, matching the way the Marid variant ships
/// without the flight half.
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to Marid's Cold row, Radiant Soul's radiant
/// row, Heart of the Storm's lightning + thunder row, and Psychic
/// Defenses' psychic row. Single-type slice (Bludgeoning only) — same
/// shape as the Marid Cold row and the Radiant Soul Radiant row on the
/// single-type-per-genie-kind lane.
///
/// Sibling to `MARID_ELEMENTAL_GIFT_TAG` (Marid, Cold) on the Genie patron
/// lane — same "one feature tag drives one cohort row" declarative-
/// table pattern, different damage axis. Sibling to `RADIANT_SOUL_TAG` /
/// `HEART_OF_THE_STORM_TAG` / `PSYCHIC_DEFENSES_TAG` on the Otherworldly
/// Patron / Sorcerous Origin subclass passive lane — same "one feature
/// tag drives one cohort row" declarative-table pattern.
///
/// Ships on `DAO_WARLOCK_TEMPLATE` — the Otherworldly Patron: The Genie
/// (Dao) subclass template — alongside the baseline Warlock envelope.
/// Distinct from `MARID_WARLOCK_TEMPLATE` (Marid genie kind — Cold),
/// `FIEND_WARLOCK_TEMPLATE` (Dark One's Blessing + Dark One's Own Luck +
/// Fiendish Resilience — the fiery kill-focused build),
/// `UNDYING_WARLOCK_TEMPLATE` (Aspect of the Moon — the insomniac's
/// build), `GREAT_OLD_ONE_WARLOCK_TEMPLATE` (Entropic Ward — the alien-
/// awareness reactive build), `ARCHFEY_WARLOCK_TEMPLATE` (Beguiling
/// Defenses — the Charmed-bounce build), `CELESTIAL_WARLOCK_TEMPLATE`
/// (Radiant Soul — the radiant-resistance build), and the baseline
/// `WARLOCK_TEMPLATE` (patron-less baseline).
///
/// Ships on the CR-4 template above the strict RAW lv6 gate for the
/// same reason `MARID_WARLOCK_TEMPLATE` ships Elemental Gift (RAW lv6),
/// `CELESTIAL_WARLOCK_TEMPLATE` ships Radiant Soul (RAW lv6),
/// `GREAT_OLD_ONE_WARLOCK_TEMPLATE` ships Entropic Ward (RAW lv6), and
/// `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience (RAW lv10): class
/// templates target a balanced playable level, not lockstep PHB
/// progression.
pub const DAO_ELEMENTAL_GIFT_TAG: &str = "warlock.dao_elemental_gift";

/// 5e Sorcerer Sorcerous Origin — **Storm Sorcery** — **Heart of the
/// Storm** subclass feature tag (level 6, XGtE). Passive: the sorcerer
/// gains resistance to both **lightning** AND **thunder** damage AND
/// a burst-on-cast eruption rider: when the sorcerer casts a lv1+ spell
/// that deals lightning or thunder damage, each hostile creature within
/// 10 ft takes `HEART_OF_THE_STORM_ERUPTION_DAMAGE` damage of the
/// matching type. Auto-hit, no save — RAW: "half your sorcerer level"
/// damage (flat magnitude here, see the constant docstring for the
/// level-mapping rationale).
///
/// The eruption clause is wired at
/// `EncounterInstance::trigger_heart_of_the_storm_eruption` and fires
/// from the shared `Action::execute` chokepoint — sibling post-cast
/// trigger to Wild Magic Surge on the same "after the spell" surface.
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` — the row uses the multi-type slice shape to
/// fold both damage types through one entry rather than two duplicated
/// flag closures. Sibling row to Fiendish Resilience (fire) and
/// Draconic Resilience (fire) on the passive typed-resistance lane
/// — same halving rule, different subclass source and different damage
/// axis. The two-type coverage is the first user of the cohort's
/// multi-type slice shape; single-type entries (Dwarven / Fiendish /
/// Draconic) still ride as one-element slices.
///
/// Ships on `STORM_SORCERER_TEMPLATE` — the Sorcerous Origin: Storm
/// Sorcery subclass template — alongside the baseline Sorcerer envelope
/// (CHA-primary Sorcery Points, Empowered / Quickened / Heightened /
/// Twinned / Careful / Distant / Extended / Seeking / Subtle / Transmuted
/// metamagic, Wild Magic Surge / Tides of Chaos / Bend Luck / Sorcerous
/// Restoration). Distinct from `DRACONIC_SORCERER_TEMPLATE` on the
/// resistance axis (Lightning + Thunder vs. Fire) and from the baseline
/// Wild Magic `SORCERER_TEMPLATE` in that the Storm Sorcerer trades
/// none of the shared metamagic / feature envelope for the added
/// passive — the resistance stacks on top of the baseline chassis.
///
/// Ships on the CR-4 template above the strict RAW lv6 gate for the
/// same reason `DRACONIC_SORCERER_TEMPLATE` ships Draconic Resilience
/// (RAW lv6) and `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience
/// (RAW lv10) — class templates target a balanced playable level, not
/// lockstep PHB progression.
pub const HEART_OF_THE_STORM_TAG: &str = "sorcerer.heart_of_the_storm";

/// 5e Sorcerer Storm Sorcery **Heart of the Storm** eruption damage.
/// RAW: "half your sorcerer level" damage on every eligible eruption.
/// The engine doesn't track class levels separately from XP-driven
/// `level`, and the `STORM_SORCERER_TEMPLATE` targets a CR-4 chassis
/// that represents roughly the RAW lv6 gate — half of 6 rounds to 3.
/// Kept as a named constant so the eruption trigger, the surface's
/// docstring, and any future test that pins the magnitude all read
/// from a single source of truth (same shape as
/// `UNARMORED_MOVEMENT_SPEED_BONUS` next to `UNARMORED_MOVEMENT_TAG`).
pub const HEART_OF_THE_STORM_ERUPTION_DAMAGE: u32 = 3;

/// 5e Sorcerer Sorcerous Origin — **Aberrant Mind** — **Psychic
/// Defenses** subclass feature tag (level 14, TCE). Passive: the
/// sorcerer's aberrant mind grants two mechanically visible clauses on
/// one feature:
///   1. **Resistance to psychic damage** — folds through the shared
///      `PASSIVE_TYPED_RESISTANCES` cohort in `actor_template.rs` next
///      to Heart of the Storm's lightning + thunder row. Same halving
///      rule, different subclass source and different damage axis
///      (Psychic vs. Lightning + Thunder / Fire / Poison).
///   2. **Immunity to `Charmed` AND `Frightened` install** — folds
///      through the shared `FLAG_DRIVEN_IMMUNITIES` cohort next to
///      Nature's Ward's Charmed + Frightened row. RAW's "advantage on
///      saves against being charmed or frightened" collapses to
///      immunity for the same reason Halfling Brave's advantage-vs-
///      Frightened clause does — the engine doesn't tag saves by what
///      condition they defend against, so approximating the
///      advantage-on-save clause as install-immunity is the cleanest
///      surface. Same shape as the sibling Nature's Ward (paladin
///      lv15 capstone) row on the Charmed + Frightened trio.
///
/// Sibling to `HEART_OF_THE_STORM_TAG` on the Sorcerous Origin subclass
/// passive lane — same "one feature tag drives one cohort row"
/// declarative-table pattern. Distinct on the damage axis (Psychic vs.
/// Lightning + Thunder) and the condition axis (Charmed / Frightened
/// vs. no-condition-clause). The two subclass features never legally
/// co-occur on a single build since the sorcerer picks one Sorcerous
/// Origin.
///
/// Ships on `ABERRANT_MIND_SORCERER_TEMPLATE` — the Sorcerous Origin:
/// Aberrant Mind subclass template — alongside the baseline Sorcerer
/// envelope (CHA-primary Sorcery Points, Empowered / Quickened /
/// Heightened / Twinned / Careful / Distant / Extended / Seeking /
/// Subtle / Transmuted metamagic, Wild Magic Surge / Tides of Chaos /
/// Bend Luck / Sorcerous Restoration).
///
/// Ships on the CR-4 template above the strict RAW lv14 gate for the
/// same reason `STORM_SORCERER_TEMPLATE` ships Heart of the Storm
/// (RAW lv6) and `DRACONIC_SORCERER_TEMPLATE` ships Draconic
/// Resilience (RAW lv6) on their CR-4 chassis — class templates
/// target a balanced playable level, not lockstep PHB progression.
///
/// RAW's Aberrant Mind picks up other features not shipped on this
/// template — **Telepathic Speech** (lv1: 30ft telepathy, no combat
/// surface), **Psionic Spells** (lv1: subclass-only expanded spell
/// list, currently folded into the shared sorcerer roster), **Psionic
/// Sorcery** (lv6: cast Psionic Spells for 0 material / verbal /
/// somatic + reduced SP cost — needs a per-spell prime gate not yet
/// wired), and **Warping Implosion** (lv18 capstone: teleport +
/// force burst — an SP-fueled apex burst). The passive lv14
/// resistance + immunity trio is the cleanest CR-4-appropriate
/// mechanical surface without a spend-side hook, so we ship that
/// half and leave the rest as future work.
pub const PSYCHIC_DEFENSES_TAG: &str = "sorcerer.psychic_defenses";

/// 5e Fighter Champion — **Survivor** (level 18) feature tag. Passive
/// at-start-of-turn regen: while combat-active and above 0 HP but at or
/// below half max HP, the holder regains `5 + CON modifier` HP at the
/// start of each of their turns. The capstone "I will not die" envelope —
/// composes with Second Wind (bonus-action big chunk) and Indomitable
/// (failed-save reroll) so a level-18 Champion stabilizes themselves
/// passively round-over-round without burning either per-rest charge.
///
/// Read at `ActorInstance::reset_for_new_round` next to the once-per-turn
/// flag resets so the regen lands before any condition-based start-of-turn
/// damage (Ongoing burn from Hellish Rebuke / Cloudkill) is rolled. Floor
/// at 1 HP recovered when CON is negative — a -2 CON Champion still ticks
/// up 3 HP (5 - 2). The gate routes through `is_combat_active` so a
/// downed Champion doesn't auto-resurrect — Survivor is a stabilization
/// tool, not a revival one.
pub const SURVIVOR_TAG: &str = "fighter.survivor";

/// 5e Barbarian **Relentless Rage** (level 11) feature tag. Passive
/// rest-charged save-intercept: when a killing blow would otherwise
/// drop the barbarian to 0 HP *while raging*, they make a Constitution
/// save vs DC 10 (climbs by 5 each subsequent successful use, resets to
/// 10 on short / long rest). On a pass, HP pins at 1 instead. Distinct
/// from Half-Orc Relentless Endurance (RELENTLESS_ENDURANCE_TAG) in
/// three ways:
///   1. **Gated on Raging** — non-raging hits fall through to the
///      normal Downed transition.
///   2. **Save-based, not auto-pass** — a failed save burns nothing
///      (the feature has no charge to spend; it just doesn't fire).
///   3. **DC climbs** — each successful save makes the next attempt
///      harder, so a barbarian can't lean on this indefinitely.
///
/// Tied to the `relentless_rage_dc` field on `ActorInstance` for the
/// per-rest DC progression; the save is rolled in the encounter-side
/// intercept `EncounterInstance::try_relentless_rage`, which the
/// `DealDamage::apply` path calls before the Downed transition lands.
pub const RELENTLESS_RAGE_TAG: &str = "barbarian.relentless_rage";

/// 5e Rogue **Sneak Attack** feature tag. Passive once-per-turn +Nd6
/// damage rider on any weapon attack that qualifies (advantage or
/// ally-adjacent target, no disadvantage; RAW: PHB p. 96). Used as
/// the ledger key for the shared `ONCE_PER_TURN_RIDER_TAGS` cohort on
/// `ActorInstance` — the once-per-turn gate reads / writes through
/// `actor.once_per_turn_used(SNEAK_ATTACK_TAG)` /
/// `mark_once_per_turn_used(SNEAK_ATTACK_TAG)`. Cleared at turn-start
/// by `reset_for_new_round` alongside the sibling rider tags.
///
/// The tag exists solely for the ledger — Sneak Attack is exposed as
/// an attack-time property of the rogue's weapon path, NOT as a
/// `has_passive_feature` check on the actor (the rogue's chassis
/// implicitly qualifies). The `class_attacks::sneak_attack_dice_for_level`
/// helper reads the caster's level to size the die pool.
pub const SNEAK_ATTACK_TAG: &str = "rogue.sneak_attack";

/// Ordered cohort of feature tags whose "once-per-turn used" ledger
/// lives on `ActorInstance::once_per_turn_marks`. Adding a future
/// once-per-turn attack rider (a new Battle Master maneuver's
/// per-turn window, a subclass equivalent to Colossus Slayer) lands
/// as a fresh const + one entry here rather than a fresh `bool`
/// field + accessor pair + reset call. The shared ledger is a
/// `HashSet<&'static str>` cleared once at `reset_for_new_round`.
///
/// Entries are listed for docs / self-check purposes; the ledger
/// itself doesn't consult this array at runtime — any tag can be
/// marked / read through the shared API without pre-registration.
/// Keeps the cohort discoverable in one place, mirroring the shape
/// of `SHORT_REST_FEATURES` / `LETHAL_DAMAGE_ABSORBER_FEATURES`.
pub const ONCE_PER_TURN_RIDER_TAGS: &[&str] = &[
    SNEAK_ATTACK_TAG,
    COLOSSUS_SLAYER_TAG,
    FOE_SLAYER_TAG,
    DIVINE_FURY_TAG,
    DREADFUL_STRIKES_TAG,
    PSYCHIC_BLADES_TAG,
    PLANAR_WARRIOR_TAG,
    SLAYERS_PREY_TAG,
    GATHERED_SWARM_TAG,
    PSIONIC_STRIKE_TAG,
    DEFT_STRIKE_TAG,
    GIANTS_MIGHT_RIDER_TAG,
    // The only entry here that isn't a damage rider: Ancestral
    // Protectors uses the ledger to enforce RAW's "the *first* creature
    // you hit on your turn" rather than to cap a die pool. Same
    // mechanism, different purpose — which is the argument for the
    // ledger being keyed by plain tag rather than by rider identity.
    ANCESTRAL_PROTECTORS_TAG,
    // Also not a damage rider: Form of Dread uses the ledger to enforce
    // RAW's "once on each of your turns" on its fear rider, which keeps
    // the form itself alive across the trigger where
    // `consume_on_trigger` would have ended it.
    FORM_OF_DREAD_TAG,
];

/// 5e **Colossus Slayer** — Hunter Ranger subclass feature (level 3).
/// Passive once-per-turn rider: on a weapon hit, if the target's
/// current HP is less than its maximum, the attack deals +1d8 of the
/// weapon's damage type. Stored as a `has_passive_feature` flag (no
/// per-rest charge — it's always-on but rate-limited to one trigger
/// per turn) and read at the attack-resolution chokepoint in
/// `engine::attack::resolve_attack_outcome` right after the
/// Improved Divine Smite block. The "once per turn" gate is the
/// `colossus_slayer_used` flag on the actor, cleared at turn-start
/// by `reset_for_new_round` (mirrors the rogue Sneak Attack guard).
///
/// Crits double the rider die per 5e RAW; shared `roll_rider` helper
/// handles the doubling so the rule lives in one place. The rider
/// fires on melee AND ranged weapon hits (RAW: "When you hit a
/// creature with a weapon attack") — no melee-only gate.
pub const COLOSSUS_SLAYER_TAG: &str = "ranger.colossus_slayer";

/// 5e **Multiattack Defense** — Hunter Ranger subclass feature (level
/// 7 Defensive Tactics, "Multiattack Defense" option). Passive: when a
/// creature hits the holder with an attack, the holder gains a +4
/// bonus to AC against all subsequent attacks made by that same
/// creature for the rest of the turn.
///
/// Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on but rate-limited by "already hit me this turn" state on
/// the attacker). Read at both attack chokepoints
/// (`resolve_attack_outcome` for weapon swings and `spell_attack_outcome`
/// for spell attacks) right after the initial AC read: if the
/// attacker's `has_hit_target_this_turn(target_id)` returns true AND
/// the target holds this tag, +4 is added to the target's effective
/// AC. The hit-mark is written on the first connecting swing so the
/// second swing of a multi-attack sequence (Extra Attack, Action
/// Surge, Scorching Ray beam 2 / 3) is the earliest one to eat the
/// penalty — exactly RAW.
///
/// Composes cleanly with cover (+2 / +5 from intervening creatures)
/// and the target's own template AC — all three lanes sum into a
/// single effective AC read once per attack.
pub const MULTIATTACK_DEFENSE_TAG: &str = "ranger.multiattack_defense";

/// 5e Ranger **Foe Slayer** (level 20 capstone). Passive once-per-turn
/// weapon-hit rider: on any connecting weapon attack, the ranger adds
/// their Wisdom modifier as flat bonus damage of the weapon's damage
/// type. RAW: "Once on each of your turns, you can add your Wisdom
/// modifier to the attack roll or the damage roll of an attack you
/// make." — we collapse to the damage-roll lane (the load-bearing
/// pick; the attack-roll lane is already covered by Colossus Slayer's
/// die-based rider plus advantage sources).
///
/// Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on but rate-limited by the once-per-turn `foe_slayer_used`
/// ledger on the actor, sibling to `sneak_attack_used` and
/// `colossus_slayer_used`). Read at the attack-resolution chokepoint
/// in `engine::attack::resolve_attack_outcome` right after the Colossus
/// Slayer block so both riders can fire on the same hit (RAW: Foe
/// Slayer is a separate feature, not a Hunter subclass rider — a
/// Hunter ranger with both Colossus Slayer AND Foe Slayer stacks the
/// two once-per-turn +damage lanes on the opening shot).
///
/// Ships on the CR-1 baseline RANGER_TEMPLATE and the CR-1 Hunter
/// Ranger subclass template above their strict RAW level gate for the
/// same reason Colossus Slayer / Multiattack Defense / Superior
/// Hunter's Defense do — class templates target a balanced playable
/// level, not lockstep PHB progression. Fires on melee AND ranged
/// weapon hits (RAW: "an attack you make" — no melee-only gate);
/// crits don't double the flat mod (RAW: the crit-doubling rule
/// applies to damage dice, not flat modifiers).
pub const FOE_SLAYER_TAG: &str = "ranger.foe_slayer";

/// 5e Ranger **Fey Wanderer** subclass — **Dreadful Strikes** (level 3,
/// TCE). Passive once-per-turn weapon-hit rider: on any weapon hit, lay
/// +1d4 Psychic damage on the target. RAW gates on "a creature that
/// isn't already affected by your Dreadful Strikes this turn" — we
/// collapse the per-target gate to the once-per-turn ledger (any
/// target), matching the shape Colossus Slayer / Foe Slayer / Divine
/// Fury already use on the shared `ONCE_PER_TURN_RIDER_TAGS` cohort.
/// The collapse trades away the RAW ability to fire the rider a second
/// time this turn against a distinct target — a small approximation
/// since the typical two-swing Extra Attack chain lands both hits on
/// the same primary target where RAW would also collapse to a single
/// fire — in exchange for slotting into the existing once-per-turn
/// ledger without a per-target hit-map surface.
///
/// Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on but rate-limited to one trigger per turn) and read at the
/// attack-resolution chokepoint in `engine::attack::resolve_attack_outcome`
/// right after the Colossus Slayer block. The "once per turn" gate is
/// the `once_per_turn_used(DREADFUL_STRIKES_TAG)` ledger on the actor,
/// cleared at turn-start by `reset_for_new_round` (mirrors Colossus
/// Slayer / Foe Slayer / Divine Fury on the same ledger).
///
/// Crits double the rider die per 5e RAW; shared `roll_rider` helper
/// handles the doubling so the rule lives in one place. The rider
/// fires on melee AND ranged weapon hits (RAW: "When you hit a
/// creature with a weapon attack") — no melee-only gate. Damage is
/// always typed Psychic — RAW's "your association with the Feywild
/// has infused your weapon strikes with dread" fixes the type at
/// Psychic regardless of the weapon's base type, distinct from
/// Colossus Slayer's `weapon-typed` die which mirrors the weapon.
///
/// RAW's Dreadful Strikes die scales from 1d4 → 1d6 at ranger level
/// 11. We ship the lv3 1d4 base die on the FEY_WANDERER_RANGER_TEMPLATE
/// — class templates target a balanced playable level, not lockstep
/// PHB progression; the lv11 die bump is left as future work behind a
/// per-caster level gate (same shape Divine Fury's `+ level / 2` flat
/// mod already uses on the Zealot chassis).
///
/// Sibling on the shared `ONCE_PER_TURN_RIDER_TAGS` cohort to
/// COLOSSUS_SLAYER_TAG (Hunter Ranger lv3 — +1d8 weapon-typed with a
/// wounded-target gate), FOE_SLAYER_TAG (Ranger lv20 capstone — flat
/// +WIS-mod on any weapon hit), DIVINE_FURY_TAG (Zealot Barbarian lv3
/// — +1d6 + level/2 Radiant while raging), and SNEAK_ATTACK_TAG (Rogue
/// once-per-turn +Nd6 with the qualifying-attack gate).
pub const DREADFUL_STRIKES_TAG: &str = "ranger.dreadful_strikes";

/// 5e Bard **College of Whispers** subclass — **Psychic Blades** (level 3,
/// XGtE). Passive once-per-turn weapon-hit rider: on any weapon hit, lay
/// +1d6 Psychic damage on the target. RAW's Psychic Blades RAW-strictly
/// spends a Bardic Inspiration die per activation and scales the die pool
/// with bard level (2d6 at lv3 → 3d6 lv5 → 5d6 lv10 → 8d6 lv15); we
/// collapse both the RAW BI-die cost AND the level-scaled die pool down
/// to a plain "once-per-turn +1d6 Psychic" rider on the same shared
/// `ONCE_PER_TURN_RIDER_TAGS` ledger as Colossus Slayer / Dreadful
/// Strikes so the feature slots into the existing rider chokepoint
/// without either an inspiration-die burn accounting surface or a per-
/// caster level die-pool scaler. Matches the same "one-die-typed +
/// no-target-gate + no-per-rest-charge" corner Dreadful Strikes already
/// sits at — the two features converge on the same shape from different
/// class chassis (Ranger's Fey Wanderer conclave lv3 psychic rider vs.
/// Bard's Whispers college lv3 psychic rider).
///
/// Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on but rate-limited to one trigger per turn) and read at the
/// attack-resolution chokepoint in `engine::attack::resolve_attack_outcome`
/// right after the Dreadful Strikes block. The "once per turn" gate is
/// the `once_per_turn_used(PSYCHIC_BLADES_TAG)` ledger on the actor,
/// cleared at turn-start by `reset_for_new_round` (mirrors Colossus
/// Slayer / Dreadful Strikes / Foe Slayer / Divine Fury on the same
/// ledger).
///
/// Crits double the rider die per 5e RAW; shared `roll_rider` helper
/// handles the doubling so the rule lives in one place. The rider fires
/// on melee AND ranged weapon hits (RAW: "When you hit a creature with a
/// weapon attack") — no melee-only gate. Damage is always typed Psychic
/// — RAW's "your whispers infuse your weapon strikes" fixes the type at
/// Psychic regardless of the weapon's base type, sibling to Dreadful
/// Strikes' Psychic-fixed damage and distinct from Colossus Slayer's
/// `weapon-typed` die which mirrors the weapon.
///
/// Sibling on the shared `ONCE_PER_TURN_RIDER_TAGS` cohort to
/// COLOSSUS_SLAYER_TAG (Hunter Ranger lv3 — +1d8 weapon-typed with a
/// wounded-target gate), DREADFUL_STRIKES_TAG (Fey Wanderer Ranger lv3
/// — +1d4 Psychic on any weapon hit), FOE_SLAYER_TAG (Ranger lv20
/// capstone — flat +WIS-mod on any weapon hit), DIVINE_FURY_TAG (Zealot
/// Barbarian lv3 — +1d6 + level/2 Radiant while raging), and
/// SNEAK_ATTACK_TAG (Rogue once-per-turn +Nd6 with the qualifying-attack
/// gate). Psychic Blades lands mid-die-size between Dreadful Strikes
/// (1d4) and Colossus Slayer (1d8), matching its RAW-level-3 anchor
/// where the equivalent Fey Wanderer / Hunter riders also unlock.
pub const PSYCHIC_BLADES_TAG: &str = "bard.psychic_blades";

/// 5e Ranger **Horizon Walker** subclass — **Planar Warrior** (level 3,
/// XGtE). Passive once-per-turn weapon-hit rider: on any weapon hit, lay
/// +1d8 Force damage on the target. RAW-strict Planar Warrior costs a
/// bonus action to mark a specific creature within 30 ft; the next weapon
/// hit against that marked target *this turn* deals the +1d8 Force. We
/// collapse both the bonus-action-mark and the per-target gate down to a
/// plain "once-per-turn +1d8 Force" rider on the shared
/// `ONCE_PER_TURN_RIDER_TAGS` ledger — matching the same collapse
/// Dreadful Strikes / Psychic Blades apply to their own RAW per-target
/// gates (both trade the per-target lookup for slotting cleanly into the
/// once-per-turn ledger). Trades away the RAW "mark first, hit second"
/// two-step for slotting into the existing rider chokepoint without a
/// separate bonus-action-mark action surface plus a per-target-mark
/// ledger.
///
/// Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on but rate-limited to one trigger per turn) and read at the
/// attack-resolution chokepoint in `engine::attack::resolve_attack_outcome`
/// alongside the sibling once-per-turn weapon-die riders. The "once per
/// turn" gate is the `once_per_turn_used(PLANAR_WARRIOR_TAG)` ledger on
/// the actor, cleared at turn-start by `reset_for_new_round` (mirrors
/// Colossus Slayer / Dreadful Strikes / Psychic Blades / Foe Slayer /
/// Divine Fury on the same ledger).
///
/// Crits double the rider die per 5e RAW; shared `roll_rider` helper
/// handles the doubling so the rule lives in one place. The rider fires
/// on melee AND ranged weapon hits (RAW: "when you hit the target with
/// a weapon attack") — no melee-only gate. Damage is always typed Force
/// — RAW's "shifts partially into the Ethereal Plane to deal force
/// damage" fixes the type regardless of the weapon's base type, sibling
/// to Dreadful Strikes' / Psychic Blades' Psychic-fixed damage and
/// distinct from Colossus Slayer's `weapon-typed` die which mirrors the
/// weapon. Force is one of the rarest-resisted damage types in the
/// engine (essentially no monster carries Force resistance), so the
/// Horizon Walker's rider punches through nearly every typed-defense
/// lane cleanly — the RAW "planar warrior slips past mundane armor"
/// tell.
///
/// RAW's Planar Warrior die scales from 1d8 → 2d8 at ranger level 11.
/// We ship the lv3 1d8 base die on the HORIZON_WALKER_RANGER_TEMPLATE —
/// class templates target a balanced playable level, not lockstep PHB
/// progression; the lv11 die bump is left as future work behind a per-
/// caster level gate (same shape Divine Fury's `+ level / 2` flat mod
/// already uses on the Zealot chassis).
///
/// Sibling on the shared `ONCE_PER_TURN_RIDER_TAGS` cohort to
/// COLOSSUS_SLAYER_TAG (Hunter Ranger lv3 — +1d8 weapon-typed with a
/// wounded-target gate), DREADFUL_STRIKES_TAG (Fey Wanderer Ranger lv3 —
/// +1d4 Psychic on any weapon hit), PSYCHIC_BLADES_TAG (Whispers Bard
/// lv3 — +1d6 Psychic on any weapon hit), FOE_SLAYER_TAG (Ranger lv20
/// capstone — flat +WIS-mod on any weapon hit), DIVINE_FURY_TAG (Zealot
/// Barbarian lv3 — +1d6 + level/2 Radiant while raging), and
/// SNEAK_ATTACK_TAG (Rogue once-per-turn +Nd6 with the qualifying-attack
/// gate). Planar Warrior lands at the same die size as Colossus Slayer
/// (1d8) — the two lv3 subclass riders converge on the same magnitude
/// from different angles (Hunter's wounded-target gate + weapon-typed
/// vs. Horizon Walker's no-gate + Force-typed).
pub const PLANAR_WARRIOR_TAG: &str = "ranger.planar_warrior";

/// 5e Ranger **Swarmkeeper** subclass — **Gathered Swarm** (level 3,
/// TCE). Passive once-per-turn weapon-hit rider: on any weapon hit, the
/// swarm of nature spirits bound to the ranger lays +1d6 piercing damage
/// on the target. RAW's Gathered Swarm offers three per-hit riders — the
/// +1d6 piercing spirit-swarm bite, an STR-save forced 15-ft move on the
/// target, or a 5-ft move on the swarmkeeper themselves — and we collapse
/// the choice down to the load-bearing damage-rider lane so the feature
/// slots into the shared `ONCE_PER_TURN_WEAPON_DIE_RIDERS` cohort. The
/// two RAW alternatives (STR-save shove and self-teleport) would need a
/// per-hit optional-side-effect surface plus AI heuristics for when to
/// take the movement over the damage — the collapse trades those two
/// alternative riders away in exchange for slotting cleanly into the
/// existing rider chokepoint.
///
/// Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on but rate-limited to one trigger per turn) and read at the
/// attack-resolution chokepoint in `engine::attack::resolve_attack_outcome`
/// via the shared `ONCE_PER_TURN_WEAPON_DIE_RIDERS` cohort alongside
/// the sibling once-per-turn weapon-die riders. The "once per turn"
/// gate is the `once_per_turn_used(GATHERED_SWARM_TAG)` ledger on the
/// actor, cleared at turn-start by `reset_for_new_round` (mirrors
/// Colossus Slayer / Dreadful Strikes / Psychic Blades / Planar Warrior
/// / Slayer's Prey / Foe Slayer / Divine Fury on the same ledger).
///
/// Crits double the rider die per 5e RAW; shared `roll_rider` helper
/// handles the doubling so the rule lives in one place. The rider fires
/// on melee AND ranged weapon hits (RAW: "when you hit a creature with
/// an attack") — no melee-only gate. Damage is always typed Piercing
/// — RAW's "the swarm attacks it with claws, teeth, or stingers" fixes
/// the type at piercing regardless of the weapon's base type, distinct
/// from Colossus Slayer's / Slayer's Prey's weapon-typed damage and
/// sibling to Dreadful Strikes' / Psychic Blades' Psychic-fixed damage
/// and Planar Warrior's Force-fixed damage. Piercing is a physical
/// damage type that a handful of skeleton-like undead resist per RAW —
/// the swarm's bite is chipped down on that lane, distinct from the
/// unshaved Force lane the Horizon Walker's Planar Warrior enjoys.
///
/// Sibling on the shared `ONCE_PER_TURN_RIDER_TAGS` cohort to
/// COLOSSUS_SLAYER_TAG (Hunter Ranger lv3 — +1d8 weapon-typed with a
/// wounded-target gate), DREADFUL_STRIKES_TAG (Fey Wanderer Ranger lv3
/// — +1d4 Psychic on any weapon hit), PSYCHIC_BLADES_TAG (Whispers
/// Bard lv3 — +1d6 Psychic on any weapon hit), PLANAR_WARRIOR_TAG
/// (Horizon Walker Ranger lv3 — +1d8 Force on any weapon hit),
/// SLAYERS_PREY_TAG (Monster Slayer Ranger lv3 — +1d6 weapon-typed on
/// any weapon hit), FOE_SLAYER_TAG (Ranger lv20 capstone — flat
/// +WIS-mod on any weapon hit), DIVINE_FURY_TAG (Zealot Barbarian lv3
/// — +1d6 + level/2 Radiant while raging), and SNEAK_ATTACK_TAG (Rogue
/// once-per-turn +Nd6 with the qualifying-attack gate). Gathered Swarm
/// lands at the same die size as Psychic Blades / Slayer's Prey (1d6)
/// — three lv3 subclass riders converge on the same magnitude from
/// different angles (Whispers' Psychic-fixed, Monster Slayer's weapon-
/// typed, Swarmkeeper's Piercing-fixed).
pub const GATHERED_SWARM_TAG: &str = "ranger.gathered_swarm";

/// 5e Fighter **Psi Warrior** subclass — **Psionic Strike** (level 3,
/// TCE). Passive once-per-turn weapon-hit rider: on any weapon hit
/// against a target within 30 ft, lay +1d8 Force damage on the swing.
///
/// RAW spends one Psionic Energy die from the subclass pool and adds the
/// holder's INT modifier to the rolled die. We ship the die alone, with
/// neither the pool cost nor the flat INT bump, for two reasons that
/// pull the same way. The engine's per-rest charge lane is binary — one
/// `features_remaining` entry per tag, not a counter — so a
/// pool-accurate Psionic Strike would fire once per short rest rather
/// than the four-plus times RAW allows, which is a worse approximation
/// than "free but once per turn". And the pool's other consumer here,
/// `PROTECTIVE_FIELD_TAG`, is the feature whose whole character is
/// deciding *when* to spend; letting the offensive rider compete for the
/// same single charge would mean the defensive half essentially never
/// fires. Splitting them — strike free, field charged — keeps both
/// features legible at the cost of RAW's shared-resource tension. The
/// flat +INT is dropped alongside the pool cost because the shared
/// `ONCE_PER_TURN_WEAPON_DIE_RIDERS` cohort is the "one die, one damage
/// type, one target gate" corner of the rider space and has no flat-
/// bonus column; a +2 that would need one is not worth widening every
/// row for. Same "collapse to the load-bearing lane" call the sibling
/// Gathered Swarm / Slayer's Prey / Planar Warrior riders make on their
/// own RAW riders.
///
/// The RAW 30 ft range gate is also dropped: every weapon swing this
/// rider can attach to is already inside its own weapon's reach, and no
/// weapon in the engine outranges 30 ft by enough for the gate to bite
/// on a melee build. Force damage is the rarest-resisted type in the
/// engine — the same lane Planar Warrior sits on, and the reason both
/// features read as "your hits just land harder, against anything".
///
/// Stored as a `has_passive_feature` flag and read at the attack-
/// resolution chokepoint in `engine::attack::resolve_attack_outcome`
/// via the shared `ONCE_PER_TURN_WEAPON_DIE_RIDERS` cohort. The "once
/// per turn" gate is the `once_per_turn_used(PSIONIC_STRIKE_TAG)`
/// ledger, cleared at turn-start by `reset_for_new_round`. Crits double
/// the die via the shared `roll_rider` helper. Fires on melee AND
/// ranged weapon hits — RAW's "when you hit a creature with a weapon
/// attack" has no melee-only gate.
///
/// Sibling in die size and damage type to PLANAR_WARRIOR_TAG (Horizon
/// Walker Ranger lv3 — +1d8 Force); the two are mechanically identical
/// riders reached from opposite class chassis, which is why the Psi
/// Warrior's identity rests on `PROTECTIVE_FIELD_TAG` rather than here.
pub const PSIONIC_STRIKE_TAG: &str = "fighter.psionic_strike";

/// 5e Fighter **Psi Warrior** subclass — **Protective Field** (level 3,
/// TCE). Reactive damage clamp: when the holder *or* a creature they can
/// see within 30 ft takes damage, the holder may spend their reaction
/// and one Psionic Energy die to reduce that damage by `1d8 + INT
/// modifier`.
///
/// Ships as a row on the shared `REACTIVE_DAMAGE_CLAMPS` cohort in
/// `engine::attack` with `ClampScope::HolderOrAlly(12)` (12 tiles = 30
/// ft on the 2.5 ft grid) — the first row to use that scope, and the
/// reason the scope exists. The row carries this tag, so firing burns
/// the reaction *and* a `feature_available` charge; the tag is
/// registered on `SHORT_REST_FEATURES` so the Psionic Energy pool
/// re-arms between engagements. RAW's pool of four-plus dice collapses
/// to one charge per short rest — the same collapse the Cleric Channel
/// Divinity charges and Warding Flare's WIS-mod uses take.
///
/// The scope is what makes this feature distinct from every clamp
/// already in the engine. Uncanny Dodge, Deflect Missiles and Parry are
/// self-only; Interception is ally-only and adjacent. Protective Field
/// is the only clamp whose holder chooses between shielding themselves
/// and shielding someone across the battlefield, which is also why it
/// pairs with a charge rather than being always-on: an always-on clamp
/// with a 30 ft ally reach would strictly dominate both Fighting Styles.
///
/// Because the cohort walk visits `Holder`-scoped rows before this one,
/// a Psi Warrior who also carries Parry (as the fighter chassis does)
/// spends the parry charge first on a melee hit and reaches for the
/// field only when parry is already gone — which is the ordering a
/// player would pick anyway, the cheaper die first.
pub const PROTECTIVE_FIELD_TAG: &str = "fighter.protective_field";

/// 5e Fighter **Cavalier** subclass — **Unwavering Mark** (level 3,
/// XGtE). Passive weapon-hit mark: when the cavalier hits with a melee
/// weapon attack, the target is marked until the end of the cavalier's
/// next turn, and while marked it has disadvantage on any attack roll
/// that doesn't target the cavalier.
///
/// Ships as a row on the shared `ON_HIT_CONDITION_MARKS` cohort in
/// `engine::attack`, stamping `Condition::Dueled` with `Rounds(2)` and
/// the `Dueled` back-link. That condition already exists — it's what
/// the Compelled Duel spell installs, and its "disadvantage on attacks
/// that don't target the anchor" clause is Unwavering Mark's RAW clause
/// verbatim. So the Cavalier's headline feature *is* a Compelled Duel
/// that costs no action, no slot and no concentration, and instead of a
/// WIS save asks only that the cavalier land a hit. Reusing the
/// condition rather than minting a parallel one keeps the engine at one
/// implementation of the rule and means the two sources compose: a
/// paladin/cavalier who casts the spell and then swings just refreshes
/// the same lock.
///
/// The `Rounds(2)` timer, not `UntilStartOfNextTurn`, for the same
/// reason Eldritch Strike uses it: RAW's window closes at the end of
/// the *marker's* next turn, and an until-start-of-next-turn timer
/// decays on the marked creature's clock instead.
///
/// RAW's second clause — the marked creature damaging someone other
/// than the cavalier lets the cavalier make a special bonus-action
/// melee attack on their next turn — is not modeled. It needs a
/// per-mark "the mark was violated" ledger written at the damage site
/// and read at the cavalier's next turn start, plus a conditional
/// bonus-action grant; the disadvantage clause is the half that makes
/// the mark worth applying, and it lands at a chokepoint the engine
/// already has.
/// 5e Cleric **Arcana Domain** subclass — **Arcane Abjuration** (Channel
/// Divinity, level 2, SCAG). Action: every celestial, elemental, fey or
/// fiend within 30 ft rolls a WIS save vs the cleric's WIS-anchored spell
/// DC and is Frightened for 10 rounds on a fail. Once per short rest;
/// refreshes via `SHORT_REST_FEATURES`.
///
/// Ships as a `TurnBurst` config alongside Turn Undead, Turn the
/// Faithless and Dreadful Aspect — same 30 ft WIS-save-or-Frighten burst,
/// differing only in the creature-type filter and the DC anchor.
///
/// The type filter is the exact complement of Turn Undead's, which is the
/// domain's whole argument: an Arcana Cleric standing next to any other
/// cleric covers every extraplanar creature type in the engine between
/// them. Wider than Turn the Faithless's fey / fiend narrowing because
/// there's no ally-flavor reason to spare celestials here — the Arcana
/// Cleric abjures outsiders, not evil ones.
///
/// RAW's second clause — a target whose CR is at or below a
/// level-scaling threshold is banished to its home plane instead of
/// merely frightened — is not modeled. Banishment needs an off-board
/// actor lane the engine doesn't have, and it's also the clause that
/// would make this strictly better than Turn Undead rather than
/// sideways from it.
/// 5e Cleric **Nature Domain** subclass — **Dampen Elements** (level 6).
/// Reactive damage clamp: when the cleric or a creature within 30 ft of
/// them takes acid, cold, fire, lightning or thunder damage, the cleric
/// may spend their reaction to grant that creature resistance to it. Once
/// per short rest; refreshes via `SHORT_REST_FEATURES`.
///
/// Ships as a row on the shared `REACTIVE_DAMAGE_CLAMPS` cohort with
/// `ClampScope::HolderOrAlly(12)` and `ClampFormula::Halve` — and it is
/// the row that put the `damage_types` filter column on that cohort.
/// Every clamp before it keyed off *how* the damage arrived (a melee
/// attack, a ranged weapon attack, any attack); this is the first that
/// keys off *what* the damage is.
///
/// Which makes it the sharpest clamp in the engine and the narrowest at
/// once. Against a fire-breathing dragon or a lightning-heavy caster it
/// halves the biggest number on the table; against a room full of
/// scimitars it never fires at all. Compare Warding Maneuver, the other
/// `Halve` row: same formula, adjacency instead of 30 ft, no type gate,
/// so it answers everything at shorter range. The Nature Cleric is the
/// build you want when you know what's coming.
///
/// RAW's uses are unlimited (it costs only the reaction); the engine's
/// once-per-short-rest charge is the standard collapse, and here it is
/// also a balance choice — an unlimited 30 ft elemental halve would make
/// every elemental encounter in the engine a non-event.
/// 5e Cleric **Nature Domain** subclass — **Charm Animals and Plants**
/// (Channel Divinity, level 2, PHB). Action: every beast and plant within
/// 30 ft rolls a WIS save vs the cleric's WIS-anchored spell DC and is
/// Charmed by the cleric for 10 rounds on a fail. Once per short rest;
/// refreshes via `SHORT_REST_FEATURES`.
///
/// Ships as a `TurnBurst` config, and it is the config that generalized
/// that struct past Frightened. The resolver installs the condition
/// through `install_condition_with_link`, so Charmed's back-link
/// back-link — the anchor for the "can't attack your charmer" gate —
/// comes along without the burst code knowing which conditions carry one.
///
/// Charmed is stronger than the Frightened its three siblings install: a
/// charmed creature can't attack the cleric or target them with harmful
/// effects at all, where a frightened one only rolls at disadvantage. The
/// type filter is correspondingly the narrowest in the cohort — against a
/// druid's summons or a pack of wolves this ends the fight, and against
/// anything humanoid it does nothing.
///
/// RAW's "ends early if the creature takes any damage" clause is not
/// modeled: `Charmed` has no damage-clears-it rule and adding one would
/// change every charm source in the engine. The party can therefore beat
/// on a charmed bear in a way RAW forbids, which makes the narrow type
/// filter load-bearing rather than incidental.
pub const CHARM_ANIMALS_AND_PLANTS_TAG: &str = "cleric.charm_animals_and_plants";

pub const DAMPEN_ELEMENTS_TAG: &str = "cleric.dampen_elements";

pub const ARCANE_ABJURATION_TAG: &str = "cleric.arcane_abjuration";

pub const UNWAVERING_MARK_TAG: &str = "fighter.unwavering_mark";

/// 5e Fighter **Cavalier** subclass — **Warding Maneuver** (level 7,
/// XGtE). Reactive damage clamp: when the cavalier or a creature within
/// 5 ft of them is hit by an attack, the cavalier may spend their
/// reaction to roll 1d8 and add it to the target's AC against that
/// attack; if the attack still hits, the target gains resistance to its
/// damage.
///
/// Ships as a row on the shared `REACTIVE_DAMAGE_CLAMPS` cohort with
/// `ClampScope::HolderOrAlly(0)` and `ClampFormula::Halve`, gated on one
/// charge per short rest.
///
/// The AC half of RAW is collapsed into the resistance half. The clamp
/// cohort runs *after* a hit is confirmed and its damage rolled, so a
/// retroactive AC bump has nowhere to attach — the engine would need a
/// second reaction hook at attack-roll time, which is where the
/// reactive-*disadvantage* cohort lives and which can't express "+1d8 to
/// AC" either. Between the two RAW outcomes (the attack misses outright,
/// or the target resists it) always-halve sits in the middle: strictly
/// weaker than the good case, strictly better than the bad one.
///
/// The charge, and the `Halve` formula, are what keep this from
/// duplicating Fighting Style: Interception, the engine's other adjacent-
/// ally clamp. Interception is free and rolls `1d10 + proficiency` —
/// a flat ~7 that shines against small hits; Warding Maneuver is
/// rationed and proportional, so it wants to be spent on the biggest
/// swing of the fight. RAW's uses-equal-to-CON-modifier collapse to one
/// charge, the same collapse the Channel Divinity family takes.
///
/// RAW ranks this at subclass level 7; it ships on the CR-3 Cavalier
/// template for the same reason the Champion's Survivor (lv18) rides
/// there — class templates target a balanced playable level.
pub const WARDING_MANEUVER_TAG: &str = "fighter.warding_maneuver";

/// 5e Ranger **Monster Slayer** subclass — **Slayer's Prey** (level 3,
/// XGtE). Passive once-per-turn weapon-hit rider: on any weapon hit, lay
/// +1d6 damage of the weapon's damage type on the target. RAW-strict
/// Slayer's Prey costs a bonus action to mark a specific creature; the
/// first weapon hit against the marked target *this turn* deals the
/// +1d6. We collapse both the bonus-action-mark and the per-target gate
/// down to a plain "once-per-turn +1d6 weapon-typed on any target"
/// rider on the shared `ONCE_PER_TURN_RIDER_TAGS` ledger — matching the
/// same collapse Planar Warrior / Dreadful Strikes / Psychic Blades
/// apply to their own RAW per-target gates (all four trade the per-
/// target lookup for slotting cleanly into the once-per-turn ledger).
/// Trades away the RAW "mark first, hit second" two-step for slotting
/// into the existing rider chokepoint without a separate bonus-action-
/// mark action surface plus a per-target-mark ledger.
///
/// Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on but rate-limited to one trigger per turn) and read at the
/// attack-resolution chokepoint in `engine::attack::resolve_attack_outcome`
/// via the shared `ONCE_PER_TURN_WEAPON_DIE_RIDERS` cohort alongside
/// the sibling once-per-turn weapon-die riders. The "once per turn"
/// gate is the `once_per_turn_used(SLAYERS_PREY_TAG)` ledger on the
/// actor, cleared at turn-start by `reset_for_new_round` (mirrors
/// Colossus Slayer / Dreadful Strikes / Psychic Blades / Planar
/// Warrior / Foe Slayer / Divine Fury on the same ledger).
///
/// Crits double the rider die per 5e RAW; shared `roll_rider` helper
/// handles the doubling so the rule lives in one place. The rider
/// fires on melee AND ranged weapon hits (RAW: "the first time you hit
/// that target with a weapon attack") — no melee-only gate. Damage
/// type matches the weapon (via `p.damage_type`) so a fire-imbued bow
/// shot still reads as fire on the Slayer's Prey log line — sibling to
/// Colossus Slayer's weapon-typed damage and distinct from Dreadful
/// Strikes' / Psychic Blades' Psychic-fixed damage or Planar Warrior's
/// Force-fixed damage.
///
/// Sibling on the shared `ONCE_PER_TURN_RIDER_TAGS` cohort to
/// COLOSSUS_SLAYER_TAG (Hunter Ranger lv3 — +1d8 weapon-typed with a
/// wounded-target gate), DREADFUL_STRIKES_TAG (Fey Wanderer Ranger lv3
/// — +1d4 Psychic on any weapon hit), PSYCHIC_BLADES_TAG (Whispers
/// Bard lv3 — +1d6 Psychic on any weapon hit), PLANAR_WARRIOR_TAG
/// (Horizon Walker Ranger lv3 — +1d8 Force on any weapon hit),
/// FOE_SLAYER_TAG (Ranger lv20 capstone — flat +WIS-mod on any weapon
/// hit), DIVINE_FURY_TAG (Zealot Barbarian lv3 — +1d6 + level/2
/// Radiant while raging), and SNEAK_ATTACK_TAG (Rogue once-per-turn
/// +Nd6 with the qualifying-attack gate). Slayer's Prey lands at the
/// same die size as Psychic Blades (1d6) — the two lv3 subclass riders
/// converge on the same magnitude from different angles (Whispers'
/// Psychic-fixed damage vs. Monster Slayer's weapon-typed damage).
pub const SLAYERS_PREY_TAG: &str = "ranger.slayers_prey";

/// 5e Paladin **Improved Divine Smite** (level 11). Passive feature: every
/// melee weapon hit lays +1d8 radiant damage on the target — the paladin's
/// signature mid-tier damage spike, independent of the Divine Smite slot
/// burn. Stored as a `has_passive_feature` flag (no per-rest charge — it's
/// always-on). Read at the attack-resolution chokepoint in `engine::attack`
/// right after the `ON_HIT_RIDERS` loop so the rider stacks cleanly with
/// any active Smite prime (Divine Smite / Searing / Wrathful etc.) on the
/// same swing — the 1d8 fires whether or not a Smite is up.
///
/// Crits double the rider die per 5e RAW; shared `roll_rider` helper
/// handles the doubling so the rule lives in one place. Melee-only — RAW
/// Improved Divine Smite gates on "melee weapon attack" so a ranged shot
/// from a paladin without a thrown weapon doesn't pick up the rider.
pub const IMPROVED_DIVINE_SMITE_TAG: &str = "paladin.improved_divine_smite";

/// Class-feature tag for Paladin's Lay on Hands — once per long rest.
/// We collapse 5e's "pool of HP equal to 5 × level" healing well into a
/// single chunky use per rest so the once-per-rest gating pattern stays
/// uniform with the rest of the codebase (Second Wind, Action Surge,
/// Indomitable, Rage). The flat heal value is bigger than Cure Wounds
/// to compensate for the loss of pool flexibility.
pub const LAY_ON_HANDS_TAG: &str = "paladin.lay_on_hands";

/// Lay on Hands — paladin feature, touch range. Spend the once-per-rest
/// feature to heal an ally (or self) for `5 × level + CHA` HP. RAW's
/// pool mechanic lets the paladin split the heal across many casts; we
/// collapse to a single big chunk per rest so the feature follows the
/// same once-per-rest pattern as Second Wind. Plenty of healing for a
/// melee class that doesn't have spammable Cure Wounds slots.
pub struct LayOnHands {}

impl Action for LayOnHands {
    fn name(&self) -> &str {
        "lay on hands"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["loh", "hands"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — same tile as the target, footprint-adjacent.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
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
        feature_ready(encounter, caster_id, LAY_ON_HANDS_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        use crate::engine::types::AbilityScoreType;
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let level = actor.level();
        let cha_mod = actor.ability_modifier(AbilityScoreType::Charisma);
        // 5 HP per paladin level + CHA modifier. At level 3 with CHA 16
        // (+3), that's 18 HP — beats Cure Wounds at 1d8+3 (avg 7) and
        // makes the once-per-rest gate worth the slot.
        let amount = (5 * level as i32 + cha_mod).max(1) as u32;
        if let Some(paladin) = encounter.actors.get_mut(&caster_id) {
            paladin.spend_feature(LAY_ON_HANDS_TAG);
        }
        encounter.log(format!(
            "  lay on hands: 5*{}{:+} = {} HP",
            level, cha_mod, amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static LAY_ON_HANDS: LazyLock<LayOnHands> = LazyLock::new(|| LayOnHands {});

/// Class-feature tag for the Paladin's Cleansing Touch (Oath capstone,
/// once per long rest in our model — RAW: CHA-mod uses per long rest;
/// we collapse to a single charge so the gating stays uniform with
/// Lay on Hands / Second Wind / Action Surge).
pub const CLEANSING_TOUCH_TAG: &str = "paladin.cleansing_touch";

/// Cleansing Touch — Paladin action, touch range. Spend the once-per-rest
/// feature to end one spell affecting a willing creature (or self). The
/// load-bearing late-game paladin tool: removes a heavyweight debuff
/// (Hold Person / Charm / Fear / Confusion) from an ally without burning
/// a level-5 Greater Restoration slot, or drops a concentrating
/// enemy's spell entirely without paying the level-3 Dispel Magic tax.
///
/// Three-tier dispel logic lives inside `CleansingTouchOn`:
///   1. Drop target's concentration.
///   2. Strip one canonical spell-installed debuff.
///   3. Fallback: strip one beneficial buff (Dispel Magic shape).
///
/// Custom-validates that the feature is available *and* there's something
/// on the target worth cleansing so a misclick doesn't burn the once-per-
/// rest charge on a clean target.
pub struct CleansingTouch {}

impl Action for CleansingTouch {
    fn name(&self) -> &str {
        "cleansing touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ct", "cleanse"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        // Targeting an ally is the headline use case; the AI's support
        // pipeline reads this lane to pick the action for debuffed
        // teammates rather than queueing it as offense.
        false
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
        if !feature_ready(encounter, caster_id, CLEANSING_TOUCH_TAG) {
            return false;
        }
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        if !target.is_combat_active() {
            return false;
        }
        // Mirror Lesser Restoration's gate: there must be *something*
        // on the target for the cleanse to grab. Walking the three
        // fallback lanes (concentration, debuff, buff) up front spares
        // the once-per-rest charge from a no-op apply.
        target.is_concentrating()
            || crate::engine::side_effects::CLEANSING_TOUCH_DEBUFFS
                .iter()
                .any(|c| target.has_condition(*c))
            || target
                .conditions()
                .keys()
                .any(|c| c.is_dispellable_buff())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(paladin) = encounter.actors.get_mut(&caster_id) {
            paladin.spend_feature(CLEANSING_TOUCH_TAG);
        }
        encounter.log("  cleansing touch: paladin channels divine cleansing.".to_string());
        vec![Box::new(crate::engine::side_effects::CleansingTouchOn {
            target_id,
        })]
    }
}

pub static CLEANSING_TOUCH: LazyLock<CleansingTouch> = LazyLock::new(|| CleansingTouch {});

/// Tag for the Aasimar Healing Hands racial trait. Once per long rest,
/// the aasimar touches a creature (or themselves) and heals them for a
/// number of HP equal to their level (RAW: 1 minute action, no other
/// resource cost). We collapse the duration to an Action cost — single
/// chunky heal per rest, mirroring Lay on Hands but smaller and racial
/// rather than class-locked.
pub const HEALING_HANDS_TAG: &str = "aasimar.healing_hands";

/// Healing Hands — Aasimar racial, touch range. Spend the once-per-rest
/// feature to heal an ally (or self) for `level` HP. The smaller heal
/// (compared to Lay on Hands) reflects that this is a racial trait
/// available to any class chassis, not a paladin-only kit feature.
/// Action cost (not bonus action) so the aasimar can't combo it with a
/// big swing — RAW: "Action" per the SRD.
pub struct HealingHands {}

impl Action for HealingHands {
    fn name(&self) -> &str {
        "healing hands"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hh", "heal-hands"]
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
    fn is_heal(&self) -> bool {
        true
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
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.feature_available(HEALING_HANDS_TAG) {
            return false;
        }
        // Target must be an ally (or self) and combat-active. Mirrors
        // Lay on Hands' gating — no wasted heal on a corpse.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() == actor.team() && target.is_combat_active()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let amount = actor.level().max(1);
        if let Some(aasimar) = encounter.actors.get_mut(&caster_id) {
            aasimar.spend_feature(HEALING_HANDS_TAG);
        }
        encounter.log(format!(
            "  healing hands: aasimar channels celestial light, healing {} HP.",
            amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static HEALING_HANDS: LazyLock<HealingHands> = LazyLock::new(|| HealingHands {});

/// Divine Smite — paladin feature, bonus action. Spends a level-1 spell
/// slot to prime the next successful melee weapon hit with +2d8 radiant
/// damage (consumed at the hit site in `resolve_attack`). RAW lets the
/// paladin spend higher-level slots for more radiant dice; we collapse
/// to the flat 2d8 lane to keep the resource model clean and avoid an
/// override-style level picker. The Smiting condition acts as the
/// primed flag — short timer (2 rounds) so a swing-less smite expires
/// rather than dangling indefinitely.
pub struct DivineSmite {}

impl Action for DivineSmite {
    fn name(&self) -> &str {
        "divine smite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ds", "smite"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Indirectly: the rider damage lands on the next hit, not on
        // this action's resolution. Returning false keeps the AI's
        // focus-fire pipeline from picking it as a damage option.
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
        // Bonus action + level-1 spell slot. Burning the slot is the
        // load-bearing resource cost; the bonus action just prevents the
        // paladin from chaining smites with other bonus actions.
        bonus_action_and_slot(1)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't double-prime: re-casting Divine Smite while already
        // primed is a waste of a slot. The AI's pipeline doesn't deeply
        // model this; the gate is here for symmetry with other
        // self-buff actions (Mage Armor / Rage / Sacred Weapon).
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && !a.has_condition(Condition::Smiting))
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Smiting,
            // 2-round window so a primed paladin who can't connect on
            // their own turn still has one more attack to land it on the
            // following round (e.g. a reaction-attack-of-opportunity).
            timer: ConditionTimer::Rounds(2),
        })]
    }
}

pub static DIVINE_SMITE: LazyLock<DivineSmite> = LazyLock::new(|| DivineSmite {});

/// Class-feature tag for Paladin's Channel Divinity: Sacred Weapon —
/// once per long rest. The Channel Divinity *resource* is shared between
/// multiple paladin sub-feature variants in RAW (Oath of Devotion's
/// Sacred Weapon + Turn the Unholy etc.); we only model Sacred Weapon so
/// the tag is sub-feature-specific.
pub const SACRED_WEAPON_TAG: &str = "paladin.sacred_weapon";

/// Channel Divinity: Sacred Weapon — paladin action. The paladin's
/// weapon glows with divine light: attack rolls gain a flat +CHA bonus
/// (read by `condition_attack_bonus`) for up to 10 rounds (1 minute
/// RAW). Once per long rest. We use a regular condition timer rather
/// than concentration so it stacks with the paladin's own spell
/// concentration (e.g. Compelled Duel + Sacred Weapon).
pub struct SacredWeapon {}

impl Action for SacredWeapon {
    fn name(&self) -> &str {
        "sacred weapon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sw-pal", "cd-sacred", "consecrate"]
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
        feature_ready(encounter, caster_id, SACRED_WEAPON_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            SACRED_WEAPON_TAG,
            Condition::Sacred,
            ConditionTimer::Rounds(10),
            "  sacred weapon: paladin's blade glows with divine light.",
        )
    }
}

pub static SACRED_WEAPON: LazyLock<SacredWeapon> = LazyLock::new(|| SacredWeapon {});

/// Class-feature tag for the Vengeance Paladin's Channel Divinity: Vow
/// of Enmity (RAW: lv3 subclass feature, once per short rest in RAW; we
/// collapse to once per long rest so the gating stays uniform with the
/// rest of the Channel Divinity envelope — Sacred Weapon / Turn Undead).
/// The actual advantage rider fires in `compute_attack_mode` via the
/// `matched_link_mode` helper reading the `Sworn` condition + `Sworn` back-link
/// link on the target.
pub const VOW_OF_ENMITY_TAG: &str = "paladin.vow_of_enmity";

/// Vow of Enmity — Vengeance Paladin Channel Divinity, bonus action.
/// Mark one creature within 10 ft (4 tiles) as the paladin's quarry;
/// the paladin (and only the paladin) gets advantage on attack rolls
/// against the marked target for up to 10 rounds (1 minute RAW). Once
/// per long rest.
///
/// Engine wiring:
///   - Installs `Condition::Sworn` on the target with a 10-round timer.
///   - Sets the target's `Sworn` back-link to the paladin's id via
///     `SetConditionLink(Condition::Sworn)` (mirrors Compelled Duel's Dueled + its back-link /
///     Goading Attack's Goaded + its back-link chain — same flag-plus-link
///     install shape, distinct field).
///   - `compute_attack_mode` reads the (Sworn, Sworn back-link == attacker)
///     pair via `matched_link_mode` and combines advantage when the
///     paladin attacks the sworn target.
///
/// Range gate (4 tiles = 10 ft RAW) matches the Compelled Duel envelope
/// but is shorter than Hunter's Mark (90 ft); the vow is meant for a
/// committed melee paladin marking the foe they're already closing on.
pub struct VowOfEnmity {}

impl Action for VowOfEnmity {
    fn name(&self) -> &str {
        "vow of enmity"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["voe", "vow", "enmity"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 10ft RAW = 4 tiles.
        Some(4)
    }
    fn is_harmful(&self) -> bool {
        // Vow of Enmity is *cast on* an enemy but has no harmful effect
        // by itself (no damage, no save). Marking the AI's targeting
        // pipeline as harmful keeps the helpful-action lane from
        // accidentally picking the vow against an ally; the
        // ally-vs-enemy gate in `custom_validate_input` enforces the
        // hostile-target requirement either way.
        true
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Shared "feature ready + hostile target" preamble via the
        // sibling helper the other single-target CD gates ride. The
        // vow's extra clause (already-sworn-by-us dedup) is layered
        // on top since it needs the caster_id back-reference on the
        // Sworn condition — the shared helper only exposes the
        // target, so we route through `hostile_target_burst_ready`
        // to drive the preamble and then check the caster-linked
        // dedup on the resolved target inline.
        if !hostile_target_burst_ready(
            encounter,
            caster_id,
            target_ids,
            VOW_OF_ENMITY_TAG,
        ) {
            return false;
        }
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        // Skip if the target is already sworn-by-us: re-applying the
        // vow just refreshes the timer without granting a new
        // mechanical benefit, and burns a once-per-rest charge for
        // nothing. Distinct from the shared `hostile_target_feature_ready`
        // helper's `skip_if_condition` gate — that one checks a single
        // condition without a caster-back-reference, while this one
        // needs to verify `linked_by(Condition::Sworn)` points at the same caster (a
        // different sworn-by-someone-else target should still allow
        // the vow to install, taking over the "sworn by" back-link).
        target.linked_by(Condition::Sworn) != Some(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(paladin) = encounter.actors.get_mut(&caster_id) {
            paladin.spend_feature(VOW_OF_ENMITY_TAG);
        }
        encounter.log("  vow of enmity: paladin swears wrath against the foe.".to_string());
        let mut out: Vec<Box<dyn ApplicableSideEffect>> = vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Sworn,
                // 10 rounds = 1 minute RAW. Same envelope as Sacred
                // Weapon — the vow's accuracy buff and the weapon
                // glow both ride the same per-fight window.
                timer: ConditionTimer::Rounds(10),
            }),
        ];
        // Pull the SetConditionLink(Sworn) install from the central
        // `condition_link_side_effect` dispatch — same source of truth
        // the weapon on-hit rider chain and Compelled Duel use, so a
        // single match arm there serves every flag-plus-link install.
        if let Some(link) = crate::engine::side_effects::condition_link_side_effect(
            Condition::Sworn,
            target_id,
            caster_id,
        ) {
            out.push(link);
        }
        out
    }
}

pub static VOW_OF_ENMITY: LazyLock<VowOfEnmity> = LazyLock::new(|| VowOfEnmity {});

/// Class-feature tag for the Open Hand Monk's **Wholeness of Body** (lv6
/// subclass feature, once per long rest in our model — RAW: once per
/// long rest at lv6 already). Action; self-heal for `3 × level` HP.
/// Distinct from Second Wind (bonus action, fighter-only, 1d10 + level)
/// and Lay on Hands (action, paladin-only, `5 × level + CHA`) on the
/// once-per-rest self-heal lane — same gating envelope, distinct numbers
/// and class lock.
pub const WHOLENESS_OF_BODY_TAG: &str = "monk.wholeness_of_body";

/// Wholeness of Body — Open Hand Monk action. Spend the once-per-rest
/// feature to heal self for `3 × level` HP (no scaling ability mod —
/// pure level scaling, mirroring the RAW). At level 6 (the strict RAW
/// gate) this is 18 HP; on higher-level Open Hand templates the heal
/// climbs linearly. Action cost (not bonus action) so the monk can't
/// stack it with a Flurry of Blows — the heal is a tempo trade, not
/// a free burst.
pub struct WholenessOfBody {}

impl Action for WholenessOfBody {
    fn name(&self) -> &str {
        "wholeness of body"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wob", "wholeness"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
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
        feature_ready(encounter, caster_id, WHOLENESS_OF_BODY_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let level = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.level())
            .unwrap_or(1);
        // 3 HP per monk level — at level 6 that's 18, comparable to a
        // mid-tier Lay on Hands but action-cost not bonus.
        let amount = (3 * level).max(1);
        if let Some(monk) = encounter.actors.get_mut(&caster_id) {
            monk.spend_feature(WHOLENESS_OF_BODY_TAG);
        }
        encounter.log(format!(
            "  wholeness of body: monk channels ki for {} HP.",
            amount
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static WHOLENESS_OF_BODY: LazyLock<WholenessOfBody> = LazyLock::new(|| WholenessOfBody {});

/// Class-feature tag for the Monk's **Empty Body** (RAW: level 18 monk
/// capstone-adjacent, once per long rest). Action; the monk spends 4 ki
/// points to project their body as a semi-corporeal echo: **Invisible**
/// for 10 rounds (1 minute RAW), plus **DamageResistant** for the
/// duration (RAW: resistance to all damage except force — we collapse
/// the "except force" carve-out into the general resistance since the
/// engine's DamageResistant lane halves every type; the delta from RAW
/// only shows on the rare Magic Missile / Disintegrate hit against a
/// monk holding this buff up). Distinct from `WholenessOfBody`
/// (self-heal): Empty Body is a defensive-invisibility burst, no HP
/// restore.
///
/// Composes cleanly with Patient Defense (bonus-action Dodge) and Step
/// of the Wind (bonus-action Dash+Disengage) on the same turn — the monk
/// enters Empty Body via the Action lane, then dashes clear via a bonus
/// action, leaving them a 30ft-away invisible + damage-halved threat
/// that no attacker can plausibly close for the first round.
pub const EMPTY_BODY_TAG: &str = "monk.empty_body";

/// Empty Body — Monk action, once per long rest. Spends the feature
/// charge to install both `Invisible` and `DamageResistant` on the monk
/// for 10 rounds (1 minute RAW). Action cost (not bonus action) so the
/// monk can't stack it with a Flurry — burning the Action lane is the
/// tempo trade for the defensive envelope. The DamageResistant install
/// covers every damage type in this engine (RAW carves out force damage
/// — a minor delta since the only in-engine force damage sources are
/// Magic Missile / Disintegrate / Bigby's Hand, all of which are rare
/// against a lv18 monk anyway).
///
/// Both installs use `Rounds(10)` timers so a stray Dispel Magic or a
/// long fight burn-off both drop naturally — no bespoke concentration
/// wire-up needed.
pub struct EmptyBody {}

impl Action for EmptyBody {
    fn name(&self) -> &str {
        "empty body"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["eb", "empty"]
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
        if !feature_ready(encounter, caster_id, EMPTY_BODY_TAG) {
            return false;
        }
        // No-op if the monk is already invisible AND damage-resistant —
        // re-priming would just refresh timers without granting a new
        // mechanical benefit, and burns the once-per-rest charge for
        // nothing.
        encounter.actors.get(&caster_id).is_some_and(|a| {
            !(a.has_condition(Condition::Invisible)
                && a.has_condition(Condition::DamageResistant))
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if let Some(monk) = encounter.actors.get_mut(&caster_id) {
            monk.spend_feature(EMPTY_BODY_TAG);
        }
        encounter.log(
            "  empty body: monk projects a semi-corporeal echo, becoming invisible and resistant to damage.".to_string(),
        );
        // Two installs, both `Rounds(10)` — 1 minute RAW. Same timer
        // envelope as Sacred Weapon / Rage / Vow of Enmity.
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Invisible,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::DamageResistant,
                timer: ConditionTimer::Rounds(10),
            }),
        ]
    }
}

pub static EMPTY_BODY: LazyLock<EmptyBody> = LazyLock::new(|| EmptyBody {});

/// Class-feature tag for the Circle of the Moon Druid's **Combat Wild
/// Shape** (subclass level 2). Gates both halves of the feature: the
/// `WILD_SHAPE` bonus action below and the `WILD_HEAL` slot-to-hit-
/// points conversion that only works while in form.
///
/// One charge per short rest. RAW gives the druid two Wild Shape uses
/// per short rest; the engine's per-tag charge model is one-per-tag, so
/// the collapse to a single use is the same one every other multi-use
/// feature already makes (Stunning Strike's ki pool, Bardic
/// Inspiration's CHA-mod pool, the Battle Master's superiority dice).
/// Registered in `SHORT_REST_FEATURES` so the rest cadence is RAW-exact
/// even though the count isn't.
pub const COMBAT_WILD_SHAPE_TAG: &str = "druid.combat_wild_shape";

/// Beast-form hit points, granted as temp HP on transformation.
///
/// 34 — the brown bear's hit points, matching the form
/// `BEAST_FORM_CLAWS` swings for. Circle of the Moon's **Circle Forms**
/// caps the druid at CR 1 at subclass level 2, and the bear is the CR-1
/// beast the class is famous for taking, so one number describes the
/// whole form: 34 HP and 2d6+4 claws.
///
/// Temp HP rather than a second HP bar because that is what the engine
/// has, and because the semantics line up better than they might
/// appear: RAW's beast form takes damage to its own pool and reverts
/// when that pool empties, leaving the druid's own HP untouched, which
/// is exactly how temp HP drains. The deviation is at the seam —
/// emptying the pool doesn't force an early revert here, so the form's
/// `Rounds(10)` timer is what ends it either way.
///
/// Flat rather than level-scaled for the same reason `Polymorph` grants
/// a flat 30: the pool describes the *form*, not the caster, and
/// `ActorInstance::level` tracks in-run XP progression from 1 rather
/// than the build level a class template targets.
const BEAST_FORM_TEMP_HP: u32 = 34;

/// Wild Shape — Circle of the Moon Druid bonus action, one charge per
/// short rest. Installs `Condition::WildShaped` and hands the druid the
/// beast form's hit points as temp HP.
///
/// The bonus-action cost *is* Combat Wild Shape — baseline Wild Shape
/// is an Action, and the whole subclass feature at level 2 is that the
/// moon druid can transform and still swing on the same turn.
///
/// What makes the button a decision rather than a freebie is what
/// `WildShaped` costs: the form blocks spell slots outright, so a
/// full-caster who takes it is trading Moonbeam, Call Lightning, Sleet
/// Storm and every heal on the list for 34 temp HP and a 2d6+4 melee
/// swing. That is a good trade when the druid is out of position or
/// already concentrating on nothing, and a bad one when the party needs
/// the control. The AI's gate is where that judgement lives.
pub struct WildShape {}

impl Action for WildShape {
    fn name(&self) -> &str {
        "wild shape"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ws", "shape"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !feature_ready(encounter, caster_id, COMBAT_WILD_SHAPE_TAG) {
            return false;
        }
        // Already a bear — re-transforming would burn the charge to
        // refresh a timer and re-grant a temp HP pool that
        // `GainTempHp` would mostly discard anyway (temp HP replaces
        // rather than stacks, so a partly-drained pool is the only
        // case that gains anything, and not enough to be worth a rest
        // charge).
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| !a.has_condition(Condition::WildShaped))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if let Some(druid) = encounter.actors.get_mut(&caster_id) {
            druid.spend_feature(COMBAT_WILD_SHAPE_TAG);
        }
        encounter.log(format!(
            "  wild shape: the druid's form runs like water into a bear ({} temp HP).",
            BEAST_FORM_TEMP_HP
        ));
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::WildShaped,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(GainTempHp {
                actor_id: caster_id,
                amount: BEAST_FORM_TEMP_HP,
            }),
        ]
    }
}

pub static WILD_SHAPE: LazyLock<WildShape> = LazyLock::new(|| WildShape {});

/// Wild Heal — the second half of Combat Wild Shape: "you can expend a
/// spell slot to regain 1d8 hit points per level of the spell slot
/// expended." Bonus action, only while `WildShaped`.
///
/// RAW gives this clause no name of its own; "wild heal" is the
/// engine's handle for it.
///
/// **The slot is drained directly, not spent as a `Resource`.** That
/// looks like a shortcut and isn't: `WildShaped` rides
/// `blocks_spell_slots`, so a `Resource::SpellSlot` cost would be
/// refused by `can_consume_resource` and the action could never fire in
/// the only state it is legal in. RAW is careful about exactly this —
/// the clause says *expend* a slot, not *cast* — so bypassing the
/// can't-cast gate is the faithful reading rather than a workaround.
/// The consume happens in `side_effects` alongside the log line, which
/// is the same place `spend_feature` fires for every per-rest feature.
///
/// **It always spends the lowest available slot.** RAW lets the druid
/// pick, and the engine has no channel for "this action, but at level
/// 3" outside the spell-slot cost machinery this action deliberately
/// sidesteps. Lowest-first is the right default for both drivers: a
/// human wants their high slots kept for the spells they'll cast after
/// reverting, and an AI with no way to answer the question shouldn't be
/// handed it. The consequence is a deliberately modest heal — 1d8 off a
/// level-1 slot — which keeps the conversion a top-up rather than a
/// second HP bar on top of the 34 temp HP the form already granted.
pub struct WildHeal {}

impl WildHeal {
    /// Lowest slot level the druid can still spend, or `None` when the
    /// pool is dry. Shared by the validator and the side-effect body so
    /// the two can't disagree about which slot is being burned.
    fn slot_level(encounter: &EncounterInstance, caster_id: usize) -> Option<u32> {
        encounter
            .actors
            .get(&caster_id)
            .and_then(|a| a.lowest_available_spell_slot())
    }
}

impl Action for WildHeal {
    fn name(&self) -> &str {
        "wild heal"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wh"]
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
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Bonus action only — the slot is drained in `side_effects`
        // rather than declared here, because `WildShaped` blocks the
        // `SpellSlot` resource lane outright. See the type doc.
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(druid) = encounter.actors.get(&caster_id) else {
            return false;
        };
        druid.is_combat_active()
            && druid.has_condition(Condition::WildShaped)
            // Nothing to top up — don't burn a slot at full HP.
            && druid.hitpoints() < druid.max_hitpoints()
            && Self::slot_level(encounter, caster_id).is_some()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(level) = Self::slot_level(encounter, caster_id) else {
            return Vec::new();
        };
        let rolled = encounter.roll(&Dice::new(level, 8));
        if let Some(druid) = encounter.actors.get_mut(&caster_id) {
            druid.spell_slot_manager.consume_spell_slot(level);
        }
        encounter.log(format!(
            "  wild heal: the beast body knits shut — level-{} slot, {}d8({}) HP",
            level, level, rolled
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount: rolled,
        })]
    }
}

pub static WILD_HEAL: LazyLock<WildHeal> = LazyLock::new(|| WildHeal {});

/// Class-feature tag for the Way of Shadow Monk's **Shadow Arts**
/// (subclass level 3). RAW: spend 2 ki to cast Darkness, Darkvision,
/// Pass without Trace or Silence, plus the Minor Illusion cantrip at
/// will.
///
/// Purely a marker — the spells themselves ride the monk's action list
/// and are paid for out of the shadow monk's slot table, which exists
/// only to be Shadow Arts' ki budget (see `SHADOW_MONK_TEMPLATE`).
/// Darkness and Darkvision are dropped: the engine has no light level
/// for either to act on, the same reason the Diviner's Third Eye and
/// the Transmuter's stone both drop their darkvision options. Minor
/// Illusion has no combat surface at all.
///
/// The tag earns its keep as the thing that makes the loadout legible:
/// without it, a shadow monk holding two lv2 slots and two illusion
/// spells looks like a template that got a caster's fields by accident
/// rather than one that spent its ki.
pub const SHADOW_ARTS_TAG: &str = "monk.shadow_arts";

/// Class-feature tag for the Way of Shadow Monk's **Shadow Step**
/// (subclass level 6). Gates the `SHADOW_STEP` bonus action below.
///
/// At-will in RAW (no ki cost at all — it's the one free thing the
/// subclass does), which is exactly how it ships: the bonus action is
/// the entire price. That makes it the longest at-will repositioning
/// tool in the engine at 60 ft, twice Misty Step's range and without
/// the slot.
///
/// RAW gates the teleport on starting *and* ending in dim light or
/// darkness. The engine has no light level, so the gate is dropped
/// wholesale rather than approximated — every candidate proxy
/// (obscuring terrain, distance from allies) would be a different
/// restriction wearing the clause's name.
pub const SHADOW_STEP_TAG: &str = "monk.shadow_step";

/// Shadow Step — Way of Shadow Monk bonus action, at-will. Teleport up
/// to 60 ft (24 tiles) to a spot the monk can see, and gain advantage
/// on the first melee attack made before the end of the turn.
///
/// The teleport half is `MistyStep`'s exactly — same `TeleportActor`
/// side-effect, same `can_move_to` landing validation, same explicit
/// OA-freedom (the monk doesn't traverse the intervening tiles). It
/// differs on price and reach: bonus action and nothing else, at double
/// the range, where Misty Step spends a level-2 slot for 30 ft.
///
/// The advantage half is what makes it an attack rather than an escape.
/// Misty Step is what a caster uses to *leave*; Shadow Step is what a
/// monk uses to *arrive*, and the `Shadowstepping` prime is the
/// difference — it turns a 60 ft gap-closer into a 60 ft gap-closer
/// that also lands the Stunning Strike the monk primed on the way in.
/// The prime is melee-gated (`grants_self_melee_attack_advantage`)
/// because RAW says melee, and one-shot via `CONSUMED_ON_ATTACK`.
pub struct ShadowStep {}

impl Action for ShadowStep {
    fn name(&self) -> &str {
        "shadow step"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ss", "shadow"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.has_passive_feature(SHADOW_STEP_TAG))
        {
            return false;
        }
        // Destination must be a legal landing spot for this monk's full
        // footprint — same constraint Misty Step applies, minus the
        // movement-budget check the teleport bypasses.
        let Some(point) = first_target_location(target_locations) else {
            return false;
        };
        encounter.can_move_to(caster_id, point)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        encounter.log(
            "  shadow step: the monk melts into one shadow and rises from another.".to_string(),
        );
        vec![
            // Teleport, not movement — no intervening tiles, so no
            // opportunity attacks. Same reasoning as Misty Step.
            Box::new(crate::engine::side_effects::TeleportActor {
                actor_id: caster_id,
                dest: point,
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Shadowstepping,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }),
        ]
    }
}

pub static SHADOW_STEP: LazyLock<ShadowStep> = LazyLock::new(|| ShadowStep {});

/// Class-feature tag for the Monk's Stunning Strike (once per long
/// rest, in our model — RAW is one per ki point, but we collapse the
/// ki pool into a single big-burst prime to keep the once-per-rest
/// gating pattern uniform). The actual stun save fires on the next
/// melee hit via the StunningStrike condition rider in
/// `EncounterInstance::resolve_attack`.
pub const STUNNING_STRIKE_TAG: &str = "monk.stunning_strike";

/// Monk Stunning Strike — bonus action. Primes the monk's next melee
/// hit: when the swing lands, the target makes a CON save vs the monk's
/// WIS-based DC (8 + prof + WIS). On fail, the target is Stunned until
/// the end of the monk's next turn. We model the prime as a caster-side
/// condition (StunningStrike) that the on-hit hook in `resolve_attack`
/// consumes — mirrors the Smiting pattern.
pub struct StunningStrike {}

impl Action for StunningStrike {
    fn name(&self) -> &str {
        "stunning strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ss-monk", "stun"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_prime_ready(encounter, caster_id, STUNNING_STRIKE_TAG, Condition::StunningStrike)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // 2-round prime window so a primed monk who misses the
        // first swing still has the rest of this turn + next to
        // connect (same envelope as Divine Smite).
        prime_self_condition(
            encounter,
            caster_id,
            STUNNING_STRIKE_TAG,
            Condition::StunningStrike,
            ConditionTimer::Rounds(2),
            "  stunning strike: monk's next hit primes a stun save.",
        )
    }
}

pub static STUNNING_STRIKE: LazyLock<StunningStrike> = LazyLock::new(|| StunningStrike {});

/// Class-feature tag for the Monk's Patient Defense — bonus-action
/// Dodge. At-will (RAW: 1 ki point per use; we drop the ki pool to keep
/// the bonus-action mobility tools uniform with Cunning Action).
pub const PATIENT_DEFENSE_TAG: &str = "monk.patient_defense";

/// Patient Defense — Monk bonus action. Take the Dodge action as a
/// bonus action: attacks vs the monk have disadvantage and DEX saves
/// gain advantage until the start of the monk's next turn. Mirrors
/// `CunningDisengage` / `CunningHide` — same one-shot bonus-action
/// pattern, just a different resulting flag.
pub struct PatientDefense {}

impl Action for PatientDefense {
    fn name(&self) -> &str {
        "patient defense"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pd", "patient"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::SetDodging {
            actor_id: caster_id,
            dodging: true,
        })]
    }
}

pub static PATIENT_DEFENSE: LazyLock<PatientDefense> = LazyLock::new(|| PatientDefense {});

/// Step of the Wind — 5e Monk level-2 bonus action. Spends ki to take the
/// Dash AND Disengage actions for free as a bonus action (we collapse
/// the ki cost into the bonus-action lane since the engine doesn't track
/// a ki pool — same simplification as Patient Defense / Flurry of Blows).
/// Mechanically a fusion of the rogue's `CunningDash` (extra movement
/// equal to speed) and `CunningDisengage` (movement this turn doesn't
/// provoke OAs) — fired in one bonus action instead of two separate
/// activations, matching the monk's signature "blow past the front line"
/// flavor. RAW also doubles jump distance for the turn; we don't model
/// vertical movement so that clause is a no-op.
pub struct StepOfTheWind {}

impl Action for StepOfTheWind {
    fn name(&self) -> &str {
        "step of the wind"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sotw", "step", "wind"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let speed = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.speed())
            .unwrap_or(0.0);
        encounter.log("  step of the wind: monk surges past the front line.".to_string());
        vec![
            Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::Movement(speed),
            }),
            Box::new(crate::engine::side_effects::SetDisengaging {
                actor_id: caster_id,
                disengaging: true,
            }),
        ]
    }
}

pub static STEP_OF_THE_WIND: LazyLock<StepOfTheWind> = LazyLock::new(|| StepOfTheWind {});

/// Stillness of Mind — Monk action (5e level 7). At-will: spend an Action
/// to end one Charmed or Frightened condition currently affecting the
/// monk. RAW gates on "you can use your action" so it's never resource-
/// gated — a long-rest cap or feature tag would be over-restrictive. We
/// model exactly as RAW: removes both conditions in a single action when
/// either is up, and silently no-ops when neither is up so a misguided
/// click doesn't burn the Action lane. Self-targeted; needs no spell slot
/// or other resource beyond the standard Action.
///
/// Sits adjacent to Patient Defense (the other in-combat survival action
/// on the monk's sheet). The "auto-clear both" simplification matches our
/// Charmed/Frightened coverage — both are handled at the same chokepoints
/// (compute_attack_mode for the disadvantage on attacks, Charmed back-link for
/// the can't-target-charmer gate), so stripping both at once stays
/// consistent with how the conditions are read.
pub struct StillnessOfMind {}

impl Action for StillnessOfMind {
    fn name(&self) -> &str {
        "stillness of mind"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["som", "still"]
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
        // Only fire when there's actually something to cleanse; otherwise
        // a stray click would burn the monk's Action for nothing.
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.is_combat_active()
                && (a.has_condition(Condition::Charmed)
                    || a.has_condition(Condition::Frightened))
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        encounter.log("  stillness of mind: monk centers their mind.".to_string());
        // Drop both Charmed and Frightened in one swing — the
        // RemoveCondition side-effect is a no-op for missing conditions
        // so an actor with only one of the two gets the clean cleanse.
        vec![
            Box::new(RemoveCondition {
                actor_id: caster_id,
                condition: Condition::Charmed,
            }),
            Box::new(RemoveCondition {
                actor_id: caster_id,
                condition: Condition::Frightened,
            }),
        ]
    }
}

pub static STILLNESS_OF_MIND: LazyLock<StillnessOfMind> = LazyLock::new(|| StillnessOfMind {});

/// Class-feature tag for the Bard's Bardic Inspiration (RAW: a pool of
/// CHA-mod uses per long rest — we collapse to a single big use to keep
/// the once-per-rest pattern uniform).
pub const BARDIC_INSPIRATION_TAG: &str = "bard.bardic_inspiration";

/// Bardic Inspiration — Bard bonus action, single ally. Grants the
/// Inspired condition on a willing ally within 60ft (24 tiles), letting
/// them add a flat +3 (the d6-average) to their next attack roll, save,
/// or ability check. We tag both the attack-roll bonus (via
/// `condition_attack_bonus`) and the save bonus (`condition_save_bonus`)
/// so the inspiration die is useful regardless of which roll comes up
/// next. The condition has a 10-round timer (1 minute RAW); the next
/// attack / save consumes it implicitly when the on-hit / save site
/// strips the condition (see `clear_inspired_on_attack`).
pub struct BardicInspiration {}

impl Action for BardicInspiration {
    fn name(&self) -> &str {
        "bardic inspiration"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bi", "inspire"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles.
        Some(24)
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.feature_available(BARDIC_INSPIRATION_TAG) {
            return false;
        }
        // Target must be an ally (same team), combat-active, and not
        // already Inspired — re-inspiration would just refresh the
        // timer without giving the AI a meaningful new effect.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() == actor.team()
            && target.is_combat_active()
            && !target.has_condition(Condition::Inspired)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(BARDIC_INSPIRATION_TAG);
        }
        encounter.log("  bardic inspiration: ally rallies, gaining a die.".to_string());
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Inspired,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static BARDIC_INSPIRATION: LazyLock<BardicInspiration> = LazyLock::new(|| BardicInspiration {});

/// Class-feature tag for Cleric Channel Divinity: Turn Undead.
pub const TURN_UNDEAD_TAG: &str = "cleric.turn_undead";

/// Class-feature tag for the Devotion Paladin's level-3 subclass
/// Channel Divinity: **Turn the Faithless**. Mechanically identical to
/// Turn Undead (30ft WIS-save burst → Frightened on fail) except the
/// creature-type filter is fey / fiend rather than undead. Once per
/// short rest; refreshes via `SHORT_REST_FEATURES`.
///
/// Distinct from the shared Cleric Turn Undead in three ways:
///   1. **Class** — rides on the paladin's spell save DC (CHA-based)
///      rather than the cleric's (WIS-based). RAW: the Turn CD lane
///      always keys off the caster's spellcasting ability, so this
///      matches per-caster.
///   2. **Creature-type filter** — RAW targets any celestial, elemental,
///      fey, fiend, or undead the paladin can see. We narrow to fey /
///      fiend so the paladin has a distinct-from-cleric target set;
///      celestials are RAW allies of the Devotion oath (a Turn against
///      them is edge-case) and elementals / undead overlap with other
///      lanes (Turn Undead for undead, Protection From Evil for the
///      broader elemental/fey/fiend cohort).
///   3. **Once per short rest** — refreshed via `SHORT_REST_FEATURES`.
///      RAW Turn Undead is also once per short rest but our baseline
///      cleric collapses it to long-rest for parity with the other
///      cleric long-rest features; Turn the Faithless stays RAW-exact
///      here since it ships with the paladin's short-rest CD family
///      (Guided Strike / Radiance of the Dawn / Warding Flare all
///      short-rest on their respective subclass templates).
///
/// Ships on `DEVOTION_PALADIN_TEMPLATE` above its strict RAW level
/// gate for the same reason every other subclass template runs above
/// strict RAW level (class templates target a balanced playable
/// level, not lockstep PHB progression).
pub const TURN_THE_FAITHLESS_TAG: &str = "paladin.turn_the_faithless";

/// Shared "turn a creature-type cohort within 30ft with a WIS-save
/// Frightened install" body. Turn Undead and Turn the Faithless are
/// mechanically identical except for the creature-type filter and the
/// caster's spellcasting ability, so the loop lives here once. Adding
/// a future "Turn Elementals" / "Turn Fey" CD (Cleric domain, druid
/// subclass) drops in as a fresh caller with a distinct filter closure.
///
/// - `caster_id` — spends `feature_tag` before running.
/// - `spellcasting_ability` — used both for the save DC and the log
///   line (the WIS-based cleric turns use WIS; the CHA-based paladin
///   turns use CHA).
/// - `is_affected` — creature-type gate: returns true iff the target's
///   `CreatureType` is in the "turn" cohort (undead for Turn Undead,
///   fey / fiend for Turn the Faithless).
/// - `installed` — condition laid on each failed-save target. Frightened
///   for the three Turn / dread variants; Charmed for the Nature
///   Domain's Charm Animals and Plants. Routed through
///   `install_condition_with_link`, so a condition carrying a back-link
///   (Charmed's back-link) gets it without this resolver knowing which
///   conditions do.
/// - `label` — log prefix ("turn undead" / "turn the faithless").
fn resolve_turn_burst(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    feature_tag: &'static str,
    spellcasting_ability: AbilityScoreType,
    is_affected: fn(crate::engine::types::CreatureType) -> bool,
    installed: Condition,
    label: &'static str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let Some(dc) = spend_feature_and_get_dc(
        encounter,
        caster_id,
        feature_tag,
        spellcasting_ability,
    ) else {
        return Vec::new();
    };
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return Vec::new();
    };
    let caster_loc = caster.location();
    let caster_team = caster.team();
    let caster_size = get_tiles_from_size(caster.size());
    // Destroy Undead (Cleric lv5 passive) gate — checked once up front
    // so the per-target branch below is a cheap flag read rather than a
    // fresh `has_passive_feature` lookup per candidate. The destroy
    // branch only fires for undead targets at or below the CR ceiling
    // AND requires the caster to hold the passive tag; the "target
    // undead" half of the gate is checked inside the loop against the
    // per-target creature type so a mixed-cohort Turn (a hypothetical
    // future "Turn any Faithless" that swept both fey AND undead)
    // would still route the undead half through destroy and the fey
    // half through Frighten. Safe against `resolve_turn_burst` callers
    // whose cohort doesn't include undead (e.g. Turn the Faithless
    // targets fey / fiend only) — the per-target check short-circuits.
    let destroy_undead = caster.has_passive_feature(DESTROY_UNDEAD_TAG);
    encounter.log(format!(
        "  {}: every affected creature within 30ft saves (DC {}).",
        label, dc
    ));

    let candidates: Vec<usize> = encounter
        .sorted_actor_ids()
        .into_iter()
        .filter(|id| {
            let Some(a) = encounter.actors.get(id) else {
                return false;
            };
            if *id == caster_id || a.team() == caster_team || !a.is_combat_active() {
                return false;
            }
            if !is_affected(a.creature_type()) {
                return false;
            }
            let dist = footprint_chebyshev(
                a.location(),
                get_tiles_from_size(a.size()),
                caster_loc,
                caster_size,
            );
            dist <= 12
        })
        .collect();

    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    for id in candidates {
        let save = encounter.roll_save(id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            continue;
        }
        // Destroy Undead branch: if the caster carries the passive tag
        // AND the failed-save target is Undead AND its CR is at or
        // below `DESTROY_UNDEAD_CR_CEILING`, deal HP-matching radiant
        // damage to kill outright instead of installing Frightened.
        // Falls back to the standard Frighten install on any gate miss
        // (non-undead target, above-ceiling CR, cleric without the
        // passive tag) — the two branches are mutually exclusive per
        // target, matching RAW's "instead of turned" clause.
        if destroy_undead
            && let Some(target) = encounter.actors.get(&id)
            && target.creature_type().is_undead()
            && target.cr() <= DESTROY_UNDEAD_CR_CEILING
        {
            let killing_damage = target.hitpoints();
            encounter.log(format!(
                "  {}: destroys undead ({} radiant, CR {} ≤ {}).",
                label,
                killing_damage,
                target.cr(),
                DESTROY_UNDEAD_CR_CEILING
            ));
            effects.push(Box::new(crate::engine::side_effects::DealDamage {
                actor_id: id,
                amount: killing_damage,
                damage_type: DamageType::Radiant,
            }));
            continue;
        }
        // `install_condition_with_link` covers the back-link half for
        // conditions that carry one — Charmed's back-link anchors the
        // "can't attack your charmer" gate, and Frightened has no link,
        // so the same call serves every variant.
        effects.extend(crate::engine::side_effects::install_condition_with_link(
            installed,
            id,
            caster_id,
            ConditionTimer::Rounds(10),
        ));
    }
    effects
}

/// Class-feature tag for the Oathbreaker Paladin's level-3 subclass
/// Channel Divinity: **Dreadful Aspect**. Mechanically a sibling of
/// Turn Undead / Turn the Faithless (30ft WIS-save burst → Frightened
/// on fail) with the creature-type filter *dropped* — every hostile
/// within 30ft that fails the WIS save picks up Frightened for 10
/// rounds (1 minute RAW). Once per short rest; refreshes via
/// `SHORT_REST_FEATURES`.
///
/// The Oathbreaker's "unshakeable dread" tell — where Devotion turns
/// fey / fiend and vanilla clerics turn undead, the Oathbreaker
/// projects a raw fear-aura that doesn't care what type its target is.
/// Distinct from Turn Undead / Turn the Faithless on the CD lane in
/// two ways:
///   1. **No creature-type filter** — every combat-active hostile in
///      range that fails the save picks up Frightened. The Oathbreaker
///      is the paladin whose Channel Divinity works against a party
///      of humanoid bandits or a horde of undead alike.
///   2. **Ability anchor** — CHA-based DC per RAW (all paladin CDs).
///      Sibling to Turn the Faithless's CHA anchor and distinct from
///      Turn Undead's WIS anchor (cleric spellcasting ability).
///
/// Ships on `OATHBREAKER_PALADIN_TEMPLATE` — sibling to Aura of Hate
/// (lv7 passive melee damage bump) and Fanatical Focus (lv15 auto-
/// reroll of a failed save) already on that template. The CR-1.5
/// template lists the lv3 CD action + short-rest charge alongside the
/// higher-level subclass features for the same reason every other
/// paladin subclass template ships its lv3 CD at CR 1.5 (Nature's
/// Wrath on Ancients, Turn the Faithless on Devotion, Abjure Enemy on
/// Vengeance) — class templates target a balanced playable level, not
/// lockstep PHB progression.
pub const DREADFUL_ASPECT_TAG: &str = "paladin.dreadful_aspect";

/// Config-driven Channel Divinity turn-burst action. Every "action; each
/// hostile of some creature-type set within 30 ft rolls a WIS save vs the
/// caster's spell DC or is Frightened for 10 rounds" feature in the engine
/// is one instance of this struct.
///
/// Four features share the shape, differing only on the alias set, the
/// per-rest charge tag, which ability anchors the DC, and which creature
/// types are eligible. They used to be four near-identical `impl Action`
/// blocks — same `NoArgs` schema, same `is_harmful` / `deals_damage`, same
/// one-line `custom_validate_input`, same `resolve_turn_burst` call with
/// four arguments changed — which meant adding a fifth turn variant cost
/// ~55 lines of boilerplate to express four values. Now it costs a struct
/// literal, exactly as the sibling config-driven `ManeuverPrime` /
/// `SpellSlotRecovery` / Smite actions already do.
///
/// The creature-type filter is a `fn(CreatureType) -> bool` rather than a
/// type set, because the variants disagree about shape as well as
/// contents: Turn Undead delegates to `CreatureType::is_undead()`, Arcane
/// Abjuration matches four unrelated types, and Dreadful Aspect drops the
/// gate entirely.
pub struct TurnBurst {
    /// Display name — the action list entry, the prompt parser's
    /// canonical name, and the log line prefix.
    pub name: &'static str,
    /// Alias set for the prompt parser. `&'static [&'static str]` keeps
    /// the struct plain data at LazyLock init.
    pub aliases: &'static [&'static str],
    /// Feature tag whose per-rest charge gates the burst. Read via
    /// `feature_ready` on the validate side; spent inside
    /// `resolve_turn_burst`.
    pub tag: &'static str,
    /// Ability that anchors the save DC — WIS for the cleric domains,
    /// CHA for the paladin oaths, matching each class's spellcasting
    /// ability.
    pub dc_ability: AbilityScoreType,
    /// Which creature types the burst can affect. The team and
    /// combat-active filters live inside `resolve_turn_burst`; this
    /// closure adds only the RAW type gate, so `|_| true` means "any
    /// hostile".
    pub type_filter: fn(crate::engine::types::CreatureType) -> bool,
    /// Condition laid on each failed-save target, for 10 rounds. The
    /// three Turn / dread variants install Frightened; the Nature
    /// Domain's Charm Animals and Plants installs Charmed. Any back-link
    /// the condition carries is queued by the resolver.
    pub installed: Condition,
}

impl Action for TurnBurst {
    fn name(&self) -> &str {
        self.name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        true
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
        feature_ready(encounter, caster_id, self.tag)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        resolve_turn_burst(
            encounter,
            caster_id,
            self.tag,
            self.dc_ability,
            self.type_filter,
            self.installed,
            self.name,
        )
    }
}

/// Turn Undead — Cleric Channel Divinity, action. Every Undead within
/// 30 ft (12 tiles) makes a WIS save vs the cleric's WIS-based DC; on
/// fail they're Frightened for 10 rounds (1 minute RAW). Once per long
/// rest in our model (RAW Channel Divinity is once per short rest —
/// collapsed to long-rest here for parity with the other baseline cleric
/// long-rest features).
pub static TURN_UNDEAD: LazyLock<TurnBurst> = LazyLock::new(|| TurnBurst {
    name: "turn undead",
    aliases: &["turn", "cd-turn"],
    tag: TURN_UNDEAD_TAG,
    dc_ability: AbilityScoreType::Wisdom,
    type_filter: |ct| ct.is_undead(),
    installed: Condition::Frightened,
});

/// Turn the Faithless — Devotion Paladin Channel Divinity, action. Every
/// fey and fiend within 30 ft makes a WIS save vs the paladin's CHA-based
/// DC. Once per short rest.
///
/// RAW gates on "any celestial, elemental, fey, fiend, or undead" the
/// paladin can see; we narrow to fey / fiend so the paladin has a
/// distinct-from-cleric target set (celestials are RAW allies of the
/// Devotion oath; elementals / undead overlap with Turn Undead and
/// Protection From Evil).
pub static TURN_THE_FAITHLESS: LazyLock<TurnBurst> = LazyLock::new(|| TurnBurst {
    name: "turn the faithless",
    aliases: &["ttf", "cd-turnf", "faithless"],
    tag: TURN_THE_FAITHLESS_TAG,
    // Paladin spellcasting ability is Charisma per PHB; every paladin
    // subclass uses CHA for spell save DCs. Distinct from the Cleric
    // domains' WIS anchor.
    dc_ability: AbilityScoreType::Charisma,
    type_filter: |ct| {
        matches!(
            ct,
            crate::engine::types::CreatureType::Fey | crate::engine::types::CreatureType::Fiend
        )
    },
    installed: Condition::Frightened,
});

/// Arcane Abjuration — Arcana Domain Cleric Channel Divinity (lv2,
/// SCAG), action. Every celestial, elemental, fey or fiend within 30 ft
/// makes a WIS save vs the cleric's WIS-based DC; on fail they're
/// Frightened for 10 rounds. Once per short rest.
///
/// The widest type filter of the three gated variants, and deliberately
/// the complement of Turn Undead rather than an overlap: between an
/// Arcana Cleric and any other cleric, a party covers every extraplanar
/// creature type in the engine. That complementarity is the domain's
/// argument for existing — RAW's Arcane Abjuration also banishes a
/// low-CR target outright at lv5, which is the half that would make it
/// strictly better than Turn Undead and is not modeled (banishment needs
/// an off-board actor lane the engine doesn't have).
///
/// Wider than Turn the Faithless's fey / fiend narrowing because there's
/// no ally-flavor reason to spare celestials here: the Arcana Cleric
/// abjures *outsiders*, not evil ones.
pub static ARCANE_ABJURATION: LazyLock<TurnBurst> = LazyLock::new(|| TurnBurst {
    name: "arcane abjuration",
    aliases: &["aa", "cd-abjure", "abjuration", "abjure"],
    tag: ARCANE_ABJURATION_TAG,
    dc_ability: AbilityScoreType::Wisdom,
    type_filter: |ct| {
        use crate::engine::types::CreatureType;
        matches!(
            ct,
            CreatureType::Celestial
                | CreatureType::Elemental
                | CreatureType::Fey
                | CreatureType::Fiend
        )
    },
    installed: Condition::Frightened,
});

/// Charm Animals and Plants — Nature Domain Cleric Channel Divinity
/// (lv2, PHB), action. Every beast and plant within 30 ft makes a WIS
/// save vs the cleric's WIS-based DC; on fail it is Charmed by the cleric
/// for 10 rounds. Once per short rest.
///
/// The only `TurnBurst` config that installs Charmed rather than
/// Frightened, and the reason the struct carries an `installed` column at
/// all. Charmed is the stronger of the two on paper — a charmed creature
/// can't attack the cleric or target them with harmful effects at all,
/// where a frightened one merely rolls at disadvantage — but the type
/// filter is the narrowest in the cohort. Against a druid's summons, a
/// pack of wolves or an awakened forest it ends the fight; against
/// anything humanoid it does nothing.
///
/// RAW's charm ends early "if the creature takes any damage", which the
/// engine doesn't model: `Charmed` has no damage-clears-it clause, and
/// adding one would change every other charm source. So the party can
/// safely beat on a charmed bear here in a way RAW wouldn't allow — a
/// real over-grant, and the reason this config's narrow type filter is
/// load-bearing rather than incidental.
pub static CHARM_ANIMALS_AND_PLANTS: LazyLock<TurnBurst> = LazyLock::new(|| TurnBurst {
    name: "charm animals and plants",
    aliases: &["cap", "cd-charm", "charm-nature"],
    tag: CHARM_ANIMALS_AND_PLANTS_TAG,
    dc_ability: AbilityScoreType::Wisdom,
    type_filter: |ct| {
        use crate::engine::types::CreatureType;
        matches!(ct, CreatureType::Beast | CreatureType::Plant)
    },
    installed: Condition::Charmed,
});

/// Dreadful Aspect — Oathbreaker Paladin Channel Divinity (lv3), action.
/// Every hostile within 30 ft makes a WIS save vs the paladin's CHA-based
/// DC. Once per short rest. The only variant with no creature-type gate:
/// RAW is "each creature of your choice that you can see within 30 feet",
/// so the Oathbreaker's dread works on a party of humanoid bandits as
/// readily as on a horde of undead.
pub static DREADFUL_ASPECT: LazyLock<TurnBurst> = LazyLock::new(|| TurnBurst {
    name: "dreadful aspect",
    aliases: &["da", "cd-dread", "dreadful", "dread"],
    tag: DREADFUL_ASPECT_TAG,
    // CHA-anchored DC per RAW paladin CD.
    dc_ability: AbilityScoreType::Charisma,
    // No creature-type filter — the team + combat-active filters inside
    // `resolve_turn_burst` handle the "hostile and standing" half.
    type_filter: |_| true,
    installed: Condition::Frightened,
});

/// Flurry of Blows — Monk bonus action. After the monk takes the Attack
/// action, they may spend a ki point (modeled as a bonus action — we don't
/// track ki) to make two unarmed strikes against a target. We collapse to
/// a single side-effect: the monk gets one extra Action (which they can
/// then use on a martial-arts strike, double-dipping their swing cap for
/// the turn). The "must have already attacked" gate from RAW is dropped
/// for simplicity — the bonus action is gated on the monk having a Martial
/// Arts attack available, which proxies the same intent.
///
/// At-will (RAW: 1 ki point per use; we drop the ki pool for symmetry
/// with Patient Defense, the other monk bonus action). The economic
/// payoff is real: spending a bonus action to gain a second main-action
/// swing puts the monk's per-turn damage well ahead of any other PC.
pub struct FlurryOfBlows {}

impl Action for FlurryOfBlows {
    fn name(&self) -> &str {
        "flurry of blows"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fob", "flurry"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Gate on the monk having a Martial Arts action — keeps Flurry
        // out of the dispatcher for non-monk actors that somehow got the
        // template.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.find_action("martial arts").is_some())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  flurry of blows: monk gains an extra Action for a follow-up strike.".to_string());
        grant_extra_action(caster_id)
    }
}

pub static FLURRY_OF_BLOWS: LazyLock<FlurryOfBlows> = LazyLock::new(|| FlurryOfBlows {});

/// Class-feature tag for the Cleric's Divine Strike (5e level-8 RAW;
/// once per long rest in our model). RAW exposes Divine Strike as a
/// passive "+1d8 typed damage on weapon hits" at level 8, but we model
/// it as an explicit bonus-action prime (mirrors the Smiting pattern)
/// so the cleric has a flavorful spike-damage button paired with the
/// Channel Divinity: Turn Undead lane.
pub const DIVINE_STRIKE_TAG: &str = "cleric.divine_strike";

/// Class-feature tag for **Potent Spellcasting** (5e cleric subclass
/// level 8 — Knowledge, Light and Nature Domains all get the same
/// text): "you add your Wisdom modifier to the damage you deal with any
/// cleric cantrip."
///
/// A pure passive with no charge and no action surface — the whole
/// feature is a flat bonus read at
/// `EncounterInstance::potent_spellcasting_bonus`, the chokepoint that
/// already carries the Evocation Wizard's Empowered Evocation. The two
/// are the same shape on opposite halves of the spell list: INT on
/// levelled evocations, WIS on cantrips.
///
/// The bonus is quiet per cast and large in aggregate. A cleric's
/// Sacred Flame is 2d8 on the CR-0.5 chassis, so +3 from a WIS 16 is
/// roughly a third again on every cantrip, every round, for free and
/// forever — which is exactly why the domains that get it get little
/// else at level 8, and why the Trickery and Tempest domains take a
/// Divine Strike instead.
pub const POTENT_SPELLCASTING_TAG: &str = "cleric.potent_spellcasting";

/// Class-feature tag for the Trickery Domain Cleric's **Divine Strike
/// (poison)** (5e level-8 subclass feature; once per long rest in our
/// model, matching the baseline `DIVINE_STRIKE_TAG` cadence).
///
/// RAW gives every cleric domain a Divine Strike at level 8 and varies
/// only the damage type — radiant for Life / Light / Twilight, thunder
/// for Tempest, cold for Nature, fire for Forge, poison for Trickery.
/// The engine ships the radiant flavor on the baseline cleric chassis
/// and this one on the Trickery domain, both as `PrimeStrike` rows, so
/// a third variant costs a tag, a condition and a rider row rather than
/// a fresh `impl Action`.
///
/// Poison is the sharpest of the eight typings *and* the most
/// situational: it is the damage type more monsters resist or are
/// outright immune to than any other in the engine (every undead and
/// construct, most fiends), which is exactly the trade the Trickery
/// Cleric makes elsewhere — Invoke Duplicity is an advantage engine
/// with no floor, and the strike is a damage rider with no ceiling
/// against the things it does bite.
pub const DIVINE_STRIKE_POISON_TAG: &str = "cleric.divine_strike_poison";

/// Class-feature tag for the Four Elements Monk's **Fangs of the Fire
/// Snake** elemental discipline (5e PHB, Way of the Four Elements
/// level 3). RAW spends 1 ki to wreathe the monk's arms in fire: the
/// unarmed strike gains 10 ft of reach and deals an extra 1d10 fire
/// damage on a hit.
///
/// The engine models the damage half as a `PrimeStrike` row and lets
/// the reach half go — the monk's `MONK_UNARMED_STRIKE` declares its
/// own `reach_tiles`, and the engine has no per-swing reach override
/// lane (the Battle Master's Lunging Attack owns the only one, and it
/// is wired to `LungingAttacking` specifically). What survives is the
/// part that makes the discipline worth a ki point on a chassis whose
/// staple swing is 1d8: a +1d10 rider is more than doubling it.
pub const FANGS_OF_THE_FIRE_SNAKE_TAG: &str = "monk.fangs_of_the_fire_snake";

/// Config-driven "bonus action; spend a per-rest charge to prime the
/// next melee hit with a typed damage rider" class-feature action.
///
/// Three features share the shape and differ only in four values — the
/// display name and aliases, which per-rest tag pays for it, which
/// caster-side condition the `ON_HIT_RIDERS` table keys off, and the
/// log line:
///
///   - **Divine Strike** (`DIVINE_STRIKE`) — baseline cleric, +1d8
///     radiant.
///   - **Divine Strike (poison)** (`DIVINE_STRIKE_POISON`) — Trickery
///     Domain cleric, +1d8 poison.
///   - **Fangs of the Fire Snake** (`FANGS_OF_THE_FIRE_SNAKE`) — Way of
///     the Four Elements monk, +1d10 fire.
///
/// Note what is *not* on this struct: the dice, the damage type, and
/// the melee gate. All three live on the rider row in
/// `ON_HIT_RIDERS`, keyed by `prime`, which is where every other
/// per-hit rider in the engine already declares them. Duplicating them
/// here would create two places for "how much does Divine Strike hit
/// for" to be written down and one of them to drift; the action's job
/// is to install the flag, and the rider table's job is to say what
/// the flag means.
///
/// Sibling in shape to the config-driven `TurnBurst` (Channel Divinity
/// burst) and `ManeuverPrime` (Battle Master save-rider prime) chassis:
/// a fourth variant costs a struct literal, not an `impl Action`.
pub struct PrimeStrike {
    /// Display name — action list entry, prompt parser's canonical
    /// name, and the log line's implicit subject.
    pub name: &'static str,
    /// Alias set for the prompt parser. `&'static [&'static str]` keeps
    /// the struct plain data at `LazyLock` init.
    pub aliases: &'static [&'static str],
    /// Per-rest feature tag that funds the prime. Read by
    /// `feature_prime_ready` on the validate side; spent by
    /// `prime_self_condition`.
    pub tag: &'static str,
    /// Caster-side flag installed on use. The matching `ON_HIT_RIDERS`
    /// row carries the dice, the damage type and the melee gate.
    pub prime: Condition,
    /// Log line emitted on use, verbatim.
    pub log_line: &'static str,
}

impl Action for PrimeStrike {
    fn name(&self) -> &str {
        self.name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Indirect: the rider lands on the next hit, not on cast.
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_prime_ready(encounter, caster_id, self.tag, self.prime)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            self.tag,
            self.prime,
            // Two rounds is the shared prime envelope: long enough to
            // survive a turn spent closing to reach, short enough that
            // an idle holder can't bank the charge across an encounter.
            ConditionTimer::Rounds(2),
            self.log_line,
        )
    }
}

/// Divine Strike — Cleric feature, bonus action. Spends the once-per-rest
/// feature to prime the cleric's next melee hit with +1d8 radiant
/// damage (consumed at the hit site in `resolve_attack` via the
/// OnHitRider table — see the `DivineStriking` rider entry). Tick-down
/// timer caps the prime to 2 rounds so an idle cleric doesn't carry
/// the prime across rests.
pub static DIVINE_STRIKE: LazyLock<PrimeStrike> = LazyLock::new(|| PrimeStrike {
    name: "divine strike",
    aliases: &["dstrike", "cd-strike"],
    tag: DIVINE_STRIKE_TAG,
    prime: Condition::DivineStriking,
    log_line: "  divine strike: cleric's next melee hit will land with radiant fury.",
});

/// Divine Strike (poison) — Trickery Domain Cleric level-8 subclass
/// feature, bonus action. Same envelope as the radiant baseline; the
/// `DivineStrikingPoison` rider row swaps 1d8 radiant for 1d8 poison.
///
/// The three-token canonical name is deliberate. The prompt parser
/// resolves the longest token-prefix that names an action *before*
/// falling back to shorter ones, so `divine strike poison` reaches
/// this action even on a template that also carried the two-token
/// `divine strike` — and no Trickery cleric does, since the domain
/// swaps rather than stacks.
pub static DIVINE_STRIKE_POISON: LazyLock<PrimeStrike> = LazyLock::new(|| PrimeStrike {
    name: "divine strike poison",
    aliases: &["pstrike", "cd-poison"],
    tag: DIVINE_STRIKE_POISON_TAG,
    prime: Condition::DivineStrikingPoison,
    log_line: "  divine strike (poison): cleric's next melee hit will land envenomed.",
});

/// Fangs of the Fire Snake — Way of the Four Elements Monk elemental
/// discipline, bonus action. Primes the monk's next melee hit with
/// +1d10 fire damage (the `FangsOfTheFireSnake` rider row).
///
/// The largest single die on the whole `ON_HIT_RIDERS` table, and it
/// sits on the chassis with the *smallest* base weapon die — a monk's
/// unarmed strike is 1d8, so the discipline more than doubles a
/// connecting swing. That is the Four Elements trade in miniature: the
/// subclass buys big numbers with a resource (ki, modeled as slots)
/// that the rest of the monk kit never needed.
/// Water Whip — 5e **Way of the Four Elements Monk** elemental
/// discipline (PHB). A bonus action and 2 ki: a whip of water lashes a
/// creature within 30 ft, which makes a Dexterity save against the
/// monk's ki save DC. On a fail it takes 3d10 bludgeoning and is
/// knocked prone; on a save, half damage and it stays on its feet.
///
/// The discipline that most repays the ki, and the only one on the
/// subclass that does two things at once. Prone is the strongest
/// single rider in the engine's control vocabulary — it hands every
/// melee ally advantage against the target *and* costs the target its
/// movement getting up — and this is the only way any monk template
/// reaches it. Stunning Strike, the chassis's other lockdown, has to
/// land a swing first; Water Whip's damage arrives whether the save
/// lands or not.
///
/// **Ki is spelled as a level-2 spell slot**, the same currency the
/// Shadow Monk's Shadow Arts uses, and the exchange rate is RAW's own:
/// 2 ki, one slot level 2. That keeps the Four Elements monk's whole
/// discipline budget — this plus the elemental spell list — in a
/// single pool the engine already knows how to spend, rather than a
/// per-rest charge that would make the subclass's signature button a
/// once-a-fight event.
///
/// **The DC is the monk's WIS**, per RAW's ki save DC (8 + prof + WIS)
/// — the same anchor the chassis's Stunning Strike already uses, so a
/// Four Elements monk invests in exactly one stat for both halves of
/// its kit.
///
/// RAW's alternative rider — pull the target up to 25 ft toward the
/// monk instead of proning it — is not offered. The engine has a
/// forced-pull lane (Thorn Whip uses it), but a per-cast choice
/// between two riders needs an override the action surface doesn't
/// carry, and prone is the stronger half on a chassis built to stand
/// next to what it hits.
pub struct WaterWhip {}

impl Action for WaterWhip {
    fn name(&self) -> &str {
        "water whip"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["whip", "ww"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft RAW = 12 tiles on the 2.5 ft grid.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // 2 ki, spelled as one level-2 slot — see the type doc.
        bonus_action_and_slot(2)
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
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Ki save DC — 8 + prof + WIS, the monk's spellcasting anchor
        // and the same one Stunning Strike reads.
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let (dmg, passed) = crate::actions::spells::save_for_half_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(3, 10),
            DamageType::Bludgeoning,
            "water whip",
        );
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Bludgeoning,
            }));
        }
        // Prone rides the failed save only, and independently of the
        // damage: a target that resists everything the 3d10 could do
        // (bludgeoning immunity) still goes down if it failed the save,
        // because RAW's trigger is the save, not the damage.
        if !passed {
            encounter.log("  water whip: the lash sweeps the target off its feet.".to_string());
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

pub static WATER_WHIP: LazyLock<WaterWhip> = LazyLock::new(|| WaterWhip {});

pub static FANGS_OF_THE_FIRE_SNAKE: LazyLock<PrimeStrike> = LazyLock::new(|| PrimeStrike {
    name: "fangs of the fire snake",
    aliases: &["fangs", "firesnake"],
    tag: FANGS_OF_THE_FIRE_SNAKE_TAG,
    prime: Condition::FangsOfTheFireSnake,
    log_line: "  fangs of the fire snake: the monk's arms wreathe in flame.",
});

/// Class-feature tag for the Fighter's Trip Attack Battle Master
/// maneuver (once per long rest). RAW exposes maneuvers as a pool of
/// superiority dice; we collapse to a single charge per rest so the
/// once-per-rest gating pattern stays uniform with Second Wind / Action
/// Surge / Indomitable.
pub const TRIP_ATTACK_TAG: &str = "fighter.trip_attack";

/// Shared "bonus-action prime → next melee hit rides a save-vs-condition
/// (or accuracy / splash / reach) rider" shape for Battle Master
/// maneuvers and any other class feature whose only surface is a
/// self-installed priming condition. Every field ships as a `&'static`
/// so the type can be a `pub const` (matching how `CreateSpellSlot` /
/// `ConvertSpellSlot` are declared elsewhere in this file).
///
/// Collapses the ~60-line `impl Action for XxxAttack {}` block that
/// nine Battle Master primes previously open-coded — every one wrote
/// the same trait body (NoArgs / bonus-action-only / feature-prime-
/// gated / self-condition-install) with only the tag / condition /
/// timer / log line changing. Sibling to the `SpellSlotRecovery` shape
/// (which collapses Arcane Recovery + Natural Recovery through the
/// same channel) — one row per maneuver instead of one impl block per
/// maneuver.
///
/// The shared `feature_prime_ready` + `prime_self_condition` helpers
/// already carried the gate + install logic — this struct just gives
/// them a shape that the `Action` trait can hang off. Adding a future
/// Battle Master pickup (Riposte, Parry, Bait and Switch, etc.) that
/// installs a caster-side prime lands as one `pub const` row here
/// instead of a fresh 60-line trait impl.
pub struct ManeuverPrime {
    /// Display name — surfaces in the action list + log line prefix and
    /// as the prompt parser's canonical entry (`Action::name`).
    pub name: &'static str,
    /// Alias set for the prompt parser (`Action::aliases`). Kept as a
    /// `&'static [&'static str]` so the struct stays plain data at
    /// LazyLock init.
    pub aliases: &'static [&'static str],
    /// Feature tag whose once-per-rest charge gates the prime. Read via
    /// `feature_available` in `custom_validate_input`; spent via
    /// `spend_feature` when the prime installs.
    pub tag: &'static str,
    /// The self-condition this prime installs on the caster. Read on
    /// both the validate side (`feature_prime_ready`'s no-stack clause
    /// bounces a duplicate cast while the prime is still up) and the
    /// install side (`prime_self_condition` runs an `ApplyCondition`).
    pub prime_condition: Condition,
    /// Duration of the installed prime. RAW's per-maneuver "until end of
    /// your next turn" varies (Rounds(2) covers the typical prime; the
    /// Distracting Strike case uses `UntilStartOfNextTurn` to match
    /// RAW's until-end-of-target's-next-turn window on the ally-side
    /// advantage rider).
    pub timer: ConditionTimer,
    /// Log flavor line emitted by `prime_self_condition` when the prime
    /// installs — surfaces in the combat log so the player can see
    /// exactly which prime is up.
    pub log_line: &'static str,
}

impl Action for ManeuverPrime {
    fn name(&self) -> &str {
        self.name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        // The prime targets the caster, not an enemy — mirrors how
        // every other self-installed prime (Sacred Weapon, Divine Smite,
        // Stunning Strike) declares itself non-harmful. The eventual
        // damage / debuff lands on the *consuming* swing, which routes
        // through the weapon's `is_harmful` gate.
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_prime_ready(encounter, caster_id, self.tag, self.prime_condition)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            self.tag,
            self.prime_condition,
            self.timer,
            self.log_line,
        )
    }
}

/// Trip Attack — Fighter Battle Master maneuver. Bonus action; primes
/// the next melee weapon hit: on connect, the target makes a STR save
/// vs the fighter's maneuver DC (8 + prof + STR); on fail, they're
/// knocked Prone. RAW's superiority-die damage rider is skipped; the
/// prone-on-fail half is the load-bearing tactical effect. One-shot
/// — the OnHitRider table strips the prime the moment a melee swing
/// lands. Tick-down timer (2 rounds) caps the prime if the fighter
/// can't connect. Backed by the shared `ManeuverPrime` shape.
pub static TRIP_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "trip attack",
    aliases: &["trip", "ta"],
    tag: TRIP_ATTACK_TAG,
    prime_condition: Condition::TripAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  trip attack: fighter's next hit forces a STR save vs prone.",
});

/// Build the "+1 Action token" side-effect vector for bonus-action
/// economy-trade actions: `Action Surge`, `Flurry of Blows`, `Frenzy`,
/// and `Quickened Spell` all spend a bonus action (or feature charge,
/// or SP pool) and hand the caster a fresh Action to spend on a
/// follow-up attack / spell this turn. The literal
///
/// ```ignore
/// vec![Box::new(GiveResource {
///     actor_id: caster_id,
///     resource: Resource::Action,
/// })]
/// ```
///
/// fires from four different action sites; folding it behind one
/// helper keeps the per-action `side_effects` block shorter and gives
/// us a single chokepoint if the action-token grant ever needs an
/// engine-side hook (e.g. a future "extra Action provokes opportunity
/// attacks" rule). Public so the `metamagic` module can share the same
/// helper from outside this file.
pub fn grant_extra_action(caster_id: usize) -> Vec<Box<dyn ApplicableSideEffect>> {
    vec![Box::new(GiveResource {
        actor_id: caster_id,
        resource: Resource::Action,
    })]
}

/// Combined "spend a once-per-rest feature charge, log a flavor line,
/// and grant an extra Action token" side-effect builder. Action Surge
/// (Fighter) and War Priest (War Domain Cleric) share the same shape:
/// burn the per-rest charge, log the flavor line, and hand back the
/// single-entry `GiveResource(Action)` vector.
///
/// Sibling to `grant_extra_action` (the raw grant) — this wrapper adds
/// the spend + log layer for per-rest gated variants. Distinct from
/// Flurry of Blows / Frenzy (at-will / Rage-gated grants that don't
/// spend a feature charge) — those still call `grant_extra_action`
/// directly. Adding a future per-rest extra-Action feature drops in as
/// a one-liner instead of the three-line `if let Some(actor) →
/// spend_feature → encounter.log → grant_extra_action` boilerplate.
fn spend_feature_and_grant_extra_action(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    tag: &'static str,
    log_line: &'static str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    if let Some(actor) = encounter.actors.get_mut(&caster_id) {
        actor.spend_feature(tag);
    }
    encounter.log(log_line.to_string());
    grant_extra_action(caster_id)
}

/// Spend a once-per-rest feature charge and install a self-applied prime
/// condition on the caster. The classic "bonus-action prime" shape:
/// `spend_feature(tag)` then `ApplyCondition` for `prime`. Centralizes
/// the pattern repeated by every Battle Master maneuver, every Smite
/// prime, Sacred Weapon, Divine Strike, Stunning Strike, etc. Logs the
/// flavor line so the caller doesn't have to repeat the format string.
///
/// Returns the side-effect vector the action site should hand back to
/// the engine — typically just the one `ApplyCondition`. Note: this is
/// for *self-only* primes; targeted effects (Cutting Words, Bardic
/// Inspiration) still inline their own logic since they apply to an
/// ally / enemy id, not the caster.
fn prime_self_condition(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    feature_tag: &'static str,
    prime: Condition,
    timer: ConditionTimer,
    log_line: &str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    if let Some(actor) = encounter.actors.get_mut(&caster_id) {
        actor.spend_feature(feature_tag);
    }
    encounter.log(log_line.to_string());
    vec![Box::new(ApplyCondition {
        actor_id: caster_id,
        condition: prime,
        timer,
    })]
}

/// Class-feature tag for the Fighter's Menacing Attack Battle Master
/// maneuver (once per long rest in our model). RAW: a pool of superiority
/// dice; we collapse to a single charge per rest so the gating stays
/// uniform with Trip Attack / Second Wind / Indomitable.
pub const MENACING_ATTACK_TAG: &str = "fighter.menacing_attack";

/// Menacing Attack — Fighter Battle Master maneuver. Bonus action; primes
/// the next melee weapon hit: on connect, the target makes a WIS save
/// vs the fighter's STR-based maneuver DC; on fail, they're Frightened
/// until the end of the fighter's next turn. RAW's +1d8 superiority-die
/// damage is skipped (same caveat as Trip Attack); the frighten-on-fail
/// IS the load-bearing tactical effect. Backed by the shared
/// `ManeuverPrime` shape.
pub static MENACING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "menacing attack",
    aliases: &["menace", "ma"],
    tag: MENACING_ATTACK_TAG,
    prime_condition: Condition::MenacingAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  menacing attack: fighter's next hit forces a WIS save vs frighten.",
});

/// Class-feature tag for the Fighter's Disarming Attack Battle Master
/// maneuver (once per long rest in our model).
pub const DISARMING_ATTACK_TAG: &str = "fighter.disarming_attack";

/// Disarming Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit: on connect, the target makes a
/// STR save vs the fighter's STR-based maneuver DC; on fail, they're
/// Disarmed — attack rolls have disadvantage until the start of their
/// next turn. RAW: target drops their weapon; we collapse the
/// pickup-takes-a-move clause into the `UntilStartOfNextTurn` timer
/// since the engine doesn't track held items. Backed by the shared
/// `ManeuverPrime` shape.
pub static DISARMING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "disarming attack",
    aliases: &["disarm", "da"],
    tag: DISARMING_ATTACK_TAG,
    prime_condition: Condition::DisarmingAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  disarming attack: fighter's next hit forces a STR save vs disarm.",
});

/// Class-feature tag for the Fighter's Pushing Attack Battle Master
/// maneuver (once per long rest in our model).
pub const PUSHING_ATTACK_TAG: &str = "fighter.pushing_attack";

/// Pushing Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit: on connect, the target makes a
/// STR save vs the fighter's STR-based maneuver DC; on fail, they're
/// shoved 15 ft (4 tiles in our 2.5ft grid) away from the fighter via
/// the standard `PushActor` helper. RAW's +1d8 superiority-die damage
/// is skipped (same caveat as the other maneuvers); the displacement
/// IS the load-bearing tactical effect. First maneuver to use the
/// `Push` variant of `FollowUpEffect`. Backed by the shared
/// `ManeuverPrime` shape.
pub static PUSHING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "pushing attack",
    aliases: &["pa", "shovea"],
    tag: PUSHING_ATTACK_TAG,
    prime_condition: Condition::PushingAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  pushing attack: fighter's next hit forces a STR save vs shove.",
});

/// Class-feature tag for the Fighter's Goading Attack Battle Master
/// maneuver (once per long rest in our model). Refreshes on a short
/// rest via the `BATTLE_MASTER_MANEUVERS` registry above.
pub const GOADING_ATTACK_TAG: &str = "fighter.goading_attack";

/// Goading Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit: on connect, the target makes a
/// WIS save vs the fighter's STR-based maneuver DC; on fail, they're
/// Goaded — attack rolls against anyone other than the fighter are at
/// disadvantage until the start of their next turn. RAW's +1d8
/// superiority-die damage is skipped (same caveat as the other
/// maneuvers); the goad debuff IS the load-bearing tactical effect.
/// Mirrors Compelled Duel's tank-anchor envelope but is per-rest
/// rather than concentration-bound. Backed by the shared
/// `ManeuverPrime` shape.
pub static GOADING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "goading attack",
    aliases: &["goad", "ga"],
    tag: GOADING_ATTACK_TAG,
    prime_condition: Condition::GoadingAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  goading attack: fighter's next hit forces a WIS save vs goad.",
});

/// Class-feature tag for the Fighter's Distracting Strike Battle Master
/// maneuver (once per long rest in our model). Refreshes on a short rest
/// via the `BATTLE_MASTER_MANEUVERS` registry.
pub const DISTRACTING_ATTACK_TAG: &str = "fighter.distracting_attack";

/// Distracting Strike — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit: on connect, the target takes
/// +1d6 bonus damage (the superiority die) and is tagged Distracted —
/// the next attack roll against them by an attacker *other* than the
/// fighter has advantage until the end of the fighter's next turn.
/// RAW's superiority-die damage IS the load-bearing damage rider
/// (no save needed — the target-side advantage rider lands
/// unconditionally on hit). Mirrors Goading Attack's prime + target-
/// link pairing, but flipped to a target-side advantage rather than
/// an attacker-side disadvantage. One-shot — the OnHitRider table
/// strips this flag the moment a melee swing lands. Backed by the
/// shared `ManeuverPrime` shape; uses `UntilStartOfNextTurn` for the
/// timer to match RAW's until-end-of-target's-next-turn envelope on
/// the ally-side advantage rider.
pub static DISTRACTING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "distracting strike",
    aliases: &["distract", "dsa"],
    tag: DISTRACTING_ATTACK_TAG,
    prime_condition: Condition::DistractingAttacking,
    timer: ConditionTimer::UntilStartOfNextTurn,
    log_line: "  distracting strike: fighter's next melee hit will rattle the target's guard.",
});

/// Class-feature tag for the Fighter's **Parry** Battle Master maneuver
/// (once per short rest in our model, matching the shared superiority-
/// dice pool). RAW: when another creature damages you with a melee
/// attack, you can spend one superiority die as a reaction to reduce
/// that damage by `1d8 + DEX modifier`. Unlike the bonus-action primes
/// (Trip / Menacing / Disarming / Pushing / etc.), Parry has no active
/// action to spend on your own turn — the fighter carries the passive
/// `has_parry` flag on their template, and the resolve-attack chokepoint
/// spends this charge automatically when a hit lands and the reaction
/// is available. Refreshes on short rest via `BATTLE_MASTER_MANEUVERS`
/// (which is spliced into `SHORT_REST_FEATURES`).
pub const PARRY_TAG: &str = "fighter.parry";

/// Class-feature tag for the Fighter's **Riposte** Battle Master maneuver
/// (once per short rest in our model, matching the shared superiority-
/// dice pool). RAW: when a creature misses you with a melee attack, you
/// can spend one superiority die as a reaction to make a melee weapon
/// attack against them. Same passive/auto-fire shape as `PARRY_TAG` —
/// the fighter carries the `has_riposte` flag on their template, and
/// the resolve-attack chokepoint's miss branch spends this charge
/// automatically when the reaction is available and a melee weapon
/// action is on the fighter's action list. Refreshes on short rest via
/// `BATTLE_MASTER_MANEUVERS`.
pub const RIPOSTE_TAG: &str = "fighter.riposte";

/// Class-feature tag for the Eldritch Knight Fighter's **Weapon Bond**
/// (subclass level 3). RAW: the knight performs a ritual binding one
/// weapon to themselves; while it is bonded they "can't be disarmed of
/// that weapon unless [they are] incapacitated," and they can summon it
/// to hand as a bonus action.
///
/// The summoning half has no engine surface — the engine tracks no
/// weapon location, so a weapon is never anywhere but in its wielder's
/// hands. The disarm half does: `Condition::Disarmed` is a live debuff
/// installed by the Battle Master's Disarming Attack maneuver (and any
/// future disarm source), so Weapon Bond reads as a conditional
/// immunity to it.
///
/// The conditionality is the whole feature — an unconditional immunity
/// would be a strictly better `condition_immunities` entry and would
/// lose RAW's escape hatch. It rides `dynamic_immunity_to` next to
/// Halfling Brave (Frightened) and Fey Ancestry (Charmed / Asleep),
/// which is the chokepoint that already evaluates immunity against live
/// actor state rather than a static template set. The "unless
/// incapacitated" carve-out maps onto `is_incapacitated`, so a
/// Stunned / Paralyzed / Unconscious knight can still be disarmed —
/// exactly RAW.
pub const WEAPON_BOND_TAG: &str = "fighter.weapon_bond";

/// Class-feature tag for the Eldritch Knight Fighter's **War Magic**
/// (subclass level 7). RAW: "When you use your action to cast a
/// cantrip, you can make one weapon attack as a bonus action."
///
/// Passive — there is no charge to spend and no rest cadence. The tag
/// gates the post-cast hook `trigger_war_magic_prime`, which installs
/// `Condition::WarMagicPrimed` on the knight whenever they finish
/// casting a cantrip (`spell_level == 0`). The `WAR_MAGIC_STRIKE`
/// bonus action below reads the prime and hands back the swing.
///
/// The prime's `UntilStartOfNextTurn` timer is what enforces RAW's
/// same-turn window: it clears at the start of the holder's *next*
/// turn, so a cantrip cast on turn N can only be cashed in on turn N's
/// bonus action, and an uncashed prime never lingers into turn N+1's
/// action economy.
pub const WAR_MAGIC_TAG: &str = "fighter.war_magic";

/// War Magic — Eldritch Knight bonus action, at-will, gated on having
/// cast a cantrip earlier this turn (`Condition::WarMagicPrimed`).
///
/// RAW hands back "one weapon attack"; we hand back one **Action**,
/// which the knight then spends on a weapon action. That is the same
/// collapse `FlurryOfBlows` and `WarPriest` already make, and for the
/// same reason: the engine's action economy has no "make exactly one
/// weapon attack right now" primitive, and building one for a single
/// feature would duplicate the whole attack-targeting pipeline. The
/// deviation is that a granted Action can technically be spent on a
/// second cantrip instead of a swing — which costs the knight the swing
/// they wanted and cannot loop, since re-priming still needs a bonus
/// action the turn has already spent.
///
/// The cantrip-first ordering is what makes the feature a real tempo
/// decision rather than free damage: the knight's Action is committed
/// to a cantrip *before* the prime exists, so taking War Magic means
/// giving up the Attack action (and with it Extra Attack's second
/// swing) in exchange for a cantrip plus one swing.
pub struct WarMagicStrike {}

impl Action for WarMagicStrike {
    fn name(&self) -> &str {
        "war magic"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wm", "warmagic"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
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
                && a.has_passive_feature(WAR_MAGIC_TAG)
                && a.has_condition(Condition::WarMagicPrimed)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Burn the prime immediately so a knight who somehow reaches a
        // second bonus action in a turn can't cash the same cantrip
        // twice. The `UntilStartOfNextTurn` timer would eventually do
        // this anyway; the explicit strip makes the one-shot contract
        // local to the feature rather than dependent on the tick order.
        if let Some(knight) = encounter.actors.get_mut(&caster_id) {
            knight.remove_condition(Condition::WarMagicPrimed);
        }
        encounter.log(
            "  war magic: the knight's cantrip flows into a follow-up weapon swing.".to_string(),
        );
        grant_extra_action(caster_id)
    }
}

pub static WAR_MAGIC_STRIKE: LazyLock<WarMagicStrike> = LazyLock::new(|| WarMagicStrike {});

/// Class-feature tag for the Eldritch Knight Fighter's **Eldritch
/// Strike** (subclass level 10). RAW: "When you hit a creature with a
/// weapon attack, that creature has disadvantage on the next saving
/// throw it makes against a spell you cast before the end of your next
/// turn."
///
/// Passive, no charge, no rest cadence — the tag is read at the
/// weapon-hit chokepoint in `resolve_attack_outcome`, which stamps
/// `Condition::EldritchStruck` plus the `EldritchStruck` back-link
/// onto the target. The rider is cashed by the shared
/// `CASTER_SAVE_MODE_RIDERS` cohort in `roll_save_against_caster`.
///
/// The feature is what makes the Eldritch Knight's two halves one
/// build rather than two: a fighter who only swings never notices it,
/// and a fighter who only casts never arms it. Landing a swing and
/// *then* casting is the line, which is also the line War Magic wants
/// reversed (cantrip first, swing second) — the subclass rewards
/// alternating rather than committing, across turns.
pub const ELDRITCH_STRIKE_TAG: &str = "fighter.eldritch_strike";

/// Class-feature tag for the Arcane Trickster Rogue's **Magical
/// Ambush** (subclass level 9). RAW: "If you are hidden from a creature
/// when you cast a spell on it, the creature has disadvantage on any
/// saving throw it makes against the spell this turn."
///
/// Passive, no charge — read at the shared `CASTER_SAVE_MODE_RIDERS`
/// cohort in `roll_save_against_caster`, gated on the trickster holding
/// `Condition::Hidden` at cast time.
///
/// Two RAW clauses collapse. "Hidden *from that creature*" becomes
/// "Hidden" flat: the engine's `Hidden` is a global concealment flag
/// with no per-observer ledger, the same simplification every other
/// Hidden consumer (`grants_self_attack_advantage`, the Assassin's
/// opening-round advantage) already makes. And "any saving throw ...
/// this turn" becomes the first one, because the cohort consumes
/// `Hidden` on the trigger — casting a spell at someone gives your
/// position away just as surely as shooting at them does, which is
/// exactly why `CONSUMED_ON_ATTACK` already drops `Hidden` on an attack
/// roll. Both collapses cut the same direction: toward the rogue
/// spending their concealment rather than holding it.
pub const MAGICAL_AMBUSH_TAG: &str = "rogue.magical_ambush";

/// Class-feature tag for the Arcane Trickster Rogue's **Versatile
/// Trickster** (subclass level 13). RAW: "you can use a bonus action on
/// your turn to designate a creature within 5 feet of the [mage] hand.
/// Doing so gives you advantage on attack rolls against that creature
/// until the end of the turn."
///
/// At-will, no charge — the resource is the bonus action, which the
/// rogue's Cunning Action lane already competes hard for. That
/// competition is the feature: spending it here means not spending it
/// on Hide, which is what arms Magical Ambush on the same build.
///
/// The mage hand is elided. The engine has no summoned-object layer, so
/// modelling it would mean tracking a second entity purely to gate a
/// range check that the action's own `reach_tiles` already expresses.
/// RAW's reach is "within 5 ft of the hand" and the hand is cast within
/// 30 ft, so the action ships at the 30 ft (12-tile) envelope — the
/// outer bound of where the pair could legally reach, and the same
/// range Rally and Bardic Inspiration use.
pub const VERSATILE_TRICKSTER_TAG: &str = "rogue.versatile_trickster";

/// Versatile Trickster — Arcane Trickster bonus action, at-will, one
/// enemy within 30 ft. Grants the rogue advantage on their next attack
/// roll against that target.
///
/// Mechanically the twin of the Battle Master's Feinting Attack: both
/// install a self-help-grant (`HelpGrant { helper_id: self, against:
/// target }`) that `attack_mode_with_riders` folds in and consumes on
/// the next swing. The two differ on cost rather than effect — Feinting
/// Attack spends a per-rest maneuver charge and reaches only as far as
/// the fighter can swing, where this is at-will at 30 ft.
///
/// Which is the right trade for the chassis it sits on. The rogue's
/// Sneak Attack already keys off advantage, so a rogue with a reliable
/// advantage button doesn't need a flanking ally — the same problem
/// Steady Aim solves by zeroing the rogue's speed and Rakish Audacity
/// solves by requiring a solo duel. Versatile Trickster solves it
/// without either restriction, which is why RAW gates it to lv13 and
/// why it competes with Hide for the bonus action rather than stacking
/// with it.
pub struct VersatileTrickster {}

impl Action for VersatileTrickster {
    fn name(&self) -> &str {
        "versatile trickster"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["vt", "trickster"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles — the mage hand's own cast range, which is
        // the outer bound of where the hand-plus-rogue pair could
        // legally designate a target.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.has_passive_feature(VERSATILE_TRICKSTER_TAG) {
            return false;
        }
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() != actor.team() && target.is_combat_active()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            // Self-help-grant — identical shape to Feinting Attack's.
            // `attack_mode_with_riders` reads it and `consume_help_for`
            // pops it on the next swing, so the advantage lands once
            // and doesn't leak onto a follow-up attack.
            actor.set_help_grant(Some(crate::actors::actor_template::HelpGrant {
                helper_id: caster_id,
                against: target_id,
            }));
        }
        encounter.log(
            "  versatile trickster: the spectral hand jabs; advantage on the next attack vs the target."
                .to_string(),
        );
        Vec::new()
    }
}

pub static VERSATILE_TRICKSTER: LazyLock<VersatileTrickster> =
    LazyLock::new(|| VersatileTrickster {});

/// Class-feature tag for the Wizard's Arcane Recovery — once per long
/// rest, refreshes on long rest. RAW: once per day during a short rest,
/// recover spell slots whose combined levels equal half the wizard's
/// level (rounded up), with no slot above 5th. We collapse that pool
/// into a fixed-shape recovery (one level-1 + one level-2 slot for any
/// level-3+ wizard, only one level-1 slot below that) so the gate stays
/// a single feature-flag check rather than a slot picker. Listed in the
/// `SHORT_REST_FEATURES` registry so a short rest can re-enable the
/// feature; the long rest already enables it via the default refresh.
pub const ARCANE_RECOVERY_TAG: &str = "wizard.arcane_recovery";

/// Shared once-per-rest spell-slot recovery shape. Both the Wizard's
/// **Arcane Recovery** and the Druid Circle of the Land's **Natural
/// Recovery** collapse RAW's "recover slots totaling ceil(level/2), no
/// slot above 5th" pool into the same flat per-tier envelope: one
/// level-1 slot at any caster level, plus one level-2 slot at level 3+.
/// The two features are mechanically identical — only the tag, name,
/// aliases, and log flavor differ — so both fold through this shared
/// struct rather than duplicating the ~100-line validate + effects
/// bodies. Adding a future "recover a level-N slot on short rest"
/// feature (Sorcerer's Font of Magic recovery variants, etc.) lands as
/// a fresh `SpellSlotRecovery` const with a new tag and no touching
/// the Action impl body.
pub struct SpellSlotRecovery {
    /// Display name — surfaces in the action list, log lines, and the
    /// prompt parser (`Action::name`).
    pub name: &'static str,
    /// Alias set for the prompt parser (`Action::aliases`). Kept as a
    /// `&'static [&'static str]` so the struct stays a plain-data
    /// literal at LazyLock init.
    pub aliases: &'static [&'static str],
    /// Feature tag whose once-per-rest charge gates this recovery.
    /// Reads via `feature_available` in `custom_validate_input`;
    /// spent via `spend_feature` when the recovery fires.
    pub tag: &'static str,
}

impl Action for SpellSlotRecovery {
    fn name(&self) -> &str {
        self.name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // No resource cost — RAW the recovery itself is free during a
        // short rest. The once-per-rest gate lives on the feature flag.
        free_cost()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Gate on the feature flag, combat-active state, AND at least
        // one missing spell slot in the level-1 or level-2 tier —
        // recovering a slot you didn't spend is a no-op, and gating
        // here keeps the AI from burning the feature on empty.
        encounter.actors.get(&caster_id).is_some_and(|a| {
            if !a.is_combat_active() || !a.feature_available(self.tag) {
                return false;
            }
            let l1 = a.spell_slot_manager.spell_slots(1);
            let l2 = a.spell_slot_manager.spell_slots(2);
            l1.spell_slots < l1.max_spell_slots || l2.spell_slots < l2.max_spell_slots
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let (level, want_l1, want_l2) = {
            let Some(a) = encounter.actors.get(&caster_id) else {
                return Vec::new();
            };
            let l1 = a.spell_slot_manager.spell_slots(1);
            let l2 = a.spell_slot_manager.spell_slots(2);
            (
                a.level(),
                l1.spell_slots < l1.max_spell_slots,
                l2.spell_slots < l2.max_spell_slots,
            )
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(self.tag);
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if want_l1 {
            effects.push(Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::SpellSlot(1),
            }));
        }
        // RAW: pool of slot-levels equal to ceil(level/2), no slot
        // above 5th. We hand out the level-2 slot only at level 3+
        // (where a ceil(3/2)=2 pool can afford it) and only if a slot
        // was spent.
        if want_l2 && level >= 3 {
            effects.push(Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::SpellSlot(2),
            }));
        }
        encounter.log(format!(
            "  {}: {} restores {} spell slot{}.",
            self.name,
            encounter.actor_name(caster_id),
            effects.len(),
            if effects.len() == 1 { "" } else { "s" },
        ));
        effects
    }
}

/// Arcane Recovery — Wizard feature. Spend the once-per-rest charge
/// to restore one level-1 spell slot (plus a level-2 slot if the
/// wizard is at least level 3 and has a level-2 slot to restore).
/// Centralizes the RAW "half-level pool, no slot above 5th" math into
/// a flat per-tier shape; lets the wizard keep firing low-tier control
/// spells (Magic Missile / Shield / Web) across encounters without a
/// full long rest. Backed by the shared `SpellSlotRecovery` shape.
pub static ARCANE_RECOVERY: LazyLock<SpellSlotRecovery> = LazyLock::new(|| SpellSlotRecovery {
    name: "arcane recovery",
    aliases: &["ar", "recover"],
    tag: ARCANE_RECOVERY_TAG,
});

/// Class-feature tag for the Druid Circle of the Land's **Natural
/// Recovery** (level 2). RAW: once per day during a short rest, recover
/// spell slots whose combined levels equal half the druid's level
/// (rounded up), with no slot above 5th. We collapse that pool into a
/// fixed-shape recovery — one level-1 slot at any level, plus one
/// level-2 slot at level 3+ — mirroring the Arcane Recovery shape so
/// the gate stays a single feature-flag check rather than a slot
/// picker. Listed in `SHORT_REST_FEATURES` so a short rest re-enables
/// the once-per-rest charge alongside Arcane Recovery.
///
/// Distinct from Arcane Recovery only in flavor and template placement:
/// the recovery math is identical (RAW pool: ceil(level/2) slot-levels,
/// no slot above 5th). Ships on a new LAND_DRUID_TEMPLATE — the
/// baseline DRUID_TEMPLATE stays feature-free so the Circle of the
/// Land subclass reads unambiguously.
pub const NATURAL_RECOVERY_TAG: &str = "druid.natural_recovery";

/// Natural Recovery — Druid Circle of the Land feature. Free-cost
/// (no spell slot / no action-lane burn beyond the free-action tick),
/// gated on the once-per-rest feature flag. Restores one level-1
/// spell slot plus a level-2 slot at level 3+, matching the Arcane
/// Recovery envelope for engine uniformity — both features fold
/// through the shared `SpellSlotRecovery` shape. Fires the recovery
/// even when the druid isn't in combat — the RAW gate is "during a
/// short rest," which our engine collapses to the free-cost action
/// lane (mirrors Arcane Recovery's out-of-combat usage pattern).
pub static NATURAL_RECOVERY: LazyLock<SpellSlotRecovery> = LazyLock::new(|| SpellSlotRecovery {
    name: "natural recovery",
    aliases: &["nr", "natural"],
    tag: NATURAL_RECOVERY_TAG,
});

/// Class-feature tag for Cleric Channel Divinity: Preserve Life — once
/// per short or long rest. Shares the "Channel Divinity" RAW lane with
/// Turn Undead at the cleric level, but they're distinct features in our
/// model (each tag is a separate once-per-rest charge) so we don't have
/// to plumb a shared resource pool. Listed in `SHORT_REST_FEATURES`.
pub const PRESERVE_LIFE_TAG: &str = "cleric.preserve_life";

/// Channel Divinity: Preserve Life — Cleric action. Distribute a pool of
/// 5 * cleric level HP across wounded allies within 30ft (12 tiles),
/// healing each up to half their maximum HP. We collapse the RAW per-
/// target allocation choice into a deterministic algorithm: sort wounded
/// allies (including self) by HP fraction ascending so the most-hurt
/// allies get healed first, and bring each up to 50% max HP (or as close
/// as the remaining pool allows). Once per rest (short or long).
pub struct PreserveLife {}

impl Action for PreserveLife {
    fn name(&self) -> &str {
        "preserve life"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pl", "cd-life", "preserve"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
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
        feature_ready(encounter, caster_id, PRESERVE_LIFE_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(PRESERVE_LIFE_TAG);
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let level = caster.level();
        let mut pool = 5 * level;
        let caster_team = caster.team();
        let caster_loc = caster.location();
        let caster_size = get_tiles_from_size(caster.size());

        // Sort by HP fraction ascending so the most-wounded ally heals
        // first. Sorted-by-id within the same fraction keeps the choice
        // deterministic across runs with the same RNG seed (HashMap
        // iteration would otherwise shuffle ties).
        let mut candidates: Vec<(u32, u32, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if a.team() != caster_team || !a.is_combat_active() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    caster_loc,
                    caster_size,
                );
                if dist > 12 {
                    return None;
                }
                let cap = a.max_hitpoints();
                if a.hitpoints() >= cap / 2 + (cap % 2) {
                    // Already at >= 50% max HP — RAW: feature can't heal
                    // a creature whose HP is at half or more.
                    return None;
                }
                // HP fraction scaled to a sortable u32 — multiply by max
                // so a wholly arbitrary `cap` size doesn't dominate.
                let frac = (a.hitpoints().saturating_mul(1_000_000)) / cap.max(1);
                Some((frac, *id as u32, *id))
            })
            .collect();
        candidates.sort_unstable();

        encounter.log(format!(
            "  preserve life: cleric distributes {} HP among wounded allies.",
            pool
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for (_frac, _id_sort, target_id) in candidates {
            if pool == 0 {
                break;
            }
            let Some(t) = encounter.actors.get(&target_id) else {
                continue;
            };
            let cap = t.max_hitpoints();
            let half = cap / 2 + (cap % 2);
            let needed = half.saturating_sub(t.hitpoints());
            if needed == 0 {
                continue;
            }
            let amount = needed.min(pool);
            pool -= amount;
            effects.push(Box::new(Heal {
                actor_id: target_id,
                amount,
            }));
        }
        effects
    }
}

pub static PRESERVE_LIFE: LazyLock<PreserveLife> = LazyLock::new(|| PreserveLife {});

/// Class-feature tag for the Bard's Cutting Words — once per short rest
/// (RAW: spends one Bardic Inspiration use). We collapse the
/// inspiration-die pool to a single per-rest charge to keep the gating
/// uniform with the other once-per-rest features; the recovery happens
/// via `SHORT_REST_FEATURES`.
pub const CUTTING_WORDS_TAG: &str = "bard.cutting_words";

/// Cutting Words — Bard bonus action (RAW reaction; collapsed to bonus
/// action because the engine doesn't yet have a reactive cast-on-attack
/// hook). Targets one enemy within 60ft and applies the `Mocked`
/// condition: their next attack roll has disadvantage. We approximate
/// the RAW "subtract a Bardic Inspiration die from an attack roll,
/// ability check, or damage roll" with the disadvantage clause, which
/// captures the load-bearing tactical effect (the bard caging an enemy
/// strike before it lands).
pub struct CuttingWords {}

impl Action for CuttingWords {
    fn name(&self) -> &str {
        "cutting words"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cw-bard", "cut"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles. Matches Bardic Inspiration's range.
        Some(24)
    }
    fn is_harmful(&self) -> bool {
        true
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Same shared gate as Nature's Wrath / Intimidating Presence /
        // Abjure Enemy / Path to the Grave — once-per-rest charge +
        // hostile-target + already-holds-Mocked dedup all fold into
        // `hostile_target_feature_ready`. Mocked dedup keeps the bard
        // from burning a bonus action on a target whose disadvantage
        // rider is still live from an earlier quip.
        hostile_target_feature_ready(
            encounter,
            caster_id,
            target_ids,
            CUTTING_WORDS_TAG,
            Condition::Mocked,
        )
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(CUTTING_WORDS_TAG);
        }
        encounter.log("  cutting words: bard quips, fouling the target's strike.".to_string());
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Mocked,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static CUTTING_WORDS: LazyLock<CuttingWords> = LazyLock::new(|| CuttingWords {});

/// Class-feature tag for the Fighter's Precision Attack Battle Master
/// maneuver (once per long rest in our model). Listed in
/// `SHORT_REST_FEATURES` so a short rest refreshes the charge alongside
/// every other maneuver tag.
pub const PRECISION_ATTACK_TAG: &str = "fighter.precision_attack";

/// Precision Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next attack roll with a flat +4 (modeling the +1d8
/// superiority die, d8 avg rounded down). RAW lets the fighter spend
/// the die *after* seeing the d20 result; we apply the bonus
/// proactively so the AI can use it as a "the next swing better land"
/// accuracy buff. The +4 is read at every attack-roll site via
/// `condition_attack_bonus`; the prime is consumed by
/// `clear_attack_advantage_riders` the moment the swing resolves,
/// mirroring Bardic Inspiration's single-shot lane.
///
/// Unlike Trip / Menacing / Disarming / Pushing / Goading, this
/// maneuver has no melee-only or save gate — it lands on any attack
/// roll the fighter makes that turn (RAW: weapon attack roll, melee or
/// ranged). One-shot via the clear-on-attack consume site. Backed by
/// the shared `ManeuverPrime` shape.
pub static PRECISION_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "precision attack",
    aliases: &["precision", "pra"],
    tag: PRECISION_ATTACK_TAG,
    prime_condition: Condition::PrecisionAttacking,
    // 2-round window so the prime survives until the fighter's next
    // swing even if their turn ends on a movement-only sequence
    // (mirrors Trip / Menacing / Smite primes).
    timer: ConditionTimer::Rounds(2),
    log_line: "  precision attack: fighter's next attack roll gains +4.",
});

/// Class-feature tag for the Fighter's Sweeping Attack Battle Master
/// maneuver (once per long rest in our model). Listed in
/// `SHORT_REST_FEATURES` so a short rest refreshes the charge.
pub const SWEEPING_ATTACK_TAG: &str = "fighter.sweeping_attack";

/// Sweeping Attack — Fighter Battle Master maneuver. Bonus action;
/// primes the next melee weapon hit to splash 1d8 slashing damage onto
/// one footprint-adjacent enemy of the primary target (via the
/// `SweepingAttacking` condition + `FollowUpEffect::Splash` rider).
/// RAW: damage matches the original weapon's type; we collapse to a
/// flat 1d8 slashing since the on-hit rider table doesn't carry
/// per-weapon typing into the splash. The splash auto-applies on hit
/// (no save) — RAW: the original attack roll is reused.
///
/// One-shot — the rider table strips the `SweepingAttacking` flag the
/// moment a melee swing lands. Backed by the shared `ManeuverPrime`
/// shape.
pub static SWEEPING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "sweeping attack",
    aliases: &["sweep", "swa"],
    tag: SWEEPING_ATTACK_TAG,
    prime_condition: Condition::SweepingAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  sweeping attack: fighter's next melee hit splashes to an adjacent foe.",
});

/// Class-feature tag for the Fighter's Feinting Attack Battle Master
/// maneuver (once per long rest in our model). Listed in
/// `SHORT_REST_FEATURES` so a short rest refreshes the charge.
pub const FEINTING_ATTACK_TAG: &str = "fighter.feinting_attack";

/// Feinting Attack — Fighter Battle Master maneuver. Bonus action
/// targeting one enemy within melee reach. The fighter gains advantage
/// on the next attack roll against the chosen creature this turn.
///
/// Modeled by installing a self-help-grant via `set_help_grant`: the
/// helper is the fighter themselves, the designated target is the
/// feinted enemy. `attack_mode_with_riders` reads the grant on the
/// next swing against that target, folding in advantage and consuming
/// the grant. Mirrors how Help's lane works but the fighter targets
/// themselves rather than a separate ally.
///
/// Unlike the prime-style maneuvers (Trip / Menacing / Sweeping), this
/// doesn't go through the OnHitRider table — the entire effect is the
/// pre-roll advantage, which lands cleanly through the help-grant
/// machinery already wired into `compute_attack_mode`.
pub struct FeintingAttack {}

impl Action for FeintingAttack {
    fn name(&self) -> &str {
        "feinting attack"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["feint", "fa"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — same envelope as Help / a melee weapon swing.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        // The action targets an enemy but doesn't itself deal damage or
        // apply a condition — it's a setup buff for the *next* swing.
        // Marking it harmful would route it through the AI's hostile
        // pipeline (good) but also block it via Sanctuary / Charmed-by
        // gating (we want Feint to behave like a Help / Hide setup —
        // unblocked by Sanctuary on the feinter's side). Leaving false
        // mirrors how Help's lane handles the targeting.
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.feature_available(FEINTING_ATTACK_TAG) {
            return false;
        }
        // Target must be a live enemy (RAW: any creature, but feinting a
        // friendly is a wasted bonus action — the AI's hostile pipeline
        // is the consumer).
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() != actor.team() && target.is_combat_active()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(FEINTING_ATTACK_TAG);
            // Self-help-grant: the fighter helps themselves get
            // advantage on the next attack vs the feinted target. Read
            // by `attack_mode_with_riders` which folds in
            // `consume_help_for` on the next swing.
            actor.set_help_grant(Some(crate::actors::actor_template::HelpGrant {
                helper_id: caster_id,
                against: target_id,
            }));
        }
        encounter.log(
            "  feinting attack: fighter feints; advantage on the next attack vs the target."
                .to_string(),
        );
        Vec::new()
    }
}

pub static FEINTING_ATTACK: LazyLock<FeintingAttack> = LazyLock::new(|| FeintingAttack {});

/// Class-feature tag for the Fighter's Lunging Attack Battle Master
/// maneuver (once per long rest in our model). Listed in
/// `SHORT_REST_FEATURES` so a short rest refreshes the charge.
pub const LUNGING_ATTACK_TAG: &str = "fighter.lunging_attack";

/// Lunging Attack — Fighter Battle Master maneuver. Bonus action prime;
/// the next melee weapon attack gains +5 ft of reach (one tile in this
/// engine's 2.5ft grid). Drives the `LungingAttacking` condition, which
/// `Action::validate_input` consults via `ActorInstance::extra_melee_reach`
/// to extend the action's reach check. The prime is consumed by
/// `CONSUMED_ON_ATTACK` on the next attack the fighter makes.
///
/// One-shot — mirrors the Trip / Menacing / Sweeping shape but with the
/// reach extension as the load-bearing effect instead of a save-or-debuff.
/// Backed by the shared `ManeuverPrime` shape.
pub static LUNGING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "lunging attack",
    aliases: &["lunge", "la"],
    tag: LUNGING_ATTACK_TAG,
    prime_condition: Condition::LungingAttacking,
    // Short timer caps the prime so an idle fighter doesn't carry the
    // reach extension across rests. CONSUMED_ON_ATTACK clears it on
    // the next swing; the timer is just a safety net.
    timer: ConditionTimer::Rounds(2),
    log_line: "  lunging attack: fighter's next melee swing gains +5 ft of reach.",
});

/// Class-feature tag for the Fighter's Rally Battle Master maneuver
/// (once per long rest in our model). Listed in `SHORT_REST_FEATURES`
/// so a short rest refreshes the charge.
pub const RALLY_TAG: &str = "fighter.rally";

/// Rally — Fighter Battle Master maneuver. Bonus action targeting one
/// ally within 30 ft (12 tiles). The chosen ally gains
/// `1d10 + CHA modifier` temp HP. RAW: superiority die scaling
/// (d8 → d12 by level); we use a flat 1d10 since the engine's per-class
/// scaling pool doesn't carry into this lane. Minimum +0 on the CHA
/// floor — a CHA-dump fighter still hands out d10 worth of buffer.
///
/// Non-priming maneuver — the effect is the immediate temp HP grant
/// rather than an OnHitRider prime. Distinct from Second Wind / Lay on
/// Hands because the target is an *ally* and the buffer is temp HP
/// rather than healing.
pub struct Rally {}

impl Action for Rally {
    fn name(&self) -> &str {
        "rally"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rl"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles. Matches Bardic Inspiration / Healing Word's
        // medium-range buff envelope.
        Some(12)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        // Temp HP is the rally's buffer — slot it into the AI's support
        // pipeline alongside other healing taps so the fighter rallies
        // a low-HP ally rather than handing the buff to a full-HP one.
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.feature_available(RALLY_TAG) {
            return false;
        }
        // Ally-only — RAW: "you can choose a friendly creature who can
        // see or hear you". The AI's support pipeline picks live allies
        // already; we just enforce the team-match gate here so a
        // misqueued enemy-target invocation doesn't slip through.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() == actor.team() && target.is_combat_active()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let raw = encounter.roll(&Dice::new(1, 10));
        let cha_mod = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.ability_modifier(AbilityScoreType::Charisma))
            .unwrap_or(0);
        // RAW: temp HP can't drop below the roll itself, so floor the
        // CHA contribution at 0 (a -1 CHA fighter still hands out d10).
        let amount = (raw as i32 + cha_mod).max(raw as i32) as u32;
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(RALLY_TAG);
        }
        encounter.log(format!(
            "  rally: 1d10({}){:+} = {} temp HP",
            raw, cha_mod, amount
        ));
        vec![Box::new(GainTempHp {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static RALLY: LazyLock<Rally> = LazyLock::new(|| Rally {});

/// Class-feature tag for the Fighter's Commander's Strike Battle Master
/// maneuver (once per long rest in our model). Listed in
/// `SHORT_REST_FEATURES` so a short rest refreshes the charge.
pub const COMMANDERS_STRIKE_TAG: &str = "fighter.commanders_strike";

/// Commander's Strike — Fighter Battle Master maneuver. Bonus action
/// targeting one ally within 60 ft (24 tiles). The fighter directs the
/// ally to make one weapon attack with advantage, right now, as a
/// reaction.
///
/// Two engine lanes together: a help-grant against the ally's target so
/// `attack_mode_with_riders` folds advantage into the roll, and
/// `try_fire_directed_attack` to actually take the swing. The grant is
/// installed first so the swing it fires reads it.
///
/// It used to be the grant plus a `GiveResource(Reaction)` and nothing
/// else, which meant the maneuver's headline clause — "the creature
/// makes one weapon attack" — depended on the ally later finding an
/// opportunity attack to spend that reaction on. Usually they didn't,
/// and a bonus action and a per-rest charge bought a stray reaction and
/// some idle advantage. The fresh reaction still goes in: RAW hands the
/// ally the reaction, and if the geometry refuses the swing they should
/// at least keep it.
pub struct CommandersStrike {}

impl Action for CommandersStrike {
    fn name(&self) -> &str {
        "commander's strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["command", "cs"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles. The fighter shouts orders across the
        // battlefield; LOS isn't required by RAW so we skip the LOS
        // check (`requires_los` defaults to false).
        Some(24)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // The ally's swing deals damage, but the command itself doesn't.
        // Mirrors how Feinting / Help register in the support pipeline.
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !actor.is_combat_active() || !actor.feature_available(COMMANDERS_STRIKE_TAG) {
            return false;
        }
        // Ally-only — the target must be on the fighter's team and live.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(ally) = encounter.actors.get(&target_id) else {
            return false;
        };
        if ally.team() != actor.team() || !ally.is_combat_active() || target_id == caster_id {
            return false;
        }
        // Also need a hostile enemy on the map for the ally to attack —
        // otherwise the order has no recipient. We find one inside the
        // side_effects body so the validation here just checks the ally
        // is in a position to act.
        true
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::actor_template::HelpGrant;
        let Some(ally_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Pick the closest visible enemy of the ally as the strike target.
        // The fighter doesn't get to pick — RAW: "the creature uses its
        // reaction to make one weapon attack" against a target the
        // commander chooses, but the engine has no per-action target
        // picker for ally-driven reactions, so we auto-target the nearest
        // enemy to the ally.
        let ally_team = match encounter.actors.get(&ally_id) {
            Some(a) => a.team(),
            None => return Vec::new(),
        };
        let strike_target = encounter
            .actors
            .iter()
            .filter(|(_, a)| a.team() != ally_team && a.is_combat_active())
            .min_by_key(|(id, _)| {
                encounter
                    .footprint_distance(ally_id, **id)
                    .unwrap_or(isize::MAX)
            })
            .map(|(id, _)| *id);
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(COMMANDERS_STRIKE_TAG);
        }
        encounter.log(format!(
            "  commander's strike: fighter directs ally for a reaction attack{}.",
            if strike_target.is_some() {
                ""
            } else {
                " (no visible enemy; reaction granted but advantage idle)"
            }
        ));
        // Install the help-grant on the ally — advantage on next swing
        // against the chosen enemy (if any). Before the reaction is
        // handed over, so the directed swing below rolls with it.
        if let Some(target_id) = strike_target
            && let Some(ally) = encounter.actors.get_mut(&ally_id)
        {
            ally.set_help_grant(Some(HelpGrant {
                helper_id: caster_id,
                against: target_id,
            }));
        }
        // RAW hands the ally a reaction and has them spend it on one
        // weapon attack immediately. Grant it in place rather than
        // through the returned `GiveResource`, because the swing has to
        // see it — side-effects apply after this builder returns.
        if let Some(ally) = encounter.actors.get_mut(&ally_id) {
            ally.give_resource(Resource::Reaction);
        }
        if !crate::engine::attack::try_fire_directed_attack(
            encounter,
            ally_id,
            caster_id,
            "command",
        ) {
            // Nothing in the ally's reach, or no melee weapon to swing.
            // The reaction stays granted — RAW gave it to them, and the
            // geometry is what refused the swing.
            encounter.log(
                "  commander's strike: the ally has nothing in reach; the reaction stands."
                    .to_string(),
            );
        }
        Vec::new()
    }
}

pub static COMMANDERS_STRIKE: LazyLock<CommandersStrike> = LazyLock::new(|| CommandersStrike {});

/// Tag for the Tiefling Infernal Legacy: Hellish Rebuke racial trait.
/// Once per long rest the tiefling fires a CHA-based Hellish Rebuke
/// without spending a spell slot — RAW: "Starting at 3rd level, you
/// can cast hellish rebuke as a 2nd-level spell once with this trait
/// and regain the ability to do so when you finish a long rest." We
/// gate on a feature flag and refresh on long rest like Lay on Hands /
/// Relentless Endurance.
pub const INFERNAL_LEGACY_REBUKE_TAG: &str = "tiefling.infernal_legacy_rebuke";

/// Shared "spend the once-per-rest feature charge on the caster, then
/// return the caster's save DC anchored on `spellcasting_ability`" step.
/// Returns `None` if the caster is gone (defensive early-out for the
/// three save-driven CD resolvers below) — callers short-circuit on
/// `None` and skip the save / damage / condition install.
///
/// Centralizes the (spend feature → look up DC) preamble that
/// `resolve_turn_burst`, `resolve_single_target_cd_save_condition`, and
/// `resolve_single_target_burst_save_for_half` all previously open-
/// coded. Uses a single `get_mut` for the spend and a single `get`
/// for the DC — the two lookups can't collapse to one because
/// `spend_feature` mutates but `spell_save_dc` reads, and the borrow
/// checker won't accept an overlapping mut+imm pair. The important
/// invariant this helper enforces is *ordering*: the charge is spent
/// BEFORE the DC read, so a target that saves still pays the once-per-
/// rest cost — RAW for all three of Turn Undead, Intimidating Presence,
/// Wrath of the Storm, and their siblings.
///
/// Sibling to `feature_ready` / `hostile_target_feature_ready` on the
/// class-feature-plumbing lane: those two check the charge is *there*
/// (gate), this one spends the charge and gets the DC (effect).
fn spend_feature_and_get_dc(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    feature_tag: &'static str,
    spellcasting_ability: AbilityScoreType,
) -> Option<i32> {
    if let Some(actor) = encounter.actors.get_mut(&caster_id) {
        actor.spend_feature(feature_tag);
    }
    encounter
        .actors
        .get(&caster_id)
        .map(|c| c.spell_save_dc(spellcasting_ability))
}

/// Shared "spend a once-per-rest feature charge, roll target save vs the
/// caster's spellcasting-ability DC, deal Xdy damage of type T with
/// save-for-half" body for single-target class-feature damage bursts.
/// Sibling to `resolve_single_target_cd_save_condition` on the class-
/// feature family — that helper installs a *condition* on a failed save,
/// this one deals *damage* with a save-for-half fold. Both spend the
/// feature charge before the save so the once-per-rest cost is paid
/// whether the target passes or fails.
///
/// The save DC is derived from the caster's `spellcasting_ability`
/// (CHA for tieflings, WIS for clerics), the target rolls with
/// `save_ability`, and on the save-for-half fold the damage lands via
/// `save_for_half_damage` — half on save, full on fail. `DealDamage`
/// then routes through the normal target-side resistance / immunity /
/// vulnerability pipeline so a fire-immune target still bounces the
/// tiefling's Infernal Rebuke cleanly per RAW.
///
/// Parameters mirror `resolve_single_target_cd_save_condition` where the
/// shape overlaps:
///   - `caster_id` — spends `feature_tag` before the save.
///   - `target_id` — the single target of the save.
///   - `spellcasting_ability` — anchor for the save DC (8 + prof + mod).
///   - `save_ability` — the ability the target rolls the save with.
///   - `dice` — the damage die pool rolled once, halved on save.
///   - `damage_type` — the damage type of the burst.
///   - `label` — log prefix ("infernal rebuke" / "wrath of the storm").
///
/// Adding a future single-target save-for-half class-feature damage
/// burst (a hypothetical "Radiant Rebuke" that deals 2d10 radiant via a
/// CON-DC save, an Aasimar racial variant, etc.) lands as a fresh call
/// with a distinct (save_ability, dice, damage_type) tuple — no re-
/// implementation of the spend-tag / roll-DC / roll-damage / save-for-
/// half dance.
#[allow(clippy::too_many_arguments)]
fn resolve_single_target_burst_save_for_half(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    feature_tag: &'static str,
    spellcasting_ability: AbilityScoreType,
    save_ability: AbilityScoreType,
    dice: Dice,
    damage_type: DamageType,
    label: &'static str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(dc) = spend_feature_and_get_dc(
        encounter,
        caster_id,
        feature_tag,
        spellcasting_ability,
    ) else {
        return Vec::new();
    };
    let (dmg, _) = crate::actions::spells::save_for_half_damage(
        encounter,
        caster_id,
        target_id,
        save_ability,
        dc,
        dice,
        damage_type,
        label,
    );
    if dmg == 0 {
        return Vec::new();
    }
    vec![Box::new(DealDamage {
        actor_id: target_id,
        amount: dmg,
        damage_type,
    })]
}

/// Tiefling Infernal Legacy — Hellish Rebuke (racial flavor). One swing
/// per long rest of a CHA-based DEX-save burst that deals 3d10 fire
/// (RAW: "cast hellish rebuke as a 2nd-level spell"). No slot cost —
/// the once-per-rest feature gate is the load-bearing resource.
///
/// Action cost rather than the RAW reaction cost — the engine doesn't
/// have a clean "reactive on being damaged" hook for player-driven
/// actions, and the action cost keeps the racial useful even when the
/// tiefling hasn't been hit yet.
///
/// Routes through the shared
/// `resolve_single_target_burst_save_for_half` helper — the
/// spend-tag / roll-DC / roll-damage / save-for-half dance lives in one
/// place, and mirrors Wrath of the Storm (Tempest Cleric CD lv1) on the
/// single-target burst lane with a distinct (save-ability, dice,
/// damage_type) tuple.
pub struct InfernalLegacyRebuke {}

impl Action for InfernalLegacyRebuke {
    fn name(&self) -> &str {
        "infernal rebuke"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["infernal", "ireb", "tiefling-rebuke"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, INFERNAL_LEGACY_REBUKE_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        resolve_single_target_burst_save_for_half(
            encounter,
            caster_id,
            target_id,
            INFERNAL_LEGACY_REBUKE_TAG,
            // Tiefling Infernal Legacy is CHA-anchored per RAW.
            AbilityScoreType::Charisma,
            // Hellish Rebuke uses a DEX save RAW.
            AbilityScoreType::Dexterity,
            // 3d10 fire — matches Hellish Rebuke cast at level 2 (RAW).
            Dice::new(3, 10),
            DamageType::Fire,
            "infernal rebuke",
        )
    }
}

pub static INFERNAL_LEGACY_REBUKE: LazyLock<InfernalLegacyRebuke> =
    LazyLock::new(|| InfernalLegacyRebuke {});

/// Tag for the Dragonborn Breath Weapon racial trait. Once per short
/// rest (RAW: "Once you use your breath weapon, you can't use it again
/// until you complete a short or long rest"). Listed in the
/// `SHORT_REST_FEATURES` registry so a short rest refreshes the charge
/// alongside the other once-per-rest features.
pub const BREATH_WEAPON_TAG: &str = "dragonborn.breath_weapon";

/// Dragonborn Breath Weapon — racial action. 2-tile burst from the
/// dragonborn's footprint, DEX save vs the dragonborn's CON-based DC
/// (8 + prof + CON), 2d6 of the draconic ancestor's damage type, half
/// on save. Once per short rest. Scales by character level (3d6 at
/// L6, 4d6 at L11, 5d6 at L16).
///
/// The damage type is sourced from the actor's `draconic_ancestry()` —
/// `None` means the actor has no ancestor and the action falls back to
/// fire (defensive default; the action is gated to dragonborn templates
/// in `custom_validate_input` so the fallback should never fire in
/// gameplay).
pub struct BreathWeapon {}

impl Action for BreathWeapon {
    fn name(&self) -> &str {
        "breath weapon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["breath", "bw"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 5e RAW: 15-ft cone (or 5x30-ft line). We collapse to a
        // 2-tile burst — same envelope as Burning Hands — so the
        // AI's `try_attack_aoe` lane picks it up alongside the
        // sorcerer / wizard cone spells.
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 15-ft cone RAW — we approximate as a 2-tile burst targeted
        // anywhere within ~6 tiles (the cone's reach).
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        // Surfaced for the UI / AI heuristics. The actual type is
        // resolved at cast time from `draconic_ancestry`; we report a
        // representative list rather than a single guess, since
        // dragonborn variants pick different ancestries.
        vec![
            DamageType::Fire,
            DamageType::Cold,
            DamageType::Lightning,
            DamageType::Acid,
            DamageType::Poison,
        ]
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
                && a.feature_available(BREATH_WEAPON_TAG)
                && a.draconic_ancestry().is_some()
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // RAW: Save DC = 8 + proficiency + CON modifier. Matches the
        // standard spell-save-DC formula with CON as the casting
        // ability — `spell_save_dc` reuses the same arithmetic.
        let dc = caster.spell_save_dc(AbilityScoreType::Constitution);
        let damage_type = caster.draconic_ancestry().unwrap_or(DamageType::Fire);
        // RAW scaling: 2d6 at L1, 3d6 at L6, 4d6 at L11, 5d6 at L16.
        // We use 1 + level/5 (clamped at 1) which yields 2d6 / 3d6 / 4d6
        // / 5d6 at the listed breakpoints.
        let dice_count = (1 + caster.level() / 5).max(1);
        if let Some(c) = encounter.actors.get_mut(&caster_id) {
            c.spend_feature(BREATH_WEAPON_TAG);
        }
        let raw = encounter.roll(&Dice::new(dice_count, 6));
        encounter.log(format!(
            "  breath weapon: {}d6({}) = {} {} cone",
            dice_count, raw, raw, damage_type
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            damage_type,
        )
    }
}

pub static BREATH_WEAPON: LazyLock<BreathWeapon> = LazyLock::new(|| BreathWeapon {});

/// Warlock Eldritch Invocation — **Agonizing Blast**. Passive feature:
/// when the holder casts Eldritch Blast, they add their Charisma
/// modifier to the damage of each beam (RAW: "When you cast Eldritch
/// Blast, add your Charisma modifier to the damage it deals on a hit").
/// Read at the EldritchBlast cast site via `feature_available` (the
/// invocation never gets spent — it's permanent, but the same gate is
/// the cleanest hook). Add this tag to a warlock template's `features`
/// set to install it.
pub const AGONIZING_BLAST_TAG: &str = "warlock.agonizing_blast";

/// Warlock Eldritch Invocation — **Repelling Blast**. Passive feature:
/// when the holder hits a Large or smaller creature with Eldritch Blast,
/// they can push the creature up to 10 feet (4 tiles in our 2.5ft grid)
/// away in a straight line (RAW). We approximate the size gate as
/// "Large or smaller" by skipping the push on Huge / Gargantuan
/// targets — those creatures are too massive for the cantrip's force.
/// Read at the EldritchBlast cast site via `feature_available`. Add this
/// tag to a warlock template's `features` set to install it.
pub const REPELLING_BLAST_TAG: &str = "warlock.repelling_blast";

/// Warlock Eldritch Invocation — **Eldritch Mind**. Passive feature: the
/// holder rolls with advantage on Constitution saving throws to maintain
/// concentration. Read at the concentration save chokepoint
/// (`EncounterInstance::roll_concentration_save`) which the DealDamage
/// pipeline drives whenever a concentrating actor takes damage and lives.
/// Permanent passive — never consumed. Add this tag to a warlock
/// template's `features` set to install it.
pub const ELDRITCH_MIND_TAG: &str = "warlock.eldritch_mind";

/// 5e Warlock — Otherworldly Patron **The Fiend**, level-1 feature
/// **Dark One's Blessing**. Passive: whenever the warlock reduces a
/// hostile creature to 0 HP, they gain temporary hit points equal to
/// their Charisma modifier + warlock level (min 1). RAW: "When you
/// reduce a hostile creature to 0 hit points, you gain temporary hit
/// points equal to your Charisma modifier + your warlock level (a
/// minimum of 1)."
///
/// Read at the `DealDamage::apply` chokepoint on the `Downed` /
/// `Killed` outcome branches: the current turn actor is looked up
/// via `EncounterInstance::current_turn_actor_id`, and if they hold
/// this tag AND the dropped target is on a different team (RAW
/// "hostile"), the temp HP is granted through the standard
/// `GainTempHp` side effect so the max-of-current-and-new stack rule
/// still holds. Attributing the "reduced-to-0 hit" to the current
/// turn actor sidesteps threading an attacker id through every damage
/// path (reactive burns, ongoing DoT, condition drips) — RAW's plain
/// reading is that the warlock is the one landing the killing hit,
/// and out-of-turn triggers (Hellish Rebuke fired on someone else's
/// turn) don't reward the wrong warlock.
///
/// Passive with no per-rest charge — fires every time the trigger
/// condition holds. Add this tag to a warlock template's `features`
/// set to install it. Not shipped on the baseline
/// `WARLOCK_TEMPLATE` (Patron is a subclass pick); rides on the
/// dedicated `FIEND_WARLOCK_TEMPLATE`.
///
/// Sibling to `TOUCH_OF_DEATH_TAG` (Long Death Monk lv3) on the shared
/// **kill-triggered temp HP** cohort in
/// `EncounterInstance::pay_kill_triggered_temp_hp` — same
/// "reduce hostile to 0 HP → grant self temp HP" pattern with a
/// different stat + level formula and a different class chassis. Both
/// rows fold through the same `KILL_TRIGGERED_TEMP_HP_SOURCES` table so
/// adding a new kill-triggered temp HP feature is a one-line row entry
/// rather than a fresh open-coded trigger function.
pub const DARK_ONES_BLESSING_TAG: &str = "warlock.dark_ones_blessing";

/// 5e Monk — Way of the Long Death, level-3 subclass feature
/// **Touch of Death**. Passive: whenever the monk reduces a hostile
/// creature to 0 HP, they gain temporary hit points equal to
/// `1 + CON mod + monk level` (min 1). RAW: "Starting when you choose
/// this tradition at 3rd level, your study of death allows you to
/// wring extra life force from a foe. Immediately after you reduce a
/// creature within 5 feet of you to 0 hit points, you gain temporary
/// hit points equal to 1 + your Constitution modifier + your monk
/// level (minimum of 1)."
///
/// Read at the `DealDamage::apply` chokepoint on the `Downed` /
/// `Killed` outcome branches via the shared
/// `KILL_TRIGGERED_TEMP_HP_SOURCES` cohort — the current turn actor
/// is looked up, and if they hold this tag AND the dropped target is
/// on a different team (RAW "hostile"), the temp HP is granted through
/// the standard `GainTempHp` side effect so the max-of-current-and-new
/// stack rule still holds.
///
/// **Range clause**: RAW gates the trigger to a target "within 5 feet
/// of you" — the monk's touch is the killing blow's channel, so the
/// grant only fires on melee kills. We drop the 5ft gate for the same
/// reason `DARK_ONES_BLESSING_TAG` drops any range gate — the current-
/// turn-actor attribution already anchors the trigger to whoever's
/// swinging, and cantrip / ranged kills from a monk are rare enough
/// that broadening the trigger is a wash (matching the same design
/// call the code base's other RAW-clause simplifications make, e.g.
/// dropping the extreme-heat / extreme-cold clauses on Storm Soul).
///
/// Sibling to `DARK_ONES_BLESSING_TAG` (Warlock Fiend Patron lv1) on
/// the shared kill-triggered temp HP cohort — same pattern, different
/// stat + level formula (Warlock: CHA mod + warlock level; Monk: 1 +
/// CON mod + monk level) and different chassis. Add this tag to a
/// monk template's `features` set to install it. Not shipped on the
/// baseline `MONK_TEMPLATE` (Way is a subclass pick); rides on the
/// dedicated `LONG_DEATH_MONK_TEMPLATE`.
pub const TOUCH_OF_DEATH_TAG: &str = "monk.touch_of_death";

/// 5e Warlock — Otherworldly Patron **The Great Old One**, level-6
/// subclass feature **Entropic Ward**. Reactive per-rest gate: when a
/// creature makes an attack roll against the warlock, the warlock may
/// spend their reaction + the once-per-short-rest charge to impose
/// disadvantage on that attack roll. RAW: "As a reaction when a
/// creature makes an attack roll against you, you can impose
/// disadvantage on that roll. If the attack misses you, your next
/// attack roll against the creature has advantage if you make it
/// before the end of your next turn. Once you use this feature, you
/// can't use it again until you finish a short or long rest."
///
/// Sibling on the shared **reactive per-rest disadvantage on an
/// incoming attack roll** lane with Warding Flare (Light Cleric
/// lv1); both flow through the shared
/// `REACTIVE_ATTACK_DISADVANTAGE_SOURCES` cohort at the attack
/// chokepoint via `EncounterInstance::apply_reactive_attack_disadvantage`.
/// Distinct in shape from Bend Luck (Wild Magic Sorcerer lv6, cost
/// 2 SP + reaction, subtracts 1d4 from the attacker's total instead
/// of imposing disadvantage) and from Uncanny Dodge (Rogue lv5,
/// halves damage instead of imposing disadvantage).
///
/// **Range**: RAW gates only on "a creature makes an attack roll
/// against you" — no 30ft "must be within" cap the way Warding Flare
/// carries. The patron's telepathic tie reaches anywhere on the
/// battlefield; the ward reads the attacker's intent regardless of
/// distance. Encoded on the cohort row as `range_tiles: None`
/// (vs. `Some(12)` for Warding Flare's 30ft cap).
///
/// **Sight gate**: RAW does NOT require the warlock to see the
/// attacker — "the ward hums against your skin when hostile intent
/// stirs, even from beyond your sight". We elide the
/// `viewer_can_see` gate on this row (vs. Warding Flare which
/// requires it) via the `requires_sight: false` flag on the cohort
/// row. Rides the shared cohort helper uniformly with its sight-gated
/// siblings so the two features stay one iterator apart, not two
/// forks in the attack-resolution code.
///
/// **The bonus advantage-on-miss half is left as future work.** RAW's
/// "if the attack misses you, your next attack roll against the
/// creature has advantage" is a target-side one-shot rider tied to a
/// specific attacker id, which would need a new `EntropicWardAdvantage`
/// condition or a `pending_advantage_against_id` slot on the warlock.
/// The disadvantage-on-incoming half is the load-bearing tell — the
/// bonus rider is left for a future nudge, mirroring how Storm
/// Sorcery's "eruption on cast" half was staged behind Heart of the
/// Storm's resistance clause.
///
/// Fires at the shared attack chokepoint in
/// `EncounterInstance::apply_reactive_attack_disadvantage` (called
/// from `resolve_attack` for weapon attacks and `spell_attack_outcome`
/// for spell attacks). Refreshes via `SHORT_REST_FEATURES` — the tag
/// lands on `features_max` for the holder and `features_remaining`
/// refills on short rest. Add this tag to a warlock template's
/// `features` set to install it (ships on
/// `GREAT_OLD_ONE_WARLOCK_TEMPLATE`).
pub const ENTROPIC_WARD_TAG: &str = "warlock.entropic_ward";

/// 5e Warlock — Otherworldly Patron **The Fiend**, level-6 feature
/// **Dark One's Own Luck**. Auto-fire once-per-short-rest gate: when
/// the holder fails a saving throw (or ability check RAW — save-only
/// in this engine), spend the charge to add 1d10 to the failing
/// total. If the boosted total meets or beats the DC, the save
/// becomes a Pass. RAW: "when you make an ability check or a saving
/// throw, you can use this feature to add a d10 to your roll... You
/// can do so after seeing the initial roll but before any of the
/// roll's effects occur."
///
/// Sibling to Fanatical Focus (Oathbreaker Paladin lv15) on the
/// failed-save recovery lane, but distinct in shape:
///   - **Fanatical Focus** rerolls the d20 with the same modifier.
///   - **Dark One's Own Luck** keeps the d20 and adds 1d10 to the
///     final total. A failed save with a d20 of 3 that Fanatical
///     Focus rerolls to another 3 stays failed; the same save that
///     Dark One's Own Luck fires on picks up +1d10 (avg +5.5),
///     turning a mid-DC miss into a pass more consistently.
///
/// Fires at the shared save chokepoint
/// (`EncounterInstance::roll_save_with_extra_mode`) after the initial
/// d20 lands on a fail but *before* the reroll cohort — RAW gates on
/// "the initial roll", so the +1d10 stacks on the d20 the holder just
/// saw. The reroll cohort still fires if the boosted total still
/// misses (a paladin/warlock multiclass with both DOOL and Fanatical
/// Focus can burn both charges on the same failed save if the d10 +
/// d20 total is still short). Legendary Resistance stays a third
/// layer below both.
///
/// Refreshes via `SHORT_REST_FEATURES` — the tag lands on
/// `features_max` for the holder and `features_remaining` refills
/// on short rest. Add this tag to a warlock template's `features`
/// set to install it (ships on `FIEND_WARLOCK_TEMPLATE`).
pub const DARK_ONES_OWN_LUCK_TAG: &str = "warlock.dark_ones_own_luck";

/// 5e Sorcerer — Sorcerous Origin **Divine Soul** (XGtE), level-1
/// feature **Favored by the Gods**. Auto-fire once-per-short-rest
/// gate: when the holder fails a saving throw, spend the charge to
/// add 2d4 to the failing total. If the boosted total meets or
/// beats the DC, the save becomes a Pass. RAW: "If you fail a
/// saving throw or miss with an attack roll, you can roll 2d4 and
/// add it to the total, possibly changing the outcome... Once you
/// use this feature, you can't use it again until you finish a
/// short or long rest."
///
/// Sibling to Dark One's Own Luck (Fiend Warlock lv6) on the shared
/// `FAILED_SAVE_ADD_DIE_SOURCES` cohort — same "add die to failed
/// save total" shape, differentiated by the die pool:
///   - **Dark One's Own Luck** adds 1d10 (avg +5.5).
///   - **Favored by the Gods** adds 2d4 (avg +5.0, tight variance
///     — 2..=8 vs. 1..=10). The narrower spread makes FBTG more
///     reliable at rescuing mid-DC misses where the +5 is enough
///     but blunts the ceiling on low-d20-roll rescue.
///
/// Distinct in shape from Fanatical Focus (Oathbreaker Paladin
/// lv15) on the failed-save recovery lane: FBTG keeps the d20 and
/// adds dice; Fanatical Focus rerolls the d20 with the same
/// modifier. A hypothetical Divine Soul Sorcerer / Oathbreaker
/// Paladin multiclass burns FBTG first (add-die cohort runs before
/// reroll cohort at the save chokepoint) — matches the RAW gate:
/// FBTG's RAW trigger "before or after making the attack roll or
/// saving throw" fires at initial-roll time, ahead of the reroll
/// lane's "when you fail" trigger.
///
/// RAW also covers the "miss with an attack roll" lane; we narrow
/// to saves for the same reason Dark One's Own Luck (RAW: any
/// ability check or saving throw) narrows to saves — the save
/// chokepoint has a single well-defined reroll / add-die surface;
/// the attack-roll miss surface would need a separate wire in
/// `resolve_attack_outcome`. Same simplification pattern as DOOL.
///
/// Fires at the shared save chokepoint
/// (`EncounterInstance::roll_save_with_extra_mode`) via the shared
/// `FAILED_SAVE_ADD_DIE_SOURCES` cohort loop. Refreshes via
/// `SHORT_REST_FEATURES` — the tag lands on `features_max` for the
/// holder and `features_remaining` refills on short rest. Add this
/// tag to a sorcerer template's `features` set to install it (ships
/// on `DIVINE_SOUL_SORCERER_TEMPLATE`).
pub const FAVORED_BY_THE_GODS_TAG: &str = "sorcerer.favored_by_the_gods";

/// 5e Wild Magic Sorcerer **Tides of Chaos** feature tag. Once per long
/// rest charge — the sorcerer leans into the chaos of their bloodline to
/// gain advantage on their next attack roll, ability check, or saving
/// throw. We honor the attack-roll lane (the highest-leverage in our
/// combat model) via the `TidesOfChaos` condition, which grants
/// `grants_self_attack_advantage` and lives in `CONSUMED_ON_ATTACK`.
/// Tag is checked at the Tides of Chaos action's validate, decremented
/// on use, refilled on long rest.
pub const TIDES_OF_CHAOS_TAG: &str = "sorcerer.tides_of_chaos";

/// 5e Sorcerer **Sorcerous Restoration** feature tag (lv20 capstone).
/// Passive: the sorcerer regains 4 expended sorcery points the first
/// time they finish a short rest after using a metamagic option. We
/// collapse the trigger to "always restore 4 SP on short rest" — our
/// short-rest cadence is rare enough that the once-per-rest cap and
/// the metamagic-trigger gate would barely fire. Read by
/// `ActorInstance::short_rest` via a templated branch (no consume
/// because the feature is passive — the SP-give is the entire effect).
pub const SORCEROUS_RESTORATION_TAG: &str = "sorcerer.sorcerous_restoration";

/// 5e Wild Magic Sorcerer **Bend Luck** feature tag (level 6). Passive
/// reaction: when a creature you can see makes an attack roll against
/// you, you may spend 2 sorcery points + your reaction to subtract a
/// rolled 1d4 from the attack's total. The bend can drop a hit to a
/// miss but never undoes a natural-20 crit (the d20 face is decided
/// before the subtraction, mirroring RAW).
///
/// RAW also covers ability checks and saving throws; we model only the
/// attack-roll lane (the highest-leverage in our combat model) — the
/// save / check lanes have no central save chokepoint that reads
/// passive reaction features yet. The trigger fires automatically when
/// the resource gates pass (mirrors Uncanny Dodge / Halfling Lucky's
/// "no player prompt" shape — the engine commits the SP + reaction the
/// moment the attack would land). See
/// `EncounterInstance::apply_bend_luck_penalty` for the wire site.
pub const BEND_LUCK_TAG: &str = "sorcerer.bend_luck";

/// 5e Wild Magic Sorcerer **Wild Magic Surge** feature tag (level 1).
/// Passive: whenever the sorcerer casts a sorcerer spell of 1st level
/// or higher, the DM can have them roll a d20; on a 1, a Wild Magic
/// Surge fires from the surge table.
///
/// We honor the RAW trigger: every level-1+ cast rolls. The check lives
/// in `EncounterInstance::trigger_wild_magic_surge`, which is called
/// from the cross-cutting `Action::execute` site (the same chokepoint
/// the other "consume on cast" metamagic primes use). The function
/// reads this tag from `features_max` (it's passive, not consumed) and
/// short-circuits when the spell-slot level sniffed off the action's
/// cost is 0 (cantrips / non-spell actions never surge).
///
/// Tag is checked via `ActorInstance::has_passive_feature`. Tag is
/// added to a creature template's `features` set to install it.
pub const WILD_MAGIC_SURGE_TAG: &str = "sorcerer.wild_magic_surge";

/// 5e Wild Magic Sorcerer **Tides of Chaos**. Bonus action; once per
/// long rest, install the `TidesOfChaos` prime → advantage on the next
/// attack roll. RAW also grants advantage on the next ability check or
/// saving throw, but the attack-roll lane is the load-bearing one in
/// our combat model; the prime is consumed at the first swing via the
/// `CONSUMED_ON_ATTACK` cohort. Self-target, no SP cost — the once-per-
/// long-rest gate is the entire resource cost (mirrors Second Wind /
/// Indomitable's rest-charge shape). Pairs naturally with a metamagic
/// prime: the Tides advantage stacks on the same swing the metamagic
/// prime modifies, so a Tides + Empowered + Fire Bolt combo gives both
/// the advantage on the attack roll and the damage reroll.
pub struct TidesOfChaos {}

impl Action for TidesOfChaos {
    fn name(&self) -> &str {
        "tides of chaos"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tides", "toc", "chaos"]
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
        bonus_action_only()
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
        if !actor.is_combat_active() || !actor.feature_available(TIDES_OF_CHAOS_TAG) {
            return false;
        }
        // No-stack on the prime condition: re-priming would just refresh
        // the timer and waste the once-per-long-rest charge.
        !actor.has_condition(Condition::TidesOfChaos)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let line = encounter.actors.get_mut(&caster_id).map(|actor| {
            actor.spend_feature(TIDES_OF_CHAOS_TAG);
            format!("{} embraces the tides of chaos.", actor.name())
        });
        if let Some(line) = line {
            encounter.log(line);
        }
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::TidesOfChaos,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static TIDES_OF_CHAOS: LazyLock<TidesOfChaos> = LazyLock::new(|| TidesOfChaos {});

/// 5e Sorcerer **Font of Magic** — Create Spell Slot. Bonus action; spend
/// `sp_cost` sorcery points to recreate one expended spell slot of the
/// matching level. RAW cost table caps at 5th-level slot:
///
/// |   slot   | SP cost |
/// |  level   |         |
/// |    1     |    2    |
/// |    2     |    3    |
/// |    3     |    5    |
/// |    4     |    6    |
/// |    5     |    7    |
///
/// We expose one static per slot level (rather than a single action with
/// an override parameter) to match how the other Action statics are
/// shaped — each entry is a focused, named option the picker UI surfaces.
/// The action validates the caster has the SP, has at least one expended
/// slot at the requested level, and is combat-active. The side-effect
/// debits SP up front (mirroring metamagic's eager-debit pattern) and
/// restores the slot via the engine's `restore_spell_slot` lane.
pub struct CreateSpellSlot {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub slot_level: u32,
    pub sp_cost: u32,
}

impl Action for CreateSpellSlot {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
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
        bonus_action_only()
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
        if !actor.is_combat_active() {
            return false;
        }
        if actor.sorcery_points() < self.sp_cost {
            return false;
        }
        // Only valid if at least one slot at this level is currently
        // spent — recreating an already-full level is a no-op that would
        // waste the SP.
        let ssi = actor.spell_slot_manager.spell_slots(self.slot_level);
        ssi.spell_slots < ssi.max_spell_slots
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Direct mutation pattern mirrors the metamagic primes:
        // eager-debit + log + restore-slot, all inside `side_effects`.
        // No dedicated `SorceryPoint` Resource lane (avoids a third
        // resource enum entry for a single feature).
        let line = encounter.actors.get_mut(&caster_id).map(|actor| {
            actor.spend_sorcery_points(self.sp_cost);
            actor.spell_slot_manager.restore_spell_slot(self.slot_level, 1);
            format!(
                "{} converts {} SP into a level-{} slot ({} SP left).",
                actor.name(),
                self.sp_cost,
                self.slot_level,
                actor.sorcery_points()
            )
        });
        if let Some(line) = line {
            encounter.log(line);
        }
        Vec::new()
    }
}

pub static CREATE_SPELL_SLOT_1: CreateSpellSlot = CreateSpellSlot {
    display_name: "create level-1 slot",
    aliases: &["cs1", "fontslot1"],
    slot_level: 1,
    sp_cost: 2,
};

pub static CREATE_SPELL_SLOT_2: CreateSpellSlot = CreateSpellSlot {
    display_name: "create level-2 slot",
    aliases: &["cs2", "fontslot2"],
    slot_level: 2,
    sp_cost: 3,
};

pub static CREATE_SPELL_SLOT_3: CreateSpellSlot = CreateSpellSlot {
    display_name: "create level-3 slot",
    aliases: &["cs3", "fontslot3"],
    slot_level: 3,
    sp_cost: 5,
};

/// 5e Sorcerer **Font of Magic** — Convert Spell Slot. Bonus action; burn
/// one expended-able spell slot of `slot_level` to gain `slot_level`
/// sorcery points (RAW: "you can transform one of your spell slots into
/// sorcery points; the slot value is added to your sorcery points pool,
/// up to your maximum"). Hard-capped at slot_level <= 5 — RAW lets you
/// convert any slot but the highest practical use is refilling SP for
/// metamagic, and a level-6+ slot is more valuable held.
///
/// We expose one static per slot level (rather than a single action with
/// an override parameter) to match how the other Action statics are
/// shaped. The action validates an open slot is available and the SP
/// pool has room (we cap at `sorcery_points_max` per RAW — the surplus
/// is wasted). The side-effect debits the slot up front and grants SP
/// via the actor's pool directly (no dedicated `SorceryPoint` Resource).
pub struct ConvertSpellSlot {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub slot_level: u32,
}

impl Action for ConvertSpellSlot {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
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
        // The spell slot cost lives in the resource lane so the engine's
        // existing slot debit/log pipeline handles it cleanly (vs an
        // inline `consume_spell_slot` in `side_effects`).
        bonus_action_and_slot(self.slot_level)
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
        if !actor.is_combat_active() {
            return false;
        }
        // Only valid if SP isn't already at the long-rest cap — RAW says
        // SP gained "up to your maximum"; converting beyond the cap is
        // strictly a slot waste, so we just block it.
        actor.sorcery_points() < actor.sorcery_points_max()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let line = encounter.actors.get_mut(&caster_id).map(|actor| {
            // RAW "up to your maximum": cap the gain at the long-rest pool
            // size so we don't grow `sorcery_points` past `sorcery_points_max`.
            let cap = actor.sorcery_points_max();
            let gained = self.slot_level.min(cap.saturating_sub(actor.sorcery_points()));
            actor.give_sorcery_points(gained);
            format!(
                "{} converts a level-{} slot into {} SP ({} SP total).",
                actor.name(),
                self.slot_level,
                gained,
                actor.sorcery_points()
            )
        });
        if let Some(line) = line {
            encounter.log(line);
        }
        Vec::new()
    }
}

pub static CONVERT_SPELL_SLOT_1: ConvertSpellSlot = ConvertSpellSlot {
    display_name: "convert level-1 slot",
    aliases: &["convertslot1", "fontsp1"],
    slot_level: 1,
};

pub static CONVERT_SPELL_SLOT_2: ConvertSpellSlot = ConvertSpellSlot {
    display_name: "convert level-2 slot",
    aliases: &["convertslot2", "fontsp2"],
    slot_level: 2,
};

pub static CONVERT_SPELL_SLOT_3: ConvertSpellSlot = ConvertSpellSlot {
    display_name: "convert level-3 slot",
    aliases: &["convertslot3", "fontsp3"],
    slot_level: 3,
};

/// 5e War Domain Cleric **War Priest** feature tag (level 1 subclass).
/// RAW: WIS-mod uses per long rest of a bonus-action extra weapon attack
/// after taking the Attack action. We collapse the WIS-scaled charge
/// pool to a single per-short-rest charge so the gating stays uniform
/// with the other class-feature tags (Second Wind / Action Surge /
/// Sacred Weapon) — the once-per-short-rest cadence sits between the
/// once-per-long-rest floor and the WIS-mod / long-rest ceiling.
/// Registered in `SHORT_REST_FEATURES` so short rests refresh it.
pub const WAR_PRIEST_TAG: &str = "cleric.war_priest";

/// War Priest — War Domain Cleric bonus action. Grants the cleric an
/// extra Action token for a follow-up weapon swing this turn. Same
/// action-economy trade as Flurry of Blows / Frenzy / Action Surge —
/// spend a bonus action (plus a short-rest charge) to buy a fresh main
/// Action. The AI's normal attack picker handles the weapon / target
/// selection on the granted swing.
///
/// Once per short rest. Gated on the cleric holding the `WAR_PRIEST_TAG`
/// feature flag AND being combat-active. Pairs naturally with the
/// cleric's Divine Strike prime — bonus-action Divine Strike into
/// bonus-action War Priest wouldn't work RAW (only one bonus action per
/// turn), but Divine Strike prime one round → War Priest next round
/// with the prime still up gets the follow-up swing riding the +1d8
/// radiant rider.
pub struct WarPriest {}

impl Action for WarPriest {
    fn name(&self) -> &str {
        "war priest"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wp", "priest"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, WAR_PRIEST_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        spend_feature_and_grant_extra_action(
            encounter,
            caster_id,
            WAR_PRIEST_TAG,
            "  war priest: cleric gains an extra Action for a follow-up strike.",
        )
    }
}

pub static WAR_PRIEST: LazyLock<WarPriest> = LazyLock::new(|| WarPriest {});

/// 5e War Domain Cleric **Guided Strike** feature tag (Channel Divinity,
/// level 2 subclass). Once per short rest, prime the cleric's next
/// attack roll with a flat +10 accuracy buff — the largest single-swing
/// accuracy buff in the game, meant to turn a marginal near-miss into a
/// guaranteed connect. Registered in `SHORT_REST_FEATURES` so short
/// rests refresh it.
///
/// Distinct from Sacred Weapon (Devotion Paladin Channel Divinity):
/// Sacred Weapon lasts 10 rounds and adds +CHA (typically +2-4);
/// Guided Strike is a one-shot flat +10, higher per-swing buff at the
/// cost of the one-and-done lifecycle.
pub const GUIDED_STRIKE_TAG: &str = "cleric.guided_strike";

/// Channel Divinity: Guided Strike — War Domain Cleric bonus action.
/// Installs the `GuidedStriking` prime on the cleric for their next
/// attack roll: +10 flat bonus, consumed on the first swing that fires.
/// Once per short rest.
///
/// Sibling to Sacred Weapon (Devotion Paladin Channel Divinity) on the
/// attack-roll buff lane — Sacred Weapon spreads +CHA across a 10-round
/// concentration window, Guided Strike concentrates a fatter +10 on a
/// single-shot prime. Backed by the same `feature_prime_ready` gate
/// and `prime_self_condition` install helper the other bonus-action
/// primes route through.
pub struct GuidedStrike {}

impl Action for GuidedStrike {
    fn name(&self) -> &str {
        "guided strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gs", "cd-guided", "guided"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_prime_ready(encounter, caster_id, GUIDED_STRIKE_TAG, Condition::GuidedStriking)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            GUIDED_STRIKE_TAG,
            Condition::GuidedStriking,
            // Tick-down timer caps the prime to a single round so an
            // idle cleric doesn't carry it across encounters. Consumed
            // by `CONSUMED_ON_ATTACK` on the first swing that fires.
            ConditionTimer::UntilStartOfNextTurn,
            "  guided strike: cleric's next attack rides a +10 divine accuracy buff.",
        )
    }
}

pub static GUIDED_STRIKE: LazyLock<GuidedStrike> = LazyLock::new(|| GuidedStrike {});

/// 5e **Trickery Domain Cleric** Channel Divinity: **Invoke Duplicity**
/// feature tag (subclass level 2). Once per short rest — registered in
/// `SHORT_REST_FEATURES` alongside the other cleric Channel Divinity
/// charges (Turn Undead, Preserve Life, Guided Strike, Radiance of the
/// Dawn).
///
/// RAW conjures a perfect illusory double within 30 ft, moved with a
/// bonus action, granting the cleric advantage on attack rolls against
/// any creature within 5 ft of it while the cleric is also within 5 ft
/// of that creature. The engine has no second body to place or move, so
/// the positional clause collapses into the `Duplicity` self-buff: ten
/// rounds of advantage on the cleric's attack rolls, full stop.
///
/// That is the generous direction — RAW's illusion can be played around
/// by spreading out and this one cannot — and it is why the charge is
/// the Trickery cleric's *entire* Channel Divinity budget rather than
/// one option among several.
pub const INVOKE_DUPLICITY_TAG: &str = "cleric.invoke_duplicity";

/// Channel Divinity: Invoke Duplicity — Trickery Domain Cleric action.
/// Installs the `Duplicity` self-buff (advantage on the cleric's attack
/// rolls) for ten rounds. Once per short rest.
///
/// **Action, not bonus action**, which is the one place this deviates
/// upward in cost from its siblings on the lane. Guided Strike is a
/// bonus action because it buys one swing; Invoke Duplicity buys every
/// swing for a minute, and RAW charges an action for it. On a chassis
/// whose bonus action is otherwise idle that ordering matters: the
/// Trickery cleric spends a whole turn arming, then collects.
///
/// The gate is `feature_prime_ready` — the same "has a charge and isn't
/// already carrying the flag" shape every other prime uses — so a
/// cleric can't burn the short-rest charge refreshing a live illusion.
pub struct InvokeDuplicity {}

impl Action for InvokeDuplicity {
    fn name(&self) -> &str {
        "invoke duplicity"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["duplicity", "cd-duplicity"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_prime_ready(encounter, caster_id, INVOKE_DUPLICITY_TAG, Condition::Duplicity)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            INVOKE_DUPLICITY_TAG,
            Condition::Duplicity,
            // Ten rounds — the engine's standard 1-minute envelope, and
            // RAW's duration. Unlike its one-shot neighbours on this
            // lane, the flag is off `CONSUMED_ON_ATTACK`: it is meant to
            // ride every swing in the window.
            ConditionTimer::Rounds(10),
            "  invoke duplicity: an illusory double steps out beside the cleric.",
        )
    }
}

pub static INVOKE_DUPLICITY: LazyLock<InvokeDuplicity> = LazyLock::new(|| InvokeDuplicity {});

/// Class-feature tag for the **Beast Master Ranger**'s *Ranger's
/// Companion* (5e PHB, subclass level 3). Once per long rest — the
/// beast is a permanent bond in RAW, and the per-rest charge is the
/// engine's way of saying "you get one, and if it dies you don't get
/// another this fight."
pub const RANGERS_COMPANION_TAG: &str = "ranger.rangers_companion";

/// Ranger's Companion — Beast Master Ranger action. Calls the bonded
/// beast to a free tile beside the ranger, on the ranger's team, once
/// per long rest.
///
/// **Not a summoning spell**, and the differences are the feature.
/// Conjure Animals and Animate Dead cost a slot, hold concentration,
/// and evaporate when the caster's concentration breaks — they are
/// temporary allies rented with the caster's attention. The companion
/// costs no slot, no concentration, and stays until it drops. What it
/// costs instead is the ranger's Action on the turn they spend calling
/// it, and the once-per-rest charge: a Beast Master who loses the beast
/// has lost the subclass for the rest of the day.
///
/// That trade puts the timing decision squarely on the player. Spent
/// on round one it is an Action not swung and a body that fights for
/// the whole encounter; held back it is a full ranger turn and a
/// companion arriving into a fight that may already be decided.
///
/// Declares `summons_allies` so the AI's summon rung picks it up
/// without a name to remember — the same hook Conjure Animals and
/// Animate Dead use. The validator owns the part the AI shouldn't
/// guess at: whether a Medium creature fits anywhere nearby.
pub struct RangersCompanion {}

impl Action for RangersCompanion {
    fn name(&self) -> &str {
        "ranger's companion"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["companion", "beast", "rc"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Indirect: the beast deals the damage, on its own turns.
        false
    }
    fn summons_allies(&self) -> bool {
        true
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, RANGERS_COMPANION_TAG)
            && encounter
                .find_adjacent_spawn(caster_id, crate::engine::types::Size::Medium, 2)
                .is_some()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::creatures::wolves::RANGERS_COMPANION_TEMPLATE;
        // Spend the charge before the spawn attempt, not after: the
        // validator has already confirmed a free tile, and a partial
        // failure inside `spawn_adjacent_summons` (the arena filled in
        // between) should still cost the ranger the call rather than
        // leaving a charge to retry with the same result.
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(RANGERS_COMPANION_TAG);
        }
        // Instance ids from 70 — a private band that doesn't collide
        // with the summon spells' 90+ range.
        crate::actions::spells::spawn_adjacent_summons(
            encounter,
            caster_id,
            &RANGERS_COMPANION_TEMPLATE,
            crate::engine::types::Size::Medium,
            1,
            2,
            70,
            "ranger's companion",
        );
        // No side-effects to queue: unlike the conjuration spells there
        // is no `Conjured` marker and no `StartConcentration` anchor to
        // hang the beast's lifetime on. It stays until it drops, which
        // is the whole distinction from a summoning spell.
        Vec::new()
    }
}

pub static RANGERS_COMPANION: LazyLock<RangersCompanion> = LazyLock::new(|| RangersCompanion {});

/// 5e Light Domain Cleric **Radiance of the Dawn** Channel Divinity tag
/// (level 2 subclass). Once per short rest, action-cost 30ft self-centered
/// radiant burst — every enemy in range makes a CON save vs the cleric's
/// spell save DC. On fail: 2d10 + cleric level radiant. On save: half.
/// RAW clause "any magical darkness within 30 ft is dispelled" is a
/// no-op in our engine (magical darkness isn't a modeled hazard).
///
/// Registered in `SHORT_REST_FEATURES` so short rests refresh the charge
/// alongside Turn Undead / Preserve Life / Guided Strike / War Priest.
pub const RADIANCE_OF_THE_DAWN_TAG: &str = "cleric.radiance_of_the_dawn";

/// Channel Divinity: Radiance of the Dawn — Light Domain Cleric action.
/// Self-centered 30ft (12-tile) enemy burst. Each enemy rolls a CON save
/// vs the cleric's spell save DC (WIS-based); pass = half, fail = full
/// `2d10 + cleric level` radiant. Once per short rest.
///
/// The Light Domain's signature crowd-control-and-damage lane — sibling
/// to Turn Undead (Frightened install, undead-only) and Preserve Life
/// (mass heal). Where Turn Undead targets a narrow creature type and
/// Preserve Life heals allies, Radiance of the Dawn is undiscriminating
/// damage against every enemy in the burst. RAW gates on the Light
/// Domain subclass; we surface it through the `LIGHT_CLERIC_TEMPLATE`
/// subclass template so a baseline cleric can't fire it.
///
/// Pairs naturally with Guiding Bolt (single-target 4d6 radiant) and
/// Sacred Flame (single-target 1d8 radiant) for a radiant-damage
/// combo — the Light cleric plays the "burst of light" damage lane
/// where Turn Undead / Preserve Life play the crowd-control / heal
/// lanes.
pub struct RadianceOfTheDawn {}

impl Action for RadianceOfTheDawn {
    fn name(&self) -> &str {
        "radiance of the dawn"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rod", "cd-radiance", "dawn"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, RADIANCE_OF_THE_DAWN_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Spend the charge up-front so a mid-resolution actor lookup can't
        // double-fire (same shape as Preserve Life / Turn Undead).
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(RADIANCE_OF_THE_DAWN_TAG);
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let level = caster.level();
        // 2d10 + cleric level radiant — rolled once and shared across
        // the burst (5e AoE damage rolls are shared). Log the breakdown
        // for the same reason Flame Strike / Destructive Wave do:
        // players can trace back exactly how much each target ate.
        let raw = encounter.roll(&Dice::new(2, 10));
        let damage = raw + level;
        encounter.log(format!(
            "  radiance of the dawn: 2d10({})+{} radiant, DC {} CON save for half.",
            raw, level, dc
        ));
        resolve_enemy_burst_save_damage(
            encounter,
            caster_id,
            center,
            12,
            AbilityScoreType::Constitution,
            dc,
            damage,
            DamageType::Radiant,
        )
    }
}

pub static RADIANCE_OF_THE_DAWN: LazyLock<RadianceOfTheDawn> =
    LazyLock::new(|| RadianceOfTheDawn {});

/// 5e Light Domain Cleric level-1 subclass feature — **Warding Flare**.
/// Passive reaction: when a creature within 30 ft that the cleric can
/// see makes an attack roll against them, the cleric can use their
/// reaction to impose disadvantage on the attack roll. RAW uses per
/// long rest equal to WIS mod (min 1); we collapse to a once-per-
/// short-rest charge (registered in `SHORT_REST_FEATURES`) so the
/// Light cleric doesn't lose the flare between engagements — same
/// gating shape as the Cleric's other Channel Divinity charges.
///
/// No action surface — the trigger fires automatically at the attack
/// chokepoint (`resolve_attack` for weapon swings and
/// `spell_attack_outcome` for spell attacks) via
/// `EncounterInstance::apply_warding_flare_disadvantage`. Ships as a
/// pure passive template tag on `LIGHT_CLERIC_TEMPLATE` — no per-
/// action `Action` impl, no bonus-action lane, mirroring how
/// Blood Frenzy / Fast Movement / Mindless Rage layer as passive
/// tags without a bonus-action shape.
///
/// RAW blindness clause ("can see"): we approximate with `!Blinded`.
/// The 30ft range collapses to a footprint-Chebyshev cap of 12 tiles
/// (2.5ft grid). The engine reads the trigger in
/// `apply_warding_flare_disadvantage`, spending the reaction + charge
/// on fire and combining `Disadvantage` into the attack mode.
pub const WARDING_FLARE_TAG: &str = "cleric.warding_flare";

/// 5e Oathbreaker Paladin (DMG) level-15 subclass feature —
/// **Fanatical Focus**. Auto-fire once-per-short-rest reroll of a
/// failed saving throw. RAW: "If you fail a saving throw while your
/// aura is active, you can reroll it. Once you use this ability, you
/// can't use it again until you finish a short or long rest."
///
/// Distinct from Fighter Indomitable (`INDOMITABLE_TAG`) in three
/// ways:
///   1. **Auto-fire vs pre-primed** — Indomitable requires an Action
///      call to set `mark_indomitable_pending` *before* the save; if
///      the fighter didn't pre-prime, a failed save doesn't trigger.
///      Fanatical Focus fires automatically on the first failed save
///      after a short rest — no advance planning needed.
///   2. **Short rest vs long rest** — Indomitable is once per long
///      rest (not in `SHORT_REST_FEATURES`). Fanatical Focus is once
///      per short rest — the Oathbreaker paladin gets one on every
///      engagement rather than one per adventuring day.
///   3. **Uses a feature charge, not a pending latch** — Indomitable
///      spends the tag at Action time and stashes a pending latch on
///      the actor; Fanatical Focus spends the tag *at the save site*
///      the first time the paladin fails a save with an unspent
///      charge. One less state field to carry per actor, and the
///      "sees an unspent charge → fire" gate reads uniformly with
///      Legendary Resistance's shape.
///
/// RAW gates on "while your aura is active" (Aura of Protection, lv6+).
/// Since Aura of Protection isn't a togglable resource in the engine —
/// it's always on for any level-6+ paladin holding the
/// `has_aura_of_protection` flag — the "while active" clause collapses
/// to a no-op on the Oathbreaker paladin chassis. A hypothetical build
/// that lost the aura (e.g. Silenced-cohort suppression down the line)
/// would still fire this: the RAW clause exists as flavor rather than
/// a mechanical gate, so we don't tie the reroll to a re-check of the
/// aura flag.
///
/// Wired in `EncounterInstance::roll_save_with_extra_mode`: on a
/// failed save (post-Indomitable, pre-Legendary-Resistance), if the
/// actor holds `FANATICAL_FOCUS_TAG` on `features_remaining`, the tag
/// is spent and the save re-rolled once with the same modifier /
/// mode. Legendary Resistance stays checked below the Fanatical
/// Focus gate — RAW: LR is an active DM/boss resource, so the
/// once-per-short-rest passive fires first.
pub const FANATICAL_FOCUS_TAG: &str = "paladin.fanatical_focus";

/// 5e Oathbreaker Paladin (DMG) level-7 subclass feature — **Aura of
/// Hate**. Passive template flag: the paladin and any fiends / undead
/// within 10 ft gain a bonus to melee weapon damage rolls equal to the
/// paladin's Charisma modifier (minimum +1). We collapse the RAW aura
/// shape to a self-only bonus at the caster-side melee bumps table in
/// `engine::attack::resolve_attack_outcome` — the paladin themselves
/// picks up the +CHA mod on every melee swing while the flag is set.
///
/// The "any fiends and undead within 10 ft" clause is dropped in the
/// current model since we don't tag allied fiends / undead as an
/// aura-eligible cohort at the template level. Adding a broader
/// "adjacent-fiend-or-undead ally gets +CHA mod melee damage" scan
/// would need a fresh footprint-Chebyshev pass at the attack site,
/// which is an aura-shape mismatch with the other paladin auras (Aura
/// of Protection, Aura of Courage, Aura of Devotion) that read the
/// EMITTER's flag on the ally-side rather than the ATTACKER's own
/// flag on the caster-side. Self-only keeps the read local and the
/// bump-table plumbing uniform.
///
/// Reads through the existing melee bumps table next to Rage (+2),
/// Dueling (+2), Two-Weapon Fighting (+STR mod) — one new tuple, no
/// re-shape of the attack-outcome shape. Minimum +1 clause folds in
/// via `max(1)` on the CHA modifier lookup, matching RAW.
///
/// Distinguished from Vow of Enmity (Vengeance paladin, once-per-
/// long-rest Advantage prime): Aura of Hate is passive, always-on
/// (once the flag ships on the Oathbreaker template), and a damage
/// bump rather than an attack-mode bump. Distinguished from
/// Improved Divine Smite (+1d8 radiant on every melee hit, no gate):
/// Aura of Hate is a flat mod rather than a die, and its damage rides
/// as part of the base weapon damage roll rather than a separate
/// `push_die_rider` payload — so a resistance-halving on the weapon
/// type also halves the Aura of Hate bonus, mirroring how Rage /
/// Dueling / Two-Weapon Fighting fold into the base swing.
pub const AURA_OF_HATE_TAG: &str = "paladin.aura_of_hate";

/// 5e Berserker Barbarian level-10 subclass feature — **Intimidating
/// Presence**. Class-feature tag; refreshed on a short rest via
/// `SHORT_REST_FEATURES`. RAW: as an action, choose a creature within
/// 30ft that can see or hear the barbarian; the target must succeed on
/// a Wisdom save (DC 8 + prof + CHA mod) or be Frightened until the
/// end of the barbarian's next turn. RAW also allows extending the
/// duration on subsequent turns and includes a 24-hour immunity cap
/// on saved targets; we collapse both to a fixed 10-round Frightened
/// timer per install so the debuff shape stays uniform with the
/// other single-target Frighten installs (Cause Fear, Wrathful
/// Smite's Wrathful rider, Fear cone) — the short-rest gate keeps the
/// resource cadence in check without threading a per-target 24-hour
/// cooldown ledger through actor state.
///
/// Sibling to Turn Undead / Turn the Faithless on the shared class-
/// feature "save-vs-Frightened" family, differentiated by shape:
///   - Turn Undead / Turn the Faithless are 30ft *radial* bursts with
///     a creature-type filter (undead / fey / fiend) — Wisdom-saved,
///     WIS- or CHA-anchored, `resolve_turn_burst` helper.
///   - Intimidating Presence is a single-target install with no
///     creature-type filter — Wisdom-saved, CHA-anchored, the
///     `resolve_single_target_cd_save_condition` helper below.
///     Adding a future "Cause Fear" class-feature analogue or Ancients
///     Paladin's Nature's Wrath (below) drops in as a fresh call to the
///     shared single-target helper with a distinct `(save_ability,
/// condition)` pair.
///
/// Ships on `BERSERKER_BARBARIAN_TEMPLATE` above its strict RAW lv10
/// gate for the same reason Mindless Rage (lv6) already ships there —
/// class templates target a balanced playable level, not lockstep PHB
/// progression. Composes cleanly with the Berserker's raging loop:
/// the frightened target eats disadvantage on attack rolls (RAW
/// Frightened clause) while the barbarian's own raging swings still
/// land unhindered.
pub const INTIMIDATING_PRESENCE_TAG: &str = "barbarian.intimidating_presence";

/// 5e Ancients Paladin level-3 Channel Divinity — **Nature's Wrath**.
/// Class-feature tag; refreshed on a short rest via
/// `SHORT_REST_FEATURES`. RAW: as an action, expend one use of Channel
/// Divinity to cause spectral vines to spring up and reach for a
/// creature within 10ft. The target must succeed on a Strength OR
/// Dexterity save (target's choice) or be Restrained by the vines
/// until the end of the paladin's next turn. On subsequent turns
/// the target can re-roll the save at the same DC to end the effect;
/// we collapse both the target-choice and the re-roll cadence to a
/// fixed Strength save + 10-round timer per install so the shape
/// stays uniform with the other Restrained-installing spells
/// (Entangle, Grasping Vine, Watery Sphere) — targets whose STR is
/// worse than DEX pay a slightly higher DC than RAW, but the loss
/// is bounded by the CR-4 barbarian's typical spread on the two.
///
/// Sibling to Intimidating Presence (Berserker Barbarian lv10)
/// above on the `resolve_single_target_cd_save_condition` helper:
/// same "single target, save-vs-CHA-DC, install condition on fail"
/// shape, differentiated by the (save-ability, condition) pair
/// (WIS + Frightened for Intimidating Presence, STR + Restrained
/// for Nature's Wrath). Distinct from Vow of Enmity (Vengeance
/// Paladin CD, no-save Sworn install on a single hostile within
/// 10ft) — Vow of Enmity is an *accuracy* prime for the paladin's
/// own attacks, while Nature's Wrath is a *lockdown* debuff on the
/// target.
///
/// Ships on `ANCIENTS_PALADIN_TEMPLATE` at its RAW lv3 gate. Pairs
/// naturally with the paladin's smite family — a Restrained target
/// eats disadvantage on the STR / DEX save re-roll AND every
/// paladin swing lands with advantage per the Restrained clause,
/// stacking a guaranteed-crit-chance window with the paladin's
/// smite spike-damage lane.
pub const NATURES_WRATH_TAG: &str = "paladin.natures_wrath";

/// Shared "pick a single target, roll a save vs the caster's spellcasting-
/// ability DC, install a condition on fail" body for single-target
/// class-feature actions. Sibling to `resolve_turn_burst` on the
/// class-feature family — Turn Undead / Turn the Faithless share the
/// 30ft radial-burst shape, and Intimidating Presence (Berserker
/// Barbarian lv10) / Nature's Wrath (Ancients Paladin CD lv3) share
/// the single-target shape.
///
/// The feature charge is spent up front (before the save) so the
/// once-per-rest cost is paid whether the target passes or fails —
/// mirrors how the Turn burst helpers spend the tag before rolling
/// each save. The save DC is derived from the caster's
/// `spellcasting_ability` (CHA for paladins / barbarians), the target
/// rolls with `save_ability`, and on a failed save `condition` is
/// installed with `timer`. Aura suppressors and target-side condition
/// immunity route through the standard `ApplyCondition` chokepoint
/// (aura-driven suppression in `ApplyCondition::apply` and holder-side
/// immunity in `add_condition`) so a Restrained-immune Ooze / a
/// Charmed-immune Undead / an ally standing in an Aura of Devotion
/// bounces the install cleanly per RAW.
///
/// Parameters mirror `resolve_turn_burst` where the shape overlaps:
///   - `caster_id` — spends `feature_tag` before the save.
///   - `target_id` — the single target of the save.
///   - `spellcasting_ability` — anchor for the save DC (8 + prof + mod).
///   - `save_ability` — the ability the target rolls the save with.
///   - `condition` / `timer` — installed on a failed save.
///   - `label` — log prefix ("intimidating presence" / "nature's wrath").
///
/// Adding a future single-target save-vs-condition class feature
/// (a hypothetical "Compelled Retreat" that installs Frightened via
/// a CHA-DC save, an Ancients paladin "Turn the Faithless" single-
/// target variant, etc.) lands as a fresh call with a distinct
/// (save_ability, condition, timer) tuple — no re-implementation of
/// the spend-tag / roll-DC / on-fail-install dance.
#[allow(clippy::too_many_arguments)]
fn resolve_single_target_cd_save_condition(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    feature_tag: &'static str,
    spellcasting_ability: AbilityScoreType,
    save_ability: AbilityScoreType,
    condition: Condition,
    timer: ConditionTimer,
    label: &'static str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(dc) = spend_feature_and_get_dc(
        encounter,
        caster_id,
        feature_tag,
        spellcasting_ability,
    ) else {
        return Vec::new();
    };
    encounter.log(format!(
        "  {}: target rolls {:?} save (DC {}).",
        label, save_ability, dc
    ));
    let save = encounter.roll_save(target_id, save_ability, dc);
    if save.passed() {
        return Vec::new();
    }
    vec![Box::new(ApplyCondition {
        actor_id: target_id,
        condition,
        timer,
    })]
}

/// Intimidating Presence — Berserker Barbarian class feature action.
/// Once-per-short-rest single-target Frighten install: the target
/// (within 30ft, 12 tiles on our 2.5ft grid) rolls a WIS save vs the
/// barbarian's CHA-anchored DC (8 + prof + CHA mod). On fail the
/// target is Frightened for 10 rounds (1 minute RAW). Routes through
/// the shared `resolve_single_target_cd_save_condition` helper — the
/// gate + spend + roll + install dance lives in one place, and
/// mirrors Turn Undead / Turn the Faithless on the burst-side lane.
///
/// Range gate (12 tiles = 30ft RAW) matches the RAW range and mirrors
/// Turn Undead / Turn the Faithless's radius — the shared "30ft
/// intimidation" mental envelope stays consistent across barbarian /
/// cleric / paladin fear-installers.
pub struct IntimidatingPresence {}

impl Action for IntimidatingPresence {
    fn name(&self) -> &str {
        "intimidating presence"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ip", "intimidate", "presence"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft RAW = 12 tiles on the 2.5ft grid.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        // RAW: "a creature that you can see" — line of sight anchor.
        // The Deafened / Blinded gate is dropped since the RAW clause
        // is "see OR hear you", which we treat as always-true for
        // combat-active targets.
        true
    }
    fn is_harmful(&self) -> bool {
        true
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
        // Once-per-rest + hostile-target + already-Frightened dedup all
        // fold into the shared `hostile_target_feature_ready` gate. The
        // Frightened dedup keeps the AI from burning the short-rest
        // charge on a target that's already Frightened (via Cause Fear,
        // Turn Undead, Wrathful Smite, a dragon fear cone, etc.).
        hostile_target_feature_ready(
            encounter,
            caster_id,
            target_ids,
            INTIMIDATING_PRESENCE_TAG,
            Condition::Frightened,
        )
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        resolve_single_target_cd_save_condition(
            encounter,
            caster_id,
            target_id,
            INTIMIDATING_PRESENCE_TAG,
            // Barbarian's spellcasting-anchor ability collapses to CHA
            // per the RAW Intimidating Presence text — the barbarian
            // isn't a caster, but the DC formula (8 + prof + CHA mod)
            // reads CHA the same way.
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
            Condition::Frightened,
            // 10 rounds = 1 minute — matches Turn Undead / Turn the
            // Faithless's Frighten window. The RAW "until end of your
            // next turn" collapses to the standard 1-minute install
            // since the engine doesn't track the extend-on-subsequent-
            // turns clause.
            ConditionTimer::Rounds(10),
            "intimidating presence",
        )
    }
}

pub static INTIMIDATING_PRESENCE: LazyLock<IntimidatingPresence> =
    LazyLock::new(|| IntimidatingPresence {});

/// Nature's Wrath — Ancients Paladin Channel Divinity action. Once-
/// per-short-rest single-target Restrained install: the target
/// (within 10ft, 4 tiles) rolls a STR save vs the paladin's CHA-
/// anchored DC (8 + prof + CHA mod). On fail the target is Restrained
/// for 10 rounds (1 minute — matches the other Restrained-installers
/// on the paladin / druid families). Routes through the shared
/// `resolve_single_target_cd_save_condition` helper — same body as
/// Intimidating Presence with a distinct (save-ability, condition)
/// pair.
///
/// RAW's STR-OR-DEX target choice collapses to a fixed STR save so
/// the debuff has a consistent save-ability lane; targets with a
/// significantly better DEX than STR pay a marginally higher DC
/// than RAW (bounded by the ability spread — a typical CR-4 target
/// has at most a 4-point gap between STR and DEX).
///
/// Range gate (4 tiles = 10ft RAW) matches Vow of Enmity's melee-
/// commitment envelope on the same paladin chassis — the paladin's
/// smite loop wants adjacency anyway, so both single-target CDs sit
/// at the same 10ft window.
pub struct NaturesWrath {}

impl Action for NaturesWrath {
    fn name(&self) -> &str {
        "nature's wrath"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["nw", "wrath", "natures-wrath"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 10ft RAW = 4 tiles on the 2.5ft grid.
        Some(4)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        true
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
        // Same shared gate as Intimidating Presence — once-per-rest
        // charge + hostile-target + already-Restrained dedup all fold
        // into `hostile_target_feature_ready`. Restrained dedup keeps
        // the AI from burning the charge on a target already Restrained
        // by Entangle, Grasping Vine, Watery Sphere, or a natural grapple.
        hostile_target_feature_ready(
            encounter,
            caster_id,
            target_ids,
            NATURES_WRATH_TAG,
            Condition::Restrained,
        )
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        resolve_single_target_cd_save_condition(
            encounter,
            caster_id,
            target_id,
            NATURES_WRATH_TAG,
            // Paladin spellcasting ability is CHA — same anchor as
            // Turn the Faithless / Sacred Weapon and every other
            // paladin CD DC.
            AbilityScoreType::Charisma,
            // STR save per RAW's Strength-OR-Dexterity choice
            // collapsed to Strength (see the tag docs above for
            // the collapse rationale).
            AbilityScoreType::Strength,
            Condition::Restrained,
            ConditionTimer::Rounds(10),
            "nature's wrath",
        )
    }
}

pub static NATURES_WRATH: LazyLock<NaturesWrath> = LazyLock::new(|| NaturesWrath {});

/// 5e Tempest Domain Cleric level-1 subclass feature — **Wrath of the
/// Storm**. Class-feature tag; refreshed on a short rest via
/// `SHORT_REST_FEATURES`. RAW: as a reaction when a creature within 5ft
/// hits you with an attack, roll a DEX save vs the cleric's spell save
/// DC; on fail the target takes 2d8 lightning OR thunder damage (RAW:
/// caster's choice per use — we collapse to lightning to keep the log
/// line stable and let the Thunderbolt Strike lane cleanly compose if
/// added later). Uses per long rest RAW = WIS mod (min 1), refreshed on
/// long rest; we collapse to a single once-per-short-rest charge so the
/// gating stays uniform with the other cleric Channel Divinity charges
/// (Guided Strike / Radiance of the Dawn / Warding Flare).
///
/// Ships as an **Action-cost** attack rather than the RAW reaction —
/// mirrors the Infernal Rebuke collapse rationale on the Tiefling
/// racial: the engine doesn't have a clean "reactive on being hit"
/// hook for player-driven actions, and the action cost keeps the
/// feature useful even when the cleric hasn't been hit yet. The 5ft
/// range (2 tiles) matches the RAW "within 5 ft" clause on the
/// reactive shape.
///
/// Sibling to Infernal Rebuke (Tiefling racial single-target damage
/// burst) on the `resolve_single_target_burst_save_for_half` helper:
/// same "spend feature charge, roll target save vs the caster's
/// spellcasting-ability DC, deal Xdy save-for-half" body, differentiated
/// by the (save-ability, dice, damage_type, spellcasting_ability) tuple
/// — CHA+3d10+Fire for Infernal Rebuke, WIS+2d8+Lightning for Wrath of
/// the Storm. Adding a future single-target save-for-half damage burst
/// (a hypothetical "Radiant Rebuke" Aasimar variant, etc.) drops in as
/// a fresh call to the shared helper with a distinct tuple.
pub const WRATH_OF_THE_STORM_TAG: &str = "cleric.wrath_of_the_storm";

/// Wrath of the Storm — Tempest Cleric subclass level-1 feature action.
/// Once-per-short-rest single-target 2d8 lightning damage burst: the
/// target (within 5ft, 2 tiles on our 2.5ft grid) rolls a DEX save vs
/// the cleric's WIS-anchored DC (8 + prof + WIS mod). On save the
/// target takes half damage; on fail, full damage. Routes through the
/// shared `resolve_single_target_burst_save_for_half` helper — the
/// spend + save + roll + damage dance lives in one place, mirrored by
/// Infernal Rebuke on the same helper.
///
/// Range gate (2 tiles = 5ft RAW) matches the RAW reactive shape's
/// "creature within 5 ft of you that hits you" clause — the tempest
/// cleric's flavor is a close-quarters retaliation zap, distinct from
/// Infernal Rebuke's 60ft ranged retort (the RAW Hellish Rebuke's
/// reaction fires up to 60ft away, not adjacent). Composes cleanly with
/// the tempest cleric's storm-themed spell picks (Thunderwave, Lightning
/// Bolt, Call Lightning) — Wrath of the Storm gives the cleric an
/// adjacent-target damage lane that doesn't burn a spell slot, freeing
/// the slots for their higher-tier bursts.
pub struct WrathOfTheStorm {}

impl Action for WrathOfTheStorm {
    fn name(&self) -> &str {
        "wrath of the storm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wots", "wrath-storm", "storm-wrath"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 5ft RAW = 2 tiles on the 2.5ft grid.
        Some(2)
    }
    fn requires_los(&self) -> bool {
        // RAW: the reaction fires when a creature "hits you with an
        // attack" — implicitly within reach, so LOS holds naturally.
        true
    }
    fn is_harmful(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Once-per-rest + hostile-target gate via the shared damage-burst
        // helper. Unlike Intimidating Presence / Nature's Wrath (target-
        // side condition install), the burst deals damage and doesn't
        // install a condition on fail — so `hostile_target_burst_ready`
        // (the no-dedup sibling of `hostile_target_feature_ready`) fits.
        hostile_target_burst_ready(encounter, caster_id, _target_ids, WRATH_OF_THE_STORM_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        resolve_single_target_burst_save_for_half(
            encounter,
            caster_id,
            target_id,
            WRATH_OF_THE_STORM_TAG,
            // Cleric spellcasting ability is WIS — same anchor as every
            // other cleric CD DC (Turn Undead, Guided Strike, Radiance
            // of the Dawn, Warding Flare).
            AbilityScoreType::Wisdom,
            // DEX save per RAW.
            AbilityScoreType::Dexterity,
            // 2d8 lightning — matches the RAW Wrath of the Storm dice.
            Dice::new(2, 8),
            DamageType::Lightning,
            "wrath of the storm",
        )
    }
}

pub static WRATH_OF_THE_STORM: LazyLock<WrathOfTheStorm> =
    LazyLock::new(|| WrathOfTheStorm {});

/// 5e Bard **Font of Inspiration** (level 5 class feature) tag. Passive
/// no-action feature: while the holder carries this tag, spent uses of
/// Bardic Inspiration refresh on a short rest as well as a long rest.
/// Read at the `SHORT_REST_FEATURES` registry cascade — the
/// `BARDIC_INSPIRATION_TAG` entry there gates on the short-rest refresh
/// path via `has_passive_feature(FONT_OF_INSPIRATION_TAG)` inside
/// `ActorInstance::short_rest`, so a bard without Font of Inspiration
/// (a lv1-4 bard build) still has to long-rest to reset the die.
///
/// Ships as a `has_passive_feature` template flag rather than a
/// per-rest charge — RAW is "you regain all expended uses when you
/// finish a short or long rest" from lv5 on, which is a permanent
/// unlock rather than a resource. Sibling to `CUNNING_ACTION_TAG` /
/// `VANISH_TAG` on the always-on class-feature lane — both are
/// permanent passive tags with no per-rest counter.
///
/// Ships on the baseline `BARD_TEMPLATE` above its strict RAW level
/// gate for the same reason Persistent Rage (lv15) ships on the CR-4
/// baseline barbarian, Improved Divine Smite (lv11) ships on the
/// CR-1.5 paladin, and Purity of Body (lv10) ships on the CR-1.5 monk
/// — class templates target a balanced playable level, not lockstep
/// PHB progression. Composes cleanly with Cutting Words (already once-
/// per-short-rest) so the bard's two per-rest charges — offensive
/// (Bardic Inspiration die) and defensive (Cutting Words reduction) —
/// both refresh on the same short-rest cadence.
pub const FONT_OF_INSPIRATION_TAG: &str = "bard.font_of_inspiration";

/// 5e Vengeance Paladin level-3 subclass Channel Divinity — **Abjure
/// Enemy**. Class-feature tag; refreshed on a short rest via
/// `SHORT_REST_FEATURES`. RAW: as an action, choose a creature within
/// 60 ft that you can see; the target must succeed on a Wisdom save vs
/// the paladin's spell save DC (8 + prof + CHA mod) or be Frightened
/// AND have its speed reduced to 0 for 1 minute; on save, its speed is
/// only halved for the same duration. We collapse the twin outcomes to
/// a single Frightened install on fail (the paladin's smite loop wants
/// the fear-driven attack-roll disadvantage on the target more than a
/// clean speed halving — Frightened already implies disadvantage on
/// attacks against the source, matching the RAW "cannot approach"
/// intent), matching the shape of Intimidating Presence / Nature's
/// Wrath / Turn the Faithless on the single-target Frighten family.
///
/// Sibling to Vow of Enmity on the Vengeance Paladin lv3 CD lane — RAW
/// gives the paladin a *choice* between Abjure Enemy (this) and Vow of
/// Enmity when they spend a CD charge. In our model both live on the
/// `VENGEANCE_PALADIN_TEMPLATE` as distinct per-rest tags — the AI can
/// pick either depending on the target: Vow of Enmity primes an
/// accuracy buff on the paladin's own attacks against a chosen target,
/// while Abjure Enemy installs a lockdown debuff *on* the target. The
/// two CDs cover the two lanes (attack prime vs target debuff)
/// separately rather than being mutually exclusive.
///
/// Routes through the shared `resolve_single_target_cd_save_condition`
/// helper — same body as Intimidating Presence / Nature's Wrath, with a
/// distinct (save-ability=WIS, condition=Frightened, spellcasting-
/// ability=CHA) tuple. Sibling to Turn the Faithless (30ft radial fey/
/// fiend Frighten burst on Devotion paladin) — Abjure Enemy is the
/// single-target no-creature-type-filter counterpart on the Vengeance
/// paladin, mirroring how Intimidating Presence covers "any hostile"
/// on the barbarian side vs Turn Undead's undead-only filter.
///
/// Undead / fiends get advantage on the WIS save per RAW ("Fiends and
/// undead have advantage on this saving throw"); we drop this rider
/// since the engine doesn't thread an ability-save creature-type
/// disadvantage lane, and the Vengeance paladin's most-common quarry
/// is exactly those two types — collapsing the rider matches the
/// paladin's smite-vs-fiend/undead spike-damage flavor (both smite and
/// Abjure Enemy fire hardest against the paladin's chosen enemy).
pub const ABJURE_ENEMY_TAG: &str = "paladin.abjure_enemy";

/// Abjure Enemy — Vengeance Paladin lv3 Channel Divinity action.
/// Once-per-short-rest single-target Frighten install: the target
/// (within 60ft, 24 tiles on our 2.5ft grid) rolls a WIS save vs the
/// paladin's CHA-anchored DC (8 + prof + CHA mod). On fail the target
/// is Frightened for 10 rounds (1 minute RAW). Routes through the
/// shared `resolve_single_target_cd_save_condition` helper.
///
/// Range gate (24 tiles = 60ft RAW) matches the RAW envelope — the
/// Vengeance paladin's ranged Frighten reach dwarfs the Ancients
/// paladin's Nature's Wrath 10ft grab (Restrained install wants
/// adjacency, Frighten install works at range). Pairs naturally with
/// the paladin's ranged Bow / Sacred Flame lane — a Frightened target
/// at 60ft can't approach and eats disadvantage on ranged shots back,
/// while the paladin keeps distance for a follow-up smite loop after
/// closing.
pub struct AbjureEnemy {}

impl Action for AbjureEnemy {
    fn name(&self) -> &str {
        "abjure enemy"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ae", "abjure", "abjure-enemy"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles on the 2.5ft grid.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        // RAW: "a creature that you can see" — line of sight anchor.
        true
    }
    fn is_harmful(&self) -> bool {
        true
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
        // Once-per-rest + hostile-target + already-Frightened dedup all
        // fold into the shared `hostile_target_feature_ready` gate. The
        // Frightened dedup keeps the AI from burning the short-rest
        // charge on a target that's already Frightened (via Cause Fear,
        // Turn Undead, Wrathful Smite, a dragon fear cone, etc.).
        hostile_target_feature_ready(
            encounter,
            caster_id,
            target_ids,
            ABJURE_ENEMY_TAG,
            Condition::Frightened,
        )
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        resolve_single_target_cd_save_condition(
            encounter,
            caster_id,
            target_id,
            ABJURE_ENEMY_TAG,
            // Paladin spellcasting ability is CHA — same anchor as
            // Turn the Faithless / Sacred Weapon / Nature's Wrath and
            // every other paladin CD DC.
            AbilityScoreType::Charisma,
            // WIS save per RAW.
            AbilityScoreType::Wisdom,
            Condition::Frightened,
            // 10 rounds = 1 minute — matches Turn the Faithless /
            // Intimidating Presence / Nature's Wrath's install window.
            ConditionTimer::Rounds(10),
            "abjure enemy",
        )
    }
}

pub static ABJURE_ENEMY: LazyLock<AbjureEnemy> = LazyLock::new(|| AbjureEnemy {});

/// 5e Devotion Paladin level-15 subclass feature — **Rebuke the
/// Violent**. Class-feature tag; refreshed on a short rest via
/// `SHORT_REST_FEATURES`. RAW: as a reaction when a creature within 30
/// ft deals damage to a creature other than the paladin, the offending
/// attacker must make a Wisdom save vs the paladin's spell save DC or
/// take radiant damage equal to the damage they dealt (max 4d10) —
/// half on save. We collapse the reactive shape to an Action-cost
/// attack per the same rationale that ships Infernal Rebuke / Wrath of
/// the Storm as actions (the engine doesn't have a clean "reactive on
/// ally being damaged" hook for player-driven actions), and we lock
/// the damage to a flat 4d10 radiant burst rather than mirror the
/// attacker's most recent damage (the "match the damage dealt" clause
/// needs a per-attacker last-damage ledger the engine doesn't thread).
/// Uses per long rest RAW = paladin-CD refresh rate (once per short
/// rest at level 3, then every use once at high level), collapsed to a
/// single once-per-short-rest charge to match the CD gating shape.
///
/// Sibling to Wrath of the Storm (Tempest Cleric CD lv1) and Infernal
/// Rebuke (Tiefling racial) on the `resolve_single_target_burst_save_for_half`
/// helper — same "spend feature charge, roll target save vs the
/// caster's spellcasting-ability DC, deal Xdy save-for-half" body,
/// differentiated by the (save-ability, dice, damage_type, spellcasting-
/// ability) tuple: CHA+4d10+Radiant+WIS-save for Rebuke the Violent,
/// WIS+2d8+Lightning+DEX-save for Wrath of the Storm, CHA+3d10+Fire+
/// DEX-save for Infernal Rebuke. Adding a future single-target save-for-
/// half damage burst drops in as a fresh call with a distinct tuple.
///
/// Ships on `DEVOTION_PALADIN_TEMPLATE` above its strict RAW lv15 gate
/// for the same reason Nature's Ward / Undying Sentinel (lv15 features)
/// ship on the CR-1.5 Ancients paladin — class templates target a
/// balanced playable level, not lockstep PHB progression. Composes
/// cleanly with the Devotion paladin's existing CD (Turn the Faithless
/// at lv3) — one CD charge lane for target-side Frighten burst, one
/// for target-side damage burst, both refreshed on a short rest.
pub const REBUKE_THE_VIOLENT_TAG: &str = "paladin.rebuke_the_violent";

/// Rebuke the Violent — Devotion Paladin lv15 subclass feature action.
/// Once-per-short-rest single-target 4d10 radiant damage burst: the
/// target (within 30ft, 12 tiles on our 2.5ft grid) rolls a WIS save
/// vs the paladin's CHA-anchored DC (8 + prof + CHA mod). On save the
/// target takes half damage; on fail, full damage. Routes through the
/// shared `resolve_single_target_burst_save_for_half` helper — the
/// spend + save + roll + damage dance lives in one place, mirrored by
/// Wrath of the Storm / Infernal Rebuke on the same helper.
///
/// Range gate (12 tiles = 30ft RAW) matches the RAW envelope — the
/// Devotion paladin's mid-range radiant retort. Pairs naturally with
/// the paladin's radiant-damage lane (Divine Smite radiant on undead /
/// fiends, Improved Divine Smite passive +1d8 radiant) — Rebuke the
/// Violent's 4d10 radiant lands as a slot-free burst that leans into
/// the same radiant-type advantage the paladin's smite loop already
/// exploits against fiend / undead opponents.
pub struct RebukeTheViolent {}

impl Action for RebukeTheViolent {
    fn name(&self) -> &str {
        "rebuke the violent"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rtv", "rebuke", "violent-rebuke"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft RAW = 12 tiles on the 2.5ft grid.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        // RAW: the reaction fires when the paladin "can see" the
        // attacker damaging an ally — LOS holds naturally.
        true
    }
    fn is_harmful(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Once-per-rest + hostile-target gate via the shared damage-burst
        // helper. Unlike Abjure Enemy (target-side Frighten install), the
        // burst deals damage and doesn't install a condition on fail — so
        // `hostile_target_burst_ready` (no target-side condition dedup)
        // fits, mirroring Wrath of the Storm's gate.
        hostile_target_burst_ready(encounter, caster_id, _target_ids, REBUKE_THE_VIOLENT_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        resolve_single_target_burst_save_for_half(
            encounter,
            caster_id,
            target_id,
            REBUKE_THE_VIOLENT_TAG,
            // Paladin spellcasting ability is CHA — same anchor as
            // every other paladin CD DC.
            AbilityScoreType::Charisma,
            // WIS save per RAW ("Wisdom saving throw" on the reactive
            // shape).
            AbilityScoreType::Wisdom,
            // 4d10 radiant — RAW caps the damage at 4d10 for lv15+
            // Devotion paladins; we lock at the cap since the collapse
            // to a fixed action-cost drops the "match the damage dealt"
            // clause.
            Dice::new(4, 10),
            DamageType::Radiant,
            "rebuke the violent",
        )
    }
}

pub static REBUKE_THE_VIOLENT: LazyLock<RebukeTheViolent> =
    LazyLock::new(|| RebukeTheViolent {});

/// 5e Fiend Warlock level-14 subclass capstone — **Hurl Through Hell**.
/// Class-feature tag; refreshed on a short rest via
/// `SHORT_REST_FEATURES`. RAW: after you hit a creature with an attack,
/// once per long rest, the target is transported through the lower
/// planes and returns at the end of your next turn; on return it takes
/// 10d10 psychic damage (no save — RAW). We collapse to an Action-cost
/// single-target burst with a CHA save for half so the shape stays
/// uniform with the sibling save-for-half class-feature damage-burst
/// family (Wrath of the Storm / Rebuke the Violent / Infernal Rebuke)
/// and the AI's regular target-picking lane can queue it without
/// needing a fresh "attack-first-then-fire" hook. RAW's once-per-long-
/// rest cadence collapses to once-per-short-rest so it lands on the
/// Warlock's Pact Magic slot-refresh cadence (RAW warlock slots
/// themselves refresh on a short rest); the burst competes with the
/// slot pool for the action budget, so sharing the same refresh clock
/// keeps the cadence uniform.
///
/// Sibling to Wrath of the Storm (Tempest Cleric CD lv1) / Rebuke the
/// Violent (Devotion Paladin lv15) / Infernal Rebuke (Tiefling racial)
/// on the `resolve_single_target_burst_save_for_half` helper — same
/// "spend feature charge, roll target save vs the caster's
/// spellcasting-ability DC, deal Xdy save-for-half" body, differentiated
/// by the (save-ability, dice, damage_type, spellcasting-ability)
/// tuple: CHA+10d10+Psychic+CHA-save for Hurl Through Hell (both anchor
/// on CHA — the warlock's spellcasting ability is CHA, and RAW's
/// "psychic damage as it reels from the horrific experience" reads as
/// a CHA/WIL-adjacent trauma resist). Adding a future single-target
/// save-for-half damage burst drops in as a fresh call with a distinct
/// tuple.
///
/// Ships on `FIEND_WARLOCK_TEMPLATE` (Fiend Patron capstone-adjacent
/// feature) above its strict RAW lv14 gate for the same reason
/// Fiendish Resilience (lv10) ships on the CR-4 template — class
/// templates target a balanced playable level, not lockstep PHB
/// progression. Composes cleanly with the fiend warlock's fire-heavy
/// spell kit (Burning Hands / Fireball) — Hurl Through Hell's 10d10
/// psychic slips past fire-immune targets (devils / demons) that would
/// no-op the warlock's usual fire cantrips, giving the fiend warlock a
/// slot-free psychic burst for those specific quarry.
pub const HURL_THROUGH_HELL_TAG: &str = "warlock.hurl_through_hell";

/// Hurl Through Hell — Fiend Warlock lv14 subclass feature action.
/// Once-per-short-rest single-target 10d10 psychic damage burst: the
/// target (within 60ft, 24 tiles on our 2.5ft grid) rolls a CHA save
/// vs the warlock's CHA-anchored DC (8 + prof + CHA mod). On save the
/// target takes half damage; on fail, full damage. Routes through the
/// shared `resolve_single_target_burst_save_for_half` helper — the
/// spend + save + roll + damage dance lives in one place, mirrored by
/// Wrath of the Storm / Rebuke the Violent / Infernal Rebuke on the
/// same helper.
///
/// Range gate (24 tiles = 60ft RAW) matches the RAW envelope on the
/// attack-precondition ("hit a creature with an attack") side — the
/// warlock's Eldritch Blast reach is 120ft but the fire-blast /
/// hellish rebuke family sits at the 60ft window. Pairs naturally with
/// the fiend warlock's slot-driven fire lane (Burning Hands / Fireball)
/// — Hurl Through Hell adds a psychic burst that slips past fire-
/// resistant / fire-immune quarry (devils / demons / salamanders /
/// magmins) that would otherwise no-op the warlock's usual cantrip and
/// spell picks. Distinct from Dark One's Own Luck (failed-save re-roll
/// prime, defensive utility) on the Fiend Warlock chassis — this is
/// the offensive slot-free burst; both refresh on the same short rest
/// so a Fiend Warlock enters each engagement with one defensive charge
/// AND one offensive charge in the tank.
pub struct HurlThroughHell {}

impl Action for HurlThroughHell {
    fn name(&self) -> &str {
        "hurl through hell"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hth", "hurl", "hurl-hell", "hurl-through-hell"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles on the 2.5ft grid.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        // RAW: "after you hit a creature with an attack" — the target
        // is by definition within LOS at the trigger point.
        true
    }
    fn is_harmful(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Once-per-rest + hostile-target gate via the shared damage-
        // burst helper — no target-side condition dedup (the burst
        // deals damage without installing a lingering condition).
        // Same shape as Wrath of the Storm / Rebuke the Violent.
        hostile_target_burst_ready(encounter, caster_id, target_ids, HURL_THROUGH_HELL_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        resolve_single_target_burst_save_for_half(
            encounter,
            caster_id,
            target_id,
            HURL_THROUGH_HELL_TAG,
            // Warlock spellcasting ability is CHA — same anchor as
            // every other warlock save-DC (Eldritch Blast, Hex,
            // Hellish Rebuke). Sibling to the sacred-weapon-anchored
            // paladin CDs (Abjure Enemy / Rebuke the Violent) and the
            // charisma-anchored tiefling racial (Infernal Rebuke).
            AbilityScoreType::Charisma,
            // CHA save per RAW: the target resists the psychic trauma
            // with willpower / charisma rather than reflex or grit —
            // matches the "reels from its horrific experience" clause
            // in the RAW text. Distinct from Rebuke the Violent's WIS
            // save (spiritual retribution) and Wrath of the Storm's
            // DEX save (reflexive dodge of a lightning bolt).
            AbilityScoreType::Charisma,
            // 10d10 psychic — matches the RAW damage die pool.
            Dice::new(10, 10),
            DamageType::Psychic,
            "hurl through hell",
        )
    }
}

pub static HURL_THROUGH_HELL: LazyLock<HurlThroughHell> = LazyLock::new(|| HurlThroughHell {});

/// 5e Cleric level-5 class feature — **Destroy Undead**. Passive
/// template flag rather than a per-rest charge — RAW is "starting at
/// 5th level, when an undead fails its saving throw against your Turn
/// Undead feature, the creature is instantly destroyed if its CR is at
/// or below a certain threshold." The threshold ramps with cleric
/// level (CR ½ at lv5, CR 1 at lv8, CR 2 at lv11, CR 3 at lv14, CR 4
/// at lv17); we collapse to a single threshold (CR 1) that matches the
/// baseline cleric's target playable-level window (roughly lv5-8).
///
/// Ships on the baseline `CLERIC_TEMPLATE` (inheriting onto War /
/// Light / Tempest / Devotion / any future subclass cleric) as a
/// passive `has_passive_feature` tag rather than a resource charge —
/// the trigger fires implicitly on every Turn Undead cast without
/// costing an extra action or slot. Read inside `resolve_turn_burst`:
/// when the caster carries this tag AND the failed-save target is
/// Undead AND its CR ≤ threshold, the target is destroyed (dealt HP-
/// killing radiant damage) instead of Frightened. Falls back to the
/// standard Frighten install when any of the three gates fails —
/// higher-CR undead still get the fear treatment, non-undead never
/// fire the destroy branch, and a cleric without the passive tag
/// runs vanilla Turn Undead.
///
/// The destroy damage rides as radiant (Turn Undead is a divine
/// effect and undead commonly carry radiant vulnerability, so a
/// radiant-typed kill blast reads cleanly) and equals the target's
/// current HP so the DealDamage pipeline resolves to a clean kill
/// even against a target with an idiosyncratic resistance profile;
/// we don't overshoot to `max_hp` (which would trigger the massive-
/// damage instant-kill lane and needlessly bypass Death Ward /
/// Undying Sentinel — those are irrelevant against Undead in RAW,
/// but the codepath stays uniform).
///
/// Distinct from the `TURN_UNDEAD_TAG` per-rest charge — that tag
/// gates the action's availability while `DESTROY_UNDEAD_TAG` gates
/// the on-fail branch. Both must be present on the caster for the
/// destroy to fire; a cleric who has spent Turn Undead can't destroy
/// because they can't Turn. Sibling to `FONT_OF_INSPIRATION_TAG`
/// (Bard) and `SORCEROUS_RESTORATION_TAG` (Sorcerer) on the always-
/// on passive-feature lane — permanent unlocks that piggy-back on
/// another action's effect rather than exposing an action of their
/// own.
pub const DESTROY_UNDEAD_TAG: &str = "cleric.destroy_undead";

/// CR threshold for `DESTROY_UNDEAD_TAG` — undead with CR at or below
/// this ceiling are destroyed outright by a failed Turn Undead save.
/// Locked at 1.0 to match the baseline cleric's target playable-level
/// window (roughly lv5-8 on the RAW ramp: CR ½ at lv5, CR 1 at lv8);
/// the ramp itself collapses to a single value to keep the passive
/// gate a one-line check inside `resolve_turn_burst`.
pub const DESTROY_UNDEAD_CR_CEILING: f32 = 1.0;

/// 5e Zealot Barbarian **Zealous Presence** (subclass level 10).
/// Once-per-long-rest bonus action: up to 10 allies within 60ft
/// (24 tiles on the 2.5ft grid) gain the Blessed condition (RAW: "each
/// creature of your choice within 60 feet of you has advantage on
/// attack rolls and saving throws until the end of your next turn").
///
/// RAW's "advantage on attacks and saves until end of your next turn"
/// clause collapses to the Blessed condition for 10 rounds (1
/// minute) — the engine's Blessed models the +1d4 attack / save
/// bonus from the Bless spell, which is the closest sibling on the
/// "buff attacks and saves" lane. The over-tuning (advantage →
/// flat +1d4) is small in practice: any single ally swing that
/// benefits from the bump is still a step ahead of the un-inspired
/// baseline; the extended timer approximates the "concentration-free
/// team buff" flavor of RAW Zealous Presence which lets the raging
/// zealot lay down a mass buff without eating the concentration slot
/// that Bless would occupy.
///
/// Ships on `ZEALOT_BARBARIAN_TEMPLATE` above the strict RAW lv10
/// gate — the CR-4 (level-9) baseline is one level shy — for the same
/// reason Divine Fury (RAW lv3) rides on the same chassis and Iron
/// Mind (RAW lv7) does: class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// The Blessed install routes through the standard `ApplyCondition`
/// chokepoint so aura suppressors, condition immunities, and the
/// combat-active filter that everything else uses all fire cleanly —
/// e.g. a downed / non-combat-active ally won't pick up the buff
/// since `ally_burst_targets` already filters on `is_combat_active`.
pub const ZEALOUS_PRESENCE_TAG: &str = "barbarian.zealous_presence";

/// Maximum number of allies that can be buffed by a single Zealous
/// Presence cast. RAW: "up to ten creatures of your choice within 60
/// feet". Locked at 10 here as a defensive cap so a mass-battle with
/// dozens of allies doesn't overrun the buff pool — matches PHB.
pub const ZEALOUS_PRESENCE_MAX_TARGETS: usize = 10;

/// Zealous Presence — Zealot Barbarian class-feature action. Bonus
/// action, once per long rest. Every combat-active ally (including
/// the caster) within 24 tiles (60ft RAW) gains the Blessed condition
/// for 10 rounds (1 minute). Sibling to Bardic Inspiration on the
/// ally-buff lane, but a *burst* rather than a single-target grant —
/// where Bardic Inspiration spends one charge to give one ally a
/// bonus, Zealous Presence spends one charge to blanket the whole
/// nearby team at once.
///
/// Routes through the shared `spend_feature_and_install_ally_burst`
/// helper so a future ally-burst class-feature buff (a hypothetical
/// Bard "Song of Freedom" installing Purified, a Cleric CD ally-buff
/// aura, etc.) drops in as a fresh call with a distinct (feature
/// tag, condition, timer, radius, max targets) tuple rather than a
/// hand-rolled `ally_burst_targets → spend_feature → for each →
/// ApplyCondition` chain.
pub struct ZealousPresence {}

impl Action for ZealousPresence {
    fn name(&self) -> &str {
        "zealous presence"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["zp", "presence", "zealous"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Self-centered burst — the "target" is the caster themselves;
        // the burst radius covers the ally cohort. Same shape as
        // Turn Undead / Radiance of the Dawn (self-centered burst
        // action) but ally-flavored rather than enemy-flavored.
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_ready(encounter, caster_id, ZEALOUS_PRESENCE_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        spend_feature_and_install_ally_burst(
            encounter,
            caster_id,
            ZEALOUS_PRESENCE_TAG,
            // 60ft RAW = 24 tiles on the 2.5ft grid.
            24,
            ZEALOUS_PRESENCE_MAX_TARGETS,
            Condition::Blessed,
            // 10 rounds (1 minute) — the standard "medium-duration
            // team buff" window, matching Aura of Purity / Aura of
            // Vitality / Bardic Inspiration's cadence. RAW's "until
            // end of your next turn" collapses upward here since
            // Zealous Presence has no concentration cost — the extra
            // rounds trade for the RAW-tighter timer.
            ConditionTimer::Rounds(10),
            "zealous presence",
        )
    }
}

pub static ZEALOUS_PRESENCE: LazyLock<ZealousPresence> = LazyLock::new(|| ZealousPresence {});

/// Shared "spend a once-per-rest feature charge on the caster, then
/// install `condition` for `timer` on up to `max_targets` combat-active
/// allies within `radius` of the caster (including the caster
/// themselves)" body. Centralizes the shape that Zealous Presence uses
/// and that a future ally-burst class-feature buff (a hypothetical
/// Bard "Song of Freedom" burst installing Purified, a Cleric CD ally-
/// aura, etc.) will use — adding a fresh caller lands as one call with
/// a distinct (feature tag, condition, timer, radius, max targets)
/// tuple rather than a hand-rolled `ally_burst_targets → spend_feature
/// → for each → ApplyCondition` chain.
///
/// Uses `ally_burst_targets` for the target sweep — the same helper
/// Prayer of Healing / Beacon of Hope / Aura of Purity use. Truncates
/// to `max_targets` after sorting by id so the sort order is stable
/// and the truncation cut point is deterministic across seed sweeps.
///
/// Sibling to `resolve_single_target_cd_save_condition` on the class-
/// feature condition-install lane — that helper installs a condition
/// on a *single hostile target* via a save roll, this one installs a
/// condition on *up to N friendly targets* without a save (RAW ally
/// buffs don't allow the target to "save" against being buffed).
/// Sibling to `resolve_single_target_burst_save_for_half` on the
/// class-feature damage-burst lane — that helper deals save-for-half
/// damage to one hostile target, this one blanket-buffs allies.
///
/// Spends the feature charge before the install loop so a caster with
/// an unspent charge always pays the cost even if zero allies are in
/// range — matches the RAW "you use the feature" semantics where the
/// charge is spent on the action, not on the successful install. A
/// caller that wants a "no allies in range → skip the spend" gate can
/// pre-filter via `custom_validate_input` (Zealous Presence currently
/// doesn't since the caster themselves is always a valid target).
#[allow(clippy::too_many_arguments)]
fn spend_feature_and_install_ally_burst(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    feature_tag: &'static str,
    radius: isize,
    max_targets: usize,
    condition: Condition,
    timer: ConditionTimer,
    label: &'static str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
        return Vec::new();
    };
    if let Some(actor) = encounter.actors.get_mut(&caster_id) {
        actor.spend_feature(feature_tag);
    }
    let mut targets = encounter.ally_burst_targets(caster_id, caster_loc, radius);
    targets.truncate(max_targets);
    let n = targets.len();
    encounter.log(format!(
        "  {}: {} {} rallied with resolve.",
        label,
        n,
        if n == 1 { "ally" } else { "allies" }
    ));
    targets
        .into_iter()
        .map(|id| {
            Box::new(ApplyCondition {
                actor_id: id,
                condition,
                timer,
            }) as Box<dyn ApplicableSideEffect>
        })
        .collect()
}

/// 5e Ranger **Roving** (optional class feature, 2024 PHB level 6).
/// Passive: the ranger's walking speed increases by 5 feet, and they
/// gain a climbing speed and a swimming speed matching that walking
/// speed. In this engine only the flat +5 ft walking-speed bump has a
/// combat surface — climbing / swimming speeds fold into the same
/// `speed()` accessor (no 3D terrain to differentiate). Read at the
/// shared `passive_feature_speed_bonus` chokepoint in
/// `condition_speed_bonus` next to the barbarian's Fast Movement
/// (+10 ft) and Tiger Totem (+10 ft while raging) — one lookup table,
/// one source of truth.
///
/// Sibling to `FAST_MOVEMENT_TAG` on the always-on passive-speed-bump
/// lane, but weaker (+5 vs +10) — the ranger doesn't sprint like a
/// raging barbarian, they just kite half a step further out. Composes
/// naturally with the ranger's Longstrider self-buff (+10 ft, 1 hour):
/// a Roving ranger pre-casting Longstrider on themselves opens the
/// fight at +15 ft over a stock 30-ft baseline (45 ft = 18 tiles on
/// our 2.5-ft grid), a full extra move on the opening round.
///
/// Ships on `RANGER_TEMPLATE` (and inherits to `HUNTER_RANGER_TEMPLATE`
/// via `..RANGER_TEMPLATE.clone()`) above its strict RAW lv6 level
/// gate for the same reason Foe Slayer (lv20) and Feral Senses (lv18)
/// already ride the CR-1 baseline template — class templates target
/// a balanced playable level, not lockstep PHB progression.
///
/// Always-on passive; no per-rest charge and no condition gate. The
/// tag lives in the actor's `features` pool, not in
/// `SHORT_REST_FEATURES` / long-rest tables — nothing consumes it and
/// nothing refreshes it.
pub const ROVING_TAG: &str = "ranger.roving";

/// Flat walking-speed bonus in feet granted by the Roving passive.
/// Pinned to +5 ft per RAW; exposed as a constant so the
/// `passive_feature_speed_bonus` table stays declarative rather than
/// scattering magic numbers into the accessor.
pub const ROVING_SPEED_BONUS: f32 = 5.0;

/// 5e Scout Rogue **Superior Mobility** (subclass level 9, XGtE).
/// Passive: the scout's walking speed increases by 10 feet, and they
/// also gain climbing and swimming speeds matching that walking speed.
/// In this engine only the flat +10 ft walking-speed bump has a combat
/// surface — climbing / swimming speeds fold into the same `speed()`
/// accessor (no 3D terrain to differentiate). Read at the shared
/// `passive_feature_speed_bonus` chokepoint in `condition_speed_bonus`
/// next to the barbarian's Fast Movement (+10 ft), the ranger's Roving
/// (+5 ft), and the monk's Unarmored Movement (+10 ft) — one lookup
/// table, one source of truth.
///
/// Sibling to `FAST_MOVEMENT_TAG` / `UNARMORED_MOVEMENT_TAG` on the
/// always-on passive-speed-bump lane at the same magnitude (+10 ft) —
/// three unrelated class chassis converge on the same "mid-level +10
/// walking-speed capstone" identity. Distinct from Roving (+5 ft) by
/// the higher magnitude and the different subclass source (Scout Rogue
/// vs. baseline Ranger optional feature); the two never legally
/// co-occur on a single build (Scout is a Rogue subclass, Roving is a
/// Ranger class feature) but a hypothetical multiclass carrier would
/// stack both under the additive-cohort rule.
///
/// Ships on `SCOUT_ROGUE_TEMPLATE` (subclass build on the CR-1 rogue
/// chassis) above its strict RAW lv9 gate for the same reason
/// `ASSASSIN_ROGUE_TEMPLATE` / `SWASHBUCKLER_ROGUE_TEMPLATE` ship their
/// lv3 subclass features on the CR-1 baseline chassis — class templates
/// target a balanced playable level, not lockstep PHB progression.
///
/// Always-on passive; no per-rest charge and no condition gate. The
/// tag lives in the actor's `features` pool, not in
/// `SHORT_REST_FEATURES` / long-rest tables — nothing consumes it and
/// nothing refreshes it.
pub const SUPERIOR_MOBILITY_TAG: &str = "rogue.superior_mobility";

/// Flat walking-speed bonus in feet granted by Scout Rogue's Superior
/// Mobility passive. Pinned to +10 ft per RAW; exposed as a constant so
/// the `passive_feature_speed_bonus` table stays declarative rather
/// than scattering magic numbers into the accessor. Sibling to
/// `FAST_MOVEMENT_SPEED_BONUS` (+10) / `UNARMORED_MOVEMENT_SPEED_BONUS`
/// (+10) at the same magnitude; sibling to `ROVING_SPEED_BONUS` (+5) at
/// half magnitude.
pub const SUPERIOR_MOBILITY_SPEED_BONUS: f32 = 10.0;

/// 5e Life Domain Cleric **Disciple of Life** (level 1 subclass feature).
/// Passive: whenever the cleric uses a spell of level 1 or higher to
/// restore hit points to a creature, that creature regains an additional
/// `2 + slot_level` HP. The bonus rides once per creature per cast, not
/// per healing die — so a Cure Wounds at level 1 heals `1d8 + WIS + 3`
/// and Mass Cure Wounds at level 5 heals `3d8 + WIS + 7` to *each*
/// creature caught in the burst.
///
/// Read at every leveled cleric-heal chokepoint (`HealSpell` for Cure
/// Wounds / Healing Word, plus the ad-hoc `MassHealingWord` /
/// `MassCureWounds` sites) via `disciple_of_life_bonus(caster,
/// slot_level)`. The helper folds the "must be spell of lv≥1"
/// gate — cantrip heals (Spare the Dying's stabilize is not a heal,
/// and the current cantrip catalog has none) return 0 either way.
///
/// Ships on `LIFE_CLERIC_TEMPLATE` (the RAW gate is the domain pick at
/// character creation — no RAW level gate to compare against). Sibling
/// to the War / Light / Tempest domain subclass flavors — those layer
/// on damage-oriented Channel Divinities (Guided Strike / Radiance of
/// the Dawn / Wrath of the Storm), Life leans into the pure-support
/// flavor by amplifying the healing lane the baseline cleric already
/// carries. No per-rest charge and no condition gate — always-on
/// passive read at the heal-amount site.
pub const DISCIPLE_OF_LIFE_TAG: &str = "cleric.disciple_of_life";

/// Compute the RAW Disciple of Life bonus for a heal cast at
/// `spell_slot_lvl`. Returns `2 + spell_slot_lvl` when `caster` has the
/// passive tag AND the slot is level 1+; returns 0 for the cantrip
/// tier (no slot) or a caster without the tag. Callers add the return
/// value onto their computed heal amount right before constructing the
/// `Heal` side-effect so the boost hits every eligible target.
///
/// Single chokepoint so future changes (e.g. RAW-tightening to only
/// spells the cleric prepared as a Life Domain spell) land in one
/// place instead of across the ~4-5 heal-spell impls that call it.
pub fn disciple_of_life_bonus(
    caster: &crate::actors::actor_template::ActorInstance,
    spell_slot_lvl: u32,
) -> u32 {
    if spell_slot_lvl == 0 || !caster.has_passive_feature(DISCIPLE_OF_LIFE_TAG) {
        return 0;
    }
    2 + spell_slot_lvl
}

/// Companion to `disciple_of_life_bonus`: format the log-line suffix
/// callers splice into their existing heal-log format string right
/// before the ` = <amount> HP` tail. Returns `""` for `bonus == 0`
/// (non-Life caster / cantrip heal) so the base log stays untouched,
/// or `"+<n>(disciple of life)"` when the bonus fired. Keeps every
/// leveled-heal call site to a single `format!` with a stitched-in
/// `{}` rather than the earlier `if bonus > 0 { format!(...) } else
/// { format!(...) }` two-branch shape at each site.
pub fn disciple_of_life_log_suffix(bonus: u32) -> String {
    if bonus == 0 {
        String::new()
    } else {
        format!("+{}(disciple of life)", bonus)
    }
}

/// 5e Grave Domain Cleric — **Circle of Mortality** subclass feature tag
/// (Grave subclass level 1, XGtE). Passive: whenever the cleric would
/// normally roll one or more dice to restore hit points with a spell to
/// a creature at 0 hit points, they instead use the highest number
/// possible for each die.
///
/// The signature "the grave cleric drags allies back from the edge with
/// perfect precision" tell — where a baseline Cleric casts Cure Wounds
/// on a downed ally and rolls `1d8 + WIS`, the Grave Cleric picks up the
/// same slot and guarantees `8 + WIS` HP through the max-dice floor.
/// Pairs naturally with the cleric's heal kit (Cure Wounds, Healing
/// Word, Mass Cure Wounds, Mass Healing Word) — the Grave Cleric's
/// clutch-heal on a dying party member trades variance for reliability
/// exactly when it matters most (a bad 1d8 roll leaves the ally at
/// `1 + WIS` HP and still one hit from the ground).
///
/// The "at 0 hit points" gate covers both Dying (rolling death saves)
/// AND Stable (stabilized at 0 HP) actors — both share the same
/// `hitpoints() == 0` predicate. Downed monsters (which don't roll
/// death saves) never reach the gate: they're removed from the encounter
/// on the killing blow, so the tag's gate is de-facto ally-facing on the
/// PC-vs-monster axis this engine models.
///
/// Read at every leveled cleric-heal chokepoint (`HealSpell` for Cure
/// Wounds / Healing Word, plus the ad-hoc `MassHealingWord` /
/// `MassCureWounds` sites) via `should_use_max_heal_dice(caster,
/// target)`. The helper folds the "caster must hold the tag AND target
/// must be at 0 HP" compound gate — a Grave Cleric casting on a healthy
/// ally returns false and the stock roll fires unchanged. The
/// fixed-70-HP `HealSpellHigh` (Heal spell) never rolls dice, so the tag
/// has no effect there (already at maximum by construction).
///
/// Sibling to `DISCIPLE_OF_LIFE_TAG` (Life Cleric) on the "cleric domain
/// heal-amplifier" lane — Disciple of Life adds `2 + slot_level` to
/// every leveled heal, Circle of Mortality replaces the dice roll with
/// its max when the target is at 0 HP. The two never legally co-occur
/// on a single PC build (RAW: one Divine Domain pick per cleric), but
/// under the multiclass rule via the same heal chokepoint they'd stack
/// additively — a hypothetical Life-Grave multi-domain cleric would get
/// both max-rolled dice AND the flat Disciple bonus on a downed ally.
///
/// Ships on `GRAVE_CLERIC_TEMPLATE` (the RAW gate is the domain pick at
/// character creation — no RAW level gate to compare against). Sibling
/// Channel Divinity `PATH_TO_THE_GRAVE_TAG` already rides on the same
/// chassis; this tag completes the two RAW lv1-lv2 always-on Grave
/// Domain features on the template.
pub const CIRCLE_OF_MORTALITY_TAG: &str = "cleric.circle_of_mortality";

/// True when the caster's Circle of Mortality gate fires: caster holds
/// the `CIRCLE_OF_MORTALITY_TAG` AND the target is at 0 hit points
/// (Dying or Stable). Callers use this to decide whether to substitute
/// max-die-face totals for the rolled heal dice — a `true` return means
/// "use `dice.max_roll()` for the raw", a `false` return means "roll
/// normally".
///
/// Single chokepoint so future changes (e.g. RAW-tightening to
/// spells the cleric prepared as a Grave Domain spell, or extending the
/// gate to short-of-max-HP allies for a hypothetical variant) land in
/// one place instead of across the ~4-5 heal-spell impls that call it.
pub fn should_use_max_heal_dice(
    caster: &crate::actors::actor_template::ActorInstance,
    target: &crate::actors::actor_template::ActorInstance,
) -> bool {
    caster.has_passive_feature(CIRCLE_OF_MORTALITY_TAG) && target.hitpoints() == 0
}

/// Companion to `should_use_max_heal_dice`: format the log-line suffix
/// callers splice into their existing heal-log format string right
/// before the ` = <amount> HP` tail. Returns `""` when the gate didn't
/// fire so the base log stays untouched, or `"(circle of mortality)"`
/// when the substitution fired. Keeps every heal call site to a single
/// `format!` with a stitched-in `{}` rather than a branchy `if used`
/// duplicated at each site — same shape the sibling
/// `disciple_of_life_log_suffix` uses on the same heal chokepoints.
pub fn circle_of_mortality_log_suffix(used: bool) -> &'static str {
    if used {
        "(circle of mortality)"
    } else {
        ""
    }
}

/// 5e Warlock Otherworldly Patron — **The Genie (Djinni)** — **Elemental
/// Gift** subclass feature tag (level 6, TCE). Passive: the Djinni-pact
/// warlock gains **resistance to thunder damage** — the djinni's sky-
/// and storm-flavored patron pact leaks its elemental affinity into the
/// warlock's own resilience, hardening them against thundercracks,
/// Shatter bursts, and Thunderwave shoves.
///
/// Third of the four RAW genie-kind variants of Elemental Gift to land
/// on the passive typed-resistance cohort — completes the four-quadrant
/// physical-element coverage grid on the "Genie patron" lane alongside
/// **Marid** (Cold, water/ice) and **Dao** (Bludgeoning, earth/stone).
/// The remaining **Efreeti** (Fire) variant is a semantic duplicate of
/// Fiendish / Draconic Resilience on the resistance axis (all three
/// share the Fire lane), so it would land as an alias tag rather than
/// a fresh damage slot; that variant is left as future work in favor
/// of the three genie kinds whose damage axes are otherwise uncovered.
///
/// RAW's Elemental Gift picks a damage type based on the warlock's
/// chosen genie kind: **Dao** (bludgeoning), **Djinni** (thunder),
/// **Efreeti** (fire), **Marid** (cold). This tag ships the Djinni
/// variant alongside the pre-existing Marid (`MARID_ELEMENTAL_GIFT_TAG`) and
/// Dao (`DAO_ELEMENTAL_GIFT_TAG`) variants — the three never legally
/// co-occur on a single build (RAW: one genie kind per warlock), so the
/// split-tag shape is a template-drift lock rather than a stacking
/// concern. Overlaps the Thunder axis with **Heart of the Storm** on
/// the Storm Sorcerer chassis (a different class), matching how the
/// Fire axis overlaps between Fiendish / Draconic Resilience today —
/// the shared "one halving per damage instance" rule caps a
/// hypothetical multiclass Storm-Sorcerer / Djinni-Warlock at a single
/// /2 per Thunder hit; the double coverage is a taxonomic tell rather
/// than a stacking bug.
///
/// RAW's Elemental Gift also grants a per-day 10-minute **flying speed
/// equal to walking speed** clause; the flight half needs a per-cast
/// timer / activated-buff surface not yet wired on this chassis. Left
/// as future work — the resistance clause is the load-bearing defensive
/// half and rides here alone, matching the way the Marid and Dao
/// variants ship without the flight half.
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to Marid's Cold row, Dao's Bludgeoning row,
/// Radiant Soul's radiant row, Heart of the Storm's lightning + thunder
/// row, and Psychic Defenses' psychic row. Single-type slice (Thunder
/// only) — same shape as the Marid Cold / Dao Bludgeoning / Radiant
/// Soul Radiant rows on the single-type-per-genie-kind lane.
///
/// Sibling to `MARID_ELEMENTAL_GIFT_TAG` (Marid, Cold) and
/// `DAO_ELEMENTAL_GIFT_TAG` (Dao, Bludgeoning) on the Genie patron
/// lane — same "one feature tag drives one cohort row" declarative-
/// table pattern, different damage axis. Sibling to `RADIANT_SOUL_TAG`
/// / `HEART_OF_THE_STORM_TAG` / `PSYCHIC_DEFENSES_TAG` on the
/// Otherworldly Patron / Sorcerous Origin subclass passive lane — same
/// "one feature tag drives one cohort row" declarative-table pattern.
///
/// Ships on `DJINNI_WARLOCK_TEMPLATE` — the Otherworldly Patron: The
/// Genie (Djinni) subclass template — alongside the baseline Warlock
/// envelope. Distinct from `MARID_WARLOCK_TEMPLATE` (Cold),
/// `DAO_WARLOCK_TEMPLATE` (Bludgeoning), `FIEND_WARLOCK_TEMPLATE`
/// (Fire), `UNDYING_WARLOCK_TEMPLATE` / `GREAT_OLD_ONE_WARLOCK_TEMPLATE`
/// / `ARCHFEY_WARLOCK_TEMPLATE` / `CELESTIAL_WARLOCK_TEMPLATE` (their
/// respective single-patron flavor tells), and the baseline
/// `WARLOCK_TEMPLATE` (patron-less baseline).
///
/// Ships on the CR-4 template above the strict RAW lv6 gate for the
/// same reason `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` ship
/// Elemental Gift (RAW lv6) — class templates target a balanced
/// playable level, not lockstep PHB progression.
pub const DJINNI_ELEMENTAL_GIFT_TAG: &str = "warlock.djinni_elemental_gift";

/// 5e Warlock Otherworldly Patron — **The Genie (Efreeti)** — **Elemental
/// Gift** subclass feature tag (level 6, TCE). Passive: the Efreeti-pact
/// warlock gains **resistance to fire damage** — the efreeti's flame-
/// and desert-flavored patron pact leaks its elemental affinity into the
/// warlock's own resilience, hardening them against Burning Hands,
/// Fireball, Wall of Fire, and every other blaze the fiery lords of the
/// Elemental Plane of Fire have ever taught a mortal to cast.
///
/// Fourth (and final) of the four RAW genie-kind variants of Elemental
/// Gift to land on the passive typed-resistance cohort — completes the
/// four-quadrant Genie patron coverage grid alongside **Marid** (Cold,
/// water/ice), **Dao** (Bludgeoning, earth/stone), and **Djinni**
/// (Thunder, sky/storm). The four never legally co-occur on a single
/// build (RAW: one genie kind picked at lv1), so the split-tag shape is
/// a template-drift lock rather than a stacking concern.
///
/// Semantic duplicate on the resistance axis of `FIEND_WARLOCK_TEMPLATE`'s
/// **Fiendish Resilience** (Warlock Fiend Patron lv10) and
/// `DRACONIC_SORCERER_TEMPLATE`'s **Draconic Resilience** (Sorcerer
/// Draconic Bloodline lv6) — all three cover the Fire axis. The
/// semantic duplication is a **taxonomic completeness** grant, not a
/// mechanical-coverage grant: the Efreeti variant lands so the four-
/// genie Genie patron family reads as a full quadrant on the map even
/// though the Fire axis is already covered by two other passive-
/// resistance rows on distinct chassis. The three Fire-resistance rows
/// never legally co-occur on a single build (Barbarian vs. Warlock
/// Fiend vs. Warlock Efreeti vs. Sorcerer Draconic), and a hypothetical
/// multiclass carrier caps at a single /2 per Fire hit under the "one
/// halving per damage instance" rule — the triple coverage is a
/// taxonomic tell rather than a stacking bug.
///
/// RAW's Elemental Gift also grants a per-day 10-minute **flying speed
/// equal to walking speed** clause; the flight half needs a per-cast
/// timer / activated-buff surface not yet wired on this chassis. Left
/// as future work — the resistance clause is the load-bearing defensive
/// half and rides here alone, matching the way the Marid / Dao / Djinni
/// variants each ship without the flight half.
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to Marid's Cold row, Dao's Bludgeoning row,
/// Djinni's Thunder row, Radiant Soul's radiant row, Fiendish /
/// Draconic Resilience's fire rows, Storm Soul (Sea / Desert / Tundra)'s
/// Lightning / Fire / Cold rows, Heart of the Storm's lightning +
/// thunder row, Psychic Defenses' psychic row, and Inured to Undeath's
/// necrotic row. Single-type slice (Fire only) — same shape as the
/// Marid Cold / Dao Bludgeoning / Djinni Thunder / Radiant Soul
/// Radiant / Inured to Undeath Necrotic single-type rows on the
/// single-type-per-subclass lane.
///
/// Sibling to `MARID_ELEMENTAL_GIFT_TAG` (Marid, Cold),
/// `DAO_ELEMENTAL_GIFT_TAG` (Dao, Bludgeoning), and
/// `DJINNI_ELEMENTAL_GIFT_TAG` (Djinni, Thunder) on the Genie patron
/// lane — same "one feature tag drives one cohort row" declarative-
/// table pattern, different damage axis. Sibling to `RADIANT_SOUL_TAG`
/// / `HEART_OF_THE_STORM_TAG` / `PSYCHIC_DEFENSES_TAG` on the
/// Otherworldly Patron / Sorcerous Origin subclass passive lane — same
/// "one feature tag drives one cohort row" declarative-table pattern.
///
/// Ships on `EFREETI_WARLOCK_TEMPLATE` — the fourth Otherworldly
/// Patron: The Genie subclass template — alongside the baseline Warlock
/// envelope. Distinct from `MARID_WARLOCK_TEMPLATE` (Cold),
/// `DAO_WARLOCK_TEMPLATE` (Bludgeoning), `DJINNI_WARLOCK_TEMPLATE`
/// (Thunder), `FIEND_WARLOCK_TEMPLATE` (Fire, via the struct-field
/// `has_fiendish_resilience` flag rather than a feature tag),
/// `UNDYING_WARLOCK_TEMPLATE` / `GREAT_OLD_ONE_WARLOCK_TEMPLATE` /
/// `ARCHFEY_WARLOCK_TEMPLATE` / `CELESTIAL_WARLOCK_TEMPLATE` (their
/// respective single-patron flavor tells), and the baseline
/// `WARLOCK_TEMPLATE` (patron-less baseline).
///
/// Ships on the CR-4 template above the strict RAW lv6 gate for the
/// same reason `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` /
/// `DJINNI_WARLOCK_TEMPLATE` ship Elemental Gift (RAW lv6) — class
/// templates target a balanced playable level, not lockstep PHB
/// progression.
pub const EFREETI_ELEMENTAL_GIFT_TAG: &str = "warlock.efreeti_elemental_gift";

/// 5e Wizard Arcane Tradition — **School of Necromancy** — **Inured to
/// Undeath** subclass feature tag (level 10, PHB). Passive: the
/// necromancer's long study of death and undeath leaves the body
/// hardened against the necrotic touch — the wizard gains **resistance
/// to necrotic damage**.
///
/// RAW pairs the necrotic resistance with a "your hit-point maximum
/// can't be reduced" clause — the engine doesn't yet model max-HP-drain
/// mechanics (Wraith / Vampire's life-drain hit rider) as a first-class
/// surface, so the max-HP-can't-be-reduced clause has no combat surface
/// to gate today and is left as future work. The resistance clause is
/// the load-bearing defensive half and rides here alone, matching the
/// way Radiant Soul ships without the +CHA-mod damage rider and Marid /
/// Dao / Djinni Elemental Gift each ship without the per-day flight
/// clause.
///
/// First user of the **Necrotic** slot on the passive typed-resistance
/// lane — Cold is owned by Marid's `MARID_ELEMENTAL_GIFT_TAG`, Fire by
/// Fiendish / Draconic Resilience, Lightning + Thunder by Heart of the
/// Storm, Psychic by Psychic Defenses, Radiant by Radiant Soul,
/// Bludgeoning by Dao's `DAO_ELEMENTAL_GIFT_TAG`, and Thunder by
/// Djinni's `DJINNI_ELEMENTAL_GIFT_TAG`; Necrotic was uncovered on the
/// passive-typed-resistance cohort until this row lands. Necrotic is a
/// signature damage type of the wizard's own undead spell list
/// (Chill Touch cantrip, Ray of Enfeeblement, Vampiric Touch, Blight,
/// Circle of Death, Finger of Death, Negative Energy Flood) — the
/// necromancer's own kit stops trickling back onto its own chassis on
/// a friendly-fire miscast under the halving rule.
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to the Warlock Elemental Gift rows (Marid
/// Cold / Dao Bludgeoning / Djinni Thunder), Radiant Soul's radiant
/// row, Fiendish / Draconic Resilience's fire rows, Heart of the
/// Storm's lightning + thunder row, and Psychic Defenses' psychic row.
/// Single-type slice (Necrotic only) — same shape as the Marid Cold /
/// Dao Bludgeoning / Djinni Thunder / Radiant Soul Radiant single-type
/// rows on the single-type-per-subclass lane.
///
/// Sibling to `MARID_ELEMENTAL_GIFT_TAG` / `DAO_ELEMENTAL_GIFT_TAG` /
/// `DJINNI_ELEMENTAL_GIFT_TAG` / `RADIANT_SOUL_TAG` /
/// `HEART_OF_THE_STORM_TAG` / `PSYCHIC_DEFENSES_TAG` on the "one
/// feature tag drives one cohort row" declarative-table pattern —
/// different source chassis (Wizard Arcane Tradition vs. Warlock
/// Otherworldly Patron / Sorcerer Sorcerous Origin) and different
/// damage axis.
///
/// Ships on `NECROMANCY_WIZARD_TEMPLATE` — the first Wizard Arcane
/// Tradition subclass template on the wizard chassis (the baseline
/// `WIZARD_TEMPLATE` shipped no subclass template before this feature).
/// Distinct from the baseline `WIZARD_TEMPLATE` (patron-less baseline
/// with Arcane Recovery).
///
/// Ships on the CR-0.5 wizard chassis above the strict RAW lv10 gate
/// for the same reason `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE`
/// / `DJINNI_WARLOCK_TEMPLATE` ship Elemental Gift (RAW lv6) and
/// `ABERRANT_MIND_SORCERER_TEMPLATE` ships Psychic Defenses (RAW lv14)
/// — class templates target a balanced playable level, not lockstep
/// PHB progression.
pub const INURED_TO_UNDEATH_TAG: &str = "wizard.inured_to_undeath";

/// 5e Barbarian Primal Path — **Path of the Storm Herald (Sea)** —
/// **Storm Soul (Sea)** subclass feature tag (level 6, XGtE). Passive:
/// the sea storm herald's body absorbs the tempest's fury — the
/// barbarian gains **resistance to lightning damage**.
///
/// RAW pairs the lightning resistance with an "can breathe underwater
/// and gains a swimming speed equal to your walking speed" clause. The
/// engine doesn't model swim tiles or drowning as first-class combat
/// surfaces, so both the swim speed and the water-breathing halves are
/// non-op in combat and are left as future work. The resistance clause
/// is the load-bearing defensive half and rides here alone, matching
/// the way `INURED_TO_UNDEATH_TAG` ships without the max-HP-can't-be-
/// reduced clause and Radiant Soul ships without the +CHA-mod damage
/// rider.
///
/// First Barbarian-chassis row on the passive typed-resistance lane —
/// every prior row (Dwarven / Fiendish / Draconic / Heart of the Storm
/// / Psychic Defenses / Radiant Soul / Marid / Dao / Djinni Elemental
/// Gift / Inured to Undeath) came off a racial trait or a Warlock /
/// Sorcerer / Wizard subclass. The Barbarian chassis's rage-gated
/// broad resistance (Bear Totem, `RAGE_GATED_BROAD_RESISTANCES`) is
/// distinct on both axis (broad, not typed) and gate (rage-gated, not
/// always-on) from this always-on typed-resistance grant.
///
/// Overlaps the Lightning axis with **Heart of the Storm** (Storm
/// Sorcerer lv6, Lightning + Thunder) on a different chassis — the
/// two never legally co-occur on a single build (Barbarian vs.
/// Sorcerer subclass), and a hypothetical multiclass carrier caps at a
/// single /2 per Lightning hit via the "one halving per damage
/// instance" rule.
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to Heart of the Storm's lightning +
/// thunder row, the Warlock Elemental Gift rows (Marid Cold / Dao
/// Bludgeoning / Djinni Thunder), Radiant Soul's radiant row,
/// Fiendish / Draconic Resilience's fire rows, Psychic Defenses'
/// psychic row, and Inured to Undeath's necrotic row. Single-type
/// slice (Lightning only) — same shape as the Marid Cold / Dao
/// Bludgeoning / Djinni Thunder / Radiant Soul Radiant / Inured to
/// Undeath Necrotic single-type rows on the single-type-per-subclass
/// lane.
///
/// Sibling to `MARID_ELEMENTAL_GIFT_TAG` / `DAO_ELEMENTAL_GIFT_TAG` /
/// `DJINNI_ELEMENTAL_GIFT_TAG` / `RADIANT_SOUL_TAG` /
/// `HEART_OF_THE_STORM_TAG` / `PSYCHIC_DEFENSES_TAG` /
/// `INURED_TO_UNDEATH_TAG` on the "one feature tag drives one cohort
/// row" declarative-table pattern — different source chassis
/// (Barbarian Primal Path vs. Warlock Otherworldly Patron / Sorcerer
/// Sorcerous Origin / Wizard Arcane Tradition) and different damage
/// axis.
///
/// Ships on `SEA_STORM_HERALD_BARBARIAN_TEMPLATE` — the first Path of
/// the Storm Herald subclass template on the barbarian chassis
/// (extending the Bear / Wolf / Eagle / Tiger / Elk / Wolverine /
/// Panther / Berserker / Zealot subclass roster). Distinct from every
/// other barbarian subclass template — the Totem Warrior templates
/// stack rage-gated broad resistance (Bear) or rage-gated speed /
/// ally-aura bumps (Wolf / Eagle / Tiger / Elk / Wolverine / Panther),
/// while the Storm Herald layers on always-on typed resistance
/// regardless of rage state.
///
/// RAW's Storm Herald picks up other features not shipped on this
/// template — **Storm Aura (Sea)** (lv3: while raging, one enemy
/// within 10ft eats a DEX-save lightning bolt at the start of each of
/// the barbarian's turns; a per-turn friend-agnostic aura mechanic
/// that needs a per-turn aura fire hook and a target-picking policy),
/// **Shielding Storm** (lv10: allies within 10ft of the raging
/// barbarian ALSO gain Storm Soul benefits; an ally-aura extension
/// mechanic), and **Raging Storm (Sea)** (lv14: reaction-on-attacker-
/// hit STR-save vs. knock-prone rider; a per-attack reactive install
/// mechanic). Only the lv6 Storm Soul passive has a mechanical surface
/// that plugs cleanly into the shared passive typed-resistance cohort,
/// so we ship that half and leave the rest as future work — matching
/// the way `INURED_TO_UNDEATH_TAG` / `MARID_ELEMENTAL_GIFT_TAG` /
/// `DAO_ELEMENTAL_GIFT_TAG` / `DJINNI_ELEMENTAL_GIFT_TAG` each ship
/// only the passive resistance half of their broader RAW kit.
///
/// Ships on the CR-4 (level-9) barbarian chassis at (or above) its
/// strict RAW lv6 gate for the same reason `MARID_WARLOCK_TEMPLATE`
/// / `DAO_WARLOCK_TEMPLATE` / `DJINNI_WARLOCK_TEMPLATE` ship Elemental
/// Gift (RAW lv6), `NECROMANCY_WIZARD_TEMPLATE` ships Inured to
/// Undeath (RAW lv10), and `ABERRANT_MIND_SORCERER_TEMPLATE` ships
/// Psychic Defenses (RAW lv14) — class templates target a balanced
/// playable level, not lockstep PHB progression.
pub const STORM_SOUL_SEA_TAG: &str = "barbarian.storm_soul_sea";

/// 5e Barbarian Primal Path — **Path of the Storm Herald (Desert)** —
/// **Storm Soul (Desert)** subclass feature tag (level 6, XGtE). Passive:
/// the desert storm herald's body absorbs the heat of the sun-scorched
/// dunes — the barbarian gains **resistance to fire damage**.
///
/// RAW pairs the fire resistance with an "immune to extreme heat" clause
/// (the RAW `Adventuring/Environment/Extreme Heat` exhaustion rider) and
/// a "you can ignite an unattended flammable object within 5 ft" ribbon.
/// Neither has a combat surface on today's engine — extreme-heat
/// exhaustion sits outside the tactical loop, and the ignite ribbon is
/// out-of-combat flavor — so both are left as future work. The resistance
/// clause is the load-bearing defensive half and rides here alone,
/// matching the way `STORM_SOUL_SEA_TAG` ships without the swim /
/// water-breathing halves and `INURED_TO_UNDEATH_TAG` ships without the
/// max-HP-can't-be-reduced clause.
///
/// Second Barbarian-chassis row on the passive typed-resistance lane —
/// `STORM_SOUL_SEA_TAG` (Sea, Lightning) blazed the trail; every earlier
/// row (Dwarven / Fiendish / Draconic / Heart of the Storm / Psychic
/// Defenses / Radiant Soul / Marid / Dao / Djinni Elemental Gift /
/// Inured to Undeath) came off a racial trait or a Warlock / Sorcerer
/// / Wizard subclass. The Barbarian chassis's rage-gated broad
/// resistance (Bear Totem, `RAGE_GATED_BROAD_RESISTANCES`) is distinct
/// on both axis (broad, not typed) and gate (rage-gated, not always-on)
/// from this always-on typed-resistance grant.
///
/// Overlaps the Fire axis with **Fiendish Resilience** (Warlock Fiend
/// Patron lv10) and **Draconic Resilience** (Sorcerer Draconic
/// Bloodline lv6) on different chassis — the three never legally
/// co-occur on a single build (Barbarian vs. Warlock vs. Sorcerer
/// subclass), and a hypothetical multiclass carrier caps at a single
/// /2 per Fire hit via the "one halving per damage instance" rule.
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to Storm Soul (Sea)'s lightning row,
/// Fiendish / Draconic Resilience's fire rows, the Warlock Elemental
/// Gift rows (Marid Cold / Dao Bludgeoning / Djinni Thunder), Radiant
/// Soul's radiant row, Heart of the Storm's lightning + thunder row,
/// Psychic Defenses' psychic row, and Inured to Undeath's necrotic row.
/// Single-type slice (Fire only) — same shape as the Fiendish Resilience
/// / Draconic Resilience Fire rows on the single-type-per-subclass lane.
///
/// Sibling to `STORM_SOUL_SEA_TAG` on the Primal Path: Storm Herald
/// lane — same helper (`subclass_barbarian_template`), same level-6
/// subclass tell, same always-on resistance shape, different elemental
/// flavor and different damage axis (Fire here vs. Lightning there).
/// The two never legally co-occur on a single build (RAW: one Storm
/// Herald flavor picked at lv3). Also sibling to `MARID_ELEMENTAL_GIFT_TAG`
/// / `DAO_ELEMENTAL_GIFT_TAG` / `DJINNI_ELEMENTAL_GIFT_TAG` /
/// `RADIANT_SOUL_TAG` / `HEART_OF_THE_STORM_TAG` / `PSYCHIC_DEFENSES_TAG`
/// / `INURED_TO_UNDEATH_TAG` on the "one feature tag drives one cohort
/// row" declarative-table pattern — different source chassis (Barbarian
/// Primal Path vs. Warlock Otherworldly Patron / Sorcerer Sorcerous
/// Origin / Wizard Arcane Tradition) and different damage axis.
///
/// Ships on `DESERT_STORM_HERALD_BARBARIAN_TEMPLATE` — the second Path
/// of the Storm Herald subclass template on the barbarian chassis
/// (extending the Bear / Wolf / Eagle / Tiger / Elk / Wolverine /
/// Panther / Berserker / Zealot / Sea Storm Herald subclass roster).
/// Distinct from every other barbarian subclass template — the Totem
/// Warrior templates stack rage-gated broad resistance (Bear) or rage-
/// gated speed / ally-aura bumps (Wolf / Eagle / Tiger / Elk / Wolverine
/// / Panther), while the Storm Heralds layer on always-on typed
/// resistance regardless of rage state, differing only in elemental
/// flavor.
///
/// RAW's Storm Herald picks up other features not shipped on this
/// template — **Storm Aura (Desert)** (lv3: while raging, every hostile
/// within 10ft eats a fixed 2 fire damage at the start of each of the
/// barbarian's turns, no save; a per-turn AoE aura mechanic that needs
/// a per-turn aura fire hook and a target-filtering policy), **Shielding
/// Storm** (lv10: allies within 10ft of the raging barbarian ALSO gain
/// Storm Soul's resistance; an ally-aura extension mechanic), and
/// **Raging Storm (Desert)** (lv14: reaction-on-attacker-melee-hit fire
/// damage rider; a per-attack reactive install mechanic). Only the lv6
/// Storm Soul passive has a mechanical surface that plugs cleanly into
/// the shared passive typed-resistance cohort, so we ship that half and
/// leave the rest as future work — matching the way `STORM_SOUL_SEA_TAG`
/// / `INURED_TO_UNDEATH_TAG` / `MARID_ELEMENTAL_GIFT_TAG` /
/// `DAO_ELEMENTAL_GIFT_TAG` / `DJINNI_ELEMENTAL_GIFT_TAG` each ship
/// only the passive resistance half of their broader RAW kit.
///
/// Ships on the CR-4 (level-9) barbarian chassis at (or above) its
/// strict RAW lv6 gate for the same reason `STORM_SOUL_SEA_TAG` ships
/// on `SEA_STORM_HERALD_BARBARIAN_TEMPLATE`, `MARID_WARLOCK_TEMPLATE`
/// / `DAO_WARLOCK_TEMPLATE` / `DJINNI_WARLOCK_TEMPLATE` ship Elemental
/// Gift (RAW lv6), `NECROMANCY_WIZARD_TEMPLATE` ships Inured to Undeath
/// (RAW lv10), and `ABERRANT_MIND_SORCERER_TEMPLATE` ships Psychic
/// Defenses (RAW lv14) — class templates target a balanced playable
/// level, not lockstep PHB progression.
pub const STORM_SOUL_DESERT_TAG: &str = "barbarian.storm_soul_desert";

/// 5e Barbarian Primal Path — **Path of the Storm Herald (Tundra)** —
/// **Storm Soul (Tundra)** subclass feature tag (level 6, XGtE). Passive:
/// the tundra storm herald's body absorbs the numbing chill of the frozen
/// wastes — the barbarian gains **resistance to cold damage**.
///
/// RAW pairs the cold resistance with an "immune to extreme cold" clause
/// (the RAW `Adventuring/Environment/Extreme Cold` exhaustion rider) and a
/// "you can freeze water within 5 ft into ice" ribbon. Neither has a
/// combat surface on today's engine — extreme-cold exhaustion sits outside
/// the tactical loop, and the freeze-water ribbon is out-of-combat flavor
/// — so both are left as future work. The resistance clause is the
/// load-bearing defensive half and rides here alone, matching the way
/// `STORM_SOUL_SEA_TAG` ships without the swim / water-breathing halves,
/// `STORM_SOUL_DESERT_TAG` ships without the extreme-heat / ignite halves,
/// and `INURED_TO_UNDEATH_TAG` ships without the max-HP-can't-be-reduced
/// clause.
///
/// Third Barbarian-chassis row on the passive typed-resistance lane —
/// `STORM_SOUL_SEA_TAG` (Sea, Lightning) blazed the trail and
/// `STORM_SOUL_DESERT_TAG` (Desert, Fire) followed; this row completes the
/// three-flavor Storm Herald elemental trio (Sea Lightning / Desert Fire /
/// Tundra Cold) on the barbarian chassis. Every earlier row (Dwarven /
/// Fiendish / Draconic / Heart of the Storm / Psychic Defenses / Radiant
/// Soul / Marid / Dao / Djinni Elemental Gift / Inured to Undeath) came
/// off a racial trait or a Warlock / Sorcerer / Wizard subclass. The
/// Barbarian chassis's rage-gated broad resistance (Bear Totem,
/// `RAGE_GATED_BROAD_RESISTANCES`) is distinct on both axis (broad, not
/// typed) and gate (rage-gated, not always-on) from this always-on
/// typed-resistance grant.
///
/// Overlaps the Cold axis with **Elemental Gift (Marid)** (Warlock Genie
/// Marid Patron lv6) on a different chassis — the two never legally
/// co-occur on a single build (Barbarian vs. Warlock subclass), and a
/// hypothetical multiclass carrier caps at a single /2 per Cold hit via
/// the "one halving per damage instance" rule.
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to Storm Soul (Sea)'s lightning row, Storm
/// Soul (Desert)'s fire row, Fiendish / Draconic Resilience's fire rows,
/// the Warlock Elemental Gift rows (Marid Cold / Dao Bludgeoning / Djinni
/// Thunder), Radiant Soul's radiant row, Heart of the Storm's lightning +
/// thunder row, Psychic Defenses' psychic row, and Inured to Undeath's
/// necrotic row. Single-type slice (Cold only) — same shape as the
/// Fiendish Resilience / Draconic Resilience Fire rows and Storm Soul
/// (Sea) / (Desert) rows on the single-type-per-subclass lane.
///
/// Sibling to `STORM_SOUL_SEA_TAG` / `STORM_SOUL_DESERT_TAG` on the
/// Primal Path: Storm Herald lane — same helper
/// (`subclass_barbarian_template`), same level-6 subclass tell, same
/// always-on resistance shape, different elemental flavor and different
/// damage axis (Cold here vs. Lightning on Sea vs. Fire on Desert). The
/// three never legally co-occur on a single build (RAW: one Storm Herald
/// flavor picked at lv3). Also sibling to `MARID_ELEMENTAL_GIFT_TAG` /
/// `DAO_ELEMENTAL_GIFT_TAG` / `DJINNI_ELEMENTAL_GIFT_TAG` /
/// `RADIANT_SOUL_TAG` / `HEART_OF_THE_STORM_TAG` / `PSYCHIC_DEFENSES_TAG`
/// / `INURED_TO_UNDEATH_TAG` on the "one feature tag drives one cohort
/// row" declarative-table pattern — different source chassis (Barbarian
/// Primal Path vs. Warlock Otherworldly Patron / Sorcerer Sorcerous
/// Origin / Wizard Arcane Tradition) and different damage axis.
///
/// Ships on `TUNDRA_STORM_HERALD_BARBARIAN_TEMPLATE` — the third Path of
/// the Storm Herald subclass template on the barbarian chassis (extending
/// the Bear / Wolf / Eagle / Tiger / Elk / Wolverine / Panther / Berserker
/// / Zealot / Sea Storm Herald / Desert Storm Herald subclass roster).
/// Distinct from every other barbarian subclass template — the Totem
/// Warrior templates stack rage-gated broad resistance (Bear) or rage-
/// gated speed / ally-aura bumps (Wolf / Eagle / Tiger / Elk / Wolverine
/// / Panther), while the Storm Heralds layer on always-on typed
/// resistance regardless of rage state, differing only in elemental
/// flavor.
///
/// RAW's Storm Herald picks up other features not shipped on this
/// template — **Storm Aura (Tundra)** (lv3: while raging, every friendly
/// within 10ft gains 2 temp HP at the start of each of the barbarian's
/// turns; a per-turn ally-buff aura mechanic that needs a per-turn aura
/// fire hook and a target-filtering policy), **Shielding Storm** (lv10:
/// allies within 10ft of the raging barbarian ALSO gain Storm Soul's
/// resistance; an ally-aura extension mechanic), and **Raging Storm
/// (Tundra)** (lv14: reaction-on-attacker-melee-hit STR-save vs. speed-0
/// rider; a per-attack reactive install mechanic). Only the lv6 Storm
/// Soul passive has a mechanical surface that plugs cleanly into the
/// shared passive typed-resistance cohort, so we ship that half and
/// leave the rest as future work — matching the way `STORM_SOUL_SEA_TAG`
/// / `STORM_SOUL_DESERT_TAG` / `INURED_TO_UNDEATH_TAG` /
/// `MARID_ELEMENTAL_GIFT_TAG` / `DAO_ELEMENTAL_GIFT_TAG` /
/// `DJINNI_ELEMENTAL_GIFT_TAG` each ship only the passive resistance
/// half of their broader RAW kit.
///
/// Ships on the CR-4 (level-9) barbarian chassis at (or above) its
/// strict RAW lv6 gate for the same reason `STORM_SOUL_SEA_TAG` /
/// `STORM_SOUL_DESERT_TAG` ship on their respective templates,
/// `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` /
/// `DJINNI_WARLOCK_TEMPLATE` ship Elemental Gift (RAW lv6),
/// `NECROMANCY_WIZARD_TEMPLATE` ships Inured to Undeath (RAW lv10), and
/// `ABERRANT_MIND_SORCERER_TEMPLATE` ships Psychic Defenses (RAW lv14)
/// — class templates target a balanced playable level, not lockstep PHB
/// progression.
pub const STORM_SOUL_TUNDRA_TAG: &str = "barbarian.storm_soul_tundra";

/// 5e Cleric Divine Domain — **Grave Domain** — **Path to the Grave**
/// Channel Divinity subclass feature tag (level 2 subclass, XGtE). The
/// cleric spends their once-per-short-rest Channel Divinity charge to
/// curse one creature within 30 ft; the target holds the
/// `MarkedForGrave` condition until the start of the cleric's next
/// turn, and any attack against the target while cursed rolls with
/// advantage.
///
/// RAW's clause reads "the next attack roll made against the target
/// before the end of your next turn has advantage" AND "if the attack
/// hits, the target has vulnerability to all of that attack's damage,
/// and then the curse ends." Collapsed here to the advantage half on
/// the `grants_advantage_to_attackers` target-side cohort — the
/// vulnerability half (double damage on the first hit) needs a
/// target-side incoming-damage multiplier hook that today's engine
/// doesn't expose, and would slot in later as a `MarkedForGrave`-
/// gated damage multiplier at the `effective_damage` chokepoint. The
/// "curse ends after the attack" clause is collapsed to the
/// `UntilStartOfNextTurn` timer (matches RAW's cadence — the curse
/// naturally expires the round after install even without the
/// on-attack consume).
///
/// Sits in the same "target-side reactive-flavor curse that fattens
/// the next attack against the holder" lane as `GuidingBoltLit`
/// (Cleric Guiding Bolt lv1 install rider) — same
/// `grants_advantage_to_attackers` chokepoint on the target-side, but
/// distinguished by combat-log identity ("marked for the grave" vs
/// "marked by guiding bolt") so the curse source reads unambiguously
/// at the log site. Distinct from Guiding Bolt's install path
/// (Guiding Bolt lands only on a hit; Path to the Grave lands with no
/// attack roll, no save — the cleric spends CD and the curse is on).
///
/// Refreshes on a short rest via `SHORT_REST_FEATURES` — sibling
/// cadence to every other Cleric Channel Divinity charge (Turn Undead
/// / Preserve Life / Guided Strike / Radiance of the Dawn / Warding
/// Flare / Wrath of the Storm). Ships on `GRAVE_CLERIC_TEMPLATE`.
pub const PATH_TO_THE_GRAVE_TAG: &str = "cleric.path_to_the_grave";

/// Channel Divinity: Path to the Grave — Grave Domain Cleric action.
/// Applies the `MarkedForGrave` curse to a single target within 30 ft
/// (12 tiles on our 2.5 ft grid). No save, no attack roll — the
/// curse installs the moment the cleric spends their once-per-short-
/// rest Channel Divinity charge. The next attack against the cursed
/// target has advantage via the shared
/// `Condition::grants_advantage_to_attackers` cohort; the curse
/// expires at the start of the cleric's next turn via the
/// `UntilStartOfNextTurn` timer.
///
/// Sibling to `GuidingBolt` on the "install a target-side attack-
/// advantage rider" lane — Guiding Bolt lands the rider AS A HIT
/// RIDER after a successful spell attack roll (radiant damage plus
/// the `GuidingBoltLit` install), while Path to the Grave lands the
/// rider AS THE ACTION'S SOLE EFFECT with no attack roll or save.
/// Distinct from `GuidedStrike` (War Cleric CD) on the axis: Guided
/// Strike is a CASTER-side self-prime (+10 to your own next swing),
/// Path to the Grave is a TARGET-side curse (advantage on the next
/// attack against the cursed target — the cleric's whole party
/// benefits, not just the caster).
pub struct PathToTheGrave {}

impl Action for PathToTheGrave {
    fn name(&self) -> &str {
        "path to the grave"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ptg", "grave", "path-grave", "cd-grave"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft RAW = 12 tiles on the 2.5ft grid — matches Guiding
        // Bolt / Guided Strike / Radiance of the Dawn's 30ft envelope.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        true
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
        action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Same shared gate as Intimidating Presence / Nature's Wrath /
        // Abjure Enemy — once-per-rest charge + hostile-target + not-
        // already-MarkedForGrave dedup all fold into
        // `hostile_target_feature_ready`. Dedup keeps the AI from
        // burning the charge on a target whose curse is still live from
        // an earlier cast.
        hostile_target_feature_ready(
            encounter,
            caster_id,
            target_ids,
            PATH_TO_THE_GRAVE_TAG,
            Condition::MarkedForGrave,
        )
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(PATH_TO_THE_GRAVE_TAG);
        }
        encounter.log(
            "  path to the grave: cleric curses the target for a killing blow.".to_string(),
        );
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::MarkedForGrave,
            // RAW: "until the end of your next turn." Our
            // `UntilStartOfNextTurn` timer clears on the CASTER's next
            // start-of-turn — slightly shorter than RAW's end-of-next-
            // turn, but matches the shared short-timer cadence used by
            // every other one-round rider (Guided Strike prime,
            // Guiding Bolt Lit, Mocked, Helped) so the curse's decay
            // reads consistently with the rest of the once-per-round
            // rider family.
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static PATH_TO_THE_GRAVE: LazyLock<PathToTheGrave> = LazyLock::new(|| PathToTheGrave {});

/// 5e Cleric Divine Domain — **Forge Domain** — **Soul of the Forge**
/// subclass feature tag (level 6 subclass, XGtE). Passive: the forge
/// cleric's body hardens into the flame's forge — the cleric gains
/// **resistance to fire damage**.
///
/// RAW pairs the fire resistance with a "+1 AC while wearing heavy armor"
/// clause. The engine doesn't model armor tiers as a first-class combat
/// surface, so the AC-bump half is left as future work and would slot in
/// later as a template AC bump on the Forge Cleric chassis; the
/// resistance clause is the load-bearing defensive half and rides here
/// alone, matching the way `INURED_TO_UNDEATH_TAG` ships without the
/// max-HP-can't-be-reduced clause, `RADIANT_SOUL_TAG` ships without the
/// +CHA-mod damage rider, and every Genie `*_ELEMENTAL_GIFT_TAG` ships
/// without the RAW's ribbon halves.
///
/// First Cleric-chassis row on the passive typed-resistance lane —
/// every prior row came off a racial trait or a Warlock / Sorcerer /
/// Wizard / Barbarian subclass. Overlaps the Fire axis with three
/// existing rows: Fiendish Resilience (Warlock Fiend lv10), Draconic
/// Resilience (Sorcerer Draconic Bloodline lv6), Efreeti Elemental
/// Gift (Warlock Genie Efreeti lv6), and Storm Soul (Desert)
/// (Barbarian Storm Herald Desert lv6). The five Fire-resistance rows
/// (Fiendish / Draconic / Efreeti / Storm Soul Desert / Soul of the
/// Forge) never legally co-occur on a single build (Warlock Fiend vs.
/// Warlock Efreeti vs. Sorcerer Draconic vs. Barbarian Storm Herald
/// Desert vs. Cleric Forge Domain are five distinct
/// class-subclass slots), and a hypothetical multiclass carrier caps
/// at a single /2 per Fire hit under the "one halving per damage
/// instance" rule. The duplication is a **taxonomic completeness**
/// grant — the Fire axis now covers all five subclass chassis rather
/// than just four.
///
/// Sibling on the passive typed-resistance subclass lane to:
///   - **Fiendish Resilience** (Warlock Fiend Patron lv10): Fire.
///   - **Draconic Resilience** (Draconic Sorcerer lv6): Fire.
///   - **Efreeti Elemental Gift** (Warlock Genie Efreeti lv6): Fire.
///   - **Storm Soul (Desert)** (Storm Herald Barbarian lv6): Fire.
///   - **Marid / Dao / Djinni Elemental Gift** (Genie Warlock lv6):
///     Cold / Bludgeoning / Thunder.
///   - **Radiant Soul** (Celestial Warlock lv6): Radiant.
///   - **Heart of the Storm** (Storm Sorcerer lv6): Lightning + Thunder.
///   - **Psychic Defenses** (Aberrant Mind Sorcerer lv14): Psychic.
///   - **Inured to Undeath** (Necromancy Wizard lv10): Necrotic.
///   - **Storm Soul (Sea)** (Storm Herald Barbarian lv6): Lightning.
///   - **Storm Soul (Tundra)** (Storm Herald Barbarian lv6): Cold.
///     All share the "one feature tag drives one cohort row"
///     declarative-table pattern, different subclass flavor and
///     different damage axis.
///
/// Sibling on the Divine Domain subclass lane to the baseline
/// `CLERIC_TEMPLATE` (subclass-less baseline) and the existing War /
/// Light / Tempest / Life / Grave cousins.
///
/// Ships on the CR-0.5 cleric chassis at (or above) its strict RAW lv6
/// gate for the same reason `MARID_WARLOCK_TEMPLATE` /
/// `DAO_WARLOCK_TEMPLATE` / `DJINNI_WARLOCK_TEMPLATE` /
/// `EFREETI_WARLOCK_TEMPLATE` ship Elemental Gift (RAW lv6),
/// `NECROMANCY_WIZARD_TEMPLATE` ships Inured to Undeath (RAW lv10), and
/// `ABERRANT_MIND_SORCERER_TEMPLATE` ships Psychic Defenses (RAW lv14)
/// — class templates target a balanced playable level, not lockstep PHB
/// progression.
pub const SOUL_OF_THE_FORGE_TAG: &str = "cleric.soul_of_the_forge";

/// 5e Cleric Divine Domain — **Twilight Domain** — **Vigilant Blessing**
/// subclass feature tag (level 1 subclass, TCE). Passive: the twilight
/// cleric's senses stay attuned to the encroaching dark — advantage on
/// **initiative rolls**.
///
/// RAW's Vigilant Blessing is an action that grants advantage on the
/// **next** initiative roll made by one target before the end of the
/// cleric's next long rest — a per-encounter setup ribbon. We collapse
/// the RAW's out-of-combat prime + one-shot expiry into an always-on
/// self-buff on the same "class templates target a balanced playable
/// level, not lockstep PHB progression" grounds every other subclass
/// template ships at (Feral Instinct / Remarkable Athlete / Rakish
/// Audacity / Dread Ambusher already ride the initiative-augment lane
/// as always-on template flags). The collapse keeps the tag a purely
/// declarative one-line entry — no per-encounter action-slot expenditure,
/// no ally-target picker, no rest-timer to drive expiry.
///
/// Read at `ActorInstance::rolls_initiative_with_advantage` alongside
/// `has_feral_instinct` (Barbarian Feral Instinct — the only prior
/// initiative-advantage source) as a one-line `|| self.has_passive_feature(TAG)`
/// join. Sibling on the "passive initiative advantage from a class
/// subclass" lane to `has_feral_instinct` — same roll shape (d20 rolled
/// twice, higher kept), different class chassis (Cleric vs. Barbarian).
/// Stacks composably with the ability-mod initiative-bump cohort
/// (Rakish Audacity / Dread Ambusher on `ABILITY_MOD_INITIATIVE_BONUSES`)
/// and Remarkable Athlete's `+ceil(prof / 2)`: a hypothetical Twilight
/// Cleric multiclass carrier rolls the initiative d20 twice AND adds
/// whichever flat bumps apply.
///
/// RAW's Twilight Domain picks up other features not shipped on this
/// template — **Eyes of Night** (lv1: 300ft darkvision that can be
/// shared with allies; the sense-radius surface exists on
/// `SpecialSense::Darkvision(u32)` but the ally-share half needs a
/// per-encounter buff-distribution hook), **Channel Divinity: Twilight
/// Sanctuary** (lv2: bonus-action 30ft aura that either grants d6 + level
/// temp HP or ends Charmed / Frightened on allies entering; needs a
/// mobile per-turn aura-tick hook), **Steps of Night** (lv6: fly speed
/// while in dim light or darkness; needs a flying-movement surface),
/// **Divine Strike (Radiant)** (lv8: +1d8 radiant on weapon hits; the
/// baseline cleric already ships a Radiant Divine Strike via
/// `DIVINE_STRIKE_TAG`, so this half is already covered on the shared
/// baseline), and **Twilight Shroud** (lv17 capstone: allies in the
/// sanctuary aura get half cover; needs a cover-modifier hook). Only the
/// lv1 Vigilant Blessing passive has a mechanical surface on the CR-0.5
/// chassis that plugs cleanly into the shared
/// `rolls_initiative_with_advantage` chokepoint, so we ship that half
/// and leave the rest as future work — matching the way
/// `FORGE_CLERIC_TEMPLATE` ships only the lv6 Soul of the Forge passive
/// half of its RAW Forge Domain kit and `NECROMANCY_WIZARD_TEMPLATE`
/// ships only the lv10 Inured to Undeath passive half of its RAW School
/// of Necromancy kit.
///
/// Ships on `TWILIGHT_CLERIC_TEMPLATE` at (or above) its RAW lv1 gate.
pub const VIGILANT_BLESSING_TAG: &str = "cleric.vigilant_blessing";

/// 5e Wizard Arcane Tradition — **School of War Magic** — **Tactical
/// Wit** subclass feature tag (level 2 subclass, XGtE). Passive: the
/// war mage's honed battlefield calculus sharpens their opening move —
/// they add their **Intelligence modifier** to their **initiative
/// rolls**.
///
/// The signature "the war mage arrives with a plan already in motion"
/// tell — where a baseline Wizard rolls initiative with a middling DEX
/// mod, the War Magic Wizard folds their high INT modifier (16-20 for
/// a level-9 archmage-tier build) directly into the roll. Composes
/// cleanly with the wizard's high-value opening cast: a War Magic
/// Wizard who wins initiative reliably lands Shield / Mirror Image /
/// Fireball / Hypnotic Pattern / Slow / Counterspell before the first
/// enemy swing.
///
/// Read at the shared `ABILITY_MOD_INITIATIVE_BONUSES` cohort in
/// `actor_template.rs` next to Rakish Audacity (Swashbuckler Rogue,
/// +CHA-mod) and Dread Ambusher (Gloom Stalker Ranger, +WIS-mod) as a
/// third `AbilityModInitiativeBonus { flag, ability }` row — same
/// declarative shape, different ability axis (INT here vs. CHA /
/// WIS on the sibling rows) and different subclass chassis (Wizard
/// vs. Rogue / Ranger). Stacks additively per the cohort's "any row
/// hit is sufficient; all hitting rows sum" semantic: a hypothetical
/// War-Magic-Wizard / Swashbuckler-Rogue / Gloom-Stalker-Ranger
/// multi-classer rolls the initiative d20 flat and adds CHA-mod +
/// WIS-mod + INT-mod on top of the DEX-mod baseline.
///
/// Composes cleanly with `has_passive_feature(VIGILANT_BLESSING_TAG)`
/// (Twilight Cleric initiative advantage) and `has_feral_instinct`
/// (Barbarian Feral Instinct initiative advantage) on the sibling
/// `INITIATIVE_ADVANTAGE_SOURCES` cohort — a hypothetical carrier of
/// both a Tactical-Wit tag and one of the advantage flags rolls the
/// initiative d20 twice AND adds their INT modifier.
///
/// RAW's School of War Magic picks up other features not shipped on
/// this template — **Arcane Deflection** (lv2 reaction: +2 AC vs one
/// attack roll or +4 to a saving throw, but forfeit non-cantrip casts
/// until end of next turn; needs a reactive AC/save-modifier hook with
/// a next-turn cast lockout), **Power Surge** (lv6: store magical
/// energy from spent counterspells / dispels, add half-wizard-level
/// force damage to one spell per turn; needs a per-cast damage-boost
/// hook and a counterspell / dispel side-channel), **Durable Magic**
/// (lv10: +2 AC and +2 to saves while concentrating; needs a compound
/// AC/save modifier gated on the Concentrating condition), and
/// **Deflecting Shroud** (lv14 capstone: Arcane Deflection now radiates
/// force damage to up to three enemies within 60ft; needs the base
/// reaction plus a burst hook). Only the lv2 Tactical Wit passive has
/// a mechanical surface on the CR-0.5 chassis that plugs cleanly into
/// the shared `ABILITY_MOD_INITIATIVE_BONUSES` cohort, so we ship that
/// half and leave the rest as future work — matching the way
/// `NECROMANCY_WIZARD_TEMPLATE` ships only the lv10 Inured to Undeath
/// passive half of its RAW School of Necromancy kit and
/// `TWILIGHT_CLERIC_TEMPLATE` ships only the lv1 Vigilant Blessing
/// passive half of its RAW Twilight Domain kit.
///
/// Ships on `WAR_MAGIC_WIZARD_TEMPLATE` at (or above) its strict RAW
/// lv2 gate for the same reason `NECROMANCY_WIZARD_TEMPLATE` ships
/// Inured to Undeath (RAW lv10) and `TWILIGHT_CLERIC_TEMPLATE` ships
/// Vigilant Blessing (RAW lv1) — class templates target a balanced
/// playable level, not lockstep PHB progression.
pub const TACTICAL_WIT_TAG: &str = "wizard.tactical_wit";

/// 5e Paladin Oath of Glory — **Aura of Alacrity** subclass feature tag
/// (Glory subclass level 7, TCE). Passive: the Glory paladin emanates
/// a swiftness aura — their own walking speed increases by 10 feet
/// (RAW also extends the bump to any ally who starts a turn within 5 ft
/// of them, but that ally-aura half needs a per-turn-start aura scan
/// this engine doesn't expose as a first-class surface today; the
/// self-side +10 ft is the load-bearing tactical piece and is what
/// this tag wires up).
///
/// The signature "the glory paladin is always one step ahead" tell —
/// where a baseline paladin walks at 30 ft (default humanoid), the
/// Glory paladin opens combat at 40 ft. Composes cleanly with the
/// paladin's smite-and-melee kit (a Glory paladin who's one tile
/// further into the enemy line lands a Divine Smite one round earlier)
/// and with the aura suite the class already leans on (Aura of
/// Protection at lv6 for saves, Aura of Courage at lv10 for Frightened
/// suppression, Aura of Alacrity at lv7 for movement) — three
/// overlapping self-aura effects that all fire on the same
/// adjacent-ally scan.
///
/// Read at the shared `PASSIVE_FEATURE_SPEED_BONUSES` cohort in
/// `actor_template.rs` next to Fast Movement (Barbarian +10), Unarmored
/// Movement (Monk +10), Superior Mobility (Scout Rogue +10), Roving
/// (Ranger +5) — one lookup table, one source of truth. Same magnitude
/// as the barbarian / monk / scout rows (+10 ft) and a different
/// subclass chassis from all three (Paladin vs. Barbarian / Monk /
/// Rogue), so a hypothetical multiclass carrier stacks additively per
/// the cohort's "any row hit is sufficient; all hitting rows sum"
/// semantic — a Glory-Paladin / Scout-Rogue multi-classer walks at
/// +20 over baseline.
///
/// Sibling on the "one feature tag drives one PASSIVE_FEATURE_SPEED_BONUSES
/// cohort row" declarative-table pattern to `SCOUT_ROGUE_TEMPLATE`
/// (Superior Mobility +10), `MONK_TEMPLATE` / `OPEN_HAND_MONK_TEMPLATE`
/// / `LONG_DEATH_MONK_TEMPLATE` (Unarmored Movement +10), the four
/// barbarian totem paths that stack rage-gated speed bumps, and the
/// baseline ranger's `ROVING_TAG` (+5) — six class chassis converge on
/// the same "passive walking-speed bump keyed off a subclass tag"
/// identity from different angles.
///
/// RAW's Oath of Glory picks up other features not shipped on this
/// template — **Peerless Athlete** (lv3 Channel Divinity: advantage on
/// Athletics / Acrobatics + carrying capacity double; skills-only, no
/// combat surface), **Inspiring Smite** (lv3 CD: after Divine Smite,
/// distribute temp HP to allies within 30ft; needs a per-smite-hit
/// trigger + temp-HP distribution helper), **Glorious Defense** (lv15:
/// reaction to grant CHA-mod bonus to an ally's failed save + strike
/// the attacker; needs a save-time reaction hook), and **Living Legend**
/// (lv20 capstone: 1-minute self-buff granting Charmed / Frightened
/// immunity + weapon crits on 19-20 + reroll failed save; complex
/// multi-effect self-buff). Only the lv7 Aura of Alacrity passive has
/// a mechanical surface on the CR-1.5 chassis that plugs cleanly into
/// the shared `PASSIVE_FEATURE_SPEED_BONUSES` cohort, so we ship that
/// half and leave the rest as future work — matching the way
/// `TWILIGHT_CLERIC_TEMPLATE` ships only the lv1 Vigilant Blessing
/// passive half of its RAW Twilight Domain kit and
/// `WAR_MAGIC_WIZARD_TEMPLATE` ships only the lv2 Tactical Wit passive
/// half of its RAW School of War Magic kit.
///
/// Always-on passive; no per-rest charge and no condition gate. The
/// tag lives in the actor's `features` pool, not in
/// `SHORT_REST_FEATURES` / long-rest tables — nothing consumes it and
/// nothing refreshes it. Ships on `GLORY_PALADIN_TEMPLATE` at (or
/// above) its strict RAW lv7 gate for the same reason
/// `WAR_MAGIC_WIZARD_TEMPLATE` ships Tactical Wit (RAW lv2) and
/// `TWILIGHT_CLERIC_TEMPLATE` ships Vigilant Blessing (RAW lv1) —
/// class templates target a balanced playable level, not lockstep PHB
/// progression.
pub const AURA_OF_ALACRITY_TAG: &str = "paladin.aura_of_alacrity";

/// Flat walking-speed bonus in feet granted by Glory Paladin's Aura of
/// Alacrity passive. Pinned to +10 ft per RAW; exposed as a constant
/// so the `PASSIVE_FEATURE_SPEED_BONUSES` table stays declarative
/// rather than scattering magic numbers into the accessor. Sibling to
/// `FAST_MOVEMENT_SPEED_BONUS` (+10) / `UNARMORED_MOVEMENT_SPEED_BONUS`
/// (+10) / `SUPERIOR_MOBILITY_SPEED_BONUS` (+10) at the same magnitude;
/// sibling to `ROVING_SPEED_BONUS` (+5) at half magnitude.
pub const AURA_OF_ALACRITY_SPEED_BONUS: f32 = 10.0;

/// 5e Paladin Oath of the Watchers — **Aura of the Sentinel** subclass
/// feature tag (Watchers subclass level 7, TCE). Passive: the Watchers
/// paladin's alertness aura grants a bonus to their initiative roll
/// equal to their proficiency bonus (RAW also extends the bump to any
/// creature of the paladin's choice within 10 ft, but that ally-side
/// half needs a per-initiative-roll aura-of-choice scan surface this
/// engine doesn't expose as a first-class hook today; the self-side
/// prof-bonus bump is the load-bearing tactical piece and is what this
/// tag wires up).
///
/// The signature "the watchers paladin acts first" tell — where a
/// baseline paladin rolls initiative at flat `d20 + DEX-mod`, the
/// Watchers paladin adds `d20 + DEX-mod + prof-bonus` (a +2 bump at
/// CR 1.5, scaling to +6 at the highest tier). Composes cleanly with
/// the paladin's smite-and-melee kit (a Watchers paladin who wins
/// initiative reliably opens the round with the enemy line's saves
/// eaten by a smite prime one round earlier) and with the aura suite
/// the class already leans on (Aura of Protection at lv6 for saves,
/// Aura of Courage at lv10 for Frightened suppression, and now Aura
/// of the Sentinel at lv7 for the initiative-roll chokepoint) — three
/// overlapping self-aura effects that all fire on the same adjacent-
/// ally scan.
///
/// The Watchers Oath's initiative-flavored sibling to the other
/// Paladin oaths:
///   - **Devotion** (Aura of Devotion): ally-side Charmed suppression.
///   - **Ancients** (Nature's Ward + Undying Sentinel + Aura of
///     Warding): self-side Charmed / Frightened immunity plus spell-
///     damage-halving aura plus cheat-death.
///   - **Vengeance** (Vow of Enmity + Abjure Enemy): target-side attack
///     prime plus Frighten burst.
///   - **Oathbreaker** (Aura of Hate + Fanatical Focus + Dreadful
///     Aspect): melee damage aura plus failed-save reroll plus mass-
///     Frighten CD.
///   - **Glory** (Aura of Alacrity): passive +10 ft walking speed —
///     always-on mobility on the paladin's own chassis.
///   - **Watchers** (Aura of the Sentinel): passive proficiency-bonus
///     initiative bump — always-on initiative-roll augment on the
///     paladin's own chassis.
///
/// Where Glory projects through the movement chokepoint (+10 ft
/// walking speed on `PASSIVE_FEATURE_SPEED_BONUSES`), the Watchers
/// Oath projects through the initiative-roll chokepoint — same
/// "always-on passive that fires on a specific engine chokepoint"
/// pattern, different axis. Distinct from the sibling
/// `INITIATIVE_ADVANTAGE_SOURCES` cohort (Feral Instinct, Vigilant
/// Blessing) which flips the roll SHAPE to advantage — the two
/// cohorts stack cleanly: a hypothetical Watchers-Paladin / Twilight-
/// Cleric multiclass would roll 2d20 keep-high AND stack the prof
/// bonus on top.
///
/// Read at the shared `PROFICIENCY_INITIATIVE_BONUSES` cohort in
/// `actor_template.rs` next to Remarkable Athlete (Champion Fighter
/// half-prof) — one lookup table, one source of truth. Full prof
/// magnitude (a +2 → +6 scale on a level-3 → level-20 chassis) puts
/// this row on a larger scale than Remarkable Athlete's half-prof
/// row (a +1 → +3 scale on the same chassis span); a hypothetical
/// Champion-Fighter / Watchers-Paladin multiclass carrier would carry
/// both rows and stack the full + half prof bumps additively (per the
/// cohort's "any row hit is sufficient; all hitting rows sum"
/// semantic).
///
/// Sibling on the "one feature tag drives one initiative-roll cohort
/// row" declarative-table pattern to `SWASHBUCKLER_ROGUE_TEMPLATE`
/// (Rakish Audacity +CHA-mod), `GLOOM_STALKER_RANGER_TEMPLATE` (Dread
/// Ambusher +WIS-mod), and `WAR_MAGIC_WIZARD_TEMPLATE` (Tactical Wit
/// +INT-mod) on the `ABILITY_MOD_INITIATIVE_BONUSES` cohort — four
/// class chassis converge on the same "passive initiative bump keyed
/// off a subclass tag" identity from different angles (CHA / WIS /
/// INT ability mods there, proficiency bonus here).
///
/// RAW's Oath of the Watchers picks up other features not shipped on
/// this template — **Watcher's Will** (lv3 Channel Divinity: grant
/// allies within 30ft advantage on INT / WIS / CHA saves for 1 minute;
/// needs a per-save-check ally scan surface), **Abjure the Extraplanar**
/// (lv3 CD: 30ft WIS-save Turned on Aberrations / Celestials / Elementals
/// / Fey / Fiends; needs a creature-type-gated turn surface),
/// **Vigilant Rebuke** (lv15: reaction to grant +CHA-mod damage on a
/// creature that forced an ally within 30ft to make an INT / WIS / CHA
/// save; needs a save-time reaction hook plus counter-damage), and
/// **Mortal Bulwark** (lv20 capstone: 1-minute self-buff granting truesight,
/// advantage vs. Aberrations / Celestials / Elementals / Fey / Fiends,
/// and forced-banish on hit; complex multi-effect self-buff). Only the
/// lv7 Aura of the Sentinel passive has a mechanical surface on the
/// CR-1.5 chassis that plugs cleanly into the shared
/// `PROFICIENCY_INITIATIVE_BONUSES` cohort, so we ship that half and
/// leave the rest as future work — matching the way
/// `GLORY_PALADIN_TEMPLATE` ships only the lv7 Aura of Alacrity
/// passive half of its RAW Oath of Glory kit and
/// `TWILIGHT_CLERIC_TEMPLATE` ships only the lv1 Vigilant Blessing
/// passive half of its RAW Twilight Domain kit.
///
/// Always-on passive; no per-rest charge and no condition gate. The
/// tag lives in the actor's `features` pool, not in
/// `SHORT_REST_FEATURES` / long-rest tables — nothing consumes it and
/// nothing refreshes it. Ships on `WATCHERS_PALADIN_TEMPLATE` at (or
/// above) its strict RAW lv7 gate for the same reason
/// `GLORY_PALADIN_TEMPLATE` ships Aura of Alacrity (RAW lv7) and
/// `TWILIGHT_CLERIC_TEMPLATE` ships Vigilant Blessing (RAW lv1) —
/// class templates target a balanced playable level, not lockstep PHB
/// progression.
pub const AURA_OF_THE_SENTINEL_TAG: &str = "paladin.aura_of_the_sentinel";

/// 5e Fighter **Samurai** Martial Archetype (XGtE) — **Elegant Courtier**
/// (level 7 subclass passive tell). RAW: "your discipline and rigorous
/// training allow you to conduct yourself with poise. You gain proficiency
/// in Wisdom saving throws. If you already have this proficiency, you
/// instead gain proficiency in one of the following skills of your
/// choice: Insight, Performance, or Persuasion."
///
/// The load-bearing combat surface for Elegant Courtier is the WIS-save
/// proficiency half — the "if you already have WIS prof, pick a CHA/WIS
/// skill" fallback is a ribbon on non-combat social checks with no
/// engine surface. We ship the WIS-save-proficiency half and leave the
/// fallback as future work; a hypothetical Samurai carrier that
/// already picks up WIS prof from a multiclass chassis (Cleric,
/// Druid, Warlock, Wizard baseline) simply gets the WIS prof twice
/// via the OR-of-cohort-hits shape, which is a no-op since save
/// proficiency isn't an additive scalar.
///
/// Read at the shared `FLAG_DRIVEN_SAVE_PROFICIENCIES` cohort in
/// `actor_template.rs` next to `has_slippery_mind` (Rogue lv15) and
/// `has_iron_mind` (Gloom Stalker / Zealot Barbarian lv7) — three
/// existing rows on the "class feature grants WIS save proficiency"
/// lane, all promoting the same ability to "proficient". The Samurai
/// row uses a subclass-tag closure (`has_passive_feature(ELEGANT_COURTIER_TAG)`)
/// rather than a dedicated `has_elegant_courtier` struct-field flag,
/// matching the "tag-only cross-class helper" pattern
/// (`with_subclass_tag`) the wizard chassis already uses for
/// `NECROMANCY_WIZARD_TEMPLATE` / `WAR_MAGIC_WIZARD_TEMPLATE`.
///
/// Sibling on the "one feature tag drives one save-proficiency cohort
/// row" declarative-table pattern to Slippery Mind (Rogue struct-field
/// flag `has_slippery_mind`) and Iron Mind (Gloom Stalker / Zealot
/// Barbarian struct-field flag `has_iron_mind`) — three existing rows
/// on the shared WIS-save-proficiency axis, keyed off distinct sources
/// (subclass tag here vs. struct-field flag for the sibling rows).
/// The three never legally co-occur on a single build (Rogue vs. Ranger
/// vs. Barbarian vs. Fighter subclass slots), and a hypothetical
/// multiclass carrier picks up the proficiency via any single row
/// under the "any row hit is sufficient" OR semantic.
///
/// RAW's Samurai Fighter picks up other features not shipped on this
/// template — **Bonus Proficiency** (lv3: one skill or language;
/// ribbon on out-of-combat social checks with no engine surface),
/// **Fighting Spirit** (lv3: 3-per-long-rest bonus action for +5/10/15
/// temp HP AND advantage on weapon attacks until end of turn; needs a
/// per-turn advantage-marker plus a temp HP grant chained to a bonus-
/// action prime — future work behind a `FightingSpirit` action
/// surface), **Tireless Spirit** (lv10: refresh Fighting Spirit at
/// initiative-roll time if none left; needs a per-encounter refresh
/// tick), **Rapid Strike** (lv15: trade advantage for extra attack;
/// needs an advantage-consumption + bonus-attack hook), and
/// **Strength Before Death** (lv18 capstone: reaction to take a full
/// turn on being reduced to 0 HP; needs a dying-transition reaction
/// hook). Only the lv7 Elegant Courtier passive has a mechanical
/// surface on the CR-1 chassis that plugs cleanly into the shared
/// `FLAG_DRIVEN_SAVE_PROFICIENCIES` cohort, so we ship that half and
/// leave the rest as future work — matching the way
/// `NECROMANCY_WIZARD_TEMPLATE` ships only Inured to Undeath,
/// `FORGE_CLERIC_TEMPLATE` ships only Soul of the Forge, and every
/// other tag-only subclass template pares down to the load-bearing
/// passive half of its RAW subclass kit.
///
/// Always-on passive; no per-rest charge and no condition gate. The
/// tag lives in the actor's `features` pool, not in
/// `SHORT_REST_FEATURES` / long-rest tables — nothing consumes it and
/// nothing refreshes it. Ships on `SAMURAI_FIGHTER_TEMPLATE` at (or
/// above) its strict RAW lv7 gate for the same reason
/// `WATCHERS_PALADIN_TEMPLATE` ships Aura of the Sentinel (RAW lv7),
/// `GLORY_PALADIN_TEMPLATE` ships Aura of Alacrity (RAW lv7), and
/// every other subclass template runs above its strict RAW gate —
/// class templates target a balanced playable level, not lockstep PHB
/// progression.
pub const ELEGANT_COURTIER_TAG: &str = "fighter.elegant_courtier";

/// Class-feature tag for the Evocation Wizard's **Sculpt Spells**
/// (School of Evocation subclass level 2, PHB). Passive, at-will, no
/// per-rest charge — the tag is a pure membership marker read via
/// `has_passive_feature`, never spent through `feature_available` /
/// `spend_feature`.
///
/// RAW: "When you cast an evocation spell that affects other creatures
/// you can see, you can choose a number of them equal to 1 + the
/// spell's level. The chosen creatures automatically succeed on their
/// saving throws against the spell, and they take no damage if they
/// would normally take half damage."
///
/// Read at `EncounterInstance::auto_pass_shielded_allies`, the shared
/// ally-shield sweep that already backed the Sorcerer's Careful Spell
/// metamagic. The two features land on the same lane with three
/// differences the helper keeps straight: Careful Spell is a consumable
/// prime (a condition, burned on use) shielding CHA-mod allies on any
/// spell, while Sculpt Spells is an always-on passive shielding
/// `1 + spell level` allies but only on evocation casts. Their shielded
/// sets union, so an evoker/sorcerer multiclass gets both.
///
/// The engine's "choose creatures you can see" collapses to "the allies
/// in the blast, nearest first" — the only choice a sane caster makes,
/// and the same simplification Careful Spell already ships.
pub const SCULPT_SPELLS_TAG: &str = "wizard.sculpt_spells";

/// Class-feature tag for the Evocation Wizard's **Empowered Evocation**
/// (School of Evocation subclass level 10, PHB). Passive, at-will, no
/// per-rest charge — same membership-marker shape as
/// `SCULPT_SPELLS_TAG`.
///
/// RAW: "you can add your Intelligence modifier to one damage roll of
/// any wizard evocation spell you cast."
///
/// Read at `EncounterInstance::roll_empowered`, the caster-aware
/// spell-damage roll chokepoint that already backs the Sorcerer's
/// Empowered Spell metamagic. The two stack cleanly on the same roll —
/// Empowered Spell rerolls low dice, Empowered Evocation adds a flat
/// modifier on top — and neither is aware of the other. RAW's "one
/// damage roll" is naturally enforced by the chokepoint's shape: burst
/// spells roll damage once and share it across the blast, so the bonus
/// lands once per cast rather than once per target.
pub const EMPOWERED_EVOCATION_TAG: &str = "wizard.empowered_evocation";

/// Class-feature tag for the Evocation Wizard's **Potent Cantrip**
/// (School of Evocation subclass level 6, PHB). Passive, at-will —
/// same membership-marker shape as `SCULPT_SPELLS_TAG` /
/// `EMPOWERED_EVOCATION_TAG`.
///
/// RAW: "When a creature succeeds on a saving throw against your
/// cantrip, the creature takes half the damage but suffers no
/// additional effect." Note the gate is *cantrip*, not *evocation
/// cantrip* — the feature lifts every damaging save cantrip the evoker
/// knows, including the Poison Spray / Toll the Dead / Mind Sliver
/// pickups from other schools.
///
/// Read at `EncounterInstance::resolve_post_save_damage`, the shared
/// post-save damage chokepoint, where it upgrades the effect's
/// `SaveDamagePolicy` from `NoneOnSave` to `HalfOnSave` before Evasion
/// is consulted. Expressing it as a policy upgrade is what makes the
/// three-way interaction fall out for free: a target with Evasion who
/// makes their DEX save against a potent cantrip still takes nothing,
/// because Potent Cantrip lifts the effect into exactly the class
/// Evasion zeroes.
///
/// The cantrip gate reads the in-flight cast's level off the cast
/// stack (`level == 0`), so the feature is inert on leveled spells —
/// including the small set that also use `NoneOnSave` (Disintegrate),
/// which RAW must not benefit.
pub const POTENT_CANTRIP_TAG: &str = "wizard.potent_cantrip";

/// Class-feature tag for the Evocation Wizard's **Overchannel**
/// (School of Evocation subclass level 14, PHB). Passive membership
/// marker gating the `OVERCHANNEL` prime action; the per-rest
/// escalation lives in `ActorInstance::overchannel_uses` rather than in
/// the `features_remaining` charge pool, because Overchannel isn't
/// limited to N uses — it just gets more expensive.
///
/// RAW: "When you cast a wizard spell of 1st through 5th level that
/// deals damage, you can deal maximum damage with that spell. The first
/// time you do so, you suffer no adverse effect. If you use this feature
/// again before you finish a long rest, you take 2d12 necrotic damage
/// for each level of the spell, immediately after you cast it. Each time
/// you use this feature again before finishing a long rest, the necrotic
/// damage per level increases by 1d12."
pub const OVERCHANNEL_TAG: &str = "wizard.overchannel";

/// Class-feature tag for the Divination Wizard's **Expert Divination**
/// (School of Divination subclass level 6, PHB). Passive, at-will, no
/// per-rest charge — same membership-marker shape as
/// `SCULPT_SPELLS_TAG` / `EMPOWERED_EVOCATION_TAG` / `POTENT_CANTRIP_TAG`.
///
/// RAW: "When you cast a divination spell of 2nd level or higher using
/// a spell slot, you regain one expended spell slot. The slot you
/// regain must be of a level lower than the spell you cast and can't be
/// higher than 5th level."
///
/// Read at `EncounterInstance::trigger_expert_divination`, a post-cast
/// registry hook next to Arcane Ward's form / recharge. The two are
/// siblings in shape — both school-gated, both mutate the caster and
/// emit no side-effects — and differ only on which school they key off
/// and what resource they refill. The RAW band (`1..=min(level - 1, 5)`)
/// is resolved at the hook and handed to
/// `SpellSlotManager::restore_highest_expended_slot_up_to`, which picks
/// the most valuable expended slot in it.
pub const EXPERT_DIVINATION_TAG: &str = "wizard.expert_divination";

/// Class-feature tag for the Divination Wizard's **The Third Eye**
/// (School of Divination subclass level 10, PHB). Passive membership
/// marker read via `has_passive_feature`, never spent.
///
/// RAW: an action, once per short rest, to gain one of four benefits
/// until the next rest — Darkvision, Ethereal Sight, Greater
/// Comprehension (read any language), or See Invisibility. Three of the
/// four have no surface at the resolution this engine models: it has no
/// light level, no Ethereal Plane, and no written text. The fourth is
/// the only one that touches combat, so the feature collapses to a
/// permanent, no-action See Invisibility.
///
/// Collapsing the choice rather than modeling it is the right trade
/// here precisely *because* three options are inert: an "action to pick
/// one of four" surface where three picks do nothing is a worse model
/// of the feature than a passive that always grants the one that does.
/// The action cost goes with it — RAW's cost buys the choice, and there
/// is no choice left to buy.
///
/// Read at `EncounterInstance::concealment_piercing_of`, in the
/// `ConcealmentPiercing::Invisibility` tier next to the
/// `SeeingInvisible` condition the level-2 spell installs. That tier is
/// what keeps the collapse honest: a Truesight-shaped approximation
/// would have handed the diviner free Blur and Displacement piercing
/// RAW denies them.
pub const THIRD_EYE_TAG: &str = "wizard.third_eye";

/// Class-feature tag for the Enchantment Wizard's **Split Enchantment**
/// (School of Enchantment subclass level 6, PHB). Passive, at-will, no
/// per-rest charge — membership marker read via `has_passive_feature`.
///
/// RAW: "When you cast an enchantment spell of 1st level or higher that
/// targets only one creature, you can have it target a second
/// creature."
///
/// Read at `EncounterInstance::consume_split_enchantment`, which shares
/// its target picker with the Sorcerer's Twinned Spell metamagic and
/// resolves through the same doubling block in `Action::execute`. The
/// two features want the same second creature for the same reasons and
/// differ only in price and scope: Twinned Spell is a prime costing
/// `max(1, level)` sorcery points that covers any single-target spell
/// including cantrips; Split Enchantment is free and permanent but only
/// touches leveled enchantments.
pub const SPLIT_ENCHANTMENT_TAG: &str = "wizard.split_enchantment";

/// Class-feature tag for the Enchantment Wizard's **Hypnotic Gaze**
/// (School of Enchantment subclass level 2, PHB). Membership marker
/// gating the `HYPNOTIC_GAZE` action; the once-per-turn cadence lives
/// on the action's own Action-slot cost rather than a charge pool,
/// matching RAW's "as an action" wording.
///
/// RAW: "As an action, choose one creature that you can see within 5
/// feet of you. If the target can see or hear you, it must succeed on a
/// Wisdom saving throw against your wizard spell save DC or be charmed
/// by you until the end of your next turn. The charmed target is
/// incapacitated." Sustaining it across later turns is RAW-optional and
/// costs the enchanter their action every round; we ship the one-shot
/// install, which is the load-bearing clause — an incapacitated
/// creature that also cannot attack the enchanter is out of the fight
/// for a round without a slot being spent.
///
/// Installs both `Charmed` (with the `Charmed` back-link, so the
/// engine-wide "can't attack the charmer" restriction binds) and
/// `Incapacitated`. The pairing is the point: `Charmed` alone is a
/// targeting restriction, `Incapacitated` alone leaves the target free
/// to attack the enchanter's allies, and only together do they match
/// RAW's "out of the fight, and specifically out of *your* fight".
pub const HYPNOTIC_GAZE_TAG: &str = "wizard.hypnotic_gaze";

/// Class-feature tag for the Illusion Wizard's **Illusory Self**
/// (School of Illusion subclass level 10, PHB). Once-per-short-rest
/// reactive charge, registered in `SHORT_REST_FEATURES`.
///
/// RAW: "When a creature makes an attack roll against you, you can use
/// your reaction to interpose the illusory duplicate between the
/// attacker and yourself. The attack automatically misses you, then
/// the illusion dissipates."
///
/// Read at `EncounterInstance::illusory_self_deflect`, the second row
/// of the shared `attack_intercepted` cohort that both attack
/// chokepoints (`engine::attack::resolve_attack_outcome` for weapon
/// swings, `spells::spell_attack_outcome` for spell attacks) consult
/// once a swing is known to connect. Firing it *after* the hit is
/// known is a deliberate departure from RAW's declaration window —
/// RAW makes the illusionist commit before the d20 lands, and a
/// charge spent on a swing that would have missed anyway is pure
/// waste. The engine has no "would you like to spend this?" channel,
/// so it spends optimally instead of guessing; the same simplification
/// `apply_bend_luck_penalty` already makes one block up.
///
/// Two clauses separate it from Mirror Image, the cohort row above it:
///
///   1. **It beats crits.** Mirror Image explicitly cannot deflect a
///      critical hit; Illusory Self's "the attack automatically misses"
///      carries no such carve-out. It is the only defense in the engine
///      that erases a confirmed crit outright.
///   2. **It costs a reaction.** Mirror Image's decoys soak swings
///      passively and can absorb several per round. Illusory Self
///      competes with Shield, Absorb Elements, opportunity attacks and
///      every other reactive lane for a single slot per round.
///
/// The cohort ordering falls out of that: Mirror Image is tried first
/// because its decoys are the cheaper resource and are already paid
/// for, and the per-rest charge is held back for the swing the decoys
/// let through.
pub const ILLUSORY_SELF_TAG: &str = "wizard.illusory_self";

/// Class-feature tag for the Conjuration Wizard's **Focused
/// Conjuration** (School of Conjuration subclass level 10, PHB).
/// Passive, always-on, no charge — membership marker read via
/// `has_passive_feature`.
///
/// RAW: "Beginning at 10th level, while you are concentrating on a
/// conjuration spell, your concentration can't be broken as a result of
/// taking damage."
///
/// Read at `EncounterInstance::roll_concentration_save`, which
/// short-circuits to `Pass` ahead of the roll when the holder is
/// concentrating on a spell whose `Action::school()` is
/// `Conjuration`. Short-circuiting rather than auto-passing is the
/// RAW reading — no save is made at all — and it also keeps the
/// feature from consuming a d20 out of the encounter's seeded stream,
/// so a Conjurer in the party doesn't reshuffle every subsequent roll
/// for everyone else.
///
/// The damage lane is the *only* lane RAW covers, and the gate's
/// position encodes that: it sits on the save, not on
/// `drop_concentration`, so casting a second concentration spell,
/// going down, or failing a round-end save still ends the conjuration
/// normally.
///
/// This is the strongest concentration-protection in the engine and
/// the only unconditional one. The Warlock's Eldritch Mind invocation
/// — read at the same site — grants an advantage layer and still
/// rolls; War Caster-style flat bonuses would still roll. Focused
/// Conjuration doesn't roll. Its price is paid on the spell list
/// rather than in the feature: it protects Web, Cloudkill, Stinking
/// Cloud and Cloud of Daggers, and does nothing at all for Haste,
/// Hold Monster or Greater Invisibility.
pub const FOCUSED_CONJURATION_TAG: &str = "wizard.focused_conjuration";

/// Class-feature tag for the Conjuration Wizard's **Benign
/// Transposition** (School of Conjuration subclass level 6, PHB).
/// Gates the `BENIGN_TRANSPOSITION` action and carries its single
/// charge.
///
/// RAW: "As an action, you can teleport up to 30 feet to an unoccupied
/// space that you can see. Alternatively, you can choose a space within
/// range that is occupied by a Small or Medium creature. If that
/// creature is willing, you both teleport, swapping places. Once you
/// use this feature, you can't use it again until you finish a long
/// rest or you cast a conjuration spell of 1st level or higher."
///
/// Deliberately **not** in `SHORT_REST_FEATURES` — its recharge isn't
/// a rest at all. `EncounterInstance::trigger_benign_transposition_recharge`
/// refills the charge off the post-cast trigger registry whenever the
/// holder casts a levelled conjuration, which is the whole character of
/// the feature: a conjurer playing their own school blinks every round,
/// and one reaching for a Fireball goes without. The long-rest fallback
/// rides the normal `long_rest` refill for any tag not listed as
/// short-rest.
///
/// The RAW swap clause is left out. Both halves are a teleport of the
/// conjurer, and only the self-move half has a target the engine can
/// pick without a consent prompt — "if that creature is willing" is a
/// question the AI has no channel to ask an ally, and answering it
/// implicitly (any ally is always willing) would let the conjurer
/// yank an ally out of position against their interest. The self-blink
/// is the load-bearing clause and rides here alone, matching how
/// Hypnotic Gaze ships its one-shot install without RAW's optional
/// per-turn sustain.
pub const BENIGN_TRANSPOSITION_TAG: &str = "wizard.benign_transposition";

/// Benign Transposition — Conjuration Wizard lv6 self-teleport.
///
/// Shape-wise a Misty Step that costs an action instead of a bonus
/// action and a charge instead of a 2nd-level slot: same 30 ft (12
/// tile) range, same `SinglePoint` schema, same `can_move_to` landing
/// validation, same `TeleportActor` effect that bypasses the per-step
/// opportunity-attack dispatch a `MoveActor` would provoke.
///
/// The action cost is what keeps it from strictly dominating Misty
/// Step on the chassis that carries both: the conjurer who blinks with
/// this has spent their turn, where the one who blinks with Misty Step
/// has spent a slot and can still cast. They trade cleanly — the
/// charge is free and renewable but expensive in tempo, the slot is
/// scarce but cheap in tempo.
pub struct BenignTransposition {}

impl Action for BenignTransposition {
    fn name(&self) -> &str {
        "benign transposition"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bt", "transpose"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
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
        action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !caster.feature_available(BENIGN_TRANSPOSITION_TAG) {
            return false;
        }
        // Destination must be a legal landing spot for the conjurer's
        // full footprint — the same constraint Misty Step applies,
        // minus any movement-budget check, since a teleport bypasses
        // movement entirely.
        let Some(point) = first_target_location(target_locations) else {
            return false;
        };
        encounter.can_move_to(caster_id, point)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_feature(BENIGN_TRANSPOSITION_TAG);
        }
        vec![Box::new(crate::engine::side_effects::TeleportActor {
            actor_id: caster_id,
            dest: point,
        })]
    }
}

pub static BENIGN_TRANSPOSITION: LazyLock<BenignTransposition> =
    LazyLock::new(|| BenignTransposition {});

/// Overchannel — Evocation Wizard prime. Free (no action, no bonus
/// action, no slot): declares that the caster's next damaging spell of
/// level 1-5 deals maximum damage instead of rolling.
///
/// RAW isn't an action at all — it's a choice made while casting — but
/// the engine has no "pick an option mid-cast" surface, so it lands as a
/// prime, the same shape the six sorcerer metamagic primes use. The
/// difference from those is the price: metamagic charges sorcery points
/// up front, while Overchannel is free to declare and charges escalating
/// necrotic backlash *after* the cast it powers.
///
/// Consumed at `EncounterInstance::roll_empowered_sum` via
/// `consume_overchannel`, which swaps the dice roll for `Dice::max_roll`
/// and latches the backlash for the post-cast trigger. The prime only
/// burns on a cast that can actually use it, so declaring it and then
/// firing a cantrip or a level-6+ spell leaves it up.
pub struct Overchannel {}

impl Action for Overchannel {
    fn name(&self) -> &str {
        "overchannel"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["oc", "overchan"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        free_cost()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Holds the feature, isn't already primed. Free actions with no
        // gate would otherwise be spammable no-ops in the action list.
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.has_passive_feature(OVERCHANNEL_TAG) && !a.has_condition(Condition::Overchanneling)
        })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Surface the price before it's paid — the backlash for this use
        // is already determined by how many times the evoker has
        // overchannelled since their last long rest.
        let owed = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.overchannel_uses() + 1)
            .unwrap_or(1);
        let name = encounter.actor_name(caster_id);
        if owed <= 1 {
            encounter.log(format!("{} overchannels — the first use is free.", name));
        } else {
            encounter.log(format!(
                "{} overchannels again — {}d12 necrotic per spell level.",
                name, owed
            ));
        }
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Overchanneling,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static OVERCHANNEL: LazyLock<Overchannel> = LazyLock::new(|| Overchannel {});

/// Hypnotic Gaze — Enchantment Wizard action. Costs an Action and no
/// slot: one adjacent creature makes a WIS save against the enchanter's
/// INT-based spell save DC or is Charmed by them *and* Incapacitated
/// until the end of the enchanter's next turn.
///
/// The slot-free price is what makes this the subclass's signature
/// low-level tell. Every other "take a creature out of the fight for a
/// round" effect on the wizard chassis — Hold Person, Hypnotic Pattern,
/// Tasha's Hideous Laughter — costs a leveled slot; this one costs
/// nothing but proximity, which is also its whole restriction: an
/// INT-caster with a 2d6+2 frame standing in melee reach of the
/// creature it wants to lock down is making a real trade.
///
/// Both halves of the payload matter and neither is sufficient:
/// `Incapacitated` alone would leave the target free to walk over and
/// beat on the enchanter's allies next round, and `Charmed` alone is
/// only a targeting restriction. Installed together — with the
/// `Charmed` back-link, so the engine-wide "can't attack the charmer"
/// gate binds across declared actions, opportunity attacks and Riposte
/// alike — the target is out of the fight generally and out of the
/// enchanter's fight specifically.
///
/// RAW's sustain clause ("on subsequent turns, you can use your action
/// to maintain this effect") is left out: it is the same action spent
/// again, and re-casting the gaze each round reaches the same place
/// through the normal action economy. The one-shot install is the
/// load-bearing part.
pub struct HypnoticGaze {}

impl Action for HypnoticGaze {
    fn name(&self) -> &str {
        "hypnotic gaze"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hg", "gaze"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // RAW "within 5 feet of you" — the engine's melee envelope.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
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
        action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if !caster.has_passive_feature(HYPNOTIC_GAZE_TAG) {
            return false;
        }
        // Don't re-gaze a target that is already locked down — the
        // action is better spent elsewhere, and RAW's "sustain" clause
        // (which we don't model) is what would apply here anyway.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|t| !t.has_condition(Condition::Charmed))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let dc = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.spell_save_dc(AbilityScoreType::Intelligence))
            .unwrap_or(10);
        let caster_name = encounter.actor_name(caster_id);
        let target_name = encounter.actor_name(target_id);
        if encounter
            .roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id)
            .passed()
        {
            encounter.log(format!(
                "  hypnotic gaze: {} shakes off {}'s stare",
                target_name, caster_name
            ));
            return Vec::new();
        }
        encounter.log(format!(
            "  hypnotic gaze: {} is transfixed by {}",
            target_name, caster_name
        ));
        // RAW's duration is "until the end of your next turn". The
        // engine's `UntilStartOfNextTurn` timer clears at the start of
        // the *holder's* turn, which for a target that acts after the
        // enchanter lands one tick short; a flat 1-round timer is the
        // closer fit and matches how the sibling round-scale lockdowns
        // (Hold Person's stun window, Mind Whip) are timed.
        let mut out = crate::engine::side_effects::install_condition_with_link(
            Condition::Charmed,
            target_id,
            caster_id,
            ConditionTimer::Rounds(1),
        );
        out.push(Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Incapacitated,
            timer: ConditionTimer::Rounds(1),
        }));
        out
    }
}

pub static HYPNOTIC_GAZE: LazyLock<HypnoticGaze> = LazyLock::new(|| HypnoticGaze {});

/// Class-feature tag for the Transmutation Wizard's **Transmuter's
/// Stone** (School of Transmutation subclass level 6, PHB). A pure
/// membership marker: the stone's *benefit* lives on the
/// `transmuters_stone` template field, since RAW's pick-one-of-four is
/// a choice and a `HashSet<&'static str>` can only record a yes/no.
///
/// The tag is what a "does this wizard carry a stone at all?" read
/// should consult; the field is what the three benefit cohorts
/// (`PASSIVE_FEATURE_SPEED_BONUSES`, `FLAG_DRIVEN_SAVE_PROFICIENCIES`,
/// and the Warding OR in `has_passive_typed_resistance`) each match on.
/// Keeping both means the UI and any future "the stone shatters"
/// effect have a marker to read without having to spell out every
/// benefit variant.
pub const TRANSMUTERS_STONE_TAG: &str = "wizard.transmuters_stone";

/// Class-feature tag for the Transmutation Wizard's **Shapechanger**
/// (School of Transmutation subclass level 10, PHB). Gates the
/// `SHAPECHANGER` action and carries its once-per-short-rest charge,
/// registered in `SHORT_REST_FEATURES`.
///
/// RAW: "you can cast the polymorph spell without expending a spell
/// slot. When you do so, you can transform only into a beast with a
/// challenge rating of 1 or lower."
pub const SHAPECHANGER_TAG: &str = "wizard.shapechanger";

/// Shapechanger — Transmutation Wizard lv10 free self-Polymorph.
///
/// Mechanically the self-targeted half of the `POLYMORPH` spell with
/// the slot swapped for a per-short-rest charge: the same
/// `Polymorphed` condition, the same 30 temp HP, the same
/// concentration mark. The willing-target branch of Polymorph already
/// skips the WIS save, so a self-cast has nothing to roll and this
/// action has no randomness at all.
///
/// It is an emergency button rather than an opener. Thirty temp HP is
/// roughly triple a wizard's remaining margin at the point they'd want
/// it, and the beast form's own concentration means it costs whatever
/// control spell the wizard was holding — so the choice it poses is
/// "keep the Web up, or survive the round". RAW's CR-1-or-lower
/// restriction on the form has no surface here for the same reason
/// the base spell's form choice doesn't: `Polymorphed` is modelled as
/// a condition plus a temp-HP pool rather than a full stat-block swap.
///
/// Targets the caster unconditionally rather than taking a target, so
/// there is no way to point it at an ally — RAW's "you can cast the
/// polymorph spell" is explicitly a self-transformation clause.
pub struct Shapechanger {}

impl Action for Shapechanger {
    fn name(&self) -> &str {
        "shapechanger"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["shift", "beastform"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn requires_los(&self) -> bool {
        false
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
        action_only()
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
            .is_some_and(|a| a.feature_available(SHAPECHANGER_TAG))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if let Some(caster) = encounter.actors.get_mut(&caster_id) {
            caster.spend_feature(SHAPECHANGER_TAG);
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Polymorphed,
                timer: ConditionTimer::Permanent,
            }),
            Box::new(GainTempHp {
                actor_id: caster_id,
                amount: 30,
            }),
            Box::new(crate::engine::side_effects::StartConcentration {
                caster_id,
                data: crate::actors::actor_template::ConcentrationData::with_conditions(
                    "Polymorph",
                    vec![(caster_id, Condition::Polymorphed)],
                ),
            }),
        ]
    }
}

pub static SHAPECHANGER: LazyLock<Shapechanger> = LazyLock::new(|| Shapechanger {});

/// 5e Warlock — Otherworldly Patron **The Hexblade** (XGtE), subclass
/// level 1: **Hexblade's Curse**. Bonus action, once per short rest.
///
/// The tag gates the [`HEXBLADES_CURSE`] action's charge. The three
/// clauses it buys are all engine-side and all keyed off the
/// `HexbladeCursed` back-link rather than off this tag, because they
/// have to distinguish *the hexblade who cursed this creature* from any
/// other hexblade in the fight:
///
///   - **+proficiency bonus to damage** against the cursed target —
///     `EncounterInstance::curse_damage_bonus`, summed alongside
///     `caster_damage_buffs` at both attack-damage chokepoints.
///   - **Crit on 19-20** against the cursed target —
///     `EncounterInstance::crit_threshold_against`.
///   - **Heal on the target's death** — the curser-scoped row on
///     `EncounterInstance::trigger_creature_dropped`.
///
/// Registered in `SHORT_REST_FEATURES`: RAW is "until you finish a short
/// or long rest", which puts it on the same cadence as the Channel
/// Divinity family and Dark One's Own Luck rather than on the warlock's
/// long-rest features.
pub const HEXBLADES_CURSE_TAG: &str = "warlock.hexblades_curse";

/// 5e Warlock — Otherworldly Patron **The Hexblade** (XGtE), subclass
/// level 10: **Armor of Hexes**. Passive. When the target of the
/// hexblade's curse hits the hexblade with an attack roll, roll a d6; on
/// a 4 or higher the attack misses regardless of its total.
///
/// Third row on the `attack_intercepted` cohort, below Mirror Image and
/// Illusory Self. The ordering rule on that cohort is
/// cheapest-resource-first, and Armor of Hexes is the cheapest of the
/// three by a distance: it costs nothing at all — no reaction, no
/// charge, no spell — so it would seem to belong at the top. It sits at
/// the bottom instead because it is the only row that can *fail*. A
/// decoy or an Illusory Self always eats the swing; the hex armor eats
/// it half the time. Running the two certain rows first means their
/// resources are spent on hits the armor might have let through anyway,
/// which is the wrong trade — so the free-but-unreliable row goes last
/// and catches what the reliable ones didn't cover.
///
/// Unlike the two rows above it, this one is gated on *who* is swinging:
/// only the cursed target's own attacks are deflected, which is what
/// makes it a fair trade for a subclass that spends its bonus action
/// picking that creature out.
pub const ARMOR_OF_HEXES_TAG: &str = "warlock.armor_of_hexes";

/// Hexblade's Curse — Hexblade Warlock subclass level 1, bonus action,
/// once per short rest. Mark one hostile creature within 30 ft (12
/// tiles) for 10 rounds (1 minute RAW).
///
/// Installs `Condition::HexbladeCursed` plus its back-link to the
/// hexblade, through the shared `install_condition_with_link` helper so
/// the flag and the link can't come apart. Everything the curse does is
/// read back off that pair.
///
/// **Why it's worth a bonus action and a rest charge.** The curse is
/// three riders that all point the same way, and their value scales with
/// how many attacks the hexblade lands on one creature before it dies:
/// +proficiency per hit, a doubled crit rate, and a heal that only pays
/// if the target actually falls. That makes it the opposite of the
/// warlock's other openers — Hex spreads across whatever the warlock
/// swings at and follows the kill, Hexblade's Curse commits to a single
/// creature and cannot move once placed — the engine ships no *Master of
/// Hexes*. So the target to want is the one that will absorb the most
/// swings, not the one closest to dying: the heal is a bonus that
/// arrives when the fight is already won, and a curse spent on something
/// that drops next round leaves eight rounds of nothing. The AI's picker
/// takes the highest-HP hostile in range for exactly that reason.
///
/// **Stacking with Hex.** Both can be up at once and they compose:
/// Hex is concentration and adds 1d6 necrotic per hit, the curse is
/// neither and adds a flat proficiency bonus. A hexblade who lands both
/// on the same creature is playing the subclass as intended — the two
/// riders are the reason the build's single-target damage outruns the
/// baseline warlock's even though its spell list is the same.
pub struct HexbladesCurse {}

impl Action for HexbladesCurse {
    fn name(&self) -> &str {
        "hexblade's curse"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hexblades curse", "curse", "hex curse", "hc"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft RAW = 12 tiles.
        Some(12)
    }
    fn is_harmful(&self) -> bool {
        // No damage and no save, but it may only be aimed at an enemy —
        // same declaration Vow of Enmity makes for the same reason: it
        // keeps the AI's helpful-action lane from ever considering it,
        // and `custom_validate_input` enforces the hostile-target
        // requirement independently.
        true
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Shared "charge unspent + hostile, living target" preamble, then
        // the one clause it doesn't cover: don't re-curse a creature we
        // have already cursed. Re-applying only refreshes a 10-round
        // timer that has barely started and costs the whole rest charge
        // to do it. Same dedup Vow of Enmity carries, and for the same
        // reason — a target cursed by a *different* hexblade is still a
        // legal target, and the install takes the link over.
        if !hostile_target_burst_ready(encounter, caster_id, target_ids, HEXBLADES_CURSE_TAG) {
            return false;
        }
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter.hexblade_curse_holder(target_id) != Some(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if let Some(hexblade) = encounter.actors.get_mut(&caster_id) {
            hexblade.spend_feature(HEXBLADES_CURSE_TAG);
        }
        let target_name = encounter.actor_name(target_id);
        encounter.log(format!(
            "  hexblade's curse: the patron's mark settles on {}.",
            target_name
        ));
        crate::engine::side_effects::install_condition_with_link(
            Condition::HexbladeCursed,
            target_id,
            caster_id,
            // 10 rounds = 1 minute RAW, the same window Vow of Enmity
            // and Hunter's Mark run on.
            ConditionTimer::Rounds(10),
        )
    }
}

pub static HEXBLADES_CURSE: LazyLock<HexbladesCurse> = LazyLock::new(|| HexbladesCurse {});

/// Tag for the Circle of Spores Druid's **Symbiotic Entity** (subclass
/// level 2). Once per short rest — RAW spends a Wild Shape use, and
/// Wild Shape recharges on a short rest, so the tag rides
/// `SHORT_REST_FEATURES` rather than the long-rest default.
pub const SYMBIOTIC_ENTITY_TAG: &str = "druid.symbiotic_entity";

/// Passive tag for the Circle of Spores Druid's **Halo of Spores**
/// (subclass level 2). Never spent — the halo is at-will, priced in the
/// druid's reaction rather than in a per-rest charge — so it lives on
/// `features_max` and is read with `has_passive_feature`.
pub const HALO_OF_SPORES_TAG: &str = "druid.halo_of_spores";

/// Temporary hit points **Symbiotic Entity** hands out. RAW is 4 × druid
/// level; the druid chassis in this engine is a level-9 build, so 36.
///
/// Deliberately in the same weight class as the Moon Druid's
/// `BEAST_FORM_TEMP_HP` (34), because the two subclasses are making the
/// same offer from opposite directions and the offer should cost about
/// the same. The Moon Druid buys a body — 34 HP and a 2d6+4 claw — and
/// pays for it by locking out every spell slot they own. The Spores
/// Druid buys a rider — 36 HP and +1d6 necrotic on each melee swing —
/// and pays for it with nothing at all except the charge, because the
/// symbiote leaves their spell list entirely intact.
///
/// That looks lopsided until you notice what the two are actually
/// worth. The bear's claw is the Moon Druid's whole offense while the
/// form is up; the symbiote's 1d6 rides a scimitar, which is the worst
/// attack on a druid's sheet and the one they were least likely to make.
/// The Spores Druid's shield is real and their sword is not, so the
/// feature reads as "stay a caster, but stop dying to the first thing
/// that reaches you" — which is exactly the niche RAW gives the circle.
pub const SYMBIOTIC_ENTITY_TEMP_HP: u32 = 36;

/// Symbiotic Entity — Circle of Spores Druid action (subclass level 2).
/// Animates the druid's spore halo into a symbiote: they gain
/// `SYMBIOTIC_ENTITY_TEMP_HP` temporary hit points, their melee weapon
/// hits pick up +1d6 necrotic (the `SymbioticEntity` row on
/// `ON_HIT_RIDERS`), and Halo of Spores rolls its damage die twice.
///
/// The condition carries no timer of its own: RAW ends the feature
/// "when you lose all these temporary hit points," and the engine
/// enforces exactly that at `ActorInstance::drain_temp_hp` via the
/// `TEMP_HP_BOUND_CONDITIONS` cohort. A `Permanent` install is therefore
/// the honest encoding — the shield is the clock. RAW's other two end
/// conditions (10 minutes elapsed, or a second Wild Shape) have no
/// encounter surface: ten minutes outlasts every fight the engine runs,
/// and no Spores Druid template carries Wild Shape.
pub struct SymbioticEntityAction {}

impl Action for SymbioticEntityAction {
    fn name(&self) -> &str {
        "symbiotic entity"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["symbiote", "se"]
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
    fn is_heal(&self) -> bool {
        // Temp HP, not healing — but the AI's support pipeline is the
        // right lane for "this makes the holder harder to kill," and
        // that pipeline reads `is_heal`. Same call the Armor of Agathys
        // and False Life self-buffs make.
        true
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
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Same shape as every once-per-rest self-prime: charge unspent,
        // holder alive, and don't re-install a symbiote that is already
        // riding them (the temp HP wouldn't stack — `gain_temp_hp` keeps
        // the larger pool — so a second cast would burn the charge for
        // nothing).
        feature_prime_ready(
            encounter,
            caster_id,
            SYMBIOTIC_ENTITY_TAG,
            Condition::SymbioticEntity,
        )
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if let Some(druid) = encounter.actors.get_mut(&caster_id) {
            druid.spend_feature(SYMBIOTIC_ENTITY_TAG);
        }
        let name = encounter.actor_name(caster_id);
        encounter.log(format!(
            "  symbiotic entity: {}'s spore halo condenses into a symbiote.",
            name
        ));
        vec![
            Box::new(GainTempHp {
                actor_id: caster_id,
                amount: SYMBIOTIC_ENTITY_TEMP_HP,
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::SymbioticEntity,
                // Permanent because the temp-HP pool is the real timer —
                // see the type docs.
                timer: ConditionTimer::Permanent,
            }),
        ]
    }
}

pub static SYMBIOTIC_ENTITY: LazyLock<SymbioticEntityAction> =
    LazyLock::new(|| SymbioticEntityAction {});

/// Halo of Spores — Circle of Spores Druid reaction (subclass level 2).
/// One creature within 10 ft takes 1d6 necrotic unless it succeeds on a
/// Constitution save against the druid's spell save DC. While Symbiotic
/// Entity is up, the die is rolled twice.
///
/// The only player-declared reaction in the engine — see
/// `reaction_only` for why that matters. In play it gives the Spores
/// Druid something no other full caster has: a damage floor that costs
/// them nothing they were going to spend. A druid who casts Moonbeam
/// with their Action and steps back with their movement still gets to
/// pull the halo's trigger on whatever chased them, and the reaction was
/// otherwise going to expire unused.
///
/// RAW's damage die scales with druid level (1d4 at 2, 1d6 at 6, 1d8 at
/// 10, 1d10 at 14). The chassis is a level-9 build, so 1d6.
pub struct HaloOfSpores {}

/// The Halo of Spores damage die, before Symbiotic Entity doubles it.
const HALO_OF_SPORES_DIE: Dice = Dice::new(1, 6);

/// Reach of the Halo of Spores, in tiles. RAW 10 ft on the engine's
/// 2.5 ft grid.
const HALO_OF_SPORES_REACH: isize = 4;

impl Action for HaloOfSpores {
    fn name(&self) -> &str {
        "halo of spores"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["halo", "spores"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(HALO_OF_SPORES_REACH)
    }
    fn requires_los(&self) -> bool {
        // RAW: "one creature you can see within 10 feet of you."
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        reaction_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Passive feature, so there is no charge to check — just the
        // flag, a live holder, and a live hostile target. The reach and
        // line-of-sight gates are the engine's, via `reach_tiles` /
        // `requires_los`.
        let holds = encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.has_passive_feature(HALO_OF_SPORES_TAG));
        if !holds {
            return false;
        }
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter.actors_enemies(caster_id, target_id)
            && encounter
                .actors
                .get(&target_id)
                .is_some_and(|t| t.is_combat_active())
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
        let Some(druid) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = druid.spell_save_dc(AbilityScoreType::Wisdom);
        // RAW Symbiotic Entity: "when you damage a creature with your
        // Halo of Spores, roll the damage die a second time and add it
        // to the total."
        let doubled = druid.has_condition(Condition::SymbioticEntity);
        let dice = if doubled {
            Dice::new(HALO_OF_SPORES_DIE.count * 2, HALO_OF_SPORES_DIE.faces)
        } else {
            HALO_OF_SPORES_DIE
        };
        let save = encounter.roll_save_against_caster(
            target_id,
            AbilityScoreType::Constitution,
            dc,
            caster_id,
        );
        let target_name = encounter.actor_name(target_id);
        if save.passed() {
            // RAW is all-or-nothing here, unlike the engine's usual
            // half-on-save bursts — the halo is a small die that a
            // successful save shrugs off entirely.
            encounter.log(format!(
                "  halo of spores: {} shrugs off the spore cloud.",
                target_name
            ));
            return Vec::new();
        }
        let damage = encounter.roll(&dice);
        encounter.log(format!(
            "  halo of spores: {} breathes in {} necrotic{}",
            target_name,
            damage,
            if doubled { " (symbiote-fed)." } else { "." }
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: damage,
            damage_type: DamageType::Necrotic,
        })]
    }
}

pub static HALO_OF_SPORES: LazyLock<HaloOfSpores> = LazyLock::new(|| HaloOfSpores {});

/// Tag for the Conquest Paladin's **Conquering Presence** (Oath of
/// Conquest, subclass level 3 Channel Divinity). Once per short rest,
/// like every sibling paladin CD charge.
pub const CONQUERING_PRESENCE_TAG: &str = "paladin.conquering_presence";

/// Conquering Presence — Conquest Paladin Channel Divinity (lv3),
/// action. Every hostile within 30 ft makes a WIS save vs the paladin's
/// CHA-anchored DC or is Frightened for 10 rounds.
///
/// Mechanically the twin of the Oathbreaker's Dreadful Aspect, and that
/// is not an accident of modeling — RAW writes the two oaths' level-3
/// Channel Divinities almost identically, because the difference
/// between the two subclasses is not the fear, it is what each of them
/// does *with* the fear afterwards. The Oathbreaker frightens and then
/// hits harder (Aura of Hate). The Conqueror frightens and then locks
/// the frightened where they stand (Aura of Conquest), which turns a
/// crowd-control button into a kill box.
///
/// That is the whole reason to ship it as a second `TurnBurst` literal
/// rather than to reuse Dreadful Aspect on the Conquest chassis: the
/// two need separate per-rest charges, separate names in the log, and
/// separate entries in the prompt parser, and the config-driven struct
/// makes all three cost five lines.
pub static CONQUERING_PRESENCE: LazyLock<TurnBurst> = LazyLock::new(|| TurnBurst {
    name: "conquering presence",
    aliases: &["cp", "cd-conquer", "conquering", "conquer"],
    tag: CONQUERING_PRESENCE_TAG,
    dc_ability: AbilityScoreType::Charisma,
    type_filter: |_| true,
    installed: Condition::Frightened,
});

/// Tag for the Bladesinging Wizard's **Bladesong** (subclass level 2).
/// RAW grants two uses per short rest; the engine's per-tag charge model
/// collapses that to one, and the tag rides `SHORT_REST_FEATURES` so the
/// cadence stays RAW's.
pub const BLADESONG_TAG: &str = "wizard.bladesong";

/// Bladesong — Bladesinging Wizard bonus action (subclass level 2). For
/// one minute the wizard is a moving blade: +INT to AC, +10 ft of
/// speed, +INT to the Constitution saves that hold their concentration
/// together, and (via Song of Victory) +INT to melee weapon damage.
///
/// Four clauses, four different engine lanes, and none of them a flat
/// number — which is why the feature needed three separate pieces of
/// engine before the action could exist:
///   - AC: `ABILITY_SCALED_AC_BONUSES`, the closure-based sibling to
///     the flat AC cohort, which also absorbed Sacred Weapon's
///     hand-written +CHA branch on the attack lane.
///   - Speed: an ordinary `CONDITION_SPEED_BONUSES` row — this one
///     *is* flat.
///   - Concentration: `roll_save_with_extra_mode_and_bonus`, because
///     RAW scopes the bonus to concentration saves and every existing
///     save-bonus lane applies to all of them.
///   - Damage: a `MELEE_CASTER_BUMPS` row.
///
/// RAW's remaining clauses have no combat surface: advantage on
/// Acrobatics checks (no skill-check surface for it), and the
/// restriction to light-or-no armor with no shield (the engine doesn't
/// model armor categories, and the Bladesinger template dresses as a
/// wizard anyway).
pub struct Bladesong {}

impl Action for Bladesong {
    fn name(&self) -> &str {
        "bladesong"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bs", "sing", "blade song"]
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
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_prime_ready(encounter, caster_id, BLADESONG_TAG, Condition::Bladesinging)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            BLADESONG_TAG,
            Condition::Bladesinging,
            // 10 rounds = 1 minute RAW, the same window Rage and Sacred
            // Weapon run on.
            ConditionTimer::Rounds(10),
            "  bladesong: the wizard's blade takes up the song.",
        )
    }
}

pub static BLADESONG: LazyLock<Bladesong> = LazyLock::new(|| Bladesong {});

/// Passive tag for the Path of the Ancestral Guardian Barbarian's
/// **Ancestral Protectors** (subclass level 3). Never spent — the mark
/// is at-will, gated on the barbarian's Rage rather than on a charge —
/// so it lives on `features_max` and is read with `has_passive_feature`.
///
/// Doubles as the once-per-turn ledger key: the `FirstHitOfTurn` row on
/// `ON_HIT_CONDITION_MARKS` marks the tag used on the turn's first
/// connecting swing and reads it back on every swing after.
pub const ANCESTRAL_PROTECTORS_TAG: &str = "barbarian.ancestral_protectors";

/// Tag for the Undead Warlock's **Form of Dread** (subclass level 1).
/// RAW grants uses equal to the proficiency bonus per long rest; the
/// engine's single charge lands on the short-rest lane, which is where
/// the warlock's own Pact Magic slots refresh and therefore the cadence
/// the rest of the chassis is priced against.
///
/// Doubles as the once-per-turn ledger key for the fear rider — see the
/// `FormOfDread` row on `ON_HIT_RIDERS`, whose `once_per_turn_tag`
/// points here.
pub const FORM_OF_DREAD_TAG: &str = "warlock.form_of_dread";

/// Warlock level the Undead patron's Form of Dread scales its temporary
/// hit points off. RAW is `1d10 + warlock level`; the warlock chassis in
/// this engine is a level-9 build.
const FORM_OF_DREAD_LEVEL: u32 = 9;

/// Form of Dread — Undead Warlock bonus action (subclass level 1). The
/// warlock briefly becomes the thing their patron is. For one minute:
/// `1d10 + 9` temporary hit points, immunity to Frightened, and once on
/// each of their turns a creature they hit must make a Wisdom save
/// against their spell DC or be Frightened of them until the end of the
/// warlock's next turn.
///
/// The two fear clauses are the feature. A warlock spreading Frightened
/// around is a warlock other things want to frighten back, and the
/// immunity is what makes the offensive half safe to lean on rather
/// than a race. The temp HP is the third leg of the same idea: the form
/// is about being the scariest thing in the room and surviving long
/// enough for that to matter.
///
/// The fear rider needed the `once_per_turn_tag` lane on
/// `ON_HIT_RIDERS`. The table's existing "stop firing" mechanism is
/// `consume_on_trigger`, which strips the rider's condition — right for
/// a Smite prime, wrong here, because the form has to survive its own
/// fear going off and keep granting the immunity and the temp HP for
/// the rest of the minute.
pub struct FormOfDreadAction {}

impl Action for FormOfDreadAction {
    fn name(&self) -> &str {
        "form of dread"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fod", "dread form", "transform"]
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
    fn is_heal(&self) -> bool {
        // Temp HP rather than healing, but the AI's support pipeline is
        // the lane for "this makes the holder harder to kill" — the
        // same call Symbiotic Entity and Armor of Agathys make.
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_prime_ready(
            encounter,
            caster_id,
            FORM_OF_DREAD_TAG,
            Condition::FormOfDread,
        )
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let temp = encounter.roll(&Dice::new(1, 10)) + FORM_OF_DREAD_LEVEL;
        let mut effects = prime_self_condition(
            encounter,
            caster_id,
            FORM_OF_DREAD_TAG,
            Condition::FormOfDread,
            // 10 rounds = 1 minute RAW.
            ConditionTimer::Rounds(10),
            "  form of dread: the warlock's shape curdles into something the dark recognizes.",
        );
        effects.push(Box::new(GainTempHp {
            actor_id: caster_id,
            amount: temp,
        }));
        effects
    }
}

pub static FORM_OF_DREAD: LazyLock<FormOfDreadAction> = LazyLock::new(|| FormOfDreadAction {});

/// Passive tag for the Way of the Kensei Monk's **Deft Strike**
/// (subclass level 6). At-will once per turn — RAW's 1 ki cost is
/// dropped for the same reason Flurry of Blows' is (the engine tracks
/// no ki pool), so the tag lives on `features_max` and doubles as the
/// once-per-turn ledger key for its row on
/// `ONCE_PER_TURN_WEAPON_DIE_RIDERS`.
pub const DEFT_STRIKE_TAG: &str = "monk.deft_strike";

/// Kensei's Shot — Way of the Kensei Monk bonus action (subclass level
/// 3). Until the end of the turn, every ranged weapon attack the monk
/// lands carries an extra 1d4.
///
/// At-will, no charge: RAW costs the bonus action and nothing else,
/// which is unusual enough on this chassis to be the point. Every other
/// bonus action a monk has — Flurry of Blows, Patient Defense, Step of
/// the Wind — pays them for closing to contact. This one pays them for
/// staying at range, and it costs the same nothing, so the Kensei is
/// the first monk build with a reason to hold a bow.
pub struct KenseisShotAction {}

impl Action for KenseisShotAction {
    fn name(&self) -> &str {
        "kensei's shot"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ks", "kensei shot", "kensei"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Indirect: the rider lands on the shot, not on the prime.
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // No charge to check, so this is the live-holder gate plus the
        // don't-re-prime dedup that every bonus-action prime carries.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && !a.has_condition(Condition::KenseisShot))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  kensei's shot: the monk breathes onto the arrow before drawing.".to_string());
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::KenseisShot,
            // RAW: "until the end of the current turn".
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static KENSEIS_SHOT: LazyLock<KenseisShotAction> = LazyLock::new(|| KenseisShotAction {});

/// Per-rest charge for the Rune Knight Fighter's **Giant's Might**
/// (subclass level 3). RAW hands out proficiency-bonus uses per long
/// rest; the engine's feature set holds one charge per tag, so this is
/// one use that comes back on a short rest — the same collapse every
/// other multi-use fighter charge on this chassis takes.
pub const GIANTS_MIGHT_TAG: &str = "fighter.giants_might";

/// Once-per-turn ledger key for Giant's Might's damage rider. Separate
/// from `GIANTS_MIGHT_TAG` because the two count different things: the
/// charge is spent once to turn the feature on for a minute, and the
/// ledger is what stops the +1d6 from landing on every swing inside that
/// minute. Sharing one tag would let the first hit of the fight consume
/// the rest of the fight's growth.
pub const GIANTS_MIGHT_RIDER_TAG: &str = "fighter.giants_might.rider";

/// Giant's Might — Rune Knight Fighter bonus action (subclass level 3).
/// For a minute the fighter grows one size category, saves with
/// advantage on STR, and once on each of their turns a connecting weapon
/// hit carries an extra 1d6.
///
/// The interesting half is the one that isn't damage. Growing widens the
/// fighter's own footprint, and a footprint is what the engine measures
/// reach from — so a Large Rune Knight threatens opportunity attacks
/// across a wider ring than a Medium one, and can reach a caster hiding
/// one tile further back. That makes it the first class feature on the
/// roster whose main effect is geometric rather than numeric.
///
/// It is also the first feature that can be *refused by the board*: RAW
/// grows the fighter "if there is enough room", and in a corridor there
/// may not be. The charge is still spent and the damage rider still
/// fires — the fighter is holding the effect, the walls are just in the
/// way — and `reconcile_footprints` grows them the moment a neighbour
/// steps aside.
pub struct GiantsMightAction {}

impl Action for GiantsMightAction {
    fn name(&self) -> &str {
        "giant's might"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gm", "giants might", "giant might"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Indirect: the rider lands on the swing, not on the prime.
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
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        feature_prime_ready(
            encounter,
            caster_id,
            GIANTS_MIGHT_TAG,
            Condition::GiantsMight,
        )
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        prime_self_condition(
            encounter,
            caster_id,
            GIANTS_MIGHT_TAG,
            Condition::GiantsMight,
            // 10 rounds = the 1 minute RAW gives it.
            ConditionTimer::Rounds(10),
            "  giant's might: rune-light runs down the fighter's arms as they grow.",
        )
    }
}

pub static GIANTS_MIGHT: LazyLock<GiantsMightAction> = LazyLock::new(|| GiantsMightAction {});

/// Per-rest charge for the Rune Knight Fighter's **Fire Rune** (subclass
/// level 3). RAW recharges every rune on a short rest, which is the
/// cadence `SHORT_REST_FEATURES` already carries.
pub const FIRE_RUNE_TAG: &str = "fighter.fire_rune";

/// Fire Rune — Rune Knight Fighter (subclass level 3). Bonus action;
/// primes the next weapon hit to burn for an extra 2d6 fire and force a
/// STR save, with a failure leaving the target Restrained by chains of
/// fire.
///
/// Rides the `ManeuverPrime` shape the Battle Master's maneuvers use,
/// because it is mechanically the same object: a per-rest charge spent
/// on a bonus action to hang a condition on the fighter that the next
/// swing cashes in. The shape was named after the feature that arrived
/// first, not after a restriction — a rune spends its charge exactly the
/// way a maneuver does.
///
/// Where it differs from every maneuver is what it pays out. The
/// maneuvers trade their superiority die away for control (Trip's prone,
/// Menacing's fear) and the engine drops the die entirely; Fire Rune
/// carries real damage *and* a hard control rider, which is why it costs
/// its own charge rather than sharing the maneuver pool.
pub static FIRE_RUNE: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "fire rune",
    aliases: &["fr", "rune of fire"],
    tag: FIRE_RUNE_TAG,
    prime_condition: Condition::FireRuneInvoked,
    // Long enough to survive the fighter's own turn and be spent on an
    // opportunity attack before their next one, matching the maneuvers.
    timer: ConditionTimer::Rounds(2),
    log_line: "  fire rune: the rune on the fighter's weapon kindles.",
});

/// Passive tag for the Death Domain Cleric's **Reaper** (subclass level
/// 1): a necromancy cantrip that targets one creature can target two
/// standing within 5 ft of each other instead.
///
/// A passive with no charge and no action — the tag is the whole gate,
/// read by `EncounterInstance::consume_reaper` from the cast-doubling
/// chain in `Action::execute`. It lives on `features` (and therefore on
/// `features_max`) the same way `SPLIT_ENCHANTMENT_TAG` does, because
/// `has_passive_feature` is what both are asked.
pub const REAPER_TAG: &str = "cleric.reaper";

/// Channel Divinity charge for the Death Domain Cleric's **Reaper's
/// Touch** (subclass level 2). Short-rest cadence, like every other
/// cleric Channel Divinity in the engine.
///
/// RAW calls the feature Touch of Death, but so does the Way of the
/// Long Death Monk's level-3 feature, and both a tag and an action name
/// have to be unique here. The cleric's is renamed rather than the
/// monk's because the monk's shipped first and is already wired into
/// templates and prose; "reaper's touch" also names the domain feature
/// it shares a chassis with.
pub const REAPERS_TOUCH_TAG: &str = "cleric.reapers_touch";

/// Reaper's Touch — Death Domain Cleric Channel Divinity (subclass
/// level 2, RAW "Touch of Death"), bonus action. Primes the cleric's
/// next melee hit to carry a slab of extra necrotic damage.
///
/// RAW pays a flat `5 + twice your cleric level`, which on the level-5
/// chassis these templates target is 15. The rider table speaks in dice,
/// so the row pays 3d8 — the same average as a level-4 cleric's flat
/// value, and unlike the flat number it has a spread, which suits a
/// once-per-rest burst better than a guaranteed constant would.
///
/// The biggest single rider on a cleric's melee lane by some distance:
/// Divine Strike is 1d8, and this is triple it. That is the Death
/// Domain's shape — a caster domain whose Channel Divinity is spent
/// standing next to something.
pub static REAPERS_TOUCH: LazyLock<PrimeStrike> = LazyLock::new(|| PrimeStrike {
    name: "reaper's touch",
    aliases: &["rt", "cd-death"],
    tag: REAPERS_TOUCH_TAG,
    prime: Condition::TouchingDeath,
    log_line: "  reaper's touch: the cleric's palm goes cold to the wrist.",
});

/// Class-feature tag for the Death Domain Cleric's **Divine Strike
/// (necrotic)** (subclass level 8) — the domain's own typing of the
/// shared cleric feature, sibling to the radiant baseline and the
/// Trickery domain's poison.
pub const DIVINE_STRIKE_NECROTIC_TAG: &str = "cleric.divine_strike_necrotic";

/// Divine Strike (necrotic) — Death Domain Cleric level-8 subclass
/// feature, bonus action. Same envelope as the radiant baseline; the
/// `DivineStrikingNecrotic` rider row swaps 1d8 radiant for 1d8
/// necrotic.
///
/// The three-token canonical name follows `divine strike poison` for
/// the same parser reason: the longest token-prefix that names an
/// action wins, so a template carrying both would still resolve each by
/// its full name. No template does — the domains swap rather than
/// stack.
pub static DIVINE_STRIKE_NECROTIC: LazyLock<PrimeStrike> = LazyLock::new(|| PrimeStrike {
    name: "divine strike necrotic",
    aliases: &["nstrike", "cd-necrotic"],
    tag: DIVINE_STRIKE_NECROTIC_TAG,
    prime: Condition::DivineStrikingNecrotic,
    log_line: "  divine strike (necrotic): cleric's next melee hit will land withering.",
});

/// Passive tag for the Order Domain Cleric's **Voice of Authority**
/// (subclass level 1): casting a spell of 1st level or higher on an ally
/// lets that ally spend their reaction on one weapon attack immediately.
///
/// No charge and no action of its own — the tag is the whole gate, read
/// by `EncounterInstance::trigger_voice_of_authority` from the post-cast
/// dispatcher. Sibling in shape to `REAPER_TAG` and
/// `SPLIT_ENCHANTMENT_TAG`: an always-on passive that changes what the
/// cleric's *other* actions do.
pub const VOICE_OF_AUTHORITY_TAG: &str = "cleric.voice_of_authority";

/// Channel Divinity charge for the Order Domain Cleric's **Order's
/// Demand** (subclass level 2). Short-rest cadence, like every other
/// cleric Channel Divinity in the engine.
pub const ORDERS_DEMAND_TAG: &str = "cleric.orders_demand";

/// Order's Demand — Order Domain Cleric Channel Divinity (subclass level
/// 2). Action; every hostile within 30 ft makes a WIS save or is Charmed
/// for 10 rounds.
///
/// A `TurnBurst` literal with no creature-type gate and Charmed in place
/// of Frightened — the same shape as the Nature Domain's Charm Animals
/// and Plants, minus that feature's beast-or-plant filter. The missing
/// filter is the whole difference and it is a large one: Order's Demand
/// works on the humanoid bandits and the fiends alike, where Nature's
/// version needs the right bestiary.
///
/// RAW also drops what each target is holding. The engine has no held-
/// item lane for monsters, and the Charmed install is the load-bearing
/// half either way — a charmed creature can't attack the cleric at all.
pub static ORDERS_DEMAND: LazyLock<TurnBurst> = LazyLock::new(|| TurnBurst {
    name: "order's demand",
    aliases: &["od", "demand", "cd-order"],
    tag: ORDERS_DEMAND_TAG,
    dc_ability: AbilityScoreType::Wisdom,
    type_filter: |_| true,
    installed: Condition::Charmed,
});

/// Class-feature tag for the Order Domain Cleric's **Divine Strike
/// (psychic)** (subclass level 8) — the domain's typing of the shared
/// cleric feature, fourth after radiant, poison and necrotic.
pub const DIVINE_STRIKE_PSYCHIC_TAG: &str = "cleric.divine_strike_psychic";

/// Divine Strike (psychic) — Order Domain Cleric level-8 subclass
/// feature, bonus action. Same envelope as the radiant baseline; the
/// `DivineStrikingPsychic` rider row swaps 1d8 radiant for 1d8 psychic.
///
/// Psychic is the widest-landing typing of the four: almost nothing in
/// the bestiary resists it, where poison is shrugged off by every undead
/// and construct and radiant by the celestials. The Order cleric's
/// smaller die lands more often than the Death cleric's larger one.
pub static DIVINE_STRIKE_PSYCHIC: LazyLock<PrimeStrike> = LazyLock::new(|| PrimeStrike {
    name: "divine strike psychic",
    aliases: &["ystrike", "cd-psychic"],
    tag: DIVINE_STRIKE_PSYCHIC_TAG,
    prime: Condition::DivineStrikingPsychic,
    log_line: "  divine strike (psychic): cleric's next melee hit will land as a verdict.",
});

/// Per-rest charge for the Soulknife Rogue's **Homing Strikes** (Soul
/// Blades, subclass level 9): a psychic blade that missed gets a 1d8
/// added to the roll, which is often enough to turn it into a hit.
///
/// No action of its own — the charge is spent automatically at the
/// attack-roll site by the `MISSED_ATTACK_BOOSTS` cohort, the same way
/// Indomitable's is spent by the failed-save cohort. RAW sizes the
/// Psionic Energy pool by proficiency bonus; the engine's feature set
/// holds one charge per tag, refreshed on a short rest, which is where
/// every other multi-use charge here has landed.
pub const HOMING_STRIKES_TAG: &str = "rogue.homing_strikes";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet as Set;

    /// The two per-rest registries don't repeat themselves and don't
    /// overlap.
    ///
    /// `SHORT_REST_FEATURES` carried a comment promising that a test
    /// kept it honest. There wasn't one, and in its absence the sibling
    /// registry's docs had drifted into claiming the Battle Master
    /// maneuvers were listed in both — they aren't, and shouldn't be,
    /// because `ActorInstance::short_rest` chains the two. Writing the
    /// test is what surfaced the drift.
    ///
    /// What is *not* checked here, and why: a coverage assertion
    /// ("every registered tag is carried by some template") would be
    /// the more valuable invariant, and it can't be written honestly
    /// today. `pc_template_families` is the engine's only template
    /// registry and it holds class builds only, so a racial feature
    /// like the Dragonborn's Breath Weapon reads as an orphan against
    /// it. The gap is in the registry, not in the tag list.
    #[test]
    fn the_short_rest_registry_matches_the_templates_that_use_it() {
        let short_rest: Set<&str> = SHORT_REST_FEATURES.iter().copied().collect();
        assert_eq!(
            short_rest.len(),
            SHORT_REST_FEATURES.len(),
            "SHORT_REST_FEATURES lists some tag more than once"
        );
        let maneuvers: Set<&str> = BATTLE_MASTER_MANEUVERS.iter().copied().collect();
        assert_eq!(
            maneuvers.len(),
            BATTLE_MASTER_MANEUVERS.len(),
            "BATTLE_MASTER_MANEUVERS lists some tag more than once"
        );
        let both: Vec<&&str> = short_rest.intersection(&maneuvers).collect();
        assert!(
            both.is_empty(),
            "short_rest chains both registries, so {:?} would refresh twice",
            both
        );
    }
}
