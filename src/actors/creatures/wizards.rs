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
use std::collections::HashSet;
use std::sync::LazyLock;

/// Squishy INT-caster. Fire Bolt as the at-will ranged option, Magic
/// Missile and Burning Hands as level-1 nuke / AoE. Stat shape mirrors
/// the cleric (low HP, medium AC, tunes around ranged spell attacks)
/// but uses INT as the spellcasting ability so a separate save DC and
/// attack mod come into play.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    // Light — the evocation cantrip every one of these classes has on
    // its list, and the party's answer to an unlit board: touch an ally
    // (or yourself) and they carry 20 ft of bright light and 20 ft of
    // dim light with them for the rest of the fight. Declines to cast
    // on a board that is already bright, and declines to re-light
    // somebody who is already lit, so it costs nothing on the ambient
    // default and is there when the lights are out.
    actions.push(&*crate::actions::spells::LIGHT);
    // Dancing Lights — the cantrip that lights ground the party has
    // not walked onto yet. Free and remote at once, which neither the
    // Light cantrip (free, but has to be touched onto somebody) nor
    // Daylight (remote, but a level-3 slot) manages. Holds
    // concentration, so it competes with the real spells rather than
    // stacking on them, and declines to cast on a board that is
    // already bright. See `spells::DANCING_LIGHTS`.
    actions.push(&*crate::actions::spells::DANCING_LIGHTS);
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
    // Mislead — the level-5 illusion that is the two halves the engine
    // already priced, in one action: `Invisible` and the Trickery
    // Cleric's `Duplicity`, which until now no spell could reach. Not a
    // better Greater Invisibility but a differently shaped one — bought
    // to *not* attack from, and cashed out whenever one doubly
    // advantaged swing is worth ending it for. See `spells::MISLEAD`.
    actions.push(&*crate::actions::spells::MISLEAD);
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
    // Imprisonment — the ninth-level abjuration that ends one creature
    // and asks for nothing else: one Wisdom save, no concentration, no
    // timer. Where Maze spends a level-8 slot *and* the caster's whole
    // concentration to remove somebody for ten rounds, this removes
    // them for the fight and leaves the concentration free. Against the
    // one enemy the party cannot beat, that is the purchase.
    // See `spells::IMPRISONMENT`.
    actions.push(&*crate::actions::spells::IMPRISONMENT);
    actions.push(&*METEOR_SWARM);
    actions.push(&*SUNBEAM);
    // Antimagic Field — lv8 abjuration, concentration. The engine's
    // only magic-suppressing zone: a 10-ft sphere that travels with the
    // wizard and inside which no spell can be cast or land. Costs the
    // wizard's action economy nothing after the cast and shuts down an
    // enemy caster's entire turn — at the price of the wizard's own
    // spellcasting for as long as it is up.
    actions.push(&*crate::actions::spells::ANTIMAGIC_FIELD);
    // Abi-Dalzim's Horrid Wilting — lv8 necromancy. The largest single
    // burst on the list (12d8 necrotic, CON save for half) with a
    // creature-type filter in front of it: constructs and undead are
    // untouched, plants and water elementals save at disadvantage.
    actions.push(&*crate::actions::spells::HORRID_WILTING);
    // Enervation — lv5 necromancy, concentration. A draining tether:
    // 4d8 necrotic a round with half of it fed back to the wizard.
    actions.push(&*crate::actions::spells::ENERVATION);
    // Whirlwind — lv7 evocation, concentration. A standing column that
    // bills 10d6 to anything walking through it and knocks down what it
    // does not kill. The wizard's answer to a doorway.
    actions.push(&*crate::actions::spells::WHIRLWIND);
    // Scatter — lv6 conjuration. No damage at all: it takes an enemy
    // line apart and drops the pieces across the board.
    actions.push(&*crate::actions::spells::SCATTER);
    // Newly added wizard spells (lv4 Dimension Door / Wall of Fire / Fire
    // Shield; lv5 Cloudkill / Wall of Force).
    actions.push(&*DIMENSION_DOOR);
    actions.push(&*WALL_OF_FIRE);
    actions.push(&*FIRE_SHIELD);
    actions.push(&*CLOUDKILL);
    actions.push(&*WALL_OF_FORCE);
    // The ward family — lv3 Glyph of Warding and lv7 Symbol. The
    // engine's first areas that are *set* rather than cast: invisible
    // to the pathfinder, stepped over by the wizard's own side, and
    // spent the moment an enemy walks in. See `ZoneEffect::ward`.
    actions.push(&*crate::actions::spells::GLYPH_OF_WARDING);
    actions.push(&*crate::actions::spells::SYMBOL);
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
    actions.push(&crate::actions::spells::ANIMATE_DEAD);
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
    //   - lv2 **Enlarge / Reduce**, both halves. Enlarge grows an ally
    //     one size category for +1d4 a swing and STR-save advantage;
    //     Reduce is the same spell pointed the other way, shrinking an
    //     enemy on a failed CON save. Both are concentration, so the
    //     wizard picks a direction per fight the same way they pick
    //     between Slow and Haste.
    actions.push(&*crate::actions::spells::ICE_KNIFE);
    actions.push(&*crate::actions::spells::ENLARGE_REDUCE);
    actions.push(&*crate::actions::spells::REDUCE);
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
    actions.push(&crate::actions::spells::CONJURE_ELEMENTAL);
    // The Tasha's summon family, wizard half — the full spread, because
    // RAW gives the wizard every one of them. Six rungs from level 3 to
    // level 6, and at each of 3 and 4 a melee body and a ranged body
    // with the same price, so the wizard's summon decision is about the
    // board rather than about the slot.
    actions.push(&crate::actions::spells::SUMMON_FEY);
    actions.push(&crate::actions::spells::SUMMON_UNDEAD);
    actions.push(&crate::actions::spells::SUMMON_ABERRATION);
    actions.push(&crate::actions::spells::SUMMON_ELEMENTAL);
    actions.push(&crate::actions::spells::SUMMON_DRACONIC_SPIRIT);
    actions.push(&crate::actions::spells::SUMMON_FIEND);
    // lv4 **Conjure Minor Elementals** — the wizard's half of the SRD
    // conjure family, and the only four-body cohort on the arcane list.
    // Four Small footprints where every other wizard summon is one
    // Medium or Large one, which is a different purchase: the summon
    // family's spirits are a body that fights, and this is four bodies
    // that occupy tiles. Concentration, like all of them.
    actions.push(&crate::actions::spells::CONJURE_MINOR_ELEMENTALS);
    // lv3 **Phantom Steed** (illusion) — the arcane mount, and the
    // reason `engine::mounts` is reachable by a class other than the
    // paladin. No concentration, so the wizard rides it *and* casts
    // from it; 100 ft of speed, which is more than any other body on
    // the board can cover. See `spells::PHANTOM_STEED`.
    actions.push(&crate::actions::spells::PHANTOM_STEED);
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
    // Latest evocation / divination additions:
    //   - lv5 **Immolation** (transmutation, XGtE): single-target DEX
    //     save 8d6 fire (half-on-save); failed-save targets pick up
    //     `Immolated` for ongoing 4d6 fire / round (round-end DEX save
    //     to extinguish). Concentration-bound. Slots between Cone of
    //     Cold (lv5 burst) and Wall of Fire (lv4 zone) on the fire-
    //     damage lane as sustained single-target pressure.
    //   - lv6 **True Seeing** (divination, PHB): touch-range ally buff
    //     that suppresses the disadvantage from a target's `Invisible`
    //     / `Blurred` / `Displaced` (and the matching attacker advantage
    //     from `Invisible`). Rounds out the wizard's anti-illusion kit
    //     next to the lv4 Greater Invisibility (offensive) and lv4
    //     Polymorph (offensive shape-change) — True Seeing is the
    //     defensive-counter half against an illusionist opponent.
    actions.push(&*crate::actions::spells::IMMOLATION);
    actions.push(&*crate::actions::spells::TRUE_SEEING);
    // See Invisibility — the lv2 half of the same answer. Carried
    // alongside True Seeing rather than instead of it because the two
    // are priced four slot levels apart and pierce different amounts:
    // against a plain Invisible opponent the lv2 self-buff is the right
    // spend, and the lv6 slot stays free for Globe / Mass Suggestion.
    actions.push(&*crate::actions::spells::SEE_INVISIBILITY);
    // Darkvision — the other half of "I cannot see". See Invisibility
    // above answers a hidden enemy; this answers an unlit room, which
    // is the far commoner problem and the one the wizard is least
    // equipped for by birth.
    actions.push(&*crate::actions::spells::DARKVISION);
    // The XGE / TCE lane the wizard had none of. Every one of these is
    // on the wizard's RAW list, and each fills a slot tier with a shape
    // the chassis was missing:
    //   - lv2 **Rime's Binding Ice**: a cone that costs nothing to hold.
    //     Every other Restrained source the wizard carries (Web, Black
    //     Tentacles, Earthen Grasp, Watery Sphere) is concentration, so
    //     this is the only one that can land while something else is up.
    //   - lv3 **Melf's Minute Meteors**: an Action to light, a bonus
    //     action a turn to throw. The wizard's bonus-action lane is
    //     otherwise Misty Step and Telekinetic; this puts damage in it.
    //   - lv3 **Life Transference**: the wizard's only heal, priced in
    //     its own hit points — see the spell's own docs for the trade.
    //   - lv3 **Intellect Fortress**: psychic resistance and mental-save
    //     advantage, on the squishiest chassis on the roster.
    //   - lv3 **Enemies Abound**: single-target Confusion a slot early,
    //     off an INT save rather than a WIS one.
    //   - lv4 **Gravity Sinkhole**: 5d10 force *and* it drags the
    //     survivors into one tile for the next burst.
    //   - lv5 **Far Step**: repeatable escape for a caster whose whole
    //     defensive plan is not being reachable.
    //   - lv9 **Blade of Disaster**: 8d12 force a turn for as long as
    //     concentration holds — the lv9 that keeps paying, next to
    //     Meteor Swarm / Power Word Kill / Weird, which pay once.
    actions.push(&*crate::actions::spells::RIMES_BINDING_ICE);
    actions.push(&*crate::actions::spells::MINUTE_METEORS);
    actions.push(&*crate::actions::spells::LIFE_TRANSFERENCE);
    actions.push(&*crate::actions::spells::INTELLECT_FORTRESS);
    actions.push(&*crate::actions::spells::ENEMIES_ABOUND);
    actions.push(&*crate::actions::spells::GRAVITY_SINKHOLE);
    actions.push(&*crate::actions::spells::FAR_STEP);
    actions.push(&*crate::actions::spells::BLADE_OF_DISASTER);
    CreatureTemplate {
        name: "Wizard",
        // 'M' (mage) — keeps 'W' free for Wolf, which already claims it.
        glyph: 'M',
        ac: 12,
        hitpoints: "2d6+2".parse().unwrap(),
        strength: 8,
        dexterity: 14,
        constitution: 12,
        intelligence: 16, // primary spellcasting ability
        wisdom: 11,
        charisma: 10,
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
        // Wizards are proficient in INT and WIS saves (5e PHB).
        proficient_saves: HashSet::from([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]),
        features: HashSet::from([
            ARCANE_RECOVERY_TAG,
            // 5e **Feather Fall** — a wizard-list level-1 reaction spell
            // that is a tag rather than an action because its RAW
            // trigger is somebody else falling. See `FEATHER_FALL_TAG`
            // and `EncounterInstance::try_feather_fall`; the cost is a
            // reaction and a 1st-level slot, both spent at the hook.
            crate::actions::class_features::FEATHER_FALL_TAG,
        ]),
        ..CreatureTemplate::defaults()
    }
});

