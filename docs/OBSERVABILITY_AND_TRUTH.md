# Observability and Truth Quality

## Purpose

Define how RELAY decides what it knows, how strongly it knows it, and when logs, metrics, traces, screenshots, probes, validators, or AI interpretations are incomplete, stale, sampled, contradictory, or merely inferred.

The goal is not maximum telemetry. The goal is the minimum trustworthy evidence needed for the next correct decision.

## Core invariants

- Observability is evidence about system state, not system state itself.
- Absence of a signal is not proof that an event did not happen unless the signal path is known complete for that event.
- Sampled telemetry must carry sampling/completeness metadata.
- Timestamps do not automatically prove causal order across components.
- Instrumentation can change performance and therefore can change what is being measured.
- A finding keeps its source, version, freshness, and derivation chain.
- Native/authoritative state is preferred over derived guesses when available.
- Correlated signals are not automatically independent corroboration.
- Confidence and uncertainty are user-visible when they materially affect a decision.
- The observability pipeline has its own health state.
- RELAY does not call a system healthy merely because its monitors are quiet.

## Evidence classes

RELAY should distinguish evidence classes such as:

### Authoritative state

Direct state from the source that owns the resource, such as a current editor entity value or project file content.

### Direct observation

A runtime event, log, metric, trace/span, profiler reading, screenshot, or probe result.

### Derived measurement

A value computed from one or more observations, such as a distance, rate, diff, aggregate, or anomaly score.

### Corroborated finding

A finding supported by multiple relevant observations or an authoritative source plus independent validation.

### Inference

A model/rule/heuristic conclusion that explains evidence but is not itself direct state.

### Unknown / insufficient evidence

RELAY cannot support a reliable conclusion.

These categories can be refined in implementation, but UI/API language should avoid presenting an inference as direct fact.

## Evidence envelope

Important evidence should carry, where applicable:

- evidence ID
- project/resource
- source/integration
- source version
- modality/signal type
- timestamp
- sequence/trace/correlation identifiers
- project/entity revision where available
- freshness
- sampling policy/rate
- completeness status
- trust/provenance
- measurement units
- uncertainty/confidence
- derivation inputs
- raw evidence reference
- instrumentation configuration/version

## Freshness and staleness

Evidence can become stale even if it was once correct.

Examples:

- screenshot captured before a later edit
- device list read before UEFN changed
- cached profiler result from an older play session
- project audit generated against an earlier revision

RELAY should bind important evidence/results to project/session/revision metadata and mark it stale when relevant state advances.

## Missing telemetry

Missing data has multiple meanings:

- event did not happen
- source never emitted it
- source emitted it but collection failed
- event was sampled out
- buffer overflow/drop occurred
- parser rejected it
- transport disconnected
- retention expired
- clock/window selection excluded it

RELAY should only interpret "not observed" as "did not happen" when the instrumentation contract supports that claim.

## Sampling

Sampling is a cost-control mechanism, not invisible loss.

Requirements:

- record sampling strategy/rate where practical
- distinguish complete versus sampled evidence
- do not derive exact counts from sampled events without a valid estimator
- preserve adjusted/estimated count semantics separately from measured counts
- use higher-retention/tail strategies for important errors/rare events when justified
- allow audit/test workflows to temporarily request stronger capture if cost allows

A compact AI result should state when its supporting evidence was sampled.

## Time and causality

Wall-clock timestamps across processes/machines may be skewed.

For causal reasoning prefer, where available:

- explicit parent/child relationships
- trace/span relationships
- monotonic process-local ordering
- sequence numbers
- transaction/job IDs
- engine/session event order
- logical/causal links

Clock quality/skew should be treated as evidence metadata where distributed ordering matters.

RELAY should avoid statements such as "A caused B because A's timestamp is earlier" unless causal evidence supports it.

## Correlation and root cause

RELAY can rank likely causes, but correlation is not proof.

A root-cause conclusion should distinguish:

