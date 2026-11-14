import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
import { runInNewContext } from "node:vm";

const script = readFileSync(new URL("../web/app.js", import.meta.url), "utf8");
const tunnelId = "018f47d2-2d9f-7a41-a2aa-1aef7d847001";
const key = "a".repeat(64);

async function claimAttempts(fragment) {
  const nodes = new Map();
  const node = (name) => {
    if (!nodes.has(name)) nodes.set(name, {
      hidden: false,
      disabled: true,
      textContent: "",
      addEventListener() {},
      setAttribute() {},
      removeAttribute() {},
    });
    return nodes.get(name);
  };
  let requests = 0;
  runInNewContext(script, {
    globalThis: { __FTNL_CONFIG__: { apiOrigin: "https://api.example" } },
    location: { pathname: `/t/${tunnelId}`, hash: fragment, search: "" },
    history: { replaceState() {} },
    sessionStorage: { getItem() { return null; }, setItem() {} },
    document: { querySelector: node },
    Headers,
    fetch: async () => {
      requests += 1;
      return { ok: true, json: async () => ({ phone_capability: "capability" }) };
    },
  });
  await setImmediate();
  return { requests, filesEnabled: node("#files").disabled === false };
}

test("only a single canonical capability fragment can trigger claim", async () => {
  const valid = await claimAttempts(`#c=${key}`);
  assert.equal(valid.requests, 1);
  assert.equal(valid.filesEnabled, true);
  for (const invalid of [
    "", "#c=short", `#c=${key}&c=${key}`,
    `#c=${key}&`, `#unused=1&c=${key}`,
    `#%63=${key}`, `#c=${key}%00`,
  ]) {
    const outcome = await claimAttempts(invalid);
    assert.equal(outcome.requests, 0, "ambiguous or malformed fragment must not be redeemed");
    assert.equal(outcome.filesEnabled, false);
  }
});