/// Necromancy Wizard — Arcane Tradition **School of Necromancy**
/// subclass build (PHB). Identical envelope to the baseline
/// `WIZARD_TEMPLATE` (INT-primary full-caster with the archmage-tier
/// spell loadout, Arcane Recovery for mid-encounter slot regen) with
/// one subclass feature layered on: **Inured to Undeath** (Necromancy
/// subclass level 10, PHB) — passive **resistance to necrotic damage**.
///
/// The signature "the necromancer's flesh has grown accustomed to the
/// grave-cold" tell — where a baseline Wizard eats a Chill Touch / Ray
/// of Enfeeblement / Vampiric Touch / Blight / Finger of Death /
/// Negative Energy Flood clean, the Necromancy Wizard halves the
/// incoming necrotic damage. The wizard's own necromancy spell list
/// stops trickling back onto its own chassis on a friendly-fire
/// miscast under the halving rule — a Necromancer casting Vampiric
/// Touch at self-caster-adjacent range (a hypothetical support-ally
/// heal drain) eats /2 the reflected necrotic instead of the full
/// amount.
///
/// Read at the shared `PASSIVE_TYPED_RESISTANCES` cohort in
/// `actor_template.rs` next to the Warlock Elemental Gift rows (Marid
/// Cold / Dao Bludgeoning / Djinni Thunder), Radiant Soul's radiant
/// row, Fiendish / Draconic Resilience's fire rows, Heart of the
/// Storm's lightning + thunder row, and Psychic Defenses' psychic row
/// — same lane, different subclass flavor, different damage axis
/// (Necrotic vs. Cold / Fire / Lightning + Thunder / Psychic / Radiant
/// / Bludgeoning / Thunder).
///
/// **First subclass template on the wizard chassis** — the baseline
/// `WIZARD_TEMPLATE` shipped no subclass template before this feature,
/// leaving the wizard the only PC-facing class without a subclass
/// build. Rounds out the class subclass-template coverage matrix (every
/// other PC class — barbarian, bard, cleric, druid, fighter, monk,
/// paladin, ranger, rogue, sorcerer, warlock — already ships at least
/// one subclass template).
///
/// RAW's School of Necromancy picks up other features not shipped on
/// this template — **Grim Harvest** (lv2: regain HP = 2x spell level
/// when killing a creature with a spell of lv1+, 3x for necromancy
/// spells; needs a per-cast "did this spell kill?" hook and a
/// per-spell school tag on every wizard spell), **Undead Thralls**
/// (lv6: Animate Dead / Create Undead riders — extra minion, +wiz
/// level HP on summoned undead; needs an Animate Dead ally-summon
/// spell surface and a per-minion buff hook), **Command Undead**
/// (lv14 capstone: CHA-save undead-domination lane; needs a per-target
/// domination install on the Undead creature type). Only the lv10
/// Inured to Undeath passive has a mechanical surface on the CR-0.5
/// chassis that plugs cleanly into the shared passive typed-resistance
/// cohort, so we ship that half and leave the rest as future work —
/// matching the way `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE` /
/// `DJINNI_WARLOCK_TEMPLATE` each ship only the Elemental Gift
/// resistance half of their RAW Genie patron kit.
///
/// Sibling on the passive typed-resistance subclass lane to:
///   - **Elemental Gift (Marid)** (Marid Warlock lv6, TCE): Cold.
///   - **Elemental Gift (Dao)** (Dao Warlock lv6, TCE): Bludgeoning.
///   - **Elemental Gift (Djinni)** (Djinni Warlock lv6, TCE): Thunder.
///   - **Radiant Soul** (Celestial Warlock lv6, XGtE): Radiant.
///   - **Fiendish Resilience** (Fiend Warlock lv10): Fire.
///   - **Draconic Resilience** (Draconic Sorcerer lv6): Fire.
///   - **Heart of the Storm** (Storm Sorcerer lv6): Lightning + Thunder.
///   - **Psychic Defenses** (Aberrant Mind Sorcerer lv14, TCE): Psychic.
///     All share the "one feature tag drives one cohort row"
///     declarative-table pattern, different damage axis and different
///     source chassis (this is the first wizard-chassis row on the
///     cohort).
///
/// Sibling on the Arcane Tradition subclass lane to the baseline
/// `WIZARD_TEMPLATE` (patron-less baseline with Arcane Recovery).
///
/// Ships on the CR-0.5 wizard chassis above the strict RAW lv10 gate
/// for the same reason `MARID_WARLOCK_TEMPLATE` / `DAO_WARLOCK_TEMPLATE`
/// / `DJINNI_WARLOCK_TEMPLATE` ship Elemental Gift (RAW lv6) and
/// `ABERRANT_MIND_SORCERER_TEMPLATE` ships Psychic Defenses (RAW lv14):
/// class templates target a balanced playable level, not lockstep PHB
/// progression.
///
/// Glyph 'N' — 'N' for the "Necromancy" identity. Distinct from
/// baseline wizard 'M' (mage). Collides with no other current PC
/// template glyph — the two never legally co-occur on a single team
/// (one glyph per team-color-and-team-id combo suffices to disambiguate
/// them in a mixed encounter).
pub static NECROMANCY_WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Wizard envelope wholesale
    // and layers on the one Necromancy Arcane Tradition subclass feature
    // (`INURED_TO_UNDEATH_TAG`). The `..base.clone()` tail inside the
    // helper picks up every other field — stats, spell slots, save
    // profs, the full wizard cantrip / lv1-9 spell loadout, and the
    // Arcane Recovery feature — without an N-line field-by-field copy.
    // Sibling helper users on the "clone base + insert one tag" cross-
    // class lane: every tag-only Warlock Otherworldly Patron subclass
    // (via `subclass_warlock_template`), `LIFE_CLERIC_TEMPLATE`,
    // `ABERRANT_MIND_SORCERER_TEMPLATE`, `DIVINE_SOUL_SORCERER_TEMPLATE`.
    // Glyph 'N' — for the "Necromancy" identity; distinct from baseline
    // wizard 'M' (mage).
    WIZARD_TEMPLATE.with_subclass_tag(
        "Necromancy Wizard",
        'N',
        crate::actions::class_features::INURED_TO_UNDEATH_TAG,
    )
});

