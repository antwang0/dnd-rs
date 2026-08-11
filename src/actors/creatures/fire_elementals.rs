use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::FIRE_ELEMENTAL_TOUCH;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// The 9-condition immunity envelope every elemental in this engine
/// shares (Charmed / Frightened / Paralyzed / Petrified / Poisoned /
/// Asleep / Prone / Grappled / Restrained). Lifted to a `LazyLock` so
/// each elemental template clones the same value rather than repeating
/// a 9-line literal — a new elemental added later opts in by cloning
/// this set into `condition_immunities`.
pub static ELEMENTAL_CONDITION_IMMUNITIES: LazyLock<HashSet<Condition>> = LazyLock::new(|| {
    HashSet::from([
        Condition::Charmed,
        Condition::Frightened,
        Condition::Paralyzed,
        Condition::Petrified,
        Condition::Poisoned,
        Condition::Asleep,
        Condition::Prone,
        Condition::Grappled,
        Condition::Restrained,
    ])
});

/// Defence baseline every elemental in this engine shares, as a whole
/// `CreatureTemplate` for the `..` tail of a template literal:
///
///   - **Poison immunity** — elementals have no biology to poison.
///   - **Resistance to bludgeoning / piercing / slashing from
///     nonmagical attacks** — 5e's standard elemental clause, and
///     genuinely qualified. Seventeen stat blocks route through here and
///     every one of their docstrings already said "non-magical
///     physical"; the map they were handed said something else.
///   - **The elemental condition envelope** —
///     `ELEMENTAL_CONDITION_IMMUNITIES`.
///
/// Each template overlays its own signature entries (Fire / Cold /
/// Lightning / Acid immunity or resistance, Earth's thunder
/// vulnerability) through `overlays`.
///
/// # Why a template tail and not a map
///
/// The predecessor returned a bare `HashMap` for the `damage_modifiers`
/// field, and dropped the BPS triplet into it *unqualified* with a
/// docstring conceding "the magical-vs-non-magical split is collapsed
/// since the engine doesn't track weapon magicality". The engine now
/// does — see `crate::engine::magic` — and the qualification is exactly
/// the kind of thing a bare map cannot express, because it lives in a
/// second field. `CreatureTemplate::resistant_to_nonmagical_physical`
/// is the constructor that owns that pairing, and this one is built on
/// it for the reason its own docstring gives: a qualified resistance is
/// only correct alongside the absence of an unqualified one, and the
/// `..` tail is the one position a template cannot get that wrong.
///
/// Folding the condition immunities in at the same time is not scope
/// creep but the same fact: every one of the seventeen call sites wrote
/// `condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone()` on
/// the line after the damage modifiers, because being an elemental is
/// one property and not two.
///
/// Overlays land in the *unqualified* table, so an overlay for a
/// physical type still wins outright: `nonmagical_damage_modifier`
/// returns `None` for any type the unqualified map already carries, so
/// a magmin promoted to full bludgeoning immunity would take the
/// immunity and not a second halving underneath it.
pub fn elemental_defaults(
    overlays: impl IntoIterator<Item = (DamageType, DamageModifier)>,
) -> CreatureTemplate {
    let mut damage_modifiers = HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]);
    damage_modifiers.extend(overlays);
    CreatureTemplate {
        damage_modifiers,
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
}

/// Fire Elemental — CR 5 elemental. Walking inferno: fire-touch melee
/// for 2d6 + ignite (Burning DOT). Immune to fire and poison, resistant
/// to non-magical physical damage (we collapse to "physical resistance"
/// since the engine doesn't yet track magical-weapon distinctions).
/// Elemental condition envelope: ignores almost every mind / body
/// control condition (Charmed, Frightened, Paralyzed, Petrified,
/// Poisoned, Asleep, Prone) — close to the 5e MM line.
pub static FIRE_ELEMENTAL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_ELEMENTAL_TOUCH);
    CreatureTemplate {
        name: "Fire Elemental",
        // 'E' (uppercase) — free; uppercase 'F' is the fighter.
        glyph: 'E',
        ac: 13,
        // 10d10+20 = 75 average per MM.
        hitpoints: "10d10+20".parse().unwrap(),
        speed: 50.,
        strength: 10,
        intelligence: 6,
        dexterity: 17,
        wisdom: 10,
        constitution: 16,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        // 5e **Illumination**: "the elemental sheds bright light in
        // a 30-foot radius and dim light for an additional 30 feet."
        // The widest glow in the bestiary — a fire elemental lights a
        // room the way a bonfire does, and cannot stop.
        innate_light: Some((12, 12)),
        languages: HashSet::from([Language::Primordial]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        // Base BPS + Poison entries live in
        // `elemental_defaults`; this overlay just adds the fire
        // immunity that distinguishes the variant.
        ..elemental_defaults([(
            DamageType::Fire,
            DamageModifier::Immunity,
        )])
    }
});
