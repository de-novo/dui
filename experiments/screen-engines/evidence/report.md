## Whole-screen engine comparison

Recommend retaining the production DOM drawer for web while improving the model supplied to it. Replacing the layout stage with Yoga lost the inline control, more than doubled the compressed drawer, and recovered neither real app. Both candidates lost the shadcn error-page styling before layout. The native two-screen result establishes feasibility for literal text only.

M1 Pro, macOS 26.5.2, Chrome 158.0.8127.37, iOS 26.5 Simulator. All results use complete 1440×920 and 390×844 surfaces.

| Candidate | Match 1440 / 390 | Edit median / p95, ms at 1440 | Edit median / p95, ms at 390 | Missed | Measured memory | Code and bundle |
|---|---|---:|---:|---:|---|---|
| dui DOM | 1/5 · 1/5 | 66.5 / 106.8 | 66.1 / 92.3 | 0/80 | Viewer JS heap median 6.64 / 5.62 MiB | Existing 798-line drawer; minified 161,776 B, gzip 48,235 B |
| Yoga WASM + Canvas text + dui DOM paint | 0/5 · 0/5 | 65.8 / 73.0 | 66.1 / 69.1 | 0/80 | Viewer JS heap median 6.94 / 5.96 MiB | Same drawer + 82 adapter lines; minified 291,186 B, gzip 100,886 B |
| Swift parser + SwiftUI Text, simulator | 2/2 · 2/2 | Foreground pending | Foreground pending | Completed both rasters on 16/16 edits | Parser peak RSS median 39.3 MiB; renderer RSS median 193.8 MiB | Simulator surface executable 187,152 B; includes test reference views |
| Compose / Android | Pending | Pending | Pending | No eligible trials | — | — |

iOS text scenes matched the reference PNGs byte-for-byte on each revert. From save to completion of **both** raster files, median was **97.4 ms**, p95 **103.7 ms** (16 edits). Median parse and publication was 64.6 ms. Foreground launch stalled during CoreLocation data migration on a fresh simulator and after reboot. Raster-write timing and browser presentation timing measure different endpoints. Android SDK and AVD provisioning is coordinated with the mainline benchmark. When checked, adb listed no attached device. Both Android default-map attempts before provisioning exited 5 and produced no source model.

| Screen | Chrome vs DOM, desktop / mobile | Chrome vs Yoga, desktop / mobile | Shared default map analysis median ms / peak RSS MiB |
|---|---|---|---:|
| shadcn-admin /403 | fail / fail | fail / fail | 17.7 / 7.3 |
| shadcn-admin /503 | fail / fail | fail / fail | 35.3 / 7.3 |
| dui EmptyWorkspace | fail / fail | fail / fail | 43.9 / 8.8 |
| #441 inline | pass / pass | fail / fail | 19.0 / 6.7 |
| #441 grid | fail / fail | fail / fail | 20.4 / 6.7 |

The three real feature screens give 0/3 for each candidate at both widths. Default Swift map generated a route graph without a drawable source model (median 4,885.5 ms / 146.6 MiB peak process RSS across three warm-cache runs). Android diagnostic returns took 63.6 and 67.8 ms across the two fixture roots. No Chrome/native parity claim follows from those map timings. Source slicing, external CSS, unmodeled component behavior and layout all contribute to the failures. The samples do not isolate an engine-wide winner for native.
