//! 5e **Underwater Combat** (PHB p.198) — the three clauses that change
//! how an attack rolls when the swinger is in the water, and the two
//! weapon cohorts they read.
//!
//! RAW, in full:
//!
//! > When making a **melee weapon attack**, a creature that doesn't have
//! > a swimming speed (either natural or granted by magic) has
//! > disadvantage on the attack roll unless the weapon is a dagger,
//! > javelin, shortsword, spear, or trident.
//! >
//! > A **ranged weapon attack** automatically misses a target beyond the
//! > weapon's normal range. Even against a target within normal range,
//! > the attack roll has disadvantage unless the weapon is a crossbow, a
//! > net, or a weapon that is thrown like a javelin (including a spear,
//! > trident, or dart).
//! >
//! > Creatures and objects that are **fully immersed** in water have
//! > resistance to fire damage.
//!
//! Three details of that wording are load-bearing and are the reason
//! this module exists rather than two `if`s at the attack site:
//!
//!   1. **The melee clause has a swimming-speed escape and the ranged
//!      clause does not.** A merfolk with a trident swings at no
//!      penalty; the same merfolk with a longbow still shoots at
//!      disadvantage. Only Freedom of Movement's "being underwater
//!      imposes no penalties on the target's movement or attacks" waives
//!      both — which is why `UnderwaterVerdict` is computed from a
//!      *pair* of predicates rather than one.
//!
//!   2. **The two weapon lists are different lists**, and they overlap
//!      in three entries. A spear is fine either way; a shortsword is
//!      fine only in melee; a crossbow is fine only at range. Keeping
//!      them as two cohorts rather than one "underwater-legal" flag on
//!      the weapon is what lets the same javelin answer both questions
//!      correctly and a shortsword answer them differently.
//!
//!   3. **It says weapon attack, not attack.** A Fire Bolt cast
//!      underwater is unaffected — see `UnderwaterVerdict::for_attack`'s
//!      `is_spell` gate.
//!
//! The fire-resistance clause is not here: it is a damage-pipeline rule
//! rather than an attack-roll one, and rides the environmental-halving
//! cohort in `crate::engine::side_effects` next to Aura of Warding.

/// The five melee weapons 5e names as keeping their edge underwater —
/// "a dagger, javelin, shortsword, spear, or trident".
///
/// The common thread is that all five are *thrusting* weapons: a stab
/// pushes a narrow point through water, where a swing has to drag a
/// broad edge through it. That is the reason the list is what it is,
/// and the reason a new weapon belongs here or doesn't is worth asking
/// about its shape rather than its damage type — the engine's rapier
/// and pike are both stabs RAW never got around to listing, and both
/// are deliberately absent, because the list is a closed RAW enumeration
/// and not a physics model.
///
/// Matched against the action's own name by `melee_keeps_edge`, so a
/// bespoke printing of one of these — "rogue shortsword", "+1 trident",
/// "sahuagin spear" — resolves without needing its own row.
pub const UNDERWATER_MELEE_WEAPONS: &[&str] =
    &["dagger", "javelin", "shortsword", "spear", "trident"];

/// The ranged weapons 5e exempts from the underwater disadvantage
/// clause — "a crossbow, a net, or a weapon that is thrown like a
/// javelin (including a spear, trident, or dart)".
///
/// Note what is *not* here: the bow. A bowstring will not draw
/// underwater, which is exactly the case the rule is written to
/// penalise, and it is the single most common ranged weapon on the
/// roster. A shortbow / longbow shot from the water is at disadvantage
/// no matter who fires it.
///
/// The exemption is from the *disadvantage* clause only. Nothing on
/// this list escapes the automatic miss past normal range — RAW applies
/// that sentence to every ranged weapon attack without qualification,
/// and a crossbow bolt loses its energy to the water as fast as an
/// arrow does.
pub const UNDERWATER_RANGED_WEAPONS: &[&str] =
    &["crossbow", "net", "javelin", "spear", "trident", "dart"];

/// True if `action_name` names one of the five melee weapons that keep
/// their edge underwater.
pub fn melee_keeps_edge(action_name: &str) -> bool {
    names_weapon(action_name, UNDERWATER_MELEE_WEAPONS)
}

/// True if `action_name` names a ranged weapon that carries underwater
/// — a crossbow, a net, or something thrown like a javelin.
pub fn ranged_carries(action_name: &str) -> bool {
    names_weapon(action_name, UNDERWATER_RANGED_WEAPONS)
}

