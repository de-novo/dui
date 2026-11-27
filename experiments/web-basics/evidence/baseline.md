# Fixed web basics baseline

Main 9f622bae075cf669ad82e859fa9567556eed86dd. Chrome 158.0.8127.37, macOS 26.5.2 arm64 / Apple M1 Pro, en-US, DPR 1. Every scenario and mandatory probe must pass. Columns show desktop / mobile. Missing elements and rejected experimental inputs count as failures.

| Approach | 1440×920 | 390×844 | Both |
|---|---:|---:|---:|
| map | 9/37 | 9/37 | 9/37 |
| sheet | 15/37 | 15/37 | 15/37 |
| lower | 17/37 | 17/37 | 17/37 |
| sidecar | 37/37 | 37/37 | 37/37 |


## Layout

| Fixture | Scenarios | map | sheet | lower | sidecar | Desktop default errors | Mobile default errors | Live edit desktop / mobile |
|---|---:|---|---|---|---|---|---|---|
| block | 1 | ✓ / ✓ | ✓ / ✓ | × / × | ✓ / ✓ | — | — | 80.6 ms / 73.6 ms |
| inline | 1 | ✓ / ✓ | ✓ / ✓ | × / × | ✓ / ✓ | — | — | 99.8 ms / 100.9 ms |
| flex | 1 | ✓ / ✓ | ✓ / ✓ | × / × | ✓ / ✓ | — | — | 102.1 ms / 103.6 ms |
| grid | 1 | × / × | × / × | × / × | ✓ / ✓ | section > :first-child.width Δ1400, section > :last-child.x Δ48, section > :last-child.y Δ18, section > :last-child.width Δ1380, section.gridTemplateColumns differs | section > :first-child.width Δ350, section > :last-child.x Δ48, section > :last-child.y Δ18, section > :last-child.width Δ330, section.gridTemplateColumns differs | >2 s / >2 s |
| position | 1 | × / × | × / × | × / × | ✓ / ✓ | section > div.x Δ14, section > div.y Δ8 | section > div.x Δ14, section > div.y Δ8 | >2 s / >2 s |
| overflow | 2 | × / × | × / × | × / × | ✓ / ✓ | section.overflowX differs | section.overflowX differs | >2 s / >2 s |

## Sizing

| Fixture | Scenarios | map | sheet | lower | sidecar | Desktop default errors | Mobile default errors | Live edit desktop / mobile |
|---|---:|---|---|---|---|---|---|---|
| percent | 1 | × / × | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | strong.width Δ80, strong.height Δ12 | strong.width Δ80, strong.height Δ12 | >2 s / >2 s |
| rem | 1 | × / × | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | strong.width Δ80, strong.height Δ12 | strong.width Δ80, strong.height Δ12 | >2 s / >2 s |
| em | 1 | × / × | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | strong.width Δ60, strong.height Δ12 | strong.width Δ60, strong.height Δ12 | >2 s / >2 s |
| vh | 1 | × / × | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | strong.width Δ92, strong.height Δ12 | strong.width Δ84.39, strong.height Δ12 | >2 s / >2 s |
| vw | 1 | × / × | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | strong.width Δ144, strong.height Δ12 | strong.width Δ39, strong.height Δ12 | >2 s / >2 s |

## Text

| Fixture | Scenarios | map | sheet | lower | sidecar | Desktop default errors | Mobile default errors | Live edit desktop / mobile |
|---|---:|---|---|---|---|---|---|---|
| text-wrap | 1 | ✓ / ✓ | ✓ / ✓ | × / × | ✓ / ✓ | — | — | 75.6 ms / 80.9 ms |
| font | 1 | × / × | × / × | × / × | ✓ / ✓ | section.lineRects Δ17, section.fontLoaded differs, section.fontFamily differs, section.fontSize differs, section.lineHeight differs | section.lineRects Δ17, section.fontLoaded differs, section.fontFamily differs, section.fontSize differs, section.lineHeight differs | >2 s / >2 s |

## Paint, assets and transforms

