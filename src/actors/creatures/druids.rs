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
    // Warding Wind (lv2 evocation, concentration) — a 10 ft ring of
    // roaring wind that makes ranged attacks into and out of it roll at
    // disadvantage, modelled through the `Untracked` lane Pass Without
    // Trace already rides. It was written, tested by nothing, and
    // carried by no template on the roster: a complete `impl Action`
    // that no player could pick and no encounter could roll. RAW gives
    // it to the bard, druid, sorcerer and wizard; the three full-caster
    // chassis that already carry Wind Wall get it here, where it sits as
    // the cheap always-available sibling to Wind Wall's lv3.
    actions.push(&*crate::actions::spells::WARDING_WIND);
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
    // lv2 **Protection from Poison** (abjuration): touch cleanse of
    // `Poisoned` plus the long-duration `Purified` install. Druid SRD
    // staple — slots into the same lv2 cleanse lane as Lesser Restoration
    // with a longer-lasting poison-specific buff rider.
    actions.push(&*crate::actions::spells::PROTECTION_FROM_POISON);
    // lv5 **Conjure Elemental** (conjuration): summon a single Large fire
    // elemental ally adjacent to the caster, concentration-bound. Sibling
    // to Conjure Animals (lv3, 2× wolves) on the druid's summon lane —
    // higher slot for a single bigger minion with fire immunity and
    // resistance to non-magical physical damage. Dropping concentration
    // despawns the elemental via the shared `Conjured` cleanup path.
    actions.push(&*crate::actions::spells::CONJURE_ELEMENTAL);
    // lv4 **Dominate Beast** (enchantment): single-target concentration
    // charm + dominate on a Beast-typed enemy. WIS save vs the druid's
    // spell DC; on fail target is Charmed by the druid AND has
    // disadvantage on every attack (the `Dominated` clause) for 10
    // rounds. Beast-only gate via `custom_validate_input` keeps the AI
    // from wasting the slot on a stone golem or undead. Slots cleanly
    // into the druid's lv4 control lane next to Polymorph (target-form
    // swap) and Confusion (AoE chaos) — Dominate Beast is the single-
    // target lockdown for the wild-animal threats the druid encounters
    // most often (Brown Bear, Tiger, Wolf, Mammoth, Owlbear).
    actions.push(&*crate::actions::spells::DOMINATE_BEAST);
    // Natural Recovery — Druid Circle of the Land lv2 feature. Ships
    // on the LAND_DRUID_TEMPLATE below via a subclass-of clone; the
    // action itself is pushed into the shared `actions` list here so
    // both the baseline druid and the Land subclass can invoke it. The
    // baseline druid template stays feature-tag-free (Natural Recovery
    // only fires when the LAND_DRUID_TEMPLATE's `features` set carries
    // the tag), so pushing the action here is harmless for the
    // baseline — `custom_validate_input` gates on the feature flag.
    actions.push(&*crate::actions::class_features::NATURAL_RECOVERY);
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

/// Land Druid — Circle of the Land subclass template. Identical
/// envelope to the baseline `DRUID_TEMPLATE` (level-9 full-caster,
/// scimitar + full druid spell list, 4/3/3/2/2/1/1/1/1 slot ladder,
/// INT + WIS save profs) with one subclass feature layered on:
/// **Natural Recovery** (level 2) — once per short rest, recover
/// spell slots totaling half caster level (rounded up), no slot
/// above 5th. We collapse the RAW pool math to the same fixed shape
/// as Arcane Recovery — one level-1 slot at any level plus one
/// level-2 slot at level 3+ — so both features share the exact same
/// action + validation + effects shape and the once-per-rest gate
/// stays uniform across the class-feature lane.
///
/// Distinct from `DRUID_TEMPLATE` (subclass-less baseline) so a
/// Land-vs-Land or Land-vs-baseline encounter renders unambiguously by
/// name and the subclass feature doesn't accidentally stack RAW-
/// illegally on a single PC build. Glyph 'L' so the Land druid shows
/// up distinctly on the map next to the baseline 'D'.
pub static LAND_DRUID_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Druid envelope wholesale
    // and overwrite only the per-subclass differences (name / glyph /
    // features). The `..base.clone()` tail picks up every other field
    // — actions, spell slots, save profs — without an N-line
    // field-by-field copy.
    CreatureTemplate {
        name: "Land Druid",
        glyph: 'L',
        // Circle of the Land subclass features layered onto the
        // baseline druid envelope:
        //   - `NATURAL_RECOVERY_TAG`: Natural Recovery (lv2). Once
        //     per short rest, restore up to (level-1 + level-2)
        //     spell slots — mirroring the Arcane Recovery shape.
        //     Fires at will as a free-cost action; the once-per-rest
        //     charge is the entire resource cost.
        features: HashSet::from([crate::actions::class_features::NATURAL_RECOVERY_TAG]),
        ..DRUID_TEMPLATE.clone()
    }
});