/// Whole-word, case-insensitive search for any of `cohort` inside
/// `action_name`.
///
/// Word-boundary rather than substring, and the boundary is what makes
/// it safe. The engine names attacks descriptively and inconsistently —
/// "shortsword", "rogue shortsword", "shortsword (offhand)", "hand
/// crossbow", "heavy crossbow" — so a bare `==` would exempt one
/// printing of a weapon and tax its twin, which is precisely the class
/// of silent divergence a shared cohort is supposed to prevent. A bare
/// `contains` goes wrong in the other direction and worse: "spear"
/// falls inside "spearhead", "dart" inside "dartboard", and "net" —
/// the shortest needle on either cohort, and so the most dangerous —
/// inside "bayo**net**", "mag**net**ic" and "**net**herworld". Every
/// one of those would have been silently exempted from the rule.
///
/// A boundary is any non-alphanumeric character, so hyphens, slashes
/// and parentheses all split: "trident/net" resolves as both of its
/// words, and "cross-bow" resolves as neither — which is the right
/// answer, since the cohort lists "crossbow" and that name does not
/// contain it. Plurals do not match either, and shouldn't: an action
/// named "daggers" is a different attack from "dagger" and deserves
/// its own row rather than a prefix match that would also catch
/// "daggerfall".
fn names_weapon(action_name: &str, cohort: &[&str]) -> bool {
    let hay = action_name.to_ascii_lowercase();
    cohort.iter().any(|needle| {
        hay.match_indices(needle).any(|(at, _)| {
            let before_ok = hay[..at].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
            let after = at + needle.len();
            let after_ok = hay[after..].chars().next().is_none_or(|c| !c.is_alphanumeric());
            before_ok && after_ok
        })
    })
}

/// What 5e's Underwater Combat rules do to one particular swing.
///
/// Three outcomes rather than a bare `bool` because the ranged clause
/// has two distinct severities and they are not orderable as "more
/// disadvantage": past normal range the attack does not roll at all, it
/// misses. Collapsing that into `Disadvantage` would let a nat-20 land
/// a longbow shot across a lake that RAW says cannot connect, and
/// collapsing it into "out of range" at the targeting layer would hide
/// the shot from the player's action list entirely, which is worse —
/// the shot is legal, it just cannot hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnderwaterVerdict {
    /// The water has nothing to say about this attack — the swinger is
    /// dry, or holds Freedom of Movement, or the weapon is exempt, or
    /// it isn't a weapon attack at all.
    Unaffected,
    /// Roll at disadvantage.
    Disadvantage,
    /// A ranged weapon attack past its normal range: automatic miss.
    AutoMiss,
}

/// Everything `UnderwaterVerdict::for_attack` needs to know about one
/// swing.
///
/// A struct rather than seven positional arguments, five of which are
/// `bool`, because a run of same-typed parameters is a rule that can be
/// got wrong silently: transposing two of these compiles, passes most
/// tests, and produces a merfolk that shoots a bow at no penalty and
/// stabs at disadvantage. Named fields make each call site say which
/// clause it is answering.
pub struct AttackInWater<'a> {
    /// Is the attacker in the water at all? See
    /// `EncounterInstance::is_immersed`. Nothing below matters when this
    /// is false.
    pub immersed: bool,
    /// Does the attacker hold Freedom of Movement, whose "being
    /// underwater imposes no penalties on the target's movement or
    /// attacks" is the one blanket exemption RAW grants?
    pub waived: bool,
    /// Does the attacker have a swimming speed? Exempts the melee
    /// clause and *only* the melee clause.
    pub swims: bool,
    /// Every clause says "weapon attack", so a spell attack, a
    /// save-or-suck and a class feature are all untouched. Phrased
    /// positively, the way RAW phrases it, rather than as a negated
    /// `is_spell`: the rule is a carve-*in* for weapons, not a
    /// carve-out for magic, and the difference shows on anything that
    /// is neither.
    pub is_weapon_attack: bool,
    /// Melee or ranged — the two clauses are different rules with
    /// different escapes.
    pub is_melee: bool,
    /// The action's own name, which is what the two weapon cohorts are
    /// keyed on.
    pub weapon_name: &'a str,
    /// Is the target past the weapon's normal range? Only ever
    /// meaningful for a ranged attack; a melee weapon has no normal
    /// range and should pass `false`.
    pub beyond_normal_range: bool,
}