/// War Magic Wizard — Arcane Tradition **School of War Magic** subclass
/// build (XGtE). Identical envelope to the baseline `WIZARD_TEMPLATE`
/// (INT-primary full-caster with the archmage-tier spell loadout,
/// Arcane Recovery for mid-encounter slot regen) with one subclass
/// feature layered on: **Tactical Wit** (War Magic subclass level 2,
/// XGtE) — passive **+INT-modifier to initiative rolls**.
///
/// The signature "the war mage arrives with a plan already in motion"
/// tell — where a baseline Wizard rolls initiative on a middling DEX
/// mod, the War Magic Wizard folds their high INT modifier (16-20 on
/// a level-9 archmage-tier build) into the roll. Composes cleanly with
/// the wizard's high-value opening cast: a War Magic Wizard who wins
/// initiative reliably lands Shield / Mirror Image / Fireball /
/// Hypnotic Pattern / Slow / Counterspell before the first enemy swing.
///
/// Read at the shared `ABILITY_MOD_INITIATIVE_BONUSES` cohort in
/// `actor_template.rs` next to Rakish Audacity (Swashbuckler Rogue,
/// +CHA-mod) and Dread Ambusher (Gloom Stalker Ranger, +WIS-mod) as
/// the third `AbilityModInitiativeBonus { flag, ability }` row — same
/// declarative shape, different ability axis (INT here vs. CHA / WIS
/// on the sibling rows) and different subclass chassis (Wizard vs.
/// Rogue / Ranger). Stacks additively per the cohort's "any row hit
/// is sufficient; all hitting rows sum" semantic.
///
/// Distinct from `WIZARD_TEMPLATE` (subclass-less baseline) and
/// `NECROMANCY_WIZARD_TEMPLATE` (School of Necromancy — passive
/// necrotic resistance via `INURED_TO_UNDEATH_TAG`). The three
/// wizard-chassis templates cover distinct axes: baseline has just
/// Arcane Recovery, Necromancy layers a passive damage-halver, War
/// Magic layers a passive initiative-augment. A War-Magic-vs-Necromancy
/// or War-Magic-vs-baseline encounter renders unambiguously by name
/// AND subclass features don't stack RAW-illegally on a single PC
/// build (RAW: one Arcane Tradition pick per wizard).
///
/// Sibling on the "one feature tag drives one ABILITY_MOD_INITIATIVE_BONUSES
/// cohort row" declarative-table pattern to `SWASHBUCKLER_ROGUE_TEMPLATE`
/// (Rakish Audacity, CHA) and `GLOOM_STALKER_RANGER_TEMPLATE` (Dread
/// Ambusher, WIS) — three ability axes covered across three class
/// chassis. INT was uncovered on the initiative-mod cohort until this
/// template landed; the wizard-chassis INT-primary stat spread makes
/// it the natural fit.
///
/// RAW's School of War Magic picks up other features not shipped on
/// this template — **Arcane Deflection** (lv2 reaction: +2 AC vs one
/// attack roll or +4 to a saving throw, but forfeit non-cantrip casts
/// until end of next turn; needs a reactive AC/save-modifier hook with
/// a next-turn cast lockout), **Power Surge** (lv6: store magical
/// energy from spent counterspells / dispels, add half-wizard-level
/// force damage to one spell per turn; needs a per-cast damage-boost
/// hook and a counterspell / dispel side-channel), **Durable Magic**
/// (lv10: +2 AC and +2 to saves while concentrating; needs a compound
/// AC/save modifier gated on the Concentrating condition), and
/// **Deflecting Shroud** (lv14 capstone: Arcane Deflection now radiates
/// force damage to up to three enemies within 60ft; needs the base
/// reaction plus a burst hook). Only the lv2 Tactical Wit passive has
/// a mechanical surface on the CR-0.5 chassis that plugs cleanly into
/// the shared `ABILITY_MOD_INITIATIVE_BONUSES` cohort, so we ship that
/// half and leave the rest as future work — matching the way
/// `NECROMANCY_WIZARD_TEMPLATE` ships only the lv10 Inured to Undeath
/// passive half of its RAW School of Necromancy kit and
/// `TWILIGHT_CLERIC_TEMPLATE` ships only the lv1 Vigilant Blessing
/// passive half of its RAW Twilight Domain kit.
///
/// Ships on the CR-0.5 wizard chassis above the strict RAW lv2 gate
/// for the same reason `NECROMANCY_WIZARD_TEMPLATE` ships Inured to
/// Undeath (RAW lv10): class templates target a balanced playable
/// level, not lockstep PHB progression.
///
/// Glyph 'Σ' (uppercase Greek sigma) — evokes the war mage's tactical
/// summation / calculus that folds into the initiative math. Distinct
/// from baseline wizard 'M' (mage) and Necromancy 'N'; collides with
/// no other current PC template glyph.
pub static WAR_MAGIC_WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Wizard envelope wholesale
    // and layers on the one War Magic Arcane Tradition subclass feature
    // (`TACTICAL_WIT_TAG`: passive +INT-mod initiative bump; NOT
    // registered in `SHORT_REST_FEATURES` — RAW gates on a permanent
    // passive per XGtE text, matching the sibling Rakish Audacity /
    // Dread Ambusher entries on the `ABILITY_MOD_INITIATIVE_BONUSES`
    // cohort). The `..base.clone()` tail inside the helper picks up
    // every other field — stats, spell slots, save profs, the full
    // wizard cantrip / lv1-9 spell loadout, and the Arcane Recovery
    // feature — without an N-line field-by-field copy. Sibling helper
    // users on the "clone base + insert one tag" cross-class lane:
    // every tag-only Warlock Otherworldly Patron subclass (via
    // `subclass_warlock_template`), `LIFE_CLERIC_TEMPLATE`,
    // `NECROMANCY_WIZARD_TEMPLATE`, `TWILIGHT_CLERIC_TEMPLATE`,
    // `FORGE_CLERIC_TEMPLATE`, `SHADOW_MAGIC_SORCERER_TEMPLATE`,
    // `ABERRANT_MIND_SORCERER_TEMPLATE`, `DIVINE_SOUL_SORCERER_TEMPLATE`,
    // `LONG_DEATH_MONK_TEMPLATE`. Glyph 'Σ' — for the war mage's
    // tactical summation identity; distinct from baseline wizard 'M'
    // (mage) and Necromancy 'N'.
    WIZARD_TEMPLATE.with_subclass_tag(
        "War Magic Wizard",
        'Σ',
        crate::actions::class_features::TACTICAL_WIT_TAG,
    )
});

