# Releasing Roster

Roster releases are immutable, checksummed bundles of one native binary and
the matching public plain-file library. Landmark owns the release decision,
release evidence, and grounded public notes. GitHub Actions owns the four
native builds, provenance attestations, publication, and clean-room replay.

## Publish

1. Land a green release-preparation pull request on `master`. All package
   versions must already match the intended tag.
2. Preview the exact initial release decision. Roster's first public tag is an
   explicit anchor because no earlier tag exists for Landmark to infer from:

   ```sh
   cargo run --locked --manifest-path ../landmark/Cargo.toml -p landmark -- \
     run --provider local --repo-root . --repository misty-step/roster \
     --release-tag v0.2.0 --dry-run
   ```

3. Create and push an annotated tag:

   ```sh
   git tag -a v0.2.0 -m "Roster v0.2.0"
   git push origin v0.2.0
   ```

The `release` workflow refuses package/tag drift, validates `.landmark.yml`,
records Landmark's dry-run evidence, builds four archives, publishes SHA-256
checksums and GitHub provenance attestations, asks Landmark to synthesize the
release body, and replays the public install path on clean macOS and Linux
runners.

## Verify

```sh
gh release download v0.2.0 --pattern 'roster-v0.2.0-*.tar.gz' --pattern checksums.txt
archive=roster-v0.2.0-aarch64-apple-darwin.tar.gz # choose the local target
expected=$(grep " $archive$" checksums.txt | awk '{print $1}')
actual=$(shasum -a 256 "$archive" | awk '{print $1}')
test "$actual" = "$expected"
gh attestation verify "$archive" --repo misty-step/roster
```

The release is complete only after both `cold-start` jobs pass and an
authenticated host launches one real Tier 1 dispatch from the installed
archive. Retain the redacted receipt and bundle manifest as card evidence.

## Roll back

Consumer rollback is the same verified operation as install: download the
last known-good archive, verify its checksum and attestation, extract it, and
run its `install.sh`. The installer replaces only its owned
`PREFIX/bin/roster` and `PREFIX/share/roster` surfaces and restores the prior
pair if installation fails.

The initial v0.2.0 release has no older public archive. Its rollback-to-absent
path is removal of those two owned surfaces; cross-version rollback can first
be replayed when v0.2.1 exists. Do not describe same-version repair as evidence
of a cross-version rollback.

If a published release is bad, do not move or reuse its tag. Mark it as a
prerelease while the incident is active and publish a new patch release:

```sh
gh release edit v0.2.0 --prerelease
```

That preserves the tag, checksums, attestations, and audit trail.
