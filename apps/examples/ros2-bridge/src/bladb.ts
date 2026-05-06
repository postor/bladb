import {
  appGet,
  appPost,
  createClient,
  createTypedAppClient,
  createBrowserAppModule,
  type GatewaySession
} from "@bladb/client";

export const BLADB_URL = import.meta.env.VITE_BLADB_URL ?? "http://localhost:8787";
export const ROS2_TOKEN_KEY = "bladb.ros2.token";
export const ROS2_SESSION_KEY = "bladb.ros2.session";

export type Ros2Session = GatewaySession;

export interface Ros2PublishInput {
  robotId: string;
  topicName: string;
  messageType: string;
  payload: Record<string, unknown>;
}

export interface Ros2PublishResult {
  published: boolean;
  messageId: string;
  robotId: string;
  topicName: string;
  fullTopic: string;
  messageType: string;
  issuedBy: string;
  createdAt: string;
}

export interface Ros2Message {
  id: string;
  robotId: string;
  topicName: string;
  fullTopic: string;
  messageType: string;
  payload: Record<string, unknown>;
  issuedBy: string;
  createdAt: string;
}

export interface Ros2SubscriptionHandle {
  close(): void;
}

const ros2Routes = {
  publishMessage: appPost<Ros2PublishInput, Ros2PublishResult>("messages"),
  recentMessages: appGet<[string], Ros2Message[]>((topicName) => `messages/${encodeURIComponent(topicName)}`),
  latestMessage: appGet<[string], Ros2Message | null>((topicName) => `messages/${encodeURIComponent(topicName)}/latest`)
};

export const ros2GuestDb = createClient({
  baseUrl: BLADB_URL,
  appAuth: "optional",
  executeAuth: "optional"
});

export const ros2Module = createBrowserAppModule({
  baseUrl: BLADB_URL,
  appName: "ros2-bridge",
  tokenKey: ROS2_TOKEN_KEY,
  sessionKey: ROS2_SESSION_KEY,
  routes: ros2Routes
});

export const ros2User = ros2Module.user;
export const ros2Auth = ros2Module.auth;
export const ros2Api = createTypedAppClient(ros2GuestDb.app("ros2-bridge"), ros2Routes);
export const db = ros2GuestDb;

export function subscribeRos2Topic(
  topicName: string,
  onMessage: (message: Ros2Message) => void
): Ros2SubscriptionHandle {
  const controller = new AbortController();
  void ros2GuestDb
    .app("ros2-bridge")
    .stream<Ros2Message>(`messages/${encodeURIComponent(topicName)}/stream`, {
      signal: controller.signal,
      onMessage
    })
    .catch((error) => {
      if (controller.signal.aborted) {
        return;
      }
      console.error("ros2 stream failed", error);
    });

  return {
    close() {
      controller.abort();
    }
  };
}