/// Abjuration Wizard — Arcane Tradition **School of Abjuration**
/// subclass build (PHB). Identical envelope to the baseline
/// `WIZARD_TEMPLATE` (INT-primary full-caster with the archmage-tier
/// spell loadout, Arcane Recovery for mid-encounter slot regen) with
/// one subclass feature layered on: **Arcane Ward** (Abjuration
/// subclass level 2, PHB) — the first abjuration spell of 1st level or
/// higher the abjurer casts weaves a personal damage-absorbing ward,
/// and every later abjuration cast tops it back up by twice the slot
/// level.
///
/// The signature "the abjurer spends the fight standing inside their
/// own spell" tell. Mechanically it's a **third HP pool**, distinct
/// from both of the engine's existing ones:
///   - Unlike **temp HP** it drains *first* (RAW: "the ward takes the
///     damage instead of you" resolves ahead of damage *to you*), it
///     survives at 0 rather than vanishing, and it refills from a
///     recurring in-combat source rather than a grant-the-larger-value
///     rule.
///   - Unlike **regeneration** it isn't a per-round trickle and can't
///     be suppressed by a damage type.
///
/// Both distinctions matter in play: a baseline Wizard's Shield /
/// Mage Armor casts buy AC and nothing else, while the same two casts
/// on this chassis also weave and then feed the ward.
///
/// `arcane_ward_base: 6` is the RAW "twice your wizard level" term for
/// a level-3 abjurer, and with the chassis's INT 16 (+3) the full ward
/// maximum works out to 9 points. The level term is chosen against the
/// wizard chassis's **body** rather than its slot table: the baseline
/// template pairs a level-17 archmage spell list (4/3/3/2/2/1/1/1/1)
/// with a 2d6+2 (~9 HP) frame, so the strict-RAW level-17 term
/// (2 × 17 = 34, +3 INT = 37) would quadruple the template's effective
/// hit points and swamp every other defensive lane in the engine —
/// Shield's +5 AC, Mirror Image's decoys, Blur's disadvantage, Mage
/// Armor's AC floor. At 9 points the ward roughly doubles the
/// abjurer's survivability, which is the proportion RAW actually
/// lands at low levels (a level-2 abjurer's 14 HP alongside a 7-point
/// ward) and is exactly the "sized for a balanced playable level, not
/// lockstep PHB progression" convention every other subclass template
/// here ships under.
///
/// The abjurer's own loadout feeds the ward without any extra pickups:
/// **Shield** (lv1 reaction) and **Mage Armor** (lv1) each weave-or-
/// recharge for 2, **Dispel Magic** (lv3) for 6, and **Banishment**
/// (lv4) for 8 — so the defensive half of the wizard's spell list
/// stops being purely preventative and starts paying into a pool.
///
/// Read at three chokepoints: `weave_or_recharge_arcane_ward` on the
/// actor (pool arithmetic), `EncounterInstance::trigger_arcane_ward` on
/// the shared post-cast trigger registry next to Wild Magic Surge and
/// the Heart of the Storm eruption (the form / recharge hook, gated on
/// `Action::school() == Some(SpellSchool::Abjuration)`), and
/// `take_typed_damage` on the actor (absorption, ahead of temp HP).
///
/// Distinct from the three sibling wizard-chassis templates:
/// `WIZARD_TEMPLATE` (subclass-less baseline, Arcane Recovery only),
/// `NECROMANCY_WIZARD_TEMPLATE` (School of Necromancy — passive
/// necrotic resistance), and `WAR_MAGIC_WIZARD_TEMPLATE` (School of War
/// Magic — passive +INT-mod initiative). The four cover four distinct
/// defensive axes: nothing, a typed damage halver, an initiative
/// augment, and a rechargeable absorption pool. RAW allows exactly one
/// Arcane Tradition pick per wizard, so the features never legally
/// co-occur on a single build.
///
/// **First template on the `arcane_ward_base` lane** — the field is 0
/// (feature off) for every other creature in the engine, so a
/// non-abjurer's damage path short-circuits on the first `min` against
/// an empty pool.
///
/// RAW's School of Abjuration picks up other features not shipped on
/// this template — **Abjuration Savant** (lv2: halve the gp/time cost
/// of copying abjuration spells into the spellbook; no combat
/// surface), **Projected Ward** (lv6: spend the ward to absorb damage
/// aimed at an ally within 30 ft; needs a reaction hook that can
/// redirect a resolved damage instance across actors), **Improved
/// Abjuration** (lv10: add the proficiency bonus to Counterspell /
/// Dispel Magic ability checks; the engine resolves both spells
/// without a contested check today), and **Spell Resistance** (lv14
/// capstone: advantage on saves against spells plus resistance to
/// their damage; needs a "was this save forced by a spell?" flag on
/// the save path). Only the lv2 Arcane Ward has a mechanical surface
/// that plugs cleanly into the existing damage pipeline, so we ship
/// that and leave the rest as future work — matching the way
/// `NECROMANCY_WIZARD_TEMPLATE` ships only Inured to Undeath and
/// `WAR_MAGIC_WIZARD_TEMPLATE` only Tactical Wit.
///
/// Glyph 'Θ' — a warded circle, evoking the ward the abjurer wraps
/// themselves in. Distinct from baseline wizard 'M' (mage), Necromancy
/// 'N', and War Magic 'Σ'; collides with no other template glyph in
/// the engine.
pub static ABJURATION_WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Struct-field subclass (not `with_subclass_tag`): Arcane Ward is
    // carried by the `arcane_ward_base` scalar rather than a feature
    // tag, because the pool needs a size and the tag set is a
    // `HashSet<&'static str>` with nowhere to put one. Same explicit
    // clone-and-override shape the other flag-bearing subclass
    // templates use (Storm Sorcerer, the paladin oaths, the War /
    // Light / Tempest clerics) rather than the tag-only helper lane.
    CreatureTemplate {
        name: "Abjuration Wizard",
        glyph: 'Θ',
        arcane_ward_base: 6,
        ..WIZARD_TEMPLATE.clone()
    }
});

