use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::VAMPIRIC_BITE;
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::engine::lighting::SunlightFrailty;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Vampire Spawn — CR 5 undead. Vampiric bite hits hard with piercing +
/// 3d6 necrotic and feeds the spawn back for full necrotic HP — the
/// signature lifesteal pattern. Resistant to necrotic, immune to the
/// usual undead suite (poisoned / charmed). The natural counterpart to
/// the Wraith: instead of draining max-HP, the spawn caps off its own
/// pool and is much harder to chip down across a long encounter.
pub static VAMPIRE_SPAWN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*VAMPIRIC_BITE);
    CreatureTemplate {
        name: "Vampire Spawn",
        // 'V' for vampire — distinct from 'W' (wolf) and 'R' (wraith).
        glyph: 'V',
        ac: 15,
        hitpoints: "11d8+33".parse().unwrap(),
        strength: 16,
        dexterity: 16,
        constitution: 16,
        intelligence: 11,
        wisdom: 10,
        charisma: 12,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 5.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // Necrotic-resistant + non-magical BPS resistance + poison
        // immunity (5e MM vampire damage envelope).
        damage_modifiers: non_magical_physical_resistances([
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned, Condition::Charmed]),
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        // 5e Vampire Spawn **Sunlight Hypersensitivity**: the same
        // 20-radiant-per-turn clause the Vampire Lord carries, and on
        // the same radiant vulnerability. See `vampires.rs`.
        sunlight_frailty: Some(SunlightFrailty::Hypersensitivity),
        ..CreatureTemplate::defaults()
    }
});
