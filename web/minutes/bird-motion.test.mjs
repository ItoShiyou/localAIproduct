import { test } from 'node:test';
import assert from 'node:assert/strict';
import { MOTIONS, createMotionPlayer } from './bird-motion.mjs';

function fixture() {
  const pending = new Map();
  const rendered = [];
  let id = 0;
  const player = createMotionPlayer({
    render: (pose, motion) => rendered.push([pose, motion]),
    schedule: callback => { pending.set(++id, callback); return id; },
    cancel: timer => pending.delete(timer),
  });
  return { player, pending, rendered };
}
test('six bounded motions use only existing poses and ordered frame times', () => {
  assert.equal(Object.keys(MOTIONS).length, 6);
  for (const motion of Object.values(MOTIONS)) {
    assert.ok(motion.duration > 0 && motion.duration <= 2000);
    let previous = -1;
    for (const [time, pose] of motion.frames) {
      assert.ok(time > previous && time < motion.duration);
      assert.ok(['still', 'wave', 'blink', 'tilt'].includes(pose));
      previous = time;
    }
  }
});
test('motion stop clears frames and restores the original', () => {
  const f = fixture();
  f.player.play('wave');
  f.player.stop();
  assert.equal(f.pending.size, 0);
  assert.deepEqual(f.rendered.at(-1), ['still', 'rest']);
});
test('motion is not scheduled when reduced/offscreen/hidden gate denies it', () => {
  const f = fixture();
  assert.equal(f.player.play('wave', () => false), false);
  assert.equal(f.pending.size, 0);
  assert.deepEqual(f.rendered.at(-1), ['still', 'rest']);
});
test('visibility change during a sequence cancels later frames', () => {
  const f = fixture();
  let allowed = true;
  f.player.play('wave', () => allowed);
  allowed = false;
  [...f.pending.values()][0]();
  assert.equal(f.pending.size, 0);
  assert.deepEqual(f.rendered.at(-1), ['still', 'rest']);
});
test('a previous cancelled sequence cannot overwrite a newer motion', () => {
  const f = fixture();
  f.player.play('wave');
  const stale = [...f.pending.values()][0];
  f.player.play('nod');
  stale();
  assert.deepEqual(f.rendered.at(-1), ['still', 'nod']);
});
test('unknown names are safe and completion returns to the original', () => {
  const f = fixture();
  assert.equal(f.player.play('toString'), false);
  let done = false;
  f.player.play('blink', () => true, () => { done = true; });
  [...f.pending.values()].forEach(callback => callback());
  assert.ok(done);
  assert.equal(f.pending.size, 0);
  assert.deepEqual(f.rendered.at(-1), ['still', 'rest']);
});
