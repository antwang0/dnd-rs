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
        saves::SaveDamagePolicy,
        side_effects::{
            ApplicableSideEffect, ApplyCondition, DealDamage, GainTempHp, GiveResource, Heal,
            RemoveCondition, Resource,
        },
        types::{AbilityScoreType, Coordinate, DamageType},
    },
};

/// Class-feature tags that refresh on a 5e short rest. Read by
/// `ActorInstance::short_rest`, which walks this registry and
/// repopulates `features_remaining` for every matching tag the actor
/// carries. The remaining tags in this module are long-rest features and
/// only restore via `long_rest`.
///
/// The Battle Master maneuvers are absent on purpose and are *not* an
/// omission: they spend from a shared counter (`SUPERIORITY_DICE_TAG`,
/// which is on this list), so refilling their individual — and unread —
/// per-tag charges here would be dead work. See `SHARED_FEATURE_POOLS`.
/// `the_short_rest_registry_matches_the_templates_that_use_it` pins the
/// disjointness and checks that every row is actually carried by some
/// registered PC template — a registry entry whose feature has moved on
/// is invisible otherwise.
pub const SHORT_REST_FEATURES: &[&str] = &[
    // 5e Artificer, all five charges. RAW prices four of them off pools
    // this engine does not track (Arcane Armor uses, the Alchemist's
    // flask, the Artillerist's cannon uses, the Battle Smith's hour of
    // rebuilding) and the fifth — Flash of Genius — off the artificer's
    // Intelligence modifier per long rest. Collapsing all five to one
    // charge per short rest is the same translation every Channel
    // Divinity on this list already carries: one press per engagement.
    FLASH_OF_GENIUS_TAG,
    DEFENSIVE_FIELD_TAG,
    EXPERIMENTAL_ELIXIR_TAG,
    ELDRITCH_CANNON_TAG,
    STEEL_DEFENDER_TAG,
    SECOND_WIND_TAG,
    ACTION_SURGE_TAG,
    // 5e Fighter Battle Master: "You regain all of your expended
    // superiority dice when you finish a short or long rest." One entry
    // refills the whole maneuver suite, because one counter backs it —
    // see `SHARED_FEATURE_POOLS`.
    SUPERIORITY_DICE_TAG,
    // 5e Channel Divinity, both classes: "you must finish a short or
    // long rest to use your Channel Divinity again." One row per class
    // rather than one per option, because one counter backs each — see
    // `SHARED_FEATURE_POOLS`. The nineteen individual options used to be
    // listed here, which was nineteen chances to forget a tag and three
    // taken.
    CLERIC_CHANNEL_DIVINITY_TAG,
    PALADIN_CHANNEL_DIVINITY_TAG,
    ARCANE_RECOVERY_TAG,
    // 5e Peace Domain Cleric — Emboldening Bond. RAW's pool refreshes
    // on a long rest; it sits on the short-rest cadence here for the
    // same reason Warding Flare does, which is that a cleric who loses
    // their level-1 domain feature between engagements has lost the
    // domain.
    EMBOLDENING_BOND_TAG,
    // 5e Clockwork Soul Sorcerer, both charges. RAW refreshes each on a
    // long rest; the short-rest cadence here matches every other
    // subclass charge on the roster, so a sorcerer arrives at the next
    // engagement with a subclass.
    RESTORE_BALANCE_TAG,
    BASTION_OF_LAW_TAG,
    NATURAL_RECOVERY_TAG,
    // 5e Circle of Stars Druid — Starry Form. RAW spends a Wild Shape
    // use, and Wild Shape recovers on a short rest, so the charge
    // belongs on this cadence next to its Land-circle sibling above.
    STARRY_FORM_TAG,
    // 5e Arcane Archer Fighter — Arcane Shot. RAW: "You regain all
    // expended uses of it when you finish a short or long rest." Both
    // charges come back, which is what the pool-sized refill in
    // `short_rest` is for.
    ARCANE_SHOT_TAG,
    CUTTING_WORDS_TAG,
    // 5e College of Eloquence Bard lv3 — Unsettling Words. RAW spends a
    // Bardic Inspiration use, and the bard's inspiration pool itself
    // refreshes on a short rest from level 5 (Font of Inspiration), so
    // the charge belongs on the same cadence as its sibling quip on the
    // Lore chassis directly above.
    UNSETTLING_WORDS_TAG,
    // 5e College of Glamour Bard lv3 — Enthralling Performance. RAW:
    // "once you use this feature, you can't use it again until you
    // finish a short or long rest", the same cadence as every other
    // burst on the `TurnBurst` chassis.
    ENTHRALLING_PERFORMANCE_TAG,
    BREATH_WEAPON_TAG,
    // 5e Oath of the Crown Paladin Channel Divinity — Champion Challenge
    // and Turn the Tide. Same short-rest cadence as every other Channel
    // Divinity on the roster.
    // 5e War Domain Cleric features — both refresh on a short rest.
    // WAR_PRIEST is the once-per-rest bonus-action extra swing; GUIDED
    // STRIKE is the Channel Divinity +10 accuracy prime.
    WAR_PRIEST_TAG,
    // 5e Light Domain Cleric Channel Divinity — Radiance of the Dawn:
    // once-per-short-rest 30ft radiant burst. Shares the RAW "Channel
    // Divinity" resource lane with Turn Undead / Preserve Life / Guided
    // Strike, but each tag is a distinct per-rest charge in our model.
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
    // 5e Oath of Redemption Paladin level-3 Channel Divinity — Rebuke
    // the Violent. Single-target 4d10 radiant damage burst via WIS save
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
    // 5e Nature Domain Cleric **Dampen Elements** — RAW costs only the
    // reaction with no per-rest cap; the engine's single charge lands on
    // the short-rest cadence the rest of the reactive per-rest family
    // (Parry, Warding Flare, Warding Maneuver, Protective Field) shares.
    DAMPEN_ELEMENTS_TAG,
    // 5e Nature Domain Cleric Channel Divinity — Charm Animals and
    // Plants. RAW Channel Divinity is once per short rest.
    // 5e Trickery Domain Cleric Channel Divinity — Invoke Duplicity.
    // RAW Channel Divinity is once per short rest, the same cadence as
    // every sibling CD charge above.
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
    // 5e Order Domain Cleric Channel Divinity — Order's Demand. Same
    // short-rest cadence as the rest of the cleric CD family.
    // 5e Soulknife Rogue **Homing Strikes**. RAW recovers Psionic Energy
    // dice on a short rest, which is the cadence this registry carries.
    HOMING_STRIKES_TAG,
    // 5e Circle of Wildfire Druid **Summon Wildfire Spirit**. RAW spends
    // a Wild Shape use, which recovers on a short rest — the same
    // reasoning that puts the Stars Druid's Starry Form on this lane.
    SUMMON_WILDFIRE_SPIRIT_TAG,
    // 5e Fathomless Warlock — Tentacle of the Deep and Guardian Coil.
    // Everything a warlock owns comes back on a short rest; RAW says so
    // of Pact Magic and of both of these by name.
    TENTACLE_OF_THE_DEEP_TAG,
    GUARDIAN_COIL_TAG,
    // 5e Circle of the Shepherd Druid — the Spirit Totem call. RAW
    // spends a Wild Shape use, and Wild Shape recovers on a short rest,
    // so the charge belongs on this cadence next to its Wildfire-circle
    // sibling.
    SPIRIT_TOTEM_TAG,
    // 5e Circle of Dreams Druid, both presses. RAW refills the balm's
    // die pool and the blink's uses on a long rest; the short-rest
    // cadence matches every other subclass charge on the roster, so a
    // Dreams druid arrives at the next engagement with a subclass.
    BALM_OF_THE_SUMMER_COURT_TAG,
    HIDDEN_PATHS_TAG,
    // 5e Drakewarden Ranger — the summon charge. RAW's drake is a
    // permanent bond re-summoned with a spell slot or an hour's ritual;
    // the short-rest cadence here is the same translation the tentacle
    // and the wildfire spirit already carry, and it is what keeps a
    // Drakewarden from arriving at the second fight of the day without
    // the half of the subclass that fights.
    DRAKE_COMPANION_TAG,
    // The three Channel Divinities that were missing from this lane —
    // see `CHANNEL_DIVINITY_FEATURES`. Cleric **Turn Undead** is the
    // odd one out in the most literal sense: five comments in this
    // file cite it as the example of the short-rest cleric CD cadence,
    // and it was the only cleric CD not on it. Paladin **Sacred
    // Weapon** and **Vow of Enmity** are the Devotion and Vengeance
    // oaths' Channel Divinities, sitting beside six other oaths' CDs
    // that were already here.
    //
    // All three ride the baseline templates rather than a subclass
    // clone, so the cost of the omission landed on the plain Cleric and
    // the plain Paladin — the two chassis most likely to be picked, and
    // the two whose Channel Divinity is the whole non-spell half of
    // their turn.
    // 5e Monk **ki points**: "when you spend a ki point, it is
    // unavailable until you finish a short or long rest." One row
    // refills every feature that spends from the pool — Stunning
    // Strike, Empty Body, Searing Sunburst and Drunkard's Luck — for
    // the same reason one row refills the whole maneuver suite. Two of
    // those four used to sit on this list individually and two were
    // long-rest-only, which is what a private charge per feature buys
    // you: four features, three cadences, none of them RAW's.
    KI_POINTS_TAG,
];

/// Every **Channel Divinity** on the roster: the once-per-rest resource
/// a Cleric or Paladin spends on their domain's or oath's signature
/// effect.
///
/// RAW is unambiguous and identical for both classes — "you must finish
/// a short or long rest to use your Channel Divinity again" — so this
/// this has exactly one rule, and
/// `every_channel_divinity_comes_back_on_a_short_rest` enforces it.
///
/// It exists because the cadence was being applied one tag at a time by
/// whoever added the feature, and three of the eleven had been missed:
/// the Cleric's Turn Undead and the Paladin's Sacred Weapon and Vow of
/// Enmity all refreshed on a long rest only. Nothing detected it,
/// because the charge lane has no notion of which features are supposed
/// to share a cadence — it is a flat list of tags, and a tag that isn't
/// in it simply waits for the long rest.
///
/// Split by class because the *pool* is per class even though the
/// cadence isn't: see `CLERIC_CHANNEL_DIVINITY` and
/// `PALADIN_CHANNEL_DIVINITY`, which each hold their own count and draw
/// from their own half of this list. The whole list is what the cadence
/// rule reads.
pub fn channel_divinity_features() -> impl Iterator<Item = &'static str> {
    CLERIC_CHANNEL_DIVINITIES
        .into_iter()
        .chain(PALADIN_CHANNEL_DIVINITIES)
}

/// The Cleric's Channel Divinity options — the membership list of
/// `CLERIC_CHANNEL_DIVINITY`.
///
/// A cleric knows Turn Undead plus whatever their domain grants, and RAW
/// spends both out of one pool. Modeling each as its own charge, which
/// is what this engine did until the pool lane existed, handed a War
/// Cleric three presses of a two-press feature and a Light Cleric four —
/// the same over-count the Battle Master's maneuvers had, one class
/// over. The comment here used to say so and call it "a deliberate
/// simplification".
pub const CLERIC_CHANNEL_DIVINITIES: [&str; 11] = [
    TURN_UNDEAD_TAG,
    PRESERVE_LIFE_TAG,
    GUIDED_STRIKE_TAG,
    RADIANCE_OF_THE_DAWN_TAG,
    INVOKE_DUPLICITY_TAG,
    PATH_TO_THE_GRAVE_TAG,
    ARCANE_ABJURATION_TAG,
    CHARM_ANIMALS_AND_PLANTS_TAG,
    REAPERS_TOUCH_TAG,
    ORDERS_DEMAND_TAG,
    BALM_OF_PEACE_TAG,
];

/// The Paladin's Channel Divinity options — the membership list of
/// `PALADIN_CHANNEL_DIVINITY`.
///
/// Separate from the cleric's because the two classes hand out different
/// numbers of presses, not because the rule differs: RAW a paladin has
/// exactly one Channel Divinity use per rest at every level this
/// engine's chassis represent, where a cleric of the same tier has two.
/// One pool per class keeps that difference in the one place a pool
/// records anything — its size.
pub const PALADIN_CHANNEL_DIVINITIES: [&str; 9] = [
    SACRED_WEAPON_TAG,
    VOW_OF_ENMITY_TAG,
    ABJURE_ENEMY_TAG,
    DREADFUL_ASPECT_TAG,
    NATURES_WRATH_TAG,
    TURN_THE_FAITHLESS_TAG,
    CONQUERING_PRESENCE_TAG,
    CHAMPION_CHALLENGE_TAG,
    TURN_THE_TIDE_TAG,
];

/// The Cleric's Channel Divinity pool. RAW: one use at cleric level 2,
/// two from level 6, three from 18. The cleric templates on the roster
/// sit at level 5-9, so two is the number for the chassis they are.
pub const CLERIC_CHANNEL_DIVINITY_TAG: &str = "cleric.channel_divinity";

/// The Paladin's Channel Divinity pool. RAW: one use, regained on a
/// short or long rest, at every level below 18.
pub const PALADIN_CHANNEL_DIVINITY_TAG: &str = "paladin.channel_divinity";

/// Battle Master maneuver tags — the membership list of the superiority
/// dice pool.
///
/// RAW, a Battle Master does not have sixteen independent per-rest
/// charges; it has **one pool of superiority dice** and every maneuver
/// spends from it. This list used to be exactly that collapse — a tag
/// per maneuver, one charge each, refreshed side-by-side with
/// `SHORT_REST_FEATURES` — which meant a level-3 fighter could fire
/// every maneuver in a single fight and still walk into the next one
/// with a full sheet. That is four times the resource RAW hands out, on
/// the class whose entire identity is spending it well.
///
/// The list now names the *members of a shared pool* rather than
/// sixteen separate resources: `SHARED_FEATURE_POOLS` points every tag
/// here at `SUPERIORITY_DICE_TAG`, and `ActorInstance::spend_feature` /
/// `feature_charges_remaining` redirect the accounting there. Each
/// maneuver still needs its own tag — the template says *which*
/// maneuvers the fighter knows, and the AI gates each option on the tag
/// it is about to spend — but the count behind all of them is one
/// number.
///
/// Deliberately *not* duplicated into `SHORT_REST_FEATURES`: the pool
/// tag is what refreshes, and refilling the (now unread) per-maneuver
/// counters alongside it would be dead work plus a second place to
/// forget a maneuver.
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
    MANEUVERING_ATTACK_TAG,
    // Reaction maneuvers — both fire automatically on an incoming melee
    // swing (no active action to spend on the fighter's turn), gated on
    // a superiority die and the holder's reaction slot. RAW spends a die
    // for each exactly like the bonus-action primes above, so they draw
    // from the same pool: a fighter who burned the pool on Trip and
    // Menacing has nothing left to Parry with, which is the trade the
    // pool exists to force.
    PARRY_TAG,
    RIPOSTE_TAG,
    // Neither an action nor a reaction: Evasive Footwork's trigger is
    // the fighter's own movement, and it fires from the
    // opportunity-attack dispatcher. It spends from this pool all the
    // same, which is the point — a fighter who walked out of three
    // reach envelopes on the way to the caster arrives with one fewer
    // die to Trip with.
    EVASIVE_FOOTWORK_TAG,
];

/// The Fighter Battle Master's **superiority dice** pool.
///
/// RAW (PHB, Martial Archetype: Battle Master, level 3): "You have four
/// superiority dice, which are d8s. A superiority die is expended when
/// you use it. You regain all of your expended superiority dice when you
/// finish a short or long rest."
///
/// Unlike every other tag in this file, this one names no action of its
/// own. It is a *counter* that the sixteen tags in
/// `BATTLE_MASTER_MANEUVERS` share: a template carries it alongside the
/// maneuvers it knows, `FEATURE_CHARGES` sizes it at four, and
/// `SHORT_REST_FEATURES` refills it. Nothing looks it up by name at an
/// action site — the redirect in `ActorInstance` does that.
pub const SUPERIORITY_DICE_TAG: &str = "fighter.superiority_dice";

/// The die a spent superiority die rolls, RAW a d8 at every Battle
/// Master level this engine's chassis represent (it grows to d10 at
/// fighter 10 and d12 at 18).
///
/// Named rather than written `Dice::new(1, 8)` at each of the seven
/// maneuver rows that roll it, because "the superiority die" is one
/// quantity in the rules and seven copies of a literal is seven places
/// for a future d10 to be half-applied.
pub const SUPERIORITY_DIE: Dice = Dice::new(1, 8);

/// The Monk's **ki points** pool.
///
/// RAW (PHB, Monk, level 2): "your training allows you to harness the
/// mystic energy of ki. Your access to this energy is represented by a
/// number of ki points. Your monk level determines the number of points
/// you have… When you spend a ki point, it is unavailable until you
/// finish a short or long rest."
///
/// Nine doc comments in this file and in `creatures::monks` open with
/// some version of "the engine has no ki pool", and each of them then
/// makes a different local compromise: Flurry of Blows and Patient
/// Defense drop the cost entirely and lean on the bonus action, Shadow
/// Arts and the Four Elements disciplines borrow the spell-slot table,
/// and the four features below each took a private once-per-rest charge.
/// That last group is the one the compromise cost the most, because a
/// private charge is not a smaller pool — it is a *separate* one. A
/// level-5 monk who had spent Stunning Strike still had a full Empty
/// Body waiting, and neither of them came back on a short rest.
///
/// This is the pool, sized at 5 for the level-5 chassis every monk
/// template on the roster represents (RAW: ki points = monk level), and
/// refreshed on a short rest exactly as RAW says. It is a counter and
/// names no action of its own — the same shape as the Battle Master's
/// superiority dice and the two Channel Divinity pools above it.
///
/// **What it does not cover, and why.** The engine's ki compromises are
/// not all the same compromise, and only one kind of them can be undone
/// by a pool:
///
///   - Flurry of Blows, Patient Defense, Step of the Wind, Deft Strike,
///     Hand of Harm and Hand of Healing all trade their ki cost for the
///     monk's bonus action, which is the chassis's genuinely scarce
///     resource — a monk who flurries has not dodged. Pricing them in ki
///     *as well* would be charging twice for one turn.
///   - Shadow Arts and the Four Elements disciplines are spells, and
///     spend from a slot table that is already RAW's ki cost read by
///     tier. See `FOUR_ELEMENTS_MONK_TEMPLATE`.
///
/// What is left is the group RAW prices in ki and the engine gates with
/// a charge, which is what `MONK_KI_FEATURES` lists.
pub const KI_POINTS_TAG: &str = "monk.ki_points";

/// The features that spend from `KI_POINTS_TAG` — every monk feature
/// RAW prices in ki points *and* this engine gates behind a per-rest
/// charge.
///
/// Four rows, and RAW's own prices are 1, 4, 2 and 2 ki respectively.
/// The pool spends one charge per use rather than the RAW price,
/// because the charge lane counts uses rather than points — so a monk
/// gets five presses spread across the four features instead of RAW's
/// five points spread across the same four. The direction of the
/// approximation is deliberate and it is the cheap one: what the RAW
/// prices buy is the *ordering* decision (a Searing Sunburst costs two
/// Stunning Strikes), and what the private charges cost was the
/// existence of a decision at all.
///
/// Wholeness of Body is deliberately absent. It is the one charge on the
/// monk chassis RAW does not price in ki — "you can't use this feature
/// again until you finish a long rest" — so it keeps the private
/// long-rest charge it already had.
pub const MONK_KI_FEATURES: [&str; 6] = [
    // RAW 1 ki. The one the pool changes most: a monk had exactly one
    // stun per long rest, and now has as many as they are willing to
    // spend the pool on.
    STUNNING_STRIKE_TAG,
    // RAW 4 ki, and RAW's most expensive monk press — which is why it
    // is the row that most wants a shared pool rather than a private
    // charge: spending it should empty the monk, and now it does.
    EMPTY_BODY_TAG,
    // RAW 2 ki (plus up to 3 more for extra dice, which the fixed 2d6
    // burst here doesn't offer).
    SEARING_SUNBURST_TAG,
    // RAW 2 ki.
    DRUNKARDS_LUCK_TAG,
    // RAW's Breath of the Dragon is free a proficiency-bonus number of
    // times per long rest and 1 ki after that. Two cadences for one
    // button is a distinction the charge lane cannot draw, and of the
    // two the ki price is the one that makes the breath compete with
    // the rest of the kit — so the pool is where it lands, and the
    // free presses go.
    BREATH_OF_THE_DRAGON_TAG,
    // RAW 3 ki (or once per long rest for free — same collapse as the
    // breath above). The most expensive press the Ascendant Dragon
    // has, and the pool is what makes that cost legible: a monk who
    // frightens the room has two breaths left instead of five.
    ASPECT_OF_THE_WYRM_TAG,
];

/// Feature tags whose charges are drawn from a **shared pool** rather
/// than from a counter of their own, as `(pool tag, member tags)`.
///
/// The per-tag charge lane in `ActorInstance` answers "how many times
/// can this actor use *this* feature", which is the right question for
/// almost every feature in the book: Action Surge and Second Wind are
/// genuinely separate resources. A handful of features aren't. The
/// Battle Master's maneuvers are the canonical case — sixteen distinct
/// abilities priced out of one pool of four dice — and modeling them as
/// sixteen independent charges was not a rounding error but a
/// quadrupling of the subclass's whole resource budget.
///
/// The redirect is deliberately **opt-in per actor**: a member tag only
/// reads the pool if its holder also carries the pool tag. A template
/// that picks up a maneuver without the pool keeps the old one-charge-
/// per-tag behavior rather than silently reading someone else's empty
/// counter and finding the feature unusable. `every_pool_member_ships_with_its_pool`
/// makes that omission a test failure rather than a balance surprise.
pub const SHARED_FEATURE_POOLS: &[(&str, &[&str])] = &[
    (SUPERIORITY_DICE_TAG, BATTLE_MASTER_MANEUVERS),
    // Channel Divinity is the same story one class over, and the reason
    // the pool lane is a general mechanism rather than a Battle Master
    // special case. A cleric spends Turn Undead and their domain's
    // option out of one pool; modeling them as separate charges gave a
    // War Cleric three presses of a two-press feature.
    (CLERIC_CHANNEL_DIVINITY_TAG, &CLERIC_CHANNEL_DIVINITIES),
    (PALADIN_CHANNEL_DIVINITY_TAG, &PALADIN_CHANNEL_DIVINITIES),
    // And the same story a third time, on the class whose whole
    // resource *is* a pool. See `KI_POINTS_TAG` for what the four
    // private charges cost before this row existed, and for the ki
    // compromises a pool cannot undo.
    (KI_POINTS_TAG, &MONK_KI_FEATURES),
];

/// The shared pool `tag` spends from, or `None` if it has a counter of
/// its own.
///
/// A nested linear scan over a table with one row and sixteen members;
/// it runs on the charge-check path, which is hot enough to notice a
/// hash but nowhere near hot enough to notice sixteen pointer
/// comparisons.
pub fn shared_pool_for(tag: &str) -> Option<&'static str> {
    SHARED_FEATURE_POOLS
        .iter()
        .find(|(_, members)| members.contains(&tag))
        .map(|(pool, _)| *pool)
}

/// Features whose RAW resource is a *pool* rather than a single use,
/// and how many charges that pool holds here. Read once per actor at
/// instantiation by `ActorInstance::from_template`, which seeds both
/// `features_max` and `features_remaining` from it; every tag absent
/// from this table gets exactly one charge, which is what every
/// feature in the engine had before the table existed.
///
/// The charge lane used to be a `HashSet<&'static str>` — a tag was
/// either spent or it wasn't — and half a dozen doc comments in this
/// file apologized for it. Psionic Strike said so outright: "the
/// engine's per-rest charge lane is binary — one `features_remaining`
/// entry per tag, not a counter — so a pool-accurate Psionic Strike
/// would fire once per short rest rather than the four-plus times RAW
/// allows". The set is a count now, and this table is where a feature
/// says how big its pool is.
///
/// **A feature has to earn a row here.** Widening a pool changes what a
/// chassis can do in a fight, and a template balanced around one Action
/// Surge is not the same template with two. Two questions have to answer
/// yes: is RAW's number exact for the level this chassis is built at,
/// and is a second charge something the holder could actually spend
/// inside one fight? The second is what keeps the minute-long self-buffs
/// off the table — Starry Form, Bladesong and Giant's Might all have
/// RAW pools of two or more, and all three would spend the second charge
/// refreshing a timer that had eight rounds left on it.
pub const FEATURE_CHARGES: &[(&str, u32)] = &[
    // 5e Arcane Archer Fighter (XGE, subclass level 3): "You can use
    // this feature twice. You regain all expended uses of it when you
    // finish a short or long rest." RAW to the number.
    (ARCANE_SHOT_TAG, 2),
    // 5e Peace Domain Cleric **Emboldening Bond**: "you can use this
    // feature a number of times equal to your proficiency bonus." The
    // cleric chassis is level 9, proficiency +4 — but two is the number
    // here rather than four, for the reason the table's own preamble
    // gives: a second bond is genuinely spendable inside one fight
    // (allies die, allies arrive, the ten rounds lapse) and a fourth is
    // not. Two is the smallest pool that makes the charge a decision.
    (EMBOLDENING_BOND_TAG, 2),
    // 5e Clockwork Soul Sorcerer **Restore Balance**: RAW's pool is the
    // proficiency bonus, four on this chassis. Two, for the reason the
    // preamble above gives — and because the reaction it spends is one
    // per round regardless, so the fourth charge is a charge the
    // sorcerer would rarely reach in a single fight.
    (RESTORE_BALANCE_TAG, 2),
    // 5e Bard **Bardic Inspiration**: uses equal to the bard's Charisma
    // modifier. Every bard template on the roster carries CHA 16, so
    // three is not a compromise here — it is the number. The bard is
    // the class whose whole character is the pool, and a bard with one
    // die to hand out for the whole fight was the collapse that cost
    // the most.
    (BARDIC_INSPIRATION_TAG, 3),
    // 5e Fighter Battle Master **Superiority Dice**: "You have four
    // superiority dice." RAW to the number, and the number is the whole
    // subclass — every maneuver on the chassis is priced against it.
    (SUPERIORITY_DICE_TAG, 4),
    // 5e Cleric **Channel Divinity**: one use at level 2, two from level
    // 6. The cleric templates sit at level 5-9, and two is what makes
    // the domains playable — a Light Cleric with one press has to choose
    // between turning the undead in front of it and the Radiance of the
    // Dawn that is the domain's whole tell.
    (CLERIC_CHANNEL_DIVINITY_TAG, 2),
    // 5e Circle of the Moon Druid **Combat Wild Shape**: two uses per
    // short rest, and RAW is explicit about the count in a way most
    // pools are not. Two is also what makes the Moon druid's second
    // half work — the form ends when its temp HP drains, and a druid
    // who could only take it once was a druid who spent the rest of
    // the fight as a caster who had given away its concentration.
    (COMBAT_WILD_SHAPE_TAG, 2),
    // 5e Monk **ki points**: "your monk level determines the number of
    // points you have." Five, for the level-5 chassis every monk
    // template on the roster is built to — and the first number in this
    // table that is read as a budget across several features rather
    // than as one feature's depth. See `KI_POINTS_TAG`.
    (KI_POINTS_TAG, 5),
    // SRD 5.2 Androsphinx **Roar (3/Day)** — RAW to the number, and
    // the only row on this table whose count is read for something
    // other than "may I". See `ROAR_TAG`.
    (ROAR_TAG, ROAR_USES),
    // SRD 5.2 Goliath **Giant Ancestry**, all six benefits: "you can
    // use the chosen benefit a number of times equal to your
    // Proficiency Bonus." Two is that number on the CR-2 chassis these
    // templates are built to, so these six rows are RAW rather than the
    // "collapse a scaling pool to something spendable" compromise most
    // of this table is making.
    //
    // Sized once, at `species::GIANT_ANCESTRY_USES`, because the six
    // are one trait with six faces — a pool that differed between them
    // would be a typo rather than a decision.
    (
        crate::actions::species::CLOUDS_JAUNT_TAG,
        crate::actions::species::GIANT_ANCESTRY_USES,
    ),
    (
        crate::actions::species::FIRES_BURN_TAG,
        crate::actions::species::GIANT_ANCESTRY_USES,
    ),
    (
        crate::actions::species::FROSTS_CHILL_TAG,
        crate::actions::species::GIANT_ANCESTRY_USES,
    ),
    (
        crate::actions::species::HILLS_TUMBLE_TAG,
        crate::actions::species::GIANT_ANCESTRY_USES,
    ),
    (
        crate::actions::species::STONES_ENDURANCE_TAG,
        crate::actions::species::GIANT_ANCESTRY_USES,
    ),
    (
        crate::actions::species::STORMS_THUNDER_TAG,
        crate::actions::species::GIANT_ANCESTRY_USES,
    ),
];

/// How many charges `tag` starts a rest with. One unless
/// `FEATURE_CHARGES` says otherwise.
///
/// A linear scan of a table this size costs less than the hash it
/// would replace, and it runs once per feature per actor creation
/// rather than per use.
pub fn feature_charges(tag: &str) -> u32 {
    FEATURE_CHARGES
        .iter()
        .find(|(name, _)| *name == tag)
        .map(|(_, count)| *count)
        .unwrap_or(1)
}

/// Tags used by `ActorInstance::feature_available` / `spend_feature` to
/// gate once-per-long-rest class features. Stored as `&'static str` so
/// actor state stays a flat map instead of carrying an enum import.
pub const SECOND_WIND_TAG: &str = "fighter.second_wind";
pub const ACTION_SURGE_TAG: &str = "fighter.action_surge";

/// Per-day pool for the androsphinx's **Roar** — SRD 5.2's *"Roar
/// (3/Day) … whenever it roars, the roar has a different effect …
/// (the sequence resets when it takes a Long Rest)."*
///
/// The pool and the sequence are the same number, which is why there
/// is only one of them: `RoarStage::from_remaining` reads which roar
/// this is off what is left to spend, so the escalation and the
/// resource cannot drift apart and a long rest resets both in the one
/// place RAW resets them.
///
/// Sits in this module rather than beside the action for the same
/// reason every other charge does: `FEATURE_CHARGES` is where a pool's
/// size is declared, and a tag declared anywhere else would be a
/// second place to look.
pub const ROAR_TAG: &str = "androsphinx.roar";

/// How many roars a day. RAW to the number, and the number is the
/// whole ability — a fourth would have no effect to be.
pub const ROAR_USES: u32 = 3;

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

/// 5e Rogue Assassin **Assassinate** (level 3 subclass) feature tag.
///
/// Both RAW halves ship, and they are read in two different places
/// because they are two different questions:
///
///   - **Advantage** against a creature that hasn't taken a turn yet in
///     this combat. Read at `EncounterInstance::attack_mode_tally` next
///     to the Pack Tactics / Wolf Totem advantage clauses, off the
///     target's `has_taken_turn_in_combat` latch, which is set on the
///     first turn-start (see `start_turn_for`).
///   - **Auto-crit** on any hit against a *surprised* creature. Read at
///     `EncounterInstance::target_grants_auto_crit`, off
///     `Condition::Surprised`.
///
/// This docstring used to say the second half was unimplementable
/// because "we don't model surprise as a discrete state", which stopped
/// being true when `resolve_opening_surprise` and `Condition::Surprised`
/// arrived. The clause was wired at the same time; only the excuse was
/// left behind.
///
/// The two are deliberately not folded together: a creature can be
/// surprised without being first in the order, and can be last in the
/// order without ever having been surprised.
///
/// Stored as a `has_passive_feature` flag so it never consumes a
/// per-rest charge — both gates are purely target-side.
pub const ASSASSINATE_TAG: &str = "rogue.assassinate";

/// 5e **Shrieker Fungus** — *Shriek* (Reaction): "*Trigger:* A creature
/// or a source of Bright Light moves within 30 feet of the shrieker."
///
/// A monster trait rather than a class feature, and it lives on this
/// list anyway for the same reason Assassinate does: what the engine
/// needs is a flag that says "this creature answers this trigger", and
/// `features` is where a template puts one.
///
/// Stored as a `has_passive_feature` flag with no charges. RAW's ration
/// is not per-rest but per-fight — a shriek lasts a minute, which
/// outlasts any encounter — and that is enforced by
/// `Condition::Shrieking` at the trigger site rather than by a charge
/// pool. See `EncounterInstance::dispatch_shrieks`.
pub const SHRIEKER_TAG: &str = "monster.shrieker";

/// One of the three things RAW lets a bonus action buy in place of a
/// full Action: cover ground, leave without being hit, or disappear.
///
/// A closed set, and it stays closed on purpose. Every bonus-action
/// mobility feature in the book — the Rogue's Cunning Action, the
/// goblin's Nimble Escape, the Monk's Step of the Wind, the Ranger's
/// Vanish, the vampire familiar's Deathless Agility — is spelled out
/// as some combination of exactly these three, because they are the
/// three *Actions* the feature is standing in for. A feature that did
/// something else would not be a cheaper printing of an Action; it
/// would be a new one, and it belongs in its own impl.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Maneuver {
    /// Extra movement equal to the creature's speed.
    Dash,
    /// Movement this turn doesn't provoke opportunity attacks.
    Disengage,
    /// A Dexterity (Stealth) check against the best passive Perception
    /// watching, for the `Hidden` condition on a pass.
    Hide,
}

/// The chassis every bonus-action mobility feature is built on.
///
/// There used to be six of these, hand-written, and the diff between
/// any two of them was the name and the aliases. `CunningHide`,
/// `NimbleHide` and `Vanish` were the same forty lines three times over
/// — same cost, same gate, same payload, different string — and the
/// cost of that was not the duplication but what the duplication hid:
/// when the bonus-action Hide was found to be installing `Hidden` with
/// no roll while the Action-priced Hide rolled for it, the fix had to
/// be applied at three sites, and one of them nearly got missed.
///
/// So the payload moved into data. `maneuvers` is a slice rather than
/// one variant because RAW's features are not all "or": Cunning Action
/// offers a choice between three (which is three statics, since the
/// choice is made at the action list), but Step of the Wind is Dash
/// **and** Disengage in one activation, which is one static holding
/// two. The slice is what makes both shapes expressible without a
/// second chassis.
///
/// `gated_on` carries the one thing that genuinely differed: the
/// Ranger's Vanish is a level-14 feature and checks a passive tag
/// before it fires, so a template that picked up the action without
/// the feature can't use it. Everything else passes `None`.
///
/// Hide is deliberately never combined with the other two. Hiding has
/// a precondition — you cannot hide from something standing next to
/// you — and a combined Dash-and-Hide would have to decide whether the
/// Dash half survives a failed hideability check. RAW never asks, so
/// the chassis doesn't answer; `debug_assert` in the constructor keeps
/// it that way.
pub struct BonusManeuver {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    /// Everything one activation buys, in the order RAW prints it.
    pub maneuvers: &'static [Maneuver],
    /// A passive-feature tag the actor must carry, or `None` for the
    /// features that are available to anything holding the action.
    pub gated_on: Option<&'static str>,
}

