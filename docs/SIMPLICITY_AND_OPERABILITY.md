# Simplicity and Operability

## Purpose

Keep RELAY useful, understandable, installable, supportable, and cheap to operate as its security, privacy, recovery, observability, adapter, and collaboration capabilities grow.

The product promise is not "maximum control-plane sophistication." The product promise is to make AI-assisted development cheaper and easier without hiding dangerous uncertainty.

## Core invariants

- The safest/default path should also be the easiest path.
- A new feature has permanent maintenance, testing, documentation, support, and cognitive costs.
- Public single-user workflows must not require enterprise concepts that do not apply to them.
- Advanced control exists behind progressive depth; it is not dumped into first-run setup.
- Optional integrations stay lazy and inactive until needed.
- Configuration options are product states that require validation and support, not free flexibility.
- Automatic discovery must remain inspectable and overridable.
- RELAY should deliver useful local value before requiring cloud AI, third-party adapters, team setup, or advanced policy.
- Error recovery should point to one clear next action whenever possible.
- Removing or merging features is a valid product improvement.
- Human time and attention are part of RELAY's cost model.

## Golden path

A new personal user should eventually be able to:

~~~
Install RELAY
 -> open dashboard
 -> choose/detect project
 -> run local scan/audit
 -> see useful result
~~~

before configuring:

- remote AI
- custom adapters
- team/workspace policy
- advanced privacy classes
- custom sampling
- custom recovery policy
- enterprise identity
- manual ports
- raw configuration files

Additional capabilities are introduced when the task requires them.

## Progressive depth

Use three conceptual layers:

### Simple

Answers:

- Is RELAY working?
- What project am I in?
- What needs attention?
- What can RELAY do next?
- What changed?

### Detailed

Adds:

- evidence
- project state
- tests
- transactions
- integrations
- privacy/egress
- usage
- recovery details

### Advanced/raw

Adds:

- exact schemas
- paths
- protocol state
- raw logs
- rule IDs
- adapter manifests
- policy internals
- trace/evidence metadata
- developer diagnostics

Progressive disclosure does not justify an incoherent underlying model. Concepts and names must remain consistent across layers.

## Secure and safe defaults

Public users should not need to harden RELAY manually before using it safely.

Default policy should:

- deny unnecessary remote/project-data egress
- keep raw credentials out of model context
- require meaningful approval for destructive/external actions
- isolate projects
- disable unneeded third-party adapter capabilities
- use bounded retries
- apply retention/quota defaults
- keep detailed/high-overhead instrumentation off until needed

Advanced users may relax or customize defaults where policy permits.

## Configuration budget

Every configuration option expands the state RELAY must understand, test, document, migrate, and support.

Before adding a configurable option ask:

- Is there a safe/reasonable default?
- Can RELAY detect the value automatically?
- Is the option genuinely needed by distinct users/workloads?
- Can it be project-specific instead of global?
- Can it be represented as a higher-level policy/profile?
- What interactions does it create with existing options?
- How will it be tested?
- How will it migrate?
- What happens if it is removed later?

Avoid exposing internal implementation knobs merely because they exist.

## Supported configuration profiles

Where configuration interactions become expensive, RELAY may define supported/recommended profiles rather than claim all combinations are equally tested.

Examples:

- Personal / local-first
- Personal / remote AI enabled
- Team
- Developer/extension author

Names and final profiles require testing.

Profiles are defaults/constraints, not permanent product editions. Users can inspect what a profile means.

## Choice design

"Fewer choices" is not a universal rule.

Choice burden depends on:

- complexity of the options
- difficulty of the task
- user preference uncertainty
- consequences/risk
- expertise

RELAY should therefore prefer:

- strong defaults
- contextual choices
- recommendations with explanation
- search/filter for large sets
- advanced options only when relevant

rather than either hiding all control or presenting every possible option at once.

## Feature admission test

A feature should enter core RELAY only when its value exceeds its total complexity cost.

Evaluate:

- user problem solved
- frequency/importance
- whether an adapter/project module can own it instead
- implementation complexity
- test matrix expansion
- security/privacy surface
- migration/update cost
- dashboard/CLI/docs complexity
- background/runtime overhead
- support burden
- interaction with existing features
- measurable impact on RELAY's cost/ease mission

"Could be useful" is not enough.

## Feature retirement

RELAY should remove, merge, or demote features when:

- usage is negligible
- another feature supersedes them
- maintenance/support cost is disproportionate
- they create confusing overlapping concepts
- they block architecture simplification

Deprecation/migration paths still respect public users' data and workflows.

## Adapter support tiers

