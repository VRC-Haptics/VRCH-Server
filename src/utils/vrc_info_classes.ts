
/** A fully‑qualified OSC address path */
export type OscPath = string;

/** 64‑bit NTP timestamp encoded as milliseconds since epoch */
export type OscTime = number;

export interface OscColor { r: number; g: number; b: number; a: number; }

export interface OscMidiMessage {
  port: number;
  status: number;
  data1: number;
  data2: number;
}

export type OscArray = OscType[];

// Discriminated‑union representing any OSC payload value
export type OscType =
  | { tag: "Int";    value: number }
  | { tag: "Float";  value: number }
  | { tag: "String"; value: string }
  | { tag: "Blob";   value: Uint8Array }
  | { tag: "Time";   value: OscTime }
  | { tag: "Long";   value: bigint }
  | { tag: "Double"; value: number }
  | { tag: "Char";   value: string }         // length === 1
  | { tag: "Color";  value: OscColor }
  | { tag: "Midi";   value: OscMidiMessage }
  | { tag: "Bool";   value: boolean }
  | { tag: "Array";  value: OscArray }
  | { tag: "Nil" }
  | { tag: "Inf" };

// ──────────────────────────────────────────────────────────────
//  Enums that map 1‑to‑1 with Rust variants
// ──────────────────────────────────────────────────────────────

export enum OscAccessLevel {
  Refused   = 0, // no value associated
  OnlyRead  = 1, // value may only be retrieved
  OnlyWrite = 2, // value may only be set
  Full      = 3, // value may be both retrieved and set
}

export enum NodeGroup {
  Head             = "Head",
  UpperArmRight    = "UpperArmRight",
  UpperArmLeft     = "UpperArmLeft",
  LowerArmRight    = "LowerArmRight",
  LowerArmLeft     = "LowerArmLeft",
  TorsoRight       = "TorsoRight",
  TorsoLeft        = "TorsoLeft",
  TorsoFront       = "TorsoFront",
  TorsoBack        = "TorsoBack",
  UpperLegRight    = "UpperLegRight",
  UpperLegLeft     = "UpperLegLeft",
  LowerLegRight    = "LowerLegRight",
  LowerLegLeft     = "LowerLegLeft",
  FootRight        = "FootRight",
  FootLeft         = "FootLeft",
  All              = "All", // meta‑tag, server‑side only
}

export enum TargetBone {
  Hips            = "Hips",
  LeftUpperLeg    = "LeftUpperLeg",
  RightUpperLeg   = "RightUpperLeg",
  LeftLowerLeg    = "LeftLowerLeg",
  RightLowerLeg   = "RightLowerLeg",
  // …add remaining variants from Rust TargetBone
}

// ──────────────────────────────────────────────────────────────
//  Complex structures
// ──────────────────────────────────────────────────────────────

export interface OscInfo {
  full_path: OscPath;
  access: OscAccessLevel;
  value?: OscType[] | null;
  description?: string | null;
}

export interface ProtoTimestamp {
  secs_since_epoch: number;
  nanos_since_epoch: number;
}

export interface CacheValue {
  /** The actual OSC payload value */
  value: OscType;
  /** Epoch‑milliseconds when we received the value */
  timestamp: ProtoTimestamp;
}

export interface CacheNode {
  /** Ring buffer – newest first */
  values: CacheValue[];
  osc_type: OscType;      // accepts this type only
  max_len: number;        // maximum ring‑buffer length
  smoothing_time: number; // ms of smoothing window
  position_weight: number;
  velocity_weight: number;
}

export interface HapticNode {
  /** Standard location in x (meters) */
  x: number;
  /** Standard location in y (meters) */
  y: number;
  /** Standard location in z (meters) */
  z: number;
  /** NodeGroups this node should influence or take influence from */
  groups: NodeGroup[];
}

export interface ConfNode {
  node_data: HapticNode;
  address: string;
  is_external_address: boolean;
  radius: number;
  target_bone: TargetBone;
}

export interface ConfMetadata {
  map_name: string;
  map_version: number;
  map_author: string;
}


/**
 * Filled with values from a config json file.
 * Provides all information needed to fully define the avatar prefab.
 */
export interface GameMap {
  nodes: ConfNode[];
  meta: ConfMetadata;
}


/**
 * The avatar referred to by the VRC API.
 * Rust: id: String, prefab_names: Vec<String>, configs: Vec<GameMap>
 */
export interface Avatar {
  /** Avatar ID from the VRC API */
  id: string;
  /** Names of the prefabs from the avatar parameter */
  prefab_names: string[];
  /** All information mapping OSC Parameters to their needed formats */
  configs: GameMap[];
}

export interface VrcInfo {
  /** Whether we are currently connected to a VRChat client */
  vrc_connected: boolean;
  /** Port we receive low‑latency OSC data on */
  in_port?: number;
  /** Port we are sending data over */
  out_port?: number;
  /** Data about the currently loaded avatar */
  avatar?: Avatar | null;
  /** Parameters VRC advertises as available */
  available_parameters: Record<OscPath, OscInfo>;
  /** Buffer with values collected from the OSC stream */
  parameter_cache: Record<OscPath, CacheNode>;
  /** Number of value entries to keep around for each parameter_cache entry */
  cache_length: number;
  //** How much weight distance has, 1-`dist_weight` = the velocity weight */
  dist_weight: number;
  /** Multiplies all velocity by this number. */
  vel_multiplier: number;
}

// ──────────────────────────────────────────────────────────────
//  Reasonable defaults used while React context is loading
// ──────────────────────────────────────────────────────────────

export const defaultVrcInfo: VrcInfo = {
  vrc_connected: false,
  in_port: undefined,
  out_port: undefined,
  avatar: null,
  available_parameters: {},
  parameter_cache: {},
  cache_length: 0,
  dist_weight: 0,
  vel_multiplier: 1.0,
};