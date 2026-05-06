import { TENANT_ID, UID } from "@bladb/client";
import { useLiveValue, useMutation, useQuery } from "@bladb/react";
import { useEffect, useState } from "react";
import {
  db,
  ros2Api,
  subscribeRos2Topic,
  type Ros2Message
} from "./bladb";
import { ExampleSuiteNav } from "../../shared/ExampleSuiteNav";

type RouteTab = "publish" | "subscribe";

interface LatestSnapshot {
  robotId: string;
  topicName: string;
  messageType: string;
  payload: Record<string, unknown>;
  createdAt: string;
  issuedBy: string;
}

export default function App() {
  return <Ros2Dashboard />;
}

function Ros2Dashboard() {
  const [tab, setTab] = useState<RouteTab>("publish");
  const [selectedTopic, setSelectedTopic] = useState("cmd_vel");

  const recentMessages = useLiveValue(() => ros2Api.recentMessages(selectedTopic), 1500, [selectedTopic]);
  const latestFromApp = useLiveValue(() => ros2Api.latestMessage(selectedTopic), 1500, [selectedTopic]);
  const latestSnapshot = useQuery(
    () =>
      db
        .withMeta({
          resource: "ros2.topic.latest",
          policy: "ros2.topic.read-latest",
          params: {
            topicName: selectedTopic
          }
        })
        .mongo("ros2_messages_latest")
        .findOne<LatestSnapshot>({
          tenantId: TENANT_ID,
          topicName: selectedTopic
        }),
    [selectedTopic]
  );

  return (
    <main className="page">
      <ExampleSuiteNav currentApp="ros2-bridge" />

      <section className="hero hero-row">
        <div>
          <p className="eyebrow">Robotics bridge demo</p>
          <h1>ROS2 Operator Console</h1>
          <p className="lede">
            Anonymous example mode is enabled. The browser can publish and read tenant-scoped ROS2
            topic snapshots without stopping at a login screen.
          </p>
          <div className="hero-notes">
            <article className="hero-note">
              <span>Publish path</span>
              <strong>`POST /apps/ros2-bridge/messages`</strong>
              <p>The browser sends a topic intent and payload while the backend owns tenant prefixing and actor stamping.</p>
            </article>
            <article className="hero-note">
              <span>Read path</span>
              <strong>`/apps/ros2-bridge/*` surfaces</strong>
              <p>Latest snapshot, recent history, and live subscribe updates stay on app-owned routes instead of raw bridge access.</p>
            </article>
          </div>
        </div>
        <div className="session-card">
          <span className="label">Example identity</span>
          <strong>Robot Operator</strong>
          <small>operator@ros2.demo</small>
          <small>u_3001 / tenant_robotics</small>
        </div>
      </section>

      <section className="stats">
        <article className="panel accent">
          <span className="label">Namespace</span>
          <p className="metric">tenant_robotics</p>
          <p className="muted">Every ROS2 topic is bound to the current tenant namespace.</p>
        </article>

        <article className="panel">
          <span className="label">Active topic</span>
          <h2>{selectedTopic}</h2>
          <p className="muted">Robot-scoped topic history is filtered in Rust before it reaches the UI.</p>
        </article>
      </section>

      <section className="stats">
        <article className="panel">
          <span className="label">What this page demonstrates</span>
          <h2>Anonymous ROS2 bridge UX</h2>
          <p className="muted">
            Frontend teams can publish and inspect ROS2-style messages immediately, while the Rust
            bridge still controls tenant prefixing, topic allowlists, and actor stamping.
          </p>
        </article>

        <article className="panel">
          <span className="label">Module path</span>
          <h2>App API plus filtered subscribe stream</h2>
          <p className="muted">
            Publishing goes through `POST /apps/ros2-bridge/messages`. History, latest snapshot,
            and the realtime topic feed all stay on the app-owned `/apps/ros2-bridge/*` surface.
          </p>
        </article>

        <article className="panel panel-code">
          <span className="label">SDK shape</span>
          <h2>Frontend mental model</h2>
          <pre className="code-block">{`await ros2Api.publishMessage({ robotId, topicName, messageType, payload })
const latest = await ros2Api.latestMessage(topicName)
const recent = await ros2Api.recentMessages(topicName)
const stream = subscribeRos2Topic(topicName, onMessage)`}</pre>
        </article>

        <article className="panel">
          <span className="label">Backend ownership</span>
          <h2>Trusted bridge responsibilities</h2>
          <ul className="stack-list">
            <li>Tenant-scoped topic names are assembled by the backend, not by the browser alone.</li>
            <li>`issuedBy` and other trusted actor details come from runtime identity, not page input.</li>
            <li>Latest snapshot and stream routes stay filtered before they reach frontend consumers.</li>
          </ul>
        </article>
      </section>

      <section className="tabbar">
        <button className={tab === "publish" ? "tab selected" : "tab"} onClick={() => setTab("publish")} type="button">
          Publish Page
        </button>
        <button className={tab === "subscribe" ? "tab selected" : "tab"} onClick={() => setTab("subscribe")} type="button">
          Subscribe Page
        </button>
      </section>

      {tab === "publish" ? (
        <PublishPage
          defaultTopic={selectedTopic}
          onTopicChange={setSelectedTopic}
        />
      ) : (
        <SubscribePage
          latestFromApp={latestFromApp.data}
          latestSnapshot={latestSnapshot.data}
          onTopicChange={setSelectedTopic}
          recentMessages={recentMessages.data ?? []}
          selectedTopic={selectedTopic}
        />
      )}
    </main>
  );
}

