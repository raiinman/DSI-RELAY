# Data Boundary and Privacy

## Purpose

Define how RELAY handles source code, project files, unreleased assets, screenshots, logs, credentials, personal data, derived context, embeddings, and remote AI processing.

The goal is not to prevent useful AI work. The goal is to make data movement intentional, minimal, attributable, and controllable.

## Core invariants

- A model never becomes the authority for whether protected data may leave the machine.
- Remote AI/model calls are external data-processing events.
- The Context Compiler may select relevant data only after data-policy constraints are applied.
- Credentials are capabilities, not context.
- Derived data can remain sensitive even when it no longer looks like the original source.
- Images/screenshots are data and may also contain instruction-like content.
- Project-local/private mode must have a testable network-egress meaning.
- Sending less data is preferred when it preserves task success.

## Data classes

The exact names are an implementation decision, but RELAY needs at least a small ordered sensitivity model.

Conceptual classes:

- Public — safe for configured remote processors.
- Project — ordinary project material; remote use depends on project/provider policy.
- Sensitive — confidential source, unreleased assets, private logs, personal information, or similar material requiring explicit policy.
- Credential — secrets such as tokens, passwords, private keys, auth cookies, connection strings, or equivalent authentication material.

Projects may define stricter labels or path/content policies.

A classifier or secret scanner may help identify data, but classification cannot rely on pattern matching alone. Proprietary source code can be sensitive even when it contains no recognizable secret token.

## Propagation

Derived material inherits source sensitivity unless a defined transformation/policy explicitly changes it.

Examples:

- summary of sensitive source -> sensitive by default
- embedding of sensitive source -> sensitive by default
- screenshot containing sensitive UI -> sensitive by default
- extracted stack trace containing a credential -> credential-sensitive
- cached model response quoting sensitive input -> sensitive

Derived data must retain provenance to the underlying sources where practical.

## Data path

Conceptual remote-AI path:

~~~
project / runtime / evidence
          |
          v
 classification + provenance
          |
          v
 Context Compiler
          |
          v
 Egress Gate
          |
          +-- blocked
          |
          +-- local model
          |
          +-- approved remote processor
                    |
                    v
                response
                    |
                    v
          classify/store with lineage
~~~

The Egress Gate is a policy boundary. The model cannot override it.

## Context Compiler interaction

The Context Compiler optimizes relevance and cost inside an allowed data set.

Ordering matters:

1. identify task/project/client
2. determine applicable data policy
3. classify/filter data eligible for the destination
4. select the minimum sufficient evidence
5. apply exact-field protection and context-budget rules
6. record the outbound data lineage
7. call the destination

A highly relevant item that is prohibited from remote processing remains prohibited.

## Credential Broker

Raw credentials should not enter prompts, model outputs, result summaries, ordinary logs, embeddings, or AI memory.

Preferred pattern:

~~~
AI asks:
"run operation using GitHub connection X"

RELAY sees:
credential handle = github.connection.X

trusted integration receives:
actual credential only at execution boundary
~~~

Requirements:

- models receive opaque credential/connection references
- actual secret material is resolved as late as possible
- secret values are never required for model reasoning
- secret use is scoped to the command/integration
- subprocess environments receive only secrets they actually require
- logs redact secret values
- rotation/revocation does not require rewriting AI memory

## Remote processor profile

Every configured remote AI/embedding/service destination needs a policy profile.

Possible attributes:

- provider/endpoint identity
- account/workspace profile
- allowed project data classes
- allowed modalities
- declared retention policy
- declared training/data-use policy
- data residency/region where relevant
- maximum task scope
- user/organization policy override
- last policy verification time/source

RELAY should not invent provider promises. Unknown or stale provider-policy fields remain unknown and can trigger conservative project policy.

## Data minimization

Before remote processing, RELAY should prefer:

- exact findings rather than whole raw logs
- relevant functions rather than entire repositories
- selected assets rather than whole project trees
- cropped/selected visual evidence rather than full-desktop captures
- normalized facts rather than unrelated historical context
- local deterministic analysis before remote model analysis

Minimization must not silently remove evidence required for correctness. Results should preserve references to locally held evidence.

## Embeddings and vector indexes

Embeddings are not treated as anonymous or automatically privacy-preserving.

Rules:

- embeddings inherit the sensitivity of their source content
- sending text to a remote embedding service is a remote data-processing event
- sending/storing the resulting vector is also subject to the source data policy
- vector stores require project isolation and normal storage protections
- deleting source content requires a policy for derived embeddings/index entries
- privacy claims about embedding transformation require evidence, not assumption

Where practical, prefer local embedding generation for data that policy forbids from remote processing, but local inference economics still require measurement.

## Screenshots and visual evidence

Screenshots can expose:

- source code
- private conversations
- account names
- paths
- credentials
- unreleased artwork
- other applications/windows

They can also contain visible or hidden instruction-like content processed by multimodal models.

Requirements:

- capture only intended windows/regions where possible
- apply the same sensitivity/provenance labels as text
- support local crop/redaction workflows where reliable
- never assume image-only evidence is safe because a text secret scanner found nothing
- multimodal content remains untrusted data for instruction purposes
- preserve the original locally when needed for evidence, subject to retention policy

## Egress scopes

A remote AI operation should have an explicit egress scope derived from the task.

