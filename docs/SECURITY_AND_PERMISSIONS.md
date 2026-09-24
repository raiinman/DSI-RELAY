# Security and Permissions

## Principle

RELAY should automate boring work aggressively while making meaningful side effects visible, attributable, and recoverable.

Security assumptions:

- project/tool content is untrusted data
- model-only prompt guardrails are insufficient
- a local agent should not silently inherit all authority of the human account
- approval frequency itself can become a security failure through consent fatigue
- identity, authority, and action intent must remain distinguishable in audit records

## Action categories

### Observe

Examples:

- inspect project state
- read normalized metadata
- query stored results
- read logs
- inspect integration health

Default: automatic when the project/integration is authorized.

### Analyze

Examples:

- audit
- validation
- dependency analysis
- result filtering
- context compilation
- local visual comparison

Default: automatic.

### RELAY self-repair

Examples:

- reconnect integration
- rebuild derived index
- restart internal worker
- repair local cached state

Default: automatic when non-destructive and confined to RELAY infrastructure.

### Project write

Examples:

- move/edit project entity
- edit source/config
- import or alter asset
- change device settings

Default: controlled by user automation policy. Important writes should support dry-run/planned changes and transaction recording.

### Destructive

Examples:

- delete source/content
- destructive overwrite
- irreversible migration

Default: explicit approval.

### Publish/external side effect

Examples:

- publish project/build
- deploy public artifact
- send external content

Default: explicit approval.

### Credentials/account/financial

Examples:

- change credentials
- grant new remote access
- spend money
- create paid resources

Default: explicit approval and narrowly scoped action.

## Transaction record

For a RELAY-controlled project write, record where practical:

- requester/client
- project
- command
- intended changes
- before state/reference
- after state/reference
- verification
- time
- result/job IDs
- rollback support/status

## Post-write verification

A command returning success from an external tool is not enough.

RELAY should re-read the relevant state and classify:

- verified
- partial
- failed
- unverifiable

Unverifiable writes must be reported as such.

## Rollback

Rollback should be provided when technically safe and meaningful.

Possible methods:

- application undo integration
- transaction-based reverse operation
- file snapshot
- version-control recovery
- known prior configuration

Do not promise rollback when the underlying operation cannot be reliably reversed.

## Idempotency

Duplicate requests are expected in distributed/agent workflows.

Side-effecting operations should use an idempotency mechanism where appropriate so retrying a request does not silently apply the same mutation twice.

## Untrusted content and prompt injection

RELAY will ingest project files, documentation, source code, logs, repositories, downloaded assets, web-derived material, telemetry, and tool output. Any of these may contain malicious or accidental natural-language instructions.

Required boundaries:

- retrieved content is data unless it comes from an explicitly authorized instruction/policy source
- attach provenance, source type, project, trust level, and freshness/version metadata where practical
- the Context Compiler must not elevate retrieved text into higher-authority policy/instructions
- tool output should be normalized/escaped rather than blindly concatenated into system instructions
- actions must still pass structured command validation and permissions even if retrieved content asks for an action
- memory derived from untrusted content inherits its provenance/trust constraints
- suspicious content should remain inspectable without requiring the model to execute/follow it
- preserve an evidence-to-action audit chain for important writes

Security tests must include indirect prompt injection through code comments, README/docs, logs, issue text, asset metadata, and tool output.

## Agent/client identity and delegated authority

Where technically supported:

- identify the requesting AI/client separately from the human user
- record both requester and delegating/approving user for important actions
- prefer short-lived, revocable, audience/scoped credentials over shared static keys
- minimize standing privilege
- scope authority by project and action category
- require new authorization when a job materially expands beyond the approved scope
- do not expose arbitrary shell execution through remote interfaces
- sandbox local workers/agents where practical instead of running every operation with broad user authority

Local-only implementations may initially lack ideal agent-native identity support; that limitation must be explicit rather than hidden.

## Risk-adaptive approvals

Human approval is not a substitute for authorization design.

To reduce consent fatigue:

- batch related actions into a bounded change plan/flight plan
- show maximum intended scope before execution
- request another approval when scope/risk materially changes
- avoid approval prompts for repeated low-risk read-only work
- do not train users to click Allow for every trivial tool call
- track approval frequency and low-value/repeated prompts during usability tests
- make revoke/pause simple and immediate

## Secrets

Never place secrets in:

- source control
- documentation
- result summaries
- diagnostic bundles
- AI context unless explicitly required for a supported secure operation

