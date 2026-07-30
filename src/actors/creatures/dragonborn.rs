use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, BREATH_WEAPON, BREATH_WEAPON_TAG, SECOND_WIND, SECOND_WIND_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATSWORD;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Shared level-3 Dragonborn Champion Fighter build. Every chromatic
/// dragonborn ancestry variant (Red / Black / Blue / Green / White)
/// pairs the same Champion-fighter chassis (Second Wind + Action Surge
/// + Improved Critical crit-on-19) with **one ancestry pick** that
///   drives (a) which damage type the breath weapon exhales via
///   `draconic_ancestry` and (b) which damage type the dragonborn is
///   resistant to via `damage_modifiers`. The per-ancestry swaps are
///   (1) display name, (2) glyph, and (3) the shared `damage_type`
///   parameter that lands as both the ancestry pick and the resistance
///   row.
///
/// The one-helper pattern (single ancestry parameter over a shared
/// envelope) mirrors `subclass_barbarian_template` on the barbarian
/// chassis (single subclass-tag parameter over the shared level-9
/// totem envelope) — same "single per-variant swap over a shared
/// chassis literal" declarative-helper shape, different swap axis
/// (ancestry damage type here vs. subclass feature tag there).
/// Collapses what would otherwise be N ~40-line struct literals into
/// a single call per `LazyLock`. Adding a new chromatic ancestry
/// (Yellow / Brown / homebrew Purple, etc.) lands as a one-line
/// entry.
///
/// Stat shape targets a level-3 Champion fighter: AC 16 (chain mail),
/// 28 HP (3d10+9), STR 17, greatsword as the signature 2d6 slashing
/// swing. Fighter class chassis (Second Wind + Action Surge) plus the
/// Champion's Improved Critical (crit on 19-20) for the spike-damage
/// niche. The Breath Weapon adds a slot-free AoE on the round where
/// the dragonborn opens against multiple targets.
fn dragonborn_champion_template(
    name: &'static str,
    glyph: char,
    damage_type: DamageType,
) -> CreatureTemplate {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATSWORD);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    // Racial: Breath Weapon — once per short rest.
    actions.push(&*BREATH_WEAPON);
    CreatureTemplate {
        name,
        glyph,
        ac: 16,
        hitpoints: "3d10+9".parse().unwrap(),
        strength: 17,
        dexterity: 12,
        constitution: 16,
        intelligence: 10,
        wisdom: 11,
        charisma: 14, // mild CHA bump per Dragonborn racial
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        // 5e Dragonborn Damage Resistance: ancestor's damage type.
        damage_modifiers: HashMap::from([(damage_type, DamageModifier::Resistance)]),
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
        ]),
        // Fighter class features + the racial Breath Weapon flag.
        features: HashSet::from([SECOND_WIND_TAG, ACTION_SURGE_TAG, BREATH_WEAPON_TAG]),
        // Champion Improved Critical: crits trigger on 19 or 20.
        crit_threshold: 19,
        // 5e Draconic Ancestry drives the breath weapon's damage type
        // via the `draconic_ancestry()` accessor.
        draconic_ancestry: Some(damage_type),
        ..CreatureTemplate::defaults()
    }
}

/// Red Dragonborn Champion — STR-primary fighter on a Champion chassis
/// (Improved Critical via `crit_threshold = 19`). The defining racial
/// traits are the **Draconic Ancestry** pair:
///   - **Damage Resistance**: resistance to the ancestor's damage type
///     (fire for Red Dragonborn). Folded into `damage_modifiers`.
///   - **Breath Weapon**: once per short rest, exhale a 15-ft cone of
///     the ancestor's damage type (DEX save half, scales with level).
///     Gated by the `BREATH_WEAPON_TAG` feature flag and the
///     `BreathWeapon` action — refreshes on short rest via the
///     `SHORT_REST_FEATURES` registry, mirroring Second Wind / Action
///     Surge / Arcane Recovery's short-rest cadence.
///
/// Glyph 'Δ' (uppercase delta) — a flavorful sigil that doesn't
/// collide with any existing humanoid glyph. The four chromatic
/// siblings pick distinct Greek capitals (Θ / Λ / Γ / Χ) so the
/// Chromatic Dragonborn family renders unambiguously on the map.
pub static DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Red Dragonborn Champion", '\u{0394}', DamageType::Fire)
});

