---
name: commit
description: Plan and write scoped Conventional Commits for this repository. Use when the user asks to commit, split changes into commits, amend, or draft a commit message.
---

# commit

How to commit in this repository: the message style, how to split work, and the plan file.

## When to commit

- Only when the user asks to commit or to prepare commits.
- Do not run `git add`, `git commit` or `git push` on your own. Write **`.github/commit-plan.md`**
  with ready-to-run command blocks, and tell the user it is ready.
- If the user says to commit or push now, in that message, run exactly what they asked and nothing
  more. An approval does not carry over to later changes.
- Never stage `.github/commit-plan.md`. It is a draft and is gitignored.
- When the user has run commits from the plan, remove those blocks from it.
- Never change `git config`, skip hooks (`--no-verify`), or use interactive git (`-i`).
- Never add `Co-authored-by` or other generated-by trailers.
- Amend only if the user asks, HEAD is your own commit from this session, and it is not pushed.
  If a commit was rejected, fix the problem and make a new commit.

## Before drafting

Run these together from the repo root:

```bash
git status
git diff
git diff --staged
git log -5 --oneline
```

- Only look at tracked and changed files. Do not stage or report ignored files.
- Never include `.env`, keys or other secrets. Warn and leave them out.
- Decide how many commits the diff needs, from one to as many as it takes.

## Message format

```
type(scope): imperative subject

Body in plain sentences that explain why the change exists.
```

| Part | Rule |
|------|------|
| Type | `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert` |
| Scope | Required, from the table below |
| Subject | Imperative, no trailing period, at most 100 characters |
| Body | A blank line after the subject, one to three short paragraphs, no file lists |
| Line length | Every body line at most 100 characters; wrap near 90 |

Describe what changed compared to the code before the session. Do not narrate the steps taken
along the way.

### Scopes

| Scope | Use when the change is in |
|-------|---------------------------|
| `core` | `crates/core`, the decoders and the document model |
| `server` | `crates/server`, the TCP listener, the REST API and events |
| `web` | `web/`, the page |
| `fixtures` | `fixtures/`, the sample printer streams and their generator |
| `docs` | `docs/` and the README |
| `repo` | Root files, the workspace, tooling and CI |
| `deps` | Dependency bumps only |
| `skill` | `.agents/` |

## How to split work

- One concern per commit. A refactor does not hide a behavior change in the same commit.
- Each commit should pass `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` and
  `cargo test --workspace` on its own. When a change cannot be split without breaking the build,
  keep it in one commit and say why in the plan.
- Put what other work depends on first: for example fixtures before the tests that read them.
- Dependency-only changes can be their own commit.

## The plan file

`.github/commit-plan.md` holds one block per commit, in the order to run them:

````markdown
# Commit Plan

## Commit 1

type(scope): imperative subject

```
type(scope): imperative subject

Staged files:
- path/one
- path/two

Body in plain sentences that explain why.
```

## Command

```bash
git add \
  path/one \
  path/two

git commit -m "$(cat <<'EOF2'
type(scope): imperative subject

Body in plain sentences that explain why.
EOF2
)"
```
````

Single-quote any path that contains `$` or other special characters. Notes about commit order or
files to leave out go outside the command block. After writing the plan, reply with the subject
lines only.

## Safety

| Do not | Unless the user explicitly asks |
|--------|---------------------------------|
| `git push --force` to `main` | They understand the risk |
| `git reset --hard` | They ask for exactly that |
| Skip hooks | They ask |
| Commit secrets | Never |

## Related

- [pr](../pr/SKILL.md) for opening a pull request once commits are done.
