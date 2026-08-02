use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BANDIT_CAPTAIN_MULTI, HEAVY_CROSSBOW, SCIMITAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Bandit Captain — CR 2 humanoid mid-boss. A bandit boss with triple
/// scimitar multi-attack and the same heavy-crossbow ranged option as
/// a regular bandit. Higher AC, more HP, and proficient saves in STR /
/// DEX / WIS reflecting the 5e MM stat block. Drops in as the headline
/// enemy for low-tier bandit ambushes.
pub static BANDIT_CAPTAIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&HEAVY_CROSSBOW);
    actions.push(&*BANDIT_CAPTAIN_MULTI);
    CreatureTemplate {
        name: "Bandit Captain",
        glyph: 'X',
        ac: 15,
        hitpoints: "9d8+9".parse().unwrap(),
        speed: 30.,
        strength: 15,
        intelligence: 14,
        dexterity: 16,
        wisdom: 11,
        constitution: 14,
        charisma: 14,
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Captains are trained warriors — proficient in STR, DEX, and
        // WIS saves (per 5e MM).
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
            AbilityScoreType::Wisdom,
        ]),
        has_extra_attack: true,
        skills: HashSet::from([Skill::Athletics]),
        ..CreatureTemplate::defaults()
    }
});
