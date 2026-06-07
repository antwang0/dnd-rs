use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCIMITAR;
use crate::actions::spells::{
    ACID_SPLASH, BURNING_HANDS, CHAIN_LIGHTNING, CHARM_PERSON, CHILL_TOUCH, CLOUD_OF_DAGGERS,
    CONE_OF_COLD, COUNTERSPELL, DISINTEGRATE, FEAR, FIREBALL, FIRE_BOLT, FLY, HASTE, HOLD_PERSON,
    LIGHTNING_BOLT, LIGHTNING_LURE, MAGE_ARMOR, MAGIC_MISSILE, METEOR_SWARM, MIRROR_IMAGE,
    MISTY_STEP, POLYMORPH, POWER_WORD_KILL, RAY_OF_FROST, SCORCHING_RAY, SHATTER, SHIELD,
    SHOCKING_GRASP, SLEEP, SUNBURST, TIME_STOP, WITCH_BOLT,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Sorcerer PC template. CHA-primary full-caster — uses the same spell
/// roster as the wizard but anchors on Charisma rather than Intelligence.
/// The defining mechanical differences (RAW): Sorcery Points + Metamagic.
///
/// **Metamagic** is modeled via the `EmpoweredSpell` bonus-action prime:
/// spend 1 sorcery point to install the `EmpoweredSpelling` condition,
/// which lets the next damaging spell (Fireball, Lightning Bolt,
/// Burning Hands, Magic Missile, Cone of Cold) reroll up to CHA-mod
/// dice that came up at 1 or 2. The reroll resolves through
/// `EncounterInstance::roll_empowered` at the damage chokepoint, so
/// future spells plug in by calling that helper instead of `roll`.
/// **Quickened Spell** (bonus action + 2 SP → extra Action this turn),
/// **Heightened Spell** (bonus action + 3 SP → next save-or-suck spell
/// forces disadvantage on the first save), **Careful Spell** (bonus
/// action + 1 SP → next AoE shields up to CHA-mod allies from the blast),
/// **Distant Spell** (bonus action + 1 SP → next ranged spell has its
/// reach doubled), and **Twinned Spell** (bonus action + max(1, spell_lvl)
/// SP → next single-target spell re-fires against a second valid target)
/// extend the metamagic lane.
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
    actions.push(&*crate::actions::spells::THUNDER_STEP);
    actions.push(&*crate::actions::spells::ABSORB_ELEMENTS);
    actions.push(&*crate::actions::spells::SHADOW_BLADE);
    actions.push(&*crate::actions::spells::SILVERY_BARBS);
    actions.push(&*crate::actions::spells::PROTECTION_FROM_ENERGY);
    actions.push(&*WITCH_BOLT);
    actions.push(&*CLOUD_OF_DAGGERS);
    // 5e Sorcerer Metamagic — Empowered Spell. Bonus-action prime that
    // burns 1 sorcery point to reroll dice that came up at 1 or 2 on
    // the next spell-damage roll (up to CHA-mod of them per RAW). The
    // engine reads the EmpoweredSpelling condition at the damage-roll
    // chokepoint (`EncounterInstance::roll_empowered`).
    actions.push(&crate::actions::metamagic::EMPOWERED_SPELL);
    // 5e Sorcerer Metamagic — Quickened Spell. Burns 2 sorcery points
    // + a Bonus Action to gain an extra Action this turn (modeling the
    // RAW "cast a 1-action spell as a bonus action" transform via the
    // simpler action-economy trade).
    actions.push(&crate::actions::metamagic::QUICKENED_SPELL);
    // 5e Sorcerer Metamagic — Heightened Spell. Burns 3 sorcery points
    // + a Bonus Action; the next save-or-suck spell forces disadvantage
    // on the first creature that rolls a save against it. Engine reads
    // the prime via `EncounterInstance::roll_save_against_caster`.
    actions.push(&crate::actions::metamagic::HEIGHTENED_SPELL);
    // 5e Sorcerer Metamagic — Careful Spell. Burns 1 sorcery point
    // + a Bonus Action; the next AoE auto-passes saves AND zeroes
    // damage on up to CHA-mod allies caught in the blast. Engine reads
    // the prime via `EncounterInstance::careful_spell_shielded`, which
    // the burst-save chokepoints consult to find protected ids.
    actions.push(&crate::actions::metamagic::CAREFUL_SPELL);
    // 5e Sorcerer Metamagic — Distant Spell. Burns 1 sorcery point
    // + a Bonus Action; the next ranged spell has its reach doubled.
    // Engine reads the prime via `ActorInstance::extra_spell_reach()`
    // which `Action::validate_input` folds into the effective range;
    // consumed in `Action::execute` on the first ranged action that fires.
    actions.push(&crate::actions::metamagic::DISTANT_SPELL);
    // 5e Sorcerer Metamagic — Twinned Spell. Burns max(1, spell_level) SP
    // (paid at the consume site, RAW timing) + a Bonus Action; the next
    // single-target spell fires a second time against a different valid
    // target. Engine reads the prime via
    // `EncounterInstance::consume_twinned_spell` in `Action::execute`,
    // which picks the second target (nearest enemy / lowest-HP ally) and
    // re-runs the action's `side_effects` against it.
    actions.push(&crate::actions::metamagic::TWINNED_SPELL);
    // 5e Sorcerer Metamagic — Extended Spell. Burns 1 sorcery point
    // + a Bonus Action; the next spell that installs a long-duration
    // condition (RAW: 1 minute or longer; we gate on Rounds(n) with
    // n >= 10) has its timer doubled. Engine reads the prime via
    // `EncounterInstance::consume_extended_spell` in `Action::execute`,
    // which walks the side_effects vec and doubles any eligible
    // `ApplyCondition` timer in place — consuming the prime only when
    // at least one timer was actually extended (short-duration buffs
    // and pure-damage spells leave the prime dangling for the next
    // eligible cast).
    actions.push(&crate::actions::metamagic::EXTENDED_SPELL);
    // 5e Tasha's Sorcerer Metamagic — Seeking Spell. Burns 2 sorcery
    // points + a Bonus Action; the next missed spell-attack roll is
    // rerolled and the new face is used (RAW: "you must use the new
    // roll"). Hooked into `spell_attack_outcome` via
    // `EncounterInstance::reroll_seeking_spell` so every spell-attack
    // path benefits without per-spell wiring.
    actions.push(&crate::actions::metamagic::SEEKING_SPELL);
    // 5e Sorcerer Metamagic — Subtle Spell. Burns 1 sorcery point
    // + a Bonus Action; the next spell ignores Counterspell (RAW: no
    // verbal or somatic components → the counterspeller has nothing
    // to react to). Engine reads via `Counterspell::custom_validate_input`
    // which fails-out when the targeted caster has `SubtleSpelling` up.
    // The prime ticks down on the sorcerer's next turn (UntilStartOfNextTurn)
    // so it covers the opponent's one reaction window between casts.
    actions.push(&crate::actions::metamagic::SUBTLE_SPELL);
    // 5e Tasha's Sorcerer Metamagic — Transmuted Spell. Burns 1 sorcery
    // point + a Bonus Action; the next spell whose damage type is one of
    // the six elemental types (acid / cold / fire / lightning / poison /
    // thunder) has its damage remapped to a different element from the
    // same list. Engine reads via `EncounterInstance::consume_transmuted_spell`
    // in `Action::execute`, which scans the cast's `DealDamage` entries,
    // picks the primary target's worst weakness among the six, and
    // remaps the damage type in place. Spells whose only damage is
    // non-elemental (force / radiant / necrotic / psychic / physical)
    // leave the prime up for the next eligible cast.
    actions.push(&crate::actions::metamagic::TRANSMUTED_SPELL);
    // 5e Wild Magic Sorcerer — **Tides of Chaos**. Once per long rest,
    // bonus action; install the `TidesOfChaos` prime → advantage on the
    // next attack roll (consumed via the `CONSUMED_ON_ATTACK` cohort).
    // Pairs naturally with a metamagic prime: the Tides advantage stacks
    // on the same swing the metamagic prime modifies, so a Tides +
    // Empowered + Fire Bolt combo gives both advantage *and* the damage
    // reroll. Gated on the per-long-rest feature charge.
    actions.push(&*crate::actions::class_features::TIDES_OF_CHAOS);
    // 5e Sorcerer **Font of Magic** — convert spell slots <-> sorcery
    // points. Bonus action in either direction; three slot levels each
    // way covers the typical mid-encounter resource shuffle. The "create
    // slot" lane recreates an expended low-level slot when SP is flush;
    // the "convert slot" lane refills SP when metamagic runs dry by
    // burning a held spell slot.
    actions.push(&crate::actions::class_features::CREATE_SPELL_SLOT_1);
    actions.push(&crate::actions::class_features::CREATE_SPELL_SLOT_2);
    actions.push(&crate::actions::class_features::CREATE_SPELL_SLOT_3);
    actions.push(&crate::actions::class_features::CONVERT_SPELL_SLOT_1);
    actions.push(&crate::actions::class_features::CONVERT_SPELL_SLOT_2);
    actions.push(&crate::actions::class_features::CONVERT_SPELL_SLOT_3);
    // Latest sorcerer additions:
    //   - lv5 **Wall of Stone**: 2-tile burst DEX save; failed-save
    //     enemies are Restrained for 10 rounds (concentration-anchored).
    //     CHA-anchored DC keeps the sorcerer's pick punchy at high tier.
    //   - lv6 **Investiture of Ice**: self-only concentration buff —
    //     cold resistance + 1d10 cold melee retaliation. Symmetric to
    //     Investiture of Flame; gives the sorcerer a cold-flavored
    //     defensive concentration option to pair with Cone of Cold /
    //     Snilloc's Snowball Swarm.
    actions.push(&*crate::actions::spells::WALL_OF_STONE);
    actions.push(&*crate::actions::spells::INVESTITURE_OF_ICE);
    // Tasha's Caustic Brew — lv1 evocation, sustained acid drip in a 30ft
    // line. Plays well off the sorcerer's CHA-anchored DC at the lv1 tier
    // and pairs cleanly with the Empowered Spell metamagic (the DoT
    // damage ticks won't reroll, but the initial 2d4 acid burst can).
    actions.push(&*crate::actions::spells::TASHAS_CAUSTIC_BREW);
    // Latest sorcerer additions (XGtE / PHB):
    //   - lv2 **Phantasmal Force** (illusion): INT save vs the sorcerer's
    //     CHA-based DC for a sustained 1d6 psychic / round DoT. Slots
    //     between Witch Bolt (lv1 concentration DoT) and the bigger
    //     concentration-bound DoTs at higher tiers.
    //   - lv5 **Wall of Light** (evocation): 4d8 radiant 2-tile burst
    //     CON save for half + Blinded-on-fail. Sorcerer's first multi-
    //     target Blinded lane at lv5 — pairs cleanly with the existing
    //     Wall of Stone Restrained-on-fail counterpart.
    //   - lv6 **Investiture of Wind** (transmutation): self-only
    //     concentration buff — ranged disadvantage to attackers + +60ft
    //     flying speed. Adds the Wind variant to the sorcerer's Flame
    //     / Ice investiture lane.
    actions.push(&*crate::actions::spells::PHANTASMAL_FORCE);
    actions.push(&*crate::actions::spells::WALL_OF_LIGHT);
    actions.push(&*crate::actions::spells::INVESTITURE_OF_WIND);
    // Sorcerer blasting additions (PHB / XGtE):
    //   - lv2 **Dust Devil** (conjuration): STR-save 1d8 bludgeoning
    //     1-tile burst + 4-tile push on fail. Displacement option that
    //     pairs cleanly with Thunderwave for double-push pressure.
    //   - lv5 **Maelstrom** (evocation): 3-tile burst, 6d6 bludgeoning
    //     STR-save half + pull-into-center on fail. The anti-Tidal-Wave
    //     — bunches enemies for a follow-up Empowered Cone of Cold.
    //   - lv6 **Otiluke's Freezing Sphere** (evocation): 6-tile burst,
    //     10d6 cold CON-save half. Cold-typed nuke that benefits from
    //     the sorcerer's Empowered Spell metamagic on the shared roll.
    actions.push(&*crate::actions::spells::DUST_DEVIL);
    actions.push(&*crate::actions::spells::MAELSTROM);
    actions.push(&*crate::actions::spells::OTILUKES_FREEZING_SPHERE);
    // Sorcerer capstone — lv9 **Psychic Scream** (enchantment, XGtE):
    // self-centered 8-tile burst, 14d6 psychic INT-save for half +
    // Stunned-on-fail. The flagship lv9 mind-spike for sorcerers that
    // pairs with Empowered Spell metamagic on the shared damage roll.
    actions.push(&*crate::actions::spells::PSYCHIC_SCREAM);
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
        creature_type: CreatureType::Humanoid,
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
        // 5e Wild Magic Sorcerer features: `Tides of Chaos` (once per
        // long rest, advantage on next attack) and `Sorcerous Restoration`
        // (lv20 capstone, regain 4 SP on short rest). The CR-4 template is
        // generous on `Sorcerous Restoration` vs strictly-RAW level
        // gating, but the +4 SP only kicks in on short rest — rare enough
        // that the partial refill rarely closes the gap to the long-rest
        // cap.
        features: HashSet::from([
            crate::actions::class_features::TIDES_OF_CHAOS_TAG,
            crate::actions::class_features::SORCEROUS_RESTORATION_TAG,
            // 5e Wild Magic Sorcerer **Wild Magic Surge** — passive: every
            // level-1+ spell cast rolls a d20; on a 1, a random surge
            // table effect fires. The trigger lives in
            // `EncounterInstance::trigger_wild_magic_surge` and is wired
            // through the cross-cutting `Action::execute` site. Pairs
            // naturally with Tides of Chaos: the RAW intent is for the
            // DM to force a surge after Tides is consumed; the 5% rate
            // is the closest stable approximation that keeps the surge
            // from dominating every cast.
            crate::actions::class_features::WILD_MAGIC_SURGE_TAG,
            // 5e Wild Magic Sorcerer **Bend Luck** (lv6): passive
            // reaction — when an enemy attack would hit, spend 2 SP +
            // reaction to subtract 1d4 from the attacker's roll. Wired
            // into both the weapon-attack and spell-attack resolvers via
            // `EncounterInstance::apply_bend_luck_penalty`. Doubles as
            // a defensive sink for the sorcerer's SP pool between
            // Empowered / Heightened / Twinned casts.
            crate::actions::class_features::BEND_LUCK_TAG,
        ]),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_deflect_missiles: false,
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
        // 5e Sorcerer Sorcery Points: 2 + level points RAW. We size to
        // 6 here (rough level-6 cap; the CR-4 template sits a bit above
        // strictly RAW levels). Enough to fuel several Empowered Spells
        // across an encounter without trivializing the resource budget.
        sorcery_points: 6,
    }
});
