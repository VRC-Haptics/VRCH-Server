import { DeviceInfo } from "../bindings";


export function getDeviceId(info: DeviceInfo): string {
  switch (info.variant) {
    case "Wifi": return info.value.mac;
    case "BhapticBle": return info.value.id;
    case "Websocket": return info.value.id;
    case "Internal": return "Internal:"+ info.value.nodes.length.toString();
  }
}

export function getDeviceName(info: DeviceInfo): string {
  switch (info.variant) {
    case "Wifi": return info.value.name;
    case "BhapticBle": return info.value.model;
    case "Websocket": return info.value.name;
    case "Internal": return "Internal:"+ info.value.nodes.length.toString();
  }
}