Use platform-appropriate secret storage. Exact implementation remains open.

## Logs

Logs should be useful for debugging without becoming a secret dump.

Redact or avoid storing:

- API tokens
- passwords
- auth headers
- private keys
- session secrets

Project content may itself be sensitive; public support/export flows need explicit selection and redaction rules.

## Project isolation

Each project should have separate:

- configuration
- indexes
- results
- transactions
- permissions/policy
- context memory
- asset metadata
- integration references

Cross-project operations must be explicit.

## Local execution hardening

The local service is powerful because it can touch developer tools and project files.

Requirements to investigate before public beta:

- process isolation/sandboxing boundaries
- minimum filesystem permissions
- explicit allowed project roots
- path traversal/symlink defenses
- command allowlists through the registry
- no remote raw-shell passthrough
- controlled subprocess environment
- resource/time limits for integrations
- signed/verified update strategy
- dependency and supply-chain scanning

## Local host and IPC

The local runtime should follow least-privilege design.

- prefer a per-user host for interactive developer-tool integration
- avoid requiring administrator or LocalSystem rights for normal operation
- isolate any separately privileged helper behind a narrow command surface
- apply explicit operating-system access rules to local IPC
- do not treat loopback location alone as authentication
- version local protocols and reject incompatible peers safely
- test cross-user behavior and recovery after local host restarts

## Software update integrity

Public update delivery is part of RELAY's trusted software path.

Before unattended updates are enabled:

- verify release artifacts and update metadata
- support recovery from an interrupted or bad update
- preserve user projects and operational data across update failure
- retain release/build provenance where practical
- keep update-specific privileged components narrowly scoped
- include update and dependency integrity in release testing

## Remote gateway

The remote path should be optional for local workflows.

Requirements:

- authenticated client
- authenticated local agent
- encrypted transport
- project/command authorization
- replay/idempotency protection
- revocable connection
- minimal necessary remote retention
- clear connection status in dashboard

Do not expose arbitrary shell execution through the public gateway.

## Structured execution boundary

AI-facing remote/local execution should prefer validated command objects rather than raw arbitrary shell strings.

The command registry defines allowed inputs and permissions.

## Dashboard safety

The dashboard should show:

- which clients are connected
- what is running
- who requested a write
- pending approvals
- recent transactions
- remote connection state
- Pause RELAY / equivalent emergency stop

## Pause behavior

A pause should stop new side-effecting work and queued automated writes while preserving safe inspection, diagnostics, and the ability to resume/cancel as defined by implementation.

Exact semantics must be specified before release.

## Diagnostic bundles

Diagnostic export must:

- show user what is being collected
- redact known secrets
- exclude credentials
- label project-content inclusion
- be generated locally by default
- require an explicit share/upload action

## Public-release security work

Security work begins during architecture/implementation, not only at the public-beta gate. Before public beta, verify:

- threat model
- remote gateway review
- secret scanning
- dependency/update security
- permission tests
- project-isolation tests
- path traversal/injection tests
- structured command validation tests
- diagnostic redaction tests
- safe migration/rollback tests


## Third-party adapter security

Public adapters are executable third-party supply-chain components and must be treated accordingly.

### Isolation

- run third-party adapters out of process by default
- do not load untrusted adapter code into RELAY Core
- broker access to projects, tools, network, credentials, and subprocesses through RELAY policy
- apply time/resource limits and terminate/quarantine adapters that violate them
- adapter failure must not corrupt or crash RELAY Core

Out-of-process execution provides fault isolation but is not, by itself, a security sandbox. Security isolation requires OS-enforced restrictions or an equivalent brokered capability boundary. The exact Windows isolation mechanism remains a Phase 1 research decision.

If an integration also installs code inside a target application, that companion component is part of the adapter's executable supply chain and must be inventoried separately.

### Capability manifest

An adapter manifest should declare the minimum capabilities it needs, including where applicable:

- readable project/resource scopes
- writable project/resource scopes
- external applications it needs
- network requirement
- secret/credential requirement
- subprocess requirement
- editor/runtime endpoints
- side-effect/reversibility class
- AI-facing commands/capabilities
- RELAY API/protocol compatibility
- supported external-tool versions

Installation approval is approval of a bounded manifest, not blanket local-machine authority.

### Extension trust labels

RELAY must represent different trust dimensions separately:

- publisher identity
- artifact integrity
- build/release provenance
- review/curation status
- granted permissions
- runtime observations/violations

