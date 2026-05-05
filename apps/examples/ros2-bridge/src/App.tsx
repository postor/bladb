import { TENANT_ID, UID, type GatewaySessionState } from "@bladb/client";
import { useGatewaySession, useLiveValue, useMutation, useQuery } from "@bladb/react";
import { useEffect, useState } from "react";
import {
  db,
  ros2Api,
  subscribeRos2Topic,
  type Ros2Message,
  type Ros2Session
} from "./bladb";

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
  const auth = useGatewaySession<Ros2Session>(db.user);

  if (!auth.ready && !auth.session) {
    return (
      <main className="page auth-page">
        <section className="hero">
          <p className="eyebrow">Robotics bridge demo</p>
          <h1>ROS2 Operator Console</h1>
          <p className="lede">Restoring robotics operator session...</p>
        </section>
      </main>
    );
  }

  if (!auth.session) {
    return <Ros2Auth auth={auth} />;
  }

  return <Ros2Dashboard onLogout={auth.logout} session={auth.session} />;
}

function Ros2Auth({
  auth
}: {
  auth: GatewaySessionState<Ros2Session>;
}) {
  const [loginEmail, setLoginEmail] = useState("operator@ros2.demo");
  const [loginPassword, setLoginPassword] = useState("demo123");
  const [registerName, setRegisterName] = useState("Robot Operator");
  const [registerEmail, setRegisterEmail] = useState("new-operator@ros2.demo");
  const [registerPassword, setRegisterPassword] = useState("demo123");
  const [error, setError] = useState<string | null>(null);

  const login = useMutation(async () => {
    return await auth.login({
      app: "ros2-bridge",
      email: loginEmail,
      password: loginPassword
    });
  });

  const register = useMutation(async () => {
    return await auth.register({
      app: "ros2-bridge",
      email: registerEmail,
      password: registerPassword,
      displayName: registerName
    });
  });

  const wrap = async (runner: () => Promise<unknown>) => {
    setError(null);
    try {
      await runner();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Unknown auth error");
    }
  };

  return (
    <main className="page auth-page">
      <section className="hero">
        <p className="eyebrow">Robotics bridge demo</p>
        <h1>ROS2 Operator Console</h1>
        <p className="lede">
          Login first, then publish and subscriber pages will stay tenant-scoped while still
          looking close to native ROS2 topic workflows.
        </p>
      </section>

      <section className="auth-grid">
        <article className="panel accent">
          <span className="label">Demo sign-in</span>
          <h2>Login</h2>
          <p className="muted">
            Seed account: <code>operator@ros2.demo</code> / <code>demo123</code>
          </p>
          <label className="field">
            <span>Email</span>
            <input onChange={(event) => setLoginEmail(event.target.value)} value={loginEmail} />
          </label>
          <label className="field">
            <span>Password</span>
            <input
              onChange={(event) => setLoginPassword(event.target.value)}
              type="password"
              value={loginPassword}
            />
          </label>
          <button className="primary" disabled={login.loading} onClick={() => void wrap(login.run)}>
            {login.loading ? "Signing in..." : "Login"}
          </button>
        </article>

        <article className="panel">
          <span className="label">New operator</span>
          <h2>Register</h2>
          <label className="field">
            <span>Display name</span>
            <input onChange={(event) => setRegisterName(event.target.value)} value={registerName} />
          </label>
          <label className="field">
            <span>Email</span>
            <input onChange={(event) => setRegisterEmail(event.target.value)} value={registerEmail} />
          </label>
          <label className="field">
            <span>Password</span>
            <input
              onChange={(event) => setRegisterPassword(event.target.value)}
              type="password"
              value={registerPassword}
            />
          </label>
          <button
            className="primary secondary"
            disabled={register.loading}
            onClick={() => void wrap(register.run)}
          >
            {register.loading ? "Creating..." : "Register"}
          </button>
        </article>
      </section>

      {error ? <p className="banner banner-error">{error}</p> : null}
    </main>
  );
}

function Ros2Dashboard({
  session,
  onLogout
}: {
  session: Ros2Session;
  onLogout: () => void;
}) {
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
      <section className="hero hero-row">
        <div>
          <p className="eyebrow">Robotics bridge demo</p>
          <h1>ROS2 Operator Console</h1>
          <p className="lede">
            Signed in as <strong>{session.user.displayName}</strong>. The browser can publish and
            read topic snapshots without direct wildcard broker access.
          </p>
        </div>
        <div className="session-card">
          <span className="label">Operator session</span>
          <strong>{session.user.email}</strong>
          <small>{session.user.uid}</small>
          <small>{session.user.tenantId}</small>
          <button className="ghost" onClick={onLogout}>
            Logout
          </button>
        </div>
      </section>

      <section className="stats">
        <article className="panel accent">
          <span className="label">Namespace</span>
          <p className="metric">{session.user.tenantId}</p>
          <p className="muted">Every ROS2 topic is bound to the current tenant namespace.</p>
        </article>

        <article className="panel">
          <span className="label">Active topic</span>
          <h2>{selectedTopic}</h2>
          <p className="muted">Robot-scoped topic history is filtered in Rust before it reaches the UI.</p>
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
          session={session}
        />
      ) : (
        <SubscribePage
          latestFromApp={latestFromApp.data}
          latestSnapshot={latestSnapshot.data}
          onTopicChange={setSelectedTopic}
          recentMessages={recentMessages.data ?? []}
          selectedTopic={selectedTopic}
          session={session}
        />
      )}
    </main>
  );
}

function PublishPage({
  defaultTopic,
  onTopicChange,
  session
}: {
  defaultTopic: string;
  onTopicChange: (topic: string) => void;
  session: Ros2Session;
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
        tenantId: session.user.tenantId,
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
          <li>`issuedBy` is stamped from the session, not trusted from browser input.</li>
          <li>Frontend only chooses robot, topic, message type, and payload content.</li>
        </ul>
      </article>
    </section>
  );
}

function SubscribePage({
  session,
  selectedTopic,
  onTopicChange,
  latestFromApp,
  latestSnapshot,
  recentMessages
}: {
  session: Ros2Session;
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
            <p>{streamMessage?.robotId ?? latestSnapshot?.robotId ?? "--"}</p>
          </div>
          <div>
            <small>Issued by</small>
            <p>{streamMessage?.issuedBy ?? latestSnapshot?.issuedBy ?? session.user.uid}</p>
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
