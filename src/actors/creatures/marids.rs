use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MARID_MULTI, MARID_TRIDENT, MARID_WATER_JET};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    ELEMENTAL_CONDITION_IMMUNITIES, elemental_damage_modifiers,
};
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Marid — CR 11 large elemental (water genie). The proud noble of the
/// Plane of Water: a bronze-blue-skinned giant wreathed in living
/// surf, wielding a trident of coral and brass. One of the four noble
/// genies — Djinni (CR 11 air), Efreeti (CR 11 fire), Marid (CR 11
/// water), and Dao (CR 11 earth) — alongside the lower-tier elemental
/// quartet (Air / Earth / Fire / Water Elementals at CR 5–6) that
/// fills the bench-tier elemental flavor.
///
/// Action lanes:
/// - **marid multiattack** — 3 trident swings per Action via the shared
///   homogeneous `Multiattack` chassis. Mirrors the djinni's 3-scimitar
///   shape but on a heavier 2d6 piercing die — the marid trades the
///   djinni's per-swing 1d6 thunder rider for bigger base dice, so the
///   per-Action damage is comparable on a target without thunder
///   resistance but lands more reliably against typed-resistant
///   defenders.
/// - **marid trident** (standalone) — STR-based 2d6+STR piercing for
///   the AI's single-target fallback.
/// - **water jet** — Recharge 4–6 ranged save-for-half at 24 tiles
///   (60 ft). DC 17 DEX save, 6d6 bludgeoning on fail (half on pass),
///   plus an 8-tile (20 ft) push from the marid on a failed save.
///   The marid's stand-off pressure tool — fire it, retreat, swing
///   tridents while it recharges. Push routes through `PushActor` so
///   wall / occupancy blocking applies.
///
/// Defensive identity: AC 17 (natural armor — the water genie's
/// glistening hide), 229 HP (17d10+136). Heavier than the djinni's
/// 161 HP and the efreeti's 200 HP — the marid's elemental form soaks
/// the most punishment of the genie trio. Standard elemental envelope:
/// non-magical BPS resistance, poison immunity. Acid immunity (the
/// water genie's element dilutes corrosive splash) plus cold resistance
/// (water tolerates cold better than fire/lightning). Magic Resistance
/// gives advantage on every save vs spells. Full elemental condition
/// envelope (Charmed / Frightened / Paralyzed / Petrified / Poisoned /
/// Asleep / Prone / Grappled / Restrained) via the shared
/// `ELEMENTAL_CONDITION_IMMUNITIES`.
///
/// Stat shape: AC 17, ~229 HP (17d10+136), STR 18, DEX 15, CON 22,
/// INT 14, WIS 18, CHA 18. Speed 30 (RAW also grants swim 90, whose
/// magnitude we
/// don't model). Senses: Darkvision 120ft. Languages: Aquan collapsed
/// to Primordial in this engine. Size Large. CR 11.
pub static MARID_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MARID_MULTI);
    actions.push(&MARID_TRIDENT);
    actions.push(&*MARID_WATER_JET);
    CreatureTemplate {
        name: "Marid",
        // 'M' is taken by Mimic / Manticore / Mind Flayer / Mummy /
        // Magmin. 'I' for "ifrit-style" reads wrong (efreeti). 'd' is
        // taken by Drow / Dryad / Drider. Going with 'q' (aquatic
        // serpentine silhouette, free in the glyph map) — 'q' for
        // "aqua" reads as the watery mnemonic.
        glyph: 'q',
        ac: 17,
        // 17d10+136 ≈ 229 average per MM (CR 11).
        hitpoints: "17d10+136".parse().unwrap(),
        // RAW speed line: Speed 30 ft., fly 60 ft. (hover)
        speed: 30.0,
        fly_speed: 60.0,
        hovers: true,
        strength: 18,
        intelligence: 14,
        dexterity: 15,
        wisdom: 18,
        constitution: 22,
        charisma: 18,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 11.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        // Marid proficient saves: DEX, WIS, CHA per MM. The agile +
        // willful + force-of-personality saves befitting a water genie.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Standard elemental damage envelope (poison immune + BPS
        // resistance) overlaid with acid immunity (the water genie's
        // signature dilution affinity) and cold resistance (water
        // tolerates cold better than fire / lightning). Mirrors the
        // djinni's elemental-overlay shape (thunder + lightning
        // resistance) but on the water-genie pairing.
        damage_modifiers: elemental_damage_modifiers([
            (DamageType::Acid, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
        ]),
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        // Magic Resistance: advantage on saves vs spells / magical
        // effects. Standard upper-tier genie trait.
        has_magic_resistance: true,
        has_extra_attack: true,
        // Water Jet recharges on a d6 roll of 4+. The encounter's start-
        // of-turn recharge hook flips the resource back on automatically;
        // `MaridWaterJet::custom_validate_input` gates the cast.
        recharge_abilities: vec![("water_jet", 4)],
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
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
    fn marid_template_shape() {
        let a = ActorInstance::from_creature_template(
            &MARID_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 11.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        // Three action lanes — multi primary, trident standalone for AI
        // fallback, water jet for ranged stand-off.
        assert!(a.find_action("marid multiattack").is_some());
        assert!(a.find_action("marid trident").is_some());
        assert!(a.find_action("water jet").is_some());
    }

    #[test]
    fn marid_has_water_elemental_envelope() {
        let a = ActorInstance::from_creature_template(
            &MARID_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Acid immunity is the signature trait — also poison immune,
        // BPS resistant from the shared elemental baseline, cold
        // resistant from the water-genie overlay.
        assert_eq!(
            a.damage_modifier(DamageType::Acid),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Cold),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        assert!(a.has_magic_resistance());
        // Full elemental condition envelope.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
    }

    #[test]
    fn marid_water_jet_starts_available() {
        let a = ActorInstance::from_creature_template(
            &MARID_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Recharge abilities start in the available state — the marid
        // can fire its water jet on the first round of combat without
        // waiting for a recharge roll.
        assert!(a.is_recharge_available("water_jet"));
    }

    #[test]
    fn marid_water_jet_validates_only_while_recharged() {
        // Pin the recharge gate so a future refactor doesn't accidentally
        // strip the `custom_validate_input` check — the jet should only
        // fire while the `"water_jet"` resource is available, mirroring
        // the dragon's breath-weapon gate.
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
        let marid = e
            .instantiate_creature(&MARID_TEMPLATE, Coordinate::new(2, 2), 0, 0)
            .unwrap();
        let jet = e.actors[&marid]
            .find_action("water jet")
            .expect("marid should have water jet");
        assert!(
            jet.custom_validate_input(&e, marid, None, None, None),
            "water jet should validate while recharged",
        );
        e.actors.get_mut(&marid).unwrap().spend_recharge("water_jet");
        assert!(
            !jet.custom_validate_input(&e, marid, None, None, None),
            "water jet should not validate after spending recharge",
        );
    }
}
