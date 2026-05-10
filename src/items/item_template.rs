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
#[derive(Clone)]
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
}

pub static RING_OF_PROTECTION: Item = Item {
    name: "Ring of Protection",
    glyph: '=',
    bonuses: ItemBonuses {
        ac: 1,
        max_hp: 0,
        speed: 0,
        save: 1,
    },
    on_use: None,
};

pub static BOOTS_OF_STRIDING: Item = Item {
    name: "Boots of Striding",
    glyph: 'b',
    bonuses: ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 10,
        save: 0,
    },
    on_use: None,
};

pub static CLOAK_OF_RESISTANCE: Item = Item {
    name: "Cloak of Resistance",
    glyph: 'c',
    bonuses: ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 2,
    },
    on_use: None,
};

pub static AMULET_OF_HEALTH: Item = Item {
    name: "Amulet of Health",
    glyph: 'a',
    bonuses: ItemBonuses {
        ac: 0,
        max_hp: 10,
        speed: 0,
        save: 0,
    },
    on_use: None,
};

/// Headband of Insight — minor caster-flavor trinket. +1 save bonus,
/// no AC or speed. Distinct loot tier from Cloak of Resistance (which
/// gives +2) so the loot pool has stratified strength.
pub static HEADBAND_OF_INSIGHT: Item = Item {
    name: "Headband of Insight",
    glyph: 'h',
    bonuses: ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 1,
    },
    on_use: None,
};

/// Bracers of Defense — light AC bump. Cheaper loot than Ring of
/// Protection (which gives +1 AC and +1 save), giving the LOOT_POOL
/// a clearer common / uncommon ladder.
pub static BRACERS_OF_DEFENSE: Item = Item {
    name: "Bracers of Defense",
    glyph: 'B',
    bonuses: ItemBonuses {
        ac: 1,
        max_hp: 0,
        speed: 0,
        save: 0,
    },
    on_use: None,
};

pub static POTION_OF_HEALING: Item = Item {
    name: "Potion of Healing",
    glyph: 'p',
    bonuses: ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 0,
    },
    on_use: Some(&crate::actions::item_actions::DRINK_HEALING_POTION),
};

pub static POTION_OF_GREATER_HEALING: Item = Item {
    name: "Potion of Greater Healing",
    glyph: 'P',
    bonuses: ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 0,
    },
    on_use: Some(&crate::actions::item_actions::DRINK_GREATER_HEALING_POTION),
};

pub static SCROLL_OF_FIREBALL: Item = Item {
    name: "Scroll of Fireball",
    glyph: 's',
    bonuses: ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 0,
    },
    on_use: Some(&crate::actions::item_actions::READ_FIREBALL_SCROLL),
};

pub static SCROLL_OF_MAGIC_MISSILE: Item = Item {
    name: "Scroll of Magic Missile",
    glyph: 'm',
    bonuses: ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 0,
    },
    on_use: Some(&crate::actions::item_actions::READ_MAGIC_MISSILE_SCROLL),
};

/// Cloak of Protection — premium passive trinket. +1 AC AND +1 to all
/// saves. Strictly better than Cloak of Resistance for tanks who need
/// the AC bump; rarer in the loot pool.
pub static CLOAK_OF_PROTECTION: Item = Item {
    name: "Cloak of Protection",
    glyph: 'C',
    bonuses: ItemBonuses {
        ac: 1,
        max_hp: 0,
        speed: 0,
        save: 1,
    },
    on_use: None,
};

/// Shield — passive +2 AC, no save bonus. Classic light-armor pairing
/// with one-handed weapons. Distinct loot tier from heavy armor since
/// we don't model armor proficiency yet.
pub static SHIELD: Item = Item {
    name: "Shield",
    glyph: 'S',
    bonuses: ItemBonuses {
        ac: 2,
        max_hp: 0,
        speed: 0,
        save: 0,
    },
    on_use: None,
};

/// Antitoxin — single-use consumable. Drinking removes the Poisoned
/// condition and grants advantage on the next CON save against poison
/// (modeled as a flat +5 save buff via Bless's mechanic). One-shot:
/// the action removes the item from inventory after use.
pub static ANTITOXIN: Item = Item {
    name: "Antitoxin",
    glyph: 'A',
    bonuses: ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 0,
    },
    on_use: Some(&crate::actions::item_actions::DRINK_ANTITOXIN),
};

/// Potion of Speed — bonus action; gain an extra Action this turn plus
/// a +1 attack/save buff (a simplified Haste). Single-use consumable;
/// the buff clears on long rest with the rest of the buff state.
pub static POTION_OF_SPEED: Item = Item {
    name: "Potion of Speed",
    glyph: '!',
    bonuses: ItemBonuses {
        ac: 0,
        max_hp: 0,
        speed: 0,
        save: 0,
    },
    on_use: Some(&crate::actions::item_actions::DRINK_POTION_OF_SPEED),
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
    &SCROLL_OF_FIREBALL,
    &SCROLL_OF_MAGIC_MISSILE,
];
