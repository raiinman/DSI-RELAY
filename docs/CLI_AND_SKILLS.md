# CLI and Skills Strategy

## Decision

Structured headless execution is the stable local machine contract, and the CLI is its canonical human/script surface.

Compact skills + CLI are the leading initial strategy for local AI clients, but recent tool-interface research means this must be benchmarked rather than treated as universally superior.

MCP remains an adapter/transport option. A thin dynamically discovered MCP surface may be competitive with or better than CLI+skill for some clients.

## Why CLI-first

These are reasons to keep CLI/headless as a stable interface, not proof that every AI client should use CLI as its best tool-selection surface.

CLI gives RELAY:

- low context overhead
- easy scripting
- easy CI use
- reproducibility
- straightforward testing
- shell/SSH compatibility
- no dependence on one AI ecosystem
- stable machine-readable invocation
- simpler debugging

The CLI is an interface to RELAY Core. It does not own business logic.

## Process model

Planned conceptual split:

- relay — command-line client
- relayd — persistent local host for RELAY Core

A one-shot CLI command may connect to relayd or invoke a supported local path, depending on the final implementation. The caller should see one stable contract.

## Human commands

Target style:

~~~
relay status
relay project list
relay project add <path>
relay audit <project>
relay result <result-id>
relay test run --affected
relay integration status
relay transaction list
relay transaction rollback <id>
~~~

Exact syntax remains subject to implementation.

## Machine execution

AI and automation should prefer a strict structured request rather than composing arbitrary shell strings.

Conceptual request:

~~~
{
  "command": "spawns.validate",
  "project": "project-id",
  "arguments": {
    "team": "red"
  },
  "context_budget": 2000
}
~~~

Conceptual invocation:

~~~
relay exec --stdin --json
~~~

Benefits:

- schema validation
- less quoting trouble
- fewer hallucinated flags
- easier permission checks
- easier idempotency
- safer logging
- consistent remote forwarding

## No surprise prompts

Machine/headless commands must not unexpectedly wait for interactive confirmation.

Use explicit modes such as:

- dry-run / plan
- apply
- approval token/transaction where needed

If approval is required and missing, return a blocked result instead of hanging.

## Exit codes

Exact values are open, but the CLI needs stable categories for:

- success
- validation failure
- bad request
- dependency unavailable
- permission/approval required
- target offline
- timeout
- partial completion
- internal failure

Structured output remains the detailed source of truth.

## Compact skills

A RELAY skill should be small and task-oriented.

Potential packages:

- relay-core
- relay-uefn
- relay-runtime-testing
- relay-blender
- relay-krita
- relay-assets
- relay-debugging
- relay-performance

The agent should not load every domain at once.

## Skill contents

A typical skill may contain:

- SKILL.md — operating principles and common workflows
- reference/ — domain-specific details loaded as needed
- scripts/ — thin wrappers for common commands

Skills should teach rules like:

- query before modifying
- prefer structured JSON
- use compact output
- use result IDs
- retrieve raw evidence only when needed
- dry-run destructive changes
- verify state after writes
- let RELAY aggregate large project data locally

## Shell wrappers

Wrappers can encode safe common sequences so an AI does not have to remember many flags.

Example conceptual wrapper:

~~~
relay-audit <project>
  -> audit
  -> severity filter
  -> compact result
  -> JSON
~~~

Wrappers must not create hidden business logic that diverges from RELAY Core.

## Capability discovery

The client should discover only what it needs.

Examples:

~~~
relay help spawns
relay capabilities --project <id>
relay command describe spawns.validate
~~~

This avoids preloading a giant command schema.

Phase 1 D-154 implements this as compact `registry.list` discovery plus on-demand `registry.describe`. The current CLI's local catalog/help/describe rendering is derived from the same registry used by the host validator, and command capability IDs include their contract version. AI clients should prefer the compact list and fetch one full contract only when needed.

## Single command metadata source

The command registry should generate or feed:

- CLI help
- JSON schemas
- skill reference
- dashboard action forms
- API contracts
- thin MCP schema

