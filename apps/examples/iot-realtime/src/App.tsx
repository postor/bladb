import { TENANT_ID, UID, key } from "@bladb/client";
import { type GatewaySessionState, useGatewaySession, useMutation, useQuery, useLiveValue } from "@bladb/react";
import { useState } from "react";
import { db, iotApi, iotAuth, type CommandHistoryEntry, type IotSession } from "./bladb";

interface Device {
  id: string;
  name: string;
  status: "online" | "offline";
}

interface TelemetryPoint {
  deviceId: string;
  throughput: number;
  temp: number;
  ts: string;
}

export default function App() {
  const auth = useGatewaySession<IotSession>(iotAuth);

  if (!auth.ready && !auth.session) {
    return (
      <main className="page auth-page">
        <section className="hero">
          <p className="eyebrow">Realtime telemetry demo</p>
          <h1>IoT Control Room</h1>
          <p className="lede">Restoring operator session...</p>
        </section>
      </main>
    );
  }

  if (!auth.session) {
    return <IotAuth auth={auth} />;
  }

  return (
    <IotDashboard
      onLogout={auth.logout}
      session={auth.session}
    />
  );
}

function IotAuth({
  auth
}: {
  auth: GatewaySessionState<IotSession>;
}) {
  const [loginEmail, setLoginEmail] = useState("operator@iot.demo");
  const [loginPassword, setLoginPassword] = useState("demo123");
  const [registerName, setRegisterName] = useState("Plant Operator");
  const [registerEmail, setRegisterEmail] = useState("new-operator@iot.demo");
  const [registerPassword, setRegisterPassword] = useState("demo123");
  const [error, setError] = useState<string | null>(null);

  const login = useMutation(async () => {
    return await auth.login({
      app: "iot-realtime",
      email: loginEmail,
      password: loginPassword
    });
  });

  const register = useMutation(async () => {
    return await auth.register({
      app: "iot-realtime",
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
        <p className="eyebrow">Realtime telemetry demo</p>
        <h1>IoT Control Room</h1>
        <p className="lede">
          Login first, then the dashboard will hydrate tenant-scoped Mongo, Redis, and MQTT flows
          with the current operator identity.
        </p>
      </section>

      <section className="auth-grid">
        <article className="panel accent">
          <span className="label">Demo sign-in</span>
          <h2>Login</h2>
          <p className="muted">
            Seed account: <code>operator@iot.demo</code> / <code>demo123</code>
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

function IotDashboard({
  session,
  onLogout
}: {
  session: IotSession;
  onLogout: () => void;
}) {
  const [selectedDeviceId, setSelectedDeviceId] = useState("device-001");

  const devices = useQuery(
    () =>
      db
        .withMeta({
          resource: "devices.listMine",
          policy: "iot.devices.list-mine"
        })
        .mongo("devices")
        .find<Device[]>({
          ownerUid: UID,
          tenantId: TENANT_ID
        }),
    []
  );

  const telemetry = useLiveValue(
    () =>
      db
        .withMeta({
          resource: "telemetry.readLatest",
          policy: "iot.telemetry.read-latest",
          params: {
            deviceId: selectedDeviceId
          }
        })
        .mongo("telemetry_latest")
        .findOne<TelemetryPoint>({
          ownerUid: UID,
          tenantId: TENANT_ID,
          deviceId: selectedDeviceId
        }),
    2000,
    [selectedDeviceId]
  );

  const activeCount = useLiveValue(
    () =>
      db
        .withMeta({
          resource: "iot.activeCount",
          policy: "iot.active-count.read"
        })
        .redis.get<number>(key`iot:${TENANT_ID}:active-count`),
    2000,
    []
  );

  const commands = useLiveValue(() => iotApi.commandHistory(), 2500, []);

  const reboot = useMutation(async () => {
    await iotApi.publishCommand({
      deviceId: selectedDeviceId,
      action: "reboot"
    });
    await telemetry.refresh();
    await commands.refresh();
    return {
      selectedDeviceId
    };
  });

  return (
    <main className="page">
      <section className="hero hero-row">
        <div>
          <p className="eyebrow">Realtime telemetry demo</p>
          <h1>IoT Control Room</h1>
          <p className="lede">
            Signed in as <strong>{session.user.displayName}</strong>. Device reads and command
            command flows now run under the current operator session.
          </p>
        </div>
        <div className="session-card">
          <span className="label">Operator session</span>
          <strong>{session.user.email}</strong>
          <small>{session.user.uid}</small>
          <button className="ghost" onClick={onLogout}>
            Logout
          </button>
        </div>
      </section>

      <section className="stats">
        <article className="panel accent">
          <span className="label">Active devices</span>
          <p className="metric">{activeCount.data ?? "--"}</p>
          <p className="muted">Tenant scoped counter sourced from Redis.</p>
        </article>

        <article className="panel">
          <span className="label">Selected device</span>
          <h2>{selectedDeviceId}</h2>
          <p className="muted">{session.user.tenantId}</p>
        </article>
      </section>

      <section className="layout">
        <article className="panel">
          <span className="label">My devices</span>
          <div className="device-list">
            {(devices.data ?? []).map((device) => (
              <button
                className={device.id === selectedDeviceId ? "device selected" : "device"}
                key={device.id}
                onClick={() => setSelectedDeviceId(device.id)}
                type="button"
              >
                <strong>{device.name}</strong>
                <span>{device.status}</span>
              </button>
            ))}
          </div>
        </article>

        <article className="panel">
          <span className="label">Live telemetry</span>
          <div className="telemetry-card">
            <div>
              <small>Throughput</small>
              <p>{telemetry.data?.throughput ?? "--"} msg/s</p>
            </div>
            <div>
              <small>Temperature</small>
              <p>{telemetry.data?.temp ?? "--"} C</p>
            </div>
            <div>
              <small>Timestamp</small>
              <p>{telemetry.data?.ts ?? "--"}</p>
            </div>
          </div>
          <button className="primary" disabled={reboot.loading} onClick={() => void reboot.run()}>
            {reboot.loading ? "Sending..." : "Reboot device"}
          </button>
          {reboot.data ? <p className="banner">Command queued for {reboot.data.selectedDeviceId}.</p> : null}
        </article>
      </section>

      <section className="panel">
        <span className="label">Recent commands</span>
        <div className="table">
          {(commands.data ?? []).map((command) => (
            <div className="row" key={command.id}>
              <strong>{command.id}</strong>
              <span>{command.deviceId}</span>
              <span>{command.action}</span>
              <time>{command.createdAt}</time>
            </div>
          ))}
          {!commands.loading && (commands.data ?? []).length === 0 ? (
            <p className="muted">No command history yet.</p>
          ) : null}
        </div>
      </section>
    </main>
  );
}
