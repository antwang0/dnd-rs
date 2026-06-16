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
    /// Flat bonus added to every attack roll the holder makes (weapon
    /// and spell attacks alike — folded into `caster_attack_buffs` so
    /// the install-side `attack_bonus_buff` lane is shared with item
    /// passives). Matches the `+1 weapon` / Bracers of Archery loot
    /// archetype. 5e RAW: a +1 weapon adds to both attack AND damage
    /// rolls — we ride the attack half here; the damage half lives on
    /// `damage_bonus` below so weapon swings AND spell attacks both
    /// see the bonus once (no double-dipping). 0 by default — most
    /// trinkets leave this alone.
    pub attack_bonus: i32,
    /// Flat bonus added to every damage roll the holder lands on a hit
    /// (weapon and spell attacks alike). Used by `+1 weapon`-style items
    /// to grant the RAW "+N to attack AND damage" pair. Read at the
    /// damage-roll site in `engine::attack` / `spells.rs`'s spell-attack
    /// chokepoint. 0 by default.
    pub damage_bonus: i32,
}

impl ItemBonuses {
    /// All-zero baseline. Use as the tail of a struct-update literal so
    /// an item that only bumps one stat doesn't enumerate the other zero
    /// fields — `ItemBonuses { ac: 1, ..ItemBonuses::ZERO }` reads cleaner
    /// than spelling out every neutral field. Const-evaluable so static
    /// items can build off it.
    pub const ZERO: ItemBonuses = ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 0,
        attack_bonus: 0,
        damage_bonus: 0,
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
            attack_bonus: self.attack_bonus + other.attack_bonus,
            damage_bonus: self.damage_bonus + other.damage_bonus,
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
    /// Conditions the wearer has installed (Permanent) for as long as the
    /// item is carried. Used by passive-buff trinkets like Slippers of
    /// Spider Climbing (SpiderClimbing) and Winged Boots (Flying) so the
    /// in-fiction effect flows through the same condition map that already
    /// drives `speed()`, `compute_attack_mode`, etc. Hooked at three
    /// places: `pickup_item` installs each entry on grab, `remove_item_by_name`
    /// strips entries that no remaining carried item still grants, and
    /// `long_rest` re-installs them after the conditions clear. A
    /// `Dispel Magic` or other in-combat strip will leave the actor
    /// without the buff until the next long rest (or until the player
    /// re-picks-up the item) — acceptable tradeoff for the simpler
    /// "install on the inventory event" model. Empty for items without a
    /// persistent condition (the default for most loot).
    pub passive_conditions: &'static [crate::conditions::Condition],
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
        passive_conditions: &[],
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
/// slot of level 3 or lower" — we ladder the loot pool through three
/// pearl tiers instead (level-1 / level-2 / level-3 refunds), trading
/// the single-RAW-pearl for three distinct rolls. Drops the long-rest
/// gate in favor of one-shot consumption since the engine doesn't
/// model multi-encounter rest cycles.
pub static PEARL_OF_POWER: Item = Item {
    name: "Pearl of Power",
    glyph: 'q',
    on_use: Some(&crate::actions::item_actions::USE_PEARL_OF_POWER),
    ..Item::DEFAULTS
};

/// Greater Pearl of Power — refunds one expended level-2 spell slot.
/// Sibling to `PEARL_OF_POWER` (level-1 refund); same one-shot envelope
/// but a higher tier. Fires through the shared `PearlOfPowerItem` impl.
pub static GREATER_PEARL_OF_POWER: Item = Item {
    name: "Greater Pearl of Power",
    glyph: 'E',
    on_use: Some(&crate::actions::item_actions::USE_GREATER_PEARL_OF_POWER),
    ..Item::DEFAULTS
};

/// Supreme Pearl of Power — refunds one expended level-3 spell slot.
/// Top of the pearl ladder; matches the RAW pearl's "level 3 or lower"
/// envelope. Same one-shot envelope as the lesser tiers.
pub static SUPREME_PEARL_OF_POWER: Item = Item {
    name: "Supreme Pearl of Power",
    glyph: 'I',
    on_use: Some(&crate::actions::item_actions::USE_SUPREME_PEARL_OF_POWER),
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

/// Ring of Poison Resistance — passive trinket. Grants resistance to
/// poison damage while worn. Distinct from the Periapt of Proof
/// against Poison (immunity + Poisoned-condition-immune) — this is the
/// resistance-tier counterpart at a different weight in the loot pool.
/// Slots into the typed-resistance ring family alongside fire / cold /
/// acid / lightning.
pub static RING_OF_POISON_RESISTANCE: Item = Item {
    name: "Ring of Poison Resistance",
    glyph: 'j',
    damage_resistances: &[crate::engine::types::DamageType::Poison],
    ..Item::DEFAULTS
};

/// Ring of Radiant Resistance — passive trinket. Grants resistance to
/// radiant damage while worn. Useful against celestial / cleric burst
/// (Spirit Guardians, Sacred Burst, Sunburst). Slots into the typed-
/// resistance ring family alongside the elemental rings.
pub static RING_OF_RADIANT_RESISTANCE: Item = Item {
    name: "Ring of Radiant Resistance",
    glyph: 'u',
    damage_resistances: &[crate::engine::types::DamageType::Radiant],
    ..Item::DEFAULTS
};

/// Ring of Necrotic Resistance — passive trinket. Grants resistance to
/// necrotic damage while worn. Counterpart to the Radiant ring — useful
/// against undead drain attacks and wizard necromancy bursts (Blight,
/// Circle of Death). Slots into the typed-resistance ring family.
pub static RING_OF_NECROTIC_RESISTANCE: Item = Item {
    name: "Ring of Necrotic Resistance",
    glyph: 'e',
    damage_resistances: &[crate::engine::types::DamageType::Necrotic],
    ..Item::DEFAULTS
};

/// Ring of Thunder Resistance — passive trinket. Grants resistance to
/// thunder damage while worn. Useful against Shatter / Thunderwave /
/// Thunder Step bursts. Slots into the typed-resistance ring family
/// alongside the elemental rings.
pub static RING_OF_THUNDER_RESISTANCE: Item = Item {
    name: "Ring of Thunder Resistance",
    glyph: 'v',
    damage_resistances: &[crate::engine::types::DamageType::Thunder],
    ..Item::DEFAULTS
};

/// Ring of Psychic Resistance — passive trinket. Grants resistance to
/// psychic damage while worn. Distinct from the Ring of Mind Shielding
/// (which is full Psychic immunity + Charmed-immunity) — this is the
/// resistance-tier counterpart at a different loot weight. Useful
/// against Mind Sliver / Psychic Scream / Phantasmal-style mental
/// bursts.
pub static RING_OF_PSYCHIC_RESISTANCE: Item = Item {
    name: "Ring of Psychic Resistance",
    glyph: 'x',
    damage_resistances: &[crate::engine::types::DamageType::Psychic],
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

/// Wand of Fireballs — single-use 8d6 fire burst (Action, 60-ft range,
/// 4-tile radius, DEX save vs DC 15 for half). Sits a tier above the
/// `SCROLL_OF_FIREBALL` (6d6) — same shape, bigger payload. The wand
/// is consumed after one click; no charges tracked.
pub static WAND_OF_FIREBALLS: Item = Item {
    name: "Wand of Fireballs",
    glyph: 'F',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_FIREBALLS),
    ..Item::DEFAULTS
};

/// Wand of Lightning Bolts — single-use 10d6 lightning burst (Action,
/// 100-ft range, 2-tile radius, DEX save vs DC 15 for half). Sits a tier
/// above `SCROLL_OF_LIGHTNING_BOLT` (8d6) — same shape, bigger payload.
/// Sibling to `WAND_OF_FIREBALLS` for the lightning lane.
pub static WAND_OF_LIGHTNING_BOLTS: Item = Item {
    name: "Wand of Lightning Bolts",
    glyph: 'Z',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_LIGHTNING_BOLTS),
    ..Item::DEFAULTS
};

/// Potion of Flying — Action; grants the holder the `Flying` condition
/// for 10 rounds (≈1 minute RAW combat-scaled). Re-uses the existing
/// Flying condition so the +24-tile speed bump and ranged-attack
/// deflection flow through the same accessors a normal Fly cast does.
/// Single-use consumable.
pub static POTION_OF_FLYING: Item = Item {
    name: "Potion of Flying",
    glyph: '^',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_FLYING),
    ..Item::DEFAULTS
};

/// Potion of Climbing — Bonus Action; grants the holder the
/// `SpiderClimbing` condition for 10 rounds. Cheaper / lesser mobility
/// envelope than Potion of Flying (Action cost, full flight). Re-uses
/// the Spider Climb condition so the +12-tile speed bump flows through
/// the same accessor a normal Spider Climb cast does. Single-use
/// consumable.
pub static POTION_OF_CLIMBING: Item = Item {
    name: "Potion of Climbing",
    glyph: '*',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_CLIMBING),
    ..Item::DEFAULTS
};

/// Potion of Superior Healing — Action; 8d4+8 self-heal. Top of the
/// healing-potion tier: Healing (2d4+2) → Greater (4d4+4) → Superior
/// (8d4+8). 5e RAW also has a Supreme (10d4+20) tier — we stop at
/// Superior for the loot pool. Single-use consumable.
pub static POTION_OF_SUPERIOR_HEALING: Item = Item {
    name: "Potion of Superior Healing",
    glyph: 'X',
    on_use: Some(&crate::actions::item_actions::DRINK_SUPERIOR_HEALING_POTION),
    ..Item::DEFAULTS
};

/// Potion of Stoneskin — Action; installs `DamageResistant` for 10
/// rounds (halve all incoming damage). Mirrors the Stoneskin spell's
/// envelope (the spell installs the same condition). Single-use
/// consumable.
pub static POTION_OF_STONESKIN: Item = Item {
    name: "Potion of Stoneskin",
    glyph: 'T',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_STONESKIN),
    ..Item::DEFAULTS
};

/// Wand of Cone of Cold — single-use 10d8 cold CON-save burst (Action,
/// 6-tile radius). Sits a tier above `SCROLL_OF_CONE_OF_COLD` (8d8) —
/// same shape, bigger pool. Top-of-pool burst-wand entry alongside the
/// fire / lightning wand siblings.
pub static WAND_OF_CONE_OF_COLD: Item = Item {
    name: "Wand of Cone of Cold",
    glyph: 'Q',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_CONE_OF_COLD),
    ..Item::DEFAULTS
};

