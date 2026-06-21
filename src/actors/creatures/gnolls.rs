use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGBOW, BITE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Gnoll — CR 1/2 hyena-headed humanoid. Bite + longbow loadout so it
/// can press in melee or harass at range. The marquee mechanic is
/// "Rampage" (a free move + bite when it kills a creature on its turn)
/// — we don't yet model on-kill triggers, so we drop it for now and
/// leave the gnoll as a sturdier-than-bandit ranged mook. AC is light
/// hide (12), HP a touch above goblin.
pub static GNOLL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BITE);
    actions.push(&LONGBOW);
    CreatureTemplate {
        name: "Gnoll",
        // 'N' (gNoll) — 'G' is the goblin.
        glyph: 'N',
        ac: 12,
        hitpoints: "3d8".parse().unwrap(),
        strength: 14,
        dexterity: 12,
        constitution: 11,
        intelligence: 6,
        wisdom: 10,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        ..CreatureTemplate::defaults()
    }
});
