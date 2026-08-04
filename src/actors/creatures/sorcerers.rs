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
use std::collections::HashSet;
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
    //   - lv2 **Enlarge / Reduce**, both halves. Enlarge props up an
    //     ally's martial output between the bigger-leverage Haste /
    //     Polymorph picks; Reduce points the same spell at an enemy
    //     bruiser and takes a die off every swing they land. Both
    //     concentration, so it is one choice, not two.
    actions.push(&*crate::actions::spells::ICE_KNIFE);
    actions.push(&*crate::actions::spells::ENLARGE_REDUCE);
    actions.push(&*crate::actions::spells::REDUCE);
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
    // the prime via `EncounterInstance::auto_pass_shielded_allies`, which
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
    // Latest sorcerer additions:
    //   - lv4 **Storm Sphere** (evocation, XGtE): 4-tile burst, 2d6
    //     bludgeoning STR-save (none-on-save) + `WindBlasted` rider
    //     for the duration. Pairs with Empowered Spell metamagic on
    //     the shared 2d6 roll — small damage but the rider blanks
    //     enemy bow / ranged-spell shots for the duration, which the
    //     sorcerer's CHA-anchored DC keeps potent.
    actions.push(&*crate::actions::spells::STORM_SPHERE);
    // lv3 **Wall of Water** (evocation, XGtE): 3-tile burst, no save /
    // no damage — every enemy in the burst picks up `WindWalled` for
    // the duration. Concentration-bound. The sorcerer's ranged-defense
    // companion to Wind Wall (lv3 self-only) and Wall of Sand (which
    // is wizard-only RAW). Pairs with Twinned Spell metamagic poorly
    // (multi-target), but Subtle Spell shines through it (Counterspell
    // can't catch a no-component wall).
    actions.push(&*crate::actions::spells::WALL_OF_WATER);
    // Mobility / utility additions (RAW sorcerer list):
    //   - lv1 **Expeditious Retreat**: bonus-action self-buff that grants
    //     +30 ft speed for 10 rounds, concentration. The sorcerer's clutch
    //     kiting tool that pairs with Misty Step / Dimension Door for the
    //     full repositioning trio.
    //   - lv2 **Earthbind** (XGtE): single-target STR-save ground; strips
    //     `Flying` / `InvestedInWind` on a failed save. Sorcerer's anti-
    //     air option that pairs nicely with Heightened Spell on a single
    //     flying boss.
    actions.push(&*crate::actions::spells::EXPEDITIOUS_RETREAT);
    actions.push(&*crate::actions::spells::EARTHBIND);
    // Latest sorcerer additions (PHB sorcerer list):
    //   - lv2 **Enhance Ability** (transmutation): touch ally buff — 2d6
    //     temp HP + flat +2 saves for the duration (concentration). Pairs
    //     with the sorcerer's Twinned Spell metamagic for a 2-for-1 buff
    //     spread across two allies.
    //   - lv3 **Blink** (transmutation): self-only `Displaced` install
    //     (10 rounds, no concentration). The sorcerer's defensive lane
    //     companion to Blur — Blink doesn't burn the concentration slot
    //     so the sorcerer can run Haste / Hold Monster / Slow on the
    //     same turn while the blink ward soaks attacker swings.
    actions.push(&*crate::actions::spells::ENHANCE_ABILITY);
    actions.push(&*crate::actions::spells::BLINK);
    // Latest sorcerer additions (XGtE / TCE):
    //   - lv2 **Pyrotechnics** (transmutation): cheap fire burst with
    //     Blinded-on-fail rider. Sorcerer-flavored fire option at lv2
    //     alongside Aganazzar's Scorcher / Snilloc's Snowball Swarm.
    //   - lv3 **Ashardalon's Stride** (transmutation): mobility +
    //     control combo at lv3 — +20 ft speed plus 1d6 fire trail to
    //     adjacent enemies on each step. Mirrors the wizard list.
    actions.push(&*crate::actions::spells::PYROTECHNICS);
    actions.push(&*crate::actions::spells::ASHARDALONS_STRIDE);
    // lv3 **Flame Arrows** (transmutation, XGtE): concentration self-buff
    // that grants +1d6 fire on every ranged spell-attack hit (ranged-only
    // via the OnHitRider table). The sorcerer gets it RAW; pairs with
    // the sorcerer's at-will damage cantrips (Fire Bolt / Ray of Frost /
    // Acid Splash) — though only ranged WEAPON attacks RAW, the engine's
    // unified OnHitRider table fires on any qualifying swing.
    actions.push(&*crate::actions::spells::FLAME_ARROWS);
    // lv6 **Tasha's Otherworldly Guise** (transmutation, TCE): top-tier
    // sorcerer self-buff. The full envelope (+2 AC, +60 ft fly, radiant/
    // poison resistance, Charmed/Frightened/Poisoned immunity, +2d6
    // radiant melee weapon rider) gives the sorcerer a legendary defense
    // alongside Tenser's Transformation / Globe of Invulnerability.
    actions.push(&*crate::actions::spells::OTHERWORLDLY_GUISE);
    // Latest sorcerer utility additions: lv2 Silence (anti-caster zone),
    // lv2 Darkness (concentration symmetric-blind zone), and lv4 Freedom
    // of Movement (ally-buff cleanse + restraint immunity).
    actions.push(&*crate::actions::spells::SILENCE);
    actions.push(&*crate::actions::spells::DARKNESS);
    actions.push(&*crate::actions::spells::FREEDOM_OF_MOVEMENT);
    // lv5 **Conjure Elemental** (conjuration): summon a single Large fire
    // elemental ally adjacent to the caster, concentration-bound. RAW
    // sorcerer / wizard / druid spell list — fills the sorcerer's lv5
    // summon slot alongside Cone of Cold (burst) and Hold Monster
    // (lockdown). Despawns via the shared `Conjured` cleanup path when
    // concentration drops.
    actions.push(&crate::actions::spells::CONJURE_ELEMENTAL);
    // lv5 **Summon Draconic Spirit** (FTD) — the sorcerer's one summon,
    // and RAW's own choice of which: the spell is on the sorcerer list
    // precisely because the class's draconic bloodline is the flavour it
    // was written for. Sits beside Conjure Elemental on the lv5 rung as
    // the variance option — a recharging area breath instead of a
    // reliable melee body.
    actions.push(&crate::actions::spells::SUMMON_DRACONIC_SPIRIT);
    // lv5 **Animate Objects** (transmutation): summon ten Tiny Construct
    // minions adjacent to the caster, concentration-bound. RAW sorcerer
    // spell list — pairs with the sorcerer's metamagic kit, since
    // Extended Spell doubles the swarm's duration on the same lv5 slot.
    actions.push(&*crate::actions::spells::ANIMATE_OBJECTS);
    // Latest sorcerer additions:
    //   - lv1 **Magnify Gravity** (evocation, TCE): 5ft burst, STR save
    //     for half + Slowed-1-round on fail. Pairs natively with the
    //     sorcerer's Heightened Spell metamagic (the first STR save in
    //     the burst rolls at disadvantage).
    //   - lv3 **Elemental Weapon** (transmutation, PHB): touch ally buff,
    //     +1 attack and +1d4 fire per melee hit (concentration). Slots
    //     between the sorcerer's lv2 Magic Weapon analog and the lv4
    //     enchantment / illusion lane.
    actions.push(&*crate::actions::spells::MAGNIFY_GRAVITY);
    actions.push(&*crate::actions::spells::ELEMENTAL_WEAPON);
    CreatureTemplate {
        name: "Sorcerer",
        // 'S' — distinct from Skeleton (lowercase 's'), Sage, etc.
        glyph: 'S',
        ac: 13, // unarmored, +DEX
        hitpoints: "9d6+18".parse().unwrap(),
        strength: 8,
        dexterity: 14,
        constitution: 14,
        intelligence: 11,
        wisdom: 12,
        charisma: 18, // primary spellcasting ability
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
        // Sorcerers are proficient in CON and CHA saves (5e PHB) —
        // distinct from the wizard's INT/WIS profile and the cleric's
        // WIS/CHA, which makes them tankier against the concentration-
        // breaking CON saves that hit casters in the heat of melee.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Charisma,
        ]),
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
        // 5e Sorcerer Sorcery Points: 2 + level points RAW. We size to
        // 6 here (rough level-6 cap; the CR-4 template sits a bit above
        // strictly RAW levels). Enough to fuel several Empowered Spells
        // across an encounter without trivializing the resource budget.
        sorcery_points: 6,
        ..CreatureTemplate::defaults()
    }
});

