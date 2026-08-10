use crate::actions::class_features::{ASSASSINATE_TAG, CUNNING_DASH, CUNNING_DISENGAGE, CUNNING_HIDE};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    ASSASSIN_LIGHT_CROSSBOW, ASSASSIN_MULTI, ASSASSIN_SHORTSWORD,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Assassin — CR 8 medium humanoid. Seventy-eight hit points of body
/// carrying a hundred and fifty of threat, all of it in the venom.
///
/// Action lanes:
/// - **assassin multiattack** — 2 poisoned shortswords. Two DC 15 CON
///   saves against 7d6 each, which is the whole stat block: an opening
///   round that lands both and fails both is 48 average damage from a
///   creature whose weapon dice total seven.
/// - **assassin shortsword** (standalone) — the same blade, once.
/// - **assassin light crossbow** — the ranged half, carrying the
///   identical venom. Sixteen tiles of clean band and twenty-four of
///   reach: the assassin opens the fight from somewhere the party has
///   not looked yet.
/// - **cunning dash / disengage / hide** — RAW's Cunning Action, the
///   same three the Rogue and the Spy carry.
///
/// **Assassinate** rides `ASSASSINATE_TAG`, which is the engine's
/// existing implementation of the feature and does both halves of it:
/// advantage against anything that has not taken a turn yet, and an
/// automatic critical against a surprised target. The second half is
/// what makes the opening round decisive rather than merely good — a
/// crit doubles the 7d6, and RAW's assassin is a creature you are
/// supposed to have already lost to by the time you roll initiative.
///
/// **Evasion** (`has_evasion`) is the defensive half: no damage at all
/// on a successful DEX save, half on a failure. It is the clause that
/// makes an assassin a poor target for the fireball the party reaches
/// for when something has just done forty-eight damage to the front
/// line.
///
/// RAW's Sneak Attack (4d6) is not carried, for the reason given at
/// length on the Spy — it is not a flag in this engine but a clause on
/// the rogue-weapon chassis, and the assassin's blade is a
/// save-rider weapon instead. The omission costs the assassin
/// proportionally less than it costs the spy: 4d6 is a fifth of what
/// the venom already does.
///
/// Stat shape per the SRD NPC appendix: AC 15 (studded leather), 78 HP
/// (12d8+24), STR 11 / DEX 16 / CON 14 / INT 13 / WIS 11 / CHA 10.
/// Speed 30. Proficient DEX / INT saves. Skills: Acrobatics, Deception,
/// Perception, Stealth. CR 8.
pub static ASSASSIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ASSASSIN_MULTI);
    actions.push(&ASSASSIN_SHORTSWORD);
    actions.push(&ASSASSIN_LIGHT_CROSSBOW);
    actions.push(&*CUNNING_DASH);
    actions.push(&*CUNNING_DISENGAGE);
    actions.push(&*CUNNING_HIDE);
    CreatureTemplate {
        name: "Assassin",
        // 'A' (uppercase) — shared with the Assassin Rogue PC template,
        // which is the same creature seen from the other side of the
        // screen, and with Animated Armor at a CR five rungs away.
        glyph: 'A',
        ac: 15,
        // 12d8+24 ≈ 78 average per the SRD NPC appendix (CR 8).
        hitpoints: "12d8+24".parse().unwrap(),
        speed: 30.,
        strength: 11,
        dexterity: 16,
        constitution: 14,
        intelligence: 13,
        wisdom: 11,
        charisma: 10,
        skills: HashSet::from([
            Skill::Acrobatics,
            Skill::Deception,
            Skill::Perception,
            Skill::Stealth,
        ]),
        languages: HashSet::from([Language::Common, Language::ThievesCant]),
        cr: 8.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Intelligence,
        ]),
        features: HashSet::from([ASSASSINATE_TAG]),
        has_evasion: true,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &ASSASSIN_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn assassin_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 8.0);
        assert!(a.find_action("assassin multiattack").is_some());
        assert!(a.find_action("assassin shortsword").is_some());
        assert!(a.find_action("assassin light crossbow").is_some());
    }

    /// The two clauses that make the CR. Assassinate is the opening
    /// round and Evasion is the answer to the burst that comes back;
    /// a stat block with only one of them is a different creature.
    #[test]
    fn the_assassin_opens_with_assassinate_and_survives_the_reply() {
        let a = make();
        assert!(a.has_passive_feature(ASSASSINATE_TAG));
        assert!(a.has_evasion());
    }

    /// Both venom lanes carry the same DC and the same dice, which is
    /// the clause that keeps the assassin dangerous at every range
    /// rather than only in contact.
    #[test]
    fn the_venom_is_on_the_bolt_as_well_as_the_blade() {
        assert_eq!(ASSASSIN_SHORTSWORD.save_dc, ASSASSIN_LIGHT_CROSSBOW.save_dc);
        assert_eq!(
            ASSASSIN_SHORTSWORD.rider_dice,
            ASSASSIN_LIGHT_CROSSBOW.rider_dice
        );
        // …and the bolt declares the range band the long-range and
        // underwater rules both read.
        assert_eq!(ASSASSIN_LIGHT_CROSSBOW.normal_range, Some(16));
    }
}
