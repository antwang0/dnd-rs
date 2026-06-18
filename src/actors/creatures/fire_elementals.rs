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

/// Damage-modifier baseline every elemental in this engine shares:
/// poison immunity (elementals don't have biology to poison) plus
/// resistance to bludgeoning / piercing / slashing (the magical-vs-
/// non-magical split is collapsed since the engine doesn't track
/// weapon magicality). Each elemental template starts from this base
/// via `elemental_damage_modifiers([...])` and overlays its own
/// signature entries (Fire / Cold / Lightning / Acid immunity or
/// resistance, Earth's thunder vulnerability, etc.).
///
/// Replaces five hand-copied `(BPS triplet + Poison)` literals — the
/// four existing elementals plus future ones — with a single source
/// of truth. A new resistance / immunity added to the base lands
/// uniformly across every elemental.
pub fn elemental_damage_modifiers(
    overlays: impl IntoIterator<Item = (DamageType, DamageModifier)>,
) -> HashMap<DamageType, DamageModifier> {
    let mut m = HashMap::from([
        (DamageType::Poison, DamageModifier::Immunity),
        (DamageType::Bludgeoning, DamageModifier::Resistance),
        (DamageType::Piercing, DamageModifier::Resistance),
        (DamageType::Slashing, DamageModifier::Resistance),
    ]);
    // Overlays win on collision — an Earth Elemental's
    // (Thunder, Vulnerability) doesn't conflict with the base; an
    // overlay that promotes BPS to Immunity (e.g. a future creature
    // built on the elemental chassis) replaces the base resistance.
    m.extend(overlays);
    m
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
        languages: HashSet::from([Language::Primordial]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        // Base BPS + Poison entries live in
        // `elemental_damage_modifiers`; this overlay just adds the fire
        // immunity that distinguishes the variant.
        damage_modifiers: elemental_damage_modifiers([(
            DamageType::Fire,
            DamageModifier::Immunity,
        )]),
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        ..CreatureTemplate::defaults()
    }
});
