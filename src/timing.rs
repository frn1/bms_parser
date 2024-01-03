use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Sub;

use num::BigUint;
use ordered_float::OrderedFloat;
use regex::Regex;
use unicase::UniCase;

use super::chart::BmsChart;

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy, PartialOrd, Ord, Default)]
pub struct BmsTime {
    pub measure: u16,
    pub fraction: OrderedFloat<f64>,
}

impl Sub for BmsTime {
    fn sub(self, rhs: Self) -> Self::Output {
        let mut output = BmsTime {
            measure: self.measure - rhs.measure,
            fraction: self.fraction - rhs.fraction,
        };
        output.measure = (output.measure as i16 + output.fraction.floor() as i16) as u16;
        output.fraction %= 1.0;
        output.fraction = OrderedFloat(output.fraction.abs());
        output
    }

    type Output = BmsTime;
}

#[derive(PartialEq, Debug, Clone)]
pub struct BmsTiming {
    pub bpm_changes: HashMap<BmsTime, f64>,
    pub stops: HashMap<BmsTime, f64>,
    pub scroll_changes: HashMap<BmsTime, f64>,
}

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

pub fn generate_timings(chart: &BmsChart) -> Option<BmsTiming> {
    let bpm_regex = Regex::new(r"^bpm(\d{2})$").unwrap();
    let bpm_ids: HashMap<u16, f64> = match regex_header_thing(&chart.headers, &bpm_regex) {
        Some(v) => v,
        None => return None,
    };
    let stop_regex = Regex::new(r"^stop(\d{2})$").unwrap();
    let stop_ids: HashMap<u16, f64> = match regex_header_thing(&chart.headers, &stop_regex) {
        Some(v) => v,
        None => return None,
    };
    let scroll_regex = Regex::new(r"^scroll(\d{2})$").unwrap();
    let scroll_ids: HashMap<u16, f64> = match regex_header_thing(&chart.headers, &scroll_regex) {
        Some(v) => v,
        None => return None,
    };
    let mut bpm_changes: HashMap<BmsTime, f64> = chart
        .objects
        .iter()
        .filter(|object| {
            object.channel == 3 || (object.channel == 8 && bpm_ids.contains_key(&object.value))
        })
        .map(|object| match object.channel {
            3 => {
                // Shitty horrible thing that must be done because
                // on channel 3, values are in base 16, but we took it
                // as a base 36 value, so we undo the convertion then
                // re-parse it as base 16
                let bpm_str = BigUint::from(object.value).to_str_radix(36);
                let bpm = u8::from_str_radix(&bpm_str, 16).unwrap();
                (object.time, bpm as f64)
            }
            8 => (object.time, *bpm_ids.get(&object.value).unwrap()),
            _ => unreachable!(),
        })
        .collect();
    let start = BmsTime {
        measure: 0,
        fraction: OrderedFloat(0.0),
    };
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
    let stops: HashMap<BmsTime, f64> = chart
        .objects
        .iter()
        .filter(|object| object.channel == 9 && stop_ids.contains_key(&object.value))
        .map(|object| (object.time, *stop_ids.get(&object.value).unwrap()))
        .collect();
    let scroll_changes: HashMap<BmsTime, f64> = chart
        .objects
        .iter()
        .filter(|object| object.channel == 1020 /* SC in base 36 */ && scroll_ids.contains_key(&object.value))
        .map(|object| (object.time, *scroll_ids.get(&object.value).unwrap()))
        .collect();

    let timing = BmsTiming {
        bpm_changes,
        stops,
        scroll_changes,
    };
    Some(timing)
}

fn process_bpm_change(
    new_bpm: f64,
    time_sig: f64,
    beats: f64,
    change_time: BmsTime,
    last_change_seconds: &mut f64,
    last_change_beats: &mut f64,
    current_bpm: &mut f64,
) {
    let current_beats = beats + time_sig * 4.0 * change_time.fraction.0;
    *last_change_seconds += (current_beats - *last_change_beats) * (60.0 / *current_bpm);
    *last_change_beats = current_beats;
    *current_bpm = new_bpm;
}

impl BmsTime {
    pub fn to_seconds(
        &self,
        bpm_changes: &HashMap<BmsTime, f64>,
        stops: &HashMap<BmsTime, f64>,
        time_signatures: &HashMap<u16, f64>,
    ) -> f64 {
        let mut bpm_change_times: Vec<&BmsTime> = bpm_changes.keys().collect();
        bpm_change_times.sort();
        bpm_change_times.reverse();

        let mut stop_times: Vec<&BmsTime> = stops.keys().collect();
        stop_times.sort();
        stop_times.reverse();

        let mut current_bpm = *bpm_changes.get(bpm_change_times.pop().unwrap()).unwrap();
        let mut last_change_beats = 0.0;
        let mut last_change_seconds = 0.0;
        let mut seconds_stoped = 0.0;
        let mut time_sig = 1.0;
        let mut beats = 0.0;

        for i in 0..self.measure {
            time_sig = *time_signatures.get(&i).unwrap_or(&1.0);
            let time = BmsTime {
                measure: i + 1,
                ..Default::default()
            };
            loop {
                if let Some(next_stop_time) = stop_times.last() {
                    if let Some(next_bpm_change_time) = bpm_change_times.last() {
                        if next_stop_time < next_bpm_change_time && time > **next_stop_time {
                            seconds_stoped += (60.0 / current_bpm)
                                * (stops.get(next_stop_time).unwrap() / 192.0)
                                * 4.0;
                            stop_times.pop();
                        } else if time > **next_bpm_change_time {
                            process_bpm_change(
                                *bpm_changes.get(next_bpm_change_time).unwrap(),
                                time_sig,
                                beats,
                                **next_bpm_change_time,
                                &mut last_change_seconds,
                                &mut last_change_beats,
                                &mut current_bpm,
                            );
                            bpm_change_times.pop();
                        } else {
                            break;
                        }
                    } else if time > **next_stop_time {
                        seconds_stoped += (60.0 / current_bpm)
                            * (stops.get(next_stop_time).unwrap() / 192.0)
                            * 4.0;
                        stop_times.pop();
                    } else {
                        break;
                    }
                } else if let Some(next_bpm_change_time) = bpm_change_times.last() {
                    if time > **next_bpm_change_time {
                        process_bpm_change(
                            *bpm_changes.get(next_bpm_change_time).unwrap(),
                            time_sig,
                            beats,
                            **next_bpm_change_time,
                            &mut last_change_seconds,
                            &mut last_change_beats,
                            &mut current_bpm,
                        );
                        bpm_change_times.pop();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            beats += 4.0 * time_sig;
        }
        // Process the last change in time signature without adding the measure to beats
        while let Some(next_bpm_change_time) = bpm_change_times.last() {
            if self > *next_bpm_change_time {
                let current_beats = beats + time_sig * 4.0 * next_bpm_change_time.fraction.0;
                last_change_seconds += (current_beats - last_change_beats) * (60.0 / current_bpm);
                last_change_beats = current_beats;
                current_bpm = *bpm_changes.get(next_bpm_change_time).unwrap();
                bpm_change_times.pop();
            } else {
                break;
            }
        }
        time_sig = *time_signatures.get(&self.measure).unwrap_or(&1.0);
        beats += 4.0 * self.fraction.0 * time_sig;

        (beats - last_change_beats) * (60.0 / current_bpm) + last_change_seconds + seconds_stoped
    }
}
