/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity). Attacks
    /// vs you have advantage; auto-fail STR/DEX saves.
    Stunned,
    /// Disadvantage on attack rolls and ability checks.
    Poisoned,
    /// Disadvantage on attack rolls and ability checks while you can see
    /// the source of fear. We don't track LOS-to-fear-source today; the
    /// effect is unconditional disadvantage on attacks.
    Frightened,
    /// Speed = 0; disadvantage on attacks; attacks against you have
    /// advantage; disadvantage on DEX saves.
    Restrained,
    /// Auto-fail any check requiring sight; attack rolls against you have
    /// advantage; you have disadvantage on attacks.
    Blinded,
    /// Cannot take actions or reactions (movement is still allowed).
    /// Distinct from Stunned — Incapacitated still moves; Stunned can't.
    Incapacitated,
    /// You have no effect on combat behavior — you can't take hostile
    /// actions against the charmer. Marker only today; we don't yet
    /// model the "can't attack the charmer" enforcement.
    Charmed,
    /// Cannot move; cannot take actions or reactions; auto-fail STR/DEX
    /// saves; attacks against you have advantage; melee crits are
    /// auto-criticals on hit. We model the action-economy / movement
    /// blocks here; the auto-crit on melee hit lives at the attack site.
    Paralyzed,
    /// Cannot be seen without special vision. Attack rolls against you
    /// have disadvantage; your attacks have advantage. Bookkept as a
    /// condition for clean removal on attack (per 5e Greater Invisibility
    /// vs Invisibility — we treat both as the simple Invisible condition).
    Invisible,
    /// Active until the start of your next turn after taking the Dodge
    /// action: attacks against you have disadvantage (if you can see the
    /// attacker); you have advantage on DEX saves. Cleared by the
    /// `UntilStartOfNextTurn` timer on your next turn.
    Dodging,
    /// Speed = 0; you can't gain a speed bonus. Ends when grappler is
    /// incapacitated or target is moved out of range. We track only the
    /// movement block here; ending is up to the grappler logic.
    Grappled,
    /// +1d4 to attack rolls and saving throws (Bless spell). Tracked as a
    /// condition so it ticks down with the spell timer and clears cleanly
    /// when concentration drops.
    Blessed,
    /// +2 AC from Shield of Faith (concentration buff). Read by
    /// `armor_class()`; drops when concentration drops.
    ShieldOfFaith,
    /// +5 AC from the Shield reaction spell, until the start of your
    /// next turn. Cleared by the `UntilStartOfNextTurn` timer.
    Shielded,
    /// Generic "halve incoming damage" buff (e.g. Stoneskin).
    /// Stacks multiplicatively with damage-type resistance.
    DamageResistant,
    /// Hostile-to-hostile attack against the holder is at advantage and
    /// the holder cannot benefit from being Hidden/Invisible. Used by
    /// Faerie Fire / Hunter's Mark style effects.
    Outlined,
    /// Lit by Guiding Bolt — next attack against this actor before the
    /// end of the caster's next turn has advantage. Burns off when the
    /// next attack lands or when its short timer expires.
    GuidingBoltLit,
    /// Took the Help action against this turn — your next attack against
    /// the helped target has advantage (cleared on use or end of round).
    Helped,
    /// Successful Stealth check; you have unseen advantage on attack and
    /// attackers have disadvantage. Distinct from Invisible: it's broken
    /// by attacking, ending hidden status.
    Hidden,
    /// On fire — takes 1d4 fire at the start of each of its turns until
    /// extinguished. Burning is a DOT condition with `Rounds(n)` timer.
    Burning,
    /// Took the Disengage action this turn: their movement doesn't
    /// provoke opportunity attacks. Cleared by `UntilStartOfNextTurn`.
    Disengaging,
    /// Knocked unconscious (HP 0 or magical sleep). Stronger than
    /// Incapacitated: drops prone, fails STR/DEX saves, and melee crits
    /// on hit. Set automatically when an actor enters HpState::Dying.
    Unconscious,
    /// -1d4 (modeled as -2) to attack rolls and saving throws (Bane spell).
    /// Symmetric counterpart to `Blessed`. Tracked as a condition so it
    /// ticks down with the spell timer and clears on concentration drop.
    Baned,
    /// Cannot hear; auto-fail any check requiring hearing. We don't yet
    /// model verbal-component spell failure or audio-based perception, so
    /// the in-combat impact is mostly cosmetic — but the flag is here for
    /// spells like Blindness/Deafness so they can apply something.
    Deafened,
    /// Marked by Hunter's Mark — the marker (concentrating caster) deals
    /// an extra 1d6 weapon damage to this target. Tracked as a condition
    /// so dropping concentration cleans it up automatically.
    HuntersMarked,
    /// Mage Armor active — base AC becomes 13 + DEX modifier (we model as
    /// a flat AC boost via `condition_ac_bonus`). Lasts 8 hours; we just
    /// give it a long Rounds timer.
    MageArmored,
    /// Mocked by Vicious Mockery — disadvantage on the next attack roll
    /// before the end of the target's next turn. Short timer
    /// (`UntilStartOfNextTurn`) clears the debuff after the holder takes
    /// their turn (the disadvantage applies to attacks they make while
    /// the condition is up).
    Mocked,
    /// Heroism active — immune to Frightened and gaining temp HP each
    /// round from the spell's caster. Tracked as a condition so it
    /// clears cleanly on concentration drop.
    Heroic,
    /// Cannot cast spells / take reactions until the start of their next
    /// turn (Shocking Grasp's rider on a hit against a creature wearing
    /// metal armor — we model the simpler "no reaction" clause). Clears
    /// at start of own next turn.
    NoReaction,
    /// Asleep (5e Sleep spell). Same mechanical effect as Unconscious
    /// (prone, can't act, attacks against have advantage with auto-crit on
    /// melee hit), but distinguished from natural unconsciousness so it
    /// can be removed by taking damage or being shaken awake (any non-zero
    /// damage we receive while Asleep wakes us; modeled by stripping the
    /// condition on the damage-application path).
    Asleep,
    /// Surrounded by shimmering duplicates (5e Mirror Image). The holder
    /// has a small pool of decoys (we track count via `mirror_images()`
    /// on the actor); each incoming attack against them has a chance to
    /// hit a duplicate instead, popping one. Drops to 0 ends the spell.
    /// Doesn't require concentration.
    MirroredImages,
    /// Protected by Protection from Evil and Good. Aberrations, celestials,
    /// elementals, fey, fiends, and undead have disadvantage on attacks
    /// against this target. We approximate by giving disadvantage to *any*
    /// attacker that is undead-flavored (i.e. has the Poisoned condition
    /// immunity that mortal humanoids lack) — close enough for our pool of
    /// fiends / undead / etc.
    Warded,
    /// Hexed by the warlock Hex spell. The hex's caster deals +1d6 necrotic
    /// on weapon attacks against this target. Tracked as a condition so
    /// dropping concentration cleans it up automatically — symmetric with
    /// Hunter's Mark.
    Hexed,
    /// Blurred (5e Blur spell, concentration). Attacks against the holder
    /// have disadvantage unless the attacker can ignore the effect (no
    /// modeling of blindsight overrides — kept simple). Cleared when the
    /// caster drops concentration.
    Blurred,
    /// Stuck fast (5e Mimic Adhesive on hit). Functionally similar to
    /// Grappled but the catcher is the mimic itself; we reuse the
    /// movement-zero mechanic and keep this distinct so the log makes
    /// the cause obvious.
    Adhered,
    /// Hasted (5e Haste spell, concentration). +2 AC, advantage on DEX
    /// saves, doubled walking speed. We don't model the extra-action
    /// rider (action economy stays one Action per turn) — the AC + DEX
    /// save half is the load-bearing part for survivability and the
    /// doubled speed lets the holder reposition aggressively. Cleared
    /// when the caster's concentration ends.
    Hasted,
    /// Slowed (5e Slow spell). Halved walking speed, -2 AC, -2 DEX
    /// saves. The 5e spell also halves the holder's action economy
    /// (no reactions, can only cast a 1-action spell *or* attack);
    /// we model the static half — AC + DEX hit + movement — and skip
    /// the action-economy clause to avoid surprising the AI. Tracked
    /// as a condition so it clears cleanly on concentration drop.
    Slowed,
    /// Warded by Death Ward — the next time the holder would drop to
    /// 0 HP, they instead drop to 1 HP and the condition burns off.
    /// Damage that *would* kill outright (massive damage > max HP at 0)
    /// is also absorbed by the ward. Implemented in
    /// `ActorInstance::take_damage` so any damage path benefits — the
    /// ward intercepts before death-save / Dead transitions, then
    /// removes itself so a second killing blow lands as normal.
    DeathWarded,
    /// Petrified — turned to inanimate stone (5e Cockatrice / Flesh to
    /// Stone). Movement zero, action economy blocked (same envelope as
    /// Stunned / Paralyzed). 5e also adds wide damage resistance and
    /// poison immunity, plus auto-fail STR/DEX saves; we model the
    /// auto-fail via the existing `auto_fail_save` switch (extended to
    /// recognize Petrified alongside Paralyzed / Stunned).
    Petrified,
    /// Sanctuary (5e Sanctuary spell, level-1 abjuration, bonus action).
    /// Any creature targeting the warded actor with an attack or harmful
    /// spell must succeed on a WIS save vs the caster's spell DC or the
    /// effect fails (and the attacker can't target the warded for the
    /// rest of the turn). We model this by gating hostile actions inside
    /// `Action::validate_input`: the attacker rolls a one-shot WIS save
    /// at attack-roll time; on fail, the action silently no-ops. The
    /// warded actor loses the buff the moment they themselves attack or
    /// cast a damaging spell — the engine clears the condition on the
    /// holder's first hostile action.
    Sanctuary,
    /// Fire Shield — warm version (5e Fire Shield spell, level-4
    /// evocation). The holder gains resistance to cold damage (we
    /// approximate with the generic `DamageResistant` model — see
    /// `Stoneskin`). Any creature within reach that hits the holder
    /// takes 2d8 fire damage in retaliation (handled in
    /// `EncounterInstance::resolve_attack` via the fire-shield reflect
    /// hook). Concentration-free — fire shields are short-duration
    /// auto-clear via the Rounds timer.
    FireShielded,
    /// Daylit (5e Daylight spell, level-3 evocation). The actor sits
    /// inside a 60-foot sphere of bright light. We model this as a
    /// passive buff that gives undead / fiends (proxied by Necrotic /
    /// Poison immunity) disadvantage on attacks targeting the holder —
    /// mirroring `Warded` (Protection from Evil and Good) but for an
    /// area-buff rather than a single-target buff. Lasts for the spell's
    /// duration (1 hour RAW; we cap to a long Rounds timer).
    Daylit,
    /// Spike Growth (5e level-2 transmutation, concentration). The target
    /// is standing in spiked terrain — any movement they make this turn
    /// will deal 2d4 piercing damage per 5ft moved. We approximate by
    /// tagging the holder with the condition; the damage rider lives on
    /// the `MoveActor` apply path which reads this flag and bills the
    /// caster's spike damage once per step.
    Spiked,
    /// Raging (5e Barbarian feature). +2 melee damage on STR-based attacks,
    /// resistance to bludgeoning / piercing / slashing damage (we use the
    /// generic `DamageResistant` model in parallel), and advantage on STR
    /// checks / saves. Active for 10 rounds (an approximation of the
    /// 5e 1-minute duration). Bonus action to enter; ends early if the
    /// barbarian falls unconscious or doesn't attack / take damage on
    /// a round — we just let the timer run for simplicity.
    Raging,
    /// Polymorphed (5e level-4 transmutation, concentration). The target
    /// is transformed into a beast form — we model only the load-bearing
    /// mechanical change: their max HP is replaced by a buff pool (we
    /// approximate with +25 max HP, plus filling current HP to that cap)
    /// while the condition is up. On drop, max HP returns to base. The
    /// transformation also halts spellcasting (we don't enforce that
    /// gate). Cleared on concentration drop.
    Polymorphed,
    /// Globe of Invulnerability (5e level-6 abjuration, concentration).
    /// The holder gains immunity to damaging spells of level 5 or lower.
    /// We approximate by giving the holder a flat damage-resistant buff
    /// (halving incoming damage) — distinct from the `DamageResistant`
    /// condition so the two stack cleanly (Stoneskin + Globe halves
    /// damage twice). Concentration-bound on the caster.
    Globed,
    /// Lifted (5e Telekinesis spell, level 5, concentration). Target is
    /// suspended in the air. Mechanically: zero movement (we add this to
    /// `zeros_movement`), disadvantage on attacks made by the lifted
    /// creature (helpless dangling), and the caster can re-position them
    /// each round. We model only the movement-zero half for simplicity.
    Lifted,
    /// Booming-Blade-marked (5e cantrip rider). The blade caster touched
    /// this target on a hit; if they voluntarily move before the start of
    /// the caster's next turn, they take an extra 1d8 thunder damage. We
    /// track this as a tick-down condition on the target; the
    /// movement-damage rider lives on the `MoveActor` apply path (next
    /// to the Spike Growth rider) and consumes the mark on trigger.
    BoomingBladeMarked,
    /// Crusader's Mantle aura (5e level-3 paladin, concentration). The
    /// holder (an ally of the caster, including the caster themselves)
    /// rolls +1d4 radiant on every weapon attack hit. We layer this on
    /// `resolve_attack` as a small radiant rider, mirroring Hunter's
    /// Mark / Hex but typed Radiant. Dispelled when the caster drops
    /// concentration.
    CrusadersMantled,
    /// Mind-Whipped (5e Tasha's Mind Whip, level-2). On a failed INT save
    /// the target takes 3d6 psychic and loses one of their action /
    /// bonus-action / reaction on their next turn. We model the
    /// reaction-loss half via the existing `NoReaction` rider; the action
    /// / bonus-action loss is folded in by zeroing those resources at the
    /// start of the holder's next turn (we tag this condition and
    /// `reset_for_new_round` consumes it). Cleared on tick.
    MindWhipped,
    /// Time Stopped (5e Time Stop, level-9). The caster is given an extra
    /// Action and Bonus Action immediately on cast — we collapse the
    /// 5e "1d4+1 extra turns" clause into a single burst of action
    /// economy that resolves this turn. The condition is a marker only
    /// (it tags the caster so re-cast is detectable / dispel-prunable);
    /// no per-tick mechanics ride it.
    TimeStopped,
    /// Caged by Forcecage (5e level-7, CHA save). The target is sealed in
    /// an impassable cage of force. Mechanically: zero movement (joins
    /// `zeros_movement`). The 5e cage also blocks teleportation and
    /// passwall — neither is modeled here. Lasts the spell's duration
    /// (1 hour RAW; capped here to a long Rounds timer).
    Caged,
    /// Surrounded by a Crown of Stars (5e level-7 evocation). The caster
    /// has 7 motes of radiant light orbiting them; using a Bonus Action
    /// to fling one at a target deals 4d12 radiant. We collapse the
    /// "7 charges over the duration" RAW into a flat damage-rider buff:
    /// the holder rolls +1d8 radiant on every weapon attack while the
    /// crown is up. Mirrors Crusader's Mantle but is self-only and
    /// concentration-free (a flat Rounds timer suffices).
    CrownOfStars,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Frightened => "frightened",
            Condition::Restrained => "restrained",
            Condition::Blinded => "blinded",
            Condition::Incapacitated => "incapacitated",
            Condition::Charmed => "charmed",
            Condition::Paralyzed => "paralyzed",
            Condition::Invisible => "invisible",
            Condition::Dodging => "dodging",
            Condition::Grappled => "grappled",
            Condition::Blessed => "blessed",
            Condition::ShieldOfFaith => "shield of faith",
            Condition::Shielded => "shielded",
            Condition::DamageResistant => "damage resistant",
            Condition::Outlined => "outlined",
            Condition::GuidingBoltLit => "marked by guiding bolt",
            Condition::Helped => "helped",
            Condition::Hidden => "hidden",
            Condition::Burning => "burning",
            Condition::Disengaging => "disengaging",
            Condition::Unconscious => "unconscious",
            Condition::Baned => "baned",
            Condition::Deafened => "deafened",
            Condition::HuntersMarked => "marked by hunter's mark",
            Condition::MageArmored => "mage armored",
            Condition::Mocked => "mocked",
            Condition::Heroic => "heroic",
            Condition::NoReaction => "shocked",
            Condition::Asleep => "asleep",
            Condition::MirroredImages => "mirror imaged",
            Condition::Warded => "warded",
            Condition::Hexed => "hexed",
            Condition::Blurred => "blurred",
            Condition::Adhered => "stuck",
            Condition::Hasted => "hasted",
            Condition::Slowed => "slowed",
            Condition::DeathWarded => "warded against death",
            Condition::Petrified => "petrified",
            Condition::Sanctuary => "sanctified",
            Condition::FireShielded => "fire shielded",
            Condition::Daylit => "lit by daylight",
            Condition::Spiked => "in spiked growth",
            Condition::Raging => "raging",
            Condition::Polymorphed => "polymorphed",
            Condition::Globed => "globed in invulnerability",
            Condition::Lifted => "lifted by telekinesis",
            Condition::BoomingBladeMarked => "thunder-marked",
            Condition::CrusadersMantled => "crusader's mantled",
            Condition::MindWhipped => "mind-whipped",
            Condition::TimeStopped => "time-stopped",
            Condition::Caged => "caged in force",
            Condition::CrownOfStars => "haloed by stars",
        }
    }

    /// True if this condition completely blocks Action / BonusAction /
    /// Reaction usage (5e's Incapacitated clause). Stunned and Paralyzed
    /// inherit this clause.
    pub fn blocks_action_economy(&self) -> bool {
        matches!(
            self,
            Condition::Stunned
                | Condition::Incapacitated
                | Condition::Paralyzed
                | Condition::Unconscious
                | Condition::Asleep
                | Condition::Petrified
        )
    }

    /// True if this condition is a beneficial buff that Dispel Magic /
    /// similar "end one effect" spells should target. Used by the
    /// Dispel Magic side-effect when the target isn't concentrating —
    /// the spell falls back to stripping one helpful condition rather
    /// than failing silently.
    pub fn is_dispellable_buff(&self) -> bool {
        matches!(
            self,
            Condition::Blessed
                | Condition::ShieldOfFaith
                | Condition::MageArmored
                | Condition::Heroic
                | Condition::Hasted
                | Condition::Hidden
                | Condition::Invisible
                | Condition::DamageResistant
                | Condition::MirroredImages
                | Condition::Blurred
                | Condition::DeathWarded
                | Condition::Helped
                | Condition::Sanctuary
                | Condition::FireShielded
                | Condition::Daylit
                | Condition::Raging
                | Condition::Polymorphed
                | Condition::Globed
                | Condition::CrusadersMantled
                | Condition::CrownOfStars
                | Condition::TimeStopped
        )
    }

    /// True if this condition zeros out movement. Read by
    /// `ActorInstance::remaining_movement` to gate motion-blocking
    /// conditions in one place.
    pub fn zeros_movement(&self) -> bool {
        // Note: Prone is NOT in this list. RAW: prone halves movement
        // (you crawl). We don't yet model the half-speed reduction;
        // movement just costs the same. But blocking it entirely creates
        // a catch-22 — standing up pays in movement.
        matches!(
            self,
            Condition::Stunned
                | Condition::Restrained
                | Condition::Grappled
                | Condition::Paralyzed
                | Condition::Unconscious
                | Condition::Asleep
                | Condition::Adhered
                | Condition::Petrified
                | Condition::Lifted
                | Condition::Caged
        )
    }
}

/// How long a condition application persists. `Permanent` requires an
/// explicit removal (e.g. Stand-up clears Prone, Lesser Restoration clears
/// Poisoned). `Rounds(n)` ticks down by 1 every time the initiative queue
/// wraps; the condition is removed when the timer hits 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionTimer {
    Permanent,
    Rounds(u32),
    /// Lasts until the start of the holder's next turn — used for Dodge.
    /// The engine clears these at the start of an actor's turn.
    UntilStartOfNextTurn,
}

