use crate::engine::side_effects::ApplicableSideEffect;
use crate::actors::creatures::gray_oozes::GRAY_OOZE_TEMPLATE;
use crate::actors::creatures::ice_devils::ICE_DEVIL_TEMPLATE;
use crate::actors::creatures::planetars::PLANETAR_TEMPLATE;
use crate::actors::creatures::sphinxes_of_wonder::SPHINX_OF_WONDER_TEMPLATE;
use crate::actors::creatures::allosauruses::ALLOSAURUS_TEMPLATE;
use crate::actors::creatures::ankylosauruses::ANKYLOSAURUS_TEMPLATE;
use crate::actors::creatures::apes::APE_TEMPLATE;
use crate::actors::creatures::archelons::ARCHELON_TEMPLATE;
use crate::actors::creatures::axe_beaks::AXE_BEAK_TEMPLATE;
use crate::actors::creatures::baboons::BABOON_TEMPLATE;
use crate::actors::creatures::badgers::BADGER_TEMPLATE;
use crate::actors::creatures::black_bears::BLACK_BEAR_TEMPLATE;
use crate::actors::creatures::blood_hawks::BLOOD_HAWK_TEMPLATE;
use crate::actors::creatures::crabs::CRAB_TEMPLATE;
use crate::actors::creatures::deer::DEER_TEMPLATE;
use crate::actors::creatures::eagles::EAGLE_TEMPLATE;
use crate::actors::creatures::elephants::ELEPHANT_TEMPLATE;
use crate::actors::creatures::flying_snakes::FLYING_SNAKE_TEMPLATE;
use crate::actors::creatures::giant_elks::GIANT_ELK_TEMPLATE;
use crate::actors::creatures::hippopotamuses::HIPPOPOTAMUS_TEMPLATE;
use crate::actors::creatures::octopuses::OCTOPUS_TEMPLATE;
use crate::actors::creatures::owls::OWL_TEMPLATE;
use crate::actors::creatures::piranhas::PIRANHA_TEMPLATE;
use crate::actors::creatures::rhinoceroses::RHINOCEROS_TEMPLATE;
use crate::actors::creatures::scorpions::SCORPION_TEMPLATE;
use crate::actors::creatures::seahorses::{GIANT_SEAHORSE_TEMPLATE, SEAHORSE_TEMPLATE};
use crate::actors::creatures::venomous_snakes::VENOMOUS_SNAKE_TEMPLATE;
use crate::actors::creatures::giant_fire_beetles::GIANT_FIRE_BEETLE_TEMPLATE;
use crate::actors::creatures::giant_weasels::GIANT_WEASEL_TEMPLATE;
use crate::actors::creatures::homunculi::HOMUNCULUS_TEMPLATE;
use crate::actors::creatures::jackals::JACKAL_TEMPLATE;
use crate::actors::creatures::merfolk::MERFOLK_TEMPLATE;
use crate::actors::creatures::ravens::RAVEN_TEMPLATE;
use crate::actors::creatures::remorhazes::REMORHAZ_TEMPLATE;
use crate::actors::creatures::rugs_of_smothering::RUG_OF_SMOTHERING_TEMPLATE;
use crate::actors::creatures::vultures::VULTURE_TEMPLATE;
use crate::actors::creatures::water_weirds::WATER_WEIRD_TEMPLATE;
use crate::actors::creatures::acolytes::ACOLYTE_TEMPLATE;
use crate::actors::creatures::archmages::ARCHMAGE_TEMPLATE;
use crate::actors::creatures::assassins::ASSASSIN_TEMPLATE;
use crate::actors::creatures::azers::AZER_TEMPLATE;
use crate::actors::creatures::barbed_devils::BARBED_DEVIL_TEMPLATE;
use crate::actors::creatures::chain_devils::CHAIN_DEVIL_TEMPLATE;
use crate::actors::creatures::cultists::CULTIST_TEMPLATE;
use crate::actors::creatures::darkmantles::DARKMANTLE_TEMPLATE;
use crate::actors::creatures::duergar::DUERGAR_TEMPLATE;
use crate::actors::creatures::gladiators::GLADIATOR_TEMPLATE;
use crate::actors::creatures::nobles::NOBLE_TEMPLATE;
use crate::actors::creatures::ochre_jellies::OCHRE_JELLY_TEMPLATE;
use crate::actors::creatures::panthers::PANTHER_TEMPLATE;
use crate::actors::creatures::priests::PRIEST_TEMPLATE;
use crate::actors::creatures::satyrs::SATYR_TEMPLATE;
use crate::actors::creatures::shield_guardians::SHIELD_GUARDIAN_TEMPLATE;
use crate::actors::creatures::spies::SPY_TEMPLATE;
use crate::actors::creatures::violet_fungi::VIOLET_FUNGUS_TEMPLATE;
use crate::actors::creatures::warhorse_skeletons::WARHORSE_SKELETON_TEMPLATE;
use crate::actors::creatures::winged_kobolds::WINGED_KOBOLD_TEMPLATE;
use crate::actors::creatures::ankhegs::ANKHEG_TEMPLATE;
use crate::actors::creatures::animated_armors::{ANIMATED_ARMOR_TEMPLATE, FLYING_SWORD_TEMPLATE};
use crate::actors::creatures::bandit_captains::BANDIT_CAPTAIN_TEMPLATE;
use crate::actors::creatures::bandits::BANDIT_TEMPLATE;
use crate::actors::creatures::banshees::BANSHEE_TEMPLATE;
use crate::actors::creatures::basilisks::BASILISK_TEMPLATE;
use crate::actors::creatures::berserkers::BERSERKER_TEMPLATE;
use crate::actors::creatures::bugbears::{BUGBEAR_STALKER_TEMPLATE, BUGBEAR_TEMPLATE};
use crate::actors::creatures::chimeras::CHIMERA_TEMPLATE;
use crate::actors::creatures::chuuls::CHUUL_TEMPLATE;
use crate::actors::creatures::clerics::CLERIC_TEMPLATE;
use crate::actors::creatures::cloakers::CLOAKER_TEMPLATE;
use crate::actors::creatures::cockatrices::COCKATRICE_TEMPLATE;
use crate::actors::creatures::cult_fanatics::CULT_FANATIC_TEMPLATE;
use crate::actors::creatures::dire_wolves::DIRE_WOLF_TEMPLATE;
use crate::actors::creatures::displacer_beasts::DISPLACER_BEAST_TEMPLATE;
use crate::actors::creatures::doppelgangers::DOPPELGANGER_TEMPLATE;
use crate::actors::creatures::ettins::ETTIN_TEMPLATE;
use crate::actors::creatures::fire_elementals::FIRE_ELEMENTAL_TEMPLATE;
use crate::actors::creatures::gargoyles::GARGOYLE_TEMPLATE;
use crate::actors::creatures::gelatinous_cubes::GELATINOUS_CUBE_TEMPLATE;
use crate::actors::creatures::ghouls::GHOUL_TEMPLATE;
use crate::actors::creatures::ghosts::GHOST_TEMPLATE;
use crate::actors::creatures::giant_scorpions::GIANT_SCORPION_TEMPLATE;
use crate::actors::creatures::gnolls::GNOLL_TEMPLATE;
use crate::actors::creatures::gricks::GRICK_TEMPLATE;
use crate::actors::creatures::goblin_bosses::GOBLIN_BOSS_TEMPLATE;
use crate::actors::creatures::goblins::{GOBLIN_MINION_TEMPLATE, GOBLIN_TEMPLATE};
use crate::actors::creatures::harpies::HARPY_TEMPLATE;
use crate::actors::creatures::hell_hounds::HELL_HOUND_TEMPLATE;
use crate::actors::creatures::hill_giants::HILL_GIANT_TEMPLATE;
use crate::actors::creatures::hippogriffs::HIPPOGRIFF_TEMPLATE;
use crate::actors::creatures::hobgoblin_warlords::HOBGOBLIN_WARLORD_TEMPLATE;
use crate::actors::creatures::hobgoblins::{HOBGOBLIN_CAPTAIN_TEMPLATE, HOBGOBLIN_TEMPLATE};
use crate::actors::creatures::hydras::HYDRA_TEMPLATE;
use crate::actors::creatures::kobolds::KOBOLD_TEMPLATE;
use crate::actors::creatures::knights::KNIGHT_TEMPLATE;
use crate::actors::creatures::mages::MAGE_TEMPLATE;
use crate::actors::creatures::manticores::MANTICORE_TEMPLATE;
use crate::actors::creatures::medusas::MEDUSA_TEMPLATE;
use crate::actors::creatures::mimics::MIMIC_TEMPLATE;
use crate::actors::creatures::mind_flayers::MIND_FLAYER_TEMPLATE;
use crate::actors::creatures::minotaurs::MINOTAUR_TEMPLATE;
use crate::actors::creatures::mummies::MUMMY_TEMPLATE;
use crate::actors::creatures::nightmares::NIGHTMARE_TEMPLATE;
use crate::actors::creatures::nothics::NOTHIC_TEMPLATE;
use crate::actors::creatures::ogres::OGRE_TEMPLATE;
use crate::actors::creatures::orcs::ORC_TEMPLATE;
use crate::actors::creatures::owlbears::OWLBEAR_TEMPLATE;
use crate::actors::creatures::phase_spiders::PHASE_SPIDER_TEMPLATE;
use crate::actors::creatures::salamanders::SALAMANDER_TEMPLATE;
use crate::actors::creatures::ropers::ROPER_TEMPLATE;
use crate::actors::creatures::rust_monsters::RUST_MONSTER_TEMPLATE;
use crate::actors::creatures::shadows::SHADOW_TEMPLATE;
use crate::actors::creatures::shambling_mounds::SHAMBLING_MOUND_TEMPLATE;
use crate::actors::creatures::specters::SPECTER_TEMPLATE;
use crate::actors::creatures::giant_spiders::GIANT_SPIDER_TEMPLATE;
use crate::actors::creatures::stirges::STIRGE_TEMPLATE;
use crate::actors::creatures::stone_giants::STONE_GIANT_TEMPLATE;
use crate::actors::creatures::storm_giants::STORM_GIANT_TEMPLATE;
use crate::actors::creatures::treants::TREANT_TEMPLATE;
use crate::actors::creatures::trolls::{TROLL_LIMB_TEMPLATE, TROLL_TEMPLATE};
use crate::actors::creatures::umber_hulks::UMBER_HULK_TEMPLATE;
use crate::actors::creatures::vampire_spawns::VAMPIRE_SPAWN_TEMPLATE;
use crate::actors::creatures::veterans::{VETERAN_TEMPLATE, WARRIOR_INFANTRY_TEMPLATE};
use crate::actors::creatures::vrocks::VROCK_TEMPLATE;
use crate::actors::creatures::werewolves::WEREWOLF_TEMPLATE;
use crate::actors::creatures::wights::WIGHT_TEMPLATE;
use crate::actors::creatures::wisps::WISP_TEMPLATE;
use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
use crate::actors::creatures::yetis::YETI_TEMPLATE;
use crate::actors::creatures::wolves::WOLF_TEMPLATE;
use crate::actors::creatures::worgs::WORG_TEMPLATE;
use crate::actors::creatures::wyverns::WYVERN_TEMPLATE;
use crate::actors::creatures::drow::DROW_TEMPLATE;
use crate::actors::creatures::gnoll_pack_lords::GNOLL_PACK_LORD_TEMPLATE;
use crate::actors::creatures::bone_devils::BONE_DEVIL_TEMPLATE;
use crate::actors::creatures::erinyes::ERINYES_TEMPLATE;
use crate::actors::creatures::frost_giants::FROST_GIANT_TEMPLATE;
use crate::actors::creatures::earth_elementals::EARTH_ELEMENTAL_TEMPLATE;
use crate::actors::creatures::air_elementals::AIR_ELEMENTAL_TEMPLATE;
use crate::actors::creatures::bulettes::BULETTE_TEMPLATE;
use crate::actors::creatures::couatls::COUATL_TEMPLATE;
use crate::actors::creatures::fire_imps::FIRE_IMP_TEMPLATE;
use crate::actors::creatures::flameskulls::FLAMESKULL_TEMPLATE;
use crate::actors::creatures::imps::IMP_TEMPLATE;
use crate::actors::creatures::skeletons::{MINOTAUR_SKELETON_TEMPLATE, SKELETON_TEMPLATE};
use crate::actors::creatures::slimes::SLIME_TEMPLATE;
use crate::actors::creatures::spectators::SPECTATOR_TEMPLATE;
use crate::actors::creatures::wraiths::WRAITH_TEMPLATE;
use crate::actors::creatures::vampires::{VAMPIRE_FAMILIAR_TEMPLATE, VAMPIRE_TEMPLATE};
use crate::actors::creatures::zombies::{OGRE_ZOMBIE_TEMPLATE, ZOMBIE_TEMPLATE};
use crate::actors::creatures::giant_apes::GIANT_APE_TEMPLATE;
use crate::actors::creatures::giant_eagles::GIANT_EAGLE_TEMPLATE;
use crate::actors::creatures::lizardfolk::LIZARDFOLK_TEMPLATE;
use crate::actors::creatures::sahuagins::SAHUAGIN_TEMPLATE;
use crate::actors::creatures::centaurs::CENTAUR_TEMPLATE;
use crate::actors::creatures::behirs::BEHIR_TEMPLATE;
use crate::actors::creatures::boars::BOAR_TEMPLATE;
use crate::actors::creatures::brown_bears::BROWN_BEAR_TEMPLATE;
use crate::actors::creatures::giant_toads::GIANT_TOAD_TEMPLATE;
use crate::actors::creatures::pseudodragons::PSEUDODRAGON_TEMPLATE;
use crate::actors::creatures::tigers::TIGER_TEMPLATE;
use crate::actors::creatures::polar_bears::POLAR_BEAR_TEMPLATE;
use crate::actors::creatures::lions::LION_TEMPLATE;
use crate::actors::creatures::fire_giants::FIRE_GIANT_TEMPLATE;
use crate::actors::creatures::cyclopes::CYCLOPS_TEMPLATE;
use crate::actors::creatures::rocs::ROC_TEMPLATE;
use crate::actors::creatures::pegasi::PEGASUS_TEMPLATE;
use crate::actors::creatures::winter_wolves::WINTER_WOLF_TEMPLATE;
use crate::actors::creatures::triceratopses::TRICERATOPS_TEMPLATE;
use crate::actors::creatures::tyrannosauruses::T_REX_TEMPLATE;
use crate::actors::creatures::carrion_crawlers::CARRION_CRAWLER_TEMPLATE;
use crate::actors::creatures::water_elementals::WATER_ELEMENTAL_TEMPLATE;
use crate::actors::creatures::saber_toothed_tigers::SABER_TOOTHED_TIGER_TEMPLATE;
use crate::actors::creatures::hyenas::HYENA_TEMPLATE;
use crate::actors::creatures::giant_hyenas::GIANT_HYENA_TEMPLATE;
use crate::actors::creatures::green_hags::GREEN_HAG_TEMPLATE;
use crate::actors::creatures::gorgons::GORGON_TEMPLATE;
use crate::actors::creatures::yuan_ti::YUAN_TI_MALISON_TEMPLATE;
use crate::actors::creatures::cambions::CAMBION_TEMPLATE;
use crate::actors::creatures::dryads::DRYAD_TEMPLATE;
use crate::actors::creatures::bullywugs::BULLYWUG_TEMPLATE;
use crate::actors::creatures::quasits::QUASIT_TEMPLATE;
use crate::actors::creatures::shadow_demons::SHADOW_DEMON_TEMPLATE;
use crate::actors::creatures::incubi::INCUBUS_TEMPLATE;
use crate::actors::creatures::succubi::SUCCUBUS_TEMPLATE;
use crate::actors::creatures::intellect_devourers::INTELLECT_DEVOURER_TEMPLATE;
use crate::actors::creatures::xorns::XORN_TEMPLATE;
use crate::actors::creatures::oni::ONI_TEMPLATE;
use crate::actors::creatures::merrow::MERROW_TEMPLATE;
use crate::actors::creatures::giant_crabs::GIANT_CRAB_TEMPLATE;
use crate::actors::creatures::cloud_giants::CLOUD_GIANT_TEMPLATE;
use crate::actors::creatures::hezrous::HEZROU_TEMPLATE;
use crate::actors::creatures::gibbering_mouthers::GIBBERING_MOUTHER_TEMPLATE;
use crate::actors::creatures::iron_golems::IRON_GOLEM_TEMPLATE;
use crate::actors::creatures::mummy_lords::MUMMY_LORD_TEMPLATE;
use crate::actors::creatures::rakshasas::RAKSHASA_TEMPLATE;
use crate::actors::creatures::hook_horrors::HOOK_HORROR_TEMPLATE;
use crate::actors::creatures::dragon_turtles::DRAGON_TURTLE_TEMPLATE;
use crate::actors::creatures::krakens::KRAKEN_TEMPLATE;
use crate::actors::creatures::helmed_horrors::HELMED_HORROR_TEMPLATE;
use crate::actors::creatures::pixies::PIXIE_TEMPLATE;
use crate::actors::creatures::androsphinxes::ANDROSPHINX_TEMPLATE;
use crate::actors::creatures::unicorns::UNICORN_TEMPLATE;
use crate::actors::creatures::driders::DRIDER_TEMPLATE;
use crate::actors::creatures::sea_hags::SEA_HAG_TEMPLATE;
use crate::actors::creatures::night_hags::NIGHT_HAG_TEMPLATE;
use crate::actors::creatures::spirit_nagas::SPIRIT_NAGA_TEMPLATE;
use crate::actors::creatures::otyughs::OTYUGH_TEMPLATE;
use crate::actors::creatures::sprites::SPRITE_TEMPLATE;
use crate::actors::creatures::death_dogs::DEATH_DOG_TEMPLATE;
use crate::actors::creatures::griffons::GRIFFON_TEMPLATE;
use crate::actors::creatures::lamias::LAMIA_TEMPLATE;
use crate::actors::creatures::werebears::WEREBEAR_TEMPLATE;
use crate::actors::creatures::wereboars::WEREBOAR_TEMPLATE;
use crate::actors::creatures::wererats::WERERAT_TEMPLATE;
use crate::actors::creatures::weretigers::WERETIGER_TEMPLATE;
use crate::actors::creatures::ettercaps::ETTERCAP_TEMPLATE;
use crate::actors::creatures::magmins::MAGMIN_TEMPLATE;
use crate::actors::creatures::galeb_duhrs::GALEB_DUHR_TEMPLATE;
use crate::actors::creatures::mephits::{
    DUST_MEPHIT_TEMPLATE, ICE_MEPHIT_TEMPLATE, MAGMA_MEPHIT_TEMPLATE, STEAM_MEPHIT_TEMPLATE,
};
use crate::actors::creatures::awakened_trees::AWAKENED_TREE_TEMPLATE;
use crate::actors::creatures::dretches::DRETCH_TEMPLATE;
use crate::actors::creatures::lemures::LEMURE_TEMPLATE;
use crate::actors::creatures::bearded_devils::BEARDED_DEVIL_TEMPLATE;
use crate::actors::creatures::blink_dogs::BLINK_DOG_TEMPLATE;
use crate::actors::creatures::black_puddings::BLACK_PUDDING_TEMPLATE;
use crate::actors::creatures::flesh_golems::FLESH_GOLEM_TEMPLATE;
use crate::actors::creatures::horned_devils::HORNED_DEVIL_TEMPLATE;
use crate::actors::creatures::nalfeshnees::NALFESHNEE_TEMPLATE;
use crate::actors::creatures::constrictor_snakes::{
    CONSTRICTOR_SNAKE_TEMPLATE, GIANT_CONSTRICTOR_SNAKE_TEMPLATE,
};
use crate::actors::creatures::djinn::DJINNI_TEMPLATE;
use crate::actors::creatures::efreeti::EFREETI_TEMPLATE;
use crate::actors::creatures::marids::MARID_TEMPLATE;
use crate::actors::creatures::crocodiles::{CROCODILE_TEMPLATE, GIANT_CROCODILE_TEMPLATE};
use crate::actors::creatures::daos::DAO_TEMPLATE;
use crate::actors::creatures::invisible_stalkers::INVISIBLE_STALKER_TEMPLATE;
use crate::actors::creatures::mammoths::MAMMOTH_TEMPLATE;
use crate::actors::creatures::purple_worms::PURPLE_WORM_TEMPLATE;
use crate::actors::creatures::giant_octopuses::GIANT_OCTOPUS_TEMPLATE;
use crate::actors::creatures::plesiosauruses::PLESIOSAURUS_TEMPLATE;
use crate::actors::creatures::pteranodons::PTERANODON_TEMPLATE;
use crate::actors::creatures::guardian_nagas::GUARDIAN_NAGA_TEMPLATE;
use crate::actors::creatures::pirates::{PIRATE_CAPTAIN_TEMPLATE, PIRATE_TEMPLATE};
use crate::actors::creatures::sphinxes_of_lore::SPHINX_OF_LORE_TEMPLATE;
use crate::actors::creatures::thugs::{THUG_TEMPLATE, TOUGH_BOSS_TEMPLATE};
use crate::actors::creatures::tribal_warriors::TRIBAL_WARRIOR_TEMPLATE;
use crate::actors::creatures::scouts::SCOUT_TEMPLATE;
use crate::actors::creatures::giant_rats::GIANT_RAT_TEMPLATE;
use crate::actors::creatures::ghasts::GHAST_TEMPLATE;
use crate::actors::creatures::commoners::COMMONER_TEMPLATE;
use crate::actors::creatures::mastiffs::MASTIFF_TEMPLATE;
use crate::actors::creatures::guards::{GUARD_CAPTAIN_TEMPLATE, GUARD_TEMPLATE};
use crate::actors::creatures::grimlocks::GRIMLOCK_TEMPLATE;
use crate::actors::creatures::giant_frogs::GIANT_FROG_TEMPLATE;
use crate::actors::creatures::hawks::HAWK_TEMPLATE;
use crate::actors::creatures::giant_lizards::GIANT_LIZARD_TEMPLATE;
use crate::actors::creatures::giant_wolf_spiders::GIANT_WOLF_SPIDER_TEMPLATE;
use crate::actors::creatures::reef_sharks::REEF_SHARK_TEMPLATE;
use crate::actors::creatures::hunter_sharks::HUNTER_SHARK_TEMPLATE;
use crate::actors::creatures::giant_sharks::GIANT_SHARK_TEMPLATE;
use crate::actors::creatures::warhorses::WARHORSE_TEMPLATE;
use crate::actors::creatures::allips::ALLIP_TEMPLATE;
use crate::actors::creatures::quaggoths::QUAGGOTH_TEMPLATE;
use crate::actors::creatures::giant_vultures::GIANT_VULTURE_TEMPLATE;
use crate::actors::creatures::giant_bats::GIANT_BAT_TEMPLATE;
use crate::actors::creatures::giant_centipedes::GIANT_CENTIPEDE_TEMPLATE;
use crate::actors::creatures::vine_blights::VINE_BLIGHT_TEMPLATE;
use crate::actors::creatures::twig_blights::TWIG_BLIGHT_TEMPLATE;
use crate::actors::creatures::needle_blights::NEEDLE_BLIGHT_TEMPLATE;
use crate::actors::creatures::giant_boars::GIANT_BOAR_TEMPLATE;
use crate::actors::creatures::giant_goats::GIANT_GOAT_TEMPLATE;
use crate::actors::creatures::giant_owls::GIANT_OWL_TEMPLATE;
use crate::actors::creatures::giant_venomous_snakes::GIANT_VENOMOUS_SNAKE_TEMPLATE;
use crate::actors::creatures::killer_whales::KILLER_WHALE_TEMPLATE;
use crate::actors::creatures::crawling_claws::CRAWLING_CLAW_TEMPLATE;
use crate::actors::creatures::riding_horses::RIDING_HORSE_TEMPLATE;
use crate::actors::creatures::draft_horses::DRAFT_HORSE_TEMPLATE;
use crate::actors::creatures::awakened_shrubs::AWAKENED_SHRUB_TEMPLATE;
use crate::actors::creatures::bats::BAT_TEMPLATE;
use crate::actors::creatures::camels::CAMEL_TEMPLATE;
use crate::actors::creatures::cats::CAT_TEMPLATE;
use crate::actors::creatures::frogs::FROG_TEMPLATE;
use crate::actors::creatures::giant_badgers::GIANT_BADGER_TEMPLATE;
use crate::actors::creatures::giant_wasps::GIANT_WASP_TEMPLATE;
use crate::actors::creatures::lizards::LIZARD_TEMPLATE;
use crate::actors::creatures::rats::RAT_TEMPLATE;
use crate::actors::creatures::weasels::WEASEL_TEMPLATE;
use crate::actors::creatures::goats::GOAT_TEMPLATE;
use crate::actors::creatures::mules::MULE_TEMPLATE;
use crate::actors::creatures::ponies::PONY_TEMPLATE;
use crate::actors::creatures::elks::ELK_TEMPLATE;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;

use crate::actions::action_template::ActionExecutionInfo;
use crate::actors::actor_template::{ActorInstance, CreatureTemplate, DeathSaveOutcome};
use crate::conditions::Condition;
use crate::engine::actor_gen::{ActorGenParams, generate_actors};
use crate::engine::errors::{NoLegalPosition, OffMapCoord};
use crate::engine::prompt::Prompt;
use crate::engine::terrain::{TerrainInfo, TerrainType};
use crate::engine::underwater::{AttackInWater, UnderwaterVerdict};
use crate::engine::terrain_gen::{TerrainGenParams, generate_terrain};
use crate::engine::conjured_terrain::ConjuredTerrain;
use crate::engine::lighting::{AmbientLight, LightAnchor, LightLevel, LightSource};
use crate::engine::zones::Zone;
use crate::engine::triggers::TriggerEvent;
use crate::engine::types::{AbilityScoreType, Coordinate, DamageType, Size, SpellSchool};
use crate::engine::util::{TILE_FEET, footprint_chebyshev, get_tiles_from_size};
use fastrand::Rng;
use std::cmp::Ordering;
use crate::engine::dice::{Dice, FastRandRoller, RollMode, RollModeTally, Roller};

/// Single entry in the round-end repeated-save table. 5e spells like
/// Hold Person / Hold Monster allow the target to repeat the saving throw
/// at the end of each of their turns, ending the condition on a success.
/// The engine iterates `ROUND_END_SAVES` once per actor at round-end;
/// for each entry whose `condition` is set on that actor, a save is
/// rolled vs the original caster's spell save DC. On a pass, the
/// condition is removed and the caster's concentration (if anchored to
/// the same condition) is dropped.
struct RoundEndSave {
    condition: Condition,
    save_ability: crate::engine::types::AbilityScoreType,
    log_verb: &'static str,
}

const ROUND_END_SAVES: &[RoundEndSave] = &[
    // 5e Hold Person / Hold Monster — WIS save at end of each turn.
    // The spells apply Stunned (we model Hold as Stunned + concentration);
    // on a successful save the target breaks free.
    RoundEndSave {
        condition: Condition::Stunned,
        save_ability: crate::engine::types::AbilityScoreType::Wisdom,
        log_verb: "strains against the hold:",
    },
    // 5e Tasha's Hideous Laughter — WIS save at end of each turn.
    // Modeled as Incapacitated + Prone; the WIS save lets the target
    // break free early.
    RoundEndSave {
        condition: Condition::Incapacitated,
        save_ability: crate::engine::types::AbilityScoreType::Wisdom,
        log_verb: "tries to stop laughing:",
    },
    // 5e Flesh to Stone — CON save at end of each turn. RAW: the target
    // gets 3 saves before becoming permanently stone; we collapse the
    // ladder to a single break-free save (matches the Hold Person /
    // Hideous Laughter shape). Concentration-anchored, so the
    // `find_concentration_owner` lookup skips non-spell petrification
    // (Medusa Gaze, Cockatrice Bite, Basilisk) — those rely on their
    // own short Rounds timers to expire.
    RoundEndSave {
        condition: Condition::Petrified,
        save_ability: crate::engine::types::AbilityScoreType::Constitution,
        log_verb: "strains against the stone curse:",
    },
    // 5e Immolation — DEX save at end of each turn to extinguish the
    // flames. Concentration-anchored on the caster (so non-spell fire
    // sources like environmental hazards don't accidentally piggyback on
    // this save), matching the Hold Person / Flesh to Stone pattern.
    RoundEndSave {
        condition: Condition::Immolated,
        save_ability: crate::engine::types::AbilityScoreType::Dexterity,
        log_verb: "tries to put out the flames:",
    },
    // 5e Power Word: Pain (XGtE level-7 necromancy) — CON save at end of
    // each turn to shake off the pain. Concentration-anchored on the
    // caster (the pain rides `PowerWordPained` which the shared
    // `find_concentration_owner` lookup skips for non-spell installs).
    // Sibling to Hold Person's WIS-vs-Stunned save on the "target
    // repeatedly saves to break free" corner — same save-then-clear-
    // and-drop-concentration semantics, distinct save axis (CON vs
    // WIS) and distinct condition (PowerWordPained vs Stunned).
    RoundEndSave {
        condition: Condition::PowerWordPained,
        save_ability: crate::engine::types::AbilityScoreType::Constitution,
        log_verb: "strains against the racking pain:",
    },
    // 5e Enervation (XGtE level-5 necromancy) — DEX at the end of each
    // of the victim's turns to tear the tendril loose. RAW spends the
    // victim's *action* on a Dexterity check; folded onto this table
    // instead so it costs them nothing to try — see
    // `Condition::Enervated` for why.
    RoundEndSave {
        condition: Condition::Enervated,
        save_ability: crate::engine::types::AbilityScoreType::Dexterity,
        log_verb: "tears at the draining tendril:",
    },
];

/// Single entry in the round-end damage-over-time table. The engine
/// iterates `ROUND_END_DOTS` once per actor at round-end and rolls each
/// entry whose `condition` is set on that actor. Adding a new DoT
/// condition (e.g. Earthen Grasp's 2d6 bludgeoning drip, Vitriolic
/// Sphere's residual acid) is a one-line table entry rather than a
/// hand-rolled if-block inside `round_end`.
///
/// `log_verb` is the action verb in the log line — chosen per-effect so
/// the message reads naturally ("burns" for Burning, "crushes against"
/// for Earthen Grasp, etc.). Damage rolls through the standard
/// `DealDamage` pipeline so resistance / immunity / temp HP / death
/// saves all apply uniformly.
struct RoundEndDot {
    condition: Condition,
    dice: Dice,
    damage_type: DamageType,
    log_verb: &'static str,
    /// 5e Enervation: "you regain hit points equal to half the amount
    /// of necrotic damage dealt."
    ///
    /// The drip is a *tether* rather than a burn: what it takes from
    /// one end it gives to the other. The other end is found the same
    /// way every concentration-bound entry in this table already finds
    /// it — `find_concentration_owner`, keyed on the exact
    /// `(victim, condition)` pair the install anchored — so a drip with
    /// no concentration behind it (a monster ability, an expired
    /// anchor) simply drains into nobody rather than healing the wrong
    /// creature.
    ///
    /// Half of the damage the victim *took*, not half of the dice:
    /// resistance, immunity and temp HP all sit between the roll and
    /// the wound, and RAW's "damage dealt" is the number on the far
    /// side of them. A tether on a creature immune to necrotic heals
    /// its holder for nothing, which is the right answer and not the
    /// one a dice-half would give.
    drains_to_owner: bool,
}

/// Round-end DoT registry. Order is the order damage rolls each round
/// — deterministic for log replay across seeded runs. New
/// concentration-bound or timer-bound DoTs slot in here as one entry.
const ROUND_END_DOTS: &[RoundEndDot] = &[
    // 5e Burning (Searing Smite ignition, Fire Bolt / Fireball /
    // Flaming Sphere riders). 1d4 fire per round, Rounds-timer clears
    // the flag naturally.
    RoundEndDot {
        condition: Condition::Burning,
        dice: Dice::new(1, 4),
        damage_type: DamageType::Fire,
        drains_to_owner: false,
        log_verb: "burns:",
    },
    // 5e Heat Metal — concentration-bound. 2d8 fire per round; the
    // condition is anchored to the caster's concentration so dropping
    // concentration removes the flag and ends the drip.
    RoundEndDot {
        condition: Condition::HeatMetaled,
        dice: Dice::new(2, 8),
        damage_type: DamageType::Fire,
        drains_to_owner: false,
        log_verb: "'s gear sears:",
    },
    // 5e Maximilian's Earthen Grasp — concentration-bound. 2d6
    // bludgeoning per round as the earthen fist crushes the grasped
    // target. Dropping concentration clears the EarthenGrasped flag and
    // ends the crush.
    RoundEndDot {
        condition: Condition::EarthenGrasped,
        dice: Dice::new(2, 6),
        damage_type: DamageType::Bludgeoning,
        drains_to_owner: false,
        log_verb: "is crushed by the earthen grasp:",
    },
    // 5e Vitriolic Sphere — one-shot residual drip the spell leaves on
    // failed-save targets. 5d4 acid at the next round-end then the
    // condition's `Rounds(1)` timer expires it.
    RoundEndDot {
        condition: Condition::VitriolicAcidCoated,
        dice: Dice::new(5, 4),
        damage_type: DamageType::Acid,
        drains_to_owner: false,
        log_verb: "drips with vitriolic acid:",
    },
    // 5e Witch Bolt — concentration-bound. 1d12 lightning per round as
    // the tether crackles. Dropping concentration severs the bolt.
    RoundEndDot {
        condition: Condition::WitchBolted,
        dice: Dice::new(1, 12),
        damage_type: DamageType::Lightning,
        drains_to_owner: false,
        log_verb: "is shocked by witch bolt:",
    },
    // 5e Tasha's Caustic Brew — concentration-bound. 2d4 acid per round
    // as the clinging acid eats at the target. Dropping concentration
    // ends the drip; the target can also wipe it off with an Action
    // (via `WipeAcid`). The `Rounds(10)` timer caps the duration at ~1
    // minute RAW so the drip eventually expires even without cleanse.
    RoundEndDot {
        condition: Condition::CausticBrewed,
        dice: Dice::new(2, 4),
        damage_type: DamageType::Acid,
        drains_to_owner: false,
        log_verb: "is eaten by caustic brew:",
    },
    // 5e Phantasmal Force — concentration-bound. 1d6 psychic per round
    // as the target's mind invents wounds from the illusion. Dropping
    // concentration dispels the illusion and ends the drip cleanly.
    RoundEndDot {
        condition: Condition::PhantasmalForced,
        dice: Dice::new(1, 6),
        damage_type: DamageType::Psychic,
        drains_to_owner: false,
        log_verb: "is wounded by the phantasm:",
    },
    // 5e Immolation — concentration-bound, lv5. 4d6 fire per round as
    // the target burns. RAW: the target can end the spell early with a
    // DEX save at the end of each of its turns (wired through the
    // matching `ROUND_END_SAVES` entry). Dropping concentration also
    // extinguishes the flame.
    RoundEndDot {
        condition: Condition::Immolated,
        dice: Dice::new(4, 6),
        damage_type: DamageType::Fire,
        drains_to_owner: false,
        log_verb: "burns from immolation:",
    },
    // 5e Enervation — concentration-bound, lv5. 4d8 necrotic a round
    // from the tendril, half of it fed back to the caster. The only
    // entry in the table that gives anything back; see
    // `RoundEndDot::drains_to_owner`. RAW's break-free clause is wired
    // through the matching `ROUND_END_SAVES` entry.
    RoundEndDot {
        condition: Condition::Enervated,
        dice: Dice::new(4, 8),
        damage_type: DamageType::Necrotic,
        drains_to_owner: true,
        log_verb: "is drained by the tendril:",
    },
];

/// Conditions consumed by `clear_attack_advantage_riders` when the
/// attacker makes any attack roll. Centralizes the one-shot
/// attack-buff cohort — Hidden / Helped (drop on attack), Inspired
/// (Bardic die fires once), PrecisionAttacking (Battle Master prime
/// fires once). New conditions whose entire effect is a single-shot
/// attack-roll bonus (a flat +N in `condition_attack_bonus` or an
/// advantage flip in `compute_attack_mode`) add an entry here so the
/// engine consumes them uniformly on the next swing.
const CONSUMED_ON_ATTACK: &[Condition] = &[
    Condition::Helped,
    Condition::Hidden,
    Condition::Inspired,
    Condition::PrecisionAttacking,
    // 5e Battle Master Lunging Attack — the prime extends reach for the
    // *next* swing. RAW gates the bonus to a melee weapon attack; we
    // accept the minor "ranged swing wastes the prime" deviation in
    // exchange for uniformity with the other one-shot primes here.
    Condition::LungingAttacking,
    // 5e Wild Magic Sorcerer **Tides of Chaos** — the prime grants
    // advantage on the next attack roll, ability check, or saving
    // throw. We honor the attack-roll lane via `grants_self_attack_advantage`
    // and consume here so a single swing burns the once-per-long-rest
    // charge (matches the Inspired / PrecisionAttacking one-shot shape).
    // The ability-check / saving-throw lanes are out of scope — most of
    // the tactical leverage in our combat model is on the attack roll.
    Condition::TidesOfChaos,
    // 5e War Domain Cleric **Guided Strike** — Channel Divinity prime
    // (lv2 subclass): +10 to the next attack roll. Consumed here so a
    // single swing burns the once-per-short-rest charge (matches
    // Inspired / PrecisionAttacking / TidesOfChaos). The flat +10 lives
    // in `condition_attack_bonus`; the one-shot lifecycle lives here.
    Condition::GuidedStriking,
    // 5e Way of Shadow Monk **Shadow Step** — the teleport grants
    // advantage on the first melee attack made before the end of the
    // turn (via `grants_self_melee_attack_advantage`, the melee-gated
    // sibling of the cohort every other entry here rides). Consumed on
    // the first swing regardless of lane: this table has no `is_melee`
    // to read, so a ranged swing burns the prime without collecting it,
    // the same deviation `LungingAttacking` above already accepts —
    // and a near-moot one on a chassis whose kit is unarmed strikes.
    Condition::Shadowstepping,
    // 5e Sap weapon mastery — the only entry here that is a *penalty*
    // rather than a prime. It rides the cohort for the same reason the
    // others do: its whole effect is one attack roll's worth of mode,
    // and the swing that eats the disadvantage is the swing that should
    // clear it.
    Condition::Sapped,
];

/// Conditions the *target* of an attack loses the moment the attacker
/// who put them there swings again — the counterparty-scoped sibling of
/// `CONSUMED_ON_ATTACK`.
///
/// Every entry is back-linked (see `LINKED_CONDITIONS`), and the link is
/// what the clear is keyed on: a `Vexed` creature stays vexed for
/// everybody else on the board when the vexer takes their shot, because
/// the advantage the flag hands out was never theirs. A flag with no
/// link and a blanket clear would have spent one attacker's setup on
/// another attacker's swing.
const CONSUMED_BY_LINKED_ATTACKER: &[Condition] = &[
    // 5e Vex weapon mastery: "advantage on your next attack roll
    // against that creature". Spent by that next roll, hit or miss —
    // RAW's grant is on the roll, not on the outcome.
    Condition::Vexed,
];

/// Conditions consumed at the saving-throw site the moment their holder
/// rolls one — the save-roll sibling of `CONSUMED_ON_ATTACK`.
///
/// This existed as a hand-written `let inspired_used = ...` capture and
/// a matching `remove_condition(Inspired)` twenty lines further down,
/// which was fine while `Inspired` was the only one-shot rider a save
/// could spend. The Eloquence Bard's Unsettling Words is the second,
/// and it sits on exactly the same three pieces — a flat magnitude on
/// `CONDITION_SAVE_BONUSES`, a short timer, and a single save's worth
/// of life — so the capture and the clear are a cohort walk now rather
/// than two more inline branches.
///
/// Both entries are read into the save total by `condition_save_bonus`
/// *before* this cohort clears them, so the roll being paid for still
/// gets the number; the clear only stops the next roll from collecting
/// it again. A new condition whose whole effect is a single-shot save
/// delta lands as one row here.
const CONSUMED_ON_SAVE: &[Condition] = &[
    // 5e Bardic Inspiration (+3): RAW spends the die on one "ability
    // check, attack roll, or saving throw", so the save site has to
    // burn it for the same reason `CONSUMED_ON_ATTACK` does — without
    // both, one die pays for a save *and* the swing that follows it.
    Condition::Inspired,
    // 5e College of Eloquence Bard **Unsettling Words** (−4): the
    // inverse die, spent by the creature it was aimed at rather than
    // by its holder's ally.
    Condition::Unsettled,
];

/// One row in the `CASTER_SAVE_MODE_RIDERS` cohort — a single
/// caster-attributed rider that bends the mode of a saving throw the
/// *target* is about to roll against the *caster's* spell.
///
/// The three closures split the row into the three things every such
/// rider has to say, and nothing else:
///   - `applies`: does this rider fire for this (caster, target) pair?
///     Reads either side — Heightened Spell keys off a caster-side
///     prime, Eldritch Strike off a target-side mark plus its
///     back-link, Magical Ambush off the caster's concealment.
///   - `consume`: burn the one-shot prime. Every current row is
///     once-per-trigger, so a multi-target cast bends only its first
///     save; a hypothetical persistent rider would land here as a
///     no-op closure without widening the row shape.
///   - `mode` + `label`: the notch to fold in and the log tag.
///
/// Sibling in spirit to `CONSUMED_ON_ATTACK` on the "one-shot prime,
/// consumed at the roll site" lane — that cohort covers the attack-roll
/// axis with a bare condition list because every entry there is a plain
/// caster-side self-buff; this one covers the save-roll axis and needs
/// the predicate pair because its rows read *both* sides of the roll.
/// Psychic damage the Conquest Paladin's Aura of Conquest deals to a
/// Frightened enemy that starts its turn inside it. RAW is half the
/// paladin's level; the paladin chassis in this engine ships its
/// subclass features on a level-10-equivalent build, so 5.
///
/// A flat constant rather than a read off `ActorInstance::level()` for
/// the reason every other level-scaled number on the PC templates is
/// one: instantiated actors are all level 1 in this engine, so reading
/// the field would silently collapse the aura to 0.
const AURA_OF_CONQUEST_PSYCHIC: u32 = 5;

/// How far below their rolled initiative a Thief's Reflexes extra turn
/// sits. RAW is a flat 10, and the flatness is the point — it is a fixed
/// distance down a d20-scale order, so on a typical table the extra turn
/// lands after roughly half the room rather than immediately behind the
/// Thief's own.
const THIEFS_REFLEXES_INITIATIVE_PENALTY: i32 = 10;

/// Which side of the emitter's team an aura projects onto. Every
/// paladin aura but one helps the emitter's allies; Oath of Conquest's
/// hurts their enemies. Read by `EncounterInstance::aura_emitters`,
/// which is otherwise identical for both.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AuraSide {
    Allied,
    Hostile,
}

pub struct CasterSaveModeRider {
    /// Gate: does the rider fire for `(caster_id, target_id)`?
    pub applies: fn(&EncounterInstance, usize, usize) -> bool,
    /// Burn the prime that `applies` just matched on.
    pub consume: fn(&mut EncounterInstance, usize, usize),
    /// Notch folded into the save's mode via `RollMode::combine`.
    pub mode: RollMode,
    /// Log-friendly tag ("heightened spell", "eldritch strike", ...).
    pub label: &'static str,
}

/// Caster-attributed save-mode riders, walked by
/// `roll_save_against_caster` before the save is rolled. Every row that
/// fires combines its `mode` into the roll and consumes its prime;
/// `RollMode::combine` keeps two disadvantage rows at a single notch,
/// matching 5e's no-stacking rule.
///
/// "Consumes its prime" is the shape of most rows rather than a rule of
/// the table: a *standing* ward — one that applies to every save
/// against every spell for as long as it is up — belongs here too, and
/// says so with a `consume` that does nothing. Circle of Power is the
/// first of those.
///
/// What makes this the right table for a target-side ward, rather than
/// `BLANKET_SAVE_ADVANTAGE_CONDITIONS` one lane over, is that the rows
/// here know a spell is what is being saved against. RAW's wards are
/// almost always scoped that way ("against spells and other magical
/// effects"), and the blanket table would also lift a save against a
/// dragon's landing.
///
/// Adding a future "the target has advantage/disadvantage on saves
/// against *my* spell" feature (a Heightened-Spell-shaped metamagic, a
/// subclass mark, a hypothetical Gnome Cunning-style ward that flips
/// the polarity to advantage) lands as one row here rather than another
/// branch in `roll_save_against_caster`.
///
/// Entries (in order — the walk is order-independent since every row
/// combines into the same accumulator):
///   - **Heightened Spell** (Sorcerer Metamagic): the caster spent
///     three sorcery points on a bonus-action prime; the first save
///     against their next spell is at disadvantage.
///   - **Eldritch Strike** (Eldritch Knight Fighter lv10): the caster
///     landed a weapon hit on this target, so the target's next save
///     against a spell *this* caster casts is at disadvantage. The
///     back-link is what makes the mark
///     caster-specific — a second spellcaster on the team gets no
///     benefit from the fighter's swing.
///   - **Magical Ambush** (Arcane Trickster Rogue lv9): the caster is
///     Hidden, so their spell's first save lands at disadvantage.
///     Consuming `Hidden` on the trigger mirrors the way
///     `CONSUMED_ON_ATTACK` drops it on an attack roll — casting a
///     spell at someone gives your position away just as surely as
///     shooting at them does.
pub const CASTER_SAVE_MODE_RIDERS: &[CasterSaveModeRider] = &[
    CasterSaveModeRider {
        applies: |e, caster_id, _target_id| {
            e.actors
                .get(&caster_id)
                .is_some_and(|a| a.has_condition(Condition::HeightenedSpelling))
        },
        consume: |e, caster_id, _target_id| {
            if let Some(caster) = e.actors.get_mut(&caster_id) {
                caster.remove_condition(Condition::HeightenedSpelling);
            }
        },
        mode: RollMode::Disadvantage,
        label: "heightened spell",
    },
    CasterSaveModeRider {
        applies: |e, caster_id, target_id| {
            e.actors.get(&target_id).is_some_and(|t| {
                t.linked_by(Condition::EldritchStruck) == Some(caster_id)
            })
        },
        consume: |e, _caster_id, target_id| {
            if let Some(target) = e.actors.get_mut(&target_id) {
                target.remove_condition(Condition::EldritchStruck);
            }
        },
        mode: RollMode::Disadvantage,
        label: "eldritch strike",
    },
    CasterSaveModeRider {
        applies: |e, caster_id, _target_id| {
            e.actors.get(&caster_id).is_some_and(|a| {
                a.has_passive_feature(crate::actions::class_features::MAGICAL_AMBUSH_TAG)
                    && a.has_condition(Condition::Hidden)
            })
        },
        consume: |e, caster_id, _target_id| {
            if let Some(caster) = e.actors.get_mut(&caster_id) {
                caster.remove_condition(Condition::Hidden);
            }
        },
        mode: RollMode::Disadvantage,
        label: "magical ambush",
    },
    // 5e Circle of Power: "you and friendly creatures within 30 feet
    // have advantage on saving throws against spells and other magical
    // effects." The first row on this table that reads the *target*
    // rather than the caster, and the first whose prime is a standing
    // ward rather than a one-shot — see the `consume` note above.
    CasterSaveModeRider {
        applies: |e, _caster_id, target_id| {
            e.actors
                .get(&target_id)
                .is_some_and(|t| t.has_condition(Condition::PowerCircled))
        },
        consume: |_e, _caster_id, _target_id| {},
        mode: RollMode::Advantage,
        label: "circle of power",
    },
];

/// Conditions whose presence combines a blanket **disadvantage** into
/// the actor's save-roll mode regardless of which ability the save
/// rolls off of. Read by `compute_save_mode`. Adding a new "condition
/// X gives disadvantage on every save" install (a future Sickened /
/// Cursed-tier debuff) lands as a one-line entry here instead of
/// another `if actor.has_condition(...) { tally.add(...) }` clause in
/// the save-mode body.
const BLANKET_SAVE_DISADVANTAGE_CONDITIONS: &[Condition] = &[
    // 5e Poisoned: disadvantage on ability checks and saves.
    Condition::Poisoned,
    // 5e Frightened: disadvantage on ability checks while you can see
    // the source. Tests treat this as blanket save disadvantage too.
    Condition::Frightened,
    // Exhausted is deliberately absent: the flag means "at least tier
    // 1", and RAW's save penalty does not arrive until tier 3. The gate
    // lives in `compute_save_mode`, which can read the tier.
];

/// Conditions whose presence combines a blanket **advantage** into
/// the actor's save-roll mode. Sibling to
/// `BLANKET_SAVE_DISADVANTAGE_CONDITIONS`. Any future save-buff aura
/// (Guardian Angel, Sanctuary-tier ward) lands here as a one-line
/// entry.
const BLANKET_SAVE_ADVANTAGE_CONDITIONS: &[Condition] = &[
    // 5e Bless (as an active concentration): advantage on saves. The
    // Bless die's +1d4 lives at the roll site in `roll_save`; the
    // advantage source lives here.
    Condition::Blessed,
    // 5e Holy Aura: advantage on every save inside the 30ft bubble.
    Condition::HolyAuraed,
    // 5e Foresight: advantage on every save (single-target, 8-hour
    // buff RAW; we model as the concentration-installed condition).
    Condition::Foreseen,
];

/// Conditions that flip the save-roll mode on the three **mental**
/// saves — Intelligence, Wisdom and Charisma — and leave the three
/// physical ones alone. Read by `compute_save_mode` under a single
/// mental-ability gate, the same way the DEX and STR clusters branch
/// once and fold their rows inside.
///
/// The cohort is deliberately signed (each row carries its own
/// `RollMode`) rather than split into advantage and disadvantage
/// lists the way the blanket lane is. The blanket lane has room for
/// that split because its two tables are read at different points; a
/// scoped cluster is read under one gate, so two tables here would be
/// two loops behind one `if` — the STR cluster already learned that
/// and carries `(condition, mode)` pairs for the same reason.
///
/// Rows:
///   - **Feeblemind**: INT and CHA drop to 1, so every mental save
///     lands at disadvantage while STR / DEX / CON are untouched — the
///     target can still throw itself away from a fireball. This clause
///     predates the cohort and used to be a hand-written
///     `has_condition(Feebled) && matches!(ability, ...)` branch below
///     the clusters.
///   - **Intellect Fortress**: "advantage on Intelligence, Wisdom, and
///     Charisma saving throws" — the clause that wanted a scoped lane
///     and found only a blanket one, which is what turned the
///     Feeblemind branch into this table.
const MENTAL_SAVE_MODE_CONDITIONS: &[(Condition, RollMode)] = &[
    (Condition::Feebled, RollMode::Disadvantage),
    (Condition::IntellectFortified, RollMode::Advantage),
];

/// The mirror of `MENTAL_SAVE_MODE_CONDITIONS` on the other three
/// abilities: conditions that flip the save-roll mode on **Strength,
/// Dexterity and Constitution** together and leave the mental saves
/// alone.
///
/// The cluster 5e keeps writing and the engine had no lane for. STR had
/// one (shared with checks, because every row on it says "checks and
/// saving throws"), DEX had four clauses inlined at the call site, and
/// CON had none at all — so a feature whose text is the physical trio
/// as a unit had to be entered three times or not at all.
///
/// Rows:
///   - **Gaseous Form**: "it has Advantage on Strength, Dexterity, and
///     Constitution saving throws" — a cloud is hard to grab, hard to
///     catch and hard to poison, and the sentence names all three.
const PHYSICAL_SAVE_MODE_CONDITIONS: &[(Condition, RollMode)] =
    &[(Condition::Gaseous, RollMode::Advantage)];

/// Conditions whose presence combines a blanket **disadvantage** into
/// the actor's *ability check* mode. Sibling of
/// `BLANKET_SAVE_DISADVANTAGE_CONDITIONS` one lane over — same row
/// shape, different roll.
///
/// The two lists are deliberately not the same list. RAW splits these
/// clauses per condition — Poisoned reaches checks and attacks but not
/// saves, Blessed reaches saves but not checks — so a single shared
/// cohort would over-apply roughly half its rows on one of the two
/// lanes.
const BLANKET_CHECK_DISADVANTAGE_CONDITIONS: &[Condition] = &[
    // 5e Poisoned: "disadvantage on attack rolls and ability checks."
    Condition::Poisoned,
    // 5e Frightened: "disadvantage on ability checks and attack rolls
    // while the source of its fear is within line of sight." We don't
    // track LOS-to-the-fear-source, so the clause is unconditional —
    // the same simplification the attack lane already makes.
    Condition::Frightened,
    // 5e Flesh Golem Aversion to Fire: "Disadvantage on attack rolls
    // and ability checks until the end of its next turn." The other
    // clause rides `imposes_attacker_disadvantage`.
    Condition::Flinching,
    // 5e Feeblemind: INT and CHA drop to 1. The save lane scopes the
    // penalty to the three mental abilities; a check is asked for by
    // ability here too, so the scoping lives in `compute_check_mode`
    // rather than on this blanket row — see the mental-ability gate
    // there. Kept off this table for that reason.
];

/// Conditions whose presence combines a blanket **advantage** into the
/// actor's ability-check mode. Sibling to
/// `BLANKET_CHECK_DISADVANTAGE_CONDITIONS`.
const BLANKET_CHECK_ADVANTAGE_CONDITIONS: &[Condition] = &[
    // 5e Foresight: "advantage on attack rolls, ability checks, and
    // saving throws". The attack and save lanes already read the
    // condition; this is the third clause finally being honored.
    Condition::Foreseen,
    // 5e Wild Magic Sorcerer **Tides of Chaos**: "advantage on one
    // attack roll, ability check, or saving throw". Spent by
    // `CONSUMED_ON_CHECK` when a check actually collects it, the same
    // way `CONSUMED_ON_ATTACK` burns it on the attack lane.
    Condition::TidesOfChaos,
];

/// One-shot riders a *check* spends when it collects them — the
/// ability-check sibling of `CONSUMED_ON_ATTACK` / `CONSUMED_ON_SAVE`.
///
/// RAW spends a Bardic Inspiration die on one "ability check, attack
/// roll, or saving throw", so all three lanes have to burn it or one
/// die pays twice. The check lane is also the *only* lane Guidance was
/// ever supposed to reach — it maps onto `Inspired` here, and until
/// checks became real rolls a cast of Guidance could not affect
/// anything at all.
const CONSUMED_ON_CHECK: &[Condition] = &[
    Condition::Inspired,
    // Tides of Chaos grants its advantage to exactly one roll; the
    // check lane burns it for the same reason the attack lane does.
    Condition::TidesOfChaos,
];

/// Conditions that flip the roll mode on **Strength** checks and
/// **Strength** saves, and which way. Read by `compute_save_mode` and
/// `compute_check_mode` inside their single STR gates.
///
/// Sibling to the blanket tables above, one ability narrower. The
/// membership is really one idea seen from four angles: RAW gives
/// advantage on STR checks and saves to a creature that is angrier or
/// bigger than it was, and disadvantage to one that is smaller.
///
/// Every row's RAW text names checks and saves in the same breath
/// ("advantage on Strength checks and Strength saving throws"), which
/// is why one table serves both callers rather than two tables that
/// would have to be kept in step by hand.
const STRENGTH_CHECK_AND_SAVE_MODE_CONDITIONS: &[(Condition, RollMode)] = &[
    // 5e Barbarian Rage: advantage on STR checks and saves while raging.
    (Condition::Raging, RollMode::Advantage),
    // 5e Enlarge (the growth half of Enlarge / Reduce): "the target has
    // advantage on Strength checks and Strength saving throws".
    (Condition::Enlarged, RollMode::Advantage),
    // 5e Reduce, the mirror clause: disadvantage on both.
    (Condition::Reduced, RollMode::Disadvantage),
    // 5e Rune Knight **Giant's Might**: same advantage clause as Enlarge,
    // reached by the same "you are briefly a bigger creature" flavor.
    (Condition::GiantsMight, RollMode::Advantage),
];

/// A single "reroll the failed save once" source read at
/// `roll_save_with_extra_mode` after the initial roll lands on a
/// `Fail`. Each entry's `consume` closure returns true iff its
/// per-rest charge (or one-shot latch) was spent — the caller then
/// rolls a new d20, logs it with `label`, and stops iterating if the
/// reroll passes. Consumption fires only when the initial roll
/// failed AND no earlier source in the table has already granted a
/// pass, so a paladin/fighter multiclass burns Indomitable's pre-
/// primed latch before Fanatical Focus's short-rest charge.
struct FailedSaveRerollSource {
    /// Log-friendly tag ("indomitable", "fanatical focus"). Appears
    /// in the "  {} reroll: 1d20(X)+Y = Z — pass/fail" line.
    label: &'static str,
    /// Attempts to spend this source's per-rest / pre-primed charge
    /// on the failing actor. Returns true on a successful spend (the
    /// reroll fires); false when the charge was unavailable
    /// (unprimed Indomitable, spent Fanatical Focus, etc.).
    consume: fn(&mut crate::actors::actor_template::ActorInstance) -> bool,
}

/// Ordered cohort of "reroll the failed save once" sources, read by
/// `roll_save_with_extra_mode`. Iteration stops as soon as one
/// source's reroll produces a `Pass`, so at most one source burns
/// its charge per save; ordering is significant because
/// pre-primed / action-cost sources appear ahead of automatic ones
/// (the pre-primed choice was explicit, so it should burn first on a
/// multiclass holding both).
///
/// Entries:
///   - **Indomitable** (Fighter lv9): pre-primed once-per-long-rest
///     latch set by the `Indomitable` Action. Consumes the
///     `indomitable_pending` field; returns false if the latch was
///     never set (RAW: the reroll must be declared before rolling —
///     since we don't model the "before I roll" cadence, we surface
///     it as an explicit Action call the actor takes on their turn
///     as insurance for the next save).
///   - **Fanatical Focus** (Oathbreaker Paladin lv15): auto-fire
///     once-per-short-rest gate. Consumes the `FANATICAL_FOCUS_TAG`
///     charge on `features_remaining` — no pre-priming, so the
///     first failed save while the tag is unspent triggers the
///     reroll. Ships in `SHORT_REST_FEATURES` so a short rest
///     refills the charge.
///
/// A new failed-save reroll source (Halfling's "you can reroll a
/// nat-1", except we already model that as `has_lucky` at the roll
/// site rather than the fail site; a hypothetical racial reroll, a
/// future "Reroll One" feat) drops in as a new entry.
const FAILED_SAVE_REROLL_SOURCES: &[FailedSaveRerollSource] = &[
    FailedSaveRerollSource {
        label: "indomitable",
        consume: |a| a.consume_indomitable(),
    },
    FailedSaveRerollSource {
        label: "fanatical focus",
        // `spend_feature` already returns true iff a charge was there
        // to take, so the previous check-then-spend body collapses to
        // the one-line call — sibling of the `spend_feature(source.tag)`
        // call in the add-die cohort's loop body.
        consume: |a| a.spend_feature(crate::actions::class_features::FANATICAL_FOCUS_TAG),
    },
];

/// A single "add die(s) to the failing save total" source read at
/// `roll_save_with_extra_mode` after the initial roll lands on a
/// `Fail`. Sibling shape to `FailedSaveRerollSource` on the shared
/// failed-save recovery lane, distinct in mechanic: the reroll cohort
/// spins a fresh d20, this cohort keeps the initial d20 and adds a
/// die pool to the total. A failed d20(3) that the reroll cohort
/// rerolls to another 3 stays failed; the same d20(3) that the
/// add-die cohort boosts picks up the extra pool (avg +5-ish) and
/// pushes past most mid-DC saves.
///
/// Each entry's `tag` is spent at the failing actor via
/// `ActorInstance::spend_feature`, which returns true iff the
/// per-rest charge was actually consumed (HashSet::remove semantics —
/// no separate `feature_available` check needed). On a successful
/// spend the caller rolls `dice`, logs it with `label`, and returns
/// immediately if the boosted total meets or beats the DC.
/// Consumption fires only when the initial roll failed AND no earlier
/// source in the table has already granted a pass, so a hypothetical
/// Divine Soul Sorcerer / Fiend Warlock multiclass burns DOOL's
/// 1d10 (first-listed) before FBTG's 2d4 in table order. All fires
/// happen before the reroll cohort — RAW's "after seeing the initial
/// roll but before any of the roll's effects occur" gate for both
/// DOOL and FBTG places them ahead of any reroll surface.
///
/// Pre-cleanup this row carried a `consume: fn(&mut ActorInstance) ->
/// bool` closure that open-coded the "check feature_available, then
/// spend_feature, return whether it fired" 5-line pattern per row.
/// The check-then-spend was redundant — `spend_feature` already
/// returns a bool from HashSet::remove semantics — so both rows
/// collapsed to the same 1-line closure. Promoted here to a bare
/// `tag: &'static str` so the row shape mirrors
/// `ReactiveDisadvantageSource` (tag + gating parameters) and future
/// tag-driven add-die sources land as a `(label, tag, dice)` triple.
struct FailedSaveAddDieSource {
    /// Log-friendly label ("dark one's own luck", "favored by the
    /// gods"). Appears in the "  {}: +{} = {} vs DC {} — pass/fail"
    /// line. Distinct from `tag` because tags carry a `class.feature`
    /// namespace prefix ("warlock.dark_ones_own_luck") that would be
    /// noisy in the combat log.
    label: &'static str,
    /// Feature tag read via `spend_feature(tag)` on the failing actor
    /// — returns true iff the per-rest charge was consumed. Sibling
    /// shape to `ReactiveDisadvantageSource::tag` on the "tag +
    /// gating parameters" row pattern.
    tag: &'static str,
    /// Die pool added to the initial d20 + modifier + extra total.
    /// A `Dice { count, faces }` value read at cohort-iteration time
    /// so each entry can pick its own die shape (1d10 for DOOL, 2d4
    /// for Favored by the Gods).
    dice: Dice,
}

/// Ordered cohort of "add die(s) to a failed save total" sources,
/// read by `roll_save_with_extra_mode` after the initial d20 lands
/// on a fail and BEFORE the reroll cohort — RAW's "after seeing the
/// initial roll but before any of the roll's effects occur" gate
/// for both entries places them at initial-roll time. Iteration
/// stops as soon as one source's boost produces a `Pass`, so at
/// most one source burns its charge per save.
///
/// Order: entries are consulted in listed order; on a multiclass
/// holding multiple sources, the first-listed one fires first. Both
/// current entries are once-per-short-rest auto-fire — either order
/// is defensible.
///
/// Entries:
///   - **Dark One's Own Luck** (Fiend Warlock lv6): +1d10 (avg
///     +5.5). Ships on `FIEND_WARLOCK_TEMPLATE` via
///     `DARK_ONES_OWN_LUCK_TAG`. Sibling to Fanatical Focus on the
///     failed-save recovery lane but distinct in shape (add-die vs.
///     reroll).
///   - **Favored by the Gods** (Divine Soul Sorcerer lv1, XGtE):
///     +2d4 (avg +5.0). Ships on `DIVINE_SOUL_SORCERER_TEMPLATE`
///     via `FAVORED_BY_THE_GODS_TAG`. Same add-die shape as DOOL
///     with a tighter die pool (2..=8 vs. 1..=10).
///
/// A new failed-save add-die source (a hypothetical "Reroll +Nd4"
/// feat, a future Peace Cleric Balm of the Summer Court on the
/// attack-roll lane collapsed to a save-side add-die) drops in as a
/// new entry with its own `(label, dice, consume)` triple.
/// Cohort row shape for a target-side reactive per-rest feature that,
/// on an incoming attack roll against the holder, spends the holder's
/// reaction + a `feature_available(tag)` charge to impose disadvantage
/// on the swing. Rows carry only their identifying tag + gating
/// parameters; the shared gate + spend + log body lives in
/// `EncounterInstance::try_apply_reactive_disadvantage_source` so a new
/// sibling drops in as a fresh row rather than a new method.
///
/// Rows differ on two axes:
///   - `range_tiles`: `Some(n)` gates on footprint-Chebyshev distance
///     (n tiles); `None` skips the range gate entirely (RAW: no range
///     cap on the feature). Warding Flare's 30ft RAW cap → `Some(12)`
///     on the 2.5ft grid; Entropic Ward's un-ranged RAW → `None`.
///   - `requires_sight`: `true` gates on `viewer_can_see(target,
///     attacker)` (folds the Blinded clause AND the illusion-piercing
///     concealment clause); `false` skips the sight gate entirely.
///     Warding Flare's RAW "when a creature you can see" → `true`;
///     Entropic Ward's un-sighted RAW (the patron's tie transcends
///     line-of-sight) → `false`.
///
/// Both current entries are once-per-short-rest — the `SHORT_REST_FEATURES`
/// registry refreshes them on the same cadence, so ordering only
/// matters when a multiclass carries both flags on one turn (the
/// first-listed row fires first, mirroring
/// `FAILED_SAVE_ADD_DIE_SOURCES` ordering).
struct ReactiveDisadvantageSource {
    /// Feature tag for the row — read via `has_passive_feature(tag)`
    /// (installed on template) and `feature_available(tag)` /
    /// `spend_feature(tag)` (the per-rest charge lane). Appears in the
    /// log line as the row's identity: "warding flare: ..." or
    /// "entropic ward: ...".
    tag: &'static str,
    /// Log-friendly label ("warding flare", "entropic ward"). Distinct
    /// from `tag` because tags carry a `class.feature` namespace prefix
    /// ("cleric.warding_flare") that would be noisy in the combat log.
    log_label: &'static str,
    /// `Some(n)` if the row gates on footprint-Chebyshev distance ≤ n
    /// tiles between target and attacker; `None` if the row has no
    /// range gate.
    range_tiles: Option<isize>,
    /// `true` if the row gates on `viewer_can_see(target, attacker)`
    /// (the shared sight helper folding Blinded + illusion-piercing);
    /// `false` if the row has no sight gate.
    requires_sight: bool,
}

/// Ordered cohort of "target-side reactive per-rest features that
/// impose disadvantage on an incoming attack roll" sources, walked by
/// `EncounterInstance::apply_reactive_attack_disadvantage` at the two
/// attack chokepoints (weapon in `engine::attack::resolve_attack`, spell
/// in `spell_attack_outcome`). Iteration stops as soon as one source
/// fires, so at most one per-rest charge burns per incoming attack —
/// mirrors the "at most one add-die per save" ordering semantics on
/// `FAILED_SAVE_ADD_DIE_SOURCES`.
///
/// Order: entries are consulted in listed order. On a multiclass
/// holding multiple sources, the first-listed one fires first. Both
/// current entries are once-per-short-rest; the multiclass ordering
/// pick is defensible either way.
///
/// Entries:
///   - **Warding Flare** (Light Cleric lv1): 30ft range,
///     `requires_sight: true`. Ships on `LIGHT_CLERIC_TEMPLATE` via
///     `WARDING_FLARE_TAG`.
///   - **Entropic Ward** (Great Old One Warlock lv6): no range gate,
///     `requires_sight: false`. Ships on
///     `GREAT_OLD_ONE_WARLOCK_TEMPLATE` via `ENTROPIC_WARD_TAG`.
///
/// A new sibling drops in as a fresh row with its own `(tag,
/// log_label, range_tiles, requires_sight)` quadruple.
const REACTIVE_ATTACK_DISADVANTAGE_SOURCES: &[ReactiveDisadvantageSource] = &[
    ReactiveDisadvantageSource {
        tag: crate::actions::class_features::WARDING_FLARE_TAG,
        log_label: "warding flare",
        range_tiles: Some(12),
        requires_sight: true,
    },
    ReactiveDisadvantageSource {
        tag: crate::actions::class_features::ENTROPIC_WARD_TAG,
        log_label: "entropic ward",
        range_tiles: None,
        requires_sight: false,
    },
];

const FAILED_SAVE_ADD_DIE_SOURCES: &[FailedSaveAddDieSource] = &[
    FailedSaveAddDieSource {
        label: "dark one's own luck",
        tag: crate::actions::class_features::DARK_ONES_OWN_LUCK_TAG,
        dice: Dice::new(1, 10),
    },
    FailedSaveAddDieSource {
        label: "favored by the gods",
        tag: crate::actions::class_features::FAVORED_BY_THE_GODS_TAG,
        dice: Dice::new(2, 4),
    },
];

/// Cohort row shape for a passive subclass feature that grants the
/// swinging actor temporary hit points whenever their damage drops a
/// hostile creature to 0 HP. Rows carry the identifying tag plus a
/// stat-block-driven amount closure — the shared trigger body lives
/// in `EncounterInstance::pay_kill_triggered_temp_hp` so a new
/// sibling drops in as a fresh row rather than a new open-coded
/// method.
///
/// The closure takes an `&ActorInstance` (the swinger) rather than
/// running against raw ability scores because each source pulls from
/// a different ability + level combination — Fiend Warlock keys off
/// CHA + warlock level (min 1); Long Death Monk keys off CON + monk
/// level with a base +1 offset. Passing the actor lets each row read
/// its own stat pair through the shared `ability_modifier` + `level`
/// accessors without a per-source column in the row.
struct KillTriggeredTempHpSource {
    /// Feature tag for the row — read via `has_passive_feature(tag)`.
    /// Appears as the row's identity: `DARK_ONES_BLESSING_TAG`,
    /// `TOUCH_OF_DEATH_TAG`.
    tag: &'static str,
    /// Formula closure: given the swinger, return the temp HP amount
    /// (already `max(1, ...)`-floored). Runs once per matching kill;
    /// the returned value goes through the standard `GainTempHp` side
    /// effect so the max-of-current-and-new stack rule still holds.
    amount: fn(&ActorInstance) -> u32,
}

/// Ordered cohort of "kill-triggered temp HP" sources, walked by
/// `EncounterInstance::pay_kill_triggered_temp_hp` under the
/// `DealDamage::apply` chokepoint on the `Downed` / `Killed` outcome
/// branches. Iteration stops as soon as the first row's tag matches
/// on the swinger, so at most one temp HP grant fires per kill —
/// mirrors the "at most one add-die per save" / "at most one reactive
/// disadvantage per attack" ordering semantics on
/// `FAILED_SAVE_ADD_DIE_SOURCES` / `REACTIVE_ATTACK_DISADVANTAGE_SOURCES`.
///
/// Entries:
///   - **Dark One's Blessing** (Fiend Warlock lv1): temp HP =
///     `max(1, CHA mod + warlock level)`. Ships on
///     `FIEND_WARLOCK_TEMPLATE` via `DARK_ONES_BLESSING_TAG`.
///   - **Touch of Death** (Long Death Monk lv3): temp HP =
///     `max(1, 1 + CON mod + monk level)`. Ships on
///     `LONG_DEATH_MONK_TEMPLATE` via `TOUCH_OF_DEATH_TAG`.
///
/// Order: entries are consulted in listed order. On a hypothetical
/// multiclass holding both flags (RAW rules this out — Warlock Fiend
/// vs. Monk Long Death are different classes with different
/// subclasses, so no legal single-build carries both), the first-
/// listed row would fire; the ordering is a stable pick rather than
/// an inventory of expected multiclass co-occurrence.
///
/// A new kill-triggered temp HP feature drops in as a fresh row
/// with its own `(tag, amount)` pair.
const KILL_TRIGGERED_TEMP_HP_SOURCES: &[KillTriggeredTempHpSource] = &[
    KillTriggeredTempHpSource {
        tag: crate::actions::class_features::DARK_ONES_BLESSING_TAG,
        amount: |a| {
            let cha = a.ability_modifier(crate::engine::types::AbilityScoreType::Charisma);
            let level = a.level() as i32;
            (cha + level).max(1) as u32
        },
    },
    KillTriggeredTempHpSource {
        tag: crate::actions::class_features::TOUCH_OF_DEATH_TAG,
        amount: |a| {
            let con = a.ability_modifier(crate::engine::types::AbilityScoreType::Constitution);
            let level = a.level() as i32;
            (1 + con + level).max(1) as u32
        },
    },
];

/// How much of a subject's concealment a given viewer sees through.
/// Produced by `EncounterInstance::concealment_piercing_of` and read at
/// the concealment-suppression clauses, which need to know not just
/// *whether* the viewer pierces but *what*: RAW's See Invisibility
/// lifts `Invisible` and leaves `Blurred` / `Displaced` fully in play,
/// which the pre-existing bool couldn't say.
///
/// The variants are a strict hierarchy — each tier pierces everything
/// the tier below it does — which is what lets
/// `concealment_piercing_of` return the first (strongest) source it
/// finds instead of unioning cohorts. `pierces` is the only place the
/// tiers meet their condition cohorts, so a new concealment condition
/// is classified once, on `Condition`, rather than at every consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConcealmentPiercing {
    /// Sees through nothing. The overwhelmingly common answer.
    None,
    /// Sees through invisibility and nothing else — See Invisibility,
    /// the Divination Wizard's Third Eye.
    Invisibility,
    /// Sees through the whole illusion cohort — Truesight, Feral
    /// Senses, and every range-gated non-visual sense in
    /// `nonvisual_sense_reaches` that reaches the subject.
    ///
    /// Also finds a creature that is merely *hidden*, which is a
    /// different rule reached by a different half of the tier — see
    /// `Condition::countered_by_keen_senses` for which sources RAW
    /// grants it to and which two are along for the ride.
    All,
}

impl ConcealmentPiercing {
    /// True if this tier sees through `c`'s concealment. A condition
    /// outside the concealment cohort entirely (Dodging, Holy Aura,
    /// Foreseen — active defenses rather than illusions) is never
    /// pierced by any tier, which is what keeps the suppression clauses
    /// from having to re-check cohort membership themselves.
    pub fn pierces(self, c: Condition) -> bool {
        match self {
            ConcealmentPiercing::None => false,
            ConcealmentPiercing::Invisibility => c.countered_by_see_invisibility(),
            ConcealmentPiercing::All => {
                c.countered_by_truesight() || c.countered_by_keen_senses()
            }
        }
    }
}

/// Envelope, in tiles, of the two 10-ft class-granted blindsight
/// analogues — Rogue **Blindsense** (lv14) and the **Blind Fighting**
/// Fighting Style. 10 ft is 4 tiles on the 2.5-ft grid.
///
/// Named because both features quote the same radius from RAW and a
/// bare `4` at two `.max()` terms reads as a coincidence rather than as
/// the one number it is.
const CLASS_BLINDSIGHT_TILES: isize = 4;

/// True if some non-visual sense of `viewer`'s reaches `subject` — the
/// one question both sight gates ask, and the one place a new
/// non-visual sense has to land to be read by all of them.
///
/// The gates it answers for:
///
///   - **`obscurement_blinds`** — can the viewer pick the subject out
///     of a fog bank? RAW's heavily-obscured area "blocks vision", and
///     a sense that doesn't rely on vision is untouched by it.
///   - **`concealment_piercing_of`** — can the viewer pick the subject
///     out from under an illusion? Same clause, same answer: a bat
///     doesn't care that the mage went Invisible.
///
/// Both used to keep their own list, and the lists disagreed —
/// blindsight pierced fog but not invisibility, Blind Fighting pierced
/// invisibility but not fog, and RAW grants both from the same
/// sentence. One helper, one answer.
///
/// The sources, and what gates each:
///
///   - **Blindsight** (`SpecialSense::Blindsight`) — a bat's 60 ft is
///     24 tiles, a constrictor's 10 ft is 4, so a wide enough fog bank
///     still hides an archer from the snake. No further gate.
///   - **Tremorsense** (`SpecialSense::Tremorsense`) — gated on the
///     *subject* standing on the floor (`is_grounded`). This is the
///     only source with a subject-side gate, and the only reason this
///     helper takes the subject rather than returning a bare radius:
///     RAW's "provided that the creature and the source of the
///     vibrations are in contact with the same ground" means a purple
///     worm feels the invisible rogue and loses the flying wizard.
///   - **Blindsense** (Rogue lv14) — `CLASS_BLINDSIGHT_TILES`, gated
///     on the viewer not being Deafened (RAW "while able to hear", so
///     the Silence cohort suppresses it).
///   - **Blind Fighting** (Tasha Fighting Style) — the same envelope
///     with no hearing gate (RAW "even if you're blinded or in
///     darkness" carries no hearing clause).
///
/// Distance is footprint-Chebyshev so a Large / Huge subject's *edge*
/// counts: a Huge creature 6 ft away is in a 10-ft envelope even
/// though its origin tile is 15+ ft off.
///
/// Unbounded piercers — Truesight, Ranger Feral Senses — deliberately
/// aren't here. They have no radius to compare, so their callers
/// short-circuit before the distance read rather than passing an
/// `isize::MAX` through it.
fn nonvisual_sense_reaches(viewer: &ActorInstance, subject: &ActorInstance) -> bool {
    let mut envelope = viewer.blindsight_tiles();
    if subject.is_grounded() {
        envelope = envelope.max(viewer.tremorsense_tiles());
    }
    if viewer.has_blindsense() && !viewer.has_condition(Condition::Deafened) {
        envelope = envelope.max(CLASS_BLINDSIGHT_TILES);
    }
    if viewer.has_blind_fighting_style() {
        envelope = envelope.max(CLASS_BLINDSIGHT_TILES);
    }
    if envelope == 0 {
        return false;
    }
    let dist = footprint_chebyshev(
        viewer.location(),
        get_tiles_from_size(viewer.size()),
        subject.location(),
        get_tiles_from_size(subject.size()),
    );
    dist <= envelope
}

/// Lowest foretold face a Divination Wizard will spend on a d20 they
/// want to land *high* (their own roll, or an ally's). See
/// `EncounterInstance::try_substitute_portent` for why the spend policy
/// is a pair of decisiveness cutoffs rather than a DC comparison.
///
/// 18 clears the AC / DC band the engine's mid-tier creatures sit in
/// once a caster's or martial's modifier is added, so a die at or above
/// it converts a coin-flip into near-certainty on its own. Below that
/// the die is worth more banked: with only two or three per long rest,
/// spending one to turn a likely hit into a slightly likelier hit is
/// how the feature gets wasted.
const PORTENT_HIGH_FACE: u32 = 18;

/// Highest foretold face a Divination Wizard will spend on a d20 they
/// want to land *low* (an enemy's). Mirror of `PORTENT_HIGH_FACE` on
/// the other end of the die, and deliberately not its exact reflection
/// (which would be 3): the low end is worth spending slightly wider
/// because the rolls a diviner most wants to sink — a boss's save
/// against the party's one control spell, a giant's swing at the
/// party's downed healer — are the ones where the target's own
/// modifier is large enough that only a genuinely bad face helps.
const PORTENT_LOW_FACE: u32 = 5;

pub enum StackElementEntry {
    SideEffect(Box<dyn ApplicableSideEffect>),
    Action(Box<ActionExecutionInfo>),
    Prompt(Prompt),
}

/// Snapshot of the engine's top-of-stack for UI consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackState {
    /// No prompts and no pending side-effects — initiative is between turns.
    Idle,
    /// A prompt is open for the given actor; the player or AI must act.
    AwaitingPrompt(usize),
    /// Side effects are mid-resolution (the player will see this between
    /// processing of an AI turn and the next prompt).
    Processing,
}

pub struct StackElement {
    pub entry: StackElementEntry,
    pub id: usize,
}

/// One entry in the initiative queue, as the UI sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitiativeSlot {
    pub actor_id: usize,
    /// True when this is a bonus slot rather than the one the actor
    /// rolled for — see `InitiativeElement::is_extra`. The UI marks these
    /// so a name appearing twice in the panel is legible.
    pub is_extra: bool,
}

struct InitiativeElement {
    pub actor_id: usize,
    pub initiative: i32,
    /// Dex modifier of the actor at insertion time. 5e RAW: ties on the
    /// initiative roll are broken by Dex modifier (higher first), with
    /// the DM's discretion as a final tiebreak. We deterministically
    /// fall back to actor_id ascending so the queue is stable across
    /// seeded runs.
    pub dex_mod: i32,
    /// True for a *second* slot handed to an actor who already owns one
    /// — the Thief Rogue's Thief's Reflexes, which grants a whole extra
    /// turn during the first round of combat at initiative minus 10.
    ///
    /// The queue is keyed by position, not by actor, so a duplicate id
    /// is a legal thing to hold: `current_player` returns the id twice
    /// per round, `advance_initiative` clears the turn-started latch on
    /// the way through, and the second visit therefore opens a real
    /// turn with a real `start_turn_for` — fresh action, fresh bonus
    /// action, fresh once-per-turn riders. What the flag exists for is
    /// the teardown: extra slots are swept at the end of round 1 by
    /// `clear_extra_turns`, and nothing else in the queue may be swept
    /// with them.
    ///
    /// Deliberately absent from `Ord` — and therefore from `Eq`, which
    /// is defined in terms of it below. Sort position is a question
    /// about *when* a slot acts, and an extra slot's answer to that is
    /// already fully encoded in the lowered `initiative` it was inserted
    /// with.
    pub is_extra: bool,
}

/// Equality is defined as "sorts to the same place," which is what `Ord`
/// requires of it: `a == b` must hold exactly when `a.cmp(b)` is
/// `Equal`. A derive would have compared `is_extra` too and broken that
/// the moment a slot carried the flag, since `cmp` deliberately ignores
/// it. Nothing in the queue compares elements for equality today — the
/// insert walk is the only consumer of the ordering — so this exists to
/// keep the trait contract honest rather than to serve a caller.
impl PartialEq for InitiativeElement {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for InitiativeElement {}

impl Ord for InitiativeElement {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher initiative first; higher dex modifier next; actor_id
        // ascending as a final deterministic tiebreaker.
        other
            .initiative
            .cmp(&self.initiative)
            .then_with(|| other.dex_mod.cmp(&self.dex_mod))
            .then(self.actor_id.cmp(&other.actor_id))
    }
}

impl PartialOrd for InitiativeElement {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct InitiativeTracker {
    // this is probably slightly more efficient as a linked list
    initiatives: Vec<InitiativeElement>,
    curr_index: usize,
}

impl InitiativeTracker {
    pub fn new() -> InitiativeTracker {
        InitiativeTracker {
            initiatives: Vec::new(),
            curr_index: 0,
        }
    }

    pub fn current_player(&self) -> Option<usize> {
        self.initiatives.get(self.curr_index).map(|ie| ie.actor_id)
    }

    /// Move to the next slot. Returns `true` when the queue wraps back to
    /// the first actor — the engine reads this to fire the round-end
    /// hook (condition timers tick, future concentration saves go here).
    /// With a one-actor queue every advance "wraps," which is fine: that
    /// queue's owner takes a turn per round.
    pub fn advance(&mut self) -> bool {
        if self.initiatives.is_empty() {
            self.curr_index = 0;
            return false;
        }
        self.curr_index = (self.curr_index + 1) % self.initiatives.len();
        self.curr_index == 0
    }

    pub fn add_actor(&mut self, actor_id: usize, initiative: i32, dex_mod: i32) {
        // Build a temporary element to use the canonical Ord — we want the
        // same multi-key (initiative DESC, dex DESC, id ASC) used for
        // initial sort. Insert at the first position whose existing element
        // sorts *after* the new one, preserving order.
        self.insert_slot(InitiativeElement {
            actor_id,
            initiative,
            dex_mod,
            is_extra: false,
        });
    }

    /// Hand `actor_id` a *second* slot at `initiative`, on top of the one
    /// they already hold — the queue shape behind Thief's Reflexes.
    ///
    /// Split from `add_actor` rather than folded into it with a flag
    /// because the two say different things: `add_actor` is "somebody new
    /// joined the fight," and this is "somebody already in the fight acts
    /// twice." Only the latter is swept by `clear_extra_turns`.
    pub fn add_extra_turn(&mut self, actor_id: usize, initiative: i32, dex_mod: i32) {
        self.insert_slot(InitiativeElement {
            actor_id,
            initiative,
            dex_mod,
            is_extra: true,
        });
    }

    /// Place `elem` at its sorted position, keeping `curr_index` pointed
    /// at whoever it pointed at before.
    fn insert_slot(&mut self, elem: InitiativeElement) {
        // Insert at the first position whose existing element sorts
        // *after* the new one, so the queue stays in the canonical
        // (initiative DESC, dex DESC, id ASC) order `initialize_actors`
        // established.
        let idx = self
            .initiatives
            .iter()
            .position(|ie| elem.cmp(ie) == Ordering::Less)
            .unwrap_or(self.initiatives.len());
        self.initiatives.insert(idx, elem);
        // If we inserted at or before the active slot, the active actor
        // shifted down by one; bump curr_index to keep pointing at them.
        if idx <= self.curr_index && !self.initiatives.is_empty() {
            self.curr_index = (self.curr_index + 1).min(self.initiatives.len() - 1);
        }
    }

    /// Drop **every** slot `actor_id` owns. Called when an actor leaves
    /// the fight for any reason (death, despawn), so it has to sweep all
    /// of them: a Thief who dies on their first turn of round 1 still
    /// holds an extra slot ten points down the queue, and leaving it
    /// behind would hand a corpse a turn.
    pub fn remove_actor(&mut self, actor_id: usize) {
        // Snapshot before the retain so the closure isn't borrowing
        // `self` while `self.initiatives` is mutably borrowed.
        let curr = self.curr_index;
        let mut removed_before_curr = 0usize;
        let mut idx = 0usize;
        self.initiatives.retain(|ie| {
            let keep = ie.actor_id != actor_id;
            if !keep && idx < curr {
                removed_before_curr += 1;
            }
            idx += 1;
            keep
        });
        if self.initiatives.is_empty() {
            self.curr_index = 0;
            return;
        }
        // Removals ahead of the active slot shift it up by that many; a
        // removal *of* the active slot doesn't move the index at all, so
        // the next actor naturally slides into place there.
        self.curr_index = curr - removed_before_curr;
        if self.curr_index >= self.initiatives.len() {
            self.curr_index = 0;
        }
    }

    /// Drop every extra slot in the queue and report whose they were, in
    /// queue order. Called once, at the end of round 1, to retire the
    /// Thief's Reflexes turns.
    ///
    /// Safe to call from the wrap point and nowhere else: `advance`
    /// reports a wrap exactly when `curr_index` has landed back on 0, and
    /// an extra slot can never *be* index 0 — it sorts strictly below the
    /// slot its own owner rolled, so at least that one precedes it. So
    /// the surviving element at index 0 is the same one before and after.
    pub fn clear_extra_turns(&mut self) -> Vec<usize> {
        let cleared: Vec<usize> = self
            .initiatives
            .iter()
            .filter(|ie| ie.is_extra)
            .map(|ie| ie.actor_id)
            .collect();
        if !cleared.is_empty() {
            self.initiatives.retain(|ie| !ie.is_extra);
        }
        cleared
    }

    pub fn initialize_actors(&mut self, actors: &BTreeMap<usize, ActorInstance>) {
        for (id, actor) in actors.iter() {
            self.initiatives.push(InitiativeElement {
                actor_id: *id,
                initiative: actor.initiative().expect("Expected initiative"),
                dex_mod: actor.initiative_mod(),
                is_extra: false,
            });
        }
        self.initiatives.sort();
    }
}

/// Hands out monotonically-increasing ids for stack elements so future
/// dependent-effect logic (e.g. "this side-effect only fires if action #N
/// hit") has something to key on. Resets between encounter rounds.
struct OutcomeTracker {
    next_id: usize,
}

impl OutcomeTracker {
    pub fn new() -> OutcomeTracker {
        OutcomeTracker { next_id: 0 }
    }

    pub fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn reset(&mut self) {
        self.next_id = 0;
    }
}

/// Authoritative state for one combat encounter. Most fields are kept
/// private; access goes through methods so engine invariants (actor map
/// stays in sync with actor locations, initiative queue stays in sync with
/// the actor table, etc.) can't be broken from the outside. `actors` is
/// still public read/write because every consumer (AI, picker UI, action
/// validation) needs deep access to actor state — narrowing it would
/// require a much larger accessor surface.
/// The tiles a straight Bresenham line from `from` to `to` passes
/// through, **excluding both endpoints**. Empty when the two are the
/// same tile or adjacent.
///
/// One walk, three callers. Line-of-sight asks whether any of these
/// tiles is a wall, cover asks how many of them obstruct, and the
/// obscurement gate asks whether any of them is fog — three questions
/// about the same geometry that used to be three hand-inlined copies of
/// the same nine lines of Bresenham. Exclusive of the endpoints because
/// that is what all three want: you are never your own cover, and the
/// tile you are shooting *at* is not in the way of the shot.
pub fn tiles_between(
    from: Coordinate,
    to: Coordinate,
) -> impl Iterator<Item = Coordinate> {
    TilesBetween::new(from, to)
}

/// The tiles exactly `ring` Chebyshev steps from `center` — the square
/// shell, not its interior.
///
/// Row-major (dy outer, dx inner), which makes it deterministic: same
/// board, same seed, same tile. That matters more than which corner of
/// a shell comes first — a summon that landed somewhere different on a
/// replay would make a seeded encounter unreproducible.
fn ring_at(center: Coordinate, ring: isize) -> impl Iterator<Item = Coordinate> {
    (-ring..=ring).flat_map(move |dy| {
        (-ring..=ring)
            // Only the outer edge — the interior belongs to smaller rings.
            .filter(move |dx| dx.abs() == ring || dy.abs() == ring)
            .map(move |dx| Coordinate::new(center.x + dx, center.y + dy))
    })
}

/// The tiles around `center`, **closest ring first**, out to `radius`
/// Chebyshev steps. `center` itself is never yielded.
///
/// One walk, two callers, and the reason they exist as a pair is that
/// they used to disagree. `find_adjacent_teleport_anchor` walked rings
/// outward and got the closest legal tile; `find_adjacent_spawn` walked
/// a plain `-radius..=radius` double loop and got the *corner* — the
/// most negative offset that happened to be legal, which is as far from
/// the caster as the search box allows. Both call themselves "adjacent"
/// and only one was.
pub(crate) fn rings_outward(center: Coordinate, radius: isize) -> impl Iterator<Item = Coordinate> {
    (1..=radius).flat_map(move |ring| ring_at(center, ring))
}

struct TilesBetween {
    x: isize,
    y: isize,
    x1: isize,
    y1: isize,
    dx: isize,
    dy: isize,
    sx: isize,
    sy: isize,
    err: isize,
    done: bool,
}

impl TilesBetween {
    fn new(from: Coordinate, to: Coordinate) -> Self {
        let dx = (to.x - from.x).abs();
        let dy = -(to.y - from.y).abs();
        Self {
            x: from.x,
            y: from.y,
            x1: to.x,
            y1: to.y,
            dx,
            dy,
            sx: if from.x < to.x { 1 } else { -1 },
            sy: if from.y < to.y { 1 } else { -1 },
            err: dx + dy,
            done: from == to,
        }
    }
}

impl Iterator for TilesBetween {
    type Item = Coordinate;

    fn next(&mut self) -> Option<Coordinate> {
        if self.done {
            return None;
        }
        let e2 = 2 * self.err;
        if e2 >= self.dy {
            self.err += self.dy;
            self.x += self.sx;
        }
        if e2 <= self.dx {
            self.err += self.dx;
            self.y += self.sy;
        }
        if self.x == self.x1 && self.y == self.y1 {
            self.done = true;
            return None;
        }
        Some(Coordinate::new(self.x, self.y))
    }
}

/// One row in `EncounterInstance::DAMAGE_INTERPOSERS` — a feature whose
/// holder spends a reaction to have a blow aimed at somebody nearby
/// land on them instead. See that cohort for what the lane is and how
/// it differs from the clamp and intercept cohorts beside it.
#[derive(Debug, Clone, Copy)]
struct DamageInterposer {
    /// Passive feature tag the interposer must carry.
    tag: &'static str,
    /// How far from the creature taking the blow the interposer can
    /// stand, as a footprint gap on the 2.5 ft grid.
    radius: isize,
    /// A condition the *covered* creature must be holding, or `None`
    /// for the rows whose RAW sentence asks nothing of them.
    ///
    /// One row uses it — Protective Bond, whose text is about bonded
    /// creatures rather than about anyone in range — and it is the
    /// column that keeps the cohort's widest reach from also being its
    /// least discriminating.
    covers: Option<Condition>,
    /// What the log line calls the feature that paid.
    label: &'static str,
}

/// The walker-side half of "is this tile bad ground" — the facts about
/// a particular creature that decide whether a tile it could stand on
/// is one it should.
///
/// A struct rather than a pair of `bool` parameters because the
/// pathfinder resolves them together, once per search, and passes them
/// through two call layers to reach `tile_is_bad_ground`. Two bare
/// bools at that depth are two chances to swap them, and the compiler
/// would not notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WalkerAversions {
    /// This walker has a spell on its list, so an Antimagic Field is
    /// worse for it than a web — it does not take damage there, it
    /// simply loses its turn.
    casts: bool,
    /// This walker has run out of breath and has no gills, so every
    /// round it spends in the water costs it a rung of the exhaustion
    /// ladder. See `engine::breath`.
    drowns: bool,
}

impl WalkerAversions {
    /// The walker who minds nothing — what the pathfinder's second,
    /// unrestricted pass carries so that pass has no per-tile checks to
    /// make at all.
    const NONE: WalkerAversions = WalkerAversions {
        casts: false,
        drowns: false,
    };
}

pub struct EncounterInstance {
    /// Non-zero while a damage instance is being carried by somebody
    /// other than the creature it was aimed at — see
    /// `claim_divine_allegiance`. Damage that has already been taken for
    /// somebody cannot be taken for them again, which is what stops two
    /// Crown Paladins standing beside each other from passing a blow
    /// back and forth.
    redirect_depth: u32,
    /// The seed both RNGs were built from — the one passed in, or the
    /// one `empty` drew when none was. Read-only after construction and
    /// surfaced by `seed()`; see `empty` for why an unseeded encounter
    /// still has one.
    seed: u64,
    initialized: bool,
    /// One-shot latch for 5e's opening surprise check — see
    /// `resolve_opening_surprise`.
    ///
    /// The check does not run at `initialize`, and the reason is the
    /// order the program actually assembles an encounter in:
    /// `from_params` initializes, and *then* `main` sets the ambient
    /// light off the command line. A surprise decided at initialize
    /// would therefore be decided on a bright board in every game,
    /// including the ones started with `--dark`, which are the only
    /// ones it can fire in. It runs on the first `process_stack`
    /// instead, when whatever else the caller meant to configure is
    /// configured and before anyone has taken a turn.
    surprise_resolved: bool,
    pub width: usize,
    pub height: usize,
    /// 1-indexed encounter round counter. Bumped each time the initiative
    /// queue wraps back to the first actor (see `advance_initiative`).
    /// Surfaced to the UI so the player can see "round 5" in the side
    /// panel and so future round-aware effects (e.g. Bless ending after
    /// N rounds) can read the absolute round.
    round: u32,
    /// The round whose lair action has already fired, so it fires once
    /// per round however the dispatcher is reached. Two callers reach
    /// it — the initiative wrap, and the first turn to open in a round
    /// — and neither alone is enough: the wrap misses round one
    /// entirely (nothing has wrapped yet), and the turn hook misses a
    /// round in which the encounter ends before anyone is prompted.
    lair_acted_round: Option<u32>,
    /// The lowest total hit points the fight has ever been down to,
    /// summed over every combat-active actor, or `None` before the
    /// first round has ended.
    ///
    /// The odometer for `is_stalemate`'s attrition half. A fight that
    /// is going somewhere drives this number down: creatures take
    /// damage they do not fully get back, and when one drops out of the
    /// fight entirely its whole remaining pool leaves the sum. A fight
    /// that is *not* going anywhere leaves it alone, and the round at
    /// which it last moved is the only thing that distinguishes the two.
    ///
    /// A running minimum rather than a per-round delta because damage
    /// alone is not progress. A Yeti freezing a Shield Guardian for 15
    /// every third round deals damage every time it lands and gets
    /// nowhere at all, because the guardian regenerates 10 a round: the
    /// total oscillates around a floor it never goes below. Minimums
    /// notice that; deltas do not.
    lowest_active_hitpoints: Option<u32>,
    /// The round in which `lowest_active_hitpoints` last set a new
    /// record — i.e. the last round in which the fight got measurably
    /// closer to being over.
    last_attrition_progress_round: u32,
    /// Casters whose concentration-held map layer expired on its own
    /// timer this round, queued for `release_concentration_with_nothing_left`
    /// to look at once every timer has finished ticking.
    ///
    /// A queue rather than a direct call because the two producers
    /// (`tick_zones`, `tick_conjured_terrain`) run mid-teardown, and a
    /// caster who is holding both a zone and a patch would otherwise be
    /// judged by the first of them to expire — with the second still
    /// standing and about to expire in the same tick. Draining once at
    /// the end of `round_end` asks the question exactly when the answer
    /// is stable. Always empty between rounds.
    pending_concentration_review: Vec<usize>,
    terrain: Vec<TerrainInfo>,
    actor_id_next: usize,
    actor_map: Vec<Option<usize>>,
    /// Every creature in the encounter, live or fallen, keyed by id.
    ///
    /// A `BTreeMap` rather than a `HashMap`, and the difference is not
    /// a performance one. This engine promises reproducibility from a
    /// seed — the UI prints the seed on the initiative panel, the AI's
    /// pickers document their tie-breaks as "deterministic ordering
    /// keeps seed reproducibility intact", and a hundred tests sweep a
    /// range of seeds and assert on what comes out. A `HashMap`'s
    /// iteration order is seeded from the operating system, per
    /// process and per thread, so every one of the several dozen sites
    /// that walks this map and takes a `max_by_key`, a `find`, or a
    /// first-match was quietly deciding ties by something no seed
    /// controls.
    ///
    /// Every one of those sites turns out to sort or otherwise settle
    /// its own ties — `the_same_seed_fights_the_same_fight_twice`
    /// passes under either map, which is a real credit to whoever wrote
    /// them. What the `BTreeMap` buys is that it stays true without
    /// anybody having to remember: a walk of this table is now
    /// id-ordered by the type rather than by the care of its author,
    /// which is the order the careful sites were sorting themselves
    /// into by hand anyway. Lookups on a table this size are no slower
    /// for it.
    pub actors: BTreeMap<usize, ActorInstance>,
    /// Loot piles indexed by tile. Items dropped by slain enemies sit
    /// here until a PC walks onto the tile and auto-picks them up
    /// (`MoveActor::apply` calls `pickup_items_at`). HashMap (not a
    /// flat grid) because most tiles are empty and we want O(1) lookup
    /// only when a pickup actually happens.
    items_on_ground: HashMap<Coordinate, Vec<&'static crate::items::item_template::Item>>,
    initiative_tracker: InitiativeTracker,
    encounter_stack: Vec<StackElement>,
    roller: FastRandRoller,
    rng: Rng,
    messages: Vec<String>,
    outcome_tracker: OutcomeTracker,
    /// Non-zero while resolution is nested inside a `Multiattack` /
    /// `CompoundAttack` invocation. Incremented before delegating to a
    /// sub-attack and decremented after. Read by `SimpleWeapon` /
    /// `Greataxe` / etc. to gate the Extra Attack rider: a creature with
    /// both a Multiattack action AND `has_extra_attack: true` — an
    /// ogre chieftain, a veteran — should not have each sub-swing of
    /// its multiattack fire an additional swing, because Multiattack
    /// already encodes the per-Action swing count. Bare swings outside
    /// any Multi (the standalone Rend a dragon's AI picks when the
    /// Multi is unavailable) still chain correctly because depth is 0.
    ///
    /// The dragons used to be the example here and are no longer: the
    /// ladder in `creatures::dragons` dropped `has_extra_attack`
    /// outright, on the grounds that a stat block carrying both flags
    /// is the same rule written twice.
    multiattack_depth: u32,
    /// Stack of in-flight spell casts. `Action::execute` pushes a frame
    /// before building the action's side-effects and pops it after, so
    /// any resolution site nested inside — a burst's per-target save
    /// loop, a shared damage roll, an ally-shield sweep — can ask what
    /// spell it is currently resolving without every helper in the call
    /// chain growing two more parameters.
    ///
    /// A stack rather than a single slot because casts nest: a
    /// Counterspell reaction resolves inside the cast it answers, and a
    /// Twinned Spell re-issues its `side_effects` builder within the
    /// enclosing `execute`. `current_cast` always reads the innermost
    /// frame, which is the cast a nested site is actually part of.
    cast_stack: Vec<CastContext>,
    /// Actor whose turn `start_turn_for` has already run for, or `None`
    /// when the current initiative slot hasn't been opened yet.
    ///
    /// The latch exists because "the current actor's turn has started"
    /// and "the initiative index moved" are not the same event, and the
    /// engine has paths where only the second happens. Cleared by
    /// `advance_initiative` (the index moved, so nobody's turn has
    /// started yet) and set by `start_turn_for`; `ensure_turn_started`
    /// closes the gap immediately before a prompt is issued.
    ///
    /// Two paths need it. The **first actor of the encounter** never
    /// advances into their slot, so nothing else would ever open their
    /// turn. And an actor who **dies on their own turn** vacates the
    /// slot without an advance —
    /// `InitiativeTracker::remove_actor` deliberately lets the next
    /// actor slide into the vacated index — so that actor is prompted
    /// off a slot change that no `advance_initiative` accompanied.
    ///
    /// It also keeps the *idempotence* the old code got by construction:
    /// `process_stack` runs once per action, so an actor taking three
    /// actions in a turn must not have their resources reset between
    /// them. Matching ids is what suppresses that.
    turn_started_for: Option<usize>,
    /// Persistent magical areas — see `crate::engine::zones`. A `Vec`
    /// rather than a per-tile grid because there are never many (one or
    /// two in a busy fight) and every question the engine asks of the
    /// layer is "which zones cover this tile", which a short linear scan
    /// answers as fast as an index would while keeping each zone's
    /// identity, owner, and timer in one place.
    zones: Vec<Zone>,
    zone_id_next: usize,
    /// The "for the first time on a turn" ledger: `(zone id, actor id)`
    /// pairs that have already paid this turn's contact clause. Cleared
    /// wholesale by `start_turn_for`, which is exactly the RAW window —
    /// a creature shoved into a web on somebody else's turn triggers it,
    /// and shoving them back in on that same turn does not.
    zone_contacts_this_turn: std::collections::HashSet<(usize, usize)>,
    /// Map tiles a spell has retyped, and the ledger that hands them
    /// back — see `crate::engine::conjured_terrain`. Sibling to `zones`
    /// in every respect but one: a zone overlays the map and this
    /// *replaces* it, which is what lets a conjured wall block a line
    /// of sight when no combination of `ZoneEffect` fields can.
    conjured_terrain: Vec<ConjuredTerrain>,
    conjured_terrain_id_next: usize,
    /// The light the board has before anybody lights anything — see
    /// `crate::engine::lighting`. `BrightLight` by default, which is
    /// the fully-lit board every encounter behaved as before the
    /// lighting layer existed.
    ambient_light: AmbientLight,
    /// Everything currently shedding light: lit torches, Light
    /// cantrips, Daylight spheres.
    ///
    /// A `Vec` for the same reason `zones` is one — there are never
    /// many, and every question the layer is asked is "which of these
    /// reaches this tile", which a short linear scan answers as fast as
    /// an index would.
    light_sources: Vec<LightSource>,
    light_source_id_next: usize,
}

/// One frame of the in-flight spell-cast stack — the resolved identity
/// of the spell whose effects are being built right now.
///
/// Both fields come straight off the `Action` being executed:
/// `school` is its `school()` (None for non-spells and untagged spells)
/// and `level` is the `SpellSlot(n)` entry sniffed off its resolved
/// cost (0 for cantrips and slot-less actions). Consumers gate on both
/// — "an evocation spell of 1st level or higher", "a cantrip" — so the
/// frame carries exactly the two facts every school-keyed feature in
/// 5e keys off and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastContext {
    pub school: Option<SpellSchool>,
    pub level: u32,
    /// Whether this cast has already paid out its once-per-cast flat
    /// damage bonus (Empowered Evocation's +INT, Potent Spellcasting's
    /// +WIS).
    ///
    /// RAW's unit for both features is "one damage roll" — the spell,
    /// not the die and not the target. Most spells roll damage once, so
    /// for most of them the distinction never comes up; a handful roll
    /// two separate pools in one cast (Storm of Vengeance's thunder and
    /// lightning, Acid Arrow's impact and splash) and a couple roll
    /// fresh dice per target. Without a latch every one of those would
    /// collect the flat bonus once per roll.
    ///
    /// Latched on the frame rather than on the caster because the frame
    /// is exactly the lifetime the rule describes, and because nesting
    /// then behaves correctly for free: a spell that somehow triggers
    /// another spell pushes a second frame with its own unspent latch,
    /// which is what RAW would say if it had thought about it.
    ///
    /// The reroll half of the chokepoint (the Sorcerer's Empowered
    /// Spell metamagic) needs no latch — it self-consumes by clearing
    /// its own prime condition on first use.
    pub flat_damage_bonus_paid: bool,
    /// What the action opening this frame declared it can damage, from
    /// `Action::damage_types()`.
    ///
    /// The school axis answers "what kind of magic is this"; this one
    /// answers "what does it do to the target", and a family of 5e
    /// features keys off the second rather than the first — the
    /// Draconic Sorcerer's Elemental Affinity and the Wildfire Druid's
    /// Enhanced Bond both read "a spell that deals <type> damage", with
    /// no school anywhere in the wording.
    ///
    /// Empty for every action that declares nothing, which is the
    /// default on the trait and covers non-spells wholesale — so a gate
    /// reading this set fails closed on a weapon swing, a Move, and on
    /// any spell whose typing is decided at runtime (Chaos Bolt,
    /// Chromatic Orb) rather than declared up front. That is the honest
    /// answer for the runtime-typed ones: the frame is opened before
    /// the die that picks the type is rolled.
    pub damage_types: crate::engine::types::DamageTypeSet,
}

impl CastContext {
    /// True when this frame is a spell of `school` at 1st level or
    /// higher — the RAW shape of essentially every school-keyed
    /// subclass feature ("when you cast an abjuration spell of 1st
    /// level or higher…", "when you cast a wizard evocation spell…").
    pub fn is_leveled_spell_of(&self, school: SpellSchool) -> bool {
        self.school == Some(school) && self.level > 0
    }

    /// True when this frame is a cantrip of `school`. The complement of
    /// `is_leveled_spell_of` on the same school axis — Potent Cantrip
    /// is the mirror image of Empowered Evocation, one gating on level
    /// 0 and the other on level 1+.
    pub fn is_cantrip_of(&self, school: SpellSchool) -> bool {
        self.school == Some(school) && self.level == 0
    }

    /// True when this frame is a cantrip of *any* school — a spell with
    /// no slot cost. The school-agnostic gate Potent Cantrip needs
    /// ("your cantrips", not "your evocation cantrips").
    ///
    /// The `school.is_some()` leg is what separates a cantrip from a
    /// non-spell action: `Action::execute` opens a frame for every
    /// action it runs, so a weapon swing or a class feature also
    /// arrives here at level 0. Every cantrip in the engine carries a
    /// `school()` override and no other action at level 0 does, which is
    /// exactly the distinction — and
    /// `every_cantrip_declares_its_school` pins it so a new cantrip
    /// added without the override is caught rather than silently
    /// dropping out of every cantrip-gated feature.
    pub fn is_cantrip(&self) -> bool {
        self.school.is_some() && self.level == 0
    }

    /// True when the action that opened this frame declared it deals
    /// `damage_type`. See the `damage_types` field for why a spell whose
    /// typing is only decided at roll time answers `false`.
    pub fn deals(&self, damage_type: DamageType) -> bool {
        self.damage_types.contains(damage_type)
    }
}

/// How much a `FlatSpellDamageBonus` row is worth when its gate passes.
///
/// Two shapes, because RAW writes these two ways and no third: a
/// spellcasting-ability modifier off the holder, or a die.
#[derive(Debug, Clone, Copy)]
enum FlatBonusAmount {
    /// The caster's modifier in this ability, floored at 0.
    Ability(AbilityScoreType),
    /// A roll, made fresh each time the row pays out.
    Die(Dice),
}

/// One row of the flat-bonus-on-a-spell's-damage-roll cohort.
///
/// `applies` gets the board, the in-flight cast frame and the caster —
/// everything a RAW gate on this family has ever needed. Most rows read
/// only the frame and the caster's feature tags; the one that reads the
/// board is Enhanced Bond, whose gate is "your wildfire spirit is
/// summoned and within 60 feet of you".
struct FlatSpellDamageBonus {
    /// Prefixed to the log line when the row pays out. Lowercase, the
    /// feature's RAW name — the log reads `"  elemental affinity: Sorc
    /// adds +3"`.
    label: &'static str,
    applies: fn(&EncounterInstance, &CastContext, &ActorInstance) -> bool,
    amount: FlatBonusAmount,
}

/// RAW's 60 ft between the Wildfire Druid and their spirit, in tiles on
/// the 2.5 ft grid. Both halves of Enhanced Bond measure against it.
pub const ENHANCED_BOND_REACH_TILES: isize = 24;

/// The leash between a Drakewarden and their drake, in tiles on the
/// 2.5 ft grid — 30 ft, half of Enhanced Bond's.
///
/// Shorter than its sibling because the two bonds pay for different
/// things. Enhanced Bond buys a die on a *spell*, and a druid casting
/// spells is already standing back; the 60 ft is generous because it
/// has to be. Bond of Fang and Scale buys a die on a *weapon swing*, so
/// the ranger is in the front rank already and the drake has no excuse
/// not to be. See `BOND_OF_FANG_AND_SCALE_TAG` for why there is a leash
/// at all when RAW has none.
pub const FANG_AND_SCALE_REACH_TILES: isize = 12;

/// Features that add a flat amount to **one damage roll of a spell**.
///
/// The unit is the cast, not the die and not the target — see
/// `CastContext::flat_damage_bonus_paid`, which latches the whole
/// cohort after its first payout so a spell rolling two damage pools
/// pays once.
///
/// The rows split cleanly on which axis of the cast they read. Two gate
/// on the *school* (Empowered Evocation) or the *tier* (Potent
/// Spellcasting); two gate on what the spell **does** — the damage type
/// it declared — which is the axis `CastContext::damage_types` exists
/// for. Nothing here reads more than one axis, and a future row that
/// needs a second one adds a field to the frame rather than a
/// parameter to this signature.
///
/// Additive rather than exclusive, deliberately: the four belong to
/// four different classes, so no legal build holds two and the sum is
/// never a stack in practice. A hypothetical multiclass that did hold
/// two would collect both, which is what RAW says when two features
/// with no interaction clause both trigger.
static FLAT_SPELL_DAMAGE_BONUSES: &[FlatSpellDamageBonus] = &[
    // 5e Evocation Wizard **Empowered Evocation** (subclass lv10): add
    // the caster's INT modifier to one damage roll of a wizard
    // evocation spell.
    //
    // Gated on the cast stack rather than on anything the caster is
    // holding, so it is inert outside a cast and on every non-evocation
    // spell — a Fireball from an evoker gets the bonus, the same
    // evoker's Vampiric Touch (necromancy) does not. Cantrips qualify:
    // RAW says "any wizard evocation spell", with no level floor, which
    // is what makes the feature a real cantrip-scaling boost.
    FlatSpellDamageBonus {
        label: "empowered evocation",
        applies: |_e, cast, caster| {
            cast.school == Some(SpellSchool::Evocation)
                && caster.has_passive_feature(
                    crate::actions::class_features::EMPOWERED_EVOCATION_TAG,
                )
        },
        amount: FlatBonusAmount::Ability(AbilityScoreType::Intelligence),
    },
    // 5e **Potent Spellcasting**, which the Knowledge, Light and Nature
    // Domain Clerics all get at level 8 with the same wording: "you add
    // your Wisdom modifier to the damage you deal with any cleric
    // cantrip". The cantrip-tier sibling of the Warlock's Agonizing
    // Blast and of Empowered Evocation above.
    //
    // `is_cantrip` is both halves of the gate in one place: level 0
    // makes it a cantrip bonus rather than a blanket damage buff (a
    // Knowledge Cleric's Guiding Bolt and Flame Strike get nothing),
    // and its `school.is_some()` leg separates a cantrip from the
    // level-0 frame `Action::execute` opens for every weapon swing and
    // Move. Without the second leg the cleric's mace would quietly
    // carry the feature too.
    FlatSpellDamageBonus {
        label: "potent spellcasting",
        applies: |_e, cast, caster| {
            cast.is_cantrip()
                && caster.has_passive_feature(
                    crate::actions::class_features::POTENT_SPELLCASTING_TAG,
                )
        },
        amount: FlatBonusAmount::Ability(AbilityScoreType::Wisdom),
    },
    // 5e Draconic Bloodline Sorcerer **Elemental Affinity** (subclass
    // lv6, damage half): "when you cast a spell that deals damage of
    // the type associated with your draconic ancestry, you can add your
    // Charisma modifier to one damage roll of that spell."
    //
    // The first row on this cohort to gate on the damage type rather
    // than on the school, and the reason the cast frame carries a
    // `DamageTypeSet` at all. The ancestry is fire, which is what
    // `has_draconic_resilience` already picked for the same template:
    // the two halves of a draconic sorcerer's lv6 are "you resist your
    // element" and "your element hits harder", and shipping them on
    // different elements would be a bug wearing a feature's clothes.
    //
    // RAW's second half — spend a sorcery point to add the same
    // modifier to a spell's *resistance-piercing*, i.e. Elemental
    // Affinity's "resistance to that damage type for 1 hour" clause —
    // is left out: the engine has no per-caster elemental-resistance
    // grant lane that a spell cast could open, and the damage half is
    // the one that reads at a site the engine already has.
    FlatSpellDamageBonus {
        label: "elemental affinity",
        applies: |_e, cast, caster| {
            cast.deals(DamageType::Fire)
                && caster.has_passive_feature(
                    crate::actions::class_features::ELEMENTAL_AFFINITY_TAG,
                )
        },
        amount: FlatBonusAmount::Ability(AbilityScoreType::Charisma),
    },
    // 5e Circle of Wildfire Druid **Enhanced Bond** (subclass lv6,
    // damage half): "while your spirit is summoned, ... when you cast a
    // spell that deals fire damage ..., roll a d8 and add the number
    // rolled to one damage roll of that spell."
    //
    // The only row on the cohort whose gate reads the board: the bond
    // is with a creature, and a druid whose spirit is dead or across
    // the map gets nothing. That is the whole shape of the subclass —
    // every Wildfire feature is worth what the spirit's position makes
    // it worth — so collapsing the range check would be collapsing the
    // feature.
    //
    // A die rather than a modifier, which is also the only row that
    // rolls. RAW's healing half of the same sentence lives at the slot-
    // heal chokepoint (`slot_heal_effects`) for the reason the two
    // sites exist separately: one adds to damage, the other to hit
    // points, and neither knows about the other.
    FlatSpellDamageBonus {
        label: "enhanced bond",
        applies: |encounter, cast, caster| {
            cast.deals(DamageType::Fire) && encounter.wildfire_bond_active(caster)
        },
        amount: FlatBonusAmount::Die(Dice::new(1, 8)),
    },
    // 5e Artillerist Artificer **Arcane Firearm** (subclass lv5, TCE):
    // "when you cast an artificer spell through the firearm, roll a d8,
    // and you gain a bonus to one of the spell's damage rolls equal to
    // the number rolled."
    //
    // The second die on this cohort and the first row with no gate at
    // all beyond "is this a spell". Enhanced Bond directly above rolls
    // the same d8 and asks where the druid's spirit is standing; this
    // asks nothing, which is what makes the Artillerist the reliable
    // one of the four artificers and the Wildfire druid the positional
    // one.
    //
    // `school().is_some()` is the engine's marker for "this action is a
    // spell" — the same leg `is_cantrip` uses to tell a cantrip from
    // the level-0 frame `Action::execute` opens for every weapon swing
    // and Move. Without it the artificer's shortsword would carry the
    // die too. RAW narrows further to the *artificer* spell list, which
    // on a chassis carrying only artificer spells is the same set.
    FlatSpellDamageBonus {
        label: "arcane firearm",
        applies: |_e, cast, caster| {
            cast.school.is_some()
                && caster
                    .has_passive_feature(crate::actions::class_features::ARCANE_FIREARM_TAG)
        },
        amount: FlatBonusAmount::Die(Dice::new(1, 8)),
    },
    // 5e Alchemist Artificer **Alchemical Savant** (subclass lv5, TCE):
    // "whenever you cast a spell using your alchemist's supplies, you
    // can add your Intelligence modifier to one roll of the spell that
    // restores hit points or deals acid, fire, necrotic, or poison
    // damage."
    //
    // The widest damage-type gate on the cohort — four types against
    // Elemental Affinity's one — which is what makes the baseline
    // artificer's Tasha's Caustic Brew and Create Bonfire stop being
    // filler on this chassis. What it deliberately does not reach is
    // the force / lightning / psychic lane, so the Alchemist's answer
    // to a fire-immune enemy is a different spell rather than a bigger
    // one.
    //
    // The healing half of RAW's sentence is not here: the heal
    // chokepoint takes no cast frame, so it would be a second site
    // keyed off the same tag with no shared body between them. See
    // `ALCHEMICAL_SAVANT_TAG`.
    FlatSpellDamageBonus {
        label: "alchemical savant",
        applies: |_e, cast, caster| {
            [
                DamageType::Acid,
                DamageType::Fire,
                DamageType::Necrotic,
                DamageType::Poison,
            ]
            .iter()
            .any(|&t| cast.deals(t))
                && caster
                    .has_passive_feature(crate::actions::class_features::ALCHEMICAL_SAVANT_TAG)
        },
        amount: FlatBonusAmount::Ability(AbilityScoreType::Intelligence),
    },
];

/// Apply `mode_on_mismatch` to `current` when `holder` carries `condition`
/// AND the actor id its `link` field points at is set to someone OTHER
/// than `counterparty`. Centralizes the "flag-plus-link locked onto the
/// wrong opponent" pattern in `compute_attack_mode`:
/// * Dueled + its back-link ≠ target → attacker disadvantage,
/// * Goaded + its back-link ≠ target → attacker disadvantage,
/// * Distracted + its back-link ≠ attacker → target-side advantage to
///   the *other* attackers (the tagger themselves gets no benefit).
///
/// Returns the (possibly combined) mode so the call sites stay terse:
/// `mode = focus_link_mode(mode, holder, counterparty, cond, m);`.
/// Adding a future "flag plus link" rider (Sentinel pinning, a vow
/// against a specific foe, etc.) becomes a one-liner instead of a
/// re-inlined `has_condition + link.is_some_and(!=)` block.
///
/// The condition alone identifies the link — `linked_by` looks it up in
/// the keyed table and already returns `None` unless the flag is held —
/// so there is no accessor to pass in and no way for a call site to pair
/// a condition with the wrong one.
/// Back-linked conditions that impose disadvantage on their holder's
/// attacks against anyone *other than* the creature the link points at.
///
/// Three features arrive at the same shape from different directions —
/// Compelled Duel (a spell), the Battle Master's Goading Attack (a
/// maneuver), and the Ancestral Guardian's Ancestral Protectors (a
/// passive mark) — and the mechanic is identical in all three: the
/// holder can swing at their counterparty freely and pays for swinging
/// at anyone else. Listing them means a fourth is a row rather than
/// another near-identical call in `compute_attack_mode`.
///
/// Note that this is only ever half of a feature. Compelled Duel adds a
/// WIS save to leave, Goading Attack is a one-swing prime, and
/// Ancestral Protectors also halves the damage the holder deals
/// elsewhere. The disadvantage is the part they share, and it's the
/// part that belongs in one place.
const FOCUS_LINK_DISADVANTAGES: &[Condition] = &[
    Condition::Dueled,
    Condition::Goaded,
    Condition::AncestrallyHaunted,
];

fn focus_link_mode(
    holder: &ActorInstance,
    counterparty: usize,
    condition: Condition,
    mode_on_mismatch: RollMode,
) -> RollMode {
    if holder
        .linked_by(condition)
        .is_some_and(|linked| linked != counterparty)
    {
        mode_on_mismatch
    } else {
        RollMode::Normal
    }
}

/// Positive-polarity sibling of `focus_link_mode`. Apply `mode_on_match`
/// to `current` when `holder` carries `condition` AND the actor id its
/// `link` field points at IS `counterparty`. Centralizes the "flag-plus-
/// link locked onto THIS opponent" pattern in `compute_attack_mode`:
/// * Sworn + its back-link == attacker → target-side advantage for the
///   swearing paladin only (Vengeance Paladin Vow of Enmity, lv3
///   subclass: the paladin who swore the vow gets advantage on attack
///   rolls against the sworn quarry; non-sworn allies get no benefit).
///
/// Returns the mode this rider contributes, or `Normal` when it does
/// not apply, so the call site reads
/// `tally.add(matched_link_mode(holder, counterparty, cond, m));` and
/// the tally decides what a contribution is worth.
/// Adding a future "buff against this specific foe" rider (Favored Foe
/// damage rider, Mark of Vendetta, etc.) becomes a one-liner instead of a
/// re-inlined `has_condition + link == Some(counterparty)` block.
fn matched_link_mode(
    holder: &ActorInstance,
    counterparty: usize,
    condition: Condition,
    mode_on_match: RollMode,
) -> RollMode {
    if holder.linked_by(condition) == Some(counterparty) {
        mode_on_match
    } else {
        RollMode::Normal
    }
}

impl EncounterInstance {
    pub fn messages(&self) -> &Vec<String> {
        &self.messages
    }

    pub fn log(&mut self, msg: impl Into<String>) {
        self.messages.push(msg.into());
    }

    /// Owned name for an actor, or the empty string if the id has no live
    /// actor. Centralizes the `self.actors.get(&id).map(|a|
    /// a.name().to_string()).unwrap_or_default()` chain that fired at ~30
    /// call sites — log formatters in spells / item-actions / engine hooks
    /// uniformly want "name the actor if it's still alive, otherwise blank
    /// the substitution out." Returns String (not &str) because every
    /// caller wants a stable owned name they can hold across mutable
    /// borrows on `self.actors` later in the function.
    pub fn actor_name(&self, id: usize) -> String {
        self.actors
            .get(&id)
            .map(|a| a.name().to_string())
            .unwrap_or_default()
    }

    /// True when `actor_id` should be billed a bonus action, rather than
    /// an Action, for reaching into their pack right now — the Thief
    /// Rogue's **Fast Hands**.
    ///
    /// Both halves of the condition matter. The tag says the rogue is
    /// allowed to make the trade; the remaining-bonus-action check says
    /// there is still something to trade with. Read only from
    /// `item_actions::item_use_cost`, which documents the lane this
    /// covers and why the price is resolved per call instead of baked
    /// into the item.
    pub fn handles_items_as_a_bonus_action(&self, actor_id: usize) -> bool {
        use crate::actions::class_features::FAST_HANDS_TAG;
        self.actors.get(&actor_id).is_some_and(|a| {
            a.has_passive_feature(FAST_HANDS_TAG)
                && a.can_consume_resource(crate::engine::side_effects::Resource::BonusAction)
        })
    }

    /// True iff the actor with `id` exists AND is effectively immune to
    /// `c` (template, dynamic, or item-granted). False for an unknown id
    /// (a vanished target can't take the install anyway). Centralizes
    /// the `self.actors.get(&id).is_some_and(|t|
    /// t.effectively_immune_to_condition(c))` chain — used by the
    /// `BurstSaveConditionItem` / `SingleSaveConditionItem` / Wand of
    /// Polymorph save-roll skip and any other site that needs to short-
    /// circuit a save against a target whose install can't land.
    pub fn actor_immune_to_condition(&self, id: usize, c: Condition) -> bool {
        self.actors
            .get(&id)
            .is_some_and(|t| t.effectively_immune_to_condition(c))
    }

    /// Stamp the swinger's two-weapon ledger if `aei` is the swing RAW
    /// asks for: "the Attack action … with a light melee weapon that
    /// you're holding in one hand".
    ///
    /// Both halves are checked here, and both are facts about the
    /// action rather than about the attack it resolves into — which is
    /// why the write lives at this chokepoint and not in
    /// `engine::attack`. `AttackParams` knows the die, the reach and
    /// the damage type; it does not know whether the swing cost an
    /// Action or which weapon on the sheet produced it, and both of
    /// those are load-bearing. A bonus-action shortbow shot and an
    /// opportunity attack are melee-or-not by the same resolver and
    /// neither opens a second swing.
    ///
    /// Runs before `execute` rather than after, so the ledger is
    /// already stamped for anything the swing itself enqueues. Nothing
    /// today reads it that early — the off-hand swing is declared on a
    /// later pass through the stack — but the ordering costs nothing
    /// and the other order would be a latent trap.
    fn mark_two_weapon_opening(&mut self, aei: &ActionExecutionInfo) {
        use crate::engine::side_effects::Resource;
        if !aei.action().is_light_melee_weapon() {
            return;
        }
        if !aei.cost(self).contains(&Resource::Action) {
            return;
        }
        if let Some(caster) = self.actors.get_mut(&aei.caster_id()) {
            caster.mark_light_weapon_swing_this_turn();
        }
    }

    /// Stamp the swinger's once-per-turn off-hand ledger after an
    /// off-hand swing resolves.
    ///
    /// The ledger is what enforces 5e **Nick**'s *"you can make this
    /// extra attack only once per turn"* — see
    /// `two_weapon::OffHandAttack::cost`, which reads it back. Every
    /// off-hand swing stamps it, not only the free ones: RAW's Light
    /// property grants one extra attack per turn whatever it is paid
    /// for with, so a wielder who spent a bonus action on the first
    /// off-hand swing has already used the opening Nick would have
    /// discounted.
    ///
    /// Runs *after* `execute` rather than before, and that ordering is
    /// the whole reason this is a separate hook from
    /// `mark_two_weapon_opening` directly above it. `Action::execute`
    /// asks for `cost` twice — once through `validate_input` and once
    /// to build the `ConsumeResource` — and a ledger stamped before
    /// those two calls would answer them differently: the swing would
    /// be waved through as free and then billed a bonus action.
    ///
    /// Whether the swing was real is the caller's question, asked
    /// before `execute` ran; see the call site.
    fn mark_offhand_swing(&mut self, swinger: usize) {
        if let Some(caster) = self.actors.get_mut(&swinger) {
            caster.mark_once_per_turn_used(crate::engine::mastery::NICK_TAG);
        }
    }

    /// Logs a play-by-play line for an action that consumes the
    /// action-economy (Action / BonusAction / Reaction / LegendaryAction).
    /// Movement and free actions are intentionally excluded — the AI takes
    /// many move-steps per turn and they'd drown out useful events.
    fn log_action_use(&mut self, aei: &ActionExecutionInfo) {
        // Pick the most prominent action-economy cost for the log tag.
        // SpellSlot/Movement are secondary; we tag by whichever main slot
        // got consumed (Action / BonusAction / Reaction / Legendary).
        use crate::engine::side_effects::Resource;
        let costs = aei.cost(self);
        let Some(slot) = costs.iter().find_map(|r| match r {
            Resource::Action => Some("action"),
            Resource::BonusAction => Some("bonus action"),
            Resource::Reaction => Some("reaction"),
            Resource::LegendaryAction => Some("legendary"),
            _ => None,
        }) else {
            return;
        };

        let caster_name = self
            .actors
            .get(&aei.caster_id())
            .map(|a| a.name().to_string())
            .unwrap_or_else(|| format!("actor#{}", aei.caster_id()));
        let action_name = aei.action().name();
        let target_suffix = aei
            .target_ids()
            .and_then(|ids| ids.first().copied())
            .and_then(|id| self.actors.get(&id))
            .map(|a| format!(" on {}", a.name()))
            .unwrap_or_default();

        self.log(format!(
            "[{}] {} uses {}{}",
            slot, caster_name, action_name, target_suffix
        ));
    }

    pub fn next_actor_id(&mut self) -> usize {
        let next_actor_id = self.actor_id_next;
        self.actor_id_next += 1;
        next_actor_id
    }

    /// Roll dice through the encounter's seedable roller. Use this from
    /// action side-effects so reproducibility-by-seed is preserved.
    pub fn roll(&mut self, dice: &Dice) -> u32 {
        self.roller.roll(dice)
    }

    /// 5e Fighting Style: **Great Weapon Fighting** — roll a weapon
    /// damage bundle, and (if `apply_gwf_reroll` is set) reroll any
    /// die that comes up 1 or 2 once, taking the new value even if it
    /// comes up 1 or 2 again per RAW ("but you must use the new roll").
    /// The reroll routes through the same seedable roller so
    /// reproducibility-by-seed is preserved.
    ///
    /// Callers gate `apply_gwf_reroll` on the attacker's
    /// `has_great_weapon_fighting()` flag AND `is_melee` — the RAW
    /// "two-handed or versatile-two-handed melee weapon" gate collapses
    /// to "melee weapon attack" since the engine doesn't track
    /// weapon-hand-usage (same shape as Dueling's gate collapse).
    ///
    /// The GWF-off fast path (the vast majority of damage rolls) is a
    /// single delegated `self.roll(&dice)` call, keeping the common case
    /// as cheap as the un-rerolled path was before this helper existed.
    pub fn roll_weapon_damage_dice(&mut self, dice: Dice, apply_gwf_reroll: bool) -> u32 {
        if !apply_gwf_reroll || dice.count == 0 || dice.faces == 0 {
            return self.roll(&dice);
        }
        let one_die = Dice::new(1, dice.faces);
        let mut sum: u32 = 0;
        for _ in 0..dice.count {
            let mut roll = self.roll(&one_die);
            if roll <= 2 {
                roll = self.roll(&one_die);
            }
            sum = sum.saturating_add(roll);
        }
        sum
    }

    /// Roll `count` individual dice of `faces` faces, applying the
    /// 5e Sorcerer **Empowered Spell** metamagic if the caster has the
    /// `EmpoweredSpelling` condition primed. Returns the rolled values
    /// (caller sums / logs / per-die-applies as needed).
    ///
    /// Empowered Spell RAW: when the caster rolls damage for a spell,
    /// they may reroll up to CHA-mod of the damage dice and must take
    /// the new rolls. We approximate the "should I reroll?" tactic with
    /// the obvious answer — reroll any die that came up at 1 or 2
    /// (the reroll's expected value is `(faces+1)/2`, which beats 2 for
    /// every die face count we use, d4 through d12). The condition is
    /// consumed on any non-trivial spell-damage roll path the caster
    /// takes through this helper, so a single prime fuels exactly one
    /// damage roll.
    ///
    /// Call this from the spell `side_effects` lane in place of
    /// `roll(&Dice::new(count, faces))` whenever the spell's damage roll
    /// should honor Empowered Spell. Non-empowered casters pay no extra
    /// cost (a single dice roll path falls through to the same total).
    pub fn roll_empowered(&mut self, caster_id: usize, count: u32, faces: u32) -> Vec<u32> {
        use crate::engine::types::AbilityScoreType;
        let mut values: Vec<u32> = (0..count)
            .map(|_| self.roll(&Dice::new(1, faces)))
            .collect();
        let empowered = self
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_condition(Condition::EmpoweredSpelling));
        if !empowered {
            return values;
        }
        // Reroll up to CHA-mod (min 1) dice that came up at 1 or 2.
        // Tied low rolls break by index — deterministic ordering keeps
        // seed reproducibility intact.
        let cha_mod = self
            .actors
            .get(&caster_id)
            .map(|a| a.ability_modifier(AbilityScoreType::Charisma).max(1))
            .unwrap_or(1) as usize;
        let mut candidates: Vec<usize> = (0..values.len())
            .filter(|&i| values[i] <= 2)
            .collect();
        candidates.sort_by_key(|&i| (values[i], i));
        candidates.truncate(cha_mod);
        let reroll_count = candidates.len();
        for i in candidates {
            values[i] = self.roll(&Dice::new(1, faces));
        }
        if reroll_count > 0 {
            self.log(format!(
                "  empowered spell: rerolled {} low {}-dice",
                reroll_count,
                Dice::new(1, faces)
            ));
        }
        // Consume the prime so the metamagic is one-shot per spell-damage
        // roll. RAW: "you can reroll a number of the damage dice" — the
        // spell is the unit, so a multi-roll spell still consumes once.
        if let Some(caster) = self.actors.get_mut(&caster_id) {
            caster.remove_condition(Condition::EmpoweredSpelling);
        }
        values
    }

    /// Sum variant of `roll_empowered`, plus the flat-bonus half of the
    /// spell-damage roll: the 5e Evocation Wizard **Empowered Evocation**
    /// (subclass lv10) adds the caster's Intelligence modifier to one
    /// damage roll of a wizard evocation spell.
    ///
    /// The flat bonus lives here rather than in `roll_empowered` because
    /// it is a property of the *roll total*, not of any individual die —
    /// per-die callers (a chain-lightning arc that applies each die
    /// separately, say) would otherwise multiply it. RAW's "one damage
    /// roll" is enforced naturally by the chokepoint's shape: burst
    /// spells roll once and share the value across the blast, so the
    /// bonus lands once per cast rather than once per target.
    ///
    /// Stacks with Empowered Spell (the sorcerer metamagic inside
    /// `roll_empowered`, which rerolls low dice) — different features,
    /// different classes, neither aware of the other.
    pub fn roll_empowered_sum(&mut self, caster_id: usize, count: u32, faces: u32) -> u32 {
        let base: u32 = self.spell_damage_pool(caster_id, count, faces).iter().sum();
        self.with_flat_spell_damage_bonus(caster_id, base)
    }

    /// `roll_empowered_sum` for a spell whose dice **explode**: RAW's
    /// *"if you roll an 8 on a d8 for this spell, you can roll another
    /// d8, and add it to the damage"*, capped at `max_extra` additional
    /// dice for the whole cast.
    ///
    /// Sorcerous Burst (SRD 5.2) is the spell the lane exists for, and
    /// its cap is the caster's spellcasting ability modifier. The
    /// exploded dice are themselves dice of the spell, so a fresh
    /// maximum on one of *them* buys another roll — the budget, not the
    /// chain, is what stops it.
    ///
    /// Layered on top of `spell_damage_pool` rather than beside it so
    /// the explosion sees the pool everything else in the engine sees:
    /// Empowered Spell has already rerolled the low dice by the time
    /// the maxima are counted, and an Overchanneled pool is entirely
    /// maxima and therefore explodes to the cap. The extra dice are
    /// plain rolls — RAW's clause is about the number rolled, and
    /// neither metamagic re-reaches a die it has already paid for.
    pub fn roll_exploding_spell_damage(
        &mut self,
        caster_id: usize,
        count: u32,
        faces: u32,
        max_extra: u32,
    ) -> u32 {
        let pool = self.spell_damage_pool(caster_id, count, faces);
        let mut total: u32 = pool.iter().sum();
        // Dice still owed an explosion. Counting rather than recursing
        // keeps the walk flat and the RNG draw order deterministic,
        // which the seeded runs need.
        let mut owed = pool.iter().filter(|&&v| v == faces).count() as u32;
        let mut spent = 0;
        while owed > 0 && spent < max_extra {
            owed -= 1;
            spent += 1;
            let value = self.roll(&Dice::new(1, faces));
            total = total.saturating_add(value);
            if value == faces {
                owed += 1;
            }
        }
        if spent > 0 {
            self.log(format!(
                "  exploding dice: +{} d{} (cap {})",
                spent, faces, max_extra
            ));
        }
        self.with_flat_spell_damage_bonus(caster_id, total)
    }

    /// The dice one spell-damage roll actually rolls, before anything
    /// is done with the total.
    ///
    /// Split out of `roll_empowered_sum` so a caller that needs the
    /// individual faces — the exploding lane above, which has to know
    /// how many of them came up maximum — reads the same pool the sum
    /// does instead of rolling its own beside it. A second pool would
    /// have been the wrong shape twice over: it would double the RNG
    /// draws for one cast, and it would consume neither of the primes
    /// resolved here, so an Empowered or Overchanneled Sorcerous Burst
    /// would have exploded off dice the caster never rolled.
    fn spell_damage_pool(&mut self, caster_id: usize, count: u32, faces: u32) -> Vec<u32> {
        let dice = Dice::new(count, faces);
        // Overchannel replaces the roll outright rather than modifying
        // it, so it is resolved first — there are no dice left for
        // Empowered Spell's reroll to improve once every die is showing
        // its top face, and calling through anyway would burn that
        // separate prime for nothing.
        match self.consume_overchannel(caster_id, dice) {
            Some(_) => vec![faces; count as usize],
            None => self.roll_empowered(caster_id, count, faces),
        }
    }

    /// Add the once-per-cast flat spell-damage bonus to a rolled total.
    ///
    /// The `FLAT_SPELL_DAMAGE_BONUSES` cohort hangs off this
    /// chokepoint, and its rows are deliberately additive rather than
    /// exclusive: they belong to different classes reading different
    /// stats on different halves of the spell list, so no legal build
    /// holds two and the sum is never a stack in practice. Each row
    /// gates itself on the in-flight cast.
    ///
    /// All of them are once per *cast*, not once per roll — see
    /// `CastContext::flat_damage_bonus_paid`. The latch is claimed
    /// before any is computed so a spell that rolls two damage pools
    /// (Storm of Vengeance, Acid Arrow) pays the bonus on the first
    /// and not the second, matching RAW's "one damage roll".
    fn with_flat_spell_damage_bonus(&mut self, caster_id: usize, base: u32) -> u32 {
        if !self.claim_flat_damage_bonus() {
            return base;
        }
        base + self.flat_spell_damage_bonus(caster_id)
    }

    /// Claim this cast's once-per-cast flat damage bonus, returning
    /// `true` the first time and `false` on every later call within the
    /// same cast frame.
    ///
    /// Returns `true` outside any cast frame so a non-spell damage roll
    /// that reaches `roll_empowered_sum` isn't silently gated — every
    /// row behind the latch checks the frame itself and returns 0 there
    /// anyway, so the permissive default costs nothing and keeps the
    /// latch from becoming a second place that decides what counts as a
    /// spell.
    fn claim_flat_damage_bonus(&mut self) -> bool {
        match self.cast_stack.last_mut() {
            Some(frame) => !std::mem::replace(&mut frame.flat_damage_bonus_paid, true),
            None => true,
        }
    }

    /// True when `caster` holds Enhanced Bond and their wildfire spirit
    /// is on the board, alive, and within the RAW 60 ft — the shared
    /// gate for both halves of the Circle of Wildfire Druid's lv6
    /// feature (the damage row on `FLAT_SPELL_DAMAGE_BONUSES` and the
    /// heal rider at `slot_heal_effects`).
    ///
    /// The spirit is identified by the passive `WILDFIRE_SPIRIT_TAG` it
    /// carries on its own template rather than by name or by a back-link
    /// on the druid. A tag survives the two things a link would not: a
    /// second Wildfire druid on the same team (each finds a spirit, and
    /// RAW does not care whose), and the spirit dying and being
    /// re-summoned (a link would dangle at the old id; the tag search
    /// simply finds the new body). It also costs nothing to keep
    /// correct, which a link would not — every despawn path would have
    /// to remember to clear it.
    ///
    /// Takes `&ActorInstance` rather than an id because every caller
    /// already has the borrow in hand, and because the caster's identity
    /// is not needed: the search is "an ally within reach with the tag",
    /// and the druid does not carry the tag themselves.
    pub fn wildfire_bond_active(&self, caster: &ActorInstance) -> bool {
        caster.has_passive_feature(crate::actions::class_features::ENHANCED_BOND_TAG)
            && self.friendly_beacon_within(
                caster,
                crate::actions::class_features::WILDFIRE_SPIRIT_TAG,
                ENHANCED_BOND_REACH_TILES,
            )
    }

    /// True if some live creature on `subject`'s team, other than
    /// `subject` itself, carries the passive feature `beacon` and stands
    /// within `tiles` footprint-Chebyshev of them.
    ///
    /// The shared read behind both summon-anchored subclass features on
    /// the roster: the Wildfire Druid's Enhanced Bond ("while your
    /// spirit is within 60 feet") and the Fathomless Warlock's Guardian
    /// Coil ("within 10 feet of your tentacle"). Both ask the same
    /// question of the board — is a particular kind of summoned body
    /// close enough to this creature — and both identify the summon by
    /// a tag on its own template rather than by a back-link from the
    /// summoner, for the reasons `WILDFIRE_SPIRIT_TAG` sets out.
    ///
    /// Takes `&ActorInstance` rather than an id because both callers
    /// already hold the borrow, and because the subject's identity is
    /// not needed beyond its team and footprint.
    pub fn friendly_beacon_within(
        &self,
        subject: &ActorInstance,
        beacon: &'static str,
        tiles: isize,
    ) -> bool {
        let subject_tiles = get_tiles_from_size(subject.size());
        self.actors.values().any(|a| {
            // "other than `subject` itself", which this used to promise
            // and not do. It cost nothing while the only caller was
            // Enhanced Bond — a druid does not carry the wildfire
            // spirit's tag — but the tag-sharing summons do: the
            // Drakewarden carries `DRAKE_COMPANION_TAG` as their summon
            // charge, so a ranger with no drake on the board was
            // standing within 0 ft of a beacon and collecting the die
            // for free. Identity by address rather than by id because
            // that is what a `&ActorInstance` is; the map's values are
            // the only `ActorInstance`s alive, so the comparison is
            // exact.
            !std::ptr::eq(a, subject)
                && a.team() == subject.team()
                && a.is_combat_active()
                && a.has_passive_feature(beacon)
                && footprint_chebyshev(
                    subject.location(),
                    subject_tiles,
                    a.location(),
                    get_tiles_from_size(a.size()),
                ) <= tiles
        })
    }

    /// Walk `FLAT_SPELL_DAMAGE_BONUSES` and return what the rows whose
    /// gates pass contribute to the in-flight cast's damage total.
    ///
    /// Split into a read pass and a roll pass because one row's amount
    /// is a die: the gates need `&self` (they read both the encounter
    /// and the caster), and rolling needs `&mut self`. Collecting the
    /// hits first means a row can look at the board without the
    /// borrow checker forcing every gate to be an actor-only predicate.
    ///
    /// Each row logs its own line when it pays out, so a player reading
    /// the log sees *which* feature moved the number rather than an
    /// unexplained delta between the dice and the total.
    fn flat_spell_damage_bonus(&mut self, caster_id: usize) -> u32 {
        let Some(cast) = self.current_cast() else {
            return 0;
        };
        let hits: Vec<&'static FlatSpellDamageBonus> = {
            let Some(caster) = self.actors.get(&caster_id) else {
                return 0;
            };
            FLAT_SPELL_DAMAGE_BONUSES
                .iter()
                .filter(|row| (row.applies)(self, &cast, caster))
                .collect()
        };
        let mut total = 0;
        for row in hits {
            let amount = match row.amount {
                // Floored at 0 so a negative-modifier holder can't
                // invert the feature into a damage penalty.
                FlatBonusAmount::Ability(ability) => self
                    .actors
                    .get(&caster_id)
                    .map(|c| c.ability_modifier(ability).max(0) as u32)
                    .unwrap_or(0),
                FlatBonusAmount::Die(dice) => self.roll(&dice),
            };
            if amount > 0 {
                let name = self.actor_name(caster_id);
                self.log(format!("  {}: {} adds +{}", row.label, name, amount));
                total += amount;
            }
        }
        total
    }

    /// 5e Evocation Wizard **Overchannel** (subclass lv14): if the caster
    /// has the prime up and the cast is a damaging spell of level 1-5,
    /// consume the prime, return `dice` at maximum instead of rolling,
    /// and latch the escalating backlash for the post-cast trigger to
    /// charge. Returns `None` — leaving the normal roll to happen —
    /// whenever any gate fails.
    ///
    /// The level window is RAW ("a wizard spell of 1st through 5th
    /// level") and is what keeps the feature from trivializing the
    /// evoker's level 6-9 slots; cantrips are excluded by the same
    /// clause. The prime is only consumed on a cast that can actually
    /// use it, so declaring Overchannel and then firing a cantrip leaves
    /// it up rather than wasting it — the same forgiving semantics the
    /// metamagic primes use.
    ///
    /// Zero-die pools (`count == 0`) are skipped too: they carry no
    /// damage to maximize, and burning the prime on one would be a pure
    /// loss.
    fn consume_overchannel(&mut self, caster_id: usize, dice: Dice) -> Option<u32> {
        const MAX_OVERCHANNEL_LEVEL: u32 = 5;
        if dice.count == 0 {
            return None;
        }
        let level = self.current_cast().map(|c| c.level).unwrap_or(0);
        if level == 0 || level > MAX_OVERCHANNEL_LEVEL {
            return None;
        }
        let caster = self.actors.get_mut(&caster_id)?;
        if !caster.has_condition(Condition::Overchanneling) {
            return None;
        }
        caster.remove_condition(Condition::Overchanneling);
        let backlash_dice = caster.note_overchannel_use();
        if backlash_dice > 0 {
            caster.set_overchannel_backlash_pending();
        }
        let maxed = dice.max_roll();
        let name = self.actor_name(caster_id);
        self.log(format!(
            "  overchannel: {} maximizes {} → {}",
            name, dice, maxed
        ));
        Some(maxed)
    }

    /// Post-cast half of Overchannel: charge the necrotic backlash the
    /// damage-roll site latched. RAW: "you take 2d12 necrotic damage for
    /// each level of the spell, immediately after you cast it", rising
    /// by 1d12 per level on each further use before a long rest.
    ///
    /// Emitted as a normal `DealDamage` side-effect rather than applied
    /// in place, so the backlash goes through the full damage pipeline —
    /// it can break the evoker's own concentration, drop them, and be
    /// logged like any other hit. That is the whole reason the feature is
    /// split across two phases instead of being resolved inside the dice
    /// helper.
    ///
    /// RAW adds "this damage ignores resistance and immunity", which the
    /// engine's damage pipeline has no lane for; the wizard chassis has
    /// neither against necrotic, so the approximation is invisible today
    /// and would only surface on a Necromancy-multiclass build.
    fn trigger_overchannel_backlash(
        &mut self,
        caster_id: usize,
        spell_level: u32,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let pending = self
            .actors
            .get_mut(&caster_id)
            .is_some_and(|a| a.take_overchannel_backlash_pending());
        if !pending || spell_level == 0 {
            return Vec::new();
        }
        let uses = self
            .actors
            .get(&caster_id)
            .map(|a| a.overchannel_uses())
            .unwrap_or(0);
        let dice = Dice::new(uses * spell_level, 12);
        let amount = self.roll(&dice);
        let name = self.actor_name(caster_id);
        self.log(format!(
            "  overchannel backlash: {} takes {}({}) necrotic",
            name, dice, amount
        ));
        vec![Box::new(crate::engine::side_effects::DealDamage {
            actor_id: caster_id,
            amount,
            damage_type: DamageType::Necrotic,
        })]
    }

    /// Roll a single d20 with advantage / disadvantage applied. `Advantage`
    /// rolls two d20s and takes the higher; `Disadvantage` takes the lower;
    /// `Normal` rolls once. All rolls advance the same seedable roller, so
    /// reproducibility is preserved.
    pub fn roll_d20_with_mode(&mut self, mode: RollMode) -> u32 {
        let d20 = Dice::new(1, 20);
        match mode {
            RollMode::Normal => self.roll(&d20),
            RollMode::Advantage => {
                let a = self.roll(&d20);
                let b = self.roll(&d20);
                a.max(b)
            }
            RollMode::Disadvantage => {
                let a = self.roll(&d20);
                let b = self.roll(&d20);
                a.min(b)
            }
        }
    }

    /// 5e Improved Critical: minimum d20 face that promotes a swing to
    /// a critical hit for `actor_id`. Defaults to 20 (RAW). Champion
    /// fighters drop to 19; Superior Critical (level 15) drops to 18.
    /// Single source of truth for the attack-resolution sites that
    /// previously each carried an `.actors.get(...).map(...).unwrap_or(20)`
    /// chain — fewer chains means one less place to forget the default
    /// when adding a new attack-roll path.
    pub fn crit_threshold(&self, actor_id: usize) -> i32 {
        self.actors
            .get(&actor_id)
            .map(|a| a.crit_threshold() as i32)
            .unwrap_or(20)
    }

    /// The crit threshold `attacker_id` uses **against this particular
    /// target** — their own `crit_threshold`, lowered by any expansion
    /// that is scoped to one victim rather than to the attacker.
    ///
    /// Champion's Improved Critical widens the attacker's crit range
    /// against everyone, which is what `crit_threshold` alone can
    /// express. **Hexblade's Curse** widens it against exactly one
    /// creature, and only for the hexblade who cursed them — a
    /// distinction the attacker-only accessor had no way to carry, so
    /// every attack-resolution site now asks this instead.
    ///
    /// The two combine by taking the lower face, matching how Superior
    /// Critical stacks onto Improved Critical: a hypothetical
    /// Champion/Hexblade swinging at their cursed quarry crits on 19
    /// from either source, not on 18 from both. Nothing in 5e adds crit
    /// ranges together, so `min` is the general rule rather than a
    /// special case for this pair.
    pub fn crit_threshold_against(&self, attacker_id: usize, target_id: usize) -> i32 {
        let base = self.crit_threshold(attacker_id);
        if self.hexblade_curse_holder(target_id) == Some(attacker_id) {
            // RAW: "your attack rolls against the cursed target score a
            // critical hit on a roll of 19 or 20."
            base.min(19)
        } else {
            base
        }
    }

    /// The hexblade whose **Hexblade's Curse** is currently on
    /// `target_id`, if any.
    ///
    /// Reads the `HexbladeCursed` back-link, which returns `None` unless
    /// the flag is still held — so a curse that timed out stops paying
    /// out without any of the three consumers checking the timer
    /// themselves. All three (the damage bonus, the crit range, the
    /// heal-on-drop) route through here so they can never disagree about
    /// whose curse is live.
    pub fn hexblade_curse_holder(&self, target_id: usize) -> Option<usize> {
        self.actors
            .get(&target_id)
            .and_then(|t| t.linked_by(Condition::HexbladeCursed))
    }

    /// Flat damage bonus `attacker_id` adds to a damage roll against
    /// `target_id` because they cursed them — their proficiency bonus,
    /// per RAW, or 0 when there is no curse between the two.
    ///
    /// Folded in at both attack-damage chokepoints (weapon swings in
    /// `engine::attack`, spell attacks in `spells::spell_attack_outcome`)
    /// rather than at one, because a hexblade's output is split across
    /// them: Eldritch Blast is a spell attack and the pact weapon is a
    /// weapon swing, and a bonus that only paid on one of the two would
    /// silently pick a build for the player.
    ///
    /// Save-for-damage spells are deliberately *not* covered. RAW's
    /// "damage rolls against that target" does include them, but the
    /// engine rolls save-spell damage once and applies it to every
    /// creature in the area, so there is no per-target damage roll to
    /// add to — folding the bonus in there would either pay it to every
    /// target in the burst or require splitting the shared roll. Neither
    /// is worth it for a subclass whose damage is overwhelmingly single-
    /// target attack rolls.
    pub fn curse_damage_bonus(&self, attacker_id: usize, target_id: usize) -> i32 {
        if self.hexblade_curse_holder(target_id) != Some(attacker_id) {
            return 0;
        }
        self.actors
            .get(&attacker_id)
            .map(|a| a.proficiency_bonus())
            .unwrap_or(0)
    }

    /// 5e Way of the Drunken Master Monk **Drunkard's Luck**: if `mode`
    /// is disadvantage and `actor_id` has a charge left, spend it and
    /// hand back `RollMode::Normal`.
    ///
    /// The first row on the roll-mode cancel lane, reached through
    /// `steady_the_d20` — which is what the two d20 chokepoints that
    /// can see the mode before the die lands and still hold `&mut`
    /// actually call. Anything not disadvantaged passes straight
    /// through, so the cost of the feature on every other actor in the
    /// game is one enum comparison.
    ///
    /// Returns the mode to roll under. See `DRUNKARDS_LUCK_TAG` for why
    /// it clears to Normal rather than combining an advantage in, and
    /// for why the charge is spent on the first disadvantaged roll
    /// rather than saved for a better one.
    pub fn cancel_disadvantage_with_luck(&mut self, actor_id: usize, mode: RollMode) -> RollMode {
        use crate::actions::class_features::DRUNKARDS_LUCK_TAG;
        if mode != RollMode::Disadvantage {
            return mode;
        }
        let lucky = self.actors.get(&actor_id).is_some_and(|a| {
            a.has_passive_feature(DRUNKARDS_LUCK_TAG) && a.feature_available(DRUNKARDS_LUCK_TAG)
        });
        if !lucky {
            return mode;
        }
        if let Some(a) = self.actors.get_mut(&actor_id) {
            a.spend_feature(DRUNKARDS_LUCK_TAG);
        }
        let name = self.actor_name(actor_id);
        self.log(format!(
            "  drunkard's luck: {} shrugs off the disadvantage.",
            name
        ));
        RollMode::Normal
    }

    /// How far a Clockwork Soul Sorcerer's **Restore Balance** reaches —
    /// 60 ft on the 2.5 ft grid, RAW.
    const RESTORE_BALANCE_REACH: isize = 24;

    /// 5e Clockwork Soul Sorcerer **Restore Balance** (subclass level
    /// 1): "when a creature you can see within 60 feet of you is about
    /// to roll a d20 with advantage or disadvantage, you can use your
    /// reaction to prevent the roll from being affected."
    ///
    /// The second row on the roll-mode lane, and the one that reaches
    /// past the roller's own sheet. Drunkard's Luck above answers the
    /// disadvantage on *your* die; this answers whatever is happening
    /// to somebody else's, from up to 60 ft away, on a reaction the
    /// engine spends.
    ///
    /// RAW's sentence is symmetric and takes no side — a Clockwork Soul
    /// may flatten an ally's disadvantage or an enemy's advantage with
    /// the same words. An engine that spends the reaction on the
    /// holder's behalf has to supply the judgement RAW leaves to the
    /// player, so the two useful readings are the only ones taken:
    ///
    ///   - a **hostile** rolling with **advantage** loses it, and
    ///   - an **ally** (or the sorcerer) rolling with **disadvantage**
    ///     is straightened out.
    ///
    /// The two cases RAW also permits — cancelling an ally's advantage
    /// or an enemy's disadvantage — are never what the holder wanted,
    /// and a lane that spends charges on them would be worse than not
    /// having the feature.
    ///
    /// Ties break on the lowest holder id so a seeded run reproduces,
    /// and the scan is ordered cheapest-gate-first so a table with no
    /// Clockwork Soul in it never measures a distance.
    pub fn cancel_mode_with_restore_balance(
        &mut self,
        roller_id: usize,
        mode: RollMode,
    ) -> RollMode {
        use crate::actions::class_features::RESTORE_BALANCE_TAG;
        if mode == RollMode::Normal {
            return mode;
        }
        let Some(roller_team) = self.actors.get(&roller_id).map(|a| a.team()) else {
            return mode;
        };
        let Some(holder) = self
            .actors
            .iter()
            .filter(|(_id, a)| {
                a.has_passive_feature(RESTORE_BALANCE_TAG)
                    && a.feature_available(RESTORE_BALANCE_TAG)
                    && a.is_combat_active()
                    && a.has_reaction()
                    // The judgement clause: flatten a hostile's
                    // advantage, or a friend's disadvantage. The
                    // sorcerer's own disadvantaged rolls fall in the
                    // second branch, since a creature shares a team
                    // with itself.
                    && match mode {
                        RollMode::Advantage => a.team() != roller_team,
                        RollMode::Disadvantage => a.team() == roller_team,
                        RollMode::Normal => false,
                    }
            })
            .map(|(id, _)| *id)
            .filter(|&id| {
                // A holder may steady their *own* disadvantaged roll —
                // RAW's "a creature you can see" includes nothing about
                // excluding yourself, and the reach check below would
                // trivially pass anyway.
                id == roller_id
                    || self
                        .footprint_distance(id, roller_id)
                        .is_some_and(|d| d <= Self::RESTORE_BALANCE_REACH)
            })
            .min()
        else {
            return mode;
        };
        if let Some(a) = self.actors.get_mut(&holder) {
            a.spend_feature(RESTORE_BALANCE_TAG);
            a.consume_resource(crate::engine::side_effects::Resource::Reaction);
        }
        let (holder_name, roller_name) = (self.actor_name(holder), self.actor_name(roller_id));
        self.log(format!(
            "  restore balance: {} evens out {}'s roll.",
            holder_name, roller_name
        ));
        RollMode::Normal
    }

    /// **The** roll-mode cancel chokepoint: run every feature that can
    /// flatten advantage or disadvantage before the die lands, in
    /// cheapest-first order, and hand back the mode to roll under.
    ///
    /// Two rows today — Drunkard's Luck on the roller's own sheet and
    /// Restore Balance from up to 60 ft away — and the wrapper exists
    /// so the two d20 sites that can still hold `&mut` when they know
    /// the mode (`engine::attack::resolve_attack_outcome` and
    /// `roll_save_with_extra_mode_and_bonus`) each call one function
    /// rather than accumulating a line per feature. A third canceller
    /// is a call added here and nowhere else.
    ///
    /// Self-lane first: it costs the roller a charge and nobody a
    /// reaction, where Restore Balance costs a bystander both. When
    /// both could fire on the same disadvantaged roll, spending the
    /// cheaper one is right — and the first to return `Normal`
    /// short-circuits the rest, so the second is never also spent on a
    /// roll that is already even.
    pub fn steady_the_d20(&mut self, roller_id: usize, mode: RollMode) -> RollMode {
        let mode = self.cancel_disadvantage_with_luck(roller_id, mode);
        self.cancel_mode_with_restore_balance(roller_id, mode)
    }

    /// Roll a d20 with mode, then apply the 5e Lucky trait reroll if the
    /// actor has it and rolled a natural 1. RAW: Lucky lets the holder
    /// reroll the die and "must use the new roll" — the second result is
    /// taken even if it's worse. Used by every attack / save / check
    /// rolled by an actor with `has_lucky`. Missing actor falls back to
    /// the un-modified roll.
    ///
    /// A Divination Wizard's **Portent** gets first refusal on the die,
    /// ahead of the roll itself: RAW's "you must choose to do so before
    /// the roll" means the substitution can't look at what would have
    /// come up, so it has to short-circuit here rather than post-process
    /// a result. A substituted face therefore bypasses the Lucky reroll
    /// entirely, which is also the RAW reading — Lucky rerolls "the
    /// d20", and a foretold face was never rolled.
    pub fn roll_d20_lucky(&mut self, actor_id: usize, mode: RollMode) -> u32 {
        if let Some(foretold) = self.try_substitute_portent(actor_id) {
            return foretold;
        }
        let raw = self.roll_d20_with_mode(mode);
        if raw != 1 {
            return raw;
        }
        let lucky = self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.has_lucky());
        if !lucky {
            return raw;
        }
        // 5e Lucky: reroll the *single* die, not the advantage/disadvantage
        // pair. We follow that by re-rolling the same mode — the holder's
        // advantage state still applies on the second roll, which is the
        // common-table interpretation. Either way the reroll happens
        // through the same seedable roller so determinism by seed holds.
        let reroll = self.roll_d20_with_mode(mode);
        let name = self.actor_name(actor_id);
        self.log(format!(
            "  lucky: {} re-rolls nat-1 → {}",
            name, reroll
        ));
        reroll
    }

    /// 5e Divination Wizard **Portent** (subclass level 2): offer every
    /// diviner in the encounter the chance to replace `roller_id`'s
    /// incoming d20 with one of their banked foretold faces. Returns
    /// `Some(face)` when one is spent — the caller then skips the roll
    /// entirely — and `None` on the overwhelmingly common no-diviner /
    /// no-worthwhile-die path.
    ///
    /// RAW: "You can replace any attack roll, saving throw, or ability
    /// check made by you or a creature that you can see with one of
    /// these foretold rolls. You must choose to do so before the roll."
    /// Every one of those roll kinds funnels through `roll_d20_lucky`,
    /// so the hook needs exactly one site.
    ///
    /// **Direction.** A diviner wants their own and their allies' rolls
    /// to land high and their enemies' to land low, so the pool is read
    /// from opposite ends depending on whose die is in flight
    /// (`want_high`). The diviner is always eligible against their own
    /// roll regardless of sight — RAW's "made by you" clause is not
    /// gated on seeing yourself.
    ///
    /// **Thresholds.** RAW leaves the spend decision to the player, who
    /// knows the DC, the stakes, and how many rounds are left. The
    /// engine has none of that at this site — `roll_d20_lucky` is
    /// deliberately context-free, taking only a roller and a mode — so
    /// the policy is a pair of decisiveness cutoffs: burn a die only
    /// when it is near-certain to swing the outcome on its own
    /// (`>= PORTENT_HIGH_FACE` for a roll the diviner wants to succeed,
    /// `<= PORTENT_LOW_FACE` for one they want to fail). A middling
    /// forecast is held rather than wasted, which is also how the
    /// feature is played at the table. With a two-die bank the policy
    /// caps the feature at two swings per long rest, so the cutoffs
    /// bound the blast radius without needing any of the context.
    ///
    /// Diviners are polled in ascending id order so a hypothetical
    /// two-diviner party spends dice deterministically under a fixed
    /// seed. The first one holding a decisive die wins; the rest keep
    /// theirs, matching RAW's one-replacement-per-roll rule.
    fn try_substitute_portent(&mut self, roller_id: usize) -> Option<u32> {
        // Cheapest possible bail on the hot path: this runs on every
        // d20 the engine rolls, and no encounter without a diviner in
        // it should pay more than a scan of a flag per actor.
        if !self.actors.values().any(|a| a.has_portent()) {
            return None;
        }
        for diviner_id in self.sorted_actor_ids() {
            let Some(diviner) = self.actors.get(&diviner_id) else {
                continue;
            };
            if !diviner.has_portent() || !diviner.is_combat_active() {
                continue;
            }
            let is_self = diviner_id == roller_id;
            if !is_self && !self.viewer_can_see(diviner_id, roller_id) {
                continue;
            }
            self.forecast_portent(diviner_id);
            let want_high = is_self || self.actors_allied(diviner_id, roller_id);
            let Some(face) = self
                .actors
                .get(&diviner_id)
                .and_then(|d| d.peek_portent_die(want_high))
            else {
                continue;
            };
            let decisive = if want_high {
                face >= PORTENT_HIGH_FACE
            } else {
                face <= PORTENT_LOW_FACE
            };
            if !decisive {
                continue;
            }
            let taken = self
                .actors
                .get_mut(&diviner_id)
                .and_then(|d| d.take_portent_die(want_high))?;
            let diviner_name = self.actor_name(diviner_id);
            let remaining = self
                .actors
                .get(&diviner_id)
                .map(|d| d.portent_pool().len())
                .unwrap_or(0);
            if is_self {
                self.log(format!(
                    "  portent: {} substitutes a foretold {} ({} left)",
                    diviner_name, taken, remaining
                ));
            } else {
                self.log(format!(
                    "  portent: {} substitutes a foretold {} for {} ({} left)",
                    diviner_name,
                    taken,
                    self.actor_name(roller_id),
                    remaining
                ));
            }
            return Some(taken);
        }
        None
    }

    /// Roll this diviner's banked foretold faces if they haven't been
    /// rolled since their last long rest. No-op for non-holders and for
    /// an already-forecast (even fully-spent) pool.
    ///
    /// RAW rolls the dice at the end of the long rest. We roll them at
    /// the first substitution opportunity instead, because
    /// `ActorInstance::long_rest` takes no roller and threading one
    /// through every rest call site would buy nothing observable — the
    /// pool is opaque until it is read, and rolling off the encounter's
    /// seeded roller keeps the values reproducible by seed either way.
    /// The `portent_forecast` latch on the actor is what makes the fill
    /// happen exactly once per rest.
    fn forecast_portent(&mut self, diviner_id: usize) {
        let count = self
            .actors
            .get(&diviner_id)
            .filter(|d| !d.portent_forecast())
            .map(|d| d.portent_dice_max())
            .unwrap_or(0);
        if count == 0 {
            return;
        }
        let faces: Vec<u32> = (0..count).map(|_| self.roll_d20_with_mode(RollMode::Normal)).collect();
        let rendered = faces
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        if let Some(diviner) = self.actors.get_mut(&diviner_id) {
            diviner.set_portent_pool(faces);
        }
        let name = self.actor_name(diviner_id);
        self.log(format!("  portent: {} foresees {}", name, rendered));
    }

    /// Compute the attack mode with all per-attack riders folded in:
    /// condition state, Dodge, the per-target Help grant, Bless. Used by
    /// every weapon / spell attack so the rider stack stays in one
    /// place. Returns the final mode for `roll_d20_with_mode`.
    ///
    /// **Mutating**, and unconditionally so: the Help grant is consumed
    /// and the Fancy Footwork mark is written. A caller that only wants
    /// to *predict* the mode — the AI ranking candidate targets — must
    /// use `peek_attack_mode` instead, which reads the same rider set
    /// through `&self`. This used to be a `consume_help: bool` knob, but
    /// no caller ever passed `false` (the AI couldn't: the knob only
    /// gated the grant, and the method still needed `&mut` for the
    /// mark), so the flag documented a capability the signature didn't
    /// actually offer.
    pub fn attack_mode_with_riders(
        &mut self,
        attacker_id: usize,
        target_id: usize,
        is_melee: bool,
    ) -> RollMode {
        let tally = self.attack_mode_tally_with_riders(attacker_id, target_id, is_melee);
        self.resolve_attack_mode_against(target_id, tally)
    }

    /// The mutating sweep behind `attack_mode_with_riders`, stopping one
    /// step short: it returns the tally rather than the resolved mode.
    ///
    /// `engine::attack::resolve_attack` is why. Four more disadvantage
    /// sources land after this point — the defender's reactive taxes, a
    /// shot past normal range, a lance used up close, and the
    /// underwater clause — and folding each onto an already-resolved
    /// mode is the information loss `RollModeTally` exists to stop: a
    /// swing that came out `Normal` because it held one of each is
    /// indistinguishable from one that held nothing, and the next
    /// disadvantage turns the first into `Disadvantage` when RAW says
    /// it stays `Normal`.
    ///
    /// Everything this does *besides* the sweep — the Fancy Footwork
    /// mark, the Help grant consumption — happens exactly once here,
    /// which is why the two wrappers cannot simply both call it.
    pub fn attack_mode_tally_with_riders(
        &mut self,
        attacker_id: usize,
        target_id: usize,
        is_melee: bool,
    ) -> RollModeTally {
        let mut tally = self.attack_mode_tally(attacker_id, target_id, is_melee);
        // 5e Swashbuckler Rogue Fancy Footwork (subclass level 3):
        // mirror of the mark placed at the top of
        // `engine::attack::resolve_attack` — every melee attack that
        // routes through this rider helper (Rogue Shortsword, Booming
        // Blade / Green Flame Blade / Shocking Grasp spell melee
        // touches, and any other inline attacker that calls this
        // helper instead of `resolve_attack`) writes the target id
        // onto the attacker's per-turn ledger. The write is
        // unconditional (no `has_fancy_footwork` gate) so the mark
        // stays cheap; the OA-suppression read in
        // `dispatch_opportunity_attacks` is where the flag gates the
        // suppression. Idempotent HashSet insert, which is what makes
        // the double-write safe: `resolve_attack_outcome` routes every
        // weapon swing through this helper AND writes the mark itself
        // beforehand, because its own write is the only one that
        // survives a swing Sanctuary turns away.
        if is_melee
            && let Some(attacker) = self.actors.get_mut(&attacker_id)
        {
            attacker.mark_melee_attacked_this_turn(target_id);
        }
        // Help: one-shot advantage if the attacker has a grant against
        // this target. Pop it before the roll regardless of hit/miss so
        // it can't double-fire on a follow-up.
        let help_active = self
            .actors
            .get_mut(&attacker_id)
            .map(|a| a.consume_help_for(target_id))
            .unwrap_or(false);
        tally.add_if(help_active, RollMode::Advantage);
        // Bless gives a flat +2 (handled at roll time via
        // condition_attack_bonus); we don't promote it to Advantage.
        // Keep this lane focused on mode (advantage / disadvantage) only.
        tally
    }

    /// Read-only twin of `attack_mode_with_riders`: the mode an attack
    /// *would* resolve at, with nothing consumed and no mark written.
    ///
    /// Exists for the AI's target ranking, which is a prediction rather
    /// than an attack and therefore only has `&EncounterInstance` to
    /// work with. Before it existed that ranking called
    /// `compute_attack_mode` directly — the only `&self` option — and so
    /// couldn't see the per-target Help grant. The visible effect was
    /// small but exactly backwards: a fighter who spent a bonus action
    /// feinting a target, or a rogue who designated one with Versatile
    /// Trickster, then ranked that target as no more attractive than any
    /// other, because the advantage they had just bought was invisible
    /// to the picker.
    ///
    /// Kept as a thin wrapper rather than a shared inner helper with a
    /// flag: the difference is one clause, and a flag is what the
    /// mutating twin used to carry before it turned out nobody could
    /// pass it.
    pub fn peek_attack_mode(
        &self,
        attacker_id: usize,
        target_id: usize,
        is_melee: bool,
    ) -> RollMode {
        let mut tally = self.attack_mode_tally(attacker_id, target_id, is_melee);
        let helped = self
            .actors
            .get(&attacker_id)
            .is_some_and(|a| a.help_grant(target_id));
        tally.add_if(helped, RollMode::Advantage);
        self.resolve_attack_mode_against(target_id, tally)
    }

    /// Compute the attack-roll mode given attacker / target conditions.
    /// 5e clauses we model today:
    /// - Attacker Prone / Poisoned / Frightened / Restrained / Blinded →
    ///   disadvantage on all attacks.
    /// - Target Prone → melee attacks have advantage, ranged have disadvantage.
    /// - Target Stunned / Restrained / Blinded / Incapacitated → advantage
    ///   on attacks vs them.
    ///
    /// Multiple sources of the same direction don't stack, and any
    /// advantage against any disadvantage cancels to Normal however
    /// many of each there are — PHB p.173's "no matter how many
    /// circumstances of each kind you have". Both halves come from
    /// `RollModeTally`, which is what `attack_mode_tally` accumulates
    /// into and this method resolves; see that type for what a
    /// left-fold over `RollMode::combine` got wrong instead.
    ///
    /// Attacker side (disadvantage):
    /// Prone, Poisoned, Blinded, Restrained, Frightened.
    ///
    /// Attacker side (advantage):
    /// Invisible.
    ///
    /// Target side (advantage on attacks against them):
    /// Stunned, Blinded, Restrained, Prone (melee only), Incapacitated.
    ///
    /// Target side (disadvantage on attacks against them):
    /// Invisible, Prone (ranged only).
    pub fn compute_attack_mode(
        &self,
        attacker_id: usize,
        target_id: usize,
        is_melee: bool,
    ) -> RollMode {
        self.resolve_attack_mode_against(
            target_id,
            self.attack_mode_tally(attacker_id, target_id, is_melee),
        )
    }

    /// Every advantage and disadvantage source an attack is subject to,
    /// collected as two flags rather than folded into a running
    /// `RollMode` — see `RollModeTally` for why, and for the kobold
    /// that demonstrated the difference.
    ///
    /// Split out of `compute_attack_mode` so the riders its two wrappers
    /// layer on (the per-target Help grant) can join the *tally* rather
    /// than being combined onto an already-resolved mode. Combining onto
    /// a resolved mode loses exactly the information the rule needs: a
    /// sweep that came out `Normal` because it held one of each is
    /// indistinguishable from one that held nothing, and a Help grant
    /// landing on the first should still be `Normal` and on the second
    /// should be `Advantage`.
    ///
    /// Deliberately excludes the Rogue's Elusive clause, which is not a
    /// source but a cap on the result — see
    /// `resolve_attack_mode_against`.
    pub fn attack_mode_tally(
        &self,
        attacker_id: usize,
        target_id: usize,
        is_melee: bool,
    ) -> RollModeTally {
        let mut tally = RollModeTally::NONE;

        // 5e Ranged Attacks in Close Combat — disadvantage when a
        // hostile creature that can see the shooter, and that isn't
        // Incapacitated, is standing within five feet. All three
        // clauses live in the predicate; see it for what the first
        // draft of this line left out.
        if !is_melee && self.ranged_attack_is_crowded(attacker_id) {
            tally.add(RollMode::Disadvantage);
        }

        // 5e heavy obscurement, both ways. "A creature effectively
        // suffers from the blinded condition when trying to see
        // something in that area" — so a fog bank between two creatures
        // is read here exactly as the Blinded condition is read below:
        // the attacker who can't see disadvantages, the target who
        // can't see hands out advantage.
        //
        // The symmetry is the point. Two creatures inside the same
        // cloud each get one of each and cancel to Normal, which is
        // 5e's unseen-attacker rule arriving for free rather than as a
        // special case. An archer shooting *into* a cloud gets only the
        // disadvantage; a creature inside shooting *out* at somebody in
        // the clear gets only that too, because the fog is on its side
        // of the line either way.
        //
        // Read off the zone layer rather than off a condition, because
        // obscurement is a property of the ground and the two creatures'
        // positions, and it stops applying the moment either of them
        // steps clear.
        //
        // The dark rides the same two clauses through
        // `sight_denied_between`, and composes with the fog exactly as
        // it should: an archer in a lit room shooting into an unlit one
        // takes the disadvantage and hands out no advantage, because
        // the darkness is on the target's side of the line only.
        if self.sight_denied_between(attacker_id, target_id) {
            tally.add(RollMode::Disadvantage);
        }
        if self.sight_denied_between(target_id, attacker_id) {
            tally.add(RollMode::Advantage);
        }

        // 5e Sunlight Sensitivity / Weakness / Hypersensitivity: "while
        // in sunlight, the creature has disadvantage on attack rolls."
        // Attacker-side only — RAW taxes the creature that is standing
        // in the sun, not the one being shot at from it — and inert
        // unless the encounter is actually under an open sky, which is
        // what `is_sunlit` is careful about.
        if let Some(attacker) = self.actors.get(&attacker_id)
            && attacker
                .sunlight_frailty()
                .is_some_and(|f| f.disadvantages_attacks())
            && self.is_sunlit(attacker.location())
        {
            tally.add(RollMode::Disadvantage);
        }

        // 5e **Mounted Combatant**: "You have advantage on melee attack
        // rolls against an unmounted creature that is smaller than your
        // mount." Read from the saddle down — the comparison RAW makes
        // is against the *horse's* size, not the rider's, which is the
        // whole point of the clause: a Medium knight on a Large
        // warhorse rides down anything Medium or smaller.
        if is_melee && self.rides_down(attacker_id, target_id) {
            tally.add(RollMode::Advantage);
        }

        // 5e Darkmantle, while attached: "the darkmantle can attack
        // only the target but has Advantage on its attack rolls."
        // Beside the mounted clause above because it is the same shape
        // — one creature riding another, and the ride is what grants
        // the bonus — with the polarity of the ride reversed.
        //
        // Not gated on `is_melee`: RAW attaches the advantage to the
        // creature's attack rolls without qualification, and a
        // darkmantle wrapped around a head has nothing but its Crush to
        // make anyway.
        if self.attachment_grants_advantage(attacker_id, target_id) {
            tally.add(RollMode::Advantage);
        }

        // 5e concealment-piercing snapshot: does the attacker see through
        // the target's illusion / invisibility, and does the target see
        // through the attacker's? Both booleans feed the suppression
        // clauses on the per-side condition sweeps below, so the
        // condition-cohort loops stay branch-free of the piercing
        // lookup. `pierces_illusion_of` unifies the four sources:
        //   - `has_truesight` (Truesight sense OR True Seeing condition,
        //     any range — Deva, Solar, Pit Fiend, Lich, etc.)
        //   - `has_feral_senses` (Ranger lv18 capstone — any range)
        //   - `has_blindsense` (Rogue lv14 — 10 ft footprint envelope,
        //     gated on !Deafened)
        //   - `has_blind_fighting_style` (Tasha Fighter / Ranger /
        //     Paladin Fighting Style — 10 ft footprint envelope, no
        //     hearing gate)
        // Pre-refactor this gate only cohorted `has_truesight`; folding
        // the ranger / rogue passive piercers + Blind Fighting into the
        // same helper lets each new "sees through illusion" tag land as
        // a one-line extension to `pierces_illusion_of` instead of a
        // new suppression clause at every attack-mode caller.
        let attacker_piercing = self.concealment_piercing_of(attacker_id, target_id);
        let target_piercing = self.concealment_piercing_of(target_id, attacker_id);

        // 5e's *other* way to lose your invisibility: not a viewer who
        // sees through it, but an outline burned onto you that every
        // viewer sees. Faerie Fire and Starry Wisp both say "can't
        // benefit from the Invisible condition", and both halves matter
        // here — an outlined creature stops hiding behind its
        // invisibility (target side) and stops attacking out of it
        // (attacker side). See `Condition::suppresses_invisibility`.
        let attacker_outlined = self.concealment_burned_off(attacker_id);
        let target_outlined = self.concealment_burned_off(target_id);

        // Attacker-side modifiers. The disadvantage / advantage cohorts
        // live on `Condition` itself (`imposes_attacker_disadvantage` /
        // `grants_self_attack_advantage`) so adding a new condition is a
        // one-line change to the helper rather than a re-edit here.
        if let Some(attacker) = self.actors.get(&attacker_id) {
            // 5e exhaustion tier 3: "disadvantage on attack rolls and
            // saving throws". Off the condition cohort for the same
            // reason it is off the save one — that table is keyed by
            // condition, and the flag is up two rungs before the
            // penalty is earned.
            if attacker.exhaustion_level()
                >= crate::actors::actor_template::EXHAUSTION_ROLL_PENALTY_TIER
            {
                tally.add(RollMode::Disadvantage);
            }
            for c in attacker.conditions().keys() {
                if c.imposes_attacker_disadvantage() {
                    tally.add(RollMode::Disadvantage);
                }
                if c.grants_self_attack_advantage() {
                    // 5e concealment-piercing on the target (Truesight /
                    // Feral Senses / Blindsense-in-range) neuters the
                    // attacker's invisibility-style concealment advantages
                    // (Invisible, Blurred, Displaced). Hidden / Helped /
                    // Bless / etc. are unaffected — they're not concealment.
                    //
                    // An outline on the attacker themselves takes the
                    // same advantage away, from the other direction:
                    // RAW's clause is "can't benefit from the Invisible
                    // condition", and swinging out of it is a benefit.
                    let suppressed = target_piercing.pierces(*c)
                        || (attacker_outlined && c.countered_by_see_invisibility());
                    if !suppressed {
                        tally.add(RollMode::Advantage);
                    }
                }
                // Ranged-only attacker disadvantage cohort: Storm Sphere's
                // gusts throw off bow shots / spell arrows but leave the
                // attacker's melee swings unaffected. Symmetric to the
                // target-side `imposes_disadvantage_to_ranged_attackers`
                // clause, but on the attacker side.
                if !is_melee && c.imposes_attacker_disadvantage_on_ranged() {
                    tally.add(RollMode::Disadvantage);
                }
                // Melee-only attacker advantage cohort: the Shadow Monk's
                // Shadow Step buffs "the first melee attack you make",
                // so a thrown-weapon swing after the teleport reads
                // clean. Mirror of the ranged-only clause directly
                // above — same attacker side, opposite lane, opposite
                // polarity. Not routed through the concealment-piercing
                // suppression: this is footwork, not invisibility, and
                // Truesight does nothing about a monk who is simply
                // somewhere else now.
                if is_melee && c.grants_self_melee_attack_advantage() {
                    tally.add(RollMode::Advantage);
                }
            }
            // 5e Pack Tactics (Wolf, Dire Wolf, Kobold): advantage on
            // attack rolls when a non-incapacitated ally is adjacent to
            // the target. Checks the team-relative adjacency rather than
            // a condition flag — the trait is a template feature.
            if attacker.has_pack_tactics()
                && self.has_ally_adjacent_to(attacker_id, target_id, |_| true)
            {
                tally.add(RollMode::Advantage);
            }
            // 5e Sahuagin Blood Frenzy: melee attacks against a wounded
            // target get advantage. Passive trait tagged via the features
            // pool; the gate fires only on melee swings (RAW). The
            // wounded predicate routes through `is_wounded` so any future
            // refinement (half-HP threshold, etc.) lands in one place.
            use crate::actions::class_features::BLOOD_FRENZY_TAG;
            if is_melee
                && attacker.has_passive_feature(BLOOD_FRENZY_TAG)
                && self
                    .actors
                    .get(&target_id)
                    .is_some_and(|t| t.is_wounded())
            {
                tally.add(RollMode::Advantage);
            }
            // 5e Barbarian Path of the Totem Warrior — Wolf Totem Spirit.
            // While a teammate (not the attacker themselves) is raging
            // AND has the WOLF_TOTEM_TAG passive feature AND is footprint-
            // adjacent to the target, melee attackers on that team get
            // advantage. Mirrors Pack Tactics' ally-side adjacency shape —
            // same `has_ally_adjacent_to` scan — but gated on melee,
            // raging, and the totem feature rather than the pack-tactics
            // template trait. The attacker doesn't help themselves (RAW:
            // "your friends") so a wolf totem barbarian attacking solo
            // gets no benefit from their own aura — guaranteed by the
            // `id != attacker_id` clause inside the helper.
            use crate::actions::class_features::WOLF_TOTEM_TAG;
            if is_melee
                && self.has_ally_adjacent_to(attacker_id, target_id, |a| {
                    a.has_passive_feature(WOLF_TOTEM_TAG)
                        && a.has_condition(Condition::Raging)
                })
            {
                tally.add(RollMode::Advantage);
            }
            // 5e Rogue Assassin **Assassinate** (level 3 subclass). The
            // assassin rolls with advantage on every attack against any
            // creature whose `has_taken_turn_in_combat` latch is still
            // unset — RAW reads "any creature that hasn't taken a turn in
            // the combat yet." The latch flips on the *start* of the
            // target's first turn (see `start_turn_for`) rather than the
            // end, so a slow-initiative target who hasn't acted yet but
            // whose slot has come up doesn't qualify — matches RAW more
            // closely than a "first round only" approximation. Fires on
            // melee AND ranged attacks (no weapon-type gate).
            use crate::actions::class_features::ASSASSINATE_TAG;
            if attacker.has_passive_feature(ASSASSINATE_TAG)
                && self
                    .actors
                    .get(&target_id)
                    .is_some_and(|t| !t.has_taken_turn_in_combat())
            {
                tally.add(RollMode::Advantage);
            }
            // Every "locked onto someone, and this isn't them" debuff,
            // through one walk. See `FOCUS_LINK_DISADVANTAGES`.
            for &condition in FOCUS_LINK_DISADVANTAGES {
                tally.add(focus_link_mode(
                    attacker,
                    target_id,
                    condition,
                    RollMode::Disadvantage,
                ));
            }
        }

        // Target-side modifiers.
        if let Some(target) = self.actors.get(&target_id) {
            // Prone target: melee attackers get advantage, ranged get
            // disadvantage. Single source of truth for the prone clause.
            if target.has_condition(Condition::Prone) {
                tally.add(if is_melee {
                    RollMode::Advantage
                } else {
                    RollMode::Disadvantage
                });
            }
            // Static cohorts: target-side advantage / disadvantage. Same
            // refactor pattern as the attacker side — adding a new
            // attack-impacting condition is a one-line change to the
            // `Condition` helpers, not a re-edit of this function.
            for c in target.conditions().keys() {
                if c.grants_advantage_to_attackers() {
                    tally.add(RollMode::Advantage);
                }
                if c.imposes_disadvantage_to_attackers() {
                    // 5e concealment-piercing on the attacker (Truesight
                    // / Feral Senses / Blindsense-in-range) neuters the
                    // target's invisibility-style concealment
                    // disadvantages (Invisible, Blurred, Displaced).
                    // Dodging / Holy Aura / Foreseen / etc. are
                    // unaffected — those are active defenses, not
                    // illusory concealment.
                    //
                    // An outline on the target burns off the same
                    // concealment without anybody having to see through
                    // it — this is the half that makes Faerie Fire's
                    // second sentence mean something.
                    let suppressed = attacker_piercing.pierces(*c)
                        || (target_outlined && c.countered_by_see_invisibility());
                    if !suppressed {
                        tally.add(RollMode::Disadvantage);
                    }
                }
                // Ranged-only disadvantage cohort: Wind Wall deflects
                // arrows but does nothing against a sword swing. Gated on
                // `!is_melee` so melee attackers eat no penalty.
                if !is_melee && c.imposes_disadvantage_to_ranged_attackers() {
                    tally.add(RollMode::Disadvantage);
                }
            }
            // Two attacker-type-gated disadvantage lanes. Neither can live
            // in the static condition cohort: both read the *attacker's*
            // creature type, not the target's conditions alone.
            //
            // 5e Protection from Evil and Good: aberrations / celestials /
            // elementals / fey / fiends / undead have disadvantage on
            // attacks against the Warded target. `affected_by_protection`
            // on `CreatureType` is exactly that list.
            //
            // The Devotion Paladin's **Purity of Spirit** (subclass level
            // 15) is RAW "you are always under the effects of a
            // protection from evil and good spell", so it reads the same
            // clause from a passive tag instead of from a condition — one
            // `||` here rather than a permanent condition install the
            // engine would have to keep re-applying and would have to
            // exempt from Dispel Magic. See `PURITY_OF_SPIRIT_TAG`.
            //
            // Daylight: the engine's simplification of RAW Sunlight
            // Sensitivity — an undead attacker caught in the aura rolls at
            // disadvantage. RAW's own trait belongs to specific undead
            // (and to a few subterranean humanoids the engine doesn't
            // tag), so undead is the load-bearing cohort.
            if let Some(attacker) = self.actors.get(&attacker_id) {
                let attacker_type = attacker.creature_type();
                let warded = target.has_condition(Condition::Warded)
                    || target.has_passive_feature(
                        crate::actions::class_features::PURITY_OF_SPIRIT_TAG,
                    );
                if (warded && attacker_type.affected_by_protection())
                    || (target.has_condition(Condition::Daylit) && attacker_type.is_undead())
                {
                    tally.add(RollMode::Disadvantage);
                }
            }
            // 5e Battle Master Distracting Strike — target-side mirror
            // of the Dueled / Goaded pattern. A distracted creature is
            // open to attack by anyone other than the fighter who
            // tagged them: that *other* attacker rolls with advantage,
            // the distractor themselves gets no benefit from their own
            // setup (RAW: "the next attack roll against the target by
            // an attacker other than you"). Same flag-plus-link shape
            // as Dueled / Goaded, but reversed polarity (advantage
            // instead of disadvantage) and reversed side (target-side
            // rather than attacker-side).
            tally.add(focus_link_mode(
                target,
                attacker_id,
                Condition::Distracted,
                RollMode::Advantage,
            ));
            // 5e Vengeance Paladin Vow of Enmity (lv3 subclass Channel
            // Divinity). Positive-polarity sibling of Distracted: the
            // paladin who swore the vow gets advantage on attack rolls
            // against the sworn target (the buff is exclusive to the
            // swearer — RAW: "you gain advantage on attack rolls").
            // Same flag-plus-link shape as Distracted but matches ON
            // the linked id rather than mismatches against it. Routes
            // through `matched_link_mode` so any future "I marked you
            // — I get the buff" rider (Hunter's Quarry single-target
            // damage prime, etc.) lands as a one-liner.
            tally.add(matched_link_mode(
                target,
                attacker_id,
                Condition::Sworn,
                RollMode::Advantage,
            ));
            // 5e **Vex** weapon mastery — the same "I marked you, I get
            // the buff" shape as Vow of Enmity directly above, and the
            // reason `matched_link_mode` was worth extracting. Spent by
            // the roll it buys at `clear_attack_advantage_riders`; see
            // `CONSUMED_BY_LINKED_ATTACKER`.
            tally.add(matched_link_mode(
                target,
                attacker_id,
                Condition::Vexed,
                RollMode::Advantage,
            ));
        }
        tally
    }

    /// Resolve a completed tally into the mode the d20 is actually
    /// rolled at, applying the one clause that is a *cap on the result*
    /// rather than a source feeding into it.
    ///
    /// 5e Rogue **Elusive** (level 18 capstone): "no attack roll has
    /// advantage against you" while the rogue isn't Incapacitated. It
    /// has to run after every source has been counted — a Hidden / Vow
    /// of Enmity / Pack Tactics attacker gets `Normal` instead of
    /// `Advantage`, while a Restrained / Prone / Poisoned attacker's
    /// `Disadvantage` passes through untouched — and it has to run
    /// after the *riders* too, which is why it lives here rather than
    /// at the bottom of `attack_mode_tally`. Before the split it ran
    /// inside the sweep, so the Help grant that
    /// `attack_mode_with_riders` combines on afterwards handed the
    /// advantage straight back to an attacker RAW says cannot have it.
    ///
    /// The "not Incapacitated" cohort matches RAW: Stunned / Paralyzed
    /// / Unconscious inherit Incapacitated, so a stun-locked rogue
    /// loses the perk — the swings go back to Advantage through the
    /// target-side condition sweep, and Elusive stops suppressing.
    pub fn resolve_attack_mode_against(
        &self,
        target_id: usize,
        tally: RollModeTally,
    ) -> RollMode {
        let mode = tally.resolve();
        let elusive = self.actors.get(&target_id).is_some_and(|t| {
            t.has_elusive()
                && !t.has_condition(Condition::Incapacitated)
                && !t.has_condition(Condition::Stunned)
                && !t.has_condition(Condition::Paralyzed)
                && !t.has_condition(Condition::Unconscious)
        });
        if elusive && matches!(mode, RollMode::Advantage) {
            RollMode::Normal
        } else {
            mode
        }
    }

    /// True if `viewer` can "see" `subject` in the RAW sense used by
    /// "when a creature you can see..." reaction gates (Warding Flare,
    /// Fighting Style: Protection, Fighting Style: Interception, and
    /// friends). Three clauses in RAW:
    ///
    ///   1. **Viewer isn't Blinded** — a blinded observer sees nothing
    ///      period, regardless of what the subject is doing.
    ///   2. **Subject isn't illusion-concealed from the viewer** — an
    ///      Invisible / Blurred / Displaced subject is unseen UNLESS the
    ///      viewer's piercing lookup (Truesight / Feral Senses /
    ///      Blindsense-in-range) sees through it.
    ///   3. **Nothing solid in between** — a wall between the two blocks
    ///      sight in exactly the sense every targeting gate in the engine
    ///      already means by it, via the footprint-aware
    ///      `actor_has_line_of_sight`.
    ///
    /// Clause 3 is checked last because it is the only one that costs
    /// more than a flag read: the Bresenham walk runs per footprint-tile
    /// pair, and this helper sits on the per-attack reaction path
    /// (Uncanny Dodge, Deflect Missiles, Protection, Interception). The
    /// two cheap gates reject the common "blinded" / "invisible" cases
    /// before it.
    ///
    /// Returns false when either actor is unknown. Sibling helper to
    /// `pierces_illusion_of` — that one answers "does the viewer see
    /// through the illusion cohort?" and this one answers "does the
    /// viewer actually see the subject right now?", folding in the
    /// Blinded gate and the "no illusion in play means yes" default.
    ///
    /// Centralized so the "you can see" clauses across Warding Flare
    /// (Cleric Light) and future sight-gated reactions (a lifetime of
    /// "when a creature you can see..." wordings — Sentinel-flavored
    /// riders, Cutting Words RAW's "creature you can see", the Rune
    /// Knight's Giant Might reactive shield, etc.) all read the same
    /// gate. Pre-refactor Warding Flare only checked `Blinded` on the
    /// cleric, letting an Invisible attacker still draw the flare
    /// charge even though RAW the cleric couldn't see them.
    ///
    /// The wall clause landed later, for the same class of reason: the
    /// helper claimed to answer "can the viewer see the subject" while
    /// silently ignoring the terrain, so every consumer that isn't
    /// already downstream of a targeting LOS check — the Divination
    /// Wizard's Portent substitution is the first — would have reached
    /// through solid rock.
    pub fn viewer_can_see(&self, viewer_id: usize, subject_id: usize) -> bool {
        let Some(viewer) = self.actors.get(&viewer_id) else {
            return false;
        };
        if viewer.has_condition(Condition::Blinded) {
            return false;
        }
        let Some(subject) = self.actors.get(&subject_id) else {
            return false;
        };
        // "Illusion-concealed" cohort matches the `countered_by_truesight`
        // set the attack-mode suppression clauses read — same cohort so
        // a future addition (a hypothetical Hide-in-Mists condition)
        // lands in one place instead of at every sight-gated caller.
        // Asked per condition rather than in bulk: a viewer whose
        // piercing tops out at See Invisibility does see an `Invisible`
        // subject, but a `Blurred` one is still hidden from them, and a
        // subject wearing both is hidden on the strength of the Blur.
        let piercing = self.concealment_piercing_of(viewer_id, subject_id);
        let concealed = subject
            .conditions()
            .keys()
            .any(|c| c.countered_by_truesight() && !piercing.pierces(*c));
        if concealed {
            return false;
        }
        if !self.actor_has_line_of_sight(viewer_id, subject_id) {
            return false;
        }
        !self.sight_denied_between(viewer_id, subject_id)
    }

    /// True if the *environment* — fog or the dark — stops `viewer_id`
    /// seeing `subject_id`, with neither's own conditions considered.
    ///
    /// The two clauses are genuinely different rules and are kept as
    /// separate predicates (`obscurement_blinds` walks the whole line
    /// and ignores darkvision; `darkness_blinds` reads one tile and
    /// respects it), but every caller wants both, and there are three:
    /// `viewer_can_see`, and the two polarities of the attack-mode
    /// sweep. Folding the `||` into one named helper is what stops a
    /// fourth caller picking up one of the two and silently missing the
    /// other — which is exactly how the darkness half would have gone
    /// in if the fog half had not already been there to copy.
    pub fn sight_denied_between(&self, viewer_id: usize, subject_id: usize) -> bool {
        self.obscurement_blinds(viewer_id, subject_id)
            || self.darkness_blinds(viewer_id, subject_id)
    }

    /// The ambient light this encounter was set up with.
    pub fn ambient_light(&self) -> AmbientLight {
        self.ambient_light
    }

    /// Set the ambient light. Called once at setup — by the CLI, by a
    /// scenario builder, by a test — rather than mid-fight; nothing in
    /// 5e changes the sky, and the spells that change the *local* light
    /// go through `add_light_source` and the darkening zones instead.
    pub fn set_ambient_light(&mut self, ambient: AmbientLight) {
        self.ambient_light = ambient;
    }

    /// Every light source currently burning.
    pub fn light_sources(&self) -> &[LightSource] {
        &self.light_sources
    }

    /// Light something up, and hand back the id that takes it away
    /// again.
    ///
    /// The id is assigned here rather than by the caller for the same
    /// reason zone ids are: a caller that picked its own would have to
    /// know what is already burning.
    pub fn add_light_source(&mut self, mut source: LightSource) -> usize {
        let id = self.light_source_id_next;
        self.light_source_id_next += 1;
        source.id = id;
        self.light_sources.push(source);
        id
    }

    /// Put out one source by id. Silent no-op if it has already gone
    /// out, which is the same shape `remove_zone` uses and for the same
    /// reason: the two ways a light ends (a timer, a dispel) can race.
    pub fn remove_light_source(&mut self, id: usize) {
        self.light_sources.retain(|s| s.id != id);
    }

    /// Un-anchor everything `actor_id` was carrying, leaving it burning
    /// where they fell.
    ///
    /// A torch does not go out because the hand holding it did, and the
    /// engine already has a policy for this exact question one line
    /// away: `remove_actor` and `despawn_actor` both drop the dead
    /// actor's carried *items* on their tile rather than voiding them.
    /// Light is treated the same way, which is both RAW and the more
    /// interesting board — a corpse lighting the corridor it fell in.
    ///
    /// Must be called while the actor is still in the table, since that
    /// is where the tile comes from. A source whose bearer is already
    /// gone has no last known position to pin it to and is snuffed
    /// instead, which is the honest answer rather than a guess.
    ///
    /// A creature that *is* the light is the exception, and the only
    /// one: an azer's glow is the azer being made of fire, so it goes
    /// out with them rather than lying on the flagstones. That is what
    /// `LightSource::innate` marks, and it is the difference between
    /// killing something that carries a lamp and killing something that
    /// is one — the second darkens the room.
    pub fn drop_light_sources_carried_by(&mut self, actor_id: usize) {
        let dropped_at = self.actors.get(&actor_id).map(|a| a.location());
        self.light_sources.retain_mut(|source| {
            if source.anchor != LightAnchor::Carried(actor_id) {
                return true;
            }
            if source.innate {
                return false;
            }
            match dropped_at {
                Some(loc) => {
                    source.anchor = LightAnchor::Fixed(loc);
                    true
                }
                None => false,
            }
        });
    }

    /// True if `actor_id` is already carrying a light of their own.
    ///
    /// The gate the Light cantrip and the torch both need: RAW's "if
    /// you cast this spell again, the previous casting is dispelled"
    /// makes a second cast on an already-lit bearer a wasted action,
    /// and an AI that re-runs its ladder every turn will take a wasted
    /// action every turn unless something says no. The same shape as
    /// the `Aided` marker's stacking gate, and it exists for the same
    /// observed reason.
    pub fn actor_carries_light(&self, actor_id: usize) -> bool {
        self.light_sources
            .iter()
            .any(|s| s.anchor == LightAnchor::Carried(actor_id))
    }

    /// True if a spell-created darkness covers this tile.
    ///
    /// The gate on two separate rules — nonmagical light cannot lift it
    /// (`light_at`), and darkvision cannot see through it
    /// (`perceived_light`) — so it is one named predicate rather than a
    /// zone scan open-coded at both.
    pub fn magically_dark_at(&self, coord: Coordinate) -> bool {
        self.zones
            .iter()
            .any(|z| z.effect.darkens.is_some() && z.covers(coord))
    }

    /// How brightly lit `coord` is, for everybody.
    ///
    /// Magical darkness first and unconditionally: RAW's "nonmagical
    /// light can't illuminate it" means the sources below never get a
    /// vote inside one, and neither does the sun.
    ///
    /// Otherwise the brightest of the ambient level and every source
    /// that reaches — `max`, not a sum, because two torches in a room
    /// do not make it brighter than one.
    pub fn light_at(&self, coord: Coordinate) -> LightLevel {
        if self.magically_dark_at(coord) {
            return LightLevel::Dark;
        }
        let mut level = self.ambient_light.level();
        for source in &self.light_sources {
            if level == LightLevel::Bright {
                break;
            }
            if let Some(dist) = self.distance_from_light(source, coord) {
                level = level.brighter_of(source.level_at_distance(dist));
            }
        }
        level
    }

    /// How far `coord` is from `source`, or `None` if the source is
    /// carried by somebody who has left the table.
    ///
    /// A fixed source is a point and measures point-to-tile. A carried
    /// one measures from its bearer's whole *body*, through the same
    /// `footprint_chebyshev` every sense envelope in the engine uses —
    /// because a Huge creature does not hold its torch in the corner of
    /// its space, and measuring from the anchor tile would light the
    /// north-east of a giant three tiles further than the south-west.
    fn distance_from_light(&self, source: &LightSource, coord: Coordinate) -> Option<isize> {
        match source.anchor {
            LightAnchor::Fixed(origin) => Some(origin.chebyshev_to(coord)),
            LightAnchor::Carried(bearer_id) => self.actors.get(&bearer_id).map(|bearer| {
                footprint_chebyshev(
                    bearer.location(),
                    get_tiles_from_size(bearer.size()),
                    coord,
                    1,
                )
            }),
        }
    }

    /// True if `coord` is in actual sunlight — the trigger for every
    /// Sunlight Sensitivity / Weakness / Hypersensitivity clause.
    ///
    /// Only the sky qualifies. No light *source* sets this, not even
    /// the Daylight spell: RAW is explicit that the spell does not
    /// count, and a vampire that could be destroyed by a 3rd-level
    /// spell would be a different monster.
    ///
    /// Magical darkness lifts it, which is the interaction that makes
    /// the Darkness spell a drow's answer to being caught in the open.
    pub fn is_sunlit(&self, coord: Coordinate) -> bool {
        self.ambient_light.is_sunlight() && !self.magically_dark_at(coord)
    }

    /// How brightly lit `coord` is *as far as `viewer_id` is
    /// concerned* — the objective answer, then darkvision.
    ///
    /// 5e darkvision: "you can see in dim light within the radius as if
    /// it were bright light, and in darkness as if it were dim light."
    /// One rung, gated on the radius, and explicitly *not* applicable
    /// to magical darkness — the Darkness spell's second sentence is
    /// "a creature with darkvision can't see through this darkness",
    /// and it is what makes the spell worth a 2nd-level slot in a
    /// bestiary where nearly everything has darkvision.
    ///
    /// Devil's Sight is the counter RAW provides, and it is checked
    /// against the magical case only: the invocation's whole text is
    /// "you can see normally in darkness, both magical and nonmagical",
    /// and the nonmagical half is already covered by the upgrade below
    /// for anyone who has any darkvision at all.
    pub fn perceived_light(&self, viewer_id: usize, coord: Coordinate) -> LightLevel {
        let Some(viewer) = self.actors.get(&viewer_id) else {
            return self.light_at(coord);
        };
        if self.magically_dark_at(coord) {
            return if viewer.has_devils_sight() {
                LightLevel::Bright
            } else {
                LightLevel::Dark
            };
        }
        let base = self.light_at(coord);
        if base == LightLevel::Bright {
            return base;
        }
        // Measured from the viewer's body rather than from its anchor
        // tile, for the same reason a carried light is — and, more
        // importantly, so that darkvision and the non-visual envelopes
        // it sits beside (`nonvisual_sense_reaches`) answer the same
        // geometry. A bat's 60 ft of blindsight and a goblin's 60 ft of
        // darkvision reaching different distances would be a difference
        // nothing in the rules asks for.
        let reach = viewer.darkvision_tiles();
        let dist = footprint_chebyshev(
            viewer.location(),
            get_tiles_from_size(viewer.size()),
            coord,
            1,
        );
        if reach > 0 && dist <= reach {
            base.upgraded()
        } else {
            base
        }
    }

    /// True if the dark is what stops `viewer_id` seeing
    /// `subject_id` — the lighting layer's half of the sight gate,
    /// sitting beside `obscurement_blinds`.
    ///
    /// **Asymmetric, unlike obscurement, and that is the whole
    /// difference between the two.** Only the *subject's* tile is
    /// asked about. A creature standing in an unlit corridor sees the
    /// torchlit room ahead of it perfectly well and is itself unseen by
    /// anyone in that room, which is 5e's unseen-attacker rule arriving
    /// exactly where it should: the one in the dark gets advantage, the
    /// one in the light gets disadvantage, and neither cancels the
    /// other. A fog bank, by contrast, blinds both ends of the line at
    /// once, and `obscurement_blinds` walks the whole line to say so.
    ///
    /// The senses that get around it are the same three that get around
    /// fog — Truesight, the non-visual envelope (Blindsight,
    /// Tremorsense, Blindsense, Blind Fighting), and here also
    /// darkvision, which is folded in by `perceived_light` rather than
    /// checked here.
    pub fn darkness_blinds(&self, viewer_id: usize, subject_id: usize) -> bool {
        // The overwhelmingly common case is a lit board with nothing
        // darkening it, and every lookup below is wasted work there.
        if self.ambient_light.level() != LightLevel::Dark
            && self.zones.iter().all(|z| z.effect.darkens.is_none())
        {
            return false;
        }
        let (Some(viewer), Some(subject)) =
            (self.actors.get(&viewer_id), self.actors.get(&subject_id))
        else {
            return false;
        };
        if viewer.has_truesight() || nonvisual_sense_reaches(viewer, subject) {
            return false;
        }
        // Magical darkness first, and on its own terms: darkvision does
        // not lift it and Devil's Sight is the only thing that does.
        let where_they_stand = subject.location();
        if self.magically_dark_at(where_they_stand) {
            return !viewer.has_devils_sight();
        }
        if self.light_at(where_they_stand) != LightLevel::Dark {
            return false;
        }
        // Body to body, exactly as `nonvisual_sense_reaches` measures
        // the envelopes darkvision sits beside — not through
        // `perceived_light`, which answers a *tile* query and therefore
        // has only one body to measure from. A creature is seen if its
        // space is within the radius, and for a dragon that is a very
        // different question from whether its anchor tile is.
        let reach = viewer.darkvision_tiles();
        if reach == 0 {
            return true;
        }
        footprint_chebyshev(
            viewer.location(),
            get_tiles_from_size(viewer.size()),
            where_they_stand,
            get_tiles_from_size(subject.size()),
        ) > reach
    }

    /// Burn one round off every light source with a timer and sweep the
    /// ones that ran out. Called from `round_end` beside `tick_zones`.
    fn tick_light_sources(&mut self) {
        let mut guttered: Vec<&'static str> = Vec::new();
        self.light_sources.retain_mut(|source| {
            let Some(left) = source.rounds_remaining else {
                return true;
            };
            let left = left.saturating_sub(1);
            source.rounds_remaining = Some(left);
            if left == 0 {
                guttered.push(source.name);
                false
            } else {
                true
            }
        });
        for name in guttered {
            self.log(format!("The {} goes out.", name));
        }
    }

    /// Snuff every light source of level `max_level` or lower whose
    /// origin lies inside the given area — the Darkness spell's "if any
    /// of this spell's area overlaps with an area of light created by a
    /// spell of 2nd level or lower, the spell that created the light is
    /// dispelled."
    ///
    /// Returns how many went out so the caller can log it.
    ///
    /// Nonmagical flame (`spell_level == 0`) is deliberately included:
    /// a torch is "light of 2nd level or lower" in every sense that
    /// matters, and RAW's stronger clause — "nonmagical light can't
    /// illuminate it" — already stops it working inside the sphere. Put
    /// out rather than merely suppressed is the simpler state and the
    /// same outcome while the darkness stands; the difference only
    /// shows once the darkness lapses, and a torch dropped into a
    /// sphere of magical darkness going out is the better of the two
    /// answers to offer there.
    pub fn dispel_light_in(
        &mut self,
        origin: Coordinate,
        radius: isize,
        max_level: u32,
    ) -> usize {
        let doomed: Vec<usize> = self
            .light_sources
            .iter()
            .filter(|s| s.spell_level <= max_level)
            .filter(|s| {
                s.origin_in(&self.actors)
                    .is_some_and(|o| o.chebyshev_to(origin) <= radius)
            })
            .map(|s| s.id)
            .collect();
        let count = doomed.len();
        for id in doomed {
            self.remove_light_source(id);
        }
        count
    }

    /// Tear down every magical darkness of level `max_level` or lower
    /// overlapping the given area — the Daylight spell's "if any of
    /// this spell's area overlaps with an area of darkness created by a
    /// spell of 3rd level or lower, the spell that created the darkness
    /// is dispelled", which was a documented no-op until the lighting
    /// layer gave it something to act on.
    ///
    /// Returns how many were dispelled. The caster of each loses the
    /// concentration that was holding it up, through the same
    /// `pending_concentration_review` queue an expired zone uses — a
    /// dispelled Darkness must not leave its caster gripping nothing.
    pub fn dispel_magical_darkness_in(
        &mut self,
        origin: Coordinate,
        radius: isize,
        max_level: u32,
    ) -> usize {
        let doomed: Vec<(usize, usize, bool)> = self
            .zones
            .iter()
            .filter(|z| z.effect.darkens.is_some_and(|level| level <= max_level))
            .filter(|z| z.origin.chebyshev_to(origin) <= radius + z.radius)
            .map(|z| (z.id, z.owner_id, z.concentration))
            .collect();
        let count = doomed.len();
        for (id, owner_id, concentration) in doomed {
            self.remove_zone(id);
            if concentration {
                self.pending_concentration_review.push(owner_id);
            }
        }
        count
    }

    /// True if heavy obscurement stands between the two and the viewer
    /// has no sense that gets around it.
    ///
    /// 5e: "A heavily obscured area — such as darkness, opaque fog, or
    /// dense foliage — blocks vision entirely. A creature effectively
    /// suffers from the blinded condition when trying to see something
    /// in that area." Fog is symmetric — it stops the archer outside
    /// picking a target inside as surely as it stops the target picking
    /// the archer — so the geometry check (`obscured_between`) includes
    /// both endpoints as well as the tiles in between.
    ///
    /// The senses that get around it are the ones RAW says do — the
    /// unbounded `Truesight` short-circuit, plus every range-gated
    /// non-visual sense in `nonvisual_sense_reaches`.
    ///
    /// Darkvision deliberately does *not*: RAW it upgrades darkness by
    /// one step, and a fog cloud is not darkness. A creature with
    /// darkvision in a fog bank is as blind as one without.
    fn obscurement_blinds(&self, viewer_id: usize, subject_id: usize) -> bool {
        // The common case is a board with no fog on it at all, and the
        // whole walk below is wasted work there.
        if self.zones.iter().all(|z| !z.effect.obscures) {
            return false;
        }
        let (Some(viewer), Some(subject)) =
            (self.actors.get(&viewer_id), self.actors.get(&subject_id))
        else {
            return false;
        };
        if viewer.has_truesight() {
            return false;
        }
        if nonvisual_sense_reaches(viewer, subject) {
            return false;
        }
        self.obscured_between(viewer.location(), subject.location())
    }

    /// True if `viewer` sees through *every* concealment in
    /// `subject`'s illusion cohort — the `ConcealmentPiercing::All`
    /// answer, kept as a named predicate because "does this viewer have
    /// full truesight-grade piercing?" is the question most callers
    /// actually have.
    ///
    /// Consumers that must distinguish *which* concealment is being
    /// pierced (the attack-mode suppression clauses, `viewer_can_see`)
    /// take `concealment_piercing_of` instead and ask it per condition:
    /// a See Invisibility holder pierces `Invisible` but not `Blurred`,
    /// which a bool can't express.
    pub fn pierces_illusion_of(
        &self,
        viewer_id: usize,
        subject_id: usize,
    ) -> bool {
        self.concealment_piercing_of(viewer_id, subject_id) == ConcealmentPiercing::All
    }

    /// How much of `subject`'s concealment `viewer` sees through.
    /// Unifies every "sees through illusion" source in the engine into
    /// one directed lookup, tiered by how much RAW lets each one see:
    ///
    /// **`All`** — the whole `countered_by_truesight` cohort:
    ///   - **Truesight** — sense (`SpecialSense::Truesight(_)` on Deva,
    ///     Solar, Pit Fiend, Lich, Kraken, Nalfeshnee, etc.) OR
    ///     transient `TrueSighted` condition (True Seeing spell / Eyes
    ///     of Truth trinket). Unbounded range.
    ///   - **Feral Senses** (Ranger lv18 class feature). Unbounded range.
    ///   - Every range-gated non-visual sense — Blindsight,
    ///     Tremorsense, Rogue Blindsense, Blind Fighting — through the
    ///     shared `nonvisual_sense_reaches` envelope. See that helper
    ///     for each one's radius and gate.
    ///
    /// **`Invisibility`** — the `countered_by_see_invisibility` cohort
    /// only (`Invisible`; Blur and Displacement still fool the viewer):
    ///   - **See Invisibility** (`SeeingInvisible` condition, from the
    ///     level-2 divination spell). Unbounded range.
    ///   - **The Third Eye** (Divination Wizard subclass lv10, which
    ///     RAW grants as See Invisibility). Unbounded range.
    ///
    /// **`None`** — everything else, including an unknown actor id, so
    /// callers stay free of `is_some_and` chains.
    ///
    /// Sources are checked strongest-first and the first hit wins:
    /// a viewer holding both Truesight and See Invisibility is simply
    /// `All`, which is what the tier ordering means. A new piercing
    /// source lands as one clause in the tier it belongs to, and every
    /// consumer picks it up through `ConcealmentPiercing::pierces`
    /// without touching a `matches!` at the call site.
    ///
    /// The "attacker → target" and "target → attacker" symmetry lives
    /// at the call site (both angles matter in `compute_attack_mode`):
    /// this helper takes an already-directed (viewer, subject) pair
    /// and stays polarity-neutral.
    /// True when `actor_id` is carrying an outline that burns its own
    /// invisibility off — 5e's *"the affected creature can't benefit
    /// from the Invisible condition"*, which Faerie Fire and Starry
    /// Wisp both print.
    ///
    /// The undirected counterpart of `concealment_piercing_of`, and
    /// deliberately not a `ConcealmentPiercing` tier: piercing is a
    /// property of a viewer's senses and this is a property of the
    /// subject's own skin. Nobody is doing the seeing, so there is no
    /// (viewer, subject) pair to hand it. See
    /// `Condition::suppresses_invisibility` for the cohort and for what
    /// the engine did before it existed.
    pub fn concealment_burned_off(&self, actor_id: usize) -> bool {
        self.actors.get(&actor_id).is_some_and(|a| {
            a.conditions()
                .keys()
                .any(Condition::suppresses_invisibility)
        })
    }

    pub fn concealment_piercing_of(
        &self,
        viewer_id: usize,
        subject_id: usize,
    ) -> ConcealmentPiercing {
        let Some(viewer) = self.actors.get(&viewer_id) else {
            return ConcealmentPiercing::None;
        };
        // Truesight and Feral Senses are unbounded-range so they short-
        // circuit before we look up the subject at all — saves a
        // hashmap lookup on the vastly more common attack-mode path
        // where neither actor holds the flag.
        if viewer.has_truesight() || viewer.has_feral_senses() {
            return ConcealmentPiercing::All;
        }
        // The invisibility-only tier is unbounded-range, so it can be
        // answered without looking the subject up. It sits *below* the
        // range-gated `All` sources: a rogue's Blindsense inside its
        // envelope is strictly better than See Invisibility, so the
        // envelope check has to come first and only fall through to
        // this tier on a miss.
        let sees_invisible = viewer.has_condition(Condition::SeeingInvisible)
            || viewer.has_passive_feature(crate::actions::class_features::THIRD_EYE_TAG);
        let fallback = if sees_invisible {
            ConcealmentPiercing::Invisibility
        } else {
            ConcealmentPiercing::None
        };
        // An unknown subject id (an actor removed mid-resolution) has
        // no position to measure against, so the range-gated tier
        // can't fire and the unbounded answer stands.
        let Some(subject) = self.actors.get(&subject_id) else {
            return fallback;
        };
        if nonvisual_sense_reaches(viewer, subject) {
            ConcealmentPiercing::All
        } else {
            fallback
        }
    }

    /// Compute the save-roll mode for an actor's ability save.
    /// - `Poisoned` imposes disadvantage on all ability-check saves.
    /// - `Restrained` imposes disadvantage on DEX saves specifically.
    pub fn compute_save_mode(
        &self,
        actor_id: usize,
        ability: crate::engine::types::AbilityScoreType,
    ) -> RollMode {
        self.save_mode_tally(actor_id, ability).resolve()
    }

    /// Every advantage and disadvantage source a save is subject to,
    /// as a tally rather than a resolved mode — the save lane's twin of
    /// `attack_mode_tally`, and split out for the same reason.
    ///
    /// The roll sites layer more sources on afterwards: a Heightened
    /// Spell's disadvantage, a spell text's own clause ("plants and
    /// water elementals have disadvantage on this saving throw"), and
    /// the `CASTER_SAVE_MODE_RIDERS` cohort, which can contribute
    /// several at once. Folding those onto a resolved mode loses
    /// whether the sweep had already seen one of each.
    fn save_mode_tally(
        &self,
        actor_id: usize,
        ability: crate::engine::types::AbilityScoreType,
    ) -> RollModeTally {
        use crate::conditions::Condition;
        use crate::engine::types::AbilityScoreType;
        let mut tally = RollModeTally::NONE;
        let Some(actor) = self.actors.get(&actor_id) else {
            return tally;
        };
        // Blanket save-mode cohorts — conditions that flip the mode
        // regardless of which ability the save rolls off of. Each list
        // is a one-line entry point for new conditions; the arm-by-arm
        // shape used to inline the individual condition checks four
        // times over.
        for c in BLANKET_SAVE_DISADVANTAGE_CONDITIONS {
            if actor.has_condition(*c) {
                tally.add(RollMode::Disadvantage);
            }
        }
        for c in BLANKET_SAVE_ADVANTAGE_CONDITIONS {
            if actor.has_condition(*c) {
                tally.add(RollMode::Advantage);
            }
        }
        // 5e exhaustion tier 3: "disadvantage on attack rolls and saving
        // throws". Off the blanket cohort because that table is keyed by
        // condition and this is keyed by tier — the flag is up from tier
        // 1, three rungs before this penalty is earned.
        if actor.exhaustion_level()
            >= crate::actors::actor_template::EXHAUSTION_ROLL_PENALTY_TIER
        {
            tally.add(RollMode::Disadvantage);
        }
        // 5e Sunlight Weakness / Hypersensitivity: "disadvantage on
        // attack rolls, ability checks, and saving throws" while in
        // sunlight. Off the blanket condition cohort for the same
        // reason exhaustion is — that table is keyed by condition, and
        // this is keyed by a template trait plus where the creature is
        // standing. The Sensitivity tier is deliberately excluded here:
        // a kobold in the sun swings badly and saves normally, and
        // `disadvantages_saves` is where that line is drawn once.
        if actor
            .sunlight_frailty()
            .is_some_and(|f| f.disadvantages_saves())
            && self.is_sunlit(actor.location())
        {
            tally.add(RollMode::Disadvantage);
        }
        // DEX-save cluster — every clause here gates on
        // `AbilityScoreType::Dexterity` in RAW so we branch once and
        // fold the individual condition / flag checks inside. Pre-cluster
        // this shape open-coded the `matches!(ability, Dexterity)` gate
        // four times over.
        if matches!(ability, AbilityScoreType::Dexterity) {
            // Restrained / Sphered envelope: disadvantage on DEX saves.
            // Both conditions share the "physically pinned" flavor — RAW
            // Restrained explicitly states the clause; Sphered (Resilient
            // Sphere) is also an immobilization envelope by extension.
            if actor.has_condition(Condition::Restrained)
                || actor.has_condition(Condition::Sphered)
            {
                tally.add(RollMode::Disadvantage);
            }
            // Dodge → advantage on DEX saves (5e).
            if actor.is_dodging() {
                tally.add(RollMode::Advantage);
            }
            // Haste → advantage on DEX saves; Slow → disadvantage on
            // DEX saves. Both clauses are DEX-specific per the 5e PHB.
            if actor.has_condition(Condition::Hasted) {
                tally.add(RollMode::Advantage);
            }
            if actor.has_condition(Condition::Slowed) {
                tally.add(RollMode::Disadvantage);
            }
        }
        // STR-save cluster. Every entry gates on Strength in RAW, so the
        // ability check happens once and the per-condition rows ride a
        // table — same shape as the blanket cohorts above, scoped to one
        // ability. Rage was the only member until the size lane arrived
        // and brought three more; a fourth "you are bigger / smaller than
        // you were" effect lands as one row.
        if matches!(ability, AbilityScoreType::Strength) {
            for (condition, effect) in STRENGTH_CHECK_AND_SAVE_MODE_CONDITIONS {
                if actor.has_condition(*condition) {
                    tally.add(*effect);
                }
            }
        }
        // Physical-save cluster — the clauses 5e writes as "Strength,
        // Dexterity, and Constitution saving throws", which is a unit
        // the STR and DEX clusters above cannot express between them
        // and which CON had no lane for at all. Same branch-once shape
        // as the mental cluster below it.
        if matches!(
            ability,
            AbilityScoreType::Strength
                | AbilityScoreType::Dexterity
                | AbilityScoreType::Constitution
        ) {
            for (condition, effect) in PHYSICAL_SAVE_MODE_CONDITIONS {
                if actor.has_condition(*condition) {
                    tally.add(*effect);
                }
            }
        }
        // Mental-save cluster — the INT / WIS / CHA counterpart of the
        // DEX and STR clusters above. Branch once on the ability, then
        // walk the signed cohort; see `MENTAL_SAVE_MODE_CONDITIONS` for
        // what each row is.
        if matches!(
            ability,
            AbilityScoreType::Intelligence
                | AbilityScoreType::Wisdom
                | AbilityScoreType::Charisma
        ) {
            for (condition, effect) in MENTAL_SAVE_MODE_CONDITIONS {
                if actor.has_condition(*condition) {
                    tally.add(*effect);
                }
            }
        }
        // Passive-feature-driven save-advantage cohort — Magic Resistance
        // (all abilities), Dwarven Resilience (CON), Gnome Cunning (INT /
        // WIS / CHA), Barbarian Danger Sense (DEX with sensory gate) all
        // live on the shared `FLAG_DRIVEN_SAVE_ADVANTAGES` cohort in
        // `actor_template.rs`. Adding a new passive save-advantage
        // feature (a future Land's Stride save rider, an Undying-patron
        // Aspect of the Sun, etc.) lands as one cohort row rather than
        // another if-branch here.
        if actor.has_flag_driven_save_advantage(ability) {
            tally.add(RollMode::Advantage);
        }
        tally
    }

    /// Every clause in the game that promotes an ordinary hit against
    /// `target_id` into a critical one. Shared across weapon swings
    /// (`resolve_attack_outcome`) and spell attacks
    /// (`spell_attack_outcome`), because RAW words all of them as "any
    /// attack" or "any hit" and none of them as "any weapon attack".
    ///
    /// Two cohorts, and they differ on `is_melee`:
    ///
    ///   - **Paralyzed / Unconscious / Asleep** — "any attack that hits
    ///     the creature is a critical hit *if the attacker is within 5
    ///     feet*". `is_melee` gates the range clause by attack reach
    ///     rather than literal distance: a melee swing already implies
    ///     reach, and an explicit-distance check would have to know
    ///     each spell's effective range. That is the load-bearing
    ///     simplification which keeps callers from threading distance
    ///     everywhere.
    ///   - **Assassinate against a Surprised target** — RAW attaches no
    ///     range clause at all, so this one is not gated: an assassin's
    ///     crossbow bolt into an unaware sentry crits exactly as their
    ///     dagger does.
    ///
    /// The name lost its `melee_` for the second cohort's sake. It was
    /// accurate while every row on the list carried the five-foot
    /// clause; a caller reading `target_grants_melee_auto_crit(.., false)`
    /// and concluding the answer must be `false` would now be wrong.
    pub fn target_grants_auto_crit(
        &self,
        attacker_id: usize,
        target_id: usize,
        is_melee: bool,
    ) -> bool {
        if attacker_id == target_id {
            return false;
        }
        let Some(target) = self.actors.get(&target_id) else {
            return false;
        };
        // 5e Rogue Assassin **Assassinate**, second half: "any hit you
        // score against a surprised creature is a critical hit." Read
        // before the melee gate below, and not folded into it, because
        // RAW's clause says *any* hit — the assassin's crossbow bolt
        // counts and so does their Booming Blade. The rest of the
        // cohort is melee-only because the conditions on it are: RAW
        // grants the auto-crit on a Paralyzed or Unconscious target
        // only "if the attacker is within 5 feet".
        //
        // The first half of Assassinate — advantage against anything
        // that hasn't taken a turn yet — lives in `attack_mode_tally`,
        // and the two halves are deliberately separate: a creature can
        // be surprised without being first in the order, and can be
        // last in the order without ever having been surprised.
        if target.has_condition(Condition::Surprised)
            && self.actors.get(&attacker_id).is_some_and(|a| {
                a.has_passive_feature(crate::actions::class_features::ASSASSINATE_TAG)
            })
        {
            return true;
        }
        if !is_melee {
            return false;
        }
        // Stunned isn't on the RAW auto-crit list — only Paralyzed and
        // Unconscious carry the "any hit is a crit in melee" clause.
        // Petrified inherits Incapacitated but not the auto-crit rider
        // (RAW: "the creature is incapacitated... unaware of its
        // surroundings"). Asleep is modeled as Unconscious here for the
        // action-economy lockout but RAW does grant the same auto-crit
        // since natural unconsciousness applies.
        target.has_condition(Condition::Paralyzed)
            || target.has_condition(Condition::Unconscious)
            || target.has_condition(Condition::Asleep)
    }

    /// True if the actor auto-fails saves of the given ability. Paralyzed
    /// and Stunned auto-fail STR/DEX saves in 5e. Used by `roll_save` to
    /// short-circuit before the d20 roll. The "physical lock" cohort
    /// (Paralyzed / Stunned / Petrified / Unconscious / Asleep) lives on
    /// `Condition::auto_fails_str_dex_saves` so adding a new locked-out
    /// condition is a one-line change to the helper.
    pub fn auto_fail_save(
        &self,
        actor_id: usize,
        ability: crate::engine::types::AbilityScoreType,
    ) -> bool {
        use crate::conditions::Condition;
        use crate::engine::types::AbilityScoreType;
        let Some(actor) = self.actors.get(&actor_id) else {
            return false;
        };
        if !matches!(
            ability,
            AbilityScoreType::Strength | AbilityScoreType::Dexterity
        ) {
            return false;
        }
        // Dancing actors auto-fail DEX saves only — RAW: Otto's
        // Irresistible Dance is explicit about the DEX-save clause and
        // keeps STR / mental saves intact (mind is willing, body won't
        // cooperate). Handled outside the cohort so the STR-fail cohort
        // stays distinct.
        if actor.has_condition(Condition::Dancing)
            && matches!(ability, AbilityScoreType::Dexterity)
        {
            return true;
        }
        actor.conditions().keys().any(|c| c.auto_fails_str_dex_saves())
    }

    /// Footprint-gap radius of the Paladin's auras (Aura of Protection
    /// at level 6, Aura of Courage at level 10). 10 ft in 5e RAW; this
    /// engine's 2.5ft-tile grid puts that at a 4-tile gap from the
    /// paladin's footprint. Shared by the save-bonus and Frightened-
    /// immunity helpers so the radius lives in one place.
    pub const PALADIN_AURA_RADIUS: isize = 4;

    /// Iterate over `actor_id`'s allied aura emitters whose aura is
    /// currently projecting and whose footprint sits within
    /// `PALADIN_AURA_RADIUS` of `actor_id`'s footprint. `predicate` selects
    /// the aura flag of interest (`has_aura_of_protection` /
    /// `has_aura_of_courage`). Shared body for the two aura helpers — the
    /// per-aura check (return CHA mod / return any-hit) is folded in by
    /// the caller. The aura emitter must be combat-active and not
    /// incapacitated per RAW ("you must be conscious to grant this bonus").
    fn paladin_aura_emitters(
        &self,
        actor_id: usize,
        predicate: fn(&ActorInstance) -> bool,
    ) -> impl Iterator<Item = &ActorInstance> {
        self.aura_emitters(actor_id, predicate, AuraSide::Allied)
    }

    /// Team-agnostic body shared by `paladin_aura_emitters` (the five
    /// ally-facing auras) and `in_hostile_aura_of_conquest` (the one
    /// enemy-facing aura).
    ///
    /// Every clause except the team comparison is identical across the
    /// two, and the ones that are easy to forget are the ones that
    /// matter: an emitter has to be conscious *and* not incapacitated
    /// for its aura to project, which is RAW ("you must be conscious to
    /// grant this bonus") and is just as true of an aura that hurts
    /// people as of one that helps them. Lifting the body means the
    /// Conquest aura inherits those clauses instead of re-deriving
    /// them.
    fn aura_emitters(
        &self,
        actor_id: usize,
        predicate: fn(&ActorInstance) -> bool,
        side: AuraSide,
    ) -> impl Iterator<Item = &ActorInstance> {
        let target = self.actors.get(&actor_id);
        let target_team = target.map(|t| t.team());
        let target_loc = target.map(|t| t.location());
        let target_size = target.map(|t| get_tiles_from_size(t.size())).unwrap_or(1);
        self.actors.values().filter(move |paladin| {
            let Some(team) = target_team else {
                return false;
            };
            let Some(loc) = target_loc else {
                return false;
            };
            let side_ok = match side {
                AuraSide::Allied => paladin.team() == team,
                AuraSide::Hostile => paladin.team() != team,
            };
            predicate(paladin)
                && side_ok
                && paladin.is_combat_active()
                && !paladin.is_incapacitated()
                && footprint_chebyshev(
                    paladin.location(),
                    get_tiles_from_size(paladin.size()),
                    loc,
                    target_size,
                ) <= Self::PALADIN_AURA_RADIUS
        })
    }

    /// True if `actor_id` is standing inside the 10 ft Aura of Conquest
    /// of any *enemy* Conquest Paladin (Conquest subclass level 7).
    ///
    /// The mirror image of `is_in_aura_of_courage` and friends — same
    /// emitter model, opposite team filter. Read at
    /// `apply_aura_of_conquest`, which is where both of the aura's
    /// clauses (speed 0, psychic drip) land.
    pub fn in_hostile_aura_of_conquest(&self, actor_id: usize) -> bool {
        self.aura_emitters(
            actor_id,
            ActorInstance::has_aura_of_conquest,
            AuraSide::Hostile,
        )
        .next()
        .is_some()
    }

    /// 5e Paladin Aura of Protection bonus for `actor_id`'s saves.
    /// Returns the best CHA modifier (min +1, max +6 in 5e RAW —
    /// uncapped here since we don't track stat caps) of any aura-bearing
    /// ally within `PALADIN_AURA_RADIUS` tiles of the actor's footprint
    /// (including the actor themselves if they emit the aura). 0 if no
    /// aura-bearer is in range, the actor is unknown, or the aura-bearer
    /// is incapacitated (unconscious / dead — RAW: the aura requires the
    /// paladin to be conscious). Multiple paladins don't stack — the
    /// largest bonus wins.
    pub fn aura_of_protection_bonus(&self, actor_id: usize) -> i32 {
        self.paladin_aura_emitters(actor_id, ActorInstance::has_aura_of_protection)
            // 5e RAW: minimum +1 even with low CHA. Tracks how the SRD
            // writes the feature ("add your Charisma modifier (minimum
            // of +1)").
            .map(|p| p.ability_modifier(AbilityScoreType::Charisma).max(1))
            .max()
            .unwrap_or(0)
    }

    /// True if `actor_id` is inside the 10 ft Aura of Courage of any
    /// allied paladin (level 10+). Read by `ApplyCondition::apply` (the
    /// engine-aware install path that needs encounter geometry) to
    /// suppress the Frightened install on allies inside the bubble.
    /// Distinct from the actor-local `dynamic_immunity_to` check, which
    /// handles self-contained immunities (Heroism, Mind Blank, Halfling
    /// Brave) that don't need encounter context. The aura goes down with
    /// the paladin — combat-active filter mirrors the Aura of Protection
    /// helper.
    pub fn is_in_aura_of_courage(&self, actor_id: usize) -> bool {
        self.paladin_aura_emitters(actor_id, ActorInstance::has_aura_of_courage)
            .next()
            .is_some()
    }

    /// True if `actor_id` is inside the 10 ft Aura of Devotion of any
    /// allied Devotion Paladin (Devotion subclass level 7+). Read by
    /// `ApplyCondition::apply` to suppress the Charmed install on
    /// allies inside the bubble. Same emitter model as
    /// `is_in_aura_of_courage` (combat-active + not incapacitated + in
    /// range) — a downed / dominated paladin's aura goes dark.
    pub fn is_in_aura_of_devotion(&self, actor_id: usize) -> bool {
        self.paladin_aura_emitters(actor_id, ActorInstance::has_aura_of_devotion)
            .next()
            .is_some()
    }

    /// True if `actor_id` is inside the 10 ft Aura of Warding of any
    /// allied Ancients Paladin (Ancients subclass level 7+). Read by
    /// `DealDamage::apply` to halve the raw amount of spell-typical
    /// damage before the target's own resistance / immunity pipeline
    /// runs. Same emitter model as `is_in_aura_of_courage` (combat-
    /// active + not incapacitated + in range) — a downed paladin's
    /// aura goes dark.
    pub fn is_in_aura_of_warding(&self, actor_id: usize) -> bool {
        self.paladin_aura_emitters(actor_id, ActorInstance::has_aura_of_warding)
            .next()
            .is_some()
    }

    /// RAW's 30 ft on Flash of Genius, in tiles on the 2.5 ft grid.
    /// Wider than the paladin auras by a factor of three, which is the
    /// difference between "stand next to me" and "be in the room".
    pub const FLASH_OF_GENIUS_RADIUS: isize = 12;

    /// RAW's 60 ft on Feather Fall, in tiles on the 2.5 ft grid — twice
    /// the reach of Flash of Genius directly above, which is the
    /// difference between a rescue that needs the artificer in the room
    /// and one that only needs the wizard to be looking up.
    pub const FEATHER_FALL_RADIUS: isize = 24;

    /// 5e Artificer **Flash of Genius** (lv7): "whenever you or another
    /// creature you can see within 30 feet of you makes an ability
    /// check or a saving throw, you can use your reaction to add your
    /// Intelligence modifier to the roll."
    ///
    /// Called from `roll_save_with_extra_mode_and_bonus` once the
    /// initial d20 has landed on a fail and the add-die cohort has
    /// declined. Returns `Some(Pass)` if some allied artificer paid for
    /// the rescue, `None` if nobody could or nobody's modifier reached.
    ///
    /// **Why this isn't a row on `FAILED_SAVE_ADD_DIE_SOURCES`.** Every
    /// entry on that cohort is a charge the *failing actor* is
    /// carrying, which is why the loop can spend it with one
    /// `actors.get_mut(&actor_id)`. This one is a charge somebody else
    /// is carrying and a reaction somebody else has to still have, so
    /// the lookup is a scan of the board and the spend touches two
    /// actors. Folding it into the cohort would mean giving every row a
    /// "who pays" column that only one row would ever use.
    ///
    /// **What it costs, and why both halves matter.** The reaction
    /// means an artificer who has already fired one this round — a
    /// Shield, an opportunity attack, an Absorb Elements — has nothing
    /// to give, so the feature competes with the rest of the chassis
    /// rather than sitting outside the economy. The charge means one
    /// rescue per fight rather than one per save.
    ///
    /// **Spent only on a save it can rescue.** The shortfall gate is
    /// the same judgement the add-die cohort and `fire_missed_attack_boost`
    /// both make: RAW permits adding the modifier to a save it cannot
    /// save, and nobody holding the charge would.
    ///
    /// Ties are broken by picking the *largest* modifier among eligible
    /// artificers rather than the lowest id, so a party fielding two
    /// spends the one who can actually clear the gap; the id order is
    /// the secondary key so the choice stays deterministic under a
    /// fixed seed.
    fn try_flash_of_genius(
        &mut self,
        actor_id: usize,
        dc: i32,
        total: i32,
    ) -> Option<crate::engine::saves::SaveOutcome> {
        use crate::engine::saves::SaveOutcome;
        let shortfall = dc - total;
        let target = self.actors.get(&actor_id)?;
        let team = target.team();
        let loc = target.location();
        let size = get_tiles_from_size(target.size());
        let mut best: Option<(i32, usize)> = None;
        for (id, artificer) in self.actors.iter() {
            if artificer.team() != team
                || !artificer.is_combat_active()
                || artificer.is_incapacitated()
                || !artificer
                    .has_passive_feature(crate::actions::class_features::FLASH_OF_GENIUS_TAG)
                || !artificer
                    .feature_available(crate::actions::class_features::FLASH_OF_GENIUS_TAG)
                || !artificer.can_consume_resource(crate::engine::side_effects::Resource::Reaction)
            {
                continue;
            }
            // RAW's "a creature you can see". The artificer looks at
            // the roller, not the other way round — which matters for
            // an invisible ally, who cannot be helped.
            if *id != actor_id && !self.viewer_can_see(*id, actor_id) {
                continue;
            }
            if footprint_chebyshev(
                artificer.location(),
                get_tiles_from_size(artificer.size()),
                loc,
                size,
            ) > Self::FLASH_OF_GENIUS_RADIUS
            {
                continue;
            }
            let bonus = artificer.ability_modifier(AbilityScoreType::Intelligence);
            if bonus < shortfall {
                continue;
            }
            let candidate = (bonus, *id);
            if best.is_none_or(|(b, bid)| bonus > b || (bonus == b && *id < bid)) {
                best = Some(candidate);
            }
        }
        let (bonus, helper_id) = best?;
        let helper = self.actors.get_mut(&helper_id)?;
        helper.spend_feature(crate::actions::class_features::FLASH_OF_GENIUS_TAG);
        helper.consume_resource(crate::engine::side_effects::Resource::Reaction);
        let helper_name = self.actor_name(helper_id);
        self.log(format!(
            "  flash of genius: {} adds {:+} = {} vs DC {} \u{2014} pass",
            helper_name,
            bonus,
            total + bonus,
            dc
        ));
        Some(SaveOutcome::Pass)
    }

    /// 5e Hunter Ranger **Multiattack Defense** (Defensive Tactics
    /// option, lv7) AC bonus for the target's effective AC. Returns 4
    /// when the target holds `MULTIATTACK_DEFENSE_TAG` AND the
    /// `attacker_id` has already landed a connecting hit on
    /// `target_id` this turn; 0 otherwise. Read by both the weapon
    /// (`resolve_attack_outcome`) and spell (`spell_attack_outcome`)
    /// chokepoints so the +4 rider fires on any attack from a
    /// repeat-attacker, RAW.
    ///
    /// The "already hit me this turn" state lives on the attacker's
    /// `hit_targets_this_turn` set — cleared at the attacker's
    /// turn-start by `reset_for_new_round`, mirroring the RAW "rest
    /// of the turn" clause. Multiple attackers with hits into the
    /// same target each independently trigger their own +4 penalty
    /// (each attacker reads their own `hit_targets_this_turn`).
    pub fn multiattack_defense_ac_bonus(
        &self,
        attacker_id: usize,
        target_id: usize,
    ) -> i32 {
        let holds_tag = self
            .actors
            .get(&target_id)
            .is_some_and(|t| {
                t.has_passive_feature(
                    crate::actions::class_features::MULTIATTACK_DEFENSE_TAG,
                )
            });
        if !holds_tag {
            return 0;
        }
        if self
            .actors
            .get(&attacker_id)
            .is_some_and(|a| a.has_hit_target_this_turn(target_id))
        {
            4
        } else {
            0
        }
    }

    /// Roll a saving throw for `actor_id` against `dc` using `ability`.
    /// Auto-applies advantage / disadvantage based on the actor's
    /// conditions (see `compute_save_mode`). Missing actor auto-fails.
    /// Blessed actors get a fresh +1d4 added to the total per RAW.
    pub fn roll_save(
        &mut self,
        actor_id: usize,
        ability: crate::engine::types::AbilityScoreType,
        dc: i32,
    ) -> crate::engine::saves::SaveOutcome {
        self.roll_save_with_extra_mode(actor_id, ability, dc, RollModeTally::NONE)
    }

    /// `roll_save`, told what failing it would cost.
    ///
    /// The engine's saves used to be anonymous: an ability, a DC, and a
    /// d20. That is enough to resolve the roll and not enough to resolve
    /// the *features*, because 5e writes a whole family of them as
    /// "advantage on saving throws against being frightened" — against a
    /// condition, not against an ability. Every such feature therefore
    /// had to be rounded to something a save could see, and
    /// `CONDITION_SAVE_ADVANTAGES` is the docstring on what that cost.
    ///
    /// Call this instead of `roll_save` wherever the failure branch is
    /// an `ApplyCondition`, which the shared condition-install
    /// chokepoints now all do. An untagged site still works — it simply
    /// rolls without the cohort, which under-grants rather than
    /// over-grants (see the cohort docstring's "fails open" note).
    pub fn roll_save_vs_condition(
        &mut self,
        actor_id: usize,
        ability: crate::engine::types::AbilityScoreType,
        dc: i32,
        against: crate::conditions::Condition,
    ) -> crate::engine::saves::SaveOutcome {
        let extra = self.condition_save_tally(actor_id, Some(against));
        self.roll_save_with_extra_mode(actor_id, ability, dc, extra)
    }

    /// The `CONDITION_SAVE_ADVANTAGES` contribution to one save, as a
    /// tally rather than a mode — same reason `save_mode_tally` is one:
    /// this is merged with the caster-rider cohort and the target's own
    /// conditions before anything resolves, and folding it early would
    /// lose whether an advantage had been contributed at all.
    ///
    /// `None` — an untagged save — contributes nothing, which is what
    /// makes routing a site through the tagged lane a strict
    /// improvement rather than a behaviour swap.
    fn condition_save_tally(
        &self,
        actor_id: usize,
        against: Option<crate::conditions::Condition>,
    ) -> RollModeTally {
        let mut tally = RollModeTally::NONE;
        let Some(against) = against else {
            return tally;
        };
        if self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.has_save_advantage_against(against))
        {
            tally.add(RollMode::Advantage);
        }
        tally
    }

    /// Roll a save with extra advantage / disadvantage sources counted
    /// alongside the actor's condition-derived ones. Used by callers
    /// that have out-of-band reasons to skew the roll (Heightened Spell
    /// metamagic, a spell text's own clause, the caster-side rider
    /// cohort). Pass `RollModeTally::NONE` to get the same behaviour as
    /// `roll_save`.
    ///
    /// A tally rather than a `RollMode` because the caller can hold
    /// more than one — `roll_save_against_caster_at` walks a cohort —
    /// and because handing over an already-resolved mode is what let a
    /// cancelled pair look like no sources at all.
    fn roll_save_with_extra_mode(
        &mut self,
        actor_id: usize,
        ability: crate::engine::types::AbilityScoreType,
        dc: i32,
        extra: RollModeTally,
    ) -> crate::engine::saves::SaveOutcome {
        self.roll_save_with_extra_mode_and_bonus(actor_id, ability, dc, extra, 0, 0)
    }

    /// `roll_save_with_extra_mode` plus a flat bonus that applies to
    /// *this* save only.
    ///
    /// The distinction the existing bonus lanes couldn't make. Every
    /// other flat save bonus in the engine — `condition_save_bonus`,
    /// `save_bonus_buff`, the Aura of Protection sweep — applies to
    /// every save the holder rolls, because that is what the features
    /// feeding them say. The Bladesinger's Bladesong adds its
    /// Intelligence modifier to Constitution saves made *to maintain
    /// concentration* and to nothing else, so putting it on any of
    /// those lanes would over-grant it to every poison save and every
    /// Fireball the wizard ever ducks.
    /// `d20_floor` is the second call-site-scoped lane, and it is a
    /// different kind of thing from `call_site_bonus`: it raises the
    /// *die*, not the total, so a 3 on the d20 becomes a 10 before any
    /// modifier is added rather than after. Pass 0 to disable.
    ///
    /// The distinction matters because the features that grant it —
    /// the Circle of Stars Druid's Dragon constellation is the first —
    /// are worth nothing on a roll that was already going to clear the
    /// floor and are worth a great deal on the bad half of the die. A
    /// flat +N would smear the same value across every roll and would
    /// keep helping a save that rolled a 19, which is not what "counts
    /// as a 10" says.
    fn roll_save_with_extra_mode_and_bonus(
        &mut self,
        actor_id: usize,
        ability: crate::engine::types::AbilityScoreType,
        dc: i32,
        extra: RollModeTally,
        call_site_bonus: i32,
        d20_floor: u32,
    ) -> crate::engine::saves::SaveOutcome {
        use crate::engine::saves::SaveOutcome;

        // Paralyzed / Stunned auto-fail STR & DEX saves (5e). Log it so
        // the player can see why the save tanked.
        if self.auto_fail_save(actor_id, ability) {
            let name = self.actor_name(actor_id);
            self.log(format!(
                "  {} {:?} save: auto-fail (incapacitated)",
                name, ability
            ));
            return SaveOutcome::Fail;
        }

        let mut tally = self.save_mode_tally(actor_id, ability);
        tally.merge(extra);
        let mode = tally.resolve();
        // 5e Drunken Master Monk **Drunkard's Luck**: RAW names the
        // saving throw as one of the three contexts, so the cancel sits
        // on the final mode — after `extra_mode` has folded in, because
        // a Heightened Spell's disadvantage is exactly the kind of
        // disadvantage the feature exists to answer.
        let mode = self.steady_the_d20(actor_id, mode);
        // 5e Lucky: same nat-1 reroll hook as on attack rolls. RAW
        // explicitly lists "saving throw" as one of the trigger contexts.
        let rolled = self.roll_d20_lucky(actor_id, mode);
        // Apply the call-site die floor (Starry Form: Dragon) after
        // Lucky has had its say, so a rerolled 1 is floored on the
        // number the reroll actually produced. `raw` from here down is
        // the face the save is resolved on; `rolled` survives only for
        // the log, which shows both when they differ.
        let raw = rolled.max(d20_floor);
        let floor_suffix = if raw > rolled {
            format!("\u{2192}{}(dragon)", raw)
        } else {
            String::new()
        };
        // Bless / Bane rider — add or subtract 1d4 to the save total
        // (cancel out if both). Roll early so we can include the
        // breakdown in the log.
        let (extra, extra_suffix) = self.bless_bane_attack_die(actor_id);
        let Some(actor) = self.actors.get(&actor_id) else {
            return SaveOutcome::Fail;
        };
        // Everything the actor's own sheet contributes — ability
        // modifier, save proficiency, carried-item bonus, the
        // spell-installed `save_bonus_buff` and the condition-only flat
        // lane (a Bardic Inspiration die already handed over). Summed
        // behind `save_modifier` so the AI, which asks the same
        // question when it is picking which save to attack, cannot
        // drift from what the save actually rolls.
        let sheet_modifier = actor.save_modifier(ability);
        // One-shot save riders (Bardic Inspiration's +3, Unsettling
        // Words' −4). Their magnitudes are already folded into
        // `cond_save_bonus` above; what's captured here is which of
        // them this roll is spending, so the clear below the log line
        // can burn exactly those. Without the clear the same die pays
        // for a save *and* the swing that follows it — the attack path
        // has the mirror-image cohort in `CONSUMED_ON_ATTACK`.
        let spent_riders: Vec<Condition> = CONSUMED_ON_SAVE
            .iter()
            .copied()
            .filter(|&c| actor.has_condition(c))
            .collect();
        // 5e College of Eloquence Bard **Unfailing Inspiration**: an
        // Inspired die granted by that bard survives a roll it failed
        // to rescue. Read before the clear because the answer depends
        // on a back-link that the clear drops; acted on after it, so
        // the ordinary case pays no attention to it at all.
        let unfailing = self.unfailing_inspiration_granter(actor_id);
        // 5e Paladin Aura of Protection: every ally (and the paladin) within
        // 10ft adds the paladin's CHA mod (min +1) to all saves. Computed
        // outside the immutable borrow chain — we re-immut-borrow inside the
        // helper. Stacks via "best bonus wins" rather than summing so two
        // paladins don't double-pump every save.
        let aura_bonus = self.aura_of_protection_bonus(actor_id);
        // The two lanes no actor can answer alone ride on top: a nearby
        // paladin's aura, and whatever the feature rolling this save
        // brought with it.
        let modifier = sheet_modifier + aura_bonus + call_site_bonus;
        let total = raw as i32 + modifier + extra;
        let outcome = if total >= dc {
            SaveOutcome::Pass
        } else {
            SaveOutcome::Fail
        };
        let name = actor.name().to_string();
        self.log(format!(
            "  {} {:?} save: 1d20({}{}){:+}{} = {} vs DC {}{} \u{2014} {}",
            name,
            ability,
            rolled,
            floor_suffix,
            modifier,
            extra_suffix,
            total,
            dc,
            mode.log_suffix(),
            if outcome.passed() { "pass" } else { "fail" }
        ));
        // Burn the one-shot riders this roll spent. Done after the log
        // so the breakdown still credits their contribution.
        if !spent_riders.is_empty()
            && let Some(a) = self.actors.get_mut(&actor_id)
        {
            for c in &spent_riders {
                a.remove_condition(*c);
            }
        }
        // 5e Unfailing Inspiration: "if the roll fails, the creature can
        // keep the die." Hand it straight back — the clear above ran
        // unconditionally so the log still reads as a spend, which is
        // what happened.
        //
        // Judged against the save as first rolled, before the add-die
        // and reroll cohorts below get their shot at it. A d20 the
        // inspiration die failed to rescue is a failed roll at the
        // moment RAW asks the question; that a later charge drags the
        // same save over the line doesn't retroactively make the die
        // the thing that saved it.
        if !outcome.passed()
            && spent_riders.contains(&Condition::Inspired)
            && let Some(granter) = unfailing
        {
            self.refund_unfailing_inspiration(actor_id, granter);
        }
        // 5e "add die(s) to the failing save total" cohort — Fiend
        // Warlock Dark One's Own Luck (lv6, +1d10) and Divine Soul
        // Sorcerer Favored by the Gods (XGtE lv1, +2d4). RAW gates
        // on "after seeing the initial roll but before any of the
        // roll's effects occur" — fires on the initial d20's fail
        // *before* the reroll cohort so the added dice stack on the
        // d20 the holder just saw. Iteration stops as soon as one
        // source's boost produces a `Pass` (at most one add-die
        // charge burns per save); a failed boost still lets the
        // reroll cohort below take a shot at a fresh d20. On a
        // multiclass carrier the first-listed source fires first;
        // both current entries are once-per-short-rest auto-fire so
        // either order is defensible.
        //
        // Distinct from the reroll cohort in shape: this cohort
        // keeps the d20 and adds dice, so a d20(3) that Fanatical
        // Focus rerolls to another 3 stays failed, while the same
        // d20(3) that DOOL boosts picks up +1d10 (avg +5.5) and
        // pushes past most mid-DC saves.
        if !outcome.passed() {
            let mut boosted_pass: Option<SaveOutcome> = None;
            // How far under the DC the save landed. A source whose die
            // cannot reach that far is skipped rather than spent — the
            // same judgement `fire_missed_attack_boost` makes on the
            // attack lane, for the same reason: RAW permits adding the
            // die to a save it has no chance of rescuing, but nobody
            // holding the charge would, and here the engine is holding
            // it for them.
            let shortfall = dc - total;
            for source in FAILED_SAVE_ADD_DIE_SOURCES {
                if shortfall > source.dice.max_roll() as i32 {
                    continue;
                }
                let Some(actor) = self.actors.get_mut(&actor_id) else { break; };
                // `spend_feature` returns true iff a charge was actually
                // there to take, so no separate `feature_available`
                // check is needed — an unspent tag on the actor is
                // exactly the pass-gate case here. Pre-cleanup this ran
                // through a per-row `consume` closure that open-coded
                // the same check-then-spend body per row.
                if !actor.spend_feature(source.tag) {
                    continue;
                }
                let bonus = self.roll(&source.dice);
                let bonus_total = raw as i32 + modifier + extra + bonus as i32;
                let bonus_outcome = if bonus_total >= dc {
                    SaveOutcome::Pass
                } else {
                    SaveOutcome::Fail
                };
                self.log(format!(
                    "  {}: +{}({}) = {} vs DC {} \u{2014} {}",
                    source.label,
                    source.dice,
                    bonus,
                    bonus_total,
                    dc,
                    if bonus_outcome.passed() {
                        "pass"
                    } else {
                        "fail"
                    }
                ));
                if bonus_outcome.passed() {
                    boosted_pass = Some(bonus_outcome);
                    break;
                }
                // A failed boost from this source doesn't cascade
                // into the next add-die source — RAW's "you can use
                // this feature to add" fires once per save. Fall
                // through to the reroll cohort below.
                break;
            }
            if let Some(pass) = boosted_pass {
                return pass;
            }
        }
        // 5e Artificer **Flash of Genius** (lv7): a nearby artificer
        // spends their reaction and a charge to add their Intelligence
        // modifier to somebody else's failed save.
        //
        // Sits between the add-die cohort above and the reroll cohort
        // below because it is the same kind of thing as the first —
        // keep the d20, add to the total — and RAW puts it at the same
        // moment. What separates it is who pays: every row above is a
        // charge the failing actor is carrying, and this is a scan of
        // the board for someone else willing to spend a reaction. That
        // is why it is a method rather than a row.
        if !outcome.passed()
            && let Some(passed) = self.try_flash_of_genius(actor_id, dc, total)
        {
            return passed;
        }
        // 5e Fighter Indomitable + Oathbreaker Paladin Fanatical
        // Focus: each is a "reroll the failed save once per {long,
        // short} rest" gate. Indomitable is pre-primed (Action call
        // sets `mark_indomitable_pending` before the save); Fanatical
        // Focus auto-fires the first failed save while the paladin
        // holds an unspent charge. Both reroll shapes are identical
        // once the trigger fires — same lookup, same log line — so
        // one shared loop over `FAILED_SAVE_REROLL_SOURCES` handles
        // the pass through both sources; each source's "did we
        // consume?" gate lives inside its own `consume` closure.
        //
        // Order is significant: Indomitable fires first (it's pre-
        // primed by the actor's own choice, so a paladin/fighter
        // multiclass burns the pre-primed charge before the passive
        // one). Only ONE reroll fires per failed save regardless of
        // its outcome — RAW rerolls are once-per-save, not stack-
        // and-chain — so the loop breaks after the first consume
        // returns true. A missed reroll falls through to the shared
        // Legendary Resistance gate below so a boss-tier paladin
        // still gets LR as a third layer.
        // A reroll replaces the d20 and nothing else, so the best it
        // can produce is `20 + modifier + extra`. Against a DC above
        // that, the reroll is a formality with a charge attached — the
        // same judgement the add-die cohort above and
        // `fire_missed_attack_boost` on the attack lane both make. A
        // fighter who has been hit by something they cannot save
        // against at all should still be holding Indomitable when they
        // meet something they can.
        let reroll_could_pass = 20 + modifier + extra >= dc;
        if !outcome.passed() && reroll_could_pass {
            for source in FAILED_SAVE_REROLL_SOURCES {
                let Some(actor) = self.actors.get_mut(&actor_id) else { break; };
                if !(source.consume)(actor) {
                    continue;
                }
                // Reroll routes through `roll_d20_lucky` for the
                // same reason the initial save does — a nat-1 on
                // the reroll still triggers Lucky for a Halfling
                // holder.
                let reroll = self.roll_d20_lucky(actor_id, mode);
                let reroll_total = reroll as i32 + modifier + extra;
                let reroll_outcome = if reroll_total >= dc {
                    SaveOutcome::Pass
                } else {
                    SaveOutcome::Fail
                };
                self.log(format!(
                    "  {} reroll: 1d20({}){:+}{} = {} \u{2014} {}",
                    source.label,
                    reroll,
                    modifier,
                    extra_suffix,
                    reroll_total,
                    if reroll_outcome.passed() {
                        "pass"
                    } else {
                        "fail"
                    }
                ));
                if reroll_outcome.passed() {
                    return reroll_outcome;
                }
                // Only one reroll per failed save — RAW rerolls
                // don't chain, so a failed Indomitable reroll
                // doesn't cascade into Fanatical Focus. Break out
                // of the cohort iteration and fall through to the
                // Legendary Resistance gate below.
                break;
            }
        }
        // 5e Legendary Resistance: on a fail, boss-tier creatures may
        // choose to succeed instead. We spend a charge whenever a fail
        // would land — simplest "always burn" heuristic. Tarrasques /
        // dragons / etc. have a small pool (3-5) so this still rationing
        // itself; over-eager spending is more forgiving for the AI than
        // hoarding charges and letting Power Word Stun land on round 1.
        if !outcome.passed()
            && self
                .actors
                .get_mut(&actor_id)
                .is_some_and(|a| a.consume_legendary_resistance())
        {
            let remaining = self
                .actors
                .get(&actor_id)
                .map(|a| a.legendary_resistance_remaining())
                .unwrap_or(0);
            self.log(format!(
                "  {} invokes legendary resistance ({} remaining) \u{2014} pass",
                name, remaining
            ));
            return SaveOutcome::Pass;
        }
        outcome
    }

    /// True iff `actor_id` is currently concentrating on a spell of the
    /// given school.
    ///
    /// The school is resolved by name off the actor's own action list
    /// rather than stored on `ConcentrationData`. That keeps
    /// `Action::school()` the single source of truth — a spell's school
    /// is declared exactly once, on its impl — and it means a future
    /// concentration spell can never install a mark carrying the wrong
    /// school, because it never carries one at all. The alternative
    /// (a `school` field on `ConcentrationData`) would have to be set
    /// correctly at each of the ~100 install sites, and a missed one
    /// would fail silently.
    ///
    /// The lookup is sound because a caster necessarily holds the spell
    /// they are concentrating on: `ConcentrationData::spell_name` is
    /// written from the casting action's own `name()`, and that action
    /// came off this actor's list. An untagged spell reads `None` and
    /// fails the comparison closed.
    pub fn concentrating_on_school(&self, actor_id: usize, school: SpellSchool) -> bool {
        let Some(actor) = self.actors.get(&actor_id) else {
            return false;
        };
        let Some(conc) = actor.concentration() else {
            return false;
        };
        actor
            .find_action(&conc.spell_name)
            .and_then(|a| a.school())
            .is_some_and(|s| s == school)
    }

    /// Roll a Constitution save to maintain concentration vs the
    /// post-mitigation damage DC (max(10, dmg/2)). Honors the 5e Warlock
    /// **Eldritch Mind** invocation: actors holding the
    /// `ELDRITCH_MIND_TAG` feature roll with an extra advantage layer,
    /// folded on top of any condition-derived mode. Routed through one
    /// helper so the concentration-save tag check lives in one place
    /// rather than every damage site re-implementing it.
    pub fn roll_concentration_save(
        &mut self,
        actor_id: usize,
        dc: i32,
    ) -> crate::engine::saves::SaveOutcome {
        use crate::actions::class_features::ELDRITCH_MIND_TAG;
        use crate::engine::types::AbilityScoreType;
        // 5e Conjuration Wizard **Focused Conjuration** (subclass level
        // 10): "your concentration can't be broken as a result of
        // taking damage" while concentrating on a conjuration spell.
        // RAW makes no save at all rather than granting an auto-pass,
        // so this short-circuits ahead of the roll — which also keeps
        // the feature from perturbing the seeded RNG stream for every
        // *other* actor in the encounter, the same property the
        // Diviner's Portent substitution is pinned on.
        //
        // Only the damage lane is covered, which is exactly RAW's
        // wording and is why the gate sits here rather than in
        // `drop_concentration`: casting a second concentration spell,
        // dying, or failing a Hold Person-style round-end save all
        // still end the conjuration.
        if self.concentrating_on_school(actor_id, SpellSchool::Conjuration)
            && self
                .actors
                .get(&actor_id)
                .is_some_and(|a| {
                    a.has_passive_feature(crate::actions::class_features::FOCUSED_CONJURATION_TAG)
                })
        {
            let name = self.actor_name(actor_id);
            self.log(format!(
                "  focused conjuration: {}'s concentration holds through the damage",
                name
            ));
            return crate::engine::saves::SaveOutcome::Pass;
        }
        let mut extra = RollModeTally::NONE;
        extra.add_if(
            self.actors
                .get(&actor_id)
                .is_some_and(|a| a.feature_available(ELDRITCH_MIND_TAG)),
            RollMode::Advantage,
        );
        // 5e Bladesinging Wizard **Bladesong**: "you gain a bonus to
        // Constitution saving throws you make to maintain your
        // concentration on a spell" equal to the wizard's Intelligence
        // modifier. Scoped to this call site rather than to a condition
        // cohort because RAW scopes it to this one kind of save — see
        // `roll_save_with_extra_mode_and_bonus`.
        //
        // It is also the clause that makes the subclass coherent. A
        // wizard in melee is a wizard whose Haste or Greater
        // Invisibility is about to be knocked out of them; the AC bump
        // reduces how often they are hit and this reduces what a hit
        // costs when it lands.
        let bladesong_bonus = self
            .actors
            .get(&actor_id)
            .filter(|a| a.has_condition(Condition::Bladesinging))
            .map(|a| a.ability_modifier(AbilityScoreType::Intelligence).max(1))
            .unwrap_or(0);
        // 5e Circle of Stars Druid **Starry Form: Dragon**: "whenever
        // you make an Intelligence or Wisdom check or a Constitution
        // saving throw to maintain concentration, you can treat a roll
        // of 9 or lower on the d20 as a 10." Scoped to this call site
        // for the same reason Bladesong's bonus is — RAW names this one
        // kind of save, and the two ability checks in the other half of
        // the clause have no combat surface in this engine.
        //
        // It sits on the die rather than on the total (see
        // `d20_floor`), which is what makes it the defensive answer to
        // the Moon Druid's offensive one: a Stars druid holding
        // Moonbeam through a round of focused fire keeps it, and a
        // druid who was going to make the save anyway gains nothing.
        let dragon_floor = if self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.has_condition(Condition::StarryFormDragon))
        {
            10
        } else {
            0
        };
        self.roll_save_with_extra_mode_and_bonus(
            actor_id,
            AbilityScoreType::Constitution,
            dc,
            extra,
            bladesong_bonus,
            dragon_floor,
        )
    }

    /// 5e Barbarian **Relentless Rage** intercept. Called from
    /// `DealDamage::apply` when a barbarian's damage outcome is `Downed`
    /// and the holder is still raging. Rolls a CON save against the
    /// actor's per-rest DC (starts at 10, climbs by 5 each successful
    /// use, resets on short / long rest); on a pass, snaps the holder
    /// back to 1 HP and bumps the DC for the next attempt. Returns
    /// `true` if the actor was revived (caller skips the unconscious
    /// log / concentration drop), `false` otherwise (fall through to
    /// the normal Downed handling).
    ///
    /// Gated on Raging + RELENTLESS_RAGE_TAG so a non-raging barbarian
    /// — or any non-barbarian — short-circuits to `false` without
    /// rolling. Distinct from Death Ward / Relentless Endurance (which
    /// fire automatically and never roll): Relentless Rage genuinely
    /// rolls the save, and a failed roll lets the barbarian go down
    /// without burning anything.
    pub fn try_relentless_rage(&mut self, actor_id: usize) -> bool {
        use crate::actions::class_features::RELENTLESS_RAGE_TAG;
        use crate::conditions::Condition;
        use crate::engine::types::AbilityScoreType;

        let (eligible, dc) = match self.actors.get(&actor_id) {
            Some(a) => (
                a.has_passive_feature(RELENTLESS_RAGE_TAG) && a.has_condition(Condition::Raging),
                a.relentless_rage_dc() as i32,
            ),
            None => (false, 0),
        };
        if !eligible {
            return false;
        }
        let name = self.actor_name(actor_id);
        self.log(format!(
            "  {} fights against the brink — relentless rage CON save vs DC {}",
            name, dc
        ));
        let save = self.roll_save(actor_id, AbilityScoreType::Constitution, dc);
        if !save.passed() {
            return false;
        }
        let new_dc = if let Some(actor) = self.actors.get_mut(&actor_id) {
            actor.revive_at_one_hp();
            actor.bump_relentless_rage_dc();
            actor.relentless_rage_dc()
        } else {
            return false;
        };
        self.log(format!(
            "  {} refuses to fall — relentless rage pins HP at 1 (next DC {})",
            name, new_dc
        ));
        true
    }

    /// 5e **Undead Fortitude** intercept. Called from the damage
    /// chokepoint when a blow reduces the holder to 0 hit points, from
    /// *both* of the branches that can mean: `Downed` for a creature
    /// that rolls death saves and `Killed` for one that does not.
    ///
    /// RAW: *"it makes a Constitution saving throw (DC 5 plus the
    /// damage taken) unless the damage is Radiant or from a Critical
    /// Hit. On a successful save, the zombie drops to 1 Hit Point
    /// instead."* Returns true when the zombie got back up, and the
    /// caller then skips everything it would have done about a corpse.
    ///
    /// Sibling of `try_relentless_rage` on the same lane and
    /// deliberately shaped like it — both are "a save, at the moment
    /// the creature would fall, to stand at 1 HP instead" — but the two
    /// differ in the way that matters to a party. The barbarian's DC
    /// climbs with each use and resets on a rest, so it is a resource;
    /// the zombie's is priced off the *blow*, so it is a property of
    /// how hard you hit it. A zombie finished off by a dagger rolls
    /// against DC 8 and gets up; one finished off by a maul rolls
    /// against DC 19 and does not. That is the trait teaching a party
    /// to commit.
    ///
    /// **The Critical Hit exemption is not modeled.** `DealDamage`
    /// carries the amount and the type and does not carry whether the
    /// swing that produced it crit — the damage pipeline is
    /// deliberately decoupled from the attack roll, and hundreds of
    /// sites construct a `DealDamage` from things that are not attacks
    /// at all. Threading a crit flag through all of them to reach one
    /// stat block's clause would cost more than the clause is worth.
    /// The radiant half of the exemption ships, which is the half a
    /// party can play toward: a cleric's Sacred Flame puts a zombie
    /// down and stays down.
    pub fn try_undead_fortitude(
        &mut self,
        actor_id: usize,
        damage_type: crate::engine::types::DamageType,
        damage: u32,
    ) -> bool {
        use crate::actions::class_features::UNDEAD_FORTITUDE_TAG;
        use crate::engine::types::{AbilityScoreType, DamageType};

        // Radiant is RAW's own exemption and the reason a party carries
        // a cleric. Checked before the tag so the common case — a
        // creature that is not a zombie — still costs one hash lookup.
        if damage_type == DamageType::Radiant {
            return false;
        }
        if !self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.has_passive_feature(UNDEAD_FORTITUDE_TAG))
        {
            return false;
        }
        // RAW's "DC 5 plus the damage taken", read off the
        // post-mitigation number — the one the creature actually took,
        // which is what the sentence says.
        let dc = 5 + damage as i32;
        let name = self.actor_name(actor_id);
        self.log(format!(
            "  {} will not lie down \u{2014} undead fortitude CON save vs DC {}",
            name, dc
        ));
        let save = self.roll_save(actor_id, AbilityScoreType::Constitution, dc);
        if !save.passed() {
            return false;
        }
        if let Some(actor) = self.actors.get_mut(&actor_id) {
            actor.revive_at_one_hp();
        } else {
            return false;
        }
        self.log(format!("  {} gets back up with 1 HP.", name));
        true
    }

    /// Roll a saving throw attributed to `caster_id`'s spell. Identical to
    /// `roll_save` except it walks the shared `CASTER_SAVE_MODE_RIDERS`
    /// cohort first: any row whose gate fires for this (caster, target)
    /// pair folds its mode into the roll and consumes its one-shot
    /// prime.
    ///
    /// Damaging spells that already thread `caster_id` (almost every
    /// save-for-half AoE in `resolve_burst_save_damage`, and single-
    /// target lockdown spells like Hold Person / Hold Monster /
    /// Dominate Person) should route their saves through this helper so
    /// the cohort actually bites. Spells that don't bother with a
    /// caster_id (NPC-only attack riders, environmental DoTs) fall
    /// through to `roll_save` and no rider engages.
    ///
    /// Rows combine via `RollMode::combine`, so a Heightened Spell cast
    /// at an Eldritch-Struck target still resolves at a single notch of
    /// disadvantage (5e never stacks advantage/disadvantage) — but both
    /// primes are consumed, which is the RAW-correct bookkeeping for two
    /// independent once-each riders that both found their trigger.
    pub fn roll_save_against_caster(
        &mut self,
        target_id: usize,
        ability: crate::engine::types::AbilityScoreType,
        dc: i32,
        caster_id: usize,
    ) -> crate::engine::saves::SaveOutcome {
        self.roll_save_against_caster_at(target_id, ability, dc, caster_id, RollMode::Normal)
    }

    /// `roll_save_against_caster` with a notch the *spell* supplies.
    ///
    /// The riders on `CASTER_SAVE_MODE_RIDERS` are properties of the
    /// caster and the target; this parameter is a property of the spell
    /// text, and 5e has a steady trickle of them — Abi-Dalzim's Horrid
    /// Wilting's "plants and water elementals have disadvantage on this
    /// saving throw" is the shape. Counted into the same
    /// `RollModeTally` as the rider rows and as the target's own
    /// conditions, so a per-spell disadvantage, a Heightened Spell
    /// prime and a Magic Resistance advantage all resolve at a single
    /// notch, per RAW.
    ///
    /// Passing `RollMode::Normal` is exactly `roll_save_against_caster`,
    /// which is how that function is written.
    pub fn roll_save_against_caster_at(
        &mut self,
        target_id: usize,
        ability: crate::engine::types::AbilityScoreType,
        dc: i32,
        caster_id: usize,
        spell_mode: RollMode,
    ) -> crate::engine::saves::SaveOutcome {
        self.roll_save_against_caster_tagged(target_id, ability, dc, caster_id, spell_mode, None)
    }

    /// `roll_save_against_caster`, told what failing it would cost — the
    /// caster-aware twin of `roll_save_vs_condition`, and the entry
    /// point the spell-side condition installers use.
    ///
    /// It has to be its own call rather than a `roll_save_vs_condition`
    /// wrapped around `roll_save_against_caster` because both cohorts
    /// contribute to the *same* tally: a Heightened Spell's
    /// disadvantage and a countercharmed listener's advantage cancel to
    /// one straight roll, per RAW, and two nested calls would resolve
    /// each to a mode separately and roll twice.
    pub fn roll_save_against_caster_vs_condition(
        &mut self,
        target_id: usize,
        ability: crate::engine::types::AbilityScoreType,
        dc: i32,
        caster_id: usize,
        against: crate::conditions::Condition,
    ) -> crate::engine::saves::SaveOutcome {
        self.roll_save_against_caster_tagged(
            target_id,
            ability,
            dc,
            caster_id,
            RollMode::Normal,
            Some(against),
        )
    }

    /// Shared body of the caster-aware save lane: the spell's own notch,
    /// the `CASTER_SAVE_MODE_RIDERS` cohort, and the
    /// `CONDITION_SAVE_ADVANTAGES` cohort all folded into one tally
    /// before a single die is rolled.
    fn roll_save_against_caster_tagged(
        &mut self,
        target_id: usize,
        ability: crate::engine::types::AbilityScoreType,
        dc: i32,
        caster_id: usize,
        spell_mode: RollMode,
        against: Option<crate::conditions::Condition>,
    ) -> crate::engine::saves::SaveOutcome {
        let mut extra = self.condition_save_tally(target_id, against);
        extra.add(spell_mode);
        for rider in CASTER_SAVE_MODE_RIDERS {
            if !(rider.applies)(self, caster_id, target_id) {
                continue;
            }
            // Consume the prime up front so a multi-target spell only
            // forces the rider on its *first* save (RAW for Heightened
            // Spell: "The first time the target makes a saving throw
            // against the spell, the target has disadvantage"; the same
            // once-per-trigger shape covers the sibling rows).
            // Subsequent saves in the same cast fall through clean.
            (rider.consume)(self, caster_id, target_id);
            let target_name = self.actor_name(target_id);
            self.log(format!(
                "  {}: {} rolls the save at {}",
                rider.label,
                target_name,
                match rider.mode {
                    RollMode::Advantage => "advantage",
                    RollMode::Disadvantage => "disadvantage",
                    RollMode::Normal => "normal",
                }
            ));
            extra.add(rider.mode);
        }
        // `NONE`, not "resolved to Normal". A rider cohort that
        // contributed one advantage and one disadvantage cancels *at
        // the die*, together with whatever the target's own conditions
        // contribute — it does not mean nothing was contributed. Taking
        // the `roll_save` shortcut on a cancelled pair would have
        // dropped both flags and let a Poisoned target roll a save at
        // disadvantage that RAW says is straight.
        if extra == RollModeTally::NONE {
            return self.roll_save(target_id, ability, dc);
        }
        self.roll_save_with_extra_mode(target_id, ability, dc, extra)
    }

    /// Direct mutable handle to the encounter's general-purpose RNG. Used
    /// by content generation (terrain, actors) where dice abstraction
    /// doesn't fit. Do not call this from action side-effects — use `roll`.
    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }

    /// 5e ability check (skill check): roll 1d20 + ability modifier (+
    /// proficiency bonus if proficient in the given skill, or no skill
    /// passed). Returns the total. The check is a generic, non-save
    /// d20 — no automatic-fail conditions apply (unlike `roll_save`
    /// for Paralyzed / Stunned STR/DEX saves).
    ///
    /// Use this for skill-based mechanics where the holder rolls (e.g.
    /// an Athletics shove contest, a Stealth check vs a passive perception
    /// DC, an Investigation roll). Logs the breakdown.
    /// Advantage / disadvantage state of an ability check `actor_id` is
    /// about to roll. The check-lane sibling of `compute_save_mode`, and
    /// the reason `roll_ability_check` is a real d20 roll rather than a
    /// flat one.
    ///
    /// Every clause 5e writes as "…on ability checks" now lands here:
    /// the blanket cohorts (`BLANKET_CHECK_{DIS,}ADVANTAGE_CONDITIONS`),
    /// the shared Strength table the save lane also reads, Feeblemind's
    /// mental-ability scoping, and exhaustion's first tier — which is
    /// *only* a check penalty and had nowhere to live before this
    /// function existed.
    pub fn compute_check_mode(
        &self,
        actor_id: usize,
        ability: crate::engine::types::AbilityScoreType,
    ) -> RollMode {
        use crate::conditions::Condition;
        use crate::engine::types::AbilityScoreType;
        let mut tally = RollModeTally::NONE;
        let Some(actor) = self.actors.get(&actor_id) else {
            return RollMode::Normal;
        };
        for c in BLANKET_CHECK_DISADVANTAGE_CONDITIONS {
            if actor.has_condition(*c) {
                tally.add(RollMode::Disadvantage);
            }
        }
        for c in BLANKET_CHECK_ADVANTAGE_CONDITIONS {
            if actor.has_condition(*c) {
                tally.add(RollMode::Advantage);
            }
        }
        // 5e exhaustion tier 1: "disadvantage on ability checks", and
        // nothing else until tier 3. This is the rung the ladder starts
        // on, and until the check lane existed it had nothing to bite.
        if actor.exhaustion_level()
            >= crate::actors::actor_template::EXHAUSTION_CHECK_DISADVANTAGE_TIER
        {
            tally.add(RollMode::Disadvantage);
        }
        if matches!(ability, AbilityScoreType::Strength) {
            for (condition, effect) in STRENGTH_CHECK_AND_SAVE_MODE_CONDITIONS {
                if actor.has_condition(*condition) {
                    tally.add(*effect);
                }
            }
        }
        // 5e Feeblemind: INT and CHA effectively drop to 1, so checks
        // rolled off the mental abilities suffer. Same scoping the save
        // lane applies, for the same reason — a feebleminded creature
        // can still shove someone over.
        if actor.has_condition(Condition::Feebled)
            && matches!(
                ability,
                AbilityScoreType::Intelligence
                    | AbilityScoreType::Wisdom
                    | AbilityScoreType::Charisma
            )
        {
            tally.add(RollMode::Disadvantage);
        }
        tally.resolve()
    }

    /// Roll one ability check: `1d20 + ability modifier + proficiency
    /// (if `skill` is one the actor is proficient in) + one-shot rider`,
    /// under the mode `compute_check_mode` derives.
    ///
    /// This used to be a bare `1d20 + modifier`. Nothing that 5e says
    /// about ability checks reached it — not Poisoned's disadvantage,
    /// not Rage's advantage on Strength checks, not exhaustion, not
    /// Bardic Inspiration, not Guidance (whose *only* RAW effect is on
    /// this roll), not Lucky, not Portent. It also had one caller, so
    /// the gap was invisible: the four contests in the game each rolled
    /// their own naked d20 beside it and skipped the skill proficiency
    /// they are named after.
    ///
    /// Routed through `roll_d20_lucky` so Portent substitution and the
    /// Lucky nat-1 reroll cover checks the way RAW says they do — both
    /// features name "attack roll, saving throw, or ability check" and
    /// were two-thirds implemented.
    pub fn roll_ability_check(
        &mut self,
        actor_id: usize,
        ability: crate::engine::types::AbilityScoreType,
        skill: Option<crate::engine::types::Skill>,
    ) -> i32 {
        let mode = self.compute_check_mode(actor_id, ability);
        let raw = self.roll_d20_lucky(actor_id, mode) as i32;
        let Some(actor) = self.actors.get(&actor_id) else {
            return raw;
        };
        let prof = if skill
            .as_ref()
            .is_some_and(|s| actor.has_skill(s.clone()))
        {
            actor.proficiency_bonus()
        } else {
            0
        };
        // One-shot check riders (Bardic Inspiration's / Guidance's +3).
        // The magnitude is folded in by `condition_check_bonus`; what's
        // captured here is which rider this roll is spending, so the
        // clear below the log burns exactly those. Mirror image of the
        // attack and save lanes.
        let rider_bonus = actor.condition_check_bonus();
        let spent_riders: Vec<Condition> = CONSUMED_ON_CHECK
            .iter()
            .copied()
            .filter(|&c| actor.has_condition(c))
            .collect();
        let modifier = actor.ability_modifier(ability) + prof + rider_bonus;
        let total = raw + modifier;
        let label = match &skill {
            Some(s) => format!(" ({:?})", s),
            None => String::new(),
        };
        self.log(format!(
            "  {} {:?}{} check: 1d20({}){:+} = {}{}",
            actor.name(),
            ability,
            label,
            raw,
            modifier,
            total,
            mode.log_suffix(),
        ));
        if !spent_riders.is_empty()
            && let Some(a) = self.actors.get_mut(&actor_id)
        {
            for c in &spent_riders {
                a.remove_condition(*c);
            }
        }
        total
    }

    /// The best `(ability, skill)` pairing `actor_id` has among
    /// `options`, judged by the static bonus each would contribute.
    ///
    /// 5e lets the defender in a contest choose which of the offered
    /// skills to answer with ("Strength (Athletics) or Dexterity
    /// (Acrobatics), the target's choice"), and a defender picks the one
    /// they are best at. Proficiency is part of that comparison, which
    /// is why this can't just compare ability modifiers: a rogue with
    /// +2 DEX and Acrobatics proficiency answers a shove better than the
    /// same rogue's +3 STR without Athletics.
    ///
    /// Ties resolve to the earlier row, so a caller lists its preferred
    /// option first. Returns `None` only for an empty list or a missing
    /// actor.
    pub fn best_check_option(
        &self,
        actor_id: usize,
        options: &[(crate::engine::types::AbilityScoreType, crate::engine::types::Skill)],
    ) -> Option<(crate::engine::types::AbilityScoreType, crate::engine::types::Skill)> {
        let actor = self.actors.get(&actor_id)?;
        let score = |(ability, skill): &(
            crate::engine::types::AbilityScoreType,
            crate::engine::types::Skill,
        )| {
            let prof = if actor.has_skill(skill.clone()) {
                actor.proficiency_bonus()
            } else {
                0
            };
            actor.ability_modifier(*ability) + prof
        };
        // Written as a fold rather than `max_by_key` because the tie
        // rule is part of the contract and the two disagree:
        // `max_by_key` keeps the *last* of several equal maxima, and a
        // caller that listed its preferred option first would silently
        // get its least preferred one back.
        options.iter().fold(None, |best: Option<&(_, _)>, option| {
            match best {
                Some(current) if score(current) >= score(option) => Some(current),
                _ => Some(option),
            }
        })
        .cloned()
    }

    /// Resolve one 5e contested ability check: both sides roll through
    /// `roll_ability_check`, each with the best pairing they hold out of
    /// the options offered them, and the challenger wins ties (RAW: "if
    /// the contest results in a tie, the situation remains the same as
    /// it was before").
    ///
    /// **The** contest chokepoint. Shove, Grapple, and Escape each used
    /// to open-code `roll(&Dice::new(1,20)) + ability_modifier(...)`
    /// beside each other, which meant three copies of a rule that was
    /// wrong in the same three ways every time: no Athletics or
    /// Acrobatics proficiency (the skills the rules name explicitly), no
    /// advantage or disadvantage from anything, and no path for a rider
    /// like Guidance or Bardic Inspiration to reach a roll RAW says it
    /// covers. Routing all of them here fixes the rule once.
    ///
    /// Returns true when the challenger wins.
    pub fn roll_contest(
        &mut self,
        label: &str,
        challenger_id: usize,
        challenger_options: &[(crate::engine::types::AbilityScoreType, crate::engine::types::Skill)],
        defender_id: usize,
        defender_options: &[(crate::engine::types::AbilityScoreType, crate::engine::types::Skill)],
    ) -> bool {
        let Some((c_ability, c_skill)) =
            self.best_check_option(challenger_id, challenger_options)
        else {
            return false;
        };
        let Some((d_ability, d_skill)) = self.best_check_option(defender_id, defender_options)
        else {
            return false;
        };
        let challenger_name = self.actor_name(challenger_id);
        let defender_name = self.actor_name(defender_id);
        let challenger_roll = self.roll_ability_check(challenger_id, c_ability, Some(c_skill));
        let defender_roll = self.roll_ability_check(defender_id, d_ability, Some(d_skill));
        let won = challenger_roll >= defender_roll;
        self.log(format!(
            "  {}: {} {} vs {} {} \u{2014} {}",
            label,
            challenger_name,
            challenger_roll,
            defender_name,
            defender_roll,
            if won { "wins" } else { "loses" }
        ));
        won
    }

    /// Actor ids sorted ascending. Use when iteration order matters for
    /// determinism — e.g. AoE saves, splash sweeps, AI tiebreakers — since
    /// `HashMap` iteration order is non-deterministic across runs.
    pub fn sorted_actor_ids(&self) -> Vec<usize> {
        let mut ids: Vec<usize> = self.actors.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    pub fn get_actor(&mut self, actor_id: usize) -> Option<&mut ActorInstance> {
        self.actors.get_mut(&actor_id)
    }

    /// The row-major index of `coord`, or an error if it is not a tile
    /// on this map.
    ///
    /// The bound on `x` is load-bearing and used not to be there. A
    /// row-major index has no way to represent "one column past the
    /// right edge" — the arithmetic produces the index of the *next
    /// row's first tile*, which is a perfectly valid index into a
    /// perfectly wrong tile. Every reader built on this (`terrain_at`,
    /// `actor_id_at`, `set_terrain_at`) silently wrapped, and
    /// `find_path` indexed its distance vector with the result, where
    /// the bottom-right corner's overflow is one past the end of the
    /// vector.
    pub fn idx(&self, coord: Coordinate) -> Result<usize, OffMapCoord> {
        if !self.in_bounds(coord) {
            return Err(OffMapCoord::new(coord));
        }
        Ok(coord.x as usize + coord.y as usize * self.width)
    }

    /// True if `coord` names a tile that is actually on the map.
    ///
    /// The predicate `idx` is built on, and the one three callers were
    /// each spelling out as the same pair of comparisons.
    pub fn in_bounds(&self, coord: Coordinate) -> bool {
        coord.x >= 0
            && coord.y >= 0
            && (coord.x as usize) < self.width
            && (coord.y as usize) < self.height
    }

    pub fn is_spawnable(&self, coord: Coordinate) -> bool {
        if !self.in_bounds(coord) {
            return false;
        }

        if self.actor_id_at(coord).is_some() {
            return false;
        }

        matches!(self.terrain_at(coord), Some(ti) if ti.terrain_type.is_passable())
    }

    /// True if a `size` footprint anchored at `origin` would sit
    /// entirely on spawnable ground — in bounds, passable, and nobody
    /// else's tiles.
    ///
    /// The footprint-shaped `is_spawnable`, and the ownerless twin of
    /// `footprint_fits`: that one asks whether a *particular actor* can
    /// stand somewhere and forgives the tiles that actor already
    /// occupies, which is the right question for a move and the wrong
    /// one for a body arriving from nowhere. Shared by the summoner's
    /// ring walk and the banishment return, which each used to spell the
    /// double loop out.
    pub(crate) fn footprint_is_clear(&self, origin: Coordinate, size: Size) -> bool {
        let w = get_tiles_from_size(size) as isize;
        (0..w).all(|ox| {
            (0..w).all(|oy| self.is_spawnable(Coordinate::new(origin.x + ox, origin.y + oy)))
        })
    }

    fn can_move_to_subtile(&self, coord: Coordinate, actor_id: usize) -> bool {
        if !self.in_bounds(coord) {
            return false;
        }

        if let Some(other_id) = self.actor_id_at(coord)
            && other_id != actor_id {
                return false;
            }

        matches!(self.terrain_at(coord), Some(ti) if ti.terrain_type.is_passable())
    }

    fn get_random_coord_list(&mut self) -> Vec<Coordinate> {
        let mut all_coords: Vec<Coordinate> = Vec::new();
        for x in 0..self.width {
            for y in 0..self.height {
                all_coords.push(Coordinate::new(x as isize, y as isize));
            }
        }
        self.rng.shuffle(&mut all_coords);
        all_coords
    }

    pub fn get_random_spawn(&mut self, size: Size) -> Result<Coordinate, NoLegalPosition> {
        let actor_width: usize = get_tiles_from_size(size);
        let coords = self.get_random_coord_list();

        'coord_loop: for &coord in coords.iter() {
            for x_off in 0..actor_width {
                for y_off in 0..actor_width {
                    let offset = Coordinate::new(x_off as isize, y_off as isize);
                    if !self.is_spawnable(coord + offset) {
                        continue 'coord_loop;
                    }
                }
            }
            return Ok(coord);
        }
        Err(NoLegalPosition)
    }

    pub fn actor_id_at(&self, coord: Coordinate) -> Option<usize> {
        let idx = self.idx(coord).ok()?;
        self.actor_map.get(idx).copied().flatten()
    }

    fn set_actor_id_at(&mut self, actor_id: Option<usize>, coord: Coordinate) {
        if let Ok(idx) = self.idx(coord)
            && let Some(slot) = self.actor_map.get_mut(idx) {
                *slot = actor_id;
            }
    }

    /// Stamp `actor_id` (or `None` to clear) into every tile of the size's
    /// `width × width` footprint anchored at `origin`. Out-of-bounds offsets
    /// no-op (set_actor_id_at silently rejects them).
    fn write_footprint(
        &mut self,
        actor_id: Option<usize>,
        origin: Coordinate,
        size: Size,
    ) {
        let width = get_tiles_from_size(size);
        for x_off in 0..width {
            for y_off in 0..width {
                let offset = Coordinate::new(x_off as isize, y_off as isize);
                self.set_actor_id_at(actor_id, origin + offset);
            }
        }
    }

    /// Stamp `actor_id` onto the `size` footprint anchored at `origin`.
    ///
    /// The named half of `write_footprint`'s `Option` parameter, exposed
    /// to the mounted-combat lane — which is the only thing outside this
    /// module that puts a body back on the grid without moving it there,
    /// because a dismounting rider was never on the grid to move from.
    pub(crate) fn stamp_footprint_of(
        &mut self,
        actor_id: usize,
        origin: Coordinate,
        size: Size,
    ) {
        self.write_footprint(Some(actor_id), origin, size);
    }

    /// Release the `size` footprint anchored at `origin`. The erasing
    /// half of the pair above; `actor_id` is taken for symmetry and to
    /// document whose tiles are being freed.
    pub(crate) fn clear_footprint_of(
        &mut self,
        _actor_id: usize,
        origin: Coordinate,
        size: Size,
    ) {
        self.write_footprint(None, origin, size);
    }

    pub fn terrain_at(&self, coord: Coordinate) -> Option<&TerrainInfo> {
        let idx = self.idx(coord).ok()?;
        self.terrain.get(idx)
    }

    /// 5e Underwater Combat's "fully immersed" — is this actor *in* the
    /// water, rather than beside it or above it?
    ///
    /// The single predicate behind every underwater rule the engine
    /// enforces: the melee and ranged attack clauses in
    /// `engine::underwater`, and the fire resistance on the
    /// environmental-halving cohort in `engine::side_effects`. The
    /// movement surcharge is the one water rule that does *not* read it,
    /// because that one is charged per step taken rather than per tile
    /// stood on.
    ///
    /// Two clauses, and RAW puts the emphasis on the first:
    ///
    ///   - **Every tile of the footprint is water.** "Fully immersed",
    ///     not "touching water". A Huge kraken with two of its four
    ///     tiles on the shingle is hauled half out of the sea, and a
    ///     knight standing at the water's edge with one boot wet is not
    ///     swimming. Reading only the anchor tile would have made both
    ///     of those turn on which corner of the creature the engine
    ///     happens to store, which is not a rule anybody could play
    ///     around.
    ///
    ///   - **Flight lifts you out of it.** A creature aloft over a lake
    ///     is over it, and it makes no difference whether it got there
    ///     on a spell or on its own wings — a giant eagle is no wetter
    ///     than a wizard. Deliberately the same `is_airborne`
    ///     predicate `WATER_SURCHARGE_IMMUNITIES` reads, so the two
    ///     lanes cannot disagree about what counts — an actor that
    ///     crossed the water for free is exactly an actor the water has
    ///     no other hold on.
    ///
    /// A derived query rather than a stored condition on purpose. Every
    /// path that moves a creature would otherwise have to remember to
    /// resync it — and the engine has a dozen of them (a walk, a shove,
    /// a teleport, a summon's placement, a dismount's landing, a
    /// round-end drift) — so a stored flag would be correct only until
    /// the next one was added. Recomputing costs one terrain lookup per
    /// footprint tile, on paths that already do more work than that.
    ///
    /// Read through `movement_body`, like every other question about
    /// which tiles a creature is standing on. A rider has no footprint
    /// of its own while mounted — the horse's is the one on the board —
    /// so measuring the rider's own 2x2 span from the horse's anchor
    /// would read a sub-rectangle of the horse that nothing is actually
    /// occupying, and a Large mount straddling a shoreline could report
    /// its Medium rider as fully immersed while it was itself half out
    /// of the water.
    ///
    /// Note which half of the underwater rules this redirect covers and
    /// which it doesn't, because the split is deliberate and is RAW's.
    /// *Being in the water* is a fact about the body carrying you, so it
    /// is the mount's. *Having a swimming speed* is a fact about the
    /// creature swinging the sword, so it stays the rider's: a knight on
    /// a swimming horse is underwater and still doesn't know how to
    /// fight there.
    pub fn is_immersed(&self, actor_id: usize) -> bool {
        let Some(actor) = self.actors.get(&self.movement_body(actor_id)) else {
            return false;
        };
        // Off the surface, or on top of it. Flight was the only way out
        // of the water until Water Walk; RAW's "as if it were harmless
        // solid ground" puts the holder on the lake rather than in it,
        // which is what makes the spell more than a swimming speed —
        // the surcharge waiver alone would leave them swinging at
        // disadvantage and resisting fire while standing on the water.
        if actor.is_airborne() || actor.has_condition(Condition::WaterWalking) {
            return false;
        }
        let anchor = actor.location();
        let width = get_tiles_from_size(actor.size()) as isize;
        (0..width).all(|dx| {
            (0..width).all(|dy| {
                self.terrain_at(anchor + Coordinate::new(dx, dy))
                    .is_some_and(|t| t.terrain_type.is_water())
            })
        })
    }

    /// True if `actor_id` is drawing breath this round — the single
    /// gate on 5e's **Suffocation** hazard, read once per actor per
    /// round by `tick_breath`. See `engine::breath` for the rule.
    ///
    /// Three questions in a deliberate order, and the order is the
    /// rule rather than an optimisation:
    ///
    ///   1. **Is the airway blocked?** `Condition::Choking` — the
    ///      darkmantle over the face, the rug wrapped around the body.
    ///      Asked first because nothing waives it: RAW's Necklace of
    ///      Adaptation lets you breathe in any *environment*, and a
    ///      darkmantle is not an environment.
    ///   2. **Is the creature under water at all?** `is_immersed`,
    ///      which already answers correctly for a flier, a
    ///      water-walker, and a rider on a swimming mount. On a map
    ///      with no lake on it — which is most of them — this is where
    ///      every actor leaves, at the cost of one terrain read.
    ///   3. **Does it have the lungs for it?** `breathes_underwater` —
    ///      the stat-block line, and the necklace.
    ///
    /// Air is free, which is a scope cut and not a rule: RAW's nine
    /// water-breathing stat blocks suffocate out of the water and this
    /// engine lets them walk around. See `engine::breath` for why.
    ///
    /// Defaults to `true` for an id that is not on the board. An actor
    /// the map has lost is not one this rule should be quietly killing.
    pub fn can_breathe(&self, actor_id: usize) -> bool {
        let Some(actor) = self.actors.get(&actor_id) else {
            return true;
        };
        if actor.has_condition(Condition::Choking) {
            return false;
        }
        if !self.is_immersed(actor_id) {
            return true;
        }
        actor.breathes_underwater()
    }

    /// True if any tile on the board is water.
    ///
    /// Read by Water Walk's validator, which has no business spending a
    /// 3rd-level slot on a map with no lake in it. Deliberately not
    /// "is the caster standing in water" — the spell's whole use is the
    /// crossing you are about to make.
    pub fn has_water(&self) -> bool {
        self.terrain.iter().any(|t| t.terrain_type.is_water())
    }

    /// What 5e's Underwater Combat rules do to one attack — the shared
    /// chokepoint the die and the AI's attack picker both read, so the
    /// two can never disagree about whether a swing is worth making.
    ///
    /// `weapon_name` is the action's own name, which is what the two
    /// RAW weapon cohorts are keyed on; see
    /// `engine::underwater::names_weapon` for why.
    /// `beyond_normal_range` is the caller's, because the two callers
    /// measure it from different things: the attack site already holds
    /// `AttackParams::long_range` and a resolved footprint distance,
    /// while the picker asks the action for `normal_range` and the
    /// board for the gap.
    pub fn underwater_verdict(
        &self,
        attacker_id: usize,
        weapon_name: &str,
        is_melee: bool,
        is_weapon_attack: bool,
        beyond_normal_range: bool,
    ) -> UnderwaterVerdict {
        // Cheapest gate first, and by a wide margin: on a map with no
        // water on it — which is most of them — this is one hash lookup
        // and one terrain read, and nothing below it runs.
        if !self.is_immersed(attacker_id) {
            return UnderwaterVerdict::Unaffected;
        }
        let Some(attacker) = self.actors.get(&attacker_id) else {
            return UnderwaterVerdict::Unaffected;
        };
        UnderwaterVerdict::for_attack(AttackInWater {
            immersed: true,
            waived: attacker.underwater_penalties_waived(),
            swims: attacker.has_swim_speed(),
            is_weapon_attack,
            is_melee,
            weapon_name,
            beyond_normal_range,
        })
    }

    /// Retype one tile. Returns false — and changes nothing — for a
    /// coordinate off the map.
    ///
    /// The write counterpart to `terrain_at`. The generator has always
    /// been the only thing that writes the map, which was fine while the
    /// map was scenery and is the reason this didn't exist; but the
    /// terrain layer now carries rules (a movement surcharge, a cover
    /// bonus) that effects can plausibly want to place, and there was no
    /// door into it from outside this module. This is that door.
    pub fn set_terrain_at(&mut self, coord: Coordinate, terrain_type: TerrainType) -> bool {
        let Ok(idx) = self.idx(coord) else {
            return false;
        };
        let Some(tile) = self.terrain.get_mut(idx) else {
            return false;
        };
        tile.terrain_type = terrain_type;
        true
    }

    /// Every persistent area currently on the board, in install order.
    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }

    /// Place a persistent area and return the id it was given. The id is
    /// the handle the "first time on a turn" ledger and every teardown
    /// path key off; callers that only ever tear their zone down through
    /// concentration can discard it.
    ///
    /// Installing does *not* fire the contact clause. Every spell that
    /// creates one has its own answer for the creatures already standing
    /// there — Web and Grease save immediately, Cloud of Daggers does
    /// not — and folding one of those answers in here would make the
    /// other one impossible to write.
    pub fn install_zone(&mut self, mut zone: Zone) -> usize {
        let id = self.zone_id_next;
        self.zone_id_next += 1;
        zone.id = id;
        self.zones.push(zone);
        id
    }

    /// Every patch of conjured map currently standing, in install order.
    pub fn conjured_terrain(&self) -> &[ConjuredTerrain] {
        &self.conjured_terrain
    }

    /// Retype the tiles a spell asked for, remembering what was there,
    /// and return the handle the teardown paths key off.
    ///
    /// `patch.tiles` is the request; `patch.restore` is what was
    /// actually taken, and the two differ wherever a tile was off the
    /// map or the new terrain would have buried somebody. Both
    /// exclusions are silent by design — a wall raised across a corridor
    /// with a goblin in it is a wall with a goblin-shaped gap, which is
    /// the shape RAW's "the creature is pushed to one side" produces
    /// without needing a forced-movement resolution to reach.
    ///
    /// A patch that took no tiles at all is still installed, still
    /// expires, and is still torn down by its owner's concentration —
    /// which keeps "the spell is up" and "the spell got something" as
    /// separate questions rather than making a wall raised entirely off
    /// the map look like a wall that was never cast.
    pub fn conjure_terrain(&mut self, mut patch: ConjuredTerrain) -> usize {
        let id = self.conjured_terrain_id_next;
        self.conjured_terrain_id_next += 1;
        patch.id = id;
        let blocks_movement = !patch.terrain_type.is_passable();
        let requested = std::mem::take(&mut patch.tiles);
        for coord in requested {
            if !self.in_bounds(coord) {
                continue;
            }
            if blocks_movement && self.actor_id_at(coord).is_some() {
                continue;
            }
            let Some(was) = self.terrain_at(coord).map(|t| t.terrain_type) else {
                continue;
            };
            if was == patch.terrain_type {
                // Nothing to hold and nothing to hand back. Skipping
                // keeps the ledger honest: a patch that "restores" a
                // tile to what it already was owns a tile it never
                // changed, and would undo somebody else's write.
                continue;
            }
            self.set_terrain_at(coord, patch.terrain_type);
            patch.restore.push((coord, was));
        }
        let (name, taken) = (patch.name, patch.restore.len());
        self.conjured_terrain.push(patch);
        self.log(format!("  {} rises across {} tiles.", name, taken));
        id
    }

    /// Take a patch of conjured map back down, handing every tile it is
    /// still holding to whatever was there before. Returns true if one
    /// was there.
    ///
    /// A tile is only restored if it still carries the terrain this
    /// patch wrote. Anything else has been claimed since — by a second
    /// wall, or by a future terrain-mutating effect — and handing back
    /// a tile somebody else owns would turn their stone into this
    /// patch's floor.
    pub fn dispel_conjured_terrain(&mut self, id: usize) -> bool {
        let Some(index) = self.conjured_terrain.iter().position(|p| p.id == id) else {
            return false;
        };
        let patch = self.conjured_terrain.remove(index);
        for (coord, was) in patch.restore {
            if self.terrain_at(coord).map(|t| t.terrain_type) == Some(patch.terrain_type) {
                self.set_terrain_at(coord, was);
            }
        }
        true
    }

    /// Expire one round off every patch and take down the ones that ran
    /// out. Called from `round_end`, beside `tick_zones`.
    fn tick_conjured_terrain(&mut self) {
        let mut expired: Vec<(usize, String)> = Vec::new();
        // The terrain-layer twin of `tick_zones`'s bereaved list.
        let mut bereaved: Vec<usize> = Vec::new();
        for patch in self.conjured_terrain.iter_mut() {
            patch.rounds_remaining = patch.rounds_remaining.saturating_sub(1);
            if patch.rounds_remaining == 0 {
                expired.push((patch.id, patch.name.to_string()));
                if patch.concentration {
                    bereaved.push(patch.owner_id);
                }
            }
        }
        for (id, name) in expired {
            self.dispel_conjured_terrain(id);
            self.log(format!("The {} crumbles away.", name));
        }
        self.pending_concentration_review.append(&mut bereaved);
    }

    /// Take down every concentration-held patch `actor_id` is
    /// sustaining. The terrain-layer twin of
    /// `remove_concentration_zones_of`, called from the same chokepoint
    /// and for the same reason: `drop_concentration` is the one place
    /// that knows a caster's grip has failed, however it failed.
    fn remove_concentration_terrain_of(&mut self, actor_id: usize) {
        let doomed: Vec<(usize, String)> = self
            .conjured_terrain
            .iter()
            .filter(|p| p.concentration && p.owner_id == actor_id)
            .map(|p| (p.id, p.name.to_string()))
            .collect();
        for (id, name) in doomed {
            self.dispel_conjured_terrain(id);
            self.log(format!("The {} fades back into nothing.", name));
        }
    }

    /// The area `owner_id` is sustaining under the given name, if any.
    ///
    /// The handle the steered cohort re-enters through: Moonbeam cast a
    /// second time is not a second Moonbeam, it is the first one being
    /// walked somewhere, and the only way for the spell to tell those
    /// two cases apart is to ask whether its own beam is already up.
    /// Matched on `(owner, name)` rather than on a remembered id
    /// because an `Action` is a zero-sized static with nowhere to keep
    /// one — the name is the identity a spell carries.
    pub fn zone_sustained_by(&self, owner_id: usize, name: &str) -> Option<&Zone> {
        self.zones
            .iter()
            .find(|z| z.owner_id == owner_id && z.name == name)
    }

    /// Move a persistent area to `dest`, firing its contact clause at
    /// whoever it has newly covered.
    ///
    /// Returns false — and changes nothing — for an unknown id or a
    /// move that lands where the area already is.
    ///
    /// The "newly" is the whole rule. RAW's trigger for a moving area is
    /// the area arriving on somebody ("when you move the beam into a
    /// creature's space"), not the area being on them, so a creature
    /// the beam was already burning and stays on doesn't pay a second
    /// time for standing still — it pays again at the start of its own
    /// turn, through `touch_zones`, like every other creature in every
    /// other area. Symmetrically, a creature the area has *left* has
    /// its ledger row dropped, so a beam walked off a target and back
    /// on again in the same round bills for the second arrival.
    ///
    /// Charged in sorted-id order, the determinism every other
    /// multi-target site in the engine keeps.
    pub fn move_zone(&mut self, zone_id: usize, dest: Coordinate) -> bool {
        let Some(index) = self.zones.iter().position(|z| z.id == zone_id) else {
            return false;
        };
        if self.zones[index].origin == dest {
            return false;
        }
        let covered_before: Vec<usize> = self
            .sorted_actor_ids()
            .into_iter()
            .filter(|&id| self.actor_in_zone(id, &self.zones[index]))
            .collect();
        let name = self.zones[index].name;
        self.zones[index].origin = dest;
        self.log(format!("  {} moves to {}.", name, dest));
        let covered_after: Vec<usize> = self
            .sorted_actor_ids()
            .into_iter()
            .filter(|&id| self.actor_in_zone(id, &self.zones[index]))
            .collect();
        // Everyone the area is no longer over loses their claim on this
        // turn's ledger; everyone it has newly arrived on loses theirs
        // too, and then pays.
        for id in &covered_before {
            if !covered_after.contains(id) {
                self.zone_contacts_this_turn.remove(&(zone_id, *id));
            }
        }
        for id in covered_after {
            if covered_before.contains(&id) {
                continue;
            }
            self.zone_contacts_this_turn.remove(&(zone_id, id));
            self.touch_zone(zone_id, id);
        }
        true
    }

    /// Move every engine-run area `actor_id` is sustaining to where it
    /// belongs at the top of their turn: a `DriftsFromOwner` cloud one
    /// step further away, a `FollowsOwner` sphere back onto its owner.
    ///
    /// A cloud whose new centre has left the map is swept rather than
    /// tracked off-board: the two spells that drift both say the cloud
    /// keeps going, and a cloud that has gone is not something either
    /// side of the fight has to keep asking about. Its concentration is
    /// deliberately left alone — RAW the caster is still holding a
    /// spell that is still burning, just not here.
    fn advance_owned_zones(&mut self, actor_id: usize) {
        let Some(owner_at) = self.actors.get(&actor_id).map(|a| a.location()) else {
            return;
        };
        let moves: Vec<(usize, Coordinate)> = self
            .zones
            .iter()
            .filter(|z| z.owner_id == actor_id)
            .filter_map(|z| Some((z.id, z.turn_start_destination(owner_at)?)))
            .collect();
        for (zone_id, dest) in moves {
            if !self.in_bounds(dest) {
                let name = self
                    .zones
                    .iter()
                    .find(|z| z.id == zone_id)
                    .map(|z| z.name)
                    .unwrap_or("cloud");
                self.log(format!("The {} drifts off the field.", name));
                self.remove_zone(zone_id);
                continue;
            }
            self.move_zone(zone_id, dest);
        }
    }

    /// Drop a zone by id. Returns true if one was there.
    pub fn remove_zone(&mut self, zone_id: usize) -> bool {
        let before = self.zones.len();
        self.zones.retain(|z| z.id != zone_id);
        self.zone_contacts_this_turn.retain(|(z, _)| *z != zone_id);
        self.zones.len() != before
    }

    /// True if any tile of `actor_id`'s footprint is inside `zone`.
    ///
    /// Footprint rather than origin tile, so a Huge creature standing
    /// with one corner in a web is caught by it — the same rule
    /// `actors_in_burst` applies to every other area in the engine.
    fn actor_in_zone(&self, actor_id: usize, zone: &Zone) -> bool {
        let Some(a) = self.actors.get(&actor_id) else {
            return false;
        };
        footprint_chebyshev(
            a.location(),
            get_tiles_from_size(a.size()),
            zone.origin,
            1,
        ) <= zone.radius
    }

    /// Ids of every zone whose area `actor_id` is standing in.
    pub fn zones_covering_actor(&self, actor_id: usize) -> Vec<usize> {
        self.zones
            .iter()
            .filter(|z| self.actor_in_zone(actor_id, z))
            .map(|z| z.id)
            .collect()
    }

    /// True if `coord` sits under a heavy-obscurement zone.
    pub fn tile_is_obscured(&self, coord: Coordinate) -> bool {
        self.zones
            .iter()
            .any(|z| z.effect.obscures && z.covers(coord))
    }

    /// True if `coord` sits under a magic-suppressing zone — Antimagic
    /// Field's sphere, and nothing else today.
    pub fn tile_suppresses_magic(&self, coord: Coordinate) -> bool {
        self.zones
            .iter()
            .any(|z| z.effect.suppresses_magic && z.covers(coord))
    }

    /// True if any tile of `actor_id`'s footprint stands in a
    /// magic-suppressing zone.
    ///
    /// Footprint rather than anchor tile, matching every other zone
    /// question the engine asks: a Huge creature with one claw inside
    /// an Antimagic Field is inside it, the same way one with one claw
    /// in a web is caught by it.
    pub fn actor_suppresses_magic(&self, actor_id: usize) -> bool {
        self.zones
            .iter()
            .any(|z| z.effect.suppresses_magic && self.actor_in_zone(actor_id, z))
    }

    /// The gate 5e's Antimagic Field closes: **may this caster reach
    /// what they are aiming at with a spell?**
    ///
    /// RAW is symmetric and this predicate is too. "Spells … are
    /// suppressed in the sphere and can't protrude into it" forbids
    /// three things at once, and all three are the same sentence read
    /// from different sides:
    ///
    ///   - a caster standing in the field casting anything at all,
    ///   - a caster outside reaching a target inside,
    ///   - a caster inside reaching a target outside.
    ///
    /// So: blocked whenever *any* end is under a suppressing zone. A
    /// spell that names nobody leaves only the caster's own square to
    /// ask about, which is the first clause — so Misty Step out of a
    /// field fails as surely as Fire Bolt into one.
    ///
    /// Every named target is asked, not just the primary. The reach and
    /// line-of-sight clauses beside this one in `validate_input` are
    /// deliberately first-target-only, because a multi-target action
    /// measures its envelope off its primary; "no spell reaches into
    /// the sphere" binds on each name independently, the same way the
    /// charm gate does.
    ///
    /// Named actors are asked by footprint and bare points by tile,
    /// which is the difference between the two arguments rather than an
    /// inconsistency: a Huge creature with one claw inside the sphere
    /// is inside it, and a point is a point.
    ///
    /// Deliberately *not* a walk of the tiles between the two ends. A
    /// fireball arcing over a sphere it never lands in is not
    /// protruding into anything, and the line-walk would also have made
    /// the field a wall — which it is not; it stops magic, not arrows.
    pub fn magic_suppressed_for_cast(
        &self,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
    ) -> bool {
        if self.zones.iter().all(|z| !z.effect.suppresses_magic) {
            return false;
        }
        self.actor_suppresses_magic(caster_id)
            || target_ids.is_some_and(|ids| {
                ids.iter().any(|&id| self.actor_suppresses_magic(id))
            })
            || target_locations
                .is_some_and(|locs| locs.iter().any(|&c| self.tile_suppresses_magic(c)))
    }

    /// The movement-cost multiplier the zone layer adds at `coord`: 2.0
    /// under any zone that is difficult terrain, 1.0 otherwise.
    ///
    /// Composed with the terrain layer's own multiplier by `max` at the
    /// pathing site rather than multiplied, because 5e's difficult
    /// terrain does not stack — "a space is difficult terrain" is a
    /// property, not a counter, and a web laid over rubble costs 2× and
    /// not 4×.
    pub fn zone_movement_multiplier(&self, coord: Coordinate) -> f32 {
        if self
            .zones
            .iter()
            .any(|z| z.effect.difficult && z.covers(coord))
        {
            2.0
        } else {
            1.0
        }
    }

    /// True if a creature that can be hurt by standing here would be —
    /// the AI's "is this tile worth walking through" question.
    pub fn tile_is_hazardous(&self, coord: Coordinate) -> bool {
        self.zones
            .iter()
            .any(|z| z.effect.deters_walkers() && z.covers(coord))
    }

    /// True if heavy obscurement stands between (or on top of) the two
    /// tiles — the geometry half of `viewer_can_see`'s obscurement gate.
    ///
    /// Endpoints are included, unlike `has_line_of_sight`'s wall walk,
    /// and that difference is the rule: a wall you are standing against
    /// does not blind you, but a fog cloud you are standing *in* does.
    /// RAW's heavy obscurement "blocks vision entirely", and a creature
    /// inside the cloud is as blind looking out as one outside is
    /// looking in.
    pub fn obscured_between(&self, from: Coordinate, to: Coordinate) -> bool {
        if self.zones.iter().all(|z| !z.effect.obscures) {
            return false;
        }
        if self.tile_is_obscured(from) || self.tile_is_obscured(to) {
            return true;
        }
        for tile in tiles_between(from, to) {
            if self.tile_is_obscured(tile) {
                return true;
            }
        }
        false
    }

    /// Expire one round off every zone and sweep the ones that ran out.
    /// Called from `round_end`, alongside the condition timers it is the
    /// map-layer sibling of.
    fn tick_zones(&mut self) {
        let mut expired: Vec<(usize, String)> = Vec::new();
        // Owners whose concentration-held area is the thing that just
        // ran out. Reported so `round_end` can ask whether the caster
        // has anything left to concentrate *on* — see
        // `release_concentration_with_nothing_left`.
        let mut bereaved: Vec<usize> = Vec::new();
        for zone in self.zones.iter_mut() {
            zone.rounds_remaining = zone.rounds_remaining.saturating_sub(1);
            if zone.rounds_remaining == 0 {
                expired.push((zone.id, zone.name.to_string()));
                if zone.concentration {
                    bereaved.push(zone.owner_id);
                }
            }
        }
        for (id, name) in expired {
            self.remove_zone(id);
            self.log(format!("The {} disperses.", name));
        }
        self.pending_concentration_review.append(&mut bereaved);
    }

    /// Tear down every concentration-held zone `actor_id` is sustaining.
    /// Called from `drop_concentration`, which is the only thing that
    /// knows a caster's grip has failed — and which reaches every way it
    /// can fail (damage, a second concentration spell, death, a lapsed
    /// timer) at one chokepoint.
    /// Take down everything on the *map* layers that `actor_id`'s
    /// concentration was holding up — the persistent areas and the
    /// conjured terrain both.
    ///
    /// The one call every "this caster has stopped concentrating"
    /// path makes, so that a second map layer is wired into all of
    /// them at once rather than into whichever ones somebody
    /// remembered. There are three, and they are not variations on each
    /// other: `drop_concentration` (the grip failed), `remove_actor`
    /// (death, which never routes through it), and `despawn_actor` (a
    /// summon unbinding). A dead wizard's wall holding a doorway for the
    /// rest of the fight is the kind of leak the map layers make very
    /// visible.
    fn release_map_layers_of(&mut self, actor_id: usize) {
        self.remove_concentration_zones_of(actor_id);
        self.remove_concentration_terrain_of(actor_id);
    }

    fn remove_concentration_zones_of(&mut self, actor_id: usize) {
        let doomed: Vec<(usize, String)> = self
            .zones
            .iter()
            .filter(|z| z.concentration && z.owner_id == actor_id)
            .map(|z| (z.id, z.name.to_string()))
            .collect();
        for (id, name) in doomed {
            self.remove_zone(id);
            self.log(format!("The {} thins away to nothing.", name));
        }
    }

    /// Fire the contact clause of every zone `actor_id` is standing in
    /// and hasn't already paid this turn.
    ///
    /// The single entry point for both RAW triggers — "enters the area
    /// for the first time on a turn" (called per step by `MoveActor`)
    /// and "starts its turn there" (called by `start_turn_for`) — because
    /// they are the same sentence and differ only in when they are
    /// asked. The ledger is what makes calling it on every step of a
    /// six-tile walk through a web cost one save rather than six.
    /// A horse that walks into a Web has carried its rider into the Web,
    /// and each of them saves for themselves — so the trigger is asked
    /// of both halves of a rider/mount pair. `ride_pair` is the identity
    /// for the overwhelming majority of actors, who are nobody's
    /// passenger; the ledger keeps a second ask on the same turn free.
    pub fn touch_zones(&mut self, actor_id: usize) {
        if self.zones.is_empty() {
            return;
        }
        for id in self.ride_pair(actor_id) {
            self.touch_zones_alone(id);
        }
    }

    /// `touch_zones` for exactly one creature. The body of the old
    /// single-actor routine, split out so the pair walk above has
    /// something to call once per half without recursing.
    fn touch_zones_alone(&mut self, actor_id: usize) {
        if !self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.is_combat_active())
        {
            return;
        }
        let due: Vec<usize> = self
            .zones
            .iter()
            .filter(|z| z.effect.contact.is_some())
            .filter(|z| !self.zone_contacts_this_turn.contains(&(z.id, actor_id)))
            .filter(|z| self.actor_in_zone(actor_id, z))
            .map(|z| z.id)
            .collect();
        for zone_id in due {
            self.touch_zone(zone_id, actor_id);
            // A zone that drops the creature ends the walk; the caller's
            // own liveness check picks that up, but the remaining zones
            // on this tile must not keep hitting a corpse.
            if !self
                .actors
                .get(&actor_id)
                .is_some_and(|a| a.is_combat_active())
            {
                return;
            }
        }
    }

    /// Bill every zone that charges by the tile for one step of
    /// `actor_id`'s walk.
    ///
    /// The third and rarest of the zone triggers, and the only one that
    /// isn't governed by the once-per-turn ledger — Spike Growth's
    /// "2d4 piercing for every 5 feet it travels" means every 5 feet,
    /// and a creature that crosses six tiles of thorns pays six times.
    /// Called after the step has landed, so the question is whether the
    /// tile just entered is thorny.
    /// Billed to both halves of a rider/mount pair, for the reason
    /// `touch_zones` is: five feet of thorns crossed by a horse is five
    /// feet of thorns crossed by everyone on it.
    pub fn charge_zone_movement(&mut self, actor_id: usize) {
        if self.zones.iter().all(|z| z.effect.per_step_damage.is_none()) {
            return;
        }
        for id in self.ride_pair(actor_id) {
            self.charge_zone_movement_alone(id);
        }
    }

    /// `charge_zone_movement` for exactly one creature. See
    /// `touch_zones_alone` for why the split exists.
    fn charge_zone_movement_alone(&mut self, actor_id: usize) {
        // Footprint coverage, not the anchor tile — the same question
        // `touch_zones` asks, so a Large creature with one corner in
        // the thorns is in the thorns for both triggers.
        let due: Vec<(&'static str, crate::engine::dice::Dice, DamageType)> = self
            .zones
            .iter()
            .filter_map(|z| {
                let (dice, dt) = z.effect.per_step_damage?;
                self.actor_in_zone(actor_id, z).then_some((z.name, dice, dt))
            })
            .collect();
        for (name, dice, damage_type) in due {
            if !self
                .actors
                .get(&actor_id)
                .is_some_and(|a| a.is_combat_active())
            {
                return;
            }
            let amount = self.roll(&dice);
            let actor_name = self.actor_name(actor_id);
            self.log(format!(
                "  {}: {} takes {} {} crossing it.",
                name, actor_name, amount, damage_type
            ));
            crate::engine::side_effects::DealDamage {
                actor_id,
                amount,
                damage_type,
            }
            .apply(self);
        }
    }

    /// Fire one named zone's contact clause at one creature, if the
    /// creature is standing in it and hasn't already paid this turn.
    ///
    /// The narrow sibling of `touch_zones`, for the caller that already
    /// knows which zone it means: a spell whose text charges "each
    /// creature in the area when it appears" is charging for exactly
    /// the zone it just laid down, and must not also collect for the
    /// web somebody else spun on the far side of the room.
    pub fn touch_zone(&mut self, zone_id: usize, actor_id: usize) {
        if self.zone_contacts_this_turn.contains(&(zone_id, actor_id)) {
            return;
        }
        if !self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.is_combat_active())
        {
            return;
        }
        let Some(zone) = self.zones.iter().find(|z| z.id == zone_id) else {
            return;
        };
        if zone.effect.contact.is_none() || !self.actor_in_zone(actor_id, zone) {
            return;
        }
        // 5e wards (Glyph of Warding, Symbol): the trigger is refined to
        // the setter's enemies, so their own side walks over the glyph
        // without springing it. See `ZoneEffect::ward` for why this is
        // the one friend-or-foe clause on a friend-or-foe-blind layer.
        let ward = zone.effect.ward;
        if let Some(setter_team) = ward
            && self
                .actors
                .get(&actor_id)
                .is_some_and(|a| a.team() == setter_team)
        {
            return;
        }
        self.zone_contacts_this_turn.insert((zone_id, actor_id));
        self.apply_zone_contact(zone_id, actor_id);
        if ward.is_some() {
            self.detonate_ward(zone_id, actor_id);
        }
    }

    /// Spring a ward that `sprung_by` has just set off: charge its
    /// contact clause to everybody else standing in the area, then
    /// spend it.
    ///
    /// The blast is *not* filtered the way the trigger is. RAW's
    /// eruption catches "each creature in the area", so the setter's
    /// own allies — and the setter — are caught by a glyph their enemy
    /// stepped on. Only the tripwire knows whose side anyone is on.
    ///
    /// The ward is removed rather than left to a timer, which is the
    /// lifecycle that made this a layer change rather than a spell:
    /// every other area here ends on a round-end tick or on dropped
    /// concentration, and "the spell ends when it is triggered" is
    /// neither.
    fn detonate_ward(&mut self, zone_id: usize, sprung_by: usize) {
        let (name, caught) = {
            let Some(zone) = self.zones.iter().find(|z| z.id == zone_id) else {
                return;
            };
            let mut caught: Vec<usize> = self
                .actors
                .iter()
                .filter(|(id, a)| **id != sprung_by && a.is_combat_active())
                .map(|(id, _)| *id)
                .filter(|id| self.actor_in_zone(*id, zone))
                .collect();
            // Sorted so a seeded replay charges them in the same order,
            // which matters because each one rolls a save.
            caught.sort_unstable();
            (zone.name, caught)
        };
        self.log(format!("The {} flares and is spent.", name));
        for id in caught {
            self.zone_contacts_this_turn.insert((zone_id, id));
            self.apply_zone_contact(zone_id, id);
        }
        self.remove_zone(zone_id);
    }

    /// Resolve one zone's contact clause against one creature: the save
    /// (if it has one), then the damage, then the condition.
    ///
    /// A successful save negates the condition outright and either
    /// halves or negates the damage depending on the zone's
    /// `half_on_success`. A zone with no save applies both unconditionally
    /// — Cloud of Daggers offers none.
    fn apply_zone_contact(&mut self, zone_id: usize, actor_id: usize) {
        let Some(zone) = self.zones.iter().find(|z| z.id == zone_id) else {
            return;
        };
        let Some(contact) = zone.effect.contact else {
            return;
        };
        let (name, owner_id) = (zone.name, zone.owner_id);
        let actor_name = self.actor_name(actor_id);
        let saved = match contact.save {
            Some(s) => {
                let outcome =
                    self.roll_save_against_caster(actor_id, s.ability, s.dc, owner_id);
                outcome.passed()
            }
            None => false,
        };
        let half_on_success = contact.save.is_some_and(|s| s.half_on_success);
        if let Some((dice, damage_type)) = contact.damage {
            let rolled = self.roll(&dice);
            let amount = if saved {
                if half_on_success { rolled / 2 } else { 0 }
            } else {
                rolled
            };
            if amount > 0 {
                self.log(format!(
                    "  {}: {} takes {} {}.",
                    name,
                    actor_name,
                    amount,
                    damage_type
                ));
                crate::engine::side_effects::DealDamage {
                    actor_id,
                    amount,
                    damage_type,
                }
                .apply(self);
            }
        }
        // Damage can drop the creature the rest of this clause was
        // aimed at. Nothing below applies to somebody who is already
        // on the floor — a corpse has no concentration to lose and
        // cannot be restrained — and rolling for it would put saves in
        // the log that nobody made.
        if !self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.is_combat_active())
        {
            return;
        }
        // The concentration clause is its own roll, and it is asked
        // whether or not the first save landed — a wizard who kept its
        // feet in the sleet has said nothing yet about keeping its
        // spell. Only ever asked of somebody with a spell to lose.
        if contact.breaks_concentration
            && self
                .actors
                .get(&actor_id)
                .is_some_and(|a| a.is_concentrating())
            && let Some(s) = contact.save
        {
            let outcome = self.roll_save_against_caster(
                actor_id,
                AbilityScoreType::Constitution,
                s.dc,
                owner_id,
            );
            if !outcome.passed() {
                self.log(format!("  {}: {} loses their grip.", name, actor_name));
                self.drop_concentration(actor_id);
            }
        }
        if saved {
            return;
        }
        if let Some((condition, timer)) = contact.condition {
            if self.actor_immune_to_condition(actor_id, condition) {
                self.log(format!(
                    "  {}: {} is unaffected.",
                    name, actor_name
                ));
                return;
            }
            if let Some(a) = self.actors.get_mut(&actor_id) {
                a.add_condition(condition, timer);
            }
            self.log(format!(
                "  {}: {} is {}.",
                name,
                actor_name,
                condition.name()
            ));
        }
    }

    /// True if a `size`-wide footprint anchored at `coord` would sit
    /// entirely on passable tiles that are either empty or already this
    /// actor's own. Takes the size explicitly rather than reading it off
    /// the actor so the resize lane can ask the counterfactual question
    /// — "would this actor fit here if it were one category bigger?" —
    /// which is exactly what `reconcile_footprints` needs before it grows
    /// anyone.
    fn footprint_fits(&self, actor_id: usize, coord: Coordinate, size: Size) -> bool {
        let actor_width = get_tiles_from_size(size);
        for x_off in 0..actor_width {
            for y_off in 0..actor_width {
                let offset: Coordinate = Coordinate::new(x_off as isize, y_off as isize);
                if !self.can_move_to_subtile(coord + offset, actor_id) {
                    return false;
                }
            }
        }
        true
    }

    pub fn can_move_to(&self, actor_id: usize, coord: Coordinate) -> bool {
        match self.actors.get(&actor_id) {
            Some(actor) => self.footprint_fits(actor_id, coord, actor.size()),
            None => false,
        }
    }

    /// Bring every actor's stamped footprint back in line with the size
    /// their conditions ask for. The single writer of
    /// `ActorInstance::size`, and the only place that may change a
    /// footprint without also moving the actor.
    ///
    /// Growth and shrink effects install a condition and stop there;
    /// `desired_size` reports what the conditions want and this sweep is
    /// what the board actually agrees to. Splitting it that way is what
    /// makes the lane safe: a condition can arrive or expire down any of
    /// the engine's many paths (a cast, a dispel, a dropped
    /// concentration, a round-end timer tick) without any of them needing
    /// to know that the actor map exists.
    ///
    /// Growing is refused when the wider footprint would overlap a wall,
    /// the map edge, or another creature — RAW's "if there is enough
    /// room" clause. A refusal is silent and *not* final: the sweep runs
    /// on every pump, so a fighter hemmed in against a wall grows the
    /// moment the neighbour who was in the way moves or dies. Shrinking
    /// always succeeds, since a smaller box is a subset of a larger one.
    pub fn reconcile_footprints(&mut self) {
        // Collected first so the loop below can take `&mut self` — and,
        // more importantly, so one actor's growth is stamped before the
        // next one's room check runs, rather than every check racing
        // against a stale map.
        let pending: Vec<(usize, Size, Size)> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                // A mounted rider has no footprint to reconcile — the
                // mount owns the tiles the pair stands on, and
                // `resize_actor` writes the grid, so letting a rider
                // through here would stamp a second body over the
                // horse's. So a growth or shrink that lands on someone
                // in a saddle is *deferred*, not lost: the sweep runs on
                // every pump, and the rider takes their new size the
                // moment they come out of it. Which is also the RAW
                // answer to a Rune Knight who grows to Large on a
                // Large horse — RAW's own "if there is enough room"
                // clause, with a saddle as the room.
                // A latched creature is off the grid for the same
                // reason and gets the same deferral: the host owns the
                // tiles, so a growth that resized it here would stamp a
                // second body over its victim. It takes its new size
                // the moment it lets go.
                if a.mounted_on().is_some() || a.attached_to().is_some() {
                    return None;
                }
                let want = a.desired_size();
                (want != a.size()).then_some((*id, a.size(), want))
            })
            .collect();
        for (actor_id, from, to) in pending {
            self.resize_actor(actor_id, from, to);
        }
    }

    /// Move one actor from footprint `from` to footprint `to` in place,
    /// keeping the actor map and the actor's own size field in step.
    /// Called only by `reconcile_footprints`, which owns the decision of
    /// when a resize is due.
    fn resize_actor(&mut self, actor_id: usize, from: Size, to: Size) {
        let Some((origin, name)) = self
            .actors
            .get(&actor_id)
            .map(|a| (a.location(), a.name().to_string()))
        else {
            return;
        };
        let growing = to.ordinal() > from.ordinal();
        // Only growth can be refused, and it is checked against the map
        // *including* this actor's own current tiles — `footprint_fits`
        // treats them as free, so the check is "is the extra ring clear?"
        // rather than "is the whole box empty?".
        if growing && !self.footprint_fits(actor_id, origin, to) {
            return;
        }
        self.write_footprint(None, origin, from);
        if let Some(a) = self.get_actor(actor_id) {
            a.set_size(to);
        }
        self.write_footprint(Some(actor_id), origin, to);
        let verb = if growing { "swells" } else { "dwindles" };
        self.log(format!("{} {} to {}.", name, verb, to));
    }

    /// Hold every actor's height above the floor in step with what is
    /// holding them up, and collect from anyone the ground is owed.
    ///
    /// The altitude twin of `reconcile_footprints`, wired at the same
    /// three chokepoints and for the same reason: an actor's height is a
    /// consequence of their state, so making it a sweep means every
    /// present and future way of gaining or losing flight is already
    /// handled and no individual effect has to remember to fix anything.
    /// A wizard's Fly can end four ways in this engine — a failed
    /// concentration save, a Dispel Magic, a lapsed timer, a second
    /// concentration spell replacing it — and before this sweep all four
    /// resolved as the wizard standing quietly back on the floor.
    ///
    /// Two directions, and they are not symmetric. Rising is free and
    /// instantaneous (a creature that gains a flying speed is in the air;
    /// there is nothing to resolve). Falling is a `resolve_fall` per
    /// actor, which rolls damage and can drop them — so the pending list
    /// is collected before anything is spent, exactly as
    /// `reconcile_footprints` does, and for a sharper reason than borrow
    /// juggling: a fall can kill, and killing inside a walk of
    /// `self.actors` would resolve some actors against a board the
    /// earlier deaths had already changed and others against a stale one,
    /// depending on hash order.
    ///
    /// Ordered by actor id so a round in which two fliers drop together
    /// is reproducible from the seed.
    ///
    /// # Riders
    ///
    /// A rider's supported altitude is their **mount's** — the same
    /// redirect `movement_body` performs for every other question about
    /// where a mounted creature is, and for the same reason: the pegasus
    /// owns the tiles and the wings, and the paladin on it is wherever
    /// the pegasus is. RAW says so outright ("if you and your mount
    /// fall, you take falling damage"), and the two of them are charged
    /// separately because they are two creatures with two hit point
    /// pools and two sets of resistances.
    ///
    /// This is a real case rather than a hypothetical one, and it became
    /// real the moment a mount could fly under its own power: the
    /// pegasus, the griffon and the hippogriff are all mountable and all
    /// have a flying speed. Without the redirect a paladin would ride a
    /// pegasus thirty feet up, watch it get Earthbound out from under
    /// them, and step off onto thin air at altitude zero having never
    /// been in the sky at all.
    /// The altitude `actor_id`'s current state holds it at, with the
    /// rider redirect applied — the encounter-level counterpart of
    /// `ActorInstance::supported_altitude_ft`, which can only see one
    /// creature and so cannot know about the horse.
    ///
    /// A rider borrows their mount's answer outright rather than taking
    /// the maximum of the two. A paladin under *Fly* on a grounded
    /// pegasus is sitting on a pegasus: they cannot be at cruising
    /// altitude while the thing they are strapped to is on the floor.
    /// Should they want the height, RAW's answer is to dismount, and the
    /// engine's is the same.
    ///
    /// Reads through `movement_body`, which is the same redirect
    /// `is_immersed` and the pathing geometry use, so "where is this
    /// creature" has one answer across the engine rather than one per
    /// question.
    fn supported_altitude_for(&self, actor_id: usize) -> u32 {
        self.actors
            .get(&self.movement_body(actor_id))
            .map(|a| a.supported_altitude_ft())
            .unwrap_or(0)
    }

    pub fn reconcile_altitudes(&mut self) {
        // Resolved before the walk because the closure below cannot
        // borrow `self.actors` again to look a mount up.
        let supported: HashMap<usize, u32> = self
            .actors
            .keys()
            .map(|&id| (id, self.supported_altitude_for(id)))
            .collect();
        let mut pending: Vec<(usize, u32)> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                // `checked_sub` and not a `>` guard with a plain
                // subtraction: an actor who just *gained* flight is the
                // common case here, and their `supported` exceeds their
                // `current` by the whole cruising altitude. A
                // `then_some` would evaluate the difference before the
                // guard ever ran and underflow on every single one of
                // them.
                a.altitude_ft()
                    .checked_sub(supported[id])
                    .filter(|&drop| drop > 0)
                    .map(|drop| (*id, drop))
            })
            .collect();
        pending.sort_unstable();
        // The rising half is a plain field write with nothing to resolve,
        // so it runs in the same borrow rather than through a pending
        // list. Done after the fallers are collected — the two sets are
        // disjoint by construction (an actor is either above or below
        // where its state puts it), so the order is a matter of clarity
        // rather than correctness.
        for (id, actor) in self.actors.iter_mut() {
            let target = supported[id];
            if target > actor.altitude_ft() {
                actor.set_altitude_ft(target);
            }
        }
        if pending.is_empty() {
            return;
        }
        for (actor_id, distance_ft) in pending {
            self.resolve_fall(actor_id, distance_ft);
        }
        // A fall is real damage and can therefore kill, and every other
        // site that deals damage clears the corpse behind it. Two of the
        // three chokepoints this sweep runs at do call `cleanup_dead_actors`
        // — but both of them call it *before* the sweep, so a creature
        // the ground finished would sit on the board with its footprint
        // still stamped until something else happened to die. Guarded on
        // a fall having actually happened so the overwhelmingly common
        // "nobody is in the air" pass stays a single walk of the actor
        // map.
        self.cleanup_dead_actors();
    }

    /// Drop one actor `distance_ft` feet and charge them for it — SRD's
    /// "1d6 bludgeoning damage for every 10 feet it fell, to a maximum of
    /// 20d6. The creature lands prone, unless it avoids taking damage
    /// from the fall."
    ///
    /// Both halves of that second sentence are honoured, and the "unless"
    /// is not a formality here: this engine has creatures immune to
    /// bludgeoning, and one of them landing face-down would be a rule the
    /// SRD explicitly does not have. The gate reads the damage the actor
    /// would actually take (`effective_damage`) rather than the raw roll,
    /// so immunity clears the Prone and mere resistance does not.
    ///
    /// The damage is a real `DealDamage` rather than a subtraction, which
    /// is what makes a fall behave like everything else that hurts: temp
    /// HP absorbs it, a concentration save fires, resistances apply,
    /// and it can drop a creature to Dying. That is deliberate — a
    /// Fly that ends because its caster lost concentration, dropping the
    /// caster hard enough to break a *second* concentration, is exactly
    /// the cascade RAW describes.
    ///
    /// Altitude is spent before the damage lands, so an actor killed by
    /// the fall is on the floor when they die and nothing can charge them
    /// for the same drop twice.
    fn resolve_fall(&mut self, actor_id: usize, distance_ft: u32) {
        use crate::conditions::ConditionTimer;
        use crate::engine::falling::fall_damage_dice;
        use crate::engine::side_effects::DealDamage;
        let Some(actor) = self.actors.get_mut(&actor_id) else {
            return;
        };
        let landed_at = actor.altitude_ft().saturating_sub(distance_ft);
        actor.set_altitude_ft(landed_at);
        let name = actor.name().to_string();
        // RAW's trigger is "when you or a creature within 60 feet of you
        // falls" — asked here, after the altitude is spent and before any
        // damage is rolled, because that is the moment the fall exists.
        let feathered = self.try_feather_fall(actor_id);
        if feathered {
            self.log(format!(
                "{} falls {} ft and drifts down feather-light.",
                name, distance_ft
            ));
            return;
        }
        let dice = fall_damage_dice(distance_ft);
        if dice.count == 0 {
            // Under ten feet: RAW buys no dice, so this is a step down
            // rather than a fall. No damage and no Prone.
            return;
        }
        let raw = self.roll(&dice);
        let hurts = self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.effective_damage(raw, DamageType::Bludgeoning) > 0);
        self.log(format!(
            "{} falls {} ft and hits the ground ({} {}).",
            name, distance_ft, dice, raw
        ));
        DealDamage {
            actor_id,
            amount: raw,
            damage_type: DamageType::Bludgeoning,
        }
        .apply(self);
        // "Lands prone, *unless it avoids taking damage from the fall*."
        // Checked against the pre-damage verdict rather than re-read
        // afterwards: an actor the fall killed is no longer a legal
        // target for a condition, and knocking a corpse over is not what
        // the clause is about.
        if hurts
            && let Some(a) = self.actors.get_mut(&actor_id)
            && a.add_condition(Condition::Prone, ConditionTimer::Permanent)
        {
            self.log(format!("{} is prone.", name));
        }
    }

    /// 5e **Feather Fall** (level-1 transmutation, reaction) as the
    /// reaction it actually is.
    ///
    /// RAW's trigger — *"when you or a creature within 60 feet of you
    /// falls"* — has no analogue in a turn-ordered action list, and there
    /// is no board state in which a player would sensibly pre-cast a
    /// spell whose entire effect is contingent on a fall that has not
    /// happened. So the spell lives here, on the same lane as
    /// `try_flash_of_genius`: a scan of the board for an ally willing to
    /// spend a reaction and a slot on somebody else's emergency.
    ///
    /// Cost is RAW and both halves are real. The reaction competes with
    /// every other reaction the chassis carries, and the 1st-level slot
    /// competes with a Shield the same caster may want two seconds later.
    /// The saver must be able to *see* the faller (RAW's "a creature
    /// within 60 feet of you" plus the spell's own targeting), which
    /// matters for an invisible one — and, since the faller is a legal
    /// self-target, a wizard with the tag can catch themselves.
    ///
    /// Returns true when the fall was caught. The `Feathered` condition
    /// it installs carries RAW's one-minute duration, so a caster who
    /// spends the reaction on the first drop of a fight covers a second
    /// drop for free — which is the RAW reading, the spell does not end
    /// on the first landing.
    ///
    /// Preference among eligible savers is lowest id, which is the same
    /// tiebreak every other board scan uses: the alternative is hash
    /// order, and a rescue that depends on hash order is not reproducible
    /// from the seed.
    fn try_feather_fall(&mut self, falling_id: usize) -> bool {
        use crate::actions::class_features::FEATHER_FALL_TAG;
        use crate::conditions::ConditionTimer;
        use crate::engine::side_effects::Resource;
        // Already drifting from an earlier catch this fight — RAW's
        // one-minute duration covers this fall too, and nobody spends a
        // second slot on it.
        if self
            .actors
            .get(&falling_id)
            .is_some_and(|a| a.has_condition(Condition::Feathered))
        {
            return true;
        }
        let Some(faller) = self.actors.get(&falling_id) else {
            return false;
        };
        let (team, loc, size) = (
            faller.team(),
            faller.location(),
            get_tiles_from_size(faller.size()),
        );
        let mut saver: Option<usize> = None;
        for (id, caster) in self.actors.iter() {
            if caster.team() != team
                || !caster.is_combat_active()
                || !caster.has_passive_feature(FEATHER_FALL_TAG)
                || !caster.can_consume_resource(Resource::Reaction)
                || !caster.can_consume_resource(Resource::SpellSlot(1))
            {
                continue;
            }
            if *id != falling_id && !self.viewer_can_see(*id, falling_id) {
                continue;
            }
            if footprint_chebyshev(
                caster.location(),
                get_tiles_from_size(caster.size()),
                loc,
                size,
            ) > Self::FEATHER_FALL_RADIUS
            {
                continue;
            }
            if saver.is_none_or(|best| *id < best) {
                saver = Some(*id);
            }
        }
        let saver_id = match saver {
            Some(id) => id,
            None => return false,
        };
        let Some(caster) = self.actors.get_mut(&saver_id) else {
            return false;
        };
        caster.consume_resource(Resource::Reaction);
        caster.consume_resource(Resource::SpellSlot(1));
        let saver_name = caster.name().to_string();
        if let Some(faller) = self.actors.get_mut(&falling_id) {
            faller.add_condition(Condition::Feathered, ConditionTimer::Rounds(10));
        }
        let faller_name = self.actor_name(falling_id);
        self.log(format!(
            "[reaction] feather fall: {} slows {}'s descent.",
            saver_name, faller_name
        ));
        true
    }

    /// True if a straight Bresenham line from `from` to `to` passes through
    /// only non-wall tiles between (exclusive of endpoints). Endpoints are
    /// not checked so callers can target the tile they currently occupy or
    /// the tile they want to attack into. Actors do *not* block LOS — only
    /// walls do, which is RAW: a creature in the way grants its target
    /// half cover (PHB p.196 lists "a creature" alongside the low wall
    /// and the tree trunk), and half cover is an AC bonus rather than a
    /// blocked shot. `cover_ac_bonus` is where that lands. The two
    /// questions used to be conflated in this docstring, which cited a
    /// "creatures don't grant cover" default 5e does not have and this
    /// engine does not implement.
    pub fn has_line_of_sight(&self, from: Coordinate, to: Coordinate) -> bool {
        !tiles_between(from, to)
            .any(|c| matches!(self.terrain_at(c), Some(t) if t.terrain_type.blocks_sight()))
    }

    /// 5e **half cover**: "+2 bonus to AC and Dexterity saving
    /// throws". The lower of the two rungs `cover_ac_bonus` returns.
    ///
    /// Named because three places compare against it — the AC lookup,
    /// the log suffix, and the Hide action's "behind Three-Quarters
    /// Cover" gate — and a bare `2` at each reads as a coincidence
    /// rather than as the one number it is.
    pub const HALF_COVER_AC: i32 = 2;
    /// 5e **three-quarters cover**: "+5 bonus to AC and Dexterity
    /// saving throws", and the rung SRD 5.2's Hide action names as one
    /// of the two things that let you try to hide at all.
    pub const THREE_QUARTERS_COVER_AC: i32 = 5;

    /// 5e cover. Counts the obstructions a straight origin-to-origin
    /// line from attacker to target passes through: combat-active actors
    /// other than the two ends, and `TerrainType::LowWall` tiles. 0
    /// obstructions = no cover; 1 = half cover (+2 AC); 2+ =
    /// three-quarters cover (+5 AC). Total cover (line fully blocked by
    /// a `Wall`) is handled upstream via `actor_has_line_of_sight`; this
    /// routine assumes LOS already validated.
    ///
    /// Creatures and low walls count on the same ladder rather than on
    /// separate ones, so standing behind a low wall *and* behind an ally
    /// is three-quarters cover, the same as standing behind two allies.
    /// One ladder is the RAW half of the design — a target's cover comes
    /// from whatever is in the way, not as a bonus per obstruction
    /// *type*. Climbing that ladder by counting obstructions is not:
    /// RAW takes the most protective single source rather than adding
    /// them up, so by the book two half-covers is still half cover. See
    /// the comment at the low-wall branch below for why the engine
    /// counts anyway.
    ///
    /// The routine is deliberately conservative: it walks the Bresenham
    /// line between the two actors' anchor tiles and stops counting after
    /// 2 hits (the bonus saturates at +5). It deliberately doesn't count
    /// `Wall` — that is total cover and gates the attack via LOS, so a
    /// line that crosses one never reaches here.
    pub fn cover_ac_bonus(&self, attacker_id: usize, target_id: usize) -> i32 {
        let (Some(a), Some(b)) = (
            self.actors.get(&attacker_id),
            self.actors.get(&target_id),
        ) else {
            return 0;
        };
        // 5e: adjacent attackers ignore cover. The clause keeps melee
        // swings clean (a grappler isn't shielded from their grappling
        // partner by a third creature).
        if footprint_chebyshev(
            a.location(),
            get_tiles_from_size(a.size()),
            b.location(),
            get_tiles_from_size(b.size()),
        ) <= crate::actions::action_template::MELEE_REACH
        {
            return 0;
        }
        let from = a.location();
        let to = b.location();
        let mut hits = 0u32;
        let mut last_hit: Option<usize> = None;
        // The two endpoints' own footprints, so the terrain walk below
        // can skip them.
        //
        // The creature check doesn't need this — it excludes both ids by
        // name — but the terrain check has no id to exclude, and the
        // anchor-to-anchor line runs *through* both footprints on the
        // way out and in. Without the skip, a shooter standing on a low
        // wall would be granting their target cover with the wall they
        // are themselves braced against, and a target could be given
        // cover by a tile inside its own square.
        let endpoint_tiles = |c: Coordinate, anchor: Coordinate, span: isize| -> bool {
            c.x >= anchor.x && c.x < anchor.x + span && c.y >= anchor.y && c.y < anchor.y + span
        };
        let a_span = get_tiles_from_size(a.size()) as isize;
        let b_span = get_tiles_from_size(b.size()) as isize;
        for coord in tiles_between(from, to) {
            if let Some(blocker_id) = self.actor_id_at(coord)
                && blocker_id != attacker_id
                && blocker_id != target_id
                && self
                    .actors
                    .get(&blocker_id)
                    .is_some_and(|a| a.is_combat_active())
                && last_hit != Some(blocker_id)
            {
                last_hit = Some(blocker_id);
                hits = hits.saturating_add(1);
                if hits >= 2 {
                    return Self::THREE_QUARTERS_COVER_AC;
                }
            }
            // A low wall under the line obstructs it the same way a body
            // does, and is counted independently of the creature check
            // above: a creature standing *on* a low-wall tile is two
            // things in the way, and this routine's ladder reads two
            // obstructions as three-quarters cover.
            //
            // That ladder is a house simplification and worth naming as
            // one. RAW is "if two sources of cover apply to a target,
            // the target's cover is determined by the most protective
            // degree, not by adding them together" — so by the book, two
            // half-covers is half cover, and the +5 rung is reached only
            // by an obstruction that is three-quarters cover on its own,
            // which nothing on this board is. Counting instead of
            // maxing is what gives the engine a second rung at all, and
            // it is the reading that makes a firing lane worth thinking
            // about: an archer who can find an angle past the second
            // body has bought something.
            //
            // Written down here because the previous version of this
            // comment cited that same RAW sentence *in support of* the
            // counting, which reads as fidelity and is the opposite of
            // it. A deliberate divergence that describes itself as RAW
            // is worse than an undocumented one — the next reader has no
            // reason to look.
            if !endpoint_tiles(coord, from, a_span)
                && !endpoint_tiles(coord, to, b_span)
                && self
                    .terrain_at(coord)
                    .is_some_and(|t| t.terrain_type.grants_cover())
            {
                hits = hits.saturating_add(1);
                if hits >= 2 {
                    return Self::THREE_QUARTERS_COVER_AC;
                }
            }
        }
        if hits >= 1 { Self::HALF_COVER_AC } else { 0 }
    }

    /// 5e cover measured from **a point** rather than from an
    /// attacker — the shape RAW's *"take cover behind the pillar"*
    /// needs for an area effect, whose origin is a spot on the floor
    /// and not a creature.
    ///
    /// Same ladder as `cover_ac_bonus`, same counting, one clause
    /// missing and one added. Missing: the adjacency exemption, which
    /// exists because a swing at arm's length is not obstructed by
    /// a third body; a fireball's origin has no arm and no reach, and a
    /// creature standing next to the point of detonation is as much
    /// behind the ally in front of it as one across the room. Added:
    /// `origin_owner`, the caster, whose own body is skipped — a
    /// self-centred burst should not be shielded by the person casting
    /// it.
    ///
    /// RAW measures an area's cover *from its point of origin*, which
    /// is what makes this a different function rather than a second
    /// caller of the old one: the line to walk starts at a tile nobody
    /// is standing on.
    pub fn cover_bonus_from_point(
        &self,
        origin: Coordinate,
        origin_owner: usize,
        target_id: usize,
    ) -> i32 {
        let Some(target) = self.actors.get(&target_id) else {
            return 0;
        };
        let to = target.location();
        let span = get_tiles_from_size(target.size()) as isize;
        let inside_target = |c: Coordinate| -> bool {
            c.x >= to.x && c.x < to.x + span && c.y >= to.y && c.y < to.y + span
        };
        let mut hits = 0u32;
        let mut last_hit: Option<usize> = None;
        for coord in tiles_between(origin, to) {
            if let Some(blocker_id) = self.actor_id_at(coord)
                && blocker_id != target_id
                && blocker_id != origin_owner
                && self
                    .actors
                    .get(&blocker_id)
                    .is_some_and(|a| a.is_combat_active())
                && last_hit != Some(blocker_id)
            {
                last_hit = Some(blocker_id);
                hits = hits.saturating_add(1);
                if hits >= 2 {
                    return Self::THREE_QUARTERS_COVER_AC;
                }
            }
            if !inside_target(coord)
                && self
                    .terrain_at(coord)
                    .is_some_and(|t| t.terrain_type.grants_cover())
            {
                hits = hits.saturating_add(1);
                if hits >= 2 {
                    return Self::THREE_QUARTERS_COVER_AC;
                }
            }
        }
        if hits >= 1 { Self::HALF_COVER_AC } else { 0 }
    }

    /// *Which* creature is standing in the way, rather than how much
    /// good it does — `cover_ac_bonus`'s question asked from the other
    /// end.
    ///
    /// Returns the first combat-active creature the attacker's line to
    /// `target_id` passes through that is also within `max_tiles` of the
    /// target, or `None` if the shot is clear. The distance clause is
    /// the Mastermind Rogue's, and it is what stops the feature from
    /// naming a bystander thirty feet up the line who happens to be
    /// under the shot: RAW is "a creature within 5 feet of you is
    /// granting you cover."
    ///
    /// Walk order, not id order, so the answer is the nearest thing to
    /// the *shooter* — the body the arrow reaches first is the body it
    /// can be made to hit. That is also deterministic, which the seeded
    /// runs need.
    pub fn cover_granting_creature(
        &self,
        attacker_id: usize,
        target_id: usize,
        max_tiles: isize,
    ) -> Option<usize> {
        let (a, b) = (self.actors.get(&attacker_id)?, self.actors.get(&target_id)?);
        let (from, to) = (a.location(), b.location());
        for coord in tiles_between(from, to) {
            let Some(blocker_id) = self.actor_id_at(coord) else {
                continue;
            };
            if blocker_id == attacker_id || blocker_id == target_id {
                continue;
            }
            if !self
                .actors
                .get(&blocker_id)
                .is_some_and(|c| c.is_combat_active())
            {
                continue;
            }
            if self
                .footprint_distance(blocker_id, target_id)
                .is_some_and(|d| d <= max_tiles)
            {
                return Some(blocker_id);
            }
        }
        None
    }

    /// Footprint-aware LOS: clear if *any* tile of A's footprint can see
    /// *any* tile of B's footprint. Catches the common case where the
    /// origin-to-origin line is blocked but the creatures can still see
    /// around their own bulk (e.g. two Medium creatures around a corner).
    pub fn actor_has_line_of_sight(&self, a_id: usize, b_id: usize) -> bool {
        let Some(a) = self.actors.get(&a_id) else {
            return false;
        };
        let Some(b) = self.actors.get(&b_id) else {
            return false;
        };
        let a_size = get_tiles_from_size(a.size()) as isize;
        let b_size = get_tiles_from_size(b.size()) as isize;
        let a_loc = a.location();
        let b_loc = b.location();
        for ay in 0..a_size {
            for ax in 0..a_size {
                let from = Coordinate::new(a_loc.x + ax, a_loc.y + ay);
                for by in 0..b_size {
                    for bx in 0..b_size {
                        let to = Coordinate::new(b_loc.x + bx, b_loc.y + by);
                        if self.has_line_of_sight(from, to) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Fire reactions matching `event`. Today this only handles opportunity
    /// attacks on `ActorLeaving`; future variants (damage taken, attack
    /// resolved, etc.) plug in here. Reactions execute eagerly — their
    /// side-effects apply directly to the encounter, not via the stack —
    /// because they conceptually happen "during" the triggering event.
    pub fn dispatch_reaction(&mut self, event: TriggerEvent) {
        match event {
            TriggerEvent::ActorLeaving { actor_id, from, to } => {
                // Readied attacks first. Both dispatchers spend the same
                // Reaction and the same step, and RAW resolves a readied
                // attack "in response to" the trigger — the readier has
                // been waiting for this since their own turn, so they
                // get the swing before anyone who is merely reacting to
                // it. It also matters mechanically: a readied shot that
                // drops the mover ends the step, and the opportunity
                // attacks that would have followed have nobody to hit.
                self.dispatch_readied_attacks(actor_id, from, to);
                self.dispatch_opportunity_attacks(actor_id, from, to);
            }
        }
    }

    /// The attack `actor_id` would hold if it took the Ready action:
    /// the longest-reaching single-target harmful attack it carries
    /// that costs nothing but the swing, with ties going to the earlier
    /// entry in its action list.
    ///
    /// 5e lets the readier choose and we can't ask — the prompt
    /// resolves one action name per line and has nowhere to put a
    /// second. Longest reach is the choice that matches what readying
    /// an attack is *for*: the hold fires the moment a target crosses
    /// into range, so the longest weapon is both the one that fires
    /// soonest and the one a player holding a bow and a sword would
    /// raise. With one attack the selection collapses to "your attack".
    ///
    /// **The slot filter is load-bearing, not tidiness.** Like the
    /// opportunity-attack dispatcher, the readied swing runs the
    /// action's `side_effects` directly and pays only the reaction —
    /// it never goes through `execute`, so no cost is ever charged.
    /// A caster's spell list is full of single-target harmful attacks
    /// with reach, and the *longest*-reaching of them is invariably the
    /// most expensive: without this filter a lich would ready
    /// Disintegrate and fire a level-6 slot's worth of damage, for
    /// free, every round for the rest of the fight. So the candidate
    /// has to be something whose whole price is the action taking it —
    /// a weapon, a monster attack, a cantrip.
    ///
    /// Lives on the encounter rather than on `ActorInstance` because
    /// resolving a cost needs one: `Action::cost` takes the board.
    ///
    /// Deliberately narrower than `first_melee_weapon_action`, which
    /// answers a different question (what swings on an opportunity
    /// attack) and is therefore capped at melee reach rather than
    /// sorted by it.
    pub fn best_readyable_attack(
        &self,
        actor_id: usize,
    ) -> Option<&'static (dyn crate::actions::action_template::Action + Send + Sync)> {
        use crate::actions::action_template::TargetingSchema;
        use crate::engine::side_effects::{Resource, spell_slot_level};
        let actor = self.actors.get(&actor_id)?;
        actor
            .actions
            .iter()
            .filter(|act| {
                act.is_harmful()
                    && act.deals_damage()
                    && matches!(act.targeting_schema(), TargetingSchema::SingleActor)
                    && act.reach_tiles().is_some()
            })
            .filter(|act| {
                // Cost resolved with no target, which is all the shape
                // check needs: every cost lane that varies with the
                // target varies in movement, and none of them conjures
                // a spell slot that wasn't already declared.
                let costs = act.cost(self, actor_id, None, None, None);
                spell_slot_level(&costs).is_none()
                    && !costs.iter().any(|c| {
                        matches!(c, Resource::Reaction | Resource::LegendaryAction)
                    })
            })
            .fold(None, |best: Option<&&'static (dyn crate::actions::action_template::Action + Send + Sync)>, act| {
                match best {
                    Some(current)
                        if current.reach_tiles().unwrap_or(0) >= act.reach_tiles().unwrap_or(0) =>
                    {
                        Some(current)
                    }
                    _ => Some(act),
                }
            })
            .copied()
    }

    /// Fire every readied attack the mover has just walked into.
    ///
    /// The mirror image of `dispatch_opportunity_attacks`, off the same
    /// event and the same `(from, to)` pair: an opportunity attack asks
    /// "were they in reach, and are they leaving it?", a readied attack
    /// asks "were they out of reach, and are they entering it?". The
    /// readier holds `Condition::Readied` (see the `Ready` action), and
    /// the swing is their longest-reaching attack — the same one they
    /// chose to hold.
    ///
    /// Both the reaction and the hold are spent on the swing, so a
    /// readier fires once. Unlike an opportunity attack, Disengage does
    /// nothing to stop this: RAW's Disengage suppresses attacks
    /// provoked by *leaving* reach, and walking into a raised bow was
    /// never a provocation in the first place.
    fn dispatch_readied_attacks(
        &mut self,
        mover_id: usize,
        from: Coordinate,
        to: Coordinate,
    ) {
        use crate::engine::side_effects::Resource;

        let (mover_team, mover_size) = match self.actors.get(&mover_id) {
            Some(a) => (a.team(), get_tiles_from_size(a.size())),
            None => return,
        };
        // Snapshot up-front: the loop body mutates the actor map.
        type ReadiedCandidate = (
            usize,
            &'static (dyn crate::actions::action_template::Action + Send + Sync),
            Coordinate,
            usize,
            isize,
        );
        let mut candidates: Vec<ReadiedCandidate> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if *id == mover_id || a.team() == mover_team || !a.is_combat_active() {
                    return None;
                }
                if !a.has_condition(Condition::Readied)
                    || !a.can_consume_resource(Resource::Reaction)
                {
                    return None;
                }
                // A charmed readier can't spend their held swing on the
                // creature that charmed them — the same gate the
                // opportunity-attack dispatcher applies, for the same
                // reason: neither path goes through `Action::validate`.
                if a.linked_by(Condition::Charmed) == Some(mover_id) {
                    return None;
                }
                let attack = self.best_readyable_attack(*id)?;
                let reach = attack.reach_tiles()?;
                Some((
                    *id,
                    attack,
                    a.location(),
                    get_tiles_from_size(a.size()),
                    reach,
                ))
            })
            .collect();
        // Sorted so several readiers covering the same doorway fire in a
        // fixed order rather than in whatever order the map iterates.
        candidates.sort_unstable_by_key(|(id, ..)| *id);

        for (reactor_id, attack, r_loc, r_size, reach) in candidates {
            if !self
                .actors
                .get(&reactor_id)
                .is_some_and(|a| a.is_combat_active() && a.can_consume_resource(Resource::Reaction))
            {
                continue;
            }
            let was_in_reach = footprint_chebyshev(r_loc, r_size, from, mover_size) <= reach;
            let now_in_reach = footprint_chebyshev(r_loc, r_size, to, mover_size) <= reach;
            if was_in_reach || !now_in_reach {
                continue;
            }
            // A readied ranged shot still needs to see its target; a
            // readied swing doesn't, for the same reason melee never
            // does. Checked against the tile the mover is stepping into,
            // which is where the shot is aimed.
            if attack.requires_los() && !self.has_line_of_sight(r_loc, to) {
                continue;
            }

            let reactor_name = self.actor_name(reactor_id);
            let mover_name = self.actor_name(mover_id);
            self.log(format!(
                "[reaction] {} looses their readied {} as {} closes",
                reactor_name,
                attack.name(),
                mover_name
            ));

            let target_vec = vec![mover_id];
            let effects = attack.side_effects(self, reactor_id, Some(&target_vec), None, None);
            for e in effects {
                e.apply(self);
            }
            if let Some(r) = self.actors.get_mut(&reactor_id) {
                r.consume_resource(Resource::Reaction);
                r.remove_condition(Condition::Readied);
            }

            self.cleanup_dead_actors();
            if self
                .actors
                .get(&mover_id)
                .is_none_or(|a| !a.is_combat_active())
            {
                return;
            }
        }
    }

    /// Iterate enemy actors with a Reaction slot and a melee attack; for each
    /// whose reach covered `mover` at `from` but no longer covers them at
    /// `to`, run the attack against the mover and consume the reaction.
    /// Stops early if the mover is downed mid-loop. Disengaging movers
    /// don't trigger OAs at all.
    fn dispatch_opportunity_attacks(
        &mut self,
        mover_id: usize,
        from: Coordinate,
        to: Coordinate,
    ) {
        use crate::actions::action_template::MELEE_REACH;
        use crate::engine::side_effects::Resource;

        // Mover-side **blanket** suppression — Disengage, Flyby, Agile —
        // read as one cohort off `MOVER_OA_SUPPRESSORS`. Bails before
        // snapshotting the candidate list (and the `None` arm covers the
        // no-such-actor case, since a mover who doesn't exist can hardly
        // provoke).
        //
        // 5e Swashbuckler Rogue **Fancy Footwork** (subclass level 3) is
        // deliberately *not* in that cohort: it doesn't blanket-suppress
        // OAs the way the three above do, it only suppresses them from
        // reactors the swash has already made a melee attack against
        // this turn. That surgical skip lives inside the candidate loop
        // below via `mover_fancy_footwork_targets`. Keeping the two
        // lanes separate lets a Swashbuckler with a spent bonus action
        // benefit from Fancy Footwork's targeted suppression without
        // burning Cunning Disengage's bonus action.
        let (mover_team, mover_size, mover_fancy_footwork_targets) =
            match self.actors.get(&mover_id) {
                Some(a) if a.suppresses_opportunity_attacks() => return,
                Some(a) => {
                    let footwork_targets = if a.has_fancy_footwork() {
                        Some(a.melee_attack_targets_this_turn_snapshot())
                    } else {
                        None
                    };
                    (a.team(), get_tiles_from_size(a.size()), footwork_targets)
                }
                None => return,
            };
        // Snapshot reactor candidates up-front — the loop body will mutate
        // self, which would conflict with holding an iterator into self.actors.
        type OaCandidate = (usize, &'static (dyn crate::actions::action_template::Action + Send + Sync), Coordinate, usize, isize);
        let candidates: Vec<OaCandidate> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if *id == mover_id || a.team() == mover_team || !a.is_combat_active() {
                    return None;
                }
                if !a.can_consume_resource(Resource::Reaction) {
                    return None;
                }
                // 5e Charmed: the charmer can walk away from a creature
                // they charmed without provoking. The OA dispatcher runs
                // the attack's side-effects directly rather than through
                // `Action::validate`, so the restriction has to be
                // re-checked here or it would only bind the charmed
                // creature on its own turn.
                if a.linked_by(Condition::Charmed) == Some(mover_id) {
                    return None;
                }
                // Shared "find the first melee weapon action" predicate —
                // `first_melee_weapon_action` gates on is_harmful (excludes
                // touch-range buffs / heals like Cure Wounds so an ally
                // doesn't opportunity-heal a fleeing target) + SingleActor
                // schema + reach <= MELEE_REACH. Riposte reads the same
                // helper on the target side.
                let attack = a.first_melee_weapon_action()?;
                let reach = attack.reach_tiles().unwrap_or(MELEE_REACH);
                Some((*id, attack, a.location(), get_tiles_from_size(a.size()), reach))
            })
            .collect();

        for (reactor_id, attack, r_loc, r_size, reach) in candidates {
            // Re-check liveness (an earlier OA in this loop may have changed things).
            if !self
                .actors
                .get(&reactor_id)
                .is_some_and(|a| a.is_combat_active() && a.can_consume_resource(Resource::Reaction))
            {
                continue;
            }
            let was_in_reach =
                footprint_chebyshev(r_loc, r_size, from, mover_size) <= reach;
            let still_in_reach =
                footprint_chebyshev(r_loc, r_size, to, mover_size) <= reach;
            if !was_in_reach || still_in_reach {
                continue;
            }
            // 5e Swashbuckler Rogue Fancy Footwork (subclass level 3):
            // if the mover holds the flag AND has made a melee attack
            // against this specific reactor during their current turn,
            // that reactor's OA is silently suppressed. Sibling gate to
            // Disengage above — Disengage blanket-suppresses every
            // reactor's OA for the turn, Fancy Footwork surgically
            // suppresses only the swash's melee-attack targets, so a
            // swash-vs-swarm move-out fires OAs from any flanker the
            // swash didn't swing at while sparing the ones they did.
            // Snapshot the ledger up-front to avoid re-borrowing the
            // mutating actor map inside the loop body.
            if let Some(footwork_targets) = mover_fancy_footwork_targets.as_ref()
                && footwork_targets.contains(&reactor_id)
            {
                continue;
            }

            let reactor_name = self.actor_name(reactor_id);
            let mover_name = self.actor_name(mover_id);
            self.log(format!(
                "[reaction] {} opportunity-attacks {} as they leave reach",
                reactor_name, mover_name
            ));

            // Run the underlying attack's side_effects directly (consumes
            // Reaction below, NOT the action's normal cost).
            let target_vec = vec![mover_id];
            let effects =
                attack.side_effects(self, reactor_id, Some(&target_vec), None, None);
            for e in effects {
                e.apply(self);
            }
            if let Some(r) = self.actors.get_mut(&reactor_id) {
                r.consume_resource(Resource::Reaction);
            }

            self.cleanup_dead_actors();
            // If the OA dropped the mover, no further OAs (and the move
            // caller is expected to abort).
            if self
                .actors
                .get(&mover_id)
                .is_none_or(|a| !a.is_combat_active())
            {
                return;
            }
        }
    }

    /// Sorted ids of every combat-active actor whose footprint lies
    /// within `radius` (footprint-Chebyshev gap) of `point`. Used by
    /// AoE / burst actions to find their hit list. Sorted by id so save
    /// rolls happen in deterministic order — the encounter roller is
    /// shared, and HashMap iteration order would otherwise leak through
    /// individual saves.
    pub fn actors_in_burst(&self, point: Coordinate, radius: isize) -> Vec<usize> {
        let mut ids: Vec<usize> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if !a.is_combat_active() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    point,
                    1,
                );
                if dist <= radius { Some(*id) } else { None }
            })
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Min footprint-Chebyshev gap from `actor_id`'s body to a single
    /// tile `point`. Used by Burst-targeted actions whose "reach" is the
    /// max distance from the caster's footprint to the burst origin.
    pub fn footprint_distance_to_point(
        &self,
        actor_id: usize,
        point: Coordinate,
    ) -> Option<isize> {
        let a = self.actors.get(&actor_id)?;
        Some(footprint_chebyshev(
            a.location(),
            get_tiles_from_size(a.size()),
            point,
            1,
        ))
    }

    /// True if any tile of `actor_id`'s footprint can see `point`. Used by
    /// AoE spells that require LOS to the burst origin (most do).
    pub fn actor_has_line_of_sight_to_point(
        &self,
        actor_id: usize,
        point: Coordinate,
    ) -> bool {
        let Some(a) = self.actors.get(&actor_id) else {
            return false;
        };
        let a_size = get_tiles_from_size(a.size()) as isize;
        let a_loc = a.location();
        for ay in 0..a_size {
            for ax in 0..a_size {
                let from = Coordinate::new(a_loc.x + ax, a_loc.y + ay);
                if self.has_line_of_sight(from, point) {
                    return true;
                }
            }
        }
        false
    }

    /// Shared body for the three public burst-target helpers
    /// (`enemy_burst_targets` / `ally_burst_targets` / `neutral_burst_targets`).
    /// `keep` decides whether a candidate id should be admitted; it sees
    /// the candidate's id, the candidate actor, and the caster's team.
    /// Returning `true` keeps the id; `false` drops it. The geometry +
    /// combat-active + sort invariants live here so adding a new
    /// burst-target lens is a one-line lambda over a single chokepoint.
    fn burst_targets_with<F>(
        &self,
        caster_id: usize,
        point: Coordinate,
        radius: isize,
        keep: F,
    ) -> Vec<usize>
    where
        F: Fn(usize, &ActorInstance, usize) -> bool,
    {
        let Some(caster) = self.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_team = caster.team();
        let mut ids: Vec<usize> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if !a.is_combat_active() {
                    return None;
                }
                if !keep(*id, a, caster_team) {
                    return None;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    point,
                    1,
                );
                if dist <= radius { Some(*id) } else { None }
            })
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Sorted ids of combat-active actors inside the burst that are *not*
    /// on the caster's team. Stinking Cloud / Cloud of Daggers / Synaptic
    /// Static and similar enemy-only AoEs use this so allies inside the
    /// blast don't catch friendly fire. Caster is implicitly excluded
    /// via the team match.
    pub fn enemy_burst_targets(
        &self,
        caster_id: usize,
        point: Coordinate,
        radius: isize,
    ) -> Vec<usize> {
        self.burst_targets_with(caster_id, point, radius, |_id, a, caster_team| {
            a.team() != caster_team
        })
    }

    /// Sorted ids of combat-active actors inside the burst that *are* on
    /// the caster's team. Beacon of Hope / Mass Cure Wounds use this for
    /// the friendly-only target list. Caster is implicitly included if
    /// they're in the burst.
    pub fn ally_burst_targets(
        &self,
        caster_id: usize,
        point: Coordinate,
        radius: isize,
    ) -> Vec<usize> {
        self.burst_targets_with(caster_id, point, radius, |_id, a, caster_team| {
            a.team() == caster_team
        })
    }

    /// `ally_burst_targets` for a burst that *heals*: the same team and
    /// footprint filters, plus the allies who are down.
    ///
    /// The shared burst helper keeps only `is_combat_active()` actors,
    /// which is right for a buff — Bless on an unconscious ally does
    /// nothing — and exactly wrong for a heal, because the unconscious
    /// ally is the one the heal is for. A cleric standing over a dying
    /// friend casting Prayer of Healing used to top up everyone else in
    /// the room and step over the body.
    ///
    /// Mass Cure Wounds and Mass Healing Word already got this right,
    /// by hand, in their own candidate walks — `!is_combat_active() &&
    /// !is_dying()` is the filter both of them spell out. The three
    /// burst heals that reached for the shared helper instead inherited
    /// a rule written for buffs. This is that filter, named, so the
    /// next burst heal picks it up by asking for the heal variant
    /// rather than by remembering.
    ///
    /// Stable-at-0 allies count too: `Heal` lifts them back to
    /// consciousness through the same `HealOutcome::Revived` path, so
    /// the gate is "at 0 HP but not gone" rather than "actively rolling
    /// death saves".
    pub fn ally_heal_burst_targets(
        &self,
        caster_id: usize,
        point: Coordinate,
        radius: isize,
    ) -> Vec<usize> {
        let Some(caster) = self.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_team = caster.team();
        let mut ids: Vec<usize> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if a.team() != caster_team {
                    return None;
                }
                if !a.is_combat_active() && !a.is_dying() && !a.is_stable() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    point,
                    1,
                );
                if dist <= radius { Some(*id) } else { None }
            })
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Sorted ids of every combat-active actor inside the burst —
    /// friend or foe, *except* the caster themselves. Friend-or-foe-
    /// agnostic spells (Web, Sleet Storm, Plant Growth, Spike Stones)
    /// use this so any creature caught in the area gets snared
    /// regardless of allegiance.
    pub fn neutral_burst_targets(
        &self,
        caster_id: usize,
        point: Coordinate,
        radius: isize,
    ) -> Vec<usize> {
        self.burst_targets_with(caster_id, point, radius, |id, _a, _ct| id != caster_id)
    }

    /// Sorted ids of every combat-active enemy standing **within five
    /// feet** of `actor_id` — a footprint gap of `MELEE_REACH`.
    ///
    /// Both callers are RAW clauses that say "within 5 feet" in those
    /// words: the ranged-attack disadvantage in
    /// `compute_attack_mode`, and Ashardalon's Stride's blazing wake.
    /// Both used to read a gap of **zero** — footprints actually
    /// touching, which on a 2.5 ft grid is a creature standing in
    /// contact rather than one five feet away, and which is strictly
    /// narrower than the reach of every melee weapon in the game.
    ///
    /// The visible consequence was on the disadvantage clause. A caster
    /// with an enemy one tile out — inside that enemy's reach, and
    /// inside its own, and unable to move without provoking — shot at no
    /// penalty at all. So every gish in the engine had a free ranged
    /// attack from exactly the tile 5e's rule exists to punish, and the
    /// AI's attack picker, which reads the mode, was told the cantrip
    /// was as good as the sword.
    ///
    /// Uses `footprint_distance` so both sides of the gap check honor
    /// the actor's full size category; a Medium caster next to a Small
    /// goblin reads as adjacent even though their origin tiles sit one
    /// tile apart. Returns an empty vec when `actor_id` is missing.
    pub fn combat_active_enemy_ids_adjacent(&self, actor_id: usize) -> Vec<usize> {
        use crate::actions::action_template::MELEE_REACH;
        let Some(actor) = self.actors.get(&actor_id) else {
            return Vec::new();
        };
        let team = actor.team();
        let mut ids: Vec<usize> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if *id == actor_id || !a.is_combat_active() || a.team() == team {
                    return None;
                }
                let dist = self.footprint_distance(actor_id, *id)?;
                if dist <= MELEE_REACH { Some(*id) } else { None }
            })
            .collect();
        ids.sort_unstable();
        ids
    }

    /// True iff `attacker_id` has at least one combat-active teammate
    /// (excluding themselves) that is footprint-adjacent to `target_id`
    /// AND passes the caller's predicate. The shared "ally anchors the
    /// target" scan — Pack Tactics (no extra predicate), Wolf Totem
    /// (predicate: raging + WOLF_TOTEM_TAG), and any future ally-aura
    /// rider (Help-from-an-adjacent-ally, Mark of the Pack-style auras)
    /// route through this single chokepoint. Returns false if either
    /// id is unknown — defensive null-shielding mirrors the rest of
    /// the compute-mode plumbing.
    pub fn has_ally_adjacent_to<F>(
        &self,
        attacker_id: usize,
        target_id: usize,
        predicate: F,
    ) -> bool
    where
        F: Fn(&ActorInstance) -> bool,
    {
        let Some(attacker) = self.actors.get(&attacker_id) else {
            return false;
        };
        let team = attacker.team();
        self.actors.iter().any(|(id, a)| {
            *id != attacker_id
                && a.team() == team
                && a.is_combat_active()
                && predicate(a)
                && self.footprint_distance(*id, target_id).is_some_and(|d| d == 0)
        })
    }

    /// 5e Fighting Style: **Protection** — find the first ally of the
    /// target that qualifies to burn a reaction and impose disadvantage
    /// on this attack. Qualifier RAW clauses (approximated):
    /// - Ally is footprint-adjacent to the target (RAW "within 5 feet").
    /// - Ally is combat-active AND has an unspent reaction this round.
    /// - Ally is NOT the target itself (RAW "target other than you").
    /// - Ally is NOT incapacitated / stunned / unconscious (a slot RAW
    ///   forbids from spending reactions).
    /// - Ally holds `has_protection_style()`.
    /// - Attacker isn't Invisible to the protector (approximated as
    ///   "protector is not Blinded" — the RAW "you can see" gate is a
    ///   sight check the engine doesn't model with per-actor line-of-
    ///   sight, so we tax only the obvious blindness case).
    ///
    /// Returns `None` if the attacker or target id is unknown, if the
    /// attacker and target are on the same team (protection doesn't fire
    /// on friendly-fire pings — protectors want to shield their own
    /// team's members from the OTHER team), or if no ally qualifies.
    ///
    /// Returned id ordering is deterministic in actor id — the actor
    /// map's iteration is stable-by-id via `BTreeMap`, so seed
    /// reproducibility is preserved across the eligibility scan.
    ///
    /// This is a `&self` lookup — the reaction spend and log line are
    /// handled by the caller (`engine::attack::resolve_attack`) after
    /// the mode read completes.
    pub fn first_eligible_protector(
        &self,
        attacker_id: usize,
        target_id: usize,
    ) -> Option<usize> {
        // The "don't burn a reaction on a swing that is already taxed"
        // rule used to live here, as a `Blinded`-attacker
        // short-circuit — the one disadvantage source this function
        // could see. It now lives in `apply_reactive_attack_taxes`,
        // which holds the whole tally and can therefore see every
        // other one too. Deliberately not duplicated: two places
        // spelling the same rule at different strengths is how the
        // narrow version survived as long as it did.
        //
        // Interception is the sibling that correctly has no such gate —
        // it reduces damage after the swing lands and does not care
        // whether the roll was taxed — which is why the shared scan
        // below never folded this in.
        self.first_reactive_ally_within(
            attacker_id,
            target_id,
            0,
            &|a| a.has_protection_style(),
            None,
        )
    }

    /// 5e Fighting Style: **Interception** (XGtE, lv1 pick). When a
    /// creature the interceptor can see hits a target OTHER than them
    /// with a weapon or spell attack within 5 ft, the interceptor may
    /// use their reaction to reduce the damage by `1d10 + proficiency
    /// bonus`. Sibling to `first_eligible_protector` — same target-
    /// adjacent-ally scan, same reaction / sight gates, but no
    /// attacker-Blinded short-circuit: Interception reduces the damage
    /// that lands, so it's worth firing even against a disadvantage-
    /// taxed swing.
    ///
    /// This is a `&self` lookup — the reaction spend, 1d10 roll, and
    /// damage clamp are handled by the caller
    /// (`engine::attack::resolve_attack_outcome` and
    /// `spell_attack_outcome` — RAW says "weapon or spell attack").
    ///
    /// Returns `None` when the attacker / target ids are unknown, when
    /// they're on the same team (friendly-fire — no interception),
    /// or when no ally qualifies. Actor id order is used for the pick
    /// so seed reproducibility is preserved.
    pub fn first_eligible_interceptor(
        &self,
        attacker_id: usize,
        target_id: usize,
    ) -> Option<usize> {
        self.first_reactive_ally_within(
            attacker_id,
            target_id,
            0,
            &|a| a.has_interception_style(),
            None,
        )
    }

    /// The "I take that one" cohort: features whose holder spends a
    /// reaction to have a blow aimed at somebody nearby land on them
    /// instead.
    ///
    /// This is a different lane from every other defensive feature in
    /// the engine, and the difference is worth stating. Uncanny Dodge,
    /// Parry, Interception and Warding Maneuver all *clamp* — the damage
    /// stays where it landed and gets smaller. Mirror Image and Illusory
    /// Self *intercept* — the attack is retroactively un-hit. Warding
    /// Bond *mirrors* — the partner takes a copy, and the original still
    /// lands. These two *move* it: the target takes nothing at all, and
    /// the whole amount arrives somewhere else.
    ///
    /// Because the lane hangs off `DealDamage` rather than off an attack
    /// chokepoint, it catches everything the clamp cohort cannot — a
    /// failed save against a fireball, a poison drip at round end, a
    /// death burst. That breadth is RAW on both rows ("takes damage",
    /// with no qualifier) and it is most of what either feature is
    /// worth.
    ///
    /// Three rows, differing in reach and in what they ask of the
    /// creature being covered — which is the whole reason the cohort
    /// exists rather than three open-coded scans:
    ///
    ///   - **Divine Allegiance** (Oath of the Crown Paladin, subclass
    ///     level 7): "when a creature within 5 feet of you takes damage,
    ///     you can use your reaction to magically substitute your own
    ///     health for that of the target creature."
    ///   - **Aura of the Guardian** (Oath of Redemption Paladin,
    ///     subclass level 7): the same sentence at 10 ft, which on this
    ///     chassis is the same radius the paladin's other two auras
    ///     already project (`PALADIN_AURA_RADIUS`).
    ///   - **Protective Bond** (Peace Domain Cleric, subclass level 6):
    ///     30 ft, and the only row with a `covers` gate — RAW's
    ///     sentence is about *bonded* creatures, so the cleric takes a
    ///     blow for somebody carrying `Emboldened` and for nobody else.
    ///
    /// That gate is the reason the rows are a struct rather than a
    /// tuple. Without it the widest row on the cohort would also have
    /// been the least discriminating: a cleric volunteering for every
    /// blow landed on anyone within 30 feet, which is not the feature
    /// and is a strictly better one.
    ///
    /// Ordered narrowest-first, which is the target-favorable reading
    /// when a hypothetical multiclass holds several: the shorter-ranged
    /// feature is the one with fewer creatures it could have spent
    /// itself on, so it is the one to spend here. In practice the three
    /// belong to two oaths and a domain and never co-occur.
    const DAMAGE_INTERPOSERS: &'static [DamageInterposer] = &[
        DamageInterposer {
            tag: crate::actions::class_features::DIVINE_ALLEGIANCE_TAG,
            // 5 ft. `footprint_distance` is a gap, so 1 is "one tile
            // between us" on the 2.5 ft grid.
            radius: 1,
            covers: None,
            label: "divine allegiance",
        },
        DamageInterposer {
            tag: crate::actions::class_features::AURA_OF_THE_GUARDIAN_TAG,
            radius: Self::PALADIN_AURA_RADIUS,
            covers: None,
            label: "aura of the guardian",
        },
        DamageInterposer {
            tag: crate::actions::class_features::PROTECTIVE_BOND_TAG,
            // 30 ft — the bond's own radius, and the widest row here.
            radius: 12,
            // RAW: "when a creature that has your Emboldening Bond
            // takes damage". See `PROTECTIVE_BOND_TAG` for the two RAW
            // clauses that still don't ship.
            covers: Some(Condition::Emboldened),
            label: "protective bond",
        },
    ];

    /// Find the ally who will carry `target_id`'s damage, spend their
    /// reaction, and hand back their id plus the log label of the
    /// feature that paid for it — or `None` when nobody steps in.
    ///
    /// Called from `DealDamage::apply` before anything else touches the
    /// number, because the whole point is that the blow never reaches
    /// the creature it was aimed at.
    ///
    /// Four gates, three of them RAW:
    ///   - The interposer holds the feature, is combat-active, is within
    ///     the row's radius, and has a reaction left.
    ///   - They are on the target's team and are not the target. Nobody
    ///     takes a blow for an enemy, and nobody can take one for
    ///     themselves.
    ///   - Zero damage buys nothing, so it doesn't cost a reaction.
    ///   - **Not RAW:** damage already being carried for somebody can't
    ///     be handed on again. Nothing in either feature's text forbids
    ///     the chain, but two adjacent paladins would otherwise volley a
    ///     single blow between them until both reactions were gone,
    ///     which is not what either of them meant to do.
    ///
    /// Ties break on the lowest actor id so a seeded run reproduces.
    pub fn claim_damage_interposition(
        &mut self,
        target_id: usize,
        amount: u32,
    ) -> Option<(usize, &'static str)> {
        if amount == 0 || self.in_damage_redirect() {
            return None;
        }
        let target = self.actors.get(&target_id)?;
        let target_team = target.team();
        for &DamageInterposer {
            tag,
            radius,
            covers,
            label,
        } in Self::DAMAGE_INTERPOSERS
        {
            // The row's own gate on the creature being covered, read
            // once before the holder scan rather than per candidate —
            // a row whose condition the target isn't carrying has
            // nobody it could spend itself on.
            if covers.is_some_and(|c| {
                self.actors
                    .get(&target_id)
                    .is_none_or(|t| !t.has_condition(c))
            }) {
                continue;
            }
            // One pass keeping the lowest qualifying id, rather than
            // collect-sort-find. Every damage instance in the game runs
            // this lookup, almost none of them find anybody, and the
            // sort was an allocation per hit to order a list that is
            // thrown away. The minimum is the same answer the sorted
            // walk gave, so the determinism a seeded run depends on is
            // unchanged.
            //
            // The cheap gates go first so a table with no paladin in it
            // never reaches the distance computation.
            let Some(guardian) = self
                .actors
                .iter()
                .filter(|(id, a)| {
                    **id != target_id
                        && a.team() == target_team
                        && a.has_passive_feature(tag)
                        && a.is_combat_active()
                        && a.has_reaction()
                })
                .map(|(id, _)| *id)
                .filter(|&id| {
                    self.footprint_distance(id, target_id)
                        .is_some_and(|d| d <= radius)
                })
                .min()
            else {
                continue;
            };
            self.actors
                .get_mut(&guardian)?
                .consume_resource(crate::engine::side_effects::Resource::Reaction);
            return Some((guardian, label));
        }
        None
    }

    /// Run `body` with the damage-redirect guard raised, so a blow being
    /// carried for someone can't be handed on a second time. Paired
    /// enter/exit rather than a bare flag for the reason
    /// `enter_multiattack` is: the guard has to come back down on every
    /// path out.
    /// Whether a blow is currently being carried for somebody else.
    ///
    /// The read side of `within_damage_redirect`'s guard, named so the
    /// redirect claims can ask without reaching into the field. Both of
    /// them — Divine Allegiance and the rider's interposition — decline
    /// while it is up, so a hit can be taken for a friend once and not
    /// passed around a circle of them.
    pub(crate) fn in_damage_redirect(&self) -> bool {
        self.redirect_depth > 0
    }

    pub fn within_damage_redirect<R>(&mut self, body: impl FnOnce(&mut Self) -> R) -> R {
        self.redirect_depth += 1;
        let out = body(self);
        self.redirect_depth -= 1;
        out
    }

    /// Shared eligibility scan for the "nearby ally with a reaction and a
    /// per-feature flag" cohort — Fighting Style Protection, Fighting
    /// Style Interception, and Psi Warrior Protective Field all need the
    /// same in-range, ally-team, has-reaction, can-see-attacker,
    /// has-FLAG filter over a deterministic id order. The "can see"
    /// clause routes through `viewer_can_see` so an Invisible / Blurred
    /// / Displaced attacker (that the ally doesn't pierce) bounces every
    /// one of them — pre-refactor this only checked `!Blinded` on the
    /// ally, silently letting an invisible attacker still draw the
    /// reactive shield. Extracted so a future sibling lands as one line
    /// — the closure picks the per-feature flag.
    ///
    /// Two axes differentiate the callers:
    ///   - `max_tiles`: footprint-Chebyshev radius the ally must be
    ///     within. Both Fighting Styles are RAW "within 5 feet" → `0`
    ///     (footprint distance 0 means touching); Protective Field's
    ///     RAW "within 30 feet" → `12` on the 2.5ft grid.
    ///   - `feature_tag`: `Some(tag)` also requires an unspent
    ///     `feature_available(tag)` charge (Protective Field's psionic
    ///     energy dice); `None` for the always-on Fighting Styles.
    ///
    /// Returns `None` for missing ids or same-team swings (friendly-fire
    /// doesn't draw the tax on any of them). Callers own the reaction
    /// spend / log line and any feature-specific extra gates (e.g.
    /// Protection's attacker-Blinded early-out).
    pub(crate) fn first_reactive_ally_within(
        &self,
        attacker_id: usize,
        target_id: usize,
        max_tiles: isize,
        has_flag: &dyn Fn(&ActorInstance) -> bool,
        feature_tag: Option<&'static str>,
    ) -> Option<usize> {
        let attacker = self.actors.get(&attacker_id)?;
        let target = self.actors.get(&target_id)?;
        // Same-team swings (friendly-fire, e.g. a Confused ally) don't
        // draw the reactive-style tax — the ally's own team is doing
        // the attacking, so there's no one to shield against.
        if attacker.team() == target.team() {
            return None;
        }
        let target_team = target.team();
        // Actor id order — `HashMap` iteration isn't stable across runs,
        // so we collect + sort to keep the "first eligible" pick
        // deterministic under identical seeds. Keeps seed reproducibility
        // intact across the eligibility scan.
        let mut ids: Vec<usize> = self.actors.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            if id == target_id {
                continue;
            }
            let Some(a) = self.actors.get(&id) else {
                continue;
            };
            // Gate cohort: same-team, alive-and-combat-active, holds the
            // per-style flag, has an unspent reaction, and can see the
            // attacker.
            //
            // `has_reaction()` covers Stunned / Paralyzed / Incapacitated
            // / Unconscious / Petrified / Mazed / Sphered via
            // `blocks_action_economy` and NoReaction / Confused via
            // `blocks_reactions` — the reaction lane's action-economy
            // filter. The "you can see the attacker" clause routes
            // through `viewer_can_see` — same helper the Warding Flare
            // gate uses — so an Invisible attacker (or a Blurred /
            // Displaced attacker the ally doesn't pierce) doesn't
            // still draw the ally's reactive shield. Pre-refactor this
            // only checked `!Blinded` on the ally, letting an invisible
            // goblin still burn the paladin's Protection reaction even
            // though RAW they can't see the source of the swing.
            if a.team() != target_team
                || !a.is_combat_active()
                || !has_flag(a)
                || !a.has_reaction()
                || feature_tag.is_some_and(|t| !a.feature_available(t))
                || !self.viewer_can_see(id, attacker_id)
            {
                continue;
            }
            if self
                .footprint_distance(id, target_id)
                .is_none_or(|d| d > max_tiles)
            {
                continue;
            }
            return Some(id);
        }
        None
    }

    /// Footprint-Chebyshev distance between two living actors, or `None` if
    /// either id is unknown. 0 means they're touching/adjacent.
    pub fn footprint_distance(&self, a_id: usize, b_id: usize) -> Option<isize> {
        let a = self.actors.get(&a_id)?;
        let b = self.actors.get(&b_id)?;
        Some(footprint_chebyshev(
            a.location(),
            get_tiles_from_size(a.size()),
            b.location(),
            get_tiles_from_size(b.size()),
        ))
    }

    /// True if both ids resolve to live actors and they sit on the same
    /// team. Used by support / buff spells (Bless, Aid, Enhance Ability,
    /// Longstrider, Heroism, Healing Word) at the side-effect chokepoint
    /// to short-circuit on a hostile-aimed cast — the picker UI should
    /// have steered the caster to an ally, but the safety net catches
    /// any input-shape edge case (a mind-controlled caster trying to
    /// heal their captor, a UI override). Returns false if either id
    /// resolves to a missing actor — symmetric with `footprint_distance`'s
    /// `None`-on-missing semantics, but folded into a single bool here so
    /// call sites can collapse the four-line `is_none_or(|t| t.team() != caster_team)`
    /// dance to a one-liner.
    pub fn actors_allied(&self, a_id: usize, b_id: usize) -> bool {
        let Some(a) = self.actors.get(&a_id) else {
            return false;
        };
        let Some(b) = self.actors.get(&b_id) else {
            return false;
        };
        a.team() == b.team()
    }

    /// 5e Charmed: "A charmed creature can't attack the charmer or
    /// target the charmer with harmful abilities or magic effects."
    /// True when `actor_id` is under that restriction with respect to
    /// `target_id`.
    ///
    /// Both halves are required — the `Charmed` condition *and* the
    /// `Charmed` back-link — so an actor who is charm-immune (and thus
    /// never received the condition) is unaffected even if a
    /// `SetConditionLink(Condition::Charmed)` ran in isolation, and a charm from a source the
    /// engine didn't link doesn't silently forbid every attack.
    ///
    /// Centralized because the restriction has to hold on three lanes
    /// that reach hostility by different routes, and only the first of
    /// them runs through action validation:
    ///   - **Declared actions** (`Action::validate`) — the charmed
    ///     creature's own turn.
    ///   - **Opportunity attacks** — the charmer walks out of reach.
    ///     The OA dispatcher runs the attack's `side_effects` directly
    ///     and never calls `validate`, so this gate has to be checked
    ///     at the candidate filter.
    ///   - **Riposte** — the charmer misses the charmed creature in
    ///     melee. Same story: a counter-*attack*, dispatched outside
    ///     validation. Its sibling reactions (Uncanny Dodge, Deflect
    ///     Missiles, Parry) are deliberately *not* gated on this: they
    ///     are self-clamps, not attacks, and RAW lets a charmed
    ///     creature defend itself against its charmer perfectly well.
    pub fn charm_blocks_hostility(&self, actor_id: usize, target_id: usize) -> bool {
        self.actors.get(&actor_id).is_some_and(|a| {
            a.linked_by(Condition::Charmed) == Some(target_id)
        })
    }

    /// Walk every ally-team actor that's `is_combat_active` OR `is_dying`
    /// and within `range_tiles` footprint-Chebyshev gap of `caster_id`,
    /// returning `(id, distance)` for each. The caster itself is included
    /// (distance 0) — every multi-ally consumable today treats the holder
    /// as a candidate. Returns an empty Vec when `caster_id` is missing.
    ///
    /// Centralizes the "find candidate allies in range" walker the mass-
    /// heal / mass-buff / Aid scroll factors all share so they don't each
    /// re-implement the team-eq + alive-or-dying + chebyshev-gap loop.
    /// Callers can layer their own priority sort on top (HP deficit for
    /// heals, condition-status for buffs, HP% for Aid).
    pub fn ally_candidates_in_range(
        &self,
        caster_id: usize,
        range_tiles: isize,
    ) -> Vec<(usize, isize)> {
        let Some(caster) = self.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_team = caster.team();
        let caster_loc = caster.location();
        let caster_size = get_tiles_from_size(caster.size());
        self.actors
            .iter()
            .filter_map(|(id, a)| {
                if a.team() != caster_team {
                    return None;
                }
                if !a.is_combat_active() && !a.is_dying() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    caster_loc,
                    caster_size,
                    a.location(),
                    get_tiles_from_size(a.size()),
                );
                if dist > range_tiles {
                    return None;
                }
                Some((*id, dist))
            })
            .collect()
    }

    /// The ally within `range_tiles` who is missing the most hit
    /// points, skipping every id in `exclude` and every ally already at
    /// full HP. Returns `None` when nobody qualifies.
    ///
    /// Built on `ally_candidates_in_range` directly above, so it shares
    /// that helper's team / combat-active / footprint-Chebyshev filters
    /// — including its deliberate inclusion of *dying* allies, which is
    /// what lets an overflow heal pick up a creature at 0 HP rather
    /// than stepping over the one target on the field who most needs
    /// it.
    ///
    /// Ties break on the lower actor id so the pick stays deterministic
    /// under a fixed seed; `self.actors` is a hash map and its
    /// iteration order is not.
    ///
    /// The Circle of Stars Druid's Chalice overflow is the first
    /// caller. It is written as a general "who should this spill onto"
    /// question rather than as part of that feature because it is one:
    /// any future rider that has healing to place and no target named
    /// for it wants exactly this.
    pub fn most_wounded_ally_within(
        &self,
        caster_id: usize,
        range_tiles: isize,
        exclude: &[usize],
    ) -> Option<usize> {
        self.ally_candidates_in_range(caster_id, range_tiles)
            .into_iter()
            .filter_map(|(id, _)| {
                if exclude.contains(&id) {
                    return None;
                }
                let actor = self.actors.get(&id)?;
                let missing = actor.max_hitpoints().saturating_sub(actor.hitpoints());
                if missing == 0 {
                    return None;
                }
                Some((missing, id))
            })
            // `max_by_key` keeps the LAST maximum, so the id half of the
            // key is negated to turn "largest id wins the tie" into
            // "smallest id wins".
            .max_by_key(|&(missing, id)| (missing, std::cmp::Reverse(id)))
            .map(|(_, id)| id)
    }

    /// Negation of `actors_allied` that also fails on missing actors —
    /// "these two ids are both live AND on opposite teams." Used by
    /// harmful actions that want to short-circuit on a self-/ally-aimed
    /// cast (sanity check; the picker shouldn't allow this either). A
    /// missing actor returns false rather than true so the helper never
    /// reports a phantom enemy.
    pub fn actors_enemies(&self, a_id: usize, b_id: usize) -> bool {
        let Some(a) = self.actors.get(&a_id) else {
            return false;
        };
        let Some(b) = self.actors.get(&b_id) else {
            return false;
        };
        a.team() != b.team()
    }

    /// First step the actor should take to reach a footprint-adjacent square
    /// next to `target_id`. Uses 8-connected BFS over walkable tiles for the
    /// actor's footprint; finds the *shortest-step-count* path, ignoring
    /// movement budget (the AI may need several turns to close in). Returns
    /// `None` if already adjacent or no path exists.
    ///
    /// Tried twice: once refusing to route through any tile that is bad
    /// ground *for this walker*, and — only if that finds nothing —
    /// once without the refusal. That is the whole of the engine's
    /// answer to "should I walk through the web", and it is the right
    /// shape for it: a creature goes around a hazard when going around
    /// is possible, and walks through it when the alternative is not
    /// reaching the fight at all. Skipped entirely on a board with no
    /// bad ground on it, which is nearly every board.
    pub fn step_toward_actor(&self, actor_id: usize, target_id: usize) -> Option<Coordinate> {
        if self.has_bad_ground_for(actor_id)
            && let Some(step) = self.step_toward_actor_inner(actor_id, target_id, true)
        {
            return Some(step);
        }
        self.step_toward_actor_inner(actor_id, target_id, false)
    }

    /// True if any tile on the board is worth `actor_id` walking
    /// around. The cheap board-level pre-check that lets the pathfinder
    /// skip its first pass entirely on the common board.
    ///
    /// Ordered cheapest-first and short-circuiting, because this runs
    /// once per AI move decision on every board including the empty
    /// ones. The zone scan is a walk of a list that is usually empty;
    /// the water scan behind it is a walk of the whole terrain grid, so
    /// it is gated on the walker actually being in trouble — which is
    /// almost never, and never at all on a board with no lake.
    fn has_bad_ground_for(&self, actor_id: usize) -> bool {
        let aversions = self.aversions_of(actor_id);
        self.zones
            .iter()
            .any(|z| z.effect.deters_walkers() || (z.effect.suppresses_magic && aversions.casts))
            || (aversions.drowns && self.has_water())
    }

    /// Everything about `actor_id` that makes a tile bad ground for it
    /// and would otherwise have to be re-derived per candidate tile.
    ///
    /// Resolved once per path by `step_toward_actor_inner`, because
    /// neither answer can change while a single search runs and both
    /// are more than a field read: one is a scan of an action list, the
    /// other a scan of a cohort.
    fn aversions_of(&self, actor_id: usize) -> WalkerAversions {
        WalkerAversions {
            casts: self.actor_casts_spells(actor_id),
            // Out of breath *and* no gills. Deliberately not "cannot
            // breathe water": a creature with its lungs full has ten
            // rounds or more of slack and should wade through a pond
            // like anybody else — 5e charges nothing for that, and an
            // AI that treated every puddle as a hazard would path
            // around water for the whole game to avoid a cost it was
            // never going to pay. The tile only becomes bad ground at
            // the moment the next round in it starts costing rungs.
            drowns: self.actors.get(&actor_id).is_some_and(|a| {
                a.breath_rounds() == 0 && !a.breathes_underwater()
            }),
        }
    }

    /// Is this tile bad ground to stand on, for a walker with these
    /// aversions?
    ///
    /// Three different senses of bad, and the last two are why the
    /// walker's nature is a parameter rather than something the tile
    /// knows:
    ///
    ///   - it can hurt anybody who stands there (`tile_is_hazardous`);
    ///   - it is an Antimagic Field and the walker casts. Nothing lands
    ///     on a caster standing in dead magic — it simply loses its
    ///     turn, which for a wizard is worse than a web. For everybody
    ///     else the same tile is open ground, and telling a barbarian
    ///     to walk around it would be strictly worse pathing.
    ///   - it is water and the walker has run out of breath in it. Same
    ///     shape as the field: the tile is perfectly good ground for
    ///     the shark chasing them across it.
    ///
    /// The water clause reads the anchor tile rather than asking
    /// `is_immersed` about the whole footprint, which is stricter than
    /// the rule it serves — a Medium creature with one corner on the
    /// bank is not suffocating. Stricter is the right direction here:
    /// the question is where to *walk*, and a creature that has already
    /// run out of air has no business picking its way along the
    /// shallows counting which corner is dry.
    fn tile_is_bad_ground(&self, coord: Coordinate, aversions: WalkerAversions) -> bool {
        self.tile_is_hazardous(coord)
            || (aversions.casts && self.tile_suppresses_magic(coord))
            || (aversions.drowns
                && self
                    .terrain_at(coord)
                    .is_some_and(|t| t.terrain_type.is_water()))
    }

    /// True if `actor_id` has anything on its action list that is a
    /// spell — `Action::school()`, the same marker the casting gates
    /// read.
    ///
    /// Asked of the action list rather than of a spell-slot table
    /// because cantrips have no slots, and a creature whose only magic
    /// is a cantrip still loses it to an Antimagic Field.
    pub fn actor_casts_spells(&self, actor_id: usize) -> bool {
        self.actors
            .get(&actor_id)
            .is_some_and(|a| a.actions.iter().any(|act| act.school().is_some()))
    }

    /// The eight neighbour offsets, ordered by how directly they head at
    /// `toward`.
    ///
    /// Every one of them is a legal step and the BFS will consider them all;
    /// the order decides which of several equally-short paths it finds
    /// *first*, because `parent` is written on first visit. Walking them in
    /// raw `-1..=1` nesting order — which is what this replaces — meant
    /// "up-and-left" was always tried first, so a creature crossing open
    /// ground toward a target due east arrived by a staircase of diagonals
    /// and cardinals rather than by walking east.
    ///
    /// Nothing was wrong with those paths: they cost the same and end in the
    /// same place. Two things are better about straight ones. They look like
    /// what a creature would do, which matters in a game whose whole output
    /// is a picture of a board. And 5e has a family of rules that pay for
    /// straight movement specifically — the Charge and Pounce clauses on a
    /// dozen stat blocks — which a staircase collects almost none of.
    ///
    /// The key is Chebyshev distance to the target after the step, then
    /// Manhattan distance as the tie-break. Chebyshev alone is what the
    /// board charges for — a diagonal costs one step like a cardinal —
    /// so on an eight-connected grid it ties three different steps
    /// whenever the target is due east, and picking among them by
    /// declaration order is exactly the staircase this exists to avoid.
    /// Manhattan breaks the tie the way a creature would: of the steps
    /// that close the distance equally, take the one that doesn't also
    /// drift off the line.
    ///
    /// Together they give diagonals first and then a straight tail —
    /// which is both the shortest path and the one that collects the
    /// charge.
    ///
    /// `sort_by_key` is stable, so genuinely equivalent steps keep their
    /// declaration order and the walk stays deterministic.
    fn steps_toward(from: Coordinate, toward: Coordinate) -> [(isize, isize); 8] {
        let mut steps = [
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, -1),
            (0, 1),
            (1, -1),
            (1, 0),
            (1, 1),
        ];
        steps.sort_by_key(|(dx, dy)| {
            let gap_x = (from.x + dx - toward.x).abs();
            let gap_y = (from.y + dy - toward.y).abs();
            (gap_x.max(gap_y), gap_x + gap_y)
        });
        steps
    }

    fn step_toward_actor_inner(
        &self,
        actor_id: usize,
        target_id: usize,
        avoid_hazards: bool,
    ) -> Option<Coordinate> {
        use std::collections::{HashMap, VecDeque};

        let actor = self.actors.get(&actor_id)?;
        let target = self.actors.get(&target_id)?;
        let start = actor.location();
        let my_size = get_tiles_from_size(actor.size());
        let t_loc = target.location();
        let t_size = get_tiles_from_size(target.size());

        // Read once for the whole walk rather than per candidate tile:
        // nothing about the walker changes as the BFS spreads, and both
        // answers cost more than a field read. See `aversions_of`.
        let aversions = if avoid_hazards {
            self.aversions_of(actor_id)
        } else {
            WalkerAversions::NONE
        };

        let in_melee =
            |c: Coordinate| -> bool { footprint_chebyshev(c, my_size, t_loc, t_size) <= 1 };

        if in_melee(start) {
            return None;
        }

        let mut parent: HashMap<Coordinate, Coordinate> = HashMap::new();
        let mut queue: VecDeque<Coordinate> = VecDeque::new();
        queue.push_back(start);
        parent.insert(start, start);

        while let Some(coord) = queue.pop_front() {
            if coord != start && in_melee(coord) {
                let mut cur = coord;
                while parent[&cur] != start {
                    cur = parent[&cur];
                }
                return Some(cur);
            }
            for (dx, dy) in Self::steps_toward(coord, t_loc) {
                let next = Coordinate::new(coord.x + dx, coord.y + dy);
                if parent.contains_key(&next) {
                    continue;
                }
                if !self.can_move_to(actor_id, next) {
                    continue;
                }
                if avoid_hazards && self.tile_is_bad_ground(next, aversions) {
                    continue;
                }
                parent.insert(next, coord);
                queue.push_back(next);
            }
        }
        None
    }

    /// Min movement cost to walk from the actor's current location to `dest`,
    /// bounded by their remaining movement. Returns `None` if `dest` is
    /// unreachable on floor tiles within budget. 8-connected; cardinal steps
    /// cost 5ft, diagonal steps cost ~7.07ft (Euclidean).
    pub fn path_cost_to(&self, actor_id: usize, dest: Coordinate) -> Option<f32> {
        self.dijkstra_path(actor_id, dest).map(|(c, _)| c)
    }

    /// Cheapest path (excluding `start`, including `dest`) from the actor's
    /// current location to `dest`, alongside the path cost. Returns `None`
    /// when `dest` is unreachable within the actor's remaining-movement
    /// budget. The path is what the engine iterates per-tile so that
    /// opportunity attacks fire on every threatened-square exit, not just
    /// on the from→to endpoints.
    pub fn path_to(&self, actor_id: usize, dest: Coordinate) -> Option<Vec<Coordinate>> {
        self.dijkstra_path(actor_id, dest).map(|(_, p)| p)
    }

    fn dijkstra_path(
        &self,
        actor_id: usize,
        dest: Coordinate,
    ) -> Option<(f32, Vec<Coordinate>)> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let actor = self.actors.get(&actor_id)?;
        // A mounted rider travels on its mount's legs. Everything
        // geometric about the walk — where it starts, how wide the body
        // is, which tiles it may enter, whether it crawls, which
        // surcharges it ignores — belongs to the mount; only the
        // *budget* stays the rider's, and `start_turn_for` has already
        // filled that from the mount's speed. For an unmounted actor
        // `body` and `actor` are the same creature and this costs a
        // hash lookup.
        let body_id = self.movement_body(actor_id);
        let body = self.actors.get(&body_id)?;
        let start = body.location();
        if start == dest {
            return Some((0.0, Vec::new()));
        }
        if !self.can_move_to(body_id, dest) {
            return None;
        }

        // Encode floats as millifeet so we can use integer ordering / Eq.
        let to_mft = |f: f32| -> u32 { (f * 1000.0) as u32 };
        // 5e: prone creatures crawl at double movement cost per foot.
        let prone_factor: u32 = if body.has_condition(Condition::Prone) { 2 } else { 1 };
        let cardinal_mft = to_mft(TILE_FEET) * prone_factor;
        let diagonal_mft = to_mft(TILE_FEET * std::f32::consts::SQRT_2) * prone_factor;
        let budget_mft = to_mft(actor.remaining_movement() + 0.5);
        // Freedom of Movement, magical flight, and Land's Stride all
        // waive the difficult-terrain surcharge — see the shared
        // `DIFFICULT_TERRAIN_IMMUNITIES` cohort. Resolved once here
        // rather than per candidate step: the answer can't change while
        // a single path is being searched, and the inner loop below runs
        // eight times per expanded tile.
        let ignores_rough = body.ignores_difficult_terrain();
        // …and its water-side twin. 5e charges the same double rate for
        // swimming that it charges for difficult terrain and waives the
        // two with different things — a ranger's Land's Stride is no
        // help in a lake and a shark's swimming speed is no help in
        // rubble — so the waiver the inner loop applies depends on which
        // kind of tile the step lands on. Resolved once out here for the
        // same reason `ignores_rough` is: neither answer can change
        // while a single path is being searched.
        let swims = body.swims_freely();

        let start_idx = self.idx(start).ok()?;
        let dest_idx = self.idx(dest).ok()?;
        let n = self.width * self.height;
        let mut dist: Vec<u32> = vec![u32::MAX; n];
        let mut parent: Vec<Option<usize>> = vec![None; n];
        dist[start_idx] = 0;

        let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();
        heap.push(Reverse((0, start_idx)));

        while let Some(Reverse((cost, idx))) = heap.pop() {
            if idx == dest_idx {
                // Reconstruct path from start (exclusive) to dest (inclusive).
                let mut rev: Vec<Coordinate> = Vec::new();
                let mut cur = dest_idx;
                while cur != start_idx {
                    rev.push(Coordinate::new(
                        (cur % self.width) as isize,
                        (cur / self.width) as isize,
                    ));
                    cur = parent[cur]?;
                }
                rev.reverse();
                return Some((cost as f32 / 1000.0, rev));
            }
            if cost > dist[idx] {
                continue;
            }
            let cx = (idx % self.width) as isize;
            let cy = (idx / self.width) as isize;
            for dy in -1..=1isize {
                for dx in -1..=1isize {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let next = Coordinate::new(cx + dx, cy + dy);
                    if !self.can_move_to(body_id, next) {
                        continue;
                    }
                    let base_step = if dx == 0 || dy == 0 {
                        cardinal_mft
                    } else {
                        diagonal_mft
                    };
                    // The terrain layer and the zone layer each answer
                    // "is this tile difficult", and a tile that is
                    // difficult for both reasons — a web laid over
                    // rubble — is still just difficult. `max`, not
                    // product: 5e's difficult terrain is a property of
                    // the space, not a counter that stacks.
                    //
                    // Which waiver applies is keyed off the tile: water
                    // charges the swimming surcharge and everything else
                    // charges the difficult-terrain one. A shark's
                    // swimming speed makes the lake free and does
                    // nothing about the rubble on its shore; a ranger's
                    // Land's Stride is the exact mirror.
                    //
                    // Either waiver cancels the *terrain* surcharge and
                    // not the *zone* one, which is a change: the older
                    // shape returned a flat 1.0 the moment
                    // `ignores_rough` held, so a creature under Freedom
                    // of Movement crossed a Web or an Entangle for free
                    // as well. That was never the rule — RAW's Land's
                    // Stride is "nonmagical difficult terrain", and even
                    // Freedom of Movement's broader clause is about the
                    // terrain rather than about a spell holding you —
                    // and it was invisible while the two surcharges were
                    // waived by the same predicate. Splitting the lane
                    // put both halves of the expression on screen at
                    // once, which is where it showed.
                    let tile = self.terrain_at(next).map(|t| t.terrain_type);
                    let waived = if tile.is_some_and(|t| t.is_water()) {
                        swims
                    } else {
                        ignores_rough
                    };
                    let terrain_mult = if waived {
                        self.zone_movement_multiplier(next)
                    } else {
                        tile.map(|t| t.movement_cost())
                            .unwrap_or(1.0)
                            .max(self.zone_movement_multiplier(next))
                    };
                    let step = (base_step as f32 * terrain_mult) as u32;
                    let next_cost = cost.saturating_add(step);
                    if next_cost > budget_mft {
                        continue;
                    }
                    let Ok(next_idx) = self.idx(next) else {
                        continue;
                    };
                    if next_cost < dist[next_idx] {
                        dist[next_idx] = next_cost;
                        parent[next_idx] = Some(idx);
                        heap.push(Reverse((next_cost, next_idx)));
                    }
                }
            }
        }
        None
    }

    pub fn from_params(
        terrain_params: &TerrainGenParams,
        actor_params: &ActorGenParams,
        seed: Option<u64>,
    ) -> Result<EncounterInstance, Box<dyn Error>> {
        let mut ei = Self::empty(terrain_params, seed);
        generate_actors(&mut ei, actor_params, &Self::template_pool())?;
        ei.initialize()?;
        Ok(ei)
    }

    /// Build the next encounter in a multi-encounter game loop. Carries
    /// over `pcs` (already long-rested by the caller) onto a freshly
    /// generated terrain at random spawn locations, then fills teams
    /// 1..n_teams with random enemies via `generate_actors` (with
    /// `start_team = 1` so team 0 isn't randomized over the placed PCs).
    pub fn with_pcs(
        terrain_params: &TerrainGenParams,
        actor_params: &ActorGenParams,
        seed: Option<u64>,
        pcs: Vec<ActorInstance>,
    ) -> Result<EncounterInstance, Box<dyn Error>> {
        let mut ei = Self::empty(terrain_params, seed);

        // Place each PC at a random spawn location on the new map. Their
        // internal location field is updated to match.
        for mut pc in pcs {
            let location = ei.get_random_spawn(pc.size())?;
            let actor_id = ei.next_actor_id();
            // Ids are handed out fresh for the new board, so any
            // rider/mount or attach link a survivor arrives holding
            // names an actor in the fight that just ended — at best
            // nobody, at worst whoever inherits that number. Cut before
            // they are seated; `set_actor_map` below stamps every one
            // of them onto the grid in their own right.
            pc.clear_body_links();
            pc.set_location(location);
            ei.actors.insert(actor_id, pc);
            ei.set_actor_map(actor_id, location)?;
        }

        // Force `start_team = 1` so generate_actors never re-rolls team 0.
        let mut enemy_params = actor_params.clone();
        enemy_params.start_team = 1;
        enemy_params.pc_template = None;
        generate_actors(&mut ei, &enemy_params, &Self::template_pool())?;
        ei.initialize()?;
        Ok(ei)
    }

    /// Build the bare encounter shell: terrain generated, no actors, no
    /// initiative.
    ///
    /// `seed` of `None` means "pick one," not "run unseeded." Drawing a
    /// seed from process entropy and then using it exactly the way an
    /// explicit one is used costs a single `u64` and buys the property
    /// that *every* encounter is reproducible — `seed()` reports it, the
    /// UI prints it, and a fight worth replaying (or a bug worth
    /// reporting) can be re-entered by passing the number back on the
    /// command line. Before this, an unseeded run took its dice from a
    /// thread-local RNG whose starting state was gone the moment it was
    /// used, so the one encounter anybody actually wanted to reproduce —
    /// the one they just played — was the one that couldn't be.
    fn empty(terrain_params: &TerrainGenParams, seed: Option<u64>) -> EncounterInstance {
        let seed = seed.unwrap_or_else(|| fastrand::u64(..));
        let roller = FastRandRoller::with_seed(seed);
        let mut rng = Rng::with_seed(seed);
        EncounterInstance {
            seed,
            redirect_depth: 0,
            initialized: false,
            surprise_resolved: false,
            width: terrain_params.width,
            height: terrain_params.height,
            round: 1,
            lair_acted_round: None,
            lowest_active_hitpoints: None,
            last_attrition_progress_round: 1,
            pending_concentration_review: Vec::new(),
            terrain: generate_terrain(terrain_params, &mut rng),
            actor_id_next: 0,
            actor_map: vec![None; terrain_params.width * terrain_params.height],
            actors: BTreeMap::new(),
            items_on_ground: HashMap::new(),
            initiative_tracker: InitiativeTracker::new(),
            encounter_stack: Vec::new(),
            roller,
            rng,
            messages: Vec::new(),
            outcome_tracker: OutcomeTracker::new(),
            multiattack_depth: 0,
            cast_stack: Vec::new(),
            turn_started_for: None,
            zones: Vec::new(),
            zone_id_next: 0,
            zone_contacts_this_turn: std::collections::HashSet::new(),
            conjured_terrain: Vec::new(),
            conjured_terrain_id_next: 0,
            ambient_light: AmbientLight::default(),
            light_sources: Vec::new(),
            light_source_id_next: 0,
        }
    }

    /// True if attack resolution is currently nested inside a Multiattack
    /// / CompoundAttack expansion. SimpleWeapon-shaped attacks read this
    /// to decide whether to fire their Extra Attack rider: bare invocations
    /// (depth 0) chain into a second swing for creatures with
    /// `has_extra_attack: true`; calls from within a Multi (depth > 0)
    /// suppress the rider so the Multi's count isn't accidentally doubled.
    pub fn in_multiattack(&self) -> bool {
        self.multiattack_depth > 0
    }

    /// Increment / decrement the multi-attack nesting counter. Multiattack
    /// / CompoundAttack call `enter_multiattack` before delegating to a
    /// sub-attack and `exit_multiattack` after — symmetric guard pattern
    /// so a panic inside a sub-attack still leaves a well-formed counter
    /// when the test harness moves on. The counter is `u32` (not `bool`)
    /// so a hypothetical Multi-of-Multis doesn't deadlock the gate.
    pub fn enter_multiattack(&mut self) {
        self.multiattack_depth += 1;
    }

    pub fn exit_multiattack(&mut self) {
        if self.multiattack_depth > 0 {
            self.multiattack_depth -= 1;
        }
    }

    /// Push the cast frame for the spell whose effects are about to be
    /// built. Paired with `exit_cast` by `Action::execute` around the
    /// `side_effects` call — the symmetric-guard shape
    /// `enter_multiattack` / `exit_multiattack` already use.
    /// `damage_types` is `Action::damage_types()` packed into a mask —
    /// see `CastContext::damage_types` for why the frame carries it.
    /// Pass `DamageTypeSet::EMPTY` for a frame whose damage typing is
    /// irrelevant to what the caller is testing.
    pub fn enter_cast(
        &mut self,
        school: Option<SpellSchool>,
        level: u32,
        damage_types: crate::engine::types::DamageTypeSet,
    ) {
        self.cast_stack.push(CastContext {
            school,
            level,
            flat_damage_bonus_paid: false,
            damage_types,
        });
    }

    /// Pop the innermost cast frame. Tolerates an empty stack so a panic
    /// inside a spell's `side_effects` builder still leaves a well-formed
    /// stack when the test harness moves on — same defensive shape as
    /// `exit_multiattack`.
    pub fn exit_cast(&mut self) {
        self.cast_stack.pop();
    }

    /// The spell currently being resolved, or `None` outside any cast
    /// (a weapon swing, a class feature, an item use, or a side-effect
    /// applied after `execute` already returned).
    ///
    /// This is the read side of the cast stack: it lets a resolution
    /// site deep inside a spell — the shared burst save loop, the shared
    /// damage roll, the ally-shield sweep — ask "what school and level
    /// am I part of?" without threading the answer through every helper
    /// signature between here and `Action::execute`.
    pub fn current_cast(&self) -> Option<CastContext> {
        self.cast_stack.last().copied()
    }

    /// Whether the innermost in-flight cast is a spell of `school` at
    /// 1st level or higher. Convenience for the common gate shape;
    /// `false` outside any cast, on a cantrip, and on a different
    /// school — so a feature reading this fails closed everywhere it
    /// shouldn't fire.
    pub fn casting_leveled_spell_of(&self, school: SpellSchool) -> bool {
        self.current_cast()
            .is_some_and(|c| c.is_leveled_spell_of(school))
    }

    /// Every creature template the encounter generator can roll — the
    /// bestiary half of the engine's answer to "what exists?", with
    /// `creatures::pc_template_families` as the playable half. Between
    /// them they cover every template that can reach a battlefield.
    ///
    /// `pub` so cross-module sweeps can read it. `class_features`'s
    /// per-rest registry check needs both halves: a feature tag is only
    /// an orphan if *nothing* instantiable carries it, and half the tags
    /// in the registry are carried by monsters.
    pub fn template_pool() -> Vec<&'static CreatureTemplate> {
        let mut pool: Vec<&'static CreatureTemplate> = vec![
            &ANKHEG_TEMPLATE,
            &ANIMATED_ARMOR_TEMPLATE,
            &FLYING_SWORD_TEMPLATE,
            &BANDIT_TEMPLATE,
            &BANDIT_CAPTAIN_TEMPLATE,
            &BANSHEE_TEMPLATE,
            &BASILISK_TEMPLATE,
            &BERSERKER_TEMPLATE,
            &BUGBEAR_TEMPLATE,
            &BUGBEAR_STALKER_TEMPLATE,
            &CHIMERA_TEMPLATE,
            &CHUUL_TEMPLATE,
            &CLERIC_TEMPLATE,
            &CLOAKER_TEMPLATE,
            &COCKATRICE_TEMPLATE,
            &CULT_FANATIC_TEMPLATE,
            &DIRE_WOLF_TEMPLATE,
            &DISPLACER_BEAST_TEMPLATE,
            &DOPPELGANGER_TEMPLATE,
            &ETTIN_TEMPLATE,
            &FIRE_ELEMENTAL_TEMPLATE,
            &GARGOYLE_TEMPLATE,
            &GELATINOUS_CUBE_TEMPLATE,
            &GHOST_TEMPLATE,
            &GHOUL_TEMPLATE,
            &GIANT_SCORPION_TEMPLATE,
            &GNOLL_TEMPLATE,
            &GRICK_TEMPLATE,
            &GOBLIN_TEMPLATE,
            &GOBLIN_MINION_TEMPLATE,
            &GOBLIN_BOSS_TEMPLATE,
            &HARPY_TEMPLATE,
            &HELL_HOUND_TEMPLATE,
            &HILL_GIANT_TEMPLATE,
            &HIPPOGRIFF_TEMPLATE,
            &HOBGOBLIN_TEMPLATE,
            &HOBGOBLIN_CAPTAIN_TEMPLATE,
            &HOBGOBLIN_WARLORD_TEMPLATE,
            &HYDRA_TEMPLATE,
            &KOBOLD_TEMPLATE,
            &KNIGHT_TEMPLATE,
            &MAGE_TEMPLATE,
            &MANTICORE_TEMPLATE,
            &MEDUSA_TEMPLATE,
            &MIND_FLAYER_TEMPLATE,
            &MIMIC_TEMPLATE,
            &MINOTAUR_TEMPLATE,
            &MUMMY_TEMPLATE,
            &NIGHTMARE_TEMPLATE,
            &NOTHIC_TEMPLATE,
            &OGRE_TEMPLATE,
            &ORC_TEMPLATE,
            &OWLBEAR_TEMPLATE,
            &PHASE_SPIDER_TEMPLATE,
            &SALAMANDER_TEMPLATE,
            &ROPER_TEMPLATE,
            &RUST_MONSTER_TEMPLATE,
            &SHADOW_TEMPLATE,
            &SHAMBLING_MOUND_TEMPLATE,
            &SPECTER_TEMPLATE,
            &GIANT_SPIDER_TEMPLATE,
            &STIRGE_TEMPLATE,
            &STONE_GIANT_TEMPLATE,
            &STORM_GIANT_TEMPLATE,
            &TREANT_TEMPLATE,
            &TROLL_TEMPLATE,
            &TROLL_LIMB_TEMPLATE,
            &UMBER_HULK_TEMPLATE,
            &VAMPIRE_SPAWN_TEMPLATE,
            &VETERAN_TEMPLATE,
            &WARRIOR_INFANTRY_TEMPLATE,
            &VROCK_TEMPLATE,
            &WEREWOLF_TEMPLATE,
            &WIGHT_TEMPLATE,
            &WISP_TEMPLATE,
            &WOLF_TEMPLATE,
            &WORG_TEMPLATE,
            &WIZARD_TEMPLATE,
            &WYVERN_TEMPLATE,
            &YETI_TEMPLATE,
            &DROW_TEMPLATE,
            &GNOLL_PACK_LORD_TEMPLATE,
            &BONE_DEVIL_TEMPLATE,
            &ERINYES_TEMPLATE,
            &FROST_GIANT_TEMPLATE,
            &EARTH_ELEMENTAL_TEMPLATE,
            &AIR_ELEMENTAL_TEMPLATE,
            &BULETTE_TEMPLATE,
            &FLAMESKULL_TEMPLATE,
            &SPECTATOR_TEMPLATE,
            &WRAITH_TEMPLATE,
            &VAMPIRE_TEMPLATE,
            &VAMPIRE_FAMILIAR_TEMPLATE,
            // Low-CR undead / fiend / ooze staples. These templates have
            // existed for a while but were never added to the pool, so
            // the random encounter generator could never roll them — a
            // single dungeon room couldn't surface a humble skeleton-
            // archer ambush or an acid slime puddle. Adding them here
            // restores the "common monster" fallback for the cr_target
            // ≈ 1 default the main loop spawns at.
            &SKELETON_TEMPLATE,
            &MINOTAUR_SKELETON_TEMPLATE,
            &ZOMBIE_TEMPLATE,
            &OGRE_ZOMBIE_TEMPLATE,
            &SLIME_TEMPLATE,
            &IMP_TEMPLATE,
            &FIRE_IMP_TEMPLATE,
            // Mid-CR celestial — 5e Couatl (CR 4). Pairs with the rest
            // of the mid-tier extraplanar entries (Bone Devil, Erinyes,
            // Vrock) so a higher cr_target run has a good-aligned
            // outsider in the pool too.
            &COUATL_TEMPLATE,
            // Mid / low-CR fill-ins added alongside the new templates:
            // Giant Eagle (CR 1 large beast — aerial), Sahuagin (CR ½
            // humanoid w/ Blood Frenzy), Lizardfolk (CR ½ humanoid, sturdy
            // melee), Giant Ape (CR 7 huge beast — fills the gap between
            // Stone Giant and Frost Giant in the upper-mid pool), Centaur
            // (CR 2 hybrid monstrosity — pike + hooves multi, longbow
            // fallback).
            &GIANT_EAGLE_TEMPLATE,
            &SAHUAGIN_TEMPLATE,
            &LIZARDFOLK_TEMPLATE,
            &GIANT_APE_TEMPLATE,
            &CENTAUR_TEMPLATE,
            // Low-CR beast fillers — common druid Conjure Animals
            // picks and standard wilderness encounter material. Boar
            // (CR ¼), Brown Bear / Tiger / Giant Toad (CR 1), plus
            // the Tiny Pseudodragon (CR ¼ dragon, magic resistance).
            // Behir (CR 11 huge monstrosity) plugs the lightning-
            // immune slot above the giants and below the ancient
            // dragons in the upper-mid pool.
            &BOAR_TEMPLATE,
            &BROWN_BEAR_TEMPLATE,
            &TIGER_TEMPLATE,
            &GIANT_TOAD_TEMPLATE,
            &PSEUDODRAGON_TEMPLATE,
            &BEHIR_TEMPLATE,
            // Beast / giant fill-ins added alongside the new templates:
            // Polar Bear (CR 2, large beast — heavier sibling of Brown
            // Bear), Lion (CR 1, large beast with Pack Tactics — pride
            // hunter counterpart to Tiger), Fire Giant (CR 9, huge
            // giant — fire-immune sibling between Frost Giant and
            // Storm Giant in the giant ladder), Cyclops (CR 6, huge
            // giant — one-eyed brute slotting between Hill Giant and
            // Stone Giant), Roc (CR 11, huge beast — gargantuan eagle
            // pairing with Behir as a non-dragon upper-mid threat).
            &POLAR_BEAR_TEMPLATE,
            &LION_TEMPLATE,
            &FIRE_GIANT_TEMPLATE,
            &CYCLOPS_TEMPLATE,
            &ROC_TEMPLATE,
            // Mid-tier fill-ins added alongside the new dinosaur /
            // celestial / arctic-predator templates: Pegasus (CR 2,
            // large celestial — winged horse, single hooves swing),
            // Winter Wolf (CR 3, large monstrosity — cold-immune
            // sibling of Wolf with a cold breath weapon), Carrion
            // Crawler (CR 2, large monstrosity — paralyzing tentacles
            // + bite multi, ceiling-dweller dungeon staple),
            // Triceratops (CR 5, huge beast — dinosaur option in the
            // upper-mid melee pool), Tyrannosaurus Rex (CR 8, huge
            // beast — apex predator with bite + tail multi).
            &PEGASUS_TEMPLATE,
            &WINTER_WOLF_TEMPLATE,
            &CARRION_CRAWLER_TEMPLATE,
            &TRICERATOPS_TEMPLATE,
            &T_REX_TEMPLATE,
            // Newest additions rounding out the elemental quartet and
            // the low-to-mid pack-tactics ladder: Water Elemental (CR 5,
            // completing the four primordial flavors with the signature
            // `WHELM` burst), Saber-toothed Tiger (CR 2, heavier Pounce
            // cousin of Tiger), Hyena (CR 0, cheapest pack-tactics
            // biter), Giant Hyena (CR 1, large pack-tactics biter), and
            // Green Hag (CR 3, fey saver with Magic Resistance — the
            // first medium-CR fey-typed monster in the pool).
            &WATER_ELEMENTAL_TEMPLATE,
            &SABER_TOOTHED_TIGER_TEMPLATE,
            &HYENA_TEMPLATE,
            &GIANT_HYENA_TEMPLATE,
            &GREEN_HAG_TEMPLATE,
            // Newest additions filling the cr-1 to cr-5 gap in the
            // monstrosity / fiend / fey lanes:
            //   - Gorgon (CR 5 large monstrosity): petrifying-breath
            //     cone with the shared `"breath_weapon"` recharge key.
            //   - Yuan-Ti Malison (CR 3 fiend hybrid): poison-immune
            //     scimitar+bite multi with magic resistance.
            //   - Cambion (CR 5 fiend half-devil): fire-rider spear,
            //     ranged fire ray, mid-tier resistance envelope.
            //   - Dryad (CR 1 fey): WIS-DC fey charm + magic resistance
            //     + fey ancestry — first single-target fey charmer in
            //     the pool at the low CR tier.
            //   - Bullywug (CR ¼ humanoid): low-CR amphibian pack
            //     fodder with a spear + bite compound multi.
            &GORGON_TEMPLATE,
            &YUAN_TI_MALISON_TEMPLATE,
            &CAMBION_TEMPLATE,
            &DRYAD_TEMPLATE,
            &BULLYWUG_TEMPLATE,
            // Newest additions filling out the fiend / aberration /
            // elemental lanes:
            //   - Quasit (CR 1 tiny fiend): chaotic-evil mirror of the
            //     Imp — poisoned claws + a one-target Scare.
            //   - Shadow Demon (CR 4 medium fiend): psychic claws with
            //     wide elemental resistance and a radiant-vulnerability
            //     hook (the engine's first vulnerability holder).
            //   - Succubus (CR 4 medium fiend): seduction kit — ranged
            //     charm + draining-kiss combo gated on the Charmed
            //     condition + claws fallback.
            //   - Intellect Devourer (CR 2 tiny aberration): rare
            //     INT-save lane (`Devour Intellect`) with a damage-
            //     threshold stun rider; fills the niche between Mind
            //     Flayer and Nothic.
            //   - Xorn (CR 5 medium elemental): 3-claw + bite heavy
            //     multiattack with tremorsense and the standard
            //     elemental B/P/S resistance + Poisoned/Paralyzed/
            //     Petrified/Unconscious condition-immunity envelope.
            &QUASIT_TEMPLATE,
            &SHADOW_DEMON_TEMPLATE,
            &SUCCUBUS_TEMPLATE,
            &INCUBUS_TEMPLATE,
            &INTELLECT_DEVOURER_TEMPLATE,
            &XORN_TEMPLATE,
            // Newest additions filling the giant / aquatic-humanoid /
            // ambient-beast lanes:
            //   - Oni (CR 7 large giant): polearm multi with reach-2,
            //     Magic Resistance, and 10/round regeneration. Slots
            //     above Hill Giant / Cyclops and below Fire Giant in
            //     the giant ladder.
            //   - Merrow (CR 2 large humanoid): aquatic raider with
            //     a harpoon + bite multi. Sits next to Sahuagin and
            //     Lizardfolk on the medium-CR humanoid bench but on the
            //     Large frame for a heavier hit profile.
            //   - Giant Crab (CR ⅛ medium beast): cheapest ambient
            //     creature in the pool — single claw pinch, joins the
            //     low-end fillers (Stirge, Hyena, Boar) at the bottom
            //     of the CR ladder.
            &ONI_TEMPLATE,
            &MERROW_TEMPLATE,
            &GIANT_CRAB_TEMPLATE,
            // Newest additions filling the upper-giant / mid-demon /
            // CR-2-aberration lanes:
            //   - Cloud Giant (CR 9 huge giant): morningstar + rock + Multi
            //     completing the giant ladder between Fire Giant (CR 9)
            //     and Storm Giant (CR 13). Same 3d8 melee die as the Stone
            //     / Fire / Cyclops chassis but heavier STR-based dice.
            //   - Hezrou (CR 8 large demon): bite + 2 claws Compound
            //     multiattack, Magic Resistance, and the standard
            //     non-magical-BPS / cold / fire / lightning resistance
            //     envelope. Slots above Vrock (CR 6) and below Glabrezu
            //     (CR 9) on the demon hierarchy.
            //   - Gibbering Mouther (CR 2 medium aberration): heavy 5d6
            //     bites swing plus a recharge-5/6 Blinding Spittle bonus
            //     action that DC-10 DEX-saves Blinded on a 1-tile burst
            //     up to 30 ft. Fills the niche between Stirge (CR ⅛) and
            //     Nothic (CR 2) on the low-CR aberration bench.
            &CLOUD_GIANT_TEMPLATE,
            &HEZROU_TEMPLATE,
            &GIBBERING_MOUTHER_TEMPLATE,
            // Newest additions filling the boss-tier slot of the random
            // encounter pool — three high-CR signature monsters that
            // gate a "magic doesn't work on me" anti-caster envelope:
            //   - Mummy Lord (CR 15 undead): boss sibling of Mummy with
            //     a 6d6-necrotic Rotting Fist, 60-ft DC-17 Dreadful Glare,
            //     and the Magic Resistance + 3 Legendary Resistance combo.
            //   - Iron Golem (CR 16 construct): pinnacle of the golem
            //     ladder — 3d10 reach-2 Iron Sword, 3d8 Iron Slam,
            //     10d8-poison breath (recharge 6), with the full
            //     construct condition immunity envelope plus fire / poison
            //     / psychic damage immunity.
            //   - Rakshasa (CR 13 fiend): tiger-headed shape-shifter with
            //     a 2×(2d6+2d10-necrotic) claw multi and Magic Resistance
            //     standing in for RAW's Limited Magic Immunity.
            &MUMMY_LORD_TEMPLATE,
            &IRON_GOLEM_TEMPLATE,
            &RAKSHASA_TEMPLATE,
            // Newest additions filling the low-CR brute, the aquatic
            // boss-dragon, and the apex aquatic-titan slots:
            //   - Hook Horror (CR 3 large monstrosity): underdark
            //     vulture-pincer predator, vanilla 2-hook multi at reach 2.
            //     Fills the brute-melee bench between Owlbear (CR 3) and
            //     Hill Giant (CR 5) at the low-mid tier.
            //   - Dragon Turtle (CR 17 gargantuan dragon): the aquatic
            //     answer to Adult Red Dragon — bite + 2-claw multi plus a
            //     12d6 fire-typed steam breath (CON save) on the standard
            //     recharge-5/6 chassis.
            //   - Kraken (CR 23 gargantuan titan-monstrosity): triple
            //     tentacle multi at reach 6 (the engine's longest melee
            //     reach) plus a Lightning Storm save-burst recharge.
            //     Lightning immune, fear / paralysis immune, Magic
            //     Resistance + 3 Legendary Resistances + 3 Legendary
            //     Actions — the canonical aquatic apex boss.
            &HOOK_HORROR_TEMPLATE,
            &DRAGON_TURTLE_TEMPLATE,
            &KRAKEN_TEMPLATE,
            // Newest additions filling the construct / fey / boss-tier
            // lanes:
            //   - Helmed Horror (CR 4 medium construct): plate-armored
            //     spell-immune sentry; 2-longsword multi plus Force /
            //     Necrotic / Poison damage immunity and the full
            //     construct condition-immunity envelope. Sits between
            //     Animated Armor (CR 1) and Stone Golem (CR 10) on the
            //     construct ladder.
            //   - Pixie (CR ¼ tiny fey): 1-HP glass-cannon controller —
            //     Magic Resistance + Fey Ancestry + a 5ft Sleep Dust
            //     burst at 30ft range (DC 12 WIS, Asleep on fail). Fills
            //     the lowest fey CR slot below Dryad (CR 1).
            //   - Androsphinx (CR 17 large monstrosity): wisdom-guardian
            //     boss with a 2-claw multi, recharge-gated 50ft DC-18 WIS
            //     Roar (Frightened on fail), nonmagical-BPS resistance,
            //     Charmed / Frightened condition immunity, 3 Legendary
            //     Resistance + Magic Resistance + 3 legendary actions.
            //     The non-dragon CR-17 boss option.
            &HELMED_HORROR_TEMPLATE,
            &PIXIE_TEMPLATE,
            &ANDROSPHINX_TEMPLATE,
            &SPHINX_OF_LORE_TEMPLATE,
            &GUARDIAN_NAGA_TEMPLATE,
            // Newest additions filling the mid-CR celestial / mid-CR
            // monstrosity / low-CR fey lanes:
            //   - Unicorn (CR 5 large celestial): magic-resistant healer
            //     with heterogeneous hooves+horn multi and a recharge-
            //     gated 3d8+CHA single-target ally heal. The only mid-
            //     CR celestial in the pool until Solar opens at CR 21.
            //   - Drider (CR 6 large monstrosity): drow-spider hybrid
            //     with a 3-swing multi (2 longsword + 1 bite) plus a
            //     standalone longbow lane. Bite carries a CON 13 / 4d8
            //     poison rider (half on save). Fey Ancestry covers the
            //     drow heritage's Charm / Asleep immunity.
            //   - Sea Hag (CR 2 medium fey): low-CR glass-cannon
            //     controller — claws + DC 11 Death Glare (12-tile WIS
            //     save, 6d6 psychic on fail). Slots between Dryad (CR 1)
            //     and Green Hag (CR 3) on the fey ladder.
            &UNICORN_TEMPLATE,
            &DRIDER_TEMPLATE,
            &SEA_HAG_TEMPLATE,
            // Newest additions filling the CR-5 fiend / CR-8 caster /
            // CR-5 aberration lanes:
            //   - Night Hag (CR 5 medium fiend): apex of the hag trio
            //     (Sea Hag CR 2, Green Hag CR 3, Night Hag CR 5) —
            //     2-claw multi plus Magic Resistance, non-magical-BPS /
            //     cold / fire resistance, and Charmed condition immunity.
            //   - Spirit Naga (CR 8 large monstrosity): snake-bodied
            //     caster with a reach-2 bite (1d6+STR + 7d8 poison save
            //     half) plus a Sleep / Charm Person / Hold Person /
            //     Lightning Bolt + Sacred Flame spell slate. Slots
            //     between Drider (CR 6) and Cloud Giant (CR 9).
            //   - Otyugh (CR 5 large aberration): garbage-eating tentacle
            //     horror — 1 bite + 2 tentacles compound multi, with
            //     reach-2 tentacles that grapple via the Restrained
            //     condition envelope. The brawler-grappler answer to the
            //     Night Hag's caster-flavored CR 5 slot.
            &NIGHT_HAG_TEMPLATE,
            &SPIRIT_NAGA_TEMPLATE,
            &OTYUGH_TEMPLATE,
            // Newest additions filling the CR-¼ scout / CR-1 monstrosity /
            // CR-½ elemental / CR-6 elemental-caster-disruptor lanes:
            //   - Sprite (CR ¼ tiny fey): archer-scout variant of the
            //     CR-¼ fey lane — sleep-arrow rider on a longbow at
            //     reach 8 (40 ft) plus a flat-1 shortsword fallback in
            //     melee. The other CR-¼ fey (Pixie) leans on at-will
            //     area sleep dust; the sprite covers the single-target
            //     ranged sleep opener at the same price.
            //   - Death Dog (CR 1 medium monstrosity): the two-headed
            //     underdark cur — 2-bite multiattack, each head rolling
            //     an independent disease-save rider (CON 12 or Poisoned
            //     for 10 rounds, proxy for RAW's "diseased until cured").
            //   - Magmin (CR ½ small elemental): the lava-imp of the
            //     Plane of Fire — touch attack with a Burning DOT
            //     install (3 rounds), full elemental envelope (fire +
            //     poison immunity, BPS resistance, condition envelope
            //     via the shared `ELEMENTAL_CONDITION_IMMUNITIES`).
            //   - Galeb Duhr (CR 6 medium elemental): the granite
            //     guardian — 2-slam multi (3d8+STR per swing) plus
            //     Magic Resistance on top of the standard elemental
            //     envelope. The caster-disruption stone-cousin of the
            //     CR-5 Earth Elemental, slotting between Otyugh (CR 5)
            //     and Drider (CR 6).
            &SPRITE_TEMPLATE,
            &DEATH_DOG_TEMPLATE,
            &MAGMIN_TEMPLATE,
            &GALEB_DUHR_TEMPLATE,
            // Newest additions filling the CR-2 flying-predator / CR-4
            // monstrosity-charmer / CR-5 lycanthrope-boss lanes:
            //   - Griffon (CR 2 large monstrosity): the classic eagle-lion
            //     hybrid, beak+talons heterogeneous compound multi
            //     (1d8+STR piercing + 2d6+STR slashing) for ~19.5 average
            //     per Action against a single target. Sits between
            //     Hippogriff (CR 1) and Manticore (CR 3) on the flying-
            //     beast ladder.
            //   - Lamia (CR 4 large monstrosity): the desert temptress —
            //     2d10+STR claws + a save-or-Charmed intoxicating touch
            //     curse compound multi. The single-target charm-lockout
            //     answer to the Sea Hag's psychic-damage glare at the
            //     mid-CR tier, with the same `SetConditionLink(Condition::Charmed)` charmer-link
            //     so the cursed PC can't take hostile actions back at the
            //     lamia.
            //   - Werebear (CR 5 large humanoid lycanthrope): apex of the
            //     lycanthrope family — bite + claws heterogeneous compound
            //     multi (1d10+STR piercing + 2d8+STR slashing), with the
            //     bite carrying the CON 14 lycanthropy save rider
            //     (Poisoned 3 rounds proxy for RAW's "curse of werebear
            //     lycanthropy"). Non-magical BPS resistance via the
            //     shared `non_magical_physical_resistances` helper — same
            //     defensive envelope as the Werewolf (CR 3) but on a
            //     135-HP frame with the chunkier compound multi.
            &GRIFFON_TEMPLATE,
            &LAMIA_TEMPLATE,
            &WEREBEAR_TEMPLATE,
            // Newest additions completing the lycanthrope family and
            // adding a spider-humanoid trapper:
            //   - Wereboar (CR 4 medium lycanthrope): tusks + maul
            //     compound multi, DC-12 lycanthropy curse on the
            //     tusks. Shares the CR-4 slot with Lamia / Weretiger
            //     but with a heavier melee profile (2d6 dice vs 1d10).
            //   - Wererat (CR 2 medium lycanthrope): bite + finesse
            //     shortsword multi, DC-11 lycanthropy curse on the
            //     bite. The smallest and sneakiest wereXX — slots
            //     next to Sea Hag / Polar Bear on the CR-2 bench.
            //   - Weretiger (CR 4 large lycanthrope): bite + claws
            //     multi, DC-13 lycanthropy curse on the bite. The
            //     agile/predatory variant to the wereboar's brute
            //     melee at the same CR.
            //   - Ettercap (CR 2 medium monstrosity): bite (with venom
            //     rider) + claws compound multi plus a 30-ft DEX-save
            //     web action that lands Restrained on fail. First
            //     ranged-restraint creature in the pool.
            &WEREBOAR_TEMPLATE,
            &WERERAT_TEMPLATE,
            &WERETIGER_TEMPLATE,
            &ETTERCAP_TEMPLATE,
            // Latest additions filling out the lower-tier fiend bench
            // and a new fey + plant pick:
            //   - Awakened Tree (CR 2 huge plant): double-slam Multi at
            //     reach 10ft, fire-vulnerable / BP-resistant. The
            //     "little cousin of the treant" silhouette at a much
            //     lower CR.
            //   - Dretch (CR ¼ small fiend, demon-tier): bite + claws
            //     Compound multi plus Recharge-6 Fetid Cloud (10ft
            //     radius DC-11 CON or Poisoned). The lowest-tier demon
            //     in the pool.
            //   - Lemure (CR 0 medium fiend, devil-tier): single-attack
            //     Fist (1d4 bludgeoning), devil damage envelope (fire/
            //     poison immune, cold resistant), Charmed/Frightened/
            //     Poisoned condition immunity. Cheapest fiend in the
            //     pool — fills the swarm-grunt slot.
            //   - Bearded Devil / Barbazu (CR 3 medium fiend): glaive
            //     (reach 10ft) + beard (Poisoned rider on CON-12 save)
            //     Compound multi. Mid-tier devil with magic resistance
            //     and the standard hellish damage envelope.
            //   - Blink Dog (CR ¼ medium fey): bite (1d6+STR piercing)
            //     plus a 40-ft bonus-action Teleport (Recharge 4–6).
            //     First teleport-mobility creature in the pool — phases
            //     in for the bite, then out to reposition.
            &AWAKENED_TREE_TEMPLATE,
            &DRETCH_TEMPLATE,
            &LEMURE_TEMPLATE,
            &BEARDED_DEVIL_TEMPLATE,
            &BLINK_DOG_TEMPLATE,
            // Mephit cohort (CR ¼ – ½ small elementals). First creatures
            // wired through the new `DeathBurst` chassis — when reduced
            // to 0 HP each detonates in a small typed burst before being
            // removed from the map. Pairs with the Magmin (also a
            // death-burst entry on the elemental ladder) and fills the
            // CR-¼–½ small-elemental niche between the Imp / Fire Imp
            // (CR 1, fiend-typed) and the Magmin / Galeb Duhr lane.
            //   - Ice Mephit (CR ½, cold immune / fire vulnerable):
            //     frost breath + 1d8 slashing shard death burst.
            //   - Steam Mephit (CR ¼, fire immune): steam breath + 1d8
            //     fire vapor death burst.
            //   - Magma Mephit (CR ½, fire immune / cold vulnerable):
            //     fire breath + 2d6 fire lava death burst (matches the
            //     Magmin's burst profile on the shorter mephit radius).
            &ICE_MEPHIT_TEMPLATE,
            &STEAM_MEPHIT_TEMPLATE,
            &MAGMA_MEPHIT_TEMPLATE,
            // Dust Mephit (CR ½, no fire / cold vulnerability): grit
            // breath that imposes the Blinded condition instead of
            // dealing damage — the breath chassis with no damage
            // payload at all. Slots alongside the
            // Ice / Magma mephits on the same CR shelf but with the
            // control-flavored breath envelope.
            &DUST_MEPHIT_TEMPLATE,
            // Latest additions filling out the mid-tier monstrosity /
            // construct / boss-fiend lanes with three iconic SRD monsters
            // that were missing from the pool:
            //   - Black Pudding (CR 4 large ooze): the formless tar-
            //     blob terror of the underdark — pseudopod (1d6+STR
            //     bludgeoning + 4d8 acid rider) with the iconic ooze
            //     defensive envelope (acid + cold + lightning + slashing
            //     immunity). Slots between Gelatinous Cube (CR 2) and
            //     Shambling Mound (CR 5) on the formless-horror ladder.
            //   - Flesh Golem (CR 5 medium construct): the patchwork
            //     servitor — 2-slam multi (2d8+STR per swing) plus the
            //     full construct condition envelope and lightning +
            //     poison immunity. Slots between Salamander (CR 5) and
            //     Galeb Duhr (CR 6) at the mid-tier, and below Stone /
            //     Iron Golem on the construct ladder.
            //   - Horned Devil (CR 11 large fiend): the malebranche
            //     lieutenant — 2 forks (reach 10ft) + 1 tail (Infernal
            //     Wound save-or-Poisoned rider) compound multi, plus
            //     ranged Hurled Flame for stand-off pressure. Slots
            //     between Bone Devil (CR 9) and Erinyes (CR 12) on the
            //     devil ladder.
            &BLACK_PUDDING_TEMPLATE,
            &FLESH_GOLEM_TEMPLATE,
            &HORNED_DEVIL_TEMPLATE,
            // Nalfeshnee (CR 13 large demon, Type V): the malformed
            // boar-headed bruiser slotting between Glabrezu (CR 9) and
            // Marilith (CR 16) on the demon ladder. 1 bite + 2 claws
            // compound multi (~50 avg damage per Action) plus the
            // signature Recharge 5–6 Horror Nimbus aoe Frighten install
            // (15-ft burst, DC 15 WIS, 10 rounds). First creature wired
            // through the `resolve_burst_save_condition` chassis with
            // a unique recharge pool key (`"horror_nimbus"`) so a co-
            // located dragon's breath_weapon recharge doesn't share
            // state.
            &NALFESHNEE_TEMPLATE,
            // Newest additions filling the CR-11 genie / CR-¼ + CR-2
            // serpent slots:
            //   - Djinni (CR 11 large elemental, air genie): 3-scimitar
            //     multi with a 1d6 thunder rider on every swing. Magic
            //     Resistance + lightning / thunder resistance on top of
            //     the standard elemental envelope. The non-fiend CR-11
            //     elemental slot — pairs with the Horned Devil (CR 11
            //     fiend) and Nalfeshnee (CR 13 fiend) at the upper-mid
            //     extraplanar bench.
            //   - Efreeti (CR 11 large elemental, fire genie): 2-
            //     scimitar multi at 2d6 base + 2d6 fire rider per swing,
            //     plus a 5d6 ranged Hurl Flame stand-off lane. Fire
            //     immunity + Magic Resistance on top of the elemental
            //     envelope. Sister to the djinni — heavier per-swing,
            //     fewer swings, ranged fire option.
            //   - Constrictor Snake (CR ¼ large beast): bite + 1d8
            //     Constrict (DC 14 STR save-or-Grappled rider, 10
            //     rounds). The first snake-shaped creature in the pool;
            //     fills the low-end ambient-beast lane alongside Boar /
            //     Stirge / Giant Crab.
            //   - Giant Constrictor Snake (CR 2 huge beast): reach-2
            //     2d6 bite (1d4 poison rider) + 2d8 reach-2 Constrict
            //     (DC 16 STR save-or-Grappled). Heavier huge-beast
            //     variant slotting between Polar Bear and Carrion
            //     Crawler on the CR-2 bench.
            &DJINNI_TEMPLATE,
            &EFREETI_TEMPLATE,
            &CONSTRICTOR_SNAKE_TEMPLATE,
            &GIANT_CONSTRICTOR_SNAKE_TEMPLATE,
            // Marid (CR 11 large elemental, water genie): 3-trident
            // multi at 2d6 piercing per swing plus a Recharge 4–6 ranged
            // Water Jet (DC 17 DEX save, 6d6 bludgeoning + 20ft push on
            // fail). Acid immunity + cold resistance + Magic Resistance
            // on top of the standard elemental envelope. Completes the
            // noble genie family — Djinni (air), Efreeti (fire), and
            // now Marid (water) — at the same CR-11 tier.
            &MARID_TEMPLATE,
            // Crocodile family — canonical SRD amphibious predators:
            //   - Crocodile (CR ½ large beast): 1d10+STR piercing bite
            //     with an auto-Grappled rider on hit. The bite IS the
            //     lock-down — no save, just an automatic grapple. Slots
            //     alongside Lizardfolk / Bullywug / Boar on the low-CR
            //     ambush-predator bench.
            //   - Giant Crocodile (CR 5 huge beast): 1 bite + 1 tail
            //     compound multi at reach 2 tiles. Bite is 3d10+STR
            //     piercing with the same auto-grapple rider; tail is
            //     2d8+STR bludgeoning vanilla. Sits between Owlbear
            //     (CR 3) and Werebear (CR 5) on the upper-mid beast
            //     ladder.
            &CROCODILE_TEMPLATE,
            &GIANT_CROCODILE_TEMPLATE,
            // Dao (CR 11 large elemental, earth genie): 2-maul Multi +
            // standalone maul + Recharge-5/6 Stone Snare. Completes the
            // noble genie family with the surly Pasha of the Plane of
            // Earth — slots next to Djinni / Efreeti / Marid in the
            // upper-mid elemental bench.
            //
            // Invisible Stalker (CR 6 large elemental, air-tracker): the
            // canonical "born invisible" air-elemental. Routes through
            // the new `innate_conditions` template lane to install
            // permanent Invisibility at instantiation, so the stalker's
            // first slam already benefits from attacker-side advantage
            // and target-side disadvantage to attacks. Same per-swing
            // dice as the Air Elemental's slam (2d8+STR bludgeoning,
            // doubled in the Multi); the invisibility envelope is what
            // separates the two CR-6/CR-5 elementals at adjacent CR
            // tiers — the stalker punches above its CR via attack-mode
            // advantage rather than larger dice.
            &DAO_TEMPLATE,
            &INVISIBLE_STALKER_TEMPLATE,
            // Purple Worm (CR 15 gargantuan monstrosity): the iconic
            // dungeon devourer — 1 bite + 1 tail stinger compound
            // multiattack, the stinger carrying a DC 19 CON save-or-
            // 7d6-poison rider via the shared `save_or_damage_rider`
            // chassis. The non-dragon, non-undead boss option at CR 15
            // — slots between the Aboleth (CR 10) and the Adult Red
            // Dragon (CR 17) on the upper-tier brute ladder.
            &PURPLE_WORM_TEMPLATE,
            // Mammoth (CR 6 huge beast): ice-age elephant — gore +
            // Recharge-5/6 Trampling Charge (DC 18 STR save-or-Prone)
            // + Prone-gated Stomp. First creature wired through a
            // condition-gated `custom_validate_input` against the
            // target's condition set (rather than the caster's own
            // recharge / resource pool). Slots above the Polar Bear
            // (CR 2) on the arctic-beast ladder and fills the apex
            // huge-beast bench between Cyclops (CR 6 giant) and Roc
            // (CR 11 huge beast).
            &MAMMOTH_TEMPLATE,
            // Aquatic + dino cohort:
            //   - Giant Octopus (CR 1 large beast): reach-3 tentacles
            //     (2d6+STR bludgeoning) with a DC 16 STR save-or-
            //     Restrained rider for 10 rounds. Routes through the
            //     `WeaponWithSaveCondition` chassis (long-reach variant)
            //     alongside the Giant Constrictor Snake. The reach-3
            //     standoff + Restrained envelope (movement-zero +
            //     attack-disadvantage + advantage-to-attackers +
            //     DEX-save-disadvantage) is the load-bearing combat
            //     identity at CR 1 — heavier lock-down than the
            //     constrictor's plain Grappled at one extra tile of
            //     reach.
            //   - Plesiosaurus (CR 2 large beast): reach-2 bite (3d6+STR
            //     piercing) on a ~68 HP envelope. Vanilla heavy biter —
            //     no rider; the threat profile is fat HP + long-neck
            //     standoff. Fills the CR-2 spot under the Triceratops
            //     (CR 5) and T-Rex (CR 8) apex dinos.
            //   - Pteranodon (CR ¼ medium beast): vanilla 2d4 bite on a
            //     fly-derived speed of 60. The swarm-tier flying-dino
            //     filler; mobility is the threat profile, not damage.
            &GIANT_OCTOPUS_TEMPLATE,
            &PLESIOSAURUS_TEMPLATE,
            &PTERANODON_TEMPLATE,
            // Low-CR humanoid mook + ambient beast cohort filling the
            // NPC bench and the rat-tier vermin slot:
            //   - Thug (CR ½ humanoid): Pack Tactics + double-mace
            //     multi + heavy crossbow ranged fallback. Slots between
            //     Bandit (CR ⅛) and Bandit Captain (CR 2) as the mid
            //     bandit-family mook.
            //   - Tribal Warrior (CR ⅛ humanoid): Pack Tactics + double-
            //     spear multi. The "primitive raider" mook — soft alone,
            //     dangerous in a swarm; mirrors the wolf / kobold pack
            //     pattern at the humanoid lane.
            //   - Scout (CR ½ humanoid): Multiattack with shortsword (2x
            //     melee) OR longbow (2x ranged), picked by engagement
            //     distance. The Ranger-flavored NPC slot in the low-CR
            //     bench.
            //   - Giant Rat (CR ⅛ small beast): Pack Tactics + 1d4 bite.
            //     Cheapest pack-tactics vermin in the pool, fills the
            //     dungeon-rat ambient creature slot below Stirge (CR ⅛
            //     too but lacks Pack Tactics).
            //   - Ghast (CR 2 undead): upgraded Ghoul — 2-claw + bite
            //     multiattack with a DC 10 paralyze claw rider. Slots
            //     between Ghoul (CR 1) and Wight (CR 3) on the undead
            //     ladder; the paralysis-auto-crit envelope is the load-
            //     bearing threat.
            &THUG_TEMPLATE,
            &PIRATE_TEMPLATE,
            &PIRATE_CAPTAIN_TEMPLATE,
            &TOUGH_BOSS_TEMPLATE,
            &TRIBAL_WARRIOR_TEMPLATE,
            &SCOUT_TEMPLATE,
            &GIANT_RAT_TEMPLATE,
            &GHAST_TEMPLATE,
            // Civilian-and-watchman tier + cave-dweller / swamp-beast
            // fill-ins. These templates round out the low-CR encounter
            // bench so a random-encounter roll surfaces the canonical
            // "town watch + townsfolk" silhouette and the "swamp / cave
            // beast" ambient lane:
            //   - Commoner (CR 0 humanoid): the baseline townsfolk.
            //     1d4 club, AC 10, no Pack Tactics — a civilian, not a
            //     credible threat. Floors the NPC-CR ladder.
            //   - Mastiff (CR ⅛ medium beast): trained guard dog. Same
            //     trip-bite shape as Wolf (DC 11 STR-vs-Prone) at a
            //     heavier 1d6 die, deliberately lacking Pack Tactics
            //     so it feels distinct from the wild-pack lane.
            //   - Guard (CR ⅛ humanoid): city watchman. AC 16 (chain
            //     shirt + shield) — the *highest AC* on the CR-⅛
            //     humanoid bench. The defensive-shell tradeoff vs the
            //     bandit's damage-focus.
            //   - Grimlock (CR ¼ humanoid): blind Underdark savage.
            //     Spiked bone club (1d4 bludgeoning + 1d4 piercing
            //     rider) + Blindsight 30. The "immune to Invisibility
            //     inside its perception radius" niche.
            //   - Giant Frog (CR ¼ medium beast): swamp ambusher.
            //     Auto-Grappled bite (no save) — first user of the new
            //     `WeaponWithCondition` chassis. Pins targets in melee
            //     for the rest of the swamp pack.
            &COMMONER_TEMPLATE,
            &MASTIFF_TEMPLATE,
            &GUARD_TEMPLATE,
            &GUARD_CAPTAIN_TEMPLATE,
            &GRIMLOCK_TEMPLATE,
            &GIANT_FROG_TEMPLATE,
            // Newest additions filling the tiny-flier / large-reptile /
            // small-arachnid-ambusher / coastal-shark / cavalry-mount
            // lanes:
            //   - Hawk (CR 0 tiny beast): tiny scout with fly 60 and a
            //     flat 1-damage talons swing. Mobility-as-threat at the
            //     very bottom of the CR ladder; slots beside Stirge /
            //     Giant Crab on the CR-0 ambient bench.
            //   - Giant Lizard (CR ¼ large beast): vanilla 1d8 bite on
            //     a 19-HP large frame. The "dungeon-mount" filler beside
            //     Giant Frog / Mastiff on the CR-¼ bench; commonly
            //     ridden by goblins / kobolds in the published modules.
            //   - Giant Wolf Spider (CR ¼ medium beast): lighter-die
            //     sibling of the Spider (CR 1) — 1d6 bite + DC 11 CON
            //     save-or-2d6-poison rider via WeaponWithSaveDamage.
            //     The fragile, fast lone hunter to the regular spider's
            //     web-spinner ambusher.
            //   - Reef Shark (CR ½ medium beast): Pack Tactics shark —
            //     1d8 bite that goes to advantage when an ally shark
            //     is adjacent. The "swarm in the surf" entry on the
            //     shark ladder.
            //   - Hunter Shark (CR 2 large beast): Blood Frenzy heavy
            //     biter — 2d8 bite that goes to advantage on wounded
            //     targets. Shares the BLOOD_FRENZY_TAG chokepoint with
            //     Sahuagin. Solo hunter middle of the shark ladder.
            //   - Giant Shark (CR 5 huge beast): apex Blood Frenzy
            //     biter — 3d10 bite on a 126-HP huge frame with
            //     Blindsight 60. Top of the shark ladder.
            //   - Warhorse (CR ½ large beast): the "trained cavalry
            //     mount" — vanilla 2d6 hooves on a fast (speed 60)
            //     large frame. Pairs with the Knight / Veteran for
            //     the canonical mounted-soldier encounter shape.
            &HAWK_TEMPLATE,
            &GIANT_LIZARD_TEMPLATE,
            &GIANT_WOLF_SPIDER_TEMPLATE,
            &REEF_SHARK_TEMPLATE,
            &HUNTER_SHARK_TEMPLATE,
            &GIANT_SHARK_TEMPLATE,
            &WARHORSE_TEMPLATE,
            // Pre-existing templates that were defined but never added
            // to the random-encounter pool — the generator at any CR
            // target couldn't roll them. Adding them here restores the
            // "every defined creature is reachable" invariant:
            //   - Allip (CR 5 medium undead): the soul-shard ghost
            //     with a 4d6 psychic Maddening Touch swing. Pairs
            //     with the Wraith / Specter / Banshee cohort on the
            //     incorporeal-undead bench at the CR-5 tier.
            //   - Quaggoth (CR 2 medium humanoid): underdark slasher
            //     with claws + Wounded Fury. Fits next to the
            //     Bugbear / Hobgoblin family on the CR-2 humanoid
            //     melee bench. (Aboleth / Solar / Marilith are
            //     intentionally kept out as set-piece bosses used
            //     by scripted AI tests; Tiny Animated Object exists
            //     only as a target for the Animate Objects spell.)
            &ALLIP_TEMPLATE,
            &QUAGGOTH_TEMPLATE,
            // Newest additions filling the CR-¼ to CR-1 ambient-vermin,
            // flying-scavenger, cave-flier, and plant-grappler lanes:
            //   - Giant Vulture (CR 1 large beast): heterogeneous
            //     beak (1d4+STR) + talons (2d4+STR) compound multi with
            //     Pack Tactics — the swarming scavenger sibling of the
            //     solo Giant Eagle at the same CR. Pack Tactics is the
            //     load-bearing tactical multiplier.
            //   - Giant Bat (CR ¼ large beast): single 1d6+STR bite on
            //     a fast (fly 60) frame plus Blindsight 60 — the cave-
            //     dweller anti-stealth flier. Distinct from Hawk /
            //     Pteranodon by its echolocation cone.
            //   - Giant Centipede (CR ¼ small beast): DEX-based 1d4
            //     bite plus a DC 11 CON save-or-3d6-poison rider via
            //     `WeaponWithSaveDamage`. Heavier venom dice than the
            //     Giant Wolf Spider on a fragile 4-HP frame — fills
            //     the venom-glass-cannon niche at CR ¼.
            //   - Vine Blight (CR ½ medium plant): STR-based 2d6
            //     Constrict with a DC 12 STR save-or-Restrained rider
            //     via `WeaponWithSaveCondition`. Fire-vulnerable,
            //     lightning-resistant, Blinded/Deafened-immune. The
            //     low-CR entry on the plant ladder beside Awakened
            //     Tree / Shambling Mound — covers the vegetative
            //     ambusher / grapple-restrain identity at CR ½.
            &GIANT_VULTURE_TEMPLATE,
            &GIANT_BAT_TEMPLATE,
            &GIANT_CENTIPEDE_TEMPLATE,
            &VINE_BLIGHT_TEMPLATE,
            // Twig + Needle Blight: completes the blight family beside
            // Vine Blight so a random "haunted grove" pool can roll the
            // full RAW evil-druid trio.
            //   - Twig Blight (CR ⅛ small plant): 1d4 piercing claws
            //     ambusher with Blindsight 60 and fire vulnerability.
            //     Slots on the CR-⅛ vermin bench beside the Stirge /
            //     Mastiff / Giant Rat — the fragile entry tier of the
            //     blight ladder.
            //   - Needle Blight (CR ¼ medium plant): switch-hitter with
            //     a 2d4 claws melee lane AND a 2d6 ranged needle volley
            //     out to 30/60ft. Slots on the CR-¼ skirmisher bench
            //     between Twig and Vine Blight — the middle tier whose
            //     ranged option distinguishes it from the melee-only
            //     twig / grapple-only vine cohort.
            &TWIG_BLIGHT_TEMPLATE,
            &NEEDLE_BLIGHT_TEMPLATE,
            // Newest additions: filling the porcine / caprid / equine /
            // serpent / cetacean / minor-undead gaps on the encounter
            // ladder so the random generator at CR ¼–3 has a richer
            // mundane-beast / civilian-NPC bench.
            //   - Giant Boar (CR 2 large beast): 2d6 tusks on a chunky
            //     42-HP frame. The mid-CR forest-ambusher upgrade tier
            //     of the regular Boar (1d6 tusks, CR ¼). Fills the
            //     CR-2 single-die heavy-melee niche beside the
            //     Quaggoth / Plesiosaurus cohort.
            //   - Giant Goat (CR ½ large beast): 2d4 ram on a fast
            //     19-HP frame. Slots beside the Warhorse / Vine Blight
            //     on the CR-½ herbivore-megafauna bench — the
            //     headbutting mountain ungulate niche.
            //   - Giant Owl (CR ¼ large beast): 2d6 talons on a fast
            //     (fly 60) frame with Darkvision 120. The nocturnal
            //     aerial scout sibling of the Giant Eagle / Hawk /
            //     Pteranodon cohort at the low-CR end.
            //   - Giant Venomous Snake (CR ¼ medium beast): DEX-based
            //     1d4 bite with a DC 11 CON save-or-3d6-poison rider
            //     via `WeaponWithSaveDamage` at reach 10. The "coiled
            //     viper" sibling of the Constrictor Snake on the snake
            //     bench — venom-rider vs grappler.
            //   - Killer Whale (CR 3 huge beast): 5d6 single-die apex
            //     bite with Blindsight 60 on a 90-HP huge frame. The
            //     echolocating cetacean predator slot beside the
            //     Plesiosaurus / Giant Shark on the marine-megafauna
            //     bench.
            //   - Crawling Claw (CR 0 tiny undead): 1d4 slashing swing
            //     on a fragile 2-HP frame. The "summoner's minor
            //     cantrip" undead minion useful as wave-spawn filler
            //     beside the Commoner on the CR-0 baseline tier.
            //   - Riding Horse / Draft Horse (CR ¼ large beasts): 2d4
            //     hooves chassis shared with the Warhorse. Faster /
            //     lighter (Riding) and slower / stronger (Draft)
            //     civilian-mount tiers beneath the trained Warhorse
            //     on the equine ladder. Travel-encounter staples.
            &GIANT_BOAR_TEMPLATE,
            &GIANT_GOAT_TEMPLATE,
            &GIANT_OWL_TEMPLATE,
            &GIANT_VENOMOUS_SNAKE_TEMPLATE,
            &KILLER_WHALE_TEMPLATE,
            &CRAWLING_CLAW_TEMPLATE,
            &RIDING_HORSE_TEMPLATE,
            &DRAFT_HORSE_TEMPLATE,
            // Newest additions filling the CR-0 ambient / CR-⅛ pack-animal
            // / CR-¼ burrower / CR-½ flying-venom-rider / CR-0 sapling-
            // plant gaps in the encounter ladder:
            //   - Awakened Shrub (CR 0 small plant): the sapling cousin of
            //     the Awakened Tree — fire-vulnerable / piercing-resistant
            //     1d4-1 rake. Floors the plant ladder beneath Twig Blight
            //     (CR ⅛) and Awakened Tree (CR 2).
            //   - Bat (CR 0 tiny beast): flying tiny ambient with
            //     Blindsight 60 + flat-1 piercing bite. Sister to the
            //     Giant Bat (CR ¼) one tier up. The lowest-CR Blindsight
            //     holder in the pool.
            //   - Rat (CR 0 tiny beast): lone-rodent sibling of the Giant
            //     Rat — flat-1 piercing bite, *no* Pack Tactics, anchors
            //     the "ambient vermin" bench floor.
            //   - Camel (CR ⅛ large beast): desert-caravan pack animal —
            //     flat 1d4 bite (no STR-to-damage per RAW). Slots beside
            //     Mastiff / Guard / Bandit on the CR-⅛ civilian bench.
            //   - Giant Badger (CR ¼ medium beast): bite + claws
            //     heterogeneous compound multi (~10 avg/Action) on a
            //     darkvision-30 burrower frame. Fills the CR-¼ mustelid
            //     niche beside Giant Frog / Giant Lizard / Giant Wolf
            //     Spider.
            //   - Giant Wasp (CR ½ medium beast): flying venom drone —
            //     1d6 DEX sting + DC-11 CON save-or-3d6-poison-AND-
            //     Poisoned rider via `WeaponWithSaveDamage::
            //     melee_with_condition`. Sits one CR tier above the
            //     Giant Centipede / Giant Wolf Spider venom-crawler
            //     bench on a fly-50 frame.
            &AWAKENED_SHRUB_TEMPLATE,
            &BAT_TEMPLATE,
            &RAT_TEMPLATE,
            &CAMEL_TEMPLATE,
            &GIANT_BADGER_TEMPLATE,
            &GIANT_WASP_TEMPLATE,
            // CR-0 ambient-beast bench: Cat / Frog / Lizard / Weasel
            // round out the tiny-beast cohort beside the Bat / Rat /
            // Hawk trio. Each slots into the random encounter pool so
            // a low-`cr_target` generation can land a hearth cat, a
            // pond frog, a cave lizard, or a darting weasel as
            // ambient flavor without hand-placing one. The Frog
            // alone has *no* native attack (see `frogs.rs`); the
            // others carry a flat-1 swing matching the bat / rat /
            // hawk envelope at this CR tier.
            &CAT_TEMPLATE,
            &FROG_TEMPLATE,
            &LIZARD_TEMPLATE,
            &WEASEL_TEMPLATE,
            // Mundane domestic herbivore / pack-animal cohort —
            // rounds out the CR-0 → CR-¼ "farmyard / caravan" bench
            // beside Camel / Mastiff / Riding Horse / Draft Horse
            // already in the pool. Each entry is a stat-light
            // ambient that exists for travel-encounter texture and
            // ranch / barn scenery; none of them are credible
            // combat threats in isolation.
            //   - Goat (CR 0 medium beast): 1d4 ram, the CR-0 floor
            //     of the caprid family beside the CR-½ Giant Goat.
            //   - Mule (CR ⅛ medium beast): 1d4 hooves, the
            //     stubborn pack-hauler sibling to the Camel /
            //     Pony / Mastiff at the same CR tier.
            //   - Pony (CR ⅛ medium beast): 2d4 hooves, the
            //     small-rider mount tier — same hooves dice as
            //     the Riding Horse on a medium frame and a +1
            //     lighter STR mod.
            //   - Elk (CR ¼ large beast): 1d6 ram + 2d4 hooves
            //     dual-action lane (no Multiattack per RAW). The
            //     fastest CR-¼ ambient (speed 50) on the
            //     herbivore-megafauna bench beside Boar / Riding
            //     Horse / Draft Horse / Giant Goat.
            &GOAT_TEMPLATE,
            &MULE_TEMPLATE,
            &PONY_TEMPLATE,
            &ELK_TEMPLATE,
            // Lineage builds — humanoid opponents on class chassis, in
            // the CR 1-3 band. They sit here for the same reason
            // `CLERIC_TEMPLATE`, `KNIGHT_TEMPLATE`, `MAGE_TEMPLATE` and
            // `VETERAN_TEMPLATE` already do: a PC-shaped stat block is a
            // perfectly good enemy, and these were written, tested and
            // then reachable by nothing but a unit test. Each brings a
            // racial trait the bestiary otherwise has no source for —
            // the Tiefling's Hellish Rebuke, the Aasimar's Healing
            // Hands, Dwarven Resilience, Halfling Luck, Relentless
            // Endurance, Gnome Cunning.
            //
            // One dragonborn, not fifteen. The chromatic / metallic /
            // gem ancestries differ only in their breath weapon's damage
            // type, so putting every one in the pool would weight the
            // generator a third towards "a dragonborn, again" for no
            // variety in return. The other fourteen stay playable.
            &crate::actors::creatures::dragonborn::DRAGONBORN_TEMPLATE,
            &crate::actors::creatures::tieflings::TIEFLING_TEMPLATE,
            &crate::actors::creatures::aasimars::AASIMAR_TEMPLATE,
            &crate::actors::creatures::dwarves::DWARF_TEMPLATE,
            &crate::actors::creatures::halflings::HALFLING_SCOUT_TEMPLATE,
            &crate::actors::creatures::half_orcs::HALF_ORC_TEMPLATE,
            &crate::actors::creatures::gnomes::GNOME_TEMPLATE,
            // The five SRD swarms (CR ¼ – 2). The first entries in the
            // pool that answer "hit it with a sword" with "that will
            // not work" — four of the five resist all three physical
            // damage types, none of them can be healed, and none can be
            // knocked prone, grappled, or frightened out of the fight.
            // A party used to reaching for the fighter has to reach for
            // the fireball instead.
            //
            // Rolled in as a family through `all_swarm_templates` so
            // the pool inherits a sixth swarm the day one is written,
            // rather than needing a line here that someone has to
            // remember.

            // The boss shelf. Twelve finished stat blocks that no
            // encounter could roll — every one of them written,
            // documented and tested, and reachable only from the test
            // suite that tested them. The lich and the beholder each
            // carry a complete three-entry lair-action table in
            // `engine::lair_actions` that nothing but
            // `every_lair_action_resolves_with_and_without_anybody_to_catch`
            // ever fired.
            //
            // They are here rather than behind a separate boss pool
            // because the budget is a ceiling now
            // (`actor_gen::affordable_templates`), which is what makes
            // this safe: a CR-30 tarrasque is unreachable at the CR-1
            // budget the game opens with and becomes reachable exactly
            // when the ramp can pay for it. Before that fix, adding
            // them would have meant one first fight in two hundred and
            // fifty being unwinnable.
            //
            // The three that were already in the pool — the iron
            // golem, the androsphinx, the kraken — are the reason this
            // reads as an oversight rather than a design: nothing
            // distinguishes them from the eleven below except which
            // line someone remembered to write.
            // The aboleth is the twelfth, and the odd one out: a CR-10
            // aberration rather than a boss, and the only entry here
            // whose absence had no plausible reading at all — it is
            // alphabetically the first creature file in the directory.
            &crate::actors::creatures::aboleths::ABOLETH_TEMPLATE,
            &crate::actors::creatures::balors::BALOR_TEMPLATE,
            &crate::actors::creatures::beholders::BEHOLDER_TEMPLATE,
            &crate::actors::creatures::death_knights::DEATH_KNIGHT_TEMPLATE,
            &crate::actors::creatures::devas::DEVA_TEMPLATE,
            &crate::actors::creatures::glabrezus::GLABREZU_TEMPLATE,
            &crate::actors::creatures::liches::LICH_TEMPLATE,
            &crate::actors::creatures::mariliths::MARILITH_TEMPLATE,
            &crate::actors::creatures::pit_fiends::PIT_FIEND_TEMPLATE,
            &crate::actors::creatures::solars::SOLAR_TEMPLATE,
            &crate::actors::creatures::stone_golems::STONE_GOLEM_TEMPLATE,
            // Clay Golem (CR 9) — the rung between the flesh golem (CR
            // 5) and the stone one (CR 10), and the widest gap the
            // golem ladder had. Carries the roster's only acid
            // absorption and the only unconditional hit-point-maximum
            // drain, which is a different kind of pressure from anything
            // else at its CR: a party that out-heals it still loses.
            &crate::actors::creatures::clay_golems::CLAY_GOLEM_TEMPLATE,
            &crate::actors::creatures::tarrasques::TARRASQUE_TEMPLATE,
            // The SRD's NPC appendix, which the pool had never carried
            // a single entry of. Every other family in this list is a
            // monster; these are people, and a dungeon whose only
            // humanoids are bandits and cultists' betters is missing
            // the half of 5e's bestiary that talks.
            //
            //   - Acolyte (CR ¼): the cheapest healer in the game.
            //     Three level-1 slots of Cure Wounds and Bless, which
            //     is enough to make a mob of anything harder.
            //   - Cultist (CR ⅛): a scimitar and Dark Devotion —
            //     advantage against charm and fear, so the low-CR
            //     answer to a mob does not work on this one.
            //   - Noble (CR ⅛): AC 15 and a Parry reaction on a
            //     nine-hit-point body.
            //   - Spy (CR 1): Cunning Action and a hand crossbow. Never
            //     where the swing was aimed.
            //   - Priest (CR 2): two slot tiers, Spiritual Weapon, and
            //     Guiding Bolt to hand the front line its advantage.
            //   - Gladiator (CR 5): three attacks, Parry and Brave. The
            //     appendix's melee boss.
            //   - Assassin (CR 8): 7d6 of venom on both a blade and a
            //     bolt, Assassinate, and Evasion.
            //   - Archmage (CR 12): nine tiers of slots, Counterspell,
            //     Shield, and Magic Resistance. The apex of the list
            //     and the only NPC in it that fights like a party.
            &ACOLYTE_TEMPLATE,
            &CULTIST_TEMPLATE,
            &NOBLE_TEMPLATE,
            &SPY_TEMPLATE,
            &PRIEST_TEMPLATE,
            &GLADIATOR_TEMPLATE,
            &ASSASSIN_TEMPLATE,
            &ARCHMAGE_TEMPLATE,
            // …and the monsters that came with them, each filling a
            // rung the pool could not previously roll:
            //   - Azer (CR 2 elemental): a body that burns whatever
            //     touches it and a hammer that burns whatever it
            //     touches. The cheapest melee-reflect creature there is.
            //   - Barbed Devil (CR 5 fiend): three piercing swings in
            //     contact, 3d6 fire at range, and a hide that answers
            //     back.
            //   - Chain Devil (CR 8 fiend): two reach-10 chains a round,
            //     each a save against Restrained. The bestiary's most
            //     single-minded lockdown creature.
            //   - Darkmantle (CR ½ monstrosity): casts Darkness and
            //     then fights inside it on blindsight, and blinds
            //     whatever it lands on besides.
            //   - Duergar (CR 1 humanoid): Enlarge and Invisibility on
            //     a CR 1 frame, paid for with Sunlight Sensitivity.
            //   - Ochre Jelly (CR 2 ooze): immune to the two damage
            //     types that would have split it.
            //   - Satyr (CR ½ fey): Magic Resistance eight CRs early,
            //     on forty feet of speed.
            //   - Shield Guardian (CR 7 construct): ten hit points a
            //     round back, and nothing in the game switches it off.
            //   - Violet Fungus (CR ¼ plant): three necrotic stalks at
            //     reach 10 behind AC 5.
            //   - Warhorse Skeleton (CR ½ undead): the mount an undead
            //     knight rides, and the pool's first mountable undead.
            //   - Winged Kobold (CR ¼ humanoid): Pack Tactics that can
            //     choose where to stand.
            //   - Panther (CR ¼ beast): fifty feet of speed into a
            //     pounce, at the bottom of the feline ladder.
            &AZER_TEMPLATE,
            &BARBED_DEVIL_TEMPLATE,
            &CHAIN_DEVIL_TEMPLATE,
            &DARKMANTLE_TEMPLATE,
            &DUERGAR_TEMPLATE,
            &OCHRE_JELLY_TEMPLATE,
            &SATYR_TEMPLATE,
            &SHIELD_GUARDIAN_TEMPLATE,
            &VIOLET_FUNGUS_TEMPLATE,
            &WARHORSE_SKELETON_TEMPLATE,
            &WINGED_KOBOLD_TEMPLATE,
            &PANTHER_TEMPLATE,
            // The tail of the SRD roster — the entries left over once
            // the appendix and the named monsters were in, each one
            // filling a rung nothing else sits on:
            //   - Remorhaz (CR 11 monstrosity): the hardest single
            //     swing below the ancient dragons, immune to both fire
            //     and cold, and 3d6 fire back at everything that
            //     touches it. The largest reflect in the bestiary.
            //   - Water Weird (CR 3 elemental): arrives invisible and
            //     restrains on a hit with no save. Vulnerable to cold,
            //     which is the answer.
            //   - Rug of Smothering (CR 2 construct): restrains on hit
            //     and cannot itself be knocked over.
            //   - Merfolk (CR ⅛ humanoid): the cheapest aquatic body,
            //     and the one humanoid that swings at no penalty in a
            //     lake.
            //   - Homunculus (CR 0 construct): five hit points and a
            //     DC 10 venom that outweighs the rest of the creature.
            //   - Giant Fire Beetle (CR 0 beast): the only beast in the
            //     game that is a lamp.
            //   - Giant Weasel (CR ⅛), Jackal (CR 0), Raven (CR 0),
            //     Vulture (CR 0), Quipper (CR 0): the low end. Two of
            //     them carry Pack Tactics and one carries Blood Frenzy,
            //     which is what makes a number of them a fight.
            &REMORHAZ_TEMPLATE,
            &WATER_WEIRD_TEMPLATE,
            &RUG_OF_SMOTHERING_TEMPLATE,
            &MERFOLK_TEMPLATE,
            &HOMUNCULUS_TEMPLATE,
            &GIANT_FIRE_BEETLE_TEMPLATE,
            &GIANT_WEASEL_TEMPLATE,
            &JACKAL_TEMPLATE,
            &RAVEN_TEMPLATE,
            &VULTURE_TEMPLATE,
            // The rest of SRD 5.2's animal appendix — the block the
            // roster had been carrying in pieces. Twenty-three stat
            // blocks, from the CR-0 shelf up to the CR-4 dinosaurs,
            // grouped here because they arrived together and because
            // each one fills a rung by *shape* rather than by number:
            //   - The CR-0 floor doubles in size (Baboon, Badger, Crab,
            //     Deer, Eagle, Octopus, Owl, Piranha, Scorpion,
            //     Seahorse). Five of them carry a clause nothing else
            //     at that tier has — Pack Tactics, poison resistance,
            //     Agile, Flyby, Blood Frenzy — so the bottom of the
            //     ladder stops being ten copies of the same rat.
            //   - The CR-⅛ to CR-½ band gains the escalating Blood Hawk,
            //     two venomous snakes (one of which flies), the Ape's
            //     recharging thrown rock, the Black Bear, and the Giant
            //     Seahorse.
            //   - The CR-2 to CR-4 band gains four charge-shaped
            //     heavies (Rhinoceros, Giant Elk, Allosaurus, Elephant),
            //     the Ankylosaurus's double knockdown, and two plain
            //     hard hitters (Hippopotamus, Archelon).
            //   - The Axe Beak arrives as the cheapest fast mount in
            //     the game, and the Giant Elk as the only CR-2
            //     celestial.
            &ALLOSAURUS_TEMPLATE,
            &ANKYLOSAURUS_TEMPLATE,
            &APE_TEMPLATE,
            &ARCHELON_TEMPLATE,
            &AXE_BEAK_TEMPLATE,
            &BABOON_TEMPLATE,
            &BADGER_TEMPLATE,
            &BLACK_BEAR_TEMPLATE,
            &BLOOD_HAWK_TEMPLATE,
            &CRAB_TEMPLATE,
            &DEER_TEMPLATE,
            &EAGLE_TEMPLATE,
            &ELEPHANT_TEMPLATE,
            &FLYING_SNAKE_TEMPLATE,
            &GIANT_ELK_TEMPLATE,
            &GIANT_SEAHORSE_TEMPLATE,
            &HIPPOPOTAMUS_TEMPLATE,
            &OCTOPUS_TEMPLATE,
            &OWL_TEMPLATE,
            &PIRANHA_TEMPLATE,
            &RHINOCEROS_TEMPLATE,
            &SCORPION_TEMPLATE,
            &SEAHORSE_TEMPLATE,
            &VENOMOUS_SNAKE_TEMPLATE,
            // The top of two extraplanar ladders and the bottom of two
            // more, each filling a rung nothing else sits on:
            //   - Ice Devil (CR 14 large fiend): the hierarchy's
            //     general, closing the five-point gap between Erinyes
            //     (CR 12) and Pit Fiend (CR 20). Two ice spears and a
            //     tail per Action, all four damage lanes cold-tinged,
            //     behind cold / fire / poison immunity, Magic
            //     Resistance and blindsight at the board's full width.
            //   - Planetar (CR 16 large celestial): the missing angel
            //     between Deva (CR 10) and Solar (CR 21), and the first
            //     creature in the bestiary whose area damage knows
            //     whose side it is on — Holy Burst is the roster's only
            //     `enemies_only` point burst.
            //   - Sphinx of Wonder (CR 1 tiny celestial): Magic
            //     Resistance on a 24-hit-point frame, which inverts the
            //     usual CR-1 profile — it survives spells and dies to
            //     soldiers.
            //   - Gray Ooze (CR ½ medium ooze): the bottom of the ooze
            //     ladder below Ochre Jelly / Gelatinous Cube (CR 2).
            //     The hardest single hit at its tier off an AC of 9,
            //     and immune to every condition that would control it.
            &ICE_DEVIL_TEMPLATE,
            &PLANETAR_TEMPLATE,
            &SPHINX_OF_WONDER_TEMPLATE,
            &GRAY_OOZE_TEMPLATE,
        ];
        pool.extend(crate::actors::creatures::swarms::all_swarm_templates());
        // The whole dragon ladder — ten colours across four age
        // categories, CR 1 through 24. Extended rather than listed for
        // the same reason the swarms are: the forty are one table in
        // `creatures::dragons`, and forty names spelled out here would
        // be a second copy of it to keep in step. The generator's CR
        // ceiling is what keeps an ancient gold out of a first fight.
        pool.extend(crate::actors::creatures::dragons::all_dragon_templates());
        pool.extend(crate::actors::creatures::half_dragons::all_half_dragon_templates());
        pool
    }

    /// Drop an item onto a tile. Multiple items can stack on the same
    /// tile (a hallway with two corpses); pickup grabs them all at once.
    pub fn drop_item(&mut self, coord: Coordinate, item: &'static crate::items::item_template::Item) {
        self.items_on_ground.entry(coord).or_default().push(item);
    }

    /// Read-only access to the loot pile on a tile (empty slice if none).
    /// The renderer uses this to draw the ground-glyph; tests use it to
    /// verify drop/pickup transitions.
    pub fn items_at(
        &self,
        coord: Coordinate,
    ) -> &[&'static crate::items::item_template::Item] {
        self.items_on_ground
            .get(&coord)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Hand every item on `coord` to the actor, log the pickups, clear the
    /// pile. Called by `MoveActor::apply` after each successful step so
    /// walking over loot just absorbs it. No-op if the tile is empty or
    /// the actor has been removed mid-move.
    pub fn pickup_items_at(&mut self, actor_id: usize, coord: Coordinate) {
        let Some(items) = self.items_on_ground.remove(&coord) else {
            return;
        };
        if items.is_empty() {
            return;
        }
        let actor_name = self
            .actors
            .get(&actor_id)
            .map(|a| a.name().to_string())
            .unwrap_or_else(|| format!("actor#{}", actor_id));
        for item in items {
            if let Some(actor) = self.actors.get_mut(&actor_id) {
                actor.pickup_item(item);
            }
            self.log(format!("{} picks up {}.", actor_name, item.name));
        }
    }

    /// Long rest every actor still in the encounter — full HP, all spell
    /// slots restored, conditions and concentration cleared. PCs (team 0)
    /// also try to cash in accumulated XP for one or more level-ups in a
    /// loop until they're below the next threshold; we then re-restore
    /// HP so the bonus from the level applies cleanly.
    pub fn long_rest(&mut self) {
        // Everybody gets off their horse. A rest is not a thing you take
        // in the saddle, and — more to the point — a rider is off the
        // occupancy grid while the link is up, so any teardown that
        // merely cut the link would leave a body the board could not
        // see. `dismount` is the one that puts them back on it.
        for id in self.sorted_actor_ids() {
            if self.is_mounted(id) {
                self.dismount(id);
            }
        }
        // Iterate ids in sorted order so multiple level-up rolls are
        // deterministic with the seeded RNG (HashMap order would otherwise
        // shuffle who rolls first across runs).
        let ids = self.sorted_actor_ids();
        for id in ids {
            let mut announcements: Vec<String> = Vec::new();
            if let Some(actor) = self.actors.get_mut(&id) {
                actor.long_rest();
                if actor.team() == 0 {
                    while let Some(new_level) = actor.try_level_up(&mut self.roller) {
                        announcements.push(format!(
                            "{} reaches level {}! (HP up to {})",
                            actor.name(),
                            new_level,
                            actor.max_hitpoints()
                        ));
                    }
                }
            }
            for line in announcements {
                self.log(line);
            }
        }
    }

    /// Short rest every actor still in the encounter — partial HP
    /// recovery via Hit Dice and short-rest feature refresh (Fighter's
    /// Second Wind / Action Surge). Does not clear conditions or
    /// restore full HP.
    pub fn short_rest(&mut self) {
        let ids = self.sorted_actor_ids();
        for id in ids {
            if let Some(actor) = self.actors.get_mut(&id) {
                let name = actor.name().to_string();
                let hp_before = actor.hitpoints();
                actor.short_rest(&mut self.roller);
                let hp_after = actor.hitpoints();
                if hp_after > hp_before {
                    self.log(format!(
                        "{} rests and recovers {} HP ({} \u{2192} {}).",
                        name,
                        hp_after - hp_before,
                        hp_before,
                        hp_after,
                    ));
                }
            }
        }
    }

    pub fn skip_turn(&mut self) {
        self.advance_initiative();
        let Some(next_id) = self.initiative_tracker.current_player() else {
            return;
        };
        self.start_turn_for(next_id);
    }

    /// Actor id whose turn is currently active, or `None` if the
    /// initiative queue is empty. Public accessor for side-effect
    /// chokepoints (Warlock **Dark One's Blessing**, future
    /// current-turn-scoped triggers) that need to attribute an outcome
    /// to whoever is swinging without threading an explicit attacker
    /// argument through every call site. Reads through the same
    /// initiative-tracker slot the AI / action-execution loop uses,
    /// so an out-of-turn effect (reaction, legendary action) fires
    /// against the currently-scheduled actor rather than the reaction
    /// holder — matching RAW's "on your turn" phrasing.
    pub fn current_turn_actor_id(&self) -> Option<usize> {
        self.initiative_tracker.current_player()
    }

    /// **A creature just went down** — the shared chokepoint for every
    /// feature that pays out when a creature is reduced to 0 HP. Called
    /// from `DealDamage::apply` on the `Downed` / `Killed` outcome
    /// branches, which are the only two places a drop can happen.
    ///
    /// Two lanes hang off it, and they are scoped differently on
    /// purpose:
    ///
    ///   - **Kill-triggered temp HP** (`KILL_TRIGGERED_TEMP_HP_SOURCES`)
    ///     pays the *killer*. Dark One's Blessing and Touch of Death
    ///     both read "whenever you reduce a hostile creature to 0 hit
    ///     points", so the beneficiary is whoever is swinging.
    ///   - **Hexblade's Curse** pays the *curser*. RAW is "if the cursed
    ///     target dies, you regain hit points" — it says nothing about
    ///     who landed the blow, so an ally's arrow, a failed death save
    ///     or the target walking into a wall of fire all pay the
    ///     hexblade just the same.
    ///
    /// Keeping both on one entry point is what stops the second lane
    /// from having to find its own drop hook. A future "on kill" feature
    /// picks whichever scoping matches its RAW text and joins here.
    ///
    /// Walks the `KILL_TRIGGERED_TEMP_HP_SOURCES` cohort — each row
    /// pairs a passive-feature tag with a temp-HP formula closure. If
    /// the current turn actor holds any row's tag AND the dropped
    /// target belongs to a different team (RAW: "hostile creature"),
    /// the row's formula runs against the swinger's stat block and the
    /// resulting temp HP is granted through the standard `GainTempHp`
    /// side effect so the max-of-current-and-new stack rule still
    /// holds (a killer who drops two enemies keeps whichever gift was
    /// larger, not both stacked). First matching row wins — RAW rules
    /// out multiclass co-occurrence for the currently-shipped sources
    /// (Fiend Warlock vs. Long Death Monk are different classes with
    /// different templates), so the first-match short-circuit is a
    /// deterministic pick rather than an ordering hazard.
    ///
    /// The self-kill guard (`current_turn != dropped_target`) covers
    /// the pathological "reduce self to 0" case (e.g. Hellish Rebuke
    /// mirroring damage back onto the caster via Warding Bond). The
    /// team check runs even when both actors are hostile-to-hostile —
    /// a Fiend warlock / Long Death monk on the enemy team dropping a
    /// party PC still fires the temp HP grant per RAW ("hostile" is
    /// anchored to the swinger, not the party); the team-distinct
    /// filter is the engine's cleanest proxy.
    ///
    /// No-op when the current turn actor is missing (start-of-encounter
    /// pre-init edge case) or holds none of the cohort's tags — the
    /// check runs on every downed / killed event, and template-flag
    /// lookup is cheap.
    ///
    /// Sibling to the shared attack-chokepoint cohorts
    /// (`REACTIVE_ATTACK_DISADVANTAGE_SOURCES`,
    /// `FAILED_SAVE_ADD_DIE_SOURCES`, `PASSIVE_TYPED_RESISTANCES`) —
    /// same "walk a table of `{tag, closure}` rows at a chokepoint"
    /// pattern that lets a new feature drop in as a one-line row
    /// entry rather than a fresh open-coded trigger function.
    pub fn trigger_creature_dropped(&mut self, dropped_target_id: usize) {
        // A horse that goes down goes down with its rider on it. Run
        // first, because the two payouts below can heal and can kill,
        // and both read a board where the rider is already off.
        self.unseat(
            dropped_target_id,
            crate::engine::mounts::UnseatCause::MountDropped,
        );
        self.pay_hexblade_curse_on_death(dropped_target_id);
        self.pay_kill_triggered_temp_hp(dropped_target_id);
    }

    /// The curser-scoped half of `trigger_creature_dropped`: a hexblade
    /// whose **Hexblade's Curse** was on the dropped creature regains
    /// `warlock level + CHA modifier` hit points (RAW, minimum 1 so a
    /// low-CHA build still gets something).
    ///
    /// Reads the curse through `hexblade_curse_holder`, so a curse that
    /// had already timed out pays nothing, and a curse that was
    /// overwritten by a second hexblade pays the second one. The heal is
    /// routed through the standard `Heal` side effect, which means it
    /// respects the hexblade's HP cap and, notably, does nothing if the
    /// hexblade is themselves down — RAW's "you regain hit points"
    /// doesn't revive.
    fn pay_hexblade_curse_on_death(&mut self, dropped_target_id: usize) {
        use crate::engine::side_effects::{ApplicableSideEffect, Heal};
        let Some(hexblade_id) = self.hexblade_curse_holder(dropped_target_id) else {
            return;
        };
        let Some(hexblade) = self.actors.get(&hexblade_id) else {
            return;
        };
        if !hexblade.is_combat_active() {
            return;
        }
        let cha = hexblade.ability_modifier(crate::engine::types::AbilityScoreType::Charisma);
        let amount = (cha + hexblade.level() as i32).max(1) as u32;
        let hexblade_name = hexblade.name().to_string();
        let target_name = self.actor_name(dropped_target_id);
        self.log(format!(
            "  hexblade's curse: {} falls and {} draws {} hit points from the curse.",
            target_name, hexblade_name, amount
        ));
        // The curse ends on the payout — RAW's clause is "if the cursed
        // target dies", which happens once. Clearing it matters because
        // a creature can reach 0 HP more than once: a downed ally healed
        // back up and dropped again re-enters this hook, and without
        // this the one curse would pay every time. Removing the
        // condition takes the back-link with it, so there is nothing
        // else to unwind.
        if let Some(target) = self.actors.get_mut(&dropped_target_id) {
            target.remove_condition(Condition::HexbladeCursed);
        }
        Heal {
            actor_id: hexblade_id,
            amount,
        }
        .apply(self);
    }

    /// The killer-scoped half of `trigger_creature_dropped` — see the
    /// entry point and `KILL_TRIGGERED_TEMP_HP_SOURCES` for the cohort.
    fn pay_kill_triggered_temp_hp(&mut self, dropped_target_id: usize) {
        use crate::engine::side_effects::{ApplicableSideEffect, GainTempHp};
        let Some(swinger_id) = self.current_turn_actor_id() else {
            return;
        };
        // Team-distinct guard runs through the shared `actors_enemies`
        // helper — the same negation-of-`actors_allied` gate that the
        // harmful-action pipeline uses. This handles both the self-kill
        // case (`actors_enemies(x, x)` is false) and the friendly-fire
        // case (same team → false) in one check.
        if !self.actors_enemies(swinger_id, dropped_target_id) {
            return;
        }
        let temp_amount = {
            let Some(swinger) = self.actors.get(&swinger_id) else {
                return;
            };
            let Some(row) = KILL_TRIGGERED_TEMP_HP_SOURCES
                .iter()
                .find(|r| swinger.has_passive_feature(r.tag))
            else {
                return;
            };
            (row.amount)(swinger)
        };
        GainTempHp {
            actor_id: swinger_id,
            amount: temp_amount,
        }
        .apply(self);
    }

    /// Per-actor turn-start hook: refresh resources, clear expiring
    /// self-buffs (Dodge), and log anything that ended. Centralized so
    /// every code path that advances the queue (skip_turn, dying-loop,
    /// process_stack) does the same prep — drift between them silently
    /// breaks Dodge / future turn-start mechanics.
    fn start_turn_for(&mut self, actor_id: usize) {
        let (name, expired, restore_displacement) = match self.actors.get_mut(&actor_id) {
            Some(a) => {
                let restore = a.has_displacement()
                    && !a.has_condition(Condition::Displaced);
                // 5e Rogue Assassin **Assassinate** (level 3) tracker. The
                // first turn an actor takes in this encounter flips their
                // once-only `has_taken_turn_in_combat` latch — the
                // Assassinate gate in `compute_attack_mode` keys off the
                // *target* still having the flag at false. Setting it
                // here (rather than at the bottom of the turn) matches RAW
                // "any creature that hasn't taken a turn in the combat
                // yet" — once their slot is up, they're no longer eligible
                // even before they actually act.
                a.mark_taken_turn_in_combat();
                (a.name().to_string(), a.reset_for_new_round(), restore)
            }
            None => return,
        };
        // Latched only on the path that actually opened a turn: a
        // missing actor means the slot is stale, and `process_stack`'s
        // own guard is what repairs the queue in that case.
        self.turn_started_for = Some(actor_id);
        for c in expired {
            self.log(format!("{} is no longer {}.", name, c.name()));
        }
        if restore_displacement {
            if let Some(a) = self.actors.get_mut(&actor_id) {
                a.add_condition(
                    Condition::Displaced,
                    crate::conditions::ConditionTimer::Permanent,
                );
            }
            self.log(format!("{}'s displacement reasserts itself.", name));
        }
        // 5e Conquest Paladin Aura of Conquest: a Frightened creature
        // standing in an enemy paladin's aura loses its movement for
        // the turn and takes psychic damage. Runs after
        // `reset_for_new_round` has handed out the turn's movement,
        // because zeroing a budget that hasn't been granted yet would
        // be undone a line later.
        self.apply_aura_of_conquest(actor_id);
        // 5e Oath of Redemption Paladin **Protective Spirit**: the
        // paladin knits itself back together while it is badly hurt.
        // Runs alongside the Conquest aura because both are per-turn
        // passives that read the actor's state at the top of the turn
        // and neither depends on the other; see `apply_protective_spirit`
        // for why the heal lands here rather than at the turn's end.
        self.apply_protective_spirit(actor_id);
        // 5e Sunlight Hypersensitivity: "the vampire takes 20 radiant
        // damage when it starts its turn in sunlight". Beside the other
        // two start-of-turn passives for the same reason they are
        // beside each other — all three read the actor's state at the
        // top of the turn and none depends on the others.
        self.apply_sunlight_hypersensitivity(actor_id);
        // 5e **passive Perception**, doing the one job RAW gives it:
        // noticing something without consciously looking for it. See
        // `notice_hidden_enemies`.
        self.notice_hidden_enemies(actor_id);
        // 5e's attach clause: "the target takes 5 (2d4) Necrotic damage
        // at the start of each of the stirge's turns", and the same
        // tick refreshes whatever the latch imposes on its host.
        // Beside the other three start-of-turn passives because it is
        // the same shape — read the actor's state at the top of the
        // turn, pay out — and after them because the drain can drop the
        // host, and a host that goes down should do so on a board the
        // other three have already finished with.
        self.drain_attached_host(actor_id);
        // 5e controlled mount: "it moves as you direct it". The rider
        // walks on the horse's legs, so the turn's movement budget is
        // the horse's speed rather than their own. Runs after
        // `reset_for_new_round` has handed out the rider's own budget
        // (this replaces it) and after the Conquest aura has had its
        // say (a rooted rider is rooted whatever they are sitting on —
        // `Rooted` is a `zeros_movement` condition, which no budget can
        // buy past).
        self.grant_mounted_movement(actor_id);
        // 5e Recharge: at the start of each turn, roll a d6 for each
        // spent recharge ability. If the roll >= the ability's threshold,
        // the ability becomes available again.
        if let Some(a) = self.actors.get(&actor_id) {
            let actor_name = a.name().to_string();
            let recharge_checks: Vec<(&'static str, u32, bool)> = a
                .recharge_entries()
                .iter()
                .filter(|(_, _, avail)| !avail)
                .cloned()
                .collect();
            for (ability_name, min_roll, _) in recharge_checks {
                let roll = self.roll(&crate::engine::dice::Dice::new(1, 6));
                if roll >= min_roll {
                    if let Some(a) = self.actors.get_mut(&actor_id) {
                        a.set_recharge_available(ability_name, true);
                    }
                    self.log(format!(
                        "{}'s {} recharges!",
                        actor_name, ability_name
                    ));
                }
            }
        }
        // Same expiry again, one axis over, and first of the three
        // because it is the sweep that decides whether there is a body
        // on the board for the other two to measure: an actor whose
        // banishment lapsed has to be standing somewhere before anyone
        // asks how big it is or how far it has to fall.
        self.reconcile_board_presence();
        // `reset_for_new_round` above expires the UntilStartOfNextTurn
        // conditions, which can include a growth effect — so the actor
        // shrinks back before they spend a single tile of the movement
        // they were just handed.
        self.reconcile_footprints();
        // Same expiry, same reason, one axis over: an
        // `UntilStartOfNextTurn` flight source that just lapsed drops
        // its holder now rather than at the end of the round they no
        // longer have a spell for.
        self.reconcile_altitudes();
        // A new turn is a fresh "first time on a turn" for everybody, so
        // the ledger is cleared for the whole board rather than for the
        // actor whose turn is opening. RAW scopes the clause to *a
        // turn*, not to the holder's own turn: a creature shoved into a
        // web during somebody else's turn has entered it for the first
        // time on that turn and saves for it.
        self.zone_contacts_this_turn.clear();
        // "The cloud moves 10 feet away from you at the start of each of
        // your turns", and the attached sphere's "it moves with you".
        // Runs after the ledger clear, so a creature the cloud arrives
        // on pays for the arrival, and before `touch_zones`, so the
        // owner who has just been overtaken by their own drifting cloud
        // is standing in it by the time the "starts its turn there"
        // clause is asked.
        self.advance_owned_zones(actor_id);
        // "…or starts its turn there." Runs after the clear so the
        // creature standing in the web pays this turn's save, and after
        // `reconcile_footprints` so a creature that just grew into the
        // area is caught by it.
        self.touch_zones(actor_id);
    }

    /// 5e Conquest Paladin **Aura of Conquest** (subclass level 7), both
    /// clauses, at the start of the victim's turn:
    ///
    ///   - "its speed is 0, and it can't benefit from any bonus to its
    ///     speed" — installed as the `Rooted` condition rather than by
    ///     draining the movement budget, because the second half of
    ///     that sentence is what makes the aura hold anyone. A drained
    ///     budget is refilled by a Dash; a `zeros_movement` condition
    ///     makes `remaining_movement` read zero however much budget the
    ///     Dash hands over.
    ///   - "it takes psychic damage equal to half your paladin level if
    ///     it starts its turn there".
    ///
    /// Gated on the victim being Frightened, which is what makes the
    /// aura a *combination* rather than a standalone lockdown: the
    /// Conquest paladin has to land Conquering Presence (or any other
    /// fear) first, and the aura is what converts that fear from
    /// "disadvantage on attacks" into "cannot leave." Without the pair,
    /// a frightened creature simply walks out of the 10 ft and the aura
    /// never bites.
    fn apply_aura_of_conquest(&mut self, actor_id: usize) {
        use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};
        let caught = self.actors.get(&actor_id).is_some_and(|a| {
            a.is_combat_active() && a.has_condition(Condition::Frightened)
        }) && self.in_hostile_aura_of_conquest(actor_id);
        if !caught {
            return;
        }
        let name = self.actor_name(actor_id);
        if let Some(a) = self.actors.get_mut(&actor_id) {
            // Re-installed on every turn the aura still catches them,
            // so it lapses on its own the moment the paladin drops or
            // the fear lifts — no teardown to forget.
            a.add_condition(
                Condition::Rooted,
                crate::conditions::ConditionTimer::UntilStartOfNextTurn,
            );
        }
        self.log(format!(
            "  aura of conquest: {} is rooted in place by dread, and takes {} psychic.",
            name,
            AURA_OF_CONQUEST_PSYCHIC
        ));
        DealDamage {
            actor_id,
            amount: AURA_OF_CONQUEST_PSYCHIC,
            damage_type: DamageType::Psychic,
        }
        .apply(self);
    }

    /// 5e Oath of Redemption Paladin **Protective Spirit** (subclass
    /// level 15): "you regain hit points equal to 1d6 + half your paladin
    /// level if you end your turn in combat with fewer than half of your
    /// hit points remaining and you aren't incapacitated."
    ///
    /// Every clause of that sentence is a gate here, and the last one is
    /// what keeps the feature from being a resurrection: a paladin who
    /// has been knocked unconscious is incapacitated, so the spirit stops
    /// mending them exactly when they need it most. That is RAW and it is
    /// the reason the feature pairs with Aura of the Guardian rather than
    /// replacing the need for allies — a paladin absorbing the party's
    /// damage still has to not go down.
    ///
    /// Fires at the start of the paladin's turn rather than at the end of
    /// it; see `PROTECTIVE_SPIRIT_TAG` for why, and for what the one-tick
    /// shift is observable against.
    fn apply_protective_spirit(&mut self, actor_id: usize) {
        use crate::actions::class_features::PROTECTIVE_SPIRIT_TAG;
        let eligible = self.actors.get(&actor_id).is_some_and(|a| {
            a.has_passive_feature(PROTECTIVE_SPIRIT_TAG)
                && a.is_combat_active()
                && !a.is_incapacitated()
                // RAW's "fewer than half of your hit points remaining"
                // — the strict rung, not the Bloodied one, so a paladin
                // sitting on exactly half gets nothing.
                && a.is_below_half_hitpoints()
        });
        if !eligible {
            return;
        }
        // Half the paladin's level, rounded down per RAW's "half your
        // paladin level". Read off the chassis rather than pinned, so a
        // template that changes level changes the heal with it.
        let half_level = self.actors.get(&actor_id).map_or(0, |a| a.level() / 2);
        let rolled = self.roll(&Dice::new(1, 6));
        let healed = rolled + half_level;
        let name = self.actor_name(actor_id);
        if let Some(a) = self.actors.get_mut(&actor_id) {
            a.heal(healed);
        }
        self.log(format!(
            "  protective spirit: {} knits back 1d6({}){:+} = {} HP.",
            name, rolled, half_level, healed
        ));
    }

    /// 5e **Sunlight Hypersensitivity** (vampire, vampire spawn): "the
    /// vampire takes 20 radiant damage when it starts its turn in
    /// sunlight."
    ///
    /// The top tier of `SunlightFrailty`, and the only one of the three
    /// that does anything on its own schedule — the other two are
    /// modifiers read at a roll. Routed through `DealDamage` rather than
    /// a bare HP subtraction so the vampire's own resistances,
    /// concentration checks and death handling all fire exactly as they
    /// would for a swing: 20 radiant is enough to matter, and a rule
    /// that skipped the pipeline would be a rule that skipped the
    /// concentration save it should break.
    ///
    /// Inert on any board that is not under an open sky, which is every
    /// board unless somebody asked for daylight. That is the correct
    /// shape for it: a vampire in its crypt is simply a vampire.
    fn apply_sunlight_hypersensitivity(&mut self, actor_id: usize) {
        use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};
        let Some((frailty, burning)) = self.actors.get(&actor_id).and_then(|a| {
            let frailty = a.sunlight_frailty()?;
            Some((frailty, a.is_combat_active() && self.is_sunlit(a.location())))
        }) else {
            return;
        };
        let amount = frailty.start_of_turn_radiant();
        if !burning || amount == 0 {
            return;
        }
        let name = self.actor_name(actor_id);
        self.log(format!(
            "  {}: {} sears in the open sun for {} radiant.",
            frailty.label(),
            name,
            amount
        ));
        DealDamage {
            actor_id,
            amount,
            damage_type: DamageType::Radiant,
        }
        .apply(self);
    }

    /// Advance the initiative queue and fire `round_end` if the queue
    /// wrapped back to the first actor. Use this everywhere instead of
    /// `initiative_tracker.advance()` directly so condition timers,
    /// concentration saves, etc. all run at the right moment.
    fn advance_initiative(&mut self) {
        // 5e legendary actions: "only at the end of another creature's
        // turn". Read the slot *before* the queue moves, so the
        // dispatcher below knows whose turn just closed and can leave
        // that creature out — a dragon does not spend legendary actions
        // at the end of its own turn.
        let ended = self.initiative_tracker.current_player();
        // The slot moved, so whoever lands in it has not had their turn
        // opened yet — even when the queue has a single actor and the
        // "move" lands back on the same id. `ensure_turn_started` reads
        // this to decide whether a prompt needs a turn start first.
        self.turn_started_for = None;
        let wrapped = self.initiative_tracker.advance();
        // Dispatched *after* the advance, and the order is load-bearing
        // rather than stylistic. A legendary action can kill, and
        // killing sweeps the initiative queue: `remove_actor` leaves
        // `curr_index` where it was, so the slot behind the dead one
        // slides into it. Run before `advance`, a boss that finished
        // off the creature whose turn had just ended would have the
        // *next* creature slide into that index and then be advanced
        // straight past — one combatant silently losing a turn for
        // every boss kill. `dispatch_lair_actions` avoids the same trap
        // by reading its slot after dispatching rather than before.
        //
        // Ahead of `round_end` below, so a creature a legendary action
        // drops is swept by the round's own cleanup rather than lying
        // on the board for a tick.
        self.dispatch_legendary_actions(ended);
        if wrapped {
            self.round = self.round.saturating_add(1);
            // Thief's Reflexes is scoped to round 1, so its extra slots
            // retire the moment the queue wraps out of it. Swept before
            // `round_end` rather than after because `round_end` ends in
            // `cleanup_dead_actors`, which walks the queue removing
            // corpses — there is no reason to make it walk past slots
            // that are already spent.
            for id in self.initiative_tracker.clear_extra_turns() {
                let name = self.actor_name(id);
                self.log(format!("{}'s reflexes settle back to one turn a round.", name));
            }
            self.round_end();
            // After `round_end`, so the round's own damage, timers and
            // sweeps are all paid before the fight is asked whether it
            // got anywhere. See `note_attrition_progress`.
            self.note_attrition_progress();
            // The new round opens with whatever the place has to say.
            // After `round_end` rather than before, so the lair acts on
            // a board that has already paid out its timers and swept its
            // dead — a lair action that catches a creature the round
            // just killed would be resolving against a corpse.
            self.dispatch_lair_actions();
        }
    }

    /// 5e **legendary actions**: let every boss on the board take one
    /// option at the end of `ended`'s turn.
    ///
    /// ```text
    /// Only one legendary action option can be used at a time and only
    /// at the end of another creature's turn.
    /// ```
    ///
    /// Both halves of that sentence are enforced here. *One at a time*
    /// is one option per creature per call. *Another creature's turn*
    /// is the `ended` exclusion — a creature does not act at the end of
    /// its own turn, which is also what stops a solo boss on an empty
    /// initiative queue from acting twice for every turn it takes.
    ///
    /// **Every boss, not one.** Deliberately unlike
    /// `dispatch_lair_actions`, which picks a single resident: a lair
    /// belongs to a *place* and two lairs on one board is a situation
    /// the rules don't describe, but two legendary creatures in one
    /// fight is an ordinary encounter and each of them has its own
    /// budget. Sorted by id so a seeded replay resolves them in the
    /// same order.
    ///
    /// **The budget gates it, and so does the creature's state.**
    /// `can_consume_resource` already refuses a creature whose action
    /// economy is blocked, which is RAW's "can't take legendary actions
    /// while incapacitated or otherwise unable to take actions" — and
    /// it is the difference from a lair action, which fires on behalf
    /// of a paralyzed dragon because the cave is what is acting.
    ///
    /// **Affordability is checked per option.** RAW prices the good
    /// options at two and three points, so a creature with one point
    /// left is offered only the cheap half of its list, and a creature
    /// whose whole list is out of reach simply stops.
    fn dispatch_legendary_actions(&mut self, ended: Option<usize>) {
        let mut bosses: Vec<usize> = self
            .actors
            .iter()
            .filter(|(id, a)| {
                Some(**id) != ended
                    && a.is_combat_active()
                    && !a.legendary_actions().is_empty()
            })
            .map(|(id, _)| *id)
            .collect();
        if bosses.is_empty() {
            return;
        }
        bosses.sort_unstable();
        for boss_id in bosses {
            // Re-read liveness each time round: an earlier boss's
            // option can have killed a later one, and the id list was
            // taken before any of them acted.
            let Some(actor) = self.actors.get(&boss_id) else {
                continue;
            };
            if !actor.is_combat_active()
                || !actor.can_consume_resource(crate::engine::side_effects::Resource::LegendaryAction)
            {
                continue;
            }
            // Nothing to act *against* is not a reason to burn points.
            // Every option on every list either swings at somebody or
            // bursts around them, so a board with no enemies left is a
            // board where the whole repertoire is a no-op that costs.
            if !self.has_living_enemy_of(boss_id) {
                continue;
            }
            let (repertoire, budget) = (actor.legendary_actions(), actor.legendary_action_slots());
            let affordable: Vec<usize> = (0..repertoire.len())
                .filter(|&i| repertoire[i].cost <= budget)
                .collect();
            let Some(&index) = affordable.get(self.roll_index(affordable.len())) else {
                continue;
            };
            let entry = &repertoire[index];
            let name = self.actor_name(boss_id);
            self.log(format!("[legendary] {}: {}.", name, entry.name));
            if let Some(a) = self.actors.get_mut(&boss_id) {
                for _ in 0..entry.cost {
                    a.consume_resource(crate::engine::side_effects::Resource::LegendaryAction);
                }
            }
            (entry.fire)(self, boss_id);
            self.cleanup_dead_actors();
        }
    }

    /// True if anybody hostile to `actor_id` is still standing.
    ///
    /// The cheap "is there anything to do" pre-check the legendary
    /// dispatcher makes before it spends a point. Deliberately not a
    /// distance test: the strides and the ranged options both reach
    /// across the map, so the question that saves the point is whether
    /// there is an enemy at all.
    fn has_living_enemy_of(&self, actor_id: usize) -> bool {
        let Some(team) = self.actors.get(&actor_id).map(|a| a.team()) else {
            return false;
        };
        self.actors
            .values()
            .any(|a| a.team() != team && a.is_combat_active())
    }

    /// 5e **lair actions**: fire one, once per round, on behalf of one
    /// creature that has a lair.
    ///
    /// RAW puts these on initiative count 20, losing ties. We put them
    /// at the top of the round, which is the same place for every
    /// purpose the engine can observe: nothing in the initiative order
    /// is allowed to interleave with them either way, and "count 20" is
    /// a scheduling convention for a table that reads its initiative
    /// list aloud.
    ///
    /// **One resident.** A lair belongs to a creature, and two creatures
    /// with lairs on the same board is a situation the rules don't
    /// describe — so the lowest-id combat-active resident acts and the
    /// rest are guests in someone else's cave. Deliberately *not* "each
    /// of them in turn": three legendary residents firing three lair
    /// actions a round would be three times the rules' budget for the
    /// same rules text.
    ///
    /// **Not twice running.** RAW: "the creature can't use the same lair
    /// action two rounds in a row." Honored by excluding last round's
    /// index from the draw, which is also why a one-entry lair simply
    /// repeats — there is nothing else for it to do.
    ///
    /// The resident's own state gates nothing but life: a lair action
    /// fires while its resident is stunned, paralyzed, or unconscious,
    /// because the lair is what is acting. Death ends it — `Legendary
    /// Actions` are the creature's, lair actions are the place's, and
    /// the place stops answering when nobody is left to answer to.
    fn dispatch_lair_actions(&mut self) {
        if self.lair_acted_round == Some(self.round) {
            return;
        }
        self.lair_acted_round = Some(self.round);
        let mut residents: Vec<usize> = self
            .actors
            .iter()
            .filter(|(_, a)| a.is_combat_active() && !a.lair_actions().is_empty())
            .map(|(id, _)| *id)
            .collect();
        residents.sort_unstable();
        let Some(&resident_id) = residents.first() else {
            return;
        };
        let (repertoire, last) = match self.actors.get(&resident_id) {
            Some(a) => (a.lair_actions(), a.last_lair_action()),
            None => return,
        };
        // Draw from everything except last round's pick. A one-entry
        // lair has nothing else to offer and repeats.
        let eligible: Vec<usize> = (0..repertoire.len())
            .filter(|i| repertoire.len() == 1 || Some(*i) != last)
            .collect();
        let Some(&index) = eligible.get(self.roll_index(eligible.len())) else {
            return;
        };
        let entry = &repertoire[index];
        let resident_name = self.actor_name(resident_id);
        self.log(format!(
            "[lair] {}'s lair stirs: {}.",
            resident_name, entry.name
        ));
        if let Some(a) = self.actors.get_mut(&resident_id) {
            a.set_last_lair_action(index);
        }
        (entry.fire)(self, resident_id);
        self.cleanup_dead_actors();
    }

    /// A uniformly random index into a collection of `len` items, drawn
    /// off the encounter's seeded roller so a replay of the same seed
    /// makes the same choice. Returns 0 for an empty collection, which
    /// every caller then fails to index — the same shape as asking a
    /// `Vec` for `[0]`.
    fn roll_index(&mut self, len: usize) -> usize {
        if len <= 1 {
            return 0;
        }
        (self.roll(&Dice::new(1, len as u32)) as usize).saturating_sub(1)
    }

    /// Open the current initiative slot's turn if it hasn't been opened
    /// yet. Idempotent: repeated calls within one turn are no-ops, which
    /// is what lets `process_stack` call this before every prompt
    /// without resetting the resources of an actor mid-turn.
    ///
    /// This is the one place that guarantees the engine's
    /// "every prompted actor has had `start_turn_for` run" invariant.
    /// The two explicit `advance_initiative` + `start_turn_for` pairs
    /// (`skip_turn`, and the dying-actor sweep in `process_stack`) stay
    /// as they are — they set the latch, so this call sees nothing to
    /// do. What it catches is the slot changing *without* an advance:
    /// the encounter's very first actor, and the actor who slides into
    /// the index of someone who died on their own turn.
    fn ensure_turn_started(&mut self) {
        // The lair acts before anything in the round does — before the
        // current slot's turn opens, and before the id of whoever is in
        // that slot is read. Reached from here as well as from the
        // initiative wrap because round one never wraps into itself:
        // without this call a dragon's cave would sit silent through the
        // whole opening round. The round guard inside makes the second
        // caller free.
        //
        // Read the slot *after* the dispatch rather than before, because
        // a lair action can kill, and killing sweeps the initiative
        // queue — an id captured first could name a creature the lair
        // has since removed, and opening a turn for it would leave the
        // latch unset and this function with nothing to repair it.
        self.dispatch_lair_actions();
        let Some(curr_id) = self.initiative_tracker.current_player() else {
            return;
        };
        if self.turn_started_for == Some(curr_id) {
            return;
        }
        self.start_turn_for(curr_id);
    }

    /// 1-indexed encounter round counter. UI surfaces this so the
    /// player can see "round N" in the side panel and timer-driven
    /// effects can key off the absolute round number.
    pub fn round(&self) -> u32 {
        self.round
    }

    /// The seed this encounter's dice and terrain came from. Passing it
    /// back as the binary's seed argument reproduces the encounter
    /// exactly, whether or not the original run named one.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Clear any Help grant on `actor_id`. No-op if the actor is missing
    /// or had no grant.
    pub fn consume_help(&mut self, actor_id: usize) {
        if let Some(actor) = self.actors.get_mut(&actor_id) {
            actor.set_help_grant(None);
        }
    }

    /// Bless / Bane d4 modifier for an attack roll. Bless rolls +1d4,
    /// Bane rolls -1d4. Both: they cancel and we return (0, ""). Returns
    /// the rolled total and a log suffix to embed in the attack log.
    /// The roll uses the encounter's seedable roller for reproducibility.
    /// Log-friendly cover suffix matching the integer returned by
    /// `cover_ac_bonus`: "" for no cover, " (half cover)" for +2,
    /// " (three-quarters cover)" for +5. Shared between weapon and spell
    /// attack-roll log lines so the two paths can't drift.
    pub fn cover_log_suffix(cover_bonus: i32) -> &'static str {
        match cover_bonus {
            Self::HALF_COVER_AC => " (half cover)",
            Self::THREE_QUARTERS_COVER_AC => " (three-quarters cover)",
            _ => "",
        }
    }

    /// Sum the caster-side flat attack-roll bonuses that ride every
    /// attack roll (weapon or spell): the install-side `attack_bonus_buff`
    /// ledger (Bless's AdjustAttackBuff(+2), etc.) and the read-side
    /// `condition_attack_bonus` flag table (Sacred Weapon's +CHA,
    /// Bardic Inspiration's +3). Returns (install_buff, condition_buff)
    /// — two lanes so callers can keep the log breakdown if they want
    /// to. Missing actor returns `(0, 0)`.
    ///
    /// The install-buff lane also folds in the carried-item
    /// `attack_bonus` (`+1 Weapon`, Bracers of Archery, Ioun Stone of
    /// Mastery, …) so a single chokepoint handles every flat to-hit
    /// source. Mirrors `item_save_bonus`'s seat in `roll_save`.
    pub fn caster_attack_buffs(&self, caster_id: usize) -> (i32, i32) {
        self.actors
            .get(&caster_id)
            .map(|a| {
                (
                    a.attack_bonus_buff() + a.item_attack_bonus(),
                    a.condition_attack_bonus(),
                )
            })
            .unwrap_or((0, 0))
    }

    /// Sum the caster-side flat damage-roll bonuses that ride every
    /// damage roll (weapon or spell): the item-passive `damage_bonus`
    /// lane (`+1 Weapon` / Bracers of Archery) and the spell-installed
    /// `damage_bonus_buff` lane (Magic Weapon / Elemental Weapon).
    /// Folded at the damage-roll site in `engine::attack` and the
    /// spell-attack chokepoint so a single chokepoint handles every
    /// flat damage source. Missing actor returns 0. Symmetric with
    /// `caster_attack_buffs` on the to-hit lane.
    pub fn caster_damage_buffs(&self, caster_id: usize) -> i32 {
        self.actors
            .get(&caster_id)
            .map(|a| a.item_damage_bonus() + a.damage_bonus_buff())
            .unwrap_or(0)
    }

    /// Conditions that each add their own **1d4** to a d20 total —
    /// attack rolls, spell attack rolls, and saving throws alike, since
    /// all three route through `bless_bane_attack_die`.
    ///
    /// `(condition, log label)`. The label is what the roll breakdown
    /// names the die as, so a player reading the log can tell which
    /// buff paid for the hit.
    ///
    /// Two rows, from two classes and two cadences:
    ///
    ///   - **Bless** — the concentration spell. Its *other* half, the
    ///     blanket advantage on saving throws, lives on
    ///     `BLANKET_SAVE_ADVANTAGE_CONDITIONS`; only the die is here.
    ///   - **Emboldening Bond** — the Peace Domain Cleric's level-1
    ///     feature, which is the die and nothing else.
    ///
    /// That split is the reason this is a cohort rather than a pair of
    /// hardcoded flags: the two features overlap on one clause and
    /// diverge on another, and a second use of `Condition::Blessed`
    /// would have silently handed the domain feature the clause it
    /// doesn't have.
    const D4_BONUS_CONDITIONS: &'static [(Condition, &'static str)] =
        &[(Condition::Blessed, "bless"), (Condition::Emboldened, "bond")];

    /// The mirror cohort — conditions that each *subtract* a 1d4.
    ///
    /// One row, Bane, and the symmetry is the point: a creature under
    /// Bless and Bane at once nets zero dice, which is this engine's
    /// long-standing simplification of RAW (where both would roll and
    /// the results would rarely cancel exactly). Generalizing the pair
    /// into two lists keeps that behaviour exactly — one bonus source
    /// and one penalty source still cancel — while letting a second
    /// bonus source stack the way two independent RAW buffs should.
    const D4_PENALTY_CONDITIONS: &'static [(Condition, &'static str)] =
        &[(Condition::Baned, "bane")];

    /// Net the actor's 1d4 buff / debuff sources and roll the
    /// difference, returning the signed total and a log fragment.
    ///
    /// **The** chokepoint for the d4 lane: weapon attacks
    /// (`resolve_attack`), spell attack rolls (`spells::spell_attack`)
    /// and every saving throw (`roll_save`) all land here, which is why
    /// a new d4 source is a row on one of the two cohorts above and
    /// nothing else.
    ///
    /// Netting *before* rolling rather than rolling each source and
    /// summing is what preserves the engine's Bless-and-Bane-cancel
    /// rule; see `D4_PENALTY_CONDITIONS`. The name predates the cohorts
    /// and is kept because a rename would touch every call site for no
    /// behavioural gain.
    pub fn bless_bane_attack_die(&mut self, actor_id: usize) -> (i32, String) {
        let Some(actor) = self.actors.get(&actor_id) else {
            return (0, String::new());
        };
        let held = |cohort: &'static [(Condition, &'static str)]| -> Vec<&'static str> {
            cohort
                .iter()
                .filter(|(c, _)| actor.has_condition(*c))
                .map(|(_, label)| *label)
                .collect()
        };
        let bonuses = held(Self::D4_BONUS_CONDITIONS);
        let penalties = held(Self::D4_PENALTY_CONDITIONS);
        let net = bonuses.len() as i32 - penalties.len() as i32;
        if net == 0 {
            return (0, String::new());
        }
        let count = net.unsigned_abs();
        let (sign, labels) = if net > 0 {
            ('+', bonuses)
        } else {
            ('-', penalties)
        };
        let rolled = self.roll(&Dice::new(count, 4)) as i32;
        // Name only as many sources as there are surviving dice. A
        // creature under Bless, the bond and Bane rolls one die, and a
        // breakdown reading "bless+bond(1d4=3)" would claim two sources
        // paid for it. Which of the survivors gets named is arbitrary
        // — they are the same die — so the first is as good as any.
        let named = &labels[..(count as usize).min(labels.len())];
        (
            net.signum() * rolled,
            format!(" {} {}({}d4={})", sign, named.join("+"), count, rolled),
        )
    }

    /// 5e RAW: making an attack consumes the attacker's one-shot advantage
    /// stack — Hidden drops (attacking reveals you), Helped drops (Help is
    /// once-per-target), any per-target help grant is consumed, and
    /// concentration on Invisibility ends (attacking ends invisibility).
    /// Symmetric across weapon attacks (`resolve_attack`) and spell
    /// attacks (`spell_attack_outcome` in spells.rs) so a follow-up swing
    /// in the same turn doesn't double-dip the rider.
    ///
    /// New one-shot attack-roll riders (next-attack-only conditions like
    /// Bardic Inspiration's `Inspired` or Battle Master Precision
    /// Attack's `PrecisionAttacking`) add their condition to
    /// `CONSUMED_ON_ATTACK` below — the iteration handles the rest.
    pub fn clear_attack_advantage_riders(&mut self, caster_id: usize, target_id: usize) {
        if let Some(attacker) = self.actors.get_mut(&caster_id) {
            for c in CONSUMED_ON_ATTACK {
                attacker.remove_condition(*c);
            }
            attacker.consume_help_for(target_id);
        }
        // Target-side one-shots whose owner is this attacker — see
        // `CONSUMED_BY_LINKED_ATTACKER`. Read the link before removing,
        // because `remove_condition` drops it with the flag.
        if let Some(target) = self.actors.get_mut(&target_id) {
            for c in CONSUMED_BY_LINKED_ATTACKER {
                if target.linked_by(*c) == Some(caster_id) {
                    target.remove_condition(*c);
                }
            }
        }
        // Concentration spells that explicitly break on attack (Invisibility,
        // not Greater Invisibility) drop here. Flag-based to avoid the
        // fragile spell-name string check; see ConcentrationData::breaks_on_attack.
        if self
            .actors
            .get(&caster_id)
            .and_then(|a| a.concentration())
            .is_some_and(|c| c.breaks_on_attack)
        {
            self.drop_concentration(caster_id);
        }
    }

    /// The bard whose **Unfailing Inspiration** would let `actor_id`
    /// keep the Bardic Inspiration die they are about to spend, or
    /// `None` — because they hold no die, because nobody is on record
    /// as having granted it, or because whoever did is an ordinary
    /// bard.
    ///
    /// Read *before* the roll's one-shot riders clear, because the
    /// answer lives on `Inspired`'s back-link and `remove_condition`
    /// drops the link with the flag. Every caller therefore looks like
    /// "capture, clear, roll, refund on failure", which is also the
    /// order RAW describes: the die is spent, the roll is made, and
    /// only then does the bard's feature decide whether it comes back.
    ///
    /// `linked_by` already returns `None` for a condition the actor
    /// isn't holding, so a lone `Some` here means all three of "holds a
    /// die", "knows who gave it" and "they have the feature".
    pub fn unfailing_inspiration_granter(&self, actor_id: usize) -> Option<usize> {
        let granter = self
            .actors
            .get(&actor_id)?
            .linked_by(Condition::Inspired)?;
        self.actors
            .get(&granter)
            .is_some_and(|b| {
                b.has_passive_feature(
                    crate::actions::class_features::UNFAILING_INSPIRATION_TAG,
                )
            })
            .then_some(granter)
    }

    /// Hand a spent Bardic Inspiration die back after the roll it paid
    /// for failed anyway (5e College of Eloquence, **Unfailing
    /// Inspiration**).
    ///
    /// Re-installs the flag *and* the back-link, so the returned die is
    /// the same die: it can be spent again, fail again, and come back
    /// again. That is RAW — the feature has no per-encounter cap and no
    /// clause that stops it repeating — and it is the whole reason the
    /// Eloquence bard's die is worth more than anyone else's.
    ///
    /// The refresh deliberately restates the ten-round timer rather
    /// than preserving whatever was left of it. RAW's die lives for an
    /// hour; ten rounds is the engine's stand-in for "the rest of this
    /// fight", and a die that came back with two rounds on the clock
    /// would be a worse version of a feature whose point is that it
    /// doesn't run out.
    pub fn refund_unfailing_inspiration(&mut self, actor_id: usize, granter: usize) {
        let Some(actor) = self.actors.get_mut(&actor_id) else {
            return;
        };
        actor.add_condition(
            Condition::Inspired,
            crate::conditions::ConditionTimer::Rounds(10),
        );
        actor.set_condition_link(Condition::Inspired, Some(granter));
        let name = self.actor_name(actor_id);
        let bard = self.actor_name(granter);
        self.log(format!(
            "  unfailing inspiration: {} keeps {}'s die.",
            name, bard
        ));
    }

    /// 5e Tasha's Sorcerer Seeking Spell metamagic — if the caster has the
    /// prime up, reroll the d20 with the same mode and consume the prime.
    /// Returns the (possibly higher) d20 face the caller should use.
    /// Idempotent when no prime is up: the original `raw` value falls
    /// through unchanged. Called from `spell_attack_outcome` on a miss so
    /// the rider only fires when the original swing actually whiffed —
    /// RAW: "When you make an attack roll for a spell, you can spend 2
    /// sorcery points to reroll it." We honor the "must use the new roll"
    /// clause by replacing `raw` unconditionally on consume; the engine's
    /// `roll_d20_lucky` reroll path stays available on the new die (a
    /// nat-1 reroll still chains through Lucky if the caster has it).
    pub fn reroll_seeking_spell(&mut self, caster_id: usize, raw: u32, mode: RollMode) -> u32 {
        let primed = self
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_condition(Condition::SeekingSpelling));
        if !primed {
            return raw;
        }
        let new_raw = self.roll_d20_lucky(caster_id, mode);
        let name = self.actor_name(caster_id);
        if let Some(caster) = self.actors.get_mut(&caster_id) {
            caster.remove_condition(Condition::SeekingSpelling);
        }
        self.log(format!(
            "  seeking spell: {} rerolls {} \u{2192} {}",
            name, raw, new_raw
        ));
        new_raw
    }

    /// 5e Sorcerer Distant Spell metamagic — consume the prime on the
    /// caster (if present) and log it. Called from `Action::execute` for
    /// any ranged action (reach > 2) right after validation. Idempotent
    /// when no prime is up — returns silently. Mirrors the consume-on-
    /// trigger pattern used by Empowered / Heightened Spell at the
    /// roll-site chokepoints (`roll_empowered`, `roll_save_against_caster`).
    pub fn consume_distant_spell(&mut self, caster_id: usize) {
        let Some(caster) = self.actors.get_mut(&caster_id) else {
            return;
        };
        if !caster.has_condition(Condition::DistantSpelling) {
            return;
        }
        let name = caster.name().to_string();
        caster.remove_condition(Condition::DistantSpelling);
        self.log(format!(
            "  distant spell: {} unfurls the range bonus", name
        ));
    }

    /// Pick the second creature a single-target spell should also land
    /// on, for the features that re-fire a cast against one extra
    /// target: the Sorcerer's **Twinned Spell** metamagic and the
    /// Enchantment Wizard's **Split Enchantment**.
    ///
    /// The heuristic mirrors the AI's own targeting lanes so a doubled
    /// cast picks the target a player would: harmful spells take the
    /// **nearest** eligible enemy (focus-fire), buffs and heals take the
    /// **lowest-HP** eligible ally (heal-the-weakest). Ties break on
    /// actor id so a seeded run is reproducible. The original target is
    /// always excluded — RAW's "a second creature" is a different one —
    /// as are the caster and anyone not combat-active.
    ///
    /// `reach` and `requires_los` come from the action being doubled, so
    /// the second target has to sit inside the same envelope the first
    /// one did. A `None` reach means the action does its own range
    /// logic and the distance gate is skipped.
    ///
    /// `max_gap_from_original` adds a second envelope, measured from the
    /// first target rather than from the caster. Twinned Spell and Split
    /// Enchantment pass `None` — RAW lets the second creature stand
    /// anywhere in range. The Death Domain's Reaper passes `Some(1)`,
    /// because its RAW clause is "two creatures within 5 feet of each
    /// other", which is a constraint on the pair, not on the caster's
    /// reach.
    ///
    /// Extracted from `consume_twinned_spell`, which used to own this
    /// walk inline. The callers differ on everything *around* the
    /// pick — a consumable prime paid for in sorcery points, an
    /// always-on passive gated on the spell's school, a domain feature
    /// gated on the two targets being neighbours — and on nothing about
    /// the pick itself, which is why it is worth exactly one copy.
    fn pick_second_spell_target(
        &self,
        caster_id: usize,
        is_harmful: bool,
        reach: Option<isize>,
        requires_los: bool,
        original_target_id: usize,
        max_gap_from_original: Option<isize>,
    ) -> Option<usize> {
        let caster = self.actors.get(&caster_id)?;
        let caster_team = caster.team();
        let caster_loc = caster.location();
        let caster_size = crate::engine::util::get_tiles_from_size(caster.size());
        let mut candidates: Vec<(usize, isize, u32)> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if *id == caster_id || *id == original_target_id {
                    return None;
                }
                if !a.is_combat_active() {
                    return None;
                }
                // Harmful → opposite team; non-harmful (buff/heal) → same team.
                if (a.team() == caster_team) == is_harmful {
                    return None;
                }
                let dist = crate::engine::util::footprint_chebyshev(
                    caster_loc,
                    caster_size,
                    a.location(),
                    crate::engine::util::get_tiles_from_size(a.size()),
                );
                if let Some(r) = reach
                    && dist > r
                {
                    return None;
                }
                if requires_los && !self.actor_has_line_of_sight(caster_id, *id) {
                    return None;
                }
                if let Some(pair_gap) = max_gap_from_original {
                    let original = self.actors.get(&original_target_id)?;
                    let gap = crate::engine::util::footprint_chebyshev(
                        original.location(),
                        crate::engine::util::get_tiles_from_size(original.size()),
                        a.location(),
                        crate::engine::util::get_tiles_from_size(a.size()),
                    );
                    if gap > pair_gap {
                        return None;
                    }
                }
                Some((*id, dist, a.hitpoints()))
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        if is_harmful {
            candidates.sort_unstable_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        } else {
            candidates.sort_unstable_by(|a, b| a.2.cmp(&b.2).then(a.0.cmp(&b.0)));
        }
        Some(candidates[0].0)
    }

    /// 5e Sorcerer Twinned Spell metamagic — if the caster has the prime
    /// up, return a second target id to re-fire the action against, plus
    /// the SP cost charged for the twin. Returns `None` when the prime
    /// isn't up, when the action isn't twinnable (multi-target schemas,
    /// self-only actions with no target), when no suitable second target
    /// exists, or when the caster can't afford the SP cost. The caller is
    /// expected to re-run the action's `side_effects` against the returned
    /// id; the prime + SP are consumed inside this call so the second
    /// invocation can't observe a "still primed" caster.
    ///
    /// RAW: cost is `max(1, spell_level)` sorcery points; cantrips cost 1.
    /// Spell level is sniffed from the action's resolved cost — any
    /// `SpellSlot(lvl)` entry sets the SP debit, otherwise we default to
    /// 1 (cantrip case). The picker uses the same heuristic as the AI's
    /// focus-fire / heal-lowest lanes: harmful actions pick the nearest
    /// opposing-team combat-active actor that isn't the original target;
    /// non-harmful actions pick the lowest-HP allied combat-active actor
    /// other than the original target.
    #[allow(clippy::too_many_arguments)]
    pub fn consume_twinned_spell(
        &mut self,
        caster_id: usize,
        action_name: &str,
        is_harmful: bool,
        reach: Option<isize>,
        requires_los: bool,
        sp_cost: u32,
        original_target_id: usize,
    ) -> Option<usize> {
        let caster = self.actors.get(&caster_id)?;
        if !caster.has_condition(Condition::TwinnedSpelling) {
            return None;
        }
        if caster.sorcery_points() < sp_cost {
            return None;
        }
        let caster_name = caster.name().to_string();
        let twin_id = self.pick_second_spell_target(
            caster_id,
            is_harmful,
            reach,
            requires_los,
            original_target_id,
            None,
        )?;
        let twin_name = self.actor_name(twin_id);
        // Consume prime + SP atomically. Spending SP can fail in principle
        // (race with another mutation), so guard with a re-check before
        // returning the id — we can't unscramble a "no target" path if SP
        // was already debited.
        let caster_mut = self.actors.get_mut(&caster_id)?;
        if !caster_mut.spend_sorcery_points(sp_cost) {
            return None;
        }
        caster_mut.remove_condition(Condition::TwinnedSpelling);
        let sp_left = caster_mut.sorcery_points();
        self.log(format!(
            "  twinned spell: {} echoes {} onto {} ({} SP, {} SP left)",
            caster_name,
            action_name,
            twin_name,
            sp_cost,
            sp_left,
        ));
        Some(twin_id)
    }

    /// 5e Enchantment Wizard **Split Enchantment** (subclass level 6):
    /// if the caster holds the feature and the in-flight spell is an
    /// enchantment of 1st level or higher that named exactly one
    /// creature, return a second creature to re-fire it against.
    ///
    /// RAW: "When you cast an enchantment spell of 1st level or higher
    /// that targets only one creature, you can have it target a second
    /// creature." No resource attached — it is a passive that simply
    /// doubles every single-target enchantment the enchanter casts, and
    /// that is what makes the subclass: a Hold Person that lands on two
    /// creatures for one 2nd-level slot, a Dominate Person that takes
    /// two, a Bless on two allies.
    ///
    /// Shares `pick_second_spell_target` with the Sorcerer's Twinned
    /// Spell — the two features want the same creature for the same
    /// reasons, and differ only in what they cost and what they gate
    /// on. Where Twinned Spell is a consumable prime charging
    /// `max(1, level)` sorcery points and covering any single-target
    /// spell including cantrips, Split Enchantment is free, always on,
    /// and restricted to leveled enchantments. A caster somehow holding
    /// both doubles once, not twice: `execute` tries the paid prime
    /// first and only falls through to the free passive if the prime
    /// wasn't up, so the sorcery points are never spent on something
    /// the passive would have covered anyway.
    ///
    /// The cantrip exclusion is RAW and load-bearing here: it is what
    /// keeps Vicious Mockery / Mind Sliver from becoming free
    /// double-taps at will.
    // Same flat argument list as `consume_twinned_spell` — the two are
    // called side by side from one `.or_else` chain in `Action::execute`
    // and every argument is read straight off the action, so keeping the
    // shapes identical is worth more than the lint.
    #[allow(clippy::too_many_arguments)]
    pub fn consume_split_enchantment(
        &mut self,
        caster_id: usize,
        action_name: &str,
        school: Option<SpellSchool>,
        spell_level: u32,
        is_harmful: bool,
        reach: Option<isize>,
        requires_los: bool,
        original_target_id: usize,
    ) -> Option<usize> {
        use crate::actions::class_features::SPLIT_ENCHANTMENT_TAG;
        if school != Some(SpellSchool::Enchantment) || spell_level == 0 {
            return None;
        }
        let caster = self.actors.get(&caster_id)?;
        if !caster.has_passive_feature(SPLIT_ENCHANTMENT_TAG) {
            return None;
        }
        let caster_name = caster.name().to_string();
        let second_id = self.pick_second_spell_target(
            caster_id,
            is_harmful,
            reach,
            requires_los,
            original_target_id,
            None,
        )?;
        let second_name = self.actor_name(second_id);
        self.log(format!(
            "  split enchantment: {} splits {} onto {}",
            caster_name, action_name, second_name
        ));
        Some(second_id)
    }

    /// 5e Death Domain Cleric **Reaper** (subclass level 1) — the third
    /// and last member of the cast-doubling family, and the only one
    /// that fires on cantrips.
    ///
    /// RAW: "when you learn a necromancy cantrip that targets only one
    /// creature, the spell can instead target two creatures within 5
    /// feet of each other." Always on, no charge, no resource — which is
    /// why it is last in the `.or_else` chain in `Action::execute`: a
    /// cleric who somehow held a paid prime should spend that first, and
    /// a free passive is never the thing you regret not using.
    ///
    /// Two gates distinguish it from its siblings. It is cantrips *only*
    /// (`spell_level == 0`), where Split Enchantment is leveled spells
    /// only — the exact inverse, and both for the same reason: a free
    /// doubling has to be restricted to one tier or it dominates the
    /// other. And the second creature has to be within 5 ft of the
    /// *first*, not merely in the caster's range, which is the pair
    /// constraint `pick_second_spell_target`'s `max_gap_from_original`
    /// exists for. A Death cleric wants two enemies standing together,
    /// and gets nothing from a battlefield that has spread out.
    // Same flat argument list as its two siblings for the same reason —
    // all three are called from one `.or_else` chain and read every
    // argument straight off the action.
    #[allow(clippy::too_many_arguments)]
    pub fn consume_reaper(
        &mut self,
        caster_id: usize,
        action_name: &str,
        school: Option<SpellSchool>,
        spell_level: u32,
        is_harmful: bool,
        reach: Option<isize>,
        requires_los: bool,
        original_target_id: usize,
    ) -> Option<usize> {
        use crate::actions::class_features::REAPER_TAG;
        if school != Some(SpellSchool::Necromancy) || spell_level != 0 {
            return None;
        }
        let caster = self.actors.get(&caster_id)?;
        if !caster.has_passive_feature(REAPER_TAG) {
            return None;
        }
        let caster_name = caster.name().to_string();
        let second_id = self.pick_second_spell_target(
            caster_id,
            is_harmful,
            reach,
            requires_los,
            original_target_id,
            // RAW's "within 5 feet of each other" — one tile-gap on the
            // engine's 2.5 ft grid, measured between the two targets.
            Some(1),
        )?;
        let second_name = self.actor_name(second_id);
        self.log(format!(
            "  reaper: {}'s {} reaches {} as well",
            caster_name, action_name, second_name
        ));
        Some(second_id)
    }

    /// Resolve how much of a pre-rolled `raw` damage value actually
    /// lands on `target_id` given their save outcome — the single place
    /// the engine turns "they passed / they failed" into a number.
    ///
    /// Folds the two features that bend the standard save-for-half /
    /// save-for-nothing tables, in the order RAW composes them:
    ///
    ///   1. **Potent Cantrip** (Evocation Wizard lv6, caster side) —
    ///      upgrades a cantrip's `NoneOnSave` to `HalfOnSave`, so a
    ///      successful save leaves half standing.
    ///   2. **Evasion** (Rogue / Monk / Ranger, target side) — on DEX
    ///      saves, shifts `HalfOnSave` one notch the other way: pass
    ///      takes nothing, fail takes half.
    ///
    /// Running Potent Cantrip first is what makes the three-way case
    /// come out right: a rogue with Evasion who makes their DEX save
    /// against a potent Sacred Flame takes nothing, because Potent
    /// Cantrip lifts the effect into exactly the class Evasion zeroes.
    /// Doing it in the other order would leave the rogue eating half.
    ///
    /// Extracted from the two burst resolvers (`resolve_burst_targets`
    /// in `action_template.rs` and `burst_save_damage` in `spells.rs`)
    /// which each carried their own copy of the Evasion branch and its
    /// log line. Both now delegate here, so a future modifier on this
    /// lane — a Potent-Cantrip sibling, a "half again on a failed save"
    /// rider — lands once instead of twice.
    pub fn resolve_post_save_damage(
        &mut self,
        caster_id: usize,
        target_id: usize,
        save_ability: AbilityScoreType,
        policy: crate::engine::saves::SaveDamagePolicy,
        raw: u32,
        passed: bool,
    ) -> u32 {
        let policy = if self.potent_cantrip_applies(caster_id, policy) {
            let name = self.actor_name(caster_id);
            self.log(format!("  potent cantrip: {}'s cantrip still bites", name));
            policy.with_potent_cantrip()
        } else {
            policy
        };
        let Some(mitigation) = self.save_mitigation_for(target_id, save_ability) else {
            return policy.apply(raw, passed);
        };
        let dmg = policy.apply_mitigated(raw, passed, mitigation);
        if dmg == 0 && passed {
            let target_name = self.actor_name(target_id);
            self.log(format!(
                "  {}: {} takes no damage",
                mitigation.label(),
                target_name
            ));
        }
        dmg
    }

    /// Which of the target's standing effects, if any, improves what a
    /// made save leaves standing — returned as a `SaveMitigation` so
    /// the arithmetic and the log line both come from one answer.
    ///
    /// Two answers today, and they carve the space along different
    /// axes, which is why neither subsumes the other:
    ///
    ///   - **Evasion** (Rogue / Monk / Ranger): any Dexterity save,
    ///     whatever the damage came from — a dragon's breath, a
    ///     collapsing ceiling, a fireball.
    ///   - **Circle of Power**: any ability, but only against a spell.
    ///     The spell gate reads the in-flight cast frame, the same
    ///     marker Potent Cantrip reads a few lines up, so a Dexterity
    ///     save against a falling rock is not lifted by a paladin's
    ///     aura.
    ///
    /// A target holding both gets the union, which is what RAW's two
    /// independent sentences add up to.
    fn save_mitigation_for(
        &self,
        target_id: usize,
        save_ability: AbilityScoreType,
    ) -> Option<crate::engine::saves::SaveMitigation> {
        use crate::engine::saves::SaveMitigation;
        let target = self.actors.get(&target_id)?;
        // Evasion first: where the two overlap it is the stronger of
        // the pair, because it softens a failed save as well as
        // perfecting a made one.
        if save_ability == AbilityScoreType::Dexterity && target.has_evasion() {
            return Some(SaveMitigation::Evasion);
        }
        // 5e **Mounted Combatant**, third clause: "if your mount is
        // subjected to an effect that allows it to make a Dexterity
        // saving throw to take only half damage, it instead takes no
        // damage if it succeeds on the saving throw, and only half
        // damage if it fails." Word for word the Evasion table, granted
        // to the horse by the person sitting on it — so it belongs here
        // rather than as a fourth `SaveMitigation` variant that would
        // resolve identically. A rider who has been thrown grants
        // nothing: the link is what carries the feat down.
        if save_ability == AbilityScoreType::Dexterity
            && target
                .ridden_by()
                .and_then(|rider_id| self.actors.get(&rider_id))
                .is_some_and(|r| r.has_mounted_combatant())
        {
            return Some(SaveMitigation::Evasion);
        }
        if target.has_condition(Condition::PowerCircled)
            && self.current_cast().is_some_and(|c| c.school.is_some())
        {
            return Some(SaveMitigation::NoneOnSuccess);
        }
        None
    }

    /// Whether Potent Cantrip should upgrade `policy` on the cast
    /// currently in flight: the caster holds the feature, the cast is a
    /// cantrip, and the effect is one the upgrade can actually move
    /// (`NoneOnSave` — a `HalfOnSave` effect already leaves half
    /// standing, so firing there would log a no-op).
    ///
    /// The cantrip gate reads the in-flight cast's school and level
    /// rather than the policy, which matters twice: `NoneOnSave` also
    /// carries the handful of leveled spells that zero on a save
    /// (Disintegrate) and RAW must not lift those, and `Action::execute`
    /// opens a frame for non-spell actions too — so `is_cantrip`
    /// requires a school tag, which only spells carry.
    fn potent_cantrip_applies(
        &self,
        caster_id: usize,
        policy: crate::engine::saves::SaveDamagePolicy,
    ) -> bool {
        use crate::engine::saves::SaveDamagePolicy;
        policy == SaveDamagePolicy::NoneOnSave
            && self.current_cast().is_some_and(|c| c.is_cantrip())
            && self.actors.get(&caster_id).is_some_and(|a| {
                a.has_passive_feature(crate::actions::class_features::POTENT_CANTRIP_TAG)
            })
    }

    /// How many allies the Sorcerer's **Careful Spell** prime can spare
    /// on this cast, or `None` when the caster isn't holding it. RAW:
    /// "a number of those creatures up to your Charisma modifier
    /// (minimum of one creature)."
    ///
    /// Split out from `auto_pass_shielded_allies` so the AI can ask the
    /// same question *before* committing to a blast point — the
    /// friendly-fire gate needs the capacity, not the shielded set — and
    /// the two can't drift apart.
    fn careful_spell_capacity(&self, caster_id: usize) -> Option<usize> {
        let caster = self.actors.get(&caster_id)?;
        if !caster.has_condition(Condition::CarefulSpelling) {
            return None;
        }
        Some(
            caster
                .ability_modifier(crate::engine::types::AbilityScoreType::Charisma)
                .max(1) as usize,
        )
    }

    /// How many allies the Evocation Wizard's **Sculpt Spells** can carve
    /// out of a `school`-school, `level`-level cast, or `None` when the
    /// feature doesn't apply. RAW: "a number of them equal to 1 + the
    /// spell's level", evocation only — so a cantrip spares one ally and
    /// a Fireball four.
    ///
    /// Takes the school and level explicitly rather than reading the cast
    /// stack, because the AI's only useful call site is *before* the cast
    /// exists. `auto_pass_shielded_allies` passes the in-flight frame's
    /// values; `ally_shield_capacity` passes the candidate spell's.
    fn sculpt_spells_capacity(
        &self,
        caster_id: usize,
        school: Option<SpellSchool>,
        level: u32,
    ) -> Option<usize> {
        if school != Some(SpellSchool::Evocation) {
            return None;
        }
        let caster = self.actors.get(&caster_id)?;
        if !caster.has_passive_feature(crate::actions::class_features::SCULPT_SPELLS_TAG) {
            return None;
        }
        Some(1 + level as usize)
    }

    /// The most allies `caster_id` could spare from a `school`-school,
    /// `level`-level blast — the union size of every shielding feature
    /// they hold. 0 means a blast that catches an ally really does hurt
    /// that ally.
    ///
    /// The union is a plain `max` because both features take a prefix of
    /// the same nearest-first ally ordering, so the larger prefix
    /// contains the smaller.
    ///
    /// Exists for the AI's friendly-fire gate, which has to decide
    /// whether a candidate blast point is acceptable *before* the spell
    /// is cast and therefore can't read the shielded set that
    /// `auto_pass_shielded_allies` produces mid-resolution. Sharing the
    /// per-feature capacity helpers with that function is what keeps the
    /// AI's model of "how many allies can I tolerate in the blast" from
    /// drifting away from what the resolver will actually spare.
    pub fn ally_shield_capacity(
        &self,
        caster_id: usize,
        school: Option<SpellSchool>,
        level: u32,
    ) -> usize {
        let careful = self.careful_spell_capacity(caster_id).unwrap_or(0);
        let sculpt = self
            .sculpt_spells_capacity(caster_id, school, level)
            .unwrap_or(0);
        careful.max(sculpt)
    }

    /// The set of ally ids inside `(point, radius)` that this cast spares
    /// entirely: they don't roll a save, take no damage, and are recorded
    /// as having passed so per-target riders skip them too.
    ///
    /// Two features feed the set and their results union, so a caster
    /// holding both shields the larger group:
    ///
    ///   - **Careful Spell** (Sorcerer metamagic) — a consumable prime
    ///     (`CarefulSpelling`) that works on *any* spell and shields up
    ///     to CHA-mod allies. RAW: "you choose a number of those
    ///     creatures up to your Charisma modifier (minimum of one)." The
    ///     prime is consumed iff at least one ally was actually shielded
    ///     — an empty blast leaves it up for the next AoE, mirroring the
    ///     Empowered "consume on damage roll, not on every cast" pattern.
    ///   - **Sculpt Spells** (Evocation Wizard lv2) — an always-on
    ///     passive that shields `1 + spell level` allies but only on
    ///     evocation casts. Nothing to consume; the gate is the school
    ///     of the in-flight cast, read off the cast stack.
    ///
    /// Allies are taken nearest-first from `ally_burst_targets`, which is
    /// how the engine collapses RAW's "you choose a number of them" — the
    /// only choice a sane caster makes, and the simplification Careful
    /// Spell already shipped under.
    ///
    /// Called from the burst-save chokepoints (`burst_save_damage`,
    /// `resolve_burst_save_damage`, the Fireball-scroll item factor).
    /// Returns an empty set for casters holding neither feature, which is
    /// almost every caster — both gates short-circuit before any
    /// footprint math runs.
    pub fn auto_pass_shielded_allies(
        &mut self,
        caster_id: usize,
        point: Coordinate,
        radius: isize,
    ) -> std::collections::HashSet<usize> {
        use std::collections::HashSet;
        let mut shielded: HashSet<usize> = HashSet::new();
        // Resolve the ally list once and share it between the two lanes —
        // both pick a prefix of the same nearest-first ordering, so the
        // union is just "the longer prefix wins".
        let mut allies: Option<Vec<usize>> = None;
        let mut ally_ids = |this: &mut Self| -> Vec<usize> {
            allies
                .get_or_insert_with(|| this.ally_burst_targets(caster_id, point, radius))
                .clone()
        };

        if let Some(cha_mod) = self.careful_spell_capacity(caster_id) {
            let picked: Vec<usize> = ally_ids(self).into_iter().take(cha_mod).collect();
            if !picked.is_empty() {
                let caster_name = self.actor_name(caster_id);
                self.log(format!(
                    "  careful spell: {} shields {} ally/-ies from the blast",
                    caster_name,
                    picked.len()
                ));
                if let Some(caster) = self.actors.get_mut(&caster_id) {
                    caster.remove_condition(Condition::CarefulSpelling);
                }
                shielded.extend(picked);
            }
        }

        // Sculpt Spells: gated on the school of the cast currently being
        // resolved rather than on anything the caster is holding, so it
        // is inert outside a cast and on every non-evocation spell.
        let cast = self.current_cast();
        if let Some(count) = self.sculpt_spells_capacity(
            caster_id,
            cast.and_then(|c| c.school),
            cast.map(|c| c.level).unwrap_or(0),
        ) {
            let picked: Vec<usize> = ally_ids(self).into_iter().take(count).collect();
            if !picked.is_empty() {
                let caster_name = self.actor_name(caster_id);
                self.log(format!(
                    "  sculpt spells: {} carves {} ally/-ies out of the blast",
                    caster_name,
                    picked.len()
                ));
                shielded.extend(picked);
            }
        }
        shielded
    }

    /// 5e Sorcerer Extended Spell metamagic — if the caster has the prime
    /// up, walk `side_effects` and call `extend_duration` on each. Any
    /// `true` return doubled an eligible `Rounds(n)` timer (RAW: 1 minute
    /// or longer); the prime is consumed the first time at least one
    /// timer was extended on this cast. Returns true iff the prime was
    /// consumed — callers (e.g. the Twinned Spell re-issue path) use the
    /// flag to extend the twin's separately-built side_effects vec under
    /// the same cast so both targets see the doubled duration RAW.
    /// Idempotent when no prime is up or when no side-effect carried an
    /// eligible long timer: returns false silently so spells whose only
    /// effects are short-duration buffs (`UntilStartOfNextTurn`) or
    /// instantaneous damage don't burn the prime. Mirrors the
    /// consume-on-trigger pattern used by Empowered / Heightened /
    /// Careful / Distant Spell at the other roll-site chokepoints.
    pub fn consume_extended_spell(
        &mut self,
        caster_id: usize,
        side_effects: &mut [Box<dyn crate::engine::side_effects::ApplicableSideEffect>],
    ) -> bool {
        self.consume_side_effect_metamagic(
            caster_id,
            Condition::ExtendedSpelling,
            "extended spell",
            "doubles the duration",
            side_effects,
            crate::engine::side_effects::extend_side_effect_timers,
        )
    }

    /// Shared "check prime → run side-effect mutator → consume + log on
    /// success" body for the side-effect-mutating metamagic chokepoints
    /// (Extended Spell, Transmuted Spell). Returns true iff the prime was
    /// consumed. Idempotent when the prime isn't up or when the mutator
    /// returns false (no eligible side-effect was touched), mirroring the
    /// consume-on-trigger pattern used by the other metamagic primes.
    fn consume_side_effect_metamagic(
        &mut self,
        caster_id: usize,
        prime: Condition,
        spell_label: &str,
        effect_summary: &str,
        side_effects: &mut [Box<dyn crate::engine::side_effects::ApplicableSideEffect>],
        mutate: impl FnOnce(&mut [Box<dyn crate::engine::side_effects::ApplicableSideEffect>]) -> bool,
    ) -> bool {
        if !self
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_condition(prime))
        {
            return false;
        }
        if !mutate(side_effects) {
            return false;
        }
        let Some(caster) = self.actors.get_mut(&caster_id) else {
            return false;
        };
        let name = caster.name().to_string();
        caster.remove_condition(prime);
        self.log(format!("  {}: {} {}", spell_label, name, effect_summary));
        true
    }

    /// 5e Tasha's Sorcerer Transmuted Spell metamagic — if the caster has
    /// the prime up and `side_effects` carries at least one `DealDamage`
    /// whose damage type is one of the six elemental types
    /// (acid / cold / fire / lightning / poison / thunder), pick the best
    /// replacement type (worst vulnerability on the primary target, else
    /// best non-resisted, else best non-immune), walk the side_effects
    /// and remap every eligible damage type in place. Returns true iff
    /// the prime was consumed — callers (e.g. the Twinned Spell re-issue
    /// path) use the flag to propagate the same remap into the twin's
    /// separately-built side_effects vec.
    ///
    /// Picks the new type by sampling each transmutable target against
    /// the primary target's resistance profile. We prefer the type that
    /// the target is *vulnerable* to (double damage), falling back to
    /// any non-resisted type, then any non-immune type. With no target
    /// data (e.g. self-cast burst with no actor target), defaults to
    /// `Fire` as the broadest-coverage choice.
    pub fn consume_transmuted_spell(
        &mut self,
        caster_id: usize,
        side_effects: &mut [Box<dyn crate::engine::side_effects::ApplicableSideEffect>],
    ) -> Option<crate::engine::types::DamageType> {
        // Resolve the replacement type up front so the closure passed to
        // `consume_side_effect_metamagic` can be a thin remap call. The
        // prime check inside the helper short-circuits before we touch
        // any side-effects, so picking a type when the prime is down
        // is the only wasted work — cheap (a HashMap lookup per element).
        if !self
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_condition(Condition::TransmutedSpelling))
        {
            return None;
        }
        // Find the primary damage target — first side-effect that carries
        // elemental damage points us at the actor whose weakness profile
        // drives the pick. Fall back to "nearest combat-active enemy" when
        // no DealDamage entry exposes a target (self-centered AoEs).
        let primary_target_id = side_effects
            .iter()
            .find_map(|se| se.elemental_damage_target().map(|(id, _)| id))
            .or_else(|| {
                let caster_team = self.actors.get(&caster_id).map(|a| a.team())?;
                let caster_loc = self.actors.get(&caster_id).map(|a| a.location())?;
                self.actors
                    .iter()
                    .filter(|(id, a)| {
                        **id != caster_id && a.team() != caster_team && a.is_combat_active()
                    })
                    .min_by_key(|(_, a)| {
                        let d = a.location() - caster_loc;
                        d.x * d.x + d.y * d.y
                    })
                    .map(|(id, _)| *id)
            });
        let new_type = self.pick_transmuted_damage_type(primary_target_id);
        let consumed = self.consume_side_effect_metamagic(
            caster_id,
            Condition::TransmutedSpelling,
            "transmuted spell",
            &format!("remaps the damage to {}", new_type),
            side_effects,
            |se| crate::engine::side_effects::remap_side_effect_damage_types(se, new_type),
        );
        if consumed { Some(new_type) } else { None }
    }

    /// Pick the best damage type for a Transmuted Spell remap given a
    /// target actor. Priority (best to worst):
    /// 1. Any of the six elemental types the target is *vulnerable* to
    ///    (doubles damage; clearly the best pick).
    /// 2. Any of the six elemental types the target has no template
    ///    modifier against (full damage; no resistance lost).
    /// 3. Any of the six the target is merely resistant to (still
    ///    delivers half damage; better than burning the prime on an
    ///    immune type).
    /// 4. Default to `Fire` when no target context is available —
    ///    Fire has the broadest reach across the engine's bestiary
    ///    (only a few constructs / fiends carry fire immunity).
    fn pick_transmuted_damage_type(
        &self,
        target_id: Option<usize>,
    ) -> crate::engine::types::DamageType {
        use crate::engine::side_effects::TRANSMUTABLE_DAMAGE_TYPES;
        use crate::engine::types::{DamageModifier, DamageType};
        let Some(target_id) = target_id else {
            return DamageType::Fire;
        };
        let Some(target) = self.actors.get(&target_id) else {
            return DamageType::Fire;
        };
        // Sweep the six types once and bucket by modifier. We pick the
        // first match in each bucket (the constant's iteration order is
        // alphabetical-ish; ties break deterministically).
        let mut vuln: Option<DamageType> = None;
        let mut neutral: Option<DamageType> = None;
        let mut resisted: Option<DamageType> = None;
        for &dt in TRANSMUTABLE_DAMAGE_TYPES.iter() {
            match target.damage_modifier(dt) {
                Some(DamageModifier::Vulnerability) => {
                    if vuln.is_none() {
                        vuln = Some(dt);
                    }
                }
                Some(DamageModifier::Immunity) | Some(DamageModifier::Absorption) => {
                    // Skip — never pick a type the target takes nothing
                    // from, and least of all one it drinks.
                }
                Some(DamageModifier::Resistance) => {
                    if resisted.is_none() {
                        resisted = Some(dt);
                    }
                }
                None => {
                    if neutral.is_none() {
                        neutral = Some(dt);
                    }
                }
            }
        }
        vuln.or(neutral).or(resisted).unwrap_or(DamageType::Fire)
    }

    /// 5e Wild Magic Sorcerer **Wild Magic Surge** — after the caster
    /// resolves a sorcerer spell of 1st level or higher, the engine rolls
    /// a d20; on a 1, a random effect from the surge table fires.
    /// Returns side-effects to append to the casting action's effect
    /// list (or an empty vec for no surge / no eligible cast).
    ///
    /// Gates:
    ///   - `spell_level == 0` → no surge (cantrips never trigger RAW)
    ///   - caster lacks the `WILD_MAGIC_SURGE_TAG` passive feature
    ///   - d20 != 1 (the 5% trigger)
    ///
    /// Surge table (1d6, mapped to effects with similar tactical weight):
    ///   1. **Pyrotechnic burst** — every actor whose footprint touches
    ///      a 1-tile burst around the caster takes 1d6 fire damage. The
    ///      caster is in the blast — wild magic isn't friendly.
    ///   2. **Chaotic mending** — caster heals 2d4 HP (lifted from the
    ///      Magic Initiate Cure Wounds dice — keeps the heal modest so
    ///      the surge doesn't dwarf a Bonus Action Healing Word).
    ///   3. **Surge of force** — caster gains 5 temporary HP from
    ///      crackling protective energy.
    ///   4. **Mirror flicker** — 3 mirror images flicker into being
    ///      around the caster (same envelope as the Mirror Image spell,
    ///      no concentration). Re-uses the existing `SetMirrorImages` +
    ///      `MirroredImages` cohort.
    ///   5. **Replenishing surge** — caster regains 2 sorcery points
    ///      (capped at their long-rest max). Mutates the actor directly
    ///      since there's no `SorceryPoint` resource lane.
    ///   6. **Wild dazzle** — caster casts Faerie Fire (Outlined
    ///      condition) on every enemy in 6-tile burst around them; no
    ///      save — wild magic ignores the usual save lane.
    ///
    /// The d6 surge pick is rolled through `self.roll` so tests can pin
    /// the result by seeding `self.roller`.
    pub fn trigger_wild_magic_surge(
        &mut self,
        caster_id: usize,
        spell_level: u32,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::actions::class_features::WILD_MAGIC_SURGE_TAG;
        use crate::conditions::ConditionTimer;
        use crate::engine::dice::Dice;
        use crate::engine::side_effects::{
            ApplicableSideEffect, ApplyCondition, DealDamage, GainTempHp, Heal, SetMirrorImages,
        };
        use crate::engine::types::DamageType;

        if spell_level == 0 {
            return Vec::new();
        }
        let has_feature = self
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_passive_feature(WILD_MAGIC_SURGE_TAG));
        if !has_feature {
            return Vec::new();
        }
        let d20 = self.roll(&Dice::new(1, 20));
        if d20 != 1 {
            return Vec::new();
        }
        let caster_loc = match self.actors.get(&caster_id) {
            Some(a) => a.location(),
            None => return Vec::new(),
        };
        let caster_name = self.actor_name(caster_id);
        let pick = self.roll(&Dice::new(1, 6));
        self.log(format!(
            "{} surges with wild magic! (d20=1, table d6={})",
            caster_name, pick
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        match pick {
            1 => {
                // Pyrotechnic burst — 1d6 fire, 1-tile radius around caster.
                // Damage is rolled once and shared across the burst so the
                // numbers stay consistent with Sacred Burst / Fireball semantics.
                let dmg = self.roll(&Dice::new(1, 6));
                self.log(format!("  wild surge: pyrotechnic burst (1d6={} fire)", dmg));
                const BURST_RADIUS: isize = 1;
                for id in self.actors_in_burst(caster_loc, BURST_RADIUS) {
                    effects.push(Box::new(DealDamage {
                        actor_id: id,
                        amount: dmg,
                        damage_type: DamageType::Fire,
                    }));
                }
            }
            2 => {
                // Chaotic mending — 2d4 HP heal.
                let raw = self.roll(&Dice::new(2, 4));
                self.log(format!("  wild surge: chaotic mending (2d4={} HP)", raw));
                effects.push(Box::new(Heal {
                    actor_id: caster_id,
                    amount: raw,
                }));
            }
            3 => {
                // Surge of force — 5 temp HP shield.
                self.log("  wild surge: 5 temp HP shield");
                effects.push(Box::new(GainTempHp {
                    actor_id: caster_id,
                    amount: 5,
                }));
            }
            4 => {
                // Mirror flicker — 3 mirror images.
                self.log("  wild surge: 3 mirror images flicker into being");
                effects.push(Box::new(SetMirrorImages {
                    actor_id: caster_id,
                    count: 3,
                }));
                effects.push(Box::new(ApplyCondition {
                    actor_id: caster_id,
                    condition: Condition::MirroredImages,
                    timer: ConditionTimer::Rounds(10),
                }));
            }
            5 => {
                // Replenishing surge — +2 SP, capped at max. Direct mutation
                // mirrors Font of Magic's eager-debit pattern (no SP resource
                // lane).
                if let Some(actor) = self.actors.get_mut(&caster_id) {
                    let given = actor.give_sorcery_points(2);
                    let sp_left = actor.sorcery_points();
                    self.log(format!(
                        "  wild surge: replenishing surge (+{} SP, {} SP)",
                        given, sp_left
                    ));
                }
            }
            6 => {
                // Wild dazzle — Outlined (Faerie Fire-style) on every enemy
                // within a 6-tile burst around the caster. No save.
                const DAZZLE_RADIUS: isize = 6;
                let enemy_ids = self.enemy_burst_targets(caster_id, caster_loc, DAZZLE_RADIUS);
                self.log(format!(
                    "  wild surge: wild dazzle lights up {} enem{}",
                    enemy_ids.len(),
                    if enemy_ids.len() == 1 { "y" } else { "ies" }
                ));
                for id in enemy_ids {
                    effects.push(Box::new(ApplyCondition {
                        actor_id: id,
                        condition: Condition::Outlined,
                        timer: ConditionTimer::Rounds(10),
                    }));
                }
            }
            _ => {
                // d6 out of range — shouldn't happen, but log defensively
                // so a future expansion of the surge table doesn't drop
                // silently if the pick range isn't widened in lockstep.
                self.log(format!(
                    "  wild surge: unmapped surge result {} (no effect)",
                    pick
                ));
            }
        }
        effects
    }

    /// 5e Sorcerer Storm Sorcery **Heart of the Storm** — eruption clause
    /// (XGtE, RAW lv6). When the sorcerer casts a spell of 1st level or
    /// higher that deals lightning or thunder damage, a burst of the same
    /// element ripples out from the caster: each hostile creature within
    /// 10 ft (2-tile burst radius; the caster is spared) takes
    /// `HEART_OF_THE_STORM_ERUPTION_DAMAGE` damage. Auto-hit, no save —
    /// matches RAW's flat "half your sorcerer level" tick (we hardcode
    /// the value since the engine doesn't track class levels separately
    /// from XP-driven `level`, and the CR-4 chassis represents ~lv6).
    ///
    /// Gates (short-circuit in order):
    ///   - `spell_level == 0` → cantrips never trigger, matching the RAW
    ///     "1st level or higher" clause. Cheaper check first so
    ///     Thunderclap / Shocking Grasp spam doesn't pay for the feature
    ///     lookup.
    ///   - caster lacks the `HEART_OF_THE_STORM_TAG` passive feature —
    ///     opt-in via the Storm Sorcerer template, same shape as the
    ///     Wild Magic Surge tag gate on the parallel post-cast surface.
    ///   - `damage_types` doesn't contain Lightning or Thunder — RAW's
    ///     "deals lightning or thunder damage" clause. Non-storm spells
    ///     (Fireball, Magic Missile, Charm Person) fall through.
    ///
    /// Damage type picks: Lightning if the spell dealt Lightning, else
    /// Thunder (only reached if the spell dealt Thunder given the gate
    /// above). RAW lets the sorcerer choose either type; we pick the
    /// matching type since a Thunder spell erupting as Lightning would
    /// read as odd on log output.
    ///
    /// Symmetric structural sibling to `trigger_wild_magic_surge` — same
    /// post-cast trigger surface, same feature-tag opt-in, same
    /// spell-level-gate. Wired at the shared `Action::execute`
    /// chokepoint next to Wild Magic Surge so both post-cast triggers
    /// resolve "after the spell" per RAW.
    pub fn trigger_heart_of_the_storm_eruption(
        &mut self,
        caster_id: usize,
        spell_level: u32,
        damage_types: &[DamageType],
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::actions::class_features::{
            HEART_OF_THE_STORM_ERUPTION_DAMAGE, HEART_OF_THE_STORM_TAG,
        };
        use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};

        if spell_level == 0 {
            return Vec::new();
        }
        let has_feature = self
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_passive_feature(HEART_OF_THE_STORM_TAG));
        if !has_feature {
            return Vec::new();
        }
        let damage_type = if damage_types.contains(&DamageType::Lightning) {
            DamageType::Lightning
        } else if damage_types.contains(&DamageType::Thunder) {
            DamageType::Thunder
        } else {
            return Vec::new();
        };
        let caster_loc = match self.actors.get(&caster_id) {
            Some(a) => a.location(),
            None => return Vec::new(),
        };
        let caster_name = self.actor_name(caster_id);
        const RADIUS: isize = 2;
        let enemy_ids = self.enemy_burst_targets(caster_id, caster_loc, RADIUS);
        if enemy_ids.is_empty() {
            return Vec::new();
        }
        self.log(format!(
            "{}'s heart of the storm erupts ({} {:?} to {} enem{})",
            caster_name,
            HEART_OF_THE_STORM_ERUPTION_DAMAGE,
            damage_type,
            enemy_ids.len(),
            if enemy_ids.len() == 1 { "y" } else { "ies" }
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in enemy_ids {
            effects.push(Box::new(DealDamage {
                actor_id: id,
                amount: HEART_OF_THE_STORM_ERUPTION_DAMAGE,
                damage_type,
            }));
        }
        effects
    }

    /// Dispatch every post-cast trigger the engine models. Runs each
    /// registered post-cast hook against `caster_id` with the resolved
    /// spell context (`spell_level`, `damage_types`, `school`) and
    /// returns the concatenation of side-effects each hook produced.
    /// Called once
    /// from the shared `Action::execute` chokepoint — the single call
    /// site keeps the trigger dispatch out of every action impl and
    /// lets a new post-cast trigger drop in as one line of this method
    /// body rather than another block in `Action::execute`.
    ///
    /// Current registry (in fire order):
    ///   - **Wild Magic Surge** — d20=1 → random effect from the surge
    ///     table (feature-tag gated on `WILD_MAGIC_SURGE_TAG`; cantrip
    ///     gated).
    ///   - **Heart of the Storm eruption** — lightning / thunder cast
    ///     → 10-ft radius enemy burst (feature-tag gated on
    ///     `HEART_OF_THE_STORM_TAG`; cantrip gated; damage-type
    ///     gated).
    ///   - **Overchannel backlash** — the escalating necrotic self-hit
    ///     owed for maximizing a spell, latched at the damage-roll site
    ///     and charged here so it resolves as a normal `DealDamage`
    ///     (RAW: "immediately after you cast it").
    ///   - **Arcane Ward form / recharge** — abjuration cast of 1st
    ///     level or higher → weave the ward at full strength (first
    ///     cast) or top it up by twice the slot level (later casts).
    ///     Template gated on `arcane_ward_base > 0`; cantrip gated;
    ///     school gated. Produces no side-effects — it mutates the
    ///     caster's ward pool in place and logs — so it returns an
    ///     empty vec and exists in the registry for the single
    ///     post-cast chokepoint rather than for its return value.
    ///
    /// Each hook is responsible for its own opt-in / short-circuit
    /// gates and returns an empty vec on a miss. The dispatcher is
    /// intentionally cheap for the common case (non-caster, non-storm-
    /// sorcerer, cantrip cast) — every hook's early-out fires before
    /// any expensive work.
    ///
    /// Ordering matters when two hooks could both fire on the same
    /// cast. Today no hostile combination overlaps (Wild Magic Surge
    /// lives on the baseline Wild Magic Sorcerer template, Heart of
    /// the Storm on the Storm Sorcerer template — mutually exclusive
    /// subclass picks), but the surface is deterministic: the surge
    /// resolves before the eruption so a hypothetical multi-class
    /// carrier gets both effects in a stable order.
    pub fn dispatch_post_cast_triggers(
        &mut self,
        caster_id: usize,
        spell_level: u32,
        damage_types: &[DamageType],
        school: Option<SpellSchool>,
        target_ids: Option<&Vec<usize>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let mut effects = Vec::new();
        effects.append(&mut self.trigger_wild_magic_surge(caster_id, spell_level));
        effects.append(&mut self.trigger_heart_of_the_storm_eruption(
            caster_id,
            spell_level,
            damage_types,
        ));
        effects.append(&mut self.trigger_overchannel_backlash(caster_id, spell_level));
        self.trigger_arcane_ward(caster_id, spell_level, school);
        self.trigger_expert_divination(caster_id, spell_level, school);
        self.trigger_benign_transposition_recharge(caster_id, spell_level, school);
        self.trigger_war_magic_prime(caster_id, spell_level, school);
        effects.append(&mut self.trigger_voice_of_authority(caster_id, spell_level, target_ids));
        effects
    }

    /// 5e Order Domain Cleric **Voice of Authority** (subclass level 1)
    /// post-cast hook: a spell of 1st level or higher cast on an ally
    /// lets that ally spend their reaction on one weapon attack, right
    /// now.
    ///
    /// The first hook in this family to care *who the spell landed on*,
    /// which is why the dispatcher grew a `target_ids` parameter. Every
    /// other member keys off the caster and the cast (level, school,
    /// damage type) and needs nothing from the target list.
    ///
    /// The feature is deliberately not free: it turns the cleric's
    /// support actions into damage, so a round spent healing the
    /// front-line is also a round the front-line gets an extra swing.
    /// What bounds it is the ally's reaction — one per round, shared
    /// with opportunity attacks and every reactive defence they have —
    /// so the cleric is spending someone else's resource, and the ally
    /// pays for it the next time an enemy walks away from them.
    ///
    /// Self-targeted spells don't count: RAW says "an ally", and a
    /// cleric who could Shield of Faith themselves into a free attack
    /// every round would never cast anything else.
    ///
    /// Unlike its cantrip-gated sibling `trigger_war_magic_prime`, this
    /// hook takes no `school`. It doesn't need one: a non-zero
    /// `spell_level` means the cast paid a `SpellSlot`, and nothing but
    /// a spell does. Asking for a school as well would quietly exclude
    /// every levelled spell whose school no feature reads — which is
    /// most of the cleric's buff list, Shield of Faith included.
    fn trigger_voice_of_authority(
        &mut self,
        caster_id: usize,
        spell_level: u32,
        target_ids: Option<&Vec<usize>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::actions::class_features::VOICE_OF_AUTHORITY_TAG;
        if spell_level == 0 {
            return Vec::new();
        }
        if !self
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_passive_feature(VOICE_OF_AUTHORITY_TAG))
        {
            return Vec::new();
        }
        let Some(targets) = target_ids else {
            return Vec::new();
        };
        // First eligible ally in the spell's own target list, in the
        // order the spell named them. A multi-target buff orders one
        // swing, not one per creature — RAW's "target an ally with the
        // spell" reads as a single grant per cast.
        let ally = targets
            .iter()
            .copied()
            .find(|&id| id != caster_id && self.actors_allied(caster_id, id));
        let Some(ally_id) = ally else {
            return Vec::new();
        };
        // Queued rather than fired here: the spell's own effects are
        // still unapplied boxes at this point, and the ally has to
        // receive the spell before they answer for it. See
        // `side_effects::DirectedAttack`.
        vec![Box::new(crate::engine::side_effects::DirectedAttack {
            actor_id: ally_id,
            director_id: caster_id,
            label: "word of command",
        })]
    }

    /// 5e Eldritch Knight Fighter **War Magic** (subclass level 7)
    /// post-cast hook: casting a cantrip arms the `WAR_MAGIC_STRIKE`
    /// bonus action for a follow-up weapon swing this turn.
    ///
    /// The only member of the post-cast family gated on a *cantrip*
    /// rather than on a levelled cast — Arcane Ward, Expert Divination
    /// and Benign Transposition all key off `spell_level >= 1`, and War
    /// Magic is the mirror case: cantrips are exactly what it rewards,
    /// because the fighter's whole tempo trade is giving up the Attack
    /// action for a spell that costs no slot.
    ///
    /// Which makes the `school.is_some()` half of the gate load-bearing
    /// rather than decorative. `Action::execute` opens a level-0 cast
    /// frame for *every* action it runs, so a weapon swing, a Move and
    /// War Magic's own bonus action all arrive here indistinguishable
    /// from a cantrip on level alone — and a knight who armed the prime
    /// by swinging would get the follow-up swing for free, which is the
    /// opposite of the feature. "Has a school" is the engine's marker
    /// for "is a spell" (see `CastContext::is_cantrip`, pinned by
    /// `every_cantrip_declares_its_school`), and it is what separates
    /// the two cases here.
    ///
    /// Installing the prime is idempotent — two cantrips in one turn
    /// (Action Surge) leave one prime, and the single bonus action is
    /// the real cap. No feature charge to spend and no rest cadence:
    /// the tag is the entire gate.
    fn trigger_war_magic_prime(
        &mut self,
        caster_id: usize,
        spell_level: u32,
        school: Option<SpellSchool>,
    ) {
        use crate::actions::class_features::WAR_MAGIC_TAG;
        if spell_level != 0 || school.is_none() {
            return;
        }
        let Some(caster) = self.actors.get_mut(&caster_id) else {
            return;
        };
        if !caster.has_passive_feature(WAR_MAGIC_TAG) {
            return;
        }
        if !caster.add_condition(
            Condition::WarMagicPrimed,
            crate::conditions::ConditionTimer::UntilStartOfNextTurn,
        ) {
            return;
        }
        let name = self.actor_name(caster_id);
        self.log(format!(
            "  war magic: {}'s cantrip leaves an opening for a follow-up swing",
            name
        ));
    }

    /// 5e Conjuration Wizard **Benign Transposition** (subclass level 6)
    /// post-cast hook: casting a conjuration spell of 1st level or
    /// higher refreshes the teleport's charge.
    ///
    /// Third row of the school-gated post-cast family, and the only one
    /// that *gives back* a resource on the school axis rather than
    /// spending or growing one — Arcane Ward tops up an absorption
    /// pool, Expert Divination refunds a slot, this one re-arms an
    /// action. Same three-gate shape (school, non-cantrip, feature
    /// held), same cheapest-gate-first ordering.
    ///
    /// The recharge condition is what makes the feature interesting to
    /// play rather than a once-per-rest blink: a conjurer who keeps
    /// casting their own school teleports every round, and one who
    /// reaches for a Fireball goes without. Refreshing an already-full
    /// charge logs nothing, the same way a top-up against a full Arcane
    /// Ward does.
    fn trigger_benign_transposition_recharge(
        &mut self,
        caster_id: usize,
        spell_level: u32,
        school: Option<SpellSchool>,
    ) {
        use crate::actions::class_features::BENIGN_TRANSPOSITION_TAG;
        if school != Some(SpellSchool::Conjuration) || spell_level == 0 {
            return;
        }
        let Some(caster) = self.actors.get_mut(&caster_id) else {
            return;
        };
        if !caster.has_passive_feature(BENIGN_TRANSPOSITION_TAG)
            || caster.feature_available(BENIGN_TRANSPOSITION_TAG)
        {
            return;
        }
        caster.restore_feature_charge(BENIGN_TRANSPOSITION_TAG);
        let name = self.actor_name(caster_id);
        self.log(format!(
            "  benign transposition: {}'s conjuring re-anchors the transposition",
            name
        ));
    }

    /// 5e Divination Wizard **Expert Divination** (subclass level 6)
    /// post-cast hook: casting a divination spell of 2nd level or higher
    /// refunds one expended slot of a *lower* level, never above 5th.
    ///
    /// Three gates, cheapest first, mirroring `trigger_arcane_ward`: the
    /// school must be `Some(SpellSchool::Divination)` (an untagged spell
    /// reads `None` and fails closed), the slot level must be at least 2
    /// (RAW excludes cantrips and 1st-level casts — there is no lower
    /// band to refund into), and the caster must hold the feature.
    ///
    /// The RAW band is `1..=min(spell_level - 1, 5)`; the
    /// highest-expended-first pick inside it lives on the slot manager,
    /// so this hook stays a gate plus a log line. A cast with nothing
    /// expended in the band logs nothing, the same way a top-up against
    /// a full Arcane Ward does.
    ///
    /// The band excludes the cast's own level by construction, which is
    /// what makes the hook's position in `execute` irrelevant: post-cast
    /// triggers are dispatched before the `ConsumeResource` tail that
    /// charges the slot, so the in-flight slot still reads as available
    /// here — and it is out of the refund band anyway. A 9th-level
    /// Foresight refunds at most a 5th, never the 9th it is paying for.
    fn trigger_expert_divination(
        &mut self,
        caster_id: usize,
        spell_level: u32,
        school: Option<SpellSchool>,
    ) {
        use crate::actions::class_features::EXPERT_DIVINATION_TAG;
        if school != Some(SpellSchool::Divination) || spell_level < 2 {
            return;
        }
        let cap = (spell_level - 1).min(5);
        let Some(caster) = self.actors.get_mut(&caster_id) else {
            return;
        };
        if !caster.has_passive_feature(EXPERT_DIVINATION_TAG) {
            return;
        }
        let Some(level) = caster
            .spell_slot_manager
            .restore_highest_expended_slot_up_to(cap)
        else {
            return;
        };
        let name = self.actor_name(caster_id);
        self.log(format!(
            "  expert divination: {} regains a level-{} spell slot",
            name, level
        ));
    }

    /// 5e Abjuration Wizard **Arcane Ward** (subclass level 2) post-cast
    /// hook: casting an abjuration spell of 1st level or higher either
    /// weaves the ward (first cast since the last long rest — appears at
    /// its full `arcane_ward_max`) or recharges it by twice the slot
    /// level, capped at that maximum.
    ///
    /// Three gates, cheapest first: the school must be
    /// `Some(SpellSchool::Abjuration)` (an untagged spell reads `None`
    /// and fails closed), the slot level must be non-zero (RAW excludes
    /// cantrips — Blade Ward and Resistance don't feed the ward), and
    /// the caster must hold the feature. The actor-side helper does the
    /// pool arithmetic and reports whether anything actually moved, so a
    /// top-up against an already-full ward logs nothing.
    ///
    /// Sibling in shape to `trigger_wild_magic_surge` /
    /// `trigger_heart_of_the_storm_eruption` on the post-cast registry,
    /// but on the "mutate the caster, emit no side-effects" lane: the
    /// ward is caster-local bookkeeping with no target set to resolve,
    /// so routing it through a `Heal`-style side-effect would buy
    /// nothing but indirection.
    fn trigger_arcane_ward(
        &mut self,
        caster_id: usize,
        spell_level: u32,
        school: Option<SpellSchool>,
    ) {
        if school != Some(SpellSchool::Abjuration) || spell_level == 0 {
            return;
        }
        let Some(caster) = self.actors.get_mut(&caster_id) else {
            return;
        };
        let name = caster.name().to_string();
        let was_formed = caster.arcane_ward_formed();
        let Some((gained, now)) = caster.weave_or_recharge_arcane_ward(spell_level) else {
            return;
        };
        let max = self
            .actors
            .get(&caster_id)
            .map(|a| a.arcane_ward_max())
            .unwrap_or(now);
        if was_formed {
            self.log(format!(
                "  arcane ward: {} absorbs {} more magic ({}/{})",
                name, gained, now, max
            ));
        } else {
            self.log(format!(
                "  arcane ward: {} weaves a {}-point ward",
                name, now
            ));
        }
    }

    /// 5e Sanctuary: if `target_id` carries the Sanctuary condition, the
    /// attacker (`attacker_id`) makes a one-shot WIS save. On fail, the
    /// attack is blocked entirely (caller short-circuits the attack roll
    /// and returns a miss). On pass, the spell is breached and the buff
    /// drops so it can't keep firing for the rest of the round.
    ///
    /// The DC is the warding caster's own spell save DC, found through
    /// the `Sanctuary` back-link the spell installs alongside the flag.
    /// RAW is explicit that it is the caster's — "must first make a
    /// Wisdom saving throw against your spell save DC" — and the doc on
    /// the spell has always said so; the save site simply had no way to
    /// reach the caster until `Sanctuary` joined `LINKED_CONDITIONS`.
    /// A fixed 14 stood in, which under-priced a high-level cleric's
    /// ward and over-priced a first-level one's.
    ///
    /// `ITEM_SANCTUARY_DC` is the fallback, and it is not a
    /// stand-in — it is the right answer for the two sources that
    /// deliberately leave the link unset. A potion of sanctuary and a
    /// sanctuary scroll carry a ward of their own making, and pricing
    /// it off whoever happened to drink or read it would let a fighter
    /// with no spellcasting at all put up a DC 11 ward with the same
    /// consumable a cleric turns into a DC 17 one.
    pub fn sanctuary_save_blocks(&mut self, attacker_id: usize, target_id: usize) -> bool {
        /// 5e potion / scroll DC baseline — comparable to a level-1
        /// cleric's WIS-based spell save DC (8 + 2 prof + 4 WIS mod),
        /// which is the tier the consumable sits at.
        const ITEM_SANCTUARY_DC: i32 = 14;
        let Some(warded_actor) = self.actors.get(&target_id) else {
            return false;
        };
        if !warded_actor.has_condition(Condition::Sanctuary) {
            return false;
        }
        let dc = warded_actor
            .linked_by(Condition::Sanctuary)
            .and_then(|caster_id| self.actors.get(&caster_id))
            .map(|caster| {
                caster.best_spell_save_dc(
                    crate::actors::actor_template::ActorInstance::SPELLCASTING_ABILITIES,
                )
            })
            .unwrap_or(ITEM_SANCTUARY_DC);
        let save = self.roll_save(
            attacker_id,
            crate::engine::types::AbilityScoreType::Wisdom,
            dc,
        );
        if save.passed() {
            // 5e: "if the attacker makes a successful save, the spell is
            // breached" — we drop the buff so follow-up swings hit normally.
            if let Some(t) = self.actors.get_mut(&target_id) {
                let name = t.name().to_string();
                if t.remove_condition(Condition::Sanctuary) {
                    self.log(format!("{}'s sanctuary is breached.", name));
                }
            }
            false
        } else {
            let attacker_name = self.actor_name(attacker_id);
            let target_name = self.actor_name(target_id);
            self.log(format!(
                "  sanctuary protects {} from {}.",
                target_name, attacker_name
            ));
            true
        }
    }

    /// Drop the actor's own Sanctuary condition when they take a hostile
    /// action. 5e: "if the warded creature attacks or casts a spell that
    /// affects an enemy, this spell ends." Called from the attack-roll
    /// path before the d20 lands so the buff disappears on the very swing
    /// that violates the ward's pacifism clause.
    pub fn break_sanctuary_on_hostile(&mut self, caster_id: usize) {
        let Some(actor) = self.actors.get_mut(&caster_id) else {
            return;
        };
        let name = actor.name().to_string();
        if actor.remove_condition(Condition::Sanctuary) {
            self.log(format!("{}'s sanctuary fades.", name));
        }
    }

    /// 5e Wild Magic Sorcerer **Bend Luck** (lv6 reaction) penalty hook.
    /// When `target_id` carries the Bend Luck feature and has 2+ SP +
    /// reaction available, spend the resources and return a rolled 1d4
    /// penalty (1..=4) to subtract from the attacker's roll. Returns 0
    /// when the gate fails — caller treats it as a no-op.
    ///
    /// We approximate RAW's "can see the creature" with `!Blinded`. The
    /// 30ft range is honored via the footprint distance (12-tile gap in
    /// the 2.5ft grid). Caller (resolve_attack_outcome) only invokes
    /// this when the attack would otherwise hit but isn't a natural crit,
    /// so the d4 has a real chance of flipping the outcome — saving SP
    /// on attacks that already miss or that crit through it.
    pub fn apply_bend_luck_penalty(
        &mut self,
        target_id: usize,
        attacker_id: usize,
    ) -> u32 {
        use crate::actions::class_features::BEND_LUCK_TAG;
        use crate::engine::dice::Dice;
        use crate::engine::side_effects::Resource;
        const BEND_LUCK_SP_COST: u32 = 2;
        const BEND_LUCK_RANGE_TILES: isize = 12;
        let Some(target) = self.actors.get(&target_id) else {
            return 0;
        };
        if !target.has_passive_feature(BEND_LUCK_TAG)
            || target.sorcery_points() < BEND_LUCK_SP_COST
            || !target.can_consume_resource(Resource::Reaction)
            || !target.is_combat_active()
        {
            return 0;
        }
        // RAW "another creature you can see" gate — routes through the
        // shared `viewer_can_see` helper so both the Blinded clause AND
        // the "attacker is illusion-concealed and the sorcerer doesn't
        // pierce" clause land in one lookup. Pre-refactor this only
        // checked `!Blinded` on the sorcerer, letting an Invisible
        // attacker still draw the Bend Luck 2 SP + reaction spend even
        // though RAW the sorcerer can't see them — same correctness
        // pattern the prior nudge folded into Uncanny Dodge, Deflect
        // Missiles, and Warding Flare.
        if !self.viewer_can_see(target_id, attacker_id) {
            return 0;
        }
        if self
            .footprint_distance(target_id, attacker_id)
            .is_none_or(|d| d > BEND_LUCK_RANGE_TILES)
        {
            return 0;
        }
        let penalty = self.roll(&Dice::new(1, 4));
        let (target_name, attacker_name, sp_left) = {
            let target = match self.actors.get_mut(&target_id) {
                Some(a) => a,
                None => return 0,
            };
            target.spend_sorcery_points(BEND_LUCK_SP_COST);
            target.consume_resource(Resource::Reaction);
            let tn = target.name().to_string();
            let sp = target.sorcery_points();
            let an = self
                .actors
                .get(&attacker_id)
                .map(|a| a.name().to_string())
                .unwrap_or_default();
            (tn, an, sp)
        };
        self.log(format!(
            "  bend luck: {} bends fate, -1d4({}) on {}'s roll ({} SP left)",
            target_name, penalty, attacker_name, sp_left
        ));
        penalty
    }

    /// 5e Light Domain Cleric **Warding Flare** (lv1 subclass): when a
    /// creature the cleric can see attacks them, the cleric can spend
    /// their reaction to impose disadvantage on the attack roll. RAW
    /// gates: the attacker must be within 30 ft AND the cleric must be
    /// able to see them. The engine collapses "can see" to `!Blinded`
    /// and the 30ft range to a 12-tile footprint-Chebyshev cap
    /// (2.5ft/tile).
    ///
    /// Returns `true` when the flare fires — the caller should combine
    /// `Disadvantage` into the attack mode. Returns `false` when any
    /// gate fails (no charge, no reaction, blinded, out of range,
    /// downed): the caller leaves the mode unchanged.
    ///
    /// The reaction and per-rest charge are spent on fire so a follow-
    /// up swing this round bounces off the same target's gate cleanly.
    /// Sibling to `first_eligible_protector` (target-side Fighting Style:
    /// Protection reaction) and `apply_bend_luck_penalty` (target-side
    /// Wild Magic Sorcerer reactive d4). Shared with weapon attacks
    /// (`resolve_attack`) and spell attacks (`spell_attack_outcome`) so
    /// the flare fires uniformly against any attack roll — RAW says
    /// "attack roll" without a weapon-only qualifier.
    ///
    /// Post-cohort refactor: this is now a compat wrapper that filters
    /// the shared `REACTIVE_ATTACK_DISADVANTAGE_SOURCES` cohort down to
    /// the Warding Flare row so unit tests can pin the row's gates
    /// (range, sight, charge, reaction) in isolation. Production call
    /// sites should reach for `apply_reactive_attack_disadvantage`
    /// (which walks the full cohort — Warding Flare AND Entropic Ward
    /// AND any future sibling — in one iterator).
    pub fn apply_warding_flare_disadvantage(
        &mut self,
        target_id: usize,
        attacker_id: usize,
    ) -> bool {
        use crate::actions::class_features::WARDING_FLARE_TAG;
        for source in REACTIVE_ATTACK_DISADVANTAGE_SOURCES {
            if source.tag == WARDING_FLARE_TAG {
                return self.try_apply_reactive_disadvantage_source(
                    target_id,
                    attacker_id,
                    source,
                );
            }
        }
        false
    }

    /// 5e Great Old One Warlock **Entropic Ward** (lv6 subclass): when
    /// a creature attacks the warlock, the warlock can spend their
    /// reaction + the once-per-short-rest charge to impose disadvantage
    /// on the attack roll. Unlike Warding Flare, RAW carries no range
    /// or sight gate — the patron's telepathic tie reads the attacker's
    /// intent regardless of distance or vision.
    ///
    /// Returns `true` when the ward fires — the caller should combine
    /// `Disadvantage` into the attack mode. Returns `false` when the
    /// gate fails (no charge, no reaction, downed).
    ///
    /// Post-cohort refactor: this is now a compat wrapper that filters
    /// the shared `REACTIVE_ATTACK_DISADVANTAGE_SOURCES` cohort down to
    /// the Entropic Ward row so unit tests can pin the row's gates
    /// (charge, reaction — but *not* range or sight) in isolation.
    /// Production call sites should reach for
    /// `apply_reactive_attack_disadvantage` (which walks the full
    /// cohort in one iterator).
    ///
    /// The RAW "if the attack misses, your next attack against the
    /// target has advantage before the end of your next turn" bonus
    /// rider is left as future work — the disadvantage-on-incoming
    /// half is the load-bearing tell; the bonus rider needs a new
    /// target-side one-shot condition tied to the attacker id.
    pub fn apply_entropic_ward_disadvantage(
        &mut self,
        target_id: usize,
        attacker_id: usize,
    ) -> bool {
        use crate::actions::class_features::ENTROPIC_WARD_TAG;
        for source in REACTIVE_ATTACK_DISADVANTAGE_SOURCES {
            if source.tag == ENTROPIC_WARD_TAG {
                return self.try_apply_reactive_disadvantage_source(
                    target_id,
                    attacker_id,
                    source,
                );
            }
        }
        false
    }

    /// Walk the shared `REACTIVE_ATTACK_DISADVANTAGE_SOURCES` cohort
    /// (target-side reactive per-rest disadvantage-imposing features:
    /// Warding Flare, Entropic Ward, and any future sibling) and fire
    /// the first row whose gate passes. Returns `true` on the first
    /// firing (the caller should combine `Disadvantage` into the attack
    /// mode); returns `false` when no row fires.
    ///
    /// Ordering: entries are consulted in listed order. Iteration stops
    /// as soon as one source fires so at most one per-rest charge burns
    /// per incoming attack — mirrors the "at most one add-die per save"
    /// semantics on the `FAILED_SAVE_ADD_DIE_SOURCES` cohort. A
    /// hypothetical Light Cleric / Great Old One Warlock multiclass
    /// therefore burns Warding Flare first (listed first) on a swing
    /// within 30ft that the cleric can see; if either range or sight
    /// gate closes, Entropic Ward (no range, no sight gate) still
    /// fires as a fallback.
    ///
    /// Called from the two attack chokepoints:
    ///   - `engine::attack::resolve_attack` for weapon swings.
    ///   - `spell_attack_outcome` in `actions/spells.rs` for spell
    ///     attacks.
    ///
    /// Both call sites pre-check `mode != RollMode::Disadvantage` to
    /// skip a wasted charge burn when the swing was already at
    /// disadvantage from a Protection / long-range / other source.
    pub fn apply_reactive_attack_disadvantage(
        &mut self,
        target_id: usize,
        attacker_id: usize,
    ) -> bool {
        for source in REACTIVE_ATTACK_DISADVANTAGE_SOURCES {
            if self.try_apply_reactive_disadvantage_source(target_id, attacker_id, source) {
                return true;
            }
        }
        false
    }

    /// Apply every reactive "tax the incoming attack roll" feature and
    /// return the resulting roll mode. The single chokepoint both attack
    /// paths use for the defender-side disadvantage lane, in RAW-priority
    /// order:
    ///
    ///   1. **Fighting Style: Protection** — an ally adjacent to the
    ///      target burns their reaction to impose disadvantage. Free
    ///      (no per-rest charge), so it goes first: a target with both a
    ///      Protection ally and a self-carried per-rest ward shouldn't
    ///      spend the scarce charge when the free reaction covers it.
    ///   2. The `REACTIVE_ATTACK_DISADVANTAGE_SOURCES` cohort (Warding
    ///      Flare, Entropic Ward) — skipped when the mode is already
    ///      disadvantage, since a second source adds no tax.
    ///
    /// Neither lane has a weapon-only qualifier in RAW — Protection is
    /// "when a creature you can see attacks a target other than you",
    /// the cohort rows are "when a creature attacks you" — so both fire
    /// on weapon swings and spell attacks alike. Keeping the pair in one
    /// helper is what makes that true by construction; when the two
    /// chokepoints each open-coded the sequence, Protection was on the
    /// weapon path only and a wizard's Fire Bolt walked past a
    /// protecting ally untaxed.
    ///
    /// Resolved here rather than inside `compute_attack_mode` because
    /// both lanes spend reactions and log, and `compute_attack_mode` is
    /// a `&self` read chokepoint.
    pub fn apply_reactive_attack_taxes(
        &mut self,
        attacker_id: usize,
        target_id: usize,
        tally: &mut RollModeTally,
    ) {
        use crate::engine::dice::RollMode;
        // Nothing on this lane can improve on a swing that is already
        // rolling at disadvantage — a second disadvantage source is the
        // same disadvantage — so the reaction is saved for a swing where
        // spending it changes the die.
        //
        // The gate used to be two narrower ones: a `Blinded`-attacker
        // short-circuit inside `first_eligible_protector`, and a
        // `mode != Disadvantage` test that guarded the second source but
        // not Protection itself. Both were reaching for this, and both
        // could only see one reason at a time; the tally can see all of
        // them, so a protector no longer spends a reaction on an
        // attacker who is Poisoned, Frightened, Restrained, shooting
        // into the dark, or swinging from a saddle they are falling out
        // of.
        if tally.has_disadvantage() {
            return;
        }
        if let Some(protector_id) = self.first_eligible_protector(attacker_id, target_id) {
            tally.add(RollMode::Disadvantage);
            if let Some(protector) = self.actors.get_mut(&protector_id) {
                protector.consume_resource(crate::engine::side_effects::Resource::Reaction);
            }
            self.log(
                "  protection: attack against target imposed disadvantage (protector's reaction spent)"
                    .to_string(),
            );
            return;
        }
        if self.apply_reactive_attack_disadvantage(target_id, attacker_id) {
            tally.add(RollMode::Disadvantage);
        }
    }

    /// Common gate + spend + log body for a single
    /// `ReactiveDisadvantageSource` row. Returns `true` when the gate
    /// passes and the reaction + per-rest charge are spent. Returns
    /// `false` when any gate fails (no tag, no charge, no reaction,
    /// downed; failing sight gate if the row requires it; failing
    /// range gate if the row carries one). Factored out so both the
    /// cohort-walking `apply_reactive_attack_disadvantage` AND the
    /// row-scoped compat wrappers (`apply_warding_flare_disadvantage`,
    /// `apply_entropic_ward_disadvantage`) share one gate implementation
    /// and one log-format shape.
    fn try_apply_reactive_disadvantage_source(
        &mut self,
        target_id: usize,
        attacker_id: usize,
        source: &ReactiveDisadvantageSource,
    ) -> bool {
        use crate::engine::side_effects::Resource;
        let Some(target) = self.actors.get(&target_id) else {
            return false;
        };
        if !target.has_passive_feature(source.tag)
            || !target.feature_available(source.tag)
            || !target.can_consume_resource(Resource::Reaction)
            || !target.is_combat_active()
        {
            return false;
        }
        // RAW "when a creature you can see..." gate — routes through the
        // shared `viewer_can_see` helper so both the Blinded clause AND
        // the "attacker is illusion-concealed and the target doesn't
        // pierce" clause land in one lookup. Sight gate is per-row:
        // Warding Flare requires it (RAW: "when a creature you can
        // see"); Entropic Ward does not (RAW: the patron's ward reads
        // the attacker's intent regardless of sight).
        if source.requires_sight && !self.viewer_can_see(target_id, attacker_id) {
            return false;
        }
        // Optional range gate — Warding Flare's 30ft cap collapses to
        // 12 tiles (2.5ft grid); Entropic Ward's RAW is un-ranged and
        // rides `None`.
        if let Some(range) = source.range_tiles
            && self
                .footprint_distance(target_id, attacker_id)
                .is_none_or(|d| d > range)
        {
            return false;
        }
        // Snapshot names before the mutable spend so the log line reads
        // cleanly. The reaction + charge spend and log both fire off
        // the same &mut borrow.
        let (target_name, attacker_name) = {
            let target = match self.actors.get_mut(&target_id) {
                Some(a) => a,
                None => return false,
            };
            target.spend_feature(source.tag);
            target.consume_resource(Resource::Reaction);
            let tn = target.name().to_string();
            let an = self
                .actors
                .get(&attacker_id)
                .map(|a| a.name().to_string())
                .unwrap_or_default();
            (tn, an)
        };
        self.log(format!(
            "  {}: {} imposes disadvantage on {}'s attack",
            source.log_label, target_name, attacker_name
        ));
        true
    }

    /// Shared post-hit interception chokepoint. Called by both attack
    /// paths — `engine::attack::resolve_attack_outcome` (weapon swings)
    /// and `spells::spell_attack_outcome` (spell attacks) — once a
    /// swing is known to connect but before any damage is rolled or any
    /// "you were hit" bookkeeping is written. A `true` return means
    /// some target-side effect ate the swing whole: the caller treats
    /// it as a miss, rolls no damage, fires no riders, and does not
    /// mark the target as hit.
    ///
    /// This is the "the blow lands on something that isn't you" lane,
    /// distinct from the two neighbouring defensive lanes:
    ///
    ///   - **Pre-roll attack-mode taxes** (Protection, Warding Flare,
    ///     Entropic Ward) run before the d20 and only bend the odds.
    ///   - **Post-hit damage reducers** (Uncanny Dodge, Deflect
    ///     Missiles, Parry) run after this and clamp the number; the
    ///     hit still counts as a hit for every rider that reads it.
    ///
    /// Interception sits between them and is the only lane that
    /// retroactively un-hits a connected swing.
    ///
    /// Rows are consulted in order and the first to fire wins — every
    /// row fully negates the attack, so running a second would spend a
    /// resource for nothing. Cheapest-resource-first is therefore the
    /// ordering rule:
    ///
    ///   1. **Mirror Image** — decoys already paid for by a cast spell,
    ///      consumed passively, several available per cast. Cannot stop
    ///      a crit.
    ///   2. **Illusory Self** (Illusion Wizard lv10) — one charge per
    ///      short rest *and* the target's reaction for the round. Stops
    ///      anything, crits included.
    ///   3. **Armor of Hexes** (Hexblade Warlock lv10) — free, but only
    ///      against the hexblade's own cursed quarry, and only on a d6
    ///      that comes up 4 or better.
    ///
    /// So a decoy soaks the swing when one is available and the hit
    /// isn't a crit, and the illusionist's per-rest charge is held for
    /// what gets through.
    ///
    /// Armor of Hexes costs nothing at all, which by the
    /// cheapest-first rule would put it top of the list. It goes last
    /// instead, because it is the only row that can decline: the two
    /// above it always eat the swing, this one eats half of them. A free
    /// coin-flip run *first* would spend the reliable resources on the
    /// swings it happened to lose, which is exactly backwards — so the
    /// certain rows go first and the coin-flip catches the remainder.
    /// Ordering by reliability and ordering by cost agree on the first
    /// two rows and disagree on the third; reliability wins.
    ///
    /// A future interception ("Instinctive Charm", a Shield Guardian's
    /// redirect) drops in as a fourth row.
    pub fn attack_intercepted(
        &mut self,
        target_id: usize,
        attacker_id: usize,
        is_crit: bool,
    ) -> bool {
        self.mirror_image_deflect(target_id, is_crit)
            || self.illusory_self_deflect(target_id, attacker_id)
            || self.armor_of_hexes_deflect(target_id, attacker_id)
    }

    /// 5e Hexblade Warlock **Armor of Hexes** (subclass level 10) — the
    /// hexblade's cursed quarry swings at them and the curse turns the
    /// blow aside on a d6 of 4 or better. Third row of the
    /// `attack_intercepted` cohort; see `ARMOR_OF_HEXES_TAG`.
    ///
    /// Returns `true` when the attack is negated (the caller treats it
    /// as a miss and rolls no damage).
    ///
    /// Three gates, all of them RAW:
    ///   - The defender holds the feature and is combat-active.
    ///   - The attacker is *the* creature this defender cursed — read
    ///     through `hexblade_curse_holder`, so an ally of the quarry,
    ///     or the quarry after the curse has timed out, gets nothing.
    ///   - The d6 comes up 4+.
    ///
    /// Costs nothing on a failure, which is why it needs no charge and
    /// no reaction: RAW's price for the feature is that the hexblade had
    /// to spend a bonus action and a rest charge naming this creature in
    /// the first place, and it only ever protects against that one
    /// creature. Crits are *not* exempt — RAW says the attack "misses
    /// you" with no carve-out, unlike Mirror Image's explicit one.
    pub fn armor_of_hexes_deflect(&mut self, target_id: usize, attacker_id: usize) -> bool {
        if self.hexblade_curse_holder(attacker_id) != Some(target_id) {
            return false;
        }
        let Some(target) = self.actors.get(&target_id) else {
            return false;
        };
        if !target.is_combat_active()
            || !target.has_passive_feature(crate::actions::class_features::ARMOR_OF_HEXES_TAG)
        {
            return false;
        }
        let target_name = target.name().to_string();
        let roll = self.roll(&crate::engine::dice::Dice::new(1, 6)) as i32;
        if roll < 4 {
            self.log(format!(
                "  armor of hexes: 1d6({}) — the curse fails to turn the blow from {}.",
                roll,
                self.actor_name(attacker_id)
            ));
            return false;
        }
        self.log(format!(
            "  armor of hexes: 1d6({}) — {}'s curse turns {}'s attack aside; it misses.",
            roll,
            target_name,
            self.actor_name(attacker_id)
        ));
        true
    }

    /// 5e Illusion Wizard **Illusory Self** (subclass level 10) — the
    /// illusionist interposes a duplicate of themselves and the attack
    /// automatically misses. Second row of the `attack_intercepted`
    /// cohort; see `ILLUSORY_SELF_TAG` for the RAW text and for why
    /// this fires after the hit is known rather than before the roll.
    ///
    /// Gates, in order: the target holds the passive tag, has an
    /// unspent per-rest charge, and has an unspent reaction. Both the
    /// charge and the reaction are spent on fire.
    ///
    /// Deliberately **not** gated on sight. The three sibling reactive
    /// defenses (Warding Flare, Entropic Ward, Uncanny Dodge) route
    /// through `viewer_can_see` because each RAW text keys off
    /// perceiving the attacker; Illusory Self's does not — the
    /// duplicate is a standing illusion of the wizard, and RAW asks
    /// only that an attack roll be made. An illusionist ambushed by an
    /// invisible attacker still has the decoy standing beside them.
    ///
    /// Returns `true` iff the charge fired and the caller should treat
    /// the connecting swing as a miss.
    pub fn illusory_self_deflect(&mut self, target_id: usize, attacker_id: usize) -> bool {
        let tag = crate::actions::class_features::ILLUSORY_SELF_TAG;
        let Some(target) = self.actors.get(&target_id) else {
            return false;
        };
        if !target.is_combat_active()
            || !target.has_passive_feature(tag)
            || !target.feature_available(tag)
            || !target.has_reaction()
        {
            return false;
        }
        let target_name = target.name().to_string();
        if let Some(t) = self.actors.get_mut(&target_id) {
            t.spend_feature(tag);
            t.consume_resource(crate::engine::side_effects::Resource::Reaction);
        }
        let attacker_name = self
            .actors
            .get(&attacker_id)
            .map(|a| a.name().to_string())
            .unwrap_or_default();
        self.log(format!(
            "  illusory self: {} interposes an illusory duplicate \u{2014} {}'s attack automatically misses",
            target_name, attacker_name
        ));
        true
    }

    /// 5e Mirror Image deflection check. With N duplicates remaining on
    /// the target, roll a d20 against a threshold (RAW: 6+ for 3, 8+ for
    /// 2, 11+ for 1) to determine whether the swing pops a decoy and
    /// misses the caster outright. Crits bypass the deflection.
    ///
    /// Returns `true` if the attack was deflected onto a duplicate (the
    /// caller should treat the hit as a miss). Returns `false` if the
    /// attack found the real target, the target had no images, or the
    /// hit was a crit. Logs the deflection roll on either branch.
    ///
    /// Centralized so weapon attacks (`resolve_attack` in attack.rs) and
    /// spell attacks (`spell_attack_outcome` in spells.rs) share the
    /// same dispatch — Mirror Image RAW applies to any "attack roll
    /// against you", not just weapon swings.
    pub fn mirror_image_deflect(&mut self, target_id: usize, is_crit: bool) -> bool {
        if is_crit {
            return false;
        }
        let Some(target) = self.actors.get(&target_id) else {
            return false;
        };
        let images = target.mirror_images();
        if images == 0 {
            return false;
        }
        let dup_threshold: i32 = if images >= 3 {
            6
        } else if images == 2 {
            8
        } else {
            11
        };
        let dup_roll = self.roll(&crate::engine::dice::Dice::new(1, 20)) as i32;
        if dup_roll >= dup_threshold {
            if let Some(t) = self.actors.get_mut(&target_id) {
                t.pop_mirror_image();
            }
            let remaining = self
                .actors
                .get(&target_id)
                .map(|a| a.mirror_images())
                .unwrap_or(0);
            self.log(format!(
                "  mirror image: 1d20({}) \u{2265} {} \u{2014} attack strikes a duplicate ({} left)",
                dup_roll, dup_threshold, remaining
            ));
            return true;
        }
        self.log(format!(
            "  mirror image: 1d20({}) < {} \u{2014} attack finds the real target",
            dup_roll, dup_threshold
        ));
        false
    }

    /// Shared implementation: true iff `caster_id` is concentrating on the
    /// spell named `spell_name` and `target_id` is the actor tagged with
    /// `mark_condition` inside that concentration data. Symmetric across
    /// every "mark the target, +Xd6 on hits" pattern (Hunter's Mark, Hex,
    /// future Hex-like spells).
    pub fn is_concentration_mark_target(
        &self,
        caster_id: usize,
        target_id: usize,
        spell_name: &str,
        mark_condition: Condition,
    ) -> bool {
        let Some(caster) = self.actors.get(&caster_id) else {
            return false;
        };
        let Some(conc) = caster.concentration() else {
            return false;
        };
        if conc.spell_name != spell_name {
            return false;
        }
        conc.conditions
            .iter()
            .any(|(tid, c)| *tid == target_id && *c == mark_condition)
    }

    /// True iff `caster_id` is concentrating on Hunter's Mark and the
    /// current target is the marked one. Folded into weapon hits by
    /// `resolve_attack` to add the +1d6 mark rider.
    pub fn is_hunters_mark_target(&self, caster_id: usize, target_id: usize) -> bool {
        self.is_concentration_mark_target(
            caster_id,
            target_id,
            "Hunter's Mark",
            Condition::HuntersMarked,
        )
    }

    /// True if `caster_id` is concentrating on Hex and `target_id` is the
    /// hex'd creature. Symmetric with `is_hunters_mark_target` — the
    /// attack-roll resolver layers an extra 1d6 necrotic per RAW.
    pub fn is_hex_target(&self, caster_id: usize, target_id: usize) -> bool {
        self.is_concentration_mark_target(caster_id, target_id, "Hex", Condition::Hexed)
    }

    /// True if somebody is close enough, awake enough and sighted
    /// enough to spoil `actor_id`'s aim — 5e's **Ranged Attacks in
    /// Close Combat** (PHB p.195):
    ///
    /// > You have disadvantage on a ranged attack roll if you are
    /// > within 5 feet of a hostile creature **that can see you and
    /// > that isn't Incapacitated**.
    ///
    /// The two emphasised clauses are the whole reason this is not a
    /// bare `!combat_active_enemy_ids_adjacent(...).is_empty()`, which
    /// is what it used to be. That version asked only "is anything
    /// hostile standing next to me", and the sentence RAW wrote is
    /// three questions:
    ///
    ///   1. **Adjacent** — the shared geometry, unchanged, read off
    ///      the public helper so the "five feet" here and the five feet
    ///      Ashardalon's Stride scorches stay the same distance.
    ///   2. **Not Incapacitated** — a paralyzed ogre standing in
    ///      contact is not crowding anybody. `is_incapacitated` reads
    ///      the whole action-economy cohort, so Stunned, Unconscious,
    ///      Asleep, Petrified, Surprised and Banished all stop
    ///      spoiling the shot too, which is RAW: every one of them is
    ///      Incapacitated by definition.
    ///   3. **Can see you** — the clause the old gate got most
    ///      visibly wrong. An archer who has just turned Invisible, or
    ///      who is shooting out of a fog bank, or who is standing in
    ///      the dark beside a guard with no darkvision, was still
    ///      taxed for being crowded by a creature that has no idea
    ///      where they are.
    ///
    /// Clause 3 also composes correctly with the rest of the tally
    /// rather than double-counting against it. When the adjacent
    /// creature *is* the target, `attack_mode_tally` has already
    /// handed the shooter Advantage for the target's blindness through
    /// `sight_denied_between`; dropping the crowding disadvantage on
    /// top of that is not the same clause twice but RAW's two separate
    /// sentences agreeing, and the tally keeps them distinct because it
    /// counts sources rather than folding them.
    ///
    /// Note the asymmetry with `combat_active_enemy_ids_adjacent`'s
    /// other two callers, and why this filter does not belong on the
    /// shared helper: Ashardalon's Stride burns whoever is standing in
    /// the fire whether or not they can see the caster, and a horse
    /// deciding whether it is safe to stand up cares that something
    /// hostile is there, not that it is looking.
    fn ranged_attack_is_crowded(&self, actor_id: usize) -> bool {
        self.combat_active_enemy_ids_adjacent(actor_id)
            .into_iter()
            .any(|enemy_id| {
                self.actors
                    .get(&enemy_id)
                    .is_some_and(|e| !e.is_incapacitated())
                    && self.viewer_can_see(enemy_id, actor_id)
            })
    }

    /// True if `actor_id` exists AND is not currently concentrating on a
    /// spell. The canonical "don't burn a slot to replace our own
    /// concentration" gate used by every concentration-bound spell's
    /// `custom_validate_input`. Centralizes the
    /// `encounter.actors.get(&caster_id).is_some_and(|a| !a.is_concentrating())`
    /// idiom (~13 spell sites) into a single chokepoint so a future
    /// rule change (e.g. War Caster feat granting a concentration
    /// re-cast hook) lands in one place.
    pub fn caster_can_concentrate(&self, caster_id: usize) -> bool {
        self.actors
            .get(&caster_id)
            .is_some_and(|a| !a.is_concentrating())
    }

    /// End the actor's concentration (if any) and roll back every
    /// condition / buff that concentration installed. Logs the drop and
    /// each cleared effect. No-op if the actor isn't concentrating.
    pub fn drop_concentration(&mut self, actor_id: usize) {
        let Some(actor) = self.actors.get_mut(&actor_id) else {
            return;
        };
        let Some(data) = actor.end_concentration() else {
            return;
        };
        let actor_name = actor.name().to_string();
        let spell_name = data.spell_name.clone();
        self.log(format!(
            "{}'s concentration on {} ends.",
            actor_name, spell_name
        ));
        // The map half of the rollback. Conditions come off their
        // targets below; a concentration-held area comes off the board
        // here, and the two are the same event seen from either side of
        // a spell that has both (Web restrains creatures *and* clings to
        // the floor).
        self.release_map_layers_of(actor_id);
        for (target_id, condition) in data.conditions {
            // 5e Conjure Animals / Conjure Elemental cleanup: the
            // summoned minion holds the `Conjured` flag, and dropping
            // concentration dispels it outright (the spell ends). Vanish
            // the actor instead of just stripping the marker — leaving
            // them around as a free-team-member would warp the encounter
            // balance after the spell drops.
            if condition == Condition::Conjured {
                self.despawn_actor(target_id, "vanishes as the conjuration ends");
                continue;
            }
            let Some(target) = self.actors.get_mut(&target_id) else {
                continue;
            };
            let target_name = target.name().to_string();
            if target.remove_condition(condition) {
                self.log(format!("{} is no longer {}.", target_name, condition.name()));
            }
        }
        // Negate any flat buffs the spell installed (Bless, etc.). The
        // delta stored is the original adjustment; we subtract it to
        // restore the actor's pre-spell stats. Each lane (attack / save /
        // damage) walks the same `(target_id, delta)` vec, so the
        // negation routes through `rollback_buffs` with a per-lane
        // setter — adding a new buff lane (e.g. an AC delta) is one
        // call here plus the new vec field on `ConcentrationData`.
        self.rollback_buffs(&data.attack_buffs, |a, d| a.add_attack_bonus_buff(d));
        self.rollback_buffs(&data.save_buffs, |a, d| a.add_save_bonus_buff(d));
        self.rollback_buffs(&data.damage_buffs, |a, d| a.add_damage_bonus_buff(d));
    }

    /// Walk a `(target_id, delta)` buff vec and apply the *negated*
    /// delta to each target via `setter`. Missing targets (despawned
    /// since the buff installed) are skipped silently — the buff was
    /// already lost with the actor. Shared by every concentration-buff
    /// lane in `drop_concentration` (attack / save / damage today; any
    /// future flat-buff lane lands as one call instead of a hand-rolled
    /// for-loop).
    fn rollback_buffs<F>(&mut self, buffs: &[(usize, i32)], setter: F)
    where
        F: Fn(&mut ActorInstance, i32),
    {
        for &(target_id, delta) in buffs {
            if let Some(target) = self.actors.get_mut(&target_id) {
                setter(target, -delta);
            }
        }
    }

    /// Apply every condition-keyed round-end DoT on `actor_id` whose
    /// flag is set, in `ROUND_END_DOTS` declaration order. Each entry
    /// rolls the dice fresh, logs one line, and applies the damage
    /// through `DealDamage` so resistance / immunity / temp HP / death
    /// saves all route through the standard pipeline. Adding a new
    /// condition-based DoT (e.g. Earthen Grasp's 2d6 bludgeoning, the
    /// Vitriolic Sphere drip) is a one-line table entry in
    /// `ROUND_END_DOTS` rather than a hand-rolled if-block.
    fn apply_condition_round_end_dots(&mut self, actor_id: usize) {
        use crate::engine::side_effects::{ApplicableSideEffect, DealDamage, Heal};
        for dot in ROUND_END_DOTS {
            let has = self
                .actors
                .get(&actor_id)
                .is_some_and(|a| a.has_condition(dot.condition));
            if !has {
                continue;
            }
            let dmg = self.roll(&dot.dice);
            let name = self.actor_name(actor_id);
            self.log(format!(
                "  {} {}: {}({}) {:?}",
                name, dot.log_verb, dot.dice, dmg, dot.damage_type
            ));
            // Snapshotted across the hit rather than read off the roll,
            // so a draining tether is fed by what the victim actually
            // lost — see `RoundEndDot::drains_to_owner`. Temp HP counts:
            // it is hit points the damage consumed.
            let pool_before = dot.drains_to_owner.then(|| self.damage_pool_of(actor_id));
            DealDamage {
                actor_id,
                amount: dmg,
                damage_type: dot.damage_type,
            }
            .apply(self);
            let Some(before) = pool_before else {
                continue;
            };
            let dealt = before.saturating_sub(self.damage_pool_of(actor_id));
            let Some(owner_id) = self.find_concentration_owner(actor_id, dot.condition) else {
                continue;
            };
            let drained = dealt / 2;
            if drained == 0 {
                continue;
            }
            let owner_name = self.actor_name(owner_id);
            self.log(format!("  {} drains {} hit points back.", owner_name, drained));
            Heal {
                actor_id: owner_id,
                amount: drained,
            }
            .apply(self);
        }
    }

    /// Current hit points plus temporary hit points — the pool damage
    /// eats through, and therefore the quantity to difference across a
    /// hit when a caller needs to know how much damage actually landed
    /// after resistance, immunity and absorption have had their say.
    ///
    /// Zero for an actor who is no longer on the board, which makes the
    /// difference against a snapshot degrade to "everything they had"
    /// rather than to a panic.
    fn damage_pool_of(&self, actor_id: usize) -> u32 {
        self.actors
            .get(&actor_id)
            .map(|a| a.hitpoints().saturating_add(a.temp_hp()))
            .unwrap_or(0)
    }

    /// 5e's two ways out of a grapple that aren't an escape check:
    /// "the grapple ends if the grappler is incapacitated", and it ends
    /// "if an effect removes the grappled creature from the reach of the
    /// grappler". Lift `actor_id`'s `Grappled` when the creature named
    /// by its back-link can no longer hold on — gone from the board, out
    /// of the fight, under any condition that blocks their action
    /// economy, or simply no longer within arm's length.
    ///
    /// The reach clause is not a nicety. A grappled creature's speed is
    /// 0, so it cannot walk out of the hold; the grappler, however, can
    /// walk away, and every teleport and shove in the game can separate
    /// the two. Without this the pair would drift apart and the victim
    /// would stay pinned to the floor by a creature on the far side of
    /// the map until the timer ran out.
    ///
    /// A `Grappled` with no back-link is left alone. Those come from
    /// spells and monster abilities that pin a target with something
    /// other than a pair of hands (Evard's tentacles, an ooze's
    /// adhesive, Earthen Grasp's fist), and RAW ends each of those on
    /// its own terms — a concentration drop, a timer, an escape check —
    /// not on anybody's condition.
    fn release_broken_grapples(&mut self, actor_id: usize) {
        let Some(grappler_id) = self
            .actors
            .get(&actor_id)
            .and_then(|a| a.linked_by(Condition::Grappled))
        else {
            return;
        };
        let able = self.actors.get(&grappler_id).is_some_and(|g| {
            g.is_combat_active()
                && !g
                    .conditions()
                    .keys()
                    .any(|c| c.blocks_action_economy())
        });
        // Reach is read off the grappler's own melee envelope rather
        // than assumed to be one tile, because the creatures that
        // grapple are disproportionately the long-armed ones — a giant
        // crocodile's jaws close at two tiles and its grip should not
        // lapse the moment its victim is a tile and a half away.
        let in_reach = able
            && self
                .actors
                .get(&grappler_id)
                .and_then(|g| g.first_melee_weapon_action())
                .and_then(|a| a.reach_tiles())
                .max(Some(crate::actions::action_template::MELEE_REACH))
                .zip(self.footprint_distance(grappler_id, actor_id))
                .is_some_and(|(reach, gap)| gap <= reach);
        if able && in_reach {
            return;
        }
        let name = self.actor_name(actor_id);
        let grappler_name = self.actor_name(grappler_id);
        if let Some(a) = self.actors.get_mut(&actor_id) {
            a.remove_condition(Condition::Grappled);
        }
        self.log(if able {
            format!(
                "  {} is out of {}'s reach \u{2014} the grapple ends.",
                name, grappler_name
            )
        } else {
            format!(
                "  {} can no longer hold on \u{2014} {} slips out of the grapple.",
                grappler_name, name
            )
        });
    }

    /// 5e repeated saves: at the end of each turn, targets of certain
    /// hold / control spells get to repeat the saving throw. On a pass
    /// the condition is removed and the caster's concentration (if
    /// anchored to the same condition) is dropped. Only fires when the
    /// condition came from a concentration spell — permanent or timer-only
    /// applications (e.g. monster innate stun) don't grant repeated saves.
    fn apply_round_end_saves(&mut self, actor_id: usize) {
        for entry in ROUND_END_SAVES {
            let has = self
                .actors
                .get(&actor_id)
                .is_some_and(|a| a.has_condition(entry.condition) && a.is_combat_active());
            if !has {
                continue;
            }
            // Find the caster whose concentration anchors this condition on
            // the target. If no caster holds concentration keyed to this
            // condition on this actor, it's a non-spell source (monster
            // ability, permanent) — skip the repeated save.
            let caster_id = self.find_concentration_owner(actor_id, entry.condition);
            let Some(cid) = caster_id else {
                continue;
            };
            let dc = self
                .actors
                .get(&cid)
                .map(|a| a.best_spell_save_dc([
                    crate::engine::types::AbilityScoreType::Wisdom,
                    crate::engine::types::AbilityScoreType::Charisma,
                    crate::engine::types::AbilityScoreType::Intelligence,
                ]))
                .unwrap_or(13);
            let name = self.actor_name(actor_id);
            self.log(format!("  {} {}", name, entry.log_verb));
            let save = self.roll_save(actor_id, entry.save_ability, dc);
            if save.passed() {
                if let Some(actor) = self.actors.get_mut(&actor_id) {
                    actor.remove_condition(entry.condition);
                }
                self.log(format!("  {} breaks free!", name));
                self.drop_concentration(cid);
            }
        }
    }

    /// Find the actor who is concentrating on a spell that installed
    /// `condition` on `target_id`. Returns `None` if no such caster
    /// exists (the condition came from a non-concentration source).
    fn find_concentration_owner(
        &self,
        target_id: usize,
        condition: Condition,
    ) -> Option<usize> {
        for (&aid, actor) in &self.actors {
            if let Some(conc) = actor.concentration()
                && conc
                    .conditions
                    .iter()
                    .any(|&(tid, c)| tid == target_id && c == condition)
            {
                return Some(aid);
            }
        }
        None
    }

    /// 5e **Suffocation**, one actor, one round — RAW: *"when a
    /// creature runs out of breath or is choking, it gains 1 Exhaustion
    /// level at the end of each of its turns. When a creature can
    /// breathe again, it removes all levels of Exhaustion it gained
    /// from suffocating."* See `engine::breath` for the rule in full.
    ///
    /// Both halves of that sentence live here because they are one
    /// decision: `can_breathe` is asked once and the answer sends the
    /// actor down one branch or the other. Splitting them would mean
    /// two sweeps asking the same question of the same actor in the
    /// same round and having to agree.
    ///
    /// Runs from `round_end`, which is where this engine keeps "at the
    /// end of each of its turns" for every effect that has one — the
    /// initiative order has already been walked, so every actor gets
    /// exactly one tick per round in a deterministic order.
    ///
    /// Gated on the body being present and not already a corpse, which
    /// is deliberately *wider* than the `is_combat_active()` every
    /// other round-end sweep uses: a creature face-down at the bottom
    /// of a pool is the case the drowning rule exists for, and RAW
    /// keeps counting. What it excludes is the dead and the
    /// banished — one has nothing left to lose and the other is not in
    /// the water.
    fn tick_breath(&mut self, actor_id: usize) {
        let present = self
            .actors
            .get(&actor_id)
            .is_some_and(|a| {
                !a.is_off_board()
                    && !matches!(
                        a.hp_state(),
                        crate::actors::actor_template::HpState::Dead
                    )
            });
        if !present {
            return;
        }
        if self.can_breathe(actor_id) {
            let shed = self
                .actors
                .get_mut(&actor_id)
                .map(|a| a.refill_breath())
                .unwrap_or(0);
            if shed > 0 {
                let name = self.actor_name(actor_id);
                self.log(format!(
                    "  {} gets its breath back and sheds {} level{} of exhaustion.",
                    name,
                    shed,
                    if shed == 1 { "" } else { "s" }
                ));
            }
            return;
        }
        // Two ways to be out of breath, and they are two different
        // rules rather than two flavours of one: RAW's "runs out of
        // breath **or** is choking" gives the second no grace period,
        // so a blocked airway skips the held-breath clock entirely.
        // See `ActorInstance::spend_breath`.
        let choking = self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.has_condition(Condition::Choking));
        let Some(tier) = self
            .actors
            .get_mut(&actor_id)
            .and_then(|a| a.spend_breath(choking))
        else {
            return;
        };
        let name = self.actor_name(actor_id);
        // …and two different pictures. The log is the only place a
        // player can tell them apart, since the rung is the same either
        // way.
        let how = if choking { "is choking" } else { "is drowning" };
        self.log(format!("  {} {} \u{2014} exhaustion {}.", name, how, tier));
        // Tier 6 is death, and `gain_exhaustion` has already set the
        // state; `round_end`'s own `cleanup_dead_actors` sweeps the body
        // a few lines later, the same as for any other round-end kill.
        if tier >= crate::actors::actor_template::EXHAUSTION_DEATH_TIER {
            self.log(format!("  {} stops breathing.", name));
        }
    }

    /// 5e Spirit Guardians aura: if `actor_id` has the `SpiritGuarding`
    /// condition, every hostile creature within 6 tiles takes 3d8 radiant
    /// damage (WIS save for half). Called at round-end for each actor.
    fn apply_spirit_guardians_aura(&mut self, caster_id: usize) {
        use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};
        use crate::engine::types::AbilityScoreType;
        let has = self
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_condition(Condition::SpiritGuarding) && a.is_combat_active());
        if !has {
            return;
        }
        let caster_loc = match self.actors.get(&caster_id) {
            Some(a) => a.location(),
            None => return,
        };
        let dc = self
            .actors
            .get(&caster_id)
            .map(|a| a.best_spell_save_dc([AbilityScoreType::Wisdom, AbilityScoreType::Charisma]))
            .unwrap_or(13);
        let targets = self.enemy_burst_targets(caster_id, caster_loc, 6);
        if targets.is_empty() {
            return;
        }
        let dmg = self.roll(&Dice::new(3, 8));
        let caster_name = self.actor_name(caster_id);
        self.log(format!(
            "  {}'s spirit guardians lash out: 3d8({}) radiant",
            caster_name, dmg
        ));
        for tid in targets {
            let save = self.roll_save(tid, AbilityScoreType::Wisdom, dc);
            let actual = if save.passed() { dmg / 2 } else { dmg };
            if actual > 0 {
                DealDamage {
                    actor_id: tid,
                    amount: actual,
                    damage_type: DamageType::Radiant,
                }
                .apply(self);
            }
        }
    }

    /// Tick condition timers on every actor. `Rounds(n)` becomes
    /// `Rounds(n-1)`; `Rounds(0|1)` removes the condition. Logs each
    /// expiration. Iterates by sorted id for deterministic ordering.
    /// Runs condition DoTs (Burning / Heat Metal / Earthen Grasp /
    /// Vitriolic Sphere drip) before timer ticks so a final-round
    /// expiry still pays the drip — matches 5e DoT timing.
    fn round_end(&mut self) {
        let mut ids: Vec<usize> = self.actors.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            // Run every condition-triggered round-end DoT through the
            // central table. Order in `ROUND_END_DOTS` is the order in
            // which damage rolls — keeps logs deterministic. Damage
            // lands before timer ticks so a final-round expiry still
            // pays the drip (matches 5e DoT timing).
            self.apply_condition_round_end_dots(id);
            // 5e Spirit Guardians aura: at round-end, every hostile
            // creature within 6 tiles of a SpiritGuarding caster takes
            // 3d8 radiant (WIS save for half). We iterate the aura here
            // so it fires once per round alongside the other DoTs.
            self.apply_spirit_guardians_aura(id);
            // 5e repeated saves: Hold Person / Hold Monster / Hideous
            // Laughter grant the target a save at the end of each turn.
            // On a pass, the hold breaks and the caster's concentration
            // drops. Runs after DoTs so the damage for this round has
            // already landed; matches RAW timing.
            self.apply_round_end_saves(id);
            // 5e: "the grapple ends if the grappler is incapacitated."
            // Checked here rather than at the moment the grappler goes
            // down, because the ways to become incapacitated are many
            // (Stunned, Paralyzed, Unconscious, Hold Person, a hundred
            // spells) and the ways to stop being incapacitated are just
            // as many — a round-end sweep catches all of them without
            // any of them having to know grapples exist.
            self.release_broken_grapples(id);
            // 5e Suffocation. After the sweeps above rather than before,
            // and the order is load-bearing in one direction: a rug
            // whose grapple broke this round has stopped smothering its
            // victim, and the victim should not pay a rung of exhaustion
            // for a hold that is already over.
            self.tick_breath(id);
            // Regeneration: heal `regen_per_round` HP at end-of-round if
            // the actor is combat-active and hasn't been hit by a
            // suppressor damage type this round (5e troll: fire/acid).
            // Suppression resets after every round-end whether or not a
            // heal happened, so a single fire hit only lasts one round.
            if let Some(actor) = self.actors.get_mut(&id) {
                let amt = actor.regen_per_round();
                let suppressed = actor.regen_suppressed();
                if amt > 0 && actor.is_combat_active() {
                    let name = actor.name().to_string();
                    if suppressed {
                        self.log(format!("  {}'s regeneration is suppressed.", name));
                    } else if actor.is_wounded() {
                        let outcome = actor.heal(amt);
                        if matches!(
                            outcome,
                            crate::actors::actor_template::HealOutcome::Healed
                        ) {
                            self.log(format!("  {} regenerates {} HP.", name, amt));
                        }
                    }
                }
                if let Some(a) = self.actors.get_mut(&id) {
                    a.clear_regen_suppression();
                }
            }
            let Some(actor) = self.actors.get_mut(&id) else {
                continue;
            };
            let name = actor.name().to_string();
            // Single source of truth for round-end timer expiration.
            // tick_condition_timers handles every Rounds(n) condition,
            // including Blessed / ShieldOfFaith, and reports each
            // exact expiry so we don't double-log or false-positive.
            let expired = actor.tick_condition_timers();
            // Snapshot legendary-action state before dropping the mutable
            // borrow so the self.log calls below can proceed.
            let legendary_spent = actor.legendary_actions_per_round() > 0
                && !actor.can_consume_resource(crate::engine::side_effects::Resource::LegendaryAction);
            for c in expired {
                self.log(format!("{} is no longer {}.", name, c.name()));
            }
            // Cosmetic debug aid: flag when a legendary creature has burned
            // through all of its legendary action points for the round.
            if legendary_spent {
                self.log(format!("{}'s legendary actions are spent.", name));
            }
        }
        self.cleanup_dead_actors();
        // Round-end timers just expired, and one of the things they
        // expire is a banishment. Ahead of the other two sweeps for the
        // same reason as at the turn-start chokepoint: they measure
        // bodies, and this is the one that decides which bodies there
        // are.
        self.reconcile_board_presence();
        // Round-end timers just expired; anything that was holding a
        // creature at a larger size has now let go of it.
        self.reconcile_footprints();
        // The map layers' own timers, ticked alongside the actors'.
        // Last rather than first so a zone in its final round still
        // charged everyone who stood in it this round before it
        // disperses, and a wall in its last round still stood in
        // somebody's way.
        self.tick_zones();
        self.tick_conjured_terrain();
        self.tick_light_sources();
        // Every timer has now ticked, so this is the first moment at
        // which "does this caster still have a spell up" has a stable
        // answer.
        self.release_concentration_with_nothing_left();
        // Dead last in the round, and it has to be: the sweep above is
        // the one that ends a Fly whose anchor lapsed, and a caster
        // released a line earlier is a flier who is still in the air on
        // this line. Anything ordered before it would leave the drop to
        // wait a full round.
        self.reconcile_altitudes();
    }

    /// Release any concentration whose last anchor lapsed on a timer.
    ///
    /// Concentration used to end only when something *happened* to the
    /// caster — damage broke the grip, a second concentration spell
    /// replaced it, the caster died, a target broke free. Nothing ended
    /// it when the spell simply ran out, and every concentration spell
    /// in the game runs out: a Web's zone disperses after ten rounds, a
    /// Fear's Frightened cohort ticks to zero, a Wall of Fire crumbles.
    /// The caster went on "concentrating" on a spell with nothing left
    /// of it for the rest of the encounter, and since
    /// `caster_can_concentrate` is what gates a concentration cast, they
    /// could never cast one again. One Fear at round two cost a wizard
    /// every Web, Haste, Slow and Hold Monster for the rest of the
    /// fight, and the log said nothing.
    ///
    /// Two shapes of anchor, and both have to be gone:
    ///
    ///   - **Map layers.** A zone or a conjured patch that declared
    ///     itself concentration-held. Reached through
    ///     `pending_concentration_review`, which the two tick routines
    ///     fill with the owners of anything that just expired — so a
    ///     caster is only asked about at all when something of theirs
    ///     actually ran out, and never merely for holding a spell.
    ///   - **Tracked conditions.** The `(target, condition)` pairs the
    ///     cast recorded on its `ConcentrationData`. Swept every round
    ///     rather than reported, because a condition can leave for
    ///     several reasons that have no single chokepoint — its timer,
    ///     a cleanse, an immunity that bounced the install at cast time.
    ///
    /// A `ConcentrationData` that recorded no conditions and holds no
    /// map layer is deliberately left alone. That combination is the
    /// untracked-aura shape — Crusader's Mantle, Mordenkainen's Sword,
    /// Crown of Stars — whose whole effect is the concentration marker
    /// itself, and which has no anchor to lose. Nothing here can tell
    /// "the marker is the spell" apart from "the anchors are gone", so
    /// the sweep declines to guess and the queue is what reaches the
    /// map-layer cases the sweep cannot see.
    fn release_concentration_with_nothing_left(&mut self) {
        let reported: Vec<usize> = std::mem::take(&mut self.pending_concentration_review);
        let mut doomed: Vec<usize> = Vec::new();
        let mut ids: Vec<usize> = self.actors.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let Some(data) = self.actors.get(&id).and_then(|a| a.concentration()).cloned()
            else {
                continue;
            };
            let tracks_conditions = !data.conditions.is_empty();
            if !tracks_conditions && !reported.contains(&id) {
                // Neither anchor is in play for this caster: either they
                // hold an untracked aura, or their map layers are all
                // still standing. Nothing to review.
                continue;
            }
            let condition_alive = data.conditions.iter().any(|&(target_id, condition)| {
                self.actors
                    .get(&target_id)
                    .is_some_and(|t| t.has_condition(condition))
            });
            if condition_alive || self.sustains_concentration_layer(id) {
                continue;
            }
            doomed.push(id);
        }
        for id in doomed {
            self.drop_concentration(id);
        }
    }

    /// Whether `actor_id` is still holding up any concentration-bound
    /// map layer — a persistent area or a conjured patch.
    ///
    /// The read-side counterpart of `release_map_layers_of`, which is
    /// the write side of the same question, and phrased over both layers
    /// for the same reason: a spell that put down one of each is still
    /// up while either stands.
    fn sustains_concentration_layer(&self, actor_id: usize) -> bool {
        self.zones
            .iter()
            .any(|z| z.concentration && z.owner_id == actor_id)
            || self
                .conjured_terrain
                .iter()
                .any(|p| p.concentration && p.owner_id == actor_id)
    }

    /// Move `actor_id`'s stamp on the occupancy grid from wherever it is
    /// to `coord`, without touching the actor's own `location` field.
    ///
    /// Private, and deliberately so: this is the one routine in the
    /// engine that can leave the grid disagreeing with the actors, and
    /// its three callers are the three places that immediately put the
    /// two back in step — `relocate_actor`, and the two spawn paths.
    /// Anything else that wants to move a body wants `place_actor_at`
    /// or `walk_actor_to`.
    ///
    /// It also cannot be handed a mounted rider. A rider's stamp belongs
    /// to the mount it is sitting on, so moving it here would erase the
    /// *horse* from the tiles it is standing on and stamp the rider onto
    /// tiles it has no claim to. `relocate_actor` resolves the pair
    /// before it gets here, and that resolution is only sound because
    /// nothing outside this module can reach past it.
    fn set_actor_map(
        &mut self,
        actor_id: usize,
        coord: Coordinate,
    ) -> Result<(), Box<dyn Error>> {
        let Some(actor) = self.actors.get(&actor_id) else {
            return Err("Actor not found".into());
        };
        let size = actor.size();
        let coord_old = actor.location();
        self.write_footprint(None, coord_old, size);
        self.write_footprint(Some(actor_id), coord, size);
        Ok(())
    }

    /// Move an actor to `coord`, syncing both the actor map and the actor's
    /// own location field. Single source of truth for "teleport / step
    /// without OAs"; OA-honoring movement goes through `MoveActor::apply`.
    pub fn place_actor_at(
        &mut self,
        actor_id: usize,
        coord: Coordinate,
    ) -> Result<(), Box<dyn Error>> {
        self.relocate_actor(actor_id, coord, false)
    }

    /// Move an actor one tile *under its own power*, extending whatever
    /// straight run it has going.
    ///
    /// The only difference from `place_actor_at` is which side of that
    /// distinction the step falls on, and the distinction is load-bearing
    /// for exactly one rule family: 5e's charge clauses fire on "if the
    /// creature moves at least 20 feet straight toward a target", and a
    /// creature shoved twenty feet has not moved — it has been moved.
    ///
    /// Called from `MoveActor::apply`, which is the one path a creature
    /// walks. Everything else on the board — shoves, pulls, teleports,
    /// summon placement, the repositioning half of a dozen spells — goes
    /// through `place_actor_at` and ends the run.
    pub fn walk_actor_to(
        &mut self,
        actor_id: usize,
        coord: Coordinate,
    ) -> Result<(), Box<dyn Error>> {
        self.relocate_actor(actor_id, coord, true)
    }

    fn relocate_actor(
        &mut self,
        actor_id: usize,
        coord: Coordinate,
        walked: bool,
    ) -> Result<(), Box<dyn Error>> {
        // A rider who is *teleported* leaves the saddle rather than
        // taking the horse with them — RAW's Misty Step moves you, not
        // the thing you were sitting on. Resolved before the move so the
        // rider is a normal creature with a footprint by the time the
        // relocation happens, and so a dismount that can't find room
        // fails the whole teleport rather than half of it.
        if !walked && self.is_mounted(actor_id) {
            self.dismount(actor_id);
        }
        // Everything else about a mounted rider's movement is the
        // mount's: its tiles, its footprint, its stamp on the grid. The
        // rider's own `location` is mirrored onto the result below.
        let body_id = self.movement_body(actor_id);
        self.set_actor_map(body_id, coord)?;
        if let Some(a) = self.get_actor(body_id) {
            if walked {
                a.walk_to(coord);
            } else {
                a.set_location(coord);
            }
        }
        // The passenger, if there is one — reached from `body_id` rather
        // than from `actor_id` so the two directions collapse into one
        // line: relocating the rider found the mount above and finds the
        // rider back here, and relocating the mount finds its rider
        // directly.
        let passenger = self.actors.get(&body_id).and_then(|a| a.ridden_by());
        if let Some(rider_id) = passenger
            && let Some(r) = self.get_actor(rider_id)
        {
            // Mirrored with the same verb the mount used: a rider whose
            // horse is charging is charging, and one whose horse was
            // shoved has been shoved.
            if walked {
                r.walk_to(coord);
            } else {
                r.set_location(coord);
            }
        }
        // 5e's attach clause: "…and it moves with the target." Both
        // halves of a rider/mount pair are asked, because a stirge on a
        // knight rides wherever the horse goes and the knight's own
        // mirror has already run one block up.
        self.mirror_attachers_onto(body_id, coord, walked);
        if let Some(rider_id) = passenger {
            self.mirror_attachers_onto(rider_id, coord, walked);
        }
        // "If an effect moves your mount against its will while you're
        // on it…" — the involuntary half of that sentence is exactly
        // `walked == false`, which is the distinction `walk_actor_to`
        // and `place_actor_at` already draw for the charge clauses.
        if !walked && passenger.is_some() {
            self.unseat(body_id, crate::engine::mounts::UnseatCause::MountForcedMove);
        }
        Ok(())
    }

    /// Find a free anchor for a `size`-footprint creature within `radius`
    /// tiles of `caster_id`'s footprint (8-direction). Used by
    /// summoning-style spells (Animate Dead, Conjure Animals) that need
    /// to place a new actor near the caster without overlapping the
    /// caster's own tiles or any other occupied / non-floor tile. Returns
    /// the anchor (top-left of the new footprint) on success, or `None`
    /// if no slot fits.
    ///
    /// **Closest first**, which the function used not to be and its own
    /// name always claimed it was. The search was a plain `-radius..=radius`
    /// double loop returning the first hit, so it returned the anchor at
    /// the *most negative* offset — the far up-left corner of the search
    /// box. A Conjure Elemental cast with `radius: 4` put the elemental
    /// four tiles up and to the left of the wizard whenever that tile
    /// happened to be clear, which is ten feet away and behind them.
    /// Nothing failed; the summon just showed up in the wrong place, and
    /// the wider the caller's radius the wronger the place. The Tasha's
    /// summon family made that visible by shipping six spells at radius
    /// 3–4 where the lane previously had two.
    ///
    /// `ring_at` is the shared shell walk, and the reason this and
    /// `find_adjacent_teleport_anchor` can't drift apart again: the
    /// sibling had the ring-walk right all along, in its own hand-inlined
    /// copy.
    ///
    /// **Facing the fight.** Closest-ring-first still leaves a choice —
    /// eight tiles are equally adjacent — and the tie is broken toward
    /// the caster's nearest live enemy. Row-major order would otherwise
    /// resolve every tie to the *up-left* neighbour, which is a bias with
    /// real consequences for a summon that cannot walk: the Fathomless
    /// warlock's Tentacle of the Deep has speed 0 and reach 4, so the
    /// tile it lands on is the entire question of whether it ever hits
    /// anything. Facing is free for everything that moves and decisive
    /// for the things that don't.
    ///
    /// With no enemy on the board there is nothing to face, and the walk
    /// falls back to plain closest-first.
    ///
    /// **Not into the fire.** A tile under a harmful zone — the caster's
    /// own Web, Spike Growth, Cloudkill — is taken only when the whole
    /// search finds nothing else, which is why hazard outranks distance
    /// rather than tie-breaking under it. Standing a body one tile
    /// further out is close to free; standing it in a Cloudkill costs it
    /// the fight. `tile_is_hazardous` is the AI's own "is this tile worth
    /// walking through" predicate, so a summon now declines the same
    /// ground its summoner's pathfinder does.
    pub fn find_adjacent_spawn(
        &self,
        caster_id: usize,
        size: Size,
        radius: isize,
    ) -> Option<Coordinate> {
        let caster = self.actors.get(&caster_id)?;
        let loc = caster.location();
        let team = caster.team();
        // The nearest hostile footprint, measured from the caster. Read
        // once rather than per candidate tile — which enemy is nearest
        // doesn't change as the search walks outward, only how far the
        // candidate is from it.
        let threat = self
            .actors
            .values()
            .filter(|a| a.team() != team && a.is_combat_active())
            .map(|a| a.location())
            .min_by_key(|l| footprint_chebyshev(loc, get_tiles_from_size(caster.size()), *l, 1));
        self.closest_spawn_facing(loc, size, radius, threat, false)
            .or_else(|| self.closest_spawn_facing(loc, size, radius, threat, true))
    }

    /// The half of `find_adjacent_spawn` that actually walks: the
    /// closest legal anchor for a `size` footprint within `radius` of
    /// `origin`, ties broken toward `threat`.
    ///
    /// Split out so the hazard rule can be expressed as running the walk
    /// twice — once refusing harmful ground, once accepting it — rather
    /// than as a sort key, because hazard has to outrank distance and a
    /// single ring-by-ring walk cannot express that.
    fn closest_spawn_facing(
        &self,
        origin: Coordinate,
        size: Size,
        radius: isize,
        threat: Option<Coordinate>,
        accept_hazard: bool,
    ) -> Option<Coordinate> {
        let w = get_tiles_from_size(size) as isize;
        let tiles = move |anchor: Coordinate| {
            (0..w).flat_map(move |ox| {
                (0..w).map(move |oy| Coordinate::new(anchor.x + ox, anchor.y + oy))
            })
        };
        let usable = |anchor: &Coordinate| {
            self.footprint_is_clear(*anchor, size)
                && (accept_hazard || !tiles(*anchor).any(|t| self.tile_is_hazardous(t)))
        };
        for ring in 1..=radius {
            let found = ring_at(origin, ring).filter(usable).min_by_key(|c| {
                // No enemy on the board means nothing to face, and every
                // candidate scores 0 — leaving the ring's own row-major
                // order to decide, deterministically.
                threat.map_or(0, |t| footprint_chebyshev(*c, w as usize, t, 1))
            });
            if found.is_some() {
                return found;
            }
        }
        None
    }

    /// Sibling of `find_adjacent_spawn` for *teleporting an existing
    /// actor* near `anchor_id`. Walks rings of distance from
    /// `anchor_id`'s footprint outward and returns the closest legal
    /// anchor that `mover_id`'s footprint can occupy via the standard
    /// `can_move_to` check (which honors `mover_id`'s own current tiles
    /// as "free" — the mover is leaving those tiles to land here).
    ///
    /// Used by Vortex Warp's "yank target next to caster" landing-tile
    /// pick. Distinct from `find_adjacent_spawn` (which uses
    /// `is_spawnable`) because the mover hasn't been removed from the
    /// map yet — its current tiles aren't "spawnable" but ARE legal
    /// landing tiles for itself.
    ///
    /// Returns `None` only when no legal anchor exists inside the
    /// expanded search ring (both creatures' footprints + slack), in
    /// which case the spell's apply path silently no-ops.
    pub fn find_adjacent_teleport_anchor(
        &self,
        anchor_id: usize,
        mover_id: usize,
    ) -> Option<Coordinate> {
        let anchor = self.actors.get(&anchor_id)?;
        let mover = self.actors.get(&mover_id)?;
        let anchor_loc = anchor.location();
        let anchor_size = get_tiles_from_size(anchor.size()) as isize;
        let mover_size = get_tiles_from_size(mover.size()) as isize;
        // Search radius spans far enough to clear both footprints.
        // For two Medium (2-tile) creatures, an anchor at gap ±3 from
        // the host's origin tile is the closest spot whose footprint
        // won't overlap. Add 1 for slack so a Large host + Medium
        // mover doesn't fall off the end of the search.
        let search_radius = (anchor_size + mover_size).max(2);
        rings_outward(anchor_loc, search_radius)
            .find(|candidate| self.can_move_to(mover_id, *candidate))
    }

    pub fn instantiate_creature(
        &mut self,
        creature_template: &'static CreatureTemplate,
        location: Coordinate,
        team_id: usize,
        instance_n: usize,
    ) -> Result<usize, Box<dyn Error>> {
        let actor_id = self.next_actor_id();

        let mut actor = ActorInstance::from_creature_template(
            creature_template,
            location,
            team_id,
            &mut self.roller,
            instance_n,
        )?;
        actor.reset_for_new_round();
        if creature_template.has_displacement {
            actor.add_condition(
                Condition::Displaced,
                crate::conditions::ConditionTimer::Permanent,
            );
        }
        // 5e "creature is born already X" lane — Invisible Stalker's
        // permanent invisibility, future always-on body buffs. Applied
        // once at instantiation; not auto-restored if dispelled later
        // (the displacement-restore lane above handles that case for
        // its own mechanic).
        for &(c, timer) in &creature_template.innate_conditions {
            actor.add_condition(c, timer);
        }
        // …and the same lane on the map rather than on the body: 5e's
        // **Illumination**, the trait a creature made of fire carries
        // instead of a torch. Anchored `Carried` so it walks with the
        // azer, and flagged `innate` so it goes out with it — see
        // `LightSource::innate` for why a glow and a torch part company
        // at exactly that moment.
        if let Some((bright, dim)) = creature_template.innate_light {
            self.add_light_source(crate::engine::lighting::LightSource {
                id: 0,
                name: "illumination",
                anchor: crate::engine::lighting::LightAnchor::Carried(actor_id),
                bright_tiles: bright,
                dim_tiles: dim,
                rounds_remaining: None,
                spell_level: 0,
                innate: true,
            });
        }

        if self.initialized {
            actor.roll_initiative(&mut self.roller);
            self.initiative_tracker.add_actor(
                actor_id,
                actor.initiative().unwrap(),
                actor.initiative_mod(),
            );
            // A Thief who joins the fight while round 1 is still running
            // gets their extra slot on the same terms as one who was
            // there at the bell — RAW scopes Thief's Reflexes to "the
            // first round of any combat," not to being present for the
            // initiative roll. `grant_extra_turn_slot` is what enforces
            // the round gate, so a round-3 summon quietly gets nothing.
            if actor.has_passive_feature(crate::actions::class_features::THIEFS_REFLEXES_TAG) {
                let (init, dex) = (actor.initiative().unwrap(), actor.initiative_mod());
                self.grant_extra_turn_slot(actor_id, actor.name().to_string(), init, dex);
            }
        }

        self.actors.insert(actor_id, actor);

        self.set_actor_map(actor_id, location)?;

        Ok(actor_id)
    }

    /// The initiative queue in turn order, starting at the active slot.
    /// Empty when nobody is queued. Drives the UI's initiative panel.
    ///
    /// Slots rather than bare ids because an actor can hold more than one
    /// (the Thief Rogue's Thief's Reflexes gives them two in round 1),
    /// and a panel that listed the same name twice with nothing to
    /// distinguish the rows would read as a rendering bug rather than as
    /// the feature it is.
    pub fn initiative_slots(&self) -> Vec<InitiativeSlot> {
        let len = self.initiative_tracker.initiatives.len();
        if len == 0 {
            return Vec::new();
        }
        let curr = self.initiative_tracker.curr_index;
        (0..len)
            .map(|i| {
                let elem = &self.initiative_tracker.initiatives[(curr + i) % len];
                InitiativeSlot {
                    actor_id: elem.actor_id,
                    is_extra: elem.is_extra,
                }
            })
            .collect()
    }

    /// Top-of-stack snapshot for the UI: the actor whose prompt is open
    /// (if any), and whether the engine is mid-processing or idle.
    pub fn stack_state(&self) -> StackState {
        match self.encounter_stack.last() {
            None => StackState::Idle,
            Some(se) => match &se.entry {
                StackElementEntry::Prompt(p) => StackState::AwaitingPrompt(p.actor_id()),
                _ => StackState::Processing,
            },
        }
    }

    /// Distinct team ids with at least one combat-active actor (excludes
    /// dying / stable / dead). Drives end-of-combat detection.
    /// Every team with something left to fight for.
    ///
    /// The one place in the engine that deliberately counts a creature
    /// `is_combat_active` says is out of the fight. A banished creature
    /// is not on the board and every targeting, AoE and AI question
    /// should treat it as absent — but it is alive, unharmed, and
    /// coming back on a timer, so its team has emphatically not lost.
    /// Without this clause a party could win an encounter by banishing
    /// the last enemy, and the enemy would reappear in an arena the app
    /// had already declared cleared.
    pub fn living_teams(&self) -> std::collections::HashSet<usize> {
        self.actors
            .values()
            .filter(|a| a.is_combat_active() || a.is_off_board())
            .map(|a| a.team())
            .collect()
    }

    /// True once at most one team has living actors. Encounters with zero
    /// living actors also count as complete (mutual destruction). Also
    /// fires on stalemate — no living actor can engage any enemy via
    /// melee path or ranged LOS, so the fight has nowhere to go.
    pub fn is_complete(&self) -> bool {
        self.living_teams().len() <= 1 || self.is_stalemate()
    }

    /// Rounds of zero attrition progress after which the fight is
    /// called a draw. See `is_stalemate`'s attrition half.
    ///
    /// **Measured, not guessed.** Six hundred generated encounters and
    /// every PC template's duel against an ogre were run to completion
    /// with this check disabled, recording how long each fight went
    /// without setting a new low-water mark for total hit points. For
    /// fights that resolved on their own: 95% never went 8 rounds
    /// without progress, 99% never went 37, and the two worst — a
    /// charm-lock where neither side may legally attack the other until
    /// the condition lapses — reached 128 and 129. The fights that
    /// never resolved ran to round 3274, 5584, 9895, 9975 and 14926.
    ///
    /// Those are two populations with a gap between them wide enough to
    /// drive the threshold through the middle of, and 400 is a little
    /// over three times the worst honest fight observed. The asymmetry
    /// is deliberate: setting it too high costs a few hundred cheap
    /// rounds of nothing happening before the draw is called, and
    /// setting it too low ends a fight somebody was still winning.
    pub const NO_PROGRESS_ROUNDS: u32 = 400;

    /// Total hit points across every combat-active actor.
    ///
    /// Deliberately *combat-active* rather than every actor on the
    /// board: a creature that drops to 0 leaves the sum entirely, so
    /// felling something registers as the large step forward it is
    /// rather than as the last few points of damage that did it.
    pub fn total_active_hitpoints(&self) -> u32 {
        self.actors
            .values()
            .filter(|a| a.is_combat_active())
            .map(|a| a.hitpoints())
            .sum()
    }

    /// Record whether the round that just ended got the fight anywhere.
    ///
    /// Called once per round from the initiative wrap, after
    /// `round_end` has paid out the round's timers and swept its dead
    /// — so regeneration, ongoing burning, lapsing conditions and
    /// corpses are all reflected in the number this reads.
    fn note_attrition_progress(&mut self) {
        let total = self.total_active_hitpoints();
        if self.lowest_active_hitpoints.is_none_or(|low| total < low) {
            self.lowest_active_hitpoints = Some(total);
            self.last_attrition_progress_round = self.round;
        }
    }

    /// Rounds since the fight last got measurably closer to being over.
    ///
    /// Surfaced rather than kept private because it is the number a
    /// draw is called on, and a UI that wanted to warn "this fight is
    /// going nowhere" before the engine calls it would read exactly
    /// this.
    pub fn rounds_without_attrition_progress(&self) -> u32 {
        self.round.saturating_sub(self.last_attrition_progress_round)
    }

    /// True if the fight cannot progress — either because nobody can
    /// reach anybody, or because everybody can and it is not helping.
    ///
    /// **The positional half** is the original: no combat-active actor
    /// on any team can reach (via BFS) or shoot (via line-of-sight plus
    /// a ranged attack) any enemy. Terrain has split the parties into
    /// permanently disconnected pockets, and without this the AI loops
    /// skipping forever.
    ///
    /// **The attrition half** is the same failure one step further in.
    /// Being able to attack is not the same as being able to win, and a
    /// fight where every blow lands and none of them accumulate runs
    /// exactly as long as somebody is willing to watch it. The case
    /// that found this ran to round 4261: a Yeti with a 15-point
    /// chilling gaze against a Shield Guardian regenerating 10 hit
    /// points a round, with three other factions dashing back and forth
    /// out of reach of everyone. Every actor had something to do every
    /// round. Nobody was ever going to win.
    ///
    /// Both halves answer the same question — "is any future round
    /// different from this one?" — and the positional one is simply the
    /// case where the answer is knowable from the board alone. The
    /// attrition one has to be observed, which is why it costs the two
    /// fields it costs and why it takes [`Self::NO_PROGRESS_ROUNDS`] to
    /// be sure.
    ///
    /// Rare in a two-team fight — seven hundred generated two-team
    /// encounters settled without it — and common enough with more
    /// factions to be worth having: five in three hundred. Most of
    /// those ended with two teams left standing, which is the detail
    /// that makes this worth fixing rather than documenting: the extra
    /// factions were how the deadlock got *set up*, not what it was
    /// made of, so a two-team fight is not immune, only luckier.
    ///
    /// A fight called this way has no winner. `winning_team` already
    /// answers `None` whenever more than one team is standing, so a
    /// draw needs no special case downstream — it is the same "nobody
    /// won" the positional half has always produced.
    pub fn is_stalemate(&self) -> bool {
        // A board with somebody still held off it is a board that is
        // about to change, and the whole premise of this check is that
        // the arrangement is permanent. The party that has just banished
        // the last enemy can reach nothing, which is a stalemate by
        // every test below and by none of the ones that matter — the
        // enemy returns to the space it left in a handful of rounds.
        // Bailing early rather than counting banished creatures as
        // combatants, because `can_engage` would then be asked to path
        // to a body that owns no tiles.
        //
        // `belongs_off_board`, deliberately, and not `is_off_board`.
        // The two disagree in exactly one state: a creature whose
        // banishment has lapsed but which has found no legal tile to
        // return to (see `find_return_anchor`). Nothing is holding that
        // one away any more, so nothing is coming — and a board too
        // full to put one body back down is precisely the deadlock this
        // check exists to break rather than a reason to keep waiting.
        if self.actors.values().any(|a| a.belongs_off_board()) {
            return false;
        }
        let combatants: Vec<(usize, usize)> = self
            .actors
            .iter()
            .filter(|(_, a)| a.is_combat_active())
            .map(|(id, a)| (*id, a.team()))
            .collect();
        if combatants.len() <= 1 {
            return false;
        }
        // The attrition half. Checked after the combatant count so a
        // board with one creature left on it is never called a draw —
        // that is either a win or a fight that has not started, and
        // both are somebody else's answer.
        //
        // Ahead of the reachability walk below rather than after it
        // because it is two integer reads against a nested loop over
        // every pair of combatants, and because a fight that has been
        // going nowhere for fifty rounds is over whether or not the
        // pathfinder agrees.
        if self.rounds_without_attrition_progress() >= Self::NO_PROGRESS_ROUNDS {
            return true;
        }
        for (id, team) in &combatants {
            for (other_id, other_team) in &combatants {
                if team == other_team || id == other_id {
                    continue;
                }
                if self.can_engage(*id, *other_id) {
                    return false;
                }
            }
        }
        true
    }

    /// True if `attacker` has *some* tactical option against `target` —
    /// either there's a BFS path between their footprints (melee can
    /// eventually close in) or `attacker` has a ranged attack with LOS
    /// to `target`. Stalemate detection short-circuits as soon as one
    /// such option exists.
    fn can_engage(&self, attacker_id: usize, target_id: usize) -> bool {
        use crate::actions::action_template::{MELEE_REACH, TargetingSchema};
        // BFS step is movement-budget-independent; if it returns Some,
        // there's a path eventually (over multiple turns if needed).
        if self.step_toward_actor(attacker_id, target_id).is_some() {
            return true;
        }
        // Already in melee → step_toward returns None but engagement is
        // possible (we just stand and swing).
        if let Some(dist) = self.footprint_distance(attacker_id, target_id)
            && dist <= MELEE_REACH
        {
            return true;
        }
        // Ranged: any single-actor attack with reach > MELEE_REACH that
        // covers the current distance and has LOS counts.
        let Some(attacker) = self.actors.get(&attacker_id) else {
            return false;
        };
        let Some(dist) = self.footprint_distance(attacker_id, target_id) else {
            return false;
        };
        if !self.actor_has_line_of_sight(attacker_id, target_id) {
            return false;
        }
        // Either a SingleActor ranged attack OR a Burst-schema attack
        // (Fireball, Cone of Cold, Erupting Earth, dragon breath, etc.)
        // whose reach covers the target — both count as a viable
        // engagement option. Without the Burst clause, AoE-only
        // attackers behind a path-blocked wall were falsely marked as
        // stalemate-locked even when they could lob a Fireball at the
        // unreachable enemy.
        attacker.actions.iter().any(|a| {
            let in_range = a.reach_tiles().is_some_and(|r| r > MELEE_REACH && dist <= r);
            if !in_range {
                return false;
            }
            matches!(
                a.targeting_schema(),
                TargetingSchema::SingleActor | TargetingSchema::Burst { .. }
            )
        })
    }

    /// `Some(team_id)` if exactly one team is left standing; `None` if the
    /// fight is still on or everyone is dead.
    pub fn winning_team(&self) -> Option<usize> {
        let teams = self.living_teams();
        if teams.len() == 1 {
            teams.into_iter().next()
        } else {
            None
        }
    }

    /// Idempotent post-effect cleanup pass: remove any actor whose
    /// death-save record has hit 3 failures. The "falls unconscious" log
    /// is emitted by `DealDamage::apply` directly so the message tracks the
    /// actual transition (active → dying), not a fragile derived check on
    /// `(successes, failures) == (0, 0)`.
    ///
    /// Stable actors stay on the map at 0 HP — they're out of the fight but
    /// not removed (room for healing later).
    ///
    /// Any actor whose template carries a `DeathBurst` static (mephits,
    /// magmins, future ash-zombie variants) detonates inside `remove_actor`
    /// itself — the trigger lives at the removal chokepoint so the PC
    /// death-save path (`resolve_death_save`) gets the same fire-on-death
    /// behavior without a duplicate hook here. Any chain-killed bystanders
    /// the burst takes out land in the next call to `cleanup_dead_actors`
    /// rather than being recursively swept here — keeps the loop a single,
    /// predictable pass over the original dead list.
    pub fn cleanup_dead_actors(&mut self) {
        use crate::actors::actor_template::HpState;
        let dead: Vec<usize> = self
            .actors
            .iter()
            .filter_map(|(id, a)| match a.hp_state() {
                HpState::Dead => Some(*id),
                HpState::Dying { failures, .. } if failures >= 3 => Some(*id),
                _ => None,
            })
            .collect();
        for id in dead {
            self.remove_actor(id);
        }
    }

    /// Fire `id`'s death burst (if any). No-op for actors without a
    /// `DeathBurst` template entry. Mirrors `BreathWeapon::side_effects`'s
    /// shape — roll damage once, log the breakdown, route through
    /// `resolve_burst_save_damage` for the per-target save + half-on-pass
    /// resolution — but skips the recharge / action-economy wiring since
    /// the burst is an on-death trigger, not a turn-spent ability. Damage
    /// applies immediately (effects are flushed before returning) so the
    /// caller can safely remove the corpse afterward without holding
    /// onto stale `DealDamage` entries pointed at a removed actor.
    fn trigger_death_burst(&mut self, id: usize) {
        let Some(actor) = self.actors.get(&id) else {
            return;
        };
        let Some(burst) = actor.death_burst() else {
            return;
        };
        let center = actor.location();
        let name = actor.name().to_string();
        let dice = burst.damage_dice;
        let damage_type = burst.damage_type;
        let save_ability = burst.save_ability;
        let dc = burst.dc;
        let radius = burst.radius;
        let label = burst.display_name;
        let raw = self.roll(&dice);
        self.log(format!(
            "{} {}: {}({}) = {} {} (DC {} {}, half on save)",
            name, label, dice, raw, raw, damage_type, dc, save_ability,
        ));
        let effects = crate::actions::action_template::resolve_burst_save_damage(
            self,
            id,
            center,
            radius,
            save_ability,
            dc,
            raw,
            damage_type,
        );
        for ef in effects {
            ef.apply(self);
        }
    }

    /// Remove an actor from the world *without* awarding XP or rolling
    /// loot, then write a custom log line ("vanishes", "is dispelled",
    /// etc.). Used by summon-cleanup paths (Conjure Animals / Elemental
    /// concentration drop) where the minion isn't really dying — it's
    /// being unsummoned, so the kill rewards lane shouldn't fire.
    ///
    /// Carried items are intentionally dropped on the despawn tile so a
    /// minion that picked something up mid-fight doesn't void the loot
    /// silently — matches the `remove_actor` policy for the same reason.
    /// Cut `id`'s remaining claims on the grid ahead of a removal, and
    /// report whether its tiles are already somebody else's problem.
    ///
    /// Two creatures leave the board without tiles of their own to
    /// clear, and a removal path that blindly stamps `None` over their
    /// remembered footprint erases whatever is standing there now:
    ///
    ///   - a **rider**, whose tiles belong to the mount it is sitting
    ///     on — `sever_ride_links` owns that repair and says so by
    ///     returning true;
    ///   - a **latched creature**, whose tiles belong to the host it
    ///     bit — `sever_attachments` owns that one, and also stands any
    ///     passengers of a departing *host* back up on the box it is
    ///     about to vacate;
    ///   - a creature that is **off the board**, which gave its
    ///     footprint up when it was banished and may well have had it
    ///     walked into since.
    ///
    /// Shared by both removal paths — `despawn_actor` and
    /// `remove_actor` — which used to spell the first case out
    /// identically and would each have had to learn the others.
    fn release_grid_claim(&mut self, id: usize) -> bool {
        // Ordered so both severs always run: they are repairs, not
        // queries, and `||` would skip them for an off-board rider.
        // Ride first, so a rider whose tiles were never theirs has
        // already said so before the attach sever asks the grid who
        // owns them.
        let unseated = self.sever_ride_links(id);
        let unlatched = self.sever_attachments(id);
        unseated
            || unlatched
            || self.actors.get(&id).is_some_and(|a| a.is_off_board())
    }

    pub fn despawn_actor(&mut self, id: usize, log_verb: &str) {
        // Same reason as `remove_actor`: an actor leaving the board
        // takes their concentration — and so the areas it was holding
        // up — with them.
        self.release_map_layers_of(id);
        // …but not their torch, which stays lit where they stood. Same
        // policy as the carried items dropped at the bottom of this
        // function.
        self.drop_light_sources_carried_by(id);
        let footprint_handled = self.release_grid_claim(id);
        let Some(actor) = self.actors.remove(&id) else {
            return;
        };
        self.log(format!("{} {}.", actor.name(), log_verb));
        let size = actor.size();
        let loc = actor.location();
        let carried: Vec<&'static crate::items::item_template::Item> =
            actor.items().to_vec();
        drop(actor);
        if !footprint_handled {
            self.write_footprint(None, loc, size);
        }
        self.initiative_tracker.remove_actor(id);
        for item in carried {
            self.drop_item(loc, item);
            self.log(format!("  drops {}.", item.name));
        }
    }

    /// Remove an actor from the world: actor map, initiative queue, and
    /// actor table. Logs the death and, for non-player-team actors, rolls
    /// a chance to drop a random item from `LOOT_POOL` on their tile and
    /// awards XP (split across surviving team-0 PCs) for the kill.
    ///
    /// Death-burst trigger fires here (before the actor is removed) so
    /// every "real death" code path — monster HP→0 (`cleanup_dead_actors`)
    /// and PC's third-failed-death-save (`resolve_death_save`) — gets the
    /// burst uniformly. Distinct from `despawn_actor` (summon unbind), which
    /// intentionally skips the burst since the actor isn't truly dying.
    fn remove_actor(&mut self, id: usize) {
        self.trigger_death_burst(id);
        // Cut before the corpse is lifted out of the table: a rider and
        // a mount are the only two actors in the engine that hold ids
        // pointing at each other, and either half surviving the other
        // would leave a link nothing can resolve. The rider's DC 10 save
        // has already happened at 0 HP (`trigger_creature_dropped`); this
        // is what stands up a rider who had nowhere to fall then.
        let footprint_handled = self.release_grid_claim(id);
        // A concentration-held area outlives nothing. Swept here rather
        // than in `drop_concentration`, because death does not route
        // through it — the actor is lifted straight out of the table —
        // and a dead wizard's web holding a doorway for the rest of the
        // fight is the kind of leak the map layer makes very visible.
        self.release_map_layers_of(id);
        // The torch the corpse was holding keeps burning on the tile it
        // fell on — see `drop_light_sources_carried_by`, and the
        // carried-item drop further down that it mirrors.
        self.drop_light_sources_carried_by(id);
        let Some(actor) = self.actors.remove(&id) else {
            return;
        };
        self.log(format!("{} dies.", actor.name()));
        let size = actor.size();
        let loc = actor.location();
        let team = actor.team();
        let xp_award = actor.xp_value();
        // Carried items always drop where the actor fell so the player
        // can recover gear. Generic loot rolls a chance on top of that
        // for non-player teams.
        let carried: Vec<&'static crate::items::item_template::Item> =
            actor.items().to_vec();
        drop(actor);
        if !footprint_handled {
            self.write_footprint(None, loc, size);
        }
        self.initiative_tracker.remove_actor(id);
        for item in carried {
            self.drop_item(loc, item);
            self.log(format!("  drops {}.", item.name));
        }
        if team != 0 {
            use crate::items::item_template::LOOT_POOL;
            // 33% drop rate keeps loot meaningful per kill without
            // flooding the floor in long fights.
            if !LOOT_POOL.is_empty() && self.rng.f32() < 0.33 {
                let idx = self.rng.usize(0..LOOT_POOL.len());
                let item = LOOT_POOL[idx];
                self.drop_item(loc, item);
                self.log(format!("  drops {}.", item.name));
            }
            // XP award: split the kill across every team-0 PC still
            // combat-active. Splitting keeps the curve tame as party
            // size grows; leveling happens on long rest so we don't
            // need to throttle awards in-fight.
            let recipients: Vec<usize> = self
                .actors
                .iter()
                .filter(|(_, a)| a.team() == 0 && a.is_combat_active())
                .map(|(id, _)| *id)
                .collect();
            if !recipients.is_empty() && xp_award > 0 {
                let per = xp_award / recipients.len() as u32;
                for rid in &recipients {
                    if let Some(a) = self.actors.get_mut(rid) {
                        a.award_xp(per);
                    }
                }
                self.log(format!(
                    "  ({} XP awarded to {} PC{})",
                    per,
                    recipients.len(),
                    if recipients.len() == 1 { "" } else { "s" }
                ));
            }
        }
    }

    /// Roll a single death save for the given actor and mutate them. Logs
    /// the d20 result and outcome. Returns true if the actor is gone after
    /// this save (dead and removed).
    fn resolve_death_save(&mut self, id: usize) -> bool {
        // 5e Lucky: a Halfling / Lucky-feat character at 0 HP can re-roll
        // a nat-1 on a death save. RAW explicitly lists death saves as a
        // saving throw — they get the same reroll lane as a normal save.
        // Death saves don't have advantage/disadvantage in RAW so we pass
        // `RollMode::Normal` directly.
        let raw = self.roll_d20_lucky(id, RollMode::Normal);
        let Some(actor) = self.actors.get_mut(&id) else {
            return false;
        };
        let name = actor.name().to_string();
        let outcome = actor.apply_death_save(raw);
        let (succ, fail) = actor.death_save_record();
        match outcome {
            DeathSaveOutcome::Continuing => {
                let label = if raw == 1 {
                    "critical failure (2 fails)"
                } else if raw >= 10 {
                    "success"
                } else {
                    "failure"
                };
                self.log(format!(
                    "  {} death save: 1d20({}) — {} ({}/{} S/F)",
                    name, raw, label, succ, fail
                ));
                false
            }
            DeathSaveOutcome::Stabilized => {
                self.log(format!(
                    "  {} death save: 1d20({}) — stabilized!",
                    name, raw
                ));
                false
            }
            DeathSaveOutcome::Dead => {
                self.log(format!("  {} death save: 1d20({}) — dies!", name, raw));
                self.remove_actor(id);
                true
            }
            DeathSaveOutcome::Revived => {
                self.log(format!(
                    "  {} death save: 1d20({}) — natural 20! Conscious at 1 HP.",
                    name, raw
                ));
                false
            }
            DeathSaveOutcome::NotDying => false,
        }
    }

    pub fn initialize(&mut self) -> Result<(), &'static str> {
        if self.initialized {
            return Err("attempted to initialize already initialized encounter");
        }
        // Roll initiative in actor-id order for seed reproducibility —
        // HashMap iteration order is per-process random and would otherwise
        // assign different d20 rolls to the same actor across runs.
        for id in self.sorted_actor_ids() {
            if let Some(actor) = self.actors.get_mut(&id) {
                actor.roll_initiative(&mut self.roller);
            }
        }
        self.initiative_tracker.initialize_actors(&self.actors);
        // Features whose RAW trigger is the words "when you roll
        // initiative". Walked in actor-id order, after the base queue is
        // sorted, so a table with two Thieves in it lays out the same
        // way on every run of the same seed.
        for id in self.sorted_actor_ids() {
            let Some(actor) = self.actors.get(&id) else {
                continue;
            };
            if actor.has_passive_feature(crate::actions::class_features::THIEFS_REFLEXES_TAG) {
                let (name, init, dex) = (
                    actor.name().to_string(),
                    actor.initiative().expect("Expected initiative"),
                    actor.initiative_mod(),
                );
                self.grant_extra_turn_slot(id, name, init, dex);
            }
            self.refill_ever_ready_shot(id);
        }
        self.initialized = true;
        Ok(())
    }

    /// 5e **surprise**, decided once as the encounter opens.
    ///
    /// RAW: *"any character or monster that doesn't notice a threat is
    /// surprised at the start of the encounter."* The engine can answer
    /// "doesn't notice" exactly, because it already answers "can see" —
    /// `viewer_can_see` folds in blindness, invisibility, the Hidden
    /// condition, heavy obscurement and the lighting layer's darkness
    /// with each creature's own darkvision. A creature that can see
    /// none of the enemies on the board has noticed no threat.
    ///
    /// Two guards, and both are load-bearing:
    ///
    ///   - **Somebody has to have started it.** A creature is surprised
    ///     only if at least one enemy can see *them*. Without this
    ///     clause a party and a monster who are mutually blind — two
    ///     groups in the dark, neither aware of the other — would both
    ///     be surprised and both lose a round to a fight neither of
    ///     them started.
    ///   - **There has to be an enemy.** A lone creature on an empty
    ///     board notices nothing because there is nothing to notice.
    ///
    /// On the lit board the game is played on by default this resolves
    /// to nobody, every time, which is correct and is also why it is
    /// safe to run unconditionally: everyone standing in a bright room
    /// can see everyone else. It fires in the dark — a torchless party
    /// walking into a room of darkvision — which is precisely the
    /// fiction RAW's surprise rules are written about.
    ///
    /// Deliberately *not* a Stealth-versus-passive-Perception contest,
    /// which is the other half of RAW's sentence. That contest is about
    /// creatures who were hiding before the encounter began, and this
    /// engine has no before: actors are placed and initiative is
    /// rolled. Rolling one anyway would hand out a lost round on a die
    /// nobody could see coming, in every fight, on a board where
    /// everyone is standing in the open looking at each other.
    /// Test-only door onto `resolve_opening_surprise`. The real trigger
    /// is inside `initialize`, which a fixture-built encounter has
    /// already run — and run on a board with no actors on it yet, which
    /// is the same reason the door exists rather than a second
    /// `initialize`.
    #[cfg(test)]
    pub fn resolve_opening_surprise_for_test(&mut self) {
        self.surprise_resolved = true;
        self.resolve_opening_surprise();
    }

    /// What `actor_id` notices at the top of its turn without going
    /// looking for it: any hidden enemy standing where it would plainly
    /// see them, whose Stealth total its **passive Perception** meets.
    ///
    /// SRD 5.2's Hide ends *"immediately after […] an enemy finds
    /// you"*, and the book gives two ways to be found. The Search
    /// action is the deliberate one — spend your Action, roll Wisdom
    /// (Perception) against the hider's total — and passive Perception
    /// is the other: *"a Wisdom (Perception) check made without
    /// rolling"*, used to decide whether a creature notices something
    /// it was not consciously watching for.
    ///
    /// Without it, hiding was permanent in the only sense that matters.
    /// A rogue could duck behind a wall, pass the check, and then spend
    /// the rest of the fight walking around an open, brightly lit room
    /// with every attack against it at disadvantage, because nothing
    /// re-asked the question and nobody wanted to spend a whole Action
    /// on Search. The tighter entry condition SRD 5.2 puts on *getting*
    /// hidden made that sharper rather than better: hiding is harder to
    /// start and was still impossible to end.
    ///
    /// The gate is the mirror of `can_attempt_hide`'s, asked from the
    /// other side. A watcher notices a hider only when its own hiding
    /// is the one thing in the way — the watcher can otherwise see the
    /// tile (`viewer_can_see` folds blindness, darkness the watcher has
    /// no darkvision for, fog and walls) and the hider is not behind
    /// three-quarters cover. So a rogue who stays behind the wall stays
    /// hidden however sharp the eyes looking for it, which is right:
    /// there is nothing to notice.
    ///
    /// Runs for the creature whose turn is opening rather than for the
    /// whole board, because that is what "at the top of your turn you
    /// take stock" means, and because a board-wide sweep on every turn
    /// would find the same hider once per creature and log it four
    /// times.
    fn notice_hidden_enemies(&mut self, watcher_id: usize) {
        let Some(watcher) = self.actors.get(&watcher_id) else {
            return;
        };
        if !watcher.is_combat_active() {
            return;
        }
        let (team, perception) = (watcher.team(), watcher.passive_perception());
        let found: Vec<usize> = self
            .sorted_actor_ids()
            .into_iter()
            .filter(|&id| {
                let Some(a) = self.actors.get(&id) else {
                    return false;
                };
                id != watcher_id
                    && a.team() != team
                    && a.is_combat_active()
                    && a.has_condition(Condition::Hidden)
                    && perception >= a.hidden_find_dc()
                    && self.viewer_can_see(watcher_id, id)
                    && self.cover_ac_bonus(watcher_id, id) < Self::THREE_QUARTERS_COVER_AC
            })
            .collect();
        let watcher_name = self.actor_name(watcher_id);
        for id in found {
            let name = self.actor_name(id);
            if self
                .actors
                .get_mut(&id)
                .is_some_and(|a| a.remove_condition(Condition::Hidden))
            {
                self.log(format!("{} spots {}.", watcher_name, name));
            }
        }
    }

    fn resolve_opening_surprise(&mut self) {
        let ids = self.sorted_actor_ids();
        let mut surprised: Vec<usize> = Vec::new();
        for &id in &ids {
            let Some(actor) = self.actors.get(&id) else {
                continue;
            };
            if !actor.is_combat_active() {
                continue;
            }
            let team = actor.team();
            let enemies: Vec<usize> = ids
                .iter()
                .copied()
                .filter(|other| {
                    self.actors
                        .get(other)
                        .is_some_and(|a| a.team() != team && a.is_combat_active())
                })
                .collect();
            if enemies.is_empty() {
                continue;
            }
            let notices_something = enemies.iter().any(|&e| self.viewer_can_see(id, e));
            let is_noticed = enemies.iter().any(|&e| self.viewer_can_see(e, id));
            if !notices_something && is_noticed {
                surprised.push(id);
            }
        }
        for id in surprised {
            let Some(actor) = self.actors.get_mut(&id) else {
                continue;
            };
            let name = actor.name().to_string();
            actor.add_condition(
                Condition::Surprised,
                crate::conditions::ConditionTimer::Rounds(1),
            );
            self.log(format!("{} is caught unawares.", name));
        }
    }

    /// 5e Arcane Archer Fighter **Ever-Ready Shot** (subclass level 15):
    /// "when you roll initiative and have no uses of Arcane Shot
    /// remaining, you regain one use of it."
    ///
    /// The one feature in the engine that could not have existed before
    /// the charge lane learned to count, because "no uses remaining" and
    /// "does not have the feature" were the same state in a set: a
    /// spent tag was simply gone, and there was nothing to tell the
    /// difference between an archer who had emptied the pool and one who
    /// never had it. `features_max` keeps the shape of the pool and
    /// `features_remaining` keeps what is in it, so the question RAW
    /// asks is now answerable.
    ///
    /// Fires only on the empty pool, per RAW — an archer walking into a
    /// fight with one of two charges left does not get topped up.
    /// Test-only door onto `refill_ever_ready_shot`. The real trigger
    /// is inside `initialize`, which a fixture-built encounter has
    /// already run and will refuse to run twice.
    #[cfg(test)]
    pub fn refill_ever_ready_shot_for_test(&mut self, actor_id: usize) {
        self.refill_ever_ready_shot(actor_id);
    }

    fn refill_ever_ready_shot(&mut self, actor_id: usize) {
        use crate::actions::class_features::{ARCANE_SHOT_TAG, EVER_READY_SHOT_TAG};
        let refilled = self.actors.get_mut(&actor_id).is_some_and(|actor| {
            actor.has_passive_feature(EVER_READY_SHOT_TAG)
                && actor.feature_charges_remaining(ARCANE_SHOT_TAG) == 0
                && actor.restore_feature_charge(ARCANE_SHOT_TAG)
        });
        if refilled {
            let name = self.actor_name(actor_id);
            self.log(format!("{} always has one arrow left.", name));
        }
    }

    /// 5e Thief Rogue **Thief's Reflexes** (subclass level 17): "you can
    /// take two turns during the first round of any combat. You take your
    /// first turn at your normal initiative and your second turn at your
    /// initiative minus 10."
    ///
    /// Both halves of that sentence are load-bearing and the engine keeps
    /// them literally. The extra turn is a genuine second slot in the
    /// queue, not a bolt-on grant of a spare Action: it opens through
    /// `start_turn_for` like any other turn, so the Thief gets a fresh
    /// Action, bonus action, reaction, movement budget and — the part
    /// that actually decides fights — a fresh once-per-turn Sneak Attack.
    /// And it sits ten points *down* the order rather than immediately
    /// after the first, so the enemies in between act in the gap. A Thief
    /// who opens on a caster and wants to finish the job has to survive
    /// that caster's turn to do it.
    ///
    /// Gated on round 1 here rather than at the call sites so the "first
    /// round of any combat" clause has exactly one home. A Thief summoned
    /// into round 3 asks for a slot and quietly gets none.
    fn grant_extra_turn_slot(&mut self, actor_id: usize, name: String, initiative: i32, dex: i32) {
        if self.round != 1 {
            return;
        }
        let extra = initiative - THIEFS_REFLEXES_INITIATIVE_PENALTY;
        self.initiative_tracker.add_extra_turn(actor_id, extra, dex);
        self.log(format!(
            "{} moves twice this round — a second turn waits at initiative {}.",
            name, extra
        ));
    }

    pub fn enqueue_event(&mut self, se: StackElementEntry) {
        self.encounter_stack.push(StackElement {
            entry: se,
            id: self.outcome_tracker.next_id(),
        });
    }

    pub fn peek_prompt(&self) -> Option<&Prompt> {
        let last = self.encounter_stack.last();
        match last {
            None => None,
            Some(se) => match &se.entry {
                StackElementEntry::Prompt(p) => Some(p),
                _ => None,
            },
        }
    }

    pub fn pop_prompt(&mut self) -> Option<Prompt> {
        // Only pop if the top entry is actually a prompt — peek first so
        // we don't have to recreate the StackElement on the non-Prompt
        // branch. Borrow ends after the bool check.
        if !matches!(
            self.encounter_stack.last().map(|se| &se.entry),
            Some(StackElementEntry::Prompt(_))
        ) {
            return None;
        }
        let se = self.encounter_stack.pop()?;
        match se.entry {
            StackElementEntry::Prompt(p) => Some(p),
            // Unreachable: matches!() above guards this branch.
            _ => None,
        }
    }

    pub fn push_action(&mut self, action_execution_info: ActionExecutionInfo) {
        self.enqueue_event(StackElementEntry::Action(Box::new(action_execution_info)));
    }

    pub fn process_stack(&mut self) {
        if !self.initialized {
            return;
        }

        // Bail if we are already waiting on a player prompt.
        if self.peek_prompt().is_some() {
            return;
        }

        // 5e surprise, decided once, before the first turn opens — see
        // `resolve_opening_surprise` and the `surprise_resolved` field
        // for why it is here and not in `initialize`.
        if !self.surprise_resolved {
            self.surprise_resolved = true;
            self.resolve_opening_surprise();
        }

        // Open the current slot's turn before anything resolves. This is
        // the call that catches the encounter's very first actor, who
        // never advances into their slot and so would otherwise act with
        // no turn start at all. It has to come *before* the stack drains
        // rather than after: an action resolved this call can install
        // `UntilStartOfNextTurn` state (Dodge, Disengage), and opening
        // the turn afterwards would retroactively expire it.
        self.ensure_turn_started();

        while let Some(se) = self.encounter_stack.pop() {
            match se.entry {
                StackElementEntry::Prompt(p) => {
                    // A prompt was already on the stack; put it back and bail.
                    self.encounter_stack.push(StackElement {
                        entry: StackElementEntry::Prompt(p),
                        id: se.id,
                    });
                    return;
                }
                StackElementEntry::Action(a) => {
                    self.log_action_use(&a);
                    self.mark_two_weapon_opening(&a);
                    // Asked *before* the swing, because it is a
                    // question about the swing happening at all —
                    // `execute` re-validates and quietly does nothing
                    // if the world moved between enqueue and now, and a
                    // ledger stamped afterwards regardless would spend
                    // the wielder's once-a-turn Nick discount on a
                    // swing that never landed a blow or even rolled.
                    let offhand_swing =
                        a.action().is_offhand_swing() && a.validate(self);
                    let mut side_effects = a.execute(self);
                    if offhand_swing {
                        self.mark_offhand_swing(a.caster_id());
                    }
                    for sen in side_effects.drain(..) {
                        self.enqueue_event(StackElementEntry::SideEffect(sen));
                    }
                }
                StackElementEntry::SideEffect(s) => {
                    s.apply(self);
                    self.cleanup_dead_actors();
                    // Per-effect and ahead of the other two: one action
                    // can banish a creature and a second can Dispel the
                    // spell holding it there, and each has to land
                    // before the following effect measures the board.
                    self.reconcile_board_presence();
                    // Per-effect rather than once after the drain: a
                    // single action can grow someone and then move them,
                    // and the move has to measure the footprint the
                    // growth just bought. Running after
                    // `cleanup_dead_actors` also means a growth blocked
                    // by a neighbour lands the instant that neighbour
                    // falls.
                    self.reconcile_footprints();
                    // Per-effect for the same reason: one action can
                    // break a wizard's concentration and a second can
                    // Dispel what held the next flier up, and each drop
                    // has to land before the following effect measures
                    // the board.
                    self.reconcile_altitudes();
                }
            }
        }

        self.outcome_tracker.reset();

        // Auto-resolve any dying / stable actors before prompting. Each
        // dying actor takes their "turn" by rolling exactly one death save;
        // stable actors just have their slot skipped. The visited set
        // bounds the loop to one save per actor per process_stack call:
        // without it, in-loop removals shrink the initiative queue and
        // `advance()`'s wraparound revisits actors, double-counting saves.
        let mut visited: std::collections::HashSet<usize> =
            std::collections::HashSet::new();
        loop {
            let Some(curr_id) = self.initiative_tracker.current_player() else {
                return;
            };
            let Some(actor) = self.actors.get(&curr_id) else {
                // Active slot points at a removed actor — fix the queue.
                self.initiative_tracker.remove_actor(curr_id);
                continue;
            };
            // 5e controlled mount: "it moves as you direct it, and it
            // has only three action options: Dash, Disengage, and
            // Dodge." All three are things the rider is already choosing
            // by choosing where to walk, so a ridden mount's slot passes
            // straight through — it has spent its turn carrying somebody
            // on theirs. Skipped here rather than by draining its
            // resources so it never reaches a prompt with nothing to
            // pick; its reactions are untouched, and a horse still
            // answers a step past it with an opportunity attack.
            let ridden_by = actor.ridden_by();
            let dying = actor.is_dying();
            if actor.is_combat_active() && ridden_by.is_none() {
                break;
            }
            if !visited.insert(curr_id) {
                // We've already given this actor a save this call; bail
                // (everyone left in the queue is downed).
                break;
            }
            if let Some(rider_id) = ridden_by {
                let (mount, rider) = (self.actor_name(curr_id), self.actor_name(rider_id));
                self.log(format!("{} is under rein, and acts on {}'s turn.", mount, rider));
            } else if dying {
                self.resolve_death_save(curr_id);
            }
            // After the save (or if stable), advance to the next slot.
            // Use the wrapper so a wrap-around fires the round-end hook
            // (condition timers tick) — same semantics as a normal turn.
            self.advance_initiative();
            // Reset the next actor's resources so an active actor's first
            // turn after a sequence of skipped/dying slots starts fresh.
            if let Some(next_id) = self.initiative_tracker.current_player() {
                self.start_turn_for(next_id);
            }
        }

        // Encounter may have ended while resolving saves.
        if self.is_complete() {
            return;
        }

        // Skip the prompt if the encounter has wound down (everyone died).
        let Some(current_player_id) = self.initiative_tracker.current_player() else {
            return;
        };
        // Second call, and not a redundant one: the slot can change
        // between the top of this function and here without any
        // `advance_initiative` to announce it — an actor who dies on
        // their own turn (a damage reflect, a Hellish Rebuke) vacates
        // their index and `InitiativeTracker::remove_actor` lets the
        // next actor slide into it. That actor is about to be prompted,
        // and this is what opens their turn. A no-op whenever the slot
        // didn't move, since the latch still matches.
        self.ensure_turn_started();
        let Some(current_player) = self.actors.get(&current_player_id) else {
            return;
        };
        self.enqueue_event(StackElementEntry::Prompt(Prompt::new(
            current_player_id,
            current_player.available_actions(), // base actions + carried-consumable actions
        )));
    }
}

#[cfg(test)]
#[path = "encounter_tests.rs"]
mod tests;
