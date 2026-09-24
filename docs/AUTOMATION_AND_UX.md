# Automation and UX

## Experience goal

RELAY should remove infrastructure work from the creator's normal workflow.

The ideal first run:

~~~
Install RELAY
 -> open dashboard
 -> detect supported tools
 -> discover or add a project
 -> build initial index
 -> validate integrations
 -> create baseline
 -> show what needs attention
 -> optional AI connection
 -> work
~~~

Manual configuration exists as an escape hatch, not the default.

## Installation automation

Target behavior:

- install CLI
- install daemon/service
- install dashboard
- initialize local storage
- register safe startup behavior
- create required local configuration
- verify installation
- explain any missing dependency plainly

Packaging technology remains open.

## Tool discovery

RELAY should check standard installation/discovery paths before asking the user.

Initial targets:

- UEFN / supported Unreal components
- Fortnite runtime availability where detectable
- Blender
- Krita
- Git
- required local runtimes used by RELAY

The dashboard should show friendly status first and exact paths/versions in Advanced details.

## Project onboarding

When adding a project, RELAY should automatically:

- identify project type
- assign stable RELAY project identity
- detect integrations
- scan relevant structure
- build initial indexes
- register assets/tests/instrumentation it can identify
- validate configuration
- create a baseline
- produce a first audit
- report limitations/capability gaps

Avoid forcing users to author large configuration files before useful output exists.

## Incremental indexing

After baseline, watch for relevant changes and update only affected state.

Do not require repeated full-project rescans when local change detection is sufficient.

## Capability negotiation

Capabilities are per project and per current integration state.

Examples:

- static project inspection available
- UEFN editor control unavailable because editor is closed
- Blender build available
- runtime probes unavailable because play session is not active

Clients ask RELAY what is available rather than assuming.

## Automatic health checks

RELAY should monitor:

- daemon health
- project watcher health
- integration connectivity
- version compatibility
- stale sessions
- storage/index health
- remote gateway state when enabled

## Safe self-repair

RELAY may automatically repair its own infrastructure when the action is local, deterministic, and non-destructive.

Examples:

- reconnect an integration
- restart an internal worker
- rebuild a derived index
- choose a new internal port when safe
- refresh cached capability state

Never disguise a project-content modification as self-repair.

## Testing automation

RELAY should:

- map changes to affected checks
- run cheap deterministic checks automatically where configured
- reuse unchanged results
- support full suites at explicit gates
- record test result IDs
- compare with baseline/previous results
- surface regressions

## Visual regression

Where captures are available:

- use stable named viewpoints
- retain before/after evidence
- compare locally
- flag significant changes
- send only relevant images to AI by default

## Dependency graphs

RELAY should build useful relationships where data supports them.

Examples:

- device -> Verse handler -> reward path
- asset -> material -> texture
- gameplay object -> trigger -> objective
- test -> affected subsystem

Use graphs for impact analysis and affected-only testing, not as decorative metadata.

## Dashboard

The dashboard is the human control room.

### Global project screen

Show:

- projects
- current health
- active jobs
- integrations
- recent regressions/findings
- aggregate usage
- Add/Import Project

### Project screen

Planned sections:

- Overview
- Map/Scene
- Devices/Entities
- Tests
- Assets
- Telemetry/Probes
- Results
- History/Transactions
- Integrations
- Usage
- Settings
- Diagnostics/Advanced

The exact navigation may evolve through UX testing.

### Top-level actions

Common actions may include:

- Scan
- Audit
- Run Tests
- Capture
- Pause RELAY

Buttons must call the same core command system as CLI.

## Plain-language policy

Primary UI copy answers user intent, not protocol implementation.

Prefer:

"UEFN disconnected. RELAY is trying to reconnect."

over:

"WebSocket abnormal closure 1006."

Technical detail remains available under Advanced.

## Approvals

The dashboard is a key approval surface.

An approval should explain:

- requester
- intended action
- project
- affected objects/files
- risk/permission category
- dry-run or planned changes
- validation to be performed
- rollback capability

## Automation levels

Possible user-facing profiles:

- Safe
- Balanced
- Hands-Off
- Custom

Exact semantics must be specified before implementation.

Default public behavior should favor understandable automation over silent mutation.

## Background/queued work

Jobs may wait for dependencies.

Example:

~~~
job created
 -> waiting for UEFN
 -> user opens UEFN
 -> adapter becomes ready
 -> job runs
 -> result stored
~~~

Do not spam chat with progress. The dashboard owns detailed live progress.

## Diagnostics and support

Provide a one-click diagnostic bundle that gathers:

- RELAY version
- OS/runtime basics
- integration versions/state
- configuration validation
- recent relevant errors
- service health
- non-sensitive project metadata as needed

Redact:

- tokens
- passwords
- API keys
- secrets
- account identifiers where not required
- sensitive paths/content where practical

The user should be able to inspect what will be included before sharing.

## Accessibility and clarity

The dashboard should not require familiarity with MCP, agent jargon, or internal component names to understand basic health and next actions.

Advanced users still need complete raw/technical access.
