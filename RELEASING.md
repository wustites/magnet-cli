# Releasing

Releases are published from signed-off version tags on `main`.

1. Update `Cargo.toml`, `Cargo.lock`, and `CHANGELOG.md` to the same version.
2. Run the local release gate:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --locked --all-targets -- -D warnings
   cargo test --locked --all-targets
   cargo audit
   cargo publish --locked --dry-run
   ```

3. Push the release commit and wait for every CI job to pass.
4. Create and push the matching tag, for example:

   ```bash
   git tag -a v0.2.0 -m "magnet-cli v0.2.0"
   git push origin v0.2.0
   ```

The tag workflow verifies that the tag matches `Cargo.toml`, publishes the crate,
and creates the GitHub release. Publishing to crates.io cannot be undone, so do
not move or reuse a released version tag.
