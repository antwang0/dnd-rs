use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::ACID_SPIT;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{DamageModifier, DamageType, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Acid Slime — squishy ranged splash dealer. Spits an acidic blob at a
/// single target; the impact splashes onto every combat-active actor
/// footprint-adjacent to the primary, so positioning matters around it
/// (don't bunch up downwind of an enemy near a slime, and don't park your
/// slime next to your own front line).
///
/// Stats are rough — light HP, low AC, no melee. Slimes want range.
pub static SLIME_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ACID_SPIT);
    CreatureTemplate {
        name: "Slime",
        glyph: 's',
        ac: 10,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 20.,
        strength: 8,
        intelligence: 2,
        dexterity: 12,
        wisdom: 6,
        constitution: 12,
        charisma: 1,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::new(),
        cr: 0.25,
        size: Size::Small,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Acid slimes: immune to their own element. Resist piercing
        // and slashing — physical weapons just gum up in the ooze.
        damage_modifiers: HashMap::from([
            (DamageType::Acid, DamageModifier::Immunity),
            // Slimes are gel-like — physical weapons all gum up.
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
            // Cold turns the gel hard and brittle — vulnerability.
            (DamageType::Cold, DamageModifier::Vulnerability),
        ]),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
    }
});