/// Evocation Wizard — Arcane Tradition **School of Evocation** subclass
/// build (PHB). Identical envelope to the baseline `WIZARD_TEMPLATE`
/// (INT-primary full-caster with the archmage-tier spell loadout,
/// Arcane Recovery for mid-encounter slot regen) with the two
/// mechanically-surfaced School of Evocation features layered on:
///
///   - **Sculpt Spells** (subclass lv2) — `1 + spell level` allies
///     inside the evoker's own blast automatically succeed on their
///     save and take no damage.
///   - **Potent Cantrip** (subclass lv6) — a creature that succeeds on
///     its save against one of the evoker's cantrips still takes half
///     damage (but no rider). Note RAW's gate is *cantrip*, not
///     *evocation cantrip*, so it lifts the Poison Spray / Toll the
///     Dead / Mind Sliver pickups from other schools too.
///   - **Empowered Evocation** (subclass lv10) — +INT modifier to one
///     damage roll of any evocation spell.
///   - **Overchannel** (subclass lv14) — a free prime that makes the
///     next damaging spell of level 1-5 deal maximum damage, at the
///     cost of escalating necrotic backlash on every use after the
///     first before a long rest.
///
/// The signature "the evoker drops a Fireball on the melee and their
/// own front line walks out of it" tell. This is the template that
/// makes the baseline wizard's biggest liability go away: a
/// `WIZARD_TEMPLATE` holding Fireball / Lightning Bolt / Cone of Cold /
/// Ice Storm / Thunderwave / Burning Hands can only fire them where no
/// ally stands, which in a corridor-heavy generated map is often
/// nowhere. Sculpt Spells converts every one of those into a
/// friend-or-foe blast the evoker can drop on a melee scrum, and
/// Empowered Evocation pays a flat +3 on top of each. Potent Cantrip
/// covers the other end of the slot curve: once the evoker is out of
/// slots, their at-will damage stops being all-or-nothing.
///
/// Both features route through chokepoints that already existed for
/// the sorcerer's metamagic, which is what keeps the template's diff
/// small: Sculpt Spells joins Careful Spell on
/// `EncounterInstance::auto_pass_shielded_allies`, and Empowered
/// Evocation joins Empowered Spell on `roll_empowered_sum`. The two
/// pairs are deliberately not merged — each pair shares a lane but
/// differs on gate (consumable prime vs. always-on passive), scaling
/// axis (CHA-mod vs. `1 + spell level`; reroll-low-dice vs. flat +INT),
/// and applicability (any spell vs. evocation only) — and a caster
/// holding one of each stacks both.
///
/// Where the sibling wizard subclasses are passive one-liners, this is
/// the first wizard template whose features read the **cast context**
/// (`EncounterInstance::current_cast`): both gate on the school and
/// level of the spell being resolved, not on anything the caster is
/// holding, so they are inert on the evoker's non-evocation casts —
/// Sculpt Spells doesn't carve allies out of a Hypnotic Pattern
/// (enchantment), and Empowered Evocation adds nothing to a Vampiric
/// Touch (necromancy).
///
/// Distinct from the four sibling wizard-chassis templates:
/// `WIZARD_TEMPLATE` (subclass-less baseline), `NECROMANCY_WIZARD_TEMPLATE`
/// (passive necrotic resistance), `WAR_MAGIC_WIZARD_TEMPLATE` (passive
/// +INT-mod initiative), and `ABJURATION_WIZARD_TEMPLATE` (the
/// rechargeable Arcane Ward absorption pool). Five templates, five
/// distinct axes: nothing, a typed damage halver, an initiative
/// augment, an absorption pool, and an offensive blast-shaper. RAW
/// allows exactly one Arcane Tradition pick per wizard, so no two ever
/// legally co-occur on a single build.
///
/// This is the first subclass template in the engine to ship its RAW
/// feature set **complete** — all four School of Evocation features
/// have a mechanical surface here, where the sibling wizard traditions
/// ship one apiece (`NECROMANCY_WIZARD_TEMPLATE` only Inured to
/// Undeath, `WAR_MAGIC_WIZARD_TEMPLATE` only Tactical Wit). That fell
/// out of the chokepoints rather than from extra effort per feature:
/// once the cast context exists, three of the four are a gate plus a
/// few lines at a shared site that was already there.
///
/// Only Evocation Savant's out-of-combat spellbook-copying discount has
/// no surface, and it has no combat surface in RAW either.
///
/// Glyph 'Δ' — the evoker's raw elemental burst. Distinct from baseline
/// wizard 'M' (mage), Necromancy 'N', War Magic 'Σ', and Abjuration
/// 'Θ'; collides with no other template glyph in the engine.
pub static EVOCATION_WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Four-tag subclass, so not the single-tag `with_subclass_tag`
    // helper: clone the baseline feature set and insert every row.
    // Overchannel also contributes an action (the prime), which the
    // tag-only helper has no lane for either.
    let mut features = WIZARD_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::SCULPT_SPELLS_TAG);
    features.insert(crate::actions::class_features::POTENT_CANTRIP_TAG);
    features.insert(crate::actions::class_features::EMPOWERED_EVOCATION_TAG);
    features.insert(crate::actions::class_features::OVERCHANNEL_TAG);
    let mut actions = WIZARD_TEMPLATE.actions.clone();
    actions.push(&*crate::actions::class_features::OVERCHANNEL);
    CreatureTemplate {
        name: "Evocation Wizard",
        glyph: 'Δ',
        features,
        actions,
        ..WIZARD_TEMPLATE.clone()
    }
});

/// Divination Wizard — Arcane Tradition **School of Divination**
/// subclass build (PHB). Identical envelope to the baseline
/// `WIZARD_TEMPLATE` (INT-primary full-caster with the archmage-tier
/// spell loadout, Arcane Recovery for mid-encounter slot regen) with
/// the two mechanically-surfaced School of Divination features layered
/// on:
///
///   - **Portent / Greater Portent** (subclass lv2 / lv14) — a bank of
///     foretold d20 faces rolled once per long rest, any one of which
///     can replace an attack roll or saving throw made by the diviner
///     or a creature they can see.
///   - **Expert Divination** (subclass lv6) — casting a divination
///     spell of 2nd level or higher refunds one expended slot of a
///     lower level (never above 5th).
///   - **The Third Eye** (subclass lv10) — permanent See Invisibility.
///
/// Which is the RAW feature set **complete**, the second subclass in
/// the engine to ship that way after `EVOCATION_WIZARD_TEMPLATE`. Only
/// Divination Savant's out-of-combat spellbook-copying discount has no
/// surface, and it has no combat surface in RAW either.
///
/// The signature "the diviner already knows how this goes" tell. Where
/// every other wizard tradition changes what a spell *does*, Portent
/// changes what the dice do, and it does so on rolls the diviner is not
/// making: the boss's save against the party's one control spell, the
/// giant's swing at the downed healer, the ally's death save. It is the
/// first feature in the engine that reaches into another actor's d20.
///
/// **Portent ships at 3 dice (Greater Portent, subclass lv14)** rather
/// than the lv2 pair, matching the way the sibling
/// `EVOCATION_WIZARD_TEMPLATE` ships its own lv14 feature and the way
/// the shared wizard chassis carries a level-17 spell list: these
/// templates target a balanced playable build, not lockstep PHB
/// progression. The RAW lv2 version is the same template with
/// `portent_dice: 2` — the field *is* the count, so the level-14
/// upgrade needed no second flag.
///
/// Expert Divination has real fuel on this chassis without any extra
/// pickups: the baseline wizard already carries Mind Spike (lv2), True
/// Seeing (lv6) and Foresight (lv9), so three of the loadout's spells
/// clear the "divination, 2nd level or higher" gate and each refunds
/// the best expended slot at or below 5th.
///
/// The Third Eye collapses RAW's "action, once per short rest, pick one
/// of four benefits" to a permanent passive because three of the four
/// (Darkvision, Ethereal Sight, Greater Comprehension) have nothing to
/// act on at the resolution the engine models — it has no light level,
/// no Ethereal Plane and no written text. The fourth, See Invisibility,
/// lands in the `ConcealmentPiercing::Invisibility` tier rather than
/// being approximated as Truesight, so the diviner sees through an
/// `Invisible` opponent and stays fooled by `Blurred` / `Displaced`
/// exactly as RAW intends.
///
/// Distinct from the five sibling wizard-chassis templates:
/// `WIZARD_TEMPLATE` (subclass-less baseline),
/// `NECROMANCY_WIZARD_TEMPLATE` (passive necrotic resistance),
/// `WAR_MAGIC_WIZARD_TEMPLATE` (passive +INT-mod initiative),
/// `ABJURATION_WIZARD_TEMPLATE` (the rechargeable Arcane Ward
/// absorption pool), and `EVOCATION_WIZARD_TEMPLATE` (blast-shaping and
/// damage augmentation). Six templates, six distinct axes: nothing, a
/// typed damage halver, an initiative augment, an absorption pool, an
/// offensive blast-shaper, and a d20 substitution bank. RAW allows
/// exactly one Arcane Tradition pick per wizard, so no two ever legally
/// co-occur on a single build.
///
/// Glyph 'Ψ' — the diviner's third eye. Distinct from baseline wizard
/// 'M' (mage), Necromancy 'N', War Magic 'Σ', Abjuration 'Θ', and
/// Evocation 'Δ'; collides with no other template glyph in the engine.
pub static DIVINATION_WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Mixed tag-plus-struct-field subclass, so neither the tag-only
    // `with_subclass_tag` helper nor a bare field override fits:
    // Expert Divination is a membership tag, Portent is a scalar pool
    // size with nowhere to live in a `HashSet<&'static str>`. Same
    // explicit clone-and-override shape `EVOCATION_WIZARD_TEMPLATE`
    // uses for its four tags plus an action.
    let mut features = WIZARD_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::EXPERT_DIVINATION_TAG);
    features.insert(crate::actions::class_features::THIRD_EYE_TAG);
    CreatureTemplate {
        name: "Divination Wizard",
        glyph: 'Ψ',
        features,
        portent_dice: 3,
        ..WIZARD_TEMPLATE.clone()
    }
});

