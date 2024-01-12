use std::collections::HashMap;
use std::hash::Hash;
use std::ops::{Add, Sub};

use num::BigUint;
use ordered_float::OrderedFloat;
use regex::Regex;
use unicase::UniCase;

use super::chart::BmsChart;

#[derive(PartialEq, Debug, Clone)]
pub struct BmsTiming {
    pub bpm_changes: HashMap<u64, f64>,
    pub stops: HashMap<u64, u64>,
    pub scroll_changes: HashMap<u64, f64>,
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

// TODO: Clean up
pub fn generate_timings(chart: &BmsChart) -> Option<BmsTiming> {
    let bpm_regex = Regex::new(r"^bpm(\d{2})$").unwrap();
    let bpm_ids: HashMap<u16, f64> = match regex_header_thing(&chart.headers, &bpm_regex) {
        Some(v) => v,
        None => return None,
    };

    let stop_regex = Regex::new(r"^stop(\d{2})$").unwrap();
    let stop_ids: HashMap<u16, u64> = match regex_header_thing(&chart.headers, &stop_regex) {
        Some(v) => v,
        None => return None,
    };

    let scroll_regex = Regex::new(r"^scroll(\d{2})$").unwrap();
    let scroll_ids: HashMap<u16, f64> = match regex_header_thing(&chart.headers, &scroll_regex) {
        Some(v) => v,
        None => return None,
    };

    let mut bpm_changes: HashMap<u64, f64> = chart
        .objects
        .iter()
        .filter(|object| {
            object.channel == 3 || (object.channel == 8 && bpm_ids.contains_key(&object.value))
        })
        .map(|object| match object.channel {
            3 => (object.tick, object.value as f64),
            8 => (object.tick, *bpm_ids.get(&object.value).unwrap()),
            _ => unreachable!(),
        })
        .collect();

    let start = 0;

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
    let stops: HashMap<u64, u64> = chart
        .objects
        .iter()
        .filter(|object| object.channel == 9 && stop_ids.contains_key(&object.value))
        .map(|object| {
            (
                object.tick,
                (*stop_ids.get(&object.value).unwrap() * chart.resolution as u64) / (192 * 4),
            )
        })
        .collect();

    let scroll_changes: HashMap<u64, f64> = chart
        .objects
        .iter()
        .filter(|object| object.channel == 1020 /* SC in base 36 */ && scroll_ids.contains_key(&object.value))
        .map(|object| (object.tick, *scroll_ids.get(&object.value).unwrap()))
        .collect();

    let timing = BmsTiming {
        bpm_changes,
        stops,
        scroll_changes,
    };
    Some(timing)
}
