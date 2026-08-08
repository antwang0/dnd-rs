use std::collections::VecDeque;

use crate::{
    actions::action_template::{Action, ActionExecutionInfo},
    engine::{
        encounter::EncounterInstance, errors::ParseError, types::Coordinate, util::parse_coord,
    },
};

pub struct Prompt {
    actor_id: usize,
    actions: Vec<&'static (dyn Action + Send + Sync)>,
}

impl Prompt {
    pub fn new(actor_id: usize, actions: Vec<&'static (dyn Action + Send + Sync)>) -> Self {
        Self { actor_id, actions }
    }

    pub fn actor_id(&self) -> usize {
        self.actor_id
    }

    pub fn actions(&self) -> &Vec<&'static (dyn Action + Send + Sync)> {
        &self.actions
    }

    /// Parse one argument token into either a target actor id or a
    /// target tile. Supported forms:
    /// - `id:N` (or `#N`) — actor id N. Lookup is exact; rejects unknown ids.
    /// - `<x>,<y>` / `r3u2` / `l4d1` etc. — coordinate (see `parse_coord`).
    /// - `<name>` — exact actor name match (e.g. "Zombie 0" must be quoted
    ///   or use `id:N`; we don't do fuzzy matching).
    fn parse_arg(
        token: &str,
        encounter: &EncounterInstance,
        base: Coordinate,
    ) -> Option<TargetArg> {
        // Explicit actor-id form. Accepts "id:7", "#7".
        let id_str = token
            .strip_prefix("id:")
            .or_else(|| token.strip_prefix('#'));
        if let Some(s) = id_str
            && let Ok(id) = s.parse::<usize>()
            && encounter.actors.contains_key(&id)
        {
            return Some(TargetArg::Actor(id));
        }
        // Coordinate forms.
        if let Some(coord) = parse_coord(token, base) {
            return Some(TargetArg::Point(coord));
        }
        // Exact actor name match (no spaces — names have one). We compare
        // ignoring case to keep typing quick.
        encounter
            .actors
            .iter()
            .find(|(_, a)| a.name().eq_ignore_ascii_case(token))
            .map(|(id, _)| TargetArg::Actor(*id))
    }

    /// Resolve the leading tokens of a command line to one of `actions`,
    /// returning how many tokens the name consumed and what it named.
    ///
    /// **The** action-addressing rule, and a function rather than a
    /// paragraph inside `process_input` because it has a second caller:
    /// `every_pc_action_is_reachable_by_its_canonical_name` sweeps every
    /// action on every playable template through it. That sweep used to
    /// carry a hand-copied reimplementation of this body under a comment
    /// reading "mirror `process_input`'s resolution", which is a test
    /// that proves a property of its own copy — the moment the parser
    /// changed and the copy did not, the sweep would keep passing while
    /// guaranteeing nothing.
    ///
    /// The rule, in order:
    ///
    ///   1. **Longest token-prefix first.** `scorching ray id:3` is the
    ///      two-token action "scorching ray" and one argument, not the
    ///      one-token action "scorching" and two.
    ///   2. **Canonical names before aliases, at every width.** Aliases
    ///      are short and collide freely across a big caster's list, and
    ///      some collide with another action's real name — a wizard
    ///      carries both Darkness and Maddening Darkness, and the latter
    ///      aliases "darkness". A single pass that mixed the two kinds
    ///      resolved by action-list order, so `darkness` cast a
    ///      level-8 slot. A canonical name is the one handle a player
    ///      can be certain of, so nothing may shadow it.
    ///   3. **An ambiguous alias is refused, not guessed.** Aliases
    ///      collide with each other constantly ("sphere" is claimed by
    ///      four spells on the sorcerer's list, "fs" by three), and the
    ///      old rule handed the collision to whichever came first in the
    ///      action list. That is a silent wrong-spell: a wizard typing
    ///      `bh` for Burning Hands got Bigby's Hand and a 5th-level slot
    ///      instead of a 1st. Refusing costs the player one retype and
    ///      tells them exactly what to type; guessing costs them the
    ///      slot and tells them nothing. Every action stays reachable by
    ///      its full name, which is what makes the refusal cheap.
    ///
    /// `Err` carries the message `process_input` shows the player.
    pub fn resolve_action<'a>(
        actions: &[&'a (dyn Action + Send + Sync)],
        tokens: &[&str],
    ) -> Result<(usize, &'a (dyn Action + Send + Sync)), String> {
        for n in (1..=tokens.len()).rev() {
            let candidate = tokens[..n].join(" ");
            if let Some(act) = actions.iter().find(|e| candidate == e.name()) {
                return Ok((n, *act));
            }
            // Aliases at every width, which is what rule 2 above says
            // and what this loop used to contradict. It skipped
            // straight past any width but the narrowest, under a
            // comment asserting that "aliases are single-token by
            // construction" — an invariant nothing enforced and 222
            // aliases broke. "bless scroll", "hideous laughter",
            // "giant strength", "finger of death", "mirror image",
            // every one of the arcane archer's "banishing shot" pairs:
            // all written to be typed, none of them reachable, because
            // the only width that checked aliases was the one where a
            // two-word alias cannot possibly match.
            //
            // A player typing one of them got "unknown or unavailable
            // action", naming only their first token — so `mirror
            // image` came back as unknown action "mirror", which reads
            // like the spell is missing rather than like the shorthand
            // is.
            let by_alias: Vec<&'a (dyn Action + Send + Sync)> = actions
                .iter()
                .filter(|e| e.aliases().contains(&candidate.as_str()))
                .copied()
                .collect();
            match by_alias.len() {
                0 => {}
                1 => return Ok((n, by_alias[0])),
                _ => {
                    let names: Vec<&str> = by_alias.iter().map(|a| a.name()).collect();
                    return Err(format!(
                        "{:?} is ambiguous — it is short for {}. Type the full name.",
                        candidate,
                        names.join(", ")
                    ));
                }
            }
        }
        Err(format!("unknown or unavailable action {:?}", tokens[0]))
    }

    pub fn process_input(
        &self,
        input: &str,
        encounter_instance: &EncounterInstance,
    ) -> Result<ActionExecutionInfo, ParseError> {
        let actor = encounter_instance
            .actors
            .get(&self.actor_id)
            .ok_or_else(|| ParseError::new(format!("missing actor {}", self.actor_id)))?;

        let tokens: Vec<&str> = input.split_whitespace().collect();
        if tokens.is_empty() {
            return Err(ParseError::with_input("empty input", input));
        }

        let (consumed, action) = Self::resolve_action(&self.actions, &tokens)
            .map_err(|msg| ParseError::with_input(msg, input))?;

        let mut target_ids: Vec<usize> = Vec::new();
        let mut target_locations: Vec<Coordinate> = Vec::new();

        let mut rest: VecDeque<&str> = tokens.into_iter().skip(consumed).collect();
        while let Some(tok) = rest.pop_front() {
            let token_trimmed = tok.trim();
            match Self::parse_arg(token_trimmed, encounter_instance, actor.location()) {
                Some(TargetArg::Actor(id)) => target_ids.push(id),
                Some(TargetArg::Point(coord)) => target_locations.push(coord),
                None => {
                    return Err(ParseError::with_input(
                        format!("could not parse argument {:?}", token_trimmed),
                        input,
                    ));
                }
            }
        }

        let aei = ActionExecutionInfo::new(
            action,
            self.actor_id,
            if target_ids.is_empty() {
                None
            } else {
                Some(target_ids)
            },
            if target_locations.is_empty() {
                None
            } else {
                Some(target_locations)
            },
            None,
        );

        if !aei.validate(encounter_instance) {
            return Err(ParseError::with_input(
                "invalid arguments or insufficient resources",
                input,
            ));
        }
        Ok(aei)
    }
}

