# Product roadmap

English · [한국어](../ko/roadmap.md) · [简体中文](../zh-CN/roadmap.md)

The current CLI starts the target app in Chromium, captures real pixels, checks the observed state and saves the result under `~/.dui/`. The local viewer compares saved captures by project, branch and worktree. The next step is to keep the app running while an agent reads and updates the state of a particular element.

## v0.2.0

**First live web session.** On a screen with two instances of the same component, an agent selects one, reads a typed slot, changes its value, sees the screen update, then restores the original value. The other instance keeps its state. The first fixture covers a boolean and an enum.

| Work | Acceptance evidence |
| --- | --- |
| [Typed element states](https://github.com/de-novo/dui/issues/1) | CLI and MCP return the owner, instance, source, type, current value and editability. An ambiguous element or invalid value returns a reason and leaves state intact. |
| [Live rendering](https://github.com/de-novo/dui/issues/2) | Start the host and browser once. Set and reset a value in the running app. Check both the value read back and the visible DOM or accessibility result. |
| [Edit speed and capture](https://github.com/de-novo/dui/issues/3) | Measure 30 warm edits from request acceptance to a verified visible update. Report p50 and p95 in milliseconds, cold startup separately, and zero host rebuilds or browser restarts during the edits. Save a chosen state with its evidence under `~/.dui/`. |
| [M1 coverage](https://github.com/de-novo/dui/issues/4) | Run the existing deterministic planning and coverage gates. Compare CLI and MCP results and report missing evidence for each unverified cell. [M1 gates](architecture.md#7-m0-and-m1-gates) |

The first benchmark establishes the latency baseline on a recorded machine and fixture. A cross-project latency target remains open.

## v0.3.0

**First live native render.** Render one real GPUI screen at a known width with two typed states. Reuse the same inspect, set, read-back and reset contract. Record the OS, renderer and toolchain with the images. A matching value and independent semantic evidence are required for verification. Missing semantic evidence leaves the cell unverified. A missing render environment produces an unrenderable cell with its reason.

Implementation · [#5](https://github.com/de-novo/dui/issues/5) · [#6](https://github.com/de-novo/dui/issues/6)

## Later milestones

| Version | Outcome |
| --- | --- |
| v0.4.0 | Replay a sequence of events from the initial state and verify the resulting state and transition. |
| v0.5.0 | Build the GPUI and gpui-kit desktop viewer. Open a project from `~/.dui/`, compare worktrees, inspect an element and send a comment to the agent. The viewer consumes the same records as the browser viewer. |
| v0.6.0 | Measure the 99% coverage goal on the reference screens. Publish verified and renderable counts, missing branches and reasons for remaining cells. |

[GitHub milestones](https://github.com/de-novo/dui/milestones)
