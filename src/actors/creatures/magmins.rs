use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DeathBurst, MAGMIN_TOUCH};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    elemental_defaults,
};
use crate::engine::dice::Dice;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Magmin **Death Burst** — when reduced to 0 HP the magmin explodes in a
/// flash of smoldering rock, dealing 2d6 fire in a 10-ft radius (gap 2
/// on this 2.5 ft grid). DEX save DC 11 halves; the magmin's body is
/// gone afterward (the engine removes the corpse). Fire-immune targets
/// (other magmins, the fire elemental, the red dragon, etc.) shrug off
/// the burst entirely via the damage-modifier pipeline.
pub static MAGMIN_DEATH_BURST: DeathBurst = DeathBurst {
    display_name: "explodes",
    damage_dice: Dice::new(2, 6),
    damage_type: DamageType::Fire,
    save_ability: AbilityScoreType::Dexterity,
    dc: 11,
    radius: 2,
};

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
/// `elemental_defaults` baseline; the magmin inherits the
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
/// RAW **Death Burst** is wired via the shared `DeathBurst` chassis —
/// when reduced to 0 HP the magmin detonates for 2d6 fire in a 10-ft
/// radius (DEX save DC 11, half on save). Fires automatically at the
/// `cleanup_dead_actors` chokepoint before the corpse is removed; the
/// dying magmin is excluded from its own blast via the standard
/// `resolve_burst_save_damage` caster-exclusion gate.
///
/// **Ignited Illumination** — RAW: "the magmin sheds bright light in a
/// 10-foot radius and dim light for an additional 10 feet." Carried on
/// `innate_light`, which walks with the magmin and goes out with it.
/// It was omitted for years as a flavour clause on the grounds that the
/// engine did not model per-tile illumination from creatures; the
/// lighting layer arrived, and nobody came back for the note.
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
        // Ignited Illumination — 10 ft bright, 10 ft dim.
        innate_light: Some((
            crate::engine::lighting::GLOW_BRIGHT_TILES,
            crate::engine::lighting::GLOW_DIM_TILES,
        )),
        cr: 0.5,
        size: Size::Small,
        creature_type: CreatureType::Elemental,
        actions,
        // The magmin IS fire — its own touch would self-incinerate
        // without the fire immunity. The `elemental_defaults`
        // baseline (poison immunity + non-magical BPS resistance) is
        // shared across every elemental in the codebase; fire immunity
        // is the variant-specific overlay.
        // Shared elemental condition envelope (Charmed / Frightened /
        // Paralyzed / Petrified / Poisoned / Asleep / Prone / Grappled
        // / Restrained). The magmin has no metabolism / joints / mind
        // to coerce.
        // RAW Death Burst — 2d6 fire DC 11 DEX 10-ft radius. Fires at
        // the engine's `cleanup_dead_actors` chokepoint before the
        // corpse is removed; the magmin's own fire immunity protects
        // it from any partial double-tap and same-typed allies
        // (other magmins, fire elementals) shrug off the burst entirely.
        death_burst: Some(&MAGMIN_DEATH_BURST),
        ..elemental_defaults([(
            DamageType::Fire,
            DamageModifier::Immunity,
        )])
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
    fn magmin_template_has_death_burst() {
        let burst = MAGMIN_TEMPLATE
            .death_burst
            .expect("magmin template should carry a death burst");
        // 2d6 fire DC 11 DEX 10ft (gap-2) per MM. Centralized check so a
        // future tweak (e.g. bumping the radius for the new mephit cohort)
        // can't silently de-tune the magmin's blast.
        assert_eq!(burst.damage_type, DamageType::Fire);
        assert_eq!(burst.save_ability, AbilityScoreType::Dexterity);
        assert_eq!(burst.dc, 11);
        assert_eq!(burst.radius, 2);
    }

    /// End-to-end: a magmin reduced to 0 HP fires its 2d6 fire burst on
    /// adjacent non-fire-immune actors. Routes through the engine's death
    /// cleanup pipeline so the integration with `cleanup_dead_actors` and
    /// `resolve_burst_save_damage` is exercised (not just the static
    /// struct shape). The victim is placed gap-2 from the magmin (10 ft)
    /// to land inside the burst; we use a CON-9 wizard chassis as the
    /// victim so the DEX save is realistically failable but the test
    /// asserts the HP delta directly rather than a specific roll.
    #[test]
    fn magmin_death_burst_damages_nearby_enemy() {
        use crate::actors::actor_template::CreatureTemplate;
        use crate::engine::actor_gen::ActorGenParams;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        // Plain Medium humanoid stand-in — no fire resistance, no evasion,
        // so the burst can land its full effect.
        static VICTIM: LazyLock<CreatureTemplate> = LazyLock::new(|| CreatureTemplate {
            name: "Test Victim",
            glyph: 'v',
            ac: 10,
            hitpoints: "10d10".parse().unwrap(),
            ..CreatureTemplate::defaults()
        });
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(42)).unwrap();
        let magmin = e
            .instantiate_creature(&MAGMIN_TEMPLATE, Coordinate::new(5, 5), 0, 0)
            .unwrap();
        let victim = e
            .instantiate_creature(&VICTIM, Coordinate::new(8, 5), 1, 0)
            .unwrap();
        let hp_before = e.actors[&victim].hitpoints();
        // Knock the magmin to 0 HP via direct damage (skips the action
        // pipeline). The cleanup pass then fires the death burst and
        // removes the corpse.
        e.actors
            .get_mut(&magmin)
            .unwrap()
            .take_typed_damage(999, DamageType::Cold);
        e.cleanup_dead_actors();
        assert!(!e.actors.contains_key(&magmin), "magmin should be removed");
        let hp_after = e.actors[&victim].hitpoints();
        assert!(
            hp_after < hp_before,
            "victim should take some damage from death burst (before {} after {})",
            hp_before,
            hp_after,
        );
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
            a.nonmagical_damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
    }
}
