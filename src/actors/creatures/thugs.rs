use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    HEAVY_CROSSBOW, MACE, THUG_MULTI, TOUGH_BOSS_CROSSBOW, TOUGH_BOSS_MULTI,
    TOUGH_BOSS_WARHAMMER,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Thug — CR ½ humanoid bruiser. The "back-alley enforcer" tier of NPC
/// mooks: heavier than a bandit (more HP, harder hit), softer than a
/// bandit captain. Slots between the Bandit (CR ⅛, one scimitar) and
/// the Bandit Captain (CR 2, three scimitar swings) as the canonical
/// CR-½ humanoid melee pressure entry.
///
/// Action lanes:
/// - **mace** — STR-based 1d6+STR bludgeoning melee via the shared
///   `MACE` static. Bludgeoning rather than slashing so the thug feels
///   different from a bandit even when only the base swing fires.
/// - **double mace** — 2 mace swings per Action via the shared
///   `Multiattack` chassis (`THUG_MULTI`). Two-hit Action lands ~10
///   bludgeoning on a clean pair against a medium-AC target.
/// - **heavy crossbow** — DEX-based 1d10+DEX piercing ranged via the
///   shared `HEAVY_CROSSBOW` static. The 16-tile reach / 10-tile normal
///   range matches the bandit family so a mixed bandit / thug ambush
///   reads as one cohesive raiding party.
///
/// **Pack Tactics** — RAW: "The thug has advantage on attack rolls
/// against a creature if at least one of the thug's allies is within
/// 5 ft of the creature and the ally isn't incapacitated." Routes
/// through the shared `has_pack_tactics: true` template flag which
/// `compute_attack_mode` reads at the attack chokepoint. A pair of
/// thugs locking down one target is the canonical Pack Tactics double-
/// team — same chassis as the Wolf / Kobold pack lane.
///
/// Defensive identity: AC 12 (leather armor, no shield), 32 HP
/// (5d8+10). Vanilla humanoid envelope — no resistances or condition
/// immunities. The threat profile is Pack-Tactics-fueled multiattack
/// at close range; isolated, a thug is just a bag of HP with a club.
///
/// Stat shape: AC 12, ~32 HP (5d8+10), STR 15, DEX 12, CON 14, INT 10,
/// WIS 10, CHA 11. Speed 30. Languages: Common. Size Medium. CR ½.
/// XP: 100 per RAW.
pub static THUG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MACE);
    actions.push(&HEAVY_CROSSBOW);
    actions.push(&*THUG_MULTI);
    CreatureTemplate {
        name: "Thug",
        // 'H' — bandit-tier humanoid. 'B' is the bandit / bandit-captain
        // cohort; 'H' (for "Hoodlum"/"Heavy") keeps the thug distinct
        // on the map while still reading as a humanoid mook silhouette.
        // 'H' is otherwise untaken in the glyph map.
        glyph: 'H',
        ac: 12,
        // 5d8+10 ≈ 32 average per SRD 5.2 (CR ½).
        hitpoints: "5d8+10".parse().unwrap(),
        speed: 30.,
        strength: 15,
        intelligence: 10,
        dexterity: 12,
        wisdom: 10,
        constitution: 14,
        charisma: 11,
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // 5e Pack Tactics — the thug gets advantage when an ally is
        // adjacent to the target. Read at the `compute_attack_mode`
        // chokepoint; no per-attack code needed here.
        has_pack_tactics: true,
        ..CreatureTemplate::defaults()
    }
});

/// Tough Boss — CR 4 humanoid enforcer, the rung above the Thug. SRD
/// 5.2 files the pair as Tough and Tough Boss; the bestiary keeps the
/// older, better name for the first and takes the book's for the
/// second, because "Thug Boss" is nobody's name for anything.
///
/// What it adds over the thug is not size but *organisation*: **Pack
/// Tactics**, and a warhammer that shoves. A boss alone is a slightly
/// heavier thug; a boss with two thugs beside it has advantage on every
/// swing all three of them make, which is the whole reason the stat
/// block exists and the reason it is priced four rungs up.
///
/// Action lanes:
/// - **tough boss multiattack** — two warhammer swings.
/// - **boss warhammer** — 2d8 with the **Push** mastery, RAW's "the
///   tough pushes the target up to 10 feet straight away from itself".
/// - **boss crossbow** — 2d10 at range.
///
/// Stat shape: AC 16 (chain mail), 82 HP (11d8+33), STR 17 / DEX 14 /
/// CON 16 / INT 11 / WIS 10 / CHA 11. Speed 30. CR 4.
pub static TOUGH_BOSS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*TOUGH_BOSS_MULTI);
    actions.push(&TOUGH_BOSS_WARHAMMER);
    actions.push(&TOUGH_BOSS_CROSSBOW);
    CreatureTemplate {
        name: "Tough Boss",
        // 'Y' — the thug holds 'T'. Glyphs are chosen to read against
        // the creatures a fight is likely to put beside them rather
        // than to be unique across four hundred stat blocks (the
        // Artificer and the Cyclops also answer to it, and neither
        // turns up in an alley), and a 'Y' has the right heft for a
        // boss.
        glyph: 'Y',
        ac: 16,
        // 11d8+33 = 82 average per SRD 5.2 (CR 4).
        hitpoints: "11d8+33".parse().unwrap(),
        speed: 30.,
        strength: 17,
        dexterity: 14,
        constitution: 16,
        intelligence: 11,
        wisdom: 10,
        charisma: 11,
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // RAW: "the tough has Advantage on an attack roll against a
        // creature if at least one of the tough's allies is within 5
        // feet of the creature." The engine's shared flag, the same one
        // the wolves and the giant rats read.
        has_pack_tactics: true,
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
            &THUG_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn thug_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Humanoid);
        assert!(a.find_action("mace").is_some());
        assert!(a.find_action("heavy crossbow").is_some());
        assert!(a.find_action("double mace").is_some());
    }

    #[test]
    fn thug_carries_pack_tactics() {
        // Pin the load-bearing trait: Pack Tactics is the thug's only
        // mechanical edge over a bandit. A future template-refactor
        // that strips the flag would quietly demote the thug to "just
        // a heavier bandit", flattening its tactical identity.
        let a = make();
        assert!(a.has_pack_tactics());
    }
}
