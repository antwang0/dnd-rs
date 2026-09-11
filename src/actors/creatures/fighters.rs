use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, COMMANDERS_STRIKE, COMMANDERS_STRIKE_TAG, DISARMING_ATTACK,
    DISARMING_ATTACK_TAG, DISTRACTING_ATTACK, DISTRACTING_ATTACK_TAG, ELDRITCH_STRIKE_TAG,
    ELEGANT_COURTIER_TAG, EVASIVE_FOOTWORK_TAG, FEINTING_ATTACK, FEINTING_ATTACK_TAG,
    FEROCIOUS_CHARGER, FEROCIOUS_CHARGER_TAG, GOADING_ATTACK, GOADING_ATTACK_TAG,
    INDOMITABLE, INDOMITABLE_TAG, LUNGING_ATTACK, LUNGING_ATTACK_TAG,
    MANEUVERING_ATTACK, MANEUVERING_ATTACK_TAG, MENACING_ATTACK,
    MENACING_ATTACK_TAG, PARRY_TAG, PRECISION_ATTACK, PRECISION_ATTACK_TAG, PROTECTIVE_FIELD_TAG,
    PSIONIC_STRIKE_TAG, PUSHING_ATTACK, PUSHING_ATTACK_TAG, RALLY, RALLY_TAG, RIPOSTE_TAG,
    SECOND_WIND, SECOND_WIND_TAG, SUPERIORITY_DICE_TAG, SURVIVOR_TAG,
    SWEEPING_ATTACK, SWEEPING_ATTACK_TAG, TRIP_ATTACK, TRIP_ATTACK_TAG, UNWAVERING_MARK_TAG,
    WAR_MAGIC_STRIKE, WAR_MAGIC_TAG, WARDING_MANEUVER_TAG, WEAPON_BOND_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGSWORD, SCIMITAR};
use crate::actions::spells::{BOOMING_BLADE, FIRE_BOLT, MAGIC_MISSILE, MISTY_STEP, SHIELD};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, Skill};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Champion Fighter — the PHB's most popular fighter subclass. This is a
/// level-5 build that exposes the headline subclass feature:
/// **Improved Critical** (crit on a d20 face of 19 or 20). Higher-level
/// loadouts would drop `crit_threshold` to 18 (Superior Critical at
/// level 15). Kept distinct from the baseline Fighter template so a
/// Champion-vs-Battle Master encounter can be set up by name.
///
/// Stats target a level-5 Champion: 44 HP (5d10+10), AC 18 (plate),
/// STR 18, CON 14, longsword + Action Surge / Second Wind / Indomitable
/// suite. Extra Attack is on (level 5+).
pub static CHAMPION_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LONGSWORD);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    actions.push(&*INDOMITABLE);
    CreatureTemplate {
        name: "Champion",
        glyph: 'C',
        ac: 18,
        hitpoints: "5d10+10".parse().unwrap(),
        strength: 18,
        dexterity: 12,
        constitution: 14,
        intelligence: 10,
        wisdom: 11,
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        proficient_saves: HashSet::from([AbilityScoreType::Strength, AbilityScoreType::Constitution]),
        // Champion features layered onto the standard fighter rest pool:
        //   - SECOND_WIND_TAG / ACTION_SURGE_TAG / INDOMITABLE_TAG: shared
        //     fighter base.
        //   - SURVIVOR_TAG (level 18): passive at-start-of-turn regen
        //     while at or below half max HP — the capstone "I will not
        //     die" envelope, read at `reset_for_new_round`. Ships on the
        //     level-5 Champion template above its strict RAW gate for
        //     the same reason Relentless Rage rides the level-9
        //     Barbarian and Improved Divine Smite rides the level-3
        //     Paladin: class templates target a balanced playable
        //     level, not lockstep PHB progression.
        //   - BOON_OF_COMBAT_PROWESS_TAG: SRD 5.2's Epic Boon feat, and
        //     the Champion is who it belongs to. The subclass's whole
        //     identity is that its attack rolls land more often than
        //     anybody else's — a crit range widened to 19, and now one
        //     miss a turn that simply isn't one. See
        //     `crate::actions::feats::BOON_OF_COMBAT_PROWESS_TAG`.
        features: HashSet::from([
            SECOND_WIND_TAG,
            ACTION_SURGE_TAG,
            INDOMITABLE_TAG,
            SURVIVOR_TAG,
            crate::actions::feats::BOON_OF_COMBAT_PROWESS_TAG,
        ]),
        has_extra_attack: true,
        // 5e Champion subclass level-3 feature: critical hits trigger on
        // 19 or 20 instead of just 20. Read at every attack-roll site
        // via `actor.crit_threshold()`.
        crit_threshold: 19,
        // 5e Champion subclass level-7 feature — **Remarkable Athlete**.
        // Passive: add half of the holder's proficiency bonus (rounded
        // up) to any STR, DEX, or CON check that doesn't already
        // include the proficiency bonus. In this engine the only
        // STR/DEX/CON check with a combat surface is the initiative
        // roll (a DEX check RAW) — the grant collapses to
        // "+ceil(prof / 2) on initiative rolls", read at
        // `roll_initiative` via `initiative_flat_bonus`. Ships on the
        // CR-3 (level-5) Champion template above its strict RAW lv7
        // level gate for the same reason Survivor (lv18) ships on the
        // same chassis — class templates target a balanced playable
        // level, not lockstep PHB progression. Composes cleanly with
        // Improved Critical (the Champion's headline lv3 tell) — a
        // Champion who wins initiative reliably opens the round with
        // the 19-20 crit threshold in play before the enemy's first
        // swing.
        has_remarkable_athlete: true,
        // 5e Champion Fighter **Superior Critical** (subclass level 15).
        // Passive: critical hits trigger on 18-20 instead of the Improved
        // Critical 19-20 window. `crit_threshold()` caps the returned
        // value at 18 whenever this flag is set, so the field-level
        // `crit_threshold: 19` above transparently drops to 18 for every
        // attack-roll site (weapon + spell) without a second field mutation.
        // Ships on the CR-3 (level-5) Champion template above its strict
        // RAW lv15 gate for the same reason Survivor (lv18) and
        // Remarkable Athlete (lv7) already ride here — class templates
        // target a balanced playable level, not lockstep PHB progression.
        // Composes cleanly with Improved Critical (the Champion's
        // headline lv3 tell — same field, just widened) and Brutal
        // Critical (barbarian rider on `brutal_critical_dice` — a
        // hypothetical Champion / Barbarian multiclass adds the extra
        // die on top of every 18-20 crit).
        has_superior_critical: true,
        // 5e Fighter **Fighting Style: Defense** (lv1 pick): passive +1 AC
        // while wearing armor. RAW "while wearing armor" gate collapses
        // to "always on" since the engine doesn't model armor tiers —
        // the Champion's plate baseline AC 18 becomes 19 with the style
        // pick, folding into `armor_class` next to the item / condition
        // AC lanes. Composes cleanly with the +1 template AC lane on
        // the Champion's plate-wearing defensive profile.
        has_defense_style: true,
        // 5e Fighter **Fighting Style: Dueling** (lv1 pick, second-style
        // pickup at fighter level 10): passive +2 to melee weapon damage.
        // Inherited to keep the Champion's per-swing damage floor aligned
        // with the baseline Fighter — the two templates should differ
        // ONLY on the Champion-specific `crit_threshold: 19` capstone
        // and the Survivor regen, not on background style picks. Same
        // "class templates ship above their strict RAW gate" reasoning
        // that ships Survivor (lv18) here — a level-5 build wouldn't
        // RAW-legally hold two styles, but a level-10+ Champion would.
        has_dueling_style: true,
        skills: HashSet::from([Skill::Athletics, Skill::Perception]),
        // 5e (2024 / SRD 5.2) **Weapon Mastery** — the level-1 class
        // feature of all five martial classes, and the switch that
        // turns on the mastery property printed beside every weapon in
        // this template's kit. Inherited by every subclass template in
        // this file through its `..BASE.clone()` tail, which is why it
        // is set once on the chassis rather than at each subclass.
        has_weapon_mastery: true,
        ..CreatureTemplate::defaults()
    }
});

