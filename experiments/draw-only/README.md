# Drawing from frozen route inputs

Research-only, explicit Chrome calibration. Pin dui main at 03b07fc26b775318c50ca0759d6a13a5357a496e and shadcn-admin at 76b0248cb6a1fa6ab60c6de80a2f242cf2b3878f. No project source extraction or API inference is involved. Both routes used the light theme, en-US browser default on this machine, DPR 1 and reduced motion. The scope is the entire visible error-page feature (.h-svh) in its published production app shell. Pixel comparisons use the entire viewport.

For each case the capture serializes element types, attributes, text order, resolved CSS longhands, CSS Typed OM values, generated before/after content and font resource digests. The app CSS URL for Inter is served from a pinned local cache. CDP reports the actual Inter face, variable instance and glyph count at every text-bearing element. The manifest records the SHA-256 of each frozen input in this run. App-generated IDs can differ between fresh page loads. The app shell, CSS, viewports and font registration remain identical between measurements.

## Controls and interpretations

- **Identity** runs the production decoder and drawModel on the projected tree, restores captured attributes and places the resulting feature nodes into the original document formatting context with the original compiled stylesheet. Every source attribute, text node, element type, enumerated computed style, generated pseudo and CDP font record must agree. Also assert zero box, character-advance and pixel differences. This tests renderer-generated DOM with matching inputs. Chrome performs final layout and paint on both sides.
- **Stock** feeds the same reconstructed DOM, literal props and supported computed styles to the unmodified card drawer in its shadow root. Unrepresentable styles remain available in the frozen input, and are missing from its projection. Differences measure this boundary and the resulting drawing. **Shadow-only** applies just the source box-shadow values onto stock output.
- **Typed sidecar** replays styles on the drawer's generated nodes in the inert shadow tree. Preserve computed percentages, auto, offsets, margins and sizing. Chrome's resolved longhands supply paint/text serialization. **Used values** applies all resolved longhands, including already rounded used pixel dimensions; it is intentionally an assisted diagnostic. Neither uses reference screenshots for filling.
- **Yoga** uses the previously measured flex adapter on the same typed-sidecar tree; it does not implement full inline formatting. **Taffy** consumes typed sizes and flex/block styles with HarfBuzz OpenType text widths and explicit forced breaks. It does not consume reference rectangles. The inline br marker contributes a break and is excluded from Taffy's element-box denominator. The Taffy row measures geometry, without claiming independent pixel rendering or general inline layout.
- **Metrics** report maximum absolute component error for box origin and size; character advances are DOM Range widths. Break mismatch counts text runs whose character-index line starts changed. Full-viewport strict pixel difference means any channel changed. Also report the count above 20/255. Font, geometry, wrapping and paint errors interact; ablation and isolated probes establish attribution.
- **Text and raster** use all eleven distinct literal runs from the reference. Native shapers use the exact Inter face and variable axes; the fixed-axis instanced font used for cosmic-text has identical measured OpenType advances to the variable font on this corpus. Natural widths are compared with Chrome DOM. The separate line-break stress test applies 120, 180 and 240 CSS pixel widths to each run. Skia and Chrome Canvas metric probes share baseline and size. For isolated coverage differences, both rasterizers draw identical OpenType outlines, positions, fills, scale and origin. Differences crossing the 50% coverage threshold are reported separately. Native text bitmap differences also include hinting, placement and shaping. The fixed-outline test isolates the raster stage; it does not identify a specific implementation's AA algorithm.

## Reproduce

Prepare the two pinned checkouts and a static shadcn-admin dist as in the neighboring screen-engines experiment. Install its web-basics and engine-comparison locked packages, the viewer's dependencies, and this directory's npm lock. Use macOS arm64 with Chrome 158, HarfBuzz CLI 15.2.0, Python/Pillow 13.0.0, fontTools 4.69.1 and Brotli 1.2.0. Run from this directory with absolute directories in the variables below. The official font URL and expected byte digest are in the manifest. The archive contains the exact served font stylesheet.

```sh
mkdir -p "$stage/cache"
unzip -p evidence/oracle-and-captures.zip fonts.css > "$stage/cache/fonts.css"
node run.mjs --capture "$dui_checkout" "$admin_checkout" "$stage" "$chrome" "$viewer_node_modules"
CARGO_TARGET_DIR="$stage/target" cargo build --release --locked --manifest-path taffy/Cargo.toml
python3 shaping.py "$stage"
node raster.mjs --capture "$stage" "$chrome"
python3 text_probe.py "$stage"
python3 report.py "$stage"
```

The archive contains the frozen route inputs, complete framebuffers and per-node observations. Compare the new font resource SHA-256 to the recorded manifest before interpreting a new run. The two English error routes share one font and a centered flex layout. No grid-heavy, complex-script, animated-state or multiscale claim follows from these cases.
