use crate::actions::class_features::UNDERWATER_BREATHING_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREEN_HAG_CLAWS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Green Hag — CR 3 medium fey. The classic swamp witch: chunky 2d8 +
/// STR claws and the canonical magic-resistance envelope (advantage on
/// saves vs spells / magical effects). RAW also carries Mimicry,
/// Invisible Passage, and limited spellcasting (Dancing Lights, Minor
/// Illusion, Vicious Mockery); we skip those since the engine doesn't
/// model the illusion / stealth-from-the-swamp clauses and the magic-
/// resistance + heavy claws are the load-bearing identity. Sits in the
/// mid-CR fey slot alongside Cult Fanatic / Phase Spider — a saver
/// that punishes spell-heavy parties.
pub static GREEN_HAG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREEN_HAG_CLAWS);
    CreatureTemplate {
        name: "Green Hag",
        // 'G' is taken by Gnoll / Ghoul; 'g' lowercase fits a medium fey.
        glyph: 'g',
        ac: 17, // natural armor — the swamp hag's hide is bark-tough.
        // 11d8+33 = 82 average per MM (CR 3).
        hitpoints: "11d8+33".parse().unwrap(),
        speed: 30.,
        strength: 18,
        intelligence: 13,
        dexterity: 12,
        wisdom: 14,
        constitution: 16,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Draconic, Language::Sylvan]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Fey,
        actions,
        // Magic Resistance — advantage on saves vs spells / magical
        // effects. The load-bearing fey trait at this tier.
        has_magic_resistance: true,
        // Amphibious — "the hag can breathe air and water". The swamp
        // is where she lives, and it is the one clause of her stat
        // block the water on this board can read. No swimming speed
        // beside it: RAW gives the green hag lungs for the water and
        // no particular grace in it, so a pool still charges her
        // double to wade through.
        features: HashSet::from([UNDERWATER_BREATHING_TAG]),
        ..CreatureTemplate::defaults()
    }
});
