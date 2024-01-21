use std::collections::HashMap;
use std::hash::Hash;
use std::ops::{
    Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign,
};

use ordered_float::OrderedFloat;
use regex::Regex;
use unicase::UniCase;

use super::chart::BmsChart;

/// This exists to not mix up time in seconds and time in quarter notes
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy, PartialOrd, Ord)]
pub struct BmsTimeQuarterNotes(pub OrderedFloat<f64>);

impl BmsTimeQuarterNotes {
    pub const ZERO: BmsTimeQuarterNotes =
        BmsTimeQuarterNotes(OrderedFloat(0.0));

    pub fn new(quarter_notes: f64) -> Self {
        BmsTimeQuarterNotes(OrderedFloat(quarter_notes))
    }
}

impl Add for BmsTimeQuarterNotes {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Output::new(*self.0 + *rhs.0)
    }
}

impl AddAssign for BmsTimeQuarterNotes {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.add(rhs);
    }
}

impl Sub for BmsTimeQuarterNotes {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::Output::new(*self.0 - *rhs.0)
    }
}

impl SubAssign for BmsTimeQuarterNotes {
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.sub(rhs);
    }
}

impl Mul for BmsTimeQuarterNotes {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::Output::new(*self.0 * *rhs.0)
    }
}

impl MulAssign for BmsTimeQuarterNotes {
    fn mul_assign(&mut self, rhs: Self) {
        *self = self.mul(rhs);
    }
}

impl Div for BmsTimeQuarterNotes {
    type Output = f64;

    fn div(self, rhs: Self) -> Self::Output {
        *self.0 / *rhs.0
    }
}

/// This exists to not mix up BmsTimeSeconds in seconds and BmsTimeSeconds in quarter notes
#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy, PartialOrd, Ord)]
pub struct BmsTimeSeconds(pub OrderedFloat<f64>);

impl BmsTimeSeconds {
    pub const ZERO: BmsTimeSeconds = BmsTimeSeconds(OrderedFloat(0.0));

    pub fn new(seconds: f64) -> Self {
        BmsTimeSeconds(OrderedFloat(seconds))
    }
}

impl Add for BmsTimeSeconds {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Output::new(*self.0 + *rhs.0)
    }
}

impl AddAssign for BmsTimeSeconds {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.add(rhs);
    }
}

impl Sub for BmsTimeSeconds {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::Output::new(*self.0 - *rhs.0)
    }
}

impl SubAssign for BmsTimeSeconds {
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.sub(rhs);
    }
}

impl Mul for BmsTimeSeconds {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::Output::new(*self.0 * *rhs.0)
    }
}

impl MulAssign for BmsTimeSeconds {
    fn mul_assign(&mut self, rhs: Self) {
        *self = self.mul(rhs);
    }
}

impl Div for BmsTimeSeconds {
    type Output = f64;

    fn div(self, rhs: Self) -> Self::Output {
        *self.0 / *rhs.0
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct BmsTiming {
    pub bpm_changes: HashMap<BmsTimeQuarterNotes, f64>,
    pub stops: HashMap<BmsTimeQuarterNotes, BmsTimeQuarterNotes>,
    pub scroll_changes: HashMap<BmsTimeQuarterNotes, f64>,
    pub speed_changes: HashMap<BmsTimeQuarterNotes, f64>,
}

// TODO: Name this function better
fn regex_header_thing<T: num_traits::Num + Eq + Hash, J: std::str::FromStr>(
    headers: &HashMap<UniCase<String>, String>,
    regex: &Regex,
) -> Option<HashMap<T, J>> {
    let mut out = HashMap::new();
    for key in headers.keys() {
        let lowercase_key = key.to_lowercase();
        let captures = match regex.captures(&lowercase_key) {
            Some(v) => v,
            None => continue,
        };
        let id = match T::from_str_radix(&captures[1], 36) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let value: J = match str::parse(headers.get(key).unwrap()) {
            Ok(v) => v,
            Err(_) => return None,
        };
        out.insert(id, value);
    }
    Some(out)
}

impl BmsTiming {
    // TODO: Clean up
    pub fn generate(chart: &BmsChart) -> Option<BmsTiming> {
        let bpm_regex = Regex::new(r"^bpm([a-z0-9]{2})$").unwrap();
        let bpm_ids: HashMap<u16, f64> =
            match regex_header_thing(&chart.headers, &bpm_regex) {
                Some(v) => v,
                None => return None,
            };

        let stop_regex = Regex::new(r"^stop([a-z0-9]{2})$").unwrap();
        let stop_ids: HashMap<u16, u32> =
            match regex_header_thing(&chart.headers, &stop_regex) {
                Some(v) => v,
                None => return None,
            };

        let scroll_regex = Regex::new(r"^scroll([a-z0-9]{2})$").unwrap();
        let scroll_ids: HashMap<u16, f64> =
            match regex_header_thing(&chart.headers, &scroll_regex) {
                Some(v) => v,
                None => return None,
            };

        let speed_regex = Regex::new(r"^speed([a-z0-9]{2})$").unwrap();
        let speed_ids: HashMap<u16, f64> =
            match regex_header_thing(&chart.headers, &speed_regex) {
                Some(v) => v,
                None => return None,
            };

        let mut bpm_changes: HashMap<BmsTimeQuarterNotes, f64> = chart
            .objects
            .iter()
            .filter(|object| {
                object.channel == 3
                    || (object.channel == 8
                        && bpm_ids.contains_key(&object.value))
            })
            .map(|object| match object.channel {
                3 => (object.time, object.value as f64),
                8 => (object.time, *bpm_ids.get(&object.value).unwrap()),
                _ => unreachable!(),
            })
            .collect();

        let start = BmsTimeQuarterNotes::ZERO;

        if bpm_changes.contains_key(&start) == false {
            bpm_changes.insert(
                start,
                match chart.headers.get(&UniCase::new("BPM".to_string())) {
                    Some(v) => match v.parse() {
                        Ok(v) => v,
                        Err(_) => return None,
                    },
                    None => return None,
                },
            );
        }

        // FIXME: FIX STOPS!!!!!!!!!!!!!!!!!!!!! Idk what to do
        let stops: HashMap<BmsTimeQuarterNotes, BmsTimeQuarterNotes> = chart
            .objects
            .iter()
            .filter(|object| {
                object.channel == 9 && stop_ids.contains_key(&object.value)
            })
            .map(|object| {
                (
                    object.time,
                    BmsTimeQuarterNotes::new(
                        (*stop_ids.get(&object.value).unwrap() as f64 * 4.0)
                            / 192.0,
                    ),
                )
            })
            .collect();

        let mut scroll_changes: HashMap<BmsTimeQuarterNotes, f64> = chart
            .objects
            .iter()
            .filter(|object| object.channel == 1020 /* SC in base 36 */ && scroll_ids.contains_key(&object.value))
            .map(|object| (object.time, *scroll_ids.get(&object.value).unwrap()))
            .collect();

        if scroll_changes.contains_key(&BmsTimeQuarterNotes::ZERO) == false {
            scroll_changes.insert(BmsTimeQuarterNotes::ZERO, 1.0);
        }

        let speed_changes: HashMap<BmsTimeQuarterNotes, f64> = chart
            .objects
            .iter()
            .filter(|object| object.channel == 1033 /* SP in base 36 */ && speed_ids.contains_key(&object.value))
            .map(|object| (object.time, *speed_ids.get(&object.value).unwrap()))
            .collect();

        let timing = BmsTiming {
            bpm_changes,
            stops,
            scroll_changes,
            speed_changes,
        };
        Some(timing)
    }
}