/// Moon Druid — Druid Circle **Circle of the Moon** subclass build
/// (PHB), and with it both PHB Druid Circles have a build in the
/// engine. One subclass feature, in two halves:
///
///   - **Combat Wild Shape** (lv2, first half) — Wild Shape as a bonus
///     action rather than an Action, so the druid can transform and
///     still act on the same turn.
///   - **Combat Wild Shape** (lv2, second half) — while in form, a
///     bonus action expends a spell slot to regain 1d8 HP per slot
///     level.
///
/// **Circle Forms** (lv2) is the third piece and it ships as data
/// rather than as an action: the CR-1 cap it imposes is what fixes the
/// form at a brown bear, and the bear is what `BEAST_FORM_TEMP_HP`
/// (34 HP) and `BEAST_FORM_CLAWS` (2d6+4 slashing, +4 to hit from the
/// form's Strength) describe. One beast, two constants.
///
/// The subclass is the sharpest either-or in the engine, because
/// `WildShaped` blocks spell slots outright. A druid who takes the form
/// is trading Moonbeam, Call Lightning, Sleet Storm, Wall of Fire,
/// every heal on the list and Reverse Gravity — the whole reason to
/// play a full caster — for 34 temp HP and a 2d6+4 melee swing. Nothing
/// else on the chassis asks the player to give up that much at once,
/// and the answer genuinely varies: a druid holding a Web is throwing
/// away the fight to become a bear, and a druid at 15 HP with nothing
/// concentrating and two hostiles in contact is not.
///
/// Which is why the two halves point in opposite directions and that's
/// deliberate. The bonus-action transform is an *entry* discount — it
/// lowers the cost of committing. The slot-to-HP conversion is what the
/// druid does *after* committing, and it is the only thing their slots
/// are still good for once the spell list is locked out. So the feature
/// reads as "getting in is cheap, and once you're in, your magic is
/// hit points." A Land Druid, the sibling template, never faces this:
/// Natural Recovery hands slots *back* so the druid can keep casting,
/// which is the same resource pointed at the opposite strategy.
///
/// **Nothing about the stat block changes.** The Moon Druid is the
/// baseline `DRUID_TEMPLATE` — WIS 18, AC 14, 58 HP, the full 1-9 spell
/// list, the same scimitar — plus two actions and one tag. That is
/// RAW-faithful and it is also the point: the form's numbers come from
/// the form (34 HP, +4 claws), not from the druid, so a Moon Druid out
/// of shape is exactly a druid. `BEAST_FORM_CLAWS` sits on the action
/// list permanently and its own validator refuses it out of form.
///
/// Primal Strike (lv6) has no engine surface — it makes beast-form
/// attacks count as magical for overcoming resistance, and the engine
/// models "resistance to nonmagical bludgeoning/piercing/slashing" as
/// flat physical resistance with no magic axis to overcome. Elemental
/// Wild Shape (lv10) and Thousand Forms (lv14) would both need a second
/// form's worth of constants for a strictly better version of the
/// button that already exists.
///
/// Glyph 'B' — for the **B**ear the form settles on. Distinct from
/// baseline druid 'D' and Land Druid 'L'.
pub static MOON_DRUID_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_attacks::BEAST_FORM_CLAWS;
    use crate::actions::class_features::{COMBAT_WILD_SHAPE_TAG, WILD_HEAL, WILD_SHAPE};
    // Not the tag-only clone the Land Druid uses: the form needs an
    // attack to swing and two bonus actions to enter and sustain it.
    // The `..DRUID_TEMPLATE.clone()` tail carries the entire caster
    // chassis unchanged — which is exactly right, since a Moon Druid
    // out of shape is a druid.
    let mut actions = DRUID_TEMPLATE.actions.clone();
    actions.push(&*WILD_SHAPE);
    actions.push(&*WILD_HEAL);
    actions.push(&*BEAST_FORM_CLAWS);
    CreatureTemplate {
        name: "Moon Druid",
        glyph: 'B',
        actions,
        features: HashSet::from([COMBAT_WILD_SHAPE_TAG]),
        ..DRUID_TEMPLATE.clone()
    }
});

