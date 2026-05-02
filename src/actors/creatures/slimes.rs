use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::ACID_SPIT;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, DamageType, Size};
use std::collections::HashSet;
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
        n_instances: 0,
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
        // Acidic ooze body: acid-immune, slashing-resistant (it just
        // re-blobs around the cut), cold-vulnerable (freezing seizes it).
        resistances: HashSet::from([DamageType::Slashing]),
        immunities: HashSet::from([DamageType::Acid]),
        vulnerabilities: HashSet::from([DamageType::Cold]),
        save_proficiencies: HashSet::from([AbilityScoreType::Constitution]),
    }
});