impl UnderwaterVerdict {
    /// Decide what the water does to one swing.
    pub fn for_attack(a: AttackInWater) -> UnderwaterVerdict {
        if !a.immersed || a.waived || !a.is_weapon_attack {
            return UnderwaterVerdict::Unaffected;
        }
        if a.is_melee {
            // The swimming-speed escape is the melee clause's alone.
            if a.swims || melee_keeps_edge(a.weapon_name) {
                return UnderwaterVerdict::Unaffected;
            }
            return UnderwaterVerdict::Disadvantage;
        }
        // Ranged. The range cut is checked first because it is
        // unconditional: no weapon on either cohort escapes it, and a
        // swimming speed does not help. A crossbow fired past its
        // normal range underwater misses exactly as a longbow does.
        if a.beyond_normal_range {
            return UnderwaterVerdict::AutoMiss;
        }
        if ranged_carries(a.weapon_name) {
            return UnderwaterVerdict::Unaffected;
        }
        UnderwaterVerdict::Disadvantage
    }

    /// Log-friendly reason, or `None` for `Unaffected`. Threaded into
    /// the attack line so a player can tell a swing taxed by the water
    /// from one taxed by a fog bank.
    pub fn note(self) -> Option<&'static str> {
        match self {
            UnderwaterVerdict::Unaffected => None,
            UnderwaterVerdict::Disadvantage => Some("underwater"),
            UnderwaterVerdict::AutoMiss => Some("underwater, past normal range"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_thrusting_weapons_keep_their_edge_and_nothing_else_does() {
        for w in UNDERWATER_MELEE_WEAPONS {
            assert!(melee_keeps_edge(w), "{w} is on the RAW melee list");
        }
        for w in ["longsword", "greataxe", "scimitar", "warhammer", "mace", "club"] {
            assert!(!melee_keeps_edge(w), "{w} is not on the RAW melee list");
        }
    }

    /// The two cohorts genuinely disagree, and the disagreement is the
    /// reason there are two of them. Pinned because collapsing them into
    /// one "underwater-legal weapon" flag is the obvious simplification
    /// and it is wrong in both directions at once.
    #[test]
    fn the_melee_and_ranged_cohorts_are_not_the_same_list() {
        assert!(melee_keeps_edge("shortsword") && !ranged_carries("shortsword"));
        assert!(ranged_carries("crossbow") && !melee_keeps_edge("crossbow"));
        // …and they do overlap, on exactly the three RAW names both
        // sentences list.
        for both in ["javelin", "spear", "trident"] {
            assert!(melee_keeps_edge(both) && ranged_carries(both), "{both}");
        }
    }

    /// A bespoke printing of a listed weapon resolves. This is the whole
    /// argument for matching on the name rather than on identity: the
    /// engine has several shortswords and no shared type between them.
    #[test]
    fn a_decorated_printing_of_a_listed_weapon_still_resolves() {
        for name in [
            "rogue shortsword",
            "Shortsword",
            "shortsword (offhand)",
            "+1 trident",
            "sahuagin spear",
        ] {
            assert!(melee_keeps_edge(name), "{name}");
        }
        for name in ["hand crossbow", "heavy crossbow", "Light Crossbow"] {
            assert!(ranged_carries(name), "{name}");
        }
    }

    /// The word boundary, from both sides. A substring match would
    /// exempt every one of these.
    #[test]
    fn a_weapon_name_that_merely_contains_a_listed_one_is_not_exempt() {
        assert!(!melee_keeps_edge("spearhead"));
        assert!(!melee_keeps_edge("bespear"));
        assert!(!melee_keeps_edge("daggers"));
        // "net" is the shortest needle on either cohort and the one a
        // substring match would have exempted half the bestiary with.
        for trap in ["bayonet", "magnetic", "netherworld", "netting"] {
            assert!(!ranged_carries(trap), "{trap}");
        }
        assert!(!ranged_carries("dartboard"));
    }

    /// A hyphen, slash or parenthesis is a boundary like any other —
    /// and a name that splits into words the cohort doesn't list still
    /// doesn't match.
    #[test]
    fn punctuation_splits_words_the_way_a_space_does() {
        assert!(melee_keeps_edge("trident/net"));
        assert!(ranged_carries("trident/net"));
        assert!(melee_keeps_edge("(spear)"));
        assert!(ranged_carries("crossbow, hand"));
        assert!(
            !ranged_carries("cross-bow"),
            "splitting on the hyphen leaves 'cross' and 'bow', neither of which is listed"
        );
    }

    /// A swing in the water, defaulting to the case every test below
    /// varies one field of: a lone attacker, no exemptions, a weapon
    /// attack, within range.
    fn swinging(weapon_name: &'static str) -> AttackInWater<'static> {
        AttackInWater {
            immersed: true,
            waived: false,
            swims: false,
            is_weapon_attack: true,
            is_melee: true,
            weapon_name,
            beyond_normal_range: false,
        }
    }

    /// The melee clause's swimming-speed escape, and the fact the ranged
    /// clause does not have one. This asymmetry is the single most
    /// misread sentence in the rule and the reason `AttackInWater`
    /// carries `swims` and `waived` as separate fields.
    #[test]
    fn a_swimming_speed_saves_the_swing_and_not_the_shot() {
        let melee = |swims| {
            UnderwaterVerdict::for_attack(AttackInWater {
                swims,
                ..swinging("longsword")
            })
        };
        assert_eq!(melee(false), UnderwaterVerdict::Disadvantage);
        assert_eq!(melee(true), UnderwaterVerdict::Unaffected);

        let ranged = |swims| {
            UnderwaterVerdict::for_attack(AttackInWater {
                swims,
                is_melee: false,
                ..swinging("longbow")
            })
        };
        assert_eq!(ranged(false), UnderwaterVerdict::Disadvantage);
        assert_eq!(
            ranged(true),
            UnderwaterVerdict::Disadvantage,
            "a swimming speed does not restring a bow"
        );
    }

    /// Freedom of Movement's blanket clause covers both halves where a
    /// swimming speed covers one.
    #[test]
    fn freedom_of_movement_waives_both_halves() {
        for is_melee in [true, false] {
            let weapon = if is_melee { "longsword" } else { "longbow" };
            assert_eq!(
                UnderwaterVerdict::for_attack(AttackInWater {
                    waived: true,
                    is_melee,
                    beyond_normal_range: true,
                    ..swinging(weapon)
                }),
                UnderwaterVerdict::Unaffected,
                "melee={is_melee}"
            );
        }
    }

    /// The range cut answers to nothing — not the exempt-weapon cohort,
    /// not a swimming speed.
    #[test]
    fn past_normal_range_nothing_saves_the_shot_but_freedom_of_movement() {
        let shot = |swims, weapon| {
            UnderwaterVerdict::for_attack(AttackInWater {
                swims,
                is_melee: false,
                beyond_normal_range: true,
                ..swinging(weapon)
            })
        };
        assert_eq!(shot(false, "longbow"), UnderwaterVerdict::AutoMiss);
        assert_eq!(shot(true, "heavy crossbow"), UnderwaterVerdict::AutoMiss);
    }

    /// A melee attack has no normal range, so the flag is inert on that
    /// half — a lance jabbed underwater is at disadvantage, never an
    /// automatic miss.
    #[test]
    fn the_range_cut_does_not_leak_onto_the_melee_half() {
        assert_eq!(
            UnderwaterVerdict::for_attack(AttackInWater {
                beyond_normal_range: true,
                ..swinging("lance")
            }),
            UnderwaterVerdict::Disadvantage
        );
    }

    /// "Weapon attack", throughout. A Fire Bolt is unbothered by the
    /// lake it is cast in.
    #[test]
    fn a_spell_attack_is_untouched_by_the_water() {
        assert_eq!(
            UnderwaterVerdict::for_attack(AttackInWater {
                is_weapon_attack: false,
                is_melee: false,
                beyond_normal_range: true,
                ..swinging("fire bolt")
            }),
            UnderwaterVerdict::Unaffected
        );
    }

    /// Dry land is dry land.
    #[test]
    fn nothing_happens_to_an_attacker_who_is_not_in_the_water() {
        assert_eq!(
            UnderwaterVerdict::for_attack(AttackInWater {
                immersed: false,
                is_melee: false,
                beyond_normal_range: true,
                ..swinging("longbow")
            }),
            UnderwaterVerdict::Unaffected
        );
    }
}
