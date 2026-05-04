import {
  appGet,
  appPost,
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

export const iotModule = createBrowserAppModule({
  baseUrl: BLADB_URL,
  appName: "iot-realtime",
  tokenKey: IOT_TOKEN_KEY,
  sessionKey: IOT_SESSION_KEY,
  routes: {
    commandHistory: appGet<CommandHistoryEntry[]>("commands"),
    publishCommand: appPost<PublishCommandInput, PublishCommandResult>("commands")
  }
});

export const db = iotModule.db;
export const iotAuth = iotModule.auth;
export const iotApi = iotModule.api;
