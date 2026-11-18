# Contributing to YUKTI Digital Forensics

This repository is maintained by a 4-member team participating in **SUTRAM 2026 (ISEA-IGDTUW National Hackathon)**.
To maintain high agility and code quality, all contributors must follow these procedures.

---

## 1. Single Main Branch Policy

We use **ONLY** the `main` branch.
- **Strictly Prohibited:** Creating branches (`develop`, `feature/*`, `staging`, `testing`, personal branches).
- **Strictly Prohibited:** `git push --force` or `git reset --hard` against remote commits.
- All four members commit directly to `main` following the rebase synchronization protocol.

---

## 2. Commit Message Conventions

We adhere to the Conventional Commits specification:
```
<type>(<scope>): <short description>
```
### Allowed Types:
- `feat`: A new feature or capability.
- `fix`: A bug fix.
- `docs`: Documentation only changes.
- `style`: Changes that do not affect the meaning of the code (formatting, whitespace).
- `refactor`: A code change that neither fixes a bug nor adds a feature.
- `test`: Adding missing tests or correcting existing tests.
- `chore`: Changes to build process, dependency configuration, or tools.

### Allowed Scopes:
- `ingestion`, `extraction`, `correlation`, `reconstruction`, `reporting`, `api`, `web`, `decision`, `shared`, `infra`, `ci`

---

## 3. Daily Synchronization & Push Procedure

Before beginning work:
```bash
git pull --rebase origin main
```

Before committing:
1. Run local tests:
   ```bash
   # Backend
   cd backend && uv run pytest
   # Frontend
   cd apps/web && npm run test && npm run build
   # Decision service
   cd decision-service && npm run test
   ```
2. Check Git status:
   ```bash
   git status
   ```
3. Stage and commit specific files:
   ```bash
   git add <modified-files>
   git commit -m "feat(scope): concise description"
   ```
4. Rebase and push:
   ```bash
   git pull --rebase origin main
   git push origin main
   ```

---

## 4. Conflict Resolution Procedure

If another team member pushed to `main` while you were editing:
1. `git pull --rebase origin main` will pause if there are conflicting file edits.
2. Run `git status` to identify unmerged paths.
3. Open the conflicted files, discuss with the relevant team member, and resolve the markers.
4. Verify tests pass.
5. Mark resolved: `git add <file>`.
6. Continue rebase: `git rebase --continue`.
7. Once clean, push: `git push origin main`.

---

## 5. Dependency Management Rules

- Never install packages directly with global pip or arbitrary tools.
- Backend: Use `uv add <package>` in `backend/`.
- Frontend: Use `npm install <package>` in `apps/web/`.
- Decision service: Use `npm install <package>` in `decision-service/`.
- Do not add massive AI weights or unneeded packages in early phases.

---

## 6. Forensic Data Security Rules

- **Zero Credentials in Git:** Keep `.env` strictly local.
- **Zero Real Evidence:** Real seized data, victim PII, or authentic CCTV footage must never touch the repository.
- Use synthetic data fixtures only (`data/synthetic/`).