/// Black Dragonborn Champion — Chromatic ancestry: **Black** (acid).
/// Sibling of `DRAGONBORN_TEMPLATE` (Red / Fire) on the shared
/// `dragonborn_champion_template` helper — same Champion chassis,
/// same Second Wind / Action Surge / Breath Weapon envelope, ancestry
/// swapped to Acid. The Black ancestry pick surfaces the first user
/// of the **Acid** slot on the `draconic_ancestry` accessor — the
/// baseline Red Dragonborn covers Fire, and Chromatic Blue / Green /
/// White siblings cover Lightning / Poison / Cold on this cohort.
///
/// Glyph 'Θ' (uppercase theta) — distinct from the baseline Red
/// dragonborn 'Δ' and the sibling Blue 'Λ' / Green 'Γ' / White 'Χ'
/// chromatic variants. Θ was picked for its round-closed shape,
/// evoking the black dragon's cauldron-like acid pool.
pub static BLACK_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Black Dragonborn Champion", 'Θ', DamageType::Acid)
});

/// Blue Dragonborn Champion — Chromatic ancestry: **Blue** (lightning).
/// Sibling of `DRAGONBORN_TEMPLATE` (Red / Fire), `BLACK_DRAGONBORN_TEMPLATE`
/// (Black / Acid), `GREEN_DRAGONBORN_TEMPLATE` (Green / Poison), and
/// `WHITE_DRAGONBORN_TEMPLATE` (White / Cold) on the shared
/// `dragonborn_champion_template` helper — same Champion chassis,
/// ancestry swapped to Lightning. Overlaps the Lightning axis with
/// Heart of the Storm (Storm Sorcerer lv6) and Storm Soul (Sea)
/// (Barbarian Path of the Storm Herald lv6) on different chassis —
/// the three never legally co-occur on a single build (Dragonborn is
/// a race, not a class; Storm Sorcerer / Storm Herald are subclass
/// picks on different classes), and a hypothetical multiclass carrier
/// caps at a single /2 per Lightning hit via the "one halving per
/// damage instance" rule.
///
/// Glyph 'Λ' (uppercase lambda) — distinct from the baseline Red
/// dragonborn 'Δ' and the sibling Black 'Θ' / Green 'Γ' / White 'Χ'
/// chromatic variants. Λ was picked for its lightning-bolt-like
/// silhouette (the two diagonals meeting at a peak evoke a fork of
/// electricity).
pub static BLUE_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Blue Dragonborn Champion", 'Λ', DamageType::Lightning)
});

/// Green Dragonborn Champion — Chromatic ancestry: **Green** (poison).
/// Sibling of `DRAGONBORN_TEMPLATE` (Red / Fire), `BLACK_DRAGONBORN_TEMPLATE`
/// (Black / Acid), `BLUE_DRAGONBORN_TEMPLATE` (Blue / Lightning), and
/// `WHITE_DRAGONBORN_TEMPLATE` (White / Cold) on the shared
/// `dragonborn_champion_template` helper — same Champion chassis,
/// ancestry swapped to Poison. Overlaps the Poison axis with the
/// Dwarven Resilience racial trait (Dwarves — resistance to poison
/// damage) on a different chassis; the two never legally co-occur on
/// a single build (Dragonborn vs. Dwarf race pick), and a
/// hypothetical multiclass carrier caps at a single /2 per Poison hit
/// via the "one halving per damage instance" rule. Distinct from
/// Purity of Body (Monk lv10) which grants full immunity to poison
/// damage — the Green Dragonborn variant halves, doesn't zero.
///
/// Glyph 'Γ' (uppercase gamma) — distinct from the baseline Red
/// dragonborn 'Δ' and the sibling Black 'Θ' / Blue 'Λ' / White 'Χ'
/// chromatic variants. Γ was picked for its angular hook shape,
/// evoking the sickle-curve of a green dragon's serpentine breath.
pub static GREEN_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Green Dragonborn Champion", 'Γ', DamageType::Poison)
});

