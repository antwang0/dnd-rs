use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::WARHORSE_HOOVES;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Warhorse — CR ½ large beast. The "trained battle mount" tier of
/// equine: a heavy-built hooved combatant bred for line cavalry
/// charges. Slots beside the Riding Horse / Draft Horse cohort in
/// the SRD's horse family — RAW's "warhorse" is the militarized
/// variant, and the one whose hooves are worth rolling on its own
/// turn. The warhorse keeps the hooves-with-trample-flavor identity
/// that separates it from a generic large beast.
///
/// `mountable`, along with the rest of the horse family — see
/// `engine::mounts`. This paragraph used to say the opposite, and
/// gave that as the reason the civilian horses weren't worth
/// shipping: with no mounted-combat lane a riding horse was hooves
/// fodder with no clause of its own. It has one now, and it is the
/// only clause that ever mattered for a horse.
///
/// Slots beside the Mastiff (CR ⅛ trained guard hound) and the
/// Guard (CR ⅛ city watchman) as the "trained-NPC-companion" tier
/// at the low CR end. Paired with a Knight (CR 3 mounted) or a
/// Veteran (CR 3 dismounted leader), the warhorse rounds out the
/// "professional soldier and his mount" encounter shape that
/// dungeon modules lean on for cavalry skirmishes.
///
/// Action lane:
/// - **warhorse hooves** — STR-based 2d6+STR bludgeoning melee via
///   the shared `WARHORSE_HOOVES` static. RAW's Hooves attack is
///   `+6 to hit, reach 5 ft, one target. Hit: 11 (2d6 + 4)
///   bludgeoning damage` — the chunky 2d6 dice carry the warhorse's
///   damage profile alone (no rider, no multiattack). Pair with the
///   `has_pack_tactics` on a Knight companion's mount-and-handler
///   formation for an effective burst opening.
///
/// **Trampling Charge** (RAW: 20 ft straight charge → on hooves
/// hit, DC 14 STR save-or-Prone, then a bonus-action hooves swing
/// against a prone target) is omitted as a deliberate scope cut.
/// The engine doesn't track straight-line charge movement at attack
/// time, and the Mammoth (CR 6) already carries the "Trampling
/// Charge → Stomp" two-attack chassis at the heavy huge-beast tier.
/// Adding a recharge-gated CR-½ version would inflate the warhorse's
/// per-round threat above its CR. The plain hooves swing keeps the
/// warhorse anchored at the "trained mount with chunky hooves
/// damage" silhouette without the recharge lock-down lane.
///
/// Defensive identity: AC 11 (large, no natural armor — the
/// warhorse wears no barding RAW), 19 HP (3d10+3). Vanilla beast
/// envelope — no resistances or condition immunities. The warhorse
/// dies to two solid hits at CR ½; its threat lives in the chunky
/// hooves dice rather than survivability.
///
/// Stat shape: AC 11, ~19 HP (3d10+3), STR 18, DEX 12, CON 13,
/// INT 2, WIS 12, CHA 7. Speed 60 (a warhorse outruns the rest of
/// the CR-½ bench by 20 ft — mobility *is* the cavalry identity).
/// Size Large. CR ½. XP: 100 per RAW.
pub static WARHORSE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WARHORSE_HOOVES);
    CreatureTemplate {
        name: "Warhorse",
        // 'H' (uppercase) — shared with Half-Orc / Harpy / Hippogriff
        // / Hobgoblin / Hydra cohort. The team color disambiguates on
        // the map; in a cavalry encounter the warhorse will spawn
        // beside a Knight ('K') so 'H' reads as "horse" by silhouette
        // and position rather than by glyph alone. Lowercase 'h' is
        // already taken by Hyena; the uppercase variant is the right
        // pick for the large-mount silhouette at the small UI scale.
        glyph: 'H',
        ac: 11,
        // 3d10+3 = 19 average per MM (CR ½).
        hitpoints: "3d10+3".parse().unwrap(),
        speed: 60.,
        strength: 18,
        intelligence: 2,
        dexterity: 12,
        wisdom: 12,
        constitution: 13,
        charisma: 7,
        cr: 0.5,
        size: Size::Large,
        // 5e Mounted Combat: PHB's mount table, militarised: the warhorse is what "line cavalry" means.
        mountable: true,
        creature_type: CreatureType::Beast,
        actions,
        // RAW: when the warhorse closes at least the clause's distance in a
        // straight line and then connects with its hooves, the hit carries
        // a Strength save vs prone. Read at the melee attack
        // chokepoint off `ActorInstance::charge`.
        charge: Some(crate::actions::monster_attacks::WARHORSE_CHARGE),
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
            &WARHORSE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn warhorse_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("warhorse hooves").is_some());
    }

    #[test]
    fn warhorse_is_fast() {
        // Pin the load-bearing mobility trait: a warhorse's combat
        // identity is "fast trained mount". Speed 60 outpaces the
        // rest of the CR-½ bench by 20 ft and matches RAW. A future
        // template-refactor that dropped the speed to the default 30
        // would erase the cavalry identity and silently demote the
        // warhorse to "a slow large beast with chunky hooves" — same
        // damage profile as the mastiff at a higher CR. The speed
        // 60 IS what the warhorse pays its CR for.
        let a = make();
        assert!(a.speed() >= 60.0);
    }
}
