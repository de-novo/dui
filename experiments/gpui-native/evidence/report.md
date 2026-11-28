## GPUI native preview versus SwiftUI and Compose

**Keep Taffy as a candidate for native layout. Defer adopting the 1× cosmic-text/Swash path as a pixel-faithful text renderer.** Both centered-label fixtures placed all four ink edges within one logical pixel on both platforms after tracking conversion. The wrapped SwiftUI stack's right ink edge fell 13 px short at both widths. All four stacks wrapped into the same three visible lines as their platform reference. Pixel errors combine font metrics, glyph placement, rasterization and Retina resampling. These captures cannot attribute the remaining difference to a single engine.

| Native reference | Logical viewport | Ink pixels differing, Detail / Profile / Stack | Screens passing ≤1% ink mismatch | Save to observed GPUI pixels, median / p95 | Edits observed |
|---|---|---|---|---|---|
| SwiftUI | 390×844 | 88.7% / 89.4% / 93.4% | 0/3 | 235.2 / 266.4 ms | 24/24 |
| SwiftUI | 1440×920 | 89.0% / 89.4% / 93.4% | 0/3 | 342.8 / 377.6 ms | 24/24 |
| Compose | 390×844 | 82.7% / 90.8% / 92.6% | 0/3 | 220.1 / 409.2 ms | 24/24 |
| Compose | 1440×920 | 82.2% / 90.6% / 92.7% | 0/3 | 343.4 / 405.0 ms | 24/24 |

Full-screen pixel equality was 0/12. Ink mismatch measures the fraction of foreground-union pixels where any RGB channel differs by more than 20. The denominator excludes the white background. Geometry can therefore be close even when a large fraction of ink pixels differs.

The first Android single-label attempt treated Compose's 0.5 sp tracking as 0.5 em. Converting to 0.03125 em at size 16 reduced Detail's mobile ink width from 79 px to 42 px (reference 42), and Profile's from 86 px to 48 px (reference 48). The alternate GPUI host-text control produced a blank iOS Detail capture after source and model revisions advanced. Its latency is unverified.

Measured on Apple M1 Pro, macOS 26.5.2, Retina 2×, SwiftUI on the iOS 26.5 simulator and Compose on an API 37 arm64 emulator at 160 dpi. Reference images use SwiftUI ImageRenderer or the Compose emulator framebuffer. The experiment edits literal source text and draws a known layout scaffold in a native GPUI window. Eight alternating writes per case followed calibration. Latency includes screenshot sampling and is an upper bound on first visibility. P95 uses nearest rank. These fixtures cover centered text, nested stacks, spacing and wrapping.

GPUI 0.3.14 declares Apache-2.0, Taffy 0.16.0 MIT, and cosmic-text 0.19.0 and Swash 0.2.10 MIT or Apache-2.0. The resolved target includes BSD-3-Clause, Zlib and Unicode-3.0, plus MPL-2.0 cbindgen in the build graph. Check the latter two against repository policy before adoption. Device Roboto identifies SIL OFL 1.1. Simulator SFUI carries Apple's OS license and was read locally. No font binaries are included.
