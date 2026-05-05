import {
  appGet,
  appPost,
  appStream,
  createClient,
  createTypedAppClient,
  createBrowserAppModule,
  type GatewaySession
} from "@bladb/client";

export const BLADB_URL = import.meta.env.VITE_BLADB_URL ?? "http://localhost:8787";
export const IOT_TOKEN_KEY = "bladb.iot.token";
export const IOT_SESSION_KEY = "bladb.iot.session";

export type IotSession = GatewaySession;

export interface CommandHistoryEntry {
  id: string;
  deviceId: string;
  topic: string;
  action: string;
  issuedBy: string;
  createdAt: string;
}

export interface PublishCommandInput {
  deviceId: string;
  action: string;
}

export interface PublishCommandResult extends CommandHistoryEntry {
  published: boolean;
  commandId: string;
}

export interface CommandEvent extends CommandHistoryEntry {
  event: "mqtt-message";
}

export interface Device {
  id: string;
  name: string;
  status: "online" | "offline";
}

export interface TelemetryPoint {
  deviceId: string;
  throughput: number;
  temp: number;
  ts: string;
}

const iotRoutes = {
  devices: appGet<Device[]>("devices"),
  telemetry: appGet<[deviceId: string], TelemetryPoint>((deviceId) => `telemetry/${deviceId}`),
  activeCount: appGet<number>("active-count"),
  commandHistory: appGet<CommandHistoryEntry[]>("commands"),
  publishCommand: appPost<PublishCommandInput, PublishCommandResult>("commands"),
  commandEvents: appStream<[deviceId: string], CommandEvent>(
    (deviceId) => `commands/${deviceId}/stream`
  )
};

export const iotGuestDb = createClient({
  baseUrl: BLADB_URL,
  appAuth: "optional",
  executeAuth: "optional"
});

export const iotModule = createBrowserAppModule({
  baseUrl: BLADB_URL,
  appName: "iot-realtime",
  tokenKey: IOT_TOKEN_KEY,
  sessionKey: IOT_SESSION_KEY,
  routes: iotRoutes
});

export const iotGuestApi = createTypedAppClient(iotGuestDb.app("iot-realtime"), iotRoutes);

export const db = iotModule.db;
export const iotUser = iotModule.user;
export const iotAuth = iotModule.auth;
export const iotApi = iotModule.api;
