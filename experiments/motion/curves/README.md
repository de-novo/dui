# Curves, reversal and native playback

From experiments/motion, install the pinned dependencies and run the following with an existing empty results directory. Use the renderer and extractor described in the parent README.

```sh
node curves/measure.mjs "$PINNED_CHECKOUT" "$EXTRACTOR" "$CHROME" "$OUT"
```

The reference runs actual stylesheet keyframes and transitions. Each captured frame is explicitly sought. The model draws the extracted boxes, evaluates compatible transform-list segments and applies static transforms. Curve sampling uses bezier-easing and a four-step end-aligned sampler. The fourth element reverses a running ease transition after the first animation frame at or beyond 400 ms. Its measured forward elapsed time feeds the reversal calculation. With a first reversal to the original value, the new duration is the original duration multiplied by the forward easing output. The reverse start position uses that same fraction. The three animation timelines and the reverse timeline are sampled independently from their own zero. Both web paths use the same browser geometry/rasterizer for static transforms. Different transform lists, matrix decomposition, nested reversals and asynchronous compositor pacing remain outside this sample.

The native apps read the first linear translation track from the previously extracted evidence/motion.json. They activate the track on launch and repeat for observation. Build the SwiftUI app with swiftc native/SwiftProbe.swift and the GPUI app with cargo +1.100.0 build --locked --manifest-path native/Cargo.toml. Launch each executable with evidence/motion.json. Compile native/window-id.swift to find the titled GPUI window; the SwiftUI app prints its window number. Run native/record.py WINDOW_NUMBER OUTPUT/swift-film or OUTPUT/gpui-film, then native/film.py OUTPUT VIDEO_DIRECTORY. Each screencapture is scoped to that window. The comparison pairs the nearest samples by recording time and labels the timestamps; independent app launch phases preclude synchronized native error measurements.
