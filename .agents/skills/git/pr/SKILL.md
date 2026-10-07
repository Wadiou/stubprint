---
name: pr
description: Draft GitHub pull requests with the gh CLI in this repository. Use when the user asks to open, inspect or merge a pull request.
---

# pr

How to plan pull requests: title, body, base branch and the commands, using the GitHub CLI (`gh`).

## When to create or merge a PR

- Only when the user asks to open, inspect or merge a pull request.
- Do not run `gh pr create` or `gh pr merge` on your own. Write **`.github/pr-plan.md`** with
  ready-to-run command blocks and tell the user it is ready. If the user says to run them now, in
  that message, run exactly what they asked.
- Never stage `.github/pr-plan.md`. It is a draft and is gitignored.
- Every plan has all three command blocks: create, inspect and merge.
- Never add a Co-authored-by or generated-by line to the title or the body.

## Before drafting

```bash
git status
git log --graph --oneline -n 10
git fetch origin <origin-base-branch>
git log --oneline origin/<origin-base-branch>..HEAD
```

- Find the branch this one was cut from. That is the base, not automatically `main`. If it was cut
  from a feature branch, that branch is the base.
- The working tree should be clean, with the work already committed (see [commit](../commit/SKILL.md)).
- The branch should be pushed: `git push -u origin <branch>`. Put it in the plan if it is not.

## Title

```
type(scope): imperative summary
```

Imperative, no trailing period, at most 100 characters, and no icons or emojis. Use the scopes from
the commit skill. A PR that covers several scopes uses the one that matters most.

## Body

No icons or emojis anywhere. No test plan, verification or checklist section. The body says what
changed and why, in plain sentences, and what a reviewer should know.

```markdown
## Summary
One or two sentences on what this does and why.

## Key Changes

### Area
- What was added or changed, in a short sentence.

## Things to Know
- Only when there is something to warn about: a removed option, a behavior change, a step the
  reviewer must take.
```

Group Key Changes by area (core, server, web, fixtures and so on), not by file. Leave out Things to
Know when there is nothing to say.

## The plan file

`.github/pr-plan.md` has the title, the body, and the three command blocks.

````markdown
# Pull Request Plan

Base branch is `<origin-base-branch>`. Push first if needed:

```bash
git push -u origin <branch>
```

### Title

```
type(scope): imperative summary
```

### Body

```markdown
## Summary
...
```

## Ready Copy-Paste Commands

### Create PR

```bash
gh pr create --base <origin-base-branch> --head <branch> \
  --title "type(scope): imperative summary" \
  --body-file - << 'EOF'
## Summary
...
