use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    CAPTAINS_CHARM, DAGGER, ENTHRALLING_PANACHE, PIRATE_CAPTAIN_MULTI, PIRATE_CAPTAIN_PISTOL,
    PIRATE_CAPTAIN_RAPIER, PIRATE_MULTI, THROWN_DAGGER,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Pirate — CR 1 humanoid, and the first stat block on the roster whose
/// interesting half is a saving throw rather than a die.
///
/// The bandit and the pirate are the same silhouette on a map and very
/// nearly the same numbers, and the book still prints both, because
/// **Enthralling Panache** is a different creature. A bandit that gets
/// into reach does 5 damage. A pirate that gets into reach can spend
/// half its Action making the party's paladin sit out a turn — and RAW
/// says exactly that, "it can replace one attack with a use of
/// Enthralling Panache", so the trade is priced into the Multiattack
/// rather than bolted beside it.
///
/// Action lanes:
/// - **double dagger** — two 1d4+DEX piercing swings.
/// - **dagger** / **thrown dagger** — the shared statics; the throw is
///   the same weapon at 8/24 tiles, which is what RAW's "Melee or
///   Ranged Attack Roll: +5, reach 5 ft. or range 20/60 ft." means.
/// - **enthralling panache** — WIS DC 12 within 30 ft for one round of
///   Charmed. Short, and deliberately so: a CR-1 charm that lasted a
///   minute would be a CR-4 charm.
///
/// Stat shape: AC 14 (leather), 33 HP (6d8+6), STR 10 / DEX 16 / CON 12
/// / INT 8 / WIS 12 / CHA 14. Speed 30. Saves: DEX +5, CHA +4 — the
/// second is the one that matters, because a pirate resists being
/// charmed back. CR 1.
pub static PIRATE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*PIRATE_MULTI);
    actions.push(&DAGGER);
    actions.push(&THROWN_DAGGER);
    actions.push(&ENTHRALLING_PANACHE);
    CreatureTemplate {
        name: "Pirate",
        // 'p' beside the captain's 'P' — the crew and the one giving
        // the orders, which is how the pair turns up.
        glyph: 'p',
        ac: 14,
        // 6d8+6 = 33 average per SRD 5.2 (CR 1).
        hitpoints: "6d8+6".parse().unwrap(),
        speed: 30.,
        strength: 10,
        dexterity: 16,
        constitution: 12,
        intelligence: 8,
        wisdom: 12,
        charisma: 14,
        // RAW prints DEX +5 and CHA +4 against a +2 proficiency bonus —
        // both are the ability modifier plus proficiency.
        proficient_saves: HashSet::from([AbilityScoreType::Dexterity, AbilityScoreType::Charisma]),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        ..CreatureTemplate::defaults()
    }
});

/// Pirate Captain — CR 6 humanoid, and the cleanest illustration in the
/// bestiary of what a bonus action is worth.
///
/// The captain's charm is two points of DC better than the pirate's and
/// costs a **Bonus Action** instead of half an Attack. That second
/// difference is the whole stat block. A pirate choosing to charm gives
/// up a dagger; a captain choosing to charm gives up nothing, so it
/// charms every single turn and still takes three rapier swings. There
/// is no round in which the party is not down a member and taking
/// 39 points of piercing.
///
/// Action lanes:
/// - **triple rapier** — three 2d8+DEX swings, each with **Vex**, so
///   the first hit buys advantage on the second and the second on the
///   third. RAW's "the pirate has Advantage on the next attack roll it
///   makes before the end of this turn" is that mastery verbatim.
/// - **pistol** — 2d10+DEX at 12/20 tiles. RAW lets the Multiattack mix
///   rapier and pistol freely; the mix lives on the action list, since
///   the captain wants to be in reach and the pistol is what it does on
///   the way there.
/// - **captain's charm** — WIS DC 14 within 30 ft, one round, bonus
///   action.
///
/// **Riposte** — RAW: "Trigger: The pirate is hit by a melee attack roll
/// while holding a weapon. Response: The pirate adds 3 to its AC against
/// that attack, possibly causing it to miss. On a miss, the pirate makes
/// one Rapier attack against the triggering creature." The first half is
/// the engine's `has_parry`, which is the gladiator's reaction and adds
/// 2 rather than 3; the second half — a free swing off a reaction-forced
/// miss — has no lane, because nothing in the reaction pipeline can hand
/// a weapon back to the creature that just dodged. Shipping the parry
/// half alone is the honest subset: it makes the captain harder to hit
/// in melee, which is most of what the reaction is for, and it does not
/// invent a counterattack the engine cannot resolve.
///
/// Stat shape: AC 17, 84 HP (13d8+26), STR 10 / DEX 18 / CON 14 / INT 10
/// / WIS 14 / CHA 17. Speed 30. Skills: Acrobatics, Perception. Saves:
/// STR, DEX, WIS, CHA. CR 6.
pub static PIRATE_CAPTAIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*PIRATE_CAPTAIN_MULTI);
    actions.push(&PIRATE_CAPTAIN_RAPIER);
    actions.push(&PIRATE_CAPTAIN_PISTOL);
    actions.push(&CAPTAINS_CHARM);
    CreatureTemplate {
        name: "Pirate Captain",
        glyph: 'P',
        ac: 17,
        // 13d8+26 = 84 average per SRD 5.2 (CR 6).
        hitpoints: "13d8+26".parse().unwrap(),
        speed: 30.,
        strength: 10,
        dexterity: 18,
        constitution: 14,
        intelligence: 10,
        wisdom: 14,
        charisma: 17,
        // RAW: STR +3, DEX +7, WIS +5, CHA +6 against a +3 proficiency
        // bonus. STR +3 is +0 plus proficiency, which is the tell — a
        // captain is proficient in a save it is bad at.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Dexterity,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        skills: HashSet::from([Skill::Acrobatics, Skill::Perception]),
        languages: HashSet::from([Language::Common]),
        cr: 6.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // The first half of RAW's Riposte reaction — see the docstring.
        has_parry: true,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make(t: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            t,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn pirate_template_shape() {
        let a = make(&PIRATE_TEMPLATE);
        assert_eq!(a.cr(), 1.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("double dagger").is_some());
        assert!(a.find_action("thrown dagger").is_some());
        assert!(a.find_action("enthralling panache").is_some());
    }

    #[test]
    fn pirate_captain_template_shape() {
        let a = make(&PIRATE_CAPTAIN_TEMPLATE);
        assert_eq!(a.cr(), 6.0);
        assert!(a.find_action("triple rapier").is_some());
        assert!(a.find_action("pistol").is_some());
        assert!(a.find_action("captain's charm").is_some());
        assert!(a.has_parry());
    }

    /// The captain's rapier vexes, which is the RAW clause the stat
    /// block spells out longhand and the engine already had a name for.
    #[test]
    fn the_rapier_sets_up_the_swing_after_it() {
        use crate::actions::action_template::Action;
        use crate::engine::mastery::WeaponMastery;
        assert_eq!(
            PIRATE_CAPTAIN_RAPIER.weapon_mastery(),
            Some(WeaponMastery::Vex)
        );
    }
}