/// Fighter — the simplest player class. Heavy armor, decent HP, one
/// martial weapon (scimitar — STR-based slashing) and the standard
/// movement actions. No spells. The headline distinction from monsters
/// is `rolls_death_saves: true` — at 0 HP a Fighter enters the dying
/// state and rolls saves on each of their turns instead of dropping
/// outright.
///
/// Stats are roughly a level-3 fighter: 24 HP (3d10+6), AC 16 from
/// chain mail, STR 16 (the standard "strength build" defaults).
pub static FIGHTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    actions.push(&*INDOMITABLE);
    // Battle Master maneuvers (once per short rest each in our model).
    // Bonus-action primes that ride the next melee hit:
    //   - Trip Attack: STR save vs prone (knockdown sets up advantage).
    //   - Menacing Attack: WIS save vs frighten (one-round disadv).
    //   - Disarming Attack: STR save vs disarmed (one-round disadv).
    //   - Pushing Attack: STR save vs forced shove (4-tile push).
    //   - Goading Attack: WIS save vs goaded (the tank-anchor maneuver:
    //     target eats disadvantage on attacks against anyone other than
    //     the fighter, mirroring Compelled Duel without concentration).
    // Each maneuver is its own per-rest charge so the AI can pick the
    // right tool per fight (frighten a caster, shove a melee threat
    // away from the squishy ally, etc.).
    actions.push(&*TRIP_ATTACK);
    actions.push(&*MENACING_ATTACK);
    actions.push(&*DISARMING_ATTACK);
    actions.push(&*PUSHING_ATTACK);
    actions.push(&*GOADING_ATTACK);
    //   - Precision Attack: flat +4 to next attack roll (single-shot,
    //     consumed by the first swing this turn). The accuracy maneuver
    //     — pairs well with Action Surge for a guaranteed crit chance.
    //   - Sweeping Attack: prime that splashes 1d8 slashing onto one
    //     adjacent enemy of the primary target on hit (the cleave
    //     maneuver — solid AoE tax in crowded fights).
    //   - Feinting Attack: targeted bonus action; grants self advantage
    //     on the next attack against the feinted enemy (the duelist's
    //     "guaranteed land" tool — pairs with Smite spells or sneak-
    //     attack riders so the burst doesn't whiff).
    actions.push(&*PRECISION_ATTACK);
    actions.push(&*SWEEPING_ATTACK);
    actions.push(&*FEINTING_ATTACK);
    //   - Lunging Attack: +5ft reach prime (one extra tile in this grid)
    //     for the next melee swing. The skirmisher's gap-closer —
    //     stretches the threat zone so the fighter can lash adjacent-1
    //     enemies (Burning Hands range without committing the move).
    //   - Rally: bonus-action ally-buff dispenser. Hands a chosen
    //     friendly creature `1d10 + CHA` temp HP — a flat absorb
    //     buffer that doesn't compete with healing spells (temp HP
    //     stacks-and-replaces rather than topping off the HP bar).
    //   - Commander's Strike: long-range buff/reaction grant. The
    //     fighter spends a bonus action ordering an ally to attack
    //     with advantage (engine consumes the ally's reaction slot
    //     for the swing). The "team buff" maneuver — pairs cleanly
    //     with a high-damage rogue or paladin teammate.
    actions.push(&*LUNGING_ATTACK);
    actions.push(&*RALLY);
    actions.push(&*COMMANDERS_STRIKE);
    //   - Distracting Strike: bonus action prime that adds +1d6 damage
    //     to the next melee swing and tags the target Distracted —
    //     allies attacking the same target get advantage until the
    //     fighter's next turn. The "set up the rogue" maneuver — pairs
    //     cleanly with Sneak Attack riders or Smite spells from a
    //     follow-up ally swing.
    actions.push(&*DISTRACTING_ATTACK);
    //   - Maneuvering Attack: bonus action prime that adds a superiority
    //     die to the next melee hit and hands one ally a free
    //     half-speed reposition off its reaction. The last of RAW's
    //     sixteen to arrive, and the only one whose rider lands on
    //     somebody friendly — the fighter's die pays for the rogue
    //     getting into flanking position, or for the wizard stepping
    //     out of the ogre's reach.
    actions.push(&*MANEUVERING_ATTACK);
    CreatureTemplate {
        name: "Fighter",
        glyph: 'F',
        ac: 16,
        hitpoints: "3d10+6".parse().unwrap(),
        strength: 16,
        dexterity: 12,
        constitution: 14,
        intelligence: 10,
        wisdom: 11,
        charisma: 10,
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        // Fighters are proficient in STR and CON saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Strength, AbilityScoreType::Constitution]),
        features: HashSet::from([
            SECOND_WIND_TAG,
            ACTION_SURGE_TAG,
            INDOMITABLE_TAG,
            TRIP_ATTACK_TAG,
            MENACING_ATTACK_TAG,
            DISARMING_ATTACK_TAG,
            PUSHING_ATTACK_TAG,
            GOADING_ATTACK_TAG,
            PRECISION_ATTACK_TAG,
            SWEEPING_ATTACK_TAG,
            FEINTING_ATTACK_TAG,
            LUNGING_ATTACK_TAG,
            RALLY_TAG,
            COMMANDERS_STRIKE_TAG,
            DISTRACTING_ATTACK_TAG,
            MANEUVERING_ATTACK_TAG,
            // 5e Battle Master **Evasive Footwork** — the one maneuver
            // in the suite with no action attached to it. RAW's trigger
            // is the fighter's own movement, so there is nothing for the
            // action list to offer and nothing for the AI to pick; it
            // fires from the opportunity-attack dispatcher at the moment
            // a swing is certain. Tag-only for exactly that reason —
            // see `EVASIVE_FOOTWORK_TAG`.
            EVASIVE_FOOTWORK_TAG,
            // Reactive maneuvers — no active action to spend, fire
            // automatically at the melee-attack chokepoint. Parry
            // (1d8 + DEX damage reducer on hit) and Riposte (counter-
            // attack on miss) each burn one superiority die + a reaction
            // when they land, giving the fighter a defensive lane the
            // active-only maneuvers above don't cover — and competing
            // for the same four dice, so a fighter who opened with three
            // primes has one Parry left in the tank.
            PARRY_TAG,
            RIPOSTE_TAG,
            // The pool all sixteen tags above spend from: four d8s,
            // back on a short rest. Without this row every maneuver
            // falls back to a private charge of its own, which is
            // sixteen uses per rest instead of four — see
            // `SHARED_FEATURE_POOLS`.
            SUPERIORITY_DICE_TAG,
        ]),
        // 5e Fighter Battle Master reactive maneuvers. The `has_parry`
        // flag opts into the `1d8 + DEX` melee-damage reducer at the
        // resolve-attack chokepoint (sibling to Uncanny Dodge / Deflect
        // Missiles); `has_riposte` opts into the counter-attack-on-miss
        // hook right after the miss log line. Both are separately gated
        // by their per-rest feature charge (`PARRY_TAG` / `RIPOSTE_TAG`)
        // so a fighter with the flag but no charge left simply eats
        // damage / misses without firing.
        has_parry: true,
        has_riposte: true,
        has_extra_attack: true,
        // 5e Fighter **Fighting Style: Dueling** (lv1 pick): passive +2 to
        // damage rolls on melee weapon attacks. RAW "while wielding a
        // one-handed weapon and no other weapon" gate collapses to
        // "melee weapon attack only" since the engine doesn't track
        // weapon-hand-usage. The baseline Fighter ships this style since
        // the scimitar is a one-handed simple melee weapon — dueling
        // applies naturally without the two-handed / dual-wielding
        // exclusions. Distinct from Champion (Defense: +1 AC) — the
        // fighter's baseline lacks the Champion's plate baseline so the
        // damage-side style buys more damage per swing than +1 AC would
        // buy in AC on a chain-mail chassis.
        has_dueling_style: true,
        skills: HashSet::from([Skill::Athletics, Skill::Perception]),
        // 5e (2024 / SRD 5.2) **Weapon Mastery** — the level-1 class
        // feature of all five martial classes, and the switch that
        // turns on the mastery property printed beside every weapon in
        // this template's kit. Inherited by every subclass template in
        // this file through its `..BASE.clone()` tail, which is why it
        // is set once on the chassis rather than at each subclass.
        has_weapon_mastery: true,
        ..CreatureTemplate::defaults()
    }
});

