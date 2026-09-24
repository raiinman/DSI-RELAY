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
- relayd — persistent local daemon hosting RELAY Core

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

## Single command metadata source

The command registry should generate or feed:

- CLI help
- JSON schemas
- skill reference
- dashboard action forms
- API contracts
- thin MCP schema

Do not hand-maintain separate descriptions for the same command.

## Remote clients

A cloud-only client cannot be assumed to run local shell commands.

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
