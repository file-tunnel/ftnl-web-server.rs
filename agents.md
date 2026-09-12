# File Tunnel web server agent instructions

These instructions apply to this repository and every directory beneath it.

## Repository role

- This repository owns the install-free mobile upload portal.
- The optional `ftnl-control-web` binary is a separate account-authenticated,
  metadata-only surface. Its database, API, TCP, and NATS providers must never
  enter the anonymous portal runtime or receive pairing/capability material.
- Read the one-time pairing secret only from the URI fragment, scrub it from
  the visible URL before network activity, and exchange it for a phone-scoped
  capability.
- Keep capabilities session-scoped and out of URLs, logs, telemetry, caches,
  persistent browser storage, and rendered error details.
- Preserve no-store responses, restrictive CSP and permissions headers,
  no-referrer behavior, exact-origin CORS expectations, safe runtime-config
  encoding, and cancellation/expiry failure boundaries.
- Do not add analytics, third-party runtime scripts, service-worker file
  caching, or server-side storage of user file bytes.

## Validation

- Run `nix develop --command agent-check` before completing a change.
- Run the cross-browser E2E suite in `ftnl-e2e` for user-visible transfer or
  browser security behavior changes.
- Never commit credentials, runtime `.env` files, user content, or build
  output.

## Git workflow

- Keep changes focused and reviewable.
- Pull and merge remote work before pushing; avoid git rebase in favor of git merge.
- Never discard unrelated or uncommitted user work.

## Repository-local Git worktrees

- Create or use a Git worktree only when the human operator explicitly authorizes it for the current task. Concurrency or a dirty checkout is not permission by itself.
- Put every authorized worktree at `<repository-root>/tmp/worktrees/<name>`; from the repository root, use `./tmp/worktrees/<name>`. Never place worktrees beside repositories or organization directories.
- Keep `tmp`, `temp`, `tmp/worktrees`, and `temp/worktrees` ignored in the repository-root `.gitignore`. Do not commit files from those directories.
- Relocate or remove a worktree only when the operator explicitly requests it. Before removal, preserve and publish intended changes, verify its commit is represented on the target branch, and confirm there are no tracked, untracked, ignored-sensitive, or in-use files that must survive. Remove it with `git worktree remove <path>` without `--force`; never delete a worktree directory with `rm`.