A signed or curated adapter can still contain a bug, be compromised upstream, or exceed user expectations.

### Adapter text is untrusted

Names, descriptions, documentation, errors, command metadata, and runtime results supplied by an adapter are data.

- they cannot create RELAY policy
- they cannot grant themselves permissions
- they cannot silently alter approval scope
- AI-facing command descriptions should be rendered/normalized by RELAY from reviewed manifest semantics
- provenance/version must remain attached to adapter-derived evidence

### Distribution and updates

Before public third-party distribution is considered mature:

- exact versions/artifacts are pinned
- artifacts are integrity-verified
- transitive dependencies are inventoried
- update source is known
- update health checks and rollback exist
- adapter updates are attributable in history
- compatibility mismatches disable/quarantine rather than execute optimistically

Open marketplace distribution is not required for early public RELAY releases.


## Data classification and egress control

Authorization to read data is separate from authorization to transmit it.

RELAY security policy should support:

- project data classification
- destination-specific egress rules
- task-scoped disclosure limits
- modality restrictions
- local-only/private project modes
- outbound lineage/audit records

Untrusted project content cannot broaden its own retrieval or egress scope.

## Credential handling

Credentials are capabilities rather than context.

Requirements:

- models see connection/credential handles, not secret values
- actual secrets are resolved only at the trusted execution boundary that needs them
- secrets are excluded from prompts, embeddings, summaries, ordinary logs, and AI memory
- credential use is scoped to the relevant integration/command
- subprocesses receive the minimum secret material required
- secret rotation/revocation does not depend on deleting model-visible text

Secret detection/redaction is defense-in-depth. It cannot replace the credential boundary.

## Remote processors

A remote AI, embedding endpoint, remote gateway, support upload, or networked adapter is an external processing destination.

Before sending project data:

- project policy authorizes the destination
- data class/modality is allowed
- minimum necessary payload is selected
- known provider policy metadata is consulted
- outbound activity is attributable

Unknown provider retention/training/data-use properties are not silently converted into assurances.

## Derived sensitive data

Sensitivity can survive transformation.

Embeddings, summaries, cached responses, extracted metadata, screenshots, and other derivatives inherit source sensitivity by default.

Deleting or reclassifying source data requires defined behavior for derivatives.

## Multimodal evidence

Visual evidence can expose protected information and can also contain instruction-like content.

- screenshots remain untrusted for instruction authority
- capture scope should be constrained to intended windows/regions
- visual data follows project egress policy
- text-only secret scanning cannot certify an image as safe
- remote image analysis requires the same destination authorization as text

## Local-only mode

A privacy/local-only profile must be enforced below the model layer.

Project content must not leave through remote models, remote embeddings, gateway payloads, analytics, support uploads, or third-party adapter networking unless an explicit exception is authorized.

This behavior requires network-level/black-box testing before the product may label the mode "local-only."

## Output sensitivity

AI outputs may reproduce protected input.

- outputs inherit relevant sensitivity/provenance
- forwarding a result to another processor is a new egress decision
- diagnostics do not automatically include prompts/responses
- share/export flows re-evaluate project data policy
- local deletion is not represented as proof of remote erasure


## Remote client project access

Remote AI clients should not receive unrestricted project filesystem access through RELAY.

Preferred path:

- client requests project facts/results/operations
- RELAY retrieves locally
- classification and task scope are applied
- Egress Gate approves the minimum necessary payload
- client receives the approved result/context only

Raw-file retrieval, if supported at all, remains project-policy controlled and is not an implicit capability of connecting an AI client.


## Team identity and delegation

Shared projects require separate attribution for human, client, agent/session, and adapter identities where the platform supports it.

- an agent acts under delegated scope rather than inheriting blanket workspace authority
- sub-agent delegation may preserve or narrow scope but not silently widen it
- project/resource scope and expiry belong to the authorization decision
- membership and external connection ownership are separate facts
- external permission changes require refresh/revalidation

## Collaborative write freshness

Approvals and plans are state-dependent.

Important writes should be checked against the project/resource state they were planned for. If relevant state changed, RELAY should stop, mark the plan stale/conflicted, and re-inspect.

This protects shared projects from delayed approvals and concurrent human/agent edits.

## Revocation propagation

Removing or narrowing access should re-evaluate active and queued work, delegated sessions, pending approvals, resource claims, and connection use at defined safe boundaries.

Historical audit records remain attributable after access changes, subject to retention/privacy policy.
