pub mod aboleths;
pub mod artificers;
pub mod eldritch_cannons;
pub mod steel_defenders;
pub mod dragonborn;
pub mod gnomes;
pub mod tieflings;
pub mod air_elementals;
pub mod ankhegs;
pub mod animated_armors;
pub mod balors;
pub mod bandit_captains;
pub mod bandits;
pub mod banshees;
pub mod barbarians;
pub mod basilisks;
pub mod bards;
pub mod beholders;
pub mod berserkers;
pub mod bone_devils;
pub mod bugbears;
pub mod bullettes;
pub mod chimeras;
pub mod chuuls;
pub mod clerics;
pub mod aasimars;
pub mod cloakers;
pub mod cockatrices;
pub mod couatls;
pub mod cult_fanatics;
pub mod death_knights;
pub mod deep_tentacles;
pub mod dire_wolves;
pub mod displacer_beasts;
pub mod doppelgangers;
pub mod dragons;
pub mod drow;
pub mod druids;
pub mod dwarves;
pub mod earth_elementals;
pub mod erinyes;
pub mod ettins;
pub mod fighters;
pub mod fire_elementals;
pub mod flameskulls;
pub mod fire_imps;
pub mod frost_giants;
pub mod gargoyles;
pub mod gelatinous_cubes;
pub mod ghosts;
pub mod giant_scorpions;
pub mod gricks;
pub mod ghouls;
pub mod half_orcs;
pub mod halflings;
pub mod glabrezus;
pub mod gnoll_pack_lords;
pub mod gnolls;
pub mod goblin_bosses;
pub mod goblins;
pub mod harpies;
pub mod hell_hounds;
pub mod hill_giants;
pub mod hippogriffs;
pub mod hobgoblin_warlords;
pub mod hobgoblins;
pub mod hydras;
pub mod imps;
pub mod knights;
pub mod kobolds;
pub mod liches;
pub mod mages;
pub mod manticores;
pub mod mariliths;
pub mod medusas;
pub mod mimics;
pub mod mind_flayers;
pub mod minotaurs;
pub mod monks;
pub mod mummies;
pub mod nightmares;
pub mod nothics;
pub mod ogres;
pub mod orcs;
pub mod owlbears;
pub mod paladins;
pub mod phase_spiders;
pub mod pit_fiends;
pub mod rangers;
pub mod rogues;
pub mod ropers;
pub mod rust_monsters;
pub mod salamanders;
pub mod shadows;
pub mod shambling_mounds;
pub mod skeletons;
pub mod slimes;
pub mod solars;
pub mod sorcerers;
pub mod spectators;
pub mod specters;
pub mod spiders;
pub mod stirges;
pub mod stone_giants;
pub mod stone_golems;
pub mod storm_giants;
pub mod tarrasques;
pub mod treants;
pub mod trolls;
pub mod umber_hulks;
pub mod vampire_spawns;
pub mod vampires;
pub mod veterans;
pub mod vrocks;
pub mod warlocks;
pub mod werewolves;
pub mod wights;
pub mod wildfire_spirits;
pub mod wisps;
pub mod wizards;
pub mod wolves;
pub mod worgs;
pub mod wraiths;
pub mod wyverns;
pub mod yetis;
pub mod zombies;
pub mod giant_apes;
pub mod giant_eagles;
pub mod lizardfolk;
pub mod sahuagins;
pub mod centaurs;
pub mod behirs;
pub mod boars;
pub mod brown_bears;
pub mod giant_toads;
pub mod pseudodragons;
pub mod tigers;
pub mod tiny_animated_objects;
pub mod polar_bears;
pub mod lions;
pub mod fire_giants;
pub mod cyclopes;
pub mod rocs;
pub mod pegasi;
pub mod winter_wolves;
pub mod triceratopses;
pub mod tyrannosauruses;
pub mod carrion_crawlers;
pub mod water_elementals;
pub mod saber_toothed_tigers;
pub mod hyenas;
pub mod giant_hyenas;
pub mod green_hags;
pub mod gorgons;
pub mod yuan_ti;
pub mod cambions;
pub mod dryads;
pub mod bullywugs;
pub mod quasits;
pub mod shadow_demons;
pub mod succubi;
pub mod intellect_devourers;
pub mod xorns;
pub mod oni;
pub mod merrow;
pub mod giant_crabs;
pub mod cloud_giants;
pub mod hezrous;
pub mod gibbering_mouthers;
pub mod iron_golems;
pub mod mummy_lords;
pub mod rakshasas;
pub mod hook_horrors;
pub mod dragon_turtles;
pub mod krakens;
pub mod helmed_horrors;
pub mod pixies;
pub mod androsphinxes;
pub mod unicorns;
pub mod driders;
pub mod sea_hags;
pub mod night_hags;
pub mod spirit_nagas;
pub mod otyughs;
pub mod sprites;
pub mod death_dogs;
pub mod magmins;
pub mod galeb_duhrs;
pub mod griffons;
pub mod lamias;
pub mod werebears;
pub mod wereboars;
pub mod wererats;
pub mod weretigers;
pub mod ettercaps;
pub mod awakened_trees;
pub mod dretches;
pub mod lemures;
pub mod bearded_devils;
pub mod blink_dogs;
pub mod mephits;
pub mod black_puddings;
pub mod flesh_golems;
pub mod horned_devils;
pub mod nalfeshnees;
pub mod djinn;
pub mod efreeti;
pub mod constrictor_snakes;
pub mod marids;
pub mod crocodiles;
pub mod daos;
pub mod invisible_stalkers;
pub mod mammoths;
pub mod purple_worms;
pub mod devas;
pub mod quaggoths;
pub mod allips;
pub mod giant_octopuses;
pub mod plesiosauruses;
pub mod pteranodons;
pub mod thugs;
pub mod tribal_warriors;
pub mod scouts;
pub mod giant_rats;
pub mod commoners;
pub mod mastiffs;
pub mod guards;
pub mod grimlocks;
pub mod giant_frogs;
pub mod ghasts;
pub mod hawks;
pub mod giant_lizards;
pub mod giant_wolf_spiders;
pub mod reef_sharks;
pub mod hunter_sharks;
pub mod giant_sharks;
pub mod warhorses;
pub mod giant_vultures;
pub mod giant_bats;
pub mod giant_centipedes;
pub mod vine_blights;
pub mod twig_blights;
pub mod needle_blights;
pub mod giant_boars;
pub mod giant_goats;
pub mod giant_owls;
pub mod giant_poisonous_snakes;
pub mod killer_whales;
pub mod crawling_claws;
pub mod riding_horses;
pub mod draft_horses;
pub mod awakened_shrubs;
pub mod bats;
pub mod camels;
pub mod giant_badgers;
pub mod giant_wasps;
pub mod rats;
pub mod cats;
pub mod frogs;
pub mod lizards;
pub mod weasels;
pub mod goats;
pub mod mules;
pub mod ponies;
pub mod elks;
pub mod summoned_spirits;
pub mod swarms;