/// Samurai Fighter — Martial Archetype **Samurai** subclass build (XGtE).
/// Identical envelope to the baseline `FIGHTER_TEMPLATE` (level-3 build,
/// STR-primary martial, scimitar + Second Wind / Action Surge /
/// Indomitable / the full Battle Master maneuver suite, chain-mail AC 16,
/// Dueling fighting style) with one subclass feature layered on:
/// **Elegant Courtier** (lv7 subclass tell) — passive **proficiency in
/// Wisdom saving throws**.
///
/// The Samurai's signature "the disciplined warrior's poise steadies the
/// mind against mind-affecting magic" tell — where a baseline Fighter's
/// WIS save relies on the WIS 11 mod alone plus no proficiency, the
/// Samurai adds the proficiency bonus on every WIS-save chokepoint
/// (against Charm Person, Hold Person, Dominate Person, Suggestion,
/// Fear, Command, Sanctuary — every mind-affecting effect that
/// thematically targets the Samurai's steel-focused mind). Composes
/// cleanly with the fighter chassis's baseline STR / CON proficiency
/// set — the Samurai now covers three of the six save axes at
/// proficient (STR / CON / WIS), leaving only DEX / INT / CHA
/// unimproved for the shared level-3 Fighter build.
///
/// The Samurai's WIS-save-proficiency-flavored sibling to the other
/// Fighter subclasses:
///   - **Champion** (`CHAMPION_TEMPLATE`): Improved Critical (crit-on-19)
///     + Superior Critical (crit-on-18) + Remarkable Athlete (half-prof
///       initiative bump) + Defense / Dueling Fighting Styles + Survivor.
///       The "spike-damage / durable" archetype.
///   - **Baseline Fighter** (`FIGHTER_TEMPLATE`): all Battle Master
///     maneuvers (Trip / Menacing / Disarming / Pushing / Goading /
///     Precision / Sweeping / Feinting / Lunging / Rally /
///     Commander's Strike / Distracting / Maneuvering) + Parry +
///     Riposte + Evasive Footwork + Dueling Style. The "tactical /
///     versatile" archetype.
///   - **Samurai** (`SAMURAI_FIGHTER_TEMPLATE`): Elegant Courtier
///     WIS-save proficiency. The "disciplined / mind-hardened"
///     archetype — inherits the baseline Fighter's Battle Master
///     maneuvers via `..FIGHTER_TEMPLATE.clone()` through the shared
///     `with_subclass_tag` helper so the Samurai still has a full
///     tactical toolkit but leans additionally into the WIS-save
///     defensive lane.
///
/// Where the Champion leans on crit-threshold spikes and always-on
/// Survivor regen, the Samurai leans on the always-on WIS-save
/// proficiency — no charge to spend, no bonus action to prime, no
/// target to pick. Sibling on the "passive WIS-save proficiency as a
/// subclass tell" cross-class lane to `has_slippery_mind` (Rogue lv15
/// class capstone) and `has_iron_mind` (Gloom Stalker Ranger lv7 /
/// Zealot Barbarian lv7 subclass features) — three prior sources on
/// the same save-proficiency axis, all distinct build slots, all
/// promoting the same ability to "proficient". The four never legally
/// co-occur on a single build (Rogue vs. Ranger vs. Barbarian vs.
/// Fighter subclass slots), and a hypothetical multiclass carrier
/// picks up the proficiency via any single row under the shared
/// `FLAG_DRIVEN_SAVE_PROFICIENCIES` cohort's "any row hit is
/// sufficient" OR semantic.
///
/// Read at the shared `FLAG_DRIVEN_SAVE_PROFICIENCIES` cohort in
/// `actor_template.rs` next to Slippery Mind / Iron Mind — one lookup
/// table, one source of truth. The tag-closure row shape
/// (`|a| a.has_passive_feature(ELEGANT_COURTIER_TAG)`) matches the
/// two struct-field-flag sibling rows on the same cohort by promoting
/// the same ability to proficient without adding a new
/// `has_elegant_courtier` field to `ActorInstance` — the "tag-only
/// cross-class helper" pattern (`with_subclass_tag`) keeps the
/// subclass template one-line and the cohort row one-liner.
///
/// RAW's Samurai Fighter picks up other features not shipped on this
/// template — **Bonus Proficiency** (lv3: one skill or language;
/// ribbon on out-of-combat social checks with no engine surface),
/// **Fighting Spirit** (lv3: 3-per-long-rest bonus action for
/// +5/10/15 temp HP AND advantage on weapon attacks until end of
/// turn; needs a per-turn advantage-marker plus a temp HP grant
/// chained to a bonus-action prime — future work behind a
/// `FightingSpirit` action surface), **Tireless Spirit** (lv10:
/// refresh Fighting Spirit at initiative-roll time if none left;
/// needs a per-encounter refresh tick), **Rapid Strike** (lv15:
/// trade advantage for extra attack; needs an advantage-consumption
/// + bonus-attack hook), and **Strength Before Death** (lv18
///   capstone: reaction to take a full turn on being reduced to 0 HP;
///   needs a dying-transition reaction hook). Only the lv7 Elegant
///   Courtier passive has a mechanical surface on the CR-1 chassis
///   that plugs cleanly into the shared `FLAG_DRIVEN_SAVE_PROFICIENCIES`
///   cohort, so we ship that half and leave the rest as future work —
///   matching the way `CHAMPION_TEMPLATE` ships the lv3 / lv7 / lv15
///   / lv18 passive Champion features but leaves the reactive Battle
///   Master lane on the baseline `FIGHTER_TEMPLATE` and the way every
///   other tag-only subclass template pares down to the load-bearing
///   passive half of its RAW subclass kit.
///
/// Ships on the CR-1 (level-3) fighter chassis at (or above) its
/// strict RAW lv7 gate for the same reason `WATCHERS_PALADIN_TEMPLATE`
/// ships Aura of the Sentinel (RAW lv7), `GLORY_PALADIN_TEMPLATE`
/// ships Aura of Alacrity (RAW lv7), `NECROMANCY_WIZARD_TEMPLATE`
/// ships Inured to Undeath (RAW lv10), and every other subclass
/// template runs above its strict RAW gate — class templates target
/// a balanced playable level, not lockstep PHB progression.
///
/// Distinct from `FIGHTER_TEMPLATE` (Archetype-less baseline —
/// Battle Master maneuvers only) and `CHAMPION_TEMPLATE` (Champion
/// Archetype — Improved / Superior Critical spike lane) so a
/// Samurai-vs-Champion / Samurai-vs-Fighter encounter renders
/// unambiguously by name.
///
/// Glyph 'S' — for "Samurai" and evokes the katana's curved blade
/// silhouette. Distinct from baseline Fighter 'F' and Champion 'C'.
/// Collides with several NPC creature templates (Sphinx / Satyr /
/// Specter / Skeleton / Stirge / Swarm) but the team-color-and-team-
/// id combo disambiguates them in a mixed encounter — same overlap
/// policy the other PC subclass glyphs already follow (Scout Rogue
/// 'K' shares with Killer Whale, Assassin 'A' shares with Ape /
/// Awakened Shrub, etc.). Also distinct from Tempest Cleric 'S' —
/// the two never legally co-occur on a single team-color-and-team-id
/// combo (different classes).
pub static SAMURAI_FIGHTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Subclass-of pattern via the shared `CreatureTemplate::with_subclass_tag`
    // cross-class helper — clones the baseline Fighter envelope wholesale
    // and layers on the Elegant Courtier passive tag. The `..base.clone()`
    // tail inside the helper picks up every other field — chain-mail AC
    // 16, HP 24 (3d10+6), STR 16, all Battle Master maneuvers (Trip /
    // Menacing / Disarming / Pushing / Goading / Precision / Sweeping /
    // Feinting / Lunging / Rally / Commander's Strike / Distracting /
    // Maneuvering) + Parry + Riposte + Evasive Footwork + Dueling
    // Style — without an N-line field-by-
    // field copy. No new actions are pushed — Elegant Courtier is a
    // purely passive WIS-save-proficiency grant read at the shared
    // `FLAG_DRIVEN_SAVE_PROFICIENCIES` cohort, not a fresh action
    // surface, so the "tag-only" shape the helper wraps is a natural
    // fit. First fighter-chassis user of the `with_subclass_tag` cross-
    // class helper — Champion is a from-scratch template rather than a
    // subclass-of clone (it swaps AC / HP / weapon / features
    // significantly from the baseline Fighter). Sibling helper users on
    // the "clone base + insert one tag" cross-class lane: every tag-
    // only Warlock Otherworldly Patron subclass (via
    // `subclass_warlock_template`), `LIFE_CLERIC_TEMPLATE`,
    // `FORGE_CLERIC_TEMPLATE`, `TWILIGHT_CLERIC_TEMPLATE`,
    // `NECROMANCY_WIZARD_TEMPLATE`, `WAR_MAGIC_WIZARD_TEMPLATE`,
    // `SHADOW_MAGIC_SORCERER_TEMPLATE`, `ABERRANT_MIND_SORCERER_TEMPLATE`,
    // `DIVINE_SOUL_SORCERER_TEMPLATE`, `LONG_DEATH_MONK_TEMPLATE`,
    // `GLORY_PALADIN_TEMPLATE`, `WATCHERS_PALADIN_TEMPLATE`,
    // `FEY_WANDERER_RANGER_TEMPLATE`.
    FIGHTER_TEMPLATE.with_subclass_tag("Samurai Fighter", 'S', ELEGANT_COURTIER_TAG)
});

