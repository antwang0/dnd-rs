/// Flat sum of every passive bonus an actor gets from carried items.
/// Each `ActorInstance` aggregates one of these on demand from its
/// inventory. Keeping this a single struct (rather than per-stat lookups)
/// means stat accessors fold in item bonuses with one method call instead
/// of N inventory walks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ItemBonuses {
    pub ac: i32,
    pub max_hp: i32,
    /// Speed bonus is integer feet (5e items always grant whole-foot
    /// values like +10 boots). Stored as i32 to allow future debuffs.
    pub speed: i32,
    /// Flat bonus added to every saving throw modifier.
    pub save: i32,
}

impl ItemBonuses {
    /// All-zero baseline. Use as the tail of a struct-update literal so
    /// an item that only bumps one stat doesn't enumerate the three zero
    /// fields — `ItemBonuses { ac: 1, ..ItemBonuses::ZERO }` reads cleaner
    /// than the four-field literal. Const-evaluable so static items can
    /// build off it.
    pub const ZERO: ItemBonuses = ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 0,
    };
}

impl std::ops::Add for ItemBonuses {
    type Output = ItemBonuses;
    fn add(self, other: ItemBonuses) -> ItemBonuses {
        ItemBonuses {
            ac: self.ac + other.ac,
            max_hp: self.max_hp + other.max_hp,
            speed: self.speed + other.speed,
            save: self.save + other.save,
        }
    }
}

/// A piece of equipment. Items split implicitly into "passive trinket"
/// (only `bonuses` populated, `on_use = None`) and "consumable"
/// (`on_use` references a static action). Consumables don't grant
/// passive bonuses today — if a future item needs both, it just sets
/// both fields. Items are referenced via `&'static Item` so cloning an
/// inventory is cheap and definitions stay single-sourced.
#[derive(Clone, Copy)]
pub struct Item {
    pub name: &'static str,
    /// Map glyph for ground rendering. Convention: a single visible ASCII
    /// character that doesn't collide with terrain (`░` `█`) or creatures
    /// (uppercase letters).
    pub glyph: char,
    pub bonuses: ItemBonuses,
    /// If `Some`, the actor carrying this item gets this action added to
    /// their available-actions list. Using the action consumes one copy
    /// of this item (action's own logic handles the removal). `None` for
    /// passive-only trinkets like rings and cloaks.
    pub on_use: Option<&'static (dyn crate::actions::action_template::Action + Send + Sync)>,
    /// Conditions the wearer is immune to while carrying this item.
    /// Folded into `ActorInstance::effectively_immune_to_condition` so
    /// trinkets like the Necklace of Adaptation (Poisoned-immune) and
    /// Ring of Free Action (Paralyzed / Restrained / Grappled-immune)
    /// fall out of the same install-gate that already handles template
    /// and dynamic immunities. Empty for items that don't grant
    /// condition immunities (the default for most loot).
    pub condition_immunities: &'static [crate::conditions::Condition],
    /// Damage types the wearer is resistant to while carrying this item.
    /// Folded into `ActorInstance::effective_damage` so trinkets like the
    /// Brooch of Shielding (force-resistant) and Boots of the Winterlands
    /// (cold-resistant) halve incoming damage through the same lane that
    /// already handles template and condition-based resistance. Honors
    /// the 5e "one halving" stacking rule — item resistance won't
    /// re-halve damage that's already been halved by a template or
    /// condition source. Empty for items without typed resistance.
    pub damage_resistances: &'static [crate::engine::types::DamageType],
    /// Damage types the wearer is fully immune to while carrying this
    /// item. Folded into `ActorInstance::effective_damage` so trinkets
    /// like the Periapt of Proof against Poison (poison-immune) zero
    /// incoming damage through the same lane that already handles
    /// template-level immunity. Immunity wins over everything: an item
    /// immunity short-circuits the damage pipeline before resistance /
    /// vulnerability rolls fire. Empty for items without typed immunity.
    pub damage_immunities: &'static [crate::engine::types::DamageType],
}

