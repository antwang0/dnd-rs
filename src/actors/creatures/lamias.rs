use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LAMIA_CLAWS, LAMIA_INTOXICATING_TOUCH, LAMIA_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Lamia — CR 4 large monstrosity. The desert temptress — lion's body
/// from the waist down, a human-ish torso above, ruling a crumbling
/// ziggurat-and-illusion court with a velvet-tongued curse for every
/// intruder. Slots between Werewolf (CR 3) and Werebear (CR 5) on the
/// mid-tier humanoid-flavored monstrosity ladder — the caster-flavored
/// counterpart to the lycanthropes, distinguished by an intoxicating-
/// touch curse that locks down a single PC per Action.
///
/// Action lanes:
/// - **lamia multiattack** — 1 claws + 1 intoxicating touch per Action
///   via `CompoundAttack`. Heterogeneous compound: the claws are the
///   damage lane (2d10+STR slashing), the touch is the curse install
///   (WIS DC 13 → Charmed 10 rounds). Per-Action shape: solid melee
///   damage on the same target the curse lands on, so a single Action
///   both whittles HP and locks the PC out of the hostile-action lane.
/// - **lamia claws** (standalone) — STR-based 2d10+STR slashing
///   melee, reach 1.
/// - **intoxicating touch** (standalone) — WIS DC 13 save or Charmed
///   (10 rounds) + SetConditionLink(Charmed ← lamia) so the cursed PC can't take
///   hostile actions against their cursed mistress.
///
/// Defensive identity: AC 13 (natural armor — the lion-half's hide),
/// 97 HP (13d10+26). No damage resistances or immunities, no condition
/// immunities — the lamia is a mortal monstrosity whose defense is
/// HP pool + the per-encounter curse tempo. Darkvision 60 ft.
/// We omit the RAW "innate spellcasting" clause (Charm Person, Mirror
/// Image, Suggestion, Disguise Self, Geas) — the in-engine spell
/// chassis is per-creature-template-slot-driven and the lamia's
/// intoxicating-touch curse already covers the load-bearing tactical
/// clause (single-target charm lockout).
///
/// Stat shape: AC 13, ~97 HP (13d10+26), STR 16, DEX 13, CON 15, INT 14,
/// WIS 15, CHA 16. Speed 40. Senses: Darkvision 60. Languages: Abyssal,
/// Celestial, Common, Draconic. Size Large. CR 4.
pub static LAMIA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*LAMIA_MULTI);
    actions.push(&LAMIA_CLAWS);
    actions.push(&*LAMIA_INTOXICATING_TOUCH);
    CreatureTemplate {
        name: "Lamia",
        // 'L' (uppercase) — 'l' is taken (Lich), but the lamia is a
        // mid-tier monstrosity that warrants its own glyph slot. 'L'
        // for "lamia" reads as the larger-statured lion-bodied
        // silhouette; distinct from the lich's high-tier undead at
        // lowercase.
        glyph: 'L',
        ac: 13,
        // 13d10+26 ≈ 97 average per MM (CR 4).
        hitpoints: "13d10+26".parse().unwrap(),
        speed: 40.,
        strength: 16,
        intelligence: 14,
        dexterity: 13,
        wisdom: 15,
        constitution: 15,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([
            Language::Abyssal,
            Language::Celestial,
            Language::Common,
            Language::Draconic,
        ]),
        cr: 4.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn lamia_template_shape() {
        let a = ActorInstance::from_creature_template(
            &LAMIA_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 4.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Monstrosity);
        // The lamia's three action lanes — multi (claws + curse touch)
        // primary, with each part also exposed as a standalone fallback
        // for the AI's per-resource picking (e.g. when the AI wants to
        // burn raw damage or pure curse install without committing the
        // whole Action to both).
        assert!(a.find_action("lamia multiattack").is_some());
        assert!(a.find_action("lamia claws").is_some());
        assert!(a.find_action("intoxicating touch").is_some());
    }

    #[test]
    fn lamia_has_no_immunities_but_can_curse() {
        let a = ActorInstance::from_creature_template(
            &LAMIA_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        // The lamia is a mortal monstrosity — no magic resistance, no
        // condition immunities, no damage modifiers. The HP pool + the
        // per-Action curse lockout IS its identity.
        assert!(!a.has_magic_resistance());
        assert!(!a.effectively_immune_to_condition(Condition::Charmed));
        assert!(!a.effectively_immune_to_condition(Condition::Frightened));
    }
}
