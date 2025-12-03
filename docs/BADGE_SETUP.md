# Badge Setup Guide

This guide explains how to set up the coverage and benchmark badges for your README.

## Prerequisites

You need to create:
1. A GitHub Personal Access Token (PAT) with `gist` scope
2. A GitHub Gist to store the badge data

## Step-by-Step Setup

### 1. Create a GitHub Personal Access Token

1. Go to GitHub Settings → Developer settings → Personal access tokens → Tokens (classic)
2. Click "Generate new token (classic)"
3. Give it a descriptive name like "Thoth Badge Generator"
4. Select the `gist` scope (this allows creating/updating gists)
5. Click "Generate token"
6. **Copy the token immediately** (you won't be able to see it again)

### 2. Create a GitHub Gist

1. Go to https://gist.github.com/
2. Create a new gist with:
   - Description: "Thoth Badges"
   - Filename: `thoth-coverage.json`
   - Content: `{}`
3. Click "Create public gist"
4. Copy the Gist ID from the URL (e.g., `https://gist.github.com/USERNAME/GIST_ID`)

### 3. Add GitHub Secrets

1. Go to your repository → Settings → Secrets and variables → Actions
2. Click "New repository secret"
3. Add two secrets:

   **Secret 1: GIST_SECRET**
   - Name: `GIST_SECRET`
   - Value: The Personal Access Token you created in step 1

   **Secret 2: GIST_ID**
   - Name: `GIST_ID`
   - Value: The Gist ID you copied in step 2

### 4. Update README Badge URLs

Replace `YOUR_GIST_ID` in the README.md badges with your actual Gist ID:

```markdown
[![Coverage](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/anitnilay20/YOUR_GIST_ID/raw/thoth-coverage.json)](https://github.com/anitnilay20/thoth/actions/workflows/badges.yml)
[![Benchmarks](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/anitnilay20/YOUR_GIST_ID/raw/thoth-benchmarks.json)](https://anitnilay20.github.io/thoth/dev/bench/)
```

### 5. Enable GitHub Pages for Benchmarks (Optional)

To view benchmark history graphs:

1. Go to repository → Settings → Pages
2. Under "Source", select "Deploy from a branch"
3. Choose the `gh-pages` branch and `/ (root)` folder
4. Click "Save"
5. Benchmarks will be available at: `https://anitnilay20.github.io/thoth/dev/bench/`

## How It Works

### Coverage Badge

The `badges.yml` workflow:
1. Runs on every push to `main`
2. Uses `cargo-tarpaulin` to generate code coverage
3. Extracts the coverage percentage
4. Updates the gist with the new coverage data
5. The badge automatically reflects the updated data

Colors:
- 🟢 Green: ≥80% coverage
- 🟡 Yellow: 60-79% coverage
- 🔴 Red: <60% coverage

### Benchmark Badge

The `badges.yml` workflow:
1. Runs benchmarks using `cargo bench`
2. Stores results in the `gh-pages` branch
3. Updates the gist with a "tracked" status
4. Historical data is viewable on GitHub Pages

## Alternative: Using Codecov Badge

If you prefer using Codecov instead of a custom coverage badge:

```markdown
[![codecov](https://codecov.io/gh/anitnilay20/thoth/branch/main/graph/badge.svg)](https://codecov.io/gh/anitnilay20/thoth)
```

You'll need to:
1. Sign up at https://codecov.io with your GitHub account
2. Add the Thoth repository
3. Get the upload token from Codecov
4. Add it as a repository secret named `CODECOV_TOKEN`

## Troubleshooting

### Badge shows "invalid"
- Check that the Gist ID in README matches your actual Gist ID
- Verify the gist filenames match exactly: `thoth-coverage.json` and `thoth-benchmarks.json`
- Ensure the workflow has run at least once on the main branch

### Badge doesn't update
- Check that `GIST_SECRET` has the correct permissions (gist scope)
- Verify the workflow ran successfully in Actions tab
- Clear browser cache (badges may be cached)

### Benchmarks not showing
- Make sure GitHub Pages is enabled
- Check that the `gh-pages` branch exists
- Verify the workflow has write permissions to the repository

## Manual Trigger

You can manually trigger the badge generation workflow:

1. Go to Actions tab
2. Select "Generate Badges" workflow
3. Click "Run workflow"
4. Select the `main` branch
5. Click "Run workflow"

## Benefits

- **Coverage Badge**: Shows test coverage percentage at a glance
- **Benchmark Badge**: Links to performance tracking over time
- **CI Badge**: Shows build status across platforms
- **Automated**: Updates automatically on every push to main
- **Free**: Uses GitHub Actions, Gists, and Pages (all free for public repos)
