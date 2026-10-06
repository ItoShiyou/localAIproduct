import { test } from "node:test";
import assert from "node:assert/strict";
import { findTranscript } from "../.queue-test/transcriptSearch.js";
const segments = [
  { id: 1, speaker: "田中", text: "予算は３０００円です。" },
  { id: 2, speaker: "佐藤", text: "次回は4000円、PDFを用意します。" },
];
test("empty query preserves order and source records", () => {
  assert.equal(findTranscript(segments, "　 "), segments);
});
test("normalizes full width, case, and requires all literal terms", () => {
  assert.deepEqual(findTranscript(segments, "佐藤 pdf").map(x => x.id), [2]);
  assert.deepEqual(findTranscript(segments, "3000").map(x => x.id), [1]);
  assert.deepEqual(findTranscript(segments, "佐藤 3000"), []);
});
test("regex input is literal and unmatched evidence is not invented", () => {
  assert.deepEqual(findTranscript(segments, ".*"), []);
  assert.deepEqual(findTranscript(segments, "予算を承認した"), []);
  assert.deepEqual(segments.map(x => x.id), [1, 2]);
});