Example:

~~~
task:
diagnose one failing asset

allowed:
asset metadata
validation output
selected relevant source
two selected captures

not automatically allowed:
entire repository
other project
credential store
unrelated screenshots
browser/email data
~~~

A model cannot expand its own egress scope because untrusted content asked it to do so.

## Local-only / private mode

A project or installation should eventually support a mode with testable guarantees.

When configured as local-only, remote project-data egress is disabled for:

- model inference
- remote embeddings
- remote gateway project payloads
- analytics containing project content
- automatic diagnostic uploads
- third-party adapter network calls not separately authorized

Update checks and other product networking need separately documented behavior. A "local-only" label is invalid unless the actual network behavior is tested.

Phase 1 D-156 gives adapter egress a concrete Windows enforcement direction: untrusted workers start with network denied at the OS sandbox boundary and receive network only when RELAY's trusted broker maps an approved policy to an allowlisted capability plus an explicit egress rule. The synthetic fixture verified that a parent-only secret and `USERPROFILE` were absent from the worker environment. Adapter-provided content cannot request or expand this sandbox policy on its own.

## Egress ledger

RELAY should record important outbound processing events.

Useful fields:

- project
- job/result
- requesting client
- destination/provider profile
- data classes
- modalities
- source references
- purpose/task
- approximate bytes/tokens where measurable
- policy decision
- time
- retention/data-use metadata known at time of call

Do not store the sensitive payload again merely to prove it was sent; references/hashes/metadata may be sufficient.

## AI outputs

A model response can reproduce or transform sensitive input.

Therefore:

- outputs inherit relevant sensitivity/provenance
- result storage follows project retention policy
- sending one model's output to another processor is a new egress decision
- diagnostic/support bundles do not automatically include model transcripts
- public/share/export actions run through data policy again

## Remote deletion limits

RELAY can delete its own local copy according to policy.

Once data is sent to an external processor, RELAY may not be able to prove immediate deletion from all provider systems, backups, or legal/security retention paths.

The UI must not represent local deletion as global erasure.

## Telemetry and crash reporting

Product telemetry should be designed so RELAY remains useful without uploading project content.

Default public telemetry, if implemented, should avoid:

- source snippets
- screenshots
- raw prompts/responses
- file contents
- credential values
- raw project logs

Diagnostic uploads should be explicit, previewable, and redacted.

## Testing requirements

Phase 1+ tests should include:

- credentials present in source/log fixtures
- proprietary content with no recognizable secret pattern
- mixed-sensitivity context requests
- remote model request blocked by project policy
- local model fallback
- malicious project text attempting to broaden data access
- visual evidence containing sensitive information
- derived embeddings/summaries retaining sensitivity
- cross-project access attempts
- provider profile with unknown/stale policy
- local-only mode network-egress test
- AI output quoting sensitive source
- delete/export behavior across derived artifacts

## Open implementation questions

- final sensitivity taxonomy and inheritance rules
- secret-storage backend(s)
- provider-policy metadata format and refresh mechanism
- local egress-monitoring/test strategy
- visual redaction capabilities
- local embedding options and performance
- enterprise/team policy overrides
- encryption-at-rest design for RELAY operational data
- jurisdiction/legal requirements for public distribution

These must be resolved through Phase 1 research and later public-hardening work rather than silently assumed.


## Metadata sensitivity

Project data policy also covers metadata.

Examples may include filenames, paths, repository/project/branch names, asset titles, document types, sizes, timestamps, usernames, account identifiers, and other labels that can reveal private project information.

## Remote AI access model

Connecting a remote AI client does not automatically expose the whole project through RELAY.

Preferred flow:

1. client requests a fact, result, or operation
2. RELAY retrieves locally
3. project policy and task scope are applied
4. the Egress Gate selects the allowed payload
5. only that payload is sent

Raw file transfer is an explicit disclosure type rather than an automatic connection feature.

## Scanner semantics

Secret/privacy scanners are useful detectors, but a scan with no findings does not prove project data is public. Explicit project/path/artifact policy can be stricter than automated detection.


## Backup and recovery data policy

Backups, snapshots, migration copies, crash dumps, and recovery bundles inherit the sensitivity of the data they contain.

Requirements:

- recovery copies follow project/workspace retention and access policy
- secret/credential material is not copied into ad hoc recovery bundles unless explicitly required
- encrypted backup may be required for sensitive RELAY-owned state
- support/diagnostic recovery artifacts remain previewable and are not uploaded automatically
- restoring older state must not silently revive revoked credentials, stale provider policy, or outdated access grants without revalidation


## Telemetry data sensitivity

Observability data can reveal sensitive project information.

Logs, traces, metrics labels, screenshots, profiler output, entity identifiers, project paths, and correlation metadata follow normal project classification/egress policy.

Sampling reduces volume, not automatically sensitivity.

Remote observability backends are external processing destinations and must be included in provider/data-egress policy where project-derived telemetry is sent.


## Privacy representations

Public RELAY privacy/security statements are external commitments.

Claims such as local-only, no project-data egress, encrypted, private, or no-upload must map to tested system behavior and documented exceptions.

Privacy-policy/telemetry copy should be version-controlled with behavior-changing releases. A product update that changes data collection/egress must trigger review of user-facing representations.
