use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    DUST_MEPHIT_BLINDING_BREATH, DUST_MEPHIT_CLAWS, DUST_MEPHIT_DEATH_BURST, ICE_MEPHIT_CLAWS,
    ICE_MEPHIT_DEATH_BURST, ICE_MEPHIT_FROST_BREATH, MAGMA_MEPHIT_CLAWS, MAGMA_MEPHIT_DEATH_BURST,
    MAGMA_MEPHIT_FIRE_BREATH, STEAM_MEPHIT_CLAWS, STEAM_MEPHIT_DEATH_BURST,
    STEAM_MEPHIT_STEAM_BREATH,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    elemental_body_defaults,
};
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Ice Mephit — CR ½ small elemental. The bickering frost-imp of the
/// Para-Elemental Plane of Ice — a frail flying construct of jagged
/// snow and brittle ice that lobs frost breath, slashes with sharp
/// icicle claws, and shatters into shrapnel on death. Slots alongside
/// the Magmin (CR ½ fire) and the Steam / Magma mephits at the bottom
/// of the elemental ladder.
///
/// Action lanes:
/// - **ice mephit claws** — DEX-based 1d4+DEX slashing melee with a
///   flat 1-point cold rider. The cold rider routes through the shared
///   `add_flat_damage_rider` chokepoint so per-target cold resistance
///   applies independently from the slashing base.
/// - **frost breath** — Recharge-6 15-ft cone (burst-2 / range-3) of
///   biting cold: 1d8 cold, DC 10 DEX, half on save. Routes through
///   the shared `BreathWeapon` chassis and the `"breath_weapon"`
///   recharge pool.
/// - **death burst** (passive on-death) — when reduced to 0 HP the
///   mephit shatters into icy shards, dealing 1d8 slashing in a 5-ft
///   radius. DC 11 DEX halves. Fires via the engine's
///   `cleanup_dead_actors` chokepoint before the corpse is removed.
///
/// Defensive identity: AC 11, ~21 HP (6d6). Cold immunity (the mephit
/// IS ice — its own breath self-freezes without it). Fire vulnerability
/// (RAW — its frozen body melts under flame). Poison immunity + non-
/// magical BPS resistance from the shared `elemental_body_defaults`
/// baseline. Standard 9-condition elemental immunity envelope.
///
/// Stat shape: AC 11, ~21 HP (6d6), STR 7, DEX 13, CON 10, INT 9,
/// WIS 11, CHA 12. Speed 30 (also flies 30, but the engine doesn't
/// model flight as a separate movement lane). Senses: Darkvision 60.
/// Languages: Aquan + Auran collapsed to Primordial in this engine.
/// Size Small. CR ½.
pub static ICE_MEPHIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ICE_MEPHIT_CLAWS);
    actions.push(&ICE_MEPHIT_FROST_BREATH);
    CreatureTemplate {
        name: "Ice Mephit",
        // 'i' (lowercase) — distinct from 'I' (Imp) and 'm' (Magmin).
        // The 'i' reads as the slim frost-imp silhouette.
        glyph: 'i',
        ac: 11,
        // 6d6 ≈ 21 average per MM (CR ½).
        hitpoints: "6d6".parse().unwrap(),
        // RAW speed line: Speed 30 ft., fly 30 ft. (hover)
        speed: 30.0,
        fly_speed: 30.0,
        hovers: true,
        strength: 7,
        intelligence: 9,
        dexterity: 13,
        wisdom: 11,
        constitution: 10,
        charisma: 12,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 0.5,
        size: Size::Small,
        creature_type: CreatureType::Elemental,
        actions,
        // Cold immunity (the mephit IS ice), fire vulnerability (its
        // frozen body melts), poison immunity + non-magical BPS
        // resistance from the shared elemental baseline.
        recharge_abilities: vec![("breath_weapon", 6)],
        death_burst: Some(&ICE_MEPHIT_DEATH_BURST),
        ..elemental_body_defaults([
            (DamageType::Cold, DamageModifier::Immunity),
            (DamageType::Fire, DamageModifier::Vulnerability),
        ])
    }
});

