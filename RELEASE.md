# Releasing gb-strudel

Releases are automated with [release-plz](https://release-plz.dev) through
`.github/workflows/release-plz.yml`. You never run `cargo publish` or edit the
version by hand. The normal flow is **merge the release PR**.

## How it works

The workflow runs two jobs on every push to `main`:

- **Release PR** (`release-plz-pr`): if any file that ends up in the published
  crate changed since the last release, it opens a PR titled
  `chore: release vX.Y.Z` (or updates the one already open). That PR bumps
  `version` in `Cargo.toml` and `Cargo.lock` and adds a section to
  `CHANGELOG.md` built from the commit messages.
- **Release** (`release-plz-release`): if the version in `Cargo.toml` is not
  on crates.io yet, it publishes it, pushes a `vX.Y.Z` tag and creates a GitHub
  release with the changelog section as its notes. Otherwise it does nothing.

Merging the release PR is a push to `main` with an unpublished version, which
is what triggers the second job.

"Files in the crate" means the `include` list in `Cargo.toml`: `src/`,
`templates/`, `songs/*.gbs`, `README.md`, `LICENSE` and `CHANGELOG.md`.
Changes to tests, CI or `RELEASE.md` alone do not open a release PR.

## Cutting a release

1. **Land your changes on `main`** with
   [conventional commit](https://www.conventionalcommits.org) messages. The
   type decides both the version bump and the changelog section:

   ```
   feat: add swing to the pattern notation
   fix: stop the status line flickering on resize
   feat!: rename the `ticks` key to `tempo`
   ```

2. **Wait for the release PR.** Within a minute or two of the push, a
   `chore: release vX.Y.Z` PR appears (or the open one is updated). Each later
   push to `main` updates it again, so there is only ever one.

3. **Review it.**
   - Check the proposed version (see [Version bumps](#version-bumps)).
   - Read the new `CHANGELOG.md` section. It's the text users see on the GitHub
     release, and you can edit it directly on the PR branch.

4. **Merge it.** The release job then publishes to crates.io, tags `vX.Y.Z` and
   creates the GitHub release. Follow it under *Actions → Release-plz*.

5. **Check the result:**
   - <https://crates.io/crates/gb-strudel> shows the new version.
   - `cargo install gb-strudel` installs it.
   - The GitHub release exists for the tag.

Publishing to crates.io **cannot be undone**. A bad version can only be
yanked (`cargo yank --version X.Y.Z`), which stops new projects picking it up
but leaves it downloadable. Fix forward with a new patch release.

## Version bumps

release-plz derives the next version from the commits since the last release.
While gb-strudel is below 1.0 the rules are deliberately conservative:

| commits since last release           | 0.x (now)            | 1.0 and later   |
|--------------------------------------|----------------------|-----------------|
| breaking (`feat!:`, `BREAKING CHANGE:`) | minor: 0.3.1 → 0.4.0 | major           |
| `feat:`                              | patch: 0.3.1 → 0.3.2 | minor           |
| `fix:` and anything else             | patch                | patch           |

For a CLI, "breaking" means an existing `.gbs` file stops compiling or
produces a different `.uge`, or a command or flag changes. Mark such commits
with `!`.

If features should bump the minor version even before 1.0, add a
`release-plz.toml` at the repository root:

```toml
[workspace]
features_always_increment_minor = true
```

## Choosing the version yourself

When the computed version is wrong (a breaking change without `!`, or going to
1.0), change it on the release PR before merging. Either edit `version` in
`Cargo.toml` on the PR branch, or check out the branch and run:

```sh
cargo install release-plz --locked
release-plz set-version 1.0.0
git commit -am "chore: release v1.0.0" && git push
```

## Skipping a release

Don't merge the release PR. Leave it open and it keeps collecting changes
until you're ready. Closing it only lasts until the next push to `main`, which
opens a new one.

## Troubleshooting

- **No release PR appears.** Check that the push changed a file in `include`.
  Also check that *Settings → Actions → General → Workflow permissions →
  "Allow GitHub Actions to create and approve pull requests"* is enabled.
- **CI doesn't run on the release PR.** This is expected. GitHub does not
  start workflows for events created with the built-in `GITHUB_TOKEN`, which
  is what release-plz uses. The PR only changes the version and changelog, and
  `cargo publish` verifies the build anyway. If you want CI on it, give
  release-plz a GitHub App or personal access token instead; the release-plz
  docs explain how.
- **Release job fails to authenticate with crates.io.** Publishing uses
  crates.io trusted publishing, so no token is stored in GitHub. On crates.io,
  under *gb-strudel → Settings → Trusted Publishing*, there must be a GitHub
  entry for owner `KeeTraxx`, repository `gb-strudel`, workflow
  `release-plz.yml`. Fix it, then re-run the failed job.
- **Release job fails to build.** `cargo publish` compiles the packaged crate,
  which only contains the `include` list. A file the build needs that isn't
  listed there fails here. CI runs `cargo package` on every push to catch
  this earlier.