impl BonusManeuver {
    pub const fn new(
        display_name: &'static str,
        aliases: &'static [&'static str],
        maneuvers: &'static [Maneuver],
    ) -> Self {
        Self {
            display_name,
            aliases,
            maneuvers,
            gated_on: None,
        }
    }

    /// Builder tail for the one printing that is a class feature rather
    /// than a universally-available action — the Ranger's Vanish.
    pub const fn gated_on(self, tag: &'static str) -> Self {
        Self {
            gated_on: Some(tag),
            ..self
        }
    }

    fn does(&self, m: Maneuver) -> bool {
        // `contains` isn't const-callable on a slice of a non-const-Eq
        // type, and the slice is three entries long at most.
        let mut i = 0;
        while i < self.maneuvers.len() {
            if self.maneuvers[i] as u8 == m as u8 {
                return true;
            }
            i += 1;
        }
        false
    }
}

impl Action for BonusManeuver {
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
        if let Some(tag) = self.gated_on
            && !encounter
                .actors
                .get(&caster_id)
                .is_some_and(|a| a.has_passive_feature(tag))
        {
            return false;
        }
        // Every printing of Hide reads the same clause: there has to
        // be somewhere to hide — SRD 5.2's Heavily Obscured or
        // three-quarters cover, out of every enemy's line of sight.
        // The other two maneuvers have no precondition.
        !self.does(Maneuver::Hide)
            || crate::actions::default_actions::can_attempt_hide(encounter, caster_id)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for m in self.maneuvers {
            match m {
                Maneuver::Dash => {
                    let speed = encounter.travel_speed(caster_id);
                    encounter.log(format!("  {}: extra movement gained.", self.display_name));
                    effects.push(Box::new(GiveResource {
                        actor_id: caster_id,
                        resource: Resource::Movement(speed),
                    }));
                }
                Maneuver::Disengage => {
                    effects.push(Box::new(crate::engine::side_effects::SetDisengaging {
                        actor_id: caster_id,
                        disengaging: true,
                    }));
                }
                // Through the shared helper, so the roll a bonus-action
                // Hide makes is the roll the Action-priced Hide makes.
                Maneuver::Hide => effects.extend(bonus_action_hide_effects(encounter, caster_id)),
            }
        }
        effects
    }
}

/// Rogue **Cunning Action** — bonus-action Dash. RAW offers the rogue a
/// choice of Dash, Disengage or Hide; the choice lives in the action
/// list, so it is these three statics rather than one action that asks.
pub static CUNNING_DASH: BonusManeuver =
    BonusManeuver::new("cunning dash", &["cdash", "ca-dash"], &[Maneuver::Dash]);

/// Rogue Cunning Action, the Disengage pick — movement this turn
/// doesn't provoke, at a bonus action instead of an Action.
pub static CUNNING_DISENGAGE: BonusManeuver = BonusManeuver::new(
    "cunning disengage",
    &["cdis", "ca-dis"],
    &[Maneuver::Disengage],
);

/// Shared side-effects payload for any bonus-action Hide feature —
/// rolls one Dexterity (Stealth) check against the best passive
/// Perception watching and, on a pass, installs the `Hidden` condition
/// on the caster with a Permanent timer (cleared on the caster's next
/// attack via `clear_attack_advantage_riders`).
///
/// It used to install the condition outright, with no roll and nothing
/// to beat, while the Action-priced `Hide` rolled the check. That made
/// the cheap printing not a cheaper Hide but a better one, and the
/// difference was invisible because the two lived in different files.
/// Both go through `default_actions::resolve_hide_attempt` now.
pub fn bonus_action_hide_effects(
    encounter: &mut EncounterInstance,
    caster_id: usize,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    crate::actions::default_actions::resolve_hide_attempt(encounter, caster_id)
}

/// Rogue Cunning Action, the Hide pick.
pub static CUNNING_HIDE: BonusManeuver =
    BonusManeuver::new("cunning hide", &["chide", "ca-hide"], &[Maneuver::Hide]);

/// 5e monster trait **Nimble Escape** — "the goblin takes the Disengage
/// or Hide action" as a Bonus Action.
///
/// Two statics rather than one, because RAW's "or" is a choice the
/// creature makes each turn and the engine's action list is where a
/// choice lives. They are the Rogue's Cunning Disengage and Cunning
/// Hide in everything but the name — same cost, same payload, same
/// chassis — and they are separate statics because what is shared is
/// the *effect*, and a goblin whose action list reads "cunning hide"
/// is a goblin claiming a class feature it does not have.
///
/// Six stat blocks carry the trait in SRD 5.2 and they are not the six
/// anyone would guess: the three goblins, and then the Panther, the
/// Tiger and the Saber-Toothed Tiger. The cats are the interesting
/// half — a Nimble Escape panther fights the way a cat actually does,
/// closing, mauling and stepping back out of reach in the same turn,
/// and no amount of extra damage would have produced that.
///
/// Deliberately *not* the third of Cunning Action's three: RAW's Nimble
/// Escape has no Dash. A goblin's business is leaving without being
/// hit, not covering ground.
pub static NIMBLE_DISENGAGE: BonusManeuver = BonusManeuver::new(
    "nimble disengage",
    &["ndis", "ne-dis"],
    &[Maneuver::Disengage],
);

/// The Hide half of **Nimble Escape** — see `NIMBLE_DISENGAGE` above
/// for why the trait is two actions and why they are not the Rogue's.
pub static NIMBLE_HIDE: BonusManeuver =
    BonusManeuver::new("nimble hide", &["nhide", "ne-hide"], &[Maneuver::Hide]);

/// Vampire Familiar **Deathless Agility** — "the familiar takes the Dash
/// or Disengage action" as a Bonus Action.
///
/// Nimble Escape's other half, and the pairing is the point: the goblin
/// gets Disengage and Hide because a goblin's plan is to not be found,
/// and the familiar gets Disengage and Dash because a thrall's plan is
/// to be wherever its master needs a body. Same two-statics-for-one-"or"
/// shape as the goblins.
pub static DEATHLESS_DASH: BonusManeuver =
    BonusManeuver::new("deathless dash", &["ddash", "da-dash"], &[Maneuver::Dash]);

/// The Disengage half of **Deathless Agility** — see `DEATHLESS_DASH`.
pub static DEATHLESS_DISENGAGE: BonusManeuver = BonusManeuver::new(
    "deathless disengage",
    &["ddis", "da-dis"],
    &[Maneuver::Disengage],
);

/// 5e Ranger **Vanish** (class feature, level 14) — feature tag. Passive
/// gate on the paired `VANISH` action, which is a bonus-action Hide
/// (same one-shot attack-advantage rider as Cunning Hide / the baseline
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
/// action gates on this tag through the chassis's `gated_on` slot so a
/// template that doesn't carry the tag can't fire it even if the action
/// leaks onto its action list.
pub const VANISH_TAG: &str = "ranger.vanish";

/// Ranger Vanish — bonus-action Hide gated on the `VANISH_TAG` passive
/// feature. Ships on `RANGER_TEMPLATE` (and by inheritance on
/// `HUNTER_RANGER_TEMPLATE`); the RAW "can't be tracked by nonmagical
/// means" clause is a pure narrative rider with no mechanical surface
/// in the combat engine.
pub static VANISH: BonusManeuver =
    BonusManeuver::new("vanish", &["vnsh", "rvan"], &[Maneuver::Hide]).gated_on(VANISH_TAG);


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

/// The **Cunning Strike** chassis: a bonus-action prime that trades
/// sneak-attack dice for a tactical rider on the swing that cashes it.
///
/// 5e 2024 gives the Rogue six of these — three at level 5 (Cunning
/// Strike) and three more at level 14 (Devious Strikes) — and every one
/// of them is the same action with a different label: declare the trick,
/// install a self-condition, and let `consume_cunning_strike` in
/// `class_attacks` do the work when the sneak attack lands. They shipped
/// as four hand-written `Action` impls of ninety lines apiece that
/// differed in a name, an alias list and one enum variant, plus one
/// extra clause on Daze that every other one of them also needs and
/// three of them happened not to notice.
///
/// That clause is `dice_cost`, and it is the reason this is a chassis
/// rather than a tidy-up. RAW will not reduce a sneak-attack pool below
/// one die, so a prime is worth a bonus action only when the rogue's
/// pool is *larger* than what it costs — and the consume site enforces
/// that by silently declining, which means a rogue who primed too early
/// spent a bonus action on nothing and kept the prime. Daze carried the
/// gate by hand and the level-5 trio did not need one at 1d6, but
/// Devious Strikes' three cost 2, 3 and 6, and hand-writing the same
/// arithmetic six times is how five of them end up disagreeing.
///
/// Sibling to `ManeuverPrime` on the "bonus-action prime, all the
/// interesting part is data" lane. The difference is what funds them: a
/// maneuver spends from a per-rest pool of superiority dice, and a
/// cunning strike spends out of the damage it was about to deal.
pub struct CunningStrikePrime {
    /// Display name — the prompt's canonical entry and the log prefix.
    pub name: &'static str,
    /// Alias set for the prompt parser.
    pub aliases: &'static [&'static str],
    /// The self-condition this prime installs, and the key
    /// `consume_cunning_strike` matches on.
    pub prime_condition: Condition,
    /// Sneak-attack dice this trick costs when it is cashed. Read here
    /// to refuse a prime the rogue's pool cannot yet afford, and read
    /// again at the consume site to charge it — one number, so the gate
    /// and the charge cannot drift.
    pub dice_cost: u32,
}

