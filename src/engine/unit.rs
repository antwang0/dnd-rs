pub enum AbilityScoreType {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma
}

pub trait Unit {
    fn ability_score(&self, ast: AbilityScoreType) -> u8;

    fn armor_class(&self) -> u8;

    fn hitpoints(&self) -> u8;

    fn max_hitpoints(&self) -> u8;
    // TODO: bonus hitpoints?

    fn speed(&self) -> f32;

    fn remaining_movement(&self) -> f32;
}

pub trait Action {
    fn apply_action(&self, caster: &mut dyn Unit, targets: &mut Vec<&mut dyn Unit>);

    fn check_legal(&self, caster: &mut dyn Unit, targets: &mut Vec<&mut dyn Unit>) -> bool;
}