// Haptic‑related type definitions shared between the Tauri backend (Rust) and the
// frontend (TypeScript). These mirror the Rust structs/enums in `global_map.rs`.
//
//  • Numeric fields that represent `Duration` in Rust are **milliseconds** here.
//  • `SystemTime` is transferred as an ISO‑8601 string or `null`.
//  • The `DashMap<Id, InputNode>` becomes a plain record keyed by the string `Id`.

/* ──────────────────────────────────────────────── Shared aliases ───────── */

export type Id = string;

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

// The Rust code references `NodeGroup` but doesn’t define it in the excerpt.
// Represent it as a string label for now. Adjust if you introduce a richer type.
export type NodeGroup = string;

/* ─────────────────────────────────────────────── Haptic node ───────────── */

export interface HapticNode {
  /** Standard Location in x (metres) */
  x: number;
  /** Standard Location in y (metres) */
  y: number;
  /** Standard Location in z (metres) */
  z: number;
  /** The NodeGroups this node influences or is influenced by */
  groups: NodeGroup[];
}

/* ─────────────────────────────────────────── Event effect type ─────────── */

// Discriminated union mirroring the Rust enum `EventEffectType`.
export type EventEffectType =
  | { type: "SingleNode"; node: Id }
  | { type: "MultipleNodes"; nodes: Id[] }
  | { type: "Tags"; tags: string[] }
  | { type: "Location"; position: Vec3 }
  | { type: "MovingLocation"; path: Vec3[] };

/* ──────────────────────────────────────────────── Event ────────────────── */

export interface Event {
  /** User‑facing name */
  name: string;
  /** How this event affects the input map */
  effect: EventEffectType;
  /** Output intensities over time (unit‑less) */
  steps: number[];
  /** Total duration (ms). Steps are distributed across this span. */
  duration: number;
  /** Tags inserted into every node created by this event */
  tags: string[];
  /** Nodes managed by this event */
  managed_nodes: Id[];
  /** Time between steps (ms) */
  time_step: number;
  /** How many steps have completed so far */
  steps_completed: number;
  /** Event start time as ISO string, or null if not started */
  start_time: string | null;
}

/* ────────────────────────────────────────────── Input node ─────────────── */

export interface InputNode {
  /** Unique identifier */
  id: Id;

  /** Physical / logical location metadata */
  haptic_node: HapticNode;

  /** Feedback strength at this location */
  intensity: number;

  /** radius of influence this input node ahs */
  radius: number;

  /** Arbitrary user‑supplied labels */
  tags: string[];
}

/* ───────────────────────────────────────────── Standard menu ───────────── */

export interface StandardMenu {
  /** Global intensity multiplier set by the user */
  intensity: number;

  /** Flat enable/disable for all haptics */
  enable: boolean;
}

/* ────────────────────────────────────────────── Global map ─────────────── */

export interface GlobalMap {
  /** All active time‑based haptic events */
  active_events: Event[];

  /** Indexed collection of input nodes */
  input_nodes: Record<Id, InputNode>;

  /** Global tuning values */
  standard_menu: StandardMenu;
}

/* ─────────────────────────────────────────── Default instance ──────────── */

export const GlobalMapDefault: GlobalMap = {
  active_events: [],
  input_nodes: {},
  standard_menu: {
    intensity: 1.0,
    enable: true,
  },
};
