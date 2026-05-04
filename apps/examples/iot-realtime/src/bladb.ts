import { createClient } from "@bladb/client";

export const db = createClient({
  baseUrl: import.meta.env.VITE_BLADB_URL ?? "http://localhost:8787",
  getToken: () => window.localStorage.getItem("bladb.token") ?? undefined
});
