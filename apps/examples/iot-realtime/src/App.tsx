import { TENANT_ID, UID, key } from "@bladb/client";
import { useLiveValue, useMutation, useQuery } from "@bladb/react";
import { useState } from "react";
import { db } from "./bladb";

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
  const [selectedDeviceId, setSelectedDeviceId] = useState("device-001");

  const devices = useQuery(
    () =>
      db.mongo("devices").find<Device[]>({
        ownerUid: UID,
        tenantId: TENANT_ID
      }),
    []
  );

  const telemetry = useLiveValue(
    () =>
      db.mongo("telemetry_latest").findOne<TelemetryPoint>({
        ownerUid: UID,
        tenantId: TENANT_ID,
        deviceId: selectedDeviceId
      }),
    2000,
    [selectedDeviceId]
  );

  const activeCount = useLiveValue(
    () => db.redis.get<number>(key`iot:${TENANT_ID}:active-count`),
    2000,
    []
  );

  const reboot = useMutation(async () => {
    await db.redis.publish(
      key`iot:${TENANT_ID}:devices:${selectedDeviceId}:commands`,
      {
        action: "reboot",
        issuedBy: UID
      }
    );
  });

  return (
    <main className="page">
      <section className="hero">
        <p className="eyebrow">Realtime telemetry demo</p>
        <h1>IoT Control Room</h1>
        <p className="lede">
          Device lists come from Mongo, counters come from Redis, and tenant/user binding stays
          aligned with JWT through the same reserved values.
        </p>
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
          <p className="muted">Telemetry refreshes every 2 seconds in this initial skeleton.</p>
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
        </article>
      </section>
    </main>
  );
}