/// White Dragonborn Champion — Chromatic ancestry: **White** (cold).
/// Sibling of `DRAGONBORN_TEMPLATE` (Red / Fire), `BLACK_DRAGONBORN_TEMPLATE`
/// (Black / Acid), `BLUE_DRAGONBORN_TEMPLATE` (Blue / Lightning), and
/// `GREEN_DRAGONBORN_TEMPLATE` (Green / Poison) on the shared
/// `dragonborn_champion_template` helper — same Champion chassis,
/// ancestry swapped to Cold. Overlaps the Cold axis with Marid's
/// Elemental Gift (Warlock Genie Marid Patron lv6) and Storm Soul
/// (Tundra) (Barbarian Path of the Storm Herald lv6) on different
/// chassis — the three never legally co-occur on a single build
/// (Dragonborn is a race; Marid Warlock / Tundra Storm Herald are
/// subclass picks on different classes), and a hypothetical
/// multiclass carrier caps at a single /2 per Cold hit via the "one
/// halving per damage instance" rule.
///
/// Glyph 'Χ' (uppercase chi) — distinct from the baseline Red
/// dragonborn 'Δ' and the sibling Black 'Θ' / Blue 'Λ' / Green 'Γ'
/// chromatic variants. Χ was picked for its crossed-lines shape,
/// evoking the snowflake / frost-crystal identity of the White
/// dragon's breath.
pub static WHITE_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("White Dragonborn Champion", 'Χ', DamageType::Cold)
});

/// Amethyst Dragonborn Champion — Gem ancestry: **Amethyst** (force).
/// Second draconic ancestry taxa on the dragonborn chassis, extending
/// the roster beyond the classic five-flavor Chromatic set (Red /
/// Black / Blue / Green / White) that hangs off `DRAGONBORN_TEMPLATE`
/// and its four immediate siblings. Gem Dragonborn (Fizban's Treasury
/// of Dragons) share the Chromatic's Champion chassis wholesale — same
/// AC / HP / stat block / Second Wind + Action Surge / Breath Weapon
/// short-rest charge — with the ancestry damage type swapped to reflect
/// the gem dragon's exhale. The Amethyst variant covers **force** —
/// the first user of the **Force** slot on both the `draconic_ancestry`
/// accessor AND the entire passive typed-resistance / damage-modifier
/// lane. Force is the signature damage type of Force-anchored spells
/// (Magic Missile, Bigby's Hand, Disintegrate, Eldritch Blast); prior
/// to this variant, no PC race, no class subclass, and no monster
/// template carried a resistance row on the Force axis — the Amethyst
/// Dragonborn is the first Force-resistant chassis anywhere in the
/// engine.
///
/// Sibling of `DRAGONBORN_TEMPLATE` (Red / Fire), `BLACK_DRAGONBORN_TEMPLATE`
/// (Black / Acid), `BLUE_DRAGONBORN_TEMPLATE` (Blue / Lightning),
/// `GREEN_DRAGONBORN_TEMPLATE` (Green / Poison), and
/// `WHITE_DRAGONBORN_TEMPLATE` (White / Cold) on the shared
/// `dragonborn_champion_template` helper — same Champion chassis,
/// ancestry swapped to Force. Also sibling of the four Gem cousins
/// below (Crystal / Emerald / Sapphire / Topaz) on the "Gem ancestry
/// taxa" sub-family; the Chromatic + Gem set collectively covers 5
/// new damage axes (Force / Radiant / Psychic / Thunder / Necrotic)
/// on the dragonborn chassis beyond the 5 Chromatic slots (Fire /
/// Acid / Lightning / Poison / Cold).
///
/// Glyph 'Φ' (uppercase phi) — distinct from the baseline Red
/// dragonborn 'Δ' and every Chromatic sibling (Black 'Θ' / Blue 'Λ' /
/// Green 'Γ' / White 'Χ'), and distinct from the four Gem cousins
/// below ('✧' / '⬢' / '◆' / '❖'). Φ was picked for its rounded-vertical
/// shape, evoking a faceted amethyst gemstone standing upright.
pub static AMETHYST_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Amethyst Dragonborn Champion", 'Φ', DamageType::Force)
});

