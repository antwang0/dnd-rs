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
use std::collections::HashSet;
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
    // lv6 **Investiture of Stone** — self-only concentration buff:
    // bludgeoning + piercing + slashing resistance plus 1d10 force
    // melee retaliation. The earth-themed sibling of Investiture of
    // Ice on the druid's transmutation list — fits thematically with
    // the druid's Wall of Stone and Spike Stones. Mutually exclusive
    // with Investiture of Ice via the concentration short-circuit.
    actions.push(&*crate::actions::spells::INVESTITURE_OF_STONE);
    // Latest druid additions (XGtE):
    //   - lv4 **Watery Sphere** (conjuration): single-target STR-save
    //     restraint. Water-themed sister to Maximilian's Earthen Grasp
    //     at the lv4 tier — lifts the target out of melee envelopes and
    //     locks them down while concentration holds.
    //   - lv6 **Investiture of Wind** (transmutation): self-only
    //     concentration buff — ranged-attack disadvantage to attackers
    //     plus +60ft flying speed. Wind-themed sibling of the Ice /
    //     Stone investitures already on the druid list; fills the
    //     mobility lane the druid otherwise had to spend Fly for.
    actions.push(&*crate::actions::spells::WATERY_SPHERE);
    actions.push(&*crate::actions::spells::INVESTITURE_OF_WIND);
    // Druid water / air additions (XGtE):
    //   - lv2 **Dust Devil** (conjuration): elemental-air burst, STR
    //     save 1d8 bludgeoning + 4-tile push on fail. Pairs with the
    //     druid's existing Wind Wall as the lv2 air-themed displacement
    //     option — slots between Thunderwave (caster-self) and Gust of
    //     Wind (line-shaped push) thematically.
    //   - lv5 **Maelstrom** (evocation): 3-tile water-burst, 6d6
    //     bludgeoning STR-save half + pull-into-center on fail. The
    //     water-themed sibling of Tidal Wave at a higher slot — pulls
    //     enemies into the vortex rather than knocking them prone for
    //     spike-stones / spike-growth follow-up.
    actions.push(&*crate::actions::spells::DUST_DEVIL);
    actions.push(&*crate::actions::spells::MAELSTROM);
    // Druid earth-themed capstone — lv6 **Bones of the Earth**
    // (transmutation, XGtE): 6d6 bludgeoning 2-tile burst, DEX-save for
    // half + Prone on fail. Slots between Sleet Storm (lv3 control) and
    // Earthquake (lv8 mass Prone) on the druid's earth-themed control
    // ladder — a clean Prone-rider AoE that pairs with Spike Growth /
    // Spirit Guardians for stacked damage on enemies caught flat.
    actions.push(&*crate::actions::spells::BONES_OF_THE_EARTH);
    // lv3 **Wall of Water** (evocation, XGtE): 3-tile burst, no save /
    // no damage — every enemy in the burst picks up `WindWalled` for
    // the duration (ranged-attacker disadvantage). Concentration-bound.
    // The druid's water-themed ranged-defense option — slots alongside
    // Wind Wall (lv3 self-only) on the deflection ladder.
    actions.push(&*crate::actions::spells::WALL_OF_WATER);
    // Newest druid additions:
    //   - lv1 **Longstrider** (transmutation): touch +10 ft speed for
    //     1 hour, no concentration. Solid pre-combat ally mobility buff
    //     — composes with Fly / Spider Climb / Investiture of Wind via
    //     the central `condition_speed_bonus` lane.
    //   - lv2 **Earthbind** (transmutation, XGtE): single-target STR-save
    //     vs the druid's spell DC; on fail the target's flying speed
    //     drops to 0. Strips both `Flying` and `InvestedInWind` from a
    //     failed-save target — the druid's grounding tool for airborne
    //     enemies (wyverns, dragons, fire imps). Concentration-bound.
    actions.push(&*crate::actions::spells::LONGSTRIDER);
    actions.push(&*crate::actions::spells::EARTHBIND);
    // Latest druid additions (PHB druid list):
    //   - lv2 **Enhance Ability** (transmutation): touch ally buff — 2d6
    //     temp HP + flat +2 saves for the duration (concentration). Pairs
    //     with the druid's healing / cleanse lane on a wounded frontliner.
    //   - lv5 **Contagion** (necromancy): single-target touch CON-save
    //     vs the druid's spell DC; on fail target picks up Poisoned for
    //     10 rounds. The druid's signature single-target disease — slots
    //     between Heat Metal (lv2) and Insect Plague (lv5) on the
    //     debilitating-debuff ladder.
    actions.push(&*crate::actions::spells::ENHANCE_ABILITY);
    actions.push(&*crate::actions::spells::CONTAGION);
    // Latest druid addition (TCE):
    //   - lv3 **Ashardalon's Stride** (transmutation): mobility + control
    //     combo at lv3 — +20 ft speed plus 1d6 fire trail damage to
    //     footprint-adjacent enemies on each step. Pairs naturally with
    //     the druid's wild-shape repositioning flavor; mirrors the
    //     wizard / sorcerer additions on the same spell.
    actions.push(&*crate::actions::spells::ASHARDALONS_STRIDE);
    // Latest druid additions: lv4 Freedom of Movement (ally-buff
    // restraint cleanse + immunity) and lv2 Darkness (concentration
    // symmetric-blind zone). Both are druid SRD staples that pair with
    // the wild-shape / Entangle control kit.
    actions.push(&*crate::actions::spells::FREEDOM_OF_MOVEMENT);
    actions.push(&*crate::actions::spells::DARKNESS);
    // lv5 **Conjure Elemental** (conjuration): summon a single Large fire
    // elemental ally adjacent to the caster, concentration-bound. Sibling
    // to Conjure Animals (lv3, 2× wolves) on the druid's summon lane —
    // higher slot for a single bigger minion with fire immunity and
    // resistance to non-magical physical damage. Dropping concentration
    // despawns the elemental via the shared `Conjured` cleanup path.
    actions.push(&*crate::actions::spells::CONJURE_ELEMENTAL);
    CreatureTemplate {
        name: "Druid",
        glyph: 'D',
        ac: 14, // leather armor (11) + DEX(+2) + Wis-adjacent shield use; rounded.
        hitpoints: "9d8+18".parse().unwrap(),
        strength: 10,
        dexterity: 14,
        constitution: 14,
        intelligence: 12,
        wisdom: 18,     // primary spellcasting ability
        charisma: 10,
        // 5e druids speak Druidic in addition to their starting language.
        languages: HashSet::from([Language::Common, Language::Druidic]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Level-9 full-caster loadout — mirrors wizard / cleric.
        spell_slots_by_level: vec![4, 3, 3, 2, 2, 1, 1, 1, 1],
        rolls_death_saves: true,
        // 5e druids are proficient in INT and WIS saves.
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        ..CreatureTemplate::defaults()
    }
});
