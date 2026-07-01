use crate::actions::class_features::{
    FLURRY_OF_BLOWS, PATIENT_DEFENSE, PURITY_OF_BODY_TAG, STEP_OF_THE_WIND, STILLNESS_OF_MIND,
    STUNNING_STRIKE, STUNNING_STRIKE_TAG, WHOLENESS_OF_BODY, WHOLENESS_OF_BODY_TAG,
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
    CreatureTemplate {
        name: "Monk",
        glyph: 'M',
        ac: 15, // Unarmored Defense baseline (10 + DEX + WIS at +3/+2 = 15).
        hitpoints: "5d8+5".parse().unwrap(),
        speed: 40., // Unarmored Movement bonus (+10ft at level 2+).
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
        features: HashSet::from([STUNNING_STRIKE_TAG, PURITY_OF_BODY_TAG]),
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
