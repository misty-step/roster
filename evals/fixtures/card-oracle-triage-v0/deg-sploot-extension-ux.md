# Extension UX: reachable auth, screenshot capture, unmissable feedback

Priority: P2 · Status: ready · Estimate: M

## Goal

The Chrome extension is pleasant to use: signing in never traps you in a
dismissable popup, you can screenshot-to-save, and every save visibly succeeds or
fails.

## Context

A 2026-06-22 investigation mapped the operator's three asks against live code
(`apps/extension`). Notably, the auth complaint is mostly already fixed in code —
verify the installed build.

## Notes

Investigation 2026-06-22 "extension". File map: popup `App.tsx` ·
`background/auth-manager.ts` · `background/context-menu.ts` ·
`background/notifications.ts` · `shared/api-client.ts`. MV3 limits:
`captureVisibleTab` fails on `chrome://`, the store, and PDF; `desktopCapture` /
region-crop deferred. Sequence 1 → 2 → 3 (2 gives the popup a real save action
that 3's feedback then serves). Keep as child tickets for independent
verification, not one mega-ticket.
