import { test } from 'node:test';
import assert from 'node:assert/strict';
import { finalizeRecording } from '../.queue-test/finalizeRecording.js';

const detail = (state, hasAudio, recording = false) => ({ meeting: { state, hasAudio, recording } });
test('normal stop returns without inspecting a fallback', async () => {
  const saved = detail('queued', true);
  assert.equal(await finalizeRecording(async () => saved, async () => { throw new Error('must not inspect'); }), saved);
});
test('recognition failure opens confirmed saved audio without erasing failure state', async () => {
  const saved = detail('failed', true);
  saved.meeting.error = 'recognition failed';
  assert.equal(await finalizeRecording(async () => { throw new Error('recognition failed'); }, async () => saved), saved);
  assert.equal(saved.meeting.error, 'recognition failed');
});
test('missing audio, active recording and inspection errors preserve the stop error', async () => {
  const original = new Error('disk full');
  for (const value of [detail('failed', false), detail('failed', true, true), detail('done', true)]) {
    await assert.rejects(finalizeRecording(async () => { throw original; }, async () => value), error => error === original);
  }
  await assert.rejects(finalizeRecording(async () => { throw original; }, async () => { throw new Error('database error'); }), error => error === original);
});