/// Draconic Sorcerer — Sorcerous Origin **Draconic Bloodline** subclass
/// build (PHB). Identical envelope to the baseline `SORCERER_TEMPLATE`
/// (CHA-primary level-9 full-caster, Empowered / Quickened / Heightened
/// / Twinned / Careful / Distant / Extended / Seeking / Subtle /
/// Transmuted metamagic, 6 sorcery points, Wild Magic Surge / Bend Luck
/// / Tides of Chaos / Sorcerous Restoration) with one bloodline feature
/// layered on: **Draconic Resilience** (Draconic Bloodline lv6) —
/// passive fire-damage resistance from the Red / Gold / Brass ancestor
/// pick.
///
/// Pairs naturally with the sorcerer's existing fire-heavy blast list
/// (Burning Hands / Scorching Ray / Fireball / Wall of Fire / Wall of
/// Light / Pyrotechnics / Flame Arrows on the Fire lane) — a Draconic
/// Sorcerer casting Wall of Fire into their own tile no longer eats a
/// mirror share of their own damage; the resistance folder halves the
/// self-hit alongside the target-side spread. Distinct from the Wild
/// Magic baseline (`SORCERER_TEMPLATE`) on the "Wild Magic Surge /
/// Tides of Chaos / Bend Luck" tell: the Draconic Sorcerer trades the
/// per-cast d20 surge risk for a persistent defensive lane.
///
/// RAW's Draconic Bloodline picks up other features not shipped on this
/// template — **Draconic Ancestry** (lv1: language / dragon-ancestor
/// pick, no combat surface), **Draconic Resilience HP boost** (lv1: +1
/// HP per sorcerer level, folded into the template's `hitpoints` field
/// baseline rather than a separate flag), **Elemental Affinity** (lv6:
/// +CHA damage on chosen-type spells; a "typed damage bonus" surface
/// the engine doesn't yet track per-damage-type), **Dragon Wings**
/// (lv14: bonus-action flight, no combat surface without 3D terrain),
/// and **Draconic Presence** (lv18 capstone: CD Frighten aura, a
/// Sorcery-Points-fueled ally-radius Frighten install). Only the
/// bloodline's lv6 damage-resistance clause has a mechanical surface
/// on the CR-4 chassis without a spend-side hook, so we ship that half
/// and leave the rest as future work.
///
/// Ships the CR-4 template above the strict RAW lv6 gate for the same
/// reason `FIEND_WARLOCK_TEMPLATE` ships Fiendish Resilience (RAW lv10)
/// on its CR-4 chassis — class templates target a balanced playable
/// level, not lockstep PHB progression. Glyph 'D' so the Draconic
/// Sorcerer shows up distinctly on the map next to the baseline Wild
/// Magic Sorcerer 'S'.
pub static DRACONIC_SORCERER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Sorcerer envelope
    // wholesale and layer on the Draconic Bloodline features:
    //   - `has_draconic_resilience` (lv6): passive fire-damage resistance.
    //     Template-flag lane, no charge. Read at the shared
    //     `PASSIVE_TYPED_RESISTANCES` cohort in `effective_damage` next
    //     to Dwarven Resilience / Fiendish Resilience — same lane,
    //     different class chassis.
    //   - `ELEMENTAL_AFFINITY_TAG` (lv6, damage half): passive +CHA on
    //     one damage roll of any spell that deals fire damage. A row on
    //     the shared `FLAT_SPELL_DAMAGE_BONUSES` cohort next to the
    //     Evocation Wizard's Empowered Evocation, and the sorcerer's
    //     answer to it — where the wizard's covers a whole school at
    //     every tier, this covers one damage type across every school.
    //
    // The two lv6 halves point at the same element on purpose. Draconic
    // Resilience says "you resist fire"; Elemental Affinity says "your
    // fire hits harder"; RAW derives both from the same ancestry, and
    // shipping them on different elements would have been two half-
    // features rather than one. On a CHA-18 chassis that is +4 on every
    // Fireball, Burning Hands, Scorching Ray and Fire Bolt the sorcerer
    // throws — and nothing at all on their Lightning Bolt, which is the
    // choice the feature exists to pose.
    //
    // The `..base.clone()` tail picks up every other field — actions,
    // spell slots, sorcery points, save profs, features — without an
    // N-line field-by-field copy. Same shape as `FIEND_WARLOCK_TEMPLATE`
    // and the paladin / rogue / ranger subclass templates.
    let mut features = SORCERER_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::ELEMENTAL_AFFINITY_TAG);
    CreatureTemplate {
        name: "Draconic Sorcerer",
        glyph: 'D',
        has_draconic_resilience: true,
        features,
        ..SORCERER_TEMPLATE.clone()
    }
});

