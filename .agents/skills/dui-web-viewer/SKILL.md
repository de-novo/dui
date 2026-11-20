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
- Display the status supplied by Rust. Show the reason for an unverified or unrenderable cell in text. A captured loading skeleton is still a capture. Show its recorded status even when an image exists.
- Use a text label for each status even when the gallery also uses a color or hatch. In a dense gallery, reserve each thumbnail's space from its recorded dimensions and load full-size images on inspection. [Use of color](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html)

## Review a change

1. Open a saved screen at two recorded widths. Check that its screenshot keeps its aspect ratio and that its name, width and status remain readable on a narrow viewer.
2. Use only the keyboard to select a cell, open its inspector and return. Check the selected cell and visible focus after returning. Reload its URL to check selection independently.
3. Where the changed UI presents evidence, inspect one verified cell, one unverified cell with an image and one unrenderable cell without an image. Check the labels against the report. For live edits, compare the returned state and visible result.
4. Record the interaction exercised, the widths and the observed result. For a visual change, include a screenshot made with a redistributable fixture.
