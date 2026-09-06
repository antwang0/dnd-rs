use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::LIFE_DRAIN;
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::lighting::SunlightFrailty;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// 5e Wraith — CR 5 incorporeal undead. Resistant to most physical and
/// elemental damage, immune to necrotic and poison, vulnerable to nothing
/// (radiant resistance varies by source; we leave radiant as normal so the
/// cleric's Sacred Flame still cuts through). Life Drain is the marquee
/// attack: necrotic damage *and* a CON save against max-HP reduction —
/// the wraith chews through PCs over multiple rounds even when they're
/// not at 0 HP.
pub static WRAITH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*LIFE_DRAIN);
    CreatureTemplate {
        name: "Wraith",
        glyph: 'R', // 'W' is the wolf; pick 'R' (wRaith) so the map stays legible.
        ac: 13,
        // 9d8+27 = 67 average per MM. We approximate with a dice expression
        // that the engine's roller can parse.
        hitpoints: "9d8+27".parse().unwrap(),
        // RAW speed line: Speed 0 ft., fly 60 ft. (hover)
        speed: 0.0,
        fly_speed: 60.0,
        hovers: true,
        strength: 6,
        intelligence: 12,
        dexterity: 16,
        wisdom: 14,
        constitution: 16,
        charisma: 15,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 5.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // 5e Wraith **Sunlight Sensitivity** — the same clause the
        // specter carries, and the specter is its weaker cousin in
        // every other line of the sheet too. Sensitivity rather than
        // the shadow's Weakness: a wraith caught in the open swings
        // badly and saves normally.
        sunlight_frailty: Some(SunlightFrailty::Sensitivity),
        // 5e wraith: resistant to acid / cold / fire / lightning / thunder
        // and to non-magical bludgeoning / piercing / slashing. The BPS
        // triplet lives in `damage_modifiers_from`; the rest of
        // the elemental envelope (and the necrotic / poison immunities)
        // overlay on top.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Acid, DamageModifier::Resistance),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Thunder, DamageModifier::Resistance),
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Standard undead immunities: Poisoned, Charmed. Wraith also
        // ignores Grappled / Restrained / Prone (incorporeal) — we model
        // those as condition immunities so spell-driven control fails.
        condition_immunities: HashSet::from([
            // SRD 5.2 "Immunities Necrotic, Poison; Charmed, Exhaustion,
            // Grappled, Paralyzed, Petrified, Poisoned, Prone,
            // Restrained, Unconscious".
            Condition::Exhausted,
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Grappled,
            Condition::Restrained,
            Condition::Prone,
            Condition::Frightened,
        ]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
