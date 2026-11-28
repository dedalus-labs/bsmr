import { expect, test } from "vitest";

import { add } from "./index.js";

test("adds", () => {
  expect(add(1, 2)).toBe(3);
});

test("subtracts", () => {
  expect(add(1, 2)).toBe(-1);
});