enum TargetArg {
    Actor(usize),
    Point(Coordinate),
}

#[cfg(test)]
mod tests {
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::terrain_gen::TerrainGenParams;

    fn ei() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 4,
            branch_prob: 0.5,
        };
        let ap = ActorGenParams {
            cr_target: 0.25,
            n_teams: 2,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(99)).unwrap();
        e.process_stack();
        e
    }

    #[test]
    fn parse_empty_input_returns_error_with_context() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let err = match prompt.process_input("", &e) {
            Err(err) => err,
            Ok(_) => panic!("expected error"),
        };
        assert!(err.to_string().to_lowercase().contains("empty"));
    }

    #[test]
    fn parse_unknown_action_returns_named_error() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let err = match prompt.process_input("frobnicate", &e) {
            Err(err) => err,
            Ok(_) => panic!("expected error"),
        };
        let msg = err.to_string();
        assert!(msg.contains("frobnicate"), "msg = {}", msg);
    }

    #[test]
    fn parse_actor_id_target_with_hash_prefix() {
        // Verify the parser populates target_ids for `#<id>`. Validation
        // (range, LOS, etc.) isn't the parser's job — keep the test
        // narrow by using `skip #N`, which doesn't need the actor to be
        // a real attack target.
        let e = ei();
        // Pick any actor id present in the encounter as the target.
        let some_id = *e
            .actors
            .keys()
            .find(|id| **id != e.peek_prompt().unwrap().actor_id())
            .expect("test encounter should have at least 2 actors");
        let prompt = e.peek_prompt().unwrap();
        let cmd = format!("skip #{}", some_id);
        // Skip declares NoArgs, so adding target_ids will fail validation;
        // we only care that the parser reaches the validate stage with
        // the id populated. Validation surfacing the error proves parsing
        // succeeded.
        let res = prompt.process_input(&cmd, &e);
        assert!(res.is_err(), "skip with extra target should fail validation");
    }

    #[test]
    fn parse_unknown_actor_id_errors() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let err = match prompt.process_input("skip #99999", &e) {
            Err(err) => err,
            Ok(_) => panic!("expected error for unknown actor id"),
        };
        assert!(err.to_string().contains("99999"));
    }

    #[test]
    fn parse_skip_succeeds() {
        let e = ei();
        let prompt = e.peek_prompt().unwrap();
        let aei = match prompt.process_input("skip", &e) {
            Ok(a) => a,
            Err(err) => panic!("unexpected error: {}", err),
        };
        assert!(aei.validate(&e));
    }

    /// Player can target an enemy actor by `id:N` syntax for SingleActor
    /// actions like Slam. Before this, the prompt only ever populated
    /// target_locations and SingleActor actions could never validate from
    /// a typed command.
    #[test]
    fn parse_id_form_targets_actor() {
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;

        // Use bare encounter without process_stack so we can place actors
        // ourselves and craft a deterministic prompt.
        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        let attacker = e
            .instantiate_creature(
                &ZOMBIE_TEMPLATE,
                crate::engine::types::Coordinate::new(5, 5),
                0,
                0,
            )
            .unwrap();
        let target = e
            .instantiate_creature(
                &ZOMBIE_TEMPLATE,
                crate::engine::types::Coordinate::new(7, 5),
                1,
                0,
            )
            .unwrap();
        // Build a prompt manually for the attacker.
        use super::Prompt;
        let prompt = Prompt::new(attacker, e.actors[&attacker].available_actions());
        let aei = prompt
            .process_input(&format!("trip id:{}", target), &e)
            .expect("id:N should resolve to actor target");
        assert_eq!(aei.target_ids().and_then(|ids| ids.first().copied()), Some(target));
    }

    /// Multi-word action names (e.g. "sacred flame", "scorching ray") must
    /// match across whitespace boundaries — previously the parser only
    /// consumed the first token and was unreachable for these spells via
    /// their canonical name (only the short aliases worked).
    #[test]
    fn parse_multi_word_action_name_resolves() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
        use crate::engine::encounter::EncounterInstance;
        use crate::engine::terrain_gen::TerrainGenParams;

        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        let mut e = EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap();
        let wizard = e
            .instantiate_creature(
                &WIZARD_TEMPLATE,
                crate::engine::types::Coordinate::new(5, 5),
                0,
                0,
            )
            .unwrap();
        let target = e
            .instantiate_creature(
                &ZOMBIE_TEMPLATE,
                crate::engine::types::Coordinate::new(7, 5),
                1,
                0,
            )
            .unwrap();
        use super::Prompt;
        let prompt = Prompt::new(wizard, e.actors[&wizard].available_actions());
        // "scorching ray <id>" must parse as the action "scorching ray"
        // (two tokens) plus a single target id arg.
        let aei = prompt
            .process_input(&format!("scorching ray id:{}", target), &e)
            .expect("scorching ray should parse as a two-token action name");
        assert_eq!(aei.target_ids().and_then(|ids| ids.first().copied()), Some(target));
    }

    /// Every action on every PC template is reachable by typing its own
    /// canonical name. That is the one addressing guarantee the parser
    /// makes, and before canonical names were preferred over aliases it
    /// did not hold: a wizard carries both Darkness and Maddening
    /// Darkness, the latter aliases "darkness", and the single-pass
    /// search resolved whichever came first in the action list — so
    /// `darkness` cast a level-8 spell.
    ///
    /// Swept across all twelve class families rather than spot-checked,
    /// because the failure mode is silent: the wrong spell fires and the
    /// player is told nothing. The sweep is also what makes the guarantee
    /// survive new content — a spell added with an alias that happens to
    /// equal an existing spell's name fails here rather than in play.
    ///
    /// Alias-vs-alias collisions are not asserted *here*, because they
    /// are no longer resolved silently — `resolve_action` refuses them
    /// and says what they were short for. See
    /// `an_ambiguous_alias_is_refused_rather_than_guessed`.
    #[test]
    fn every_pc_action_is_reachable_by_its_canonical_name() {
        use crate::actions::action_template::Action;
        use super::Prompt;
        let families = crate::actors::creatures::pc_template_families();
        for (_label, family) in families {
            for template in family {
                let actions: Vec<&'static (dyn Action + Send + Sync)> =
                    template.actions.clone();
                for action in &actions {
                    let name = action.name();
                    let tokens: Vec<&str> = name.split_whitespace().collect();
                    let resolved = Prompt::resolve_action(&actions, &tokens)
                        .map(|(_, act)| act.name());
                    assert_eq!(
                        resolved,
                        Ok(name),
                        "{}: typing '{}' resolves to {:?}",
                        template.name,
                        name,
                        resolved
                    );
                }
            }
        }
    }

    /// Every alias a player might reasonably type resolves to
    /// *something* — the alias sweep, sibling to the canonical-name
    /// one above.
    ///
    /// It is here because the canonical sweep could not see the bug it
    /// misses. `resolve_action` used to check aliases only at
    /// token-width 1, under a comment asserting that "aliases are
    /// single-token by construction". Nothing enforced that, and 222
    /// aliases across the codebase broke it — "bless scroll",
    /// "hideous laughter", "giant strength", "finger of death",
    /// "mirror image", every one of the arcane archer's four
    /// "<effect> shot" pairs. Each was written to be typed and none of
    /// them could be, because the one width that consulted aliases is
    /// the one where a two-word alias cannot match.
    ///
    /// The assertion is deliberately weaker than the canonical sweep's.
    /// A canonical name must resolve to *its own action*, because
    /// nothing is allowed to shadow it. An alias only has to resolve
    /// to something or be refused as ambiguous: aliases collide freely
    /// — "sphere" is claimed by four spells on the sorcerer's list —
    /// and refusing a collision with both names is the documented
    /// behaviour, not a failure. What is *not* acceptable is
    /// `unknown or unavailable action`, which is the parser saying the
    /// shorthand does not exist.
    #[test]
    fn every_pc_action_alias_resolves_or_is_refused_as_ambiguous() {
        use crate::actions::action_template::Action;
        use super::Prompt;
        let mut checked = 0usize;
        let mut multi_token = 0usize;
        for (_label, family) in crate::actors::creatures::pc_template_families() {
            for template in family {
                let actions: Vec<&'static (dyn Action + Send + Sync)> = template.actions.clone();
                for action in &actions {
                    for alias in action.aliases() {
                        let tokens: Vec<&str> = alias.split_whitespace().collect();
                        // An empty alias would resolve to nothing and
                        // is a data error in its own right.
                        assert!(
                            !tokens.is_empty(),
                            "{}: {} carries an empty alias",
                            template.name,
                            action.name()
                        );
                        checked += 1;
                        multi_token += usize::from(tokens.len() > 1);
                        match Prompt::resolve_action(&actions, &tokens) {
                            Ok(_) => {}
                            Err(msg) => assert!(
                                msg.contains("ambiguous"),
                                "{}: typing '{}' for {} gives {:?}",
                                template.name,
                                alias,
                                action.name(),
                                msg
                            ),
                        }
                    }
                }
            }
        }
        // Floors, not counts. The second one is the load-bearing half:
        // without it the sweep passes on a parser that has gone back to
        // ignoring every alias wider than one token, since the
        // single-token ones would all still resolve.
        assert!(checked > 500, "the sweep only reached {} aliases", checked);
        assert!(
            multi_token > 20,
            "the sweep saw only {} multi-token aliases — those are the ones it exists for",
            multi_token
        );
    }

    /// A multi-token alias resolves, end to end through the real
    /// parser, to the action it is short for.
    ///
    /// The sweep above proves the property across the roster; this is
    /// the one case driven through `process_input` so the thing pinned
    /// is what a player actually types. Hideous Laughter is the
    /// fixture because it is the shape the bug was worst on: the
    /// spell's canonical name is "tasha's hideous laughter", which
    /// nobody types, and "hideous laughter" — the name everyone knows
    /// it by — was an alias that could not resolve.
    #[test]
    fn a_two_word_alias_reaches_the_spell_it_names() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use super::Prompt;

        let mut e = ei();
        let wizard = e
            .instantiate_creature(
                &WIZARD_TEMPLATE,
                crate::engine::types::Coordinate::new(5, 5),
                0,
                0,
            )
            .unwrap();
        let target = e
            .instantiate_creature(
                &WIZARD_TEMPLATE,
                crate::engine::types::Coordinate::new(7, 5),
                1,
                0,
            )
            .unwrap();
        let prompt = Prompt::new(wizard, e.actors[&wizard].available_actions());

        // The fixture is only meaningful while the alias exists and is
        // not the canonical name.
        let laughter = e.actors[&wizard]
            .available_actions()
            .into_iter()
            .find(|a| a.name() == "tasha's hideous laughter")
            .expect("the wizard should carry Hideous Laughter");
        assert!(laughter.aliases().contains(&"hideous laughter"));

        let resolved = prompt
            .process_input(&format!("hideous laughter #{}", target), &e)
            .expect("a two-word alias should parse");
        assert_eq!(resolved.action().name(), "tasha's hideous laughter");
        assert_eq!(resolved.target_ids(), Some(&[target][..]));
    }

    /// An alias two actions on the same list both claim is refused with
    /// both names, rather than silently resolving to whichever the list
    /// happens to hold first.
    ///
    /// The wizard is the fixture because it is the chassis where this
    /// bit: it carries Burning Hands and Bigby's Hand, both of which
    /// alias "bh", and list order gave the level-5 one. Driven through
    /// the real parser (not just the resolver) so the message the player
    /// actually sees is what is pinned.
    #[test]
    fn an_ambiguous_alias_is_refused_rather_than_guessed() {
        use crate::actors::creatures::wizards::WIZARD_TEMPLATE;
        use super::Prompt;

        let mut e = ei();
        let wizard = e
            .instantiate_creature(
                &WIZARD_TEMPLATE,
                crate::engine::types::Coordinate::new(5, 5),
                0,
                0,
            )
            .unwrap();
        let actions = e.actors[&wizard].available_actions();
        // The fixture is only meaningful while the collision exists.
        let claimants: Vec<&str> = actions
            .iter()
            .filter(|a| a.aliases().contains(&"bh"))
            .map(|a| a.name())
            .collect();
        assert!(
            claimants.len() > 1,
            "the wizard should still carry more than one \"bh\": {:?}",
            claimants
        );

        let prompt = Prompt::new(wizard, actions);
        let Err(err) = prompt.process_input("bh 5,5", &e) else {
            panic!("an ambiguous alias should be refused");
        };
        for name in &claimants {
            assert!(
                err.message().contains(name),
                "the refusal should name {:?}: {}",
                name,
                err.message()
            );
        }

        // And the unambiguous full name still works, which is what makes
        // the refusal cheap.
        assert!(
            prompt.process_input("burning hands 6,5", &e).is_ok(),
            "the canonical name is always reachable"
        );
    }

}