/// Storm Sorcerer — Sorcerous Origin **Storm Sorcery** subclass build
/// (XGtE). Identical envelope to the baseline `SORCERER_TEMPLATE`
/// (CHA-primary level-9 full-caster, Empowered / Quickened / Heightened
/// / Twinned / Careful / Distant / Extended / Seeking / Subtle /
/// Transmuted metamagic, 6 sorcery points, Wild Magic Surge / Bend Luck
/// / Tides of Chaos / Sorcerous Restoration) with one bloodline feature
/// layered on: **Heart of the Storm** (Storm Sorcery lv6) — passive
/// lightning + thunder damage resistance.
///
/// Pairs naturally with the sorcerer's existing lightning / thunder
/// blast list (Chain Lightning / Lightning Bolt / Thunderclap / Thunder
/// Step / Shatter / Snilloc's Snowball Swarm's cold-analogue lane) — a
/// Storm Sorcerer casting Lightning Bolt into their own tile no longer
/// eats a mirror share of their own damage; the resistance folder
/// halves the self-hit alongside the target-side spread. Distinct from
/// the Wild Magic baseline (`SORCERER_TEMPLATE`) on the "Wild Magic
/// Surge / Tides of Chaos / Bend Luck" tell: the Storm Sorcerer keeps
/// those but adds a persistent defensive lane on top. Distinct from
/// `DRACONIC_SORCERER_TEMPLATE` on the resistance axis: Draconic covers
/// Fire, Storm covers Lightning + Thunder — a hypothetical multi-class
/// carrier stacks both flag closures cleanly under the "one halving per
/// damage instance" rule (no double-halving on shared types since the
/// two rows never overlap on a single damage type).
///
/// RAW's Storm Sorcery picks up other features not shipped on this
/// template — **Tempestuous Magic** (lv1: 10ft bonus-action fly after
/// casting a lv1+ spell — a per-cast repositioning hook that needs a
/// trigger wire), **Storm Guide** (lv1: weather control, no combat
/// surface), **Storm's Fury** (lv14: reaction retaliation burst), and
/// **Wind Soul** (lv18 capstone: full lightning / thunder immunity +
/// fly speed). Shipped on the CR-4 chassis:
///   - **Wind Speaker** (lv1): the Primordial language addition on
///     top of Common. Layered on via `languages.insert(Primordial)`
///     rather than a struct-field flag — no combat surface, dialog-
///     gate only.
///   - **Heart of the Storm** (lv6, both halves): passive lightning +
///     thunder resistance via the `PASSIVE_TYPED_RESISTANCES` cohort
///     AND the eruption clause (a 10ft radius post-cast burst on
///     lv1+ lightning / thunder casts, via the `Action::execute`
///     chokepoint next to Wild Magic Surge).
///
/// The remaining features are future work.
///
/// Ships the CR-4 template above the strict RAW lv6 gate for the same
/// reason `DRACONIC_SORCERER_TEMPLATE` ships Draconic Resilience
/// (RAW lv6) on its CR-4 chassis — class templates target a balanced
/// playable level, not lockstep PHB progression. Glyph 'Ω' so the
/// Storm Sorcerer shows up distinctly on the map next to the baseline
/// Wild Magic Sorcerer 'S' and the Draconic Sorcerer 'D' — greek
/// omega for the storm's-fury flavor.
pub static STORM_SORCERER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern: clone the baseline Sorcerer envelope
    // wholesale and layer on the Storm Sorcery lv6 feature tag:
    //   - `HEART_OF_THE_STORM_TAG` (lv6): passive lightning + thunder
    //     resistance. Tag-based rather than a struct-field flag since
    //     the shared `PASSIVE_TYPED_RESISTANCES` cohort's slice-of-types
    //     shape folds both damage types through one row — no need for
    //     a new `has_heart_of_the_storm: bool` struct field. Read at
    //     the shared `PASSIVE_TYPED_RESISTANCES` cohort in
    //     `effective_damage` next to Dwarven / Fiendish / Draconic
    //     Resilience — same lane, different subclass chassis and
    //     different damage axis (Lightning + Thunder vs. Poison / Fire).
    //
    // The `..base.clone()` tail picks up every other field — actions,
    // spell slots, sorcery points, save profs, features — without an
    // N-line field-by-field copy. Same shape as
    // `UNDYING_WARLOCK_TEMPLATE`'s tag-only subclass build.
    let mut features = SORCERER_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::HEART_OF_THE_STORM_TAG);
    // 5e Storm Sorcery **Wind Speaker** (lv1): the sorcerer's storm-
    // attuned lineage grants them the Primordial tongue (Aquan / Auran
    // / Ignan / Terran dialects share one language slot in the engine).
    // Layered onto the baseline sorcerer's Common set so the Storm
    // Sorcerer can parley with elementals from the same Investiture
    // spell list they cast — thematic pairing, no combat surface
    // beyond dialog gating.
    let mut languages = SORCERER_TEMPLATE.languages.clone();
    languages.insert(Language::Primordial);
    CreatureTemplate {
        name: "Storm Sorcerer",
        // 'Ω' — distinct from baseline sorcerer 'S' and Draconic 'D',
        // greek omega for the storm's-fury / thunder-omega flavor.
        glyph: 'Ω',
        features,
        languages,
        ..SORCERER_TEMPLATE.clone()
    }
});