| Fixture | Scenarios | map | sheet | lower | sidecar | Desktop default errors | Mobile default errors | Live edit desktop / mobile |
|---|---:|---|---|---|---|---|---|---|
| inherit-style (supplement) | 1 | ✓ / ✓ | ✓ / ✓ | × / × | ✓ / ✓ | — | — | 84.5 ms / 84.9 ms |
| cascade (supplement) | 1 | × / × | × / × | × / × | ✓ / ✓ | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12 | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12 | >2 s / >2 s |
| custom-property | 1 | × / × | × / × | × / × | ✓ / ✓ | strong.color differs, strong.paddingLeft Δ12, strong.paddingRight Δ12, em.color differs, em.paddingLeft Δ6, em.paddingRight Δ6 | strong.color differs, strong.paddingLeft Δ12, strong.paddingRight Δ12, em.color differs, em.paddingLeft Δ6, em.paddingRight Δ6 | >2 s / >2 s |
| color (supplement) | 1 | ✓ / ✓ | ✓ / ✓ | × / × | ✓ / ✓ | — | — | 86.0 ms / 92.0 ms |
| border (supplement) | 1 | × / × | × / × | × / × | ✓ / ✓ | section.borderLeftWidth differs, section.borderLeftStyle differs, section.borderLeftColor differs | section.borderLeftWidth differs, section.borderLeftStyle differs, section.borderLeftColor differs | >2 s / >2 s |
| radius (supplement) | 1 | ✓ / ✓ | ✓ / ✓ | × / × | ✓ / ✓ | — | — | 99.7 ms / 105.7 ms |
| image | 1 | × / × | × / × | × / × | ✓ / ✓ | img missing | img missing | >2 s / >2 s |
| icon | 1 | × / × | × / × | × / × | ✓ / ✓ | svg missing | svg missing | >2 s / >2 s |
| translate (supplement) | 1 | × / × | × / × | × / × | ✓ / ✓ | section.x Δ12, section.y Δ6 | section.x Δ12, section.y Δ6 | >2 s / >2 s |
| transform-order (supplement) | 1 | × / × | × / × | × / × | ✓ / ✓ | section.x Δ7.97, section.y Δ17.95, section.width Δ20.06, section.height Δ35.9, section.transform differs | section.x Δ7.97, section.y Δ17.95, section.width Δ20.06, section.height Δ35.9, section.transform differs | >2 s / >2 s |

## Native controls

| Fixture | Scenarios | map | sheet | lower | sidecar | Desktop default errors | Mobile default errors | Live edit desktop / mobile |
|---|---:|---|---|---|---|---|---|---|
| input | 1 | × / × | × / × | × / × | ✓ / ✓ | input.label differs, input.value differs, input.width Δ30, input.height Δ3 | input.label differs, input.value differs, input.width Δ30, input.height Δ3 | >2 s / >2 s |
| checkbox | 2 | × / × | × / × | × / × | ✓ / ✓ | input.label differs, input.checked differs | input.label differs, input.checked differs | Already equal / Already equal |
| select | 1 | × / × | × / × | × / × | ✓ / ✓ | select.label differs, select.value differs, select.height Δ3 | select.label differs, select.value differs, select.height Δ3 | Already equal / Already equal |
| button | 2 | × / × | × / × | × / × | ✓ / ✓ | button.width Δ4.58, button.height Δ3 | button.width Δ4.58, button.height Δ3 | 78.8 ms / 86.2 ms |

## Style translation

| Fixture | Scenarios | map | sheet | lower | sidecar | Desktop default errors | Mobile default errors | Live edit desktop / mobile |
|---|---:|---|---|---|---|---|---|---|
| inline-style | 1 | ✓ / ✓ | ✓ / ✓ | × / × | ✓ / ✓ | — | — | 79.3 ms / 64.3 ms |
| class-composition | 2 | × / × | × / × | × / × | ✓ / ✓ | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12, section.paddingLeft Δ6, section.paddingRight Δ6 | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12, section.paddingLeft Δ6, section.paddingRight Δ6 | >2 s / >2 s |
| clsx | 2 | × / × | × / × | × / × | ✓ / ✓ | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12, section.paddingLeft Δ6, section.paddingRight Δ6 | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12, section.paddingLeft Δ6, section.paddingRight Δ6 | >2 s / >2 s |
| tailwind | 1 | × / × | × / × | × / × | ✓ / ✓ | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12 | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12 | >2 s / >2 s |
| stylesheet | 1 | × / × | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12 | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12 | >2 s / >2 s |
| css-module | 1 | ✓ / ✓ | × / × | ✓ / ✓ | ✓ / ✓ | — | — | 95.0 ms / 101.9 ms |
| media-query | 1 | × / × | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | section.color differs, section.paddingLeft Δ12, section.paddingRight Δ12 | section.color differs, section.paddingLeft Δ6, section.paddingRight Δ6 | >2 s / >2 s |

