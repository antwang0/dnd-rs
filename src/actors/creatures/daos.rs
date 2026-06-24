use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DAO_MAUL, DAO_MULTI, DAO_STONE_SNARE};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    ELEMENTAL_CONDITION_IMMUNITIES, elemental_damage_modifiers,
};
use crate::engine::types::{
    AbilityScoreType, CreatureType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Dao — CR 11 large elemental (earth genie). Completes the noble genie
/// family alongside the Djinni (CR 11 air), Efreeti (CR 11 fire), and
/// Marid (CR 11 water). The dao is the surly Pasha of the Plane of
/// Earth: a stocky, gem-skinned giant who manifests in a cloud of dust
/// and gravel, wielding a massive iron maul that drives the ground
/// itself into the target on impact.
///
/// Action lanes:
/// - **dao multiattack** — 2 maul swings per Action via the shared
///   homogeneous `Multiattack` chassis. Each swing carries the 2d10
///   thunder rider on top of the 2d6 bludgeoning base, so a clean
///   double-hit lands ~56 typed damage — the heaviest melee output
///   among the genie family. Routes through `add_flat_damage_rider`
///   so per-target thunder resistance applies independently from the
///   bludgeoning base.
/// - **dao maul** (standalone) — STR-based 2d6+STR bludgeoning with
///   the 2d10 thunder rider for the AI's single-target fallback.
/// - **stone snare** — Recharge 5–6 ranged save-for-half attack at
///   30ft range (12 tiles). DC 17 STR save, 4d8 bludgeoning on fail
///   (half on pass), plus an `EarthenGrasped` (Restrained envelope)
///   install for 1 round on a failed save. The dao's stand-off
///   crowd-control tool — fire it, retreat behind a wall of grasping
///   stone, swing mauls while it recharges.
///
/// Defensive identity: AC 18 (natural armor — the gem-studded hide of
/// the earth genie), 187 HP (15d10+105). Heavier than the djinni's
/// 161 HP, marid's 229 HP, and lighter than the efreeti's 200 HP but
/// at the highest AC of the genie family — the dao trades HP soak for
/// armor coverage. Standard elemental envelope: non-magical BPS
/// resistance, poison immunity. Magic Resistance gives advantage on
/// every save vs spells. Full elemental condition envelope (Charmed
/// / Frightened / Paralyzed / Petrified / Poisoned / Asleep / Prone /
/// Grappled / Restrained) via the shared `ELEMENTAL_CONDITION_IMMUNITIES`.
///
/// Stat shape: AC 18, ~187 HP (15d10+105), STR 23, DEX 12, CON 24,
/// INT 12, WIS 17, CHA 18. Speed 30 (RAW also grants burrow 30 and fly
/// 30 hover which we don't model — the earth glide / hover envelope is
/// 3D positioning the engine doesn't surface). Senses: Darkvision
/// 120ft. Languages: Terran collapsed to Primordial in this engine
/// (matching the marid / djinni / efreeti rollup of the four
/// elemental tongues into the umbrella language). Size Large. CR 11.
pub static DAO_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DAO_MULTI);
    actions.push(&DAO_MAUL);
    actions.push(&*DAO_STONE_SNARE);
    CreatureTemplate {
        name: "Dao",
        // 'Π' (Greek capital pi) — distinct from every uppercase Latin
        // letter currently in the glyph map. Pi's twin-column silhouette
        // reads as a stone arch / standing-stones glyph, matching the
        // dao's earth-genie flavor (a Pasha hewn from living rock,
        // surrounded by pillars of stone). 'D' is taken by the djinni;
        // 'q' / 'F' / 'Q' (Earth Elemental) are similarly reserved by
        // the rest of the genie / elemental family.
        glyph: 'Π',
        ac: 18,
        // 15d10+105 ≈ 187 average per MM (CR 11).
        hitpoints: "15d10+105".parse().unwrap(),
        speed: 30.,
        strength: 23,
        intelligence: 12,
        dexterity: 12,
        wisdom: 17,
        constitution: 24,
        charisma: 18,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 11.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        // Dao proficient saves: STR, CON, WIS, CHA per MM. The strong-
        // arm + heavy-constitution + willful + force-of-personality
        // envelope befitting an earth genie — heavier on the physical
        // saves than the djinni (DEX-focused) or the efreeti (mental-
        // focused), matching the dao's "tough stone-skinned brute"
        // flavor.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Standard elemental damage envelope (poison immune + BPS
        // resistance) with no signature damage-type overlay. The dao's
        // signature element (earth / stone) doesn't map cleanly to a
        // single 5e damage type — bludgeoning is already covered by
        // the elemental baseline BPS resistance, and thunder is the
        // *offensive* element (Maul rider). Distinct from the djinni
        // (thunder + lightning resistance), efreeti (fire immunity),
        // and marid (acid immunity + cold resistance) — the dao's
        // defensive envelope is "just the elemental baseline" with
        // signature offensive output on the maul rider instead.
        damage_modifiers: elemental_damage_modifiers([]),
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        // Magic Resistance: advantage on saves vs spells / magical
        // effects. Standard upper-tier genie trait — every member of
        // the genie family carries this.
        has_magic_resistance: true,
        has_extra_attack: true,
        // Stone Snare recharges on a d6 roll of 5+. The encounter's
        // start-of-turn recharge hook flips the resource back on
        // automatically; `DaoStoneSnare::custom_validate_input` gates
        // the cast. Higher threshold than the marid's water jet (4+)
        // — the stone snare hits harder (4d8 + Restrained vs 6d6 +
        // push) so it recharges less often.
        recharge_abilities: vec![("stone_snare", 5)],
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::{Coordinate, DamageModifier, DamageType};

    #[test]
    fn dao_template_shape() {
        let a = ActorInstance::from_creature_template(
            &DAO_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 11.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Elemental);
        // Three action lanes — multi primary, maul standalone for AI
        // fallback, stone snare for ranged crowd-control.
        assert!(a.find_action("dao multiattack").is_some());
        assert!(a.find_action("dao maul").is_some());
        assert!(a.find_action("stone snare").is_some());
    }

    #[test]
    fn dao_has_elemental_envelope() {
        let a = ActorInstance::from_creature_template(
            &DAO_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Standard elemental baseline: poison immune + BPS resistance.
        // No signature damage-type overlay — the dao's offensive
        // signature (thunder) doesn't double as a defensive one.
        assert_eq!(
            a.damage_modifier(DamageType::Poison),
            Some(DamageModifier::Immunity)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Bludgeoning),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Piercing),
            Some(DamageModifier::Resistance)
        );
        assert_eq!(
            a.damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        // No special thunder defensive bias (the dao deals thunder; it
        // doesn't take less of it).
        assert_eq!(a.damage_modifier(DamageType::Thunder), None);
        assert!(a.has_magic_resistance());
        // Full elemental condition envelope.
        assert!(a.effectively_immune_to_condition(Condition::Charmed));
        assert!(a.effectively_immune_to_condition(Condition::Frightened));
        assert!(a.effectively_immune_to_condition(Condition::Paralyzed));
        assert!(a.effectively_immune_to_condition(Condition::Poisoned));
        assert!(a.effectively_immune_to_condition(Condition::Petrified));
    }

    #[test]
    fn dao_stone_snare_starts_available() {
        let a = ActorInstance::from_creature_template(
            &DAO_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // Recharge abilities start in the available state — the dao
        // can fire its stone snare on the first round of combat
        // without waiting for a recharge roll.
        assert!(a.is_recharge_available("stone_snare"));
    }

    #[test]
    fn dao_stone_snare_validates_only_while_recharged() {
        // Pin the recharge gate so a future refactor doesn't
        // accidentally strip the `custom_validate_input` check — the
        // snare should only fire while the `"stone_snare"` resource is
        // available, mirroring the dragon's breath-weapon and marid's
        // water-jet gates.
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
        let dao = e
            .instantiate_creature(&DAO_TEMPLATE, Coordinate::new(2, 2), 0, 0)
            .unwrap();
        let snare = e.actors[&dao]
            .find_action("stone snare")
            .expect("dao should have stone snare");
        assert!(
            snare.custom_validate_input(&e, dao, None, None, None),
            "stone snare should validate while recharged",
        );
        e.actors
            .get_mut(&dao)
            .unwrap()
            .spend_recharge("stone_snare");
        assert!(
            !snare.custom_validate_input(&e, dao, None, None, None),
            "stone snare should not validate after spending recharge",
        );
    }

    #[test]
    fn dao_proficient_in_physical_and_mental_saves() {
        let a = ActorInstance::from_creature_template(
            &DAO_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // RAW dao save proficiencies: STR + CON + WIS + CHA. The earth
        // genie's tough-stone-skin physical envelope plus the noble
        // willful resistance to magical compulsion. Distinct from the
        // djinni (DEX/WIS/CHA — agile + willful + force-of-personality)
        // and the efreeti (INT/WIS/CHA — mental focus, no DEX).
        assert!(a.is_save_proficient(AbilityScoreType::Strength));
        assert!(a.is_save_proficient(AbilityScoreType::Constitution));
        assert!(a.is_save_proficient(AbilityScoreType::Wisdom));
        assert!(a.is_save_proficient(AbilityScoreType::Charisma));
        // INT and DEX are NOT proficient — the dao is the heaviest /
        // least nimble of the genies on the physical side, and lacks
        // the djinni's elemental cleverness on the mental side.
        assert!(!a.is_save_proficient(AbilityScoreType::Dexterity));
        assert!(!a.is_save_proficient(AbilityScoreType::Intelligence));
    }
}