/// Spores Druid — Druid Circle **Circle of Spores** subclass build
/// (TCE), and the first druid circle in the engine that keeps the whole
/// spell list *and* asks the druid to stand in the front rank. Two
/// subclass features, both level 2:
///
///   - **Halo of Spores** — a reaction, at will: one creature within
///     10 ft takes 1d6 necrotic unless it makes a Constitution save
///     against the druid's spell DC. The engine's only *declared*
///     reaction (see `action_template::reaction_only`), and the reason
///     the subclass plays differently from every other caster on the
///     roster: the halo is paid for out of a slot the druid was not
///     otherwise going to spend, so a Spores Druid who casts and then
///     retreats still deals damage on the way out.
///
///   - **Symbiotic Entity** — an Action, once per short rest: 36
///     temporary hit points, +1d6 necrotic on every melee weapon hit,
///     and the halo's die rolled twice. It ends when the temp HP runs
///     out, which the engine enforces at the temp-HP chokepoint rather
///     than on a timer (`TEMP_HP_BOUND_CONDITIONS`).
///
/// The two halves are one decision. The halo alone is a 3.5-damage
/// consolation prize; the symbiote alone is 36 temp HP on a d8 chassis.
/// Together they turn the druid's *proximity* into a resource — the
/// halo only reaches 10 ft, the melee rider only fires on a scimitar
/// swing, and both scale up the moment the symbiote is riding, so the
/// circle rewards a druid who spends the fight closer to it than a
/// WIS-18 full caster has any business being. The 36 temp HP is what
/// makes that survivable, and the fact that it is also the feature's
/// clock is what keeps it honest: standing in melee spends the
/// shield, and spending the shield ends the rider that made standing
/// there worthwhile.
///
/// Contrast the sibling circles. The Moon Druid gives up its entire
/// spell list for a bear. The Land Druid hands slots *back* so it can
/// keep casting from range. Spores is the only one that asks the druid
/// to be both things at once, and it is the only one whose defining
/// resource is depleted by the enemy rather than by the druid.
///
/// RAW features not shipped: **Fungal Infestation** (lv6 — reanimate a
/// beast or humanoid that dies within 10 ft as a zombie; a
/// summon-on-death lane the engine's drop hook doesn't expose yet),
/// **Spreading Spores** (lv10 — a movable 10-ft damage cube, which
/// needs persistent terrain-anchored AoE), and **Fungal Body** (lv14 —
/// crit immunity plus Blinded / Frightened / Poisoned immunity, RAW-
/// gated well past this chassis's level).
///
/// Glyph 'F' — for the **F**ungus. Distinct from baseline druid 'D',
/// Land Druid 'L' and Moon Druid 'B'.
pub static SPORES_DRUID_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{
        HALO_OF_SPORES, HALO_OF_SPORES_TAG, SYMBIOTIC_ENTITY, SYMBIOTIC_ENTITY_TAG,
    };
    // Two actions on top of the baseline caster chassis; nothing else
    // about the druid changes, which is the same shape the Moon Druid
    // uses. The stat line stays a druid's because the subclass adds
    // survivability rather than replacing the body.
    let mut actions = DRUID_TEMPLATE.actions.clone();
    actions.push(&*HALO_OF_SPORES);
    actions.push(&*SYMBIOTIC_ENTITY);
    CreatureTemplate {
        name: "Spores Druid",
        glyph: 'F',
        actions,
        features: HashSet::from([HALO_OF_SPORES_TAG, SYMBIOTIC_ENTITY_TAG]),
        ..DRUID_TEMPLATE.clone()
    }
});

