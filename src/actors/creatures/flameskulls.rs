use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{FIRE_BOLT, FIREBALL, MAGIC_MISSILE};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Flameskull — CR 4 undead spellcaster. A disembodied skull wreathed
/// in green flame that hovers through dungeons hurling Fire Bolt, Magic
/// Missile, and the occasional Fireball. Tiny size, absurdly fragile
/// (5d4+5 ≈ 17 HP) but offset by fire/poison immunity, resistance to
/// lightning/necrotic/piercing, and Magic Resistance on every save.
/// The spell loadout makes it a glass cannon that punishes clusters.
pub static FLAMESKULL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*FIREBALL);
    actions.push(&*MAGIC_MISSILE);
    CreatureTemplate {
        name: "Flameskull",
        // 'F' for Flameskull — distinct from fire elementals ('f').
        glyph: 'F',
        ac: 13,
        hitpoints: "5d4+5".parse().unwrap(),
        // Hover 40 ft — modeled as ground speed.
        speed: 40.,
        strength: 1,
        dexterity: 17,
        constitution: 12,
        intelligence: 16,
        wisdom: 10,
        charisma: 11,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        // 5e **Illumination**: "the flameskull sheds either bright
        // light in a 15-foot radius and dim light for an additional
        // 15 feet, or dim light in a 15-foot radius. It can switch
        // between the options as an action." The dimmed option is
        // dropped — it is an action spent to be harder to see, which
        // no stat block in the engine has a reason to take and the AI
        // has no rung for. The lit option is the one it fights under.
        innate_light: Some((6, 6)),
        // Flameskulls understand Common and retain languages from life
        // but can't speak — we list Common for targeting / interaction.
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Tiny,
        creature_type: CreatureType::Undead,
        actions,
        // Flameskull has innate spellcasting: 3rd-level slots for Fireball,
        // 1st-level slots for Magic Missile. We give it 2 slots at level 3
        // and 3 slots at level 1 so it can cast Fireball twice and Magic
        // Missile three times per encounter.
        spell_slots_by_level: vec![3, 0, 2],
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
        ]),
        // Undead + hovering condition immunities.
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Poisoned,
            Condition::Prone,
        ]),
        has_magic_resistance: true,
        ..CreatureTemplate::defaults()
    }
});
