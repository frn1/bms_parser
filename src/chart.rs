use std::collections::HashMap;

use ordered_float::OrderedFloat;
use regex::Regex;
use strum::{EnumIter, FromRepr, IntoEnumIterator};
use unicase::UniCase;

#[derive(Eq, Hash, Debug, Clone, Copy, Ord)]
pub struct BmsObject {
    pub channel: u16,
    pub tick: u64,
    pub value: u16,
}

impl PartialEq for BmsObject {
    fn eq(&self, other: &Self) -> bool {
        self.channel == other.channel && self.tick == other.tick
    }
}

impl PartialOrd for BmsObject {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.tick.partial_cmp(&other.tick) {
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
    pub resolution: u32, // Ticks per quarter note
    pub headers: HashMap<UniCase<String>, String>,
    pub objects: Vec<BmsObject>,
    pub barlines: Vec<u64>,
}

// TODO: Clean up
impl BmsChart {
    /// Updates/fixes the objects in the chart and ensures
    /// a good state for the ```objects``` property
    ///
    /// Although sometimes unecessary, this function
    /// should be called right after you finish messing
    /// around with the objects.
    ///
    /// Nothing really break if you don't, but objects
    /// shouldn't have the same time unless their
    /// channel is BGM (01)
    pub fn update_objects(&mut self) {
        // Sort them
        self.objects.sort();
        // Remove all duplicates except if they are in a BGM channel (01)
        self.objects
            .dedup_by(|a, b| a == b && a.channel != 1 && b.channel != 1);
    }

    /// Compiles a ```BmsChart``` from a ```&str```.
    ///
    /// The **inclusive** range of values returned by the rng function
    /// should be between 1 and ```max_value``` (AKA ```1..=max_value```)
    ///
    /// If you can't use a random number generator for whatever reason,
    /// then a simple function like this would work as a placeholder:
    ///
    /// ```rust
    /// fn bms_rng(max: u32) -> u32 {
    ///     max
    /// }
    /// ```
    pub fn compile(
        data: &str,
        resolution: u32,
        rng: fn(max_value: u32) -> u32,
    ) -> Result<BmsChart, &str> {
        let random_regex: Regex = Regex::new(r"^#RANDOM\s+(\d+)$").unwrap();
        let endrandom_regex: Regex = Regex::new(r"^#ENDRANDOM$").unwrap();
        let if_regex: Regex = Regex::new(r"^#IF\s+(\d+)$").unwrap();
        let endif_regex: Regex = Regex::new(r"^#ENDIF$").unwrap();
        let time_signature_regex: Regex = Regex::new(r"^#(\d\d\d)02:(\S*)$").unwrap();
        let channel_regex: Regex =
            Regex::new(r"^#(?:EXT\s+#)?(\d\d\d)(\S\S):([0-9a-zA-Z]*)$").unwrap();
        let header_regex: Regex = Regex::new(r"^#(\w+)(?:\s+(\S.*))?$").unwrap();

        let mut chart = BmsChart {
            resolution,
            headers: HashMap::new(),
            objects: vec![],
            barlines: vec![],
        };

        let mut rng_stack = vec![];
        let mut skip_stack = vec![];

        #[derive(PartialEq, PartialOrd)]
        struct BmsTime {
            measure: u32,
            fraction: f64,
        }

        let mut time_signatures: HashMap<u32, f64> = HashMap::new();
        let mut objects: Vec<(u16, BmsTime, u16)> = Vec::new();

        for line in data.trim().lines() {
            // if line.starts_with('#') == false {
            //     continue;
            // }
            let mut matched_any = false;
            if let Some(captures) = random_regex.captures(line) {
                let max = match u32::from_str_radix(&captures[1], 10) {
                    Ok(v) => v,
                    Err(_) => return Err("Couldn't parse max RANDOM value"),
                };
                let rng_value = rng(max);
                rng_stack.push(rng_value);
                matched_any = true;
            } else if endrandom_regex.is_match(line) {
                rng_stack.pop();
                matched_any = true;
            } else if let Some(captures) = if_regex.captures(line) {
                let value = match u32::from_str_radix(&captures[1], 10) {
                    Ok(v) => v,
                    Err(_) => return Err("Couldn't parse max IF value"),
                };
                let rng_value = match rng_stack.last() {
                    Some(v) => *v,
                    None => return Err("IF without a RANDOM value to use"),
                };
                skip_stack.push(rng_value != value);
                matched_any = true;
            } else if endif_regex.is_match(line) {
                skip_stack.pop();
                matched_any = true;
            }

            let skipping = *skip_stack.last().unwrap_or(&false);

            if skipping == false && matched_any == false {
                if let Some(captures) = time_signature_regex.captures(line) {
                    let measure = match u32::from_str_radix(&captures[1], 10) {
                        Ok(v) => v,
                        Err(_) => return Err("Couldn't parse measure number in time signature"),
                    };
                    let time_signature: f64 = match captures[2].parse() {
                        Ok(v) => v,
                        Err(_) => return Err("Couldn't parse time signature value"),
                    };
                    time_signatures.insert(measure, time_signature);
                } else if let Some(captures) = channel_regex.captures(line) {
                    let measure = match u32::from_str_radix(&captures[1], 10) {
                        Ok(v) => v,
                        Err(_) => return Err("Couldn't parse measure number of objects"),
                    };
                    let channel = match u16::from_str_radix(&captures[2], 36) {
                        Ok(v) => v,
                        Err(_) => return Err("Couldn't parse channel number of objects"),
                    };
                    let values_str = &captures[3];
                    // Values come in pairs so we divide by 2 to get the divisions in the measure
                    let num_values = values_str.len() / 2;
                    for i in 0..num_values {
                        let text = &values_str[i * 2..=i * 2 + 1];
                        let value = match u16::from_str_radix(text, {
                            // For some reason channel 03 uses base 16 for values
                            if channel == 3 {
                                16
                            } else {
                                36
                            }
                        }) {
                            Ok(v) => v,
                            Err(_) => return Err("Couldn't parse object value"),
                        };
                        if value != 0 {
                            let object = (
                                channel,
                                BmsTime {
                                    measure,
                                    fraction: (1.0 / num_values as f64) * i as f64,
                                },
                                value,
                            );
                            objects.push(object);
                        }
                    }
                } else if let Some(captures) = header_regex.captures(line) {
                    let name = &captures[1];
                    let value = &captures[2];
                    chart
                        .headers
                        .insert(UniCase::new(name.to_string()), value.to_string());
                }
            }
        }

        if objects.len() != 0 {
            objects.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            objects.reverse();
            let end_bms_time = &objects.first().unwrap().1;
            let mut ticks = 0;
            for measure in 0..=end_bms_time.measure {
                chart.barlines.push(ticks);
                let quarter_notes_per_measure = time_signatures.get(&measure).unwrap_or(&1.0) * 4.0;
                let ticks_in_measure =
                    (quarter_notes_per_measure * resolution as f64).round() as u64;
                while let Some(object) = objects.last() {
                    if object.1.measure != measure {
                        break;
                    }
                    let tick = (object.1.fraction * ticks_in_measure as f64).round() as u64 + ticks;
                    chart.objects.push(BmsObject {
                        channel: object.0,
                        tick,
                        value: object.2,
                    });
                    objects.pop();
                }

                ticks += ticks_in_measure;
            }
        }

        chart.update_objects();
        Ok(chart)
    }
}
