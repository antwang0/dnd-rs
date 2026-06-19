use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SALAMANDER_MULTI, SALAMANDER_SPEAR, SALAMANDER_TAIL};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Salamander — CR 5 elemental. Fire-attuned serpentine giant from the
/// Elemental Plane of Fire. Action lanes:
/// - **Multiattack** (1 spear + 1 tail) — heterogeneous compound; spear
///   pokes at reach 2, tail whips at reach 3.
/// - **Salamander Spear** (standalone) — 2d6 piercing + 1d6 fire rider.
/// - **Salamander Tail** (standalone) — 2d6 bludgeoning + 1d6 fire rider.
///
/// MM RAW: AC 15, ~90 HP (12d10+24), STR 18, DEX 14, CON 15. **Fire
/// immune**, **cold vulnerable**, **fire shield** style: every melee
/// attacker takes a hit of heat damage. We don't model the Heated Body
/// reflect (it would need its own caster-side rider table — out of scope
/// for this drop); the fire/cold envelope is the load-bearing flavor.
pub static SALAMANDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SALAMANDER_MULTI);
    actions.push(&*SALAMANDER_SPEAR);
    actions.push(&*SALAMANDER_TAIL);
    CreatureTemplate {
        name: "Salamander",
        // 'a' (lowercase) — fire-themed serpentine glyph. Distinct from
        // existing 'A' (Animated Armor) and 's' (Skeleton).
        glyph: 'a',
        ac: 15,
        // 12d10+24 ≈ 90 average per MM (CR 5).
        hitpoints: "12d10+24".parse().unwrap(),
        speed: 30.,
        strength: 18,
        dexterity: 14,
        constitution: 15,
        intelligence: 11,
        wisdom: 10,
        charisma: 12,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        damage_modifiers: non_magical_physical_resistances([
            // Fire immunity — they ARE fire.
            (DamageType::Fire, DamageModifier::Immunity),
            // Cold vulnerability — water and ice undo them.
            (DamageType::Cold, DamageModifier::Vulnerability),
        ]),
        ..CreatureTemplate::defaults()
    }
});
