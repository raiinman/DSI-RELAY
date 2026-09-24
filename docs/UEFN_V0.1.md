# UEFN / Fortnite v0.1 Integration

## Purpose

Use UEFN/Fortnite as the first production integration to prove RELAY's core ideas:

- project/editor observability
- runtime visibility
- deterministic inspection
- compact AI results
- safe actions
- test instrumentation
- screenshots
- asset workflows
- usage reduction

UEFN is the first adapter, not the architecture.

## Current platform reality

Fortnite 42.00 added Unreal MCP to UEFN, but Epic documents the underlying Unreal MCP feature in UE 5.8 as Experimental, with incomplete/missing features and APIs/data formats subject to change.

Therefore:

- Unreal/UEFN MCP is a useful adapter surface, not a RELAY Core dependency
- capability and version detection are mandatory
- commands must report unsupported/unavailable capability accurately
- Epic-specific schemas stay inside the adapter
- RELAY needs integration tests across explicitly supported UEFN versions
- fallback/non-MCP paths should exist where practical rather than being assumed impossible or guaranteed

## Integration layers

### Static project layer

Capabilities that can work from project files/metadata without a live editor where technically supported.

Potential responsibilities:

- project identification
- Verse source discovery
- asset/source registry
- RELAY configuration
- static validation
- cached project index
- source change detection

### Editor layer

Live UEFN operations where supported by the available UEFN/Unreal integration surface.

Native-tool-first rule:

Before RELAY implements a validator, profiler, session inspector, transaction/undo mechanism, or other engine-level check, determine whether UEFN/Unreal already exposes an authoritative capability. Prefer consuming and normalizing native evidence over maintaining a parallel approximation. RELAY should add value through correlation, history, automation, cross-tool state, context compilation, and gaps the native tooling does not cover.

Potential responsibilities:

- entity/device inspection
- transforms
- selected modifications
- editor session state
- play-session control
- client/output log access
- capture coordination

Exact transport/API support must be validated against target UEFN versions before implementation.

### Fortnite runtime layer

Verse instrumentation and runtime telemetry.

Responsibilities:

- structured event output
- gameplay probes
- assertions
- runtime state markers
- debug visualization where supported
- test-state support where feasible

## Runtime bridge

Rather than attempting an unrestricted external TCP server inside the game runtime, v0.1 should use supported UEFN/Verse mechanisms to expose structured telemetry and debug instrumentation.

Conceptual modules:

~~~
relay_core
relay_logger
relay_players
relay_spawns
relay_devices
relay_objectives
relay_tests
relay_probes
relay_debug_draw
relay_capture
~~~

Actual Verse file/module layout will follow UEFN constraints.

## Structured telemetry

Runtime messages should be machine-readable and easy to filter.

Conceptual events:

- session start/end
- player spawn
- elimination
- item/collectible event
- objective/trigger event
- progression change
- assertion result
- probe observation
- test-state marker

RELAY should parse and normalize them locally before an AI sees them.

## Probes

Probes are targeted temporary/optional instrumentation.

Initial candidates:

- spawn probe
- player probe
- weapon/progression probe
- collectible probe
- trigger/objective probe
- round probe
- damage probe
- zone probe
- performance probe where supported

A probe should answer a focused debugging question without enabling every telemetry stream all the time.

## Spawn inspector

v0.1 flagship workflow.

Capabilities:

- discover player spawners
- assign stable RELAY references
- inspect transforms/team metadata available to the adapter
- visualize/name spawns in editor/runtime where supported
- measure clearances/distances where data allows
- detect obvious duplicate/invalid configuration
- produce compact findings
- preserve exact coordinates/evidence

This demonstrates why RELAY is more useful than generic editor clicking.

## Gameplay assertions

Provide an assertion/test harness for project-specific tests without hard-coding one game's rules into RELAY Core.

Examples:

- expected event occurred
- expected grant/progression happened
- player reached expected state
- objective fired
- respawn occurred
- device path produced expected outcome

Tests live in project configuration/instrumentation and produce normalized RELAY test results.

