use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCIMITAR;
use crate::actions::spells::{
    CALL_LIGHTNING, CONFUSION, CURE_WOUNDS, DAYLIGHT, DISPEL_MAGIC, FAERIE_FIRE, FLY, GOODBERRY,
    HEALING_WORD, HEAT_METAL, HEROES_FEAST, ICE_STORM, LESSER_RESTORATION, LEVITATE, MAGIC_STONE,
    MASS_CURE_WOUNDS, MOONBEAM, PLANT_GROWTH, POISON_SPRAY, POLYMORPH, REVERSE_GRAVITY,
    SLEET_STORM, SPIKE_GROWTH, SPIKE_STONES, STORM_OF_VENGEANCE, THORN_WHIP, THUNDERWAVE,
    WALL_OF_FIRE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Druid PC template. WIS-primary full-caster with a nature-flavored
/// spell list. Plays as a midline support / control caster — strong
/// AoE (Moonbeam, Call Lightning, Sleet Storm), solid heals (Cure
/// Wounds, Healing Word, Goodberry, Mass Cure Wounds), terrain control
/// (Spike Growth, Wall of Fire), and the level-7 nuke Reverse Gravity
/// for boss fights.
///
/// Loadout: Poison Spray + Thorn Whip cantrips for at-will damage,
/// Scimitar as the weapon fallback. Healing line spans 1→5
/// (Goodberry / Healing Word / Cure Wounds / Lesser Restoration /
/// Mass Cure Wounds). Control line spans 1→9 (Faerie Fire / Spike
/// Growth / Sleet Storm / Polymorph / Reverse Gravity).
///
/// Stats target a level-9 druid: 58 HP (9d8+18), AC 14 (leather + DEX),
/// WIS 18 (spell save DC 8+4+4 = 16). Full-caster slot table mirrors
/// the wizard / cleric loadout in this engine: 4/3/3/2/2/1/1/1/1.
/// PC flag flips on so the druid rolls death saves at 0 HP.
pub static DRUID_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    // Cantrips
    actions.push(&*POISON_SPRAY);
    actions.push(&*THORN_WHIP);
    actions.push(&*MAGIC_STONE);
    // Frostbite — cold-themed druid cantrip with CON-save / Slowed rider.
    actions.push(&*crate::actions::spells::FROSTBITE);
    // Level 1
    actions.push(&*GOODBERRY);
    actions.push(&HEALING_WORD);
    actions.push(&*CURE_WOUNDS);
    actions.push(&*FAERIE_FIRE);
    actions.push(&*THUNDERWAVE);
    // Level 2
    actions.push(&*MOONBEAM);
    actions.push(&*SPIKE_GROWTH);
    actions.push(&*HEAT_METAL);
    actions.push(&*LESSER_RESTORATION);
    actions.push(&*LEVITATE);
    // Level 3
    actions.push(&*CALL_LIGHTNING);
    actions.push(&*SLEET_STORM);
    actions.push(&*DAYLIGHT);
    actions.push(&*DISPEL_MAGIC);
    actions.push(&*PLANT_GROWTH);
    actions.push(&*FLY);
    // Level 4
    actions.push(&*ICE_STORM);
    actions.push(&*POLYMORPH);
    actions.push(&*WALL_OF_FIRE);
    actions.push(&*CONFUSION);
    actions.push(&*SPIKE_STONES);
    // Level 5
    actions.push(&*MASS_CURE_WOUNDS);
    // Level 6 — apex pre-fight buff: ally temp-HP + heal + Heroic.
    actions.push(&*HEROES_FEAST);
    // Level 3 — Conjure Animals (summon 2 wolves on caster's team).
    // RAW: druid / ranger; we slot at druid lv3.
    actions.push(&*crate::actions::spells::CONJURE_ANIMALS);
    // Level 7 — Fire Storm joins as the druid's big elemental burst.
    actions.push(&*REVERSE_GRAVITY);
    actions.push(&*crate::actions::spells::FIRE_STORM);
    // Level 9 — the apex druid spell.
    actions.push(&*STORM_OF_VENGEANCE);
    // Latest druid additions: lv3 Tidal Wave (water-themed CON-save bludgeoning
    // + prone burst) and lv5 Dawn (radiant CON-save concentration burst).
    actions.push(&*crate::actions::spells::TIDAL_WAVE);
    actions.push(&*crate::actions::spells::DAWN);
    // Flaming Sphere — druid lv2 conjuration. Concentration-bound fire
    // burst that pairs with the druid's other concentration loops
    // (Moonbeam, Heat Metal) without stacking — adds a single-target /
    // small-cluster fire option to the kit.
    actions.push(&*crate::actions::spells::FLAMING_SPHERE);
    // Wind Wall — lv3 evocation, druid-themed (RAW: druid / ranger
    // spell list). Self-buff that imposes ranged-attack disadvantage
    // on incoming arrows / bolts — fits the wandering-naturalist
    // archetype that the druid kit centers on.
    actions.push(&*crate::actions::spells::WIND_WALL);
    // Shillelagh — cantrip prime that primes the druid's next melee
    // weapon hit with +1d8 force damage. Pairs with the scimitar /
    // thorn-whip lane for a bonus-action prime + action melee swing
    // route, scaling the druid's at-will burst.
    actions.push(&*crate::actions::spells::SHILLELAGH);
    // Maximilian's Earthen Grasp — lv2 transmutation, concentration.
    // Sticky single-target restraint with a per-round bludgeoning
    // drip. Fits the druid's "control + DoT" lane next to Moonbeam /
    // Heat Metal — distinct from Spike Growth (which is movement-
    // triggered) so the two can coexist on different concentration
    // turns.
    actions.push(&*crate::actions::spells::MAXIMILIANS_EARTHEN_GRASP);
    // Guidance — divination cantrip (cleric / druid). Touch-range buff
    // that applies Inspired (+3 flat-buff to next attack / save / check)
    // on the target. Custom-validate gates against re-priming an already-
    // inspired ally. Slots cleanly into the druid's bonus-cantrip lane
    // next to Shillelagh.
    actions.push(&*crate::actions::spells::GUIDANCE);
    // Ice Knife — lv1 conjuration (druid / sorcerer / wizard). Ranged
    // spell attack 1d10 piercing + DEX-save 2d6 cold burst at the
    // target's tile (fires hit OR miss). Gives the druid a non-
    // concentration lv1 blaster pick that pairs nicely with the
    // existing single-target heals.
    actions.push(&*crate::actions::spells::ICE_KNIFE);
    // Latest nature-themed lv1-2 additions:
    //   - lv1 **Earth Tremor**: 10-ft self-centered DEX-save burst.
    //     1d6 bludgeoning + prone on a fail — the druid's at-cost
    //     analog to Thunderwave with a permanent prone rider.
    //   - lv1 **Fog Cloud**: 4-tile concentration burst that blinds
    //     every actor caught in it.
    //   - lv2 **Gust of Wind**: line push (6 tiles) on STR-save fail,
    //     concentration. Complements Plant Growth's entangle by
    //     repositioning targets *off* the spike-growth tile.
    actions.push(&*crate::actions::spells::EARTH_TREMOR);
    actions.push(&*crate::actions::spells::FOG_CLOUD);
    actions.push(&*crate::actions::spells::GUST_OF_WIND);
    // lv3 **Conjure Barrage**: 2-tile cone burst for 3d8 piercing,
    // DEX save for half. Ranger-flavored in RAW but conjuration school
    // and ammo-arrow flavor fit the druid's nature kit cleanly — and
    // the druid has the lv3 slots the half-caster ranger template
    // currently lacks.
    actions.push(&*crate::actions::spells::CONJURE_BARRAGE);
    // Primal Savagery — cantrip melee spell attack: 1d10 acid via WIS-
    // scaled spell attack. Gives the druid a wild-shape-flavor melee
    // touch cantrip that scales off their primary stat — distinct from
    // the existing Thorn Whip (ranged pull) / Magic Stone (bonus-action
    // prime) lanes since it lands as a clean weapon-attack alternative
    // when the druid is already in melee.
    actions.push(&*crate::actions::spells::PRIMAL_SAVAGERY);
    // Newest druid additions:
    //   - lv6 **Wall of Thorns**: 7d8 piercing 15ft burst on the enemy
    //     side of a conjured wall, DEX save for half. Concentration-
    //     bound — slots cleanly between Heroes' Feast (lv6 buff) and
    //     the apex lv7 / lv8 evocations on the druid ladder.
    //   - lv8 **Tsunami**: 6d10 bludgeoning + prone in a 30-ft burst,
    //     friend-or-foe agnostic, concentration. Druid's signature
    //     elemental nuke at the apex tier — bigger footprint than
    //     Tidal Wave with a prone rider that sets up melee allies.
    actions.push(&*crate::actions::spells::WALL_OF_THORNS);
    actions.push(&*crate::actions::spells::TSUNAMI);
    // lv2 **Barkskin**: touch concentration buff that sets the target's
    // AC to 16 unless their natural / worn AC is already higher. Slots
    // into the druid's protective lane next to Healing Word / Cure
    // Wounds — a front-line ally with leather armor or hide gets a
    // meaningful AC bump for one concentration slot.
    actions.push(&*crate::actions::spells::BARKSKIN);
    // lv2 **Pass Without Trace**: concentration aura that imposes
    // disadvantage on attacks targeting any ally inside the 30ft sphere.
    // Fills the druid's "ambush / cover" lane — the AI auto-picks every
    // ally in the aura at cast time so a clustered party benefits as a
    // group.
    actions.push(&*crate::actions::spells::PASS_WITHOUT_TRACE);
    actions.push(&*crate::actions::spells::ABSORB_ELEMENTS);
    actions.push(&*crate::actions::spells::PRODUCE_FLAME);
    actions.push(&*crate::actions::spells::CREATE_BONFIRE);
    actions.push(&*crate::actions::spells::ENTANGLE);
    actions.push(&*crate::actions::spells::FLAME_BLADE);
    actions.push(&*crate::actions::spells::WITHER_AND_BLOOM);
    // lv2 Crown of Thorns: piercing burst + restrained on failed STR save.
    actions.push(&*crate::actions::spells::CROWN_OF_THORNS);
    // lv3 Conjure Locusts: 4d10 piercing burst (CON save, half).
    actions.push(&*crate::actions::spells::CONJURE_LOCUSTS);
    actions.push(&*crate::actions::spells::PROTECTION_FROM_ENERGY);
    actions.push(&*crate::actions::spells::HEALING_SPIRIT);
    // Latest druid additions:
    //   - lv5 **Wall of Stone**: 2-tile burst on a DEX save; failed-save
    //     targets are Restrained for 10 rounds (concentration-anchored).
    //     Slots in next to Wall of Fire / Spike Stones on the area-
    //     denial lane; the restraint is the load-bearing crowd-control.
    //   - lv6 **Investiture of Ice**: self-only concentration buff —
    //     cold resistance plus 1d10 cold retaliation on melee hits.
    //     Symmetric to the existing Investiture of Flame (wizard /
    //     sorcerer pick); the druid gets the cold variant for thematic
    //     fit with their other cold spells (Sleet Storm / Ice Storm).
    actions.push(&*crate::actions::spells::WALL_OF_STONE);
    actions.push(&*crate::actions::spells::INVESTITURE_OF_ICE);
    CreatureTemplate {
        name: "Druid",
        glyph: 'D',
        ac: 14, // leather armor (11) + DEX(+2) + Wis-adjacent shield use; rounded.
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 12,
        dexterity: 14,
        wisdom: 18,     // primary spellcasting ability
        constitution: 14,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        // 5e druids speak Druidic in addition to their starting language.
        languages: HashSet::from([Language::Common, Language::Druidic]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Level-9 full-caster loadout — mirrors wizard / cleric.
        spell_slots_by_level: vec![4, 3, 3, 2, 2, 1, 1, 1, 1],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // 5e druids are proficient in INT and WIS saves.
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
