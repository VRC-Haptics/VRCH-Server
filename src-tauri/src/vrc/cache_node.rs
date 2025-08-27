use rosc::OscType;
use std::collections::VecDeque;
use std::mem::discriminant;
use std::time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH};

/// A node cached by vrc, allows for historically informed cache manipulation.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CacheNode {
    /// Ring buffer of values we have recieved.
    /// Front items are the most recent.
    values: VecDeque<(OscType, SystemTime)>,
    /// contains the OscType that this CacheNode accepts.
    /// The payload should be considered the default value if the cache is empty.
    osc_type: OscType,
    /// the max_len of entries we will keep track of.
    max_len: usize,
    /// The state of haptics returned from this node.
    smoothing_time: Duration,
    pub position_weight: f32,
    pub vel_mult: f32,
    contact_scale: f32,
}

impl CacheNode {
    /// Create a new CacheNode that blends position + smoothed-velocity.
    ///
    /// `smoothing_time` is how far back (in seconds) to average velocity.
    /// `position_weight` + `velocity_weight` should each be in [0,1] and sum to 1.0.
    pub fn new(
        value_type: OscType,
        max_entries: usize,
        smoothing_time: Duration,
        position_weight: f32,
        velocity_multiplier: f32,
        contact_scale: f32,
    ) -> CacheNode {
        let mut values = VecDeque::with_capacity(max_entries);
        values.push_front((value_type.clone(), UNIX_EPOCH));
        CacheNode {
            values,
            osc_type: value_type,
            max_len: max_entries,
            smoothing_time: smoothing_time,
            position_weight: position_weight,
            vel_mult: velocity_multiplier,
            contact_scale: contact_scale,
        }
    }

    pub fn set_velocity_mult(&mut self, val: f32) {
        self.vel_mult = val;
    }

    pub fn set_position_weight(&mut self, val: f32) {
        self.position_weight = val;
    }

    pub fn set_contact_scale(&mut self, val: f32) {
        self.contact_scale = val;
    }

    pub fn raw_last(&self) -> f32 {
        self.values.front().unwrap().0.clone().float().unwrap()
    }

    /// Returns the velocity interpreted latest value.
    pub fn latest_interp(&self) -> f32 {
        if self.values.len() < 2 {
            return self.values.front().unwrap().0.clone().float().unwrap();
        }

        let (latest_value, latest_time) = self.values.front().unwrap();
        let (old_value, old_time) = &self.values[1];
        // (percentage / second) * second = new delta
        let velocity = self.value_delta(latest_value, &old_value)
            / latest_time.duration_since(*old_time).unwrap().as_secs_f32();
        let seconds_since_last = SystemTime::now()
            .duration_since(*latest_time)
            .unwrap()
            .as_secs_f32();

        let interp = latest_value.clone().float().unwrap() + (velocity * seconds_since_last);
        if interp > 1. {
            return 1.;
        } else if interp < 0. {
            return 0.;
        }

        interp
    }

    /// Calculate average velocity from entries after `limit` timestamp.
    ///
    /// The delta between `limit` and now can be seen as a smoothing time.
    ///
    /// Units: [Change Value/Second]
    pub fn velocity_since(&self, limit: &SystemTime) -> f32 {
        let mut sum: f32 = 0.;
        let mut count: f32 = 0.;

        // itterate from newest to oldest.
        for (index, (val, time)) in self.values.iter().enumerate() {
            if *time > *limit {
                if let Some((older_val, older_time)) = self.values.get(index + 1) {
                    if *older_time > *limit {
                        count += 1.;
                        sum += self.value_delta(val, older_val)
                            / time.duration_since(*older_time).unwrap().as_secs_f32();
                    } else {
                        break;
                    }
                } else {
                    // No older value, stop
                    break;
                }
            } else {
                break;
            }
        }

        if count == 0.0 {
            return count;
        }
        return sum / count;
    }

    /// Returns the velocity between the latest value and `entries_back` into the cache.
    /// Units: [Change Value/Second]
    pub fn velocity_by_entry(&self, entries_back: usize) -> Result<f32, RetrievalError> {
        // retrieve values
        if let Some((val, time)) = self.values.back() {
            if let Some((val_late, time_late)) = self.values.get(entries_back) {
                // try to get time delta
                match time.duration_since(*time_late) {
                    Ok(dur) => {
                        return Ok(self.value_delta(val, val_late) / dur.as_secs_f32());
                    }
                    Err(err) => return Err(RetrievalError::TimeError(err)),
                };
            } else {
                return Err(RetrievalError::CacheTooSmall(
                    self.values.len(),
                    entries_back,
                ));
            }
        } else {
            return Err(RetrievalError::EmptyCache);
        }
    }

    /// Pushes an update to the cached values with the current time as a timestamp.
    pub fn update(&mut self, value: OscType) -> Result<(), WrongNodeTypeError> {
        if discriminant(&self.osc_type) != discriminant(&value) {
            return Err(WrongNodeTypeError {
                entered: value,
                expected: self.osc_type.clone(),
            });
        }
        if self.values.len() >= self.max_len {
            self.values.pop_back();
        }
        self.values.push_front((value, SystemTime::now()));

        return Ok(());
    }

    /// Returns the velocity and position mixed values
    pub fn latest(&self) -> f32 {
        let now = SystemTime::now();
        let limit = now.checked_sub(self.smoothing_time).unwrap_or(UNIX_EPOCH);

        // detect when we havent recieved the "closing zero value"
        // should stop buzzing after and having to reset.
        if let Some((latest, time)) = self.values.front() {
            let age_ms = now
                .duration_since(*time)
                .unwrap_or(Duration::new(0, 0))
                .as_millis();
            if latest.clone().float().unwrap() > 0.001 && age_ms > 200 {
                return 0.0;
            }
        }

        // pull current position
        let pos = self
            .values
            .front()
            .map(|(v, _)| v.clone().float().unwrap_or(0.0))
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);

        // compute smoothed absolute velocity
        let vel = self.velocity_since(&limit).abs().clamp(0.0, 1.0);

        // blend and clamp
        ((1.0 - self.position_weight) * (vel * self.vel_mult)
            + self.position_weight * pos * self.contact_scale)
            .clamp(0.0, 1.0)
    }

    /// Trys to parse OscType into a delta value in f32.
    /// uses the order: `value-val_late`.
    fn value_delta(&self, value: &OscType, val_late: &OscType) -> f32 {
        match value {
            OscType::Float(float) => float - val_late.clone().float().unwrap(),
            OscType::Int(int) => (int - val_late.clone().int().unwrap()) as f32,
            OscType::Double(double) => (double - val_late.clone().double().unwrap()) as f32,
            OscType::Long(long) => (long - val_late.clone().long().unwrap()) as f32,
            OscType::Bool(bool) => {
                if *bool == val_late.clone().bool().unwrap() {
                    0.0
                } else {
                    1.0
                }
            }
            _ => unimplemented!(),
        }
    }
}

/// The wrong node type was inserted into this node.
pub struct WrongNodeTypeError {
    entered: OscType,
    expected: OscType,
}

/// An Error occured during retrieval from cache.
pub enum RetrievalError {
    /// Value is requested but the cache is still empty
    EmptyCache,
    /// the cache does not contain enough entries: `(required: usize, requested:usize)`
    CacheTooSmall(usize, usize),
    /// Time calcualtions results in an error: (SystemTimeError)
    TimeError(SystemTimeError),
}