/// Eldritch Knight Fighter — Martial Archetype **Eldritch Knight**
/// subclass build (PHB), and with it every PHB Martial Archetype has a
/// build in the engine: Champion (`CHAMPION_TEMPLATE`), Battle Master
/// (the baseline `FIGHTER_TEMPLATE`, which carries the full maneuver
/// suite), and now the third.
///
/// The first fighter chassis in the engine that casts. Four subclass
/// features, all shipped:
///
///   - **Weapon Bond** (lv3) — the bonded blade can't be knocked away.
///     Conditional immunity to `Disarmed`, gated on the knight not
///     being incapacitated.
///   - **War Magic** (lv7) — cast a cantrip with your Action, then take
///     a weapon swing as a bonus action.
///   - **Eldritch Strike** (lv10) — a connecting weapon hit gives the
///     target disadvantage on its next save against a spell *you* cast.
///   - **Arcane Charge** (lv15) — Action Surge also teleports you 30 ft.
///
/// The build's whole argument is that its two halves are worth less
/// apart than together, and both of the mid-tier features say so from
/// opposite directions. War Magic pays the knight for casting *before*
/// swinging; Eldritch Strike pays them for swinging *before* casting.
/// Neither ordering satisfies both in one turn — the swing War Magic
/// buys arrives after the cantrip that armed it, and the spell Eldritch
/// Strike sharpens arrives after the swing that marked the target — so
/// the knight who wants both alternates across turns: cantrip-and-swing
/// on turn N leaves a mark that turn N+1's levelled spell cashes, and
/// that spell's landing sets up the next cantrip. A knight who commits
/// to one half plays a strictly worse Champion or a strictly worse
/// wizard.
///
/// Which makes this a different answer to "martial plus X" than the two
/// siblings on the chassis give. The Battle Master's maneuvers are all
/// spent from one pool on one turn's swing; the Champion's passives ask
/// nothing at all. The Eldritch Knight is the only fighter build whose
/// features are sequenced — they reward the *order* of turns rather
/// than the contents of one.
///
/// **The spell list is deliberately narrow.** RAW restricts the
/// Eldritch Knight to abjuration and evocation (plus two free picks),
/// which is a flavor rule that also happens to be a balance rule: the
/// knight gets no lockdown, no summons, and no save-or-suck. We honor
/// it as-written rather than as a suggestion. Cantrips are Fire Bolt
/// (the reach option — a knight pinned at range still has a turn) and
/// Booming Blade (which routes through the weapon-attack chokepoint, so
/// it arms Eldritch Strike *and* counts as a cantrip for War Magic —
/// the one cast that satisfies both features at once, and the reason it
/// is on the list). Level 1 is Shield (the reaction AC spike that makes
/// the d10 chassis genuinely hard to hit) and Magic Missile (the
/// no-roll damage floor for a turn where the swing is out of reach).
/// Level 2 is Misty Step, the escape hatch a plate-armored caster
/// otherwise lacks.
///
/// Absorb Elements and Shatter would both be RAW-legal and are left
/// out: at `[4, 3]` slots the knight is choosing between Shield and
/// everything else on most turns, and a fifth and sixth option would
/// dilute a decision that is currently sharp.
///
/// **Stats.** Third-caster slots `[4, 3]` (4× level-1, 3× level-2) put
/// this at roughly fighter level 10-13, which is also where War Magic
/// and Eldritch Strike come online — the first subclass template on the
/// chassis whose shipped features actually match its slot table rather
/// than running above their RAW gate. INT rises to 14 from the
/// baseline's 10 to anchor the spell save DC and Booming Blade's rider;
/// STR stays 16 so the knight is a fighter first. HP and AC inherit the
/// baseline (24 HP / AC 16 chain mail) via `..FIGHTER_TEMPLATE.clone()`,
/// as do every Battle Master maneuver, Second Wind / Action Surge /
/// Indomitable, Parry / Riposte, Extra Attack and the Dueling style —
/// RAW-illegally, since a real Eldritch Knight doesn't get maneuvers,
/// but consistent with how `SAMURAI_FIGHTER_TEMPLATE` inherits the same
/// suite and with the engine-wide convention that a subclass template
/// is the baseline chassis plus its subclass tell.
///
/// Arcane Charge (lv15) is the one feature left as future work: the
/// teleport has to fire *as part of* Action Surge rather than as its
/// own action, and the engine's `ActionSurge` impl has no post-grant
/// hook to hang a destination picker on. Adding one for a single
/// consumer would mean either a blind auto-teleport (which can strand
/// the knight away from the enemy it just surged to reach) or a prompt
/// channel the AI can't answer — the same reasoning that leaves the
/// Transmuter's stone re-attunement and the Samurai's Rapid Strike out.
///
/// Glyph 'E' — for **E**ldritch Knight. Distinct from baseline Fighter
/// 'F', Champion 'C' and Samurai 'S'.
pub static ELDRITCH_KNIGHT_FIGHTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Not the `with_subclass_tag` one-liner the Samurai uses: this
    // subclass adds actions (four spells plus the War Magic bonus
    // action), three tags rather than one, a slot table, and an INT
    // bump. The `..FIGHTER_TEMPLATE.clone()` tail still picks up
    // everything else — AC, HP, STR, every maneuver, Parry / Riposte,
    // Extra Attack, Dueling style — without an N-line field-by-field
    // copy.
    let mut actions = FIGHTER_TEMPLATE.actions.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*BOOMING_BLADE);
    actions.push(&*SHIELD);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*MISTY_STEP);
    actions.push(&*WAR_MAGIC_STRIKE);
    let mut features = FIGHTER_TEMPLATE.features.clone();
    features.insert(WEAPON_BOND_TAG);
    features.insert(WAR_MAGIC_TAG);
    features.insert(ELDRITCH_STRIKE_TAG);
    CreatureTemplate {
        name: "Eldritch Knight",
        glyph: 'E',
        // INT 14 (+2) anchors the spell save DC at 8 + 2 prof + 2 = 12
        // and Booming Blade's thunder rider. Modest by caster standards
        // — which is correct: the knight's spells are shields and
        // openers, not the win condition.
        intelligence: 14,
        // Third-caster progression. `[4, 3]` reads as four level-1 slots
        // and three level-2s, matching a fighter around level 13.
        spell_slots_by_level: vec![4, 3],
        actions,
        features,
        ..FIGHTER_TEMPLATE.clone()
    }
});

