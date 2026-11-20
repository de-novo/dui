# Capture npmgraph

npmgraph visualizes npm package dependencies. This example runs its MIT-licensed frontend at a pinned revision. The capture manifest supplies metadata for two fictional packages, `demo-package` and its dependency `demo-leaf`. [Upstream source](https://github.com/npmgraph/npmgraph/tree/77e02ab5c29cd50670a5937490d57b5f045747f5) · [Upstream license](https://github.com/npmgraph/npmgraph/blob/77e02ab5c29cd50670a5937490d57b5f045747f5/LICENSE) · [Capture manifest](dui.json)

## Run

Start in the **dui repository root** with dui on your `PATH`, Node.js 26, npm, and Chromium or Google Chrome installed. With the default installation, add dui to the current shell with `export PATH="$HOME/.local/bin:$PATH"`.

```sh
git clone https://github.com/npmgraph/npmgraph.git .dui/projects/npmgraph
git -C .dui/projects/npmgraph checkout --detach 77e02ab5c29cd50670a5937490d57b5f045747f5
(cd .dui/projects/npmgraph && npm ci --ignore-scripts)
dui run .dui/projects/npmgraph examples/npmgraph/dui.json .dui/npmgraph
```

For subsequent captures, repeat the last command. Open `.dui/npmgraph/graph.html` in a browser.

## What to check

The manifest specifies **one entry URL**. npmgraph reads two explicit mock responses from a local registry path. dui waits for the dependency node to appear, follows the header’s home link, and captures the search page. The analysis scope covers the app header and input help components.

| Screen | Evidence | Widths |
| --- | --- | --- |
| Dependency graph | The SVG contains a node for `demo-leaf`. The graph cells record HTTP 200 for both `/registry/demo-package` and `/registry/demo-leaf`. | 375 and 1280 px |
| Search page | dui discovers `/` from the header link. The search input appears below the page heading. | 375 and 1280 px |

An agent can check `.dui/npmgraph/report.json` for `discoveredRoutes` equal to `["/"]`, both registry paths in the graph cells’ `observation.responses`, and `coverage.verified` and `coverage.renderable` both equal to 4. Those four cells cover these two screens at the selected widths.
