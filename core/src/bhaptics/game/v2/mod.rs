// v2/mod.rs

use std::{
    collections::HashMap,
    io,
    net::SocketAddr,
    time::Duration,
};

use super::network::event_map::PatternLocation;
use crate::{
    log_err,
    mapping::{
        InputEventMessage, MapHandle, NodeGroup, NodeId, event::{Event, EventEffectType}, haptic_node::HapticNode, input_node::{InputNode, InputType}
    }, util::math::Vec3,
};
use strum::IntoEnumIterator;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_websockets::Message;

const V2_TAG: &str = "Bhaptics_V2";

// ─── Server ──────────────────────────────────────────────────────────

pub async fn run_server(map: MapHandle, token: CancellationToken) {
    if let Err(e) = run_server_inner(map, token).await {
        log::error!("bHaptics V2 server error: {:?}", e);
    }
}

async fn run_server_inner(map: MapHandle, token: CancellationToken) -> io::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 15881));
    let listener = TcpListener::bind(&addr).await?;
    log::info!("bHaptics V2 API server started on {}", addr);

    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let map = map.clone();
                        let child = token.child_token();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, map, child).await {
                                log::error!("V2 connection error: {:?}", e);
                            }
                        });
                    }
                    Err(e) => log::error!("V2 accept error: {:?}", e),
                }
            }
        }
    }

    log::info!("bHaptics V2 listener terminated.");
    Ok(())
}

// ─── Connection ──────────────────────────────────────────────────────

struct ConnectionState {
    app_id: String,
    app_name: String,
    /// Registered patterns keyed by their string key.
    registered: HashMap<String, serde_json::Value>,
    /// Keys currently playing.
    active_keys: Vec<String>,
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    map: MapHandle,
    token: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (request, ws_stream) = tokio_websockets::ServerBuilder::new()
        .accept(stream)
        .await?;

    let uri = request.uri().to_string();
    let (app_id, app_name) = parse_query_params(&uri);

