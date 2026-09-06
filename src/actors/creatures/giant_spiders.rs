use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_SPIDER_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, Skill, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Giant Spider — CR 1 Large beast. A fast melee biter that injects
/// poison on a failed CON save: the bestiary's cheapest source of the
/// Poisoned condition paired with raw poison damage, and immune to its
/// own venom.
///
/// It answered to "Spider" for a long time, which was two mistakes in
/// one word. SRD 5.2 has an actual **Spider** — a Tiny CR 0 beast with
/// one hit point and a bite that deals a single point of piercing — and
/// this is not remotely it: every ability score here is the Giant
/// Spider's, the CR is the Giant Spider's, and the docstring above the
/// template said "Giant Spider" while the `name` field said otherwise.
/// So the generator could roll a "Spider" that hit like a CR 1
/// monstrosity, and no reader had a way to tell which of the two they
/// were looking at.
///
/// **Large**, per RAW, and it was Medium — the last of the three things
/// the misnaming hid. A Large spider takes a 2x2 footprint, which is
/// what makes its reach and its bulk read on the board the way the stat
/// block intends.
///
/// RAW's two traits — **Spider Climb** and **Web Walker** — are not
/// modeled: the board has no vertical axis for the first, and the
/// second waives a movement restriction (webs) that the engine's Web
/// zone already lets a creature path around. Its **Web** recharge
/// action is likewise absent, which is a scope cut rather than an
/// oversight: it is a ranged Restrained-install on a destructible
/// object, and the engine has no object HP.
pub static GIANT_SPIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_SPIDER_BITE);
    CreatureTemplate {
        name: "Giant Spider",
        glyph: 'X',
        ac: 14,
        // 4d10+4 = 26 average per SRD 5.2 (CR 1).
        hitpoints: "4d10+4".parse().unwrap(),
        strength: 14,
        dexterity: 16,
        constitution: 12,
        intelligence: 2,
        wisdom: 11,
        charisma: 4,
        senses: HashSet::from([
            SpecialSense::Blindsight(10),
            SpecialSense::Darkvision(60),
        ]),
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        ..CreatureTemplate::defaults()
    }
});
