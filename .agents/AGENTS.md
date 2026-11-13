# Agent guidelines

Project: a fake receipt printer for development. A Rust workspace (`crates/core`, `crates/render`,
`crates/server`) with a web app in `web/`. Read `docs/plans/PLAN.md` first.

## Skills

Skills live in `.agents/skills/`. Read the matching `SKILL.md` before doing the task.

| Skill | Path | Use when |
|-------|------|----------|
| commit | `skills/git/commit/SKILL.md` | The user asks to commit, split changes into commits, or draft a message |
| pr | `skills/git/pr/SKILL.md` | The user asks to open, inspect or merge a pull request |

## Rules for every task

1. Read the skill first, then do the task.
2. Do not run `git add`, `git commit`, `git push`, `gh pr create` or `gh pr merge` on your own.
   Write the plan files described in the skills. Run them only when the user asks you to in that
   message.
3. Never add `Co-authored-by` trailers or generated-by lines to commits or pull requests.
4. Describe changes in plain language, as they differ from the code before the session.