impl Item {
    /// All-empty / no-op defaults for the rare fields. Use as the tail of
    /// a struct-update literal (`Item { name: "...", ..Item::DEFAULTS }`)
    /// so items that don't grant condition immunities / damage
    /// resistances / immunities don't have to repeat the three empty-slice
    /// fields at every definition. `name`, `glyph`, and `bonuses` should
    /// always be overridden — the defaults here are just type-correct
    /// placeholders so the struct literal is total. Const-evaluable so it
    /// works in `static` initializers.
    pub const DEFAULTS: Item = Item {
        name: "",
        glyph: ' ',
        bonuses: ItemBonuses::ZERO,
        on_use: None,
        condition_immunities: &[],
        damage_resistances: &[],
        damage_immunities: &[],
    };
}

pub static RING_OF_PROTECTION: Item = Item {
    name: "Ring of Protection",
    glyph: '=',
    bonuses: ItemBonuses { ac: 1, save: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

pub static BOOTS_OF_STRIDING: Item = Item {
    name: "Boots of Striding",
    glyph: 'b',
    bonuses: ItemBonuses { speed: 10, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

pub static CLOAK_OF_RESISTANCE: Item = Item {
    name: "Cloak of Resistance",
    glyph: 'c',
    bonuses: ItemBonuses { save: 2, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

pub static AMULET_OF_HEALTH: Item = Item {
    name: "Amulet of Health",
    glyph: 'a',
    bonuses: ItemBonuses { max_hp: 10, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Headband of Insight — minor caster-flavor trinket. +1 save bonus,
/// no AC or speed. Distinct loot tier from Cloak of Resistance (which
/// gives +2) so the loot pool has stratified strength.
pub static HEADBAND_OF_INSIGHT: Item = Item {
    name: "Headband of Insight",
    glyph: 'h',
    bonuses: ItemBonuses { save: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Bracers of Defense — light AC bump. Cheaper loot than Ring of
/// Protection (which gives +1 AC and +1 save), giving the LOOT_POOL
/// a clearer common / uncommon ladder.
pub static BRACERS_OF_DEFENSE: Item = Item {
    name: "Bracers of Defense",
    glyph: 'B',
    bonuses: ItemBonuses { ac: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

pub static POTION_OF_HEALING: Item = Item {
    name: "Potion of Healing",
    glyph: 'p',
    on_use: Some(&crate::actions::item_actions::DRINK_HEALING_POTION),
    ..Item::DEFAULTS
};

pub static POTION_OF_GREATER_HEALING: Item = Item {
    name: "Potion of Greater Healing",
    glyph: 'P',
    on_use: Some(&crate::actions::item_actions::DRINK_GREATER_HEALING_POTION),
    ..Item::DEFAULTS
};

pub static SCROLL_OF_FIREBALL: Item = Item {
    name: "Scroll of Fireball",
    glyph: 's',
    on_use: Some(&crate::actions::item_actions::READ_FIREBALL_SCROLL),
    ..Item::DEFAULTS
};

pub static SCROLL_OF_MAGIC_MISSILE: Item = Item {
    name: "Scroll of Magic Missile",
    glyph: 'm',
    on_use: Some(&crate::actions::item_actions::READ_MAGIC_MISSILE_SCROLL),
    ..Item::DEFAULTS
};

/// Cloak of Protection — premium passive trinket. +1 AC AND +1 to all
/// saves. Strictly better than Cloak of Resistance for tanks who need
/// the AC bump; rarer in the loot pool.
pub static CLOAK_OF_PROTECTION: Item = Item {
    name: "Cloak of Protection",
    glyph: 'C',
    bonuses: ItemBonuses { ac: 1, save: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Shield — passive +2 AC, no save bonus. Classic light-armor pairing
/// with one-handed weapons. Distinct loot tier from heavy armor since
/// we don't model armor proficiency yet.
pub static SHIELD: Item = Item {
    name: "Shield",
    glyph: 'S',
    bonuses: ItemBonuses { ac: 2, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Antitoxin — single-use consumable. Drinking removes the Poisoned
/// condition and grants advantage on the next CON save against poison
/// (modeled as a flat +5 save buff via Bless's mechanic). One-shot:
/// the action removes the item from inventory after use.
pub static ANTITOXIN: Item = Item {
    name: "Antitoxin",
    glyph: 'A',
    on_use: Some(&crate::actions::item_actions::DRINK_ANTITOXIN),
    ..Item::DEFAULTS
};

/// Potion of Speed — bonus action; gain an extra Action this turn plus
/// a +1 attack/save buff (a simplified Haste). Single-use consumable;
/// the buff clears on long rest with the rest of the buff state.
pub static POTION_OF_SPEED: Item = Item {
    name: "Potion of Speed",
    glyph: '!',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_SPEED),
    ..Item::DEFAULTS
};

/// Potion of Heroism — bonus action; grants 10 temp HP and the Heroic
/// condition (Frightened immunity + temp HP regen tagged onto the
/// buff for 10 rounds). Single-use consumable; the buff drops with
/// the condition timer.
pub static POTION_OF_HEROISM: Item = Item {
    name: "Potion of Heroism",
    glyph: 'H',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_HEROISM),
    ..Item::DEFAULTS
};

/// Potion of Invisibility — action; grants the Invisible condition for
/// 10 rounds (attacks vs holder at disadvantage, holder's attacks at
/// advantage). Single-use consumable; the buff drops with the
/// condition timer.
pub static POTION_OF_INVISIBILITY: Item = Item {
    name: "Potion of Invisibility",
    glyph: 'i',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_INVISIBILITY),
    ..Item::DEFAULTS
};

/// Periapt of Wound Closure — +5 max HP passive trinket. Thematic
/// flavor: stabilizes a dying wearer (modeled as extra HP cushion).
pub static PERIAPT_OF_WOUND_CLOSURE: Item = Item {
    name: "Periapt of Wound Closure",
    glyph: '+',
    bonuses: ItemBonuses { max_hp: 5, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Gauntlets of Ogre Power — +1 AC from the reinforced plates on the
/// gauntlets, plus +5 max HP from the magical vigor. A martial
/// trinket that makes the front-liner stickier.
pub static GAUNTLETS_OF_OGRE_POWER: Item = Item {
    name: "Gauntlets of Ogre Power",
    glyph: 'G',
    bonuses: ItemBonuses { ac: 1, max_hp: 5, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Scroll of Lightning Bolt — one-shot 8d6 lightning burst along a
/// line. Uses the same mechanics as Fireball scroll but typed lightning.
pub static SCROLL_OF_LIGHTNING_BOLT: Item = Item {
    name: "Scroll of Lightning Bolt",
    glyph: 'l',
    on_use: Some(&crate::actions::item_actions::READ_LIGHTNING_BOLT_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Cure Wounds — single-target touch heal (2d8+2 HP). Fills
/// the "single-target heal scroll" niche between the self-only Potion
/// of Healing (2d4+2) and the spell Cure Wounds (caster-mod scaling).
/// Heals a touch-range target on use; consumed on use.
pub static SCROLL_OF_CURE_WOUNDS: Item = Item {
    name: "Scroll of Cure Wounds",
    glyph: 'w',
    on_use: Some(&crate::actions::item_actions::READ_CURE_WOUNDS_SCROLL),
    ..Item::DEFAULTS
};

/// Stone of Good Luck (Luckstone) — premium passive trinket. +1 to all
/// saving throws AND +1 to AC. 5e RAW grants a +1 luck bonus to ability
/// checks and saving throws while carried; we collapse the ability-check
/// half onto the save lane (the engine routes the most consequential
/// rolls — concentration, save-or-suck — through the save path), and
/// throw in a +1 AC as flavor for the luck shielding the holder from
/// blows. Distinct loot tier from the Cloak of Protection (also +1 / +1)
/// — same numeric profile, but priced as a separate roll so the loot
/// pool doesn't collapse to one premium passive.
pub static STONE_OF_GOOD_LUCK: Item = Item {
    name: "Stone of Good Luck",
    glyph: 'L',
    bonuses: ItemBonuses { ac: 1, save: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Necklace of Adaptation — passive trinket. Grants immunity to the
/// Poisoned condition while worn. 5e RAW: "you are immune to harmful
/// gases" plus you can breathe freely; we collapse the breath / gas
/// clause onto Poisoned-immunity (the only in-engine consequence of
/// "harmful gas"). Uses the new `condition_immunities` lane on `Item`
/// so the install gate in `add_condition` skips Poisoned silently
/// while the necklace is carried.
pub static NECKLACE_OF_ADAPTATION: Item = Item {
    name: "Necklace of Adaptation",
    glyph: 'n',
    condition_immunities: &[crate::conditions::Condition::Poisoned],
    ..Item::DEFAULTS
};

/// Ring of Free Action — passive trinket. The wearer ignores
/// movement-impeding effects: Paralyzed, Restrained, and Grappled
/// can't install while it's carried. 5e RAW: "magic can't reduce your
/// speed or cause you to be paralyzed or restrained"; we model the
/// paralysis / restraint half via condition immunity and surface the
/// Grappled clause too (grappled is mechanically a movement-zero
/// effect, matching the spirit of "free action"). Stacks with item
/// immunities — the install gate honors the union of all carried
/// trinkets.
pub static RING_OF_FREE_ACTION: Item = Item {
    name: "Ring of Free Action",
    glyph: 'r',
    condition_immunities: &[
        crate::conditions::Condition::Paralyzed,
        crate::conditions::Condition::Restrained,
        crate::conditions::Condition::Grappled,
    ],
    ..Item::DEFAULTS
};

/// Pearl of Power — caster-flavored consumable. Bonus action: restore
/// one expended level-1 spell slot to the holder, then the pearl is
/// consumed. 5e RAW: "once per long rest, restore one expended spell
/// slot of level 3 or lower" — we collapse the tier to level-1 for
/// simplicity (most caster impls in the engine spend level-1 slots for
/// their cantrip-adjacent options), and drop the long-rest gate in
/// favor of one-shot consumption since the engine doesn't model
/// multi-encounter rest cycles. Pairs with the new `USE_PEARL_OF_POWER`
/// action.
pub static PEARL_OF_POWER: Item = Item {
    name: "Pearl of Power",
    glyph: 'q',
    on_use: Some(&crate::actions::item_actions::USE_PEARL_OF_POWER),
    ..Item::DEFAULTS
};

/// Brooch of Shielding — passive trinket. Grants resistance to force
/// damage while worn. 5e RAW: "you have resistance to force damage,
/// and you are immune to magic missile" — we collapse to the
/// resistance half (the only flavor that fires through the engine's
/// damage pipeline; Magic Missile's force damage gets halved cleanly).
/// Uses the new `damage_resistances` lane on `Item` so the resistance
/// folds through `effective_damage` alongside template and condition
/// sources without code changes at the damage site.
pub static BROOCH_OF_SHIELDING: Item = Item {
    name: "Brooch of Shielding",
    glyph: 'k',
    damage_resistances: &[crate::engine::types::DamageType::Force],
    ..Item::DEFAULTS
};

/// Boots of the Winterlands — passive trinket. Grants resistance to
/// cold damage while worn. 5e RAW: "you have resistance to cold damage
/// and ignore difficult terrain created by ice or snow" — we collapse
/// to the resistance clause (the engine doesn't yet flavor icy terrain
/// as distinct difficult terrain, so the ignore-terrain half would
/// no-op anyway).
pub static BOOTS_OF_THE_WINTERLANDS: Item = Item {
    name: "Boots of the Winterlands",
    glyph: 'W',
    damage_resistances: &[crate::engine::types::DamageType::Cold],
    ..Item::DEFAULTS
};

/// Boots of Speed — bonus action: gain the `Hasted` condition for 10
/// rounds (+2 AC, advantage on DEX saves, doubled walking speed), then
/// the boots are consumed. 5e RAW: action to double speed for up to
/// 10 minutes; we collapse the duration to combat-scale (10 rounds ≈
/// 1 minute) and route through the existing `Hasted` condition for
/// the AC / DEX-save / speed bundle. Pairs with the new
/// `WEAR_BOOTS_OF_SPEED` action. Single-shot consumable — the boots
/// are "exhausted" after one click and removed from inventory.
pub static BOOTS_OF_SPEED: Item = Item {
    name: "Boots of Speed",
    glyph: 'V',
    on_use: Some(&crate::actions::item_actions::WEAR_BOOTS_OF_SPEED),
    ..Item::DEFAULTS
};

/// Periapt of Proof against Poison — passive trinket. Grants the wearer
/// immunity to poison damage AND immunity to the Poisoned condition.
/// 5e RAW: "you are immune to poison damage and the poisoned condition."
/// Strictly stronger than Necklace of Adaptation (which only covers the
/// condition half) — sits as a single rare entry in the loot pool. Uses
/// both the `damage_immunities` lane (zeroes poison damage in
/// `effective_damage`) and the `condition_immunities` lane (blocks the
/// Poisoned install in `add_condition`) so the two halves flow through
/// the same chokepoints that already handle every other source.
pub static PERIAPT_OF_PROOF_AGAINST_POISON: Item = Item {
    name: "Periapt of Proof against Poison",
    glyph: 'y',
    condition_immunities: &[crate::conditions::Condition::Poisoned],
    damage_immunities: &[crate::engine::types::DamageType::Poison],
    ..Item::DEFAULTS
};

/// Ring of Mind Shielding — passive trinket. Grants the wearer immunity
/// to the Charmed condition (mind-control protection) and to psychic
/// damage (mental shielding extends to direct thought-attacks). 5e RAW:
/// "you are immune to magic that allows other creatures to read your
/// thoughts, determine whether you are lying, know your alignment, or
/// know your creature type" — we collapse the divination clauses onto
/// the load-bearing combat clauses (Charmed-immunity for the mental-
/// control half, Psychic-damage immunity for the thought-attack half),
/// since the engine has no divination subsystem. Pairs cleanly with
/// Periapt of Proof against Poison in the rare-trinket tier — same
/// dual-lane shape, different damage type / condition pair.
pub static RING_OF_MIND_SHIELDING: Item = Item {
    name: "Ring of Mind Shielding",
    glyph: 'M',
    condition_immunities: &[crate::conditions::Condition::Charmed],
    damage_immunities: &[crate::engine::types::DamageType::Psychic],
    ..Item::DEFAULTS
};

/// Robe of the Archmagi — premium passive caster trinket. +2 AC and +2
/// to all saves, a strict upgrade on the Cloak of Protection (+1/+1).
/// 5e RAW: also grants advantage on saves vs spells and a spell save DC
/// bump — we collapse those clauses onto the load-bearing flat +2 save
/// bonus since the engine routes most save modifiers through the same
/// `ItemBonuses.save` lane. The +2 AC half is the unarmored-defense
/// equivalent for casters who don't wear heavy armor. Top-of-pool loot
/// — strictly stronger than every other passive trinket, so it sits as
/// a single low-weight entry.
pub static ROBE_OF_THE_ARCHMAGI: Item = Item {
    name: "Robe of the Archmagi",
    glyph: 'R',
    bonuses: ItemBonuses { ac: 2, save: 2, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Ring of Fire Resistance — passive trinket. Grants resistance to fire
/// damage while worn. 5e RAW: "you have resistance to fire damage." Sits
/// in the same "single-typed-resistance ring" tier as Brooch of Shielding
/// (force) and Boots of the Winterlands (cold) — distinct entries per
/// elemental type let the loot table cover a spread of common AoE
/// damage profiles without piling every resistance onto a single
/// overloaded slot.
pub static RING_OF_FIRE_RESISTANCE: Item = Item {
    name: "Ring of Fire Resistance",
    glyph: 'f',
    damage_resistances: &[crate::engine::types::DamageType::Fire],
    ..Item::DEFAULTS
};

/// Ring of Cold Resistance — passive trinket. Grants resistance to cold
/// damage while worn. Mirror of `RING_OF_FIRE_RESISTANCE` for the cold
/// damage type. Sibling to `BOOTS_OF_THE_WINTERLANDS` (also cold
/// resistance), but the ring is the loot slot that doesn't compete with
/// the boots' speed bonus / movement-bonus tier.
pub static RING_OF_COLD_RESISTANCE: Item = Item {
    name: "Ring of Cold Resistance",
    glyph: 'o',
    damage_resistances: &[crate::engine::types::DamageType::Cold],
    ..Item::DEFAULTS
};

/// Ring of Acid Resistance — passive trinket. Grants resistance to acid
/// damage while worn. Slots into the elemental-resistance ring family
/// next to fire / cold / lightning so the loot pool spreads coverage
/// over the four classic burst-damage elements.
pub static RING_OF_ACID_RESISTANCE: Item = Item {
    name: "Ring of Acid Resistance",
    glyph: 'd',
    damage_resistances: &[crate::engine::types::DamageType::Acid],
    ..Item::DEFAULTS
};

/// Ring of Lightning Resistance — passive trinket. Grants resistance to
/// lightning damage while worn. Final entry in the four-element ring
/// family (fire / cold / acid / lightning).
pub static RING_OF_LIGHTNING_RESISTANCE: Item = Item {
    name: "Ring of Lightning Resistance",
    glyph: 'g',
    damage_resistances: &[crate::engine::types::DamageType::Lightning],
    ..Item::DEFAULTS
};

/// Scroll of Cone of Cold — single-use 8d8 cold-damage burst. 60-foot
/// cone in RAW; we model as a 6-tile-radius burst centered on the target
/// tile (matching the `CONE_OF_COLD` spell's burst approximation). All
/// actors in the area make a CON save vs DC 15; pass halves, fail takes
/// full. The scroll consumes on use; no spell slot.
pub static SCROLL_OF_CONE_OF_COLD: Item = Item {
    name: "Scroll of Cone of Cold",
    glyph: 'O',
    on_use: Some(&crate::actions::item_actions::READ_CONE_OF_COLD_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Magic Missiles — single-use 5-dart variant of the Magic
/// Missile spell. Each dart deals 1d4+1 force damage at one enemy in
/// line-of-sight (range 30 tiles), auto-hit / no save. Mirrors
/// `SCROLL_OF_MAGIC_MISSILE` (which fires 3 darts) — the wand is the
/// upgraded loot slot. 5e RAW: the wand has 7 charges and casts at level
/// 1-3; we collapse to a single-shot consumable for the engine's
/// charge-less loot model, sized at the level-2 cast (5 darts) so it
/// lands between the scroll's 3 darts and a level-3 wizard's 5 darts.
pub static WAND_OF_MAGIC_MISSILES: Item = Item {
    name: "Wand of Magic Missiles",
    glyph: 'D',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_MAGIC_MISSILES),
    ..Item::DEFAULTS
};

/// Pool of items that can be dropped as random loot. Order is irrelevant;
/// the encounter picks uniformly. Add new specials here to put them in
/// rotation without touching call sites. Some entries appear multiple
/// times to weight the drop table toward more common items.
pub static LOOT_POOL: &[&Item] = &[
    &RING_OF_PROTECTION,
    &BOOTS_OF_STRIDING,
    &CLOAK_OF_RESISTANCE,
    &CLOAK_OF_PROTECTION,
    &AMULET_OF_HEALTH,
    &HEADBAND_OF_INSIGHT,
    &BRACERS_OF_DEFENSE,
    // Healing potions sit at higher weight — they're consumables and
    // expected to outpace passive trinkets across a dungeon.
    &POTION_OF_HEALING,
    &POTION_OF_HEALING,
    &POTION_OF_HEALING,
    &POTION_OF_GREATER_HEALING,
    &ANTITOXIN,
    &POTION_OF_SPEED,
    &POTION_OF_HEROISM,
    &POTION_OF_INVISIBILITY,
    &SCROLL_OF_FIREBALL,
    &SCROLL_OF_MAGIC_MISSILE,
    &SCROLL_OF_LIGHTNING_BOLT,
    // Single-target healing scroll — slots into the loot pool between
    // the self-only Potion of Healing and the bonus-action Greater
    // Healing variant. Touch-range, so it benefits front-line allies
    // (the rogue / fighter / paladin) without needing a caster.
    &SCROLL_OF_CURE_WOUNDS,
    &PERIAPT_OF_WOUND_CLOSURE,
    &GAUNTLETS_OF_OGRE_POWER,
    // Premium passive trinkets — same low odds as Cloak of Protection
    // / Ring of Protection; the immunity rings are stronger than the
    // generic save / AC bumps so they sit on the rare half of the
    // pool by virtue of single-entry weighting.
    &STONE_OF_GOOD_LUCK,
    &NECKLACE_OF_ADAPTATION,
    &RING_OF_FREE_ACTION,
    // Caster-side consumables — Pearl restores a slot, Boots grant
    // Hasted. Slot in at the same weight as the offensive scrolls so
    // arcane casters have a roughly even shot at a defensive utility.
    &PEARL_OF_POWER,
    &BOOTS_OF_SPEED,
    // Typed-resistance trinkets — same weight as the AC / save
    // trinkets, but the resistance lane fires through the damage
    // pipeline rather than the AC / save lanes.
    &BROOCH_OF_SHIELDING,
    &BOOTS_OF_THE_WINTERLANDS,
    // Typed-immunity trinkets — single low-weight entries since
    // immunity is strictly stronger than resistance and these double
    // up as condition-immunity sources too. Both ride the new
    // `damage_immunities` lane.
    &PERIAPT_OF_PROOF_AGAINST_POISON,
    &RING_OF_MIND_SHIELDING,
    // Premium caster trinket — top of the passive tier, single entry
    // by design. Strictly dominates Cloak of Protection (+1/+1) and
    // Ring of Protection (+1/+1).
    &ROBE_OF_THE_ARCHMAGI,
    // Elemental-resistance ring family — single low-weight entry per
    // element so the loot table covers the four classic burst-damage
    // types (fire / cold / acid / lightning) without over-skewing the
    // pool toward typed-resistance loot.
    &RING_OF_FIRE_RESISTANCE,
    &RING_OF_COLD_RESISTANCE,
    &RING_OF_ACID_RESISTANCE,
    &RING_OF_LIGHTNING_RESISTANCE,
    // Single-use AoE scrolls / wands — same weight as the Fireball /
    // Lightning Bolt scrolls so casters have a roughly even shot at a
    // big burst regardless of element. Cone of Cold's 8d8 cold tier sits
    // above the 6d6 Fireball / 8d6 Lightning Bolt tier; Wand of Magic
    // Missiles' 5-dart payload sits above the 3-dart scroll tier.
    &SCROLL_OF_CONE_OF_COLD,
    &WAND_OF_MAGIC_MISSILES,
];