## Test states

Investigate repeatable ways to enter known test scenarios without replaying long setup flows.

Examples:

- known round phase
- known progression index
- known test location
- predefined inventory/state

Only use mechanisms permitted by UEFN/Fortnite. Do not assume arbitrary runtime mutation is available.

## Debug visualization

Where supported, visualize:

- spawns
- facing direction
- trigger/zone boundaries
- identifiers
- objective links
- probe targets

These are development aids and must not become published gameplay content unintentionally.

## Fixed camera/capture system

Define named reference views.

RELAY should:

- request/capture repeatable views where supported
- store capture result IDs
- compare against baseline/previous capture
- identify meaningful differences locally when possible
- send only relevant images to AI by default

## UEFN audit

relay audit is an aggregator/orchestrator, not a replacement engine validator.

Each finding should identify its source when practical, for example:

- Epic/UEFN native validation
- RELAY deterministic check
- runtime assertion
- asset adapter
- user/project-defined test

relay audit for a UEFN project should grow toward:

- project/index health
- Verse compile/error state where available
- spawn/device/entity findings
- broken/missing references detectable by the adapter
- runtime test results
- asset validation
- performance/memory signals available through supported tooling
- integration/version health
- native validation/session/profiler evidence where available through supported interfaces

v0.1 may implement a subset, but the result envelope and extension model should support the full direction.

## Asset workflow

### Blender

Prefer headless/background execution for deterministic tasks when supported.

Planned checks/work:

- scale/dimensions
- pivots
- normals
- UV metadata
- collision metadata
- LOD metadata
- triangle/mesh statistics
- export

### Krita

Use for texture/2D asset workflows through an adapter where available.

Planned work:

- texture/source generation workflows
- signage/decals/UI art
- dimensions/format validation
- source-to-export tracking

Asset generation is not required to be AI-driven. RELAY should track source, validation, export, and project relationships regardless of creator.

## Asset manifest

Each project may maintain normalized asset records:

- stable ID
- source
- exported artifact
- owning integration
- validation results
- target project location
- related material/texture/mesh metadata
- build history

Exact storage format remains open.

## v0.1 acceptance tests

A supported UEFN project should be able to:

1. appear in RELAY project registry
2. report integration availability
3. be inspected without sending a whole project to AI
4. discover and report spawns
5. produce a compact audit result
6. store/retrieve full evidence by result ID
7. emit and parse structured runtime telemetry
8. run at least one project-defined gameplay assertion
9. record a verified transaction for at least one safe supported write
10. display the same underlying state in CLI and dashboard
11. expose usage metrics for the workflow
12. report UEFN adapter/version capabilities explicitly
13. distinguish native-engine findings from RELAY-derived findings

## Known boundary

"RELAY runs headlessly" does not mean "UEFN and Fortnite can perform every command without their applications running."

Commands must declare dependencies and return clear blocked/unavailable states.


## Legal/platform automation boundary

UEFN is the first integration, but RELAY must distinguish documented developer automation from general Fortnite automation.

Current engineering rule:

- use Epic-documented UEFN MCP/editor/play-session tooling
- do not automate Fortnite gameplay/player-client use outside documented developer/test workflows
- do not attempt to bypass gameplay-integrity controls
- record the Epic terms/documentation versions used for the adapter compatibility review

If future UEFN features require automation beyond clearly documented developer interfaces, public release requires legal/official clarification.

## Published runtime networking boundary

Current UEFN terms prohibit published Developer-Made Content/code from connecting to non-Epic servers.

Therefore the runtime bridge must not become a published phone-home client.

Use supported Verse/UEFN logs, debugging, play-session inspection, or Epic-hosted mechanisms. Any development-only RELAY component must be excluded from publishable content.

## Content rights metadata

UEFN projects may mix:

- developer-authored content
- Epic assets
- third-party licensed assets
- marketplace/Fab content
- generated/AI-assisted content

RELAY should preserve the provenance/license distinction and avoid representing project inclusion as proof of ownership.
