// Keep frontend purchase channel aligned with the native mac-app-store feature.
import { spawnSync } from "node:child_process";
const env = { ...process.env, VITE_MINUTES_CHANNEL: "mac-app-store" };
delete env.VITE_MINUTES_EDITION;
const result = spawnSync("npm", ["run", "build"], { stdio: "inherit", env });
if (result.error) console.error("Mac App Store画面をビルドできませんでした");
process.exit(result.status ?? 1);
