let Titles = "text-2xl font-bold padding-5 text-center";

export class GitRepo {
  owner: string;
  name: string;

  constructor(owner: string, name: string) {
    this.name = name;
    this.owner = owner;
  }
}

export interface GaussianState {
  merge: number;
  at_edge: number;
}

export type InterpAlgo = {
  algo: "Gaussian";
  state: GaussianState;
}
// Represents the factors that modulate device output.
export interface OutputFactors {
  sens_mult: number;
  start_offset: number;
  interp_algo: InterpAlgo;
}

// The possible node groups (as string literals) corresponding to the Rust enum.
export type NodeGroup =
  | "Head"
  | "ArmRight"
  | "ArmLeft"
  | "TorsoRight"
  | "TorsoLeft"
  | "TorsoFront"
  | "TorsoBack"
  | "LegRight"
  | "LegLeft"
  | "FootRight"
  | "FootLeft";

// Represents a haptic node in space.
export interface HapticNode {
  x: number;
  y: number;
  z: number;
  groups: NodeGroup[];
}

// Represents the mapping configuration for a device.
export interface HapticMap {
  game_map: HapticNode[] | null;
  device_map: HapticNode[] | null;
  game_intensity: number[];
  last_sent: number[];
  falloff_distance: number;
  merge_distance: number;
  sigma: number;
}

export type ESP32Model =
  | "ESP32"
  | "ESP32S2"
  | "ESP32S2FH16"
  | "ESP32S2FH32"
  | "ESP32S3"
  | "ESP32C3"
  | "ESP32C2"
  | "ESP32C6"
  | "ESP8266"
  | "Unknown";

// Mirror of Rust’s `HapticNode` struct
export interface HapticNode {
  /** Standard Location in x (meters) */
  x: number;
  /** Standard Location in y (meters) */
  y: number;
  /** Standard Location in z (meters) */
  z: number;
  /** The NodeGroups this node should influence or take influence from */
  groups: NodeGroup[];
}

// Mirror of Rust’s `WifiConfig` struct
export interface WifiConfig {
  wifi_ssid: string;
  wifi_password: string;
  mdns_name: string;
  node_map: HapticNode[];
  i2c_scl: number;
  i2c_sda: number;
  i2c_speed: number;
  motor_map_i2c_num: number;
  motor_map_i2c: number[];
  motor_map_ledc_num: number;
  motor_map_ledc: number[];
  config_version: number;
}


export interface WifiConnManager {
  /// Port that WE recieve from the device on
  recv_port: number;
}


// Represents the Wifi device fields.
// Note: for SystemTime we use a string representation (e.g. ISO 8601) on the TS side.
export interface WifiDevice {
  mac: string;
  ip: string;
  name: string;
  been_pinged: boolean;
  push_map: boolean;
  last_queried: string;
  send_port: number;
  connection_manager: WifiConnManager;
  // tick_channel is #[serde(skip)]
  config: WifiConfig | null;
  identifier: ESP32Model | null;
  logs: string[];
  last_heartbeat: string;
}

// The DeviceType is currently an enum with a Wifi variant.
export type DeviceType = {
  variant: "Wifi";
  value: WifiDevice;
};

// This mirrors the Rust Device struct.
export interface Device {
  id: string;
  name: string;
  num_motors: number;
  is_alive: boolean;
  factors: OutputFactors;
  device_type: DeviceType;
}

export const defaultWifiConfig: WifiConfig = {
  wifi_ssid: '',
  wifi_password: '',
  mdns_name: 'my-device',
  node_map: [],
  i2c_scl: 21,
  i2c_sda: 22,
  i2c_speed: 100_000,
  motor_map_i2c_num: 0,
  motor_map_i2c: [],
  motor_map_ledc_num: 0,
  motor_map_ledc: [],
  config_version: 1,
};

export const defaultDevice: Device = {
  id: "",
  name: "",
  num_motors: 0,
  is_alive: false,
  factors: {
    sens_mult: 1.0,
    start_offset: 0.0,
    interp_algo: {
      algo: "Gaussian",
      state: {
        merge: 0.0,
        at_edge: 0.0,
      }
    }
  },
  device_type: {
    variant: "Wifi",
    value: {
      mac: "",
      ip: "",
      name: "",
      been_pinged: false,
      push_map: false,
      last_queried: "",
      send_port: 0,
      connection_manager: {
        recv_port: 0,
      },
      config: null,
      identifier: null,
      logs: [],
      last_heartbeat: "",
    },
  },
};
export default Titles;