/// Aberrant Mind Sorcerer — Sorcerous Origin **Aberrant Mind** subclass
/// build (TCE). Identical envelope to the baseline `SORCERER_TEMPLATE`
/// (CHA-primary level-9 full-caster, Empowered / Quickened / Heightened
/// / Twinned / Careful / Distant / Extended / Seeking / Subtle /
/// Transmuted metamagic, 6 sorcery points, Wild Magic Surge / Bend Luck
/// / Tides of Chaos / Sorcerous Restoration) with one subclass feature
/// layered on: **Psychic Defenses** (Aberrant Mind lv14) — passive
/// psychic damage resistance AND passive Charmed / Frightened install
/// immunity.
///
/// Pairs naturally with the sorcerer's existing psychic-blast list
/// (Mind Spike / Psychic Lance / Psychic Scream / Weird on the psychic
/// lane) — an Aberrant Mind casting Psychic Scream into their own tile
/// no longer eats a mirror share of their own damage; the resistance
/// folder halves the self-hit alongside the target-side spread. The
/// Charmed / Frightened install immunity locks the aberrant sorcerer
/// out of most enchantment / fear presses — Charm Person, Fear, Hold
/// Person, and the various Frightened-installer bursts (Dreadful
/// Aspect, Wrath of the Storm rider) all bounce off the immunity
/// cohort's `FLAG_DRIVEN_IMMUNITIES` row.
///
/// Distinct from the Wild Magic baseline (`SORCERER_TEMPLATE`) on the
/// "Wild Magic Surge / Tides of Chaos / Bend Luck" tell: the Aberrant
/// Mind Sorcerer keeps those but adds a persistent defensive lane on
/// top. Distinct from `DRACONIC_SORCERER_TEMPLATE` on the resistance
/// axis (Psychic vs. Fire) and from `STORM_SORCERER_TEMPLATE` on the
/// resistance axis (Psychic vs. Lightning + Thunder) — a hypothetical
/// multi-Origin carrier stacks all three passive-resistance flag
/// closures cleanly under the "one halving per damage instance" rule
/// since the three subclass-picked resistance sets never overlap on a
/// single damage type. Distinct from Nature's Ward (Ancients Paladin
/// lv15) on the same Charmed + Frightened immunity lane: same
/// suppressed conditions, different chassis (Sorcerer subclass vs.
/// Paladin subclass) and different source flag — either row alone
/// suffices, both together are redundant (a hypothetical Ancients
/// Paladin / Aberrant Mind Sorcerer multi-class stacks the two rows
/// cleanly under the OR-of-cohort-hits semantics `dynamic_immunity_to`
/// already honors).
///
/// RAW's Aberrant Mind picks up other features not shipped on this
/// template — **Telepathic Speech** (lv1: 30ft telepathy, no combat
/// surface), **Psionic Spells** (lv1: subclass-only expanded spell
/// list, currently folded into the shared sorcerer roster since the
/// Mind Sliver / Mind Spike / Detect Thoughts / Calm Emotions / etc.
/// entries are already available on the baseline chassis via the
/// shared spells module), **Psionic Sorcery** (lv6: cast Psionic
/// Spells for reduced-cost SP with no material / verbal / somatic
/// components — needs a per-spell prime gate not yet wired), and
/// **Warping Implosion** (lv18 capstone: teleport + force burst — an
/// SP-fueled apex burst). Only the lv14 Psychic Defenses passive has
/// a mechanical surface on the CR-4 chassis without a spend-side
/// hook, so we ship that half and leave the rest as future work.
///
/// Ships the CR-4 template above the strict RAW lv14 gate for the
/// same reason `DRACONIC_SORCERER_TEMPLATE` ships Draconic Resilience
/// (RAW lv6) and `STORM_SORCERER_TEMPLATE` ships Heart of the Storm
/// (RAW lv6) on their CR-4 chassis — class templates target a balanced
/// playable level, not lockstep PHB progression. Glyph 'Ψ' so the
/// Aberrant Mind Sorcerer shows up distinctly on the map next to the
/// baseline Wild Magic Sorcerer 'S', the Draconic Sorcerer 'D', and
/// the Storm Sorcerer 'Ω' — greek psi for the psionic / psychic
/// flavor.
pub static ABERRANT_MIND_SORCERER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Sorcerer envelope
    // wholesale and layers on the Aberrant Mind lv14 feature tag
    // (`PSYCHIC_DEFENSES_TAG`, passive psychic resistance + Charmed /
    // Frightened install immunity — read at `PASSIVE_TYPED_RESISTANCES`
    // and `FLAG_DRIVEN_IMMUNITIES` respectively). The `..base.clone()`
    // tail inside the helper picks up every other field — actions, spell
    // slots, sorcery points, save profs, features — without an N-line
    // field-by-field copy. Sibling helper users on the "clone base +
    // insert one tag" cross-class lane: every tag-only Warlock
    // Otherworldly Patron subclass (via `subclass_warlock_template`),
    // `LIFE_CLERIC_TEMPLATE`, `NECROMANCY_WIZARD_TEMPLATE`,
    // `DIVINE_SOUL_SORCERER_TEMPLATE`. STORM_SORCERER_TEMPLATE inserts
    // one tag AND adds a language (Primordial) so it stays on the
    // explicit clone-and-insert body — the helper's tag-only interface
    // can't express the language axis. Glyph 'Ψ' (greek psi) — distinct
    // from baseline sorcerer 'S', Draconic 'D', and Storm 'Ω', for the
    // psionic / psychic flavor.
    SORCERER_TEMPLATE.with_subclass_tag(
        "Aberrant Mind Sorcerer",
        'Ψ',
        crate::actions::class_features::PSYCHIC_DEFENSES_TAG,
    )
});

