use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SHADOW_DEMON_CLAWS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Shadow Demon — CR 4 medium fiend (chaotic-evil). The incorporeal
/// horror's combat envelope is wide damage resistance + a psychic-typed
/// claw swing. RAW: resistance to acid, cold, fire, lightning, and
/// thunder (the five-element "shadowy" envelope), immunity to cold and
/// poison; vulnerability to radiant (the "shadow vs light" hook). We
/// surface the load-bearing slice: cold / poison immunity, fire / acid /
/// lightning / thunder resistance, and the radiant vulnerability hook.
///
/// The "advantage in dim light or darkness" RAW clause is omitted — no
/// global lighting model — but the psychic-typed claws + wide resistance
/// envelope give the shadow demon a distinct combat profile vs other
/// CR-4 fiends (vrocks, hell hounds with breath, succubi).
///
/// Stats roughly track MM Shadow Demon at CR 4 — high DEX, modest HP,
/// shadow-stealth flavor. INT-primary fiend with low STR (DEX-based
/// claws so the damage roll isn't capped by the weak STR score).
pub static SHADOW_DEMON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SHADOW_DEMON_CLAWS);
    CreatureTemplate {
        name: "Shadow Demon",
        // 'σ' (Greek lowercase sigma) — distinct from 'S' (Skeleton) and
        // 'D' (Dragon family). The flowing s-shape reads as a wisp /
        // shadow silhouette.
        glyph: 'σ',
        ac: 13,
        hitpoints: "11d8+11".parse().unwrap(),
        speed: 30.,
        strength: 1,
        intelligence: 14,
        dexterity: 17,
        wisdom: 14,
        constitution: 13,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Abyssal, Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: HashMap::from([
            (DamageType::Cold, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Acid, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Thunder, DamageModifier::Resistance),
            (DamageType::Necrotic, DamageModifier::Resistance),
            // Shadow vs light: radiant damage gets through cleanly,
            // and the "advantage to sun-touched attackers" RAW clause
            // collapses to a flat vulnerability on the radiant lane.
            (DamageType::Radiant, DamageModifier::Vulnerability),
        ]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn shadow_demon_is_radiant_vulnerable() {
        let a = ActorInstance::from_creature_template(
            &SHADOW_DEMON_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(
            a.damage_modifier(DamageType::Radiant),
            Some(DamageModifier::Vulnerability)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Immunity)
        );
    }
}
