import test from 'node:test';
import assert from 'node:assert/strict';
import { editionConfig, editionFeatures } from './build_edition.mjs';
test('Free has only small ASR, notices, no speaker runtime or summary sidecar', () => {
  const c = editionConfig('free');
  assert.deepEqual(c.bundle.resources, ['resources/THIRD_PARTY_NOTICES.txt', 'resources/models/ggml-small-q5_1.bin']);
  assert.deepEqual(c.bundle.externalBin, []);
  assert(!editionFeatures('free').includes('diarize'));
});
test('Pro contains all models and engine; variants preserve local data identity', () => {
  const c = editionConfig('pro');
  assert(c.bundle.resources.some(x => x.endsWith('.gguf')));
  assert(c.bundle.resources.some(x => x.endsWith('.onnx')));
  assert.equal(c.bundle.externalBin.length, 1);
  assert.equal(c.identifier, editionConfig('free').identifier);
  assert.notEqual(c.build.frontendDist, editionConfig('free').build.frontendDist);
  assert(editionFeatures('pro').includes('diarize'));
});
test('unknown edition fails closed', () => assert.throws(() => editionConfig('paid')));
