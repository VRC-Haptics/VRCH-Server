use x40_vest::{x40_vest_back, x40_vest_front};
use x6_head::x6_headset;

use std::sync::Arc;
use glam::Vec3;

use super::game::network::event_map::{
    AudioFilePattern, HapticMapping, PatternLine, PatternLocation,
};
use crate::mapping::event::{Event, Frames, MaybeConst, Steps};
/// Contains all the index -> position matricies for bhaptics devices.

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
            Vec3::new(0., 0., 0.)
        }
        _ => {
            log::trace!("Unimplemented pattern location!");
            Vec3::new(0., 0., 0.)
        }
    }
}

/// Turns a Haptic Pattern into a batch of events with a shared duration.
pub fn pattern_to_event(mapping: HapticMapping) -> Option<Event> {
    let name = mapping.key.clone();
    let duration = mapping.event_time / 10; // 10ms per event tick. 
    let frames = build_frames(mapping.audio_file_patterns, duration);

    match Event::new(name.clone(), frames, duration as u64) {
        Ok(e) => Some(e),
        Err(e) => {
            log::error!("unable to create event: {name}: {e:?}");
            None
        }
    }
}

fn build_frames(
    patterns: Vec<AudioFilePattern>,
    duration: u32,
) -> Frames {
    let mut nodes: Vec<Steps> = Vec::new();

    let mut ratios = Vec::new();
    let mut line_count = Vec::new();

    // Clips for all supported devices
    for audio_pattern in patterns {
        // down to which device this is for
        for (location, pattern_lines) in audio_pattern.clip.patterns {
            //audio_pattern.clip.duration // time this pattern should take (ms)

            // down to each motor 
            for (mtr_idx, line) in pattern_lines.iter().enumerate() {
                let loc = location.to_position(mtr_idx);

                let values: Vec<f32> = line.0.iter().map(|byte| {
                    *byte as f32 / 125.0
                }).collect(); // scale byte to 1.0 -0.0 float

                let scaling = audio_pattern.clip.duration / values.len() as u32;
                line_count.push(values.len());
                ratios.push(scaling);

                let epsilon = 0.001f32;
                if match values.first() { // all constant (this motor isn't targeted)
                    Some(&first) => values.iter().all(|&x| (x - first).abs() <= epsilon),
                    None => true,
                } {
                    nodes.push(Steps {
                        position: MaybeConst::new_const(loc),
                        muted: MaybeConst::new_const(false),
                        value: MaybeConst::new_const(*values.first().unwrap_or(&0.0f32)),
                        weight: MaybeConst::Const(1.0),
                        radius: MaybeConst::Const(0.0375f32)
                    });
                } else {
                    nodes.push(Steps {
                        position: MaybeConst::new_const(loc),
                        muted: MaybeConst::new_const(false),
                        value: MaybeConst::Var(Arc::new(*values.as_array().unwrap_or(&[]))),
                        weight: MaybeConst::Const(1.0),
                        radius: MaybeConst::Const(0.0375f32)
                    });
                }
            }
        }
    }

    log::trace!("Clip: {:?} ms/frame", ratios);
    log::trace!("Scaling: {:?}", duration / (line_count.iter().sum::<usize>() / line_count.len()) as u32);

    Frames { nodes}
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
