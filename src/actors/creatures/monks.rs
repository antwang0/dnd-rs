use crate::actions::class_features::{
    EMPTY_BODY, EMPTY_BODY_TAG, FLURRY_OF_BLOWS, PATIENT_DEFENSE, PURITY_OF_BODY_TAG,
    STEP_OF_THE_WIND, STILLNESS_OF_MIND, STUNNING_STRIKE, STUNNING_STRIKE_TAG,
    TOUCH_OF_DEATH_TAG, UNARMORED_MOVEMENT_TAG, WHOLENESS_OF_BODY, WHOLENESS_OF_BODY_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::MONK_UNARMED_STRIKE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Monk PC template. Unarmored (Wisdom + Dex AC scaling — we collapse
/// the formula into a flat AC 15 for now), DEX-primary, with a high
/// WIS secondary that anchors the Stunning Strike DC. Headline
/// mechanics:
/// - **Martial Arts** (action): 1d8+DEX bludgeoning unarmed strike,
///   the staple attack.
/// - **Stunning Strike** (bonus action, 1/rest): primes the next melee
///   hit; on connect, target makes a CON save (8 + prof + WIS) or is
///   Stunned for 1 round.
/// - **Patient Defense** (bonus action, at-will): take the Dodge action
///   for free defensive disadvantage on incoming attacks.
/// - **Flurry of Blows** (bonus action, at-will): grants an extra Action
///   for a follow-up Martial Arts strike — doubles the per-turn swing
///   cap when the bonus action is otherwise idle.
///
/// Stats target a level-5 monk: 33 HP (5d8+5), AC 15 (unarmored
/// defense baseline), DEX 16 / WIS 14, no spells. PC flag flips on so
/// the monk enters Dying at 0 HP rather than dropping straight to dead.
pub static MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MONK_UNARMED_STRIKE);
    actions.push(&*STUNNING_STRIKE);
    actions.push(&*PATIENT_DEFENSE);
    actions.push(&*FLURRY_OF_BLOWS);
    actions.push(&*STILLNESS_OF_MIND);
    actions.push(&*STEP_OF_THE_WIND);
    // Empty Body — RAW level 18 monk capstone-adjacent, once per long
    // rest. Ships on the CR-1.5 monk template above its strict RAW
    // level gate for the same reason Diamond Soul / Purity of Body do
    // (class templates target a balanced playable level, not lockstep
    // PHB progression). Action; installs Invisible + DamageResistant
    // on self for 10 rounds (1 minute RAW).
    actions.push(&*EMPTY_BODY);
    CreatureTemplate {
        name: "Monk",
        glyph: 'M',
        ac: 15, // Unarmored Defense baseline (10 + DEX + WIS at +3/+2 = 15).
        hitpoints: "5d8+5".parse().unwrap(),
        // 5e default humanoid walking speed. The RAW Unarmored Movement
        // +10 ft bump lives on the `UNARMORED_MOVEMENT_TAG` passive-
        // feature entry below rather than pre-baked into this field —
        // the `PASSIVE_FEATURE_SPEED_BONUSES` table adds it back so
        // `speed()` still reads 40 ft, but the feature stays declarative
        // (visible on template diffs, dial-able via
        // `grant_feature_for_test`) rather than a magic-number 40 with
        // a comment.
        speed: 30.,
        strength: 12,
        dexterity: 16, // primary attack stat
        constitution: 12,
        intelligence: 10,
        wisdom: 14, // Stunning Strike DC anchor
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 1.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        // Monks are proficient in STR and DEX saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
        ]),
        // Passive class features:
        //   - `STUNNING_STRIKE_TAG`: once-per-rest bonus-action prime →
        //     next melee hit lands a CON-save Stun rider.
        //   - `PURITY_OF_BODY_TAG` (level 10): passive Poisoned-condition
        //     AND poison-damage immunity. RAW "immune to disease and
        //     poison" — the disease half has no combat surface in our
        //     engine, but both poison halves fire (condition install
        //     bounces at `dynamic_immunity_to`; damage zeroes at
        //     `effective_damage`). Ships on the CR-1.5 monk template
        //     above its strict RAW level gate for the same reason
        //     Improved Divine Smite ships on the CR-1.5 paladin — class
        //     templates target a balanced playable level, not lockstep
        //     PHB progression.
        //   - `UNARMORED_MOVEMENT_TAG` (level 2): always-on +10 ft
        //     walking speed. Read at the shared
        //     `PASSIVE_FEATURE_SPEED_BONUSES` chokepoint next to Fast
        //     Movement (Barbarian lv5) — same +10 magnitude, same
        //     always-on cadence, different class chassis. The Monk's
        //     `speed` field baseline is 30 ft (the default humanoid
        //     walking speed) and the tag folds the +10 back through
        //     the shared table, so `speed()` still reads 40 ft.
        features: HashSet::from([
            STUNNING_STRIKE_TAG,
            PURITY_OF_BODY_TAG,
            EMPTY_BODY_TAG,
            UNARMORED_MOVEMENT_TAG,
        ]),
        has_evasion: true,
        has_deflect_missiles: true,
        has_extra_attack: true,
        // 5e Monk Diamond Soul (level 14 passive): proficient in every
        // saving throw. Ships on the CR-1.5 monk template above its
        // strict RAW level gate for the same reason Purity of Body
        // (lv10) ships here — class templates target a balanced
        // playable level, not lockstep PHB progression. Read by
        // `is_save_proficient` — the monk now rolls prof + ability on
        // every save, layering on top of Evasion (0 damage on passed
        // DEX save) and the paladin's Aura of Protection (+CHA to
        // every save when adjacent).
        has_diamond_soul: true,
        ..CreatureTemplate::defaults()
    }
});

