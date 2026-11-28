## Whole-screen engine comparison

Recommend retaining the production DOM drawer for web while improving the model supplied to it. Replacing the layout stage with Yoga lost the inline control, more than doubled the compressed drawer, and recovered neither real app. Both candidates lost the shadcn error-page styling before layout. The native two-screen result establishes feasibility for literal text only.

M1 Pro, macOS 26.5.2, Chrome 158.0.8127.37, iOS 26.5 Simulator. All results use complete 1440×920 and 390×844 surfaces.

| Candidate | Match 1440 / 390 | Edit median / p95, ms at 1440 | Edit median / p95, ms at 390 | Missed | Measured memory | Code and bundle |
|---|---|---:|---:|---:|---|---|
| dui DOM | 1/5 · 1/5 | 66.5 / 106.8 | 66.1 / 92.3 | 0/80 | Viewer JS heap median 6.64 / 5.62 MiB | Existing 798-line drawer; minified 161,776 B, gzip 48,235 B |
| Yoga WASM + Canvas text + dui DOM paint | 0/5 · 0/5 | 65.8 / 73.0 | 66.1 / 69.1 | 0/80 | Viewer JS heap median 6.94 / 5.96 MiB | Same drawer + 82 adapter lines; minified 291,186 B, gzip 100,886 B |
| Swift parser + SwiftUI Text, simulator | 2/2 · 2/2 | 116.6 / 148.2 | 116.8 / 177.1 | 0/32 | Parser peak RSS median 39.3 MiB; renderer RSS median 193.8 MiB | Surface executable 187,152 B; foreground executable 209,504 B (test references included) |
| Kotlin syntax projection + Compose Text, Android arm64 emulator | 2/2 · 2/2 | 148.0 / 178.8 | 177.4 / 259.4 | 0/32 | Parser peak RSS 22.6 / 22.5 MiB; app RSS 201.0 / 201.2 MiB | 19-line syntax projection + 58-line host/renderer; debug APK 12,884,080 B |

iOS text scenes matched the reference PNGs byte-for-byte on each revert. From save to completion of **both** raster files, median was **97.4 ms**, p95 **103.7 ms** (16 edits). Median parse and publication was 64.6 ms. After the iOS simulator completed its first-boot data migration, the foreground run also completed. The table uses save to two UI run-loop ticks on the scaled logical viewport. A separate real device screenshot checked each observed edit/revert. Native logical parity was measured at 1×, and the physical iPhone simulator window is 1206×2622.

Android ran on the read-only API 37.2 arm64 AVD with 160 dpi, using two actual Compose reference functions and two independently extracted literal Text records. Median analysis was 43.9 / 60.4 ms. All 32 edit/revert observations passed a separate whole-framebuffer check. An initial run captured Android's first-use fullscreen tutorial; confirming the system prompt and rerunning all conditions removed it. An initial build of the larger settings fixture failed during DEX merging; the retry with 1.5 GiB Gradle heap and two workers built successfully. The attached Android archive includes the two real settings screens as reference-only evidence. The two native text controls are intentionally narrower than the three real web feature screens.

| Screen | Chrome vs DOM, desktop / mobile | Chrome vs Yoga, desktop / mobile | Shared default map analysis median ms / peak RSS MiB |
|---|---|---|---:|
| shadcn-admin /403 | fail / fail | fail / fail | 17.7 / 7.3 |
| shadcn-admin /503 | fail / fail | fail / fail | 35.3 / 7.3 |
| dui EmptyWorkspace | fail / fail | fail / fail | 43.9 / 8.8 |
| #441 inline | pass / pass | fail / fail | 19.0 / 6.7 |
| #441 grid | fail / fail | fail / fail | 20.4 / 6.7 |

The three real feature screens give 0/3 for each candidate at both widths. Default Swift map generated a route graph without a drawable source model (median 4,885.5 ms / 146.6 MiB peak process RSS across three warm-cache runs). Android diagnostic returns took 63.6 and 67.8 ms across the two fixture roots. No Chrome/native parity claim follows from those map timings. Source slicing, external CSS, unmodeled component behavior and layout all contribute to the failures. The samples do not isolate an engine-wide winner for native.
