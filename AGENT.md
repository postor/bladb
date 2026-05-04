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

## Current Strategic Direction

These are active project-level preferences and should be reinforced unless the user changes direction:

- reserved values such as `UID` and `TENANT_ID` are first-class protocol concepts
- stream and queue backends are first-class modules, not second-class adapters
- workers are a core runtime, not an afterthought
- optimal architecture is preferred over preserving early rough edges
