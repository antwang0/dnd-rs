use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GELATINOUS_CUBE_ENGULF;
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Gelatinous Cube — CR 2 ooze. Slow, transparent acid block. Single
/// melee pseudopod attack (3d6 acid + STR save vs Restrained on hit).
/// Ooze-style condition envelope: blind on default (no eyes), but the
/// 5e MM treats Blindsight as the compensating sense — we model that
/// by making the cube Blinded-immune so spells like Color Spray can't
/// further reduce its sightlessness, and we *don't* add the Blinded
/// condition automatically (the cube would be at perma-disadvantage on
/// every attack roll if we did).
pub static GELATINOUS_CUBE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GELATINOUS_CUBE_ENGULF);
    CreatureTemplate {
        name: "Gelatinous Cube",
        // 'j' (lowercase) — "jelly". Free glyph.
        glyph: 'j',
        ac: 6,
        // 8d10+40 = 84 average per MM — surprisingly tanky for a CR 2.
        hitpoints: "8d10+40".parse().unwrap(),
        speed: 15.,
        strength: 14,
        dexterity: 3,
        constitution: 20,
        intelligence: 1,
        wisdom: 6,
        charisma: 1,
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Ooze,
        actions,
        // SRD 5.2 "Immunities Acid" — the row the cube was shipping
        // without, and the one its whole silhouette is about: the
        // creature *is* a block of digestive acid, and its own Engulf
        // deals acid damage it would otherwise have been taking a share
        // of from any friendly-fire burst.
        damage_modifiers: damage_modifiers_from([(
            DamageType::Acid,
            DamageModifier::Immunity,
        )]),
        // Ooze envelope: ignores Blinded (no eyes to gouge), Charmed,
        // Deafened, Exhaustion (no muscle to tire), Frightened, Prone
        // (no shape to knock down), and Asleep. RAW oozes also ignore
        // Grappled; we skip that one so the Treant / similar grappler
        // spells still pin the cube in place.
        condition_immunities: HashSet::from([
            Condition::Exhausted,
            Condition::Blinded,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Frightened,
            Condition::Prone,
            Condition::Asleep,
        ]),
        ..CreatureTemplate::defaults()
    }
});
