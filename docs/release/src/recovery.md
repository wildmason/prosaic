# Recovery

## A crate publish fails mid-release

Re-run:

```powershell
.\scripts\publish-workspace.ps1 -Yes -RequireHeadTag
```

The script skips exact versions that crates.io already has, then resumes at the
next unpublished crate.

## Crates.io rate-limits the release

The publish script parses Cargo's retry timestamp and sleeps until the requested
time with a small safety pad. Leave the script running unless the error is not a
rate-limit or transient registry failure.

## A GitHub release asset is missing or stale

Re-run the Binary Release workflow manually with the existing tag. The release
job uploads with `--clobber`, so rebuilt assets replace stale ones.

## A checksum verification fails

Treat that as a hard stop. Rebuild the assets from the tagged commit through the
Binary Release workflow and verify again before announcing the release.

## A published crate version is wrong

Crates.io versions cannot be overwritten. Yank the bad version if leaving it
installable would harm users, then prepare and publish a new patch version.