/// Open Hand Monk — Way of the Open Hand subclass build. Identical
/// envelope to the baseline `MONK_TEMPLATE` (unarmored AC 15, unarmed
/// strike, Stunning Strike + Patient Defense + Flurry of Blows +
/// Stillness of Mind + Step of the Wind, evasion + deflect missiles +
/// extra attack) with one subclass feature layered on: **Wholeness of
/// Body** (lv6 subclass action, once per long rest) — heal self for
/// `3 × level` HP.
///
/// Pairs naturally with the monk's evasion / patient-defense kit: the
/// open-hand monk plays the staying-power skirmisher — dodges incoming
/// damage with Patient Defense, then refills the HP bar with Wholeness
/// of Body once per fight without burning a teammate's slot. Distinct
/// from `MONK_TEMPLATE` (Way of the Mercy / Shadow / Long Death-equivalent
/// baseline) so an Open-Hand-vs-baseline encounter renders unambiguously
/// by name. Glyph 'O' so the Open Hand monk shows up distinctly on the
/// map next to the baseline 'M'.
pub static OPEN_HAND_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Monk envelope wholesale
    // and overwrite only the per-subclass differences (name / glyph /
    // actions / features). The `..base.clone()` tail picks up every
    // other field — stats, save profs, evasion / deflect missiles /
    // extra-attack — without an N-line field-by-field copy. Same shape
    // as `HUNTER_RANGER_TEMPLATE` / `ASSASSIN_ROGUE_TEMPLATE` /
    // `VENGEANCE_PALADIN_TEMPLATE`.
    let mut actions = MONK_TEMPLATE.actions.clone();
    actions.push(&*WHOLENESS_OF_BODY);
    let mut features = MONK_TEMPLATE.features.clone();
    features.insert(WHOLENESS_OF_BODY_TAG);
    CreatureTemplate {
        name: "Open Hand Monk",
        glyph: 'O',
        actions,
        features,
        ..MONK_TEMPLATE.clone()
    }
});