use crate::actors::actor_template::CreatureTemplate;

/// Every playable template in the engine, grouped by family.
///
/// Twelve class families, plus two lineage families whose chassis is a
/// class but whose identity is a racial trait — the fifteen dragonborn
/// ancestries and the six single-lineage builds (Tiefling, Aasimar,
/// Mountain Dwarf, Halfling, Half-Orc, Gnome). Those twenty-one existed
/// fully implemented and completely unreachable: absent from this
/// registry so no player could pick them, and absent from
/// `EncounterInstance::template_pool` so no encounter could roll them.
/// The only thing that ever instantiated one was a unit test. Listing
/// them here makes them playable and, more usefully, subjects them to
/// every sweep below.
///
/// One registry, two consumers today — the `every_pc_class_family_renders_unambiguously`
/// sweep in `engine::encounter` (no two templates in a family share a
/// name or a glyph) and `every_pc_action_is_reachable_by_its_canonical_name`
/// in `engine::prompt` (every action on every template resolves to
/// itself through the parser). Both used to carry their own verbatim
/// copy of this 145-line list, which meant a new subclass was correct
/// only if whoever added it remembered to edit two unrelated test
/// modules; forgetting either one silently narrowed a sweep rather than
/// failing.
///
/// **Adding a subclass template is one line here and nothing else.**
/// That is the whole point of the registry: it is the single place the
/// engine's answer to "what can a player be?" is written down, and
/// every guarantee that ought to hold across all of them reads it.
///
/// Returned by value rather than stored in a `LazyLock` because
/// `CreatureTemplate` isn't `Copy` and the callers want plain
/// `&'static` references — building the outer `Vec` is a few hundred
/// pointer copies on a path that runs twice in the test suite and never
/// in play.
pub fn pc_template_families() -> Vec<(&'static str, Vec<&'static CreatureTemplate>)> {
    vec![
            (
                "barbarian",
                vec![
                    &*barbarians::BARBARIAN_TEMPLATE,
                    &*barbarians::TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::WOLF_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::EAGLE_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::TIGER_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::ELK_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::WOLVERINE_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::PANTHER_TOTEM_BARBARIAN_TEMPLATE,
                    &*barbarians::SEA_STORM_HERALD_BARBARIAN_TEMPLATE,
                    &*barbarians::DESERT_STORM_HERALD_BARBARIAN_TEMPLATE,
                    &*barbarians::TUNDRA_STORM_HERALD_BARBARIAN_TEMPLATE,
                    &*barbarians::BERSERKER_BARBARIAN_TEMPLATE,
                    &*barbarians::ZEALOT_BARBARIAN_TEMPLATE,
                    &*barbarians::ANCESTRAL_GUARDIAN_BARBARIAN_TEMPLATE,
                    &*barbarians::BITE_BEAST_BARBARIAN_TEMPLATE,
                    &*barbarians::CLAW_BEAST_BARBARIAN_TEMPLATE,
                    &*barbarians::TAIL_BEAST_BARBARIAN_TEMPLATE,
                ],
            ),
            (
                "artificer",
                vec![
                    &*artificers::ARTIFICER_TEMPLATE,
                    &*artificers::ALCHEMIST_ARTIFICER_TEMPLATE,
                    &*artificers::ARMORER_ARTIFICER_TEMPLATE,
                    &*artificers::INFILTRATOR_ARTIFICER_TEMPLATE,
                    &*artificers::ARTILLERIST_ARTIFICER_TEMPLATE,
                    &*artificers::BATTLE_SMITH_ARTIFICER_TEMPLATE,
                ],
            ),
            (
                "bard",
                vec![
                    &*bards::BARD_TEMPLATE,
                    &*bards::VALOR_BARD_TEMPLATE,
                    &*bards::SWORDS_BARD_TEMPLATE,
                    &*bards::LORE_BARD_TEMPLATE,
                    &*bards::WHISPERS_BARD_TEMPLATE,
                    &*bards::ELOQUENCE_BARD_TEMPLATE,
                    &*bards::GLAMOUR_BARD_TEMPLATE,
                ],
            ),
            (
                "cleric",
                vec![
                    &*clerics::CLERIC_TEMPLATE,
                    &*clerics::WAR_CLERIC_TEMPLATE,
                    &*clerics::LIGHT_CLERIC_TEMPLATE,
                    &*clerics::TEMPEST_CLERIC_TEMPLATE,
                    &*clerics::LIFE_CLERIC_TEMPLATE,
                    &*clerics::GRAVE_CLERIC_TEMPLATE,
                    &*clerics::FORGE_CLERIC_TEMPLATE,
                    &*clerics::TWILIGHT_CLERIC_TEMPLATE,
                    &*clerics::ARCANA_CLERIC_TEMPLATE,
                    &*clerics::NATURE_CLERIC_TEMPLATE,
                    &*clerics::TRICKERY_CLERIC_TEMPLATE,
                    &*clerics::KNOWLEDGE_CLERIC_TEMPLATE,
                    &*clerics::DEATH_CLERIC_TEMPLATE,
                    &*clerics::ORDER_CLERIC_TEMPLATE,
                ],
            ),
            (
                "druid",
                vec![
                    &*druids::DRUID_TEMPLATE,
                    &*druids::LAND_DRUID_TEMPLATE,
                    &*druids::MOON_DRUID_TEMPLATE,
                    &*druids::SPORES_DRUID_TEMPLATE,
                    &*druids::STARS_DRUID_TEMPLATE,
                    &*druids::WILDFIRE_DRUID_TEMPLATE,
                ],
            ),
            (
                "fighter",
                vec![
                    &*fighters::FIGHTER_TEMPLATE,
                    &*fighters::CHAMPION_TEMPLATE,
                    &*fighters::SAMURAI_FIGHTER_TEMPLATE,
                    &*fighters::ELDRITCH_KNIGHT_FIGHTER_TEMPLATE,
                    &*fighters::PSI_WARRIOR_FIGHTER_TEMPLATE,
                    &*fighters::CAVALIER_FIGHTER_TEMPLATE,
                    &*fighters::RUNE_KNIGHT_FIGHTER_TEMPLATE,
                    &*fighters::ARCANE_ARCHER_FIGHTER_TEMPLATE,
                ],
            ),
            (
                "monk",
                vec![
                    &*monks::MONK_TEMPLATE,
                    &*monks::OPEN_HAND_MONK_TEMPLATE,
                    &*monks::LONG_DEATH_MONK_TEMPLATE,
                    &*monks::SHADOW_MONK_TEMPLATE,
                    &*monks::FOUR_ELEMENTS_MONK_TEMPLATE,
                    &*monks::KENSEI_MONK_TEMPLATE,
                    &*monks::SUN_SOUL_MONK_TEMPLATE,
                    &*monks::MERCY_MONK_TEMPLATE,
                    &*monks::ASTRAL_SELF_MONK_TEMPLATE,
                ],
            ),
            (
                "paladin",
                vec![
                    &*paladins::PALADIN_TEMPLATE,
                    &*paladins::DEVOTION_PALADIN_TEMPLATE,
                    &*paladins::ANCIENTS_PALADIN_TEMPLATE,
                    &*paladins::VENGEANCE_PALADIN_TEMPLATE,
                    &*paladins::OATHBREAKER_PALADIN_TEMPLATE,
                    &*paladins::GLORY_PALADIN_TEMPLATE,
                    &*paladins::WATCHERS_PALADIN_TEMPLATE,
                    &*paladins::CONQUEST_PALADIN_TEMPLATE,
                    &*paladins::CROWN_PALADIN_TEMPLATE,
                ],
            ),
            (
                "ranger",
                vec![
                    &*rangers::RANGER_TEMPLATE,
                    &*rangers::HUNTER_RANGER_TEMPLATE,
                    &*rangers::GLOOM_STALKER_RANGER_TEMPLATE,
                    &*rangers::FEY_WANDERER_RANGER_TEMPLATE,
                    &*rangers::HORIZON_WALKER_RANGER_TEMPLATE,
                    &*rangers::MONSTER_SLAYER_RANGER_TEMPLATE,
                    &*rangers::SWARMKEEPER_RANGER_TEMPLATE,
                    &*rangers::BEAST_MASTER_RANGER_TEMPLATE,
                ],
            ),
            (
                "rogue",
                vec![
                    &*rogues::ROGUE_TEMPLATE,
                    &*rogues::ASSASSIN_ROGUE_TEMPLATE,
                    &*rogues::SWASHBUCKLER_ROGUE_TEMPLATE,
                    &*rogues::SCOUT_ROGUE_TEMPLATE,
                    &*rogues::ARCANE_TRICKSTER_ROGUE_TEMPLATE,
                    &*rogues::SOULKNIFE_ROGUE_TEMPLATE,
                    &*rogues::THIEF_ROGUE_TEMPLATE,
                    &*rogues::PHANTOM_ROGUE_TEMPLATE,
                ],
            ),
            (
                "sorcerer",
                vec![
                    &*sorcerers::SORCERER_TEMPLATE,
                    &*sorcerers::DRACONIC_SORCERER_TEMPLATE,
                    &*sorcerers::STORM_SORCERER_TEMPLATE,
                    &*sorcerers::ABERRANT_MIND_SORCERER_TEMPLATE,
                    &*sorcerers::DIVINE_SOUL_SORCERER_TEMPLATE,
                    &*sorcerers::SHADOW_MAGIC_SORCERER_TEMPLATE,
                ],
            ),
            (
                "warlock",
                vec![
                    &*warlocks::WARLOCK_TEMPLATE,
                    &*warlocks::FIEND_WARLOCK_TEMPLATE,
                    &*warlocks::UNDYING_WARLOCK_TEMPLATE,
                    &*warlocks::GREAT_OLD_ONE_WARLOCK_TEMPLATE,
                    &*warlocks::ARCHFEY_WARLOCK_TEMPLATE,
                    &*warlocks::CELESTIAL_WARLOCK_TEMPLATE,
                    &*warlocks::MARID_WARLOCK_TEMPLATE,
                    &*warlocks::DAO_WARLOCK_TEMPLATE,
                    &*warlocks::DJINNI_WARLOCK_TEMPLATE,
                    &*warlocks::EFREETI_WARLOCK_TEMPLATE,
                    &*warlocks::HEXBLADE_WARLOCK_TEMPLATE,
                    &*warlocks::UNDEAD_WARLOCK_TEMPLATE,
                    &*warlocks::FATHOMLESS_WARLOCK_TEMPLATE,
                ],
            ),
            (
                "dragonborn",
                vec![
                    &*dragonborn::DRAGONBORN_TEMPLATE,
                    &*dragonborn::BLACK_DRAGONBORN_TEMPLATE,
                    &*dragonborn::BLUE_DRAGONBORN_TEMPLATE,
                    &*dragonborn::GREEN_DRAGONBORN_TEMPLATE,
                    &*dragonborn::WHITE_DRAGONBORN_TEMPLATE,
                    &*dragonborn::AMETHYST_DRAGONBORN_TEMPLATE,
                    &*dragonborn::CRYSTAL_DRAGONBORN_TEMPLATE,
                    &*dragonborn::EMERALD_DRAGONBORN_TEMPLATE,
                    &*dragonborn::SAPPHIRE_DRAGONBORN_TEMPLATE,
                    &*dragonborn::TOPAZ_DRAGONBORN_TEMPLATE,
                    &*dragonborn::SILVER_DRAGONBORN_TEMPLATE,
                    &*dragonborn::BRASS_DRAGONBORN_TEMPLATE,
                    &*dragonborn::BRONZE_DRAGONBORN_TEMPLATE,
                    &*dragonborn::COPPER_DRAGONBORN_TEMPLATE,
                    &*dragonborn::GOLD_DRAGONBORN_TEMPLATE,
                ],
            ),
            (
                "lineage",
                vec![
                    &*tieflings::TIEFLING_TEMPLATE,
                    &*aasimars::AASIMAR_TEMPLATE,
                    &*dwarves::DWARF_TEMPLATE,
                    &*halflings::HALFLING_SCOUT_TEMPLATE,
                    &*half_orcs::HALF_ORC_TEMPLATE,
                    &*gnomes::GNOME_TEMPLATE,
                ],
            ),
            (
                "wizard",
                vec![
                    &*wizards::WIZARD_TEMPLATE,
                    &*wizards::NECROMANCY_WIZARD_TEMPLATE,
                    &*wizards::WAR_MAGIC_WIZARD_TEMPLATE,
                    &*wizards::ABJURATION_WIZARD_TEMPLATE,
                    &*wizards::EVOCATION_WIZARD_TEMPLATE,
                    &*wizards::DIVINATION_WIZARD_TEMPLATE,
                    &*wizards::ENCHANTMENT_WIZARD_TEMPLATE,
                    &*wizards::ILLUSION_WIZARD_TEMPLATE,
                    &*wizards::CONJURATION_WIZARD_TEMPLATE,
                    &*wizards::TRANSMUTATION_WIZARD_TEMPLATE,
                    &*wizards::BLADESINGER_WIZARD_TEMPLATE,
                ],
            ),
    ]
}