/// Psi Warrior Fighter — Martial Archetype **Psi Warrior** subclass build
/// (TCE). The fifth fighter build in the engine, and the first whose
/// signature feature spends the fighter's reaction on somebody else.
///
/// Two subclass features ship, both level 3, both fed by RAW's Psionic
/// Energy pool:
///
///   - **Psionic Strike** — a once-per-turn +1d8 Force rider on any
///     weapon hit, via the shared `ONCE_PER_TURN_WEAPON_DIE_RIDERS`
///     cohort.
///   - **Protective Field** — a reaction that reduces damage by
///     `1d8 + INT` taken by the knight *or any ally within 30 ft*, via
///     the shared `REACTIVE_DAMAGE_CLAMPS` cohort. One charge per short
///     rest.
///
/// Plus **Guarded Mind** (lv10) as resistance to psychic damage.
///
/// The two lv3 features are RAW's single most literal expression of a
/// resource trade — one pool, spent either offensively or defensively —
/// and the engine can't represent that, because its per-rest charge lane
/// is a set membership rather than a counter. Faced with the choice, the
/// build gives the strike away for free and charges for the field. That
/// keeps the interesting decision (when do I spend the field, and on
/// whom?) and discards the uninteresting one (do I want +1d8 damage this
/// turn? — yes, always). The alternative split, charging for the strike,
/// would have produced a fighter whose defining feature almost never
/// fires. See `PROTECTIVE_FIELD_TAG` / `PSIONIC_STRIKE_TAG` for the
/// per-feature reasoning.
///
/// What distinguishes this from the four siblings on the chassis is the
/// direction its reaction points. Every other fighter reaction in the
/// engine is self-interested: Parry clamps damage aimed at the fighter,
/// Riposte answers a miss against the fighter, Indomitable rescues the
/// fighter's own save. Protective Field is the only one that can be
/// spent on a swing the fighter was never in, and it reaches 30 ft — so
/// a Psi Warrior standing between two melees is doing something none of
/// the other builds can. The natural reading is that this is the fighter
/// you put next to the wizard.
///
/// It also composes with rather than duplicates Parry, which the chassis
/// already carries. The clamp cohort visits `Holder`-scoped rows before
/// `HolderOrAlly` ones, so on a melee hit against the Psi Warrior
/// themselves the free parry die fires first and the field is only
/// reached once parry is spent. The two charges are independent, so a
/// Psi Warrior has *two* clamps per short rest against melee and one
/// against everything else.
///
/// **Stats.** INT rises to 16 from the baseline's 10 — the highest INT
/// on any fighter template — which puts Protective Field at `1d8 + 3`
/// (avg 7.5, roughly one greatsword swing absorbed). That's the only
/// stat change: HP, AC, STR, every maneuver, Parry / Riposte, Second
/// Wind / Action Surge / Indomitable, Extra Attack and the Dueling
/// style all inherit from the baseline via `..FIGHTER_TEMPLATE.clone()`.
/// The maneuver suite is RAW-illegal on a Psi Warrior, as it is on the
/// Samurai and the Eldritch Knight — engine-wide convention is that a
/// subclass template is the baseline chassis plus its subclass tell.
///
/// Guarded Mind's second RAW clause — ending Charmed / Frightened on
/// itself at the start of each of the holder's turns — is left out. It
/// needs a turn-start condition-scrub hook keyed to a feature tag, which
/// nothing else in the engine wants yet; the psychic resistance half is
/// the part that reads at a damage site the engine already has. Psionic
/// Adept's Psi-Powered Leap and Telekinetic Thrust, and the lv15
/// Bulwark of Force, are future work for the same reason the Eldritch
/// Knight's Arcane Charge is: each needs a destination or option picker
/// the AI has no channel to answer.
///
/// Glyph 'P' — for **P**si Warrior. Distinct from baseline Fighter 'F',
/// Champion 'C', Samurai 'S' and Eldritch Knight 'E'.
pub static PSI_WARRIOR_FIGHTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Not the `with_subclass_tag` one-liner the Samurai uses: this
    // subclass adds two tags rather than one, a damage-resistance entry,
    // and an INT bump. No new actions — both shipped features are
    // passive rows on shared engine cohorts (a once-per-turn weapon
    // rider and a reactive damage clamp), so the build's action surface
    // is exactly the baseline fighter's.
    let mut features = FIGHTER_TEMPLATE.features.clone();
    features.insert(PSIONIC_STRIKE_TAG);
    features.insert(PROTECTIVE_FIELD_TAG);
    CreatureTemplate {
        name: "Psi Warrior",
        glyph: 'P',
        // INT 16 (+3) sizes Protective Field's clamp at 1d8+3. Psionics
        // are the one fighter subclass whose headline number keys off
        // INT, so this is the stat that has to move.
        intelligence: 16,
        // 5e Psi Warrior **Guarded Mind** (subclass level 10): resistance
        // to psychic damage. Ships on this CR-3 (level-5-ish) chassis
        // above its RAW gate for the same reason the Champion's Survivor
        // (lv18) and Superior Critical (lv15) ride there — class
        // templates target a balanced playable level, not lockstep
        // progression. Psychic is a narrow lane in the engine's monster
        // pool (mind flayers, allips, nothics, the psychic-lance /
        // mind-spike spell family), so the resistance is a genuine but
        // situational defense rather than a broad one.
        damage_modifiers: HashMap::from([(DamageType::Psychic, DamageModifier::Resistance)]),
        features,
        ..FIGHTER_TEMPLATE.clone()
    }
});

