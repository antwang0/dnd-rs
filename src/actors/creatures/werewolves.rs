use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{WEREWOLF_BITE, WEREWOLF_MULTIATTACK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Werewolf — CR 3 lycanthrope. Bite + claws multiattack, resistant to
/// the physical trio (5e RAW: "Damage Immunities Bludgeoning, Piercing,
/// and Slashing from Nonmagical Attacks That Aren't Silvered" — the
/// qualifier is real (see `engine::magic`), the *immunity* is
/// approximated as resistance; see the template body). A bite that
/// lands forces a CON save for a poisoned-
/// debuff lycanthropy-curse rider, paying the "is werewolf scary?" tax
/// without having to model a multi-day curse. Speed is bumped to 40 to
/// match the hybrid-form profile.
pub static WEREWOLF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WEREWOLF_BITE);
    actions.push(&*WEREWOLF_MULTIATTACK);
    CreatureTemplate {
        name: "Werewolf",
        // 'W' is taken by Wolf — use lowercase 'w'. Wraith uses 'R'.
        glyph: 'w',
        ac: 15,
        hitpoints: "11d8+22".parse().unwrap(),
        speed: 40.,
        strength: 16,
        dexterity: 14,
        constitution: 14,
        intelligence: 10,
        wisdom: 11,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Lycanthrope resistance to the physical trio, qualified to
        // nonmagical attacks that aren't silvered — both exemptions
        // real, through `engine::magic`. RAW's *immunity* is still
        // approximated as resistance, and deliberately: a CR-3
        // werewolf that a party without a magic weapon and without a
        // 100 gp coating simply cannot hurt is RAW and is not a
        // fight. Halving leaves them wanting an answer and still able
        // to have the fight without one.
        skills: HashSet::from([Skill::Perception]),
        ..CreatureTemplate::resistant_to_nonmagical_nonsilvered_physical()
    }
});
