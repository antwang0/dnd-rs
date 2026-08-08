use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BULLYWUG_BITE, BULLYWUG_MULTI, SPEAR, THROWN_SPEAR};
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
/// 40 ft swim speed contributes its `SWIM_SPEED_TAG` and collapses
/// its magnitude into the standard 20 ft walk in our
/// no-water-terrain model.
pub static BULLYWUG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    // The shared `SPEAR`, not a bespoke one. `BULLYWUG_SPEAR` used to
    // sit in the armoury as a `SimpleWeapon::melee(STR, 1d6,
    // Piercing)` — which is `SPEAR` with a different display name and
    // nothing else, so the bullywug carried a private copy of a weapon
    // the armoury already had and the underwater melee cohort, which
    // matches on the word "spear", exempted one of them and taxed the
    // other.
    //
    // And the throw, which RAW's stat block has always had: "Spear.
    // Melee or Ranged Weapon Attack: +3 to hit, reach 5 ft. or range
    // 20/60 ft." A bullywug is an amphibian that fights at the water's
    // edge, so it is on both sides of the underwater rules at once —
    // the spear keeps its edge in the water and carries through it.
    actions.push(&SPEAR);
    actions.push(&THROWN_SPEAR);
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
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
        ..CreatureTemplate::defaults()
    }
});