/// Cavalier Fighter — Martial Archetype **Cavalier** subclass build
/// (XGtE). The sixth fighter build in the engine, and the one whose
/// features exist to make enemies fight *it* instead of whoever they
/// would rather be hitting.
///
/// Two subclass features ship, both as rows on cohorts that already
/// existed:
///
///   - **Unwavering Mark** (lv3) — every connecting melee swing locks
///     the target onto the cavalier: while marked, it attacks anyone
///     else at disadvantage. A row on `ON_HIT_CONDITION_MARKS`.
///   - **Warding Maneuver** (lv7) — a reaction that halves damage taken
///     by the cavalier or an adjacent ally. A row on
///     `REACTIVE_DAMAGE_CLAMPS`, one charge per short rest.
///
/// Both halves point the same direction, which is what separates this
/// from the other five builds. The Champion and the Samurai make the
/// fighter's own turn better; the Battle Master and the Eldritch Knight
/// buy options; the Psi Warrior protects at range. The Cavalier is the
/// only build that changes what the *enemy* wants to do — the mark makes
/// hitting anyone else expensive, and the maneuver punishes them for
/// trying anyway. A Cavalier standing in a doorway is doing more work
/// than its damage numbers suggest.
///
/// Unwavering Mark reuses `Condition::Dueled`, which the Compelled Duel
/// spell already installs — RAW's "disadvantage on any attack roll that
/// doesn't target you" is that condition's clause word for word. Which
/// makes the feature a Compelled Duel that costs no action, no slot and
/// no concentration, and asks only for a hit instead of a failed WIS
/// save. Being free is also why it lands on a *hit* rather than on
/// declaration: the cavalier has to earn the lock every round.
///
/// It stacks unusually well with the chassis's inherited maneuvers. Trip
/// Attack and Menacing Attack both impose their own disadvantage on the
/// target's attacks; a marked, prone, frightened enemy that wants to
/// swing at the wizard is rolling into a wall. And because the mark
/// re-stamps on every hit, Extra Attack and Action Surge extend the lock
/// rather than wasting swings on a target that is already marked.
///
/// **Stats.** CON rises to 16 from the baseline's 14 — RAW sizes Warding
/// Maneuver's uses by CON modifier, and a build whose job is to be
/// attacked wants the hit points besides. That bumps HP from 24 to 27
/// (3d10 + 9) through the inherited `3d10+6`-shaped roll being re-rolled
/// against the higher score. Everything else — AC 16 chain mail, STR 16,
/// every maneuver, Parry / Riposte, Second Wind / Action Surge /
/// Indomitable, Extra Attack, the Dueling style — inherits from the
/// baseline through the `..FIGHTER_TEMPLATE.clone()` tail.
///
/// Left out: Born to the Saddle (lv3) has no combat surface without
/// mounts; Unwavering Mark's bonus-action retaliation swing needs a
/// per-mark "the mark was violated" ledger (see `UNWAVERING_MARK_TAG`);
/// Ferocious Charger (lv10) gates on having moved 10+ ft in a straight
/// line this turn, which the engine doesn't track; Hold the Line (lv18)
/// needs an opportunity-attack trigger on movement *within* reach rather
/// than out of it.
///
/// Glyph 'V' — for Ca**v**alier, since 'C' is the Champion's. Distinct
/// from baseline Fighter 'F', Champion 'C', Samurai 'S', Eldritch Knight
/// 'E' and Psi Warrior 'P'.
pub static CAVALIER_FIGHTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    // Two tags and a CON bump; no new actions, since both shipped
    // features are passive rows on shared engine cohorts.
    let mut features = FIGHTER_TEMPLATE.features.clone();
    features.insert(UNWAVERING_MARK_TAG);
    features.insert(WARDING_MANEUVER_TAG);
    features.insert(FEROCIOUS_CHARGER_TAG);
    CreatureTemplate {
        name: "Cavalier",
        glyph: 'V',
        // CON 16 (+3): RAW sizes Warding Maneuver by CON modifier, and
        // the build's whole plan is to be the one getting hit.
        constitution: 16,
        features,
        // 5e Cavalier **Ferocious Charger** — ten straight feet and a
        // connecting swing knocks the target down, once per turn. The
        // one player-side rider on the same charge lane the boar and the
        // minotaur ride, and the reason `ChargeRider::weapon` is an
        // `Option`: RAW names no limb, because a fighter swings whatever
        // it is holding.
        charge: Some(FEROCIOUS_CHARGER),
        // The Mounted Combatant feat. RAW is a feat and not a subclass
        // feature, but the Cavalier is the subclass the feat exists for
        // — its level-3 **Born to the Saddle** and level-10 **Hold the
        // Line** are both written on the assumption that the fighter is
        // on a horse — and the engine's builds are level-complete
        // characters rather than a class plus a feat budget. So the
        // Cavalier ships with it the way the Champion ships with a
        // crit threshold of 19.
        //
        // It also gives the charge above somewhere to point. Ferocious
        // Charger wants twenty straight feet before the swing, and a
        // fighter has thirty in a turn; a warhorse has sixty.
        has_mounted_combatant: true,
        ..FIGHTER_TEMPLATE.clone()
    }
});

