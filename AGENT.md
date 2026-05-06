# AGENT

This file defines how coding agents should work in this repository.

## Mission

Build Bladb as an optimal-first platform:

- Rust owns the trusted execution core.
- JS/TS owns developer ergonomics.
- Native-looking APIs must not weaken safety, correctness, or performance.

## Core Rules

1. Prefer the best solution, not the most backward-compatible one.
2. Do not preserve weak abstractions just because they existed first.
3. Compatibility is a conscious tradeoff, not a default.
4. When the optimal design conflicts with legacy behavior, prefer the optimal design unless the user explicitly asks to preserve compatibility.
5. During pre-1.0 work, bias toward cleaner architecture, stronger guarantees, and lower long-term complexity.
6. If a change is obvious, low-risk, and does not create meaningful downside, make it directly instead of stopping to ask for confirmation.
7. Do not make the user repeat permission for harmless, straightforward housekeeping such as updating ignore files, docs, agent instructions, or other clearly implied maintenance tied to the request.
8. If a command may require administrator privileges, elevated permissions, system-wide installation, firewall changes, or machine-level configuration, stop and tell the user before running it.
9. Do not keep retrying privileged or environment-mutating commands silently. Explain why elevated access is needed and what the command will change first.
10. When the user gives a direct execution instruction such as "continue", "start", "do it", or a concrete implementation request, execute it without re-asking whether to proceed unless the decision has newly introduced hidden risk, destructive impact, or mutually exclusive tradeoffs that were not already accepted.
11. For multi-step work that spans multiple layers, services, or files, create and maintain an explicit execution plan instead of carrying the sequence only implicitly.
12. Before starting substantial implementation work, present the execution plan in enough detail that the user can see the real phases, not just a one-line intent summary.
13. Do not stop after a short planning reply waiting for another nudge when the user has already asked for execution; continue from planning into implementation unless blocked by a real risk or missing requirement.
14. If meaningful work remains after one batch, update the plan, break the remainder into concrete next steps, and keep executing instead of ending on a vague promise to continue later.
15. Avoid terse, low-information progress replies for non-trivial tasks. Progress updates should name the current phase, what was learned, and what happens next.
16. When the user explicitly asks to update `AGENT.md`, add or revise the repo rules immediately as part of the active task instead of deferring it behind later implementation.
17. If the user says "plan first", "列出计划", or otherwise asks for planning before coding, write the plan into the repo, keep it updated while implementing, and continue executing unless a real blocker appears.
18. When a requested feature still has substantial unfinished work, keep decomposing the remaining scope into concrete phases and sub-steps until the path to completion is visible.
19. If the user asks for subagent usage or parallel investigation, prefer bounded subagent scouting tasks for independent research while the main agent keeps the critical path moving locally.
20. For requests that explicitly include browser verification or end-to-end proof, do not treat implementation as done until the browser-visible flow has been exercised or an explicit environment blocker is documented.

## Self-Learning Workflow

Agents must continuously learn from implementation work and feed those learnings back into the repo.

### Before work

1. Read the relevant sections of `README.md`, `AGENT.md`, and `LEARNINGS.md`.
2. Look for existing patterns before inventing new ones.
3. Check whether a past learning already answers the current problem.

### During work

1. If a bug, design constraint, or performance rule appears more than once, treat it as a reusable learning.
2. If a design decision changes the preferred project direction, record it.
3. If a workaround is required, mark whether it is temporary or strategic.
4. If a local environment problem blocks verification, distinguish clearly between code issues and machine/toolchain issues before trying further fixes.
5. If the task is non-trivial or cross-layer, keep a visible plan updated as steps complete or change.
6. If a user explicitly asks for "plan first", write or update the plan before deeper implementation and keep that plan synchronized with actual progress.
7. When work is only partially complete, identify the remaining items explicitly and queue the next execution batch instead of leaving the tail implicit.
8. If the task includes examples, docs, tests, and browser behavior, track each lane separately in the plan so nothing silently drops out of scope.
9. When example apps are changed in a way that affects entry flow, auth assumptions, or public onboarding, update the supporting docs and smoke expectations in the same execution stream.

### After work

1. Add or update a concise entry in `LEARNINGS.md` when a durable lesson was discovered.
2. Prefer writing a rule that future agents can apply, not a diary entry.
3. If a previous learning is outdated, replace it instead of stacking contradictions.

## Decision Heuristics

When choosing between options, prefer the one that most improves:

1. safety
2. performance
3. clarity of architecture
4. ease of future extension
5. developer ergonomics

Do not overvalue:

- short-term compatibility shims
- duplicate APIs for migration comfort
- "just in case" abstractions
- magic behavior that hides security or consistency boundaries

## Architecture Bias

For this repo, agents should default to these biases:

- prefer explicit policy over hidden runtime inference
- prefer compiled rules over dynamic interpretation on hot paths
- prefer event + worker coordination over tight cross-module coupling
- prefer native-looking API surfaces with strong server-owned enforcement
- prefer typed protocol models before adapter-specific implementations
- prefer TDD for core protocol, policy, worker, and gateway behavior

## API Design Bias

Frontend APIs should feel native, but internal design should stay rigorous.

- keep SQL, Mongo, Redis, MQTT, Kafka, and MQ concepts honest
- do not force stream or queue systems into fake query semantics
- add metadata when needed for correctness, even if the top-level call remains native-looking
- reject dangerous ambiguity early

## Learning Entry Format

Add new entries to `LEARNINGS.md` in this format:

```md
## YYYY-MM-DD - Short title

- Context:
- Decision:
- Why:
- Apply when:
- Avoid when:
```

## TDD Expectations

For core crates and protocols:

1. write or update a failing test first
2. implement the minimum change to make it pass
3. refactor only after green
4. keep real example configs as test fixtures whenever possible

## Debugging Expectations

When debugging, the default goal is to shrink the search space until the failing boundary is obvious.

1. start by identifying the smallest known-good boundary and the smallest known-bad boundary
2. prefer experiments that eliminate whole classes of causes instead of tweaks that only "might help"
3. add targeted logs, assertions, probes, or temporary tests at layer boundaries to learn which side is wrong
4. after each experiment, restate what has been ruled out, what remains possible, and the next narrower slice to inspect
5. compare failing behavior against a nearby working path in the same codebase whenever possible
6. do not stack speculative fixes; if the problem scope is not getting smaller, stop changing code and gather better evidence
7. when proposing a fix, explain which boundary was proven to fail and why the change addresses that exact point

Agents should optimize for answers like "the issue is somewhere in request parsing, not execution" and then "the issue is in SQL verb classification, not policy matching" until the root cause is isolated.

## Current Strategic Direction

These are active project-level preferences and should be reinforced unless the user changes direction:

- reserved values such as `UID` and `TENANT_ID` are first-class protocol concepts
- stream and queue backends are first-class modules, not second-class adapters
- workers are a core runtime, not an afterthought
- optimal architecture is preferred over preserving early rough edges