/// Crystal Dragonborn Champion — Gem ancestry: **Crystal** (radiant).
/// Sibling of the four Chromatic dragonborn variants and the four Gem
/// cousins on the shared `dragonborn_champion_template` helper — same
/// Champion chassis, ancestry swapped to Radiant. Overlaps the Radiant
/// axis with Radiant Soul (Warlock Celestial Patron lv6, XGtE) on a
/// different chassis; the two never legally co-occur on a single build
/// (Dragonborn is a race; Celestial Warlock is a subclass pick on the
/// warlock class), and a hypothetical multiclass carrier caps at a
/// single /2 per Radiant hit via the "one halving per damage instance"
/// rule. First dragonborn-chassis row on the Radiant axis — prior to
/// this variant, only the Celestial Warlock covered Radiant resistance.
///
/// Glyph '✧' (WHITE FOUR POINTED STAR, U+2727) — distinct from every
/// Chromatic Greek-capital glyph (Δ / Θ / Λ / Γ / Χ), the sibling
/// Amethyst 'Φ', and the three Gem cousins below (⬢ / ◆ / ❖). ✧ was
/// picked for its sparkling four-pointed shape, evoking a facet of
/// clear crystal catching the light — the crystal dragon's radiant
/// breath weaponized. Distinct from the Deva's '✦' (BLACK FOUR POINTED
/// STAR, U+2726): filled vs. outline forms of the same shape.
pub static CRYSTAL_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Crystal Dragonborn Champion", '✧', DamageType::Radiant)
});

/// Emerald Dragonborn Champion — Gem ancestry: **Emerald** (psychic).
/// Sibling of the four Chromatic dragonborn variants and the four Gem
/// cousins on the shared `dragonborn_champion_template` helper — same
/// Champion chassis, ancestry swapped to Psychic. Overlaps the Psychic
/// axis with Psychic Defenses (Sorcerer Aberrant Mind lv14, TCE) on a
/// different chassis; the two never legally co-occur on a single build
/// (Dragonborn is a race; Aberrant Mind Sorcerer is a subclass pick on
/// the sorcerer class), and a hypothetical multiclass carrier caps at
/// a single /2 per Psychic hit via the "one halving per damage
/// instance" rule. First dragonborn-chassis row on the Psychic axis —
/// prior to this variant, only the Aberrant Mind Sorcerer covered
/// Psychic resistance.
///
/// Glyph '⬢' (BLACK HEXAGON, U+2B22) — distinct from every Chromatic
/// Greek-capital glyph (Δ / Θ / Λ / Γ / Χ) and every other Gem cousin
/// (Φ / ✧ / ◆ / ❖). ⬢ was picked for its faceted six-sided shape,
/// evoking the emerald-cut hexagonal profile a jeweler carves into
/// green beryl. The hexagon-face read anchors the emerald dragon's
/// crystalline body-plate identity in Fizban's illustrations.
pub static EMERALD_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Emerald Dragonborn Champion", '⬢', DamageType::Psychic)
});

