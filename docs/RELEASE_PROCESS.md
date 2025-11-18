# Release Process

Thoth uses automated release management via GitHub Actions and `cargo-release`.

## Creating a New Release

### 1. Go to GitHub Actions

1. Navigate to the repository on GitHub
2. Click on the **Actions** tab
3. Select **Create Release** workflow from the left sidebar
4. Click **Run workflow** button

### 2. Select Release Type

Choose the type of release based on semantic versioning:

- **Patch** (0.2.4 → 0.2.5): Bug fixes, minor improvements, no breaking changes
- **Minor** (0.2.4 → 0.3.0): New features, backward compatible
- **Major** (0.2.4 → 1.0.0): Breaking changes, major overhaul

### 3. Optional: Dry Run

Check the **Dry run** checkbox to preview what will happen without making any changes:

- Shows what the new version will be
- Displays what files will be modified
- No commits, tags, or releases are created

### 4. Create the Release

Uncheck **Dry run** and click **Run workflow** to create the actual release.

## Setup Required

Before using the automated release workflow, you need to set up a Personal Access Token:

1. Go to GitHub → Settings → Developer settings → Personal access tokens → Tokens (classic)
2. Generate new token with `repo` and `workflow` permissions
3. Copy the token
4. Go to repository → Settings → Secrets and variables → Actions
5. Create new secret named `RELEASE_TOKEN` with your PAT

This allows the workflow to bypass branch protection rules and push directly to main.

## What Happens Automatically

When you trigger the workflow, the following happens automatically:

### Step 1: Version Bump (create-release.yml)

```
1. Checkout code
2. Install cargo-release
3. Run: cargo release {type} --no-publish --execute
   - Updates version in Cargo.toml (package and packager metadata)
   - Updates Cargo.lock
   - Creates commit: "chore: release v0.2.5"
   - Creates git tag: v0.2.5
   - Pushes tag to GitHub
```

### Step 2: Build & Release (release.yml - triggered by tag)

```
1. Detects new tag (v*)
2. Installs cargo-packager (production-ready cross-platform bundler)
3. Builds binaries for all platforms:
   - Windows (x64) - MSI installer (via WiX) + portable EXE
   - macOS (Intel) - DMG installer + .app bundle
   - macOS (Apple Silicon) - DMG installer + .app bundle
   - Linux (x64) - DEB package + portable binary
4. Generates changelog from git commits
5. Creates GitHub Release with all artifacts
6. Uploads installers and archives
```

## Manual Release (Local Development)

If you need to create a release locally for testing:

### Prerequisites

```bash
cargo install cargo-release
```

### Dry Run (Preview Changes)

```bash
# See what would happen without making changes
cargo release patch --dry-run
cargo release minor --dry-run
cargo release major --dry-run
```

### Create Release

```bash
# Patch release (0.2.4 → 0.2.5)
cargo release patch --no-publish --execute

# Minor release (0.2.4 → 0.3.0)
cargo release minor --no-publish --execute

# Major release (0.2.4 → 1.0.0)
cargo release major --no-publish --execute
```

This will:

1. Bump version in Cargo.toml
2. Update Cargo.lock
3. Create commit and tag
4. Push to GitHub
5. Trigger the release workflow

## Semantic Versioning Guidelines

### Patch (0.0.X)

- Bug fixes
- Performance improvements
- Documentation updates
- Internal refactoring
- Dependency updates (non-breaking)

### Minor (0.X.0)

- New features
- New settings/options
- UI improvements
- Enhanced functionality
- Backward-compatible changes

### Major (X.0.0)

- Breaking changes
- Complete redesigns
- Removed features
- Changed APIs
- Migration required

## Troubleshooting

### Workflow fails at "cargo release"

- Check that version bumping is valid
- Ensure no uncommitted changes exist
- Verify git is properly configured

### Tag already exists

- Delete the tag locally: `git tag -d v0.2.5`
- Delete remote tag: `git push origin :refs/tags/v0.2.5`
- Run the workflow again

### Release workflow not triggered

- Check that the tag follows pattern `v*.*.*`
- Verify the tag was pushed to GitHub
- Check GitHub Actions is enabled for the repo

### Build failures

- Check the build logs in the release.yml workflow
- Verify all platforms are building correctly
- Test builds locally first if needed

## Rollback a Release

If you need to rollback a release:

1. **Delete the GitHub Release**:
   - Go to Releases on GitHub
   - Click the release
   - Click "Delete release"

2. **Delete the Git Tag**:

   ```bash
   git tag -d v0.2.5
   git push origin :refs/tags/v0.2.5
   ```

3. **Revert the Version Commit**:
   ```bash
   git revert HEAD
   git push origin main
   ```

## Release Checklist

Before creating a release, ensure:

- [ ] All tests pass (`cargo test`)
- [ ] Code compiles without warnings (`cargo clippy`)
- [ ] Documentation is up to date
- [ ] CHANGELOG entries are meaningful
- [ ] Breaking changes are documented
- [ ] Version bump type is correct (major/minor/patch)
- [ ] Consider running a dry run first

## Configuration

Release settings are configured in `Cargo.toml`:

```toml
[package.metadata.release]
publish = false              # Don't publish to crates.io
push = true                  # Push tags after creation
tag-prefix = "v"            # Tag format: v0.2.5
pre-release-commit-message = "chore: release v{{version}}"

[package.metadata.packager]
product_name = "Thoth"
identifier = "com.thoth.app"
category = "DeveloperTool"
# ... additional packager configuration
```

Workflow files:

- `.github/workflows/create-release.yml` - Manual release trigger
- `.github/workflows/release.yml` - Build and publish release artifacts

## Bundling Tool

Thoth uses [cargo-packager](https://github.com/crabnebula-dev/cargo-packager) for creating installers across all platforms:

- **Windows**: MSI installer using WiX Toolset v3 (automatically installed in CI)
- **macOS**: DMG disk images with native .app bundles
- **Linux**: DEB packages for Debian/Ubuntu

cargo-packager is a production-ready tool that evolved from Tauri's bundler and provides stable, reliable packaging for all platforms. It replaced the experimental cargo-bundle to fix MSI corruption issues on Windows.