    log::info!("V2 WebSocket connection: app_id={}, app_name={}", app_id, app_name);

    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_write.send(msg).await.is_err() {
                break;
            }
        }
    });

    insert_bhaptics_maps(&map).await;

    let mut state = ConnectionState {
        app_id,
        app_name,
        registered: HashMap::new(),
        active_keys: Vec::new(),
    };

    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            frame = ws_read.next() => {
                match frame {
                    Some(Ok(msg)) if msg.is_text() => {
                        let raw = msg.as_text().expect("checked is_text");
                        handle_player_request(raw, &mut state, &map, &tx).await;
                    }
                    Some(Ok(msg)) if msg.is_ping() || msg.is_pong() => {}
                    Some(Ok(_)) => log::warn!("V2: non-text message received"),
                    Some(Err(e)) => {
                        log::error!("V2 WebSocket error: {:?}", e);
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    remove_bhaptics_maps(&map).await;
    log::info!("V2 connection closed");
    Ok(())
}

fn parse_query_params(uri: &str) -> (String, String) {
    let mut app_id = String::new();
    let mut app_name = String::new();

    if let Some(query) = uri.split('?').nth(1) {
        for pair in query.split('&') {
            let mut kv = pair.splitn(2, '=');
            match (kv.next(), kv.next()) {
                (Some("app_id"), Some(v)) => {
                    app_id = urlencoding::decode(v).unwrap_or_default().into_owned()
                }
                (Some("app_name"), Some(v)) => {
                    app_name = urlencoding::decode(v).unwrap_or_default().into_owned()
                }
                _ => {}
            }
        }
    }

    (app_id, app_name)
}

// ─── Request handling ────────────────────────────────────────────────

async fn handle_player_request(
    raw: &str,
    state: &mut ConnectionState,
    map: &MapHandle,
    ws_tx: &mpsc::UnboundedSender<Message>,
) {
    let request: PlayerRequest = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(e) => {
            log::error!("V2 decode error: {} | raw: {:?}", e, raw);
            return;
        }
    };

    for reg in request.register {
        log::trace!("V2: Registered pattern: {}", reg.key);
        state.registered.insert(reg.key, reg.project);
    }

    for submit in request.submit {
        match submit.submit_type.as_str() {
            "key" => handle_submit_key(&submit, state, map).await,
            "frame" => handle_submit_frame(&submit, state, map).await,
            "turnOff" => handle_turn_off(&submit.key, state, map).await,
            "turnOffAll" => handle_turn_off_all(state, map).await,
            other => log::warn!("V2: Unknown submit type: {}", other),
        }
    }

    let response = build_response(state);
    if let Ok(json) = serde_json::to_string(&response) {
        let _ = ws_tx.send(Message::text(json));
    }
}

// ─── Submit: key (registered pattern) ────────────────────────────────

async fn handle_submit_key(
    submit: &SubmitRequest,
    state: &mut ConnectionState,
    map: &MapHandle,
) {
    let Some(project_json) = state.registered.get(&submit.key).cloned() else {
        log::warn!("V2: Unregistered key: {}", submit.key);
        return;
    };

    let scale = extract_scale_option(&submit.parameters);
    let rotation = extract_rotation_option(&submit.parameters);

    let events = project_to_events(
        &project_json,
        &submit.key,
        scale,
        rotation,
    );

    if !events.is_empty() {
        for event in &events {
            log::debug!(
                "V2: Event '{}' effect={:?} steps={:?} duration={:?}",
                event.name, event.effect, event.steps, event.duration
            );
        }
        log_err!(map.send_event(InputEventMessage::StartEvents(events)).await);
        state.active_keys.push(submit.key.clone());
    }
}

fn extract_scale_option(params: &Option<serde_json::Value>) -> ScaleOption {
    let Some(params) = params else {
        return ScaleOption::default();
    };
    ScaleOption {
        intensity: params
            .get("scaleOption")
            .and_then(|s| s.get("intensity"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32,
        duration: params
            .get("scaleOption")
            .and_then(|s| s.get("duration"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32,
    }
}

fn extract_rotation_option(params: &Option<serde_json::Value>) -> RotationOption {
    let Some(params) = params else {
        return RotationOption::default();
    };
    RotationOption {
        offset_angle_x: params
            .get("rotationOption")
            .and_then(|s| s.get("offsetAngleX"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32,
        offset_y: params
            .get("rotationOption")
            .and_then(|s| s.get("offsetY"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32,
    }
}

#[derive(Debug, Clone)]
struct ScaleOption {
    intensity: f32,
    duration: f32,
}

impl Default for ScaleOption {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            duration: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct RotationOption {
    offset_angle_x: f32,
    offset_y: f32,
}

// ─── Project → Events conversion ────────────────────────────────────

fn project_to_events(
    project_json: &serde_json::Value,
    key: &str,
    scale: ScaleOption,
    rotation: RotationOption,
) -> Vec<Event> {
    let mut events = Vec::new();

    let Some(tracks) = project_json.get("tracks").and_then(|t| t.as_array()) else {
        return events;
    };

    for (track_idx, track) in tracks.iter().enumerate() {
        let enabled = track.get("enable").and_then(|e| e.as_bool()).unwrap_or(true);
        if !enabled {
            continue;
        }

        let Some(effects) = track.get("effects").and_then(|e| e.as_array()) else {
            continue;
        };

        for (effect_idx, effect) in effects.iter().enumerate() {
            let start_time = effect
                .get("startTime")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let offset_time = effect
                .get("offsetTime")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;

            let Some(modes) = effect.get("modes") else {
                continue;
            };

            // modes is a map of position string → mode object
            // e.g. { "VestFront": { "mode": "...", "dotMode": {...}, "pathMode": {...} } }
            let Some(modes_obj) = modes.as_object() else {
                continue;
            };

            for (position_str, mode) in modes_obj {
                let Some(location) = position_str_to_pattern(position_str) else {
                    log::trace!("V2: Skipping unknown position '{}' in project", position_str);
                    continue;
                };

                let mode_type = mode
                    .get("mode")
                    .and_then(|m| m.as_str())
                    .unwrap_or("dotMode");

                match mode_type {
                    "dotMode" | "DOT_MODE" => {
                        if let Some(dot_mode) = mode.get("dotMode") {
                            let new = dot_mode_to_events(
                                dot_mode,
                                &location,
                                key,
                                track_idx,
                                effect_idx,
                                &scale,
                            );
                            events.extend(new);
                        }
                    }
                    "pathMode" | "PATH_MODE" => {
                        if let Some(path_mode) = mode.get("pathMode") {
                            let new = path_mode_to_events(
                                path_mode,
                                &location,
                                key,
                                track_idx,
                                effect_idx,
                                start_time,
                                offset_time,
                                &scale,
                                &rotation,
                            );
                            events.extend(new);
                        }
                    }
                    other => log::trace!("V2: Unknown mode type '{}'", other),
                }
            }
        }
    }

    events
}

fn dot_mode_to_events(
    dot_mode: &serde_json::Value,
    location: &PatternLocation,
    key: &str,
    track_idx: usize,
    effect_idx: usize,
    scale: &ScaleOption,
) -> Vec<Event> {
    let mut events = Vec::new();

    let Some(feedbacks) = dot_mode.get("feedback").and_then(|f| f.as_array()) else {
        return events;
    };

    for (fb_idx, feedback) in feedbacks.iter().enumerate() {
        let fb_start = feedback
            .get("startTime")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let fb_end = feedback
            .get("endTime")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let raw_duration_ms = (fb_end - fb_start).max(10) as f32 * scale.duration;
        let duration = Duration::from_millis(raw_duration_ms as u64);

        let Some(point_list) = feedback.get("pointList").and_then(|p| p.as_array()) else {
            continue;
        };

        // Group points by motor index so we get one event per motor with
        // intensity steps over time.
        let mut motor_points: HashMap<usize, Vec<(i32, f64)>> = HashMap::new();
        for point in point_list {
            let index = point.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let intensity = point.get("intensity").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let time = point.get("time").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            motor_points.entry(index).or_default().push((time, intensity));
        }

        for (motor_idx, mut points) in motor_points {
            let Some(node_id) = location.to_id(motor_idx) else {
                continue;
            };

            // Sort by time to build sequential steps.
            points.sort_by_key(|(t, _)| *t);

            let steps: Vec<f32> = points
                .iter()
                .map(|(_, intensity)| {
                    ((*intensity as f32) * scale.intensity).clamp(0.0, 1.0)
                })
                .collect();

            if steps.is_empty() {
                continue;
            }

            let name = format!("v2_{}_t{}_e{}_fb{}_m{}", key, track_idx, effect_idx, fb_idx, motor_idx);
            let tags = vec![V2_TAG.to_string(), key.to_string()];

            match Event::new(name, EventEffectType::SingleNode(node_id), steps, duration, tags) {
                Ok(event) => events.push(event),
                Err(e) => log::trace!("V2: Event creation failed: {:?}", e),
            }
        }
    }

    events
}

fn path_mode_to_events(
    path_mode: &serde_json::Value,
    location: &PatternLocation,
    key: &str,
    track_idx: usize,
    effect_idx: usize,
    start_time: i32,
    offset_time: i32,
    scale: &ScaleOption,
    rotation: &RotationOption,
) -> Vec<Event> {
    let mut events = Vec::new();

    let Some(feedbacks) = path_mode.get("feedback").and_then(|f| f.as_array()) else {
        return events;
    };

    for (fb_idx, feedback) in feedbacks.iter().enumerate() {
        let Some(point_list) = feedback.get("pointList").and_then(|p| p.as_array()) else {
            continue;
        };

        if point_list.is_empty() {
            continue;
        }

        // Collect all path points with their time/position/intensity.
        let mut path_entries: Vec<PathEntry> = Vec::new();
        for point in point_list {
            let x = point.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let y = point.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let intensity = point.get("intensity").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let time = point.get("time").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

            // Apply rotation offset: shift x by angle, clamp to [0,1].
            let rotated_x = (x + rotation.offset_angle_x / 360.0).rem_euclid(1.0);
            let shifted_y = (y + rotation.offset_y).clamp(0.0, 1.0);

            path_entries.push(PathEntry {
                x: rotated_x,
                y: shifted_y,
                intensity,
                time,
            });
        }

        path_entries.sort_by_key(|e| e.time);

        // Determine total duration from the time range of points.
        let time_min = path_entries.first().map(|e| e.time).unwrap_or(0);
        let time_max = path_entries.last().map(|e| e.time).unwrap_or(0);
        let raw_duration_ms = ((time_max - time_min).max(10) as f32 * scale.duration) as u64;
        let duration = Duration::from_millis(raw_duration_ms);

        // If points move over time, use MovingLocation. Otherwise, use single Location.
        let name = format!("v2_{}_t{}_e{}_pfb{}", key, track_idx, effect_idx, fb_idx);
        let tags = vec![V2_TAG.to_string(), key.to_string()];

        if path_entries.len() == 1 {
            let e = &path_entries[0];
            let pos = xy_to_vec3(location, e.x, e.y);
            let intensity = ((e.intensity) * scale.intensity).clamp(0.0, 1.0);

            match Event::new(name, EventEffectType::Location(pos), vec![intensity], duration, tags) {
                Ok(event) => events.push(event),
                Err(e) => log::trace!("V2: Path event creation failed: {:?}", e),
            }
        } else {
            let positions: Vec<Vec3> = path_entries
                .iter()
                .map(|e| xy_to_vec3(location, e.x, e.y))
                .collect();
            let steps: Vec<f32> = path_entries
                .iter()
                .map(|e| ((e.intensity) * scale.intensity).clamp(0.0, 1.0))
                .collect();

            match Event::new(name, EventEffectType::MovingLocation(positions), steps, duration, tags) {
                Ok(event) => events.push(event),
                Err(e) => log::trace!("V2: Moving path event creation failed: {:?}", e),
            }
        }
    }

    events
}

struct PathEntry {
    x: f32,
    y: f32,
    intensity: f32,
    time: i32,
}

/// Converts a normalized (x, y) surface coordinate into a Vec3 world position
/// by interpolating between known motor positions on the device.
///
/// Uses a weighted average of the nearest motors. Falls back to the device
/// center if no motors exist.
fn xy_to_vec3(location: &PatternLocation, x: f32, y: f32) -> Vec3 {
    let count = location.motor_count();
    if count == 0 {
        return Vec3::ZERO;
    }

    // Collect all motor positions with their normalized x/y coordinates.
    // PatternLocation.to_position(index) gives us the 3D position.
    // We use inverse-distance weighting against the motor grid.
    let mut total_weight = 0.0f32;
    let mut weighted_pos = Vec3::ZERO;

    for i in 0..count {
        let motor_pos = location.to_position(i);
        // We need to know the motor's own x/y in the normalized [0,1] space.
        // Derive it from the motor grid layout: index → (col, row).
        let (mx, my) = motor_index_to_xy(location, i);

        let dx = x - mx;
        let dy = y - my;
        let dist_sq = dx * dx + dy * dy;

        // If we land exactly on a motor, just return its position.
        if dist_sq < 1e-6 {
            return motor_pos;
        }

        // Inverse distance squared weighting for smooth falloff.
        let weight = 1.0 / dist_sq;
        weighted_pos.x += motor_pos.x * weight;
        weighted_pos.y += motor_pos.y * weight;
        weighted_pos.z += motor_pos.z * weight;
        total_weight += weight;
    }

    if total_weight > 0.0 {
        weighted_pos.x /= total_weight;
        weighted_pos.y /= total_weight;
        weighted_pos.z /= total_weight;
    }

    weighted_pos
}

/// Maps a motor index to a normalized (x, y) coordinate on the device surface.
///
/// bHaptics devices use a grid layout:
///   - Vest front/back: 4 columns × 5 rows (20 motors)
///   - Arms: 3 columns × 2 rows (6 motors)
///   - Head: 6 columns × 1 row (6 motors, wrapped)
///   - Hands: 3 columns × 2 rows (6 motors)
///   - Feet: 3 columns × 1 row (3 motors)
///
/// Motors are indexed left-to-right, top-to-bottom.
fn motor_index_to_xy(location: &PatternLocation, index: usize) -> (f32, f32) {
    let (cols, rows) = grid_dimensions(location);

    let col = index % cols;
    let row = index / cols;

    let x = if cols <= 1 {
        0.5
    } else {
        col as f32 / (cols - 1) as f32
    };

    let y = if rows <= 1 {
        0.5
    } else {
        row as f32 / (rows - 1) as f32
    };

    (x, y)
}

/// Returns (columns, rows) for each device type's motor grid.
fn grid_dimensions(location: &PatternLocation) -> (usize, usize) {
    match location {
        PatternLocation::VestFront | PatternLocation::VestBack => (4, 5),
        PatternLocation::ForearmL | PatternLocation::ForearmR => (3, 2),
        PatternLocation::Head => (6, 1),
        PatternLocation::HandL | PatternLocation::HandR => (3, 2),
        PatternLocation::FootL | PatternLocation::FootR => (3, 1),
        // Fallback: single point
        _ => (1, 1),
    }
}

// ─── Submit: frame (direct motor control) ────────────────────────────

async fn handle_submit_frame(
    submit: &SubmitRequest,
    state: &mut ConnectionState,
    map: &MapHandle,
) {
    let Some(ref frame) = submit.frame else {
        log::warn!("V2: Frame submit with no frame data");
        return;
    };

    let Some(location) = position_str_to_pattern(&frame.position) else {
        log::warn!("V2: Unknown position: {}", frame.position);
        return;
    };

    let duration = Duration::from_millis(frame.duration_millis.max(10) as u64);
    let mut events = Vec::new();

    for dot in &frame.dot_points {
        if let Some(node_id) = location.to_id(dot.index) {
            let intensity = (dot.intensity as f32).clamp(0.0, 1.0);
            let name = format!("v2_dot_{}_{}", submit.key, dot.index);
            let tags = vec![V2_TAG.to_string(), submit.key.clone()];

            match Event::new(name, EventEffectType::SingleNode(node_id), vec![intensity], duration, tags) {
                Ok(event) => events.push(event),
                Err(e) => log::trace!("V2: Dot event error: {:?}", e),
            }
        }
    }

    for (i, path) in frame.path_points.iter().enumerate() {
        let pos = xy_to_vec3(&location, path.x, path.y);
        let intensity = (path.intensity as f32).clamp(0.0, 1.0);
        let name = format!("v2_path_{}_{}", submit.key, i);
        let tags = vec![V2_TAG.to_string(), submit.key.clone()];

        match Event::new(name, EventEffectType::Location(pos), vec![intensity], duration, tags) {
            Ok(event) => events.push(event),
            Err(e) => log::trace!("V2: Path event error: {:?}", e),
        }
    }

    if !events.is_empty() {
        log_err!(map.send_event(InputEventMessage::StartEvents(events)).await);
        state.active_keys.push(submit.key.clone());
    }
}

// ─── Turn off ────────────────────────────────────────────────────────

async fn handle_turn_off(key: &str, state: &mut ConnectionState, map: &MapHandle) {
    log_err!(
        map.send_event(InputEventMessage::CancelAllWithTags(vec![key.to_string()]))
            .await
    );
    state.active_keys.retain(|k| k != key);
}

async fn handle_turn_off_all(state: &mut ConnectionState, map: &MapHandle) {
    log_err!(
        map.send_event(InputEventMessage::CancelAllWithTags(vec![V2_TAG.to_string()]))
            .await
    );
    state.active_keys.clear();
}

// ─── Response ────────────────────────────────────────────────────────

fn build_response(state: &ConnectionState) -> PlayerResponse {
    let connected: Vec<String> = PatternLocation::iter()
        .filter_map(|loc| pattern_to_position_str(loc))
        .collect();

    PlayerResponse {
        registered_keys: state.registered.keys().cloned().collect(),
        active_keys: state.active_keys.clone(),
        connected_device_count: connected.len() as i32,
        connected_positions: connected,
        status: HashMap::new(),
    }
}

// ─── Node management ─────────────────────────────────────────────────

async fn insert_bhaptics_maps(map: &MapHandle) {
    for loc in PatternLocation::iter() {
        for index in 0..loc.motor_count() {
            let pos = loc.to_position(index);
            let node = HapticNode {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                groups: vec![NodeGroup::All],
            };
            let tags = vec![V2_TAG.to_string(), loc.to_input_tag().to_string()];
            if let Some(id) = loc.to_id(index) {
                let input = InputNode::new(node, tags, id, 0.1, InputType::ADDITIVE);
                log_err!(map.send_event(InputEventMessage::InsertNode(input)).await);
            }
        }
    }
}

async fn remove_bhaptics_maps(map: &MapHandle) {
    log_err!(
        map.send_event(InputEventMessage::RemoveWithTags(vec![V2_TAG.to_string()]))
            .await
    );
}

// ─── Position mapping ────────────────────────────────────────────────

fn position_str_to_pattern(pos: &str) -> Option<PatternLocation> {
    match pos {
        "VestFront" => Some(PatternLocation::VestFront),
        "VestBack" => Some(PatternLocation::VestBack),
        "Vest" => Some(PatternLocation::VestFront),
        "ForearmL" => Some(PatternLocation::ForearmL),
        "ForearmR" => Some(PatternLocation::ForearmR),
        "HandL" | "GloveLeft" => Some(PatternLocation::HandL),
        "HandR" | "GloveRight" => Some(PatternLocation::HandR),
        "FootL" => Some(PatternLocation::FootL),
        "FootR" => Some(PatternLocation::FootR),
        "Head" => Some(PatternLocation::Head),
        _ => None,
    }
}

fn pattern_to_position_str(loc: PatternLocation) -> Option<String> {
    let s = match loc {
        PatternLocation::VestFront => "VestFront",
        PatternLocation::VestBack => "VestBack",
        PatternLocation::ForearmL => "ForearmL",
        PatternLocation::ForearmR => "ForearmR",
        PatternLocation::HandL => "GloveLeft",
        PatternLocation::HandR => "GloveRight",
        PatternLocation::FootL => "FootL",
        PatternLocation::FootR => "FootR",
        PatternLocation::Head => "Head",
        _ => return None,
    };
    Some(s.to_string())
}

// ─── Protocol types ──────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlayerRequest {
    #[serde(default)]
    register: Vec<RegisterRequest>,
    #[serde(default)]
    submit: Vec<SubmitRequest>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RegisterRequest {
    key: String,
    project: serde_json::Value,
}

#[derive(Debug, serde::Deserialize)]
struct SubmitRequest {
    #[serde(rename = "type")]
    submit_type: String,
    #[serde(default)]
    key: String,
    #[serde(rename = "Parameters")]
    parameters: Option<serde_json::Value>,
    #[serde(rename = "Frame")]
    frame: Option<Frame>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    duration_millis: u32,
    position: String,
    #[serde(default)]
    path_points: Vec<PathPoint>,
    #[serde(default)]
    dot_points: Vec<DotPoint>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DotPoint {
    index: usize,
    intensity: i32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathPoint {
    x: f32,
    y: f32,
    intensity: i32,
    #[serde(default = "default_motor_count")]
    motor_count: i32,
}

fn default_motor_count() -> i32 {
    3
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct PlayerResponse {
    registered_keys: Vec<String>,
    active_keys: Vec<String>,
    connected_device_count: i32,
    connected_positions: Vec<String>,
    status: HashMap<String, Vec<i32>>,
}