/// Sapphire Dragonborn Champion — Gem ancestry: **Sapphire** (thunder).
/// Sibling of the four Chromatic dragonborn variants and the four Gem
/// cousins on the shared `dragonborn_champion_template` helper — same
/// Champion chassis, ancestry swapped to Thunder. Overlaps the Thunder
/// axis with Heart of the Storm (Storm Sorcerer lv6) and Djinni's
/// Elemental Gift (Warlock Genie Djinni Patron lv6, TCE) on different
/// chassis — the three never legally co-occur on a single build
/// (Dragonborn is a race; Storm Sorcerer / Djinni Warlock are subclass
/// picks on different classes), and a hypothetical multiclass carrier
/// caps at a single /2 per Thunder hit via the "one halving per damage
/// instance" rule. First dragonborn-chassis row on the Thunder axis —
/// prior to this variant, only Heart of the Storm and Djinni's
/// Elemental Gift covered Thunder resistance.
///
/// Glyph '◆' (BLACK DIAMOND, U+25C6) — distinct from every Chromatic
/// Greek-capital glyph (Δ / Θ / Λ / Γ / Χ) and every other Gem cousin
/// (Φ / ✧ / ⬢ / ❖). ◆ was picked for its solid brilliant-cut diamond
/// silhouette, evoking a rich blue sapphire held to the light — the
/// signature stone of the sapphire dragon's boulder-scaled hide.
pub static SAPPHIRE_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Sapphire Dragonborn Champion", '◆', DamageType::Thunder)
});

/// Topaz Dragonborn Champion — Gem ancestry: **Topaz** (necrotic).
/// Sibling of the four Chromatic dragonborn variants and the four Gem
/// cousins on the shared `dragonborn_champion_template` helper — same
/// Champion chassis, ancestry swapped to Necrotic. Overlaps the
/// Necrotic axis with Inured to Undeath (Wizard School of Necromancy
/// lv10, PHB) on a different chassis; the two never legally co-occur
/// on a single build (Dragonborn is a race; Necromancy Wizard is a
/// subclass pick on the wizard class), and a hypothetical multiclass
/// carrier caps at a single /2 per Necrotic hit via the "one halving
/// per damage instance" rule. First dragonborn-chassis row on the
/// Necrotic axis — prior to this variant, only the Necromancy Wizard
/// covered Necrotic resistance.
///
/// Glyph '❖' (BLACK DIAMOND MINUS WHITE X, U+2756) — distinct from
/// every Chromatic Greek-capital glyph (Δ / Θ / Λ / Γ / Χ) and every
/// other Gem cousin (Φ / ✧ / ⬢ / ◆). ❖ was picked for its four-petal
/// diamond-flower shape, evoking a topaz gemstone's tapered faceted
/// crown. Fizban's topaz dragon breathes desiccating necrotic energy;
/// the sunburnt yellow-orange hue of a topaz gem doubles as the
/// weathered pallor of a necromantic breath's wake.
pub static TOPAZ_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Topaz Dragonborn Champion", '❖', DamageType::Necrotic)
});

