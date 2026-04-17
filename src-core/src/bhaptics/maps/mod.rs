use x40_vest::{x40_vest_back, x40_vest_front};
use x6_head::x6_headset;

use std::time::Duration;

use super::game::network::event_map::{
    AudioFilePattern, HapticMapping, PatternLine, PatternLocation,
};
use crate::mapping::event::Event;
/// Contains all the index -> position matricies for bhaptics devices.
use crate::{mapping::event::EventEffectType, util::math::Vec3};

pub mod x40_vest;
pub mod x6_head;

/// Describes the indices on recieved f
pub struct BhapticsDevicePositions {
    pub name: String,
    /// maps indices from bhaptics events into node locations.
    pub rows: Vec<Vec3>,
}

/// Get the location of an index of a bhaptics device.
pub fn to_position(device: PatternLocation, index: usize) -> Vec3 {
    match device {
        PatternLocation::VestBack => x40_vest_back().rows[index],
        PatternLocation::VestFront => x40_vest_front().rows[index],
        PatternLocation::Head => x6_headset().rows[index],
        PatternLocation::Unknown => {
            log::error!("Unknown pattern location!");
            return Vec3::new(0., 0., 0.);
        }
        _ => {
            log::trace!("Unimplemented pattern location!");
            return Vec3::new(0., 0., 0.);
        }
    }
}

/// Turns a Haptic Pattern into a batch of events with a shared duration.
pub fn pattern_to_events(mapping: HapticMapping) -> Vec<Event> {
    let name = mapping.key.clone();
    let tags = vec!["Bhaptics".to_string(), format!("Bhaptics_{}", name)];

    let audio_patterns = build_audio_pattern(mapping.audio_file_patterns, name, tags);

    // only return audio patterns for now.
    return audio_patterns;
}

fn build_audio_pattern(
    patterns: Vec<AudioFilePattern>,
    name: String,
    tags: Vec<String>,
) -> Vec<Event> {
    let mut audio_events: Vec<Event> = Vec::new();

    // all clips inside all patterns
    for audio_pattern in patterns {
        for (location, pattern_lines) in audio_pattern.clip.patterns {
            let dur = Duration::from_millis(audio_pattern.clip.duration as u64);
            let motor_steps = convert_to_steps(&location, &pattern_lines);

            // itterate over each motor for this location.
            for (index, steps) in motor_steps.iter().enumerate() {
                if let Some(motor_id) = location.to_id(index) {
                    let effect = EventEffectType::SingleNode(motor_id);

                    if let Ok(event) =
                        Event::new(name.clone(), effect, steps.to_vec(), dur, tags.clone())
                    {
                        audio_events.push(event);
                    } else {
                        log::error!("Couldn't add event: {:?}:{:?}", name, location);
                    }
                } // `motor_steps` should not exceed `location.motor_count()`.
            }
        }
    }

    return audio_events;
}

/// Returns nested vector [device_motor_index][time_step] = value @ timestamp for a device motor.
fn convert_to_steps(loc: &PatternLocation, lines: &[PatternLine]) -> Vec<Vec<f32>> {
    let motors = loc.motor_count();
    if motors == 0 {
        return Vec::new();
    }

    let steps = lines.len();
    // matrix[motor_index][time_step]
    let mut matrix = vec![vec![0.0f32; steps]; motors];

    for (t, line) in lines.iter().enumerate() {
        for (m, &byte) in line.0.iter().enumerate() {
            if m < motors {
                // be defensive
                matrix[m][t] = byte as f32 / 125.0;
            }
        }
    }

    matrix
}
