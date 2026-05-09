use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{MAGIC_MISSILE, SACRED_BURST};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Apprentice-style arcane caster. INT-primary, fragile body, but the
/// headline trick is **Magic Missile**: an auto-hit volley that ignores
/// AC entirely. Distinct from the Cleric's WIS-based save-vs-DC kit —
/// where the Cleric pressures resist-spam through DEX saves, the Wizard
/// punishes high-AC tanks through guaranteed force damage.
///
/// We also give the Wizard Sacred Burst — not strictly a Wizard spell in
/// 5e, but the AoE option keeps their action economy interesting until
/// Burning Hands lands here. Treat the overlap as a stand-in.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*SACRED_BURST);
    CreatureTemplate {
        name: "Wizard",
        // 'W' is already the wolf — use 'M' for magic-user.
        glyph: 'M',
        n_instances: 0,
        ac: 12,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 9,
        intelligence: 15, // primary spellcasting ability
        dexterity: 14,
        wisdom: 12,
        constitution: 12,
        charisma: 11,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // 3 level-1 slots (Magic Missile staple). No level-2 access.
        spell_slots_by_level: vec![3],
        rolls_death_saves: false,
        damage_resistances: HashSet::new(),
        damage_immunities: HashSet::new(),
        damage_vulnerabilities: HashSet::new(),
    }
});