/// Long Death Monk — Way of the Long Death subclass build (SCAG).
/// Identical envelope to the baseline `MONK_TEMPLATE` (unarmored AC
/// 15, unarmed strike, Stunning Strike + Patient Defense + Flurry of
/// Blows + Stillness of Mind + Step of the Wind + Empty Body, evasion
/// + deflect missiles + extra attack, Purity of Body + Diamond Soul +
/// Unarmored Movement passives) with one subclass passive layered on:
/// **Touch of Death** (lv3 subclass tell) — whenever the Long Death
/// monk's damage reduces a hostile creature to 0 HP, the monk gains
/// `max(1, 1 + CON mod + monk level)` temporary HP.
///
/// Pairs naturally with the monk's strike-and-move skirmisher kit:
/// the Long Death monk chains kills into a self-refilling temp HP
/// pool without spending an action, complementing the reactive Patient
/// Defense (bonus-action Dodge) and Wholeness of Body-style hard heal
/// on the Open Hand cousin. Where Open Hand fills the HP bar once per
/// fight with a chunky heal, Long Death fills the temp HP absorb
/// buffer every time an enemy drops — the two subclasses hit the
/// staying-power lane from different angles.
///
/// The signature "kill-triggered temp HP" tell is a chassis-level
/// cousin of Fiend Warlock's **Dark One's Blessing** (CHA mod + level
/// on the same trigger). Both share the `KILL_TRIGGERED_TEMP_HP_SOURCES`
/// cohort in `EncounterInstance::trigger_kill_triggered_temp_hp` — one
/// shared iteration reads whichever tag the swinger holds and applies
/// the row's stat + level formula. The two never legally co-occur on
/// a single build (Warlock Fiend Patron vs. Monk Long Death Way are
/// distinct classes with distinct subclasses).
///
/// RAW's Way of the Long Death picks up other features not shipped on
/// this template — **Hour of Reaping** (lv6: 30ft self-centered WIS
/// save vs Frightened burst; needs a per-turn cost surface for the
/// action), **Mastery of Death** (lv11: on drop to 0 HP, spend 1 ki
/// point to stay at 1 HP; needs a Downed-outcome intercept keyed off
/// a ki-point pool this engine doesn't track), and **Touch of the
/// Long Death** (lv17: cost 5+ ki for a 10d10 necrotic single-target
/// touch attack). The lv3 Touch of Death passive is the load-bearing
/// tactical feature with a first-class engine surface today, so we
/// ship that half and leave the rest as future work — matching the
/// way `OPEN_HAND_MONK_TEMPLATE` ships only the Wholeness of Body
/// (lv6) half of its RAW Way of the Open Hand kit.
///
/// Ships on the CR-1.5 monk chassis at (or above) its strict RAW lv3
/// gate for the same reason `OPEN_HAND_MONK_TEMPLATE` ships Wholeness
/// of Body (RAW lv6) and every other subclass template runs above its
/// strict RAW gate — class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Distinct from `MONK_TEMPLATE` (subclass-less baseline) and
/// `OPEN_HAND_MONK_TEMPLATE` (Wholeness of Body) so a Long-Death-vs-
/// baseline / vs-Open-Hand encounter renders unambiguously by name.
/// Glyph 'D' (for the Long **D**eath way) so the Long Death monk shows
/// up distinctly on the map next to baseline 'M' and Open Hand 'O'.
pub static LONG_DEATH_MONK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Monk envelope wholesale
    // and layers on the Way of the Long Death lv3 feature tag
    // (`TOUCH_OF_DEATH_TAG`: kill-triggered temp HP grant read at the
    // `DealDamage` chokepoint via the shared
    // `KILL_TRIGGERED_TEMP_HP_SOURCES` cohort next to Fiend Warlock's
    // Dark One's Blessing). The `..base.clone()` tail inside the helper
    // picks up every other field — actions, stats, save profs,
    // evasion / deflect missiles / extra-attack, Purity of Body /
    // Diamond Soul / Unarmored Movement passives — without an N-line
    // field-by-field copy. No new actions are pushed — Touch of Death
    // is a purely passive kill-triggered temp HP grant, not a fresh
    // action surface, so the "tag-only" shape the helper wraps is a
    // natural fit (mirrors `FORGE_CLERIC_TEMPLATE` /
    // `TWILIGHT_CLERIC_TEMPLATE` / `NECROMANCY_WIZARD_TEMPLATE` /
    // `WAR_MAGIC_WIZARD_TEMPLATE` / `LIFE_CLERIC_TEMPLATE` /
    // `SHADOW_MAGIC_SORCERER_TEMPLATE` / `ABERRANT_MIND_SORCERER_TEMPLATE`
    // / `DIVINE_SOUL_SORCERER_TEMPLATE` on the same cross-class
    // helper). Glyph 'D' — for the Long **D**eath way; distinct from
    // baseline monk 'M' and Open Hand 'O'.
    MONK_TEMPLATE.with_subclass_tag("Long Death Monk", 'D', TOUCH_OF_DEATH_TAG)
});
