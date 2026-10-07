import { createHash } from 'node:crypto';
import { createReadStream, existsSync, mkdirSync, copyFileSync, statSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

const app = fileURLToPath(new URL('../', import.meta.url));
const summary = 'Qwen3-4B-Instruct-2507-Q4_K_M.gguf';
const digests = {
  'ggml-small-q5_1.bin': 'ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb',
  'ggml-large-v3-turbo-q5_0.bin': '394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2',
  'voxceleb_resnet34_LM.onnx': '7bb2f06e9df17cdf1ef14ee8a15ab08ed28e8d0ef5054ee135741560df2ec068',
  [summary]: '3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597',
};
export function editionConfig(edition) {
  if (!['free', 'pro'].includes(edition)) throw new Error('Select free or pro');
  const pro = edition === 'pro';
  return {
    productName: pro ? 'minutes Pro' : 'minutes Free',
    // Same identifier preserves local meetings when the user replaces Free with Pro.
    identifier: 'dev.localaiproduct.minutes',
    build: {
      frontendDist: `../ui/dist/${edition}`,
      beforeBuildCommand: { script: `npm run build -- --outDir dist/${edition}`, cwd: '../ui' },
    },
    bundle: {
      externalBin: pro ? ['binaries/minutes-summarizer'] : [],
      resources: ['resources/THIRD_PARTY_NOTICES.txt', 'resources/models/ggml-small-q5_1.bin',
        ...(pro ? ['resources/models/ggml-large-v3-turbo-q5_0.bin', 'resources/models/voxceleb_resnet34_LM.onnx',
          `resources/models/${summary}`, 'resources/onnxruntime/*'] : [])],
    },
  };
}
export function editionFeatures(edition) {
  editionConfig(edition);
  return ['tauri', 'whisper', `edition-${edition}`, ...(edition === 'pro' ? ['diarize'] : [])];
}
async function verify(path, digest) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  if (hash.digest('hex') !== digest) throw new Error(`Model hash mismatch: ${path}`);
}
async function main() {
  const edition = process.argv[2];
  const config = editionConfig(edition);
  const models = join(app, 'src-tauri/resources/models');
  for (const resource of config.bundle.resources) {
    const name = resource.split('/').at(-1);
    if (!digests[name]) continue;
    const path = join(models, name);
    if (!existsSync(path) && name === summary) {
      const source = process.env.MINUTES_BUNDLE_SUMMARY_MODEL || join(app, 'testset/models', summary);
      if (!existsSync(source)) throw new Error(`Pro requires the summary model. Set MINUTES_BUNDLE_SUMMARY_MODEL to a local file.`);
      await verify(source, digests[name]);
      mkdirSync(models, { recursive: true });
      copyFileSync(source, path);
    }
    await verify(path, digests[name]);
  }
  if (!existsSync(join(app, 'src-tauri/resources/THIRD_PARTY_NOTICES.txt'))) throw new Error('Missing third-party notices');
  if (edition === 'pro') {
    const runtime = process.platform === 'win32' ? 'onnxruntime.dll' : 'libonnxruntime.dylib';
    if (!existsSync(join(app, 'src-tauri/resources/onnxruntime', runtime))) throw new Error('Missing speaker runtime');
    const target = process.platform === 'win32' ? 'x86_64-pc-windows-msvc.exe' : 'aarch64-apple-darwin';
    const sidecar = join(app, 'src-tauri/binaries', `minutes-summarizer-${target}`);
    if (!existsSync(sidecar) || statSync(sidecar).size === 0) throw new Error('Build summary sidecar first');
  }
  console.log(`Verified ${edition}: ${config.bundle.resources.join(', ')}`);
  if (process.argv.includes('--check')) return;
  const require = createRequire(join(app, 'ui/package.json'));
  const cli = require.resolve('@tauri-apps/cli/tauri.js');
  const windowsConfig = process.platform === 'win32' ? ['--config', 'tauri.windows.conf.json'] : [];
  const args = process.argv.slice(3).filter(x => !['--check', '--shared-cache'].includes(x));
  const result = spawnSync(process.execPath, [cli, 'build', ...windowsConfig, '--config', JSON.stringify(config),
    '--features', editionFeatures(edition).join(','), ...args, '--', '--no-default-features'], {
    cwd: join(app, 'src-tauri'), stdio: 'inherit',
    env: { ...process.env, VITE_MINUTES_EDITION: edition, CARGO_TARGET_DIR: process.argv.includes('--shared-cache')
      ? join(app, 'src-tauri/target') : join(app, 'src-tauri/target/editions', edition) },
  });
  if (result.error) throw result.error;
  process.exitCode = result.status ?? 1;
}
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main().catch(error => { console.error(error.message); process.exitCode = 1; });
}
