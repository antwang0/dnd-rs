use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_EAGLE_BEAK, GIANT_EAGLE_MULTI, GIANT_EAGLE_TALONS};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Giant Eagle — CR 1 large beast. Aerial predator with a one-beak +
/// one-talons CompoundAttack multi. Fast walk speed (the engine doesn't
/// model 3D fly, so the eagle's 80ft fly speed surfaces as a high ground
/// speed — fast enough to keep the bird threatening at range and able to
/// reposition between bursts). Standalone beak / talons are exposed too so
/// the AI can fall back to a single swing when it's bonus-action-tagged
/// or moving in for a bite.
pub static GIANT_EAGLE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_EAGLE_BEAK);
    actions.push(&GIANT_EAGLE_TALONS);
    actions.push(&*GIANT_EAGLE_MULTI);
    CreatureTemplate {
        name: "Giant Eagle",
        // 'E' for Eagle — capital because Large; no other E creature yet.
        glyph: 'E',
        ac: 13,
        hitpoints: "4d10+4".parse().unwrap(),
        // Approx 40ft walking; the 80ft fly is the headline but the
        // engine collapses to a single ground speed.
        speed: 40.,
        strength: 16,
        intelligence: 8,
        dexterity: 17,
        wisdom: 14,
        constitution: 13,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::new(),
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_deflect_missiles: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        // Multi runs both limbs per Action — the CompoundAttack wrapper
        // already handles the two-swing burst, so extra_attack stays off
        // to avoid double-stacking the rake.
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