/// Rune Knight Fighter — Martial Archetype **Rune Knight** subclass
/// build (TCoE). The seventh fighter in the engine, and the first
/// character of any class whose signature feature changes the shape of
/// the space they occupy rather than a number on a sheet.
///
/// Two subclass features ship:
///
///   - **Giant's Might** (lv3, bonus action, one charge per rest): for a
///     minute the fighter grows one size category, saves with advantage
///     on STR, and once on each of their turns a connecting weapon hit
///     carries an extra 1d6.
///   - **Fire Rune** (lv3, bonus action, one charge per rest): primes
///     the next weapon hit for an extra 2d6 fire, with a STR save
///     against being Restrained by chains of fire on top.
///
/// Giant's Might is the reason this build exists. Every other fighter on
/// the roster spends its subclass on the swing — the Champion's crit
/// window, the Battle Master's riders, the Samurai's saves, the Psi
/// Warrior's shield. The Rune Knight spends it on the *board*: a Large
/// fighter's footprint is 4 tiles wide instead of 2, and every reach
/// measurement in the engine is taken from the footprint, so growing
/// widens the ring in which the fighter threatens opportunity attacks
/// and shortens the walk to anything they want to hit. A Rune Knight in
/// a corridor is not the same obstacle a Fighter is.
///
/// It is also the one feature in the engine the map is allowed to
/// refuse. RAW grows the fighter "if there is enough room", and pressed
/// into a doorway there may not be — the charge is spent, the damage
/// rider still fires, and `EncounterInstance::reconcile_footprints`
/// grows them the moment the space opens up. Which gives the build a
/// consideration no other fighter has: where you stand when you press
/// the button matters.
///
/// The two features are deliberately not redundant. Fire Rune wants to
/// be spent the turn a big swing lands (its Restrained rider is worth
/// more than its 2d6); Giant's Might wants to be spent early, because
/// its value is spread across a minute of standing in the right place.
/// Both are bonus actions and the chassis has one per turn, so the Rune
/// Knight opens every fight choosing which kind of fight it is going to
/// be — the same shape of choice the Kensei Monk got, on a heavier body.
///
/// Left out: **Cloud Rune** (lv3) redirects an incoming attack onto
/// another creature, which needs an attack-roll interception lane that
/// can retarget rather than just modify; **Stone Rune** (lv3) is a
/// reaction to a creature merely *approaching*, and the engine has no
/// movement-proximity trigger; **Storm** / **Hill** / **Frost Runes**
/// (lv7 and up) sit above this chassis's level; **Great Stature** (lv7)
/// and **Master of Runes** (lv10) only scale what already ships;
/// **Runic Shield** (lv15) needs a force-a-reroll-of-someone-else's-
/// attack lane.
///
/// **Stats** inherit the baseline Fighter wholesale — AC 16 chain mail,
/// STR 16, CON 14, every maneuver, Parry / Riposte, Second Wind /
/// Action Surge / Indomitable, Extra Attack, the Dueling style. The
/// build's distinction is entirely in the two runes, which is the honest
/// way to ship it: a Rune Knight is a fighter who occasionally becomes a
/// giant, not a different fighter.
///
/// Glyph 'R' — for **R**une Knight. Distinct from baseline Fighter 'F',
/// Champion 'C', Samurai 'S', Eldritch Knight 'E', Psi Warrior 'P' and
/// Cavalier 'V'.
pub static RUNE_KNIGHT_FIGHTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{FIRE_RUNE, FIRE_RUNE_TAG, GIANTS_MIGHT, GIANTS_MIGHT_TAG};
    let mut actions = FIGHTER_TEMPLATE.actions.clone();
    actions.push(&*GIANTS_MIGHT);
    actions.push(&*FIRE_RUNE);
    // Two per-rest charges. `GIANTS_MIGHT_RIDER_TAG` deliberately does
    // *not* appear here: it is a once-per-turn ledger key, not a charge,
    // and `once_per_turn_used` reads a separate set that no template
    // populates.
    let mut features = FIGHTER_TEMPLATE.features.clone();
    features.insert(GIANTS_MIGHT_TAG);
    features.insert(FIRE_RUNE_TAG);
    // SRD 5.2's **Tough** feat, and the Rune Knight is who it belongs
    // to: the one fighter on the roster whose subclass is literally
    // about getting bigger. Giant's Might already lends the body for a
    // minute at a time; Tough is what the body keeps. See
    // `crate::actions::feats::TOUGH_TAG`.
    features.insert(crate::actions::feats::TOUGH_TAG);
    CreatureTemplate {
        name: "Rune Knight",
        glyph: 'R',
        actions,
        features,
        ..FIGHTER_TEMPLATE.clone()
    }
});

