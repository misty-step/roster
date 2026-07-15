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
     --release-tag v0.2.1 --dry-run
   ```

3. Create and push an annotated tag:

   ```sh
   git tag -a v0.2.1 -m "Roster v0.2.1"
   git push origin v0.2.1
   ```

The `release` workflow refuses package/tag drift, validates `.landmark.yml`,
records Landmark's dry-run evidence, builds four archives, publishes SHA-256
checksums and GitHub provenance attestations, asks Landmark to synthesize the
release body, and replays the public install path on clean macOS and Linux
runners.

## Verify

```sh
gh release download v0.2.1 --pattern 'roster-v0.2.1-*.tar.gz' --pattern checksums.txt
archive=roster-v0.2.1-aarch64-apple-darwin.tar.gz # choose the local target
expected=$(grep " $archive$" checksums.txt | awk '{print $1}')
actual=$(shasum -a 256 "$archive" | awk '{print $1}')
test "$actual" = "$expected"
gh attestation verify "$archive" --repo misty-step/roster
```

The release is complete only after both `cold-start` jobs pass and an
authenticated host launches one real Tier 1 dispatch from the installed
archive. Retain the redacted receipt and bundle manifest as card evidence.

## Recover an unpublished tag

If the workflow fails before `gh release create`, first verify that no release
exists and retain the failed run as evidence. An unpublished tag may then be
deleted and recreated at the corrective commit:

```sh
release_status=$(gh api --include \
  repos/misty-step/roster/releases/tags/v0.2.0 2>&1 | \
  sed -n '1s/.* \([0-9][0-9][0-9]\) .*/\1/p')
case "$release_status" in
  404) ;;
  200) echo "v0.2.0 is published; use a new patch version" >&2; exit 1 ;;
  *) echo "could not prove v0.2.0 is unpublished" >&2; exit 1 ;;
esac
git push origin :refs/tags/v0.2.0
git tag -d v0.2.0
git tag -a v0.2.0 -m "Roster v0.2.0"
git push origin v0.2.0
```

This recovery is forbidden once a GitHub Release exists. Published releases
and their tags are immutable; use a new patch version instead.

## Roll back

Consumer rollback is the same verified operation as install: download the
last known-good archive, verify its checksum and attestation, extract it, and
run its `install.sh`. The installer replaces only its owned
`PREFIX/bin/roster` and `PREFIX/share/roster` surfaces and restores the prior
pair if installation fails.

The initial v0.2.0 release has no older public archive. Its rollback-to-absent
path is removal of those two owned surfaces under the exact installation
prefix. From v0.2.1 onward, each post-publication cold start downloads the
newest prior public release, verifies its checksum and attestation, installs it
over the current version, checks both binary and library versions, then rolls
forward again. Same-version repair is not cross-version rollback evidence.

If a published release is bad, do not move or reuse its tag. Mark it as a
prerelease while the incident is active and publish a new patch release:

```sh
gh release edit v0.2.0 --prerelease
```

That preserves the tag, checksums, attestations, and audit trail.
