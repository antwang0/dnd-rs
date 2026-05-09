use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BITE, SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Troll — large regenerating brute (CR 5). High HP and two attacks
/// (claw + bite) per turn via the multiattack wrapper. Mechanically the
/// troll exercises the Large footprint and reach-2 adjacency the same
/// way the Ogre does, but with more staying power. Regeneration (3 HP
/// per round) is handled inside `EncounterInstance::round_end` by
/// checking the `regeneration` field on the actor.
///
/// Per 5e, acid/fire damage suppresses regeneration for one round and
/// a troll only truly dies when reduced to 0 HP by acid or fire. We
/// model this simply: the troll has a marker flag that `DealDamage`
/// sets when those types hit, and `round_end` skips the heal that round.
/// For now we give the troll only the passive regen without the
/// "suppress on fire/acid" clause — a good follow-up to add.
pub static TROLL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    // Claws (slam) + Bite — two attacks per action, matching 5e multiattack.
    actions.push(&*SLAM);
    actions.push(&*BITE);
    CreatureTemplate {
        name: "Troll",
        glyph: 'T',
        n_instances: 0,
        ac: 15,
        hitpoints: "8d10+24".parse().unwrap(),
        speed: 30.,
        strength: 18,
        intelligence: 7,
        dexterity: 13,
        wisdom: 9,
        constitution: 20,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Giant]),
        cr: 5.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        immunities: HashSet::new(),
        resistances: HashSet::new(),
        vulnerabilities: HashSet::new(),
        regeneration: 10,
    }
});