/// Divine Soul Sorcerer — Sorcerous Origin **Divine Soul** subclass
/// build (XGtE). Identical envelope to the baseline `SORCERER_TEMPLATE`
/// (CHA-primary level-9 full-caster, Empowered / Quickened / Heightened
/// / Twinned / Careful / Distant / Extended / Seeking / Subtle /
/// Transmuted metamagic, 6 sorcery points, Wild Magic Surge / Bend Luck
/// / Tides of Chaos / Sorcerous Restoration) with one subclass feature
/// layered on: **Favored by the Gods** (Divine Soul lv1) — auto-fire
/// once-per-short-rest "add 2d4 to a failed save total" gate.
///
/// Sibling to `FIEND_WARLOCK_TEMPLATE` on the failed-save recovery
/// axis: both templates ship an add-die cohort entry (DOOL +1d10 for
/// the Fiend Warlock, FBTG +2d4 for the Divine Soul Sorcerer). The
/// two never legally co-occur on a single build since the sorcerer
/// picks one Sorcerous Origin — but a hypothetical Fiend Warlock /
/// Divine Soul Sorcerer multi-class stacks the two cohort rows
/// cleanly under the shared `FAILED_SAVE_ADD_DIE_SOURCES` "at most
/// one add-die per save" semantics: the first-listed source (DOOL)
/// fires first, and only if its charge is spent AND its boosted
/// total still fails does the second source (FBTG) get a shot on a
/// later save (both refresh on short rest, not per-save).
///
/// Distinct from the Wild Magic baseline (`SORCERER_TEMPLATE`) on
/// the "Wild Magic Surge / Tides of Chaos / Bend Luck" tell: the
/// Divine Soul Sorcerer keeps those but adds a persistent
/// failed-save safety net on top. Distinct from
/// `DRACONIC_SORCERER_TEMPLATE` (fire resistance),
/// `STORM_SORCERER_TEMPLATE` (lightning + thunder resistance +
/// eruption), and `ABERRANT_MIND_SORCERER_TEMPLATE` (psychic
/// resistance + Charmed / Frightened immunity) — none of the three
/// prior subclasses touch the failed-save recovery lane, so a
/// hypothetical multi-Origin carrier stacks Divine Soul's FBTG
/// cleanly beside any of the others without overlap.
///
/// RAW's Divine Soul picks up other features not shipped on this
/// template — **Divine Magic** (lv1: expanded spell list drawing
/// from the Cleric list — Bless / Cure Wounds / Guiding Bolt /
/// Spiritual Weapon / etc.; RAW's cleric-spell access is currently
/// approximated by the sorcerer's existing roster), **Empowered
/// Healing** (lv6: spend 1 SP to reroll healing dice; healing
/// dice-reroll surface not yet wired), **Otherworldly Wings**
/// (lv14: bonus-action flight, no combat surface without 3D
/// terrain), and **Unearthly Recovery** (lv18 capstone: bonus
/// action heal for half max HP; a per-rest heal well). Only the
/// lv1 Favored by the Gods passive has a mechanical surface on the
/// CR-4 chassis that plugs cleanly into the shared save-recovery
/// cohort, so we ship that half and leave the rest as future work.
///
/// Glyph 'V' so the Divine Soul Sorcerer shows up distinctly on
/// the map next to the baseline Wild Magic Sorcerer 'S', the
/// Draconic Sorcerer 'D', the Storm Sorcerer 'Ω', and the
/// Aberrant Mind Sorcerer 'Ψ' — 'V' for "divine / vessel" flavor
/// (the sorcerer as a vessel for divine power).
pub static DIVINE_SOUL_SORCERER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Sorcerer envelope
    // wholesale and layers on the Divine Soul lv1 feature tag
    // (`FAVORED_BY_THE_GODS_TAG`: once-per-short-rest auto-fire "add 2d4
    // to a failed save total" gate; ships in `SHORT_REST_FEATURES` so the
    // charge refreshes alongside Dark One's Own Luck / Fanatical Focus /
    // etc. on the failed-save recovery lane; fired at the shared save
    // chokepoint via the `FAILED_SAVE_ADD_DIE_SOURCES` cohort next to
    // Dark One's Own Luck). The `..base.clone()` tail inside the helper
    // picks up every other field — actions, spell slots, sorcery points,
    // save profs, features — without an N-line field-by-field copy.
    // Sibling helper users on the "clone base + insert one tag" cross-
    // class lane: every tag-only Warlock Otherworldly Patron subclass
    // (via `subclass_warlock_template`), `LIFE_CLERIC_TEMPLATE`,
    // `NECROMANCY_WIZARD_TEMPLATE`, `ABERRANT_MIND_SORCERER_TEMPLATE`.
    // Glyph 'V' — distinct from baseline sorcerer 'S', Draconic 'D',
    // Storm 'Ω', and Aberrant Mind 'Ψ'; 'V' for the "divine vessel"
    // flavor (the sorcerer as a vessel for divine power).
    SORCERER_TEMPLATE.with_subclass_tag(
        "Divine Soul Sorcerer",
        'V',
        crate::actions::class_features::FAVORED_BY_THE_GODS_TAG,
    )
});

