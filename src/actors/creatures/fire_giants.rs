use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{FIRE_GIANT_GREATSWORD, FIRE_GIANT_ROCK};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Fire Giant — CR 9 giant. Sibling to Frost Giant (CR 8) one CR step up
/// the giant ladder: same Huge greatsword-and-thrown-rock chassis, but
/// fire-immune instead of cold-immune. Heavier AC (18 plate-clad
/// blacksmith) and a touch more HP than the Frost Giant. Charmed
/// immunity matches the Hill Giant flavor — giants brush off
/// mind-bending control spells. Slots between Frost Giant and Cloud
/// Giant in the giant pool, completing the chromatic elemental trio
/// (cold / fire / cloud) at the upper-mid CR tier.
pub static FIRE_GIANT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&FIRE_GIANT_GREATSWORD);
    actions.push(&FIRE_GIANT_ROCK);
    CreatureTemplate {
        name: "Fire Giant",
        // 'F' for Fire Giant — capital because Huge; 'G' (capital) is
        // already taken by Frost Giant in the giant family, so each
        // giant kin gets a distinct glyph (G/J/g/L/F) on the same map.
        glyph: 'F',
        ac: 18,
        // 13d12+78 ≈ 162 average per MM (CR 9). Heaviest giant HP pool
        // below the Storm Giant.
        hitpoints: "13d12+78".parse().unwrap(),
        speed: 30.,
        strength: 25,
        intelligence: 10,
        dexterity: 9,
        wisdom: 14,
        constitution: 23,
        charisma: 13,
        languages: HashSet::from([Language::Giant]),
        cr: 9.0,
        size: Size::Huge,
        creature_type: CreatureType::Giant,
        actions,
        // Fire Giants are immune to fire — their forge-lit hide
        // shrugs off Fireball / Flame Strike / dragon breath.
        damage_modifiers: HashMap::from([(DamageType::Fire, DamageModifier::Immunity)]),
        // Fire Giant proficient saves: DEX / CON / CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Constitution,
            AbilityScoreType::Charisma,
        ]),
        // Matches the Hill Giant flavor: giants brush off control
        // spells like Charm Person / Hold Monster.
        condition_immunities: HashSet::from([Condition::Charmed]),
        has_extra_attack: true,
        ..CreatureTemplate::defaults()
    }
});
