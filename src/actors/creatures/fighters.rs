use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, COMMANDERS_STRIKE, COMMANDERS_STRIKE_TAG, DISARMING_ATTACK,
    DISARMING_ATTACK_TAG, DISTRACTING_ATTACK, DISTRACTING_ATTACK_TAG, FEINTING_ATTACK,
    FEINTING_ATTACK_TAG, GOADING_ATTACK, GOADING_ATTACK_TAG, INDOMITABLE, INDOMITABLE_TAG,
    LUNGING_ATTACK, LUNGING_ATTACK_TAG, MENACING_ATTACK, MENACING_ATTACK_TAG, PRECISION_ATTACK,
    PRECISION_ATTACK_TAG, PUSHING_ATTACK, PUSHING_ATTACK_TAG, RALLY, RALLY_TAG, SECOND_WIND,
    SECOND_WIND_TAG, SURVIVOR_TAG, SWEEPING_ATTACK, SWEEPING_ATTACK_TAG, TRIP_ATTACK,
    TRIP_ATTACK_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGSWORD, SCIMITAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
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
        features: HashSet::from([SECOND_WIND_TAG, ACTION_SURGE_TAG, INDOMITABLE_TAG, SURVIVOR_TAG]),
        has_extra_attack: true,
        // 5e Champion subclass level-3 feature: critical hits trigger on
        // 19 or 20 instead of just 20. Read at every attack-roll site
        // via `actor.crit_threshold()`.
        crit_threshold: 19,
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
        ]),
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
        ..CreatureTemplate::defaults()
    }
});
