use std::{
    cmp::Ordering, ops::RangeInclusive, time::Duration, vec,
};

use unicase::UniCase;

use crate::{
    chart::{BmsChart, BmsObject},
    timing::BmsTiming,
};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BmsNoteType {
    Normal { keysound: u16 },
    Hidden { keysound: u16 },
    Long { keysound: u16, end_time: u32 },
    BGM { keysound: u16 },
    Mine { damage: u16 },
}

#[derive(Debug, Clone, Copy)]
pub struct BmsNote {
    pub tick: u32,
    pub lane: u16,
    pub note_type: BmsNoteType,
}

#[derive(Debug, Clone)]
pub struct BmsNotes {
    pub notes: Vec<BmsNote>,
    pub hit_times:
        Option<Vec<(Duration, Option<Duration>)>>,
}

impl BmsNotes {
    /// Generates a ```BmsNotes``` of ```BmsNote``` out of a ```BmsChart```
    ///
    /// This includes the ```hit_times```
    pub fn generate_with_seconds(
        chart: &BmsChart,
        timing: &BmsTiming,
    ) -> BmsNotes {
        let mut notes = BmsNotes::generate(chart);
        notes.find_seconds(chart, timing);
        notes
    }

    // TODO: Clean up
    /// Generates a ```BmsNotes``` of ```BmsNote``` out of a ```BmsChart```
    pub fn generate(chart: &BmsChart) -> BmsNotes {
        const RANGES: [RangeInclusive<u16>; 9] = [
            // Comments will show range in base 36 for clarity
            01..=01,   // BGM: 01
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
            let mut note_type =
                BmsNoteType::Normal { keysound: 0 };

            for j in 0..RANGES.len() {
                let range = &RANGES[j];
                if range.contains(&object.channel) {
                    note_type = match j {
                        0 => BmsNoteType::BGM {
                            keysound: object.value,
                        },
                        1 | 2 => {
                            if let Ok(lnobj_id) = lnobj {
                                if object.value == lnobj_id
                                {
                                    continue;
                                }
                            }
                            BmsNoteType::Normal {
                                keysound: object.value,
                            }
                        }
                        3 | 4 => BmsNoteType::Hidden {
                            keysound: object.value,
                        },
                        5 | 6 => BmsNoteType::Long {
                            keysound: object.value,
                            end_time: 0,
                        },
                        7 | 8 => BmsNoteType::Mine {
                            damage: object.value / 2, // BMS Command Memo says to do this but idk ¯\_(ツ)_/¯
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
                        lane = object.channel
                            - RANGES[0].start();
                    } else if RANGES[1]
                        .contains(&object.channel)
                    {
                        lane = object.channel
                            - RANGES[1].start()
                            + RANGES[0].len() as u16;
                    }
                    if i < objects.len() - 1
                        && lnobj.as_ref().is_ok()
                        && object.value
                            != *lnobj.as_ref().unwrap()
                    {
                        let lnobj =
                            *lnobj.as_ref().unwrap();
                        if let Some(next_idx) =
                            objects.iter().position(|e| {
                                e.channel == object.channel
                                    && e.value == lnobj
                                    && e.tick > object.tick
                            })
                        {
                            note_type = BmsNoteType::Long {
                                keysound,
                                end_time: objects[next_idx]
                                    .tick,
                            };
                        }
                    }
                }
                BmsNoteType::Hidden { keysound: _ } => {
                    if RANGES[2].contains(&object.channel) {
                        lane = object.channel
                            - RANGES[2].start();
                    } else if RANGES[3]
                        .contains(&object.channel)
                    {
                        lane = object.channel
                            - RANGES[3].start()
                            + RANGES[2].len() as u16;
                    }
                }
                BmsNoteType::Long {
                    keysound,
                    end_time: _,
                } => {
                    if RANGES[4].contains(&object.channel) {
                        lane = object.channel
                            - RANGES[4].start();
                    } else if RANGES[5]
                        .contains(&object.channel)
                    {
                        lane = object.channel
                            - RANGES[5].start()
                            + RANGES[4].len() as u16;
                    }
                    if let Some(next_idx) =
                        objects.iter().position(|e| {
                            e.channel == object.channel
                                && e.value == object.value
                                && e.tick > object.tick
                        })
                    {
                        let next = objects[next_idx];
                        note_type = BmsNoteType::Long {
                            keysound,
                            end_time: next.tick,
                        };
                    } else {
                        continue;
                    }
                }
                BmsNoteType::Mine { damage: _ } => {
                    if RANGES[6].contains(&object.channel) {
                        lane = object.channel
                            - RANGES[6].start();
                    } else if RANGES[7]
                        .contains(&object.channel)
                    {
                        lane = object.channel
                            - RANGES[7].start()
                            + RANGES[6].len() as u16;
                    }
                }
                _ => {}
            }

            notes.push(BmsNote {
                tick: object.tick,
                lane,
                note_type,
            });
        }

        BmsNotes {
            notes,
            hit_times: None,
        }
    }

    /// Find seconds from for the notes in this ```BmsNotes``` with a ```BmsTiming``` and a ```BmsChart```
    pub fn find_seconds(
        &mut self,
        chart: &BmsChart,
        timing: &BmsTiming,
    ) {
        if self.notes.is_empty() {
            return;
        }

        let mut hit_times =
            vec![(Duration::ZERO, None); self.notes.len()];

        let bpm_changes = &mut timing.bpm_changes.clone();
        let stops = &timing.stops;

        let initial_bpm = bpm_changes.remove(&0).expect(
            "Couldn't get BPM Change at tick 0 (???)",
        );
        let mut current_tick_duration =
            Duration::new(60, 0).div_f64(initial_bpm)
                / chart.resolution;

        // For safety, we make it **inmutable** so no changes get
        // made to the bpm_changes again after removing the one at 0
        let bpm_changes = bpm_changes;

        #[derive(PartialEq, Eq, Ord)]
        enum EventType {
            PlayNote { index: usize },
            EndLongNote { index: usize },
            ChangeBpm { new_duration: Duration },
            Stop { ticks_stopped: u32 },
        }

        impl PartialOrd for EventType {
            fn partial_cmp(
                &self,
                other: &Self,
            ) -> Option<Ordering> {
                match self {
                    Self::PlayNote { index: _ }
                    | Self::EndLongNote { index: _ } => {
                        return match other {
                            Self::PlayNote { index: _ }
                            | Self::EndLongNote {
                                index: _,
                            } => Some(Ordering::Equal),
                            Self::ChangeBpm {
                                new_duration: _,
                            } => Some(Ordering::Less),
                            Self::Stop {
                                ticks_stopped: _,
                            } => Some(Ordering::Less),
                        }
                    }
                    Self::ChangeBpm { new_duration: _ } => {
                        return match other {
                            Self::PlayNote { index: _ }
                            | Self::EndLongNote {
                                index: _,
                            } => Some(Ordering::Greater),
                            Self::ChangeBpm {
                                new_duration: _,
                            } => Some(Ordering::Equal),
                            Self::Stop {
                                ticks_stopped: _,
                            } => Some(Ordering::Less),
                        }
                    }
                    Self::Stop { ticks_stopped: _ } => {
                        return match other {
                            Self::PlayNote { index: _ }
                            | Self::EndLongNote {
                                index: _,
                            } => Some(Ordering::Greater),
                            Self::ChangeBpm {
                                new_duration: _,
                            } => Some(Ordering::Greater),
                            Self::Stop {
                                ticks_stopped: _,
                            } => Some(Ordering::Equal),
                        }
                    }
                }
            }
        }

        #[derive(PartialEq, Eq, Ord)]
        struct Event {
            tick: u32,
            event_type: EventType,
        }

        impl PartialOrd for Event {
            fn partial_cmp(
                &self,
                other: &Self,
            ) -> Option<Ordering> {
                match self.tick.partial_cmp(&other.tick) {
                    Some(Ordering::Equal) => {}
                    ord => return ord,
                }
                self.event_type
                    .partial_cmp(&other.event_type)
            }
        }

        let mut events: Vec<Event> = Vec::new();

        for i in 0..self.notes.len() {
            let note = self.notes[i];
            events.push(Event {
                tick: note.tick,
                event_type: EventType::PlayNote {
                    index: i,
                },
            });
            if let BmsNoteType::Long {
                keysound: _,
                end_time,
            } = note.note_type
            {
                events.push(Event {
                    tick: end_time,
                    event_type: EventType::EndLongNote {
                        index: i,
                    },
                });
            }
        }

        for (tick, new_bpm) in bpm_changes {
            events.push(Event {
                tick: *tick,
                event_type: EventType::ChangeBpm {
                    new_duration: Duration::new(60, 0)
                        .div_f64(*new_bpm)
                        / chart.resolution,
                },
            })
        }

        for (tick, ticks_stopped) in stops {
            events.push(Event {
                tick: *tick,
                event_type: EventType::Stop {
                    ticks_stopped: *ticks_stopped,
                },
            })
        }

        let mut events = events.iter();

        let mut offset_ticks = 0;
        let mut offset_time = Duration::ZERO;

        while let Some(event) = events.next() {
            match event.event_type {
                EventType::PlayNote { index } => {
                    hit_times[index].0 =
                        current_tick_duration
                            * (event.tick - offset_ticks)
                                as u32
                            + offset_time;
                }
                EventType::EndLongNote { index } => {
                    hit_times[index].1 = Some(
                        current_tick_duration
                            * (event.tick - offset_ticks)
                                as u32
                            + offset_time,
                    );
                }
                EventType::ChangeBpm { new_duration } => {
                    offset_time = current_tick_duration
                        * (event.tick - offset_ticks)
                            as u32;
                    offset_ticks = event.tick;
                    current_tick_duration = new_duration;
                    println!(
                        "New tick interval! {:#?}",
                        new_duration
                    );
                }
                EventType::Stop { ticks_stopped } => {
                    let event_time = current_tick_duration
                        * (event.tick - offset_ticks)
                            as u32;
                    let stop_duration =
                        current_tick_duration
                            * ticks_stopped;
                    offset_time =
                        event_time + stop_duration;
                    offset_ticks = event.tick;
                    println!(
                        "Stop for {:#?}! ({} ticks)",
                        stop_duration, ticks_stopped
                    );
                }
            }
        }
        self.hit_times = Some(hit_times);
    }
}