/// Enchantment Wizard — Arcane Tradition **School of Enchantment**
/// subclass build (PHB). Identical envelope to the baseline
/// `WIZARD_TEMPLATE` (INT-primary full-caster with the archmage-tier
/// spell loadout, Arcane Recovery for mid-encounter slot regen) with
/// the two mechanically-surfaced School of Enchantment features layered
/// on:
///
///   - **Hypnotic Gaze** (subclass lv2) — an action, no slot: an
///     adjacent creature makes a WIS save or is Charmed by the
///     enchanter and Incapacitated for a round.
///   - **Split Enchantment** (subclass lv6) — every single-target
///     enchantment of 1st level or higher also lands on a second
///     creature, free.
///
/// The signature "the enchanter takes two creatures out of the fight
/// with one slot" tell. The baseline wizard's control suite is deep and
/// almost entirely single-target — Hold Person, Hold Monster, Dominate
/// Person, Charm Person, Command, Suggestion, Tasha's Hideous Laughter,
/// Crown of Madness, Bestow Curse — and Split Enchantment doubles the
/// throughput of every one of them at no cost. It is the widest
/// single-feature swing on the wizard chassis: not a bigger number on
/// one target, but the same effect on twice as many.
///
/// Split Enchantment shares its target picker and its resolution path
/// with the Sorcerer's **Twinned Spell** metamagic, which is what keeps
/// the diff small — the doubling block in `Action::execute` already
/// existed. The two differ on price and scope in a way the shared code
/// keeps straight: Twinned Spell is a consumable prime charging
/// `max(1, level)` sorcery points and covering any single-target spell
/// *including cantrips*; Split Enchantment is free and always on but
/// only touches leveled enchantments. A caster holding both doubles
/// once, not twice — `execute` offers the paid prime first and falls
/// through to the free passive only if it didn't fire.
///
/// Hypnotic Gaze is the first slot-free lockdown on the wizard chassis,
/// and its price is positional rather than economic: an INT-caster with
/// a 2d6+2 frame has to be standing in melee reach of the thing it
/// wants to disable. It composes with the engine-wide Charmed
/// enforcement — the gazed creature can't attack the enchanter, on its
/// own turn or via an opportunity attack or a riposte — so the two
/// clauses of RAW's payload land as one coherent "out of the fight, and
/// specifically out of *your* fight".
///
/// Instinctive Charm (subclass lv10) and Alter Memories (lv14) are left
/// as future work. Alter Memories has no combat surface in RAW at all.
/// Instinctive Charm has one — a reaction that redirects an incoming
/// attack onto the attacker's nearest other creature — but the engine's
/// attack pipeline resolves against a target id fixed before the
/// reaction window opens, so redirecting mid-swing needs a re-targeting
/// chokepoint that no existing feature has asked for; approximating it
/// as a plain miss would lose the "your attacker hits their own ally"
/// clause that is the whole point of the feature.
///
/// Distinct from the six sibling wizard-chassis templates:
/// `WIZARD_TEMPLATE` (subclass-less baseline),
/// `NECROMANCY_WIZARD_TEMPLATE` (passive necrotic resistance),
/// `WAR_MAGIC_WIZARD_TEMPLATE` (passive +INT-mod initiative),
/// `ABJURATION_WIZARD_TEMPLATE` (the rechargeable Arcane Ward
/// absorption pool), `EVOCATION_WIZARD_TEMPLATE` (blast-shaping and
/// damage augmentation), and `DIVINATION_WIZARD_TEMPLATE` (the d20
/// substitution bank). Seven templates, seven distinct axes. RAW allows
/// exactly one Arcane Tradition pick per wizard, so no two ever legally
/// co-occur on a single build.
///
/// Glyph 'Φ' — the enchanter's hypnotic eye. Distinct from baseline
/// wizard 'M' (mage), Necromancy 'N', War Magic 'Σ', Abjuration 'Θ',
/// Evocation 'Δ', and Divination 'Ψ'; collides with no other template
/// glyph in the engine.
pub static ENCHANTMENT_WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Two-tag subclass, one of which also contributes an action, so
    // neither the tag-only `with_subclass_tag` helper nor a bare field
    // override fits — same explicit clone-and-override shape
    // `EVOCATION_WIZARD_TEMPLATE` uses.
    let mut features = WIZARD_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::HYPNOTIC_GAZE_TAG);
    features.insert(crate::actions::class_features::SPLIT_ENCHANTMENT_TAG);
    let mut actions = WIZARD_TEMPLATE.actions.clone();
    actions.push(&*crate::actions::class_features::HYPNOTIC_GAZE);
    CreatureTemplate {
        name: "Enchantment Wizard",
        glyph: 'Φ',
        features,
        actions,
        ..WIZARD_TEMPLATE.clone()
    }
});

/// Illusion Wizard — Arcane Tradition **School of Illusion** subclass
/// build (PHB). Identical envelope to the baseline `WIZARD_TEMPLATE`
/// (INT-primary full-caster with the archmage-tier spell loadout,
/// Arcane Recovery for mid-encounter slot regen) with the one
/// mechanically-surfaced School of Illusion feature layered on:
///
///   - **Illusory Self** (subclass lv10) — a reaction, once per short
///     rest: an attack that would hit the illusionist instead hits a
///     duplicate of them, and automatically misses.
///
/// The signature "the swing that should have killed you didn't touch
/// you" tell. Every other defensive feature on the caster chassis moves
/// a number — Shield's +5 AC, Blur's disadvantage, Uncanny Dodge's
/// halving, Arcane Ward's absorption pool. Illusory Self doesn't
/// negotiate with the number at all: the attack that landed is simply
/// declared not to have landed. On a 2d6+2-per-die frame with the
/// engine's lowest HP totals, one guaranteed no-sell per rest is worth
/// more than any of them, which is why it sits at subclass level 10.
///
/// It is also the only defense in the engine that erases a **critical
/// hit**. Mirror Image — the illusionist's other decoy, and one the
/// baseline wizard loadout already carries — explicitly cannot deflect
/// a crit, so the two compose into a layered screen with a natural
/// division of labour: the decoys soak the ordinary swings for free,
/// and the per-rest charge waits for the one that would otherwise be
/// doubled dice. The shared `EncounterInstance::attack_intercepted`
/// cohort encodes exactly that ordering.
///
/// The price is the reaction. On a chassis that also carries Shield,
/// Absorb Elements and Counterspell, one round's reaction is genuinely
/// contested, and Illusory Self spends it defensively without stopping
/// the *rest* of the attacker's turn the way Shield's +5 AC can.
///
/// The other three School of Illusion features are left out because
/// none has a combat surface in RAW. Improved Minor Illusion (lv2)
/// grants a cantrip that creates a sound or an image — the engine
/// models neither. Malleable Illusions (lv6) lets a standing illusion
/// be reshaped as an action, which needs a standing illusion the engine
/// doesn't track. Illusory Reality (lv14) makes one illusory object
/// briefly real; there are no illusory objects to promote. Illusory
/// Self is not a partial shipment of the subclass so much as the whole
/// of its combat-facing half.
///
/// Distinct from the seven sibling wizard-chassis templates:
/// `WIZARD_TEMPLATE` (subclass-less baseline),
/// `NECROMANCY_WIZARD_TEMPLATE` (passive necrotic resistance),
/// `WAR_MAGIC_WIZARD_TEMPLATE` (passive +INT-mod initiative),
/// `ABJURATION_WIZARD_TEMPLATE` (the rechargeable Arcane Ward
/// absorption pool), `EVOCATION_WIZARD_TEMPLATE` (blast-shaping and
/// damage augmentation), `DIVINATION_WIZARD_TEMPLATE` (the d20
/// substitution bank), and `ENCHANTMENT_WIZARD_TEMPLATE` (slot-free
/// lockdown plus free single-target doubling). Eight templates, eight
/// distinct axes — and this is the only one whose axis is a flat denial
/// of an attack that already connected. RAW allows exactly one Arcane
/// Tradition pick per wizard, so no two ever legally co-occur on a
/// single build.
///
/// Glyph 'Λ' — the illusionist's duplicate, two strokes meeting where
/// one figure stood. Distinct from baseline wizard 'M' (mage),
/// Necromancy 'N', War Magic 'Σ', Abjuration 'Θ', Evocation 'Δ',
/// Divination 'Ψ', and Enchantment 'Φ'; collides with no other template
/// glyph in the engine.
pub static ILLUSION_WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Tag-only subclass — Illusory Self needs no action surface (it
    // fires reactively at the shared interception chokepoint) and no
    // struct field (the once-per-short-rest cadence rides
    // `features_remaining` via the `SHORT_REST_FEATURES` registration),
    // so the cross-class `with_subclass_tag` helper is the exact fit.
    // First wizard-chassis user of the helper since
    // `WAR_MAGIC_WIZARD_TEMPLATE` — the four traditions in between all
    // needed either an extra action (Enchantment's Hypnotic Gaze,
    // Evocation's Overchannel) or a scalar field (Divination's
    // `portent_dice`) and stayed on the explicit clone-and-insert body.
    WIZARD_TEMPLATE.with_subclass_tag(
        "Illusion Wizard",
        'Λ',
        crate::actions::class_features::ILLUSORY_SELF_TAG,
    )
});

