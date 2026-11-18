import { describe, expect, it } from "vitest";
import {
  environmentKey,
  parseEnvironmentKey,
  safeEnvironmentKind,
} from "./web-env-collab";

describe("shared environments", () => {
  it("keeps environment and variable names distinct", () => {
    expect(
      parseEnvironmentKey(environmentKey("production", "PUBLIC_API_URL")),
    ).toEqual({ name: "production", key: "PUBLIC_API_URL" });
    expect(environmentKey("staging", "API_URL")).not.toBe(
      environmentKey("production", "API_URL"),
    );
  });
  it("rejects malformed or unsafe keys", () => {
    expect(() => environmentKey("production\x1fother", "TOKEN")).toThrow();
    expect(parseEnvironmentKey("production\x1fTOKEN\x1fextra")).toBeNull();
    expect(safeEnvironmentKind("secret")).toBeNull();
    expect(safeEnvironmentKind("custom")).toBe("custom");
  });
});
