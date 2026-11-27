## Reproduce the startup sample

Use the renderer checkout at 0ae1b92 and the staged, font-subset and wasm-opt GPUI build from experiments/gpui-web. Provide esbuild, react, react-dom, playwright-core and jpeg-js through Node module resolution. Use a separate local directory for site files and create an empty results directory. From this directory run:

```sh
node stage-startup.cjs "$RENDERER_CHECKOUT" "$GPUI_BUNDLE" "$SITE_DIR"
python3 -m http.server 18767 --bind 127.0.0.1 --directory "$SITE_DIR"
```

In a second shell run:

```sh
node startup.cjs http://127.0.0.1:18767 "$CHROME_EXECUTABLE" "$RESULTS_DIR"
node size.cjs "$GPUI_BUNDLE" "$SITE_DIR/startup/dom"
```

The Chrome harness is opt-in measurement tooling. The probing script collects a timestamp from a matching compositor screenshot. Each browser launch has two fresh contexts. The offline size script uses the original GPUI index without timing marks.

For the Korean title ablation, run ../entry-router/locale-project.mjs with the pinned renderer checkout's apps/canvas/src/i18n/messages.ts, the extracted ModelView JSON, ko, variant (title, header or weight), and an output model path. Feed that model to real-capture.cjs with the same built stylesheet as the viewport/typography probe. The locale is explicit scenario input. The dictionary is parsed as TS syntax and only literal strings are substituted.
