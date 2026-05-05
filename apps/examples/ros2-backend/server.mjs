import http from "node:http";

const PORT = Number(process.env.PORT ?? 8080);
const tenantId = process.env.ROS2_TENANT_ID ?? "tenant_robotics";
const allowedTopics = (process.env.ROS2_ALLOWED_TOPICS ?? "cmd_vel,battery_state,odom")
  .split(",")
  .map((value) => value.trim())
  .filter(Boolean);

let nextMessageId = 3;
const robots = [
  {
    id: "robot-001",
    tenantId,
    ownerUid: "u_3001",
    name: "Warehouse AMR 01",
  },
];

const messages = [
  {
    id: "ros2msg_0001",
    tenantId,
    robotId: "robot-001",
    topicName: "cmd_vel",
    fullTopic: `tenant/${tenantId}/robots/robot-001/ros2/cmd_vel`,
    messageType: "geometry_msgs/msg/Twist",
    payload: {
      linear: { x: 0.3, y: 0, z: 0 },
      angular: { x: 0, y: 0, z: 0.1 },
    },
    issuedBy: "u_3001",
    createdAt: "2026-05-05T09:10:00Z",
  },
  {
    id: "ros2msg_0002",
    tenantId,
    robotId: "robot-001",
    topicName: "battery_state",
    fullTopic: `tenant/${tenantId}/robots/robot-001/ros2/battery_state`,
    messageType: "sensor_msgs/msg/BatteryState",
    payload: {
      percentage: 0.84,
      voltage: 24.6,
    },
    issuedBy: "u_3001",
    createdAt: "2026-05-05T09:11:00Z",
  },
];
const streamSubscribers = new Map();

function nowLabel() {
  return new Date().toISOString();
}

function publishStreamEvent(topicName, message) {
  const subscribers = streamSubscribers.get(topicName);
  if (!subscribers?.size) {
    return;
  }

  const frame = `event: ros2-message\ndata: ${JSON.stringify(message)}\n\n`;
  for (const res of [...subscribers]) {
    try {
      res.write(frame);
    } catch {
      subscribers.delete(res);
    }
  }
}

function sendJson(res, status, data) {
  res.writeHead(status, {
    "content-type": "application/json",
  });
  res.end(JSON.stringify(data));
}

function parseBody(req) {
  return new Promise((resolve, reject) => {
    let raw = "";
    req.on("data", (chunk) => {
      raw += chunk.toString();
    });
    req.on("end", () => {
      if (raw.length === 0) {
        resolve({});
        return;
      }

      try {
        resolve(JSON.parse(raw));
      } catch (error) {
        reject(error);
      }
    });
    req.on("error", reject);
  });
}

function matchRobot(robotId, ownerUid) {
  return robots.find((robot) => robot.id === robotId && robot.ownerUid === ownerUid && robot.tenantId === tenantId);
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);

  if (req.method === "GET" && url.pathname === "/health") {
    sendJson(res, 200, {
      ok: true,
      service: "ros2-backend",
      tenantId,
      allowedTopics,
    });
    return;
  }

  if (req.method === "POST" && url.pathname === "/messages") {
    let body;
    try {
      body = await parseBody(req);
    } catch {
      sendJson(res, 400, {
        ok: false,
        message: "invalid json body",
      });
      return;
    }

    const robotId = body.robotId;
    const topicName = body.topicName;
    const messageType = body.messageType;
    const payload = body.payload;
    const issuedBy = body.issuedBy ?? "u_3001";

    if (!robotId || !topicName || !messageType || payload === undefined) {
      sendJson(res, 400, {
        ok: false,
        message: "robotId, topicName, messageType, and payload are required",
      });
      return;
    }

    if (!allowedTopics.includes(topicName)) {
      sendJson(res, 400, {
        ok: false,
        message: `topic \`${topicName}\` is not allowed`,
      });
      return;
    }

    if (!matchRobot(robotId, issuedBy)) {
      sendJson(res, 404, {
        ok: false,
        message: "robot not found for current owner",
      });
      return;
    }

    const message = {
      id: `ros2msg_${String(nextMessageId).padStart(4, "0")}`,
      tenantId,
      robotId,
      topicName,
      fullTopic: `tenant/${tenantId}/robots/${robotId}/ros2/${topicName}`,
      messageType,
      payload,
      issuedBy,
      createdAt: nowLabel(),
    };
    nextMessageId += 1;
    messages.push(message);
    publishStreamEvent(topicName, message);

    sendJson(res, 200, {
      ok: true,
      data: {
        published: true,
        messageId: message.id,
        robotId: message.robotId,
        topicName: message.topicName,
        fullTopic: message.fullTopic,
        messageType: message.messageType,
        payload: message.payload,
        issuedBy: message.issuedBy,
        createdAt: message.createdAt,
      },
    });
    return;
  }

  if (req.method === "GET" && url.pathname.startsWith("/messages/") && url.pathname.endsWith("/stream")) {
    const parts = url.pathname.split("/").filter(Boolean);
    const topicName = decodeURIComponent(parts[1] ?? "");
    if (!allowedTopics.includes(topicName)) {
      sendJson(res, 400, {
        ok: false,
        message: `topic \`${topicName}\` is not allowed`
      });
      return;
    }

    res.writeHead(200, {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
      connection: "keep-alive"
    });
    res.write(": connected\n\n");

    const subscribers = streamSubscribers.get(topicName) ?? new Set();
    subscribers.add(res);
    streamSubscribers.set(topicName, subscribers);

    req.on("close", () => {
      const current = streamSubscribers.get(topicName);
      current?.delete(res);
      if (current && current.size === 0) {
        streamSubscribers.delete(topicName);
      }
    });
    return;
  }

  if (req.method === "GET" && url.pathname.startsWith("/messages/")) {
    const parts = url.pathname.split("/").filter(Boolean);
    const topicName = decodeURIComponent(parts[1] ?? "");
    const topicMessages = messages
      .filter((message) => message.tenantId === tenantId && message.topicName === topicName)
      .slice()
      .reverse();

    if (parts[2] === "latest") {
      sendJson(res, 200, {
        ok: true,
        data: topicMessages[0] ?? null,
      });
      return;
    }

    sendJson(res, 200, {
      ok: true,
      data: topicMessages.slice(0, 12),
    });
    return;
  }

  sendJson(res, 404, {
    ok: false,
    message: "not found",
  });
});

server.listen(PORT, "0.0.0.0", () => {
  console.log(`ros2-backend listening on ${PORT}`);
});
