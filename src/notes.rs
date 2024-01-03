use std::{ops::RangeInclusive, vec, f64::NAN};

use ordered_float::OrderedFloat;
use unicase::UniCase;

use crate::{
    chart::{BmsChart, BmsObject},
    timing::BmsTime,
};

#[derive(Debug, PartialEq)]
pub enum BmsNoteType {
    Normal {
        keysound: u16,
    },
    Hidden {
        keysound: u16,
    },
    Long {
        keysound: u16,
        end_time: BmsTime,
    },
    Mine {
        damage: u16,
    },
}

#[derive(Debug)]
pub struct BmsNote {
    pub hit_time: BmsTime,
    pub lane: u16,
    pub note_type: BmsNoteType,
}

// TODO: Clean up
/// Generates a ```Vec``` of ```BmsNote``` out of a ```BmsChart```
pub fn generate_notes(chart: &BmsChart) -> Vec<BmsNote> {
    const RANGES: [RangeInclusive<u16>; 8] = [
        // Comments will show range in base 36 for clarity
        37..=71,   // 1P Visible: 11..=1Z
        73..=107,  // 2P Visible: 21..=2Z
        109..=143, // 1P Invisible: 31..=3Z
        145..=179, // 2P Invisible: 41..=4Z
        181..=215, // 1P Longnote: 51..=5Z
        217..=251, // 2P Longnote: 61..=6Z
        469..=477, // 1P Landmine: D1..=D9
        505..=513, // 2P Landmine: E1..=E9
    ];

    // Filter out objects in channels we aren't interested in
    let mut objects: Vec<&BmsObject> = chart
        .objects
        .iter()
        .filter(|obj| {
            for range in RANGES {
                if range.contains(&obj.channel) {
                    return true;
                }
            }
            return false;
        })
        .collect();
    objects.sort();

    // Make it something invalid when it doesn't exist
    let invalid_lnobj_string = "/".to_string();
    let lnobj_string = chart
        .headers
        .get(&UniCase::from("LNOBJ"))
        .unwrap_or(&invalid_lnobj_string);
    let lnobj = u16::from_str_radix(lnobj_string, 16);

    let mut notes: Vec<BmsNote> = vec![];
    for i in 0..objects.len() {
        let object: &BmsObject = objects[i];

        let mut lane = 0;
        // We find the note type by searching the channel ranges
        let mut note_type = BmsNoteType::Normal { keysound: 0 };

        for j in 0..RANGES.len() {
            let range = &RANGES[j];
            if range.contains(&object.channel) {
                note_type = match j {
                    0 | 1 => BmsNoteType::Normal {
                        keysound: object.value,
                    },
                    2 | 3 => BmsNoteType::Hidden {
                        keysound: object.value,
                    },
                    4 | 5 => BmsNoteType::Long {
                        keysound: object.value,
                        end_time: BmsTime { measure: 0, fraction: OrderedFloat(NAN) },
                    },
                    6 | 7 => BmsNoteType::Mine {
                        damage: object.value / 2, // BMS CMD MEMO says to this idk...
                    },
                    _ => unreachable!(),
                };
            }
        }

        // Since LNOBJ exists, it could be used for longnotes
        // and so, we check if the next note's channel is lnobj
        // We also make sure there IS a next object
        // We can't forget that if it isn't a normal note, then
        // it shouldn't be long as well
        match note_type {
            BmsNoteType::Normal { keysound } => {
                if RANGES[0].contains(&object.channel) {
                    lane = object.channel - RANGES[0].start();
                } else if RANGES[1].contains(&object.channel) {
                    lane = object.channel - RANGES[1].start();
                }
                if i < objects.len() - 1
                    && lnobj.as_ref().is_ok()
                    && object.value != *lnobj.as_ref().unwrap()
                {
                    let lnobj = *lnobj.as_ref().unwrap();
                    if let Some(next_idx) = objects.iter().position(|e| {
                        e.channel == object.channel && e.value == lnobj && e.time > object.time
                    }) {
                        note_type = BmsNoteType::Long {
                            keysound,
                            end_time: objects[next_idx].time,
                        };
                    }
                }
            }
            BmsNoteType::Hidden { keysound: _ } => {
                if RANGES[2].contains(&object.channel) {
                    lane = object.channel - RANGES[2].start();
                } else if RANGES[3].contains(&object.channel) {
                    lane = object.channel - RANGES[3].start();
                }
            }
            BmsNoteType::Long {
                keysound,
                end_time: _,
            } => {
                if RANGES[4].contains(&object.channel) {
                    lane = object.channel - RANGES[4].start();
                } else if RANGES[5].contains(&object.channel) {
                    lane = object.channel - RANGES[5].start();
                }
                if let Some(next_idx) = objects.iter().position(|e| {
                    e.channel == object.channel && e.value == object.value && e.time > object.time
                }) {
                    let next = objects[next_idx];
                    note_type = BmsNoteType::Long {
                        keysound,
                        end_time: next.time,
                    };
                } else {
                    continue;
                }
            }
            BmsNoteType::Mine { damage: _ } => {
                if RANGES[6].contains(&object.channel) {
                    lane = object.channel - RANGES[6].start();
                } else if RANGES[7].contains(&object.channel) {
                    lane = object.channel - RANGES[7].start();
                }
            }
        }

        notes.push(BmsNote {
            hit_time: object.time,
            lane,
            note_type,
        });
    }

    notes
}