/// Conjuration Wizard — Arcane Tradition **School of Conjuration**
/// subclass build (PHB). Identical envelope to the baseline
/// `WIZARD_TEMPLATE` (INT-primary full-caster with the archmage-tier
/// spell loadout, Arcane Recovery for mid-encounter slot regen) with
/// the two mechanically-surfaced School of Conjuration features layered
/// on:
///
///   - **Benign Transposition** (subclass lv6) — an action, no slot:
///     teleport up to 30 ft. Recharges not on a rest but on the
///     conjurer's own casting, any conjuration of 1st level or higher.
///   - **Focused Conjuration** (subclass lv10) — while concentrating
///     on a conjuration spell, damage cannot break the concentration.
///
/// The signature "the cloud stays up" tell. The baseline wizard's
/// battlefield-control spells are overwhelmingly conjurations — Web,
/// Stinking Cloud, Cloudkill, Cloud of Daggers are all on the chassis
/// already — and all of them are concentration, which means their real
/// failure mode has never been the save DC. It's the archer who plinks
/// the wizard for 7 and rolls the cloud off the board. Focused
/// Conjuration deletes that failure mode outright for exactly those
/// spells and does nothing at all for Haste, Hold Monster or Greater
/// Invisibility. It is the most narrowly-scoped defensive feature on
/// the wizard chassis and, inside its scope, the most absolute: the
/// only unconditional concentration protection in the engine.
///
/// The two features compose better than they look. A conjurer who
/// leads with Web and then keeps casting conjurations is holding a
/// concentration nothing can shake *and* re-arming a free 30-ft blink
/// on every one of those casts — so the fragile INT-caster body that
/// has to stay alive to hold the cloud is also the one that can leave
/// any melee it finds itself in, repeatedly, for no slot. The
/// recharge condition is what ties them: both features pay out on the
/// same axis, and playing the school is what feeds them.
///
/// Benign Transposition's action cost keeps it from dominating the
/// Misty Step this chassis also carries. The blink is free and
/// renewable but costs the turn; Misty Step costs a 2nd-level slot but
/// leaves the action up to cast with. Neither is strictly better.
///
/// Minor Conjuration (subclass lv2) is left out — it conjures an
/// inanimate object of at most 10 lb, and the engine models no
/// object the wizard could produce or use. Durable Summons (lv14)
/// grants 30 temp HP to creatures the conjurer summons, and the engine
/// has no summoning surface for it to apply to; it would ship as a
/// tag nothing reads.
///
/// Distinct from the eight sibling wizard-chassis templates:
/// `WIZARD_TEMPLATE` (subclass-less baseline),
/// `NECROMANCY_WIZARD_TEMPLATE` (passive necrotic resistance),
/// `WAR_MAGIC_WIZARD_TEMPLATE` (passive +INT-mod initiative),
/// `ABJURATION_WIZARD_TEMPLATE` (the rechargeable Arcane Ward
/// absorption pool), `EVOCATION_WIZARD_TEMPLATE` (blast-shaping and
/// damage augmentation), `DIVINATION_WIZARD_TEMPLATE` (the d20
/// substitution bank), `ENCHANTMENT_WIZARD_TEMPLATE` (slot-free
/// lockdown plus free single-target doubling), and
/// `ILLUSION_WIZARD_TEMPLATE` (the per-rest auto-miss). Nine
/// templates, nine distinct axes — this is the only one whose axis is
/// keeping an effect that already landed *on the board*. RAW allows
/// exactly one Arcane Tradition pick per wizard, so no two ever
/// legally co-occur on a single build.
///
/// Glyph 'Γ' — the conjurer's gate, an opening with something on the
/// far side of it. Distinct from baseline wizard 'M' (mage),
/// Necromancy 'N', War Magic 'Σ', Abjuration 'Θ', Evocation 'Δ',
/// Divination 'Ψ', Enchantment 'Φ', and Illusion 'Λ'; collides with no
/// other template glyph in the engine.
pub static CONJURATION_WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Two-tag subclass, one of which also contributes an action, so
    // neither the tag-only `with_subclass_tag` helper nor a bare field
    // override fits — same explicit clone-and-override shape
    // `ENCHANTMENT_WIZARD_TEMPLATE` uses.
    let mut features = WIZARD_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::BENIGN_TRANSPOSITION_TAG);
    features.insert(crate::actions::class_features::FOCUSED_CONJURATION_TAG);
    let mut actions = WIZARD_TEMPLATE.actions.clone();
    actions.push(&*crate::actions::class_features::BENIGN_TRANSPOSITION);
    CreatureTemplate {
        name: "Conjuration Wizard",
        glyph: 'Γ',
        features,
        actions,
        ..WIZARD_TEMPLATE.clone()
    }
});

