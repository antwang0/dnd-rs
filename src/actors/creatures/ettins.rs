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
/// greatclub, granting a 2-swing multiattack. Two heads means the Ettin
/// has advantage on Perception checks (not modeled) and **can't be
/// surprised** — one head is always looking the other way, which is now
/// a rule the engine can express rather than a note: `Surprised` is a
/// condition, and a condition the ettin is immune to. Size Large to
/// match the giant footprint.
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
        // RAW **Wakeful**: one of the ettin's two heads is always
        // awake, so nothing gets the drop on it. A condition immunity
        // rather than a bespoke flag, because `Surprised` is a
        // condition and `add_condition` already swallows an install a
        // creature is immune to — the trait needs no code at all.
        condition_immunities: HashSet::from([Condition::Surprised]),
        ..CreatureTemplate::defaults()
    }
});
