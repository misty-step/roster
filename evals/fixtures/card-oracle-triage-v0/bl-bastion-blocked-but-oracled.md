# Verify internal routing when admin state exists

Priority: P2 | Status: blocked | Estimate: S

## Goal
Finish the private `.internal` routing proof once the external Tailscale admin state is in place.

## Oracle
- [ ] Bastion's Tailscale peer advertises the Fly org-private IPv6 route in `AllowedIPs`.
- [ ] Split DNS for `internal` sends queries to Bastion's DNS proxy from at least two trusted devices.
- [ ] `tailscale dns query <app>.internal AAAA` returns the Fly private address and `curl -fsS http://<app>.internal/healthz` succeeds from those devices.
- [ ] README status is updated only after the live proof passes.

## Notes
Blocked on external Tailscale admin settings: route approval and split DNS. The repo can advertise and verify; it cannot mark this complete on local tests alone.
