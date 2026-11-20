---
name: dui-web-viewer
description: Build or review the dui web viewer's screen map, capture gallery, and live inspector. Use when changing viewer navigation, component styling, keyboard behavior, or the presentation of capture evidence.
---

# Work on the web viewer

Read the [viewer decision](../../../docs/en/design-v0.md#web-viewer) and use a saved report and graph as the starting point. Archive fixtures should exercise the same components as real projects.

## Implement

- Give a screen one node. Expand it to show states and recorded widths. A link found in the target app carries its discovery source. A replayed transition carries its observed result. Use distinct labels for these two kinds of edge.
- Use stable report identities for selection. Keep the project, run, screen, state and width addressable through the router. Keep hover and transient menus local. If a selected cell is unavailable, explain which selection is missing and offer a return to its screen.
- Reuse the project's shadcn/ui controls and semantic Tailwind tokens for panels, text, borders, focus and status. Keep viewer chrome outside capture images. Read the local component configuration before choosing a primitive. [shadcn/ui skill](https://ui.shadcn.com/docs/skills) · [Tailwind interaction states](https://tailwindcss.com/docs/hover-focus-and-other-states)
- Provide a named keyboard control for every operation needed to get from the overview to a cell. Offer the same selection through a list for reviewers who cannot pan the map. A modal inspector moves focus inside, closes on Escape and returns focus to its trigger. Honor reduced motion when centering the map. [Web Interface Guidelines](https://vercel.com/design/guidelines) · [Dialog pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/) · [React Flow accessibility](https://reactflow.dev/learn/advanced-use/accessibility)
- Show the recorded status as text and include the reason for unverified or unrenderable cells. This also applies to captures of loading skeletons. In a dense gallery, reserve thumbnail space when dimensions are available and load full-size images on inspection. [Use of color](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html)

## Decide where a change belongs

- **Archive format.** Update the shared decoder and its fixtures when Rust changes an output field. Keep graph and report IDs opaque. Derive React Flow nodes from decoded records with a pure function so list and inspector components can use the same records independently of the graph library.
- **Selection.** A map click and a list click navigate to the same typed route. Resolve the selected cell from that route on render. A live edit result belongs to the cell that requested it, even if the reviewer switches cells before the response arrives.
- **Components.** Keep a control next to its feature until another view needs the same behavior. For example, when both the gallery and inspector show a verification result, share the status label and its reason formatting. Keep map-specific gestures in the map. A target app’s URL and component names enter the UI as capture data.

## Review a change

1. Open a saved screen at two recorded widths. Check that its screenshot keeps its aspect ratio and that its name, width and status remain readable on a narrow viewer.
2. Use only the keyboard to select a cell, open its inspector and return. Check the selected cell and visible focus after returning. Reload its URL to check selection independently.
3. Where the changed UI presents evidence, inspect one verified cell, one unverified cell with an image and one unrenderable cell without an image. Check the labels against the report. For live edits, compare the returned state and visible result.
4. Record the interaction exercised, the widths and the observed result. For a visual change, include a screenshot made with a redistributable fixture.
5. Match regression tests to the changed boundary. For a format change, read an older archive and a current one and check their decoded values. For a selection change, enter through the list and check that the map and inspector resolve the same cell after Back or reload.