/// Silver Dragonborn Champion — Metallic ancestry: **Silver** (cold).
/// Third draconic-ancestry taxa on the dragonborn chassis, opening the
/// **Metallic** family beyond the classic five-flavor Chromatic set
/// (Red / Black / Blue / Green / White) that hangs off
/// `DRAGONBORN_TEMPLATE` and its four immediate siblings, and the
/// five-flavor Gem set (Amethyst / Crystal / Emerald / Sapphire /
/// Topaz) that hangs off `AMETHYST_DRAGONBORN_TEMPLATE` and its four
/// cousins. Metallic Dragonborn (PHB / Fizban's Treasury of Dragons)
/// share the Chromatic / Gem Champion chassis wholesale — same AC / HP
/// / stat block / Second Wind + Action Surge / Breath Weapon short-
/// rest charge — with the ancestry damage type swapped to reflect the
/// metallic dragon's exhale. The Silver variant covers **cold** — the
/// silver dragon's breath is a paralyzing frost cone, the most
/// mechanically pure Cold breath weapon on the metallic side.
///
/// Sibling of `DRAGONBORN_TEMPLATE` (Red / Fire), `BLACK_DRAGONBORN_TEMPLATE`
/// (Black / Acid), `BLUE_DRAGONBORN_TEMPLATE` (Blue / Lightning),
/// `GREEN_DRAGONBORN_TEMPLATE` (Green / Poison), `WHITE_DRAGONBORN_TEMPLATE`
/// (White / Cold), and every Gem cousin on the shared
/// `dragonborn_champion_template` helper — same Champion chassis,
/// ancestry swapped to Cold.
///
/// Overlaps the Cold axis with **White Dragonborn** (Chromatic — Cold),
/// **Marid's Elemental Gift** (Warlock Genie Marid Patron lv6, TCE), and
/// **Storm Soul (Tundra)** (Barbarian Path of the Storm Herald lv6,
/// XGtE) on different chassis — the four never legally co-occur on a
/// single build (Dragonborn is a race; White is a distinct ancestry
/// pick on the same race; Marid Warlock / Tundra Storm Herald are
/// subclass picks on different classes), and a hypothetical multiclass
/// carrier caps at a single /2 per Cold hit via the "one halving per
/// damage instance" rule. This variant is the second **dragonborn**-
/// chassis row on the Cold axis (White Dragonborn shipped first) — the
/// duplication is a **taxonomic completeness** grant, not a mechanical-
/// coverage grant, matching the way the Efreeti Warlock's Fire
/// resistance duplicates the Fiendish / Draconic Resilience Fire rows
/// for the sake of the "four Genie kinds" quadrant coverage. Here the
/// duplication anchors the "Silver as the flagship Metallic entry"
/// slot — the first Metallic dragonborn on the roster.
///
/// Glyph 'Υ' (uppercase Greek upsilon) — distinct from every Chromatic
/// Greek-capital glyph (Δ / Θ / Λ / Γ / Χ) and every Gem cousin (Φ / ✧
/// / ⬢ / ◆ / ❖). Υ was picked for its stalactite-like vertical stem
/// widening into a two-pronged crown, evoking a silver dragon's frost-
/// horns or the icy stalagmite silhouette a silver dragon's Cold breath
/// leaves in its wake. Collides with no other current PC template glyph
/// (no baseline creature or subclass uses uppercase upsilon today).
///
/// Ships on the same CR-2 Champion chassis as the Chromatic / Gem
/// siblings above — class templates target a balanced playable level,
/// not lockstep PHB progression. Rounds out the dragonborn-family
/// taxonomic coverage: Chromatic (5 variants — Fire / Acid / Lightning
/// / Poison / Cold) + Gem (5 variants — Force / Radiant / Psychic /
/// Thunder / Necrotic) + Metallic (this variant, first of the flavor)
/// = 11 total ancestry picks off the shared helper.
pub static SILVER_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Silver Dragonborn Champion", 'Υ', DamageType::Cold)
});

