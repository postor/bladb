import { BladbError } from "@bladb/client";
import { useMutation, useQuery, useLiveValue } from "@bladb/react";
import { useEffect, useState } from "react";
import {
  iotGuestApi,
  type CommandEvent,
  type PublishCommandResult
} from "./bladb";
import { ExampleSuiteNav } from "../../shared/ExampleSuiteNav";

const isAuthExpiredError = (error: unknown): error is BladbError =>
  error instanceof BladbError && error.status === 401 && error.code === "AUTH_EXPIRED";

export default function App() {
  return <IotDashboard />;
}

function IotDashboard() {
  const [selectedDeviceId, setSelectedDeviceId] = useState("device-001");
  const [latestEvent, setLatestEvent] = useState<CommandEvent | null>(null);
  const [streamState, setStreamState] = useState<"connecting" | "live" | "error">("connecting");

  const devices = useQuery(() => iotGuestApi.devices(), []);

  const telemetry = useLiveValue(() => iotGuestApi.telemetry(selectedDeviceId), 2000, [selectedDeviceId]);

  const activeCount = useLiveValue(() => iotGuestApi.activeCount(), 2000, []);

  const commands = useLiveValue(() => iotGuestApi.commandHistory(), 2500, []);

  useEffect(() => {
    const controller = new AbortController();
    setLatestEvent(null);
    setStreamState("connecting");

    void iotGuestApi
      .commandEvents(selectedDeviceId, {
        signal: controller.signal,
        onOpen() {
          setStreamState("live");
        },
        onMessage(payload) {
          setLatestEvent(payload);
          setStreamState("live");
          void commands.refresh();
        }
      })
      .catch((error) => {
        if (controller.signal.aborted) {
          return;
        }
        if (isAuthExpiredError(error)) {
          setStreamState("error");
          return;
        }
        console.error("iot mqtt stream failed", error);
        setStreamState("error");
      });

    return () => {
      controller.abort();
    };
  }, [selectedDeviceId]);

  useEffect(() => {
    if (
      isAuthExpiredError(devices.error) ||
      isAuthExpiredError(telemetry.error) ||
      isAuthExpiredError(activeCount.error) ||
      isAuthExpiredError(commands.error)
    ) {
      setStreamState("error");
    }
  }, [activeCount.error, commands.error, devices.error, telemetry.error]);

  const reboot = useMutation(async () => {
    const result = await iotGuestApi.publishCommand({
      deviceId: selectedDeviceId,
      action: "reboot"
    });
    await telemetry.refresh();
    await commands.refresh();
    return {
      selectedDeviceId,
      commandId: result.commandId
    };
  });

  return (
    <main className="page">
      <ExampleSuiteNav currentApp="iot-realtime" />

      <section className="hero hero-row">
        <div>
          <p className="eyebrow">Realtime telemetry demo</p>
          <h1>IoT Control Room</h1>
          <p className="lede">
            Anonymous example mode is enabled. Reads and command flows currently use the IoT
            runtime default identity so the page is directly usable without logging in.
          </p>
          <div className="hero-notes">
            <article className="hero-note">
              <span>Read path</span>
              <strong>`GET /apps/iot-realtime/devices`</strong>
              <p>The browser opens directly into tenant-scoped device and telemetry reads through app-owned routes.</p>
            </article>
            <article className="hero-note">
              <span>Command path</span>
              <strong>`POST /apps/iot-realtime/commands`</strong>
              <p>The browser chooses a device action while the backend owns MQTT topic generation and delivery shape.</p>
            </article>
          </div>
        </div>
        <div className="session-card">
          <span className="label">Example identity</span>
          <strong>operator@iot.demo</strong>
          <small>u_1001 / tenant_a</small>
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
          <p className="muted">tenant_a</p>
          <p className="muted">
            MQTT stream: {streamState === "live" ? "subscribed" : streamState === "connecting" ? "connecting..." : "reconnect needed"}
          </p>
        </article>
      </section>

      <section className="stats">
        <article className="panel">
          <span className="label">What this page demonstrates</span>
          <h2>Anonymous control room</h2>
          <p className="muted">
            This example opens directly, reads tenant-scoped device state, and sends allowed
            commands through the module app API while the backend owns MQTT topic generation.
          </p>
        </article>

        <article className="panel">
          <span className="label">Module path</span>
          <h2>Read state, emit commands, watch events</h2>
          <p className="muted">
            The browser reads devices and telemetry, publishes `reboot` through
            `POST /apps/iot-realtime/commands`, then waits for the first stream event from the
            module-owned realtime route.
          </p>
        </article>

        <article className="panel panel-code">
          <span className="label">SDK shape</span>
          <h2>Frontend mental model</h2>
          <pre className="code-block">{`const devices = await iotGuestApi.devices()
const telemetry = await iotGuestApi.telemetry(deviceId)
await iotGuestApi.publishCommand({ deviceId, action: "reboot" })
await iotGuestApi.commandEvents(deviceId, handlers)`}</pre>
        </article>

        <article className="panel">
          <span className="label">Backend ownership</span>
          <h2>What still stays trusted</h2>
          <ul className="stack-list">
            <li>Tenant identity and device scope come from trusted runtime configuration.</li>
            <li>MQTT topics are assembled server-side instead of trusting the browser to build them.</li>
            <li>The stream route exposes filtered delivery events rather than raw broker access.</li>
          </ul>
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
          <div className="telemetry-card">
            <div>
              <small>Last MQTT action</small>
              <p>{latestEvent?.action ?? "--"}</p>
            </div>
            <div>
              <small>Last MQTT topic</small>
              <p>{latestEvent?.topic ?? "--"}</p>
            </div>
            <div>
              <small>Delivered at</small>
              <p>{latestEvent?.createdAt ?? "--"}</p>
            </div>
          </div>
          <button className="primary" disabled={reboot.loading} onClick={() => void reboot.run()}>
            {reboot.loading ? "Sending..." : "Reboot device"}
          </button>
          {reboot.data ? (
            <p className="banner">
              Command queued for {reboot.data.selectedDeviceId}. Waiting for MQTT event {reboot.data.commandId}.
            </p>
          ) : null}
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