function PublishPage({
  defaultTopic,
  onTopicChange
}: {
  defaultTopic: string;
  onTopicChange: (topic: string) => void;
}) {
  const [robotId, setRobotId] = useState("robot-001");
  const [topicName, setTopicName] = useState(defaultTopic);
  const [messageType, setMessageType] = useState("geometry_msgs/msg/Twist");
  const [linearX, setLinearX] = useState("0.45");
  const [angularZ, setAngularZ] = useState("0.20");

  const publish = useMutation(async () => {
    onTopicChange(topicName);
    return await ros2Api.publishMessage({
      robotId,
      topicName,
      messageType,
      payload: {
        linear: {
          x: Number(linearX),
          y: 0,
          z: 0
        },
        angular: {
          x: 0,
          y: 0,
          z: Number(angularZ)
        },
        tenantId: "tenant_robotics",
        issuedBy: UID
      }
    });
  });

  return (
    <section className="layout">
      <article className="panel">
        <span className="label">ROS2 publish</span>
        <h2>Publish native-looking topic messages</h2>
        <label className="field">
          <span>Robot ID</span>
          <input onChange={(event) => setRobotId(event.target.value)} value={robotId} />
        </label>
        <label className="field">
          <span>Topic name</span>
          <input onChange={(event) => setTopicName(event.target.value)} value={topicName} />
        </label>
        <label className="field">
          <span>Message type</span>
          <input onChange={(event) => setMessageType(event.target.value)} value={messageType} />
        </label>
        <div className="double-grid">
          <label className="field">
            <span>linear.x</span>
            <input onChange={(event) => setLinearX(event.target.value)} value={linearX} />
          </label>
          <label className="field">
            <span>angular.z</span>
            <input onChange={(event) => setAngularZ(event.target.value)} value={angularZ} />
          </label>
        </div>
        <button className="primary" disabled={publish.loading} onClick={() => void publish.run()}>
          {publish.loading ? "Publishing..." : "ros2 publish"}
        </button>
        {publish.data ? (
          <p className="banner">
            Published to <strong>{publish.data.fullTopic}</strong> as {publish.data.messageType}.
          </p>
        ) : null}
      </article>

      <article className="panel accent">
        <span className="label">Safe bridge</span>
        <h2>What the backend still controls</h2>
        <ul className="stack-list">
          <li>Topic path is always tenant-prefixed before it hits the ROS2 bridge.</li>
          <li>`issuedBy` is stamped from the trusted runtime identity, not browser input.</li>
          <li>Frontend only chooses robot, topic, message type, and payload content.</li>
        </ul>
      </article>
    </section>
  );
}

function SubscribePage({
  selectedTopic,
  onTopicChange,
  latestFromApp,
  latestSnapshot,
  recentMessages
}: {
  selectedTopic: string;
  onTopicChange: (topic: string) => void;
  latestFromApp: Ros2Message | null | undefined;
  latestSnapshot: LatestSnapshot | null;
  recentMessages: Ros2Message[];
}) {
  const [streamMessage, setStreamMessage] = useState<Ros2Message | null>(null);

  useEffect(() => {
    const subscription = subscribeRos2Topic(selectedTopic, (message) => {
      setStreamMessage(message);
    });
    return () => subscription.close();
  }, [selectedTopic]);

  return (
    <section className="layout">
      <article className="panel">
        <span className="label">ROS2 subscribe</span>
        <h2>Read filtered topic stream</h2>
        <label className="field">
          <span>Topic name</span>
          <input onChange={(event) => onTopicChange(event.target.value)} value={selectedTopic} />
        </label>
        <div className="telemetry-card">
          <div>
            <small>Live stream</small>
            <p>{streamMessage?.messageType ?? "--"}</p>
          </div>
          <div>
            <small>Latest robot</small>
            <p>{streamMessage?.robotId ?? latestFromApp?.robotId ?? latestSnapshot?.robotId ?? "--"}</p>
          </div>
          <div>
            <small>Issued by</small>
            <p>{streamMessage?.issuedBy ?? latestFromApp?.issuedBy ?? latestSnapshot?.issuedBy ?? "u_3001"}</p>
          </div>
        </div>
        <pre className="code-block">
          {JSON.stringify(streamMessage?.payload ?? latestFromApp?.payload ?? latestSnapshot?.payload ?? {}, null, 2)}
        </pre>
      </article>

      <article className="panel">
        <span className="label">Recent messages</span>
        <div className="table">
          {recentMessages.map((message) => (
            <div className="row column-row" key={message.id}>
              <strong>{message.topicName}</strong>
              <span>{message.robotId}</span>
              <span>{message.messageType}</span>
              <time>{message.createdAt}</time>
            </div>
          ))}
          {recentMessages.length === 0 ? <p className="muted">No messages for this topic yet.</p> : null}
        </div>
      </article>
    </section>
  );
}
