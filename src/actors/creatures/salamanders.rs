use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SALAMANDER_MULTI, SALAMANDER_SPEAR, SALAMANDER_TAIL};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::engine::attack::{MeleeReflect, ReflectDamage};
use crate::engine::dice::Dice;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Salamander **Heated Body** — RAW: "A creature that touches the
/// salamander or hits it with a melee attack while within 5 feet of it
/// takes 7 (1d6 + 3) fire damage." We approximate the 1d6+3 with a flat
/// 1d6 die roll and let the damage pipeline read it through the natural-
/// melee-reflect lane (sibling to Black Pudding Corrosive Form). The
/// short-fall vs RAW (no +3 bonus) is small in practice — the load-
/// bearing combat clause is the reflect itself, and the engine's
/// natural-melee-reflect lane takes plain dice expressions for now.
pub static SALAMANDER_HEATED_BODY: MeleeReflect = MeleeReflect {
    damage: ReflectDamage::Dice(Dice::new(1, 6)),
    damage_type: DamageType::Fire,
    label: "heated body",
};

/// Salamander — CR 5 elemental. Fire-attuned serpentine giant from the
/// Elemental Plane of Fire. Action lanes:
/// - **Multiattack** (1 spear + 1 tail) — heterogeneous compound; spear
///   pokes at reach 2, tail whips at reach 3.
/// - **Salamander Spear** (standalone) — 2d6 piercing + 1d6 fire rider.
/// - **Salamander Tail** (standalone) — 2d6 bludgeoning + 1d6 fire rider.
///
/// MM RAW: AC 15, ~90 HP (12d10+24), STR 18, DEX 14, CON 15. **Fire
/// immune**, **cold vulnerable**, **Heated Body** reflect — every melee
/// attacker takes 1d6 fire damage in retaliation, routed through the
/// engine's natural-melee-reflect lane (sibling to the Fire Shield
/// condition keyed reflect and the Black Pudding's Corrosive Form).
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
        // Heated Body — 1d6 fire back at every melee attacker. Composes
        // additively with condition-keyed reflects (a salamander wearing
        // Fire Shield rolls both reflects on the same incoming swing).
        natural_melee_reflect: Some(SALAMANDER_HEATED_BODY),
        ..CreatureTemplate::defaults()
    }
});
