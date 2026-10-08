# Commit Plan & Conventions

Reference for drafting commit plans and running scoped Conventional Commits.
Full agent guidelines: -> [commit SKILL.md](../.agents/skills/git/commit/SKILL.md)

---

## Allowed Scopes
`core`, `render`, `server`, `web`, `fixtures`, `docs`, `repo`, `deps`, `skill`

---

## Commit Plan Structure

Apply commits in order (1 → N). For each block:
1. Review message draft and staged files
2. Run the executable command block

---

## Commit 1

### Message
```
type(scope): imperative subject

Why this commit exists, not a file list.
Wrap body lines at <=100 characters.
```

### Staged Files
- `path/to/file1.rs`
- `path/to/file2.rs`

### Command
```bash
git add \
  path/to/file1.rs \
  path/to/file2.rs

git commit -m "$(cat <<'EOF2'
type(scope): imperative subject

Why this commit exists, not a file list.
Wrap body lines at <=100 characters.
EOF2
)"
```