## JSX translation

| Fixture | Scenarios | map | sheet | lower | sidecar | Desktop default errors | Mobile default errors | Live edit desktop / mobile |
|---|---:|---|---|---|---|---|---|---|
| and | 2 | × / × | × / × | ✓ / ✓ | ✓ / ✓ | section.text differs, section.children differs | section.text differs, section.children differs | >2 s / >2 s |
| ternary | 2 | × / × | × / × | ✓ / ✓ | ✓ / ✓ | section.text differs, section.children differs | section.text differs, section.children differs | >2 s / >2 s |
| map-key | 3 | × / × | × / × | × / × | ✓ / ✓ | section.text differs, section.children differs | section.text differs, section.children differs | >2 s / >2 s |
| fragment | 1 | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | — | — | 103.3 ms / 100.0 ms |
| props | 1 | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | — | — | 82.2 ms / 84.3 ms |
| children | 1 | × / × | × / × | ✓ / ✓ | ✓ / ✓ | section.text differs, section.children differs | section.text differs, section.children differs | >2 s / >2 s |
| default-prop | 2 | × / × | × / × | ✓ / ✓ | ✓ / ✓ | section.text differs, section.children differs | section.text differs, section.children differs | >2 s / >2 s |
| spread | 2 | × / × | × / × | ✓ / ✓ | ✓ / ✓ | section.text differs, section.children differs | section.text differs, section.children differs | >2 s / >2 s |
| expression-text | 1 | × / × | × / × | ✓ / ✓ | ✓ / ✓ | section.text differs, section.children differs | section.text differs, section.children differs | >2 s / >2 s |
| initial-state | 2 | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | ✓ / ✓ | — | — | 50.8 ms / 43.8 ms |

## Analysis cost

Each cell is median wall ms / peak process RSS MiB across the three measured fresh processes per scenario, pooled within each ID. One initial run per method/scenario discarded. Costs use prebuilt fixture outputs at width 1440. Map includes archive writes; sheet emits a model; lower includes Node startup, projection and a Rust child; sidecar emits the isolated experimental representation. Process RSS is not simultaneous process-tree RSS. A rejected attempt is included with its exit status in cost.json.

