# Legal, Licensing, and Distribution

## Purpose

Define the legal/licensing boundaries RELAY must respect before public distribution, with particular attention to UEFN/Fortnite, Unreal Engine, Blender, Krita, third-party adapters/assets, AI-generated content, privacy representations, and open-source dependencies.

This document is an engineering/release-control specification, not legal advice. Public releases that depend on ambiguous platform terms, trademark use, copyleft boundaries, or jurisdiction-specific privacy obligations require qualified legal review.

## Core invariants

- RELAY does not assume that technical access implies legal permission.
- Official documented integration surfaces are preferred over reverse-engineered/private interfaces.
- RELAY does not redistribute third-party applications, engine binaries, or proprietary assets unless the applicable license clearly permits it.
- Platform terms, creator rules, and licenses are versioned external dependencies.
- User-created project content, third-party assets, Epic assets, RELAY code, adapter code, and generated outputs can have different ownership/license status.
- A signature, package source, or "free software" label does not resolve license compatibility by itself.
- RELAY does not promise copyright ownership or copyrightability of AI-generated output.
- RELAY does not represent a project or asset as legally compliant merely because automated checks passed.
- Third-party notices, source obligations, attribution, license texts, and provenance are release artifacts where required.
- Public privacy/security promises must match actual product behavior.

## Epic / UEFN integration boundary

Epic currently ships and documents Unreal MCP in UEFN as an official way for MCP-compatible agentic coding tools to drive the editor. RELAY should prefer this and other documented developer surfaces.

At the same time, Epic's general Terms of Service prohibit bot software/services used to automate Licensed Products, while the current official UEFN documentation expressly supports AI agents driving UEFN through Unreal MCP.

Engineering rule:

- treat documented UEFN developer automation as the intended supported path
- do not generalize that permission to Fortnite gameplay/player automation
- do not use RELAY to automate gameplay, evade integrity systems, or control the Fortnite client outside documented development/testing workflows
- avoid reverse-engineering/private protocol dependencies where an official interface exists
- record the Epic terms/documentation versions that the adapter was validated against
- obtain legal/official clarification before public release if RELAY depends on automation beyond clearly documented developer tooling

## UEFN runtime networking

Current UEFN Supplemental Terms prohibit Developer-Made Content/code from attempting to establish connections to non-Epic servers after upload/download by end users.

Therefore:

- do not ship an in-island RELAY "phone home" agent to a RELAY server
- runtime instrumentation should use supported Verse/UEFN logging, debug, editor/play-session, or Epic-hosted mechanisms
- any local development-only bridge must be clearly excluded from published island content
- publishing validation should check that RELAY development instrumentation does not introduce prohibited external networking

## Epic content ownership and publishing

Epic's current UEFN terms state that a developer's Developer-Made Content remains theirs apart from Epic's Licensed Products/assets and third-party material, but the developer must have sufficient rights to grant Epic the required license.

RELAY should therefore track asset/content provenance rather than assuming "inside my project = mine."

For project content, record where practical:

- creator/source
- license or rights basis
- Epic-owned asset status
- third-party asset status
- AI-generated/AI-assisted status where relevant
- permitted project/use scope
- attribution requirements
- redistribution/export limitations

A RELAY asset validation pass can flag missing rights metadata, but cannot determine legal ownership conclusively.

## Epic IP, branding, and public RELAY marketing

Epic's Fan Content Policy currently defines covered Epic-related apps/sites as personal, non-commercial, and freely accessible, and restricts use of Epic trademarks/marks as product identifiers.

RELAY must not rely on that policy as the legal basis for commercial product branding.

Public-release rules:

- product name/logo/identity remain independent of Epic/Fortnite/Unreal branding
- use Epic/Fortnite/UEFN names descriptively only where needed to identify compatibility
- do not use Epic logos or marks as RELAY branding without permission
- do not imply endorsement, certification, or official Epic affiliation
- review current Epic branding/fan-content/developer rules before public marketing

## Redistribution strategy

Default public installer behavior:

- detect user-installed UEFN/Fortnite/Blender/Krita
- do not bundle those applications by default
- install only RELAY-owned components plus dependencies whose redistribution rights are known
- provide links/instructions to official third-party installers where appropriate

This minimizes license, update, trademark, size, and security obligations.

## Unreal Engine / UEFN companion code

Current Unreal Engine licensing terms identify GPL and certain share-alike licenses as non-compatible when they would impose those terms on Epic Licensed Technology. Engine Tools also have specific distribution restrictions.

Therefore:

- do not casually license an Unreal/UEFN in-process plugin/toolset under GPL
- keep RELAY Core licensing separate from engine-integrated companion components
- before distributing any Unreal/UEFN companion code, classify whether it includes/links Engine Tools/Licensed Technology and which Epic distribution rules apply
- legal/license review is required before choosing the license and distribution channel for an Unreal/UEFN in-process companion
- favor documented external protocol boundaries when they reduce license coupling

## Blender

Blender's official license page states:

- Blender source is generally GPL-2.0-or-later, with Blender binary distributions compatible under GPL-3.0-or-later
- published Blender Python add-ons using the Blender Python API must use a GPL-compatible license
- artwork/data created with Blender remains the creator's property

RELAY implication:

- a distributed Blender add-on/companion must satisfy GPL-compatible obligations
- RELAY Core may remain separately licensed when communicating through a sufficiently separate process/protocol boundary, subject to legal review of the actual implementation
- avoid bundling a modified Blender build unless RELAY is prepared to meet redistribution/source/trademark obligations

## Krita

Krita's official license page states:

- Krita as a whole is GPLv3
- distributed Krita plugins using its extension API must be GPL
- artwork created with Krita remains the creator's property

RELAY implication:

- a distributed Krita plugin/bridge should be treated as a GPL component
- keep its source/license notices and distribution obligations separate from RELAY Core
- prefer a narrow protocol boundary between the GPL companion and differently licensed RELAY components

## Copyleft boundary rule

"Separate executable" is not a magic legal exemption.

Before declaring RELAY Core unaffected by a companion's copyleft:

- document process/linking/API boundaries
- inventory code copied/shared across the boundary
- review IPC/protocol coupling
- avoid copying GPL implementation code into differently licensed components
- obtain legal review for ambiguous derivative-work questions

The architecture should make compliance easier, but it cannot decide unsettled license-law questions by itself.

## RELAY's own license

The public RELAY Core license remains an open decision.

Selection criteria include:

- public/commercial goals
- contribution policy
- compatibility with UEFN/Unreal companion requirements
- compatibility with Blender/Krita companion separation
- third-party dependency licenses
- whether a hosted/cloud component exists
- ability to ship official proprietary services or paid features if desired
- contributor license/copyright-management model

Do not select a license merely because it is popular.

## Third-party dependency and license inventory

Every distributed RELAY artifact should have a reproducible component/license inventory.

Track where practical:

- component/package
- exact version
- source
- license(s)
- copyright notice
- required attribution/license text
- source-offer/source-distribution requirement
- copyleft/linking considerations
- patent clauses
- redistribution restrictions
- generated/bundled asset licenses

Security SBOM and legal license inventory may share data, but they answer different questions.

Phase 1 Spike 11 adds the Rust `flatbuffers` 25.12.19 runtime (Apache-2.0) to build the measured Windows sandbox specification. The Windows `processmodel.dll` implementation is supplied by the operating system rather than redistributed by RELAY. Any stable sandbox backend selected by Spike 12 must be added to the same dependency/license/notice inventory before distribution.

Unknown/incompatible licenses block release until resolved.

## Adapter license manifest

Third-party adapter manifests should include:

- adapter license
- publisher
- companion-component licenses
- dependency/license inventory
- redistribution status
- required notices/source links
- compatibility with RELAY SDK/API terms

Curated status does not imply license compatibility.

## Assets and content provenance

The asset registry should support license/provenance fields for:

- user-authored assets
- Epic-provided UEFN assets
- marketplace/Fab assets
- Creative Commons assets
- purchased commercial assets
- stock media
- fonts
- music/audio
- generated assets
- modified/derived assets

RELAY can help detect missing metadata and conflicting known license terms.

It cannot guarantee that a user actually owns the rights they claim.

## AI-generated content and copyright

The U.S. Copyright Office's 2025 Part 2 report states that generative-AI outputs are copyrightable only where sufficient human-authored expression exists; prompts alone generally do not supply that authorship. Human-authored material, selection/arrangement, or creative modification can still be protected.

RELAY must not say:

- "you automatically own copyright in this AI asset"
- "this prompt makes the result copyrighted"
- "AI-generated means public domain"
- "AI-generated means infringement-free"

Instead record provenance and, where useful, human modification/selection history without making ownership conclusions.

## AI provider output terms

Different AI providers may offer different contractual rights, warranties, indemnities, restrictions, or data-use terms for outputs.

Provider-policy profiles may therefore include output-license/rights metadata separately from privacy/training metadata.

RELAY should not merge "provider lets you use the output" with "the output is copyrightable" or "the output does not infringe third-party rights."

## Terms and policy monitoring

External terms are versioned dependencies.

Track for important integrations:

- source URL/document
- effective/last-updated date where available
- RELAY version last reviewed against it
- material assumptions derived from it
- compatibility/review status

Examples:

- Epic Games Terms of Service
- UEFN Supplemental Terms
- Fortnite Developer Rules
- Epic branding/Fan Content Policy
- Unreal Engine EULA
- Blender/Krita licenses
- AI provider/API terms

A terms change should create a review task, not silently rewrite product behavior.

## Compliance-check semantics

Automated policy/license checks are advisory engineering controls.

Use language such as:

- missing license metadata
- known incompatible license combination
- rule/version changed since last review
- requires human/legal review
- publish blocked by configured policy

Avoid:

- legally compliant
- copyright cleared
- guaranteed non-infringing
- approved by Epic

unless an authoritative process actually supports that claim.

## Privacy representations

FTC guidance repeatedly emphasizes that software companies must honor their privacy/security promises.

RELAY public messaging and UI must therefore align with the tested behavior described in DATA_BOUNDARY_AND_PRIVACY.md.

Examples:

- "local-only" must match verified egress behavior
- telemetry descriptions must match actual collection
- deletion/export promises must match actual retention behavior
- diagnostic upload must not contradict opt-outs
- privacy-enhancing claims must describe limitations accurately

## Public distribution release gate

Before a public beta/release:

- choose RELAY Core license
- complete dependency/license inventory
- generate required notices/attributions
- review licenses for each first-party companion
- review current Epic/UEFN automation and branding terms
- verify installer does not redistribute restricted third-party binaries/assets
- review AI-output/copyright claims in UI/docs
- publish privacy notice matching actual telemetry/cloud behavior
- define user terms/disclaimers if needed
- establish a process for third-party takedown/license reports
- obtain qualified legal review for unresolved high-impact questions

## Open questions

- RELAY Core license
- SDK/adapter license
- license for UEFN companion/toolset
- exact Blender/Krita companion architecture
- whether any Epic permission/clarification is needed for commercial RELAY use around UEFN MCP
- public trademark/compatibility language
- dependency scanning/tooling
- contributor copyright/CLA/DCO policy
- privacy-policy jurisdictions and team/enterprise terms
- marketplace/Fab asset-license metadata access
- AI-provider output-rights metadata format