A growing adapter catalog can multiply support complexity.

Potential support status:

- First-party supported
- Curated/validated
- Community/unverified
- Developer/local
- Incompatible/quarantined

Compatibility is versioned.

Inactive adapters should not add routine CPU/memory, dashboard noise, AI context, or health-check burden.

## Generalization rule

Do not build a universal engine abstraction based only on imagination.

Initial adapter contracts should solve the first real integration cleanly while keeping engine-specific behavior behind boundaries.

Before declaring an abstraction stable, validate it against at least one materially different second integration/toolchain.

If a generic interface requires constant escape hatches, the abstraction is probably premature.

## Automation visibility

Automatic discovery and setup should reduce work without becoming unexplained magic.

When RELAY detects:

- project
- tool installation
- port
- integration
- project type
- privacy/provider profile

the user should be able to inspect and correct the detected result.

A wrong automatic decision should be cheap to recover from.

## Self-diagnostics

Public RELAY needs a compact health entry point.

Conceptual examples:

~~~
relay doctor
relay status
~~~

and a dashboard equivalent.

It should answer:

- what is wrong
- what is still working
- what RELAY already tried
- what the user should do next
- whether the problem is local, project-specific, adapter-specific, remote, or unknown

Do not require users to understand the internal architecture to repair common problems.

## Operability and toil

RELAY can become a burden even when it technically works.

Measure repetitive work users/maintainers spend on:

- reconnecting tools
- configuration edits
- update repair
- permission cleanup
- log triage
- adapter compatibility
- reindexing
- repeated approvals
- support diagnosis

Automate recurring safe work and fix root causes rather than normalizing manual rituals.

## Complexity budget

Track product complexity as a portfolio of observable measures, not one magic score.

Candidate measures:

- steps to first useful audit
- time to first useful result
- required concepts in onboarding
- number of mandatory decisions
- default-path success rate
- configuration option count by scope
- supported profile/configuration count
- background processes/services
- first-party adapter dependencies
- compatibility matrix size
- install/update failure rate
- support incidents per active installation
- diagnostic time to root cause
- dashboard navigation depth
- CLI commands/flags needed for common tasks
- percentage of users requiring Advanced settings
- RELAY-management time versus project-work time

Use trends and user studies; do not optimize any one metric blindly.

## Documentation strategy

Avoid five manually maintained descriptions of one feature.

Where practical, generate or validate:

- CLI help
- command schemas
- dashboard action metadata
- adapter capability metadata
- AI skill references

from shared semantic sources.

Human-facing guides still need editorial writing, examples, and task-based structure.

## Public versus team complexity

Team/enterprise capabilities should not burden solo users.

A personal installation may use an implicit personal workspace and hide:

- custom roles
- team membership management
- SSO/SCIM
- organization policies
- audit administration
- contractor offboarding

until those capabilities actually exist and are relevant.

The data model may support future team use without forcing team terminology into the normal personal workflow.

## Complexity review

Each milestone should include a simplification pass:

1. list new concepts/options/services
2. identify duplicates/overlaps
3. remove unnecessary user decisions
4. verify defaults
5. confirm common tasks remain short
6. update generated/manual docs
7. measure onboarding/diagnostic regression
8. delete obsolete code/settings

Complexity is debt unless actively managed.

## Open questions

- exact personal/team product surface split
- supported profile set
- complexity-budget thresholds
- usability benchmark participants/tasks
- first-run local audit minimum
- how much advanced policy belongs in UI versus config
- feature deprecation/support policy
- extension-support tiers
- generated-documentation architecture
- product telemetry needed to measure operability without collecting project data


## Legal/licensing UX

Legal metadata should protect users without turning normal project work into license-law homework.

Default behavior:

- automatically inventory known component licenses
- surface only actionable conflicts/missing provenance in normal views
- keep full notices/license details in Detailed/Advanced views
- explain rights metadata in plain language
- avoid "legally compliant" badges
- use Review Required when the issue cannot be determined mechanically

The golden path should not require users to answer legal questions about RELAY's own bundled dependencies; those are the distributor's responsibility.


## Compatibility UX

Versioning should protect users without turning every screen into a compatibility matrix.

Normal views should say:

- Update available
- Adapter needs update
- Project data migration required
- This skill is outdated
- This historical result has limited rendering

Detailed/Advanced views can show exact versions/protocol ranges.

Deprecation warnings should be actionable and deduplicated. Legacy commands/settings stay hidden unless the user is migrating or troubleshooting.

Supporting old versions has a complexity cost; compatibility promises must remain bounded enough that the golden path stays simple.
