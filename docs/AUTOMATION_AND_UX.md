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
- install the selected per-user RELAY background host
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

RELAY may automatically repair its own infrastructure when the action is local, deterministic, reversible/rebuildable, and non-destructive. Self-repair is not allowed to become invisible endless retry behavior.

Examples:

- reconnect an integration
- restart an internal worker
- rebuild a derived index
- choose a new internal port when safe
- refresh cached capability state

Self-repair must:

- record what failed and what RELAY changed
- use retry/rate limits
- stop and escalate after repeated failure
- avoid modifying project content
- avoid widening permissions to make a repair succeed
- surface persistent degradation in plain language

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

## Situation awareness and automation surprise

Automation research shows that reliable automation can still fail at the human-automation interface. RELAY should make its operating state legible.

The dashboard should always make it possible to determine:

- current automation mode/policy
- which client/agent requested active work
- what authority that client currently has
- which project is affected
- what is running/waiting/blocked
- what RELAY changed recently
- whether a result was verified
- how to pause/revoke/recover

Background automation must not require the user to reconstruct its behavior from raw logs after something goes wrong.

## Warning and alarm quality

Too many low-quality warnings can train users to ignore the dashboard.

Requirements:

- deduplicate repeated warnings
- show severity and confidence/source where meaningful
- keep healthy state visually quiet
- distinguish actionable failure from informational noise
- track noisy/false-positive checks during testing
- permit users to inspect why a finding exists
- do not keep stale resolved warnings prominent

## Approvals

The dashboard is a key approval surface. Human-in-the-loop prompting must be designed to avoid consent fatigue.

An approval should explain:

- requester
- intended action
- project
- affected objects/files
- risk/permission category
- dry-run or planned changes
- validation to be performed
- rollback capability
- maximum approved scope/change plan where applicable
- whether further approval will be required if scope changes

Prefer approving a bounded group of related actions over prompting for every trivial sub-step. New approval is required when the requested work materially exceeds the approved plan/risk.

UX testing should measure approval frequency, repeated low-value prompts, rejection/cancel behavior, and whether users can correctly explain what they approved.

## Automation levels

Possible user-facing profiles:

- Safe
- Balanced
- Hands-Off
- Custom

Exact semantics must be specified before implementation.

Default public behavior should favor understandable automation over silent mutation.

Automation profiles must define concrete permission/risk semantics; names such as Safe or Hands-Off are not sufficient by themselves.

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


## Adapter installation UX

Installing an adapter is a security-sensitive action and must not feel like installing a cosmetic theme.

Before enabling a third-party adapter, the dashboard should show:

- publisher/identity information available to RELAY
- exact adapter version/artifact
- source/update channel
- requested capabilities/permissions
- affected projects/resources
- external applications it will control
- network/credential requirements
- review/curation status
- compatibility status
- whether the adapter is first-party, curated, or unverified

The user should approve the bounded capability request rather than a vague "full access" prompt.

Updates that request new capabilities, change publisher/provenance, or materially change trust status require renewed review/approval.

Incompatible or quarantined adapters should explain the reason plainly and remain disabled rather than failing unpredictably.


## Data and privacy UX

Users need to understand when RELAY is keeping work local versus sending project-derived data elsewhere.

The dashboard should make it possible to see:

- current project privacy/data mode
- whether remote AI is enabled for the project
- which remote processors are connected
- which data classes each destination may receive
- recent outbound processing events
- whether provider policy metadata is current, stale, or unknown
- whether an operation is blocked by data policy
- whether a model response/result is sensitive

For a remote AI action, plain-language details should answer:

- what is being sent
- why it is needed
- where it is going
- whether images/files/source are included
- whether the user needs to approve a broader disclosure scope

Do not show a vague "AI enabled" toggle that silently authorizes the whole project.

### Credential UX

Credentials should appear as named connections, not visible secret strings.

Example:

~~~
GitHub
Connection: Work Account
Status: Connected
Scope: Project repository

[ Revoke ]
~~~

AI transcripts and ordinary dashboard views should never need to reveal the credential value.

### Local-only/private mode

If RELAY offers a local-only/private mode, the dashboard must explain the guarantee precisely.

Distinguish:

- no project-data egress
- no remote AI
- fully offline/no network

Do not collapse these into one misleading badge.

### Egress history

Users should be able to inspect recent remote processing without reading raw network logs.

Useful display:

- time
- requesting client
- project
- destination
- purpose
- data class/modality
- approximate payload size
- result/job reference

The display should avoid duplicating sensitive payload contents.


## Team and collaboration UX

For shared projects, the dashboard should make it easy to answer:

- who is currently working on this project
- which client/agent requested a change
- what project revision the plan is based on
- whether another change made the plan stale
- what role/scope the current user and agent have
- whether a connection is personal, project-owned, or workspace-owned
- whether a queued job will be canceled or revalidated after access changes

Conflict states should be plain:

~~~
This plan was approved against an older project state.
RELAY will re-check it before making changes.
~~~

Avoid silently replaying stale approvals.

Multi-agent views should show bounded task ownership and conflicts rather than a noisy transcript of every agent message.