#[cfg(test)]
mod tests {
    use crate::engine::types::Skill;

    /// The four skills the engine actually rolls, and the templates that
    /// have to claim them for the lanes reading each one to have any
    /// content behind it.
    ///
    /// This is a data-coverage sweep rather than a behavior test, and it
    /// exists because the failure it guards against is silent. Every one
    /// of these skills feeds a rule — Perception the passive score and
    /// the Search action, Stealth the Hide action and the DC Search
    /// compares against, Athletics and Acrobatics both halves of every
    /// shove, grapple, and escape. A skill nothing claims makes its rule
    /// a coin flip that nobody is ever better at, which reads exactly
    /// like the rule working.
    const ENGINE_READ_SKILLS: &[Skill] = &[
        Skill::Perception,
        Skill::Stealth,
        Skill::Athletics,
        Skill::Acrobatics,
    ];

    #[test]
    fn every_skill_the_engine_rolls_is_claimed_by_some_template() {
        let families = super::pc_template_families();
        for skill in ENGINE_READ_SKILLS {
            let claimed = families
                .iter()
                .flat_map(|(_, templates)| templates.iter())
                .any(|t| t.skills.contains(skill));
            assert!(
                claimed,
                "{:?} is read by the engine but no player template is proficient in it",
                skill
            );
        }
    }

    /// A subclass template clones its family's base wholesale, so the
    /// skill picks land on every build in the family rather than only on
    /// the bare chassis. Pinned because the propagation is implicit —
    /// it rides `..BASE.clone()` in each subclass literal, and a
    /// subclass that spelled its fields out by hand would quietly drop
    /// them.
    #[test]
    fn subclass_templates_inherit_their_familys_skills() {
        for (family, templates) in super::pc_template_families() {
            let Some(base) = templates.first() else {
                continue;
            };
            if base.skills.is_empty() {
                continue;
            }
            for template in &templates[1..] {
                for skill in &base.skills {
                    assert!(
                        template.skills.contains(skill),
                        "{} ({}) dropped {:?} from the family baseline",
                        template.name,
                        family,
                        skill
                    );
                }
            }
        }
    }
}
