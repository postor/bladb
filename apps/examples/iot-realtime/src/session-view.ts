export type IotScreenState = "restoring" | "login" | "dashboard";

export function resolveIotScreenState(options: {
  ready: boolean;
  session: unknown | null;
}): IotScreenState {
  if (!options.ready) {
    return "restoring";
  }

  return options.session ? "dashboard" : "login";
}