/// Arcane Archer Fighter — the Martial Archetype **Arcane Archer**
/// (XGE), and the first fighter on the roster whose primary weapon is a
/// bow.
///
/// Every other fighter here answers the question "what happens when I
/// hit you" with a bigger number or a save-or-suck rider on a sword.
/// The Arcane Archer answers it from thirty feet away, with a menu, and
/// the menu is the subclass. Six options, two charges, and the whole
/// feature is which two you pick before the fight has finished telling
/// you what it is:
///
///   - **Banishing Arrow** takes a creature's turn away on a failed CHA
///     save. No damage attached — RAW withholds Banishing Arrow's dice
///     until subclass level 18, and it does not need them.
///   - **Beguiling Arrow** charms on a failed CHA save, which on this
///     engine means the target cannot raise a hand against the archer
///     until it shakes it off.
///   - **Bursting Arrow** detonates: 2d6 force to everything standing
///     within 10 ft of what it hit, no save, and nothing at all to the
///     creature that took the arrow. The crowd-control option, and the
///     only one that gets better the worse the archer's positioning is.
///   - **Enfeebling Arrow** halves the target's weapon damage on a
///     failed CON save. The answer to a multiattacker.
///   - **Grasping Arrow** Restrains on a failed STR save — speed zero,
///     disadvantage on its own swings, advantage for every ally
///     shooting at it.
///   - **Shadow Arrow** Blinds on a failed WIS save.
///
/// The six are mutually exclusive by construction (see `ARCANE_SHOTS`):
/// nocking one strips whatever was already on the string, so the two
/// charges buy two shots and not one doubled-up shot. Every save is
/// against 8 + proficiency + **Intelligence**, which is why this
/// fighter carries an INT 16 no other fighter on the roster has any use
/// for — RAW makes the Arcane Archer the one martial whose damage
/// depends on a caster's stat.
///
/// **Curving Shot** (lv7) rides the shared missed-attack cohort: once
/// per rest, an arrow that missed gets +1d8 on the roll. The cohort
/// adds rather than rerolls, which is the trade every member of it
/// makes.
///
/// **Stats** diverge from the baseline Fighter more than any other
/// subclass here, because the chassis has to actually be an archer:
/// DEX 18 and the Archery fighting style in place of STR 16 and
/// Dueling, AC 16 from studded leather plus the DEX rather than from
/// chain mail, and a longbow as the primary. The Battle Master
/// maneuvers inherited from the baseline stay — an archer forced into
/// melee still has a scimitar and a reason to trip somebody with it —
/// but the arcane shots are the reason to field this template.
///
/// **Ever-Ready Shot** (lv15) hands a charge back at initiative to an
/// archer who walked in empty — the engine's roll-initiative walk in
/// `EncounterInstance::initialize` is where it fires, next to the
/// Thief's second turn slot.
///
/// Left out: **Magic Arrow** (lv7) makes the archer's arrows magical for
/// overcoming resistance, and the engine's damage pipeline has no
/// non-magical-physical carve-out for it to matter against;
/// **Arcane Archer Lore** is a skill ribbon.
///
/// Glyph 'A' — for **A**rcane Archer. Distinct from baseline Fighter
/// 'F', Champion 'C', Samurai 'S', Eldritch Knight 'E', Psi Warrior 'P',
/// Cavalier 'V' and Rune Knight 'R'.
pub static ARCANE_ARCHER_FIGHTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    use crate::actions::class_features::{
        ARCANE_SHOT_ACTIONS, ARCANE_SHOT_TAG, CURVING_SHOT_TAG, EVER_READY_SHOT_TAG,
    };
    use crate::actions::monster_attacks::LONGBOW;
    // The bow goes on ahead of the inherited scimitar so the action list
    // reads primary-first, which is also the order the prompt's
    // ambiguous-prefix resolution walks.
    let mut actions = vec![&LONGBOW as &'static (dyn crate::actions::action_template::Action + Send + Sync)];
    actions.extend(FIGHTER_TEMPLATE.actions.iter().copied());
    for shot in ARCANE_SHOT_ACTIONS.iter() {
        actions.push(*shot as &'static (dyn crate::actions::action_template::Action + Send + Sync));
    }
    let mut features = FIGHTER_TEMPLATE.features.clone();
    features.insert(ARCANE_SHOT_TAG);
    features.insert(CURVING_SHOT_TAG);
    features.insert(EVER_READY_SHOT_TAG);
    // **Sharpshooter**, which is the feat this subclass would take with
    // its first free pick and the one chassis on the roster whose whole
    // identity it is. All three clauses are about a bow and this
    // template carries nothing else: no spell attacks for the "Ranged
    // weapons" qualifier to matter against, and a longbow at the head
    // of its own action list. See `feats::SHARPSHOOTER_TAG`.
    features.insert(crate::actions::feats::SHARPSHOOTER_TAG);
    CreatureTemplate {
        name: "Arcane Archer",
        glyph: 'A',
        actions,
        features,
        // Studded leather (12) + DEX 18 (+4). Two AC below the baseline
        // fighter's chain mail, which is the price of standing where the
        // bow is worth carrying.
        ac: 16,
        strength: 12,
        dexterity: 18,
        // The Arcane Shot DC anchor. A fighter's dump stat everywhere
        // else on this roster, and the only reason six of this
        // template's actions do anything at all.
        intelligence: 16,
        // Dueling is a melee-damage style and buys an archer nothing.
        has_dueling_style: false,
        has_archery_style: true,
        // Extends the baseline fighter's Athletics / Perception rather
        // than replacing it — Stealth is the archer's addition, and a
        // subclass that dropped a family skill would fail
        // `subclass_templates_inherit_their_familys_skills`.
        skills: FIGHTER_TEMPLATE
            .skills
            .iter()
            .cloned()
            .chain([Skill::Stealth])
            .collect(),
        ..FIGHTER_TEMPLATE.clone()
    }
});
