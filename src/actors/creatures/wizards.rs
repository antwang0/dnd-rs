use crate::actions::class_features::{ARCANE_RECOVERY, ARCANE_RECOVERY_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{
    ACID_SPLASH, BANISHMENT, BESTOW_CURSE, BLINDNESS, BLUR, BOOMING_BLADE, BURNING_HANDS,
    CAUSE_FEAR, CHAIN_LIGHTNING, CHARM_PERSON, CHILL_TOUCH, CLOUDKILL, CLOUD_OF_DAGGERS,
    COLOR_SPRAY, CONE_OF_COLD, CONFUSION, COUNTERSPELL, CROWN_OF_MADNESS, CROWN_OF_STARS,
    DIMENSION_DOOR, DISINTEGRATE, DISPEL_MAGIC, DOMINATE_PERSON, EARTHQUAKE, FEAR, FEEBLEMIND,
    FINGER_OF_DEATH, FIREBALL, FIRE_BOLT, FIRE_SHIELD, FLAME_STRIKE, FLY, FORCECAGE,
    GLOBE_OF_INVULNERABILITY, GREATER_INVISIBILITY, HASTE, HEAT_METAL, HOLD_MONSTER,
    HYPNOTIC_PATTERN, ICE_STORM, INVISIBILITY, LEVITATE, LIGHTNING_BOLT, LIGHTNING_LURE,
    MAGE_ARMOR, MAGIC_MISSILE,
    MAGIC_WEAPON, MASS_SUGGESTION, METEOR_SWARM, MIND_SLIVER, MIND_WHIP, MIRROR_IMAGE, MISTY_STEP,
    PHANTASMAL_KILLER, PLANT_GROWTH, POISON_SPRAY, POLYMORPH, POWER_WORD_KILL, POWER_WORD_STUN,
    PRISMATIC_SPRAY, RAY_OF_FROST,
    RAY_OF_SICKNESS, SCORCHING_RAY, SHATTER,
    SHIELD, SHOCKING_GRASP, SLEEP, SLOW, SPIDER_CLIMB, SPIKE_GROWTH,
    STINKING_CLOUD, STONESKIN, SUGGESTION, SUNBEAM, SYNAPTIC_STATIC, TASHAS_HIDEOUS_LAUGHTER,
    TELEKINESIS, THUNDERWAVE, TIME_STOP, TOLL_THE_DEAD, TRUE_STRIKE, VAMPIRIC_TOUCH, WALL_OF_FIRE,
    WALL_OF_FORCE, WEB, WISH, WITCH_BOLT,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Squishy INT-caster. Fire Bolt as the at-will ranged option, Magic
/// Missile and Burning Hands as level-1 nuke / AoE. Stat shape mirrors
/// the cleric (low HP, medium AC, tunes around ranged spell attacks)
/// but uses INT as the spellcasting ability so a separate save DC and
/// attack mod come into play.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*RAY_OF_FROST);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*THUNDERWAVE);
    actions.push(&*BURNING_HANDS);
    actions.push(&*CAUSE_FEAR);
    actions.push(&*WEB);
    actions.push(&*BLINDNESS);
    actions.push(&*MISTY_STEP);
    actions.push(&*SHIELD);
    actions.push(&*ACID_SPLASH);
    actions.push(&*CHILL_TOUCH);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*POISON_SPRAY);
    actions.push(&*RAY_OF_SICKNESS);
    actions.push(&*SHOCKING_GRASP);
    actions.push(&*LIGHTNING_LURE);
    actions.push(&*TOLL_THE_DEAD);
    actions.push(&*SHATTER);
    actions.push(&*SLEEP);
    actions.push(&*CHARM_PERSON);
    actions.push(&*MIRROR_IMAGE);
    actions.push(&*COLOR_SPRAY);
    actions.push(&*FIREBALL);
    actions.push(&*MAGIC_WEAPON);
    actions.push(&*SCORCHING_RAY);
    actions.push(&*LIGHTNING_BOLT);
    actions.push(&*VAMPIRIC_TOUCH);
    actions.push(&*HYPNOTIC_PATTERN);
    actions.push(&*BLUR);
    actions.push(&*INVISIBILITY);
    actions.push(&*BESTOW_CURSE);
    actions.push(&*MIND_SLIVER);
    actions.push(&*HOLD_MONSTER);
    actions.push(&*HASTE);
    actions.push(&*SLOW);
    actions.push(&*CONE_OF_COLD);
    actions.push(&*STINKING_CLOUD);
    actions.push(&*TRUE_STRIKE);
    actions.push(&*DISPEL_MAGIC);
    actions.push(&*GREATER_INVISIBILITY);
    actions.push(&*ICE_STORM);
    actions.push(&*WITCH_BOLT);
    actions.push(&*TASHAS_HIDEOUS_LAUGHTER);
    actions.push(&*CLOUD_OF_DAGGERS);
    actions.push(&*CROWN_OF_MADNESS);
    actions.push(&*PHANTASMAL_KILLER);
    actions.push(&*BANISHMENT);
    actions.push(&*STONESKIN);
    actions.push(&*SYNAPTIC_STATIC);
    actions.push(&*DISINTEGRATE);
    actions.push(&*FINGER_OF_DEATH);
    actions.push(&*POWER_WORD_STUN);
    actions.push(&*SUGGESTION);
    actions.push(&*MASS_SUGGESTION);
    actions.push(&*POWER_WORD_KILL);
    actions.push(&*METEOR_SWARM);
    actions.push(&*SUNBEAM);
    // Newly added wizard spells (lv4 Dimension Door / Wall of Fire / Fire
    // Shield; lv5 Cloudkill / Wall of Force).
    actions.push(&*DIMENSION_DOOR);
    actions.push(&*WALL_OF_FIRE);
    actions.push(&*FIRE_SHIELD);
    actions.push(&*CLOUDKILL);
    actions.push(&*WALL_OF_FORCE);
    // Latest additions: lv2 Spike Growth (control), lv3 Counterspell
    // (anti-caster), lv4 Polymorph (transformation buff), lv5
    // Telekinesis (forced movement), lv6 Globe of Invulnerability
    // (mass damage reduction).
    actions.push(&*SPIKE_GROWTH);
    actions.push(&*COUNTERSPELL);
    actions.push(&*POLYMORPH);
    actions.push(&*TELEKINESIS);
    actions.push(&*GLOBE_OF_INVULNERABILITY);
    // Latest spell additions: cantrip Booming Blade (melee thunder rider
    // on movement), lv2 Mind Whip (INT save psychic + action-economy
    // debuff), lv7 Forcecage (CHA save imprisonment) + Crown of Stars
    // (radiant per-hit rider self-buff), lv8 Earthquake (STR save AoE
    // bludgeoning + prone), lv9 Time Stop (extra action / bonus action),
    // lv9 Wish (mass-heal allies).
    actions.push(&*BOOMING_BLADE);
    actions.push(&*MIND_WHIP);
    actions.push(&*FORCECAGE);
    actions.push(&*CROWN_OF_STARS);
    actions.push(&*EARTHQUAKE);
    actions.push(&*TIME_STOP);
    actions.push(&*WISH);
    // Latest arcane addition: lv3 Fear (cone WIS save → Frightened),
    // a clean illusion-control option missing from the wizard list.
    actions.push(&*FEAR);
    // Latest evocation / transmutation additions: lv2 Heat Metal (con
    // DoT + attack disadvantage), lv5 Flame Strike (mixed fire+radiant
    // AoE), lv6 Chain Lightning (forks to 3 nearby creatures).
    actions.push(&*HEAT_METAL);
    actions.push(&*FLAME_STRIKE);
    actions.push(&*CHAIN_LIGHTNING);
    // Latest necromancy / divination additions: lv3 Animate Dead (raise
    // skeleton ally) and lv9 Foresight (single-target apex buff).
    actions.push(&*crate::actions::spells::ANIMATE_DEAD);
    actions.push(&*crate::actions::spells::FORESIGHT);
    // Latest enchantment / transmutation additions: lv2 Levitate (CON
    // save lift), lv3 Fly (concentration speed buff), lv3 Plant Growth
    // (Entangle burst), lv4 Confusion (WIS save burst + attack-disad),
    // lv5 Dominate Person (Charmed + Dominated marker).
    actions.push(&*LEVITATE);
    actions.push(&*FLY);
    actions.push(&*SPIDER_CLIMB);
    actions.push(&*PLANT_GROWTH);
    actions.push(&*CONFUSION);
    actions.push(&*DOMINATE_PERSON);
    // Latest apex additions: lv7 Prismatic Spray (random-typed cone of
    // 7 colors, 10d6 per target) and lv8 Feeblemind (single-target INT
    // save → blanket disadvantage on attacks + INT/WIS/CHA saves).
    actions.push(&*PRISMATIC_SPRAY);
    actions.push(&*FEEBLEMIND);
    // Newest additions: lv6 Eyebite (single-target Asleep on WIS save
    // fail), lv6 Otto's Irresistible Dance (single-target dance lock on
    // WIS save fail), lv7 Fire Storm (mass fire DEX-save burst), lv8
    // Maze (single-target inert-removal, INT-save loop in RAW). Rounds
    // out the high-level wizard control kit with the iconic single-
    // target removal / lock spells.
    actions.push(&*crate::actions::spells::EYEBITE);
    actions.push(&*crate::actions::spells::OTTOS_IRRESISTIBLE_DANCE);
    actions.push(&*crate::actions::spells::FIRE_STORM);
    actions.push(&*crate::actions::spells::MAZE);
    // Latest additions:
    //   - **Frostbite** cantrip: 1d6 cold + Slowed-1-round on CON-save fail.
    //   - **Negative Energy Flood** lv5: 5d12 CON-save necrotic burst.
    //   - **Mordenkainen's Sword** lv7: 5d10 force melee spell attack + concentration mark.
    //   - **Power Word Pain** lv7: HP≤100 gating Slowed install.
    //   - **Mass Polymorph** lv9: burst Polymorphed install on the enemy team.
    actions.push(&*crate::actions::spells::FROSTBITE);
    actions.push(&*crate::actions::spells::NEGATIVE_ENERGY_FLOOD);
    actions.push(&*crate::actions::spells::MORDENKAINENS_SWORD);
    actions.push(&*crate::actions::spells::POWER_WORD_PAIN);
    actions.push(&*crate::actions::spells::MASS_POLYMORPH);
    // Sickening Radiance — lv4 evocation, concentration AOE: enemy-only
    // 30ft radiant burst with Exhausted-on-fail. Slots cleanly between
    // Stinking Cloud (lv3) and the higher-tier Sunbeam (lv6) as a
    // mid-tier control-burst.
    actions.push(&*crate::actions::spells::SICKENING_RADIANCE);
    // Latest additions: lv2 Aganazzar's Scorcher (3d8 fire 3-tile burst
    // DEX save half — a clean cheaper Fireball alternative), lv5
    // Bigby's Hand (persistent +1d10 force per-hit rider concentration
    // self-buff), lv6 Tenser's Transformation (50 temp HP + self-attack-
    // advantage concentration self-buff).
    actions.push(&*crate::actions::spells::AGANAZZARS_SCORCHER);
    actions.push(&*crate::actions::spells::BIGBYS_HAND);
    actions.push(&*crate::actions::spells::TENSERS_TRANSFORMATION);
    // Latest spell additions: lv2 Acid Arrow (single-target attack +
    // splash), lv3 Tidal Wave (DEX-save bludgeoning + prone burst), lv5
    // Dawn (CON-save radiant burst concentration), lv6 Mental Prison
    // (INT-save psychic + Restrained-envelope concentration), lv6
    // Investiture of Flame (self-buff with fire-melee retaliation +
    // fire resistance, concentration).
    actions.push(&*crate::actions::spells::ACID_ARROW);
    actions.push(&*crate::actions::spells::TIDAL_WAVE);
    actions.push(&*crate::actions::spells::DAWN);
    actions.push(&*crate::actions::spells::MENTAL_PRISON);
    actions.push(&*crate::actions::spells::INVESTITURE_OF_FLAME);
    // Latest spell additions: lv1 Grease (DEX-save prone burst), lv2
    // Flaming Sphere (DEX-save fire burst, concentration), lv6 Blade
    // Barrier (DEX-save slashing burst, concentration). Grease + Flaming
    // Sphere are core wizard staples; Blade Barrier slots cleanly into
    // the lv6 evocation lane alongside Globe of Invulnerability.
    actions.push(&*crate::actions::spells::GREASE);
    actions.push(&*crate::actions::spells::FLAMING_SPHERE);
    actions.push(&*crate::actions::spells::BLADE_BARRIER);
    // Latest control / utility additions: lv3 Wind Wall (self-buff that
    // imposes ranged-attack disadvantage on attackers, concentration),
    // lv4 Evard's Black Tentacles (DEX-save burst 3d6 bludgeoning +
    // Restrained-on-fail concentration), lv4 Otiluke's Resilient
    // Sphere (single-target DEX-save inert envelope concentration —
    // rounds out the wizard's prison kit alongside Forcecage / Maze).
    actions.push(&*crate::actions::spells::WIND_WALL);
    actions.push(&*crate::actions::spells::EVARDS_BLACK_TENTACLES);
    actions.push(&*crate::actions::spells::OTILUKES_RESILIENT_SPHERE);
    // Maximilian's Earthen Grasp (lv2) — single-target restraint with
    // per-round 2d6 bludgeoning drip. Cheaper alternative to the lv4
    // Black Tentacles AoE when only one threat needs locking down.
    actions.push(&*crate::actions::spells::MAXIMILIANS_EARTHEN_GRASP);
    // Vitriolic Sphere (lv4) — acid AoE with a delayed 5d4 drip on
    // failed-save targets. Sits between Fireball (lv3) and Cone of
    // Cold (lv5); the residual drip punches through resistance better
    // than a flat-damage rival.
    actions.push(&*crate::actions::spells::VITRIOLIC_SPHERE);
    // Newest wizard additions:
    //   - cantrip **Thunderclap**: self-centered 1-tile CON-save burst.
    //   - lv1 **Chromatic Orb**: 3d8 ranged spell attack of caster-picked
    //     damage type (picker maximizes vs target resistance profile).
    //   - lv2 **Snilloc's Snowball Swarm**: cheap cold 1-tile burst.
    //   - lv2 **Mind Spike**: single-target psychic save-for-half.
    //   - lv4 **Psychic Lance**: psychic save-for-half + Incapacitated
    //     rider on fail — soft lock-down next to Polymorph / Confusion.
    actions.push(&*crate::actions::spells::THUNDERCLAP);
    actions.push(&*crate::actions::spells::CHROMATIC_ORB);
    actions.push(&*crate::actions::spells::SNILLOCS_SNOWBALL_SWARM);
    actions.push(&*crate::actions::spells::MIND_SPIKE);
    actions.push(&*crate::actions::spells::PSYCHIC_LANCE);
    // Newer wizard additions:
    //   - lv1 **Ice Knife**: ranged attack + neutral-burst cold
    //     shatter rider, fires hit-or-miss. Round out the lv1 lane
    //     next to Chromatic Orb / Magic Missile.
    //   - lv2 **Enlarge / Reduce**: single-target +1d4 weapon damage
    //     buff (concentration). Plays the support role next to the
    //     wizard's traditional Slow / Haste concentration picks.
    actions.push(&*crate::actions::spells::ICE_KNIFE);
    actions.push(&*crate::actions::spells::ENLARGE_REDUCE);
    // Latest cantrip / lv1-2 utility additions:
    //   - cantrip **Sword Burst**: 1-tile DEX-save force burst around
    //     caster (force-typed at-will, slots between Thunderclap and
    //     Acid Splash in the self-centered cantrip lane).
    //   - cantrip **Blade Ward**: self damage-resistance until next
    //     turn (defensive cantrip for the squishy wizard chassis).
    //   - lv1 **Catapult**: single-target 3d8 DEX-save bludgeoning,
    //     no half on save (punchier than Magic Missile when you need
    //     a single big hit and the target's DEX is low).
    //   - lv1 **Earth Tremor**: self-centered 1d6 DEX-save bludgeoning
    //     + prone, friend-or-foe. Mirrors Tidal Wave at the lv1 tier.
    //   - lv1 **Fog Cloud**: 4-tile concentration burst that installs
    //     Blinded (heavy obscurement) on every actor caught in it.
    //   - lv2 **Gust of Wind**: line push (6 tiles) on a STR save fail,
    //     concentration. Repositioning tool that pairs with Thunderwave
    //     for crowd-control routes.
    actions.push(&*crate::actions::spells::SWORD_BURST);
    actions.push(&*crate::actions::spells::BLADE_WARD);
    actions.push(&*crate::actions::spells::CATAPULT);
    actions.push(&*crate::actions::spells::EARTH_TREMOR);
    actions.push(&*crate::actions::spells::FOG_CLOUD);
    actions.push(&*crate::actions::spells::GUST_OF_WIND);
    // Additional combat spells:
    //   - lv2 **Dragon's Breath**: 2-tile self cone, 3d6 elemental (best-
    //     type picker), DEX save for half. Wizard alt to Burning Hands
    //     once a level-2 slot is spendable.
    //   - lv5 **Steel Wind Strike**: hits up to 5 enemies for 6d10 force,
    //     teleport-to-target rider. Mid-late wizard finisher when the
    //     party's swarmed.
    //   - lv6 **Wall of Ice**: 10d6 cold DEX-save for half + Prone on
    //     fail, concentration. Big slot AoE that pairs the Cone of Cold
    //     dice with a crowd-control rider.
    actions.push(&*crate::actions::spells::DRAGONS_BREATH);
    actions.push(&*crate::actions::spells::STEEL_WIND_STRIKE);
    actions.push(&*crate::actions::spells::WALL_OF_ICE);
    // Telekinetic — cantrip bonus-action shove. Pulls a target 5 ft
    // closer on a failed STR save. Fills the wizard's bonus-action lane
    // (mostly empty between Misty Step / Shield reaction casts) with a
    // free repositioning tool — no slot cost, no concentration.
    actions.push(&*crate::actions::spells::TELEKINETIC);
    // Green-Flame Blade — cantrip melee spell attack: 1d8 fire on the
    // primary target, plus an INT-modifier fire leap to the lowest-HP
    // adjacent enemy on a hit. Wizard's first dedicated melee cantrip,
    // pairs with the existing Booming Blade lane for a melee-cantrip
    // option that exploits the AI's focus-fire heuristic.
    actions.push(&*crate::actions::spells::GREEN_FLAME_BLADE);
    // Sapping Sting — Tasha's necromancy cantrip: 1d4 necrotic + prone
    // on a CON-save fail (30 ft range). The prone rider sets up melee
    // allies' next swing at advantage — punchier than Toll the Dead
    // when there's a fighter / paladin nearby to capitalize.
    actions.push(&*crate::actions::spells::SAPPING_STING);
    // Newest spell additions:
    //   - lv3 **Erupting Earth**: 3d12 bludgeoning DEX-save burst (no
    //     fire resistance dependency, complements Fireball at the lv3
    //     AoE tier).
    //   - lv4 **Blight**: 8d8 necrotic single-target CON-save for half
    //     (high-damage save-for-half against a single chunky target).
    //   - lv6 **Circle of Death**: 8d6 necrotic friend-or-foe-agnostic
    //     60ft-radius CON-save burst (mass damage at lv6).
    //   - lv7 **Delayed Blast Fireball**: 12d6 fire DEX-save burst
    //     (signature lv7 evocation, on top of Fire Storm's lv7 enemy-
    //     only fire).
    //   - lv8 **Incendiary Cloud**: 10d8 fire DEX-save burst.
    //   - lv9 **Weird**: 10d10 psychic + Frightened on WIS-save fail
    //     (boss-tier illusion lock).
    actions.push(&*crate::actions::spells::ERUPTING_EARTH);
    actions.push(&*crate::actions::spells::BLIGHT);
    actions.push(&*crate::actions::spells::CIRCLE_OF_DEATH);
    actions.push(&*crate::actions::spells::DELAYED_BLAST_FIREBALL);
    actions.push(&*crate::actions::spells::INCENDIARY_CLOUD);
    actions.push(&*crate::actions::spells::WEIRD);
    // Newest enchantment / abjuration additions:
    //   - lv4 **Charm Monster**: charm spell that works on any creature
    //     type (Charm Person is humanoid-only RAW). Slots cleanly
    //     between Charm Person (lv1) and Dominate Person (lv5) on the
    //     enchantment ladder.
    //   - lv8 **Mind Blank**: 24-hour psychic + charm immunity buff
    //     for a single ally. Self-target priority for the AI's
    //     defensive pipeline — the wizard pre-blanks themselves before
    //     a charmer / psion encounter.
    actions.push(&*crate::actions::spells::CHARM_MONSTER);
    actions.push(&*crate::actions::spells::MIND_BLANK);
    actions.push(&*crate::actions::spells::THUNDER_STEP);
    actions.push(&*crate::actions::spells::ABSORB_ELEMENTS);
    actions.push(&*crate::actions::spells::SHADOW_BLADE);
    actions.push(&*crate::actions::spells::RAY_OF_ENFEEBLEMENT);
    actions.push(&*crate::actions::spells::INFESTATION);
    actions.push(&*crate::actions::spells::SILVERY_BARBS);
    actions.push(&*crate::actions::spells::PROTECTION_FROM_ENERGY);
    actions.push(&*crate::actions::spells::REMOVE_CURSE);
    // Arcane Recovery — Wizard signature once-per-rest spell-slot
    // recovery. Slot-restoration on short rest gives the wizard a clean
    // mid-encounter "I'm out of slots" recovery without a long rest. The
    // feature itself is a free action — no slot or action-economy cost.
    actions.push(&*ARCANE_RECOVERY);
    // Latest wizard additions:
    //   - lv5 **Wall of Stone**: 2-tile burst DEX save; failed-save
    //     enemies are Restrained for 10 rounds (concentration-anchored).
    //     Slots between Wall of Fire (lv4 damage zone) and Wall of Force
    //     (lv5 prone shove) on the area-denial ladder; the restraint is
    //     the load-bearing crowd-control.
    //   - lv6 **Investiture of Ice**: self-only concentration buff —
    //     cold resistance plus 1d10 cold retaliation on melee hits.
    //     Symmetric to Investiture of Flame; gives the wizard a cold-
    //     themed defensive concentration option at lv6.
    actions.push(&*crate::actions::spells::WALL_OF_STONE);
    actions.push(&*crate::actions::spells::INVESTITURE_OF_ICE);
    // Investiture of Stone — lv6 transmutation, concentration. Hardens
    // the wizard's body: bludgeoning / piercing / slashing resistance
    // (the physical trio) plus 1d10 force retaliation on every melee
    // hit. Sibling to Investiture of Flame / Ice — same install +
    // reflect shape, but broader physical resistance against martial
    // swarms instead of a single-element shield. Force-typed
    // retaliation chips through almost any creature's defenses.
    actions.push(&*crate::actions::spells::INVESTITURE_OF_STONE);
    // Tasha's Caustic Brew — lv1 evocation, 30ft line, 2d4 acid initial +
    // 2d4 acid drip per round until the target wipes it off or the caster
    // drops concentration. Sustained-DoT differentiator at the lv1 tier
    // (Burning Hands does more upfront fire, Acid Splash is cheaper but
    // cantrip-tier; Caustic Brew sits between as the acid-themed control
    // option whose damage accumulates across multiple rounds).
    actions.push(&*crate::actions::spells::TASHAS_CAUSTIC_BREW);
    // Vortex Warp — lv2 conjuration (Tasha's). 90-ft single-target
    // teleport: willing ally auto-yanked to the wizard's side, unwilling
    // enemy makes a CON save vs the wizard's spell DC. Tactical
    // displacement at the lv2 tier — pulls a stranded ally to safety or
    // drags a back-line caster into the wizard's allies' melee envelope.
    actions.push(&*crate::actions::spells::VORTEX_WARP);
    // Latest wizard additions:
    //   - lv2 **Phantasmal Force** (illusion): INT save vs the wizard's
    //     spell DC, on fail target picks up the `PhantasmalForced`
    //     condition (1d6 psychic / round via the central DoT registry).
    //     Sustained-DoT differentiator at the lv2 illusion tier; the
    //     INT save rules anchor the wizard's strongest stat against
    //     low-INT brutes.
    //   - lv4 **Watery Sphere** (conjuration, XGtE): STR save vs the
    //     wizard's spell DC; on fail target is `WaterSphered` (Restrained
    //     + Lifted envelope, concentration-bound). Single-target trap at
    //     the lv4 tier — slots between Levitate (lv2 CON-save lift) and
    //     Otiluke's Resilient Sphere (lv4 DEX-save full lockout).
    //   - lv5 **Wall of Light** (evocation, XGtE): 4d8 radiant 2-tile
    //     burst CON save for half + Blinded-on-fail (concentration).
    //     Wizard's first multi-target Blinded lane — complements the
    //     Wall of Stone / Cloudkill / Wall of Force lv5 ladder.
    //   - lv6 **Investiture of Wind** (transmutation, XGtE): self-only
    //     concentration buff — ranged disadvantage to attackers +
    //     +60ft flying speed. Rounds out the Flame / Ice / Stone /
    //     Wind investiture quartet at the lv6 slot.
    actions.push(&*crate::actions::spells::PHANTASMAL_FORCE);
    actions.push(&*crate::actions::spells::WATERY_SPHERE);
    actions.push(&*crate::actions::spells::WALL_OF_LIGHT);
    actions.push(&*crate::actions::spells::INVESTITURE_OF_WIND);
    // Wizard blasting / displacement additions (PHB / XGtE):
    //   - lv2 **Dust Devil** (conjuration, XGtE): 1-tile burst at a
    //     target point, STR save 1d8 bludgeoning for half + 4-tile push
    //     on fail. The lv2 displacement option that complements
    //     Thunderwave (self-centered) and Vortex Warp (single-target
    //     teleport) — pushes a clustered enemy line apart at range.
    //   - lv5 **Maelstrom** (evocation, XGtE): 3-tile burst, 6d6
    //     bludgeoning STR-save for half + pull-into-center on fail. The
    //     anti-Tidal-Wave: pulls enemies INTO the center for a follow-up
    //     Fireball / Cone of Cold instead of laying them flat.
    //   - lv6 **Otiluke's Freezing Sphere** (evocation): 6-tile burst,
    //     10d6 cold CON-save for half. Slots between Cone of Cold (lv5)
    //     and Sunburst (lv8) on the AoE blasting ladder — a cold-typed
    //     nuke that pairs with the wizard's Investiture of Ice.
    actions.push(&*crate::actions::spells::DUST_DEVIL);
    actions.push(&*crate::actions::spells::MAELSTROM);
    actions.push(&*crate::actions::spells::OTILUKES_FREEZING_SPHERE);
    // Wizard lockdown / capstone additions:
    //   - lv6 **Flesh to Stone** (transmutation, PHB): CON save vs the
    //     wizard's spell DC; on fail target is Petrified for ~1 minute
    //     (concentration-bound, with a round-end CON save to break free
    //     via the shared `ROUND_END_SAVES` table). The CON-save lockdown
    //     option at the lv6 tier — slots between Hold Monster (lv5, WIS)
    //     and Otto's Irresistible Dance (lv6, WIS) on the single-target
    //     control ladder.
    //   - lv9 **Psychic Scream** (enchantment, XGtE): self-centered
    //     8-tile burst, 14d6 psychic INT-save for half + Stunned-on-fail
    //     (with the existing round-end WIS-save break-free hook). The
    //     wizard's lv9 mind-spike — distinct from Weird (WIS-save
    //     Frightened) and Meteor Swarm (DEX-save fire/bludgeoning) on
    //     the lv9 nuke ladder by save ability + stunning rider.
    actions.push(&*crate::actions::spells::FLESH_TO_STONE);
    actions.push(&*crate::actions::spells::PSYCHIC_SCREAM);
    // Latest wizard additions:
    //   - lv4 **Storm Sphere** (evocation, XGtE): 4-tile burst, 2d6
    //     bludgeoning STR save (none-on-save) + `WindBlasted` rider
    //     for the duration (concentration-bound). The lv4 control
    //     differentiator — slots between Ice Storm (lv4 DEX-save
    //     half) and Wall of Light (lv5 CON-save half + Blinded).
    //     Targets the bow-wielding back rank: the rider's ranged-
    //     attacker disadvantage compounds with Storm Sphere's
    //     initial damage.
    //   - lv5 **Geas** (enchantment, PHB): single-target Charmed-on-
    //     fail WIS save (concentration-FREE). Long-duration
    //     compulsion that locks the target out of attacking the
    //     wizard. Slots between Charm Monster (lv4) and Dominate
    //     Person (lv5) on the single-target charm ladder.
    //   - lv8 **Maddening Darkness** (evocation, XGtE): 6-tile
    //     burst, 8d8 psychic WIS save for half (concentration-
    //     bound). The lv8 burst differentiator from Sunburst (DEX
    //     save, radiant) and Power Word Stun (single-target HP-
    //     gated) — saves vs WIS hit caster/martial types that
    //     shrug off the DEX/INT lanes.
    actions.push(&*crate::actions::spells::STORM_SPHERE);
    actions.push(&*crate::actions::spells::GEAS);
    actions.push(&*crate::actions::spells::MADDENING_DARKNESS);
    // Latest wizard additions (XGtE):
    //   - lv3 **Wall of Sand** (evocation): 3-tile burst, STR save or
    //     Restrained for the duration (concentration-bound). The wizard's
    //     STR-save restraint option at the lv3 slot — slots between
    //     Web (lv2 DEX-save Restrained burst) and Black Tentacles (lv4
    //     DEX-save 3d6 + Restrained burst) on the restraint ladder.
    //     Distinct from Web by the STR-save lane (resists STR-heavy
    //     enemies less well but punishes DEX builds), distinct from
    //     Black Tentacles by the smaller slot cost and lack of damage
    //     rider.
    //   - lv3 **Wall of Water** (evocation, druid / sorcerer / wizard):
    //     3-tile burst, no save / no damage — every enemy in the burst
    //     picks up `WindWalled` for the duration (ranged-attacker
    //     disadvantage on holders). Concentration-bound. The wizard's
    //     ranged-defense companion to Wind Wall (lv3 self-only).
    actions.push(&*crate::actions::spells::WALL_OF_SAND);
    actions.push(&*crate::actions::spells::WALL_OF_WATER);
    // Latest mobility / utility additions:
    //   - lv1 **Longstrider** (transmutation): touch +10 ft speed for
    //     1 hour, no concentration. Fills the wizard's pre-combat
    //     mobility lane — composes with Fly / Spider Climb / Investiture
    //     of Wind via the central `condition_speed_bonus` lane.
    //   - lv1 **Expeditious Retreat** (transmutation): bonus-action
    //     self-buff that grants +30 ft speed for 10 rounds, concentration.
    //     Distinct from Longstrider by the action-economy cost (bonus
    //     action vs full action) and the larger / shorter / concentration-
    //     bound boost — the kiting tool for clutch repositioning.
    //   - lv2 **Earthbind** (transmutation, XGtE): single-target STR-save
    //     vs the wizard's spell DC; on fail the target's flying speed
    //     drops to 0. Wizard's grounding tool for airborne enemies that
    //     the lv2 evocation lane otherwise lacks.
    actions.push(&*crate::actions::spells::LONGSTRIDER);
    actions.push(&*crate::actions::spells::EXPEDITIOUS_RETREAT);
    actions.push(&*crate::actions::spells::EARTHBIND);
    // Latest wizard additions:
    //   - lv2 **Enhance Ability** (transmutation): touch ally buff —
    //     2d6 temp HP + flat +2 saves for the duration (concentration).
    //     Slots cleanly into the wizard's support lane next to Bless's
    //     burst cousin; the temp HP rider differentiates it from Bless.
    //   - lv3 **Blink** (transmutation): self-only `Displaced` install
    //     (10 rounds, no concentration). Defensive lane sibling to Blur
    //     (concentration-bound attacker-disadvantage); Blink frees the
    //     wizard's concentration slot for Hold Monster / Web / etc.
    //   - lv5 **Contagion** (necromancy): single-target touch CON-save
    //     vs the wizard's spell DC; on fail target picks up Poisoned
    //     for 10 rounds. Necrotic-themed CON-save lockdown next to
    //     Hold Monster's WIS-save lane.
    actions.push(&*crate::actions::spells::ENHANCE_ABILITY);
    actions.push(&*crate::actions::spells::BLINK);
    actions.push(&*crate::actions::spells::CONTAGION);
    // Latest wizard additions (XGtE / TCE):
    //   - lv2 **Pyrotechnics** (transmutation, XGtE): 2-radius CON-save
    //     fire burst (1d8) + Blinded-on-fail. Cheap entry-tier flash
    //     burst alongside Aganazzar's Scorcher / Snilloc's Snowball Swarm
    //     on the lv2 elemental-burst lane.
    //   - lv3 **Flame Arrows** (transmutation, XGtE): self-buff that
    //     grants +1d6 fire on every ranged weapon hit (ranged-only via
    //     the OnHitRider table). Concentration-bound; sibling to Spirit
    //     Shroud (melee cold rider) on the per-hit weapon buff lane.
    //   - lv3 **Ashardalon's Stride** (transmutation, TCE): self-buff
    //     that grants +20 ft speed and 1d6 fire trail damage to
    //     footprint-adjacent enemies on each move step. Concentration-
    //     bound; mobility + control combo at the lv3 slot.
    //   - lv6 **Tasha's Otherworldly Guise** (transmutation, TCE): the
    //     legendary-tier self-buff — +2 AC, +60 ft fly speed, radiant +
    //     poison resistance, Charmed / Frightened / Poisoned dynamic
    //     immunity, +2d6 radiant melee weapon rider. Concentration-bound;
    //     sits at the top of the wizard's self-buff ladder alongside
    //     Tenser's Transformation / Globe of Invulnerability.
    actions.push(&*crate::actions::spells::PYROTECHNICS);
    actions.push(&*crate::actions::spells::FLAME_ARROWS);
    actions.push(&*crate::actions::spells::ASHARDALONS_STRIDE);
    actions.push(&*crate::actions::spells::OTHERWORLDLY_GUISE);
    // Latest wizard utility additions:
    //   - lv2 **Silence**: 20ft sphere of magical hush. Anti-caster zone
    //     that locks down enemy spellslots via `blocks_spell_slots`.
    //   - lv2 **Darkness**: 15ft sphere of magical darkness. Concentration-
    //     bound symmetric blind zone (holders and attackers both eat
    //     disadvantage) — good either as a defensive shroud over allies
    //     or an offensive blind drop over a tight enemy cluster.
    //   - lv4 **Freedom of Movement**: ally-buff that breaks any active
    //     Paralyzed / Restrained / Grappled install and grants dynamic
    //     immunity for the duration.
    actions.push(&*crate::actions::spells::SILENCE);
    actions.push(&*crate::actions::spells::DARKNESS);
    actions.push(&*crate::actions::spells::FREEDOM_OF_MOVEMENT);
    // lv5 **Conjure Elemental** (conjuration): summon a single Large fire
    // elemental ally adjacent to the caster, concentration-bound. Fills
    // the wizard's lv5 summon slot — the wizard's existing lv5 lane
    // already has Cone of Cold (burst) and Hold Monster (single-target
    // lockdown); the elemental adds an action-economy multiplier in the
    // same tier. Despawns via the shared `Conjured` cleanup path when
    // concentration drops.
    actions.push(&*crate::actions::spells::CONJURE_ELEMENTAL);
    // lv5 **Animate Objects** (transmutation): summon ten Tiny Construct
    // minions adjacent to the caster. Trades the per-target damage of
    // Cone of Cold or the single-target lockdown of Hold Monster for
    // ten independent action-economy threats — each minion swings a
    // 1d4 force slam every round under the caster's command. Despawns
    // via the shared `Conjured` cleanup path when concentration drops.
    actions.push(&*crate::actions::spells::ANIMATE_OBJECTS);
    // Latest evocation / transmutation additions:
    //   - lv1 **Magnify Gravity** (evocation, TCE / EGtW): 5ft burst,
    //     STR save vs the wizard's spell DC; failed-save targets eat 2d8
    //     force AND pick up `Slowed` for 1 round. Friend-or-foe agnostic
    //     burst that slots cleanly between Earth Tremor (self-centered
    //     1d6 + prone) and Magic Missile (auto-hit force) on the lv1
    //     force-damage lane — distinguished by the targeted-point reach
    //     (24 tiles RAW) + the slow-rider follow-up.
    //   - lv3 **Elemental Weapon** (transmutation, PHB): touch-range
    //     concentration buff for a single ally weapon-wielder; +1 attack
    //     AND +1d4 fire per melee hit via the OnHitRider table. Sibling
    //     to Magic Weapon (lv2 +1/+1) on the weapon-buff lane; distinct
    //     by the typed per-hit rider that chips through resistance the
    //     flat +1 damage rider can't touch.
    actions.push(&*crate::actions::spells::MAGNIFY_GRAVITY);
    actions.push(&*crate::actions::spells::ELEMENTAL_WEAPON);
    CreatureTemplate {
        name: "Wizard",
        // 'M' (mage) — keeps 'W' free for Wolf, which already claims it.
        glyph: 'M',
        ac: 12,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 16, // primary spellcasting ability
        dexterity: 14,
        wisdom: 11,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // 4/3/3/2/2/1/1/1/1 — typical level-17 wizard archmage loadout.
        // The high-level slots (6+) fuel exactly one Disintegrate / Heal,
        // Finger of Death, and Power Word Stun apiece — late-game
        // emergency buttons rather than spam fodder. Mid-level slots
        // (3-5) still cover Fireball / Haste / Slow / Stinking Cloud /
        // Cone of Cold / Hold Monster / Synaptic Static.
        spell_slots_by_level: vec![4, 3, 3, 2, 2, 1, 1, 1, 1],
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        // Wizards are proficient in INT and WIS saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::from([ARCANE_RECOVERY_TAG]),
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
        sorcery_points: 0,
    }
});