/// Brass Dragonborn Champion — Metallic ancestry: **Brass** (fire).
/// Second Metallic ancestry variant on the shared
/// `dragonborn_champion_template` helper — the desert-dwelling talker
/// of the Metallic set, riding the same Champion chassis (Improved
/// Critical crit-on-19) and once-per-short-rest Breath Weapon feature
/// charge as the eleven Chromatic / Gem / Silver siblings. The Brass
/// variant covers **fire** — the brass dragon's signature exhale, a
/// scorching line of desert heat.
///
/// Overlaps the Fire axis with **Red Dragonborn** (the baseline
/// Chromatic ancestry — Fire), **Efreeti Warlock's Elemental Gift**
/// (Warlock Genie Efreeti Patron lv6, TCE), **Storm Soul (Desert)**
/// (Barbarian Path of the Storm Herald lv6, XGtE), **Soul of the Forge**
/// (Cleric Forge Domain lv6, XGtE), and **Fire Genasi** and **Tiefling
/// Hellish Resistance** on different chassis — the seven never legally
/// co-occur on a single build (Dragonborn is a race, distinct from
/// Genasi / Tiefling races and from Warlock / Barbarian / Cleric class
/// subclass picks), and a hypothetical multiclass carrier caps at a
/// single /2 per Fire hit via the "one halving per damage instance"
/// rule. This variant is the second **dragonborn**-chassis row on the
/// Fire axis (Red Dragonborn shipped first) — the duplication is a
/// **taxonomic completeness** grant, not a mechanical-coverage grant,
/// mirroring the Silver / White Cold-axis duplication on the same
/// chassis. The brass-fire pairing anchors the "brass = talkative
/// desert dragon" flavor slot on the Metallic taxa.
///
/// Glyph '⚒' (HAMMER AND PICK, U+2692) — distinct from every Chromatic
/// Greek-capital glyph (Δ / Θ / Λ / Γ / Χ), every Gem cousin (Φ / ✧ /
/// ⬢ / ◆ / ❖), the flagship Silver Metallic (Υ), and every other
/// Metallic sibling below (⚡ / ⚗ / ☼). ⚒ was picked for its
/// smith-tool silhouette, evoking the brass-forge hammer-and-pick of a
/// desert brass dragon's smoky, industrious kiln.
pub static BRASS_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Brass Dragonborn Champion", '⚒', DamageType::Fire)
});

/// Bronze Dragonborn Champion — Metallic ancestry: **Bronze** (lightning).
/// Third Metallic ancestry variant on the shared
/// `dragonborn_champion_template` helper — the coastal storm-patroller
/// of the Metallic set, riding the same Champion chassis and Breath
/// Weapon short-rest charge as the twelve Chromatic / Gem / Silver /
/// Brass siblings. The Bronze variant covers **lightning** — the bronze
/// dragon's signature exhale, a rolling coastal thunder-line.
///
/// Overlaps the Lightning axis with **Blue Dragonborn** (Chromatic —
/// Lightning), **Heart of the Storm** (Storm Sorcerer lv6, XGtE), and
/// **Storm Soul (Sea)** (Barbarian Path of the Storm Herald lv6, XGtE)
/// on different chassis — the four never legally co-occur on a single
/// build (Dragonborn is a race; Blue is a distinct ancestry pick on the
/// same race; Storm Sorcerer / Sea Storm Herald are subclass picks on
/// different classes), and a hypothetical multiclass carrier caps at a
/// single /2 per Lightning hit via the "one halving per damage instance"
/// rule. Second **dragonborn**-chassis row on the Lightning axis (Blue
/// Dragonborn shipped first) — same "taxonomic completeness" grant
/// pattern as the Silver / White Cold duplication and the Brass / Red
/// Fire duplication.
///
/// Glyph '⚡' (HIGH VOLTAGE SIGN, U+26A1) — distinct from every
/// Chromatic Greek-capital glyph (Δ / Θ / Λ / Γ / Χ), every Gem cousin
/// (Φ / ✧ / ⬢ / ◆ / ❖), the Metallic siblings (Υ / ⚒ / ⚗ / ☼). ⚡ was
/// picked for its lightning-bolt silhouette, mapping directly to the
/// bronze dragon's electric line-breath.
pub static BRONZE_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Bronze Dragonborn Champion", '⚡', DamageType::Lightning)
});

