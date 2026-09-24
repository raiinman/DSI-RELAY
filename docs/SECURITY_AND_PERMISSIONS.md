# Security and Permissions

## Principle

RELAY should automate boring work aggressively while making meaningful side effects visible, attributable, and recoverable.

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

Before public beta:

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
