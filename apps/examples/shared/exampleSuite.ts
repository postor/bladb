const viteEnv = (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env;

export type ExampleSuiteId =
  | "examples-portal"
  | "flash-sale"
  | "blog"
  | "iot-realtime"
  | "ros2-bridge"
  | "user-module-demo";

export interface ExampleSuiteItem {
  id: ExampleSuiteId;
  title: string;
  mode: string;
  summary: string;
  url: string;
  stage: string;
  modules: string[];
  developerFocus: string;
}

const FALLBACK_URLS: Record<ExampleSuiteId, string> = {
  "examples-portal": "http://127.0.0.1:4172",
  "flash-sale": "http://127.0.0.1:4173",
  blog: "http://127.0.0.1:4174",
  "iot-realtime": "http://127.0.0.1:4175",
  "ros2-bridge": "http://127.0.0.1:4176",
  "user-module-demo": "http://127.0.0.1:4177",
};

function readSuiteUrl(key: string, fallback: string) {
  return viteEnv?.[key] ?? fallback;
}

export function getExampleSuite(): ExampleSuiteItem[] {
  return [
    {
      id: "examples-portal",
      title: "Examples Portal",
      mode: "Suite home",
      summary: "Resolved URLs, recommended tour, and seed credentials for the whole example stack.",
      url: readSuiteUrl("VITE_EXAMPLE_PORTAL_URL", FALLBACK_URLS["examples-portal"]),
      stage: "Start here",
      modules: ["suite", "gateway"],
      developerFocus: "Understand the stack layout, demo order, and which example to open next.",
    },
    {
      id: "flash-sale",
      title: "Flash Sale",
      mode: "Anonymous",
      summary: "Queue-first purchase flow with worker-settled order state.",
      url: readSuiteUrl("VITE_EXAMPLE_FLASH_SALE_URL", FALLBACK_URLS["flash-sale"]),
      stage: "Anonymous flow",
      modules: ["queue", "sql", "user"],
      developerFocus: "See how a business app can expose direct-entry APIs while still running through a seeded identity.",
    },
    {
      id: "blog",
      title: "Blog",
      mode: "Public + user",
      summary: "Public reads plus authenticated editor writes through db.user and db.mongo.",
      url: readSuiteUrl("VITE_EXAMPLE_BLOG_URL", FALLBACK_URLS.blog),
      stage: "Mixed auth",
      modules: ["user", "mongo"],
      developerFocus: "Learn the split between anonymous app reads and authenticated editor writes in one page.",
    },
    {
      id: "iot-realtime",
      title: "IoT Control Room",
      mode: "Anonymous",
      summary: "Tenant device reads, commands, and first-event realtime feedback.",
      url: readSuiteUrl("VITE_EXAMPLE_IOT_URL", FALLBACK_URLS["iot-realtime"]),
      stage: "Anonymous flow",
      modules: ["mqtt", "stream", "user"],
      developerFocus: "Inspect anonymous command publishing plus realtime delivery feedback without auth UI friction.",
    },
    {
      id: "ros2-bridge",
      title: "ROS2 Operator Console",
      mode: "Anonymous",
      summary: "Filtered publish and subscribe bridge for browser teams.",
      url: readSuiteUrl("VITE_EXAMPLE_ROS2_URL", FALLBACK_URLS["ros2-bridge"]),
      stage: "Anonymous flow",
      modules: ["mqtt", "stream", "ros2", "user"],
      developerFocus: "See how browser teams can test publish and subscribe flows on top of module-owned ROS2 routes.",
    },
    {
      id: "user-module-demo",
      title: "User Module Demo",
      mode: "Auth workbench",
      summary: "Directly verify db.user login, register, me, and logout.",
      url: readSuiteUrl(
        "VITE_EXAMPLE_USER_MODULE_DEMO_URL",
        FALLBACK_URLS["user-module-demo"],
      ),
      stage: "Contract check",
      modules: ["user"],
      developerFocus: "Validate the standalone db.user contract and see exactly what a frontend session flow should guarantee.",
    },
  ];
}
