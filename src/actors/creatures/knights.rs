use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEAVY_CROSSBOW, KNIGHT_MULTI, LANCE, LONGSWORD};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Knight — CR 3 plate-armored melee specialist. Plate (AC 18) + heavy
/// crossbow ranged option + double longsword multiattack at the high
/// end. The lance gives them a reach-2 swing option for opening
/// engagements. Pair them with mooks to soak attacks for the big swing.
///
/// 5e MM Knight has Bravery (advantage vs Frightened, modeled as
/// proficient WIS save here), Brave (immune to Frightened — we lift to
/// proficient WIS save instead since the engine doesn't yet model the
/// fear-immunity nuance), and the Leadership reaction (skipped — it
/// requires shouted-orders mechanics we don't yet model).
pub static KNIGHT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LONGSWORD);
    actions.push(&LANCE);
    actions.push(&HEAVY_CROSSBOW);
    actions.push(&*KNIGHT_MULTI);
    CreatureTemplate {
        name: "Knight",
        // 'K' for knight — distinct from the existing letter pool.
        glyph: 'K',
        ac: 18,
        hitpoints: "8d8+16".parse().unwrap(),
        strength: 16,
        dexterity: 11,
        constitution: 14,
        intelligence: 11,
        wisdom: 11,
        charisma: 15,
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Knights are proficient in CON and WIS saves (5e MM); WIS
        // proficiency stands in for the Brave / Bravery features.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
        ]),
        has_extra_attack: true,
        // The Mounted Combatant feat. A knight's stat block carries a
        // Lance, which is a weapon that exists to be used from a horse
        // — RAW gives it disadvantage against anything within 5 feet
        // and a reach of 10 — and the MM entry describes knights as
        // "warriors who have sworn fealty… frequently mounted". Now
        // that `engine::mounts` exists, the AI's mount rung puts one on
        // any warhorse that spawns beside it, and the feat is what
        // makes that worth doing.
        has_mounted_combatant: true,
        ..CreatureTemplate::defaults()
    }
});
