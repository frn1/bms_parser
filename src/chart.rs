use std::collections::HashMap;

use ordered_float::OrderedFloat;
use regex::Regex;
use strum::{EnumIter, FromRepr, IntoEnumIterator};
use unicase::UniCase;

use super::timing::BmsTime;

#[derive(Eq, Hash, Debug, Clone, Copy, Ord)]
pub struct BmsObject {
    pub channel: u16,
    pub time: BmsTime,
    pub value: u16,
}

impl PartialEq for BmsObject {
    fn eq(&self, other: &Self) -> bool {
        self.channel == other.channel && self.time == other.time
    }
}

impl PartialOrd for BmsObject {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.time.partial_cmp(&other.time) {
            Some(core::cmp::Ordering::Equal) => {
                if self.value == other.value && self.channel == other.channel {
                    return Some(core::cmp::Ordering::Equal);
                } else {
                    return None;
                }
            }
            ord => return ord,
        }
    }
}

#[derive(Debug)]
pub struct BmsChart {
    pub headers: HashMap<UniCase<String>, String>,
    pub objects: Vec<BmsObject>,
    pub time_signatures: HashMap<u16, f64>,
}

impl BmsChart {
    pub fn update_objects(&mut self) {
        self.objects.sort();
        // Remove all duplicates except if they are in a BGM channel (01)
        self.objects
            .dedup_by(|a, b| a == b && a.channel != 1 && b.channel != 1);
    }

    pub fn compile(data: &str, rng: fn(u32) -> u32) -> Option<BmsChart> {
        // Anything that's related to the flow of the chart
        #[derive(EnumIter, FromRepr, Debug)]
        enum BmsControlMatches {
            Random,
            EndRandom,
            If,
            EndIf,
        }

        // Anything that's related to the chart or it's metadata
        #[derive(EnumIter, FromRepr, Debug)]
        enum BmsChartMatches {
            TimeSignature,
            Channel,
            Header,
        }

        impl BmsControlMatches {
            fn as_regex_str(&self) -> &'static str {
                match self {
                    Self::Random => r"^#RANDOM\s+(\d+)$",
                    Self::EndRandom => r"^#ENDRANDOM$",
                    Self::If => r"^#IF\s+(\d+)$",
                    Self::EndIf => r"^#ENDIF$",
                }
            }
        }

        impl BmsChartMatches {
            fn as_regex_str(&self) -> &'static str {
                match self {
                    Self::TimeSignature => r"^#(\d\d\d)02:(\S*)$",
                    Self::Channel => r"^#(?:EXT\s+#)?(\d\d\d)(\S\S):([0-9a-zA-Z]*)$",
                    Self::Header => r"^#(\w+)(?:\s+(\S.*))?$",
                }
            }
        }

        let mut chart = BmsChart {
            headers: HashMap::new(),
            objects: vec![],
            time_signatures: HashMap::new(),
        };

        let mut rng_stack = vec![];
        let mut skip_stack = vec![];

        let control_regex_expressions = BmsControlMatches::iter().map(|v| v.as_regex_str());
        let control_regexes: Vec<Regex> = control_regex_expressions
            .map(|expr| Regex::new(expr).unwrap())
            .collect();
        let chart_regex_expressions = BmsChartMatches::iter().map(|v| v.as_regex_str());
        let chart_regexes: Vec<Regex> = chart_regex_expressions
            .map(|expr| Regex::new(expr).unwrap())
            .collect();
        for line in data.trim().lines() {
            // if line.starts_with('#') == false {
            //     continue;
            // }
            let mut matched_any = false;
            for i in 0..control_regexes.len() {
                let v = &control_regexes[i];
                let captures = match v.captures(line) {
                    Some(v) => v,
                    None => continue,
                };

                let match_type = BmsControlMatches::from_repr(i).unwrap();

                matched_any = true;

                match match_type {
                    BmsControlMatches::Random => {
                        let max = match u32::from_str_radix(&captures[1], 10) {
                            Ok(v) => v,
                            Err(_) => return None,
                        };
                        let rng_value = rng(max);
                        rng_stack.push(rng_value);
                    }
                    BmsControlMatches::EndRandom => {
                        rng_stack.pop();
                    }
                    BmsControlMatches::If => {
                        let value = match u32::from_str_radix(&captures[1], 10) {
                            Ok(v) => v,
                            Err(_) => return None,
                        };
                        let rng_value = match rng_stack.last() {
                            Some(v) => *v,
                            None => return None,
                        };
                        skip_stack.push(rng_value != value);
                    }
                    BmsControlMatches::EndIf => {
                        skip_stack.pop();
                    }
                }
            }

            let skipping = *skip_stack.last().unwrap_or(&false);

            if skipping == false && matched_any == false {
                for i in 0..chart_regexes.len() {
                    let v = &chart_regexes[i];
                    let captures = match v.captures(line) {
                        Some(v) => v,
                        None => continue,
                    };

                    let match_type = BmsChartMatches::from_repr(i).unwrap();

                    match match_type {
                        BmsChartMatches::TimeSignature => {
                            let measure = match u16::from_str_radix(&captures[1], 10) {
                                Ok(v) => v,
                                Err(_) => return None,
                            };
                            let time_signature: f64 = match captures[2].parse() {
                                Ok(v) => v,
                                Err(_) => return None,
                            };
                            chart.time_signatures.insert(measure, time_signature);
                        }
                        BmsChartMatches::Channel => {
                            let measure = match u16::from_str_radix(&captures[1], 10) {
                                Ok(v) => v,
                                Err(_) => return None,
                            };
                            let channel = match u16::from_str_radix(&captures[2], 36) {
                                Ok(v) => v,
                                Err(_) => return None,
                            };
                            let values_str = &captures[3];
                            // Values come in pairs so we divide by 2 to get the divisions in the measurew
                            let divisions = values_str.len() / 2;
                            for i in 0..divisions {
                                let text = &values_str[i * 2..=i * 2 + 1];
                                let value = match u16::from_str_radix(text, 36) {
                                    Ok(v) => v,
                                    Err(_) => return None,
                                };
                                if value != 0 {
                                    let object = BmsObject {
                                        channel,
                                        time: BmsTime {
                                            measure,
                                            fraction: OrderedFloat(
                                                (1.0 / divisions as f64) * i as f64,
                                            ),
                                        },
                                        value,
                                    };
                                    chart.objects.push(object);
                                }
                            }
                        }
                        BmsChartMatches::Header => {
                            let name = &captures[1];
                            let value = &captures[2];
                            chart.headers.insert(UniCase::new(name.to_string()), value.to_string());
                        }
                    }
                    break;
                }
            }
        }
        chart.update_objects();
        Some(chart)
    }
}
