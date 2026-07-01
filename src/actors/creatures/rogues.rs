use crate::actions::class_attacks::ROGUE_SHORTSWORD;
use crate::actions::class_features::{
    ASSASSINATE_TAG, CUNNING_DASH, CUNNING_DISENGAGE, CUNNING_HIDE, CUNNING_STRIKE_DAZE,
    CUNNING_STRIKE_POISON, CUNNING_STRIKE_TRIP, CUNNING_STRIKE_WITHDRAW, STEADY_AIM,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Rogue PC template. Light armor (AC 14: leather + DEX), modest HP,
/// DEX-primary. The headline mechanic is **Sneak Attack** — the
/// shortsword (finesse, DEX-based 1d6) deals an extra 1d6 once per turn
/// when the rogue has advantage OR an ally is adjacent to the target.
/// `rolls_death_saves: true` (PC) so it enters the dying state at 0 HP.
pub static ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ROGUE_SHORTSWORD);
    actions.push(&*CUNNING_DASH);
    actions.push(&*CUNNING_DISENGAGE);
    actions.push(&*CUNNING_HIDE);
    // 5e Tasha's Rogue Steady Aim (lv3): bonus action; advantage on next
    // attack at the cost of zeroing speed for the rest of the turn.
    // Pairs naturally with Sneak Attack's advantage trigger so a sniping
    // rogue can fire mid-encounter without needing an adjacent ally.
    actions.push(&*STEADY_AIM);
    // 5e 2024 Rogue Cunning Strike (lv5): bonus-action primes that trade
    // Sneak Attack dice for tactical effects on the next sneak hit.
    // Mutually exclusive (one prime at a time) — the shortsword's
    // `consume_cunning_strike` chokepoint picks the first active prime
    // and applies its effect.
    actions.push(&*CUNNING_STRIKE_POISON);
    actions.push(&*CUNNING_STRIKE_TRIP);
    actions.push(&*CUNNING_STRIKE_WITHDRAW);
    actions.push(&*CUNNING_STRIKE_DAZE);
    CreatureTemplate {
        name: "Rogue",
        glyph: 'R',
        ac: 14,
        hitpoints: "3d8+3".parse().unwrap(),
        strength: 10,
        dexterity: 16, // primary
        constitution: 12,
        intelligence: 12,
        wisdom: 12,
        charisma: 10,
        languages: HashSet::from([Language::Common, Language::ThievesCant]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        // Rogues are proficient in DEX and INT saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Intelligence,
        ]),
        has_evasion: true,
        has_uncanny_dodge: true,
        // 5e Rogue Elusive (level 18 capstone): no attack roll has
        // advantage against the rogue while they aren't Incapacitated.
        // Ships on the CR-1 rogue template above its strict RAW level
        // gate for the same reason Improved Divine Smite ships on the
        // CR-1.5 paladin and Purity of Body ships on the CR-1.5 monk —
        // class templates target a balanced playable level, not
        // lockstep PHB progression. Composes cleanly with Evasion
        // (already on) and Uncanny Dodge (already on): the elusive
        // rogue drops Advantage on incoming hits, halves whichever hits
        // land (Uncanny Dodge, once per round), and takes no damage on
        // successful DEX saves (Evasion).
        has_elusive: true,
        // 5e Rogue Slippery Mind (level 15): proficient in Wisdom
        // saves. The narrower sibling to the monk's Diamond Soul (all
        // six saves) — Slippery Mind converts the rogue's WIS save
        // from a "bad save" into a "good save" so Hold Person / Dominate
        // Person / Compulsion / Fear no longer reliably lock the rogue
        // down. Ships on the CR-1 rogue template above its strict RAW
        // level gate alongside Elusive for the same reason (class
        // templates target a balanced playable level, not lockstep PHB
        // progression). Composes cleanly with the Paladin's Aura of
        // Protection — the CHA-bonus save layer stacks on top of the
        // slippery-mind proficiency floor.
        has_slippery_mind: true,
        ..CreatureTemplate::defaults()
    }
});

/// Assassin Rogue — subclass build. Identical envelope to the baseline
/// `ROGUE_TEMPLATE` (level-7 build, shortsword + cunning suite, evasion,
/// uncanny dodge) with one subclass feature layered on: **Assassinate**
/// (level 3) — advantage on every attack roll against any creature that
/// hasn't taken a turn in the combat yet.
///
/// The "alpha-strike" rogue: opens combat with a guaranteed-advantage
/// shortsword swing (the once-per-turn Sneak Attack rider keys off
/// advantage as one of its triggers, so the alpha hit lands the +Nd6
/// without needing a flanking ally). RAW also lets a hit against a
/// surprised target be a critical, but we don't model the Surprised
/// state — the advantage half (the load-bearing piece) survives intact.
///
/// Distinct from `ROGUE_TEMPLATE` (Thief-equivalent baseline) so an
/// Assassin-vs-Thief or Assassin-vs-baseline encounter renders
/// unambiguously by name and the subclass feature doesn't accidentally
/// stack RAW-illegally on a single PC build. Glyph 'A' so the Assassin
/// shows up distinctly on the map next to the baseline 'R'.
pub static ASSASSIN_ROGUE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Rogue envelope wholesale
    // and overwrite only the per-subclass differences (name / glyph /
    // features). The `..base.clone()` tail picks up every other field
    // — actions, save profs, evasion, uncanny dodge, HP dice — without
    // an N-line field-by-field copy. Same shape as
    // `HUNTER_RANGER_TEMPLATE`.
    CreatureTemplate {
        name: "Assassin Rogue",
        glyph: 'A',
        features: HashSet::from([ASSASSINATE_TAG]),
        ..ROGUE_TEMPLATE.clone()
    }
});