impl Action for CunningStrikePrime {
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
        // The prime targets the rogue. Whatever lands on the enemy lands
        // on the swing that cashes it, which routes through the weapon's
        // own `is_harmful` gate.
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
        if !cunning_strike_prime_ok(encounter, caster_id) {
            return false;
        }
        // Strictly greater, matching the consume site: RAW's "you can't
        // reduce the number of dice rolled to less than 1" means a pool
        // exactly equal to the cost buys nothing.
        crate::actions::class_attacks::max_sneak_attack_dice(encounter, caster_id)
            > self.dice_cost
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
            condition: self.prime_condition,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

/// Cunning Strike: **Poison** (5e 2024 Rogue level 5). One d6 of sneak
/// attack for a CON save against the rogue's DEX-based DC; on a failure
/// the target is Poisoned for a minute.
pub static CUNNING_STRIKE_POISON: LazyLock<CunningStrikePrime> =
    LazyLock::new(|| CunningStrikePrime {
        name: "cunning strike (poison)",
        aliases: &["cs-poison", "cspoison", "cunningpoison"],
        prime_condition: Condition::CunningStrikePoison,
        dice_cost: 1,
    });

/// Cunning Strike: **Trip** (5e 2024 Rogue level 5). One d6 for a DEX
/// save; on a failure a Large-or-smaller target is knocked Prone. The
/// size clause lives at the consume site, where the target is known.
pub static CUNNING_STRIKE_TRIP: LazyLock<CunningStrikePrime> =
    LazyLock::new(|| CunningStrikePrime {
        name: "cunning strike (trip)",
        aliases: &["cs-trip", "cstrip", "cunningtrip"],
        prime_condition: Condition::CunningStrikeTrip,
        dice_cost: 1,
    });

/// Cunning Strike: **Withdraw** (5e 2024 Rogue level 5). One d6 to move
/// up to half the rogue's speed without provoking, immediately after the
/// attack. The only one of the six with no save and no target — it is
/// the rogue's own footwork rather than something done to anybody.
pub static CUNNING_STRIKE_WITHDRAW: LazyLock<CunningStrikePrime> =
    LazyLock::new(|| CunningStrikePrime {
        name: "cunning strike (withdraw)",
        aliases: &["cs-withdraw", "cswith", "cunningwithdraw"],
        prime_condition: Condition::CunningStrikeWithdraw,
        dice_cost: 1,
    });

/// Cunning Strike: **Daze** (5e 2024 Rogue **Devious Strikes**, level
/// 14). Two d6 for a CON save; on a failure the target's next turn loses
/// its Action and its Reaction.
pub static CUNNING_STRIKE_DAZE: LazyLock<CunningStrikePrime> =
    LazyLock::new(|| CunningStrikePrime {
        name: "cunning strike (daze)",
        aliases: &["cs-daze", "csdaze", "cunningdaze"],
        prime_condition: Condition::CunningStrikeDaze,
        dice_cost: 2,
    });

/// Cunning Strike: **Obscure** (5e 2024 Rogue **Devious Strikes**, level
/// 14). Three d6 for a DEX save; on a failure the target is Blinded
/// until the end of the rogue's next turn.
///
/// The one Cunning Strike whose rider is worth more to the party than to
/// the rogue: a blinded creature swings at disadvantage against
/// everybody and is swung at with advantage by everybody, which is worth
/// three dice of one rogue's damage several times over as soon as there
/// is a second body on the rogue's side.
pub static CUNNING_STRIKE_OBSCURE: LazyLock<CunningStrikePrime> =
    LazyLock::new(|| CunningStrikePrime {
        name: "cunning strike (obscure)",
        aliases: &["cs-obscure", "csobscure", "cunningobscure"],
        prime_condition: Condition::CunningStrikeObscure,
        dice_cost: 3,
    });

/// Cunning Strike: **Knock Out** (5e 2024 Rogue **Devious Strikes**,
/// level 14). Six d6 for a CON save; on a failure the target is put to
/// sleep for a minute, and wakes on any damage.
///
/// Six dice means a pool of seven, which means level 13, which on a
/// chassis that starts at level 1 and climbs by encounter makes this the
/// last button on the rogue's sheet to come online. It ships at RAW's
/// price rather than at a discount, because a discounted Knock Out is
/// the only Cunning Strike anybody would ever pick.
pub static CUNNING_STRIKE_KNOCK_OUT: LazyLock<CunningStrikePrime> =
    LazyLock::new(|| CunningStrikePrime {
        name: "cunning strike (knock out)",
        aliases: &["cs-knockout", "csknockout", "cunningknockout"],
        prime_condition: Condition::CunningStrikeKnockOut,
        dice_cost: 6,
    });

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
        let speed = encounter.travel_speed(caster_id);
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

/// 5e Barbarian Path of the Beast — **Form of the Beast** (subclass
/// level 3, TCE), Bite. While raging, the barbarian's teeth become a
/// natural weapon: 1d8 piercing, and once per turn a landed bite on a
/// barbarian below half their hit points heals them for their
/// proficiency bonus.
///
/// The three Form of the Beast tags are the engine's first *mutually
/// exclusive* subclass tell. Every other subclass family here — the
/// seven totem spirits, the three storm heralds — reads as one tag per
/// build because the subclasses themselves are distinct. Path of the
/// Beast is one subclass whose one feature offers three weapons, and
/// RAW re-picks between them on every rage.
///
/// It ships as three templates rather than as a per-rage choice for the
/// same reason the seven totem spirits are seven templates: a template
/// is this engine's unit of "a build", the pick is made once when the
/// character is created, and the alternative — a `beast_form:
/// Option<BeastForm>` field with a choosing surface wired into Rage —
/// buys a re-pick nobody in a single encounter would use. What the
/// tags do carry is the exclusion: each natural weapon gates on its own
/// tag as well as on `Raging`, so a hypothetical template that listed
/// two of the actions would still only be able to swing the one whose
/// tag it holds.
///
/// The Bite is the sustain form, and it is the only self-heal on the
/// barbarian chassis. Rage already halves the physical damage coming
/// in; the bite turns the back half of a long fight around, because its
/// heal only starts once the barbarian is below half — exactly where
/// Relentless Rage is keeping them standing.
pub const FORM_OF_THE_BEAST_BITE_TAG: &str = "barbarian.form_of_the_beast.bite";

/// 5e Barbarian Path of the Beast — **Form of the Beast** (subclass
/// level 3, TCE), Claws. While raging, 1d6 slashing, and RAW's "you can
/// make one additional attack with them as part of the Attack action".
///
/// The extra swing stacks *on top of* Extra Attack rather than
/// replacing it, which is RAW and is the whole reason to pick this form
/// over the Bite: a level-9 barbarian with claws swings three times a
/// turn. The smaller die is the price — 3d6+12 against the Bite's
/// 2d8+8 — and Reckless Attack is what makes the trade pay, since
/// advantage on three swings is worth more than advantage on two.
///
/// See `FORM_OF_THE_BEAST_BITE_TAG` for why the three forms ship as
/// three templates.
pub const FORM_OF_THE_BEAST_CLAWS_TAG: &str = "barbarian.form_of_the_beast.claws";

/// 5e Barbarian Path of the Beast — **Form of the Beast** (subclass
/// level 3, TCE), Tail. While raging, 1d8 piercing with 10 ft of reach.
///
/// The reach is the feature. A barbarian is a melee chassis with no
/// answer to a caster who steps back five feet, and the tail is the
/// only reach weapon on the class — it swings from a tile further out,
/// which on this grid also means it threatens a wider ring for
/// opportunity attacks.
///
/// RAW's other half — a reaction that adds 1d8 to the barbarian's AC
/// against one attack that would otherwise hit — isn't shipped. The
/// engine's reactive-AC lane resolves before the attack roll rather
/// than after it (`Shield`, Warding Flare), so "an attack that would
/// hit you" has no hook to hang on, and approximating it as a flat
/// pre-roll bonus would make the tail strictly better than the other
/// two forms rather than differently good.
///
/// See `FORM_OF_THE_BEAST_BITE_TAG` for why the three forms ship as
/// three templates.
pub const FORM_OF_THE_BEAST_TAIL_TAG: &str = "barbarian.form_of_the_beast.tail";

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

/// 5e Phantom Rogue **Wails from the Grave** (subclass level 3) feature
/// tag. Passive, read via `has_passive_feature` at the Sneak Attack
/// rider in `class_attacks` — see `push_wails_from_the_grave` for the
/// target-choice and uncapped-uses rationale.
///
/// Not on `ONCE_PER_TURN_RIDER_TAGS`, deliberately: the wail's trigger
/// is Sneak Attack landing, and Sneak Attack already holds a
/// once-per-turn mark of its own. A second ledger entry keyed to the
/// same event would be a second lock on the same door.
pub const WAILS_FROM_THE_GRAVE_TAG: &str = "rogue.wails_from_the_grave";

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
    FEROCIOUS_CHARGER_TAG,
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
    BOND_OF_FANG_AND_SCALE_TAG,
    // The only entry here that isn't a damage rider: Ancestral
    // Protectors uses the ledger to enforce RAW's "the *first* creature
    // you hit on your turn" rather than to cap a die pool. Same
    // mechanism, different purpose — which is the argument for the
    // ledger being keyed by plain tag rather than by rider identity.
    ANCESTRAL_PROTECTORS_TAG,
    HAND_OF_HARM_TAG,
    EMPOWERED_ARMS_TAG,
    // Also not a damage rider: Form of Dread uses the ledger to enforce
    // RAW's "once on each of your turns" on its fear rider, which keeps
    // the form itself alive across the trigger where
    // `consume_on_trigger` would have ended it.
    FORM_OF_DREAD_TAG,
    LIGHTNING_LAUNCHER_TAG,
    ARCANE_JOLT_TAG,
    // The first entry that is a *feat* rather than a class or subclass
    // feature — SRD 5.2's **Savage Attacker**, whose "once per turn"
    // window is the same one every row above measures. Nothing about
    // the ledger cared where the tag came from, which is the argument
    // for it being keyed by plain tag; see `crate::actions::feats`.
    crate::actions::feats::SAVAGE_ATTACKER_TAG,
    // The two Eldritch Blast invocations whose RAW text carries the
    // clause this ledger exists for — "once on each of your turns when
    // you hit a creature with your Eldritch Blast". Every other row here
    // is gated on a weapon hit; these two are gated on a *beam*, and a
    // high-level warlock fires four of them in one action, which is
    // precisely the case the window is written to cap. See
    // `GRASP_OF_HADAR_TAG` / `LANCE_OF_LETHARGY_TAG`.
    GRASP_OF_HADAR_TAG,
    LANCE_OF_LETHARGY_TAG,
    // The first entries that are a *species* trait — SRD 5.2's Goliath
    // Giant Ancestry. They are also the first rows whose RAW cadence is
    // not once-a-turn at all: the ancestry is rationed by a per-rest
    // pool, and the ledger is the engine's extra narrowing on top of
    // it. See `OncePerTurnWeaponRiderSpec::charge_tag` for why both
    // gates ride together.
    crate::actions::species::FIRES_BURN_TAG,
    crate::actions::species::FROSTS_CHILL_TAG,
    // SRD 5.2's **Boon of Combat Prowess** — the first row here that
    // fires on a *miss* rather than a hit, and the first whose RAW
    // spells the ledger's window out in full: "once you use this
    // benefit, you can't use it again until the start of your next
    // turn." Every other row above says "once on each of your turns"
    // and means the same thing. See `feats::BOON_OF_COMBAT_PROWESS_TAG`.
    crate::actions::feats::BOON_OF_COMBAT_PROWESS_TAG,
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
/// neither the pool cost nor the flat INT bump. The charge lane can
/// count now — `FEATURE_CHARGES` would happily give the pool its RAW
/// depth — so the reason is no longer the engine's; it is that the
/// pool's other consumer, `PROTECTIVE_FIELD_TAG`, is the feature whose
/// whole character is deciding *when* to spend, and a shared pool that
/// the offensive rider drains automatically on every hit is a pool the
/// defensive half never sees. RAW's tension is real and interesting at
/// a table where a player chooses; here the strike has no chooser.
/// Splitting them — strike free and once per turn, field charged —
/// keeps both features legible. The
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
/// RAW's second clause — a target whose CR is at or below a threshold
/// is banished to its home plane instead of merely frightened — ships as
/// the burst's `escalation` rung, gated on
/// `ARCANE_ABJURATION_BANISH_TAG`. It needed an off-board actor lane the
/// engine did not have; `crate::engine::banishment` is that lane. The
/// rung is deliberately the same shape and the same CR ceiling as Turn
/// Undead's Destroy Undead, which keeps this sideways from Turn Undead
/// rather than strictly better than it: the two clear the same weight of
/// enemy off the board, and this one only borrows it for a minute.
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

/// 5e Cavalier Fighter **Ferocious Charger** (subclass level 10, XGtE):
/// "you can ride down your foes: if you move at least 10 feet in a
/// straight line right before attacking a creature and you hit it with
/// the attack, that target must succeed on a Strength saving throw or be
/// knocked prone. You can use this feature only once on each of your
/// turns."
///
/// The ledger key for the once-per-turn half. A Cavalier has Extra
/// Attack, so without it one run would flatten a target twice.
pub const FEROCIOUS_CHARGER_TAG: &str = "fighter.ferocious_charger";

/// Ferocious Charger as a `ChargeRider` — the same lane the bestiary's
/// Charge and Pounce clauses ride.
///
/// The two player-facing differences from a boar's are exactly the two
/// fields the struct grew to hold them. `weapon: None`, because RAW says
/// "hit it with the attack" rather than naming a limb, and a fighter is
/// carrying whatever the party found. And a once-per-turn ledger key,
/// because RAW says so and Extra Attack would otherwise cash the same
/// run twice.
///
/// No bonus damage: RAW's clause is knockdown only. Ships on the CR-1
/// Cavalier chassis above its strict lv10 gate for the same reason every
/// other subclass template here runs above its own — class templates
/// target a balanced playable level, not lockstep PHB progression.
pub const FEROCIOUS_CHARGER: crate::engine::attack::ChargeRider =
    crate::engine::attack::ChargeRider {
        weapon: None,
        dice: Dice::new(0, 0),
        damage_type: DamageType::Bludgeoning,
        run_tiles: crate::engine::attack::charge_run_tiles(10),
        knocks_prone: true,
        label: "ferocious charger",
        knockdown_label: "ferocious charger knockdown",
        once_per_turn_tag: Some(FEROCIOUS_CHARGER_TAG),
        prone_follow_up: None,
        // The one charge clause in the engine that is a *character's*
        // rather than an animal's, and the only one with no size
        // ceiling: RAW's Ferocious Charger says "force the target to
        // make a Strength saving throw" and stops there, where every
        // beast in the bestiary first asks how big the target is. A
        // cavalier can therefore knock over a giant, which is the point
        // of the feature.
        max_target_size: None,
    };

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

/// 5e Circle of Dreams Druid **Balm of the Summer Court** (subclass
/// level 2) — the charge. One press per short rest.
///
/// RAW's resource is a pool of `d6`s equal to the druid's level, spent
/// a proficiency-bonus's worth at a time and refilled on a long rest.
/// The engine's charge lane counts presses rather than dice, so this is
/// one press of the largest legal spend — see `BALM_OF_THE_SUMMER_COURT`
/// for what that comes to. Collapsing a die pool to a press is the same
/// translation the Battle Master's maneuvers took before superiority
/// dice got a counter, and it costs the same thing: a druid cannot
/// dribble the pool out in small sips across a long fight.
pub const BALM_OF_THE_SUMMER_COURT_TAG: &str = "druid.balm_of_the_summer_court";

/// Balm of the Summer Court — Circle of Dreams Druid bonus action
/// (subclass level 2). Four d6 of healing to one creature within 120
/// ft, plus four temporary hit points.
///
/// **The temporary hit points are the feature, not the healing.** Four
/// d6 is fourteen on average, which a levelled Cure Wounds matches for
/// a slot the druid was going to have anyway. What no slot buys is the
/// second pool on top: a balmed ally is fourteen points healed *and*
/// four points harder to drop for the rest of the fight, and the second
/// number is the one that keeps working after the target goes back to
/// full.
///
/// Which makes it the roster's cheapest pre-emptive heal. Every other
/// healing feature is worth nothing on a body at full hit points — Lay
/// on Hands, Cure Wounds, Preserve Life all overheal into nothing — and
/// this one is worth four points on anybody. The AI's heal rungs still
/// hold it for the wounded, because that is where the other fourteen go.
///
/// A bonus action, and 120 ft of reach: RAW's range is what separates
/// the Dreams druid from every touch-range healer in the game. The balm
/// reaches the far side of any arena this engine generates, so the
/// druid never has to walk into the fight to spend it.
pub struct BalmOfTheSummerCourt {}

impl Action for BalmOfTheSummerCourt {
    fn name(&self) -> &str {
        "balm of the summer court"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["balm", "summer court", "botsc"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft on the 2.5 ft grid.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
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
        feature_ready(encounter, caster_id, BALM_OF_THE_SUMMER_COURT_TAG)
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
        if let Some(druid) = encounter.actors.get_mut(&caster_id) {
            druid.spend_feature(BALM_OF_THE_SUMMER_COURT_TAG);
        }
        // RAW spends up to a proficiency bonus worth of dice per use,
        // and heals the total rolled while granting temporary hit points
        // equal to the *number of dice*. Four is the proficiency bonus
        // on the level-9 chassis every druid template is built to.
        let dice = Dice::new(BALM_OF_THE_SUMMER_COURT_DICE, 6);
        let healed = encounter.roll(&dice);
        encounter.log(format!(
            "  balm of the summer court: {}({}) HP and {} temporary hit points",
            dice, healed, BALM_OF_THE_SUMMER_COURT_DICE
        ));
        vec![
            Box::new(Heal {
                actor_id: target_id,
                amount: healed,
            }),
            Box::new(crate::engine::side_effects::GainTempHp {
                actor_id: target_id,
                amount: BALM_OF_THE_SUMMER_COURT_DICE,
            }),
        ]
    }
}

/// How many dice the balm spends per press — RAW's proficiency bonus,
/// four on the level-9 chassis. Doubles as the temporary-hit-point
/// grant, because RAW ties the two to the same number: "the creature
/// regains hit points equal to the total... and gains 1 temporary hit
/// point per die spent".
const BALM_OF_THE_SUMMER_COURT_DICE: u32 = 4;

pub static BALM_OF_THE_SUMMER_COURT: LazyLock<BalmOfTheSummerCourt> =
    LazyLock::new(|| BalmOfTheSummerCourt {});

/// 5e Circle of Dreams Druid **Hidden Paths** (subclass level 10) — the
/// charge. One press per short rest.
///
/// RAW gives proficiency-bonus uses per long rest; the short-rest
/// cadence matches every other subclass charge on the roster, and one
/// press is what `FEATURE_CHARGES`' own preamble asks for — a second
/// blink inside a single fight is a blink the druid would rarely reach.
pub const HIDDEN_PATHS_TAG: &str = "druid.hidden_paths";

/// Hidden Paths — Circle of Dreams Druid bonus action (subclass level
/// 10). Teleport up to 60 ft to an unoccupied space the druid can see.
///
/// **The fourth row on `SELF_TELEPORT_ESCAPES`, and the first that
/// isn't a spell.** Benign Transposition spends a charge the conjurer's
/// own casting refills, Misty Step spends a 2nd-level slot and Dimension
/// Door a 4th; this one spends nothing but a bonus action and a
/// per-rest charge, which makes it the cheapest exit on the roster and
/// the only one a full caster can take without touching their slots.
///
/// That matters more on a druid than it would on a wizard. A druid's
/// concentration is usually holding something — Moonbeam, Spike Growth,
/// Call Lightning — and the alternative escapes either cost the slot
/// that would have replaced it or cost the action the concentration was
/// bought with. Hidden Paths costs neither.
///
/// RAW also lets the druid send a willing creature instead of
/// themselves. That half is left out: it needs both a target *and* a
/// destination, and the engine's targeting schemas carry one or the
/// other. The self half is the one the escape lane can drive.
pub struct HiddenPaths {}

impl Action for HiddenPaths {
    fn name(&self) -> &str {
        "hidden paths"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hp", "hidden path", "paths"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft on the 2.5 ft grid — the same envelope Shadow Step and
        // Misty Step blink across.
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
        _ti: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !feature_ready(encounter, caster_id, HIDDEN_PATHS_TAG) {
            return false;
        }
        // The destination has to hold the druid's whole footprint — the
        // same constraint Misty Step and Shadow Step apply, minus the
        // movement budget a teleport does not spend.
        let Some(point) = first_target_location(target_locations) else {
            return false;
        };
        encounter.can_move_to(caster_id, point)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        if let Some(druid) = encounter.actors.get_mut(&caster_id) {
            druid.spend_feature(HIDDEN_PATHS_TAG);
        }
        encounter
            .log("  hidden paths: the druid steps through the Plane of Faerie and back.".to_string());
        // Teleport, not movement — no intervening tiles, so no
        // opportunity attacks. Same reasoning as Misty Step.
        vec![Box::new(crate::engine::side_effects::TeleportActor {
            actor_id: caster_id,
            dest: point,
        })]
    }
}

pub static HIDDEN_PATHS: LazyLock<HiddenPaths> = LazyLock::new(|| HiddenPaths {});

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

/// **Insightful Fighting** — Inquisitive Rogue subclass level 3 (XGtE).
/// Bonus action, at will, 30 ft: the rogue reads a creature and installs
/// `Condition::Analyzed` back-linked to themselves for 10 rounds
/// (1 minute RAW).
///
/// The mark is a fourth path through
/// `class_attacks::sneak_attack_eligible`, and the only one the rogue
/// manufactures rather than finds. See `Condition::Analyzed` for what
/// RAW's Insight-versus-Deception contest costs to drop and what the
/// bonus action buys back.
///
/// Shaped on Vow of Enmity directly above — same bonus-action,
/// single-hostile-target, flag-plus-back-link install, and the same
/// re-application dedup so a rogue doesn't spend a turn refreshing a
/// timer with nine rounds left on it. The differences are the price and
/// the reach: the vow is once per rest at 10 ft, this is free at 30, and
/// what it hands over is smaller for exactly that reason.
pub struct InsightfulFighting {}

impl Action for InsightfulFighting {
    fn name(&self) -> &str {
        "insightful fighting"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["if", "insight", "read"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft RAW = 12 tiles on the 2.5 ft grid.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        // RAW's contest is a read of the creature's tells; you cannot
        // read what you cannot see.
        true
    }
    fn is_harmful(&self) -> bool {
        // Nothing lands on the target — no damage, no save — but the
        // mark is aimed at an enemy, so the flag keeps the AI's
        // helpful-action lane from ever offering it against an ally.
        // Same reasoning as Vow of Enmity above.
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
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let (Some(rogue), Some(target)) = (
            encounter.actors.get(&caster_id),
            encounter.actors.get(&target_id),
        ) else {
            return false;
        };
        if !rogue.is_combat_active()
            || !target.is_combat_active()
            || target.team() == rogue.team()
        {
            return false;
        }
        // Already read by *this* rogue: re-applying only refreshes a
        // timer, and the bonus action is better spent on Cunning
        // Action. A target read by a different rogue is still fair
        // game — the install takes over the back-link.
        target.linked_by(Condition::Analyzed) != Some(caster_id)
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
        encounter.log(
            "  insightful fighting: the rogue reads the target's guard and finds the gap."
                .to_string(),
        );
        crate::engine::side_effects::install_condition_with_link(
            Condition::Analyzed,
            target_id,
            caster_id,
            // 10 rounds = 1 minute RAW, the same envelope every other
            // one-minute mark on the roster carries.
            ConditionTimer::Rounds(10),
        )
    }
}

pub static INSIGHTFUL_FIGHTING: LazyLock<InsightfulFighting> =
    LazyLock::new(|| InsightfulFighting {});

/// Passive tag for the Inquisitive Rogue's **Eye for Weakness**
/// (subclass level 17, XGtE): "you deal an extra 3d6 damage to that
/// target whenever you sneak attack it" — "that target" being the one
/// the rogue read with Insightful Fighting.
///
/// Read at the Sneak Attack die-count site in `class_attacks`, where it
/// adds three dice to the pool when the target carries `Analyzed` linked
/// back to this rogue. Not a flat rider on top of the sneak damage: it
/// joins the pool, so a crit doubles it and Cunning Strike can spend
/// against it, which is what RAW's "extra 3d6" on a Sneak Attack means
/// at every other site the engine models.
///
/// Ships on the CR-2 rogue chassis above its strict RAW lv17 gate for
/// the same reason every other class template runs above strict RAW
/// level — templates target a balanced playable level, not lockstep PHB
/// progression. It is also what makes Insightful Fighting worth a bonus
/// action against a target the rogue could already sneak-attack: without
/// it, a rogue standing beside an ally has nothing to buy.
pub const EYE_FOR_WEAKNESS_TAG: &str = "rogue.eye_for_weakness";

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

/// 5e Way of Mercy Monk **Hand of Harm** (subclass level 3) feature
/// tag, with **Physician's Touch**'s poison clause (lv6) folded in.
/// Passive once-per-turn weapon-hit rider: +1d6 necrotic and Poisoned
/// on the target. Lives as a row on
/// `engine::attack::ONCE_PER_TURN_WEAPON_DIE_RIDERS` and as a key on
/// the shared `ONCE_PER_TURN_RIDER_TAGS` ledger, so nothing reads it as
/// an action — the monk's ordinary unarmed strike carries it.
///
/// Sibling to `TOUCH_OF_DEATH_TAG` (Long Death Monk lv3) on the "monk
/// subclass whose flavour is necrotic" lane and the exact inverse of
/// the same subclass's Hand of Healing: one hand mends, the other
/// sickens, and the RAW subclass is built on the pair being the same
/// gesture.
pub const HAND_OF_HARM_TAG: &str = "monk.hand_of_harm";

/// 5e Way of the Astral Self Monk **Empowered Arms** (subclass level
/// 11, TCE) feature tag: "once on each of your turns when you hit a
/// creature with the Arms of the Astral Self, you can deal extra damage
/// equal to your martial arts die."
///
/// A row on `engine::attack::ONCE_PER_TURN_WEAPON_DIE_RIDERS` and a key
/// on the shared `ONCE_PER_TURN_RIDER_TAGS` ledger, next to Deft Strike
/// and Hand of Harm — the third monk subclass to buy its damage on that
/// cohort, and the first row on it whose gate is a *condition* rather
/// than the tag alone. RAW fires only on the arms, so the row carries
/// `caster_gate: Some(|a| a.has_condition(Condition::AstralArms))`: a
/// monk whose arms have lapsed punches for the ordinary die.
///
/// While the arms *are* up the gate is total rather than exact — RAW
/// scopes the rider to the arms and the engine scopes it to any weapon
/// swing made while they are summoned. On this chassis the two are the
/// same set: the Astral Self monk carries no weapon, and its two
/// attacks are the arms and the fist the arms replace.
pub const EMPOWERED_ARMS_TAG: &str = "monk.empowered_arms";

/// 5e Way of the Astral Self Monk **Body of the Astral Self: Deflect
/// Energy** (subclass level 11, TCE) feature tag: "when you take acid,
/// cold, fire, force, lightning, necrotic, poison, psychic, radiant, or
/// thunder damage, you can use your reaction to reduce it by 1d10 + your
/// Wisdom modifier."
///
/// A row on `engine::attack::REACTIVE_DAMAGE_CLAMPS`, and the second
/// there with a damage-type filter after the Nature Cleric's Dampen
/// Elements — but a far wider one. Dampen Elements names five types;
/// this names ten, every type in the game that is not bludgeoning,
/// piercing or slashing. Which is the point of it on this chassis:
/// Deflect Missiles, sitting one row above on the same monk, already
/// covers the physical half of the incoming damage the RAW subclass
/// leaves alone, so an Astral Self monk holding a reaction has an
/// answer to almost anything.
///
/// Uncharged (`tag: None` on the row) because RAW puts no per-rest cap
/// on it — the reaction is the whole limit, and the reaction economy is
/// something the engine already tracks. The tag exists only as the
/// passive-feature flag the row's `flag` closure reads.
pub const DEFLECT_ENERGY_TAG: &str = "monk.deflect_energy";

/// 5e Way of the Drunken Master Monk **Drunken Technique** (subclass
/// level 3, XGtE) feature tag: "whenever you use Flurry of Blows, you
/// gain the benefit of the Disengage action, and your walking speed
/// increases by 10 feet until the end of the current turn."
///
/// Read inside `FlurryOfBlows::side_effects`, which is the only place
/// the trigger can be seen — RAW hangs the clause off *using another
/// feature*, not off a cost or a turn boundary, and Flurry is where that
/// happens.
///
/// The pairing is the subclass. Every other monk's bonus action makes it
/// a choice between hitting more and leaving safely: Flurry buys the
/// extra strike, Step of the Wind buys the exit, and a monk gets one of
/// them. The Drunken Master's Flurry is both, which turns the chassis
/// from a creature that has to commit to a contact it entered into one
/// that can strike three times and walk out of reach in the same turn.
///
/// The +10 ft is spelled as extra movement handed to the turn rather
/// than as a speed bonus, because `speed()` is read once when the turn's
/// budget is granted and a mid-turn bump to it would arrive after the
/// budget already existed. Step of the Wind's Dash rider has the same
/// shape for the same reason, one size larger.
pub const DRUNKEN_TECHNIQUE_TAG: &str = "monk.drunken_technique";

/// 5e Way of the Drunken Master Monk **Tipsy Sway: Redirect Attack**
/// (subclass level 6, XGtE) feature tag: a melee miss against the monk
/// can be made to land on somebody else standing next to them.
///
/// Passive and uncharged — the whole feature is a reaction the engine
/// spends on the monk's behalf at the miss branch of
/// `resolve_attack_outcome`. See `engine::attack::try_fire_redirect_attack`
/// for the three places the implementation narrows RAW, and for why the
/// redirected swing rolls fresh damage rather than carrying any over.
pub const REDIRECT_ATTACK_TAG: &str = "monk.redirect_attack";

/// 5e Mastermind Rogue **Misdirection** (subclass level 13, XGtE)
/// feature tag: "when you're targeted by an attack while a creature
/// within 5 feet of you is granting you cover, you can use your reaction
/// to have the attack target that creature instead of you."
///
/// The second row on `engine::attack::ATTACK_REDIRECTS`, and the one
/// that answers a hit rather than a miss. Passive and uncharged — the
/// reaction is the whole budget.
///
/// It is also the roster's only defensive feature that costs somebody
/// else something. Every other reaction in the engine spends the
/// holder's resources: a clamp spends their reaction, an interposition
/// spends their hit points. This one spends whoever happened to be
/// standing between the rogue and the arrow, and RAW does not care
/// whether that creature is a friend. Which is the subclass — the
/// Mastermind's other shipped feature hands an ally advantage from
/// thirty feet away, and this one hands them an arrow from five.
pub const MISDIRECTION_TAG: &str = "rogue.misdirection";

/// 5e Way of the Drunken Master Monk **Drunkard's Luck** (subclass level
/// 11, XGtE) feature tag: "when you make an ability check, an attack
/// roll, or a saving throw and have disadvantage, you can spend 2 ki
/// points to cancel the disadvantage for that roll."
///
/// One charge per short rest, which is the engine's usual stand-in for a
/// ki cost (see Shadow Arts' slot table for the other one). The charge
/// is spent by the engine rather than by the player, at the two d20
/// chokepoints that can see a disadvantaged roll coming and still have
/// `&mut` in hand: the attack roll in `resolve_attack_outcome` and the
/// save in `EncounterInstance::roll_save_with_extra_mode_and_bonus`.
/// Ability checks are the third RAW context and the engine rolls none.
///
/// **Cancel, not upgrade.** RAW says the disadvantage goes away, leaving
/// whatever the roll would otherwise have been — so a monk who is both
/// prone-adjacent-advantaged and Poisoned does not come out with
/// advantage, they come out rolling straight. The implementation clears
/// the mode to `Normal` rather than combining an `Advantage` into it,
/// which is the difference.
///
/// It is deliberately spent on the first disadvantaged roll of the fight
/// rather than saved for a better one. The engine has no way to know
/// whether a bigger swing is coming, and a charge held for a moment that
/// never arrives is worth less than a charge spent on the swing in front
/// of it — the same reasoning `fire_missed_attack_boost` states for the
/// per-rest miss-rescue cohort.
pub const DRUNKARDS_LUCK_TAG: &str = "monk.drunkards_luck";

/// Arms of the Astral Self — Way of the Astral Self Monk lv3 subclass
/// action. Bonus action, at will: spectral arms settle over the monk's
/// own for a minute, and `ASTRAL_ARMS_STRIKE` — which refuses to
/// validate without them — becomes the monk's best attack.
///
/// At-will because RAW prices it in ki and this engine has no ki, the
/// same collapse Flurry of Blows, Patient Defense, Step of the Wind,
/// Deft Strike and both of Mercy's hands already make on this chassis.
/// A bonus action, because that is RAW's cost and because it is the
/// scarce thing here: summoning the arms means not flurrying, which is
/// the trade the subclass is built on. The ten-round window means the
/// monk pays that once and swings with the arms for the rest of the
/// fight — an opening-round tempo cost, not a per-round tax.
///
/// The `!has_condition` gate is what stops the AI re-summoning arms it
/// is already wearing. Without it the picker would spend every bonus
/// action of the fight on a no-op refresh, because the rung sits above
/// Flurry of Blows and the action would keep validating.
pub struct ArmsOfTheAstralSelf {}

impl Action for ArmsOfTheAstralSelf {
    fn name(&self) -> &str {
        "arms of the astral self"
    }
    fn aliases(&self) -> Vec<&str> {
        // Not "arms": `ASTRAL_ARMS_STRIKE` claims it, and alias
        // collisions resolve by list order rather than erroring — the
        // monk would have had two of its own actions fighting over one
        // token, and the one that lost would be unreachable from the
        // prompt.
        vec!["astral self", "summon arms", "aotas"]
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
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && !a.has_condition(Condition::AstralArms))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log(
            "  arms of the astral self: spectral arms settle over the monk's own.".to_string(),
        );
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::AstralArms,
            // 10 rounds = 1 minute RAW, the same window Bladesong,
            // Starry Form and Rage run on.
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static ARMS_OF_THE_ASTRAL_SELF: LazyLock<ArmsOfTheAstralSelf> =
    LazyLock::new(|| ArmsOfTheAstralSelf {});

/// Conditions **Physician's Touch** (Way of Mercy Monk lv6) lifts when
/// Hand of Healing lands. RAW's list exactly: "blinded, deafened,
/// paralyzed, poisoned, or stunned."
///
/// Ordered heaviest-first, because `RemoveOneOfConditions` pops the
/// first match and RAW lets the monk choose. Paralyzed costs its holder
/// every turn they have left; Deafened costs almost nothing in this
/// engine. A monk who would rather cure the deafness than the paralysis
/// is not a case worth modelling.
///
/// Deliberately narrower than `CLEANSING_TOUCH_DEBUFFS`, which lifts
/// any spell effect. This is a physician's list — five afflictions of
/// the body — and it does not touch Charmed, Frightened, or any of the
/// curses and marks on the paladin's list.
pub const PHYSICIANS_TOUCH_CLEANSES: &[Condition] = &[
    Condition::Paralyzed,
    Condition::Stunned,
    Condition::Blinded,
    Condition::Poisoned,
    Condition::Deafened,
];

/// Hand of Healing — Way of Mercy Monk lv3 subclass action, with
/// **Physician's Touch** (lv6) folded in. Bonus action; one ally within
/// reach is healed for `1d6 + WIS modifier` and loses one of the five
/// afflictions in `PHYSICIANS_TOUCH_CLEANSES`.
///
/// **At-will, and RAW prices it in ki.** That is the same trade Flurry
/// of Blows, Patient Defense, Step of the Wind and Deft Strike already
/// make on this chassis, and the one `KI_POINTS_TAG` deliberately does
/// not undo: the bonus action *is* the monk's scarce resource, and
/// charging ki on top of it would be charging twice. A monk who spends
/// bonus action mending has given up the Flurry, the Dodge, the
/// disengage-dash and the Stunning Strike prime for that round, which
/// is a real cost and the reason this doesn't need an artificial
/// charge on top of it.
///
/// **A bonus action, and RAW makes it an Action** until level 11, when
/// Flurry of Healing and Harm moves it onto the Flurry's bonus action.
/// The monk chassis here already ships Empty Body (RAW lv18) and
/// Diamond Soul (RAW lv14), so the lv11 version is the one that belongs
/// on it — and the Action-cost version would be unpickable anyway: an
/// Action on this chassis is two unarmed strikes, and no controller
/// would ever trade them for five hit points.
///
/// **The cleanse is what makes it a subclass feature** rather than a
/// small heal. Nothing else on the roster lifts Paralyzed or Stunned
/// for less than a level-5 slot (Greater Restoration) or a paladin's
/// once-per-rest Cleansing Touch. A Mercy monk standing next to a
/// paralyzed fighter can hand them their turn back every round, for
/// free, and still have their Action to swing with.
///
/// RAW's "a creature you touch" includes the monk. It is written as an
/// ally-only touch here because the monk already has Wholeness of Body
/// for themselves, and because `try_support_heal` — the AI rung that
/// finds this action — walks allies other than the caster. A self-target
/// lane would be a second gate for a strictly worse self-heal.
pub struct HandOfHealing {}

impl Action for HandOfHealing {
    fn name(&self) -> &str {
        "hand of healing"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hoh", "mercy-heal"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // RAW: touch.
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
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        if target_id == caster_id {
            return false;
        }
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        // Allies only, and only ones the touch can still do something
        // for — a target at full HP with nothing to cleanse would burn
        // the monk's bonus action on a no-op. A dying ally is always
        // worth touching: the heal is what stands them back up.
        if target.team() != caster.team() {
            return false;
        }
        if !target.is_combat_active() && !target.is_dying() {
            return false;
        }
        target.is_wounded()
            || PHYSICIANS_TOUCH_CLEANSES
                .iter()
                .any(|&c| target.has_condition(c))
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
        // Martial arts die + WIS modifier, RAW. The die is the monk's,
        // the modifier is the monk's — the target contributes nothing,
        // which is why both reads are off the caster.
        let wis = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.ability_modifier(AbilityScoreType::Wisdom))
            .unwrap_or(0);
        let rolled = encounter.roll(&Dice::new(1, 6)) as i32;
        let amount = (rolled + wis).max(1) as u32;
        let target_name = encounter.actor_name(target_id);
        encounter.log(format!(
            "  hand of healing: monk mends {} for {} HP.",
            target_name, amount
        ));
        vec![
            Box::new(Heal {
                actor_id: target_id,
                amount,
            }),
            Box::new(crate::engine::side_effects::RemoveOneOfConditions {
                actor_id: target_id,
                candidates: PHYSICIANS_TOUCH_CLEANSES.to_vec(),
            }),
        ]
    }
}

pub static HAND_OF_HEALING: LazyLock<HandOfHealing> = LazyLock::new(|| HandOfHealing {});

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
/// Two charges per short rest, which is RAW exactly — see
/// `FEATURE_CHARGES`. Registered in `SHORT_REST_FEATURES` so both the
/// cadence and the count come back the way the PHB says they do.
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
pub static STEP_OF_THE_WIND: BonusManeuver = BonusManeuver::new(
    "step of the wind",
    &["sotw", "step", "wind"],
    // Dash **and** Disengage in one activation — the one printing in
    // the book that is an "and" rather than an "or", and the reason
    // `maneuvers` is a slice.
    &[Maneuver::Dash, Maneuver::Disengage],
);

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

/// Class-feature tag for the Bard's Bardic Inspiration. RAW is a pool of
/// CHA-mod uses, and `FEATURE_CHARGES` sizes it at three — the modifier
/// every bard template on the roster actually has. Refreshes on a short
/// rest from level 5 (Font of Inspiration) and on a long rest before
/// that; `ActorInstance::short_rest` reads `FONT_OF_INSPIRATION_TAG` to
/// tell the two apart.
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
        // The die carries a back-link to the bard who granted it, so
        // the College of Eloquence's Unfailing Inspiration can tell its
        // own dice from anyone else's. Routed through the shared
        // installer rather than a hand-written pair — `Inspired` is a
        // row in `LINKED_CONDITIONS` and the helper is what keeps the
        // flag and the link from drifting apart.
        crate::engine::side_effects::install_condition_with_link(
            Condition::Inspired,
            target_id,
            caster_id,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static BARDIC_INSPIRATION: LazyLock<BardicInspiration> = LazyLock::new(|| BardicInspiration {});

/// Passive tag marking a bard who knows **Countercharm** (PHB lv6). No
/// charges — RAW spends the bard's Action and nothing else, so the tag
/// is a pure "can this actor take this action" gate rather than a
/// `FEATURE_CHARGES` entry, and sits on the same lane as Purity of Body
/// and Aspect of the Moon.
pub const COUNTERCHARM_TAG: &str = "bard.countercharm";

/// 30 ft of RAW on the 2.5 ft grid. The bard has to be able to be heard,
/// and this is the envelope RAW gives that.
const COUNTERCHARM_RADIUS: isize = 12;

/// Countercharm — Bard Action, self-centered ally burst. "You can use
/// your action to start a performance that lasts until the end of your
/// next turn. During that time, you and any friendly creatures within
/// 30 feet of you have advantage on saving throws against being
/// frightened or charmed."
///
/// The engine could not hold this feature until now, and it is worth
/// being precise about why: the advantage RAW grants is scoped to *two
/// named conditions*, and until `roll_save_vs_condition` existed a save
/// knew its ability and its DC and nothing about what failing it would
/// do. The only expressible approximations were the two the rest of the
/// codebase had already been forced into — blanket immunity to Charmed
/// and Frightened (a level-6 bard out-warding an Ancients Paladin's
/// level-15 capstone, on an Action, every round, for the whole team) or
/// blanket advantage on WIS and CHA saves (which would also cover the
/// team's saves against Hold Person, Banishment and Feeblemind). Both
/// are worse than not shipping it. The condition-scoped cohort is the
/// first shape that is simply correct.
///
/// No charge and no concentration: the cost is the bard's Action, and
/// they pay it again every round they want the performance to continue.
/// That is what makes the 2-round timer right rather than stingy — the
/// buff is meant to lapse the moment the bard stops singing.
///
/// RAW's "friendly creatures that can hear you" is enforced: a deafened
/// ally is not in the target set. That clause is what finally gave
/// `Deafened` a mechanical surface — see `ActorInstance::can_hear`,
/// which four monster abilities read for the same reason.
pub struct Countercharm {}

impl Action for Countercharm {
    fn name(&self) -> &str {
        "countercharm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cc", "counter"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    /// Declared so the AI's support rung can ask who is standing in the
    /// performance's envelope rather than guessing — same reason
    /// `AtWillAllyTempHpPulse` declares one, and harmless on a `NoArgs`
    /// action for the same reason.
    fn reach_tiles(&self) -> Option<isize> {
        Some(COUNTERCHARM_RADIUS)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    /// Free, repeatable and self-limiting — the three things the AI's
    /// ally-pulse rung needs to be true before it fires something every
    /// turn. The self-limiting half is the `Countercharmed` check in
    /// `custom_validate_input` below.
    fn pulses_ally_buff(&self) -> bool {
        true
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
        // Re-singing over a performance already in effect would spend
        // the bard's Action to refresh a timer that has not run out.
        // The AI re-ranks its whole ladder every turn and would
        // otherwise sit here forever; a human typing `cc` twice gets
        // the same answer for the same reason.
        actor.has_passive_feature(COUNTERCHARM_TAG)
            && !actor.has_condition(Condition::Countercharmed)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        install_ally_burst_condition(
            encounter,
            caster_id,
            COUNTERCHARM_RADIUS,
            // RAW caps by range, not by headcount — the whole party
            // inside 30 ft is protected. `usize::MAX` says "no cap"
            // through the shared helper's `truncate`.
            usize::MAX,
            Condition::Countercharmed,
            // "Until the end of your next turn" — see the condition's
            // docstring for why the short window is load-bearing.
            ConditionTimer::Rounds(2),
            "countercharm",
            "steadied by the bard's song",
            // "…that can hear you." See `ActorInstance::can_hear`.
            true,
        )
    }
}

pub static COUNTERCHARM: LazyLock<Countercharm> = LazyLock::new(|| Countercharm {});

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
/// - `label` — log prefix ("turn undead" / "turn the faithless"), taken
///   from the burst's own display name.
///
/// Takes the whole `TurnBurst` rather than its fields one at a time.
/// The parameter list had reached eight and a
/// `#[allow(clippy::too_many_arguments)]` before the escalation rung
/// wanted a ninth, and every caller was already a `&TurnBurst`
/// spreading itself out at the call site: `TurnBurst::side_effects` is
/// the only one, and it passed seven of its own fields in order.
/// How far a Channel-Divinity burst reaches, in tiles — RAW's 30 feet.
///
/// Named because two places read it now: the resolver below, and
/// `TurnBurst::self_burst_radius`, which is what stops the AI guessing
/// at it. A literal in each would be two numbers to keep in step.
const TURN_BURST_RADIUS: isize = 12;

fn resolve_turn_burst(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    config: &TurnBurst,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
    let TurnBurst {
        tag: feature_tag,
        dc_ability: spellcasting_ability,
        type_filter: is_affected,
        installed,
        timer,
        name: label,
        escalation,
        ..
    } = *config;

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
    // Escalation gate — resolved once up front so the per-target branch
    // below is a cheap `Option` read rather than a fresh
    // `has_passive_feature` lookup per candidate. `None` here means
    // either the burst has no escalation rung at all or this caster has
    // not unlocked the one it has, and every failed save takes the
    // standard install.
    //
    // The per-target half of the gate — creature type and CR — stays
    // inside the loop, which is what lets a mixed-cohort burst route
    // some of its victims through the escalation and the rest through
    // the standard install.
    let escalation = escalation.filter(|e| caster.has_passive_feature(e.tag));
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
            dist <= TURN_BURST_RADIUS
        })
        .collect();

    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    for id in candidates {
        let save = encounter.roll_save(id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            continue;
        }
        // Escalation branch: a weak enough creature of the right type
        // doesn't merely fail — it is destroyed (Destroy Undead) or
        // sent away (Arcane Abjuration). Mutually exclusive with the
        // standard install per target, matching RAW's "instead of"
        // phrasing on both features, and falling through on any gate
        // miss (wrong type, above-ceiling CR, caster without the tag).
        if let Some(rung) = escalation
            && let Some((cr, hp)) = encounter
                .actors
                .get(&id)
                .filter(|t| (rung.type_filter)(t.creature_type()))
                .map(|t| (t.cr(), t.hitpoints()))
                .filter(|(cr, _)| *cr <= rung.cr_ceiling)
        {
            encounter.log(format!(
                "  {}: {} (CR {} ≤ {}).",
                label, rung.log_verb, cr, rung.cr_ceiling
            ));
            match rung.effect {
                // Damage equal to the target's *current* HP rather than
                // its maximum: enough to finish it against any
                // resistance profile, without overshooting into the
                // massive-damage instant-kill lane.
                TurnEscalationEffect::Destroy { damage_type } => {
                    effects.push(Box::new(crate::engine::side_effects::DealDamage {
                        actor_id: id,
                        amount: hp,
                        damage_type,
                    }));
                }
                TurnEscalationEffect::Install { condition, timer } => {
                    effects.extend(crate::engine::side_effects::install_condition_with_link(
                        condition, id, caster_id, timer,
                    ));
                }
            }
            continue;
        }
        // `install_condition_with_link` covers the back-link half for
        // conditions that carry one — Charmed's back-link anchors the
        // "can't attack your charmer" gate and Frightened's anchors
        // both of RAW's "source of fear" clauses, so the same call
        // serves every variant this burst can install.
        effects.extend(crate::engine::side_effects::install_condition_with_link(
            installed,
            id,
            caster_id,
            timer,
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
    /// How long the install lasts. `Rounds(10)` is the 1-minute duration
    /// every Turn / dread variant carries in RAW and was hardcoded in
    /// the resolver until a row wanted otherwise: the Crown Paladin's
    /// Champion Challenge installs `Rooted`, and a hold that severe has
    /// to be measured in a round rather than in a minute.
    pub timer: ConditionTimer,
    /// The rung a weak enough victim falls through instead of the
    /// standard install, or `None` for a burst that has no such rung.
    ///
    /// Two features in 5e say "instead of merely being turned, a
    /// creature this weak is *removed*", and they say it in the same
    /// shape: a passive unlocked a few levels after the Channel
    /// Divinity itself, a creature-type gate, and a CR ceiling. The
    /// cleric's **Destroy Undead** burns the target to nothing; the
    /// Arcana Domain's **Arcane Abjuration** sends it home. Destroy
    /// Undead used to be spelled out inline in `resolve_turn_burst`,
    /// which is why the second one was never written.
    pub escalation: Option<TurnEscalation>,
}

/// The "instead of turned" rung on a `TurnBurst` — see
/// `TurnBurst::escalation`.
#[derive(Clone, Copy)]
pub struct TurnEscalation {
    /// Passive feature the caster must hold for the rung to exist.
    /// Distinct from the burst's own `tag`, which is the per-rest
    /// charge: this one is an always-on unlock, and a caster who has
    /// spent their Channel Divinity can't reach the rung because they
    /// can't reach the burst.
    pub tag: &'static str,
    /// Creature types the rung applies to, on top of the burst's own
    /// filter. Not redundant with it: a burst may sweep a wider cohort
    /// than the rung removes — the baseline cleric's Turn Undead
    /// happens to agree, but the Arcana Cleric's four extraplanar types
    /// and a hypothetical mixed-cohort Turn would not.
    pub type_filter: fn(crate::engine::types::CreatureType) -> bool,
    /// Highest CR the rung reaches. A single number rather than the
    /// per-level ramp RAW gives both features, for the same reason
    /// `DESTROY_UNDEAD_CR_CEILING` was one before it moved here: the
    /// engine's class templates sit at a fixed playable band rather
    /// than tracking a level.
    pub cr_ceiling: f32,
    /// What the rung does to a victim that clears both gates.
    pub effect: TurnEscalationEffect,
    /// Log fragment, in the third person and without the CR clause —
    /// "destroys undead", "banishes to its home plane". The resolver
    /// supplies the prefix and the CR arithmetic around it.
    pub log_verb: &'static str,
}

/// What a `TurnEscalation` does to a creature that clears its gates.
#[derive(Clone, Copy)]
pub enum TurnEscalationEffect {
    /// Deal damage equal to the target's remaining hit points — the
    /// cleric's Destroy Undead. `damage_type` so the kill still reads
    /// through the target's resistance profile rather than bypassing
    /// it.
    Destroy { damage_type: DamageType },
    /// Install a condition in place of the burst's usual one — the
    /// Arcana Cleric's Arcane Abjuration, which banishes rather than
    /// frightens. Routed through `install_condition_with_link` exactly
    /// as the standard install is, so a condition carrying a back-link
    /// still gets one.
    Install {
        condition: Condition,
        timer: ConditionTimer,
    },
}

impl Action for TurnBurst {
    fn affects_creature(&self, target: &crate::actors::actor_template::ActorInstance) -> bool {
        // The RAW type gate, surfaced so the AI can count only the
        // creatures the burst could actually turn. `resolve_turn_burst`
        // applies the same closure per candidate; this is the read the
        // picker needs before it spends the charge.
        (self.type_filter)(target.creature_type())
    }
    fn self_burst_radius(&self) -> Option<isize> {
        // RAW's 30 feet, and the same constant the resolver measures
        // with — see `TURN_BURST_RADIUS`.
        Some(TURN_BURST_RADIUS)
    }
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
        resolve_turn_burst(encounter, caster_id, self)
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
    // 1 minute, RAW.
    timer: ConditionTimer::Rounds(10),
    // 5e Cleric **Destroy Undead** (level 5): a failed save from a weak
    // enough undead destroys it outright instead of turning it. Data
    // here rather than a branch inside the resolver, which is what let
    // Arcane Abjuration's sibling clause be written at all.
    escalation: Some(TurnEscalation {
        tag: DESTROY_UNDEAD_TAG,
        type_filter: |ct| ct.is_undead(),
        cr_ceiling: DESTROY_UNDEAD_CR_CEILING,
        effect: TurnEscalationEffect::Destroy {
            damage_type: DamageType::Radiant,
        },
        log_verb: "destroys undead",
    }),
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
    // 1 minute, RAW.
    timer: ConditionTimer::Rounds(10),
    // No "instead of turned" rung: RAW gives this one no
    // remove-the-weak clause.
    escalation: None,
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
/// argument for existing, and it now runs all the way down: RAW's lv5
/// clause banishes a low-CR outsider to its home plane instead of
/// merely frightening it, and that ships as the `escalation` rung
/// below, mirroring Turn Undead's Destroy Undead at the same CR
/// ceiling. So a party fielding both clerics can clear every
/// extraplanar creature type in the engine off the board with one
/// Channel Divinity press each.
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
    // 1 minute, RAW.
    timer: ConditionTimer::Rounds(10),
    // 5e Arcana Domain **Arcane Abjuration**, second clause (level 5):
    // "if the creature's challenge rating is at or below a certain
    // threshold, it is instead banished for 1 minute (as in the
    // banishment spell, no concentration required)".
    //
    // The clause the feature shipped without, and its own docstring
    // named the reason: it needed an off-board actor lane the engine
    // did not have. It has one now — see `crate::engine::banishment`.
    //
    // The ceiling is pegged to Destroy Undead's rather than to RAW's
    // own ramp, which starts at CR ½ and reaches 1 at level 11 where
    // the cleric's reaches 1 at level 8. The engine has one number per
    // rung rather than a per-level table, and the two rungs are the
    // same rung — "your Channel Divinity removes a weak enemy from the
    // fight outright" — so a shared ceiling is the reading that keeps
    // the two domains comparable. Banishment is strictly the gentler
    // of the two outcomes: it is a minute, not a death.
    escalation: Some(TurnEscalation {
        tag: ARCANE_ABJURATION_BANISH_TAG,
        // The same four types the burst itself sweeps: everything this
        // feature touches is an outsider with a home plane to be sent
        // back to, which is exactly what makes the clause coherent.
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
        cr_ceiling: DESTROY_UNDEAD_CR_CEILING,
        effect: TurnEscalationEffect::Install {
            condition: Condition::Banished,
            // 1 minute, RAW, and no concentration to break — the
            // feature says so explicitly.
            timer: ConditionTimer::Rounds(10),
        },
        log_verb: "banishes to its home plane",
    }),
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
    // 1 minute, RAW.
    timer: ConditionTimer::Rounds(10),
    // No "instead of turned" rung: RAW gives this one no
    // remove-the-weak clause.
    escalation: None,
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
    // 1 minute, RAW.
    timer: ConditionTimer::Rounds(10),
    // No "instead of turned" rung: RAW gives this one no
    // remove-the-weak clause.
    escalation: None,
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
        let mut effects = grant_extra_action(caster_id);
        // 5e Way of the Drunken Master **Drunken Technique**: the same
        // bonus action also buys the exit. RAW hangs the clause off
        // *using Flurry of Blows*, so this is the site — there is no
        // cost or turn boundary the rider could otherwise be read at.
        // See `DRUNKEN_TECHNIQUE_TAG`.
        if encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_passive_feature(DRUNKEN_TECHNIQUE_TAG))
        {
            encounter.log(
                "  drunken technique: the monk reels out of reach — disengaging, +10 ft."
                    .to_string(),
            );
            effects.push(Box::new(GiveResource {
                actor_id: caster_id,
                // 10 ft, handed to the turn's budget rather than to
                // `speed()` — see `DRUNKEN_TECHNIQUE_TAG`.
                resource: Resource::Movement(10.),
            }));
            effects.push(Box::new(crate::engine::side_effects::SetDisengaging {
                actor_id: caster_id,
                disengaging: true,
            }));
        }
        effects
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
/// feature is a row on `FLAT_SPELL_DAMAGE_BONUSES`, the cohort that
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

/// Class-feature tag for the Draconic Bloodline Sorcerer's **Elemental
/// Affinity** (subclass level 6, damage half): "when you cast a spell
/// that deals damage of the type associated with your draconic
/// ancestry, you can add your Charisma modifier to one damage roll of
/// that spell."
///
/// A pure passive with no charge and no action surface — a row on
/// `FLAT_SPELL_DAMAGE_BONUSES` next to Empowered Evocation and Potent
/// Spellcasting. Where those two read the cast's *school* and *tier*,
/// this one reads the damage type the action declared, which is the
/// axis `CastContext::damage_types` was added for.
///
/// **The ancestry is fire**, chosen to match `has_draconic_resilience`
/// on the same template — the engine's Draconic Sorcerer already
/// resists fire, and a lv6 that resisted one element while empowering
/// another would be two half-features rather than one.
///
/// It is the sorcerer's answer to the wizard's Empowered Evocation, and
/// it trades in the opposite direction: Empowered Evocation covers a
/// whole school at every tier, this covers one damage type across every
/// school. A Draconic Sorcerer's Fireball, Burning Hands, Scorching
/// Ray, Fire Bolt, Wall of Fire, Delayed Blast Fireball, Immolation and
/// Investiture of Flame all carry it; their Lightning Bolt and Chain
/// Lightning do not, which is the choice the feature is asking the
/// player to make when they pick what to prepare.
///
/// RAW's other half — an hour of resistance to the same damage type for
/// a sorcery point — is left out: the engine has no lane for a cast to
/// grant its caster a typed resistance, and the damage half is the one
/// that reads at a site that already exists.
pub const ELEMENTAL_AFFINITY_TAG: &str = "sorcerer.elemental_affinity";

/// Per-rest charge for the Clockwork Soul Sorcerer's **Restore
/// Balance** (subclass level 1): a reaction that flattens somebody
/// else's advantage or disadvantage from up to 60 ft away.
///
/// No action of its own — the engine spends the reaction at the
/// roll-mode chokepoint, `EncounterInstance::steady_the_d20`, which
/// every attack roll and every saving throw in the game passes
/// through. See `cancel_mode_with_restore_balance` for the judgement
/// clause an auto-spending lane has to supply that RAW leaves to the
/// player.
///
/// RAW's pool is "a number of times equal to your proficiency bonus per
/// long rest" — four on this chassis. Two here, on the short-rest
/// cadence, for the reason `FEATURE_CHARGES`'s preamble gives: two is
/// the smallest pool that makes the spend a decision, and the
/// difference between two and four presses of a reaction the holder
/// only gets one of per round is mostly theoretical anyway.
pub const RESTORE_BALANCE_TAG: &str = "sorcerer.restore_balance";

/// Per-rest charge for the Clockwork Soul Sorcerer's **Bastion of Law**
/// (subclass level 6): a ward of protective dice laid on the sorcerer
/// or an ally within 30 ft.
///
/// RAW prices it at 1–5 sorcery points and hands over that many d8 as a
/// pool the warded creature spends to *reduce* incoming damage, die by
/// die, choosing how much to spend per blow. The engine has neither a
/// sorcery-point resource nor a per-blow spendable die pool, and both
/// absences point the same way: the closest thing it does have is
/// temporary hit points, which are a pool that incoming damage eats
/// through and that expires with the fight.
///
/// So the ward ships as 5d8 temporary hit points for one charge — RAW's
/// maximum spend, since a charge is not divisible — and loses the
/// clause that makes the RAW version interesting, which is the
/// warded creature choosing how many dice to burn on each hit. What
/// survives is the shape: a large, front-loaded shield the sorcerer can
/// put on somebody else, which is the only thing on the sorcerer
/// chassis that protects an ally at all.
pub const BASTION_OF_LAW_TAG: &str = "sorcerer.bastion_of_law";

/// Bastion of Law — Clockwork Soul Sorcerer action. 5d8 temporary hit
/// points on the sorcerer or one ally within 30 ft.
///
/// Targets `SingleActor` rather than `NoArgs`, unlike most of the
/// engine's self-buffs, because *who gets the ward* is the whole
/// decision the feature poses — a sorcerer who could only shield
/// themselves would be a worse Mage Armor.
///
/// Declines against a target already carrying temporary hit points
/// rather than overwriting them: 5e's temp HP don't stack ("choose
/// which to keep"), and an AI that re-runs its ladder every turn would
/// otherwise spend the charge replacing a full ward with a fresh one.
pub struct BastionOfLaw {}

impl Action for BastionOfLaw {
    fn name(&self) -> &str {
        "bastion of law"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bol", "bastion"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft on the 2.5 ft grid.
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
    fn is_heal(&self) -> bool {
        // Temporary hit points are what the AI's support lane is for,
        // and declaring the ward a heal is what puts it there.
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
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        if !feature_ready(encounter, caster_id, BASTION_OF_LAW_TAG) {
            return false;
        }
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        encounter.actors.get(&target_id).is_some_and(|t| {
            t.team() == caster.team() && t.is_combat_active() && t.temp_hp() == 0
        })
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
            actor.spend_feature(BASTION_OF_LAW_TAG);
        }
        // RAW's maximum spend — see `BASTION_OF_LAW_TAG` for why the
        // 1-to-5 choice collapses to its top end here.
        let amount = encounter.roll(&Dice::new(5, 8));
        let name = encounter.actor_name(target_id);
        encounter.log(format!(
            "  bastion of law: 5d8({}) temporary hit points ward {}.",
            amount, name
        ));
        vec![Box::new(GainTempHp {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static BASTION_OF_LAW: LazyLock<BastionOfLaw> = LazyLock::new(|| BastionOfLaw {});

/// Class-feature tag for the Circle of Wildfire Druid's **Enhanced
/// Bond** (subclass level 6): "while your spirit is summoned, ... when
/// you cast a spell that deals fire damage or restores hit points, roll
/// a d8 and add the number rolled to one damage or healing roll of that
/// spell."
///
/// Two halves at two chokepoints, sharing one gate
/// (`EncounterInstance::wildfire_bond_active`):
///
///   - the damage half is a row on `FLAT_SPELL_DAMAGE_BONUSES`, and the
///     only row there that rolls a die rather than reading a modifier;
///   - the healing half rides `slot_heal_effects` next to the Life
///     Cleric's Disciple of Life.
///
/// **The gate is a creature, not a flag**, and that is the subclass.
/// Every Wildfire feature is worth exactly what the spirit's position
/// makes it worth: a druid whose spirit is dead gets nothing, and a
/// druid whose spirit wandered past 60 ft gets nothing until it comes
/// back. Nothing else on the druid chassis asks the player to keep a
/// second body somewhere in particular — Conjure Animals' wolves fight
/// wherever they like and the druid's own spells never notice.
///
/// RAW's "spell of 1st level or higher" is not in the text of Enhanced
/// Bond, and we honour the omission: a Wildfire druid's Produce Flame
/// and Create Bonfire pick the die up. That is deliberately unlike
/// Disciple of Life, whose RAW *does* carry a level floor.
pub const ENHANCED_BOND_TAG: &str = "druid.enhanced_bond";

/// Marker tag carried by the **wildfire spirit** itself, not by the
/// druid — the needle `EncounterInstance::wildfire_bond_active` looks
/// for when it asks whether the bond is live.
///
/// A tag on the summon rather than a back-link on the summoner, because
/// a link would dangle in the two cases that actually happen: the
/// spirit dies and is re-summoned onto a fresh id, and two Wildfire
/// druids share a team (RAW does not care whose spirit is standing next
/// to whom). The tag search finds the right answer in both without any
/// despawn path having to remember to clean up.
pub const WILDFIRE_SPIRIT_TAG: &str = "druid.wildfire_spirit";

/// Class-feature tag for the Circle of Wildfire Druid's **Summon
/// Wildfire Spirit** (subclass level 2). One charge per short rest; see
/// `SummonWildfireSpirit` for the action.
pub const SUMMON_WILDFIRE_SPIRIT_TAG: &str = "druid.summon_wildfire_spirit";

/// Marker tag carried by the **tentacle of the deep** itself, and the
/// charge the Fathomless Warlock spends to call it — one tag doing both
/// jobs on two different creatures.
///
/// On the warlock it is a per-short-rest charge gating
/// `SummonTentacleOfTheDeep`. On the tentacle it is the beacon
/// `ClampScope::NearBeacon` searches the board for when Guardian Coil
/// asks whether the damaged creature is standing close enough to the
/// coils to be shielded by them.
///
/// The two readings do not collide, but only because
/// `friendly_beacon_within` skips the creature it is measuring *from*.
/// That exclusion was documented and missing for a while, and this tag
/// is what it cost: a warlock with no tentacle on the board stood zero
/// feet from a creature carrying the beacon — themselves — and clamped
/// their own damage with a feature whose whole premise is a second body
/// somewhere else. Pinned now by
/// `a_warlock_with_no_tentacle_does_not_shield_themselves`.
///
/// Sharing one tag rather than minting a second is still the same
/// reasoning `WILDFIRE_SPIRIT_TAG` gives for using a tag at all — the
/// fact lives on the creature, so a tentacle that dies and is re-called
/// needs nothing cleaned up. The Shepherd's totems go the other way and
/// mint a second tag, because their beacon is read to build a *target
/// list* rather than to answer yes or no, and a druid who was their own
/// beacon would put every ally near themselves inside the spirit's
/// aura.
pub const TENTACLE_OF_THE_DEEP_TAG: &str = "warlock.tentacle_of_the_deep";

/// Class-feature tag for the Fathomless Warlock's **Guardian Coil**
/// (subclass level 6): "when you or a creature you can see within 10
/// feet of your tentacle takes damage, you can use your reaction to
/// have the tentacle reduce that damage by 1d8."
///
/// A row on `REACTIVE_DAMAGE_CLAMPS`, and the row that made
/// `ClampScope::NearBeacon` necessary. Every other clamp in the cohort
/// measures from one of the two creatures already in the swing — the
/// defender shields themselves, or an ally near the defender steps in.
/// Guardian Coil measures from a *third* body that is in neither role,
/// which means a Fathomless warlock standing well back from the front
/// line can still shield whoever is on it, provided the tentacle is
/// there.
///
/// That is the subclass's whole argument, and it is the same argument
/// the Wildfire druid's Enhanced Bond makes from the other end: the
/// feature is worth exactly what the summon's position makes it worth.
/// The difference is which way the value points — Enhanced Bond pays
/// the summoner for keeping the spirit near *themselves*, Guardian Coil
/// pays them for putting the tentacle somewhere they are not.
///
/// One charge per short rest, on top of the reaction. RAW gives it
/// warlock-level uses per long rest; the engine's charge lane sizes
/// pools from `FEATURE_CHARGES`, and a second charge here would be a
/// second reaction the warlock does not have in the same round anyway.
pub const GUARDIAN_COIL_TAG: &str = "warlock.guardian_coil";

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
pub(crate) fn prime_self_condition(
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

/// Per-rest charge for the Peace Domain Cleric's **Emboldening Bond**
/// (subclass level 1): the cleric bonds a handful of creatures, and for
/// the next minute each of them adds a d4 to a roll that matters.
///
/// RAW's pool is "a number of times equal to your proficiency bonus per
/// long rest", which on this chassis is two; `FEATURE_CHARGES` says so,
/// and `SHORT_REST_FEATURES` gives them back per engagement the way
/// every other cleric charge on the roster does.
///
/// Deliberately *not* a Channel Divinity. RAW keeps Emboldening Bond
/// off that pool — it is the domain's level-1 feature and the Channel
/// Divinity arrives at level 2 — and the distinction is load-bearing
/// here: a Peace Cleric who bonds the party has still not spent the
/// Balm, which is what makes the domain a support build rather than a
/// single-press one.
pub const EMBOLDENING_BOND_TAG: &str = "cleric.emboldening_bond";

/// How many creatures one Emboldening Bond reaches, and how far.
///
/// RAW bonds "a number of creatures equal to your proficiency bonus" —
/// two on this chassis — "that you can see within 30 feet". Three here,
/// counting the cleric, because the cleric is one of the creatures RAW
/// lets you pick and a domain whose whole identity is the party
/// standing together should not have to choose between buffing itself
/// and buffing the front line.
const EMBOLDENING_BOND_TARGETS: usize = 3;

/// 30 ft on the 2.5 ft grid — the radius the bond is handed out over,
/// and the same one Preserve Life and every Channel Divinity burst on
/// the cleric chassis already use.
const EMBOLDENING_BOND_RADIUS: isize = 12;

/// Emboldening Bond — Peace Domain Cleric action. Installs
/// `Emboldened` on the cleric and the two nearest wounded-or-not allies
/// within 30 ft, for 10 rounds.
///
/// `NoArgs` rather than a target list, and the choice is the same one
/// Preserve Life makes: the engine's controllers have no channel for
/// "pick three of these", so the feature picks for them and the pick is
/// deterministic (nearest first, ties by id) so a seeded run
/// reproduces. Bonding the nearest bodies is also the right heuristic
/// for the feature RAW describes, whose payout clause is about the
/// bonded creatures being *near each other*.
///
/// Allies who already carry the bond are skipped rather than refreshed,
/// which keeps an AI that re-runs its ladder every turn from spending
/// both charges re-buffing the same three people — the same stacking
/// guard `Aided` exists for.
pub struct EmboldeningBond {}

impl Action for EmboldeningBond {
    fn name(&self) -> &str {
        "emboldening bond"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["eb", "bond", "embolden"]
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
        feature_ready(encounter, caster_id, EMBOLDENING_BOND_TAG)
            && !emboldening_bond_recipients(encounter, caster_id).is_empty()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let recipients = emboldening_bond_recipients(encounter, caster_id);
        if recipients.is_empty() {
            return Vec::new();
        }
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(EMBOLDENING_BOND_TAG);
        }
        let names: Vec<String> = recipients
            .iter()
            .map(|&id| encounter.actor_name(id))
            .collect();
        encounter.log(format!(
            "  emboldening bond: {} share a d4 for 10 rounds.",
            names.join(", ")
        ));
        recipients
            .into_iter()
            .map(|id| {
                Box::new(ApplyCondition {
                    actor_id: id,
                    condition: Condition::Emboldened,
                    // 1 minute, RAW.
                    timer: ConditionTimer::Rounds(10),
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

/// Who this cast of Emboldening Bond would reach — the cleric plus the
/// nearest un-bonded allies inside 30 ft, capped at
/// `EMBOLDENING_BOND_TARGETS`.
///
/// Shared by `custom_validate_input` and `side_effects` so the gate and
/// the resolution can never disagree about whether there is anybody
/// left to bond; a cast that would install nothing declines instead of
/// burning a charge. Sorted nearest-first with ties broken on the id so
/// a seeded run reproduces the same three names.
fn emboldening_bond_recipients(encounter: &EncounterInstance, caster_id: usize) -> Vec<usize> {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return Vec::new();
    };
    let team = caster.team();
    let origin = caster.location();
    let origin_size = get_tiles_from_size(caster.size());
    let mut ranked: Vec<(isize, usize)> = encounter
        .actors
        .iter()
        .filter(|(_, a)| {
            a.team() == team && a.is_combat_active() && !a.has_condition(Condition::Emboldened)
        })
        .filter_map(|(id, a)| {
            let gap = footprint_chebyshev(
                a.location(),
                get_tiles_from_size(a.size()),
                origin,
                origin_size,
            );
            (gap <= EMBOLDENING_BOND_RADIUS).then_some((gap, *id))
        })
        .collect();
    ranked.sort_unstable();
    ranked
        .into_iter()
        .take(EMBOLDENING_BOND_TARGETS)
        .map(|(_, id)| id)
        .collect()
}

pub static EMBOLDENING_BOND: LazyLock<EmboldeningBond> = LazyLock::new(|| EmboldeningBond {});

/// Channel Divinity charge for the Peace Domain Cleric's **Balm of
/// Peace** (subclass level 2): the cleric walks through the party and
/// everyone they pass is a little less hurt.
///
/// RAW is a *move*: "you can move up to your speed, without provoking
/// opportunity attacks, and when you move within 5 feet of any other
/// creature … you can restore a number of hit points to that creature
/// equal to 2d6 + your Wisdom modifier." Both halves ship, in the only
/// order the engine can express them — the heal lands on everyone
/// already inside 5 ft, and the cleric picks up `Disengaging` so the
/// walk that follows costs no opportunity attacks.
///
/// The divergence is that the engine cannot let an action *interleave*
/// with movement: a controller declares the action, the effects
/// resolve, and then the turn's remaining movement is spent. So the
/// heal is billed at the start of the walk rather than along it, which
/// is worth less than RAW (a cleric cannot tour the party) and is the
/// same compromise the engine's other move-and-do features take.
pub const BALM_OF_PEACE_TAG: &str = "cleric.balm_of_peace";

/// Balm of Peace — Peace Domain Cleric Channel Divinity, action. Heals
/// every ally within 5 ft (gap 1) for 2d6 + WIS, and leaves the cleric
/// Disengaging so the rest of the walk is free.
///
/// One shared roll for the whole burst, matching how every area effect
/// in the engine bills a die, and matching RAW's single "2d6 + your
/// Wisdom modifier" figure rather than a fresh roll per creature.
///
/// The radius is the feature's whole cost. Preserve Life reaches 30 ft
/// and heals to half; this reaches 5 ft and heals a flat lump, so a
/// Peace Cleric has to be standing *in* the party to spend it — which
/// on a d8 chassis is exactly the risk the domain is built around, and
/// the reason the Disengage rider is not a throwaway.
pub struct BalmOfPeace {}

impl Action for BalmOfPeace {
    fn name(&self) -> &str {
        "balm of peace"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bop", "cd-balm", "balm"]
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
        feature_ready(encounter, caster_id, BALM_OF_PEACE_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(BALM_OF_PEACE_TAG);
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let origin = caster.location();
        let wis = caster.ability_modifier(AbilityScoreType::Wisdom);
        let rolled = encounter.roll(&Dice::new(2, 6)) as i32;
        let amount = (rolled + wis).max(0) as u32;
        // 5 ft — the distance RAW says the cleric has to come within.
        let targets = encounter.ally_heal_burst_targets(caster_id, origin, 1);
        encounter.log(format!(
            "  balm of peace: 2d6({}){:+} = {} HP to {} nearby.",
            rolled,
            wis,
            amount,
            targets.len()
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = targets
            .into_iter()
            .map(|id| {
                Box::new(Heal {
                    actor_id: id,
                    amount,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect();
        // The other half of RAW's sentence: the walk that follows costs
        // no opportunity attacks. `Disengaging` is the engine's word for
        // that, and it is the same condition the Disengage action
        // installs — so a cleric who balms and then moves is already
        // covered by the movement lane's existing check.
        effects.push(Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Disengaging,
            timer: ConditionTimer::UntilStartOfNextTurn,
        }));
        effects
    }
}

pub static BALM_OF_PEACE: LazyLock<BalmOfPeace> = LazyLock::new(|| BalmOfPeace {});

/// Passive tag for the Peace Domain Cleric's **Protective Bond**
/// (subclass level 6): when a bonded creature takes damage while near
/// another bonded creature, the second one can spend its reaction to
/// teleport in and take the damage instead.
///
/// The third row on `EncounterInstance::DAMAGE_INTERPOSERS`, and the
/// widest — 30 ft, against the Crown Paladin's 5 and the Redemption
/// Paladin's 10. That reach is the domain in one number: the two
/// paladin features are about a champion standing over somebody, and
/// this one is about a party that is bonded whether or not it is
/// bunched up.
///
/// The bond half is enforced: the row carries a `covers` gate, so the
/// cleric takes a blow for a creature holding `Emboldened` and for
/// nobody else. Without it the widest reach on the cohort would also
/// have been the least discriminating — a cleric volunteering for
/// every blow landed on anyone within 30 feet, which is a strictly
/// better feature than the one RAW describes. It also ties the domain
/// together mechanically rather than only thematically: Protective
/// Bond is worth exactly as much as Emboldening Bond has been spent.
///
/// Two RAW clauses still don't ship. The interposer here is always the
/// *cleric*, where RAW lets any bonded creature take a blow for any
/// other — the lane keys off a passive tag, and the tag lives on the
/// cleric. And the teleport goes: the reaction moves the damage, not
/// the body. Both narrow the feature rather than widening it, and the
/// second is what keeps the wide radius honest — a cleric 30 ft away
/// eats the blow without ending up in the front line.
pub const PROTECTIVE_BOND_TAG: &str = "cleric.protective_bond";

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

/// Class-feature tag for the College of Eloquence Bard's **Unsettling
/// Words**. RAW spends one Bardic Inspiration use; we collapse the
/// die pool to a single per-rest charge the same way `CUTTING_WORDS_TAG`
/// does on the Lore chassis, and refresh it via `SHORT_REST_FEATURES`.
///
/// A separate charge from `BARDIC_INSPIRATION_TAG` rather than a second
/// draw on it. RAW they share a pool, but the pool in this engine is
/// one use deep, and a shared charge would mean an Eloquence bard who
/// unsettled somebody could not inspire anyone for the rest of the
/// fight — which reads as "the subclass took the bard's signature
/// feature away" rather than "the subclass gave it a second use".
pub const UNSETTLING_WORDS_TAG: &str = "bard.unsettling_words";

/// Class-feature tag for the College of Eloquence Bard's lv6
/// **Unfailing Inspiration**. Passive — it carries no charge and is
/// read via `has_passive_feature` off the *granting* bard by
/// `EncounterInstance::unfailing_inspiration_granter`, which finds them
/// through the `Inspired` back-link on whoever is holding the die.
///
/// The indirection is the feature: RAW's subject is the creature that
/// rolled, but the thing that decides whether their die survives is a
/// property of the bard who handed it over, and the holder may be
/// carrying a die from an ordinary bard, from Guidance, or from a
/// potion. Only a die with this bard's name on it comes back.
pub const UNFAILING_INSPIRATION_TAG: &str = "bard.unfailing_inspiration";

/// Class-feature tag for the College of Glamour Bard's lv3
/// **Enthralling Performance**. Once per short rest — the cadence every
/// other Channel-Divinity-shaped burst on the roster runs on, and the
/// one RAW gives it.
pub const ENTHRALLING_PERFORMANCE_TAG: &str = "bard.enthralling_performance";

/// Enthralling Performance — College of Glamour Bard lv3 subclass
/// feature. Action, once per short rest: every hostile in the burst
/// makes a Wisdom save against the bard's Charisma-anchored DC or is
/// Charmed for a minute.
///
/// A `TurnBurst` literal, which is the whole implementation — the
/// fourth subclass to land on that chassis after the Cleric's Turn
/// family, the Oathbreaker's Dreadful Aspect and the Conqueror's
/// Conquering Presence. The Nature Domain's Charm Animals and Plants is
/// the closest sibling: same installed condition, same save, same
/// cadence, and the difference is the `type_filter`. That one charms
/// beasts and plants; this one charms anything that can hear.
///
/// Two divergences from RAW, both in the same direction. The radius is
/// the chassis's 30 ft rather than RAW's 60 — `resolve_turn_burst` is
/// fixed at 30 and every row on it is, so widening it for one row would
/// mean a per-row radius for the benefit of a single subclass. And RAW
/// gates the performance on a full minute of playing beforehand, which
/// is a thing that happens before initiative is rolled; the engine has
/// no pre-combat phase, so the charge is the cost.
///
/// The duration is where it lands hardest. RAW's charm runs an hour and
/// ends early if the bard or their allies harm the target — a clause
/// that would end it on the round it was cast, since the point of
/// charming three enemies is that your side then kills them. Ten rounds
/// with no harm clause is the same collapse every Frighten on this
/// chassis already makes, and it is what makes the button worth an
/// Action.
pub static ENTHRALLING_PERFORMANCE: LazyLock<TurnBurst> = LazyLock::new(|| TurnBurst {
    name: "enthralling performance",
    aliases: &["ep", "enthrall", "perform"],
    tag: ENTHRALLING_PERFORMANCE_TAG,
    dc_ability: AbilityScoreType::Charisma,
    type_filter: |_| true,
    installed: Condition::Charmed,
    // 1 minute — see the duration note above.
    timer: ConditionTimer::Rounds(10),
    // No "instead of turned" rung: RAW gives this one no
    // remove-the-weak clause.
    escalation: None,
});

/// Temporary hit points one **Mantle of Inspiration** hands each
/// creature it covers.
///
/// RAW scales the grant with bard level on a 5 / 8 / 11 / 14 ladder;
/// five is the level-3 rung, and the bard chassis here is a level-5
/// full caster whose own Bardic Inspiration pool is sized off CHA 16.
/// Pinned as a named const rather than inlined because it is the number
/// the whole feature is priced around: five temp HP each is a little
/// under one goblin scimitar, so the mantle buys a party of four
/// roughly one absorbed hit — worth a bonus action and a die, and not
/// worth spending the pool on when nobody is being shot at.
const MANTLE_OF_INSPIRATION_TEMP_HP: u32 = 5;

/// 60 ft on the 2.5 ft grid — the radius Mantle of Inspiration covers,
/// matching the reach of the Bardic Inspiration die it is spending.
const MANTLE_OF_INSPIRATION_RADIUS: isize = 24;

/// Mantle of Inspiration — College of Glamour Bard lv3 subclass
/// feature. Bonus action; spends one Bardic Inspiration use and hands
/// `MANTLE_OF_INSPIRATION_TEMP_HP` temporary hit points to up to
/// `CHA modifier` allies within 60 ft.
///
/// The bard's signature feature, spent sideways. Bardic Inspiration
/// puts one die on one ally and waits for them to roll; the mantle
/// spends the same charge on the whole party at once and does not wait
/// for anything. That is the trade the College of Glamour is: the pool
/// is the same size it always was, and every use of it is now a choice
/// between one big effect later and four small ones now.
///
/// Three judgement calls RAW leaves open, made here:
///
///   - **Who.** The most wounded eligible allies first, by missing hit
///     points, ties broken on the lower id. RAW hands the bard the
///     choice; the most wounded is what a bard choosing on purpose
///     would choose, and it is how every other ally-facing picker in
///     the engine already chooses.
///   - **Not creatures the mantle cannot help.** An ally already
///     holding at least this much temp HP is skipped: `gain_temp_hp`
///     keeps the larger pool, so covering them would consume one of
///     the `CHA modifier` slots to change nothing. Skipping them is
///     what makes the count RAW's count rather than a count of bodies
///     that happened to be standing nearby.
///   - **The bard counts.** RAW's "a number of creatures within 60 feet
///     of you" does not exclude the caster, and a bard who is the one
///     being shot at should be able to cover themselves.
///
/// Left out: RAW's second clause lets each covered creature immediately
/// use its reaction to move up to its speed without provoking. The
/// engine's movement is a per-turn resource, so the nearest thing it
/// could do is hand out movement that becomes usable on the ally's
/// *next* turn — which is not the clause. The clause is about getting
/// out of the way now, and a version that arrives a round late is
/// further from RAW than not shipping it.
pub struct MantleOfInspiration {}

impl MantleOfInspiration {
    /// Ids the mantle would cover right now, most wounded first, capped
    /// at the bard's Charisma modifier.
    ///
    /// Shared by the validator and the resolver so the two cannot
    /// disagree about whether there is anybody to cover — a validator
    /// that said yes to an empty cohort would let the AI spend the
    /// bonus action and the die on nothing at all.
    fn covered_allies(encounter: &EncounterInstance, caster_id: usize) -> Vec<usize> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let cap = caster.ability_modifier(AbilityScoreType::Charisma).max(1) as usize;
        let center = caster.location();
        let mut candidates: Vec<(u32, usize)> = encounter
            .ally_burst_targets(caster_id, center, MANTLE_OF_INSPIRATION_RADIUS)
            .into_iter()
            .filter_map(|id| {
                let a = encounter.actors.get(&id)?;
                // Already better covered than the mantle could manage —
                // `gain_temp_hp` keeps the larger pool, so this slot
                // would buy nothing.
                if a.temp_hp() >= MANTLE_OF_INSPIRATION_TEMP_HP {
                    return None;
                }
                Some((a.max_hitpoints().saturating_sub(a.hitpoints()), id))
            })
            .collect();
        // Most wounded first; `ally_burst_targets` returns sorted ids,
        // and a stable sort by missing HP therefore leaves the lower id
        // ahead on a tie.
        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        candidates.into_iter().take(cap).map(|(_, id)| id).collect()
    }
}

impl Action for MantleOfInspiration {
    fn name(&self) -> &str {
        "mantle of inspiration"
    }
    fn aliases(&self) -> Vec<&str> {
        // Not "mantle": Crusader's Mantle is a spell on the paladin
        // list, and alias collisions resolve by list order rather than
        // erroring.
        vec!["moi", "mantle of inspo"]
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
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return false;
        };
        actor.is_combat_active()
            && actor.feature_available(BARDIC_INSPIRATION_TAG)
            && !Self::covered_allies(encounter, caster_id).is_empty()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let covered = Self::covered_allies(encounter, caster_id);
        if covered.is_empty() {
            return Vec::new();
        }
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(BARDIC_INSPIRATION_TAG);
        }
        encounter.log(format!(
            "  mantle of inspiration: {} creature(s) take heart ({} temp HP each).",
            covered.len(),
            MANTLE_OF_INSPIRATION_TEMP_HP
        ));
        covered
            .into_iter()
            .map(|id| {
                Box::new(GainTempHp {
                    actor_id: id,
                    amount: MANTLE_OF_INSPIRATION_TEMP_HP,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

pub static MANTLE_OF_INSPIRATION: LazyLock<MantleOfInspiration> =
    LazyLock::new(|| MantleOfInspiration {});

/// Unsettling Words — College of Eloquence Bard lv3 subclass feature.
/// Bonus action; one creature within 60 ft subtracts a Bardic
/// Inspiration die from its next saving throw.
///
/// The exact inverse of the bard's own signature feature, and built out
/// of the same parts: `Unsettled` is a flat −4 on
/// `CONDITION_SAVE_BONUSES` (d8 average floored, matching the +4
/// Precision Attack collapses a d8 to) and a row on `CONSUMED_ON_SAVE`,
/// which is what makes it one save deep rather than a standing debuff.
///
/// Sibling to Cutting Words on the Lore chassis — same cost, same
/// range, same once-per-rest charge, same `hostile_target_feature_ready`
/// gate — and deliberately on the other axis. Cutting Words fouls what
/// the target *does* (disadvantage on their next attack roll); this
/// fouls what the target *survives*, which is the axis the bard's own
/// spell list attacks. A bard who unsettles a target and then lands
/// Hold Person is playing a combination no other bard on the roster
/// has: the −4 is worth about a fifth of the DC.
///
/// RAW's window is "before the end of your next turn", which is
/// modelled as `Rounds(2)` plus the consume-on-save row rather than
/// `UntilStartOfNextTurn` — see `Condition::Unsettled` for why the
/// longer timer is the accurate one here.
pub struct UnsettlingWords {}

impl Action for UnsettlingWords {
    fn name(&self) -> &str {
        "unsettling words"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["uw", "unsettle"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles. Matches Bardic Inspiration and Cutting
        // Words — the bard's voice carries the same distance whoever
        // it is aimed at.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        // RAW: "a creature you can see within 60 feet of you."
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
        // Charge + hostile-target + already-Unsettled dedup, all on the
        // shared gate Cutting Words / Nature's Wrath / Abjure Enemy
        // ride. The dedup matters more here than on most of the cohort:
        // the penalty is spent by the first save the target rolls, so
        // re-applying it while it is still up would burn the bard's
        // only charge to replace a die with an identical die.
        hostile_target_feature_ready(
            encounter,
            caster_id,
            target_ids,
            UNSETTLING_WORDS_TAG,
            Condition::Unsettled,
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
            actor.spend_feature(UNSETTLING_WORDS_TAG);
        }
        encounter
            .log("  unsettling words: the bard's barb shakes the target's nerve.".to_string());
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Unsettled,
            timer: ConditionTimer::Rounds(2),
        })]
    }
}

pub static UNSETTLING_WORDS: LazyLock<UnsettlingWords> = LazyLock::new(|| UnsettlingWords {});

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

/// Class-feature tag for the Fighter's **Maneuvering Attack** Battle
/// Master maneuver. Spends from the shared superiority pool alongside
/// every other entry on `BATTLE_MASTER_MANEUVERS`.
pub const MANEUVERING_ATTACK_TAG: &str = "fighter.maneuvering_attack";

/// Maneuvering Attack — Fighter Battle Master maneuver. Bonus-action
/// prime; the next melee weapon hit adds a superiority die to the damage
/// roll and lets one ally use its reaction to move up to half its speed.
///
/// RAW: *"you choose a friendly creature who can see or hear you. That
/// creature can use its reaction to move up to half its speed without
/// provoking opportunity attacks from the target of your attack."* The
/// prime shape is the same one Trip / Menacing / Distracting use — every
/// one of those is an on-hit maneuver too, and this one is no different
/// for having its rider land on an ally.
///
/// **Who moves, and which way, is the engine's own choice**, and it has
/// to be: the prompt resolves one action name per line and has no second
/// slot for "and my comrade steps here". The rule the follow-up handler
/// applies is written out at `FollowUpEffect::AllyReposition` — it is
/// the ally that is *out of position*, and RAW's "more advantageous"
/// means toward the fight for a creature that fights up close and away
/// from it for one that does not.
pub static MANEUVERING_ATTACK: LazyLock<ManeuverPrime> = LazyLock::new(|| ManeuverPrime {
    name: "maneuvering attack",
    aliases: &["maneuver", "mna"],
    tag: MANEUVERING_ATTACK_TAG,
    prime_condition: Condition::ManeuveringAttacking,
    timer: ConditionTimer::Rounds(2),
    log_line: "  maneuvering attack: the fighter's next melee hit opens a lane for an ally.",
});

/// Class-feature tag for the Fighter's **Evasive Footwork** Battle
/// Master maneuver. Spends from the shared superiority pool like every
/// other entry on `BATTLE_MASTER_MANEUVERS`.
///
/// The only maneuver in the suite with no action attached to it. RAW:
/// *"When you move, you can expend one superiority die, rolling the die
/// and adding the number rolled to your AC until you stop moving."*
/// There is nothing for a turn-ordered action list to offer and nothing
/// for the AI to pick, so the feature fires where its trigger actually
/// lives: the two reaction dispatchers that can attack a creature
/// mid-step — the opportunity attack as the fighter leaves a reach
/// envelope and the readied shot as they enter one — on the step that
/// provokes, and only once the swing is certain.
///
/// **The lazy spend is the feature.** A fighter who declares this at the
/// top of a move buys +4 AC against nothing most of the time; RAW's
/// player spends it knowing full well they are about to be swung at,
/// because the trigger and the knowledge arrive together. Firing at the
/// dispatchers reproduces exactly that: the die is spent when a swing is
/// already committed to, and a move that provokes nothing costs nothing.
/// One die covers the whole move however many reactors it walks past and
/// whichever dispatchers they answer through, which is RAW's "until you
/// stop moving" — the once-per-turn ledger is what carries that across
/// the steps.
pub const EVASIVE_FOOTWORK_TAG: &str = "fighter.evasive_footwork";

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
        // RAW: *"That creature can immediately use **its** reaction to
        // make one weapon attack."* The reaction is the ally's own and
        // is the whole price of the maneuver on their side, so an ally
        // who has already Shielded, parried or opportunity-attacked this
        // round cannot be commanded — the order arrives and there is
        // nothing left to answer it with.
        //
        // This gate used to be absent and the effect used to hand the
        // ally a fresh reaction on the way past, which meant the price
        // was never actually paid: an ally with a reaction kept it (grant
        // one, spend one) and an ally without one was given a spare. The
        // comments called the grant RAW, which it is not — RAW gives the
        // ally nothing but permission. Same reading as Maneuvering
        // Attack's, which charges the reaction for the same sentence.
        if !ally.can_consume_resource(Resource::Reaction) {
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
                " (no visible enemy; the order finds no mark)"
            }
        ));
        // Install the help-grant on the ally — advantage on next swing
        // against the chosen enemy (if any). Before the swing below, so
        // the directed attack rolls with it.
        if let Some(target_id) = strike_target
            && let Some(ally) = encounter.actors.get_mut(&ally_id)
        {
            ally.set_help_grant(Some(HelpGrant {
                helper_id: caster_id,
                against: target_id,
            }));
        }
        // The ally spends its own reaction on the swing —
        // `try_fire_directed_attack` gates on it and consumes it, and
        // `custom_validate_input` above has already refused an ally that
        // has none. Nothing is granted here: RAW hands the ally
        // permission, not a resource.
        if !crate::engine::attack::try_fire_directed_attack(
            encounter,
            ally_id,
            caster_id,
            "command",
        ) {
            // Nothing in the ally's reach, or no melee weapon to swing.
            // The reaction stays unspent — the geometry is what refused
            // the swing, and RAW charges the reaction for the attack
            // rather than for the order.
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

/// Warlock Eldritch Invocation — **Grasp of Hadar** (XGtE). *"Once on
/// each of your turns when you hit a creature with your Eldritch Blast,
/// you can move that creature up to 10 feet closer to you."* Four tiles
/// of pull on this engine's 2.5-ft grid, toward the warlock's own
/// square, resolved through `PullActor` — the same forced-movement loop
/// Thorn Whip and Lightning Lure use, so a target dragged into a web or
/// a wall of fire enters the area exactly as it would if something else
/// had dragged it.
///
/// **The once-per-turn clause is the invocation.** Repelling Blast's
/// push has no such limit and fires on every beam that lands, because
/// RAW's push is per-hit; the grasp is explicitly "once on each of your
/// turns", so a level-17 warlock landing four beams pulls once and not
/// sixteen tiles. That gate rides `ONCE_PER_TURN_RIDER_TAGS` — the
/// ledger cleared at `reset_for_new_round` — rather than a charge,
/// because the invocation is permanent and free and it is the *window*
/// that is scarce.
///
/// **It is mutually exclusive with Repelling Blast**, and not by RAW: a
/// warlock may take both and choose per beam which to apply. The engine
/// has no channel to ask, and applying both to one beam is a push and a
/// pull of the same magnitude — the target ends where it started, and
/// the two invocations cancel into nothing. So the templates carry one
/// or the other, and `no_warlock_takes_both_blast_movement_invocations`
/// keeps it that way; the cast site itself lets Repelling Blast win, so
/// a hand-built fixture holding both still gets the unconditional
/// clause rather than an alternating jitter.
pub const GRASP_OF_HADAR_TAG: &str = "warlock.grasp_of_hadar";

/// Warlock Eldritch Invocation — **Lance of Lethargy** (XGtE). *"Once on
/// each of your turns when you hit a creature with your Eldritch Blast,
/// you can reduce that creature's speed by 10 feet until the end of your
/// next turn."*
///
/// The whole effect is `Condition::Hobbled`, which already exists and is
/// already exactly this sentence — it is the Slow weapon mastery's
/// −10 ft, and RAW's *"the reduction doesn't exceed 10 feet"* falls out
/// of a condition being a set membership rather than a stacking counter.
/// Sharing the flag is right rather than convenient: two rules that say
/// the same thing about the same target should not be two flags that can
/// disagree.
///
/// Same once-per-turn ledger as Grasp of Hadar above, for the same RAW
/// clause. Unlike the grasp it composes with Repelling Blast rather than
/// contradicting it, and the pair is the reason the baseline warlock
/// carries both: the push opens ten feet and the lance takes away the
/// ten feet that would have closed it again.
pub const LANCE_OF_LETHARGY_TAG: &str = "warlock.lance_of_lethargy";

/// Warlock Eldritch Invocation — **Eldritch Mind**. Passive feature: the
/// holder rolls with advantage on Constitution saving throws to maintain
/// concentration. Read at the concentration save chokepoint
/// (`EncounterInstance::roll_concentration_save`) which the DealDamage
/// pipeline drives whenever a concentrating actor takes damage and lives.
/// Permanent passive — never consumed. Add this tag to a warlock
/// template's `features` set to install it.
pub const ELDRITCH_MIND_TAG: &str = "warlock.eldritch_mind";

/// **Devil's Sight** — the warlock Eldritch Invocation, and the trait
/// of the same name that every devil in the bestiary carries. "You can
/// see normally in darkness, both magical and nonmagical, to a distance
/// of 120 feet."
///
/// The magical half is the load-bearing one, and it is the only counter
/// 5e offers to the Darkness spell's "a creature with darkvision can't
/// see through this darkness". Read by
/// `ActorInstance::has_devils_sight`, which
/// `EncounterInstance::perceived_light` consults before it declares a
/// darkened tile unviewable. The nonmagical half needs nothing extra:
/// every holder also has darkvision, and the lighting layer's
/// one-rung upgrade already covers ordinary dark.
///
/// This is the invocation the engine had been substituting for. Before
/// the lighting layer existed the warlock's Darkness cast blinded its
/// own caster along with everyone else, and the devils that RAW gives
/// Devil's Sight got, in the Lemure template's words, "generous
/// Darkvision 120 since the engine doesn't yet distinguish magical vs
/// mundane darkness". Both approximations can now say what they meant.
///
/// Permanent passive — never consumed. Add the tag to a template's
/// `features` set to install it.
pub const DEVILS_SIGHT_TAG: &str = "warlock.devils_sight";

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

/// Declarative chassis for a **feature summon**: a class feature that
/// spends a per-rest charge to put one friendly body on the board and
/// then gets out of the way.
///
/// Three features arrived at the same ninety lines independently — the
/// Beast Master's Ranger's Companion, the Circle of Wildfire's Summon
/// Wildfire Spirit, and the Fathomless warlock's Tentacle of the Deep —
/// and they differed in eight values: what it is called, what it
/// summons, how big that is, how far to look for a free tile, which
/// per-rest charge it spends, and whether it costs an Action or a Bonus
/// Action. Everything else was identical, down to the order the charge
/// is spent in. This is those eight values.
///
/// **What every feature summon has in common, and why it isn't a
/// spell.** No spell slot, so nothing competes with the caster's other
/// magic. No concentration, so the summoner can hold a control spell at
/// the same time — the structural difference from Conjure Animals and
/// the reason all three subclasses play as "a caster with a body" rather
/// than as pet classes. And no `Conjured` marker, so the summon outlives
/// whatever its summoner is concentrating on and stays until something
/// kills it. What it costs instead is a once-per-rest charge and a turn.
///
/// Summons that *are* spells stay where they are, in `spells.rs`, built
/// out of `spawn_adjacent_summons` directly: they have slot costs and
/// concentration anchors this chassis deliberately has no field for.
pub struct FeatureSummon {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    /// The per-short-rest charge this spends, and the gate
    /// `custom_validate_input` reads.
    pub tag: &'static str,
    /// What appears. A `&'static LazyLock` rather than a
    /// `&'static CreatureTemplate` because every creature template in
    /// the engine is lazily built, and the deref happens once, at
    /// resolution.
    pub template: &'static LazyLock<crate::actors::actor_template::CreatureTemplate>,
    /// The summon's footprint, which decides what counts as a free tile.
    pub size: crate::engine::types::Size,
    /// How far from the summoner to look for one.
    pub search_radius: isize,
    /// Instance-id band, kept distinct per summon so two of them on one
    /// board don't collide. The summon *spells* live at 90+.
    pub base_instance_id: usize,
    /// `Resource::Action` or `Resource::BonusAction`. RAW disagrees
    /// across the three, and the disagreement is a real balance lever:
    /// a bonus-action summon lands on round one alongside an attack, an
    /// Action summon costs the turn it arrives on.
    pub cost_resource: Resource,
}

impl Action for FeatureSummon {
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
    // `deals_damage` needs no override: it defaults to `is_harmful()`,
    // and the summon hurts nobody directly — whatever it does, it does
    // on its own turns.
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
        vec![self.cost_resource]
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // The charge, and then the part the AI shouldn't have to guess
        // at: whether a body of this size fits anywhere nearby.
        feature_ready(encounter, caster_id, self.tag)
            && encounter
                .find_adjacent_spawn(caster_id, self.size, self.search_radius)
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
        // Charge first, spawn second. The validator has already
        // confirmed a free tile, and a spawn that fails because the
        // arena filled in between should still cost the summoner the
        // call rather than leaving a charge to retry with the same
        // result.
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(self.tag);
        }
        crate::actions::spells::spawn_adjacent_summons(
            encounter,
            caster_id,
            self.template,
            self.size,
            1,
            self.search_radius,
            self.base_instance_id,
            self.display_name,
        );
        // Nothing to queue: no `Conjured` marker and no concentration
        // anchor. That absence is the feature.
        Vec::new()
    }
}

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
pub static RANGERS_COMPANION: FeatureSummon = FeatureSummon {
    display_name: "ranger's companion",
    aliases: &["companion", "beast", "rc"],
    tag: RANGERS_COMPANION_TAG,
    template: &crate::actors::creatures::wolves::RANGERS_COMPANION_TEMPLATE,
    size: crate::engine::types::Size::Medium,
    search_radius: 2,
    base_instance_id: 70,
    cost_resource: Resource::Action,
};

/// Summon Wildfire Spirit — Circle of Wildfire Druid level-2 action.
/// Once per short rest, a Small elemental appears beside the druid on
/// their team and stays until it drops.
///
/// **The spirit is not the payload — its position is.** A wildfire
/// spirit fights about as well as a goblin, and a druid who summoned
/// one purely for the extra body would have been better off casting
/// Conjure Animals for two wolves. What the summon actually buys is
/// Enhanced Bond: a d8 on every fire spell and every heal the druid
/// casts while the spirit stands within 60 ft. So the interesting turn
/// is not this one, it is every turn afterwards, and the question the
/// subclass keeps asking is where the second body should be standing.
///
/// RAW spends a druid's Wild Shape use rather than a charge of its own;
/// the engine's Wild Shape lane belongs to the Moon Druid's
/// `COMBAT_WILD_SHAPE_TAG` and this chassis does not carry it, so a
/// charge is the honest translation — it is the same "once per short
/// rest, and you only get one body" shape RAW is charging for.
pub static SUMMON_WILDFIRE_SPIRIT: FeatureSummon = FeatureSummon {
    display_name: "summon wildfire spirit",
    aliases: &["wildfire spirit", "spirit", "sws"],
    tag: SUMMON_WILDFIRE_SPIRIT_TAG,
    template: &crate::actors::creatures::wildfire_spirits::WILDFIRE_SPIRIT_TEMPLATE,
    size: crate::engine::types::Size::Small,
    search_radius: 2,
    base_instance_id: 71,
    cost_resource: Resource::Action,
};

/// Tentacle of the Deep — Fathomless Warlock level-1 bonus action.
/// Once per short rest, a rooted spectral limb rises beside the warlock
/// on their team.
///
/// **A bonus action, which is what separates it from the roster's other
/// two feature summons.** The Ranger's Companion and the wildfire
/// spirit each cost their summoner a whole Action — a turn not spent
/// fighting, paid up front against a body that fights later. The
/// tentacle costs the warlock a bonus action they had no other use for
/// on round one, which means a Fathomless warlock opens the fight with
/// a tentacle on the board *and* an Eldritch Blast already fired. RAW
/// prices it that way deliberately: the tentacle is weak enough that
/// making it expensive would have made it not worth calling.
///
/// **What it actually buys is a second place to measure from.** The
/// slam reaches 10 ft from the tentacle, and Guardian Coil shields
/// anything within 10 ft of the tentacle — so the warlock is choosing
/// where a ten-foot bubble of cold and protection sits, and it does not
/// have to be anywhere near them. See `GUARDIAN_COIL_TAG`.
///
/// Search radius 3 rather than the siblings' 2: RAW places the tentacle
/// anywhere within 60 ft, and since it never moves again, where it
/// lands is the only placement decision the feature ever makes. A
/// slightly wider search is the cheapest approximation of that — the
/// engine has no destination picker the AI could answer, which is the
/// same wall the Eldritch Knight's Arcane Charge runs into.
pub static SUMMON_TENTACLE_OF_THE_DEEP: FeatureSummon = FeatureSummon {
    display_name: "tentacle of the deep",
    aliases: &["tentacle", "totd"],
    tag: TENTACLE_OF_THE_DEEP_TAG,
    template: &crate::actors::creatures::deep_tentacles::TENTACLE_OF_THE_DEEP_TEMPLATE,
    size: crate::engine::types::Size::Medium,
    search_radius: 3,
    base_instance_id: 72,
    cost_resource: Resource::BonusAction,
};

/// Marker tag carried by the **drake companion** itself, and the charge
/// the Drakewarden Ranger spends to call it — one tag doing both jobs
/// on two different creatures, the same economy
/// `TENTACLE_OF_THE_DEEP_TAG` makes and for the same reasons.
///
/// On the ranger it is a per-short-rest charge gating
/// `SUMMON_DRAKE_COMPANION`. On the drake it is the beacon Bond of Fang
/// and Scale searches the board for when it asks whether the ranger is
/// swinging within 30 ft of their drake.
///
/// The two readings never collide: `friendly_beacon_within` skips the
/// subject itself, and the charge is only ever read through
/// `feature_ready` on the summoner. RAW's bond is permanent and the
/// summon is a once-per-long-rest ritual; the short-rest cadence here
/// matches every other subclass charge on the roster, so a Drakewarden
/// arrives at the next engagement with a subclass.
pub const DRAKE_COMPANION_TAG: &str = "ranger.drake_companion";

/// Summon Drake Companion — Drakewarden Ranger level-3 action. Once per
/// short rest, a Small dragon appears beside the ranger on their team
/// and stays until it drops.
///
/// **The third body-summoning feature on the roster and the only one
/// whose body is worth having for itself.** The wildfire spirit exists
/// so the druid's spells get a die; the tentacle exists so a ten-foot
/// bubble sits somewhere useful. The drake is a competent CR-1
/// combatant with a cone of fire on top — and it *also* carries the
/// die (Bond of Fang and Scale). Which is why it costs an Action rather
/// than the tentacle's bonus action: a Drakewarden who opened with a
/// free drake would be strictly ahead of every other conclave from
/// round one.
///
/// Search radius 2, like the Ranger's Companion and the wildfire
/// spirit: the drake walks, so where it lands matters far less than
/// where it goes.
pub static SUMMON_DRAKE_COMPANION: FeatureSummon = FeatureSummon {
    display_name: "summon drake",
    aliases: &["drake", "drake companion", "sdc"],
    tag: DRAKE_COMPANION_TAG,
    template: &crate::actors::creatures::drakes::DRAKE_COMPANION_TEMPLATE,
    size: crate::engine::types::Size::Small,
    search_radius: 2,
    base_instance_id: 77,
    cost_resource: Resource::Action,
};

/// 5e Circle of the Shepherd Druid **Spirit Totem** (subclass level 2)
/// — the charge, carried by the druid. One press per short rest, and
/// the same tag on both builds because a druid only ever has one totem
/// to call.
///
/// RAW spends a Wild Shape use, which on this chassis belongs to the
/// Moon Druid's `COMBAT_WILD_SHAPE_TAG`. A per-short-rest charge is the
/// same translation `SUMMON_WILDFIRE_SPIRIT_TAG` makes for the same
/// reason, and it is the same shape RAW is charging for: once per
/// engagement, and you only get one spirit.
pub const SPIRIT_TOTEM_TAG: &str = "druid.spirit_totem";

/// Marker tag carried by the **bear spirit** totem itself. The needle
/// `SpiritTotem`'s arrival payload counts allies against, and the reason
/// the beacon is not the same tag as the charge: the druid carries
/// `SPIRIT_TOTEM_TAG`, so sharing one tag would have made the druid
/// their own beacon and put every ally standing near *them* inside an
/// aura that belongs to the spirit.
///
/// That collision is not hypothetical — it is the bug
/// `friendly_beacon_within` was carrying until the Drakewarden's bond
/// tripped over it. Two tags cost one line and cannot have it.
pub const BEAR_SPIRIT_TAG: &str = "druid.bear_spirit";

/// Marker tag carried by the **unicorn spirit** totem itself. Read at
/// `slot_heal_effects`, where the spill-over half of the Shepherd's
/// aura lives — see `UNICORN_SPIRIT_SPILL`.
pub const UNICORN_SPIRIT_TAG: &str = "druid.unicorn_spirit";

/// The aura a Circle of the Shepherd totem projects, in tiles on the
/// 2.5 ft grid — RAW's 30 ft, and the same number the Drakewarden's
/// leash uses.
pub const SPIRIT_TOTEM_AURA_TILES: isize = 12;

/// What the unicorn's aura spills onto each ally inside it whenever the
/// druid spends a slot on a heal — RAW's "hit points equal to your
/// druid level", on the level-9 chassis every druid template targets.
///
/// Flat rather than rolled because RAW's clause is flat, and the
/// flatness is what makes the totem worth planting somewhere: the druid
/// knows exactly what a second body in the aura is worth before
/// deciding where to put the spirit.
pub const UNICORN_SPIRIT_SPILL: u32 = 9;

/// Temporary hit points the bear's arrival hands to every ally inside
/// the aura — RAW's "5 + your druid level" on the level-9 chassis.
pub const BEAR_SPIRIT_WARD: u32 = 14;

/// The footprint a totem occupies. Medium on both flavors, and stated
/// once so the validator's "does a body fit here" probe and the spawn
/// that follows it cannot disagree — a mismatch there is a feature that
/// validates and then fails to land, spending the charge for nothing.
const SPIRIT_TOTEM_SIZE: crate::engine::types::Size = crate::engine::types::Size::Medium;

/// How far from the druid the spawn search looks for a free tile.
///
/// 3 rather than the roster's usual 2, for the reason the tentacle's
/// is: RAW places the spirit anywhere within 60 ft, it never moves
/// again, and where it lands is the only placement decision the feature
/// makes. A slightly wider search is the cheapest approximation the
/// engine has, absent a destination picker the AI could answer.
const SPIRIT_TOTEM_SEARCH_RADIUS: isize = 3;

/// Spirit Totem — Circle of the Shepherd Druid action (subclass level
/// 2). Once per short rest: an incorporeal spirit appears near the
/// druid and projects a thirty-foot aura for the rest of the fight.
///
/// **A summon whose body is irrelevant.** The three feature summons
/// already on the roster are all creatures first — the drake bites, the
/// tentacle lashes, the wildfire spirit shoots — and their auras or
/// bonds are riders on top of a thing that fights. A totem has no
/// attack at all, so the only question the feature ever asks is where
/// thirty feet of aura should sit. That is why this is not a
/// `FeatureSummon`: the shared chassis spawns a body and stops, and the
/// bear's whole payout is the thing that happens *as* it arrives.
///
/// The two flavors differ in when they pay:
///
///   - **Bear** hands every ally inside the aura a shield of temporary
///     hit points the instant it appears, and then does nothing. Its
///     decision is timing.
///   - **Unicorn** does nothing on arrival and adds a flat spill to
///     every healing spell the druid casts thereafter, to each ally
///     inside the aura. Its decision is placement — see
///     `UNICORN_SPIRIT_TAG`.
///
/// See `SPIRIT_TOTEM_SEARCH_RADIUS` for why the spawn search is wider
/// than the roster's usual.
pub struct SpiritTotem {
    display_name: &'static str,
    aliases: &'static [&'static str],
    template: &'static LazyLock<crate::actors::actor_template::CreatureTemplate>,
    base_instance_id: usize,
    /// Temporary hit points handed to every ally inside the aura the
    /// moment the spirit lands, or 0 for the flavors whose payout comes
    /// later. The bear's whole feature; the unicorn's zero.
    arrival_ward: u32,
}

impl Action for SpiritTotem {
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
        // The charge, and then the part the AI shouldn't have to guess
        // at: whether a body fits anywhere nearby.
        feature_ready(encounter, caster_id, SPIRIT_TOTEM_TAG)
            && encounter
                .find_adjacent_spawn(caster_id, SPIRIT_TOTEM_SIZE, SPIRIT_TOTEM_SEARCH_RADIUS)
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
        // Charge first, spawn second — the same order every feature
        // summon uses, and for the same reason: a spawn that fails
        // because the arena filled in between should still cost the
        // druid the call rather than leaving a charge to retry with.
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(SPIRIT_TOTEM_TAG);
        }
        let spawned = crate::actions::spells::spawn_adjacent_summons(
            encounter,
            caster_id,
            self.template,
            SPIRIT_TOTEM_SIZE,
            1,
            SPIRIT_TOTEM_SEARCH_RADIUS,
            self.base_instance_id,
            self.display_name,
        );
        if self.arrival_ward == 0 {
            return Vec::new();
        }
        let Some(&totem_id) = spawned.first() else {
            // The aura has no centre, so there is nobody to ward. The
            // charge is still gone, which is the same call the summons
            // make.
            return Vec::new();
        };
        let Some(origin) = encounter.actors.get(&totem_id).map(|t| t.location()) else {
            return Vec::new();
        };
        // Measured from the spirit rather than from the druid, which is
        // the whole difference between an aura and a self-buff.
        let warded = encounter.ally_heal_burst_targets(caster_id, origin, SPIRIT_TOTEM_AURA_TILES);
        encounter.log(format!(
            "  {}: {} temporary hit points to {} inside the aura.",
            self.display_name,
            self.arrival_ward,
            warded.len()
        ));
        warded
            .into_iter()
            .map(|actor_id| {
                Box::new(crate::engine::side_effects::GainTempHp {
                    actor_id,
                    amount: self.arrival_ward,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

/// Spirit Totem (Bear) — the ward-on-arrival flavor. See `SpiritTotem`.
pub static SPIRIT_TOTEM_BEAR: LazyLock<SpiritTotem> = LazyLock::new(|| SpiritTotem {
    display_name: "bear spirit",
    aliases: &["bear", "bear totem", "spirit totem"],
    template: &crate::actors::creatures::spirit_totems::BEAR_SPIRIT_TOTEM_TEMPLATE,
    base_instance_id: 78,
    arrival_ward: BEAR_SPIRIT_WARD,
});

/// Spirit Totem (Unicorn) — the spill-over flavor. See `SpiritTotem`
/// and `UNICORN_SPIRIT_TAG`.
pub static SPIRIT_TOTEM_UNICORN: LazyLock<SpiritTotem> = LazyLock::new(|| SpiritTotem {
    display_name: "unicorn spirit",
    aliases: &["unicorn", "unicorn totem", "spirit totem"],
    template: &crate::actors::creatures::spirit_totems::UNICORN_SPIRIT_TOTEM_TEMPLATE,
    base_instance_id: 79,
    // Nothing on arrival: the unicorn's whole payout is downstream, at
    // `slot_heal_effects`.
    arrival_ward: 0,
});

/// Every feature summon on the roster, in one place.
///
/// The sweeps that have to hold across all of them — distinct instance
/// bands, distinct charges, and a rest that gives each charge back —
/// used to read a list written out inside the test itself, which meant
/// a fourth summon was covered only if whoever added it remembered to
/// edit an assertion in another module. That is the same failure
/// `pc_template_families` exists to prevent, and it had already been
/// paid once: the tentacle shipped with no refresh at all, and the
/// sweep only caught it because someone had remembered.
///
/// **Adding a feature summon is one line here and nothing else.**
pub const FEATURE_SUMMONS: &[&FeatureSummon] = &[
    &RANGERS_COMPANION,
    &SUMMON_WILDFIRE_SPIRIT,
    &SUMMON_TENTACLE_OF_THE_DEEP,
    &SUMMON_DRAKE_COMPANION,
    &SUMMON_FLAMETHROWER_CANNON,
    &SUMMON_FORCE_BALLISTA_CANNON,
    &SUMMON_PROTECTOR_CANNON,
    &SUMMON_STEEL_DEFENDER,
];

/// The instance-id band every summoning *spell* starts from. Feature
/// summons live below it and spell summons at or above, so a party that
/// puts a companion, a cannon and a Conjure Animals pack on the board
/// at once still gets distinct display names for all of them.
pub const SPELL_SUMMON_BAND_FLOOR: usize = 80;

/// 5e Drakewarden Ranger **Bond of Fang and Scale** (subclass level 7):
/// "while your drake is summoned, you gain... your weapon attacks deal
/// an extra 1d6 damage of the type chosen for your drake."
///
/// A row on `ONCE_PER_TURN_WEAPON_DIE_RIDERS`, and the first row on
/// that cohort whose gate is a question about the *board* rather than
/// about the swinger. Every other rider there asks what the attacker
/// carries or what condition they are under; this one asks whether a
/// particular second body is still alive and still nearby, which is
/// what widened `caster_gate` to take the encounter.
///
/// The 30-ft leash is not RAW — RAW asks only that the drake be
/// summoned, wherever it is. It is here because the alternative reading
/// makes the feature free: a Drakewarden would park the drake in a
/// corner out of reach and collect the die for the rest of the fight,
/// which is the opposite of what a bond is supposed to cost. The leash
/// is the same distance the Wildfire druid's Enhanced Bond measures and
/// it does the same job — it makes the summon's *position* the price of
/// the summoner's damage.
///
/// The rider's type is fire, matching the drake's chosen element on
/// `DRAKE_COMPANION_TEMPLATE`. Fixed rather than read off the drake
/// because the element is a template constant on both sides; the day a
/// second drake element ships, the two rows move together.
///
/// RAW's other half — resistance to the drake's element while it is
/// summoned — is left out, and the ellipsis in the quote above is
/// hiding it. The two passive-resistance cohorts key off template flags
/// and held conditions respectively, and neither can ask where a second
/// body is standing; a resistance that ignored the leash would be a
/// Drakewarden collecting half of fire damage for the whole fight for
/// having pressed a button once. That is a third cohort's worth of
/// widening for one row, and the die is the half of the feature that
/// makes the leash worth pulling on.
pub const BOND_OF_FANG_AND_SCALE_TAG: &str = "ranger.bond_of_fang_and_scale";

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
    fn self_burst_radius(&self) -> Option<isize> {
        // RAW: "each hostile creature within 30 feet of you" — the same
        // twelve tiles the burst below resolves at.
        Some(TURN_BURST_RADIUS)
    }
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
            SaveDamagePolicy::HalfOnSave,
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

/// 5e Oath of Redemption Paladin level-3 subclass Channel Divinity —
/// **Rebuke the Violent**. Class-feature tag; refreshed on a short rest
/// via `SHORT_REST_FEATURES`. RAW: as a reaction when a creature within 30
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
/// Ships on `REDEMPTION_PALADIN_TEMPLATE`, which is the oath RAW gives
/// it to. It used to ship on the Devotion paladin, at a level Devotion
/// doesn't have it either — Devotion's level 15 is Purity of Spirit,
/// which is now what that template carries (`PURITY_OF_SPIRIT_TAG`).
/// Composes cleanly with the rest of the Redemption kit: the oath's
/// whole shape is "absorb what the enemy does and hand it back", and
/// Rebuke is the handing-back half in one press.
pub const REBUKE_THE_VIOLENT_TAG: &str = "paladin.rebuke_the_violent";

/// Rebuke the Violent — Oath of Redemption Paladin lv3 Channel Divinity.
/// Once-per-short-rest single-target 4d10 radiant damage burst: the
/// target (within 30ft, 12 tiles on our 2.5ft grid) rolls a WIS save
/// vs the paladin's CHA-anchored DC (8 + prof + CHA mod). On save the
/// target takes half damage; on fail, full damage. Routes through the
/// shared `resolve_single_target_burst_save_for_half` helper — the
/// spend + save + roll + damage dance lives in one place, mirrored by
/// Wrath of the Storm / Infernal Rebuke on the same helper.
///
/// Range gate (12 tiles = 30ft RAW) matches the RAW envelope — the
/// Redemption paladin's mid-range radiant retort. Pairs naturally with
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

/// 5e Arcana Domain Cleric **Arcane Abjuration**, second clause (level
/// 5): a celestial, elemental, fey or fiend weak enough to clear the CR
/// ceiling is *banished* by a failed save instead of merely frightened.
///
/// Always-on passive, exactly like `DESTROY_UNDEAD_TAG` and paired with
/// `ARCANE_ABJURATION_TAG` the same way: that one is the per-rest charge
/// that gates whether the Channel Divinity can be pressed at all, and
/// this one gates what a failed save against it costs. Both must be
/// present, so an Arcana Cleric who has spent their Channel Divinity
/// banishes nothing.
///
/// It is the sibling half of Destroy Undead across the whole cleric
/// roster: between an Arcana Cleric and any other cleric, a party can
/// remove every extraplanar creature type in the engine from a fight
/// with one Channel Divinity press each. That complementarity is the
/// domain's argument for existing, and until the engine grew an
/// off-board actor lane it was an argument for a feature that only did
/// half of what it said.
pub const ARCANE_ABJURATION_BANISH_TAG: &str = "cleric.arcane_abjuration_banish";

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
    if let Some(actor) = encounter.actors.get_mut(&caster_id) {
        actor.spend_feature(feature_tag);
    }
    install_ally_burst_condition(
        encounter,
        caster_id,
        radius,
        max_targets,
        condition,
        timer,
        label,
        "rallied with resolve",
        // Zealous Presence is a shout in RAW's flavour and a burst in
        // its rules text — no hearing clause — so the gate stays open.
        false,
    )
}

/// The charge-free core of `spend_feature_and_install_ally_burst`:
/// sweep combat-active allies within `radius` of the caster (the caster
/// included), cap the sweep at `max_targets`, and hand each of them
/// `condition` for `timer`.
///
/// Split out from its charging sibling when Countercharm arrived,
/// because Countercharm is the same burst with no charge to spend: RAW
/// costs the bard their Action and nothing else, every round, for as
/// long as they are willing to keep singing. Threading a sentinel
/// "spend nothing" tag through the existing helper would have made the
/// charge lane a special case of itself; this way the charge is one
/// line that happens before a shared body, which is what it is.
///
/// `verb` is the tail of the log line — the two callers rally allies
/// and steady them respectively, and the difference is worth a reader's
/// eye in the transcript.
///
/// `audible` drops allies who cannot hear the caster. Countercharm's
/// RAW clause is "any friendly creatures within 30 feet of you that can
/// hear you", and a performance is the one kind of buff a deafened ally
/// genuinely misses; see `ActorInstance::can_hear` for the other four
/// effects on that lane.
#[allow(clippy::too_many_arguments)]
fn install_ally_burst_condition(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    radius: isize,
    max_targets: usize,
    condition: Condition,
    timer: ConditionTimer,
    label: &'static str,
    verb: &'static str,
    audible: bool,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(caster_loc) = encounter.actors.get(&caster_id).map(|a| a.location()) else {
        return Vec::new();
    };
    let mut targets = encounter.ally_burst_targets(caster_id, caster_loc, radius);
    if audible {
        targets.retain(|id| encounter.actors.get(id).is_some_and(|a| a.can_hear()));
    }
    targets.truncate(max_targets);
    let n = targets.len();
    encounter.log(format!(
        "  {}: {} {} {}.",
        label,
        n,
        if n == 1 { "ally" } else { "allies" },
        verb
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
/// The **swimming** half of the feature is a second, separate surface.
/// RAW's Roving is "your Speed increases by 10 feet… you also have a
/// Climb Speed and a Swim Speed equal to your Speed", and the second
/// sentence used to be flavour because the engine had no water in it.
/// This tag is a row on `SWIM_SPEED_SOURCES`, so every ranger on the
/// roster crosses `TerrainType::Water` at no surcharge and swings in it
/// without the disadvantage everyone else takes.
///
/// That is the one place Roving and Land's Stride come apart, and they
/// come apart the way RAW says: Land's Stride is scoped to "nonmagical
/// difficult terrain", which a lake is not, so a ranger swims for free
/// because of *this* feature and crosses rubble for free because of
/// that one. The two ship on the same chassis and are not
/// interchangeable.
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

/// 5e **Land's Stride** (Ranger level 8; Circle of the Land Druid level
/// 6). Passive: moving through nonmagical difficult terrain costs the
/// holder no extra movement.
///
/// This is the first *content* row on the difficult-terrain-immunity
/// lane, and the reason that lane exists. The engine has had difficult
/// terrain since `TerrainType::DifficultTerrain` landed — the terrain
/// generator scatters it across ~8% of the floor and the pathfinder
/// charges double for every tile of it — but nothing in the game could
/// ignore it, so the tax was uniform and therefore invisible: every
/// actor paid it, so no build ever routed around or through it
/// differently from any other. Land's Stride is the feature that makes
/// the rubble mean something, and `DIFFICULT_TERRAIN_IMMUNITIES` is the
/// cohort it reads through.
///
/// RAW carries two more clauses that don't ship: the "can't be impeded
/// by nonmagical plants" half needs a plant-origin tag on terrain the
/// generator doesn't emit, and the ranger's advantage-on-saves-against
/// -magical-plants half needs a per-effect-origin save filter the
/// blanket save-mode lanes don't express. The movement half is the
/// load-bearing one on a tactical grid, and it folds through the shared
/// cohort as a one-line entry.
///
/// The engine models the RAW "nonmagical" qualifier as "all difficult
/// terrain" for the same reason `item_template.rs` collapses the Boots
/// of the Winterlands' ice-and-snow clause: the terrain enum carries no
/// origin, so there is no magical difficult terrain to exclude. Every
/// tile the generator scatters is rubble and undergrowth, which is
/// exactly what the RAW clause covers.
///
/// Ships on `RANGER_TEMPLATE` (and inherits to every ranger subclass
/// via `..RANGER_TEMPLATE.clone()`) and on `LAND_DRUID_TEMPLATE`, both
/// at or above their strict RAW level gates, for the same reason
/// `ROVING_TAG` (RAW lv6) already rides the CR-1 baseline ranger —
/// class templates target a balanced playable level, not lockstep PHB
/// progression.
///
/// Always-on passive; no per-rest charge and no condition gate. The tag
/// lives in the actor's `features` pool, not in `SHORT_REST_FEATURES` /
/// long-rest tables — nothing consumes it and nothing refreshes it.
pub const LANDS_STRIDE_TAG: &str = "shared.lands_stride";

/// A **swimming speed** — the monster-stat-block line ("Speed 30 ft.,
/// swim 40 ft.") rather than a class feature, which is why it sits on
/// the `shared.` namespace next to Land's Stride rather than under a
/// class.
///
/// Read by `ActorInstance::has_swim_speed`, and through it by both
/// halves of 5e's Underwater Combat rules: the movement surcharge a
/// swimmer doesn't pay (`WATER_SURCHARGE_IMMUNITIES`) and the melee
/// disadvantage a swimmer doesn't take ("a creature that doesn't have a
/// swimming speed… has disadvantage on the attack roll").
///
/// A boolean tag rather than a `swim_speed: f32` field on
/// `CreatureTemplate` because nothing in the engine reads the
/// *magnitude*. Both rules that exist ask whether the creature has one
/// at all, and the engine has no third dimension for a separate
/// swimming budget to be spent in — a shark in the water moves at its
/// `speed`, and the only thing its swimming speed buys is that the
/// water stops charging double for it. A number would be a field every
/// aquatic template had to pick a value for and no code would ever
/// compare.
///
/// Ships on the aquatic bestiary — see `AQUATIC_TEMPLATES` in
/// `creatures::mod`'s sweep, which is what stops a new sea monster from
/// being added without one.
///
/// Always-on passive; no per-rest charge and no condition gate.
pub const SWIM_SPEED_TAG: &str = "shared.swim_speed";

/// **Undead Fortitude** — the zombie trait, and the reason a zombie
/// takes two rounds to put down when the arithmetic says one.
///
/// RAW: *"If damage reduces the zombie to 0 Hit Points, it makes a
/// Constitution saving throw (DC 5 plus the damage taken) unless the
/// damage is Radiant or from a Critical Hit. On a successful save, the
/// zombie drops to 1 Hit Point instead."*
///
/// On the `shared.` namespace beside the swimming speed, and for the
/// same reason: it is a line of a monster sheet. Read by
/// `EncounterInstance::try_undead_fortitude`, which is called from the
/// damage chokepoint's two "reduced to 0" branches — the Downed one a
/// player character takes and the Killed one a monster does.
///
/// The DC scales with the blow, which is the whole design: a zombie
/// finished off by a dagger rolls against DC 8 and gets up, and one
/// hit by a greataxe crit rolls against nothing because the crit
/// clause exempts it. The engine keeps the first half of that sentence
/// and not the second — see `try_undead_fortitude` for why the crit
/// exemption is a scope cut and what it would take to ship.
///
/// Always-on passive; no per-rest charge and no condition gate. A
/// zombie can get up as many times as its saves let it, which is RAW
/// and is exactly how a zombie is supposed to feel.
pub const UNDEAD_FORTITUDE_TAG: &str = "shared.undead_fortitude";

/// **Breathes underwater** — the stat-block line that says the water
/// will not drown this creature, on the same `shared.` namespace as the
/// swimming speed beside it and for the same reason: it is a fact about
/// a monster sheet, not a class feature.
///
/// Read by `ActorInstance::breathes_underwater`, and through it by the
/// one rule that asks — `EncounterInstance::can_breathe`, the gate on
/// the round-end suffocation tick in `engine::breath`. A creature
/// carrying this tag can stand at the bottom of a lake indefinitely;
/// one without it is on a clock the moment it goes under.
///
/// Named for what it *does* rather than after any one RAW trait,
/// because four different SRD 5.2 clauses grant it and no one of their
/// names is right for the other three:
///
///   - **Amphibious** — "the aboleth can breathe air and water".
///     Twenty-two stat blocks, counting the sixteen amphibious dragons.
///   - **Water Breathing** — "the shark can breathe only underwater".
///     The exact opposite trait in flavour and the same answer to this
///     question. Nine stat blocks: the three sharks, the two octopuses,
///     the two seahorses, the piranha and its swarm.
///   - **Hold Breath** — "the crocodile can hold its breath for 15
///     minutes", "the hydra can hold its breath for 1 hour". Fifteen
///     minutes is 150 rounds; no encounter this engine has run is a
///     tenth of that, so within a fight the creature does not run out.
///   - **Limited Amphibiousness** — the Sahuagin's "it needs to be
///     submerged at least once every 4 hours to avoid suffocating
///     outside water". Four hours is 2,400 rounds.
///
/// Splitting those would mean four tags, four template lists and four
/// sweeps, all feeding one predicate that cannot tell them apart. The
/// honest summary of all four is "being under water is never this
/// creature's problem", which is what the tag is named for.
///
/// The clause it deliberately does **not** carry is the sharks' other
/// half — "*only* underwater", which RAW says makes them suffocate in
/// air. See `engine::breath` for why that half is a scope cut and what
/// it would take to ship.
///
/// What this is *not* is a swimming speed: the Crocodile and the
/// Hippopotamus hold their breath and cross water at the ordinary
/// surcharge, and the Green Hag breathes water without a swim line at
/// all. `SWIM_SPEED_TAG` and this one overlap heavily and neither
/// implies the other — see `creatures::underwater_breathing_templates`
/// for the roster.
///
/// Always-on passive; no per-rest charge and no condition gate.
pub const UNDERWATER_BREATHING_TAG: &str = "shared.underwater_breathing";


/// Monster trait: "its weapon attacks are magical".
///
/// RAW writes this line onto the celestials, the greater fiends and a
/// handful of ancient monstrosities — the creatures whose claws are
/// supposed to cut through each other's resistance. It matters most
/// when two of them meet: a balor and a marilith both resist nonmagical
/// steel, and without this tag two demons would spend a fight halving
/// each other's damage for no reason RAW recognises.
///
/// A tag on `CreatureTemplate::features` rather than a `bool` field
/// because it belongs to the same family as every other one-line stat
/// block clause the engine already stores that way, and because
/// `MAGICAL_ATTACK_SOURCES` can then read it through the same
/// predicate shape as the rest of the cohort.
pub const MAGICAL_ATTACKS_TAG: &str = "shared.magical_attacks";

/// Monster trait **Flyby**: *"the creature doesn't provoke an
/// opportunity attack when it flies out of an enemy's reach."*
///
/// The aerial harasser's whole shtick, and until natural flight existed
/// there was nothing for it to be about. Three stat blocks carry the
/// trait — the giant owl, the giant vulture, the pteranodon — and all
/// three docstrings recorded it as dropped, in the same words and for
/// the same reason: "the engine doesn't surface OAs on
/// Disengage-equivalent moves". It does; what it did not have was a
/// creature that could be said to be flying.
///
/// Gated on `is_airborne` at the point of use rather than baked in here,
/// because RAW's clause says *"when it flies"*. A pteranodon that has
/// been knocked out of the sky by the general flying rule is walking,
/// and walking away from a halberd provokes like anything else does.
///
/// Read by `EncounterInstance::dispatch_opportunity_attacks`, in the
/// mover-side suppression lane beside Disengage. Deliberately the
/// blanket lane and not the Swashbuckler's surgical one: Disengage and
/// Flyby both say "nobody gets to swing", where Fancy Footwork picks
/// out particular reactors.
///
/// Always-on passive; no per-rest charge and no action cost. This is
/// what separates it from Disengage, which is the same effect for a
/// whole action — the owl gets it for free, every turn, forever, and
/// that is the trait.
pub const FLYBY_TAG: &str = "shared.flyby";

/// Monster trait **Agile**: *"the creature doesn't provoke an
/// Opportunity Attack when it moves out of an enemy's reach."*
///
/// Flyby's ground-bound twin, and the wider of the two: where Flyby is
/// gated on *"when it flies"* and lapses the moment the owl is walking,
/// Agile asks nothing at all — the deer is simply never worth swinging
/// at as it goes past. SRD 5.2 prints it on the skittish prey animals
/// whose entire defence is that they leave.
///
/// A separate tag rather than a second meaning for `FLYBY_TAG` because
/// the airborne gate is load-bearing on that one and absent on this
/// one, and a shared tag would have to drop the gate for both — which
/// would hand the pteranodon a clause RAW does not give it. Two tags on
/// the same cohort table is the cheaper of the two shapes.
///
/// Read by `EncounterInstance::dispatch_opportunity_attacks` through
/// `MOVER_OA_SUPPRESSORS`, in the blanket lane beside Disengage and
/// Flyby.
pub const AGILE_TAG: &str = "shared.agile";

/// 5e Monk **Ki-Empowered Strikes** (class level 6): "your unarmed
/// strikes count as magical for the purpose of overcoming resistance
/// and immunity to nonmagical attacks."
///
/// The only feature on the player side that magics a creature's own
/// body rather than an object it is holding, which is why it is a
/// separate tag from the item and spell lanes below: a monk who drops
/// their weapon still punches through a wraith.
pub const KI_EMPOWERED_STRIKES_TAG: &str = "monk.ki_empowered_strikes";


/// 5e Scout Rogue **Superior Mobility** (subclass level 9, XGtE).
/// Passive: the scout's walking speed increases by 10 feet, and they
/// also gain climbing and swimming speeds matching that walking speed.
/// The flat +10 ft walking-speed bump is read at the shared
/// `passive_feature_speed_bonus` chokepoint in `condition_speed_bonus`
/// next to the barbarian's Fast Movement (+10 ft), the ranger's Roving
/// (+5 ft), and the monk's Unarmored Movement (+10 ft) — one lookup
/// table, one source of truth.
///
/// The **swimming** half is a second, separate surface: this tag is a
/// row on `SWIM_SPEED_SOURCES`, so a Scout crosses `TerrainType::Water`
/// at no surcharge and swings underwater without the disadvantage
/// everyone else takes — the only build on the player roster that does.
/// The climbing half still folds into `speed()`, and will until the
/// engine grows a third dimension for it to mean anything in.
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

/// **The** slot-cast heal chokepoint. Every spell that spends a slot
/// to restore hit points ends here: hand it the caster, the ids it
/// picked, the amount it rolled and the slot level, and it emits the
/// `Heal` side-effects with every rider that rides a slot heal already
/// folded in.
///
/// Two riders live here today — the Life Cleric's Disciple of Life
/// amplification and the Stars Druid's Chalice overflow — and the
/// reason they live *here* rather than at the call sites is that the
/// call sites got it wrong. Disciple of Life was spliced in by hand at
/// five of the ten eligible sites. The baseline Cleric carries Prayer
/// of Healing, Healing Spirit and Aura of Vitality, so a Life Cleric —
/// whose entire subclass is "your healing is bigger" — was casting
/// three of its own healing spells for exactly baseline numbers, and
/// every test passed, because nothing anywhere compared the two lists.
///
/// The amount is the *pre-rider* roll. Callers keep their own log line
/// for the dice (the formats differ per spell, and Circle of Mortality
/// has to swap the dice before they are rolled at all), and this logs
/// the amplification separately when it fires — which also means the
/// bonus is stated once per cast rather than spliced into a per-target
/// format string.
///
/// Not every `Heal` in the game belongs here. Mass Heal and Power Word
/// Heal both restore a creature to full by construction, so an
/// amplifier would only overflow a pool that was already big enough;
/// Resurrection, True Resurrection and Wish are revivals rather than
/// heals; Vampiric Touch drains onto the caster. Those keep their bare
/// `Heal`, and `the_slot_heal_chokepoint_covers_every_amplifiable_heal`
/// pins the list so a new healing spell has to make the choice
/// deliberately instead of by omission.
pub fn slot_heal_effects(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    targets: &[usize],
    amount: u32,
    spell_slot_lvl: u32,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let bonus = encounter
        .actors
        .get(&caster_id)
        .map(|caster| disciple_of_life_bonus(caster, spell_slot_lvl))
        .unwrap_or(0);
    if bonus > 0 && !targets.is_empty() {
        encounter.log(format!("  disciple of life: +{} HP each", bonus));
    }
    // 5e Circle of Wildfire Druid **Enhanced Bond**, healing half: a d8
    // on the healing roll of any spell the druid casts while their
    // spirit stands within 60 ft. The damage half of the same sentence
    // is a row on `FLAT_SPELL_DAMAGE_BONUSES`; they share the
    // `wildfire_bond_active` gate and nothing else.
    //
    // Added to the shared `amount` rather than to one recipient, which
    // is the same reading `roll_empowered_sum` gives RAW's "one damage
    // roll" on a burst: the spell rolls its dice once and every target
    // is paid out of that one roll, so a die added to the roll reaches
    // all of them and is still added exactly once.
    let bond_bonus = {
        let active = encounter
            .actors
            .get(&caster_id)
            .is_some_and(|caster| encounter.wildfire_bond_active(caster));
        if active && !targets.is_empty() {
            let rolled = encounter.roll(&Dice::new(1, 8));
            encounter.log(format!("  enhanced bond: +{} HP", rolled));
            rolled
        } else {
            0
        }
    };
    let bonus = bonus + bond_bonus;
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = targets
        .iter()
        .map(|&actor_id| {
            Box::new(Heal {
                actor_id,
                amount: amount + bonus,
            }) as Box<dyn ApplicableSideEffect>
        })
        .collect();
    // The Chalice spills once per cast, not once per target — RAW's
    // clause is "whenever you cast a spell… one creature", and a mass
    // heal that paid one slot should not pay out six overflows.
    effects.extend(starry_chalice_overflow(
        encounter,
        caster_id,
        targets,
        spell_slot_lvl,
    ));
    effects.extend(unicorn_spirit_spill(encounter, caster_id, targets));
    effects
}

/// 5e Circle of the Shepherd Druid **Unicorn Spirit** (subclass level
/// 2), the spill-over clause: "whenever you cast a spell that restores
/// hit points, each creature of your choice in the aura also regains
/// hit points equal to your druid level."
///
/// The third rider on the slot-heal chokepoint, and the only one that
/// widens the *target list* rather than the amount. Disciple of Life
/// and Enhanced Bond both make the spell's own recipients heal for
/// more; this one pays creatures the spell never touched, which is why
/// it emits its own `Heal` effects instead of moving a number.
///
/// **Measured from the spirit, not from the druid**, which is the whole
/// difference between this and every other party-wide heal in the game.
/// A Shepherd druid who plants the unicorn in the middle of the front
/// line and then falls back is still healing the front line — the
/// feature does not care where the caster is, only where the totem is,
/// and that is the decision the subclass exists to pose.
///
/// The `targets` the spell already picked are excluded rather than paid
/// twice: RAW's "each creature of your choice" is a fresh selection,
/// and stacking the spill onto the spell's own recipient would make a
/// single-target Cure Wounds on a body standing beside the unicorn
/// strictly better than the same cast anywhere else, for no reason RAW
/// gives.
fn unicorn_spirit_spill(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    targets: &[usize],
) -> Vec<Box<dyn ApplicableSideEffect>> {
    // RAW's clause is "whenever **you** cast a spell that restores hit
    // points", so the spill is the Shepherd's own, not a bonus every
    // healer on the team collects for standing near somebody else's
    // totem. Gated on the caster carrying the subclass the same way
    // Enhanced Bond gates on the druid holding `ENHANCED_BOND_TAG`
    // rather than merely on a spirit being on the board.
    //
    // `has_passive_feature` rather than `feature_available`: the tag is
    // also the summon's charge, and a druid who has spent it is still a
    // Shepherd.
    let Some(team) = encounter
        .actors
        .get(&caster_id)
        .filter(|c| c.has_passive_feature(SPIRIT_TOTEM_TAG))
        .map(|c| c.team())
    else {
        return Vec::new();
    };
    let Some(origin) = encounter
        .actors
        .values()
        .find(|a| {
            a.team() == team && a.is_combat_active() && a.has_passive_feature(UNICORN_SPIRIT_TAG)
        })
        .map(|a| a.location())
    else {
        return Vec::new();
    };
    let spilled: Vec<usize> = encounter
        .ally_heal_burst_targets(caster_id, origin, SPIRIT_TOTEM_AURA_TILES)
        .into_iter()
        .filter(|id| !targets.contains(id))
        .collect();
    if spilled.is_empty() {
        return Vec::new();
    }
    encounter.log(format!(
        "  unicorn spirit: +{} HP to {} more inside the aura",
        UNICORN_SPIRIT_SPILL,
        spilled.len()
    ));
    spilled
        .into_iter()
        .map(|actor_id| {
            Box::new(Heal {
                actor_id,
                amount: UNICORN_SPIRIT_SPILL,
            }) as Box<dyn ApplicableSideEffect>
        })
        .collect()
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

/// **The** heal-dice roll. The roll-side twin of `slot_heal_effects`:
/// that one owns everything that happens to a heal *after* the dice,
/// and this one owns the dice themselves.
///
/// Returns `(raw, maxed)` — the total to feed the spell's own log line
/// and whether Circle of Mortality substituted the max face for every
/// die. `maxed` goes straight to `circle_of_mortality_log_suffix`.
///
/// The RAW clause is per-die, not per-target: "whenever you would
/// normally roll one or more dice to restore hit points… instead use
/// the highest number possible for each die." On a shared-roll mass
/// heal the coherent read is that if any die is being applied to a
/// 0-HP creature then that die maxes, and since all the dice are
/// shared, all of them do. That is why the gate takes the whole target
/// list and asks `any` — a burst that catches a downed ally floors the
/// entire burst, and a burst of healthy allies rolls normally.
///
/// It exists for the same reason its twin does. The substitution was
/// written out by hand at three sites and missing from three more that
/// roll dice and sit on the same cleric chassis — Prayer of Healing,
/// Healing Spirit and Aura of Vitality — so a Grave Cleric's signature
/// "drag them back from the edge with certainty" did nothing at all on
/// half the spells it should have.
pub fn roll_heal_dice(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    targets: &[usize],
    dice: &Dice,
) -> (u32, bool) {
    let maxed = encounter
        .actors
        .get(&caster_id)
        .is_some_and(|c| c.has_passive_feature(CIRCLE_OF_MORTALITY_TAG))
        && targets
            .iter()
            .any(|id| encounter.actors.get(id).is_some_and(|a| a.hitpoints() == 0));
    if maxed {
        (dice.max_roll(), true)
    } else {
        (encounter.roll(dice), false)
    }
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
/// RAW pairs the lightning resistance with "you can breathe underwater
/// and you gain a swimming speed equal to your walking speed", and all
/// three clauses now land. The swim half is a row on
/// `SWIM_SPEED_SOURCES`, which lifts the water movement surcharge and
/// 5e's melee Underwater Combat penalty; the breathing half is a row on
/// `UNDERWATER_BREATH_SOURCES`, which takes the barbarian off the
/// suffocation clock in `engine::breath`.
///
/// It shipped with the resistance alone and a note saying the other two
/// had no combat surface, which was true of an engine with no water
/// tiles and no breath clock. Both arrived; the note outlived them by
/// several features, which is the ordinary way a cut becomes a gap.
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
/// matching the way `INURED_TO_UNDEATH_TAG` ships without the
/// max-HP-can't-be-reduced clause. (Its Sea sibling no longer belongs
/// on that list — the engine grew water and a breath clock, and Storm
/// Soul (Sea) now ships whole.)
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
/// rest Channel Divinity charge.
///
/// The next attack against the cursed target has advantage via the
/// shared `Condition::grants_advantage_to_attackers` cohort, and lands
/// against **vulnerability to every damage type** via the
/// `TYPED_VULNERABILITY_CONDITIONS` cohort — RAW's "the creature has
/// vulnerability to all of that attack's damage". The attack that
/// cashes the curse in also spends it; the `UntilStartOfNextTurn`
/// timer is the backstop for a curse nobody cashes in.
///
/// The doubling is the feature's whole point at the table: a Channel
/// Divinity charge spent here is worth roughly the party's biggest
/// single hit of the round, so the cleric's read is "who is about to
/// swing hardest, and can they reach?" rather than "who do I want to
/// hit more often".
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
    /// Queues a `StartConcentration`. Declared so the AI's
    /// summon and area-control rungs can price this cast before
    /// trading a landed concentration effect for an unlanded one
    /// — and so the assertion in `Action::execute` stays quiet.
    fn holds_concentration(&self) -> bool {
        true
    }
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

/// Tag for the Circle of Stars Druid's **Starry Form** (subclass level
/// 2). RAW spends a use of Wild Shape, of which the circle has two per
/// short rest; this is one, riding `SHORT_REST_FEATURES`, so the cadence
/// stays RAW's even though the count does not. Left at one deliberately
/// rather than sized up through `FEATURE_CHARGES` — the constellation
/// runs ten rounds, which outlasts most fights here, so a second charge
/// would rarely buy a second transformation and would mostly just
/// dissolve the choice below. The same call Bladesong and Giant's Might
/// make, for the same reason.
///
/// One tag for all three shapes rather than three tags, which is what
/// makes the choice a choice: a druid gets one constellation per short
/// rest and has to decide, before the fight tells them what they
/// needed, whether this is a round they want an extra attack, an extra
/// heal, or the concentration to survive being hit.
pub const STARRY_FORM_TAG: &str = "druid.starry_form";

/// Shared shape for a bonus-action self-prime that belongs to a
/// *mutually exclusive family* — one where installing any member has to
/// strip the others. Config-driven for the same reason `TurnBurst` and
/// `ManeuverPrime` are: the members differ in a name, a condition and a
/// line of flavor text, and in nothing else at all.
///
/// The one thing this shape does that `ManeuverPrime` does not is strip
/// the siblings. Two features need that today, for two different
/// reasons that arrive at the same code:
///
///   - **Starry Form** (Circle of Stars Druid, subclass level 2).
///     Re-transforming replaces the constellation RAW, and without the
///     strip a druid with two charges (or a Dispel that missed) could
///     stand in two at once.
///   - **Arcane Shot** (Arcane Archer Fighter, subclass level 3). RAW
///     nocks one option onto one arrow; the pool is two charges deep,
///     so without the strip an archer could bank a Grasping Arrow on
///     turn one, a Shadow Arrow on turn two, and cash *both* riders on
///     a single shot.
///
/// The second case is why the family is a field rather than a hardcoded
/// walk of `STARRY_FORMS`: the strip is a property of the family, and
/// there is now more than one family.
pub struct ExclusivePrime {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    /// Per-rest charge the whole family shares. One tag for every
    /// member, which is what makes the choice a choice.
    pub tag: &'static str,
    /// Which member of the family this action installs. Must be a
    /// member of `family` — `every_exclusive_prime_is_in_its_own_family`
    /// pins that, since a prime missing from its own slice would install
    /// fine and simply never be stripped by its siblings.
    pub prime_condition: Condition,
    /// Every member of the family, this one included. Walked on install
    /// to strip whichever siblings the caster is currently holding.
    pub family: &'static [Condition],
    /// How long the installed prime rides. Starry Form runs a minute;
    /// an Arcane Shot sits on the bow until it is fired.
    pub timer: ConditionTimer,
    pub log_line: &'static str,
}

impl Action for ExclusivePrime {
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
        // `feature_prime_ready` also refuses to re-install a prime the
        // caster is already holding, which is the right answer for every
        // family member: the charge is the whole cost, and spending it
        // to refresh a timer that has nine rounds left is never what the
        // player meant.
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
        prime_exclusive_self_condition(
            encounter,
            caster_id,
            self.tag,
            self.prime_condition,
            self.family,
            self.timer,
            self.log_line,
        )
    }
}

/// Spend the family's shared charge, strip whichever siblings the
/// caster is currently holding, and install `prime`.
///
/// Split out of `ExclusivePrime::side_effects` so the strip-then-install
/// order is stated once. The `RemoveCondition`s are emitted *ahead* of
/// the `ApplyCondition` in the returned vector because side-effects
/// apply in order — reversing them would strip the prime that was just
/// installed.
///
/// The sibling filter compares against `prime` rather than trusting the
/// family slice to exclude it, so a family that lists all of its own
/// members (which every one of them does) doesn't strip what it came to
/// install.
#[allow(clippy::too_many_arguments)]
fn prime_exclusive_self_condition(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    feature_tag: &'static str,
    prime: Condition,
    family: &'static [Condition],
    timer: ConditionTimer,
    log_line: &'static str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    if let Some(actor) = encounter.actors.get_mut(&caster_id) {
        actor.spend_feature(feature_tag);
    }
    encounter.log(log_line.to_string());
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = family
        .iter()
        .filter(|&&sibling| sibling != prime)
        .map(|&sibling| {
            Box::new(RemoveCondition {
                actor_id: caster_id,
                condition: sibling,
            }) as Box<dyn ApplicableSideEffect>
        })
        .collect();
    effects.push(Box::new(ApplyCondition {
        actor_id: caster_id,
        condition: prime,
        timer,
    }));
    effects
}

/// How long a constellation holds: 10 rounds = 1 minute RAW, the same
/// window Bladesong and Rage run on.
const STARRY_FORM_TIMER: ConditionTimer = ConditionTimer::Rounds(10);

/// Starry Form: Archer — the constellation of the bowman. Buys the
/// bonus-action `STARRY_BOLT` for ten rounds.
pub static STARRY_FORM_ARCHER: LazyLock<ExclusivePrime> = LazyLock::new(|| ExclusivePrime {
    name: "starry form archer",
    aliases: &["archer", "sfa", "starry archer"],
    tag: STARRY_FORM_TAG,
    prime_condition: Condition::StarryFormArcher,
    family: crate::conditions::condition_template::STARRY_FORMS,
    timer: STARRY_FORM_TIMER,
    log_line: "  starry form: the druid takes the shape of the Archer.",
});

/// Starry Form: Chalice — the constellation of the cup. Every slot-cast
/// heal spills over onto a second wounded ally within 30 ft.
pub static STARRY_FORM_CHALICE: LazyLock<ExclusivePrime> = LazyLock::new(|| ExclusivePrime {
    name: "starry form chalice",
    aliases: &["chalice", "sfc", "starry chalice"],
    tag: STARRY_FORM_TAG,
    prime_condition: Condition::StarryFormChalice,
    family: crate::conditions::condition_template::STARRY_FORMS,
    timer: STARRY_FORM_TIMER,
    log_line: "  starry form: the druid takes the shape of the Chalice.",
});

/// Starry Form: Dragon — the constellation of the wyrm. Floors the d20
/// on concentration saves at 10.
pub static STARRY_FORM_DRAGON: LazyLock<ExclusivePrime> = LazyLock::new(|| ExclusivePrime {
    name: "starry form dragon",
    aliases: &["dragon form", "sfd", "starry dragon"],
    tag: STARRY_FORM_TAG,
    prime_condition: Condition::StarryFormDragon,
    family: crate::conditions::condition_template::STARRY_FORMS,
    timer: STARRY_FORM_TIMER,
    log_line: "  starry form: the druid takes the shape of the Dragon.",
});

/// Tag for the Arcane Archer Fighter's **Arcane Shot** pool (subclass
/// level 3, XGE). Two charges, refreshed on a short or long rest —
/// `FEATURE_CHARGES` carries the count and `SHORT_REST_FEATURES` the
/// cadence, both RAW to the letter.
///
/// One tag for all six options rather than six tags, and this is the
/// feature that made the charge lane learn to count. Six separate
/// per-rest charges would hand the archer six free riders a fight,
/// which is not a subclass but a shopping list; a single shared charge
/// would leave five of the six options untouched in any fight that
/// lasted less than two rests. Two shared charges is what RAW says and
/// also what makes the feature interesting: the archer picks twice,
/// knowing the second pick is the last.
pub const ARCANE_SHOT_TAG: &str = "fighter.arcane_shot";

/// How long a nocked arrow waits for a shot. Two rounds — the same
/// window every Battle Master prime rides, and for the same reason:
/// the prime exists to be cashed on the swing the archer is about to
/// take, and a prime that outlived the round it was declared in would
/// let the archer bank both charges in a lull and fire them in a
/// burst RAW has no room for.
const ARCANE_SHOT_TIMER: ConditionTimer = ConditionTimer::Rounds(2);

/// Arcane Shot: Banishing Arrow. CHA save or the target is swept out of
/// the fight until the end of the archer's next turn.
///
/// No rider damage. RAW gives Banishing Arrow its 2d6 force only at
/// subclass level 18, and the banishment is already the strongest thing
/// on this menu — a failed save takes a creature's whole turn away,
/// which is what Hold Person costs a slot for.
pub static BANISHING_ARROW: LazyLock<ExclusivePrime> = LazyLock::new(|| ExclusivePrime {
    name: "banishing arrow",
    aliases: &["banishing shot", "banish arrow"],
    tag: ARCANE_SHOT_TAG,
    prime_condition: Condition::ArcaneShotBanishing,
    family: crate::conditions::condition_template::ARCANE_SHOTS,
    timer: ARCANE_SHOT_TIMER,
    log_line: "  arcane shot: a banishing arrow is nocked.",
});

/// Arcane Shot: Beguiling Arrow. 2d6 psychic, then a CHA save or the
/// target is Charmed and cannot raise a hand against the archer.
pub static BEGUILING_ARROW: LazyLock<ExclusivePrime> = LazyLock::new(|| ExclusivePrime {
    name: "beguiling arrow",
    aliases: &["beguiling shot", "beguile arrow"],
    tag: ARCANE_SHOT_TAG,
    prime_condition: Condition::ArcaneShotBeguiling,
    family: crate::conditions::condition_template::ARCANE_SHOTS,
    timer: ARCANE_SHOT_TIMER,
    log_line: "  arcane shot: a beguiling arrow is nocked.",
});

/// Arcane Shot: Bursting Arrow. The arrow detonates on impact, spraying
/// 2d6 force over everything standing near what it hit — no save, no
/// attack roll of its own.
pub static BURSTING_ARROW: LazyLock<ExclusivePrime> = LazyLock::new(|| ExclusivePrime {
    name: "bursting arrow",
    aliases: &["bursting shot", "burst arrow"],
    tag: ARCANE_SHOT_TAG,
    prime_condition: Condition::ArcaneShotBursting,
    family: crate::conditions::condition_template::ARCANE_SHOTS,
    timer: ARCANE_SHOT_TIMER,
    log_line: "  arcane shot: a bursting arrow is nocked.",
});

/// Arcane Shot: Enfeebling Arrow. 2d6 necrotic, then a CON save or the
/// target's weapon damage is halved until the end of the archer's next
/// turn.
pub static ENFEEBLING_ARROW: LazyLock<ExclusivePrime> = LazyLock::new(|| ExclusivePrime {
    name: "enfeebling arrow",
    aliases: &["enfeebling shot", "enfeeble arrow"],
    tag: ARCANE_SHOT_TAG,
    prime_condition: Condition::ArcaneShotEnfeebling,
    family: crate::conditions::condition_template::ARCANE_SHOTS,
    timer: ARCANE_SHOT_TIMER,
    log_line: "  arcane shot: an enfeebling arrow is nocked.",
});

/// Arcane Shot: Grasping Arrow. 2d6 poison, then a STR save or brambles
/// erupt from the shaft and hold the target fast.
pub static GRASPING_ARROW: LazyLock<ExclusivePrime> = LazyLock::new(|| ExclusivePrime {
    name: "grasping arrow",
    aliases: &["grasping shot", "grasp arrow"],
    tag: ARCANE_SHOT_TAG,
    prime_condition: Condition::ArcaneShotGrasping,
    family: crate::conditions::condition_template::ARCANE_SHOTS,
    timer: ARCANE_SHOT_TIMER,
    log_line: "  arcane shot: a grasping arrow is nocked.",
});

/// Arcane Shot: Shadow Arrow. 2d6 psychic, then a WIS save or the
/// target can see nothing past arm's reach.
pub static SHADOW_ARROW: LazyLock<ExclusivePrime> = LazyLock::new(|| ExclusivePrime {
    name: "shadow arrow",
    aliases: &["shadow shot", "shade arrow"],
    tag: ARCANE_SHOT_TAG,
    prime_condition: Condition::ArcaneShotShadow,
    family: crate::conditions::condition_template::ARCANE_SHOTS,
    timer: ARCANE_SHOT_TIMER,
    log_line: "  arcane shot: a shadow arrow is nocked.",
});

/// Tag for the Arcane Archer Fighter's **Curving Shot** (subclass level
/// 7, XGE): "when you make an attack roll with a magic arrow and miss,
/// you can use a bonus action to reroll the attack roll against the
/// same or a different target."
///
/// Rides the shared `MISSED_ATTACK_BOOSTS` cohort in `engine::attack`,
/// which adds a die to the missed total rather than rerolling it — the
/// cohort's standing trade, and the same one the Soulknife's Homing
/// Strikes makes on the row above. RAW's bonus-action cost is dropped
/// along with the reroll: the cohort fires inside the attack resolver,
/// where there is no action economy left to spend, and the per-rest
/// charge is doing the limiting work either way.
pub const CURVING_SHOT_TAG: &str = "fighter.curving_shot";

/// Passive tag for the Arcane Archer Fighter's **Ever-Ready Shot**
/// (subclass level 15, XGE): "when you roll initiative and have no uses
/// of Arcane Shot remaining, you regain one use of it."
///
/// No action of its own — `EncounterInstance::refill_ever_ready_shot`
/// fires it from the roll-initiative walk in `initialize`, alongside
/// the Thief's Reflexes extra turn slot, which is the engine's only
/// other feature triggered by those words.
///
/// Worth noting for what it needed: "have no uses remaining" was not a
/// state the engine could describe until the charge lane became a
/// count. In a set, a spent tag was gone, and an archer who had emptied
/// the pool looked exactly like one who never carried the feature.
pub const EVER_READY_SHOT_TAG: &str = "fighter.ever_ready_shot";

/// Every Arcane Shot action, in the order a template should carry them.
/// Mirrors `ARCANE_SHOTS` (the condition family) one-for-one —
/// `every_exclusive_prime_is_in_its_own_family` pins the two against
/// each other, so an option added to one and forgotten in the other is
/// a test failure rather than a prime nothing ever strips.
pub static ARCANE_SHOT_ACTIONS: LazyLock<Vec<&'static ExclusivePrime>> = LazyLock::new(|| {
    vec![
        &BANISHING_ARROW,
        &BEGUILING_ARROW,
        &BURSTING_ARROW,
        &ENFEEBLING_ARROW,
        &GRASPING_ARROW,
        &SHADOW_ARROW,
    ]
});

/// Starry Bolt — the Archer form's payload (5e Circle of Stars Druid,
/// subclass level 2). A bonus-action ranged spell attack out to 60 ft:
/// 1d8 + WIS radiant on a hit.
///
/// RAW folds the first bolt into the bonus action that assumes the
/// form and then hands out one per turn thereafter. Here assuming the
/// form and firing the bolt are two separate bonus actions, so the
/// Archer's opening round buys the constellation and the bolts start
/// on the round after. That is a real cost — one round of tempo — and
/// it is the honest one to pay: the engine prices a bonus action as
/// the scarce thing on every other chassis (Flurry of Blows, Hand of
/// Healing, Cutting Words), and a version that fired for free on the
/// turn it was bought would make Archer strictly better than the two
/// forms it competes with rather than differently good.
///
/// Not a `SimpleWeapon` even though the numbers would fit one, because
/// the two things that make it this subclass's feature — the bonus
/// action cost and the gate on standing in the right constellation —
/// are exactly the two things `SimpleWeapon` has no room for.
pub struct StarryBolt {}

impl Action for StarryBolt {
    fn name(&self) -> &str {
        "starry bolt"
    }
    fn aliases(&self) -> Vec<&str> {
        // Neither "bolt" nor "sb": the first is already shared by Fire
        // Bolt and Guiding Bolt, and the second by four other actions
        // including the Soulknife's second blade. Alias collisions
        // resolve by list order rather than erroring, so a Stars druid
        // carrying Guiding Bolt would have had two of its own actions
        // fighting over one token.
        vec!["starbolt", "sbolt"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    /// 60 ft on the 2.5 ft grid. No long-range band — RAW gives the
    /// bolt a flat 60 ft, so the reach *is* the range.
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
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
        // The constellation is the entire gate — the bolt costs no
        // charge of its own and is at-will for as long as the form
        // holds.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && a.has_condition(Condition::StarryFormArcher))
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
        let attack_mod = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        let damage_bonus = caster.ability_modifier(AbilityScoreType::Wisdom);
        // Routed through the shared spell-attack chokepoint rather than
        // open-coded, so the bolt picks up cover, Sanctuary, Bless, the
        // defender-side reactive taxes, the interception cohort and the
        // crit dice on exactly the same terms every other spell attack
        // in the game does.
        crate::actions::spells::spell_attack_outcome(
            encounter,
            caster_id,
            target_id,
            "starry bolt",
            attack_mod,
            Dice::new(1, 8),
            damage_bonus,
            DamageType::Radiant,
            false,
        )
        .0
    }
}

pub static STARRY_BOLT: LazyLock<StarryBolt> = LazyLock::new(|| StarryBolt {});

/// The Chalice form's payload (5e Circle of Stars Druid, subclass level
/// 2): "whenever you cast a spell using a spell slot that restores hit
/// points to a creature, you or another creature within 30 feet of you
/// can regain hit points equal to 1d8 + your Wisdom modifier."
///
/// Called from the shared slot-heal sites alongside
/// `disciple_of_life_bonus`, and returns the extra `Heal` to append —
/// or nothing at all when the form isn't up, the heal was a cantrip,
/// or there is no second wounded ally in range. Returning an
/// `Option<Box<_>>` rather than mutating a vector keeps the call sites
/// to a one-line `.extend(...)` next to the bonus they already compute.
///
/// Two judgement calls RAW leaves to the player, made here:
///
///   - **Who.** The most wounded eligible creature, by missing HP. RAW
///     lets the druid pick anyone in range; the most wounded is what a
///     druid picking on purpose would pick, and it matches how the
///     rest of the AI's heal lane already chooses targets.
///   - **Not a creature the spell already healed.** RAW's "you or
///     another creature" does permit doubling up on the spell's own
///     target, but the primary `Heal`s have not been applied yet when
///     this runs, so those targets still read as fully wounded and
///     would win the comparison almost every time. `already_healed`
///     carries them — one id for Cure Wounds and Healing Word, up to
///     six for Mass Cure Wounds — and excluding them makes the Chalice
///     what its name says it is: a cup that overflows onto someone
///     else, rather than a flat healing bump on a creature the spell
///     had covered.
///
/// Nothing fires for a druid healing alone: with the spell's targets
/// excluded, a solo Chalice druid healing themselves has no second
/// creature to spill onto. That is the correct reading of a feature
/// whose whole text is about a second creature.
pub fn starry_chalice_overflow(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    already_healed: &[usize],
    spell_slot_lvl: u32,
) -> Option<Box<dyn ApplicableSideEffect>> {
    // RAW gates on "using a spell slot", which is the same cantrip
    // exclusion `disciple_of_life_bonus` makes one line above every
    // call site.
    if spell_slot_lvl == 0 {
        return None;
    }
    let caster = encounter.actors.get(&caster_id)?;
    if !caster.has_condition(Condition::StarryFormChalice) {
        return None;
    }
    let wis_mod = caster.ability_modifier(AbilityScoreType::Wisdom);
    // 30 ft on the 2.5 ft grid.
    let recipient = encounter.most_wounded_ally_within(caster_id, 12, already_healed)?;
    let raw = encounter.roll(&Dice::new(1, 8)) as i32;
    let amount = (raw + wis_mod).max(1) as u32;
    let name = encounter.actor_name(recipient);
    encounter.log(format!(
        "  chalice: 1d8({}){:+} = {} HP spills onto {}",
        raw, wis_mod, amount, name
    ));
    Some(Box::new(Heal {
        actor_id: recipient,
        amount,
    }))
}

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
    // 1 minute, RAW.
    timer: ConditionTimer::Rounds(10),
    // No "instead of turned" rung: RAW gives this one no
    // remove-the-weak clause.
    escalation: None,
});

/// Tag for the Bladesinging Wizard's **Bladesong** (subclass level 2).
/// RAW grants two uses per short rest; this is one, riding
/// `SHORT_REST_FEATURES` so the cadence stays RAW's. A minute-long
/// self-buff has little use for a second charge inside one fight, which
/// is the same reason Starry Form and Giant's Might stay at one where
/// `FEATURE_CHARGES` would happily give them RAW's number.
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
/// rest; this is one use that comes back on a short rest instead. Left
/// at one deliberately rather than sized up through `FEATURE_CHARGES`:
/// the buff runs a minute, which is longer than most fights here, so a
/// second charge would almost never be spent on anything the first had
/// not already bought.
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

/// 5e Path of the Giant Barbarian **Giant's Havoc**, the Giant Stature
/// half (subclass level 3): "while you're raging... your size becomes
/// Large, if there is enough room, and your reach increases by 5 feet."
///
/// A passive tag and nothing else — the whole implementation is one
/// gated row on `RESIZING_CONDITIONS`, keyed off `Raging` rather than
/// off a condition of its own because RAW hands the growth out with the
/// rage and charges nothing for it. The gate is the reason the row
/// needed a `holder_gate` column: every barbarian on the roster carries
/// `Raging`, and only this one grows.
///
/// The reach half comes free with the size. The engine measures reach
/// from footprints, so a Large barbarian already threatens a wider ring
/// than a Medium one — which is the same fact that makes the Rune
/// Knight's Giant's Might worth a bonus action, arriving here on a
/// chassis that was going to rage anyway.
///
/// What that buys the giant-path barbarian over its sixteen siblings is
/// a *shape*. Every other path sharpens the swing (Berserker, Zealot),
/// hardens the body (Bear Totem, Ancestral Guardian) or adds a rider
/// (Storm Herald); this one takes up four tiles instead of one from the
/// round the rage starts. A Large barbarian in a corridor is the whole
/// corridor, and a Large barbarian beside a caster is an opportunity
/// attack the caster could not step out of.
///
/// RAW's Crushing Throw — the rage damage bonus riding thrown weapons —
/// is left out: the engine has no thrown-weapon lane distinct from the
/// ranged one, so the clause would either apply to every bow on the
/// board or to nothing.
pub const GIANT_STATURE_TAG: &str = "barbarian.giant_stature";

/// Elemental Cleaver — Path of the Giant Barbarian bonus action
/// (subclass level 3). RAW: "when you rage, you can choose... acid,
/// cold, fire, lightning, or thunder. Your weapon deals an extra 1d6
/// damage of the chosen type."
///
/// **The rage is a precondition rather than a cost.** The action refuses
/// unless the barbarian is already raging, which is how RAW's "while
/// you're raging" clause is enforced without the rider having to read
/// two conditions at once — see `Condition::ElementalCleaver`. There is
/// no charge: RAW's cleaver is on for the whole rage and re-chooseable
/// at will, so a per-rest counter would have been the engine inventing
/// a limit the rules don't have.
///
/// What it costs instead is the bonus action, and on this chassis that
/// is a real price: the barbarian's bonus action is also Frenzy's extra
/// swing, and the round spent kindling the weapon is a round not spent
/// hitting with it. A giant-path barbarian who opens with rage and
/// cleaver has spent two turns' bonus actions before landing the first
/// extra die.
pub struct ElementalCleaverAction {}

impl Action for ElementalCleaverAction {
    fn name(&self) -> &str {
        "elemental cleaver"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cleaver", "ec", "elemental weapon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Indirect: the die lands on the swing, not on the kindling.
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
                // Tag-gated the way Frenzy is, so the action is inert on
                // any barbarian chassis that ends up sharing an action
                // pool without sharing the path.
                && a.has_passive_feature(GIANT_STATURE_TAG)
                // RAW's "while you're raging", enforced at the install
                // rather than at every swing.
                && a.has_condition(Condition::Raging)
                // Re-kindling an already-lit weapon buys nothing but a
                // refreshed timer, and costs the bonus action Frenzy
                // wants.
                && !a.has_condition(Condition::ElementalCleaver)
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
        encounter.log(
            "  elemental cleaver: frost creeps down the haft and the axe head goes white.",
        );
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::ElementalCleaver,
            // The rage's own ten rounds. RAW ends the cleaver with the
            // rage; matching the timer is the closest the engine's
            // condition clock gets to saying so, and a barbarian whose
            // rage lapsed has bigger problems than a stray die.
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static ELEMENTAL_CLEAVER: LazyLock<ElementalCleaverAction> =
    LazyLock::new(|| ElementalCleaverAction {});

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
    // 1 minute, RAW.
    timer: ConditionTimer::Rounds(10),
    // No "instead of turned" rung: RAW gives this one no
    // remove-the-weak clause.
    escalation: None,
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
/// Psionic Energy pool by proficiency bonus; this is one charge,
/// refreshed on a short rest. Left there rather than sized up through
/// `FEATURE_CHARGES` because the cohort spends automatically on the
/// first eligible miss — a deeper pool would drain itself on whichever
/// misses happened to come first rather than on the ones that mattered,
/// which is a worse feature than a single charge the rogue can feel.
pub const HOMING_STRIKES_TAG: &str = "rogue.homing_strikes";

/// Passive tag for the Thief Rogue's **Fast Hands** (subclass level 3):
/// "you can use the bonus action granted by your Cunning Action to …
/// use an object."
///
/// Read at `EncounterInstance::uses_objects_as_a_bonus_action`, which
/// every item action's `cost()` consults through
/// `item_actions::item_use_cost`. There is no action of its own and no
/// charge — the whole feature is a change of price on a lane that
/// already exists, which is why it ships as a tag rather than as an
/// entry in the action list.
///
/// The engine's cost model asks for a list of resources that must *all*
/// be paid, and has no way to express "an Action or a bonus action,
/// whichever you have." Fast Hands is therefore resolved dynamically at
/// `cost()` time: it prices the item at a bonus action when the holder
/// still has one free, and leaves the Action price alone when they
/// don't. That is strictly the better half of RAW's offer in every
/// state — the rogue never loses access to a potion they could
/// otherwise have drunk, and gains the turns where the Action was
/// wanted for a blade.
/// Per-rest charge for the Sun Soul Monk's **Searing Sunburst**
/// (subclass level 11): a 20-ft-radius sphere of light thrown up to 150
/// ft; every creature caught in it makes a CON save or takes 2d6
/// radiant.
///
/// RAW prices it in ki — 2 points, plus up to 3 more for an extra 2d6
/// each — and the engine's ki pool (`KI_POINTS_TAG`) counts uses rather
/// than points, so the burst spends one point of the monk's five and the
/// "up to 3 more for extra dice" clause goes with the arithmetic it
/// needed. What the pool did buy is the trade: a Sun Soul who throws two
/// sunbursts has two stuns left instead of five, which is the decision
/// the private charge this feature used to carry could not express.
///
/// **Save-for-nothing, not save-for-half**, which is the mechanically
/// interesting half of the feature and the reason it reads so
/// differently from the Light Cleric's Radiance of the Dawn despite
/// being the same shape. RAW is explicit ("takes no damage on a
/// successful save"), and it makes the burst swingy in a way a
/// save-for-half burst never is: against a room of low-CON creatures it
/// is close to a Fireball, and against a saving-throw-proficient one it
/// can do nothing at all.
pub const SEARING_SUNBURST_TAG: &str = "monk.searing_sunburst";

/// Searing Sunburst — Sun Soul Monk action. A radius-4 burst thrown up
/// to 60 tiles, CON save vs the monk's WIS-based DC, 2d6 radiant, and
/// nothing at all on a success. Once per short rest.
///
/// Same geometry as Fireball (RAW: both are 20-ft spheres at 150 ft),
/// deliberately — it is the only ranged area damage a monk on this
/// roster has, and pinning it to the shape the engine's other bursts
/// already use means the AI's area-attack lane picks it up without a
/// special case.
///
/// Targets enemies only rather than friend-or-foe. RAW catches everyone
/// in the sphere, and the divergence is the same one Radiance of the
/// Dawn takes: a self-throwing burst with no Careful Spell or Sculpt
/// Spells behind it would make the AI's area lane a liability to its own
/// team, and there is no monk-side feature to spare the allies with.
pub struct SearingSunburst {}

impl Action for SearingSunburst {
    fn name(&self) -> &str {
        "searing sunburst"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sunburst", "ssb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 20-ft sphere. Matches Fireball's radius on the engine's
        // area-of-effect scale.
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft = 60 tiles, the same throw Fireball gets.
        Some(60)
    }
    fn requires_los(&self) -> bool {
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
        feature_ready(encounter, caster_id, SEARING_SUNBURST_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(center) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Spend the charge up-front so a mid-resolution actor lookup
        // can't double-fire — same ordering as Radiance of the Dawn.
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(SEARING_SUNBURST_TAG);
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        // One shared roll for the whole burst, per 5e area damage.
        let damage = encounter.roll(&Dice::new(2, 6));
        encounter.log(format!(
            "  searing sunburst: 2d6({}) radiant, DC {} CON save for none.",
            damage, dc
        ));
        resolve_enemy_burst_save_damage(
            encounter,
            caster_id,
            center,
            4,
            AbilityScoreType::Constitution,
            dc,
            damage,
            DamageType::Radiant,
            SaveDamagePolicy::NoneOnSave,
        )
    }
}

pub static SEARING_SUNBURST: LazyLock<SearingSunburst> = LazyLock::new(|| SearingSunburst {});

/// Ki charge for the Ascendant Dragon Monk's **Breath of the Dragon**
/// (subclass level 3): the monk exhales their ancestor's element in a
/// cone, and every creature caught in it makes a DEX save for half.
///
/// The one press on the monk chassis that is neither a punch nor a
/// spell. Every other monk on the roster answers a cluster of enemies
/// by walking into the middle of it — a d8 hit die and no armour, which
/// is the standing problem the Sun Soul solved with range and this one
/// solves with area. RAW replaces *one attack of the Attack action*
/// with the breath, so it costs the monk their swing rather than their
/// turn; here it costs the Action, which is the same trade on a chassis
/// whose Extra Attack is expressed as two swings off one Action.
///
/// The damage type is read from `draconic_ancestry` at resolution
/// time, the same accessor the Dragonborn's racial Breath Weapon
/// reads. That is what makes the feature worth its ki against a
/// bestiary this resistant: an Ascendant Dragon monk with a fire
/// ancestry is throwing the one element half the bestiary shrugs off,
/// and the ancestry pick is therefore a real one.
pub const BREATH_OF_THE_DRAGON_TAG: &str = "monk.breath_of_the_dragon";

/// Breath of the Dragon — Ascendant Dragon Monk action. A radius-3
/// burst thrown up to 8 tiles, DEX save vs the monk's WIS-based DC,
/// 2d10 of the ancestry's damage type, half on a success.
///
/// Sibling to the Dragonborn's `BreathWeapon` on every axis except the
/// two that matter: the DC is anchored on Wisdom (the monk's
/// spellcasting-adjacent ability, per RAW's "Ki save DC") rather than
/// Constitution, and the cone is the bigger one — RAW's 20-ft cone
/// against the racial breath's 15 ft, which is the radius-3 / radius-2
/// split here.
///
/// **Friend-or-foe**, unlike the Sun Soul's Searing Sunburst. RAW's
/// cone catches everything in it and the monk has no Careful Spell to
/// spare an ally with, so the honest translation is the neutral burst
/// helper — and the AI's burst placer already refuses a placement that
/// catches its own team. Searing Sunburst diverges the other way
/// because it is thrown 150 ft across the room at a cluster the monk
/// is nowhere near; a cone starts at the monk's own face, where the
/// monk's own front line is standing.
pub struct BreathOfTheDragon {}

impl Action for BreathOfTheDragon {
    fn name(&self) -> &str {
        "breath of the dragon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["botd", "dragon breath", "exhale"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // RAW 20-ft cone. Radius 3 on the 2.5 ft grid is the same
        // envelope the engine gives Thunderwave and one step wider
        // than the Dragonborn's 15-ft racial breath.
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // The cone's own length — 20 ft — is how far from the monk the
        // far edge sits, so the aim point stays inside 8 tiles.
        Some(8)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        // Resolved from `draconic_ancestry` at cast time; the list here
        // is the five ancestries RAW offers, reported for the UI's
        // resistance hints and the AI's matchup heuristics the same way
        // the Dragonborn's racial breath reports them.
        vec![
            DamageType::Acid,
            DamageType::Cold,
            DamageType::Fire,
            DamageType::Lightning,
            DamageType::Poison,
        ]
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
        feature_ready(encounter, caster_id, BREATH_OF_THE_DRAGON_TAG)
            && encounter
                .actors
                .get(&caster_id)
                .is_some_and(|a| a.draconic_ancestry().is_some())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(center) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Spend the charge before anything can fail, matching Searing
        // Sunburst and Radiance of the Dawn: a burst that half-resolves
        // must not leave the press still available.
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(BREATH_OF_THE_DRAGON_TAG);
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let damage_type = caster.draconic_ancestry().unwrap_or(DamageType::Fire);
        // RAW scaling: 2d10 at lv3, 3d10 at lv11, 4d10 at lv17. The
        // chassis this ships on is level 5, so 2d10 is the number; the
        // formula is written out anyway so a level bump on the template
        // moves the die count with it.
        let dice_count = match caster.level() {
            0..=10 => 2,
            11..=16 => 3,
            _ => 4,
        };
        let damage = encounter.roll(&Dice::new(dice_count, 10));
        encounter.log(format!(
            "  breath of the dragon: {}d10({}) {} cone, DC {} DEX save for half.",
            dice_count, damage, damage_type, dc
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            center,
            3,
            AbilityScoreType::Dexterity,
            dc,
            damage,
            damage_type,
        )
    }
}

pub static BREATH_OF_THE_DRAGON: LazyLock<BreathOfTheDragon> =
    LazyLock::new(|| BreathOfTheDragon {});

/// Ki charge for the Ascendant Dragon Monk's **Aspect of the Wyrm**
/// (subclass level 11): the monk takes on their ancestor's presence and
/// every hostile that can see them makes a save or is Frightened.
///
/// RAW's aura is a menu — the monk picks *either* frightening enemies
/// *or* granting their allies resistance to the ancestral damage type,
/// and the aura persists for a minute while the monk is up. The
/// frighten half is what ships. The resistance half wants a projected
/// ally aura keyed on a damage type, which is a lane the engine's
/// resistance model does not have (typed resistance is read off held
/// conditions and passive template flags, both of which are properties
/// of the *holder* rather than of somebody standing near them), so
/// shipping it would mean a new cohort for one subclass.
///
/// The half that ships is the one that changes how the subclass plays.
/// A monk is a creature that has to be standing in contact, and
/// Frightened is the only condition in the engine that makes the
/// creatures it is in contact with worse at hitting it. Breath of the
/// Dragon and Aspect of the Wyrm therefore want the same board — a
/// cluster of hostiles the monk has walked into the middle of — and
/// spend from the same five presses, which is the decision the ki pool
/// exists to make legible.
pub const ASPECT_OF_THE_WYRM_TAG: &str = "monk.aspect_of_the_wyrm";

/// Aspect of the Wyrm — Ascendant Dragon Monk action. Every hostile
/// within 30 ft makes a WIS save vs the monk's WIS-based DC; on a fail
/// they are Frightened for 10 rounds. Spends from the ki pool.
///
/// The seventh `TurnBurst` config and the first on a monk, which is
/// the point of the config being data: the whole feature is four
/// fields and a registry row. WIS-anchored like every other DC the
/// monk chassis sets, and unfiltered like Dreadful Aspect — RAW's aura
/// asks nothing about what the creatures caught in it are made of.
pub static ASPECT_OF_THE_WYRM: LazyLock<TurnBurst> = LazyLock::new(|| TurnBurst {
    name: "aspect of the wyrm",
    aliases: &["aotw", "wyrm", "aspect"],
    tag: ASPECT_OF_THE_WYRM_TAG,
    // Monk save DCs are Wisdom-anchored — the same ability Stunning
    // Strike and Breath of the Dragon set theirs from.
    dc_ability: AbilityScoreType::Wisdom,
    // No creature-type gate: RAW is "each creature of your choice that
    // you can see within 30 feet".
    type_filter: |_| true,
    installed: Condition::Frightened,
    // 1 minute, RAW.
    timer: ConditionTimer::Rounds(10),
    // No "instead of turned" rung: RAW gives this one no
    // remove-the-weak clause.
    escalation: None,
});

/// Passive tag for the Oath of the Crown Paladin's **Divine Allegiance**
/// (subclass level 7): when a creature within 5 ft takes damage, the
/// paladin may spend their reaction to take it instead.
///
/// No action of its own and no charge — the whole feature is a reaction
/// the engine spends on the paladin's behalf, at the damage chokepoint.
/// Read at `EncounterInstance::claim_damage_interposition`, which is
/// called from the top of `DealDamage::apply`; see that method for why
/// this is its own lane rather than a row on `REACTIVE_DAMAGE_CLAMPS`,
/// and for the one RAW clause it doesn't honour.
pub const DIVINE_ALLEGIANCE_TAG: &str = "paladin.divine_allegiance";

/// Passive tag for the Oath of Redemption Paladin's **Aura of the
/// Guardian** (subclass level 7): "when a creature within 10 feet of you
/// takes damage, you can use your reaction to magically take that damage
/// instead of that creature taking it."
///
/// The second row on `EncounterInstance::DAMAGE_INTERPOSERS`, and the
/// wider of the two: the Crown paladin has to be in contact to take a
/// blow, this one covers the same ten feet the paladin's other two auras
/// already project. Which is the whole subclass in one sentence — every
/// other paladin aura *improves* what happens to the people standing in
/// it, and this one moves what happens to them onto the paladin.
///
/// No action and no charge; the engine spends the reaction. Sitting at
/// the `DealDamage` chokepoint rather than on the clamp cohort means it
/// answers a fireball and a poison drip as readily as a sword swing,
/// which is RAW ("takes damage", unqualified) and is why a Redemption
/// paladin standing beside the wizard is worth more than an equivalent
/// clamp would be.
pub const AURA_OF_THE_GUARDIAN_TAG: &str = "paladin.aura_of_the_guardian";

/// Passive tag for the Oath of Redemption Paladin's **Protective
/// Spirit** (subclass level 15): "you regain hit points equal to 1d6 +
/// half your paladin level if you are below half your hit point maximum
/// … at the end of each of your turns."
///
/// The other half of the subclass, and the half that makes the first one
/// survivable. Aura of the Guardian is a paladin volunteering to be hit
/// by everything aimed at their party; Protective Spirit is what pays
/// for it. Read at `EncounterInstance::apply_protective_spirit`.
///
/// **Start of turn, not end.** The engine has a per-turn opening hook
/// (`start_turn_for`) and no per-turn closing one — turns end when the
/// initiative index moves, which happens in three places for three
/// reasons. Firing the heal on the paladin's own turn opening is the
/// same once-per-round cadence one tick earlier, and the tick it moves
/// across is one in which nothing else happens to the paladin (their own
/// turn has not started). The one observable difference is against a
/// damage source that lands between the two moments, which for a
/// creature that has just had its turn means the rest of the round: a
/// RAW Protective Spirit tops up before that damage, and this one tops
/// up after it.
///
/// **The `is_wounded` gate is RAW's "below half".** `heal` refuses to
/// overheal, so a paladin at 90% takes the roll and keeps a point or two
/// of it; RAW gives them nothing at all. The half-HP gate is checked
/// explicitly rather than left to the healer, because "regenerates while
/// hurt" and "regenerates while nearly dead" are different features and
/// the first one is not this.
pub const PROTECTIVE_SPIRIT_TAG: &str = "paladin.protective_spirit";

/// Passive tag for the Oath of Devotion Paladin's **Purity of Spirit**
/// (subclass level 15): "you are always under the effects of a
/// protection from evil and good spell."
///
/// Which in this engine is exactly the `Warded` condition — aberrations,
/// celestials, elementals, fey, fiends and undead attack the holder at
/// disadvantage — held permanently rather than for a spell's duration.
/// Read at the same gate in `compute_attack_mode` that reads the
/// condition, so the feature costs one clause there rather than a
/// standing condition install the engine would have to keep re-applying
/// and would have to stop Dispel Magic from stripping.
///
/// This is the feature that RAW puts at Devotion's level 15, and it
/// replaces Rebuke the Violent, which the engine used to ship here.
/// Rebuke the Violent belongs to Oath of Redemption — see
/// `REBUKE_THE_VIOLENT_TAG` and `REDEMPTION_PALADIN_TEMPLATE`.
///
/// RAW's other half — the spell also blocks being charmed, frightened or
/// possessed by those same creature types — is not wired up, because the
/// engine's Charmed and Frightened installs carry no record of what
/// creature type caused them, and a blanket immunity would be a strictly
/// larger feature than the one RAW wrote. The Devotion paladin already
/// carries `has_aura_of_devotion` for the charm half at 10 ft, which
/// covers the case the oath is about.
pub const PURITY_OF_SPIRIT_TAG: &str = "paladin.purity_of_spirit";

/// Per-rest charge for the Oath of the Crown Paladin's Channel Divinity
/// **Champion Challenge** (subclass level 3): each creature of the
/// paladin's choice within 30 ft makes a WIS save or "can't willingly
/// move more than 30 feet away from you."
///
/// The engine has no leash — a distance cap measured from a moving
/// anchor, re-checked per tile of movement, is a movement-validation
/// lane that nothing else needs. What it has is `Rooted`, and the
/// Conquest Paladin's Aura of Conquest already reads RAW's "speed 0" as
/// exactly that. Champion Challenge lands on the same condition for one
/// round, which is a harder lock over a much shorter window: RAW's leash
/// lets the target fight anyone inside a 30-ft circle for a minute, and
/// this pins them where they stand until their next turn.
///
/// That trade is what makes the two Channel Divinity options on this
/// oath read differently. Champion Challenge stops a line from
/// collapsing onto the party's back rank for exactly one round; Turn the
/// Tide puts the back rank back on its feet.
pub const CHAMPION_CHALLENGE_TAG: &str = "paladin.champion_challenge";

/// Champion Challenge — Oath of the Crown Paladin Channel Divinity.
/// Self-centred 30-ft (12-tile) hostile burst; WIS save vs the paladin's
/// CHA-anchored DC or `Rooted` until the start of the target's next
/// turn. Once per short rest.
///
/// A `TurnBurst` literal rather than an impl of its own, which is what
/// the `timer` field on that struct exists for: the row differs from
/// Conquering Presence in the condition it installs and in how long it
/// lasts, and in nothing else. Being a `TurnBurst` is also what makes
/// the AI reach for it — `TURN_BURST_PICKS` is keyed on the config
/// struct, so a bespoke impl would have been invisible to every
/// controller in the engine.
pub static CHAMPION_CHALLENGE: LazyLock<TurnBurst> = LazyLock::new(|| TurnBurst {
    name: "champion challenge",
    aliases: &["challenge", "cd-challenge"],
    tag: CHAMPION_CHALLENGE_TAG,
    dc_ability: AbilityScoreType::Charisma,
    // Anything hostile, like the other two unfiltered oaths.
    type_filter: |_| true,
    installed: Condition::Rooted,
    // One round, not the minute every other row takes — see the field's
    // doc and the tag's for why a hold trades duration for severity.
    timer: ConditionTimer::UntilStartOfNextTurn,
    // No "instead of turned" rung: RAW gives this one no
    // remove-the-weak clause.
    escalation: None,
});

/// Per-rest charge for the Oath of the Crown Paladin's Channel Divinity
/// **Turn the Tide** (subclass level 3): each creature of the paladin's
/// choice within 30 ft that is at or below half its hit points regains
/// `1d6 + CHA` HP.
///
/// The half-HP gate is RAW here, and it is also what keeps this from
/// being a strictly better Lay on Hands. Lay on Hands is a large pool
/// spent on one creature at any wound level; this is a small flat heal
/// that only reaches the badly hurt, and reaches all of them at once.
pub const TURN_THE_TIDE_TAG: &str = "paladin.turn_the_tide";

/// Turn the Tide — Oath of the Crown Paladin Channel Divinity. Heals
/// every ally within 30 ft who is at or below half HP for `1d6 + CHA`.
/// Once per short rest.
pub struct TurnTheTide {}

impl Action for TurnTheTide {
    fn name(&self) -> &str {
        "turn the tide"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tide", "cd-tide"]
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
        feature_ready(encounter, caster_id, TURN_THE_TIDE_TAG)
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
            actor.spend_feature(TURN_THE_TIDE_TAG);
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let cha = caster.ability_modifier(AbilityScoreType::Charisma);
        let caster_team = caster.team();
        let caster_loc = caster.location();
        let caster_size = get_tiles_from_size(caster.size());
        // Ids collected and sorted before any rolling so the per-target
        // heal rolls land in a deterministic order under a fixed seed.
        let mut wounded: Vec<usize> = encounter
            .actors
            .iter()
            .filter(|(_, a)| {
                if a.team() != caster_team || !a.is_combat_active() {
                    return false;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    caster_loc,
                    caster_size,
                );
                // RAW: "at or below half its hit point maximum" — the
                // Bloodied line, asked by name.
                dist <= 12 && a.is_bloodied()
            })
            .map(|(id, _)| *id)
            .collect();
        wounded.sort_unstable();
        if wounded.is_empty() {
            encounter.log("  turn the tide: nobody in range is hurt enough to answer.".to_string());
            return Vec::new();
        }
        // RAW rolls the die per creature rather than sharing one, unlike
        // the AoE damage bursts — the healing is "each creature regains
        // hit points equal to 1d6 + your Charisma modifier".
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in wounded {
            let raw = encounter.roll(&Dice::new(1, 6)) as i32;
            let amount = (raw + cha).max(1) as u32;
            let name = encounter.actor_name(id);
            encounter.log(format!(
                "  turn the tide: {} rallies — 1d6({}){:+} = {} HP",
                name, raw, cha, amount
            ));
            effects.push(Box::new(Heal {
                actor_id: id,
                amount,
            }));
        }
        effects
    }
}

pub static TURN_THE_TIDE: LazyLock<TurnTheTide> = LazyLock::new(|| TurnTheTide {});

pub const FAST_HANDS_TAG: &str = "rogue.fast_hands";

/// Passive tag for the Thief Rogue's **Thief's Reflexes** (subclass
/// level 17): two turns during the first round of combat, the second at
/// initiative minus 10.
///
/// Read once, at `EncounterInstance::initialize`, which hands the holder
/// a second slot in the initiative queue — see `grant_extra_turn_slot`
/// for why the feature is modelled as a real queue entry and not as a
/// bundle of spare resources bolted onto the first turn.
///
/// This is the one rogue feature whose value is almost entirely front-
/// loaded, and it is the natural capstone for the subclass that spends
/// its bonus action on objects rather than on blades: the Thief's first
/// round is two Sneak Attacks, two Cunning Actions and two potions'
/// worth of action economy, against a table that has had one turn each.
pub const THIEFS_REFLEXES_TAG: &str = "rogue.thiefs_reflexes";


// ─── Artificer ──────────────────────────────────────────────────────
//
// The class the roster was missing, and the one whose whole identity is
// that its power lives in objects rather than in the artificer. Every
// other caster on the roster answers "what does this class do on its
// turn"; the Artificer answers "what did this class *build*", and the
// four subclasses are four different things to have built — a potion, a
// suit of armour, a turret, and a dog.
//
// That shape is why so little of the class needs new engine machinery.
// A turret and a dog are `FeatureSummon`s. A suit of armour is two
// `SimpleWeapon`s. A potion is a bonus action that installs a condition.
// What genuinely had nowhere to go is Flash of Genius — a reaction that
// fires on somebody *else's* failed save — and that is one new cohort at
// the save chokepoint, described where it lives.

/// 5e Artificer level-7 feature **Flash of Genius**. "Whenever you or
/// another creature you can see within 30 feet of you makes an ability
/// check or a saving throw, you can use your reaction to add your
/// Intelligence modifier to the roll."
///
/// Read at `EncounterInstance::try_flash_of_genius`, the last rung of
/// the failed-save recovery ladder, and the only one on that ladder
/// whose source is a *different creature from the one rolling*. Dark
/// One's Own Luck and Fanatical Focus are things the failing actor is
/// carrying; this is a thing a nearby artificer chooses to spend a
/// reaction on, which is why it could not be a row on
/// `FAILED_SAVE_ADD_DIE_SOURCES` and is a scan of the board instead.
///
/// Both halves of the cost are real. The reaction means an artificer
/// who has already fired one this round has nothing to give, and it
/// competes with every other reaction the chassis carries. The charge —
/// RAW gives uses equal to the Intelligence modifier, which we collapse
/// to one per short rest, the cadence every other charge on this file
/// uses — means the party gets one rescue per fight rather than one per
/// save.
///
/// The engine spends it the way it spends Legendary Resistance: on any
/// failed save the modifier could actually rescue, checked against the
/// shortfall first so a charge is never burned on a save it cannot
/// reach. RAW lets the artificer add the modifier *before* seeing
/// whether it helps; holding the charge for a roll it can save is the
/// same courtesy the add-die cohort already extends.
///
/// The ability-check half of RAW has no surface — this engine rolls
/// saves, not checks.
pub const FLASH_OF_GENIUS_TAG: &str = "artificer.flash_of_genius";

/// 5e **Feather Fall** (level-1 transmutation, reaction) — carried as a
/// tag rather than as an entry on the holder's action list.
///
/// Every other spell in this engine is a castable `Action`, and this one
/// is not, because its RAW trigger is *"when you or a creature within 60
/// feet of you falls"* and a turn-ordered action list has no way to
/// offer that. Pre-casting it is not the same spell: the whole effect is
/// contingent on a fall that has not happened, and a caster who spent an
/// action and a slot on the chance of one would simply be playing worse.
/// So the tag says the caster *knows* the spell, and
/// `EncounterInstance::try_feather_fall` is the trigger — the same shape
/// `FLASH_OF_GENIUS_TAG` above uses for the other reaction whose window
/// belongs to somebody else's roll.
///
/// Not a charged feature. It carries no `features_max` count worth
/// spending because RAW prices it in a reaction and a 1st-level slot,
/// and both of those are resources the engine already tracks and the
/// hook already spends. A per-rest charge on top would be a third,
/// invented cost.
///
/// Held by the four arcane / bard-list chassis that have Feather Fall on
/// their spell list RAW: Wizard, Sorcerer, Bard and Artificer. A Cleric
/// or Druid does not get to catch anybody.
pub const FEATHER_FALL_TAG: &str = "spell.feather_fall";

/// 5e **Absorb Elements** (level-1 abjuration, reaction) — carried as a
/// tag rather than as an entry on the holder's action list, for the
/// reason `FEATHER_FALL_TAG` directly above is.
///
/// > *Casting Time:* Reaction, which you take when you take Acid, Cold,
/// > Fire, Lightning, or Thunder damage. […] you have Resistance to the
/// > triggering damage type until the start of your next turn. […] the
/// > first time you hit with a melee attack on your next turn, the
/// > target takes an extra 1d6 damage of the triggering type.
///
/// It shipped here as a castable `Action` costing a reaction, which is
/// the wrong shape twice over. RAW's trigger is *damage that has
/// already been rolled*, and a turn-ordered action list cannot offer
/// that window — so the AI never once reached for it, in five class
/// loadouts, across every fight in the suite. And "the triggering
/// damage type" cannot be known by a spell cast before anything has
/// triggered, which is why the resistance collapsed to the blanket
/// `DamageResistant` flag and the melee rider dealt Force: the element
/// the spell is *about* had not happened yet.
///
/// Both halves come from the hook.
/// `EncounterInstance::try_absorb_elements` runs at the top of
/// `DealDamage::apply` — before every halving, so the resistance
/// applies to the blow that triggered it, which is RAW and is the whole
/// point of the spell — and writes the triggering type onto
/// `Condition::AbsorbedElements` through the chosen-damage-type table.
/// The resistance is that type and no other, and the rider deals it
/// too.
///
/// Not a charged feature, for `FEATHER_FALL_TAG`'s reason: RAW prices
/// it in a reaction and a 1st-level slot, both of which the engine
/// already tracks and the hook already spends.
///
/// Held by the five chassis that have it on their spell list here:
/// Wizard, Sorcerer, Druid, Ranger and Artificer.
pub const ABSORB_ELEMENTS_TAG: &str = "spell.absorb_elements";

/// 5e Battle Smith Artificer level-9 feature **Arcane Jolt**, damage
/// half. "When either you or your steel defender hits a target with an
/// attack roll, you can channel magical energy through the strike to
/// create one of the following effects: the target takes an extra 2d6
/// force damage."
///
/// A row on `ONCE_PER_TURN_WEAPON_DIE_RIDERS` — the same lane Colossus
/// Slayer and Psychic Blades ride — because RAW's cadence is "you can
/// use this feature a number of times equal to your Intelligence
/// modifier", which on a chassis that swings twice a turn is
/// indistinguishable from once per turn for the length of a fight.
///
/// The biggest die on the cohort (2d6 against everyone else's 1d6 or
/// 1d8) and the reason the Battle Smith out-damages the other three
/// artificers on a single target despite carrying the smallest weapon:
/// the subclass's whole design is that the artificer's magic is
/// delivered by whatever is doing the hitting.
///
/// RAW's other half — spending the same trigger to heal a creature
/// within 30 ft for 2d6 instead — is left out. It is a second, opposite
/// effect on one trigger, and the cohort is a damage lane; a heal-on-hit
/// would be its own site with its own targeting question, and the
/// damage half is the one the subclass is played for.
///
/// The tag is carried by the artificer, not by the defender. RAW lets
/// the jolt ride the defender's rend too; that would be a second holder
/// for one charge, and the engine's rider cohort reads the swinging
/// actor's own tags.
pub const ARCANE_JOLT_TAG: &str = "artificer.arcane_jolt";

/// 5e Artillerist Artificer level-5 feature **Arcane Firearm**. "When
/// you cast an artificer spell through the firearm, roll a d8, and you
/// gain a bonus to one of the spell's damage rolls equal to the number
/// rolled."
///
/// A row on `FLAT_SPELL_DAMAGE_BONUSES`, and the second on that cohort
/// to roll rather than to read a modifier — the Wildfire Druid's
/// Enhanced Bond is the first, and the two are near-twins in shape: a
/// d8 on a damage roll, gated on something the subclass has set up.
/// Where they differ is what the gate reads. Enhanced Bond asks where
/// the druid's spirit is standing; this asks nothing at all beyond "is
/// this a spell", which makes the Artillerist the most reliable of the
/// four and the least positional.
///
/// The gate is `cast.school.is_some()`, which is the engine's marker for
/// "this action is a spell" — the same leg `is_cantrip` uses to
/// separate a cantrip from the level-0 frame every weapon swing opens.
/// RAW's "artificer spell" narrows it to the class list, which on a
/// chassis carrying only artificer spells is the same set.
pub const ARCANE_FIREARM_TAG: &str = "artificer.arcane_firearm";

/// 5e Alchemist Artificer level-5 feature **Alchemical Savant**. "Add
/// your Intelligence modifier to one roll of the spell's damage or
/// healing — acid, fire, necrotic, or poison damage, or healing."
///
/// A row on `FLAT_SPELL_DAMAGE_BONUSES` gated on the cast's declared
/// damage types, which is the axis `CastContext::damage_types` exists
/// for and the same one Elemental Affinity reads. Four types rather
/// than Elemental Affinity's one, so the Alchemist's bonus lands on
/// most of what it casts — Tasha's Caustic Brew, Create Bonfire,
/// Vitriolic Sphere, Fireball — and on none of the force / lightning /
/// psychic lane.
///
/// The healing half is dropped rather than deferred. The heal
/// chokepoint takes no cast frame, and adding the Intelligence modifier
/// to a heal would be a second site keyed off the same tag with no
/// shared body between them; the damage half reads at a site the engine
/// already has and is the half the subclass's own spell list is built
/// around.
pub const ALCHEMICAL_SAVANT_TAG: &str = "artificer.alchemical_savant";

/// 5e Armorer Artificer **Arcane Armor: Guardian** (subclass level 3),
/// mark half. "A creature hit by the gauntlet has disadvantage on attack
/// rolls against targets other than you until the end of your next
/// turn."
///
/// A row on `ON_HIT_CONDITION_MARKS` stamping `Dueled` — the condition
/// Compelled Duel installs and the Cavalier's Unwavering Mark reuses.
/// The three arrive at the same clause from a spell, a fighter subclass
/// and an artificer subclass, and the clause is one mechanic: swing at
/// me freely, swing at anyone else at disadvantage.
///
/// `melee_only`, because the gauntlets are a melee weapon and the
/// Guardian chassis carries no other. See `THUNDER_GAUNTLETS` for why
/// keying the mark on the holder rather than on the weapon is exact on
/// this chassis rather than merely close.
pub const THUNDER_GAUNTLETS_TAG: &str = "artificer.thunder_gauntlets";

/// 5e Armorer Artificer **Arcane Armor: Infiltrator** (subclass level
/// 3), rider half. "Once on each of your turns when you hit a creature
/// with it, you can deal an extra 1d6 lightning damage to that target."
///
/// A row on `ONCE_PER_TURN_WEAPON_DIE_RIDERS`, whose cadence is RAW's
/// wording verbatim. The Infiltrator's launcher is a 1d6 weapon, so the
/// rider doubles the turn's first connecting shot and leaves every one
/// after it at the base die — the exact opposite balance of its
/// Guardian sibling's flat 1d8, and the reason the two models play
/// differently despite sharing a chassis.
pub const LIGHTNING_LAUNCHER_TAG: &str = "artificer.lightning_launcher";

/// 5e Armorer Artificer **Defensive Field** (Guardian model, subclass
/// level 3). "As a bonus action, you gain temporary hit points equal to
/// your artificer level, replacing any temporary hit points you already
/// have. You lose these temporary hit points if you doff the armor."
///
/// Once per short rest here, where RAW spends a use of Arcane Armor's
/// own pool; the charge is the engine's translation of a pool it does
/// not track, and it is the same cadence every other per-fight posture
/// on this file carries.
pub const DEFENSIVE_FIELD_TAG: &str = "artificer.defensive_field";

/// 5e Alchemist Artificer level-3 feature **Experimental Elixir**.
/// "Whenever you finish a long rest, you can magically produce an
/// experimental elixir in an empty flask you touch. Roll on the
/// Experimental Elixir table for the effect."
///
/// The engine's version keeps the two things that make the feature what
/// it is — it is a bonus action, and *the alchemist does not choose what
/// they get* — and drops the six-entry table down to the three rows
/// with a combat surface: Healing, Swiftness, and Resilience. The other
/// three (Boldness, Flight, Transformation) are an ability-check bonus,
/// a movement mode the grid has no third dimension for, and a
/// stat-swap; none of them reads at a site the engine has.
///
/// Rolling for the effect rather than picking it is the point. Every
/// other bonus-action posture on the roster is a decision made with
/// full information; this one is a decision to *drink*, and what it
/// does is the die's business. That makes the Alchemist the only
/// chassis whose best turn cannot be planned, which is a fair rendering
/// of what the subclass is for.
pub const EXPERIMENTAL_ELIXIR_TAG: &str = "artificer.experimental_elixir";

/// 5e Artillerist Artificer level-3 feature **Eldritch Cannon**. "As a
/// magical action, you can use a spell slot or a use of this feature to
/// create a Small or Tiny eldritch cannon in an unoccupied space on a
/// horizontal surface within 5 feet of you. You can have only one
/// cannon at a time."
///
/// One tag shared by all three cannon declarations, which is how "only
/// one cannon at a time" is spelled: `FeatureSummon` reads the tag on
/// validate and spends it on resolve, so calling any one of the three
/// takes the charge the other two would have needed. The choice is
/// therefore real and one-way — the Artillerist picks a cannon for the
/// fight, not for the round.
pub const ELDRITCH_CANNON_TAG: &str = "artificer.eldritch_cannon";

/// 5e Battle Smith Artificer level-3 feature **Steel Defender**. "You've
/// learned how to build a mechanical creature to aid you in your
/// exploits."
///
/// Once per short rest, like the Artillerist's cannon and the Beast
/// Master's companion, and for the same reason all three carry a charge
/// rather than being permanent: RAW rebuilds a destroyed construct over
/// an hour's work, which is a rest in this engine's units. A Battle
/// Smith whose defender is killed has lost it for the fight.
pub const STEEL_DEFENDER_TAG: &str = "artificer.steel_defender";

/// Defensive Field — Armorer Artificer (Guardian) bonus action. Temp HP
/// equal to the artificer's level, once per short rest.
///
/// A bespoke `impl` rather than a row on a chassis because the roster's
/// self-temp-HP features each compute their amount differently and none
/// of them is a table today — Rally rolls a d10 and adds Charisma to an
/// *ally*, Form of Dread and Symbiotic Entity fold temp HP into a
/// posture that does three other things. What this one is, is the
/// simplest possible member of that family: no roll, no rider, no
/// duration, just the level as a number.
///
/// That flatness is the feature. An Armorer holding Defensive Field is
/// holding a known quantity, and the decision it poses is purely one of
/// timing — spent on round one it is temp HP that soaks the opening
/// exchange, held to round three it is a second health bar at the point
/// the first one is running out.
/// The Defensive Field pool, in temporary hit points.
///
/// RAW's amount is "equal to your artificer level", and a constant is
/// the honest rendering of that here rather than a read of
/// `ActorInstance::level`. The engine's `level` field is the dungeon
/// loop's progression counter and starts every instantiated template at
/// 1 — a template's *character* level is expressed by its stat block,
/// not by that field — so `level()` would make this action grant one
/// temporary hit point and cost a charge to do it.
///
/// 13 is the artificer level the whole chassis is written to: four
/// spell slot tiers, Flash of Genius (RAW lv7), Arcane Jolt (lv9). The
/// same reasoning, and the same shape, as `SYMBIOTIC_ENTITY_TEMP_HP`.
pub const DEFENSIVE_FIELD_TEMP_HP: u32 = 13;

pub struct DefensiveField {}

impl Action for DefensiveField {
    fn name(&self) -> &str {
        "defensive field"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["df", "field"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        // Temp HP is a buffer rather than healing, but the AI's support
        // pipeline is where "spend a turn on staying alive" is decided,
        // and that is the decision this action is.
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
        feature_ready(encounter, caster_id, DEFENSIVE_FIELD_TAG)
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
            actor.spend_feature(DEFENSIVE_FIELD_TAG);
        }
        encounter.log(format!(
            "  defensive field: the armor flares \u{2014} {} temp HP",
            DEFENSIVE_FIELD_TEMP_HP
        ));
        vec![Box::new(GainTempHp {
            actor_id: caster_id,
            amount: DEFENSIVE_FIELD_TEMP_HP,
        })]
    }
}

pub static DEFENSIVE_FIELD: LazyLock<DefensiveField> = LazyLock::new(|| DefensiveField {});

/// The three rows of RAW's Experimental Elixir table that have a combat
/// surface, in the order the d6 walks them.
///
/// Kept as data rather than as a `match` in the action body so the two
/// things a reader wants to know about the feature — what can come out
/// of the flask, and with what odds — are one list rather than a
/// control-flow graph. Adding a fourth row is a row.
///
/// Each entry is `(label, effect)`. The effect is applied by
/// `ExperimentalElixir::side_effects`, which is the only consumer.
enum ElixirEffect {
    /// RAW Healing: "The creature regains 2d4 + your Intelligence
    /// modifier hit points."
    Heal(Dice),
    /// RAW Swiftness (`Hasted`) and Resilience (`ShieldOfFaith`, +2 AC
    /// — RAW's "+1 bonus to AC for 10 minutes" rounded onto the AC
    /// condition the engine already has). Both are postures the engine
    /// installs as a timed condition, so they are one row shape.
    Posture(Condition, ConditionTimer),
}

/// The elixir table itself. Equal weights, walked by a d6 folded onto
/// its length — see `ExperimentalElixir::side_effects` for why the roll
/// is a real roll rather than a pick.
const EXPERIMENTAL_ELIXIRS: &[(&str, ElixirEffect)] = &[
    ("healing", ElixirEffect::Heal(Dice::new(2, 4))),
    (
        "swiftness",
        // RAW Swiftness is "+10 ft walking speed for 1 hour", which is
        // the doubled-speed clause and nothing else. It used to borrow
        // `Hasted` and pick up that condition's AC and Dexterity-save
        // clauses on the way past; `Fleet` is the clause on its own,
        // which is both closer to RAW and no longer a way for one of
        // three elixir rolls to hand out the Haste spell's extra
        // Action.
        ElixirEffect::Posture(Condition::Fleet, ConditionTimer::Rounds(10)),
    ),
    (
        "resilience",
        ElixirEffect::Posture(Condition::ShieldOfFaith, ConditionTimer::Rounds(10)),
    ),
];

/// Experimental Elixir — Alchemist Artificer bonus action, once per
/// short rest. Rolls one of `EXPERIMENTAL_ELIXIRS` and applies it to the
/// alchemist.
///
/// RAW lets the alchemist hand the flask to somebody else; here the
/// drinker is always the alchemist, because the interesting half of the
/// feature is the randomness rather than the targeting, and a random
/// effect aimed at an ally would ask the AI to answer "who wants an
/// unknown buff" — a question with no good answer.
pub struct ExperimentalElixir {}

impl Action for ExperimentalElixir {
    fn name(&self) -> &str {
        "experimental elixir"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["elixir", "ee"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        // One of the three rows is a heal and the other two are
        // survivability postures, so the support pipeline is the right
        // lane for all three — but the AI cannot know which it will get,
        // which is exactly the position the alchemist is in.
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
        feature_ready(encounter, caster_id, EXPERIMENTAL_ELIXIR_TAG)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // A real roll off the shared seeded roller, so an encounter
        // replayed from a seed pours the same elixir. `d6 % len` rather
        // than `d3` because RAW rolls a d6 on a six-row table and the
        // three rows we keep are the ones with a surface — the fold
        // keeps the die RAW names while the table stays honest about
        // what it implements.
        let face = encounter.roll(&Dice::new(1, 6)) as usize;
        let (label, effect) = &EXPERIMENTAL_ELIXIRS[(face - 1) % EXPERIMENTAL_ELIXIRS.len()];
        if let Some(actor) = encounter.actors.get_mut(&caster_id) {
            actor.spend_feature(EXPERIMENTAL_ELIXIR_TAG);
        }
        match effect {
            ElixirEffect::Heal(dice) => {
                let rolled = encounter.roll(dice);
                let int_mod = encounter
                    .actors
                    .get(&caster_id)
                    .map(|a| a.ability_modifier(AbilityScoreType::Intelligence))
                    .unwrap_or(0);
                let amount = (rolled as i32 + int_mod).max(0) as u32;
                encounter.log(format!(
                    "  experimental elixir: {} \u{2014} {}({}){:+} = {} HP",
                    label, dice, rolled, int_mod, amount
                ));
                vec![Box::new(Heal {
                    actor_id: caster_id,
                    amount,
                })]
            }
            ElixirEffect::Posture(condition, timer) => {
                encounter.log(format!("  experimental elixir: {}", label));
                vec![Box::new(ApplyCondition {
                    actor_id: caster_id,
                    condition: *condition,
                    timer: *timer,
                })]
            }
        }
    }
}

pub static EXPERIMENTAL_ELIXIR: LazyLock<ExperimentalElixir> =
    LazyLock::new(|| ExperimentalElixir {});

/// Eldritch Cannon (Flamethrower) — Artillerist Artificer action, once
/// per short rest. A Small turret on the artificer's team.
///
/// The three cannons share `ELDRITCH_CANNON_TAG`, so summoning this one
/// spends the charge the other two would have needed: RAW's "you can
/// have only one cannon at a time", enforced by the charge rather than
/// by a board scan.
///
/// Flamethrower is the crowd cannon. Its 15 ft cone (a 2-tile burst
/// here) catches everything standing near it and asks for a Dexterity
/// save, which makes it worth the most in exactly the situation the
/// other two are worth the least — several enemies converging on one
/// point. Against a single target it is the weakest of the three.
pub static SUMMON_FLAMETHROWER_CANNON: FeatureSummon = FeatureSummon {
    display_name: "eldritch cannon (flamethrower)",
    aliases: &["flamethrower", "flame cannon", "ecf"],
    tag: ELDRITCH_CANNON_TAG,
    template: &crate::actors::creatures::eldritch_cannons::FLAMETHROWER_CANNON_TEMPLATE,
    size: crate::engine::types::Size::Small,
    search_radius: 2,
    base_instance_id: 73,
    cost_resource: Resource::Action,
};

/// Eldritch Cannon (Force Ballista) — Artillerist Artificer action, once
/// per short rest. Shares `ELDRITCH_CANNON_TAG` with its two siblings.
///
/// The sniper cannon, and the one that keeps working when the others
/// don't. 2d8 force at 120 ft is the longest reach on the artificer's
/// side of the board and force is the damage type nothing in the
/// bestiary resists, so the ballista is the answer to the fire-immune
/// and the far-away alike.
pub static SUMMON_FORCE_BALLISTA_CANNON: FeatureSummon = FeatureSummon {
    display_name: "eldritch cannon (force ballista)",
    aliases: &["force ballista cannon", "ballista cannon", "ecb"],
    tag: ELDRITCH_CANNON_TAG,
    template: &crate::actors::creatures::eldritch_cannons::FORCE_BALLISTA_CANNON_TEMPLATE,
    size: crate::engine::types::Size::Small,
    search_radius: 2,
    base_instance_id: 74,
    cost_resource: Resource::Action,
};

/// Eldritch Cannon (Protector) — Artillerist Artificer action, once per
/// short rest. Shares `ELDRITCH_CANNON_TAG` with its two siblings.
///
/// The cannon that never attacks. Its whole turn is a pulse of temp HP
/// over every ally within 10 ft, which makes it the only one of the
/// three whose value depends on where the *party* is standing rather
/// than on where the enemy is — a Protector parked behind the front
/// line pays out every round for the rest of the fight, and one left
/// behind pays nothing.
pub static SUMMON_PROTECTOR_CANNON: FeatureSummon = FeatureSummon {
    display_name: "eldritch cannon (protector)",
    aliases: &["protector cannon", "protector", "ecp"],
    tag: ELDRITCH_CANNON_TAG,
    template: &crate::actors::creatures::eldritch_cannons::PROTECTOR_CANNON_TEMPLATE,
    size: crate::engine::types::Size::Small,
    search_radius: 2,
    base_instance_id: 75,
    cost_resource: Resource::Action,
};

/// Steel Defender — Battle Smith Artificer action, once per short rest.
/// A Medium construct on the artificer's team.
///
/// The sturdiest body on the feature-summon lane by a distance — more
/// hit points than the Ranger's companion, AC 15, and immunity to the
/// poison and charm effects that take a wolf out of a fight. That is
/// the trade the subclass makes: the Battle Smith's own weapon is a
/// d8 longsword swung off Intelligence, which is the weakest martial
/// output of the four artificers, and the defender is where the rest
/// of it went.
pub static SUMMON_STEEL_DEFENDER: FeatureSummon = FeatureSummon {
    display_name: "steel defender",
    aliases: &["defender", "sd"],
    tag: STEEL_DEFENDER_TAG,
    template: &crate::actors::creatures::steel_defenders::STEEL_DEFENDER_TEMPLATE,
    size: crate::engine::types::Size::Medium,
    search_radius: 2,
    base_instance_id: 76,
    cost_resource: Resource::Action,
};

/// Declarative chassis for an **at-will self-centered enemy burst**: an
/// action that catches every hostile creature around the actor in a
/// save-for-half blast, every round, with no charge to spend.
///
/// Distinct from `TurnBurst` (charged, installs a condition, deals no
/// damage) and from the monster `BreathWeapon` (charged by a recharge
/// roll, aimed at a point). What this describes is the thing a *turret*
/// does: it has one attack, it makes it every turn, and it makes it
/// where it is standing rather than where it chooses.
///
/// The DC is read off the acting creature's own sheet, which for the
/// Artillerist's cannon is the builder's numbers written onto the
/// construct — see `eldritch_cannons` for why the engine's summons
/// carry their summoner's statistics rather than a bond channel back
/// to them.
pub struct AtWillEnemyBurst {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    /// Footprint-gap radius of the blast, centered on the actor.
    pub radius: isize,
    pub damage_dice: Dice,
    pub damage_type: DamageType,
    /// Which save the targets roll.
    pub save_ability: AbilityScoreType,
    /// Which of the actor's abilities anchors the DC (`8 + prof + mod`).
    pub dc_ability: AbilityScoreType,
}

impl Action for AtWillEnemyBurst {
    fn self_burst_radius(&self) -> Option<isize> {
        // The chassis already carries the number; this is the row that
        // hands it to the AI.
        Some(self.radius)
    }
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type]
    }
    fn expected_damage(&self, _encounter: &EncounterInstance, _caster_id: usize) -> Option<f32> {
        // Save-for-half against one target averages three quarters of
        // the pool at even odds, which is the number the attack picker
        // wants when it is comparing this to a weapon swing.
        Some(self.damage_dice.average_roll() * 0.75)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = actor.location();
        let dc = actor.spell_save_dc(self.dc_ability);
        let rolled = encounter.roll(&self.damage_dice);
        encounter.log(format!(
            "  {}: {}({}) {} (DC {} {}, half on save)",
            self.display_name, self.damage_dice, rolled, self.damage_type, dc, self.save_ability,
        ));
        resolve_enemy_burst_save_damage(
            encounter,
            caster_id,
            center,
            self.radius,
            self.save_ability,
            dc,
            rolled,
            self.damage_type,
            SaveDamagePolicy::HalfOnSave,
        )
    }
}

/// Declarative chassis for an **at-will self-centered ally temp-HP
/// pulse**: an action that hands every nearby teammate the same
/// temporary hit points, every round, with no charge to spend.
///
/// The mirror image of `AtWillEnemyBurst` on the support lane, and it
/// exists for one creature — the Artillerist's Protector cannon, whose
/// entire turn this is. It is written as a chassis rather than as that
/// creature's bespoke action because the shape is not special: "pulse a
/// buff over everyone standing near me" is what an aura would be if the
/// engine had auras that acted, and the next one that lands is a
/// declaration.
///
/// The roll is made once and shared across every recipient, matching how
/// the engine's area effects roll — see `resolve_burst_save_damage`.
/// RAW rolls the Protector's die once for the whole pulse too.
pub struct AtWillAllyTempHpPulse {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    /// Footprint-gap radius of the pulse, centered on the actor.
    pub radius: isize,
    pub dice: Dice,
    /// Ability whose modifier is added to the shared roll.
    pub bonus_ability: AbilityScoreType,
}

impl Action for AtWillAllyTempHpPulse {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    /// The pulse's own envelope, declared so the AI's support rung can
    /// ask who is standing in it rather than guessing. Harmless on a
    /// `NoArgs` action — the reach gate in `validate_input` only fires
    /// when there is a target id or a target point to measure against,
    /// and this action takes neither.
    fn reach_tiles(&self) -> Option<isize> {
        Some(self.radius)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    /// The cohort's founding member, and now says so explicitly rather
    /// than being recognised by its `is_heal` flag — see
    /// `Action::pulses_ally_buff`.
    fn pulses_ally_buff(&self) -> bool {
        true
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(actor) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = actor.location();
        let bonus = actor.ability_modifier(self.bonus_ability);
        let rolled = encounter.roll(&self.dice);
        let amount = (rolled as i32 + bonus).max(0) as u32;
        let targets = encounter.ally_burst_targets(caster_id, center, self.radius);
        encounter.log(format!(
            "  {}: {}({}){:+} = {} temp HP to {} ally(s)",
            self.display_name,
            self.dice,
            rolled,
            bonus,
            amount,
            targets.len()
        ));
        targets
            .into_iter()
            .map(|id| {
                Box::new(GainTempHp {
                    actor_id: id,
                    amount,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

/// Flamethrower — the Artillerist cannon's action. A 15 ft cone in RAW,
/// rendered as a 2-tile burst centered on the turret: the engine has no
/// cone primitive, and a turret that cannot turn is a poor place to
/// introduce one.
pub static CANNON_FLAMETHROWER: AtWillEnemyBurst = AtWillEnemyBurst {
    display_name: "flamethrower",
    aliases: &["flame", "ft"],
    // 15 ft on the 2.5 ft grid is 6 tiles of cone length; as a burst
    // around a stationary turret, 2 tiles is the radius that catches the
    // same set of bodies without also catching the artificer standing
    // behind it — the burst is enemy-only, but the radius is still what
    // decides whether the cannon is worth parking forward.
    radius: 2,
    damage_dice: Dice::new(2, 8),
    damage_type: DamageType::Fire,
    save_ability: AbilityScoreType::Dexterity,
    dc_ability: AbilityScoreType::Intelligence,
};

/// Protector pulse — the Artillerist cannon's action in Protector mode.
/// RAW: "each creature of your choice within 10 feet of it gains
/// temporary hit points equal to 1d8 + your Intelligence modifier."
///
/// "Of your choice" becomes "every ally in range", which is the same set
/// on every board the engine can produce: there is no reason the
/// artificer would decline to shield a teammate, and the pulse cannot
/// reach an enemy.
pub static CANNON_PROTECTOR_PULSE: AtWillAllyTempHpPulse = AtWillAllyTempHpPulse {
    display_name: "protector pulse",
    aliases: &["pulse", "pp"],
    // 10 ft on the 2.5 ft grid.
    radius: 4,
    dice: Dice::new(1, 8),
    bonus_ability: AbilityScoreType::Intelligence,
};

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
    /// The coverage half — "every registered tag is carried by some
    /// template" — lives in its own test below. It was described here as
    /// unwritable because `pc_template_families` held class builds only,
    /// so a racial feature like the Dragonborn's Breath Weapon read as
    /// an orphan against it. The gap was in the registry, and the
    /// registry has since been filled.
    ///
    /// The pool half used to name the Battle Master's maneuvers
    /// specifically, which was the only pool that existed when it was
    /// written. It reads `SHARED_FEATURE_POOLS` now: a pool member on
    /// the short-rest list is a refill of a counter nothing reads, and
    /// naming one pool meant the next three were unchecked.
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
        for &(pool, members) in SHARED_FEATURE_POOLS {
            let listed: Vec<&str> = members
                .iter()
                .copied()
                .filter(|tag| short_rest.contains(tag))
                .collect();
            assert!(
                listed.is_empty(),
                "{:?} spend from {}, so refilling their own counters here is dead work",
                listed,
                pool
            );
        }
    }

    /// Every tag in the two per-rest registries is carried by a template
    /// something can actually instantiate.
    ///
    /// A tag is a `&'static str`, and the two consumers of one — the
    /// template's `features` set and the registry — are in different
    /// files with nothing tying them together. A typo in either, or a
    /// feature whose template was later renamed out from under it, does
    /// not fail to compile: the tag simply never matches, the charge
    /// never refreshes on a rest, and every test still passes. This is
    /// the check that catches it.
    ///
    /// The instantiable set is both halves of the engine's answer to
    /// "what exists" — `pc_template_families` for the playable
    /// templates, `EncounterInstance::template_pool` for the bestiary.
    /// Both are needed: roughly half these tags are monster features
    /// (the Doppelganger's Shapechanger, the Ghost's Horrifying
    /// Visage), and checking against the playable half alone would
    /// report them as orphans.
    #[test]
    fn every_per_rest_tag_is_carried_by_something_instantiable() {
        use crate::actors::creatures::pc_template_families;
        use crate::engine::encounter::EncounterInstance;

        let carried: Set<&str> = pc_template_families()
            .into_iter()
            .flat_map(|(_family, templates)| templates)
            .chain(EncounterInstance::template_pool())
            .flat_map(|t| t.features.iter().copied())
            .collect();

        let orphans: Vec<&str> = SHORT_REST_FEATURES
            .iter()
            .chain(BATTLE_MASTER_MANEUVERS.iter())
            .copied()
            .filter(|tag| !carried.contains(tag))
            .collect();
        assert!(
            orphans.is_empty(),
            "registered for a rest but on no instantiable template: {:?}",
            orphans
        );
    }

    /// Every row of `FEATURE_CHARGES` names a tag some instantiable
    /// template actually carries, names it once, and asks for a pool
    /// deeper than the one it would have had anyway.
    ///
    /// The same failure mode `every_per_rest_tag_is_carried_by_something_instantiable`
    /// exists for, one lane over: a tag is a `&'static str`, and a
    /// misspelled row here silently sizes nothing. A duplicate row is
    /// worse than useless — `feature_charges` takes the first match, so
    /// the second would be a number that looks authoritative and is
    /// never read. And a row asking for one charge is a row that changes
    /// nothing, which is a claim about a feature that isn't true.
    #[test]
    fn every_charge_pool_names_a_real_feature_and_deepens_it() {
        use crate::actors::creatures::pc_template_families;
        use crate::engine::encounter::EncounterInstance;

        let names: Set<&str> = FEATURE_CHARGES.iter().map(|(tag, _)| *tag).collect();
        assert_eq!(
            names.len(),
            FEATURE_CHARGES.len(),
            "FEATURE_CHARGES lists some tag more than once; only the first is read"
        );
        for (tag, count) in FEATURE_CHARGES {
            assert!(
                *count > 1,
                "{} asks for {} charges, which is what it would get without a row",
                tag,
                count
            );
        }
        let carried: Set<&str> = pc_template_families()
            .into_iter()
            .flat_map(|(_family, templates)| templates)
            .chain(EncounterInstance::template_pool())
            .flat_map(|t| t.features.iter().copied())
            .collect();
        let orphans: Vec<&str> = names
            .iter()
            .copied()
            .filter(|tag| !carried.contains(tag))
            .collect();
        assert!(
            orphans.is_empty(),
            "given a charge pool but on no instantiable template: {:?}",
            orphans
        );
    }

    /// Every template that knows a shared-pool member also carries the
    /// pool that member spends from.
    ///
    /// The redirect in `ActorInstance::charge_counter_for` is opt-in per
    /// actor: a maneuver whose holder lacks `SUPERIORITY_DICE_TAG` falls
    /// back to a private charge of its own. That fallback is the right
    /// behavior for a fixture grafting one feature onto an unrelated
    /// chassis, and exactly the wrong thing to ship — a Battle Master
    /// missing the pool row is a Battle Master with fourteen uses of a
    /// four-use feature, and every other test in the suite passes while
    /// it is true.
    ///
    /// The pool itself is allowed to ride alone: a chassis could carry
    /// dice it has no maneuver to spend them on (nothing does today, and
    /// the reverse direction is the one that changes what a fight looks
    /// like).
    #[test]
    fn every_pool_member_ships_with_its_pool() {
        use crate::actors::creatures::pc_template_families;
        use crate::engine::encounter::EncounterInstance;

        let templates: Vec<&'static crate::actors::actor_template::CreatureTemplate> =
            pc_template_families()
                .into_iter()
                .flat_map(|(_family, templates)| templates)
                .chain(EncounterInstance::template_pool())
                .collect();
        for &(pool, members) in SHARED_FEATURE_POOLS {
            for template in &templates {
                let known: Vec<&str> = members
                    .iter()
                    .copied()
                    .filter(|tag| template.features.contains(tag))
                    .collect();
                assert!(
                    known.is_empty() || template.features.contains(&pool),
                    "{} knows {:?} but carries no {} to spend on them",
                    template.name,
                    known,
                    pool
                );
            }
        }
    }

    /// No tag belongs to two pools, and no pool is a member of itself.
    ///
    /// `shared_pool_for` takes the first match, so a tag on two rows
    /// would silently spend from whichever row was written first — and a
    /// pool listing itself would make `charge_counter_for` resolve the
    /// pool to the pool, which happens to terminate today only because
    /// the lookup is one level deep.
    #[test]
    fn the_shared_pools_partition_their_members() {
        let mut seen: Set<&str> = Set::new();
        for &(pool, members) in SHARED_FEATURE_POOLS {
            for &tag in members {
                assert!(
                    seen.insert(tag),
                    "{} is a member of more than one shared pool",
                    tag
                );
                assert_ne!(tag, pool, "{} lists itself as one of its members", pool);
            }
        }
    }

    /// Every summoned body the engine can put on the board from a
    /// class feature, paired with the instance band it is named out of.
    ///
    /// Two lists behind one name because the summons are two shapes:
    /// the eight rows of `FEATURE_SUMMONS` and the two Circle of the
    /// Shepherd totems, which are not on that cohort because their
    /// payout happens as they land rather than after. Both hand a band
    /// to `spawn_adjacent_summons`, so both have to be in any sweep
    /// that claims the bands don't collide.
    fn every_summon_band() -> Vec<(&'static str, usize)> {
        let mut out: Vec<(&'static str, usize)> = FEATURE_SUMMONS
            .iter()
            .map(|s| (s.display_name, s.base_instance_id))
            .collect();
        for totem in [&*SPIRIT_TOTEM_BEAR, &*SPIRIT_TOTEM_UNICORN] {
            out.push((totem.display_name, totem.base_instance_id));
        }
        out
    }

    /// No two summons are named out of the same band.
    ///
    /// A party can have several of these out at once — a Beast Master,
    /// a Wildfire druid and a Fathomless warlock put three on the board
    /// between them — and `instantiate_creature` keys the display name
    /// off the band, so a collision renames somebody's summon after
    /// somebody else's.
    ///
    /// The sweep used to read a three-name list written inside the
    /// assertion, which is why it never noticed that the artificer's
    /// three cannons and the steel defender were not in it at all. That
    /// omission was not free: the next summon added took band 73 and
    /// collided with the flamethrower cannon, and the test that was
    /// supposed to catch exactly that passed.
    #[test]
    fn no_two_summons_are_named_out_of_the_same_band() {
        let bands = every_summon_band();
        for (i, (name, band)) in bands.iter().enumerate() {
            for (other, other_band) in bands.iter().skip(i + 1) {
                assert_ne!(
                    band, other_band,
                    "{} and {} share instance band {}",
                    name, other, band
                );
            }
            assert!(
                *band < SPELL_SUMMON_BAND_FLOOR,
                "{} wandered into the summoning spells' bands",
                name
            );
        }
    }

    /// Every charge a summon spends is a charge something gives back.
    ///
    /// The Ranger's Companion is the long-rest exception — RAW rebonds
    /// the beast over a long rest and not a short one — and it is named
    /// rather than defaulted so a summon that simply forgot to register
    /// fails here instead of quietly costing its owner the subclass for
    /// the rest of the day. Which is what happened: the tentacle
    /// shipped with no refresh at all, and this assertion is how it was
    /// found.
    #[test]
    fn every_summon_charge_comes_back_on_a_rest() {
        let charges = FEATURE_SUMMONS
            .iter()
            .map(|s| (s.display_name, s.tag))
            .chain([("spirit totem", SPIRIT_TOTEM_TAG)]);
        for (name, tag) in charges {
            assert!(
                SHORT_REST_FEATURES.contains(&tag) || tag == RANGERS_COMPANION_TAG,
                "{} spends a charge nothing gives back",
                name
            );
        }
    }

    /// Two summons share a charge only when RAW says they are one
    /// summon wearing several names.
    ///
    /// Two rows are allowed to collide here and both are deliberate:
    /// the Artillerist's three cannons ("you can have only one cannon
    /// at a time"), and the Shepherd's two totems, which are two builds
    /// rather than two features. Anything else sharing a tag would be
    /// two subclasses quietly spending each other's charge, which is
    /// what this pins.
    #[test]
    fn only_the_declared_families_share_a_summon_charge() {
        use std::collections::HashMap as Map;
        let mut by_tag: Map<&str, Vec<&str>> = Map::new();
        for s in FEATURE_SUMMONS {
            by_tag.entry(s.tag).or_default().push(s.display_name);
        }
        for (tag, names) in by_tag {
            if names.len() == 1 {
                continue;
            }
            assert_eq!(
                tag, ELDRITCH_CANNON_TAG,
                "{:?} share the charge {} without being one feature",
                names, tag
            );
        }
    }

    /// RAW gives a Cleric and a Paladin their Channel Divinity back on
    /// a short rest, without exception and in the same words for both
    /// classes. So this is a one-line rule with a registry attached, and
    /// the registry is the point: the rule was being applied a tag at a
    /// time by whoever added the feature, and three of the nineteen had
    /// been missed — Turn Undead, Sacred Weapon and Vow of Enmity, all
    /// of them on baseline templates rather than subclass clones.
    ///
    /// The failure was invisible from the inside. A tag that isn't on
    /// the short-rest lane doesn't error; it just waits for the long
    /// rest, and a Cleric who could Turn Undead once a day instead of
    /// once a fight looks exactly like a Cleric who has already used it.
    #[test]
    fn every_channel_divinity_comes_back_on_a_short_rest() {
        // Each option draws on its class's pool, and the pool is what
        // the rest refills — so the cadence rule is now "every option
        // belongs to a pool, and every pool refreshes on a short rest".
        let stranded: Vec<&str> = channel_divinity_features()
            .filter(|tag| {
                shared_pool_for(tag).is_none_or(|pool| !SHORT_REST_FEATURES.contains(&pool))
            })
            .collect();
        assert!(
            stranded.is_empty(),
            "these Channel Divinities only recharge on a long rest: {stranded:?}"
        );
    }

    /// The registry names Channel Divinities that some playable
    /// template actually carries. A tag listed here but carried by
    /// nobody proves nothing, and one carried but not listed escapes
    /// the cadence rule above — which is exactly how the three
    /// stragglers survived.
    #[test]
    fn the_channel_divinity_registry_matches_the_roster() {
        use crate::actors::creatures::pc_template_families;
        use std::collections::HashSet;

        let carried: HashSet<&str> = pc_template_families()
            .into_iter()
            .flat_map(|(_family, templates)| templates)
            .flat_map(|t| t.features.iter().copied())
            .collect();
        let orphans: Vec<&str> = channel_divinity_features()
            .filter(|tag| !carried.contains(tag))
            .collect();
        assert!(
            orphans.is_empty(),
            "these Channel Divinities are on no playable template: {orphans:?}"
        );
    }
}
