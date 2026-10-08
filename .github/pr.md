# Pull Request Plan & Reference

Reference for drafting pull requests and executing GitHub CLI commands.
Full agent guidelines: -> [pr SKILL.md](../.agents/skills/git/pr/SKILL.md)

---

## Rules
- **No Emojis or Icons**: Strictly avoid icons and emojis in PR titles, headers, and bullet points.
- **No Verification Section**: Do not include test plans or checklists in descriptions.
- **Area Grouping**: Group key changes under the area they touch (core, server, web, fixtures).
- **No Trailers**: No Co-authored-by or generated-by line in the title or the body.

---

## PR Plan Template

### Title
```
type(scope): imperative summary
```

### Body
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

---

## Ready Copy-Paste Commands

### Create PR
```bash
gh pr create --base <origin-base-branch> --head $(git branch --show-current) \
  --title "type(scope): imperative summary" \
  --body-file - << 'EOF2'
## Summary
One or two sentences.

## Key Changes

### Area
- Detail.
EOF2
```

### Inspect PR
```bash
gh pr view --json number,title,body,state,url
```

### Merge PR
```bash
gh pr merge --merge --delete-branch
```
