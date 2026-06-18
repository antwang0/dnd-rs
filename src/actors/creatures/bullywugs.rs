use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BULLYWUG_BITE, BULLYWUG_MULTI, BULLYWUG_SPEAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Bullywug — CR 1/4 small humanoid (frog-folk). Cheap swamp raider:
/// AC 15 (natural armor), 11 HP, a spear + bite compound multi for the
/// rare "one cheap mook gets two swings" envelope at the bottom of the
/// CR ladder. Slots between the goblin (CR 1/4 humanoid) and the
/// kobold (CR 1/8 humanoid) on the low-CR pack-fodder lane.
///
/// Stats: STR 12, DEX 12, CON 13, INT 7, WIS 10, CHA 7. No special
/// senses (RAW has none); speaks Bullywug — collapsed to Common in this
/// engine since Bullywug isn't a Language variant. The amphibious
/// 40 ft swim speed collapses to the standard 20 ft walk in our
/// no-water-terrain model.
pub static BULLYWUG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BULLYWUG_SPEAR);
    actions.push(&BULLYWUG_BITE);
    actions.push(&*BULLYWUG_MULTI);
    CreatureTemplate {
        name: "Bullywug",
        // 'φ' (Greek lowercase phi) — distinct from the heavily-shared
        // single-letter glyphs in the small / medium humanoid pool
        // (B/b/G/g all collide multiple times). The amphibian silhouette
        // reads cleanly as a swollen circle with a tail.
        glyph: 'φ',
        ac: 15,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 20.,
        strength: 12,
        intelligence: 7,
        dexterity: 12,
        wisdom: 10,
        constitution: 13,
        charisma: 7,
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        ..CreatureTemplate::defaults()
    }
});
