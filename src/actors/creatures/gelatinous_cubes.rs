use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GELATINOUS_CUBE_ENGULF;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{Size};
use std::collections::{HashMap, HashSet};
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
        intelligence: 1,
        dexterity: 3,
        wisdom: 6,
        constitution: 20,
        charisma: 1,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::new(),
        cr: 2.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        // Ooze envelope: ignores Blinded (no eyes to gouge), Charmed,
        // Deafened, Frightened, Prone (no shape to knock down), and
        // Asleep. RAW oozes also ignore Exhaustion / Grappled but we
        // skip Grappled so the Treant / similar grappler spells still
        // pin the cube in place.
        condition_immunities: HashSet::from([
            Condition::Blinded,
            Condition::Charmed,
            Condition::Deafened,
            Condition::Frightened,
            Condition::Prone,
            Condition::Asleep,
        ]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
    }
});
