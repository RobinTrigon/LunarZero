---
title: 'LunarZero: a single-binary coding agent that runs on pooled free-tier language models with quota-aware failover'
tags:
  - Rust
  - large language models
  - software engineering agents
  - developer tools
  - model routing
authors:
  - name: Aifee Aadil
    orcid: 0000-0000-0000-0000
    corresponding: true
    affiliation: 1
affiliations:
  - name: Independent Researcher, Bangladesh
    index: 1
date: 18 September 2026
bibliography: paper.bib
---

# Summary

LunarZero is a terminal coding agent: a program that runs a large language
model in a loop with tools (read, edit, execute, search) inside a software
project until a task is done. Existing agents assume one configured model and
paid API access. LunarZero instead treats the *free tiers* of model providers
as a single resource. It ships a catalog of 278 free models across 13
providers, each annotated with a computed quality score, a speed score and the
provider's published rate limits, and a router that keeps a local ledger of
per-model request and token windows, classifies each request by task, and fails
over to the next available model---before any output has streamed---when a
provider throttles, rejects or exhausts a model. Cooldown lengths follow the
failure class: an exact `retry-after`, a daily cap until UTC midnight, a
retired or paid-only model for a day, an invalid key for the whole provider.

Around the router, LunarZero adds mechanisms that keep the loop productive on
weaker models: resumption from the last completed step, continuation of plans
the model abandoned, and a self-healing loop driven first by in-memory
language-server diagnostics [@lsp] and then by failed build or test commands.
A token-economy layer (compact tool schemas, pruning of stale output, stubbing
of old file writes, a tree-sitter [@treesitter] symbol index that injects only
the definitions a prompt names) keeps the fixed overhead of a step near 2.6k
tokens. A safety layer provides ordered permission rules with live modes,
hunk-by-hunk review of edits, a mandatory human confirmation when the agent
attempts to install third-party code the user did not ask for, and OS-level
sandboxing of installer builds. External tools integrate through the Model
Context Protocol [@mcp]. LunarZero is written in Rust, compiles to one 12--13
MB binary for macOS, Linux and Windows, carries a test suite and continuous
integration on all three platforms, and is released under the MIT license
[@lunarzero].

# Statement of need

Agentic coding tools---SWE-agent [@yang2024sweagent], OpenHands
[@wang2024openhands], Claude Code [@claudecode], Aider [@aider]---re-send a
growing conversation to the model on every step, so a multi-hour task costs
dollars of paid tokens. Students, independent researchers and developers
without an API budget have had one alternative: a single free-tier model whose
per-minute or per-day cap is exhausted partway through the first substantial
task. The free tiers are individually small but collectively large, with
different caps, model line-ups and failure behaviour, and no existing agent
treats them as one pool.

LunarZero makes agentic software work possible at zero cost, and makes the
routing problem this creates studyable. Every routing component---task
classifier, sticky sessions, failure-class cooldowns, policy preferences,
rescue---is a configuration flag, so ablations are cheap. The repository ships
a labelled 200-prompt routing set with a scorer (`lz pool eval`), a
deterministic mock provider and a benchmark script (`scripts/bench.py`) that
measure failover latency and per-step token overhead, and inspection commands
(`lz pool why`, `lz pool report`) that expose the ledger's reasoning and the
list-price equivalent of tokens consumed. Standard agent benchmarks such as
SWE-bench [@jimenez2023swebench] can be driven through the non-interactive
`lz run` mode.

# State of the field

Terminal agents such as Claude Code [@claudecode] and Aider [@aider], and
research agents such as SWE-agent [@yang2024sweagent] and OpenHands
[@wang2024openhands], define the agent--computer-interface pattern LunarZero
follows; all are built around a single paid model. Cost-aware routing has been
studied as a quality problem: FrugalGPT [@chen2023frugalgpt] cascades cheap to
expensive models to reach a quality target, and RouteLLM [@ong2024routellm]
learns a router from preference data. Free tiers change the question from
"which model is best" to "which model is *available* now, and for how long",
because hard per-window quotas and provider-specific failure semantics dominate
outcomes; LunarZero's ledger, cooldown classes and failover-before-output are an
implementation of that problem. LunarZero was built as a new system rather
than as a contribution to an existing agent because the routing layer touches
every step of the loop (each request is rebuilt per model, and a step can be
retried on another model mid-stream), the token-economy measures change what
the loop sends, and the safety layer changes how tool calls are gated; these
are core-loop decisions rather than plugins. Model metadata is taken from the
open models.dev catalog [@modelsdev].

# Software design

LunarZero is a Rust workspace of five crates. A schema crate holds the shared
types and an `EngineApi` trait; the core engine implements configuration, an
SQLite session store, providers and the router, tools, the permission engine,
the agent loop, MCP and LSP clients, the symbol index and the sandbox; a
terminal UI (ratatui) and a local web portal (axum) both talk to the engine
only through the trait, so an in-process engine can become a remote one; and a
command-line crate produces the `lz` binary. Provider wire formats are
implemented behind one `Protocol` trait (OpenAI-compatible chat completions and
the Anthropic Messages API), and every provider request is rebuilt per step
from stored history, which is what allows a step to be retried on a different
model. Permission decisions are an ordered ruleset (allow/ask/deny, wildcard
patterns, last match wins) layered as agent rules, live mode, the user's own
configuration, and session rules, so an explicit user allow-list or deny wins
in every mode. Actions the agent picked up from content rather than from the
user (installing third-party code) bypass the ruleset entirely and require a
human answer.

# Research impact statement

LunarZero was released in September 2026 and is early in its public life. To
date its research use is the author's own: one day of development on free tiers
produced 468 requests and 14.7 million tokens across 43 models at no cost, and
an earlier version of the agent built a 3,081-line web application on free
tiers alone; token accounting from that build motivated the stubbing of old
file writes, which reduced a late step from 46k to 28k tokens. Development
against live providers surfaced behaviours a mock cannot---tool-call signatures
that must be replayed, model-scoped authentication errors, catalog
drift---which are documented in the repository's changelog and handled by the
router. A companion preprint describing the system and its preliminary
measurements accompanies this submission. Adoption by others is not yet
established; this section will be updated as evidence accrues.

# AI usage disclosure

LunarZero was developed with the assistance of a generative AI coding
assistant (Claude, Anthropic; Opus-class models, 2026), used interactively
inside the author's editor for code generation and refactoring across the
codebase, test scaffolding, documentation, the benchmark harness, and drafting
of this manuscript and the accompanying preprint. The author framed the
problem, directed the design (the free-tier pool and router, the
finishing-work mechanisms, the token-economy and safety layers), decided
between alternatives at each step, ran and checked all measurements, and
reviewed, edited and validated all AI-assisted code and text. All design
decisions, claims and measurements in this paper are the author's
responsibility.

# Acknowledgements

Model metadata is derived from the models.dev catalog [@modelsdev]. No
financial support was received for this work.

# References
