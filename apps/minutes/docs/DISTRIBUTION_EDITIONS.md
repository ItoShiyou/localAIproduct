# Free / Pro downloadable editions

2026-10-07: owner requested independent lightweight Free and all-inclusive paid Pro,
with multiple storefronts rather than in-app activation. This supersedes the earlier
optional-summary-download distribution decision. No changes to shared core.

- Free: standard ASR, existing 60-minute cumulative / 15-minute single-recording limits,
  recording/import/edit/search and TXT export. No speaker runtime, large ASR, summary model or sidecar bundled.
- Pro: compile-time paid edition, all entitlements without customer keys. Small live ASR,
  large final ASR, speaker model/runtime and summary model/sidecar bundled.
- Release flags ignore MINUTES_TIER and existing license files for these editions.
  Existing license-based builds remain available for backward compatibility.
- Same app identifier/data directory preserves recordings when replacing Free with Pro.
  Quit the old app before installing. Do not run both against the same database.
  Paid builds can be copied; there is no remote revocation or device-count enforcement.

## Build / verify

Run from repository root:

```
node apps/minutes/tools/build_edition.mjs free --check
node apps/minutes/tools/build_edition.mjs pro --check
node apps/minutes/tools/build_edition.mjs free --bundles app --no-sign
node apps/minutes/tools/build_edition.mjs pro --bundles app --no-sign
node --test apps/minutes/tools/build_edition.test.mjs
```

Artifacts: src-tauri/target/editions/{free,pro}/release/bundle/.
For local verification only, --shared-cache reuses src-tauri/target; its bundle/macos/
contains minutes Free.app and minutes Pro.app separately. Rust feature fingerprints
select the native edition; frontend output is still isolated by edition.
Preparation uses the existing resource preparation script. Pro additionally requires the
exact summary GGUF: MINUTES_BUNDLE_SUMMARY_MODEL or local testset/models fallback.
Hash-verified local copies only; this build tool does not download models or send user data.
Frontend and native edition flags are set together; isolated build/dist directories prevent
accidentally packaging the other edition. No new production key required.

Free remains based on some shared code: unused paid UI is removed, heavy model/runtime
resources and diarization dependency are excluded. Do not describe the binary as containing
zero shared paid-function code until further feature extraction is complete.

Purchase selection page: web/minutes/purchase/. Update stores.json to enable a storefront
ONLY after its product, paid files, supported OS and purchase/delivery tests are ready.
No stores are currently enabled. New page is not yet deployed.

Release gates: both final bundles, Free-to-Pro data preservation, offline recording/summary,
Windows real-device tests, signing/notarization and actual purchase delivery per store.

## Verification so far

Mac M2: both unsigned .app bundles built. Free about 203 MiB, Pro 3.13 GiB
(summary model included; these are installed bundle sizes, not ZIP/download sizes).
Final legacy Rust regression 90 passed; edition resolution and entitlement tests passed for
both features. Frontend compilation, recording/review unit tests and resource-manifest
tests passed. Browser inspection confirmed Free controls removed and Pro badge shown.
These checks do not replace signing, Windows real-device tests or storefront purchases.

Actual bundled-model tests also passed on Mac M2 with network-disabled app settings:
Free: fictional t01_clean audio → standard transcription, paid functions rejected
(2 tests, 13.14 seconds). Pro: same audio → high-accuracy transcription → bundled
summary engine/model → draft, with no automatic confirmation (2 tests, 134.42 seconds).
This is a functional test on one fictional recording, not an accuracy benchmark or
a packet-capture audit. Tests use a fresh temporary data directory, not user meetings.

Reproduce with --release --no-default-features --features whisper,edition-free (or pro)
and --test distribution_editions -- --include-ignored. Set MINUTES_EDITION_RESOURCES
to the actual bundle's Resources/resources, MINUTES_EDITION_AUDIO to t01_clean.wav,
and for Pro MINUTES_EDITION_SIDECAR_DIR to the bundle's Contents/MacOS directory.

BOOTH currently limits each uploaded file to 1.2 GB. The all-inclusive Pro is larger,
so BOOTH must remain disabled until a verified multi-file packaging or delivery method
is selected. Do not publish a partial Pro bundle just to fit that limit.
Official source: https://booth.pm/announcements/916

Preflight for these editions: python3 apps/minutes/spike/release_preflight.py --edition pro
(or free). Still checks seller attribution and notices; no activation public key is needed.
