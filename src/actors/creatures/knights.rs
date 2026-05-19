use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEAVY_CROSSBOW, KNIGHT_MULTI, LANCE, LONGSWORD};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::{HashMap, HashSet};
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
    actions.push(&*HEAVY_CROSSBOW);
    actions.push(&*KNIGHT_MULTI);
    CreatureTemplate {
        name: "Knight",
        // 'K' for knight — distinct from the existing letter pool.
        glyph: 'K',
        ac: 18,
        hitpoints: "8d8+16".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 11,
        dexterity: 11,
        wisdom: 11,
        constitution: 14,
        charisma: 15,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        // Knights are proficient in CON and WIS saves (5e MM); WIS
        // proficiency stands in for the Brave / Bravery features.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
    }
});