- observed symptom
- correlated change
- tested causal hypothesis
- verified cause

Where practical, RELAY should validate a proposed cause with a controlled test, reproduction, state change, or authoritative dependency relationship before labeling it verified.

## Instrumentation overhead

Instrumentation consumes CPU, memory, storage, network, and time and may perturb the behavior being measured.

Each instrumentation mode should have an overhead profile or benchmark where practical.

RELAY should support:

- low-overhead normal mode
- targeted/high-detail diagnostic mode
- instrumentation budget
- temporary probe activation
- explicit heavy-capture warnings
- comparisons against a minimally instrumented baseline when diagnosing performance

Do not diagnose an instrumentation-induced slowdown as a project regression.

## Validator and detector quality

A validator/anomaly detector can be wrong.

For automated findings track where useful:

- source/rule/model version
- confidence/calibration
- historical false-positive/noise rate
- evidence used
- affected population/base rate
- verification status
- human disposition/feedback

Operational alert volume matters in addition to accuracy/precision/recall metrics.

Repeated low-value findings should be deduplicated, tuned, demoted, or disabled rather than training the user to ignore RELAY.

## Observability pipeline health

RELAY must monitor its monitoring path.

Potential signals:

- dropped logs/events
- parser failures
- sampling status
- queue/backlog
- collector/adapter disconnect
- stale last-seen time
- buffer overflow
- disk/quota pressure
- clock/time-sync health where relevant

If observability is degraded, downstream findings should inherit that uncertainty rather than remain green.

## Visual evidence

Screenshots/captures are partial observations.

Store:

- capture target/window/viewpoint
- project/session/revision
- camera/view identity where applicable
- timestamp
- resolution/crop
- capture method/version
- comparison baseline

A screenshot from the wrong session or old project revision is stale evidence even if the pixels are valid.

## Native tool findings

Engine/tool validators, profilers, logs, and session inspectors are preferred when they are authoritative for their domain.

Still record:

- source
- tool/version
- check/rule version if available
- scope/coverage
- timestamp/session

"Native" does not mean infallible or timeless. RELAY should not silently reinterpret native findings after tool versions change.

## Metrics and Goodhart risk

Usage and quality metrics are decision aids, not single targets.

Examples of dangerous optimization:

- reducing AI calls while increasing wrong decisions
- reducing token count while losing necessary evidence
- reducing alerts by hiding failures
- increasing cache hit rate by serving stale data

Product decisions should use metric portfolios and explicit quality/safety guardrails rather than maximizing one number.

## Truth status in the dashboard

Plain-language states may include:

- Verified
- Supported
- Inferred
- Sampled
- Stale
- Conflicting
- Incomplete
- Unknown

The exact vocabulary needs UX testing, but users should be able to tell whether RELAY measured something directly or inferred it.

## AI context behavior

The Context Compiler should prioritize:

1. current authoritative state
2. verified/corroborated findings
3. relevant direct observations
4. derived measurements with provenance
5. inferred hypotheses

Conflicting or degraded evidence should be included when it affects the decision rather than being summarized away.

The model receives evidence quality metadata when material.

## Testing requirements

Phase 1+ should test:

- dropped event/log
- sampled-out rare failure
- collector disconnected while project remains active
- stale screenshot/result after project edit
- conflicting native validator and RELAY rule
- clock skew / out-of-order event arrival
- instrumentation overhead changing performance
- noisy detector with high false-alert volume
- two correlated detectors making the same mistake
- silent observability-pipeline failure
- stale cache presented beside fresh authoritative state
- metrics optimized at expense of correctness
- AI root-cause hypothesis disproved by controlled test

## Open questions

- final evidence-confidence vocabulary
- confidence/calibration model
- trace/time model for local versus remote components
- default sampling policy
- observability overhead budgets
- how much raw evidence to retain per signal class
- detector feedback/calibration workflow
- causal/root-cause verification mechanisms
- first-party engine coverage metadata
- metrics portfolio for product quality
