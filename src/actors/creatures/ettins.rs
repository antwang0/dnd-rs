use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CompoundAttack, ETTIN_BATTLEAXE, ETTIN_MORNINGSTAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// SRD 5.2: *"Multiattack. The ettin makes one Battleaxe attack and one
/// Morningstar attack."*
///
/// A `CompoundAttack` rather than a `Multiattack`, and the difference is
/// the stat block: the ettin does not swing one weapon twice, it swings
/// two different ones once each. It used to be two greatclub swings,
/// which cost it both its printed damage (`2d8` a head, not `1d10`) and
/// the thing that makes two heads worth modeling — a creature that
/// deals Slashing *and* Piercing cannot be shrugged off whole by
/// anything that resists one of them.
static ETTIN_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "axe + morningstar",
    parts: vec![(&ETTIN_BATTLEAXE, 1), (&ETTIN_MORNINGSTAR, 1)],
});

/// Ettin — two-headed giant (CR 4). One head swings a battleaxe and the
/// other a morningstar, which is RAW's Multiattack and the reason the
/// two are separate weapons; see `ETTIN_MULTI`. Size Large to match the
/// giant footprint.
///
/// Both of the ettin's traits are about having two heads, and each one
/// waited on a different piece of engine:
///
///   - **Wakeful**: one head is always awake, so nothing gets the drop
///     on it. A condition immunity rather than a bespoke flag, because
///     `Surprised` is a condition and `add_condition` already swallows
///     an install a creature is immune to — the trait needs no code at
///     all.
///   - **Two Heads**: "advantage on Wisdom (Perception) checks and on
///     saving throws against being blinded, charmed, deafened,
///     frightened, stunned, and knocked unconscious." The save half is
///     a `has_multiple_heads` row on `CONDITION_SAVE_ADVANTAGES` — the
///     cohort that exists because six named conditions could not be
///     said any other way. Rounding them up to immunities would have
///     given a CR-4 brute a better mind than a Solar's; rounding them
///     out to "advantage on WIS and CON saves" would have covered its
///     poison and its fear alike. The Perception half stays unmodeled:
///     there is no contested-Perception surface for it to bite on.
pub static ETTIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ETTIN_MULTI);
    CreatureTemplate {
        name: "Ettin",
        glyph: 'E',
        ac: 12,
        hitpoints: "10d10+30".parse().unwrap(),
        speed: 40.,
        strength: 21,
        intelligence: 6,
        dexterity: 8,
        wisdom: 10,
        constitution: 17,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Giant, Language::Orc]),
        cr: 4.0,
        size: Size::Large,
        creature_type: CreatureType::Giant,
        actions,
        has_extra_attack: true,
        condition_immunities: HashSet::from([Condition::Surprised]),
        has_multiple_heads: true,
        ..CreatureTemplate::defaults()
    }
});
