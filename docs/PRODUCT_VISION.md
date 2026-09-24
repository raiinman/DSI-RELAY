# Product Vision

## Problem

AI-assisted development often becomes expensive and fragile because the model is asked to do work that software should do:

- inspect thousands of objects one at a time
- repeatedly read unchanged files
- consume raw logs
- carry project history inside chat
- call many narrowly scoped tools
- guess whether editor actions succeeded
- rediscover the same project state every session
- depend on one vendor's coding environment

Game development makes this worse because an AI also needs visibility into the live editor, runtime state, visual output, assets, and gameplay behavior.

## RELAY's role

RELAY is the operator between the human, AI, and development tools.

It should:

- observe the project
- measure and validate state
- maintain durable history
- perform deterministic bulk work locally
- expose safe actions
- test outcomes
- retain evidence
- compile only relevant context for AI
- explain problems in plain language
- make advanced/raw information available on demand

The AI is used for reasoning and judgment, not as the database or execution engine for deterministic work.

## Users

RELAY should be useful to:

- creators who are not infrastructure experts
- developers using normal chat AI
- developers using coding agents
- teams mixing human and AI workflows
- advanced users scripting from CLI/CI
- future plugin/integration authors

A user's AI subscription level should change available reasoning quality, not whether the project is fundamentally usable.

## Product promises

### Provider independence

ChatGPT, Codex, Claude, local models, and future clients are callers. No one provider owns project state or core execution.

### Multi-project by default

RELAY manages projects as isolated workspaces. One project's rules, assets, history, or credentials must not leak into another.

### Local-first efficiency

Measure, parse, diff, index, validate, filter, deduplicate, and execute locally whenever possible.

### Recoverable evidence

Compact output must never mean discarded truth. Raw evidence and exact structured state remain retrievable.

### Simple-first interface

Primary dashboard language answers: What is happening? What needs attention? What can I do? Technical implementation details live behind advanced views.

### Reproducibility

Important work can be invoked headlessly and returns structured machine-readable results.

### Public readiness

Installation and onboarding should not require knowledge of the original developer's computer, projects, or accounts.

## Initial domain

UEFN/Fortnite is the launch integration, with Blender and Krita as companion asset tools.

The core must support additional engines and creative tools through adapters without redesigning project state, jobs, results, transactions, context management, or the dashboard shell.

## Experience target

A new user should eventually be able to:

~~~
Install RELAY
 -> open dashboard
 -> RELAY detects supported tools
 -> choose/add a project
 -> RELAY builds an initial index and baseline
 -> issues are explained plainly
 -> connect an AI client if desired
 -> work
~~~

Advanced setup remains available, but it is not the normal path.
