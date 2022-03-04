use std::ops::RangeInclusive;

#[derive(Clone)]
pub enum Value {
    Single(u8),
    Range(RangeInclusive<u8>),
    Singles(Vec<u8>),
}

impl Value {
    pub fn in_range(&self, value: u8) -> bool {
        match self {
            Value::Single(single) => value == *single,
            Value::Range(range) => value < *range.end() && value > *range.start(),
            Value::Singles(singles) => singles.iter().any(|v| *v == value),
        }
    }
}

#[derive(Clone)]
pub struct Rule {
    pub survival_rule: Value,
    pub birth_rule: Value,
    pub states: u8,
    pub start_state_value: u8,
    pub bounding: i32,
}

impl Rule {
    pub(crate) fn get_bounding_ranges(
        &self,
    ) -> (
        RangeInclusive<i32>,
        RangeInclusive<i32>,
        RangeInclusive<i32>,
    ) {
        let x_range = -self.bounding..=self.bounding;
        let y_range = -self.bounding..=self.bounding;
        let z_range = -self.bounding..=self.bounding;
        (x_range, y_range, z_range)
    }
}