/// Transmutation Wizard — Arcane Tradition **School of Transmutation**
/// subclass build (PHB), and the **eighth and final** Arcane Tradition
/// on the wizard chassis. Identical envelope to the baseline
/// `WIZARD_TEMPLATE` (INT-primary full-caster with the archmage-tier
/// spell loadout, Arcane Recovery for mid-encounter slot regen) with
/// the two mechanically-surfaced School of Transmutation features
/// layered on:
///
///   - **Transmuter's Stone** (subclass lv6) — a carried stone granting
///     one of three benefits. This build attunes it to **Resilience**:
///     proficiency in Constitution saving throws.
///   - **Shapechanger** (subclass lv10) — an action, once per short
///     rest, no slot: cast Polymorph on yourself.
///
/// The signature "the wizard is harder to shift than a wizard should
/// be" tell. Both features answer the same question — what happens
/// when something finally connects with the d6-hit-die caster — and
/// they answer it at two different depths. The stone's Constitution
/// proficiency is the shallow, always-on answer: the damage-driven
/// concentration save is the one an unproficient wizard fails most, and
/// it is the one that costs them the spell they spent the turn on.
/// Shapechanger is the deep answer, and an expensive one: 30 temp HP
/// arrives roughly tripling the wizard's remaining margin, but the
/// beast form is itself a concentration, so the button that saves the
/// wizard is the button that drops whatever they were holding. "Keep
/// the Web up, or survive the round" is a real choice, and the stone
/// exists to make it come up less often.
///
/// Which is a deliberately different shape from the sibling
/// `CONJURATION_WIZARD_TEMPLATE`, the other tradition whose axis is
/// staying power: Focused Conjuration protects the *spell*
/// unconditionally within one school, where the stone protects the
/// *caster's roll* conditionally across all of them. A conjurer never
/// loses a Web; a transmuter loses fewer of everything.
///
/// **The stone's attunement is a template axis, not a runtime choice.**
/// RAW offers four benefits — darkvision, +10 ft speed, Constitution
/// save proficiency, or resistance to one of acid / cold / fire /
/// lightning / thunder — chosen on a long rest and changeable on any
/// levelled transmutation cast. Darkvision is dropped (the engine has
/// no light level for it to act on, the same reason the Diviner's
/// Third Eye drops its own darkvision option); the other three are
/// live, each read by the cohort that already owns its effect. A
/// Swiftness or Warding transmuter is this template with
/// `transmuters_stone` changed and nothing else, exactly the way
/// `portent_dice: 2` gives the subclass-level-2 Diviner and the way the
/// three Storm Herald Barbarians are three templates over one aura.
///
/// Resilience is the shipped attunement because it is the only one of
/// the three that improves a roll the wizard is already making every
/// round they hold a spell. Swiftness competes with a chassis that
/// carries Misty Step, and Warding is a bet on a damage type the build
/// can't know in advance.
///
/// Minor Alchemy (subclass lv2) and Master Transmuter (lv14) are left
/// out. Minor Alchemy transmutes one material into another over ten
/// minutes and has no combat surface in RAW at all. Master Transmuter
/// spends the stone on one of four out-of-combat effects — removing a
/// curse, restoring youth, creating a magic item, or a full-heal
/// panacea; only the last has any combat shape, and shipping it alone
/// would turn the capstone into "a second Shapechanger that heals",
/// which is not what the feature is.
///
/// Distinct from the nine sibling wizard-chassis templates:
/// `WIZARD_TEMPLATE` (subclass-less baseline),
/// `NECROMANCY_WIZARD_TEMPLATE` (passive necrotic resistance),
/// `WAR_MAGIC_WIZARD_TEMPLATE` (passive +INT-mod initiative),
/// `ABJURATION_WIZARD_TEMPLATE` (the rechargeable Arcane Ward
/// absorption pool), `EVOCATION_WIZARD_TEMPLATE` (blast-shaping and
/// damage augmentation), `DIVINATION_WIZARD_TEMPLATE` (the d20
/// substitution bank), `ENCHANTMENT_WIZARD_TEMPLATE` (slot-free
/// lockdown plus free single-target doubling), `ILLUSION_WIZARD_TEMPLATE`
/// (the per-rest auto-miss), and `CONJURATION_WIZARD_TEMPLATE`
/// (unbreakable conjuration concentration plus a self-recharging
/// blink). Ten templates, ten distinct axes, and with this one the
/// eight PHB Arcane Traditions are complete. RAW allows exactly one
/// Arcane Tradition pick per wizard, so no two ever legally co-occur on
/// a single build.
///
/// Glyph '◊' — the transmuter's stone. Distinct from baseline wizard 'M' (mage),
/// Necromancy 'N', War Magic 'Σ', Abjuration 'Θ', Evocation 'Δ',
/// Divination 'Ψ', Enchantment 'Φ', Illusion 'Λ', and Conjuration 'Γ'.
pub static TRANSMUTATION_WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Mixed tag-plus-action-plus-struct-field subclass — the widest of
    // the ten wizard templates on plumbing, and the reason neither the
    // tag-only `with_subclass_tag` helper nor a bare field override
    // fits: Shapechanger is a tag plus an action, Transmuter's Stone is
    // a tag plus an enum field with nowhere to live in a
    // `HashSet<&'static str>`.
    let mut features = WIZARD_TEMPLATE.features.clone();
    features.insert(crate::actions::class_features::TRANSMUTERS_STONE_TAG);
    features.insert(crate::actions::class_features::SHAPECHANGER_TAG);
    let mut actions = WIZARD_TEMPLATE.actions.clone();
    actions.push(&*crate::actions::class_features::SHAPECHANGER);
    CreatureTemplate {
        name: "Transmutation Wizard",
        glyph: '◊',
        features,
        actions,
        transmuters_stone: Some(
            crate::actors::actor_template::TransmutersStoneBenefit::Resilience,
        ),
        ..WIZARD_TEMPLATE.clone()
    }
});

/// Bladesinger Wizard — Arcane Tradition **Bladesinging** subclass build
/// (TCE), and the only wizard on the roster built to be hit. The other
/// ten answer melee by leaving it: Misty Step, Blink, Benign
/// Transposition, or the AI's kite rung. The Bladesinger answers it by
/// being harder to land a swing on than the fighter standing next to
/// them.
///
/// Three subclass features:
///
///   - **Bladesong** (lv2, bonus action, once per short rest): +INT to
///     AC, +10 ft speed, and +INT to the Constitution saves that keep
///     their concentration alive, for one minute. On the INT-16
///     chassis that is AC 15 and a +3 on the save that decides whether
///     the Haste they are holding survives the hit.
///
///   - **Extra Attack** (lv6): two swings per Attack action.
///
///   - **Song of Victory** (lv14): +INT to melee weapon damage while
///     the song is up — a `MELEE_CASTER_BUMPS` row, and the clause
///     that makes the shortsword worth swinging instead of casting a
///     cantrip.
///
/// The shortsword is the fourth piece and it is not a subclass feature
/// in RAW so much as a consequence of one: Bladesinging grants
/// proficiency with a one-handed melee weapon, and a wizard who has one
/// is a different creature from a wizard who doesn't. Every other
/// template here reaches for Fire Bolt when something closes; this one
/// reaches for steel, at DEX 14 with two attacks and +3 damage each.
///
/// What ties the four together is concentration. A Bladesinger in melee
/// is a Bladesinger about to be knocked out of their Haste, and the
/// subclass spends all three of its defensive clauses on that one
/// problem from different angles — the AC reduces how often a hit
/// lands, the speed lets them pick their ground, and the save bonus
/// reduces what a landed hit costs. That is why the concentration bonus
/// needed its own engine lane rather than riding
/// `condition_save_bonus`: RAW scopes it to concentration saves, and
/// granting it on every Constitution save would have handed the
/// squishiest chassis in the game a blanket poison / Cloudkill defense
/// it has no business having.
///
/// **Song of Defense** (lv10 — expend a spell slot as a reaction to
/// reduce damage by five times the slot level) isn't shipped: the
/// reactive-damage-clamp cohort takes a fixed reduction per row and has
/// no way to price one in slots at the moment the damage lands.
///
/// Glyph 'Ω' — the singer's open mouth. Distinct from baseline wizard
/// 'M', Necromancy 'N', War Magic 'Σ', Abjuration 'Θ', Evocation 'Δ',
/// Divination 'Ψ', Enchantment 'Φ', Illusion 'Λ', Conjuration 'Γ' and
/// Transmutation '◊'.
pub static BLADESINGER_WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{BLADESONG, BLADESONG_TAG};
    let mut actions = WIZARD_TEMPLATE.actions.clone();
    actions.push(&*BLADESONG);
    // DEX-based finesse blade — the shared `SHORTSWORD` static rather
    // than a bespoke one, since the Bladesinger's swing carries no
    // rider of its own. Song of Victory's +INT arrives through the
    // caster-side melee bump table, which every weapon on the roster
    // already reads.
    actions.push(&crate::actions::monster_attacks::SHORTSWORD);
    let mut features = WIZARD_TEMPLATE.features.clone();
    features.insert(BLADESONG_TAG);
    CreatureTemplate {
        name: "Bladesinger Wizard",
        glyph: 'Ω',
        // Bladesinging lv6. The flag rather than a tag because the
        // engine reads Extra Attack off a template field at the
        // action-economy site, same as it does for the Fighter and the
        // Paladin.
        has_extra_attack: true,
        actions,
        features,
        ..WIZARD_TEMPLATE.clone()
    }
});