Do not hand-maintain separate descriptions for the same command.

D-154 makes this concrete: the built-in Phase 1 registry is plain JSON using a bounded JSON Schema 2020-12 profile. Surface-specific prose may be layered on top, but command identity/version, schemas, declared errors, effect/permission/idempotency classes, and surface visibility stay registry-owned.

## Remote clients

A cloud-only client cannot be assumed to run local shell commands.

As of the current platform review, ChatGPT connects to remote MCP servers rather than directly to a local MCP listener, and plugin/app availability depends on the user's plan, region, and account controls. The loopback discovery gateway is therefore a local transport prototype, not proof that ordinary or free ChatGPT can use RELAY. A supported remote connection or published plugin, its eligibility, and end-to-end account testing remain Phase 9 work. Recheck [OpenAI's MCP app guidance](https://help.openai.com/en/articles/12584461-developer-mode-and-mcp-apps-in-chatgpt) and [plugin availability guidance](https://help.openai.com/en/articles/20001256-plugins-in-chatgpt-and-codex) before setting a public support promise.

Path:

~~~
cloud AI
  -> thin connector/gateway
  -> authenticated structured request
  -> local relayd
  -> same Command Bus
  -> compact result
~~~

The remote path should preserve the same command semantics as local CLI.

## MCP posture

If MCP is used, prefer a small or dynamically discovered surface. Avoid preloading large catalogs when the client can retrieve the relevant command/tool contract on demand.

Candidate pattern:

- capability/help discovery
- execute structured RELAY command
- retrieve result/detail

Exact count and schema must be benchmarked. Do not expose one MCP tool for every underlying editor operation unless evidence shows a clear benefit.

Required comparison before setting a client default:

- task success
- tool-selection accuracy
- total schema/context tokens
- calls/steps
- latency
- recovery after errors
- maintenance/versioning complexity
- compatibility with small/free model classes

## Versioning

Skills and generated command documentation must declare compatibility with RELAY versions.

A future command such as relay skills verify may report stale or incompatible skill packages.

## Cost requirement

Every local-AI integration should be evaluated against the same task using:

- CLI + skill
- thin MCP
- larger MCP tool catalog

Measure context overhead, calls, tokens, latency, success, and error recovery before choosing the default.


## Operability commands

The CLI should provide a small, memorable health surface.

Conceptual commands:

~~~
relay status
relay doctor
~~~

Common diagnostic output should be concise by default and include structured detail on request.

Normal supported workflows should not require users/agents to memorize internal daemon, database, IPC, adapter-broker, or policy implementation details.


## Krita build identity

`relay krita-export` returns a project-scoped build `result_id` after a successful
create-only export. It requires `project_write` for publication and
`state_write`/`relay_state_write` for the build record before the native command
starts. The response and stored build payload share
`source_identity_sha256` and `export_identity_sha256`, so the original source and
export request can be matched to the record without returning filenames. Each
record can be retrieved with `relay result-get <result-id>`. Each identity is
SHA-256 over the UTF-8 bytes of
`relay-project-relative-path-v1` followed by a zero byte and the validated
project-relative path exactly as supplied to the command. File-content hashes
are separate fields. If the PNG is published but its Result Store write fails,
the command returns `ASSET_BUILD_RECORD_FAILED`; the PNG remains in place and
the transaction is marked failed. A process crash between file publication and
record storage has no automatic reconciliation yet.

## CLI contract versioning

Human-readable CLI text is not a stable parsing surface.

Machine clients should use structured output that declares its contract/schema version.

Stable CLI compatibility includes:

- command identity
- structured request/response shape
- exit-code categories
- field meaning
- side-effect semantics
- deprecation metadata

Human prose, spacing, progress rendering, and explanatory wording may change without being considered machine-contract compatibility.

## Skill compatibility

Generated skills/wrappers are clients of the command contract.

Skills should carry:

- RELAY contract/version range
- generation version
- required capabilities
- deprecated-command references where any

RELAY should detect stale/incompatible skills and offer regeneration/migration rather than preserving every obsolete command in normal model context.
