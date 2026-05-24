use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCIMITAR;
use crate::actions::spells::{
    ACID_SPLASH, BURNING_HANDS, CHAIN_LIGHTNING, CHARM_PERSON, CHILL_TOUCH, CONE_OF_COLD,
    COUNTERSPELL, DISINTEGRATE, FEAR, FIREBALL, FIRE_BOLT, FLY, HASTE, HOLD_PERSON,
    LIGHTNING_BOLT, LIGHTNING_LURE, MAGE_ARMOR, MAGIC_MISSILE, METEOR_SWARM, MIRROR_IMAGE,
    MISTY_STEP, POLYMORPH, POWER_WORD_KILL, RAY_OF_FROST, SCORCHING_RAY, SHATTER, SHIELD,
    SHOCKING_GRASP, SLEEP, SUNBURST, TIME_STOP,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Sorcerer PC template. CHA-primary full-caster — uses the same spell
/// roster as the wizard but anchors on Charisma rather than Intelligence.
/// The defining mechanical difference (RAW): Sorcery Points + Metamagic.
/// We don't model the per-cast metamagic toggle yet (would need an
/// ActionOverride wired through every spell's resolution); the template
/// instead leans on a tighter "blaster" spell list and higher CHA-anchored
/// DCs — sorcerers in this engine play as the wizard's evocation-first
/// cousin with extra CON / CHA save resilience.
///
/// Loadout: Fire Bolt / Ray of Frost / Chill Touch / Acid Splash / Shocking
/// Grasp cantrips for at-will, Burning Hands / Magic Missile / Shield as
/// level-1 staples, Scorching Ray / Mirror Image / Shatter / Misty Step
/// at level-2, Fireball / Lightning Bolt / Counterspell / Fear / Haste at
/// level-3, Polymorph at level-4, Cone of Cold at level-5, Chain Lightning
/// at level-6, Time Stop at level-9, Power Word Kill / Meteor Swarm /
/// Sunburst for the apex slots. PC flag flips on so the sorcerer rolls
/// death saves at 0 HP.
pub static SORCERER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    // Cantrips
    actions.push(&*FIRE_BOLT);
    actions.push(&*RAY_OF_FROST);
    actions.push(&*CHILL_TOUCH);
    actions.push(&*ACID_SPLASH);
    actions.push(&*SHOCKING_GRASP);
    actions.push(&*crate::actions::spells::FROSTBITE);
    actions.push(&*LIGHTNING_LURE);
    // Level 1
    actions.push(&*BURNING_HANDS);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*SHIELD);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*CHARM_PERSON);
    actions.push(&*SLEEP);
    // Level 2
    actions.push(&*SCORCHING_RAY);
    actions.push(&*MIRROR_IMAGE);
    actions.push(&*SHATTER);
    actions.push(&*MISTY_STEP);
    actions.push(&*HOLD_PERSON);
    // Level 3
    actions.push(&*FIREBALL);
    actions.push(&*LIGHTNING_BOLT);
    actions.push(&*COUNTERSPELL);
    actions.push(&*FEAR);
    actions.push(&*HASTE);
    actions.push(&*FLY);
    // Level 4
    actions.push(&*POLYMORPH);
    // Level 5
    actions.push(&*CONE_OF_COLD);
    // Level 6
    actions.push(&*CHAIN_LIGHTNING);
    // Level 6 — Disintegrate as the sorcerer's apex single-target nuke.
    actions.push(&*DISINTEGRATE);
    // Level 8
    actions.push(&*SUNBURST);
    // Level 7 — Mordenkainen's Sword as the sorcerer's force-melee
    // single-target burst (5d10 force, concentration). Sorcerer / wizard
    // / warlock share this spell RAW.
    actions.push(&*crate::actions::spells::MORDENKAINENS_SWORD);
    actions.push(&*crate::actions::spells::POWER_WORD_PAIN);
    // Level 9
    actions.push(&*TIME_STOP);
    actions.push(&*POWER_WORD_KILL);
    actions.push(&*METEOR_SWARM);
    actions.push(&*crate::actions::spells::MASS_POLYMORPH);
    // Latest spell additions: lv2 Acid Arrow, lv3 Tidal Wave, lv5 Dawn,
    // lv6 Investiture of Flame. Round out the sorcerer's mid-tier
    // blaster lineup with the new water / radiant / self-buff options.
    actions.push(&*crate::actions::spells::ACID_ARROW);
    actions.push(&*crate::actions::spells::TIDAL_WAVE);
    actions.push(&*crate::actions::spells::DAWN);
    actions.push(&*crate::actions::spells::INVESTITURE_OF_FLAME);
    // Latest sorcerer additions: lv1 Grease (DEX-save prone burst), lv2
    // Flaming Sphere (concentration fire burst). Both fit the blaster
    // archetype — Grease as a cheap lv1 disabler, Flaming Sphere as a
    // mid-cost AoE that pairs well with the sorcerer's concentration
    // lane (Haste / Polymorph would lose to it, which is the design intent).
    actions.push(&*crate::actions::spells::GREASE);
    actions.push(&*crate::actions::spells::FLAMING_SPHERE);
    // Latest control additions: lv3 Wind Wall (self-buff ranged-attack
    // disadvantage) + lv4 Otiluke's Resilient Sphere (single-target
    // inert envelope concentration). Skip Black Tentacles — RAW
    // wizard-only (a sorcerer's spells-known cap is tight enough that
    // the AoE niche is filled by Fireball / Cone of Cold / Sunburst).
    actions.push(&*crate::actions::spells::WIND_WALL);
    actions.push(&*crate::actions::spells::OTILUKES_RESILIENT_SPHERE);
    // Maximilian's Earthen Grasp (lv2) — single-target restraint +
    // 2d6 per round DoT. Sorcerers / wizards / druids all share the
    // spell RAW; the CHA-anchored DC keeps the sorcerer's pick punchy.
    actions.push(&*crate::actions::spells::MAXIMILIANS_EARTHEN_GRASP);
    // Vitriolic Sphere (lv4) — acid AoE with a delayed-drip rider on
    // failed-save targets. Slots between Fireball (lv3) and Cone of
    // Cold (lv5) in the sorcerer's blaster line; the residual drip
    // makes it a stronger pick when fire / cold resistance is dense.
    actions.push(&*crate::actions::spells::VITRIOLIC_SPHERE);
    // Newest sorcerer additions:
    //   - cantrip **Thunderclap**: self-centered 1-tile CON-save burst.
    //   - lv1 **Chromatic Orb**: 3d8 ranged spell attack of best
    //     damage type vs target. Fixes the sorcerer's stale lv1 lineup
    //     (only Magic Missile / Burning Hands for offense).
    //   - lv2 **Snilloc's Snowball Swarm**: cheap cold burst, fills the
    //     sorcerer's lv2 AoE slot (Shatter is the only existing pick).
    //   - lv2 **Mind Spike**: single-target psychic save-for-half, ranges
    //     out to 60ft and bypasses AC.
    //   - lv4 **Psychic Lance**: psychic save-for-half + Incapacitated
    //     on fail, single-target soft lock-down at lv4.
    actions.push(&*crate::actions::spells::THUNDERCLAP);
    actions.push(&*crate::actions::spells::CHROMATIC_ORB);
    actions.push(&*crate::actions::spells::SNILLOCS_SNOWBALL_SWARM);
    actions.push(&*crate::actions::spells::MIND_SPIKE);
    actions.push(&*crate::actions::spells::PSYCHIC_LANCE);
    // Newer sorcerer additions:
    //   - lv1 **Ice Knife**: ranged attack + neutral-burst cold
    //     shatter rider, fires hit-or-miss.
    //   - lv2 **Enlarge / Reduce**: single-target +1d4 weapon damage
    //     buff (concentration). Routes through the on-hit rider
    //     table — lets the sorcerer prop up an ally's martial
    //     output between the bigger-leverage Haste / Polymorph picks.
    actions.push(&*crate::actions::spells::ICE_KNIFE);
    actions.push(&*crate::actions::spells::ENLARGE_REDUCE);
    // Latest cantrip / lv1-2 additions shared with the wizard / druid:
    //   - cantrip **Sword Burst**: 1-tile force burst around caster.
    //   - cantrip **Blade Ward**: self damage-resistance till next turn.
    //   - lv1 **Catapult**: punchy single-target DEX-save bludgeoning.
    //   - lv1 **Earth Tremor**: self-centered DEX-save + prone burst.
    //   - lv1 **Fog Cloud**: concentration Blinded burst.
    //   - lv2 **Gust of Wind**: line push, concentration.
    actions.push(&*crate::actions::spells::SWORD_BURST);
    actions.push(&*crate::actions::spells::BLADE_WARD);
    actions.push(&*crate::actions::spells::CATAPULT);
    actions.push(&*crate::actions::spells::EARTH_TREMOR);
    actions.push(&*crate::actions::spells::FOG_CLOUD);
    actions.push(&*crate::actions::spells::GUST_OF_WIND);
    // Signature sorcerer additions:
    //   - lv1 **Chaos Bolt** (sorcerer-only): single-target attack with
    //     random elemental typing rolled per cast; doubled type-d8 chains
    //     to the nearest other enemy at 30 ft.
    //   - lv2 **Dragon's Breath**: 2-tile self cone with caster's choice
    //     of damage type (best-vs-target picker), DEX save for half.
    actions.push(&*crate::actions::spells::CHAOS_BOLT);
    actions.push(&*crate::actions::spells::DRAGONS_BREATH);
    // Telekinetic — cantrip bonus-action shove (5ft pull on STR-save
    // fail). Cheap repositioning tool; the sorcerer's bonus-action lane
    // is otherwise mostly empty (Misty Step / Quickened Spell aren't
    // modeled per-spell here).
    actions.push(&*crate::actions::spells::TELEKINETIC);
    // Green-Flame Blade — cantrip CHA-scaled melee touch (1d8 fire +
    // ability-modifier fire leap to the lowest-HP adjacent enemy).
    // Gives the sorcerer a melee touch option that scales off their
    // primary stat — Booming Blade is INT-only, so this slots into the
    // CHA-caster melee lane that was previously dead.
    actions.push(&*crate::actions::spells::GREEN_FLAME_BLADE);
    // Sapping Sting — Tasha's necromancy cantrip: 1d4 necrotic + prone
    // on a CON-save fail (30 ft range). Sorcerer pickup since the spell
    // is sorcerer / wizard in RAW; the prone rider sets up the
    // sorcerer's next-turn ranged spells at advantage.
    actions.push(&*crate::actions::spells::SAPPING_STING);
    // Newest sorcerer additions:
    //   - lv3 **Erupting Earth**: 3d12 bludgeoning DEX-save AoE
    //     (bypasses fire/cold resistance with bludgeoning typing).
    //   - lv4 **Blight**: 8d8 necrotic single-target nuke (necromancy
    //     against fleshy chunky enemies).
    //   - lv6 **Circle of Death**: 8d6 necrotic 30ft-radius AoE.
    //   - lv7 **Delayed Blast Fireball**: 12d6 fire AoE (signature lv7
    //     evocation; pairs with the sorcerer's blaster archetype).
    //   - lv8 **Incendiary Cloud**: 10d8 fire AoE (lv8 fire nuke,
    //     friend-or-foe agnostic).
    //   - lv9 **Weird**: 10d10 psychic + Frightened on fail (mass
    //     terror-lock at the apex).
    actions.push(&*crate::actions::spells::ERUPTING_EARTH);
    actions.push(&*crate::actions::spells::BLIGHT);
    actions.push(&*crate::actions::spells::CIRCLE_OF_DEATH);
    actions.push(&*crate::actions::spells::DELAYED_BLAST_FIREBALL);
    actions.push(&*crate::actions::spells::INCENDIARY_CLOUD);
    actions.push(&*crate::actions::spells::WEIRD);
    CreatureTemplate {
        name: "Sorcerer",
        // 'S' — distinct from Skeleton (lowercase 's'), Sage, etc.
        glyph: 'S',
        ac: 13, // unarmored, +DEX
        hitpoints: "9d6+18".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 11,
        dexterity: 14,
        wisdom: 12,
        constitution: 14,
        charisma: 18, // primary spellcasting ability
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        actions,
        // Level-9 full-caster loadout — mirrors wizard / cleric / druid.
        // Sorcerers have the same slot table as wizards RAW; the
        // difference shows up in spells-known caps, not slots.
        spell_slots_by_level: vec![4, 3, 3, 2, 2, 1, 1, 1, 1],
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Sorcerers are proficient in CON and CHA saves (5e PHB) —
        // distinct from the wizard's INT/WIS profile and the cleric's
        // WIS/CHA, which makes them tankier against the concentration-
        // breaking CON saves that hit casters in the heat of melee.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
    }
});