/// Steam Mephit — CR ¼ small elemental. The bubbling vapor-imp of the
/// Para-Elemental Plane of Steam — a hissing cloud of superheated water
/// that scalds nearby foes with its claws and breath, and explodes into
/// boiling steam on death. The Steam / Ice / Magma trio fills the
/// CR-¼–½ niche above the imp (CR 1) and below the Magmin chassis on
/// the small-elemental ladder.
///
/// Action lanes:
/// - **steam mephit claws** — DEX-based 1d4+DEX slashing melee with a
///   flat 1-point fire rider (the steam scalds skin on impact).
/// - **steam breath** — Recharge-6 15-ft cone of scalding vapor: 1d6
///   fire, DC 10 DEX, half on save.
/// - **death burst** (passive on-death) — 1d8 fire in a 5-ft radius
///   when reduced to 0 HP. DC 10 DEX halves.
///
/// Defensive identity: AC 10, ~17 HP (5d6). Fire immunity (the mephit
/// IS heat). Poison immunity + non-magical BPS resistance from the
/// shared elemental baseline. Standard 9-condition elemental immunity
/// envelope. No cold vulnerability per RAW (the steam mephit is heat-
/// pressure, not ice — cold doesn't condense it back as fast as fire
/// melts the ice mephit).
///
/// Stat shape: AC 10, ~17 HP (5d6), STR 5, DEX 11, CON 10, INT 11,
/// WIS 10, CHA 12. Speed 30. Senses: Darkvision 60. Languages:
/// Primordial. Size Small. CR ¼.
pub static STEAM_MEPHIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*STEAM_MEPHIT_CLAWS);
    actions.push(&STEAM_MEPHIT_STEAM_BREATH);
    CreatureTemplate {
        name: "Steam Mephit",
        // 's' (lowercase) — distinct from 'S' (Skeleton), 'i' (Ice
        // Mephit), 'm' (Magmin). Reads as the wispy steam-imp shape.
        glyph: 's',
        ac: 10,
        hitpoints: "5d6".parse().unwrap(),
        // RAW speed line: Speed 30 ft., fly 30 ft. (hover)
        speed: 30.0,
        fly_speed: 30.0,
        hovers: true,
        strength: 5,
        intelligence: 11,
        dexterity: 11,
        wisdom: 10,
        constitution: 10,
        charisma: 12,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 0.25,
        size: Size::Small,
        creature_type: CreatureType::Elemental,
        actions,
        recharge_abilities: vec![("breath_weapon", 6)],
        death_burst: Some(&STEAM_MEPHIT_DEATH_BURST),
        ..elemental_body_defaults([(
            DamageType::Fire,
            DamageModifier::Immunity,
        )])
    }
});

/// Magma Mephit — CR ½ small elemental. The smoldering lava-imp of the
/// Para-Elemental Plane of Magma — a glowing crust of molten rock that
/// swipes with searing claws, breathes fire, and erupts in a final
/// spray of lava on death. Sister to the Magmin (CR ½, also fire-themed
/// with a 2d6 death burst) but with the added Recharge-6 fire breath
/// cone for ranged pressure.
///
/// Action lanes:
/// - **magma mephit claws** — DEX-based 1d4+DEX slashing with a flat
///   1-point fire rider.
/// - **magma fire breath** — Recharge-6 15-ft fire cone: 1d8 fire,
///   DC 11 DEX, half on save.
/// - **death burst** (passive on-death) — 2d6 fire in a 5-ft radius
///   when reduced to 0 HP. Same damage profile as the Magmin but on
///   the shorter mephit radius. DC 11 DEX halves.
///
/// Defensive identity: AC 11, ~18 HP (4d6+4). Fire immunity. Cold
/// vulnerability (RAW — the lava crust solidifies and shatters under
/// magical cold). Poison immunity + non-magical BPS resistance via the
/// shared elemental baseline. Standard 9-condition elemental immunity
/// envelope.
///
/// Stat shape: AC 11, ~18 HP (4d6+4), STR 8, DEX 12, CON 12, INT 7,
/// WIS 10, CHA 10. Speed 30. Senses: Darkvision 60. Languages:
/// Primordial. Size Small. CR ½.
pub static MAGMA_MEPHIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MAGMA_MEPHIT_CLAWS);
    actions.push(&MAGMA_MEPHIT_FIRE_BREATH);
    CreatureTemplate {
        name: "Magma Mephit",
        // 'g' (lowercase) — 'm' is taken by Magmin and 'M' by other
        // mediums. 'g' reads as the lumpy magma-imp silhouette and is
        // free in the small-elemental cohort.
        glyph: 'g',
        ac: 11,
        hitpoints: "4d6+4".parse().unwrap(),
        // RAW speed line: Speed 30 ft., fly 30 ft. (hover)
        speed: 30.0,
        fly_speed: 30.0,
        hovers: true,
        strength: 8,
        intelligence: 7,
        dexterity: 12,
        wisdom: 10,
        constitution: 12,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 0.5,
        size: Size::Small,
        creature_type: CreatureType::Elemental,
        actions,
        recharge_abilities: vec![("breath_weapon", 6)],
        death_burst: Some(&MAGMA_MEPHIT_DEATH_BURST),
        ..elemental_body_defaults([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Vulnerability),
        ])
    }
});