/// Scroll of Shatter — single-use 3d8 thunder CON-save burst (Action,
/// 2-tile radius). Fills the thunder lane in the burst-damage scroll
/// family — alongside Fireball (fire), Lightning Bolt (lightning), and
/// Cone of Cold (cold).
pub static SCROLL_OF_SHATTER: Item = Item {
    name: "Scroll of Shatter",
    glyph: 't',
    on_use: Some(&crate::actions::item_actions::READ_SHATTER_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Mass Healing Word — single-use ally-aura heal. Bonus action;
/// heal up to 6 nearest allies (combat-active or dying) within 60 ft
/// (24 tiles) of the reader for 1d4+3 HP each. Fires through the
/// `READ_MASS_HEALING_WORD_SCROLL` action, which mirrors the
/// `MASS_HEALING_WORD` spell's envelope at a fixed +3 caster-mod
/// stand-in. Sits in the loot pool as the multi-target counterpart to
/// the single-target Scroll of Cure Wounds.
pub static SCROLL_OF_MASS_HEALING_WORD: Item = Item {
    name: "Scroll of Mass Healing Word",
    glyph: 'z',
    on_use: Some(&crate::actions::item_actions::READ_MASS_HEALING_WORD_SCROLL),
    ..Item::DEFAULTS
};

/// +1 Weapon — passive trinket. Grants +1 to attack rolls AND +1 to
/// damage rolls while carried. 5e RAW: a magical weapon adds the bonus
/// to both attack and damage with that weapon; we abstract over the
/// weapon-vs-weapon binding (the engine doesn't model weapon ownership
/// beyond "this swing came from this actor") and have the trinket
/// modify every attack the holder makes — weapon swings AND spell
/// attacks alike. Sits in the loot pool as the common "magical weapon"
/// archetype; the +2 and +3 tiers stack the same fields linearly.
pub static WEAPON_PLUS_ONE: Item = Item {
    name: "+1 Weapon",
    glyph: '/',
    bonuses: ItemBonuses {
        attack_bonus: 1,
        damage_bonus: 1,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// +2 Weapon — passive trinket. Premium tier of the magical-weapon
/// loot ladder: +2 attack AND +2 damage on every swing. Single-entry
/// in the loot pool (one notch above the common +1 tier).
pub static WEAPON_PLUS_TWO: Item = Item {
    name: "+2 Weapon",
    glyph: '\\',
    bonuses: ItemBonuses {
        attack_bonus: 2,
        damage_bonus: 2,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// Bracers of Archery — passive trinket. RAW: +2 to damage rolls with
/// longbows / shortbows. The engine doesn't yet split swings by weapon
/// type, so we collapse the gate to "all damage rolls" (a small over-
/// tune: a melee fighter wearing the bracers picks up the bonus too,
/// but the loot tier still slots between Weapon +1 (+1/+1) and Weapon
/// +2 (+2/+2) at a useful niche). The attack-bonus stays 0 — the
/// bracers explicitly don't grant a to-hit bump RAW.
pub static BRACERS_OF_ARCHERY: Item = Item {
    name: "Bracers of Archery",
    glyph: 'Y',
    bonuses: ItemBonuses {
        damage_bonus: 2,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// Ioun Stone of Mastery — passive trinket. RAW (DMG): "your proficiency
/// bonus increases by 1 while you have this stone." We collapse the
/// proficiency-bump clause onto the load-bearing attack lane: +1 attack
/// AND +1 save (the two rolls that proficiency-bonus most consequentially
/// drives). Distinct loot tier from `+1 Weapon` since this also stacks
/// the save bonus rather than the damage bonus — caster-flavored.
pub static IOUN_STONE_OF_MASTERY: Item = Item {
    name: "Ioun Stone of Mastery",
    glyph: 'J',
    bonuses: ItemBonuses {
        attack_bonus: 1,
        save: 1,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// Sentinel Shield — passive trinket. RAW (XGtE): "you have advantage on
/// initiative rolls and Wisdom (Perception) checks." We don't model
/// initiative or perception checks at this granularity, so we collapse
/// the trait onto a defensive AC bump (+1, matching the shield slot) and
/// a +1 save bonus (the "alert" half of the trinket). Same numeric profile
/// as Ring of Protection but a different in-fiction flavor — gives the
/// loot pool another low-tier defensive trinket without sliding in a
/// straight Ring of Protection duplicate.
pub static SENTINEL_SHIELD: Item = Item {
    name: "Sentinel Shield",
    glyph: '#',
    bonuses: ItemBonuses { ac: 1, save: 1, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// +3 Weapon — passive trinket. Top tier of the magical-weapon loot
/// ladder: +3 attack AND +3 damage on every swing. Single-entry rare
/// drop sitting one notch above `WEAPON_PLUS_TWO`. The numbers stack
/// linearly with the `attack_bonus_buff` / `damage_bonus_buff` lanes
/// the lower tiers already ride, so no new code paths fire.
pub static WEAPON_PLUS_THREE: Item = Item {
    name: "+3 Weapon",
    glyph: '|',
    bonuses: ItemBonuses {
        attack_bonus: 3,
        damage_bonus: 3,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// Belt of Giant Strength — passive trinket. 5e RAW: sets the wearer's
/// STR score to a fixed value (19 for Hill Giant, 25 for Storm Giant);
/// the engine doesn't model overwriting ability scores, so we collapse
/// the STR-set clause onto the load-bearing combat effects of a STR
/// bump: +2 damage on every swing (STR mod's typical +3 → +5 shift) and
/// +10 max HP (the CON-adjacent vitality the belt represents). Distinct
/// from `GAUNTLETS_OF_OGRE_POWER` (+1 AC / +5 HP) — the belt's damage
/// rider is the offensive niche; the gauntlets sit on the defensive lane.
pub static BELT_OF_GIANT_STRENGTH: Item = Item {
    name: "Belt of Giant Strength",
    glyph: '~',
    bonuses: ItemBonuses {
        damage_bonus: 2,
        max_hp: 10,
        ..ItemBonuses::ZERO
    },
    ..Item::DEFAULTS
};

/// Potion of Supreme Healing — Action; 10d4+20 self-heal. Top of the
/// healing-potion tier: Healing (2d4+2) → Greater (4d4+4) → Superior
/// (8d4+8) → Supreme (10d4+20). Matches 5e RAW. Single-use consumable.
pub static POTION_OF_SUPREME_HEALING: Item = Item {
    name: "Potion of Supreme Healing",
    glyph: '%',
    on_use: Some(&crate::actions::item_actions::DRINK_SUPREME_HEALING_POTION),
    ..Item::DEFAULTS
};

/// Potion of Mage Armor — Action; installs `MageArmored` for 10 rounds
/// (AC floor of 13 + DEX). Single-use consumable. The Mage Armor spell
/// is a level-1 abjuration; the potion bypasses the spell-slot cost so
/// non-casters can dip into the buff. Rejects re-drink while the buff is
/// up so the consumable isn't burned on a no-op timer refresh.
pub static POTION_OF_MAGE_ARMOR: Item = Item {
    name: "Potion of Mage Armor",
    glyph: 'N',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_MAGE_ARMOR),
    ..Item::DEFAULTS
};

/// Potion of Blur — Action; installs `Blurred` for 10 rounds (attacks
/// against the holder have disadvantage). Single-use consumable. The
/// Blur spell is a level-2 concentration; the potion bypasses
/// concentration so the holder can stack it on top of an existing
/// concentration buff. Sits in the loot pool as a defensive consumable
/// alongside Potion of Invisibility / Potion of Stoneskin.
pub static POTION_OF_BLUR: Item = Item {
    name: "Potion of Blur",
    glyph: '?',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_BLUR),
    ..Item::DEFAULTS
};

/// Greater Wand of Magic Missiles — 7-dart variant of the Magic Missile
/// spell. Top of the MM loot ladder: Scroll (3 darts) → Wand (5 darts)
/// → Greater Wand (7 darts). Matches the RAW level-4 upcast. Single-use
/// consumable. Fires through the shared `MagicMissileItem` impl.
pub static GREATER_WAND_OF_MAGIC_MISSILES: Item = Item {
    name: "Greater Wand of Magic Missiles",
    glyph: '>',
    on_use: Some(&crate::actions::item_actions::USE_GREATER_WAND_OF_MAGIC_MISSILES),
    ..Item::DEFAULTS
};

/// Scroll of Burning Hands — single-use 3d6 fire DEX-save burst (Action,
/// 2-tile radius). Fills the entry-level fire-burst niche in the scroll
/// family — distinct from the rare Scroll of Fireball (6d6). Mirrors
/// the BURNING_HANDS spell at level 1.
pub static SCROLL_OF_BURNING_HANDS: Item = Item {
    name: "Scroll of Burning Hands",
    glyph: '&',
    on_use: Some(&crate::actions::item_actions::READ_BURNING_HANDS_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Thunderwave — single-use 2d8 thunder CON-save burst (Action,
/// 2-tile radius). Fills the thunder lane at the cheap loot tier
/// alongside the rare Scroll of Shatter (3d8). Mirrors the THUNDERWAVE
/// spell at level 1, minus the push rider (the shared BurstSaveDamageItem
/// helper doesn't fork into push follow-ups).
pub static SCROLL_OF_THUNDERWAVE: Item = Item {
    name: "Scroll of Thunderwave",
    glyph: '@',
    on_use: Some(&crate::actions::item_actions::READ_THUNDERWAVE_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Web — single-use 4-tile burst, DEX save vs DC 15, fail =
/// Restrained for 10 rounds. 5e RAW: 7 charges casting the Web spell;
/// we collapse to a one-shot fire-and-forget cast — no concentration,
/// no charges tracked, no allies caught in the strands (the wand uses
/// the player-friendly enemy-burst lane like every other harmful
/// consumable). Fills the Restrained-installer niche in the loot pool
/// next to the burst-damage scrolls / wands.
pub static WAND_OF_WEB: Item = Item {
    name: "Wand of Web",
    glyph: '$',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_WEB),
    ..Item::DEFAULTS
};

/// Pipes of Haunting — single-use 4-tile burst, WIS save vs DC 13,
/// fail = Frightened for 10 rounds. 5e RAW: 30-ft cone fear-burst with
/// 3 charges; we collapse to a one-shot envelope. Lower DC (13 vs the
/// usual 15) reflects the "low-tier mood music" flavor — pairs with the
/// rarer Wand of Fear (also Frightened, DC 15, single-target) so the
/// loot pool covers the Frightened lane at two tiers.
pub static PIPES_OF_HAUNTING: Item = Item {
    name: "Pipes of Haunting",
    glyph: '(',
    on_use: Some(&crate::actions::item_actions::PLAY_PIPES_OF_HAUNTING),
    ..Item::DEFAULTS
};

/// Wand of Paralysis — single-use single-target, CON save vs DC 15,
/// fail = Paralyzed for 10 rounds. 5e RAW: 7 charges firing a line of
/// paralysis at one creature; we collapse to a one-shot beam — no
/// charges tracked. Paralyzed is one of the engine's hardest CC
/// envelopes (zero movement, action economy blocked, auto-fail
/// STR/DEX saves, melee crits land automatically), so the consumable
/// sits in the rare half of the loot pool.
pub static WAND_OF_PARALYSIS: Item = Item {
    name: "Wand of Paralysis",
    glyph: ')',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_PARALYSIS),
    ..Item::DEFAULTS
};

/// Wand of Fear — single-use single-target, WIS save vs DC 15, fail =
/// Frightened for 10 rounds. 5e RAW: 7 charges casting Fear (a 30-ft
/// cone) at level 3; we collapse to a single-target single-use cast.
/// Distinct from Pipes of Haunting (same condition, wider burst, lower
/// DC) — the wand is the "hard single-target fear" niche, the pipes
/// cover the "soft area fear" niche.
pub static WAND_OF_FEAR: Item = Item {
    name: "Wand of Fear",
    glyph: '[',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_FEAR),
    ..Item::DEFAULTS
};

/// Scroll of Hold Person — single-use single-target, WIS save vs DC 13,
/// fail = Paralyzed for 10 rounds. 5e RAW: level-2 enchantment with
/// concentration / re-save each turn; the scroll collapses to the
/// fixed 10-round paralysis envelope every other CC consumable rides.
/// Entry-level CC scroll alongside Pipes of Haunting (DC 13 burst
/// Frightened) — the scroll is the single-target hard-CC niche at the
/// cheap tier.
pub static SCROLL_OF_HOLD_PERSON: Item = Item {
    name: "Scroll of Hold Person",
    glyph: ']',
    on_use: Some(&crate::actions::item_actions::READ_HOLD_PERSON_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Hold Monster — single-use single-target, WIS save vs DC 15,
/// fail = Paralyzed for 10 rounds. Sits a tier above Scroll of Hold
/// Person (DC 13, 24-tile reach) — same shape, harder DC, longer reach
/// (90 ft RAW).
pub static SCROLL_OF_HOLD_MONSTER: Item = Item {
    name: "Scroll of Hold Monster",
    glyph: '{',
    on_use: Some(&crate::actions::item_actions::READ_HOLD_MONSTER_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Confusion — single-use 4-tile burst, WIS save vs DC 15,
/// fail = Confused for 10 rounds (disadvantage on attacks AND no
/// reactions). 5e RAW: level-4 enchantment, 90-ft range / 10-ft cube,
/// concentration; we collapse to a single-shot fire-and-forget cast.
/// Top-of-pool burst CC alongside Wand of Paralysis — the confusion
/// wand trades single-target lockdown for a wider soft-CC blanket.
pub static WAND_OF_CONFUSION: Item = Item {
    name: "Wand of Confusion",
    glyph: '}',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_CONFUSION),
    ..Item::DEFAULTS
};

/// Scroll of Hypnotic Pattern — single-use 4-tile burst, WIS save vs
/// DC 14, fail = Incapacitated for 10 rounds. 5e RAW: level-3 illusion,
/// 120-ft range / 30-ft cube, concentration; we collapse to a single-
/// shot fire-and-forget cast. Mid-tier burst CC between Pipes of
/// Haunting (DC 13 Frightened) and Wand of Confusion (DC 15 Confused).
pub static SCROLL_OF_HYPNOTIC_PATTERN: Item = Item {
    name: "Scroll of Hypnotic Pattern",
    glyph: '`',
    on_use: Some(&crate::actions::item_actions::READ_HYPNOTIC_PATTERN_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Vitriolic Sphere — single-use 10d4 acid DEX-save burst,
/// 4-tile radius. Fills the acid lane in the burst-damage scroll family
/// alongside Fireball (fire), Lightning Bolt (lightning), Cone of Cold
/// (cold), and Shatter (thunder).
pub static SCROLL_OF_VITRIOLIC_SPHERE: Item = Item {
    name: "Scroll of Vitriolic Sphere",
    glyph: ':',
    on_use: Some(&crate::actions::item_actions::READ_VITRIOLIC_SPHERE_SCROLL),
    ..Item::DEFAULTS
};

/// Archmage Pearl of Power — bonus action; restore one expended level-4
/// spell slot. Top of the pearl ladder above Supreme Pearl of Power
/// (level-3 refund). 5e RAW pearls cap at level-3 slots; we extend the
/// ladder to cover the level-4 slot tier as the rarest-tier caster
/// consumable.
pub static ARCHMAGE_PEARL_OF_POWER: Item = Item {
    name: "Archmage Pearl of Power",
    glyph: ';',
    on_use: Some(&crate::actions::item_actions::USE_ARCHMAGE_PEARL_OF_POWER),
    ..Item::DEFAULTS
};

/// Potion of Sanctuary — Bonus Action; installs `Sanctuary` for 10
/// rounds. 5e RAW: the spell is level-1 abjuration, bonus action, on a
/// willing target; the potion collapses to a self-only envelope. The
/// buff routes hostile actions against the drinker through a WIS save
/// vs the source's DC (silently no-ops on fail) and drops the moment
/// the drinker themselves attacks or casts a damaging spell.
pub static POTION_OF_SANCTUARY: Item = Item {
    name: "Potion of Sanctuary",
    glyph: ',',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_SANCTUARY),
    ..Item::DEFAULTS
};

/// Wand of Cure Wounds — Action; touch (1-tile) ally heal for 3d8+3.
/// Sits a tier above the Scroll of Cure Wounds (2d8+2) — same shape,
/// bigger pool. 5e RAW: 7 charges casting Cure Wounds at level 1-3;
/// we collapse to a single 3d8 cast for the engine's charge-less loot
/// model.
pub static WAND_OF_CURE_WOUNDS: Item = Item {
    name: "Wand of Cure Wounds",
    glyph: '"',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_CURE_WOUNDS),
    ..Item::DEFAULTS
};

/// Scroll of Healing Word — Bonus Action; ranged ally heal for 1d4+3
/// at 24-tile reach. 5e RAW: level-1 evocation, bonus action, 60-ft
/// range. Pairs with the touch-range Scroll of Cure Wounds — the
/// healing-word scroll trades payload for reach and action economy.
pub static SCROLL_OF_HEALING_WORD: Item = Item {
    name: "Scroll of Healing Word",
    glyph: '\'',
    on_use: Some(&crate::actions::item_actions::READ_HEALING_WORD_SCROLL),
    ..Item::DEFAULTS
};

/// Potion of Growth — Action; installs `Enlarged` for 10 rounds
/// (+1d4 weapon damage rider, size bump). 5e RAW: 1d4-hour duration;
/// we collapse to the combat-scale 10-round timer every other buff
/// consumable rides. Sibling to Belt of Giant Strength on the
/// offensive bruiser lane — the belt is a passive +2 damage / +10 HP,
/// the potion is a single-shot +1d4 damage rider on hits.
pub static POTION_OF_GROWTH: Item = Item {
    name: "Potion of Growth",
    glyph: '<',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_GROWTH),
    ..Item::DEFAULTS
};

/// Wand of Greater Healing — Action; touch ally heal for 4d8+4. Top
/// tier of the single-target ally-heal ladder: Scroll of Cure Wounds
/// (2d8+2) → Wand of Cure Wounds (3d8+3) → Wand of Greater Healing
/// (4d8+4).
pub static WAND_OF_GREATER_HEALING: Item = Item {
    name: "Wand of Greater Healing",
    glyph: 'K',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_GREATER_HEALING),
    ..Item::DEFAULTS
};

/// Potion of Longstrider — Bonus-Action consumable that installs
/// `Longstriding` (+10 ft speed) for 100 rounds. Mirrors the
/// Longstrider spell's effect for non-casters; cheap-tier mobility
/// consumable alongside Potion of Climbing.
pub static POTION_OF_LONGSTRIDER: Item = Item {
    name: "Potion of Longstrider",
    glyph: '>',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_LONGSTRIDER),
    ..Item::DEFAULTS
};

/// Scroll of Bless — Action; install `Blessed` for 10 rounds on a single
/// ally (+1d4 to attack rolls and saving throws). Sits in the loot pool
/// as the ally-buff counterpart to the harmful single-target CC scrolls.
pub static SCROLL_OF_BLESS: Item = Item {
    name: "Scroll of Bless",
    glyph: 'B',
    on_use: Some(&crate::actions::item_actions::READ_BLESS_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Shield of Faith — Action; install `ShieldOfFaith` for 10
/// rounds (+2 AC) on a single ally. Pairs with Scroll of Bless on the
/// ally-buff lane — the latter buffs attack rolls / saves, the former
/// boosts AC.
pub static SCROLL_OF_SHIELD_OF_FAITH: Item = Item {
    name: "Scroll of Shield of Faith",
    glyph: 'F',
    on_use: Some(&crate::actions::item_actions::READ_SHIELD_OF_FAITH_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Blindness — Action; single-target, CON save vs DC 13, fail
/// = Blinded for 10 rounds. Fills the single-target Blinded niche
/// alongside Wand of Paralysis (Paralyzed) and Wand of Fear (Frightened).
pub static SCROLL_OF_BLINDNESS: Item = Item {
    name: "Scroll of Blindness",
    glyph: '!',
    on_use: Some(&crate::actions::item_actions::READ_BLINDNESS_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Bane — Action; 4-tile burst, CHA save vs DC 13, fail =
/// Baned for 10 rounds. Mirror of Scroll of Bless on the debuff lane —
/// enemies caught in the burst eat -1d4 to attack rolls and saves.
pub static SCROLL_OF_BANE: Item = Item {
    name: "Scroll of Bane",
    glyph: 'b',
    on_use: Some(&crate::actions::item_actions::READ_BANE_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Faerie Fire — Action; 4-tile burst, DEX save vs DC 13, fail
/// = Outlined for 10 rounds (attacks against them have advantage, can't
/// benefit from Hidden / Invisible). Cheap pre-burst setup for the
/// martial-heavy party.
pub static SCROLL_OF_FAERIE_FIRE: Item = Item {
    name: "Scroll of Faerie Fire",
    glyph: 'i',
    on_use: Some(&crate::actions::item_actions::READ_FAERIE_FIRE_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Polymorph — Action; single-target, WIS save vs DC 15, fail =
/// Polymorphed for 10 rounds. Top-of-pool single-target CC consumable —
/// Polymorphed shuts down the target's entire spellcasting toolkit.
pub static WAND_OF_POLYMORPH: Item = Item {
    name: "Wand of Polymorph",
    glyph: 'p',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_POLYMORPH),
    ..Item::DEFAULTS
};

/// Potion of Barkskin — Bonus Action; installs `Barkskinned` for 10
/// rounds (AC floor of 16). Cheap defensive consumable for low-AC
/// casters; pairs with Potion of Mage Armor (AC 13 + DEX floor) and
/// Potion of Blur (disadvantage on attackers) in the defensive consumable
/// trio.
pub static POTION_OF_BARKSKIN: Item = Item {
    name: "Potion of Barkskin",
    glyph: '+',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_BARKSKIN),
    ..Item::DEFAULTS
};

/// Potion of Fire Resistance — Action; installs `DamageResistant` for 10
/// rounds. Resists everything in the engine model (the `DamageResistant`
/// condition isn't typed); the loot pool keeps the flavored name to give
/// the player a tactical "elemental shield" feel without proliferating
/// typed-resistance condition variants. Sibling to Potion of Cold
/// Resistance.
pub static POTION_OF_FIRE_RESISTANCE: Item = Item {
    name: "Potion of Fire Resistance",
    glyph: 'F',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_FIRE_RESISTANCE),
    ..Item::DEFAULTS
};

/// Potion of Cold Resistance — Action; installs `DamageResistant` for 10
/// rounds. Mirror of Potion of Fire Resistance — same envelope, distinct
/// flavor.
pub static POTION_OF_COLD_RESISTANCE: Item = Item {
    name: "Potion of Cold Resistance",
    glyph: 'C',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_COLD_RESISTANCE),
    ..Item::DEFAULTS
};

/// Potion of Hill Giant Strength — Action; installs `Enlarged` for 10
/// rounds (+1d4 weapon damage rider, size bump). Sibling to Potion of
/// Growth — same condition envelope, distinct in-fiction trigger so the
/// loot pool covers the offensive bruiser consumable lane at two rolls.
pub static POTION_OF_HILL_GIANT_STRENGTH: Item = Item {
    name: "Potion of Hill Giant Strength",
    glyph: 'G',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_HILL_GIANT_STRENGTH),
    ..Item::DEFAULTS
};

/// Slippers of Spider Climbing — passive trinket that grants the wearer
/// a climbing speed equal to their walking speed (modeled via the
/// `SpiderClimbing` condition's flat +30 ft / +12 tile speed bump). 5e
/// RAW: "you have a climbing speed equal to your walking speed; you can
/// move up, down, and across vertical surfaces and along ceilings." The
/// engine doesn't model vertical terrain — the load-bearing combat clause
/// is the speed bonus, which flows through `condition_speed_bonus`
/// alongside Longstrider / Fly. Sibling to Potion of Climbing (same
/// condition, but the potion is a single-use consumable, while the
/// slippers are passive-on-while-worn).
pub static SLIPPERS_OF_SPIDER_CLIMBING: Item = Item {
    name: "Slippers of Spider Climbing",
    glyph: '_',
    passive_conditions: &[crate::conditions::Condition::SpiderClimbing],
    ..Item::DEFAULTS
};

/// Winged Boots — passive trinket that grants the wearer a flying speed
/// equal to their walking speed (modeled via the `Flying` condition's +60
/// ft / +24 tile speed bump plus the ranged-attacker disadvantage rider).
/// 5e RAW: "while you wear these boots, you have a flying speed equal to
/// your walking speed... a total of 4 hours of flying time, split however
/// you like; recharges 2 hours at dawn." The engine doesn't model fuel
/// reserves — the boots are passive-on-while-worn. Sibling to Potion of
/// Flying (same condition, consumable variant); the boots are the rare-
/// tier permanent counterpart, with the AC bump and ranged-deflection
/// rolling through the same condition the spell installs.
pub static WINGED_BOOTS: Item = Item {
    name: "Winged Boots",
    glyph: 'w',
    passive_conditions: &[crate::conditions::Condition::Flying],
    ..Item::DEFAULTS
};

/// Boots of the Forest — passive trinket. Grants the wearer Longstriding
/// (+10 ft / +4 tile speed bump) while worn. 5e flavor mimic of the
/// Longstrider spell's effect routed through the same condition (joins
/// `condition_speed_bonus` for the additive stack with Boots of Striding's
/// flat ItemBonuses.speed). Sibling to Potion of Longstrider (consumable)
/// — the boots cover the always-on speed-bump niche.
pub static BOOTS_OF_THE_FOREST: Item = Item {
    name: "Boots of the Forest",
    glyph: '~',
    passive_conditions: &[crate::conditions::Condition::Longstriding],
    ..Item::DEFAULTS
};

/// Cloak of Etherealness — passive trinket. Grants the wearer
/// `DamageResistant` (halve all incoming damage) while worn. 5e RAW: the
/// cloak's Etherealness ability lets the wearer enter the Ethereal Plane
/// at will; we collapse the "shift planes to dodge damage" envelope to a
/// flat damage-halve buff that flows through the existing condition lane.
/// Top-of-pool defensive trinket — strictly stronger than the typed-
/// resistance rings.
pub static CLOAK_OF_ETHEREALNESS: Item = Item {
    name: "Cloak of Etherealness",
    glyph: '$',
    passive_conditions: &[crate::conditions::Condition::DamageResistant],
    ..Item::DEFAULTS
};

/// Amulet of the Vigilant — passive trinket. Grants the wearer the
/// Barbarian-style `DangerSense` (always-on advantage on DEX saves while
/// not Blinded / Incapacitated / Deafened). 5e RAW flavor: a stylized
/// amulet that hums warm when danger approaches; we abstract the
/// "preternatural sixth sense" to the Barbarian feature's mechanical
/// envelope. Distinct from the typed-resistance rings — the amulet
/// boosts DEX saves rather than halving damage, sliding cleanly under the
/// "AoE survival" niche for low-DEX casters.
pub static AMULET_OF_THE_VIGILANT: Item = Item {
    name: "Amulet of the Vigilant",
    glyph: 'V',
    passive_conditions: &[crate::conditions::Condition::DangerSense],
    ..Item::DEFAULTS
};

/// Cloak of Displacement — passive trinket (DMG, rare). Light bends around
/// the wearer so attackers see a phantom image a half-step off true: the
/// `Displaced` condition flickers on at install time and self-restores
/// at the start of every turn (so a hit that strips it mid-round comes
/// back next turn). Attackers eat disadvantage on the first swing each
/// round. 5e RAW: "while you wear this cloak, it projects an illusion
/// that makes you appear to be standing in a place near your actual
/// location" — the load-bearing combat clause is the disadvantage rider,
/// which flows through the existing `Displaced` condition install lane.
pub static CLOAK_OF_DISPLACEMENT: Item = Item {
    name: "Cloak of Displacement",
    glyph: 'd',
    passive_conditions: &[crate::conditions::Condition::Displaced],
    ..Item::DEFAULTS
};

/// Scarab of Protection — passive trinket (DMG, legendary). A beetle-
/// shaped amulet that wards the wearer against magical compulsion and
/// fear: dynamic immunity to `Charmed` and `Frightened` while carried,
/// plus a small flat +1 save bonus (the RAW "advantage on saves vs
/// spells" clause collapsed onto the load-bearing save lane, since the
/// engine doesn't have a per-school advantage hook). Pairs with the
/// Necklace of Adaptation (Poisoned-immune) and the Ring of Free Action
/// (movement-condition-immune) in the condition-immunity trinket family.
pub static SCARAB_OF_PROTECTION: Item = Item {
    name: "Scarab of Protection",
    glyph: 'S',
    bonuses: ItemBonuses { save: 1, ..ItemBonuses::ZERO },
    condition_immunities: &[
        crate::conditions::Condition::Charmed,
        crate::conditions::Condition::Frightened,
    ],
    ..Item::DEFAULTS
};

/// Ring of Heroism — passive trinket. Grants the wearer the `Heroic`
/// condition while worn: dynamic immunity to `Frightened` (the load-
/// bearing combat clause of the Heroism spell) plus the temp-HP rider
/// the condition carries. Mirrors the consumable Potion of Heroism on
/// the passive-trinket lane — the ring is always-on, the potion is a
/// one-shot 10-round buff. Sits in the rare half of the loot pool
/// alongside the other condition-installer trinkets (Amulet of the
/// Vigilant / Slippers of Spider Climbing).
pub static RING_OF_HEROISM: Item = Item {
    name: "Ring of Heroism",
    glyph: 'H',
    passive_conditions: &[crate::conditions::Condition::Heroic],
    ..Item::DEFAULTS
};

/// Wand of Sleep — single-use consumable. Single-target WIS save vs DC 13;
/// fail = `Asleep` for 10 rounds. 5e RAW: the Sleep spell is HP-pool based
/// (5d8 HP of creatures fall asleep, lowest first); we collapse to a
/// per-target save envelope every other CC consumable rides. Fills the
/// entry-level lockdown niche alongside Scroll of Hold Person (Paralyzed
/// DC 13) — Sleep wakes on damage (the engine strips `Asleep` on any non-
/// zero hit through the existing wake-on-damage hook), so it pairs with a
/// martial follow-up cleanly.
pub static WAND_OF_SLEEP: Item = Item {
    name: "Wand of Sleep",
    glyph: 'z',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_SLEEP),
    ..Item::DEFAULTS
};

/// Scroll of Slow — single-use 4-tile burst, WIS save vs DC 13, fail =
/// `Slowed` for 10 rounds (halved speed, -2 AC, -2 DEX saves). 5e RAW:
/// level-3 transmutation, WIS save, concentration; the scroll collapses
/// to the standard fixed-duration burst envelope and drops the
/// concentration gate. Mirrors Scroll of Bane / Faerie Fire on the burst
/// debuff lane — Slowed is the AC/movement counterpart to Bane's roll
/// penalties.
pub static SCROLL_OF_SLOW: Item = Item {
    name: "Scroll of Slow",
    glyph: 'l',
    on_use: Some(&crate::actions::item_actions::READ_SLOW_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Stinking Cloud — single-use 4-tile burst, CON save vs DC 15,
/// fail = `Poisoned` for 10 rounds (disadvantage on attacks / ability
/// checks). 5e RAW: level-3 conjuration, CON save, concentration; the
/// scroll collapses to a fixed-duration burst envelope and drops the
/// concentration gate. Fills the burst-Poisoned niche in the loot pool
/// alongside Pipes of Haunting (burst Frightened) and Wand of Web
/// (burst Restrained).
pub static SCROLL_OF_STINKING_CLOUD: Item = Item {
    name: "Scroll of Stinking Cloud",
    glyph: 'c',
    on_use: Some(&crate::actions::item_actions::READ_STINKING_CLOUD_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Death Ward — single-use Action; install `DeathWarded` on a
/// single ally for 100 rounds. 5e RAW: level-4 abjuration, action, touch,
/// 8-hour duration; the scroll collapses to the engine's standard fixed-
/// duration buff envelope. The ward absorbs the next lethal blow (any
/// damage that would drop the holder to 0 HP instead leaves them at 1)
/// and then burns off. Sits in the loot pool as a defensive ally-buff
/// scroll alongside Scroll of Bless / Scroll of Shield of Faith.
pub static SCROLL_OF_DEATH_WARD: Item = Item {
    name: "Scroll of Death Ward",
    glyph: 'W',
    on_use: Some(&crate::actions::item_actions::READ_DEATH_WARD_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Aid — single-use Action; bumps up to 3 allies' max HP and
/// current HP by 5 within a 4-tile burst. 5e RAW: level-2 abjuration,
/// 8-hour duration, max 3 targets within 30 ft; the scroll collapses to
/// a burst-targeted version that picks the lowest-HP allies first. Sits
/// in the loot pool as a multi-target permanent buff scroll alongside
/// Mass Healing Word (multi-ally instant heal).
pub static SCROLL_OF_AID: Item = Item {
    name: "Scroll of Aid",
    glyph: 'A',
    on_use: Some(&crate::actions::item_actions::READ_AID_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Binding — single-use Action; single-target DEX save vs DC 15,
/// fail = `Restrained` for 10 rounds. 5e flavor: a wand carved with iron
/// runes that fixes a target in place with crackling bands of force.
/// Single-target counterpart to the Wand of Web (burst Restrained).
/// Distinct loot tier from the Wand of Web — the burst hits more targets
/// but the wand is harder to dodge (single-target DC 15 vs burst DC 15).
pub static WAND_OF_BINDING: Item = Item {
    name: "Wand of Binding",
    glyph: 'B',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_BINDING),
    ..Item::DEFAULTS
};

/// Scroll of Banishment — single-use Action; single-target CHA save vs
/// DC 15, fail = `Mazed` (banished demi-plane envelope) for 10 rounds.
/// 5e RAW: level-4 abjuration, concentration, 60-ft range; the scroll
/// drops concentration and uses a fixed 10-round timer. Sits at the top
/// of the single-target CC tier alongside Wand of Polymorph — both
/// effectively remove the target from the encounter.
pub static SCROLL_OF_BANISHMENT: Item = Item {
    name: "Scroll of Banishment",
    glyph: 'X',
    on_use: Some(&crate::actions::item_actions::READ_BANISHMENT_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Fear — single-use Action; 4-tile burst, WIS save vs DC 15,
/// fail = `Frightened` for 10 rounds. 5e RAW: level-3 illusion,
/// concentration, 30-ft cone; the scroll drops concentration and uses a
/// burst envelope. Harder DC version of Pipes of Haunting (burst
/// Frightened DC 13) — sits alongside Wand of Fear (single-target
/// Frightened DC 15) so the loot pool covers all three combinations of
/// (burst/single, soft/hard DC) on the Frightened lane.
pub static SCROLL_OF_FEAR: Item = Item {
    name: "Scroll of Fear",
    glyph: 'r',
    on_use: Some(&crate::actions::item_actions::READ_FEAR_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Charm Person — single-use Action; single-target, WIS save vs
/// DC 13, fail = `Charmed` for 10 rounds. 5e RAW: level-1 enchantment, 30-ft
/// range, 1-hour duration; the scroll drops concentration (Charm Person has
/// none anyway) and lands the engine's standard 10-round CC envelope. Entry-
/// level enchantment scroll alongside Scroll of Hold Person / Wand of Sleep.
/// Charm-immune families (undead, constructs, fiends) silently skip the save
/// through the up-front immunity filter in `SingleSaveConditionItem`.
pub static SCROLL_OF_CHARM_PERSON: Item = Item {
    name: "Scroll of Charm Person",
    glyph: 'p',
    on_use: Some(&crate::actions::item_actions::READ_CHARM_PERSON_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Charm Monster — single-use Action; single-target, WIS save vs DC
/// 15, fail = `Charmed` for 10 rounds. Top-tier enchantment consumable: same
/// shape as `SCROLL_OF_CHARM_PERSON` but a harder DC and longer reach (60 ft
/// vs 30 ft). 5e RAW: level-4 enchantment, no concentration, 1-hour duration.
/// Distinct from the scroll variant since Charm Monster lands on creature
/// types the Person variant can't reach — the engine doesn't gate by creature
/// type today, so the niche is the harder DC + longer reach.
pub static WAND_OF_CHARM_MONSTER: Item = Item {
    name: "Wand of Charm Monster",
    glyph: 'm',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_CHARM_MONSTER),
    ..Item::DEFAULTS
};

/// Scroll of Tasha's Hideous Laughter — single-use Action; single-target,
/// WIS save vs DC 13, fail = `Incapacitated` for 10 rounds. 5e RAW: level-1
/// enchantment, 30-ft range, concentration, re-save each turn; the scroll
/// drops concentration and uses the engine's standard fixed-duration CC
/// envelope. Sits in the loot pool as the entry-level Incapacitated
/// installer alongside Scroll of Charm Person — both are WIS save vs DC 13.
pub static SCROLL_OF_TASHAS_HIDEOUS_LAUGHTER: Item = Item {
    name: "Scroll of Tasha's Hideous Laughter",
    glyph: 'L',
    on_use: Some(&crate::actions::item_actions::READ_TASHAS_HIDEOUS_LAUGHTER_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Heat Metal — single-use Bonus Action; single-target, CON save
/// vs DC 13, fail = `HeatMetaled` for 10 rounds (disadvantage on attack
/// rolls / ability checks). 5e RAW: level-2 transmutation, action, no save
/// on cast (CON save each turn to drop the gear); the scroll collapses to
/// a single up-front CON save vs the standard scroll DC. Drops the per-
/// round fire damage rider from the spell — the load-bearing combat clause
/// is the attack-roll disadvantage, which flows through the existing
/// `HeatMetaled` condition. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static SCROLL_OF_HEAT_METAL: Item = Item {
    name: "Scroll of Heat Metal",
    glyph: 'h',
    on_use: Some(&crate::actions::item_actions::READ_HEAT_METAL_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Ice Storm — single-use Action; 4-tile burst, DEX save vs DC 15,
/// fail = 4d8 cold, pass = half. 5e RAW: level-4 evocation that deals
/// 2d8 bludgeoning + 4d6 cold; we collapse the dual-type damage to a single
/// cold roll (4d8) so the scroll fires through the shared
/// `BurstSaveDamageItem` impl. Sits between Wand of Cone of Cold (10d8 cold,
/// 6-radius) and the scroll-tier cold burst as the mid-tier cold-burst
/// scroll.
pub static SCROLL_OF_ICE_STORM: Item = Item {
    name: "Scroll of Ice Storm",
    glyph: 'I',
    on_use: Some(&crate::actions::item_actions::READ_ICE_STORM_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Web — single-use Action; 4-tile burst, DEX save vs DC 13, fail
/// = `Restrained` for 10 rounds. 5e RAW: level-2 conjuration, concentration,
/// 60-ft range / 20-ft cube, no concentration on the scroll. Sibling to
/// Wand of Web (DC 15 Restrained burst) at the cheap-tier weight — same
/// shape, easier DC. Both ride the shared `BurstSaveConditionItem` impl.
pub static SCROLL_OF_WEB: Item = Item {
    name: "Scroll of Web",
    glyph: 'W',
    on_use: Some(&crate::actions::item_actions::READ_WEB_SCROLL),
    ..Item::DEFAULTS
};

/// Potion of Resistance — Action; installs `DamageResistant` (halve all
/// incoming damage) for 10 rounds. 5e RAW grants resistance to a single
/// damage type; we collapse to the engine's blanket `DamageResistant`
/// envelope every other resistance potion (Fire / Cold / Stoneskin) rides.
/// Single-use; rejects re-drink when already resistant. Sits in the loot
/// pool as the un-flavored generic counterpart to the typed resistance
/// potions — the player can grab whichever flavor they roll without
/// stratifying the buff itself.
pub static POTION_OF_RESISTANCE: Item = Item {
    name: "Potion of Resistance",
    glyph: 'R',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_RESISTANCE),
    ..Item::DEFAULTS
};

/// Potion of Vigilance — Bonus Action; installs `DangerSense` (advantage on
/// DEX saves while not Blinded / Incapacitated / Deafened) for 10 rounds.
/// Consumable counterpart to the passive `AMULET_OF_THE_VIGILANT`: the
/// amulet is always-on, the potion is a single-shot 10-round buff. Fits the
/// "AoE survival burst" niche for low-DEX casters who can't afford an
/// amulet slot. Single-use; rejects re-drink when already active.
pub static POTION_OF_VIGILANCE: Item = Item {
    name: "Potion of Vigilance",
    glyph: 'v',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_VIGILANCE),
    ..Item::DEFAULTS
};

/// Necklace of Fireballs — Action; 5d6 fire DEX-save burst (DC 15)
/// centered on a target tile within 60 ft. Single-bead consumable (RAW
/// has a multi-bead necklace ladder; we collapse to a single-use scroll-
/// style envelope so the loot pool stays simple). Fires through the
/// shared `BurstSaveDamageItem` impl. Sits between Scroll of Fireball
/// (6d6) and Wand of Fireballs (8d6) — same envelope, smaller payload
/// to mark the bead-tier of the magic-fireball ladder.
pub static NECKLACE_OF_FIREBALLS: Item = Item {
    name: "Necklace of Fireballs",
    glyph: 'N',
    on_use: Some(&crate::actions::item_actions::USE_NECKLACE_OF_FIREBALLS),
    ..Item::DEFAULTS
};

/// Dust of Disappearance — Bonus Action; installs the `Invisible`
/// condition on the holder for 10 rounds. 5e RAW: 2d4 minutes invisible;
/// the engine collapses to the standard combat-scale 10-round timer
/// every Invisibility-flavored consumable rides. Counterpart to Potion
/// of Invisibility (Action cost) — the dust trades action lane for the
/// bonus-action quickness. Fires through the shared `SelfConditionItem`
/// impl.
pub static DUST_OF_DISAPPEARANCE: Item = Item {
    name: "Dust of Disappearance",
    glyph: 'd',
    on_use: Some(&crate::actions::item_actions::USE_DUST_OF_DISAPPEARANCE),
    ..Item::DEFAULTS
};

/// Wand of Suggestion — Action; single-target WIS save vs DC 15, fail =
/// `Charmed` for 10 rounds. 5e RAW: level-2 enchantment, concentration,
/// 30-ft range; the wand collapses to the standard fixed-duration
/// consumable envelope and drops the concentration gate. Sibling to
/// Wand of Charm Monster (also Charmed, also DC 15) — different reach
/// (24 vs 30 tiles RAW) so the loot pool has two flavored entries on
/// the same condition lane. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static WAND_OF_SUGGESTION: Item = Item {
    name: "Wand of Suggestion",
    glyph: 'u',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_SUGGESTION),
    ..Item::DEFAULTS
};

/// Scroll of Calm Emotions — Action; 4-tile burst, CHA save vs DC 13,
/// fail = `Charmed` for 10 rounds. 5e RAW: level-2 enchantment,
/// concentration, two-option toggle (suppress fear OR Charm). The
/// engine collapses to the Charm-installer half (the engine-relevant
/// combat clause) and drops the concentration gate. Burst counterpart
/// to Scroll of Charm Person (single-target, same DC) and
/// debuff-burst-AoE counterpart to Scroll of Bane (CHA-save burst).
/// Fires through the shared `BurstSaveConditionItem` impl.
pub static SCROLL_OF_CALM_EMOTIONS: Item = Item {
    name: "Scroll of Calm Emotions",
    glyph: 's',
    on_use: Some(&crate::actions::item_actions::READ_CALM_EMOTIONS_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Blindness — Action; single-target CON save vs DC 15, fail =
/// `Blinded` for 10 rounds. 5e RAW: level-2 necromancy (Blindness /
/// Deafness), 30-ft range, CON save; the wand bumps the DC to the
/// standard wand tier (15) and the reach to 24 tiles (60 ft RAW). Top-
/// tier counterpart to Scroll of Blindness (CON save, DC 13) on the
/// single-target Blinded lane. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static WAND_OF_BLINDNESS: Item = Item {
    name: "Wand of Blindness",
    glyph: 'b',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_BLINDNESS),
    ..Item::DEFAULTS
};

/// Scroll of Mass Cure Wounds — Action; touches every ally within a
/// 4-tile burst centered on the caster (RAW: 3d8 + spellcasting modifier
/// per ally; the scroll uses a flat 3d8+5 per ally). 5e RAW: level-5
/// conjuration, 60-ft range, up to 6 targets in a 30-ft radius. The
/// scroll collapses to a self-centered burst with the standard
/// 4-tile envelope and reuses the existing mass-heal item lane via the
/// hand-rolled `READ_MASS_HEALING_WORD_SCROLL` (no shared factor yet for
/// this exact shape — Mass Cure Wounds heals more per ally but uses an
/// Action cost vs the bonus-action Healing Word variant).
pub static SCROLL_OF_MASS_CURE_WOUNDS: Item = Item {
    name: "Scroll of Mass Cure Wounds",
    glyph: 'X',
    on_use: Some(&crate::actions::item_actions::READ_MASS_CURE_WOUNDS_SCROLL),
    ..Item::DEFAULTS
};

/// Boots of Levitation — passive trinket. Wearer is treated as Flying
/// (the engine's binary flight model). 5e RAW: action toggle to
/// levitate for up to 10 minutes; the engine collapses to a permanent
/// always-on flight install via the `passive_conditions` lane so the
/// boots sit alongside Winged Boots (also Flying) and Slippers of
/// Spider Climbing on the mobility-trinket lane. Difference from Winged
/// Boots: cosmetic (vertical-only vs full flight RAW) — both grant the
/// same in-engine `Flying` flag, so the loot pool keeps two flavored
/// entries on the same mechanical envelope.
pub static BOOTS_OF_LEVITATION: Item = Item {
    name: "Boots of Levitation",
    glyph: 'L',
    passive_conditions: &[crate::conditions::Condition::Flying],
    ..Item::DEFAULTS
};

/// Cloak of Elvenkind — passive trinket. Grants the wearer the
/// `Untracked` condition (Pass Without Trace's +10 stealth-flavored
/// rider, modeled in the engine as a flat to-hit-vs-the-wearer
/// disadvantage chokepoint). 5e RAW: "creatures that try to spot you
/// have disadvantage on Wisdom (Perception) checks" — the engine
/// collapses Perception to the attack-against-the-wearer disadvantage
/// since stealth-as-cover-for-the-next-swing is the load-bearing
/// in-combat consequence. Sits alongside Cloak of Displacement on the
/// "attacker-disadvantage" trinket lane.
pub static CLOAK_OF_ELVENKIND: Item = Item {
    name: "Cloak of Elvenkind",
    glyph: 'e',
    passive_conditions: &[crate::conditions::Condition::Untracked],
    ..Item::DEFAULTS
};

/// Ring of Spell Storing — Action; the ring discharges into a Magic-
/// Missile-style auto-hit dart against a single target (3 darts ×
/// 1d4+1 force, no save). Single-use; we collapse the RAW "stored
/// spells" subsystem (which would require an Action / SpellSlot ledger
/// on the trinket) to a fixed force-dart payload, matching the size of
/// the level-1 Magic Missile scroll. Fires through the shared
/// `MagicMissileItem` impl.
pub static RING_OF_SPELL_STORING: Item = Item {
    name: "Ring of Spell Storing",
    glyph: 'S',
    on_use: Some(&crate::actions::item_actions::USE_RING_OF_SPELL_STORING),
    ..Item::DEFAULTS
};

/// Scroll of Cloudkill — Action; 4-tile burst, CON save vs DC 15, fail =
/// 5d8 poison damage. 5e RAW: level-5 conjuration, 40-ft moving cloud
/// dealing 5d8 poison; the scroll collapses the moving-cloud lane to a
/// single one-shot burst-damage roll. Sits in the rare half of the
/// poison-burst lane (the only poison-damage burst consumable; Stinking
/// Cloud uses poison-CONDITION rather than poison-DAMAGE). Fires through
/// the shared `BurstSaveDamageItem` impl.
pub static SCROLL_OF_CLOUDKILL: Item = Item {
    name: "Scroll of Cloudkill",
    glyph: 'C',
    on_use: Some(&crate::actions::item_actions::READ_CLOUDKILL_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Prayer of Healing — Action; self-centered burst that heals
/// up to 6 nearest allies for 2d8+3 HP each (within 6 tiles). 5e RAW:
/// level-2 evocation; 10-min cast time and 30-ft range. The scroll
/// collapses the 10-min cast to an Action and uses a tighter ally pick
/// envelope. Sits between Mass Healing Word (1d4+3 bonus action) and
/// Mass Cure Wounds (3d8+5 action) on the multi-target heal ladder.
/// Fires through the shared `MultiTargetHealItem` impl.
pub static SCROLL_OF_PRAYER_OF_HEALING: Item = Item {
    name: "Scroll of Prayer of Healing",
    glyph: 'P',
    on_use: Some(&crate::actions::item_actions::READ_PRAYER_OF_HEALING_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Greater Cure Wounds — Action; 4d8+5 single-target heal at
/// touch range. Mid-tier between Scroll of Cure Wounds (2d8+2) and the
/// Wand of Greater Healing (4d8+4) on the single-target ally heal lane.
/// 5e RAW: Cure Wounds upcast at level 4 = 5d8 + caster mod; we collapse
/// to 4d8+5 to slot cleanly between the scroll and wand tiers. Fires
/// through the shared `SingleTargetHealItem` impl.
pub static SCROLL_OF_GREATER_CURE_WOUNDS: Item = Item {
    name: "Scroll of Greater Cure Wounds",
    glyph: 'g',
    on_use: Some(&crate::actions::item_actions::READ_GREATER_CURE_WOUNDS_SCROLL),
    ..Item::DEFAULTS
};

/// Potion of Haste — Bonus Action; installs `Hasted` on the holder for
/// 10 rounds (+2 AC, advantage on DEX saves, doubled walking speed).
/// Distinct from Potion of Speed (extra Action this turn plus a flat
/// +1 attack/save): Haste rides the engine's `Hasted` condition for the
/// AC / DEX-save / speed bundle. Fires through the shared
/// `SelfConditionItem` impl. Rejects re-drink when already Hasted.
pub static POTION_OF_HASTE: Item = Item {
    name: "Potion of Haste",
    glyph: 'H',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_HASTE),
    ..Item::DEFAULTS
};

/// Scroll of Flesh to Stone — Action; single-target CON save vs DC 15,
/// fail = `Petrified` for 10 rounds. 5e RAW: level-6 transmutation,
/// concentration, three-save ladder; the scroll collapses to a single
/// save-or-stone install and drops concentration. Top of the single-
/// target lockdown ladder — Petrified blocks the action economy AND
/// drops the target's AC against physical damage (auto-fail STR / DEX
/// saves). Fires through the shared `SingleSaveConditionItem` impl.
pub static SCROLL_OF_FLESH_TO_STONE: Item = Item {
    name: "Scroll of Flesh to Stone",
    glyph: 'F',
    on_use: Some(&crate::actions::item_actions::READ_FLESH_TO_STONE_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Synaptic Static — 8d6 psychic-damage burst at DC 15 INT save.
/// Fills the psychic burst-damage niche in the scroll family alongside
/// Fire / Lightning / Cold / Acid / Thunder / Poison. Fires through the
/// shared `BurstSaveDamageItem` action impl.
pub static SCROLL_OF_SYNAPTIC_STATIC: Item = Item {
    name: "Scroll of Synaptic Static",
    glyph: 'y',
    on_use: Some(&crate::actions::item_actions::READ_SYNAPTIC_STATIC_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Circle of Death — 8d6 necrotic-damage burst at DC 15 CON save.
/// The only necrotic-damage burst consumable in the loot pool; sits in
/// the rare half of the lane alongside Synaptic Static (psychic) as a
/// premium typed-burst scroll. Fires through the shared
/// `BurstSaveDamageItem` action impl.
pub static SCROLL_OF_CIRCLE_OF_DEATH: Item = Item {
    name: "Scroll of Circle of Death",
    glyph: 'O',
    on_use: Some(&crate::actions::item_actions::READ_CIRCLE_OF_DEATH_SCROLL),
    ..Item::DEFAULTS
};

/// Potion of Mind Blank — installs `MindBlanked` on the drinker for 10
/// rounds. Top-tier mental-defense consumable: immune to psychic damage
/// AND immune to the Charmed condition for the duration. Sits in the
/// rare half of the loot pool alongside Periapt of Proof against Poison
/// (poison immunity passive) on the typed-immunity lane. Fires through
/// the shared `SelfConditionItem` action impl.
pub static POTION_OF_MIND_BLANK: Item = Item {
    name: "Potion of Mind Blank",
    glyph: 'M',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_MIND_BLANK),
    ..Item::DEFAULTS
};

/// Scroll of Disintegrate — Action; single-target DEX save vs DC 15.
/// On fail: 10d6+40 force damage; on save: nothing (no save half). 5e
/// RAW: level-6 transmutation, force-typed (rarely resisted in the
/// engine's pool). Top of the single-target burst-scroll lane. Fires
/// through the shared `SingleSaveDamageItem` impl.
pub static SCROLL_OF_DISINTEGRATE: Item = Item {
    name: "Scroll of Disintegrate",
    glyph: 'D',
    on_use: Some(&crate::actions::item_actions::READ_DISINTEGRATE_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Finger of Death — Action; single-target CON save vs DC 15.
/// On fail: 7d8+30 necrotic damage; on save: half. 5e RAW: level-7
/// necromancy with a "rise as zombie" rider — the scroll drops the
/// raise clause and surfaces the damage half. Sibling to Scroll of
/// Disintegrate (force, no-save-half) on the rare single-target
/// damage-scroll lane. Fires through the shared `SingleSaveDamageItem`
/// impl.
pub static SCROLL_OF_FINGER_OF_DEATH: Item = Item {
    name: "Scroll of Finger of Death",
    glyph: 'X',
    on_use: Some(&crate::actions::item_actions::READ_FINGER_OF_DEATH_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Hold Monster — Action; single-target WIS save vs DC 17, fail =
/// `Paralyzed` for 10 rounds. Top of the Hold-Paralyzed ladder above
/// Scroll of Hold Monster (DC 15) and Wand of Paralysis (DC 15, shorter
/// reach). Single-use; consumed on use. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static WAND_OF_HOLD_MONSTER: Item = Item {
    name: "Wand of Hold Monster",
    glyph: '!',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_HOLD_MONSTER),
    ..Item::DEFAULTS
};

/// Potion of Foresight — Action; installs `Foreseen` for 10 rounds
/// (advantage on attacks / saves / ability checks; attackers have
/// disadvantage against the holder). 5e RAW: level-9 divination, 8-hour
/// concentration; the potion collapses to a combat-scale fixed-duration
/// self-buff. Top-tier offensive AND defensive consumable; sits alongside
/// the rare passive trinkets in the loot pool. Single-use; rejects
/// re-drink while already up. Fires through the shared `SelfConditionItem`
/// impl.
pub static POTION_OF_FORESIGHT: Item = Item {
    name: "Potion of Foresight",
    glyph: 'F',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_FORESIGHT),
    ..Item::DEFAULTS
};

/// Necklace of Prayer Beads — Bonus Action; touch-range single-ally
/// install of `Blessed` for 10 rounds. 5e RAW: a strand of 24-30 beads,
/// each storing one cleric spell; we collapse to a single-bead consumable
/// firing the Bless spell. Sibling to Scroll of Bless on the Blessed
/// lane — the necklace is the trinket-flavored bonus-action variant.
/// Single-use; the bead crumbles to dust on use. Fires through the
/// shared `SingleTargetBuffItem` impl.
pub static NECKLACE_OF_PRAYER_BEADS: Item = Item {
    name: "Necklace of Prayer Beads",
    glyph: 'p',
    on_use: Some(&crate::actions::item_actions::USE_NECKLACE_OF_PRAYER_BEADS),
    ..Item::DEFAULTS
};

/// Scroll of Heal — Action; touch-range single-target flat 70 HP heal.
/// 5e RAW: level-6 evocation, 70 HP heal + clears Blinded / Deafened /
/// Diseased on the target. The scroll collapses to the raw-HP-heal half.
/// Top of the single-target ally heal ladder above Wand of Greater
/// Healing (4d8+4). Custom `Action` impl rather than the shared
/// `SingleTargetHealItem` factor (flat heal — no dice).
pub static SCROLL_OF_HEAL: Item = Item {
    name: "Scroll of Heal",
    glyph: 'h',
    on_use: Some(&crate::actions::item_actions::READ_HEAL_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Phantasmal Killer — Action; single-target WIS save vs DC 15,
/// fail = `Frightened` for 10 rounds. 5e RAW: level-4 illusion,
/// concentration, recurring 4d10 psychic damage; the scroll drops the
/// per-turn-damage ramp and surfaces the Frightened install at the rare
/// DC 15 / 48-tile reach tier. Sibling to Wand of Fear (also Frightened
/// DC 15) — same envelope, illusion flavor. Fires through the shared
/// `SingleSaveConditionItem` impl.
pub static SCROLL_OF_PHANTASMAL_KILLER: Item = Item {
    name: "Scroll of Phantasmal Killer",
    glyph: 'K',
    on_use: Some(&crate::actions::item_actions::READ_PHANTASMAL_KILLER_SCROLL),
    ..Item::DEFAULTS
};

/// Potion of Mirror Image — Action; installs `MirroredImages` for 10
/// rounds (three illusory duplicates intercept attacks until popped
/// one-by-one). 5e RAW: level-2 illusion spell; the potion drops the
/// spell-slot cost and collapses to the fixed-duration consumable
/// envelope. Defensive consumable on the rare half of the pool. Single-
/// use; rejects re-drink while already up. Fires through the shared
/// `SelfConditionItem` impl.
pub static POTION_OF_MIRROR_IMAGE: Item = Item {
    name: "Potion of Mirror Image",
    glyph: 'i',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_MIRROR_IMAGE),
    ..Item::DEFAULTS
};

/// Periapt of Health — passive trinket. Grants the wearer immunity to
/// the Poisoned condition while carried. 5e RAW (DMG): "you are immune
/// to contracting any disease while you wear this pendant"; we collapse
/// the disease clause onto Poisoned-immunity (the load-bearing in-engine
/// equivalent) and use the existing `condition_immunities` install gate.
/// Distinct from Necklace of Adaptation (same single condition, different
/// in-fiction flavor) — pads the loot pool with a second Poisoned-immune
/// passive trinket.
pub static PERIAPT_OF_HEALTH: Item = Item {
    name: "Periapt of Health",
    glyph: '*',
    condition_immunities: &[crate::conditions::Condition::Poisoned],
    ..Item::DEFAULTS
};

/// Wings of Flying — passive trinket. Grants the wearer the Flying
/// condition while worn (modeled via the existing Flying envelope:
/// +24 tile / +60 ft speed bump and ranged-attacker disadvantage).
/// 5e RAW (DMG): a cape that unfurls into wings, granting a 60 ft flying
/// speed for up to 1 hour; the engine doesn't track fuel reserves, so
/// the wings are passive-on-while-worn. Sibling to Winged Boots and
/// Boots of Levitation — distinct cape-slot flavor on the always-on
/// flight lane, lets the loot pool drop a flight option without
/// committing the boots / feet slot.
pub static WINGS_OF_FLYING: Item = Item {
    name: "Wings of Flying",
    glyph: 'W',
    passive_conditions: &[crate::conditions::Condition::Flying],
    ..Item::DEFAULTS
};

/// Carpet of Flying — passive trinket. Grants the wearer the Flying
/// condition while carried. 5e RAW (DMG): a finely woven carpet that the
/// owner can command to fly; the engine doesn't model riding/dismounting
/// or carrying-capacity, so we collapse to a passive Flying trinket
/// (same envelope as Wings of Flying / Winged Boots). Distinct loot
/// entry for the carpet-flavor; lets a single dungeon roll surface more
/// than one flight option without piling identical entries on the boots
/// slot.
pub static CARPET_OF_FLYING: Item = Item {
    name: "Carpet of Flying",
    glyph: 'F',
    passive_conditions: &[crate::conditions::Condition::Flying],
    ..Item::DEFAULTS
};

/// Talisman of Pure Good — passive trinket. RAW (DMG): legendary holy
/// relic with a Channel Divinity-style burst against evil; we collapse
/// the per-day burst clauses to a flat defensive stack (+1 AC, +2 save)
/// — the load-bearing combat envelope a paladin / cleric wants from a
/// holy talisman. Slots above Ring of Protection (+1/+1) and Stone of
/// Good Luck (+1/+1) on the save-bonus ladder; the +2 save edge marks
/// it as a premium passive entry.
pub static TALISMAN_OF_PURE_GOOD: Item = Item {
    name: "Talisman of Pure Good",
    glyph: 'T',
    bonuses: ItemBonuses { ac: 1, save: 2, ..ItemBonuses::ZERO },
    ..Item::DEFAULTS
};

/// Periapt of Mind Blocking — passive trinket. Grants the wearer
/// immunity to psychic damage AND to the Charmed condition while worn.
/// 5e RAW flavor: an amulet that shields the bearer's mind from
/// telepathic probes and psychic assault; the engine collapses the
/// mental-defense envelope to the two load-bearing combat clauses
/// (typed psychic damage zero, Charmed install blocked). Sibling to
/// Ring of Mind Shielding (which already grants psychic immunity) —
/// pads the loot pool with a second mental-defense passive on a
/// distinct slot, and adds the Charmed-immune lane the ring doesn't
/// cover.
pub static PERIAPT_OF_MIND_BLOCKING: Item = Item {
    name: "Periapt of Mind Blocking",
    glyph: 'M',
    damage_immunities: &[crate::engine::types::DamageType::Psychic],
    condition_immunities: &[crate::conditions::Condition::Charmed],
    ..Item::DEFAULTS
};

/// Scroll of Hellish Rebuke — single-target DEX save vs DC 13 fire damage
/// consumable. RAW (level-1 evocation, reaction): the caster wreathes the
/// attacker in flames for 2d10 fire on a failed save, half on a pass. The
/// scroll drops the reaction-timing clause and surfaces the save-or-half
/// payload as an Action consumable.
pub static SCROLL_OF_HELLISH_REBUKE: Item = Item {
    name: "Scroll of Hellish Rebuke",
    glyph: 'h',
    on_use: Some(&crate::actions::item_actions::READ_HELLISH_REBUKE_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Mind Spike — single-target WIS save vs DC 15 psychic damage
/// consumable. RAW: level-2 divination, 3d8 psychic with a tracking
/// rider; the wand surfaces the save-for-half damage half and drops
/// the concentration tracker. Fills the psychic-damage single-target
/// slot in the loot pool — sibling to Wand of Lightning Bolts and the
/// Scroll of Hellish Rebuke on the typed-damage consumable lane.
pub static WAND_OF_MIND_SPIKE: Item = Item {
    name: "Wand of Mind Spike",
    glyph: 'X',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_MIND_SPIKE),
    ..Item::DEFAULTS
};

/// Eyes of Charming — passive trinket consumable. RAW (DMG): "as an
/// action, you can cast the Charm Person spell on a humanoid within 30
/// feet, expending 1 of 3 charges." The engine collapses the 3-charge
/// ladder to a single-use scroll-style envelope: one Charm Person cast
/// (WIS save vs DC 13, fail = Charmed for 10 rounds) and the item is
/// consumed. Sibling to Scroll of Charm Person on the entry-tier Charmed
/// installer lane.
pub static EYES_OF_CHARMING: Item = Item {
    name: "Eyes of Charming",
    glyph: 'e',
    on_use: Some(&crate::actions::item_actions::USE_EYES_OF_CHARMING),
    ..Item::DEFAULTS
};

/// Gem of Brightness — burst-blind consumable. RAW (DMG): a gem with
/// charges that flash a blinding light at one creature or in a cone.
/// The engine collapses the multi-mode utility to the single combat-
/// relevant clause — the cone Blind — and uses a burst envelope
/// (4-tile radius) with a CON save vs DC 14. On a failed save the
/// target is Blinded for 10 rounds.
pub static GEM_OF_BRIGHTNESS: Item = Item {
    name: "Gem of Brightness",
    glyph: 'G',
    on_use: Some(&crate::actions::item_actions::USE_GEM_OF_BRIGHTNESS),
    ..Item::DEFAULTS
};

/// Scroll of Resilient Sphere — single-target DEX save vs DC 15 lockdown
/// consumable. RAW: level-4 evocation (Otiluke's Resilient Sphere),
/// concentration; the scroll drops the concentration gate. On a failed
/// save the target is `Sphered` for 10 rounds — a full lockdown envelope
/// (zero movement, action economy blocked, attacks against advantage,
/// holder's own attacks at disadvantage, no reactions). Top-tier
/// single-target CC consumable, sitting alongside Wand of Polymorph
/// and Scroll of Flesh to Stone in the rare half of the loot pool.
pub static SCROLL_OF_RESILIENT_SPHERE: Item = Item {
    name: "Scroll of Resilient Sphere",
    glyph: 'O',
    on_use: Some(&crate::actions::item_actions::READ_RESILIENT_SPHERE_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Telekinesis — single-target STR save vs DC 15 lift
/// consumable. RAW: level-5 transmutation, concentration; the scroll
/// surfaces the lift-and-hold half and drops the concentration gate.
/// On a failed save the target is `Lifted` for 10 rounds (movement
/// zeroed; melee swings get advantage since the target's dangling
/// helplessly). Mirror of Resilient Sphere on the movement-pin lane,
/// at a lighter lockdown (no action-economy block).
pub static SCROLL_OF_TELEKINESIS: Item = Item {
    name: "Scroll of Telekinesis",
    glyph: 'T',
    on_use: Some(&crate::actions::item_actions::READ_TELEKINESIS_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Telekinesis — single-target STR save vs DC 17, fail = `Lifted`
/// for 10 rounds. Top tier of the Lifted ladder above the Scroll of
/// Telekinesis (DC 15). Harder DC and longer reach (90 ft RAW vs the
/// scroll's 60 ft) — mirrors the Scroll vs Wand of Hold Monster tier
/// split on the Paralyzed lane.
pub static WAND_OF_TELEKINESIS: Item = Item {
    name: "Wand of Telekinesis",
    glyph: 'K',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_TELEKINESIS),
    ..Item::DEFAULTS
};

/// Scroll of Earthen Grasp — single-target STR save vs DC 13 grapple
/// consumable. RAW: Maximilian's Earthen Grasp (level-2 transmutation,
/// concentration); the scroll drops the concentration gate. On a failed
/// save the target gets `EarthenGrasped` for 10 rounds — a Restrained
/// envelope plus 2d6 bludgeoning round-end drip from the existing
/// `ROUND_END_DOTS` registry. Sibling to Scroll of Web (burst Restrained)
/// on the entry-tier CC lane — trades the burst envelope for a single-
/// target DoT rider.
pub static SCROLL_OF_EARTHEN_GRASP: Item = Item {
    name: "Scroll of Earthen Grasp",
    glyph: 'E',
    on_use: Some(&crate::actions::item_actions::READ_EARTHEN_GRASP_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Sleep — 4-tile burst centered on a picked tile. Rolls a
/// 5d8 HP pool and sweeps enemy creatures in ascending current-HP order,
/// putting each to `Asleep` + `Prone` until the pool is consumed. RAW:
/// level-1 enchantment, no save — the HP-bucket IS the gate. The scroll
/// reuses the `pool_sweep_targets` chokepoint the SLEEP spell rides, so
/// undead / constructs / fey ancestry races are correctly spared via
/// the Charmed-immunity "mind-affecting" proxy.
pub static SCROLL_OF_SLEEP: Item = Item {
    name: "Scroll of Sleep",
    glyph: 'Z',
    on_use: Some(&crate::actions::item_actions::READ_SLEEP_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Sacred Flame — single-target DEX save vs DC 13 radiant
/// damage consumable. RAW: cantrip (1d8 at level 1, scaling to 2d8 at
/// level 5 / 3d8 at level 11 / 4d8 at level 17); the scroll bakes in
/// the level-5 damage tier (2d8) since scrolls don't carry caster-level.
/// Pure radiant single-target damage — sibling to Wand of Mind Spike
/// (psychic) and Scroll of Hellish Rebuke (fire) on the entry-tier
/// damage-scroll lane.
pub static SCROLL_OF_SACRED_FLAME: Item = Item {
    name: "Scroll of Sacred Flame",
    glyph: 'r',
    on_use: Some(&crate::actions::item_actions::READ_SACRED_FLAME_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Mind Sliver — single-target INT save vs DC 13 psychic
/// damage consumable. RAW (Tasha's): cantrip 1d6 with a "subtract 1d4
/// from the target's next save" rider; the scroll bakes in the level-5
/// damage tier (2d6) and drops the rider. Cheap psychic single-target
/// damage — sits on the same shape as Scroll of Sacred Flame but with
/// an INT save rather than DEX, so a different stat profile gets bitten.
pub static SCROLL_OF_MIND_SLIVER: Item = Item {
    name: "Scroll of Mind Sliver",
    glyph: 'y',
    on_use: Some(&crate::actions::item_actions::READ_MIND_SLIVER_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Moonbeam — 3-tile burst, CON save vs DC 15, 5d10 radiant
/// damage on fail (half on pass). RAW: level-2 evocation, concentration,
/// sustained zone (4d10 / round in a 5-ft cylinder); the scroll collapses
/// the persistent drip to a single burst-on-cast envelope at a slightly
/// boosted single-cast tier (5d10 vs the 4d10 per-tick). Fills the
/// radiant burst niche between Scroll of Shatter (3d8 thunder DC 13)
/// and Scroll of Synaptic Static (8d6 psychic DC 15).
pub static SCROLL_OF_MOONBEAM: Item = Item {
    name: "Scroll of Moonbeam",
    glyph: 'm',
    on_use: Some(&crate::actions::item_actions::READ_MOONBEAM_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Guiding Bolt — single-target auto-hit consumable that deals
/// 4d6 radiant damage AND installs `GuidingBoltLit` for 1 round on the
/// target. RAW: level-1 evocation, spell attack roll, 4d6 radiant +
/// "next attack against target has advantage" rider. The scroll bakes
/// in the "guaranteed hit" envelope SRD scrolls auto-resolve as: no
/// attack roll, no save. The condition rider sets up the rest of the
/// party for a free advantaged swing on the same target.
pub static SCROLL_OF_GUIDING_BOLT: Item = Item {
    name: "Scroll of Guiding Bolt",
    glyph: 'g',
    on_use: Some(&crate::actions::item_actions::READ_GUIDING_BOLT_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Enhance Ability — Action; single-target ally buff (touch).
/// Installs `Heroic` for 10 rounds — Frightened-immunity envelope. RAW:
/// level-2 transmutation, concentration, touch; the scroll bypasses the
/// concentration gate. Sibling to Scroll of Bless / Shield of Faith on
/// the support-buff lane.
pub static SCROLL_OF_ENHANCE_ABILITY: Item = Item {
    name: "Scroll of Enhance Ability",
    glyph: 'e',
    on_use: Some(&crate::actions::item_actions::READ_ENHANCE_ABILITY_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Blink — Action; self-buff. Installs `Displaced` for 10
/// rounds (attackers roll at disadvantage; broken on first damage taken).
/// RAW: level-3 transmutation, no concentration. Sibling to Potion of
/// Blur / Potion of Mirror Image on the attacker-disadvantage defensive
/// consumable lane — distinct by the scroll envelope (any caster can
/// read it) and the on-damage break.
pub static SCROLL_OF_BLINK: Item = Item {
    name: "Scroll of Blink",
    glyph: 'k',
    on_use: Some(&crate::actions::item_actions::READ_BLINK_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Contagion — Action; single-target CON save vs DC 15
/// (touch). On fail target picks up `Poisoned` for 10 rounds. RAW:
/// level-5 necromancy, three-save chain to disease; the scroll collapses
/// to a single save vs the fixed DC. Slots in the rare half of the loot
/// pool alongside the other lockdown scrolls.
pub static SCROLL_OF_CONTAGION: Item = Item {
    name: "Scroll of Contagion",
    glyph: 'X',
    on_use: Some(&crate::actions::item_actions::READ_CONTAGION_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Spider Climb — Action; installs `SpiderClimbing` for 10
/// rounds on the reader. Self-only mobility consumable: the holder gets
/// a +20 ft climb speed and the wall-climb fiction the existing
/// Slippers of Spider Climbing trinket rides. Sibling to Potion of
/// Climbing on the cheap-mobility consumable lane.
pub static SCROLL_OF_SPIDER_CLIMB: Item = Item {
    name: "Scroll of Spider Climb",
    glyph: 'C',
    on_use: Some(&crate::actions::item_actions::READ_SPIDER_CLIMB_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Heroism — Action; installs `Heroic` for 10 rounds + 10
/// temp HP on the reader. RAW: level-1 enchantment, concentration,
/// touch; the scroll bypasses the concentration gate. Sibling to
/// Potion of Heroism (same install, bonus-action cost) on the
/// Frightened-immunity + temp-HP self-buff lane.
pub static SCROLL_OF_HEROISM: Item = Item {
    name: "Scroll of Heroism",
    glyph: 'H',
    on_use: Some(&crate::actions::item_actions::READ_HEROISM_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Bless — Bonus Action; single-target install of `Blessed`
/// for 10 rounds. Sibling to Scroll of Bless on the support-buff lane;
/// distinct from the scroll by the bonus-action cost (a wounded martial
/// can buff AND swing on the same turn) and the longer 30-ft reach.
pub static WAND_OF_BLESS: Item = Item {
    name: "Wand of Bless",
    glyph: 'b',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_BLESS),
    ..Item::DEFAULTS
};

/// Necklace of Lightning Bolts — 5d6 lightning DEX-save burst (DC 15,
/// 2 radius). Sibling to Necklace of Fireballs on the lightning lane —
/// same payload shape and consumable envelope, different damage type
/// so the resistance landscape differs.
pub static NECKLACE_OF_LIGHTNING_BOLTS: Item = Item {
    name: "Necklace of Lightning Bolts",
    glyph: 'N',
    on_use: Some(&crate::actions::item_actions::USE_NECKLACE_OF_LIGHTNING_BOLTS),
    ..Item::DEFAULTS
};

/// Scroll of Mind Blank — Action; installs `MindBlanked` for 10 rounds
/// on the reader (Charmed + psychic immunity). 5e RAW: level-8
/// abjuration, 24-hour duration; the scroll collapses to the engine's
/// combat-scale envelope. Sibling to Potion of Mind Blank (same
/// install, bonus-action cost) on the top-tier mental-defense
/// consumable lane.
pub static SCROLL_OF_MIND_BLANK: Item = Item {
    name: "Scroll of Mind Blank",
    glyph: 'M',
    on_use: Some(&crate::actions::item_actions::READ_MIND_BLANK_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Mass Bless — Action; install `Blessed` on up to 3 allies
/// within a 12-tile burst (30 ft RAW) of the reader. 5e RAW: Bless is
/// a level-1 concentration buff hitting up to 3 creatures; the scroll
/// collapses to the fixed 10-round non-concentration envelope every
/// support-scroll buff rides. Sibling to `SCROLL_OF_BLESS` (single
/// target, same condition / duration) — the mass variant is the multi-
/// ally counterpart at one tier up in the loot pool. Fires through the
/// shared `MultiTargetBuffItem` impl.
pub static SCROLL_OF_MASS_BLESS: Item = Item {
    name: "Scroll of Mass Bless",
    glyph: '1',
    on_use: Some(&crate::actions::item_actions::READ_MASS_BLESS_SCROLL),
    ..Item::DEFAULTS
};

/// Banner of Valor — Bonus Action; raise the banner to install `Heroic`
/// (Frightened-immunity) on up to 4 allies within a 6-tile self-burst
/// (15 ft) AND grant each 5 temp HP. Bardic / paladin trinket flavor.
/// Single-use consumable — the banner unfurls once. Sits in the loot
/// pool as a mass-buff alternative to Scroll of Mass Bless (3 allies,
/// Blessed, Action cost) — distinct by the action-economy cost, the
/// shorter self-centered burst, and the temp-HP cushion rider. Fires
/// through the shared `MultiTargetBuffItem` impl via the `temp_hp` lane.
pub static BANNER_OF_VALOR: Item = Item {
    name: "Banner of Valor",
    glyph: '2',
    on_use: Some(&crate::actions::item_actions::USE_BANNER_OF_VALOR),
    ..Item::DEFAULTS
};

/// Drum of Inspiration — Action; beat the drum to install `Inspired`
/// (single-d6 buff die on the next attack roll / save / check) on up
/// to 4 allies within a 12-tile burst (30 ft) of the holder. Single-use
/// consumable — the drum-skin tears after one beat. Bardic flavor;
/// distinct from Banner of Valor (Heroic + temp HP, bonus action) on
/// the Inspired condition lane and the longer reach. Fires through the
/// shared `MultiTargetBuffItem` impl.
pub static DRUM_OF_INSPIRATION: Item = Item {
    name: "Drum of Inspiration",
    glyph: '3',
    on_use: Some(&crate::actions::item_actions::USE_DRUM_OF_INSPIRATION),
    ..Item::DEFAULTS
};

/// Wand of Stunning — Action; single-target CON save vs DC 15, fail =
/// Stunned for 10 rounds. The only consumable in the loot pool that
/// installs Stunned (blocks every action-economy slot AND movement RAW).
/// Sibling to Wand of Hold Monster (Paralyzed DC 17) on the rare half of
/// the single-target lockdown lane — distinct by Stunned's strictly
/// stronger envelope (no movement either) and the lower CON DC tier.
/// Fires through the shared `SingleSaveConditionItem` impl.
pub static WAND_OF_STUNNING: Item = Item {
    name: "Wand of Stunning",
    glyph: '4',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_STUNNING),
    ..Item::DEFAULTS
};

/// Iron Bands of Bilarro — Action; throw at a target up to 24 tiles away
/// (60 ft RAW), STR save vs DC 17, fail = Restrained for 10 rounds. Top
/// of the single-target Restrained ladder — sibling to Scroll of Earthen
/// Grasp (DC 13 STR-save Restrained) at the rare DC 17 tier. Single-use
/// thrown consumable; the bands tighten on impact and lock the target in
/// place. Fires through the shared `SingleSaveConditionItem` impl.
pub static IRON_BANDS_OF_BILARRO: Item = Item {
    name: "Iron Bands of Bilarro",
    glyph: '5',
    on_use: Some(&crate::actions::item_actions::USE_IRON_BANDS_OF_BILARRO),
    ..Item::DEFAULTS
};

/// Scroll of Sanctuary — Bonus Action; install `Sanctuary` for 10 rounds
/// on a single ally within 12 tiles (30 ft RAW). Sibling to Potion of
/// Sanctuary (self-only, BA drink) — the scroll variant wards a different
/// ally (the rogue in the back, the cleric setting up a heal) so the
/// caster doesn't have to drink-then-attack. Fires through the shared
/// `SingleTargetBuffItem` impl.
pub static SCROLL_OF_SANCTUARY: Item = Item {
    name: "Scroll of Sanctuary",
    glyph: '6',
    on_use: Some(&crate::actions::item_actions::READ_SANCTUARY_SCROLL),
    ..Item::DEFAULTS
};

/// Wand of Mass Cure Wounds — Action; heal up to 6 allies within a
/// 12-tile burst (30 ft RAW) for 5d8+5 HP each. Top of the multi-target
/// ally-heal ladder above Scroll of Mass Cure Wounds (3d8+5). 5e RAW:
/// level-5 Mass Cure Wounds upcast pool; the wand variant collapses to a
/// one-shot cast at the level-5 envelope. Single rare entry in the loot
/// pool. Fires through the shared `MultiTargetHealItem` impl.
pub static WAND_OF_MASS_CURE_WOUNDS: Item = Item {
    name: "Wand of Mass Cure Wounds",
    glyph: '7',
    on_use: Some(&crate::actions::item_actions::USE_WAND_OF_MASS_CURE_WOUNDS),
    ..Item::DEFAULTS
};

/// Scroll of Crusader's Mantle — Action; install `CrusadersMantled` on
/// up to 4 allies within a 12-tile burst (30 ft RAW) for 10 rounds. The
/// condition rides the `ON_HIT_RIDERS` table in `engine::attack` to add
/// +1d4 radiant to every weapon hit. Sibling to Scroll of Mass Bless on
/// the multi-target offensive buff lane — distinct by the radiant
/// damage rider vs. Bless's flat +1d4 attack / save modifier. Fires
/// through the shared `MultiTargetBuffItem` impl.
pub static SCROLL_OF_CRUSADERS_MANTLE: Item = Item {
    name: "Scroll of Crusader's Mantle",
    glyph: '8',
    on_use: Some(&crate::actions::item_actions::READ_CRUSADERS_MANTLE_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Pyrotechnics — Action; 1d8 fire CON-save burst at DC 13,
/// 2-radius / 24-tile reach. XGE level-2 transmutation (Fireworks
/// variant). Entry-tier elemental-burst consumable; sibling to Scroll
/// of Burning Hands / Thunderwave on the cheap typed-damage burst lane.
pub static SCROLL_OF_PYROTECHNICS: Item = Item {
    name: "Scroll of Pyrotechnics",
    glyph: '9',
    on_use: Some(&crate::actions::item_actions::READ_PYROTECHNICS_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Flame Arrows — Action; self-buff. Installs `FlamingArrowed`
/// for 10 rounds (the +1d6 fire ranged-only on-hit rider rides through
/// `ON_HIT_RIDERS`). XGE level-3 transmutation, concentration RAW; the
/// scroll bypasses the concentration gate. Sibling to Scroll of Spirit
/// Shroud (melee-only cold rider) on the per-hit weapon buff scroll
/// lane — distinct by the ranged-only gate.
pub static SCROLL_OF_FLAME_ARROWS: Item = Item {
    name: "Scroll of Flame Arrows",
    glyph: '0',
    on_use: Some(&crate::actions::item_actions::READ_FLAME_ARROWS_SCROLL),
    ..Item::DEFAULTS
};

/// Potion of Ashardalon's Stride — Bonus Action; self-buff. Installs
/// `AshardalonStriding` for 10 rounds (+20 ft speed plus 1d6 fire trail
/// damage to footprint-adjacent enemies per step). TCE level-3
/// transmutation, concentration RAW; the potion bypasses the
/// concentration gate. Sibling to Potion of Longstrider (passive speed)
/// — distinct by the combat-flavored trail-damage hook.
pub static POTION_OF_ASHARDALONS_STRIDE: Item = Item {
    name: "Potion of Ashardalon's Stride",
    glyph: 'a',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_ASHARDALONS_STRIDE),
    ..Item::DEFAULTS
};

/// Potion of Otherworldly Guise — Bonus Action; self-buff. Installs
/// `OtherworldlyGuised` for 10 rounds: +2 AC, +60 ft fly speed,
/// radiant / poison resistance, Charmed / Frightened / Poisoned dynamic
/// immunity, +2d6 radiant per melee weapon hit (via `ON_HIT_RIDERS`).
/// TCE level-6 transmutation, concentration RAW; the potion bypasses
/// the concentration gate. Top-tier offensive / defensive consumable
/// alongside Potion of Foresight.
pub static POTION_OF_OTHERWORLDLY_GUISE: Item = Item {
    name: "Potion of Otherworldly Guise",
    glyph: 'O',
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_OTHERWORLDLY_GUISE),
    ..Item::DEFAULTS
};

/// Horn of Blasting — self-centered 6-tile thunder burst (5d6, CON DC 15
/// save-for-half) plus a Deafened rider on failed save. Single-use
/// consumable in this engine (RAW: 7 charges with recharge — we collapse
/// to a one-shot drop so the loot pool stays flat). Sibling to Necklace
/// of Fireballs / Lightning Bolts on the elemental-burst consumable lane —
/// distinct by the self-centered shape (no targeted tile) and the thunder
/// damage type.
pub static HORN_OF_BLASTING: Item = Item {
    name: "Horn of Blasting",
    glyph: 'H',
    on_use: Some(&crate::actions::item_actions::BLOW_HORN_OF_BLASTING),
    ..Item::DEFAULTS
};

/// Javelin of Lightning — thrown 120-ft (48 tile) lightning burst
/// (4d6, DEX DC 13 save-for-half, 2-tile radius). Single-use consumable.
/// Mirror of Necklace of Lightning Bolts (5d6 DC 15) at a cheaper save
/// DC + smaller payload tier — fills the entry-level lightning-burst
/// niche above Scroll of Lightning Bolt (which is 8d6 DC 15 burst, no
/// throw envelope) and below the necklace's per-bead tier.
pub static JAVELIN_OF_LIGHTNING: Item = Item {
    name: "Javelin of Lightning",
    glyph: 'J',
    on_use: Some(&crate::actions::item_actions::THROW_JAVELIN_OF_LIGHTNING),
    ..Item::DEFAULTS
};

/// Bead of Force — small force-typed burst consumable (5d4 force, DEX
/// DC 15 save-for-half, 2-tile radius). Drops the rarely-resisted force
/// damage type into the burst-scroll lane. Sibling to Scroll of Magic
/// Missile (auto-hit force darts) on the force-damage consumable lane —
/// distinct by the burst envelope (vs single-target dart pile) and the
/// per-target save semantics.
pub static BEAD_OF_FORCE: Item = Item {
    name: "Bead of Force",
    glyph: 'q',
    on_use: Some(&crate::actions::item_actions::THROW_BEAD_OF_FORCE),
    ..Item::DEFAULTS
};

/// Scroll of Fly — Action; install `Flying` for 10 rounds on a single
/// ally within touch (1 tile). Sibling to Winged Boots (passive Flying)
/// and Potion of Flying (self-only consumable) — distinct by the
/// ally-target envelope (let the rogue / fighter sky-dance without their
/// own caster needing a free hand).
pub static SCROLL_OF_FLY: Item = Item {
    name: "Scroll of Fly",
    glyph: '0',
    on_use: Some(&crate::actions::item_actions::READ_FLY_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Bestow Curse — touch single-target, WIS save vs DC 15 or
/// `Baned` for 10 rounds. Sibling to Scroll of Bane (burst Baned DC 13)
/// on the Baned lane — distinct by the single-target shape and the
/// meaner DC tier.
pub static SCROLL_OF_BESTOW_CURSE: Item = Item {
    name: "Scroll of Bestow Curse",
    glyph: '0',
    on_use: Some(&crate::actions::item_actions::READ_BESTOW_CURSE_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Conjure Animals — Action; summons two spectral wolves on
/// the caster's team adjacent to them. Concentration-bound; dropping
/// concentration despawns the cohort via the shared `Conjured` cleanup
/// path. Surfaces the CONJURE_ANIMALS spell envelope as a consumable
/// for non-druid loot drops — distinct from Animate Dead's permanent
/// skeleton minion in that the wolves vanish when the spell ends.
pub static SCROLL_OF_CONJURE_ANIMALS: Item = Item {
    name: "Scroll of Conjure Animals",
    glyph: '0',
    on_use: Some(&crate::actions::item_actions::READ_CONJURE_ANIMALS_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Longstrider — Action consumable that installs the
/// `Longstriding` condition (+10 ft walking speed) for 100 rounds on a
/// touched ally. 5e RAW: Longstrider is a level-1 transmutation; the
/// scroll surfaces the mobility buff as a fire-and-forget consumable.
/// Lower-tier sibling of `SCROLL_OF_FLY` (vertical lift via the Flying
/// condition) — Longstrider's flat-ground speed bump slots in below
/// the level-3 fly tier so the loot pool covers both ends of the
/// movement-buff range.
pub static SCROLL_OF_LONGSTRIDER: Item = Item {
    name: "Scroll of Longstrider",
    glyph: '0',
    on_use: Some(&crate::actions::item_actions::READ_LONGSTRIDER_SCROLL),
    ..Item::DEFAULTS
};

/// Scroll of Barkskin — Action consumable that installs the
/// `Barkskinned` condition (AC floor of 16 — see the condition impl)
/// for 100 rounds on a touched ally. 5e RAW: Barkskin is a level-2
/// transmutation, concentration; the scroll bypasses the concentration
/// cost and surfaces the AC-floor envelope as a fire-and-forget ally
/// buff. Common druid / ranger trinket — pairs with
/// `SCROLL_OF_LONGSTRIDER` for a low-tier mobility + defense kit.
pub static SCROLL_OF_BARKSKIN: Item = Item {
    name: "Scroll of Barkskin",
    glyph: '0',
    on_use: Some(&crate::actions::item_actions::READ_BARKSKIN_SCROLL),
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
    // Secondary-resistance ring family — covers the remaining damage
    // types the engine actually fires (poison / radiant / necrotic /
    // thunder / psychic). Same single-entry weighting as the elemental
    // four so the loot pool spreads coverage across every burst type
    // without piling weight onto any single resistance source.
    &RING_OF_POISON_RESISTANCE,
    &RING_OF_RADIANT_RESISTANCE,
    &RING_OF_NECROTIC_RESISTANCE,
    &RING_OF_THUNDER_RESISTANCE,
    &RING_OF_PSYCHIC_RESISTANCE,
    // Single-use AoE scrolls / wands — same weight as the Fireball /
    // Lightning Bolt scrolls so casters have a roughly even shot at a
    // big burst regardless of element. Cone of Cold's 8d8 cold tier sits
    // above the 6d6 Fireball / 8d6 Lightning Bolt tier; Wand of Magic
    // Missiles' 5-dart payload sits above the 3-dart scroll tier.
    &SCROLL_OF_CONE_OF_COLD,
    &WAND_OF_MAGIC_MISSILES,
    // Wand of Fireballs / Lightning Bolts sit one tier above their
    // scroll counterparts: 8d6 fire vs 6d6, 10d6 lightning vs 8d6.
    &WAND_OF_FIREBALLS,
    &WAND_OF_LIGHTNING_BOLTS,
    // Mobility potions — Flying is the premium full-flight envelope,
    // Climbing is the cheaper bonus-action variant. Single entries each
    // since mobility buffs are situationally strong (kiting / chasing).
    &POTION_OF_FLYING,
    &POTION_OF_CLIMBING,
    // Superior Healing — top-tier consumable heal. Single entry weights
    // it below the regular Potion of Healing (3 entries) — it's a
    // premium drop.
    &POTION_OF_SUPERIOR_HEALING,
    // Stoneskin — premium defensive consumable; halves all damage for
    // 10 rounds. Single entry — same tier as the premium passive
    // trinkets.
    &POTION_OF_STONESKIN,
    // Wand of Cone of Cold rounds out the burst-wand trio (fire /
    // lightning / cold), all at the "scroll + 1 tier" pool size.
    &WAND_OF_CONE_OF_COLD,
    // Scroll of Shatter fills the thunder lane in the scroll family
    // alongside fire / lightning / cold.
    &SCROLL_OF_SHATTER,
    // Pearl-of-Power ladder — three tiers of spell-slot refunds. Single
    // entries each since slot-refund consumables are situationally
    // strong (the Supreme Pearl refunds a level-3 slot worth far more
    // than the base Pearl). Sits one tier above Boots of Speed in the
    // caster-consumable lane.
    &GREATER_PEARL_OF_POWER,
    &SUPREME_PEARL_OF_POWER,
    // Mass Healing Word scroll — bonus-action ally aura heal. Single
    // entry; complements the single-target Cure Wounds scroll for
    // multi-ally emergency healing.
    &SCROLL_OF_MASS_HEALING_WORD,
    // Magical weapon ladder — +1 sits at common weight (mirroring
    // Cloak of Resistance / Ring of Protection); +2 is single-entry
    // premium. Both grant +N attack AND +N damage on every swing so
    // the loot tier slots cleanly between trinkets (defensive) and
    // burst scrolls (offensive).
    &WEAPON_PLUS_ONE,
    &WEAPON_PLUS_ONE,
    &WEAPON_PLUS_TWO,
    // Bracers of Archery — +2 damage trinket. Same weight as the
    // single-element resistance rings; sits as an offensive-niche
    // trinket alongside the defensive AC / save bumps.
    &BRACERS_OF_ARCHERY,
    // Ioun Stone of Mastery — caster-flavored +1/+1 (attack/save) trinket.
    // Sits alongside Stone of Good Luck (+1/+1 AC/save) and the Cloak of
    // Protection (+1/+1 AC/save) as a third "+1 to two stats" passive.
    &IOUN_STONE_OF_MASTERY,
    // Sentinel Shield — low-tier defensive trinket. Same weight as the
    // generic Ring of Protection / Cloak of Protection siblings.
    &SENTINEL_SHIELD,
    // +3 Weapon — top tier of the magical-weapon ladder. Single-entry
    // rare drop, paired with the existing +1 (common, weight 2) and
    // +2 (single entry) tiers.
    &WEAPON_PLUS_THREE,
    // Belt of Giant Strength — offensive bruiser trinket: +damage and
    // +max-HP. Single low-weight entry alongside Gauntlets of Ogre
    // Power.
    &BELT_OF_GIANT_STRENGTH,
    // Supreme Healing — rarest tier of the healing-potion ladder.
    // Single low-weight entry above Superior Healing (also single).
    &POTION_OF_SUPREME_HEALING,
    // Caster-flavored buff potions. Same weight as the Stoneskin /
    // Invisibility tier — defensive consumables for low-AC casters.
    &POTION_OF_MAGE_ARMOR,
    &POTION_OF_BLUR,
    // Greater Wand of Magic Missiles — top tier of the MM ladder
    // (3-dart scroll → 5-dart wand → 7-dart greater wand).
    &GREATER_WAND_OF_MAGIC_MISSILES,
    // Entry-level burst scrolls — fire / thunder lane at the common
    // weight tier (one notch below the Fireball / Lightning Bolt
    // scrolls in damage payload).
    &SCROLL_OF_BURNING_HANDS,
    &SCROLL_OF_THUNDERWAVE,
    // Crowd-control consumables — burst save-or-condition wands fill
    // the CC niche alongside the burst-damage scrolls / wands. Single
    // entry per item; the Restrained / Frightened / Paralyzed lanes
    // are all situationally strong (a paralyzed boss is effectively
    // free damage for the rest of the party), so the loot pool keeps
    // them rare.
    &WAND_OF_WEB,
    &PIPES_OF_HAUNTING,
    &WAND_OF_PARALYSIS,
    &WAND_OF_FEAR,
    // Hold Person / Monster scrolls — single-target Paralyzed at the
    // cheap (DC 13, 24-tile) and rare (DC 15, 36-tile) tiers. Sibling
    // to the Wand of Paralysis (DC 15, 24-tile) — the scroll variants
    // cover the entry-level lockdown niche and the long-reach niche
    // respectively.
    &SCROLL_OF_HOLD_PERSON,
    &SCROLL_OF_HOLD_MONSTER,
    // Burst CC scrolls — Hypnotic Pattern (Incapacitated DC 14) and
    // Wand of Confusion (Confused DC 15). Fill the mid- and top-tier
    // burst CC slots between Pipes of Haunting (Frightened DC 13) and
    // the single-target Wand of Paralysis.
    &SCROLL_OF_HYPNOTIC_PATTERN,
    &WAND_OF_CONFUSION,
    // Vitriolic Sphere scroll — acid lane in the burst-damage scroll
    // family, sized between Fireball (6d6) and Cone of Cold (8d8).
    &SCROLL_OF_VITRIOLIC_SPHERE,
    // Archmage Pearl of Power — top of the pearl ladder, single entry
    // since level-4 slot refunds are the rarest tier.
    &ARCHMAGE_PEARL_OF_POWER,
    // Potion of Sanctuary — defensive consumable; routes hostile
    // actions through a WIS save until the holder swings back. Single
    // entry alongside the other defensive potions (Mage Armor, Blur).
    &POTION_OF_SANCTUARY,
    // Cure Wounds wand + Healing Word scroll — round out the single-
    // target ally-heal slot. Wand sits a tier above the scroll
    // (3d8+3 vs 2d8+2 touch); the Healing Word scroll covers the
    // long-reach kite-heal niche at BA cost.
    &WAND_OF_CURE_WOUNDS,
    &SCROLL_OF_HEALING_WORD,
    // Potion of Growth — Enlarged buff consumable on the offensive
    // bruiser lane. Single low-weight entry alongside the other
    // size-mod trinkets (Belt of Giant Strength).
    &POTION_OF_GROWTH,
    // Wand of Greater Healing — top tier of the single-target ally
    // heal ladder (2d8+2 scroll → 3d8+3 wand → 4d8+4 greater wand).
    &WAND_OF_GREATER_HEALING,
    // Potion of Longstrider — cheap mobility consumable. Same low-weight
    // tier as Potion of Climbing; +10 ft for the encounter. Bonus-action
    // drink so it doesn't compete with the holder's main turn budget.
    &POTION_OF_LONGSTRIDER,
    // Ally-buff scrolls — Bless and Shield of Faith fill the support
    // scroll niche alongside the Mass Healing Word / Cure Wounds heal
    // scrolls. Both buff a single ally for 10 rounds; Bless boosts
    // attack rolls / saves (+1d4), Shield of Faith boosts AC (+2).
    &SCROLL_OF_BLESS,
    &SCROLL_OF_SHIELD_OF_FAITH,
    // Single-target CC scrolls — Blindness (CON save Blinded) fills the
    // entry-level "save-or-suck single target" niche alongside the rare
    // Wand of Paralysis. Wand of Polymorph sits at the top of the same
    // ladder — Polymorphed locks out the target's spellcasting toolkit.
    &SCROLL_OF_BLINDNESS,
    &WAND_OF_POLYMORPH,
    // Burst CC scrolls — Bane (CHA save Baned) is the burst debuff
    // counterpart to Bless; Faerie Fire (DEX save Outlined) sets up the
    // martial-heavy party for advantage on follow-up swings.
    &SCROLL_OF_BANE,
    &SCROLL_OF_FAERIE_FIRE,
    // Defensive consumables — Barkskin (AC 16 floor), Fire / Cold
    // Resistance potions (blanket damage halve), and Hill Giant Strength
    // (Enlarged size+damage). Single entries each; the resistance
    // potions share the `DamageResistant` lane with Potion of Stoneskin
    // so the loot pool has multiple flavored entries on the same
    // mechanical envelope.
    &POTION_OF_BARKSKIN,
    &POTION_OF_FIRE_RESISTANCE,
    &POTION_OF_COLD_RESISTANCE,
    &POTION_OF_HILL_GIANT_STRENGTH,
    // Passive-condition trinkets — Slippers of Spider Climbing
    // (SpiderClimbing speed bump), Winged Boots (Flying), Boots of the
    // Forest (Longstriding +10 ft), Cloak of Etherealness (DamageResistant
    // blanket halve), Amulet of the Vigilant (DangerSense DEX-save
    // advantage). All ride the new `passive_conditions` item lane so the
    // install lives on `pickup_item` and persists across long rests via
    // the `reinstall_item_passive_conditions` hook.
    &SLIPPERS_OF_SPIDER_CLIMBING,
    &WINGED_BOOTS,
    &BOOTS_OF_THE_FOREST,
    &CLOAK_OF_ETHEREALNESS,
    &AMULET_OF_THE_VIGILANT,
    // Cloak of Displacement — rare passive trinket with the Displaced
    // condition rider. Pairs with the Amulet of the Vigilant in the
    // "save-flavored" trinket niche on a different defensive lane
    // (attacker-disadvantage instead of save-advantage).
    &CLOAK_OF_DISPLACEMENT,
    // Scarab of Protection — legendary-tier dual-immunity trinket plus a
    // small flat save bump. Sits in the rare half of the pool alongside
    // the Periapt of Proof against Poison / Ring of Mind Shielding
    // dual-lane trinkets.
    &SCARAB_OF_PROTECTION,
    // Ring of Heroism — passive-condition counterpart to Potion of Heroism;
    // single low-weight entry alongside Slippers / Winged Boots.
    &RING_OF_HEROISM,
    // Wand of Sleep — entry-tier single-target CC (DC 13 Asleep). Lighter
    // tier than Scroll of Hold Person (DC 13 Paralyzed) since Asleep
    // breaks on damage; the loot pool keeps both as single-entry
    // alternatives for the rogue / fighter's "first knockout swing"
    // niche.
    &WAND_OF_SLEEP,
    // Burst-debuff scrolls — Slow (WIS save, AC/movement penalty) and
    // Stinking Cloud (CON save, Poisoned blanket). Single entries each
    // alongside Scroll of Bane / Faerie Fire on the burst-debuff lane.
    &SCROLL_OF_SLOW,
    &SCROLL_OF_STINKING_CLOUD,
    // Defensive ally-buff scrolls — Death Ward (next-lethal-blow absorb)
    // and Aid (multi-ally permanent +5 max HP / current HP). Single
    // entries each in the rare half of the support-scroll lane.
    &SCROLL_OF_DEATH_WARD,
    &SCROLL_OF_AID,
    // Top-tier single-target CC consumables — Banishment (CHA save
    // banished-to-demiplane) and Binding (DEX save Restrained). Single
    // entries each in the rare half of the CC-consumable lane.
    &SCROLL_OF_BANISHMENT,
    &WAND_OF_BINDING,
    // Scroll of Fear — burst Frightened at the rare DC 15 tier. Sits
    // between Pipes of Haunting (burst DC 13) and Wand of Fear (single-
    // target DC 15).
    &SCROLL_OF_FEAR,
    // Charm consumables — single-target Charmed at the cheap (DC 13) and
    // rare (DC 15) tiers. Sit in the loot pool as the enchantment lane
    // counterparts to the Hold Person / Hold Monster Paralyzed family.
    &SCROLL_OF_CHARM_PERSON,
    &WAND_OF_CHARM_MONSTER,
    // Tasha's Hideous Laughter — entry-tier single-target Incapacitated
    // installer. Same WIS-save-DC-13 envelope as Charm Person but a
    // different lockdown condition (Incapacitated blocks actions; Charmed
    // is marker-only today).
    &SCROLL_OF_TASHAS_HIDEOUS_LAUGHTER,
    // Heat Metal scroll — single-target HeatMetaled (attack-roll
    // disadvantage) at the cheap CON-save tier. Distinct from the burst
    // debuff scrolls (Bane / Slow / Stinking Cloud) — single-target,
    // attack-roll-targeted.
    &SCROLL_OF_HEAT_METAL,
    // Ice Storm scroll — mid-tier cold burst between Cone of Cold (8d8)
    // and Shatter (3d8 thunder). Single entry — same rarity as the
    // existing cold-burst Cone of Cold scroll.
    &SCROLL_OF_ICE_STORM,
    // Web scroll — cheap-tier counterpart to Wand of Web (DC 15
    // Restrained burst). Same shape, easier DC; sits alongside Pipes of
    // Haunting (burst Frightened DC 13) at the entry-level burst CC tier.
    &SCROLL_OF_WEB,
    // Generic / un-flavored Potion of Resistance — DamageResistant for
    // 10 rounds. Drops alongside the flavored Fire / Cold variants for
    // a third roll on the same envelope.
    &POTION_OF_RESISTANCE,
    // Vigilance potion — consumable counterpart to Amulet of the Vigilant.
    // Single low-weight entry; installs the DangerSense condition.
    &POTION_OF_VIGILANCE,
    // Necklace of Fireballs — sub-tier fire burst (5d6, between scroll's
    // 6d6 and wand's 8d6). Single entry alongside the rest of the fire-
    // burst consumable family.
    &NECKLACE_OF_FIREBALLS,
    // Dust of Disappearance — bonus-action Invisibility counterpart to
    // Potion of Invisibility (Action cost). Single entry.
    &DUST_OF_DISAPPEARANCE,
    // Wand of Suggestion — single-target Charmed at the rare DC 15 tier.
    // Sibling to Wand of Charm Monster on the Charmed lane.
    &WAND_OF_SUGGESTION,
    // Scroll of Calm Emotions — burst Charmed at the cheap DC 13 tier;
    // burst counterpart to Scroll of Charm Person (single, same DC).
    &SCROLL_OF_CALM_EMOTIONS,
    // Wand of Blindness — single-target Blinded at the rare DC 15 tier.
    // Top-tier counterpart to Scroll of Blindness (CON save DC 13).
    &WAND_OF_BLINDNESS,
    // Scroll of Mass Cure Wounds — burst ally heal at Action cost.
    // Pairs with Mass Healing Word scroll (bonus-action variant) on the
    // mass-heal scroll lane.
    &SCROLL_OF_MASS_CURE_WOUNDS,
    // Boots of Levitation — passive Flying trinket. Sibling to Winged
    // Boots on the always-on flight lane.
    &BOOTS_OF_LEVITATION,
    // Cloak of Elvenkind — passive Untracked trinket. Attacker-disadvantage
    // counterpart to Cloak of Displacement; lower weight as a single entry.
    &CLOAK_OF_ELVENKIND,
    // Ring of Spell Storing — single-use Magic-Missile-style force dart
    // volley. Sibling to Scroll of Magic Missile on the auto-hit lane.
    &RING_OF_SPELL_STORING,
    // Scroll of Cloudkill — 5d8 poison-damage burst at DC 15. The only
    // poison-damage burst consumable; sits alongside Stinking Cloud
    // (poison-CONDITION) on the poison-flavored AoE lane.
    &SCROLL_OF_CLOUDKILL,
    // Scroll of Prayer of Healing — mid-tier multi-ally heal between
    // Mass Healing Word (1d4+3 bonus action) and Mass Cure Wounds
    // (3d8+5 action). Single entry on the multi-target heal lane.
    &SCROLL_OF_PRAYER_OF_HEALING,
    // Scroll of Greater Cure Wounds — 4d8+5 single-target touch heal.
    // Slots between the cheap Cure Wounds scroll and the Wand of
    // Greater Healing on the ally-heal ladder.
    &SCROLL_OF_GREATER_CURE_WOUNDS,
    // Potion of Haste — Hasted install on a bonus action. Distinct from
    // Potion of Speed (extra-action burst); same condition envelope as
    // the Boots of Speed but bonus-action timing.
    &POTION_OF_HASTE,
    // Scroll of Flesh to Stone — top-tier single-target Petrified
    // installer. Single entry alongside Wand of Polymorph on the
    // rare half of the single-target lockdown lane.
    &SCROLL_OF_FLESH_TO_STONE,
    // Scroll of Synaptic Static — 8d6 psychic-damage burst at DC 15 INT
    // save. Fills the psychic-burst niche between Cone of Cold (cold)
    // and Vitriolic Sphere (acid) — single entry alongside the rare
    // typed-burst scroll tier.
    &SCROLL_OF_SYNAPTIC_STATIC,
    // Scroll of Circle of Death — 8d6 necrotic-damage burst at DC 15 CON
    // save. The only necrotic-typed burst in the loot pool; rounds out
    // the typed-burst scroll family.
    &SCROLL_OF_CIRCLE_OF_DEATH,
    // Potion of Mind Blank — top-tier mental-defense consumable.
    // Psychic + Charmed immunity for 10 rounds. Single low-weight entry
    // alongside the other premium typed-defense consumables.
    &POTION_OF_MIND_BLANK,
    // Scroll of Disintegrate / Finger of Death — top of the single-target
    // burst-damage scroll lane. Single entries each; sit alongside the
    // rare burst scrolls (Synaptic Static / Circle of Death). The two
    // damage flavors split the resistance landscape: Force (rarely
    // resisted) vs Necrotic (commonly resisted by undead).
    &SCROLL_OF_DISINTEGRATE,
    &SCROLL_OF_FINGER_OF_DEATH,
    // Wand of Hold Monster — top of the Hold-Paralyzed ladder at DC 17.
    // Single rare entry above the DC 15 scroll variant.
    &WAND_OF_HOLD_MONSTER,
    // Potion of Foresight — top-tier offensive+defensive consumable.
    // Single rare entry alongside Robe of the Archmagi on the legendary
    // tier.
    &POTION_OF_FORESIGHT,
    // Necklace of Prayer Beads — trinket-flavored Bless on bonus action.
    // Single entry alongside Scroll of Bless on the support-buff lane.
    &NECKLACE_OF_PRAYER_BEADS,
    // Scroll of Heal — top of the single-target ally heal ladder; flat
    // 70 HP. Single rare entry above Wand of Greater Healing.
    &SCROLL_OF_HEAL,
    // Scroll of Phantasmal Killer — illusion-flavored Frightened install
    // at the rare DC 15 tier. Sits alongside Wand of Fear on the
    // Frightened single-target lane.
    &SCROLL_OF_PHANTASMAL_KILLER,
    // Potion of Mirror Image — three illusory duplicates. Single rare
    // entry alongside Potion of Blur / Potion of Invisibility on the
    // attacker-disadvantage defensive lane.
    &POTION_OF_MIRROR_IMAGE,
    // Periapt of Health — Poisoned-immunity passive at a low-weight slot
    // alongside Necklace of Adaptation. Loot-pool stratification: lets a
    // single dungeon roll two distinct Poisoned-immune trinkets without
    // double-rolling Necklace of Adaptation.
    &PERIAPT_OF_HEALTH,
    // Wings of Flying / Carpet of Flying — passive Flying trinkets on
    // distinct slots (cape / carry). Sit alongside Winged Boots and
    // Boots of Levitation on the always-on flight lane; the extra entries
    // let a single dungeon roll surface a flight option for a non-boots
    // loadout without piling weight onto the boots slot.
    &WINGS_OF_FLYING,
    &CARPET_OF_FLYING,
    // Talisman of Pure Good — premium passive +1 AC / +2 save trinket.
    // Single rare entry above Cloak of Protection (+1/+1) on the
    // defensive-passive ladder.
    &TALISMAN_OF_PURE_GOOD,
    // Periapt of Mind Blocking — psychic-immunity + Charmed-immunity
    // passive. Sibling to Ring of Mind Shielding (psychic immune only)
    // on the mental-defense passive lane.
    &PERIAPT_OF_MIND_BLOCKING,
    // Scroll of Hellish Rebuke — entry-tier single-target fire consumable
    // (2d10 DEX save 13, save-for-half). Sibling to the Acid Arrow / Mind
    // Spike scrolls on the single-target damage-scroll lane.
    &SCROLL_OF_HELLISH_REBUKE,
    // Wand of Mind Spike — single-target psychic damage at DC 15 WIS save
    // (3d8 save-for-half). Fills the psychic single-target damage niche
    // alongside Scroll of Hellish Rebuke (fire) and the burst Scroll of
    // Synaptic Static (psychic burst).
    &WAND_OF_MIND_SPIKE,
    // Eyes of Charming — single-target Charmed installer at DC 13. Sibling
    // to Scroll of Charm Person on the entry-tier Charmed lane.
    &EYES_OF_CHARMING,
    // Gem of Brightness — burst Blinded installer (DC 14 CON save).
    // Single low-weight entry on the burst-CC lane alongside Pipes of
    // Haunting (burst Frightened) and Wand of Web (burst Restrained).
    &GEM_OF_BRIGHTNESS,
    // Top-tier single-target lockdown consumables. Resilient Sphere
    // installs `Sphered` (full action-economy block + movement zero);
    // Forcecage's lighter sibling. Sit alongside Wand of Polymorph /
    // Scroll of Flesh to Stone in the rare half of the single-target
    // CC lane.
    &SCROLL_OF_RESILIENT_SPHERE,
    // Telekinesis ladder — Scroll (DC 15 / 60 ft) and Wand (DC 17 /
    // 90 ft) install the `Lifted` condition. Sibling to the Hold
    // Person / Hold Monster ladder on the single-target movement-pin
    // lane.
    &SCROLL_OF_TELEKINESIS,
    &WAND_OF_TELEKINESIS,
    // Earthen Grasp — single-target Restrained-equivalent at the cheap
    // DC 13 STR-save tier, plus a 2d6 bludgeoning DoT. Sibling to
    // Scroll of Web (burst Restrained) on the entry-tier CC lane.
    &SCROLL_OF_EARTHEN_GRASP,
    // Scroll of Sleep — entry-tier burst Asleep installer at the DC 13
    // WIS-save tier. Sits alongside Wand of Sleep (single-target DC 13)
    // on the entry-tier knockout lane.
    &SCROLL_OF_SLEEP,
    // Entry-tier damage scrolls — Sacred Flame (radiant DEX save) and
    // Mind Sliver (psychic INT save) round out the cheap typed-damage
    // single-target scroll family alongside Scroll of Hellish Rebuke
    // (fire). Both at DC 13.
    &SCROLL_OF_SACRED_FLAME,
    &SCROLL_OF_MIND_SLIVER,
    // Scroll of Moonbeam — radiant burst between Shatter (3d8 thunder
    // DC 13) and Synaptic Static (8d6 psychic DC 15). Fills the
    // radiant burst lane.
    &SCROLL_OF_MOONBEAM,
    // Scroll of Guiding Bolt — auto-hit radiant single-target damage
    // (4d6) + `GuidingBoltLit` rider that grants the next attacker
    // advantage. Unique support niche; sets up the next ally swing
    // for an advantaged hit on the same target.
    &SCROLL_OF_GUIDING_BOLT,
    // Scroll of Enhance Ability — touch ally buff (Heroic, 10 rounds).
    // Sibling to Scroll of Bless / Shield of Faith on the support-buff
    // scroll lane. Single low-weight entry.
    &SCROLL_OF_ENHANCE_ABILITY,
    // Scroll of Blink — self-buff (Displaced, 10 rounds). Sibling to
    // Potion of Blur / Mirror Image on the attacker-disadvantage defensive
    // consumable lane. Single low-weight entry.
    &SCROLL_OF_BLINK,
    // Scroll of Contagion — single-target CON save Poisoned installer
    // (touch, DC 15). Top-tier single-target lockdown alongside Scroll of
    // Flesh to Stone / Wand of Polymorph; single low-weight entry.
    &SCROLL_OF_CONTAGION,
    // Scroll of Spider Climb — cheap self-mobility consumable on the
    // wall-climb lane (sibling to Potion of Climbing). Single low-weight
    // entry; the install rides the existing `SpiderClimbing` condition
    // that Slippers of Spider Climbing already grants as a passive.
    &SCROLL_OF_SPIDER_CLIMB,
    // Scroll of Heroism — self-buff (Heroic, 10 rounds, +10 temp HP).
    // Sibling to Potion of Heroism on the Frightened-immunity lane —
    // distinct from the potion by Action cost and the "any caster can
    // read it" envelope.
    &SCROLL_OF_HEROISM,
    // Wand of Bless — bonus-action single-target Blessed installer.
    // Sibling to Scroll of Bless on the support-buff scroll lane;
    // distinct from the scroll by bonus-action cost (a wounded martial
    // can buff AND swing on the same turn) and the longer 30-ft reach.
    &WAND_OF_BLESS,
    // Necklace of Lightning Bolts — 5d6 lightning DEX-save burst (DC 15,
    // 2 radius). Sibling to Necklace of Fireballs on the lightning lane;
    // same single-bead envelope, different damage type so the resistance
    // landscape differs.
    &NECKLACE_OF_LIGHTNING_BOLTS,
    // Scroll of Mind Blank — top-tier mental-defense self-buff (Charmed
    // + psychic immunity for 10 rounds). Sibling to Potion of Mind Blank
    // on the rare half of the mental-defense lane; distinct from the
    // potion by Action cost.
    &SCROLL_OF_MIND_BLANK,
    // Multi-ally buff consumables — fire through the shared
    // `MultiTargetBuffItem` factor. Scroll of Mass Bless is the RAW
    // 3-target Bless cap; Banner of Valor and Drum of Inspiration are
    // bardic-flavored mass-buff siblings on the Heroic / Inspired lanes.
    &SCROLL_OF_MASS_BLESS,
    &BANNER_OF_VALOR,
    &DRUM_OF_INSPIRATION,
    // Wand of Stunning — first item to install Stunned. Single rare entry
    // alongside Wand of Hold Monster (Paralyzed DC 17) on the rare half of
    // the single-target lockdown lane.
    &WAND_OF_STUNNING,
    // Iron Bands of Bilarro — top of the single-target Restrained ladder
    // at DC 17. Sibling to Scroll of Earthen Grasp (DC 13) — same install,
    // meaner save DC.
    &IRON_BANDS_OF_BILARRO,
    // Scroll of Sanctuary — ally-target counterpart to Potion of Sanctuary
    // (self-only). Single entry on the Sanctuary lane.
    &SCROLL_OF_SANCTUARY,
    // Wand of Mass Cure Wounds — top of the multi-target ally-heal ladder
    // above Scroll of Mass Cure Wounds. Single rare entry.
    &WAND_OF_MASS_CURE_WOUNDS,
    // Scroll of Crusader's Mantle — mass-buff variant of the paladin aura
    // spell. Sits alongside Scroll of Mass Bless on the multi-target
    // offensive buff lane; +1d4 radiant rider per hit rides
    // `ON_HIT_RIDERS`.
    &SCROLL_OF_CRUSADERS_MANTLE,
    // Scroll of Pyrotechnics — XGE level-2 fire burst at the entry-tier
    // CON-save 1d8 envelope. Sibling to Scroll of Burning Hands /
    // Thunderwave on the cheap typed-damage burst lane; single low-weight
    // entry.
    &SCROLL_OF_PYROTECHNICS,
    // Scroll of Flame Arrows — XGE level-3 self-buff scroll. The +1d6
    // fire on-hit rider is ranged-only (`OnHitRider.ranged_only = true`)
    // so the buff carves out a niche distinct from Spirit Shroud /
    // Crusader's Mantle (melee + persistent) on the on-hit-buff lane.
    // Single low-weight entry.
    &SCROLL_OF_FLAME_ARROWS,
    // Potion of Ashardalon's Stride — TCE level-3 mobility + trail-damage
    // consumable. Sits alongside Potion of Longstrider on the speed-buff
    // lane, but combat-flavored (the 1d6 fire trail to footprint-adjacent
    // enemies per step is the spell's signature mechanic). Single
    // low-weight entry.
    &POTION_OF_ASHARDALONS_STRIDE,
    // Potion of Otherworldly Guise — TCE level-6 legendary-tier self-buff
    // consumable. The single most envelope-rich consumable in the loot
    // pool: +2 AC, +60 ft fly, radiant + poison resistance, three-
    // condition dynamic immunity, +2d6 radiant melee weapon rider. Single
    // rare entry alongside Potion of Foresight on the legendary tier.
    &POTION_OF_OTHERWORLDLY_GUISE,
    // Horn of Blasting — self-centered thunder burst (5d6 CON DC 15
    // save-for-half + Deafened on fail). Sibling to Necklace of Fireballs /
    // Lightning Bolts on the elemental-burst consumable lane; distinct
    // by the self-centered no-target shape.
    &HORN_OF_BLASTING,
    // Javelin of Lightning — thrown lightning burst (4d6 DEX DC 13). Mid-
    // tier lightning consumable between Scroll of Lightning Bolt (8d6
    // DC 15) and Necklace of Lightning Bolts (5d6 DC 15) on the lightning
    // burst lane.
    &JAVELIN_OF_LIGHTNING,
    // Bead of Force — small force-typed burst (5d4 DEX DC 15). The only
    // force-damage burst in the loot pool; fills the rarely-resisted
    // force lane that Scroll of Magic Missile already pioneers in the
    // auto-hit family.
    &BEAD_OF_FORCE,
    // Scroll of Fly — ally-target Flying install for 10 rounds. Sibling
    // to Winged Boots (passive) and Potion of Flying (self-only) on the
    // flight-buff lane; distinct by the ally-target envelope.
    &SCROLL_OF_FLY,
    // Scroll of Bestow Curse — single-target Baned install at the rare
    // WIS DC 15 tier. Sibling to Scroll of Bane (burst DC 13) on the
    // Baned lane.
    &SCROLL_OF_BESTOW_CURSE,
    // Scroll of Conjure Animals — summons two spectral wolves adjacent
    // to the reader, concentration-bound. The first action-economy
    // multiplier in the consumable loot pool — every other consumable
    // pays a single one-shot effect; this one adds two attacker bodies
    // to the caster's team for the rest of the concentration window.
    // Single low-weight entry.
    &SCROLL_OF_CONJURE_ANIMALS,
    // Low-tier ally-buff scrolls — Longstrider (+10 ft speed) and
    // Barkskin (AC floor 16) ride the existing SingleTargetBuffItem
    // factor for 100-round installs at touch range. Single entry each
    // — the loot pool already weights Bless / Mass Bless / Crusader's
    // Mantle for the heavier buff tier, so these two slot in as
    // cheaper utility consumables a martial can hand to the rogue or
    // wizard for a swift first-round setup.
    &SCROLL_OF_LONGSTRIDER,
    &SCROLL_OF_BARKSKIN,
];
