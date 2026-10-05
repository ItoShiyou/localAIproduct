import { test } from "node:test";
import assert from "node:assert/strict";
import { recordingQueue } from "../.queue-test/recordingQueue.js";

test("stop waits for ordered writes including the last partial audio chunk", async () => {
  const written = [];
  let release;
  const gate = new Promise((resolve) => { release = resolve; });
  const queue = recordingQueue(async (chunk) => {
    if (chunk === "first") await gate;
    written.push(chunk);
  });
  queue.push("first");
  queue.push("tail");
  let stopped = false;
  const done = queue.finish().then(() => { stopped = true; });
  await Promise.resolve();
  assert.equal(stopped, false);
  release();
  await done;
  assert.deepEqual(written, ["first", "tail"]);
  queue.push("after stop");
  await queue.finish();
  assert.equal(written.length, 2);
});

test("failed audio writes prevent successful stop and later writes", async () => {
  const written = [];
  const queue = recordingQueue(async (chunk) => {
    written.push(chunk);
    throw new Error("disk full");
  });
  queue.push("first");
  queue.push("tail");
  await assert.rejects(queue.finish(), /disk full/);
  assert.deepEqual(written, ["first"]);
});
