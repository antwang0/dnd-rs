use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{WEREWOLF_BITE, WEREWOLF_MULTIATTACK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Werewolf — CR 3 lycanthrope. Bite + claws multiattack, resistant to
/// the physical trio (5e RAW: "Damage Immunities Bludgeoning, Piercing,
/// and Slashing from Nonmagical Attacks That Aren't Silvered" — we
/// approximate as Resistance because we don't track magical/silvered
/// weapon flags). A bite that lands forces a CON save for a poisoned-
/// debuff lycanthropy-curse rider, paying the "is werewolf scary?" tax
/// without having to model a multi-day curse. Speed is bumped to 40 to
/// match the hybrid-form profile.
pub static WEREWOLF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WEREWOLF_BITE);
    actions.push(&*WEREWOLF_MULTIATTACK);
    CreatureTemplate {
        name: "Werewolf",
        // 'W' is taken by Wolf — use lowercase 'w'. Wraith uses 'R'.
        glyph: 'w',
        ac: 12,
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 40.,
        strength: 15,
        intelligence: 10,
        dexterity: 13,
        wisdom: 11,
        constitution: 14,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Lycanthrope resistance to the physical trio (modeling
        // non-magical/non-silver immunity as resistance — the test pool
        // doesn't include silver weapons so full immunity would make
        // werewolves untouchable).
        damage_modifiers: HashMap::from([
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
    }
});