/// Shadow Magic Sorcerer — Sorcerous Origin **Shadow Magic** subclass
/// build (XGtE). Identical envelope to the baseline `SORCERER_TEMPLATE`
/// (CHA-primary level-9 full-caster, Empowered / Quickened / Heightened
/// / Twinned / Careful / Distant / Extended / Seeking / Subtle /
/// Transmuted metamagic, 6 sorcery points, Wild Magic Surge / Bend Luck
/// / Tides of Chaos / Sorcerous Restoration) with one subclass feature
/// layered on: **Strength of the Grave** (Shadow Magic lv1) — passive
/// once-per-long-rest "drop to 1 HP instead of 0" cheat-death gate.
///
/// The signature "the shadow-touched sorcerer refuses to fall to the
/// grave" tell — where a baseline Sorcerer eats a killing blow and
/// enters the Dying state (or dies outright if not death-save-eligible),
/// the Shadow Magic Sorcerer pins their HP at 1 and stays on their
/// feet for one more round while the once-per-rest charge holds.
///
/// Mechanically identical to Half-Orc Relentless Endurance (racial
/// trait, same "drop to 1 HP" mechanic) and Ancients Paladin Undying
/// Sentinel (subclass lv15, same mechanic). All three route through
/// the shared `LETHAL_DAMAGE_ABSORBER_FEATURES` cohort in
/// `take_typed_damage` so a hypothetical multi-source carrier
/// (a half-orc Shadow Sorcerer, or a multi-class Shadow Sorcerer /
/// Ancients Paladin) spends the tags in cohort order — Relentless
/// Endurance first, then Undying Sentinel, then Strength of the Grave
/// — rather than double-dipping on the same lethal hit. RAW's XGtE
/// save-vs-DC gate (DC 5 + damage taken CHA save, fails on radiant
/// damage or a crit killing blow) collapses to a guaranteed proc for
/// uniformity with the sibling cohort entries — see the
/// `STRENGTH_OF_THE_GRAVE_TAG` docstring for the rationale.
///
/// Distinct from the Wild Magic baseline (`SORCERER_TEMPLATE`) on
/// the "Wild Magic Surge / Tides of Chaos / Bend Luck" tell: the
/// Shadow Magic Sorcerer keeps those but adds a persistent
/// cheat-death safety net on top. Distinct from
/// `DRACONIC_SORCERER_TEMPLATE` (fire resistance),
/// `STORM_SORCERER_TEMPLATE` (lightning + thunder resistance +
/// eruption), `ABERRANT_MIND_SORCERER_TEMPLATE` (psychic resistance +
/// Charmed / Frightened immunity), and `DIVINE_SOUL_SORCERER_TEMPLATE`
/// (failed-save add-die recovery) — none of the four prior subclasses
/// touch the lethal-damage-absorber lane, so a hypothetical multi-
/// Origin carrier stacks Shadow Magic's Strength of the Grave cleanly
/// beside any of the others without overlap.
///
/// RAW's Shadow Magic picks up other features not shipped on this
/// template — **Eyes of the Dark** (lv1: 120ft darkvision +
/// Darkness spell that pierces the caster's own vision; no combat
/// surface without a light-level model), **Hound of Ill Omen** (lv6:
/// spend 3 SP to summon a Dire Wolf variant that dogs a target;
/// needs an ally-summon action surface and a distinct summon-scale
/// heuristic), **Shadow Walk** (lv14: teleport between dim / dark
/// tiles; needs a dim/dark-terrain model), and **Umbral Form** (lv18
/// capstone: bonus action, 6 SP for resistance to all damage except
/// force/radiant + move-through-creatures; needs a broad-resistance
/// spend-side hook). Only the lv1 Strength of the Grave passive has
/// a mechanical surface on the CR-4 chassis that plugs cleanly into
/// the shared lethal-damage-absorber cohort, so we ship that half
/// and leave the rest as future work — matching the way
/// `NECROMANCY_WIZARD_TEMPLATE` ships only the lv10 Inured to
/// Undeath passive half of its RAW School of Necromancy kit.
///
/// Ships on the CR-4 sorcerer chassis at (or above) its strict RAW
/// lv1 gate for the same reason `DIVINE_SOUL_SORCERER_TEMPLATE`
/// ships Favored by the Gods (RAW lv1) — class templates target a
/// balanced playable level, not lockstep PHB progression. Glyph 'H'
/// so the Shadow Magic Sorcerer shows up distinctly on the map next
/// to the baseline Wild Magic Sorcerer 'S', the Draconic Sorcerer
/// 'D', the Storm Sorcerer 'Ω', the Aberrant Mind Sorcerer 'Ψ', and
/// the Divine Soul Sorcerer 'V' — 'H' for the "sHadow" identity
/// (the sorcerer's Shadowfell-linked lineage manifesting as the
/// pallor of the grave). Collides with the Storm Herald Barbarian's
/// 'H' glyph, but the two never legally co-occur on a single team
/// (a Shadow Sorcerer isn't a Storm Herald Barbarian, and one glyph
/// per team-color-and-team-id combo suffices to disambiguate them
/// in a mixed encounter).
pub static SHADOW_MAGIC_SORCERER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Sorcerer envelope
    // wholesale and layers on the Shadow Magic lv1 feature tag
    // (`STRENGTH_OF_THE_GRAVE_TAG`: once-per-long-rest "drop to 1 HP
    // instead of 0" cheat-death gate; NOT registered in
    // `SHORT_REST_FEATURES` — RAW gates on a long rest per XGtE text,
    // matching the sibling Relentless Endurance / Undying Sentinel
    // entries on the same lane; fired at the shared take-damage
    // chokepoint via the `LETHAL_DAMAGE_ABSORBER_FEATURES` cohort next
    // to Relentless Endurance and Undying Sentinel). The
    // `..base.clone()` tail inside the helper picks up every other
    // field — actions, spell slots, sorcery points, save profs,
    // features — without an N-line field-by-field copy. Sibling
    // helper users on the "clone base + insert one tag" cross-class
    // lane: every tag-only Warlock Otherworldly Patron subclass (via
    // `subclass_warlock_template`), `LIFE_CLERIC_TEMPLATE`,
    // `NECROMANCY_WIZARD_TEMPLATE`, `ABERRANT_MIND_SORCERER_TEMPLATE`,
    // `DIVINE_SOUL_SORCERER_TEMPLATE`. Glyph 'H' — distinct from
    // baseline sorcerer 'S', Draconic 'D', Storm 'Ω', Aberrant Mind
    // 'Ψ', and Divine Soul 'V'; 'H' for the "sHadow" identity.
    SORCERER_TEMPLATE.with_subclass_tag(
        "Shadow Magic Sorcerer",
        'H',
        crate::actions::class_features::STRENGTH_OF_THE_GRAVE_TAG,
    )
});
