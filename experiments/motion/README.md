# Source-model motion research

## Additive design

The v1 schema is closed to additional properties. Keep the experimental sidecar outside production JSON. A prospective schema revision can add one optional top-level `motion` object with optional arrays of timelines and bindings. No existing required fields, property enums or rendered-node shapes need to change. Old documents validate under the proposed schema. An old strict validator still rejects a new document carrying the field: gate emission by reader capability and retain the v1 projection without motion.

- A timeline has a stable, source-scoped ID, ordered keyframes with offsets in [0,1], and declarations with typed values and source locations. Preserve multiple transform operations in authored order and preserve units and transform origin. For example, translate-x 112 CSS px is one operation. Unresolved variables and runtime expressions retain their text and reason. Do not normalize matrices, percentages or springs into invented px values.
- A binding references an existing rendered-node ID and a timeline, or identifies a transition property with explicit source and destination states. Keep state predicates (such as hover), activation/deactivation, animation name, duration, delay, iterations, direction, fill and timing distinct. Preserve segment-specific timing. Record the authored and built locations when a source map establishes both. Retain competing rules, conditions, reduced-motion media and cascade provenance until resolution. Equal keyframe offsets need source order.
- Extraction records declared behavior. Playback owns its clock and interrupted-transition history. Unsupported easing, composition, runtime endpoints and missing geometry are unverified. An absent motion object carries no new assertion. Validate node/timeline references, finite durations and offsets, and deterministic serialization. Gate any production change with check:source-model-additive, old-document and projection tests.

## Measured subset

The sidecar is a minimal executable subset. Oxc reads literal JSX classes; Tailwind compiles candidates; css-tree reads simple class rules, a hover target and named keyframe stops. The probe binds to v1 element IDs in the fixture's straight-line child order. The sample grammar supports linear, one-second single-property translateX(px) tracks. Mount and hover-enter are both covered; interruption, hover exit, paused page time, reverse, variable substitution, nested selectors and arbitrary JS were not tested. The mobile viewport uses a pointer to exercise the same hover transition. Tailwind and handwritten CSS share one named timeline.

The pinned dui drawer renders geometry from the original model. At each logical timestamp the model-side probe samples numeric tracks with d3-interpolate and applies the resulting static translation. Only the reference runs CSS animations/transitions, paused and sought through the Web Animations API. Chrome also provides screenshot and rectangle readback for the model frame. Compare actual viewport coordinates, before cropping the screenshots. These are synchronized poses, not realtime frame pacing.

## Native portability assessment

SwiftUI can map the hover binding to onHover, linear transition to a state-driven offset, and multi-stop values to KeyframeAnimator/LinearKeyframe segments. GPUI 0.3.14 exposes AnimationExt::with_animation with a normalized progress callback and Animation::with_easing. A desktop adapter can sample the same segments and apply the resulting visual translation. Define CSS px to native logical-unit conversion at the viewport boundary. Avoid substituting layout offsets that move siblings. Platform event lifetimes, reduced motion, transform order, coordinate origins, state interruption and repeat semantics need adapter conformance tests. This is an API-level assessment; native frame equivalence remains unmeasured.

## Reproduction

Use the checkout and ratio_measure extractor prepared as in ../relative-units/reproduce.sh (renderer commit 0ae1b92). Install the locked dependencies here and in ../relative-units. From this directory run against an empty temporary directory:

```sh
node measure.mjs --capture "$CHROME" "$PINNED_CHECKOUT" "$EXTRACTOR" "$EMPTY_OUTPUT"
python3 film.py "$EMPTY_OUTPUT"
```

Film generation requires Pillow and ffmpeg. The video presents eleven sampled poses at four poses per second for inspection.
