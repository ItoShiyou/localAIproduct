import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  base: "./",
  // core/brand(シリーズ共通のデザイン)を読むため、親のフォルダも許可する
  server: { host: "127.0.0.1", port: 5175, strictPort: true, fs: { allow: [".."] } },
});
