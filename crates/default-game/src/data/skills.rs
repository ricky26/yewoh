use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Skill {
    name: String,
    noun: String,
    str_scale: f32,
    dex_scale: f32,
    int_scale: f32,
    str_gain: f32,
    dex_gain: f32,
    int_gain: f32,
    gain_scale: f32,
}

impl Default for Skill {
    fn default() -> Self {
        Self {
            name: String::new(),
            noun: String::new(),
            str_scale: 0.0,
            dex_scale: 0.0,
            int_scale: 0.0,
            str_gain: 0.0,
            dex_gain: 0.0,
            int_gain: 0.0,
            gain_scale: 1.0
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Skills {
    pub skills: HashMap<u8, Skill>,
}
