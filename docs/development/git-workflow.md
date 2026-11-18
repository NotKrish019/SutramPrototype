# Git & Collaboration Workflow — Single Main Branch Policy

To ensure high velocity and eliminate merge hell during hackathon sprints, **YUKTI development operates on a strict single-branch (`main`) policy**.

---

## 1. Golden Rules
1. **NO EXTRA BRANCHES:** Do not create `develop`, `staging`, `feature/*`, or personal branches. All commits are pushed directly to `main`.
2. **NO FORCE PUSHING:** `git push --force` or `--force-with-lease` is strictly prohibited.
3. **NO HISTORY REWRITING:** Never run `git reset --hard` on commits that have been pushed to `origin/main`.
4. **SYNCHRONIZE CONSTANTLY:** Always rebase from `origin/main` before starting work and before pushing.

---

## 2. Daily Workflow for All 4 Developers

### Step A: Synchronize Before Writing Code
Before opening your editor or modifying files:
```bash
git pull --rebase origin main
```

### Step B: Work in Small Increments
Focus on small, modular changes within your assigned module.

### Step C: Verify Locally
Never commit broken code:
- Run backend tests: `cd backend && uv run pytest`
- Run frontend tests: `cd apps/web && npm run test && npm run build`
- Run decision tests: `cd decision-service && npm run test`

### Step D: Commit Using Conventional Commits
```bash
git status
git add <specific-files-you-changed>
git commit -m "feat(ingestion): implement sha256 checksum validator"
```

### Step E: Rebase and Push
Before pushing, ensure no teammate pushed in the meantime:
```bash
git pull --rebase origin main
git push origin main
```

---

## 3. Resolving Divergences / Conflicts

If another developer pushed while you were working:
1. When you run `git pull --rebase origin main`, Git will replay your local commits on top of origin's latest commit.
2. If Git pauses with a conflict:
   - Run `git status` to see conflicting files.
   - Open the conflicting files and resolve the markers (`<<<<<<<`, `=======`, `>>>>>>>`).
   - Run tests to confirm code stability.
   - Stage resolved files: `git add <resolved-file>`
   - Continue rebase: `git rebase --continue`
3. Push cleanly: `git push origin main`.