/// Copper Dragonborn Champion — Metallic ancestry: **Copper** (acid).
/// Fourth Metallic ancestry variant on the shared
/// `dragonborn_champion_template` helper — the trickster hillside dragon
/// of the Metallic set, riding the same Champion chassis and Breath
/// Weapon short-rest charge as the thirteen Chromatic / Gem / Silver /
/// Brass / Bronze siblings. The Copper variant covers **acid** — the
/// copper dragon's signature exhale, a corrosive stream that dissolves
/// through stone and steel alike.
///
/// Overlaps the Acid axis with **Black Dragonborn** (Chromatic — Acid)
/// on the same dragonborn chassis — the two never legally co-occur on a
/// single build (Copper and Black are distinct ancestry picks on the
/// same race), and a hypothetical multiclass carrier caps at a single
/// /2 per Acid hit via the "one halving per damage instance" rule.
/// Second **dragonborn**-chassis row on the Acid axis (Black
/// Dragonborn shipped first) — same "taxonomic completeness" grant
/// pattern as the other duplicated Metallic / Chromatic pairings.
///
/// Glyph '⚗' (ALEMBIC, U+2697) — distinct from every Chromatic
/// Greek-capital glyph (Δ / Θ / Λ / Γ / Χ), every Gem cousin (Φ / ✧ /
/// ⬢ / ◆ / ❖), and every Metallic sibling (Υ / ⚒ / ⚡ / ☼). ⚗ was
/// picked for its alchemist's-distillation-vessel silhouette, evoking
/// the corrosive acid brew the copper dragon spits from its narrow
/// gullet — the alchemy sigil doubles as the copper wire coil an
/// alembic's condenser rides.
pub static COPPER_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Copper Dragonborn Champion", '⚗', DamageType::Acid)
});

/// Gold Dragonborn Champion — Metallic ancestry: **Gold** (fire).
/// Fifth and final Metallic ancestry variant on the shared
/// `dragonborn_champion_template` helper, closing out the classic
/// five-flavor Metallic set (Brass / Bronze / Copper / Gold / Silver)
/// alongside the five-flavor Chromatic (Red / Black / Blue / Green /
/// White) and five-flavor Gem (Amethyst / Crystal / Emerald / Sapphire /
/// Topaz) taxa. The Gold variant covers **fire** — the gold dragon's
/// signature exhale, the wise sun-anointed breath of the most powerful
/// Metallic dragon.
///
/// Overlaps the Fire axis with **Red Dragonborn** (Chromatic — Fire) and
/// **Brass Dragonborn** (Metallic — Fire) on the same dragonborn chassis
/// — the three never legally co-occur on a single build (Red / Brass /
/// Gold are distinct ancestry picks on the same race), and a
/// hypothetical multiclass carrier caps at a single /2 per Fire hit via
/// the "one halving per damage instance" rule. Third **dragonborn**-
/// chassis row on the Fire axis (Red shipped first, Brass second) —
/// mirroring the Silver / White Cold duplication, extended to a triple.
/// The gold-fire pairing anchors the "gold as the flagship Metallic
/// entry" slot: the wisest, most powerful Metallic dragon of D&D
/// canonical lore lands on the same signature damage type as its
/// Chromatic counterpart Red, reflecting the parallel evolutionary
/// niche that Chromatic and Metallic families occupy in draconic
/// biology.
///
/// This variant completes the 15-ancestry roster on the shared
/// `dragonborn_champion_template` helper: 5 Chromatic + 5 Gem + 5
/// Metallic = 15 total. Damage axis coverage on the dragonborn chassis:
/// Fire (Red / Brass / Gold), Cold (White / Silver), Acid (Black /
/// Copper), Lightning (Blue / Bronze), Poison (Green), Force (Amethyst),
/// Radiant (Crystal), Psychic (Emerald), Thunder (Sapphire), Necrotic
/// (Topaz) — 10 damage axes covered by 15 ancestry variants.
///
/// Glyph '☼' (WHITE SUN WITH RAYS, U+263C) — distinct from every
/// Chromatic Greek-capital glyph (Δ / Θ / Λ / Γ / Χ), every Gem cousin
/// (Φ / ✧ / ⬢ / ◆ / ❖), and every other Metallic sibling (Υ / ⚒ / ⚡ /
/// ⚗). ☼ was picked for its radiant-sun silhouette, evoking the gold
/// dragon's sun-anointed hide and the divine-golden warmth of its
/// signature fire breath — the sun-with-rays reads as both the gold-
/// leaf lustre of a gold dragon's scales and the ember-glow of the
/// fire it exhales.
pub static GOLD_DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    dragonborn_champion_template("Gold Dragonborn Champion", '☼', DamageType::Fire)
});
