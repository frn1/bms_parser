use std::collections::HashMap;

use regex::Regex;

use super::chart::BmsChart;

pub fn generate_keysounds(chart: &BmsChart) -> HashMap<u16, String> {
    let mut keysounds = HashMap::new();
    let keysound_regex = Regex::new(r"^wav(\S\S)$").unwrap();
    for key in chart.headers.keys() {
        let lowercase_key = key.to_lowercase();
        let captures = match keysound_regex.captures(&lowercase_key) {
            Some(v) => v,
            None => continue,
        };
        let id = match u16::from_str_radix(&captures[1], 36) {
            Ok(v) => v,
            Err(_) => continue,
        };
        keysounds.insert(id, chart.headers[key].clone());
    }
    keysounds
}