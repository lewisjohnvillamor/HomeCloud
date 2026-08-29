# Skills and AI-Agent Workflow

This project treats agent skills as **engineering process**, not decoration.

## 1. Sources

- Superpowers: https://github.com/obra/superpowers
- Taste Skill: https://github.com/Leonxlnx/taste-skill
- Spartan AI Toolkit: https://github.com/c0x12c/ai-toolkit
- PlanetScale database-skills: https://github.com/planetscale/database-skills
- Vercel Web Interface Guidelines: https://github.com/vercel-labs/web-interface-guidelines
- Anthropic code-simplifier: https://github.com/anthropics/claude-plugins-official/tree/main/plugins/code-simplifier

Pin or periodically review upstream versions; do not assume third-party skill behavior is immutable.

## 2. Required Order of Operations

### New feature
1. **Superpowers: brainstorming** — understand scope, constraints, alternatives, and write/validate a spec.
2. **Superpowers: writing-plans** — create a detailed implementation plan in `docs/superpowers/plans/`.
3. Create/verify isolated worktree when implementation begins.
4. Execute through **subagent-driven-development** when the agent supports subagents; otherwise **executing-plans**.
5. **TDD** for each behavior-bearing task.
6. Apply domain skills during implementation:
   - PlanetScale Postgres for DB changes;
   - Vercel guidelines for all product UI;
   - Taste principles for visual/media/landing composition where applicable;
   - Spartan React/Next.js/security/test packs as quality checks.
7. Request code review: first spec compliance, then code quality.
8. Run **code-simplifier** behavior on recently changed code after green tests.
9. Run **verification-before-completion**.
10. Finish branch with merge/PR/cleanup workflow.

### Bug
1. Superpowers systematic debugging.
2. Reproduce and identify root cause.
3. Regression test.
4. Minimum fix.
5. Relevant domain skill review.
6. Code simplification.
7. Verification before completion.

## 3. Superpowers

Current upstream emphasizes mandatory skill discovery, TDD, systematic debugging, evidence before completion, planning, code review, worktrees, and branch finishing.

Project rule: **process skills come before implementation skills**.

Plans live at:

```text
docs/superpowers/plans/YYYY-MM-DD-<feature>.md
```

Every task in a plan should be independently testable and include concrete files, test commands, implementation steps, verification, and commit boundary.

## 4. Taste Skill

Use selectively and correctly.

The current primary Taste Skill explicitly positions itself for landing pages, portfolios, and redesigns rather than dashboards/data tables. Therefore:
- use it strongly for public site, onboarding, Memories, Photos presentation, shared album pages, empty states, visual hierarchy, typography, motion, and brand quality;
- do **not** distort the file manager into a marketing page;
- for dense application interaction, `EXPERIENCE_SPEC.md` + Vercel guidelines take priority.

## 5. Spartan AI Toolkit

The toolkit currently provides structured workflows, configurable rules, quality gates, and packs across stacks. Recommended local installation pattern from upstream:

```bash
npx @c0x12c/ai-toolkit@latest --local
```

Select relevant React/Next.js, JavaScript security, testing, browser QA, and backend quality packs rather than installing unrelated rules blindly.

Project policy: if Spartan rules conflict with a repository-specific documented invariant, the repository invariant wins and the conflict should be documented.

## 6. PlanetScale Database Skills

Install/reference the Postgres skill:

```bash
npx skills add planetscale/database-skills
```

Use it for:
- schema design;
- indexes;
- query tuning;
- transaction design;
- MVCC/VACUUM concerns;
- connection pooling;
- migrations;
- query plan review;
- Postgres operational behavior.

Although the skill contains PlanetScale-specific material, this project is self-hosted-first. Apply portable Postgres practices and respect the deployment target rather than assuming managed PlanetScale hosting.

## 7. Vercel Web Interface Guidelines

Current upstream provides a review skill through Vercel agent skills:

```bash
npx skills add https://github.com/vercel-labs/agent-skills --skill web-design-guidelines
```

Binding areas:
- keyboard everywhere;
- visible, unobscured focus;
- WAI-ARIA interaction patterns;
- specific labels;
- actionable error messages;
- responsive behavior;
- form behavior;
- hydration/React correctness;
- animation/interface performance.

## 8. Anthropic Code Simplifier

Apply after functionality is correct and tests pass.

Rules:
- focus on recently modified code;
- preserve exact functionality;
- improve clarity/consistency/maintainability;
- prefer readable explicit code over compressed cleverness;
- rerun tests afterward.

Do not use simplification as permission to change APIs, database invariants, authorization, or observable behavior.

## 9. Agent Files

`AGENTS.md` is the cross-agent repository contract.

If using Claude Code, optionally add a short `CLAUDE.md` that points to `AGENTS.md` and skill locations rather than duplicating all rules.

If using Codex/Cursor/Windsurf/Copilot, install skills/rules in the tool-supported location, but keep repository-level engineering truth in version-controlled Markdown here.

## 10. Required Agent Status Format

At the end of implementation work, report:

```text
Implemented
- ...

Verification
- command: result
- command: result

Security / Data
- ...

UX / Accessibility
- ...

Known follow-ups
- only non-blocking items
```

Do not write “done,” “fixed,” or “production-ready” without verification evidence.
