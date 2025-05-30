let Titles = "text-2xl font-bold padding-5 text-center";

export interface GaussianState {
  sigma: number;
  cutoff: number;
  merge: number;
}

export type InterpAlgo = {
  variant: "Gaussian";
  value: GaussianState;
}

// Represents the factors that modulate device output.
export interface OutputFactors {
  /// the lowest value that produces feedback
  start_offset: number;
  sens_mult: number;
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
  config: WifiConfig;
}


// Represents the Wifi device fields.
// Note: for SystemTime we use a string representation (e.g. ISO 8601) on the TS side.
export interface WifiDevice {
  mac: string;
  ip: string;
  name: string;
  been_pinged: boolean;
  last_queried: string;
  send_port: number;
  connection_manager: WifiConnManager;
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
  node_map: [] as HapticNode[],
  i2c_scl: 21,           // ESP32 default SCL
  i2c_sda: 22,           // ESP32 default SDA
  i2c_speed: 100_000,    // 100 kHz
  motor_map_i2c_num: 0,
  motor_map_i2c: [] as number[],
  motor_map_ledc_num: 0,
  motor_map_ledc: [] as number[],
  config_version: 1,
};

// A default instance for convenience.
export const defaultDevice: Device = {
  id: "",
  name: "",
  num_motors: 0,
  is_alive: false,
  factors: {
    sens_mult: 1.0,
    start_offset: 0.0,
    interp_algo: {
      variant: "Gaussian",
      value: {
        sigma: 0.0,
        cutoff: 0.0,
        merge: 0.0,
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
      last_queried: "",
      send_port: 0,
      connection_manager: {
        recv_port: 0,
        config: defaultWifiConfig,
      }
    },
  },
};

export default Titles;
