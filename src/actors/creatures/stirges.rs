use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::STIRGE_PROBOSCIS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::attachment::AttachProfile;
use crate::engine::dice::Dice;
use crate::engine::types::{CreatureType, DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// The stirge's half of SRD 5.2's attach clause: "…and the stirge
/// attaches to the target. While attached, the stirge can't make
/// Proboscis attacks, and the target takes 5 (2d4) Necrotic damage at
/// the start of each of the stirge's turns. The stirge can detach
/// itself by spending 5 feet of its movement. The target or a creature
/// within 5 feet of it can detach the stirge as an action."
///
/// The whole of the creature is in the two clauses this sets. The drain
/// is the only one in the SRD's attach family — the cloaker and the
/// darkmantle hold on and hit you, a stirge holds on and *eats* — and
/// it is hung on the stirge's own turn rather than the victim's, which
/// is what makes a cloud of them a clock the party stops by killing
/// them one at a time.
///
/// `pry_dc: None` is RAW and not an omission. The cloaker and the
/// darkmantle each name a Strength (Athletics) DC; the stirge's
/// sentence names no check at all — "can detach the stirge as an
/// action" — so an Action pulls it off and that is the end of it.
/// Inventing a DC would make the cheapest creature in the book harder
/// to shift than the CR 8 one.
///
/// No `max_host_size`: a stirge lands on whatever it can reach.
static STIRGE_ATTACH: AttachProfile = AttachProfile {
    verb: "sinks its proboscis into",
    drain: Some((Dice::new(2, 4), DamageType::Necrotic)),
    blocked_while_attached: true,
    ..AttachProfile::defaults()
};

/// Stirge — CR 1/8 swarm-encounter staple. Tiny blood-drinking flier
/// whose proboscis latches on and then feeds every turn until somebody
/// pulls it off — see `STIRGE_ATTACH`. Low HP (5) and AC (13) means
/// they're individually fragile; the design intent is to throw them in
/// clouds of 4-6 around the party so AoE damage feels relevant again,
/// and the drain is what makes ignoring one expensive.
///
/// What shipped before was a stirge that re-stabbed for 1d4 every turn
/// with a docstring conceding the attach was unmodelled. The difference
/// is not the damage — it is that the old one could be walked away
/// from.
pub static STIRGE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&STIRGE_PROBOSCIS);
    CreatureTemplate {
        name: "Stirge",
        // 's' lower (small swarm creature).
        glyph: 's',
        ac: 13,
        hitpoints: "2d4".parse().unwrap(),
        // RAW speed line: Speed 10 ft., fly 40 ft.
        speed: 10.0,
        fly_speed: 40.0,
        strength: 4,
        intelligence: 2,
        dexterity: 16,
        wisdom: 8,
        constitution: 11,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.125,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        actions,
        attach: Some(&STIRGE_ATTACH),
        ..CreatureTemplate::defaults()
    }
});
