use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GREATCLUB, Multiattack};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

static ETTIN_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "two-headed smash",
    sub_attack: &GREATCLUB,
    count: 2,
});

/// Ettin — two-headed giant (CR 4, MM p.132). Each head wields a
/// greatclub, granting a 2-swing multiattack. Size Large to match the
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
        hitpoints: "10d10+20".parse().unwrap(),
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
