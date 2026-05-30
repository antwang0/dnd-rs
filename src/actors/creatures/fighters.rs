use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, COMMANDERS_STRIKE, COMMANDERS_STRIKE_TAG, DISARMING_ATTACK,
    DISARMING_ATTACK_TAG, FEINTING_ATTACK, FEINTING_ATTACK_TAG, GOADING_ATTACK, GOADING_ATTACK_TAG,
    INDOMITABLE, INDOMITABLE_TAG, LUNGING_ATTACK, LUNGING_ATTACK_TAG, MENACING_ATTACK,
    MENACING_ATTACK_TAG, PRECISION_ATTACK, PRECISION_ATTACK_TAG, PUSHING_ATTACK, PUSHING_ATTACK_TAG,
    RALLY, RALLY_TAG, SECOND_WIND, SECOND_WIND_TAG, SWEEPING_ATTACK, SWEEPING_ATTACK_TAG,
    TRIP_ATTACK, TRIP_ATTACK_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGSWORD, SCIMITAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
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
        speed: 30.,
        strength: 18,
        intelligence: 10,
        dexterity: 12,
        wisdom: 11,
        constitution: 14,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::from([AbilityScoreType::Strength, AbilityScoreType::Constitution]),
        condition_immunities: HashSet::new(),
        features: HashSet::from([SECOND_WIND_TAG, ACTION_SURGE_TAG, INDOMITABLE_TAG]),
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
        has_extra_attack: true,
        brutal_critical_dice: 0,
        // 5e Champion subclass level-3 feature: critical hits trigger on
        // 19 or 20 instead of just 20. Read at every attack-roll site
        // via `actor.crit_threshold()`.
        crit_threshold: 19,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        sorcery_points: 0,
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
    CreatureTemplate {
        name: "Fighter",
        glyph: 'F',
        ac: 16,
        hitpoints: "3d10+6".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 10,
        dexterity: 12,
        wisdom: 11,
        constitution: 14,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        // Fighters are proficient in STR and CON saves (5e PHB).
        proficient_saves: HashSet::from([AbilityScoreType::Strength, AbilityScoreType::Constitution]),
        condition_immunities: HashSet::new(),
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
        ]),
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
        has_extra_attack: true,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        sorcery_points: 0,
    }
});
