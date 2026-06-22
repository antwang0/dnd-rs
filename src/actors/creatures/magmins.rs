use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::MAGMIN_TOUCH;
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    ELEMENTAL_CONDITION_IMMUNITIES, elemental_damage_modifiers,
};
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Magmin — CR ½ small elemental. The lava-imp of the Plane of Fire — a
/// tiny smoldering creature whose body is half rock, half flame, whose
/// touch ignites the toughest leather and chars exposed flesh. Slots
/// between Fire Imp (CR 1, fiend-typed) and the larger Fire Elemental
/// (CR 5) on the fire-themed ladder — the cheapest elemental pick in
/// the pool and the only elemental with the "Burning rider on touch"
/// tactical clause at sub-1 CR.
///
/// Action lanes:
/// - **magmin touch** — DEX-based 1d6+DEX fire melee, on hit installs
///   Burning for 3 rounds. Pure fire damage (no physical component);
///   the rider is the load-bearing slice. Fire-immune targets shrug off
///   both the damage and the install.
///
/// Defensive identity: AC 14 (natural armor — the magmin's smoldering
/// rock crust), 9 HP (2d6+2). Fire immunity (the magmin IS fire — its
/// own touch would self-incinerate without the immunity). Poison
/// immunity + non-magical BPS resistance via the shared
/// `elemental_damage_modifiers` baseline; the magmin inherits the
/// canonical elemental envelope (the engine collapses all elementals
/// to the same defensive shape for uniformity, even though MM RAW
/// gives the magmin a smaller condition / resistance set than the
/// CR-5 Fire Elemental). The full elemental condition envelope
/// (Charmed / Frightened / Paralyzed / Petrified / Poisoned / Asleep
/// / Prone / Grappled / Restrained) is shared via
/// `ELEMENTAL_CONDITION_IMMUNITIES`.
///
/// Stat shape: AC 14, ~9 HP (2d6+2), STR 7, DEX 15, CON 12, INT 8,
/// WIS 11, CHA 10. Speed 30 (the magmin's small frame compensates with
/// canid burst speed). Senses: Darkvision 60 (standard elemental
/// envelope). Languages: Primordial (the engine collapses the four
/// elemental dialects — Aquan / Auran / Ignan / Terran — into the
/// single Primordial enum). Size Small. CR ½.
///
/// RAW also gives the magmin **Death Burst** (when reduced to 0 HP it
/// explodes, dealing 7 (2d6) fire damage in a 10-ft radius — DEX save
/// halves). We omit the death-burst clause because the engine's
/// `on_death` hook chassis isn't surfaced through the Action system,
/// and the magmin's load-bearing combat clause is the touch's Burning
/// rider, not the post-mortem explosion. Future "on death" trigger
/// addition could light this up via the magmin template flag.
///
/// **Ignited Illumination** (the magmin sheds bright light in a 10-ft
/// radius and dim light for another 10 ft) — omitted as a flavor clause
/// since the engine doesn't model per-tile illumination from creatures.
pub static MAGMIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MAGMIN_TOUCH);
    CreatureTemplate {
        name: "Magmin",
        // 'm' (lowercase) — distinct from 'M' (Mind Flayer / Marilith /
        // Manticore at uppercase). 'm' for "magmin" reads as the
        // smoldering small elemental silhouette.
        glyph: 'm',
        ac: 14,
        // 2d6+2 ≈ 9 average per MM (CR ½). The cheap-elemental pool.
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 7,
        intelligence: 8,
        dexterity: 15,
        wisdom: 11,
        constitution: 12,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 0.5,
        size: Size::Small,
        creature_type: CreatureType::Elemental,
        actions,
        // The magmin IS fire — its own touch would self-incinerate
        // without the fire immunity. The `elemental_damage_modifiers`
        // baseline (poison immunity + non-magical BPS resistance) is
        // shared across every elemental in the codebase; fire immunity
        // is the variant-specific overlay.
        damage_modifiers: elemental_damage_modifiers([(
            DamageType::Fire,
            DamageModifier::Immunity,
        )]),
        // Shared elemental condition envelope (Charmed / Frightened /
        // Paralyzed / Petrified / Poisoned / Asleep / Prone / Grappled
        // / Restrained). The magmin has no metabolism / joints / mind
        // to coerce.
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn magmin_template_shape() {
        let a = ActorInstance::from_creature_template(
            &MAGMIN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        // The magmin's one action lane — touch with Burning rider.
        assert!(a.find_action("magmin touch").is_some());
    }

    #[test]
    fn magmin_is_fire_and_poison_immune() {
        let a = ActorInstance::from_creature_template(
            &MAGMIN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // The signature fire elemental envelope — the magmin IS fire,
        // so its own ignite rider rolls through harmlessly against
        // another magmin (matters for magmin-vs-magmin clashes, which
        // happen in the elemental encampments of the Plane of Fire).
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        // Standard elemental condition envelope — no metabolism / joints
        // / mind to coerce.
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        // Non-magical BPS resistance — RAW "from nonmagical attacks";
        // the magmin's smoldering crust shrugs off a CR-½ party's
        // mundane weapons.
        assert_eq!(
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
    }
}