/// Dust Mephit — CR ½ small elemental. The whirling grit-imp of the
/// Para-Elemental Plane of Dust — a Dust Bowl in miniature, kicking up
/// fine sand and choking grit with every flap of its translucent
/// papery wings. Distinct from the Ice / Steam / Magma mephits in that
/// its signature breath is **damage-free** — it just blinds — making
/// it the cohort's control variant rather than a tiny AoE damager.
///
/// Action lanes:
/// - **dust mephit claws** — DEX-based 1d4+DEX slashing melee. No
///   typed-rider tail (RAW the dust mephit's scrape is just grit, not
///   a typed energy bite).
/// - **blinding breath** — Recharge-6 15-ft cone (burst-2 / range-3) of
///   choking grit. DC 10 CON, **Blinded for 1 round on fail** — the
///   breath chassis with `damage: None`, which is the whole stat
///   block. Routes through the same
///   `"breath_weapon"` recharge pool the damage-cone mephits share so a
///   mixed mephit ambush can't double-tap.
/// - **death burst** (passive on-death) — 1d4 bludgeoning in a 5-ft
///   radius when reduced to 0 HP. DC 10 CON halves. The grit is
///   physical sand-spray, not a typed elemental energy.
///
/// Defensive identity: AC 12, ~17 HP (5d6). Poison immunity + non-
/// magical BPS resistance from the shared elemental baseline. NO
/// fire / cold vulnerability — the dust mephit doesn't have a paired
/// elemental opposite (unlike Ice ↔ Fire and Magma ↔ Cold). Standard
/// 9-condition elemental immunity envelope.
///
/// Stat shape: AC 12, ~17 HP (5d6), STR 5, DEX 14, CON 10, INT 9,
/// WIS 11, CHA 10. Speed 30. Senses: Darkvision 60. Languages:
/// Primordial. Size Small. CR ½.
pub static DUST_MEPHIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DUST_MEPHIT_CLAWS);
    actions.push(&DUST_MEPHIT_BLINDING_BREATH);
    CreatureTemplate {
        name: "Dust Mephit",
        // 'd' (lowercase) — distinct from 'D' (Djinni) and the other
        // mephit glyphs ('i' Ice, 's' Steam, 'g' Magma). 'd' reads as
        // the slim grit-imp silhouette and slots cleanly into the
        // lowercase-mephit family pattern.
        glyph: 'd',
        ac: 12,
        // 5d6 ≈ 17 average per MM (CR ½).
        hitpoints: "5d6".parse().unwrap(),
        // RAW speed line: Speed 30 ft., fly 30 ft. (hover)
        speed: 30.0,
        fly_speed: 30.0,
        hovers: true,
        strength: 5,
        intelligence: 9,
        dexterity: 14,
        wisdom: 11,
        constitution: 10,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 0.5,
        size: Size::Small,
        creature_type: CreatureType::Elemental,
        actions,
        recharge_abilities: vec![("breath_weapon", 6)],
        death_burst: Some(&DUST_MEPHIT_DEATH_BURST),
        // SRD 5.2 "Vulnerabilities Fire". The comment this replaces
        // said the dust mephit had no vulnerability because it had no
        // "paired elemental opposite" — but RAW gives every mephit one,
        // and dust's is fire: a cloud of grit is exactly the thing a
        // flame goes through. It is also the only one of the four that
        // was missing it, which is what made the omission look like a
        // rule rather than a gap.
        ..elemental_body_defaults([(DamageType::Fire, DamageModifier::Vulnerability)])
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn ice_mephit_template_shape() {
        let a = ActorInstance::from_creature_template(
            &ICE_MEPHIT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        assert!(a.find_action("ice mephit claws").is_some());
        assert!(a.find_action("frost breath").is_some());
        // Death burst should be wired through.
        assert!(a.death_burst().is_some());
    }

    #[test]
    fn ice_mephit_cold_immune_fire_vulnerable() {
        let a = ActorInstance::from_creature_template(
            &ICE_MEPHIT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // The signature ice mephit envelope — RAW immunity to its own
        // element, vulnerability to its opposing element.
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Vulnerability)
        );
        // Shared elemental baseline.
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
    }

    #[test]
    fn steam_mephit_template_shape() {
        let a = ActorInstance::from_creature_template(
            &STEAM_MEPHIT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        assert!(a.find_action("steam mephit claws").is_some());
        assert!(a.find_action("steam breath").is_some());
        assert!(a.death_burst().is_some());
        // No cold vulnerability on the steam mephit (RAW — steam isn't
        // ice). The Ice Mephit vs Magma Mephit comparison test catches
        // any future drift in the elemental-pair vulnerability lane.
        assert!(
            a.damage_modifier(DamageType::Cold).is_none(),
            "Steam Mephit should NOT be cold-vulnerable (only Magma / Ice are)"
        );
    }

    #[test]
    fn magma_mephit_template_shape() {
        let a = ActorInstance::from_creature_template(
            &MAGMA_MEPHIT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        // Fire immunity + cold vulnerability — the inverse of the Ice
        // Mephit's defensive profile.
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Vulnerability)
        );
    }

    #[test]
    fn dust_mephit_blinding_breath_is_recharge_gated() {
        // Pin the recharge gate so a future refactor of the
        // `BreathWeapon` chassis doesn't strip the
        // `custom_validate_input` check. The breath should only fire
        // while `"breath_weapon"` is available — symmetric to the
        // damage-variant Breath weapons.
        use crate::engine::actor_gen::ActorGenParams;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;
        let tp = TerrainGenParams {
            width: 20,
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
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        let mephit = e
            .instantiate_creature(&DUST_MEPHIT_TEMPLATE, Coordinate::new(2, 2), 0, 0)
            .unwrap();
        let breath = e.actors[&mephit]
            .find_action("blinding breath")
            .expect("dust mephit should have blinding breath");
        // Available on round 1 — recharge resources start in the
        // "available" state.
        assert!(
            breath.custom_validate_input(&e, mephit, None, None, None),
            "blinding breath should validate while recharged",
        );
        e.actors
            .get_mut(&mephit)
            .unwrap()
            .spend_recharge("breath_weapon");
        assert!(
            !breath.custom_validate_input(&e, mephit, None, None, None),
            "blinding breath should not validate after the recharge is spent",
        );
    }

    #[test]
    fn dust_mephit_template_shape() {
        let a = ActorInstance::from_creature_template(
            &DUST_MEPHIT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        assert!(a.find_action("dust mephit claws").is_some());
        assert!(a.find_action("blinding breath").is_some());
        // Death burst should be wired through.
        assert!(a.death_burst().is_some());
        // SRD 5.2 "Vulnerabilities Fire" — every mephit in the book has
        // a paired opposite and dust's is fire. Cold is not one: a
        // cloud of grit does not care about the temperature.
        assert_eq!(
            a.damage_modifier(DamageType::Fire),
            Some(DamageModifier::Vulnerability),
            "Dust Mephit is fire-vulnerable per SRD 5.2",
        );
        assert!(
            a.damage_modifier(DamageType::Cold).is_none(),
            "Dust Mephit should NOT have cold vulnerability (RAW)",
        );
        // Standard elemental baseline still applies.
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity),
        );
    }
}