/// Stars Druid — Druid Circle **Circle of the Stars** subclass build
/// (Tasha's), and the fourth circle on the roster. One subclass
/// feature, in three shapes, plus the star map that comes with it.
///
///   - **Star Map** (lv2) — the circle's spell ribbon. Guiding Bolt
///     joins the list, and it is the one thing the baseline druid kit
///     is missing: a level-1 ranged spell attack whose payload is
///     4d6 radiant *and* advantage for the next creature to swing at
///     the target. Guidance is already on the baseline chassis.
///   - **Starry Form** (lv2) — a bonus action, once per short rest,
///     that puts the druid inside one of three constellations for ten
///     rounds. `STARRY_FORM_ARCHER`, `STARRY_FORM_CHALICE` and
///     `STARRY_FORM_DRAGON`, sharing one charge.
///   - **Starry Bolt** — the Archer's payload, and the only one of the
///     three that is an action rather than a passive. Bonus action,
///     1d8 + WIS radiant at 60 ft, at-will while the form holds.
///
/// The three shapes are the subclass, and the reason it plays unlike
/// the other three circles is that they are not three grades of the
/// same thing — they are three different answers to "what is this
/// round short of":
///
///   - **Archer** buys damage the druid otherwise has no way to spend
///     a bonus action on. A druid holding concentration on Moonbeam
///     has an Action committed to moving the beam and nothing at all
///     to do with the rest of the turn; the bolt is that turn's
///     second half.
///   - **Chalice** buys reach for the heal lane. Cure Wounds is one
///     ally at touch range and Healing Word is one ally at 60 ft; the
///     Chalice makes every one of them two, and picks the second
///     target itself — the most wounded ally the spell did not already
///     cover.
///   - **Dragon** buys nothing on the turn it is cast, and is the
///     strongest of the three in the fight where the druid's
///     concentration is what the enemy is trying to break. Floored at
///     10, a DC 10 concentration save on this chassis stops being a
///     roll.
///
/// Contrast the sibling circles, which all answer the same question by
/// changing what the druid *is*. Moon trades the spell list for a bear.
/// Spores trades hit points for a melee rider. Land trades nothing and
/// hands slots back. Stars is the only one whose feature changes what
/// the druid's *existing* kit is worth without touching the kit, which
/// is why it is also the only one that stacks cleanly with everything
/// on the baseline list.
///
/// RAW features not shipped: **Cosmic Omen** (lv6 — a reaction that
/// adds or subtracts 1d6 from a roll, gated on a coin-flip the engine
/// has nowhere to consult), **Twinkling Constellations** (lv10 — bumps
/// the Archer die to 2d8, gives the Chalice and Dragon flight, and lets
/// the form change shape every turn, all of it RAW-gated well past this
/// chassis), and **Full of Stars** (lv14 — bludgeoning / piercing /
/// slashing resistance while transformed).
///
/// Glyph 'S' — for the **S**tars. Distinct from baseline druid 'D',
/// Land Druid 'L', Moon Druid 'B' and Spores Druid 'F'.
pub static STARS_DRUID_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{
        STARRY_BOLT, STARRY_FORM_ARCHER, STARRY_FORM_CHALICE, STARRY_FORM_DRAGON, STARRY_FORM_TAG,
    };
    // Four actions on top of the baseline caster chassis plus the Star
    // Map spell — the same additive shape the Spores Druid above uses.
    // Nothing about the druid's body changes: Stars adds reach and
    // reliability to the kit it already has rather than replacing it.
    let mut actions = DRUID_TEMPLATE.actions.clone();
    actions.push(&*crate::actions::spells::GUIDING_BOLT);
    actions.push(&*STARRY_FORM_ARCHER);
    actions.push(&*STARRY_FORM_CHALICE);
    actions.push(&*STARRY_FORM_DRAGON);
    actions.push(&*STARRY_BOLT);
    CreatureTemplate {
        name: "Stars Druid",
        glyph: 'S',
        // One tag for all three constellations — see `STARRY_FORM_TAG`.
        // The choice between them is the feature; three charges would
        // make it three features.
        features: HashSet::from([STARRY_FORM_TAG]),
        actions,
        ..DRUID_TEMPLATE.clone()
    }
});
