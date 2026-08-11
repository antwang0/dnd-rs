use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BANSHEE_WAIL, CORRUPTING_TOUCH};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Banshee — CR 4 undead. Frail body (12 AC, 58 average HP) wrapped
/// around a brutal AoE: a once-per-encounter wail that frightens and
/// damages every nearby non-undead. Pair the wail with the corrupting
/// touch (3d6+CHA necrotic) for the finisher — the banshee is a glass
/// cannon, not a brawler. Like other undead it's immune to poison,
/// necrotic, and the standard charm/frighten lockdown.
pub static BANSHEE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CORRUPTING_TOUCH);
    actions.push(&*BANSHEE_WAIL);
    CreatureTemplate {
        name: "Banshee",
        // 'b' was free (B is Bugbear); use 'b' for banshee.
        glyph: 'b',
        ac: 12,
        // 13d8 = 58 average per MM.
        hitpoints: "13d8".parse().unwrap(),
        // RAW speed line: Speed 0 ft., fly 40 ft. (hover)
        speed: 0.0,
        fly_speed: 40.0,
        hovers: true,
        strength: 1,
        intelligence: 12,
        dexterity: 14,
        wisdom: 11,
        constitution: 10,
        charisma: 17, // primary stat — drives CORRUPTING_TOUCH
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // 5e: resistance to non-magical bludgeoning/piercing/slashing.
        // We don't track magical weapon flags so we apply the resistance
        // directly (mirrors the wraith / specter pattern).
        damage_modifiers: damage_modifiers_from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::from([AbilityScoreType::Charisma]),
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Grappled,
            Condition::Prone,
            Condition::Restrained,
        ]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