| ID | map | sheet | lower | sidecar |
|---|---:|---:|---:|---:|
| block | 16.7 / 6.7 | 6.3 / 3.8 | 41.6 / 50.2 | 36.9 / 50.0 |
| inline | 17.3 / 6.7 | 6.9 / 3.7 | 42.7 / 50.2 | 38.1 / 49.9 |
| flex | 16.6 / 6.7 | 6.4 / 3.8 | 41.4 / 50.2 | 37.3 / 49.9 |
| grid | 16.7 / 6.7 | 6.3 / 3.8 | 41.0 / 50.2 | 37.0 / 50.1 |
| position | 16.7 / 6.7 | 6.7 / 3.8 | 42.5 / 50.2 | 37.1 / 50.0 |
| overflow | 17.5 / 6.7 | 6.4 / 3.8 | 42.0 / 50.1 | 37.6 / 50.0 |
| percent | 16.9 / 6.8 | 7.8 / 4.0 | 42.1 / 50.0 | 37.3 / 50.0 |
| rem | 16.6 / 6.7 | 6.7 / 4.0 | 42.5 / 50.1 | 37.7 / 49.9 |
| em | 21.7 / 6.8 | 7.0 / 4.0 | 45.5 / 50.1 | 41.3 / 49.8 |
| vh | 18.7 / 6.7 | 8.0 / 4.0 | 48.7 / 50.2 | 42.4 / 50.1 |
| vw | 18.8 / 6.8 | 8.2 / 4.0 | 46.8 / 50.2 | 43.5 / 50.0 |
| text-wrap | 19.1 / 6.7 | 7.3 / 3.8 | 47.3 / 50.2 | 41.6 / 50.0 |
| font | 19.5 / 6.8 | 8.1 / 3.3 | 47.7 / 50.6 | 42.2 / 49.9 |
| inherit-style | 16.7 / 6.7 | 6.4 / 3.8 | 42.3 / 50.2 | 37.2 / 49.9 |
| cascade | 16.7 / 6.7 | 6.7 / 4.0 | 41.8 / 50.1 | 37.6 / 49.9 |
| custom-property | 18.5 / 6.8 | 7.3 / 4.1 | 46.2 / 50.1 | 38.2 / 50.0 |
| color | 21.1 / 6.7 | 7.7 / 3.8 | 42.3 / 50.2 | 39.3 / 50.0 |
| border | 17.2 / 6.7 | 6.9 / 3.8 | 42.8 / 50.0 | 37.3 / 49.9 |
| radius | 18.5 / 6.7 | 6.5 / 3.8 | 42.6 / 50.1 | 39.2 / 50.1 |
| image | 19.1 / 6.7 | 6.7 / 3.7 | 44.0 / 50.3 | 39.9 / 49.9 |
| icon | 17.5 / 6.7 | 6.7 / 3.8 | 42.2 / 50.2 | 38.4 / 50.0 |
| translate | 17.6 / 6.7 | 6.6 / 3.7 | 41.7 / 50.1 | 37.4 / 50.0 |
| transform-order | 16.8 / 6.7 | 6.8 / 3.8 | 43.3 / 50.3 | 38.0 / 50.0 |
| input | 17.0 / 6.7 | 6.3 / 3.6 | 45.1 / 50.1 | 37.5 / 50.0 |
| checkbox | 17.8 / 6.7 | 6.6 / 3.7 | 41.9 / 50.1 | 37.6 / 50.0 |
| select | 17.3 / 6.7 | 6.4 / 3.7 | 41.8 / 50.2 | 37.5 / 50.0 |
| button | 16.8 / 6.7 | 6.3 / 3.7 | 41.8 / 50.1 | 37.4 / 49.9 |
| inline-style | 16.7 / 6.7 | 6.7 / 3.8 | 41.6 / 50.1 | 37.6 / 50.0 |
| class-composition | 17.3 / 6.9 | 6.7 / 4.2 | 42.0 / 50.2 | 38.7 / 50.1 |
| clsx | 17.6 / 6.9 | 6.9 / 4.2 | 42.9 / 50.3 | 38.0 / 50.1 |
| tailwind | 17.2 / 6.8 | 6.6 / 4.0 | 42.3 / 50.1 | 37.6 / 50.0 |
| stylesheet | 16.4 / 6.8 | 6.7 / 4.0 | 41.7 / 50.1 | 37.7 / 49.9 |
| css-module | 19.1 / 7.0 | 7.0 / 4.2 | 43.2 / 50.2 | 41.2 / 50.2 |
| media-query | 16.7 / 6.8 | 6.7 / 4.0 | 41.9 / 50.0 | 37.1 / 49.9 |
| and | 17.8 / 6.7 | 6.6 / 3.8 | 43.0 / 50.1 | 38.3 / 50.0 |
| ternary | 16.9 / 6.8 | 6.6 / 3.8 | 42.4 / 50.1 | 37.6 / 49.9 |
| map-key | 17.9 / 6.7 | 6.6 / 3.8 | 42.1 / 50.2 | 37.5 / 50.0 |
| fragment | 17.2 / 6.7 | 6.5 / 3.7 | 43.6 / 50.1 | 38.7 / 49.9 |
| props | 16.6 / 6.8 | 6.8 / 3.8 | 41.8 / 50.2 | 37.5 / 50.0 |
| children | 17.5 / 6.8 | 6.5 / 3.9 | 41.8 / 50.2 | 37.6 / 50.0 |
| default-prop | 17.6 / 6.8 | 6.6 / 3.9 | 42.7 / 50.2 | 37.8 / 50.1 |
| spread | 17.4 / 6.8 | 6.6 / 3.9 | 42.0 / 50.2 | 37.4 / 50.1 |
| expression-text | 16.8 / 6.7 | 6.4 / 3.8 | 41.6 / 50.1 | 37.4 / 49.8 |
| initial-state | 17.5 / 6.8 | 6.4 / 3.9 | 42.3 / 50.2 | 37.5 / 50.0 |
