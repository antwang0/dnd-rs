use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGBOW, BITE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Gnoll — CR 1/2 hyena-headed humanoid. Bite + longbow loadout so it
/// can press in melee or harass at range. The marquee mechanic is
/// "Rampage" (a free move + bite when it kills a creature on its turn)
/// — we don't yet model on-kill triggers, so we drop it for now and
/// leave the gnoll as a sturdier-than-bandit ranged mook. AC is light
/// hide (12), HP a touch above goblin.
pub static GNOLL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BITE);
    actions.push(&LONGBOW);
    CreatureTemplate {
        name: "Gnoll",
        // 'N' (gNoll) — 'G' is the goblin.
        glyph: 'N',
        ac: 12,
        hitpoints: "3d8".parse().unwrap(),
        speed: 30.,
        strength: 14,
        intelligence: 6,
        dexterity: 12,
        wisdom: 10,
        constitution: 11,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
    }
});
