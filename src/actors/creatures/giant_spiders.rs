use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_SPIDER_BITE, SPIDER_WEB};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
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
/// zone already lets a creature path around.
///
/// Its **Web** recharge action was absent for the same length of time,
/// as a scope cut with a stated reason — *"it is a ranged
/// Restrained-install on a destructible object, and the engine has no
/// object HP"* — and the reason expired when
/// [`crate::engine::objects`] arrived for the two conjured walls. The
/// clause ships now, all of it: DC 13 Dexterity at sixty feet,
/// Restrained until destroyed, and a web that is an object with ten
/// armour class, five hit points and a Fire vulnerability. See
/// [`crate::actions::monster_attacks::SPIDER_WEB`] and
/// [`crate::actions::default_actions::CUT_FREE`], which is how anybody
/// gets out of one.
///
/// It changes what a giant spider *is* on a board, which is the point:
/// a CR 1 beast with a bite was a speed bump, and one that can take a
/// character out of the fight from across the room until somebody
/// spends a turn cutting them loose is the ambush predator the stat
/// block describes.
pub static GIANT_SPIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_SPIDER_BITE);
    actions.push(&*SPIDER_WEB);
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
        // **No damage line at all**, which is what SRD 5.2 prints. The
        // Poison immunity that used to be here was not in the 2014
        // printing either — a spider that cannot be poisoned is a
        // reasonable guess about spiders and is not what either book
        // says, and it made the bestiary's own venom useless against
        // the creature most likely to be standing next to one.
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        // RAW's *Web (Recharge 5–6)*.
        recharge_abilities: vec![("web", 5)],
        ..CreatureTemplate::defaults()
    }
});
