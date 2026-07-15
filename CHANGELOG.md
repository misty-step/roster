# Changelog

## [0.2.1] - 2026-07-15

- Make rollback-to-absent instructions honor the same installation prefix as
  install, including custom prefixes.
- Replay an attested previous public archive over the new release during each
  post-publication cold start, then roll forward to the current version.
- Publish the redacted authenticated-dispatch and cold-review evidence for the
  first public release, with exact timing and fail-closed workflow history.

## [0.2.0] - 2026-07-15

- Ship reproducible, checksummed native archives for Apple Silicon, Intel
  macOS, arm64 Linux, and x86 Linux with the matching public primitive library.
- Initialize an installed Roster without a development checkout through a
  dependency-free starter role and explicit Harness/model selection.
- Gate publication on exact version parity, the full repository oracle,
  Landmark release intelligence, provenance attestations, and installed-archive
  acceptance on clean macOS and Linux runners.
- Keep installation prefix-scoped and transactional, with redacted receipts,
  public-path leak checks, and a post-publication cold-start replay.
