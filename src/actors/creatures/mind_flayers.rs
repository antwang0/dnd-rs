use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MIND_FLAYER_MIND_BLAST, MIND_FLAYER_TENTACLES};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Mind Flayer (Illithid) — CR 7 aberration boss. The signature
/// monster of the underdark, a psionic terror that opens with Mind
/// Blast (60ft cone, INT save, 4d8 psychic + Stun on fail) and follows
/// up with Tentacle attacks that grapple targets with low INT.
///
/// Mechanical envelope (MM): AC 15, 71 HP, INT-primary (19), psychic
/// immunity, telepathy. Proficient in INT / WIS / CHA saves — the
/// classic "save vs mental" defense profile, with the inverted weakness
/// that physical STR / DEX saves auto-fail when Stunned (no proficiency
/// helps once the flayer takes a hit and gets stunned itself).
pub static MIND_FLAYER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MIND_FLAYER_MIND_BLAST);
    actions.push(&*MIND_FLAYER_TENTACLES);
    CreatureTemplate {
        name: "Mind Flayer",
        // 'I' for Illithid — keeps 'M' for Mage and lets the flayer
        // stand out from the other M-tier aberrations on the map.
        glyph: 'I',
        ac: 15,
        // 13d8+13 ≈ 71 average per the MM Mind Flayer stat block.
        hitpoints: "13d8+13".parse().unwrap(),
        strength: 11,
        dexterity: 12,
        constitution: 12,
        intelligence: 19, // primary — drives Mind Blast DC + tentacle grapple
        wisdom: 17,
        charisma: 17,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::DeepSpeech, Language::Undercommon]),
        cr: 7.0,
        size: Size::Medium,
        creature_type: CreatureType::Aberration,
        actions,
        // MM Mind Flayer is not damage-resistant by default — psychic
        // beings aren't tough in the physical sense. We add psychic
        // immunity here because RAW lists it under Magic Resistance +
        // psionic shield bundle.
        damage_modifiers: HashMap::from([
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        // MM saves: INT / WIS / CHA proficient — the trifecta of
        // mental defense saves a psionic creature should ace.
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        has_magic_resistance: true,
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});
