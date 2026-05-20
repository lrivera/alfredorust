# OpenSpec Workflow

This repository uses OpenSpec-style spec-driven development for non-trivial changes. Specs live with the code and describe intended behavior, not implementation details.

OpenSpec is local to the repository. It does not require Harness.io, API keys, MCP, or a hosted service.

## Recommended Commands

Install the CLI when you want native `/opsx` support:

```bash
npm install -g @fission-ai/openspec@latest
openspec init
openspec update
```

Use the workflow:

```text
/opsx:propose <change-name-or-description>
/opsx:apply <change-name>
/opsx:verify <change-name>
/opsx:archive <change-name>
```

## Repository Layout

```text
openspec/
  specs/              # Current expected behavior by domain
  changes/            # Proposed changes, one folder per change
  config.yaml         # Project guidance for OpenSpec and agents
```

## When A Change Needs OpenSpec

Use OpenSpec for:

- Features touching tenant data, permissions, SAT/CFDI, finance, projects, resources, time tracking, or PDF behavior.
- Security-sensitive changes.
- Changes that create, update, or delete financial records.
- Changes that need product clarification before implementation.

Small typo fixes, mechanical refactors, and obvious one-line bugs can skip OpenSpec.

## Required Review Questions

Before implementation, every meaningful change should answer:

- Which company owns the data?
- Which role or permission can see it?
- Which role or permission can mutate it?
- Which MongoDB collections are read or written?
- What financial side effects are expected?
- Which harness fixture or integration test proves the behavior?
- What should fail loudly instead of falling back silently?
