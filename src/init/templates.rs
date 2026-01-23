use super::detect::ProjectType;

/// Get the workflow template for a project type
pub fn get_workflow_template(project_type: ProjectType) -> &'static str {
    match project_type {
        ProjectType::Rust => RUST_TEMPLATE,
        ProjectType::Go => GO_TEMPLATE,
        ProjectType::Bun => BUN_TEMPLATE,
        ProjectType::Pnpm => PNPM_TEMPLATE,
        ProjectType::NextJs => NEXTJS_TEMPLATE,
        ProjectType::NodeJs => NODEJS_TEMPLATE,
        ProjectType::ReactNative => REACT_NATIVE_TEMPLATE,
        ProjectType::Xcode => XCODE_TEMPLATE,
        ProjectType::Python => PYTHON_TEMPLATE,
        ProjectType::Generic => GENERIC_TEMPLATE,
    }
}

const RUST_TEMPLATE: &str = r#"name: Auto Release

on:
  push:
    branches: [main]
    paths-ignore:
      - '*.md'

permissions:
  contents: write

jobs:
  check-version:
    runs-on: ubuntu-latest
    outputs:
      new_version: ${{ steps.bump.outputs.new_version }}
      should_release: ${{ steps.bump.outputs.should_release }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Determine version bump
        id: bump
        run: |
          # Get latest tag
          LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
          CURRENT_VERSION=${LATEST_TAG#v}

          echo "Current version: $CURRENT_VERSION"

          # Get commits since last tag
          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          if [ -z "$COMMITS" ]; then
            echo "No new commits"
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          echo "Commits since $LATEST_TAG:"
          echo "$COMMITS"

          # Determine bump type from conventional commits
          BUMP="none"

          if echo "$COMMITS" | grep -qiE "^BREAKING CHANGE:|^[a-z]+(\(.+\))?!:"; then
            BUMP="major"
          elif echo "$COMMITS" | grep -qiE "^feat(\(.+\))?:"; then
            BUMP="minor"
          elif echo "$COMMITS" | grep -qiE "^fix(\(.+\))?:|^perf(\(.+\))?:|^refactor(\(.+\))?:"; then
            BUMP="patch"
          fi

          if [ "$BUMP" = "none" ]; then
            echo "No version bump needed (no feat/fix/breaking commits)"
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          echo "Bump type: $BUMP"

          # Parse version
          IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

          # Apply bump
          case $BUMP in
            major)
              MAJOR=$((MAJOR + 1))
              MINOR=0
              PATCH=0
              ;;
            minor)
              MINOR=$((MINOR + 1))
              PATCH=0
              ;;
            patch)
              PATCH=$((PATCH + 1))
              ;;
          esac

          NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
          echo "New version: $NEW_VERSION"

          echo "new_version=$NEW_VERSION" >> $GITHUB_OUTPUT
          echo "should_release=true" >> $GITHUB_OUTPUT

  build:
    name: Build ${{ matrix.target }}
    needs: check-version
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-apple-darwin
            os: macos-latest
            archive: tar.gz
          - target: aarch64-apple-darwin
            os: macos-latest
            archive: tar.gz
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            archive: tar.gz
          - target: x86_64-pc-windows-msvc
            os: windows-latest
            archive: zip

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Update version in Cargo.toml
        shell: bash
        run: |
          sed -i.bak 's/^version = ".*"/version = "${{ needs.check-version.outputs.new_version }}"/' Cargo.toml
          rm -f Cargo.toml.bak

      - name: Build
        run: cargo build --release --target ${{ matrix.target }}

      - name: Package (Unix)
        if: matrix.archive == 'tar.gz'
        run: |
          cd target/${{ matrix.target }}/release
          tar -czvf ../../../app-${{ matrix.target }}.tar.gz app
          cd ../../..

      - name: Package (Windows)
        if: matrix.archive == 'zip'
        run: |
          cd target/${{ matrix.target }}/release
          7z a ../../../app-${{ matrix.target }}.zip app.exe
          cd ../../..

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: app-${{ matrix.target }}
          path: app-${{ matrix.target }}.${{ matrix.archive }}

  release:
    name: Create Release
    needs: [check-version, build]
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Update Cargo.toml and create tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          sed -i 's/^version = ".*"/version = "${{ needs.check-version.outputs.new_version }}"/' Cargo.toml
          git add Cargo.toml
          git commit -m "chore: release v${{ needs.check-version.outputs.new_version }} [skip ci]"
          git tag "v${{ needs.check-version.outputs.new_version }}"
          git push origin main --tags

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Generate changelog
        id: changelog
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -z "$LATEST_TAG" ]; then
            COMMITS=$(git log --pretty=format:"- %s" HEAD)
          else
            COMMITS=$(git log --pretty=format:"- %s" ${LATEST_TAG}..HEAD^)
          fi
          echo "CHANGELOG<<EOF" >> $GITHUB_OUTPUT
          echo "$COMMITS" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ needs.check-version.outputs.new_version }}
          name: v${{ needs.check-version.outputs.new_version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.CHANGELOG }}
          files: artifacts/**/*
"#;

const GO_TEMPLATE: &str = r#"name: Auto Release

on:
  push:
    branches: [main]
    paths-ignore:
      - '*.md'

permissions:
  contents: write

jobs:
  check-version:
    runs-on: ubuntu-latest
    outputs:
      new_version: ${{ steps.bump.outputs.new_version }}
      should_release: ${{ steps.bump.outputs.should_release }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Determine version bump
        id: bump
        run: |
          # Get latest tag
          LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
          CURRENT_VERSION=${LATEST_TAG#v}

          echo "Current version: $CURRENT_VERSION"

          # Get commits since last tag
          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          if [ -z "$COMMITS" ]; then
            echo "No new commits"
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          # Determine bump type from conventional commits
          BUMP="none"

          if echo "$COMMITS" | grep -qiE "^BREAKING CHANGE:|^[a-z]+(\(.+\))?!:"; then
            BUMP="major"
          elif echo "$COMMITS" | grep -qiE "^feat(\(.+\))?:"; then
            BUMP="minor"
          elif echo "$COMMITS" | grep -qiE "^fix(\(.+\))?:|^perf(\(.+\))?:|^refactor(\(.+\))?:"; then
            BUMP="patch"
          fi

          if [ "$BUMP" = "none" ]; then
            echo "No version bump needed"
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          # Parse and apply bump
          IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
          case $BUMP in
            major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
            minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
            patch) PATCH=$((PATCH + 1)) ;;
          esac

          NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
          echo "new_version=$NEW_VERSION" >> $GITHUB_OUTPUT
          echo "should_release=true" >> $GITHUB_OUTPUT

  build:
    name: Build ${{ matrix.goos }}-${{ matrix.goarch }}
    needs: check-version
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    strategy:
      matrix:
        include:
          - goos: linux
            goarch: amd64
          - goos: linux
            goarch: arm64
          - goos: darwin
            goarch: amd64
          - goos: darwin
            goarch: arm64
          - goos: windows
            goarch: amd64
    steps:
      - uses: actions/checkout@v4

      - name: Set up Go
        uses: actions/setup-go@v5
        with:
          go-version: 'stable'

      - name: Build
        env:
          GOOS: ${{ matrix.goos }}
          GOARCH: ${{ matrix.goarch }}
        run: |
          mkdir -p dist
          EXT=""
          if [ "$GOOS" = "windows" ]; then EXT=".exe"; fi
          go build -ldflags "-X main.Version=${{ needs.check-version.outputs.new_version }}" -o dist/app-${{ matrix.goos }}-${{ matrix.goarch }}${EXT} ./...

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: app-${{ matrix.goos }}-${{ matrix.goarch }}
          path: dist/*

  release:
    name: Create Release
    needs: [check-version, build]
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Create tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git tag "v${{ needs.check-version.outputs.new_version }}"
          git push origin "v${{ needs.check-version.outputs.new_version }}"

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Generate changelog
        id: changelog
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -z "$LATEST_TAG" ]; then
            COMMITS=$(git log --pretty=format:"- %s" HEAD)
          else
            COMMITS=$(git log --pretty=format:"- %s" ${LATEST_TAG}..HEAD)
          fi
          echo "CHANGELOG<<EOF" >> $GITHUB_OUTPUT
          echo "$COMMITS" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ needs.check-version.outputs.new_version }}
          name: v${{ needs.check-version.outputs.new_version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.CHANGELOG }}
          files: artifacts/**/*
"#;

const BUN_TEMPLATE: &str = r#"name: Auto Release

on:
  push:
    branches: [main]
    paths-ignore:
      - '*.md'

permissions:
  contents: write

jobs:
  check-version:
    runs-on: ubuntu-latest
    outputs:
      new_version: ${{ steps.bump.outputs.new_version }}
      should_release: ${{ steps.bump.outputs.should_release }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Determine version bump
        id: bump
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
          CURRENT_VERSION=${LATEST_TAG#v}

          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          if [ -z "$COMMITS" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          BUMP="none"
          if echo "$COMMITS" | grep -qiE "^BREAKING CHANGE:|^[a-z]+(\(.+\))?!:"; then
            BUMP="major"
          elif echo "$COMMITS" | grep -qiE "^feat(\(.+\))?:"; then
            BUMP="minor"
          elif echo "$COMMITS" | grep -qiE "^fix(\(.+\))?:|^perf(\(.+\))?:|^refactor(\(.+\))?:"; then
            BUMP="patch"
          fi

          if [ "$BUMP" = "none" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
          case $BUMP in
            major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
            minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
            patch) PATCH=$((PATCH + 1)) ;;
          esac

          echo "new_version=${MAJOR}.${MINOR}.${PATCH}" >> $GITHUB_OUTPUT
          echo "should_release=true" >> $GITHUB_OUTPUT

  build:
    needs: check-version
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Bun
        uses: oven-sh/setup-bun@v1

      - name: Install dependencies
        run: bun install

      - name: Build
        run: bun run build

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: build
          path: dist/

  release:
    name: Create Release
    needs: [check-version, build]
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Update package.json and create tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          npm version ${{ needs.check-version.outputs.new_version }} --no-git-tag-version
          git add package.json
          git commit -m "chore: release v${{ needs.check-version.outputs.new_version }} [skip ci]"
          git tag "v${{ needs.check-version.outputs.new_version }}"
          git push origin main --tags

      - name: Download artifact
        uses: actions/download-artifact@v4
        with:
          name: build
          path: dist

      - name: Generate changelog
        id: changelog
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -z "$LATEST_TAG" ]; then
            COMMITS=$(git log --pretty=format:"- %s" HEAD)
          else
            COMMITS=$(git log --pretty=format:"- %s" ${LATEST_TAG}..HEAD^)
          fi
          echo "CHANGELOG<<EOF" >> $GITHUB_OUTPUT
          echo "$COMMITS" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ needs.check-version.outputs.new_version }}
          name: v${{ needs.check-version.outputs.new_version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.CHANGELOG }}
"#;

const PNPM_TEMPLATE: &str = r#"name: Auto Release

on:
  push:
    branches: [main]
    paths-ignore:
      - '*.md'

permissions:
  contents: write

jobs:
  check-version:
    runs-on: ubuntu-latest
    outputs:
      new_version: ${{ steps.bump.outputs.new_version }}
      should_release: ${{ steps.bump.outputs.should_release }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Determine version bump
        id: bump
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
          CURRENT_VERSION=${LATEST_TAG#v}

          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          if [ -z "$COMMITS" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          BUMP="none"
          if echo "$COMMITS" | grep -qiE "^BREAKING CHANGE:|^[a-z]+(\(.+\))?!:"; then
            BUMP="major"
          elif echo "$COMMITS" | grep -qiE "^feat(\(.+\))?:"; then
            BUMP="minor"
          elif echo "$COMMITS" | grep -qiE "^fix(\(.+\))?:|^perf(\(.+\))?:|^refactor(\(.+\))?:"; then
            BUMP="patch"
          fi

          if [ "$BUMP" = "none" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
          case $BUMP in
            major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
            minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
            patch) PATCH=$((PATCH + 1)) ;;
          esac

          echo "new_version=${MAJOR}.${MINOR}.${PATCH}" >> $GITHUB_OUTPUT
          echo "should_release=true" >> $GITHUB_OUTPUT

  build:
    needs: check-version
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup pnpm
        uses: pnpm/action-setup@v2
        with:
          version: latest

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 'lts/*'
          cache: 'pnpm'

      - name: Install dependencies
        run: pnpm install --frozen-lockfile

      - name: Build
        run: pnpm build

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: build
          path: dist/

  release:
    name: Create Release
    needs: [check-version, build]
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Update package.json and create tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          npm version ${{ needs.check-version.outputs.new_version }} --no-git-tag-version
          git add package.json
          git commit -m "chore: release v${{ needs.check-version.outputs.new_version }} [skip ci]"
          git tag "v${{ needs.check-version.outputs.new_version }}"
          git push origin main --tags

      - name: Generate changelog
        id: changelog
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -z "$LATEST_TAG" ]; then
            COMMITS=$(git log --pretty=format:"- %s" HEAD)
          else
            COMMITS=$(git log --pretty=format:"- %s" ${LATEST_TAG}..HEAD^)
          fi
          echo "CHANGELOG<<EOF" >> $GITHUB_OUTPUT
          echo "$COMMITS" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ needs.check-version.outputs.new_version }}
          name: v${{ needs.check-version.outputs.new_version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.CHANGELOG }}
"#;

const NEXTJS_TEMPLATE: &str = r#"name: Auto Release

on:
  push:
    branches: [main]
    paths-ignore:
      - '*.md'

permissions:
  contents: write

jobs:
  check-version:
    runs-on: ubuntu-latest
    outputs:
      new_version: ${{ steps.bump.outputs.new_version }}
      should_release: ${{ steps.bump.outputs.should_release }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Determine version bump
        id: bump
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
          CURRENT_VERSION=${LATEST_TAG#v}

          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          if [ -z "$COMMITS" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          BUMP="none"
          if echo "$COMMITS" | grep -qiE "^BREAKING CHANGE:|^[a-z]+(\(.+\))?!:"; then
            BUMP="major"
          elif echo "$COMMITS" | grep -qiE "^feat(\(.+\))?:"; then
            BUMP="minor"
          elif echo "$COMMITS" | grep -qiE "^fix(\(.+\))?:|^perf(\(.+\))?:|^refactor(\(.+\))?:"; then
            BUMP="patch"
          fi

          if [ "$BUMP" = "none" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
          case $BUMP in
            major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
            minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
            patch) PATCH=$((PATCH + 1)) ;;
          esac

          echo "new_version=${MAJOR}.${MINOR}.${PATCH}" >> $GITHUB_OUTPUT
          echo "should_release=true" >> $GITHUB_OUTPUT

  build:
    needs: check-version
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Detect package manager
        id: pm
        run: |
          if [ -f "bun.lockb" ]; then
            echo "manager=bun" >> $GITHUB_OUTPUT
          elif [ -f "pnpm-lock.yaml" ]; then
            echo "manager=pnpm" >> $GITHUB_OUTPUT
          else
            echo "manager=npm" >> $GITHUB_OUTPUT
          fi

      - name: Setup Bun
        if: steps.pm.outputs.manager == 'bun'
        uses: oven-sh/setup-bun@v1

      - name: Setup pnpm
        if: steps.pm.outputs.manager == 'pnpm'
        uses: pnpm/action-setup@v2
        with:
          version: latest

      - name: Setup Node.js
        if: steps.pm.outputs.manager != 'bun'
        uses: actions/setup-node@v4
        with:
          node-version: 'lts/*'
          cache: ${{ steps.pm.outputs.manager }}

      - name: Install dependencies
        run: |
          case "${{ steps.pm.outputs.manager }}" in
            bun) bun install ;;
            pnpm) pnpm install --frozen-lockfile ;;
            npm) npm ci ;;
          esac

      - name: Build
        run: |
          case "${{ steps.pm.outputs.manager }}" in
            bun) bun run build ;;
            pnpm) pnpm build ;;
            npm) npm run build ;;
          esac

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: build
          path: |
            .next/
            out/

  release:
    name: Create Release
    needs: [check-version, build]
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Update package.json and create tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          npm version ${{ needs.check-version.outputs.new_version }} --no-git-tag-version
          git add package.json
          git commit -m "chore: release v${{ needs.check-version.outputs.new_version }} [skip ci]"
          git tag "v${{ needs.check-version.outputs.new_version }}"
          git push origin main --tags

      - name: Generate changelog
        id: changelog
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -z "$LATEST_TAG" ]; then
            COMMITS=$(git log --pretty=format:"- %s" HEAD)
          else
            COMMITS=$(git log --pretty=format:"- %s" ${LATEST_TAG}..HEAD^)
          fi
          echo "CHANGELOG<<EOF" >> $GITHUB_OUTPUT
          echo "$COMMITS" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ needs.check-version.outputs.new_version }}
          name: v${{ needs.check-version.outputs.new_version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.CHANGELOG }}
"#;

const NODEJS_TEMPLATE: &str = r#"name: Auto Release

on:
  push:
    branches: [main]
    paths-ignore:
      - '*.md'

permissions:
  contents: write

jobs:
  check-version:
    runs-on: ubuntu-latest
    outputs:
      new_version: ${{ steps.bump.outputs.new_version }}
      should_release: ${{ steps.bump.outputs.should_release }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Determine version bump
        id: bump
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
          CURRENT_VERSION=${LATEST_TAG#v}

          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          if [ -z "$COMMITS" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          BUMP="none"
          if echo "$COMMITS" | grep -qiE "^BREAKING CHANGE:|^[a-z]+(\(.+\))?!:"; then
            BUMP="major"
          elif echo "$COMMITS" | grep -qiE "^feat(\(.+\))?:"; then
            BUMP="minor"
          elif echo "$COMMITS" | grep -qiE "^fix(\(.+\))?:|^perf(\(.+\))?:|^refactor(\(.+\))?:"; then
            BUMP="patch"
          fi

          if [ "$BUMP" = "none" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
          case $BUMP in
            major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
            minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
            patch) PATCH=$((PATCH + 1)) ;;
          esac

          echo "new_version=${MAJOR}.${MINOR}.${PATCH}" >> $GITHUB_OUTPUT
          echo "should_release=true" >> $GITHUB_OUTPUT

  build:
    needs: check-version
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 'lts/*'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Build
        run: npm run build

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: build
          path: dist/

  release:
    name: Create Release
    needs: [check-version, build]
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Update package.json and create tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          npm version ${{ needs.check-version.outputs.new_version }} --no-git-tag-version
          git add package.json
          git commit -m "chore: release v${{ needs.check-version.outputs.new_version }} [skip ci]"
          git tag "v${{ needs.check-version.outputs.new_version }}"
          git push origin main --tags

      - name: Generate changelog
        id: changelog
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -z "$LATEST_TAG" ]; then
            COMMITS=$(git log --pretty=format:"- %s" HEAD)
          else
            COMMITS=$(git log --pretty=format:"- %s" ${LATEST_TAG}..HEAD^)
          fi
          echo "CHANGELOG<<EOF" >> $GITHUB_OUTPUT
          echo "$COMMITS" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ needs.check-version.outputs.new_version }}
          name: v${{ needs.check-version.outputs.new_version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.CHANGELOG }}
"#;

const REACT_NATIVE_TEMPLATE: &str = r#"name: Auto Release

on:
  push:
    branches: [main]
    paths-ignore:
      - '*.md'

permissions:
  contents: write

jobs:
  check-version:
    runs-on: ubuntu-latest
    outputs:
      new_version: ${{ steps.bump.outputs.new_version }}
      should_release: ${{ steps.bump.outputs.should_release }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Determine version bump
        id: bump
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
          CURRENT_VERSION=${LATEST_TAG#v}

          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          if [ -z "$COMMITS" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          BUMP="none"
          if echo "$COMMITS" | grep -qiE "^BREAKING CHANGE:|^[a-z]+(\(.+\))?!:"; then
            BUMP="major"
          elif echo "$COMMITS" | grep -qiE "^feat(\(.+\))?:"; then
            BUMP="minor"
          elif echo "$COMMITS" | grep -qiE "^fix(\(.+\))?:|^perf(\(.+\))?:|^refactor(\(.+\))?:"; then
            BUMP="patch"
          fi

          if [ "$BUMP" = "none" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
          case $BUMP in
            major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
            minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
            patch) PATCH=$((PATCH + 1)) ;;
          esac

          echo "new_version=${MAJOR}.${MINOR}.${PATCH}" >> $GITHUB_OUTPUT
          echo "should_release=true" >> $GITHUB_OUTPUT

  build:
    needs: check-version
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 'lts/*'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Run tests
        run: npm test -- --passWithNoTests

      # Note: For actual builds, you'll need to set up EAS or native build tools
      # This workflow creates a release tag for tracking purposes
      # Configure EAS Build or Fastlane separately for app store builds

  release:
    name: Create Release
    needs: [check-version, build]
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Update package.json and create tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          npm version ${{ needs.check-version.outputs.new_version }} --no-git-tag-version
          git add package.json
          git commit -m "chore: release v${{ needs.check-version.outputs.new_version }} [skip ci]"
          git tag "v${{ needs.check-version.outputs.new_version }}"
          git push origin main --tags

      - name: Generate changelog
        id: changelog
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -z "$LATEST_TAG" ]; then
            COMMITS=$(git log --pretty=format:"- %s" HEAD)
          else
            COMMITS=$(git log --pretty=format:"- %s" ${LATEST_TAG}..HEAD^)
          fi
          echo "CHANGELOG<<EOF" >> $GITHUB_OUTPUT
          echo "$COMMITS" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ needs.check-version.outputs.new_version }}
          name: v${{ needs.check-version.outputs.new_version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.CHANGELOG }}

            ## Build Instructions
            To build native apps, use EAS Build or run locally:
            - iOS: `npx react-native run-ios --mode Release`
            - Android: `npx react-native run-android --mode release`
"#;

const XCODE_TEMPLATE: &str = r#"name: Auto Release

on:
  push:
    branches: [main]
    paths-ignore:
      - '*.md'

permissions:
  contents: write

jobs:
  check-version:
    runs-on: ubuntu-latest
    outputs:
      new_version: ${{ steps.bump.outputs.new_version }}
      should_release: ${{ steps.bump.outputs.should_release }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Determine version bump
        id: bump
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
          CURRENT_VERSION=${LATEST_TAG#v}

          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          if [ -z "$COMMITS" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          BUMP="none"
          if echo "$COMMITS" | grep -qiE "^BREAKING CHANGE:|^[a-z]+(\(.+\))?!:"; then
            BUMP="major"
          elif echo "$COMMITS" | grep -qiE "^feat(\(.+\))?:"; then
            BUMP="minor"
          elif echo "$COMMITS" | grep -qiE "^fix(\(.+\))?:|^perf(\(.+\))?:|^refactor(\(.+\))?:"; then
            BUMP="patch"
          fi

          if [ "$BUMP" = "none" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
          case $BUMP in
            major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
            minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
            patch) PATCH=$((PATCH + 1)) ;;
          esac

          echo "new_version=${MAJOR}.${MINOR}.${PATCH}" >> $GITHUB_OUTPUT
          echo "should_release=true" >> $GITHUB_OUTPUT

  build:
    needs: check-version
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Select Xcode
        run: sudo xcode-select -s /Applications/Xcode.app

      - name: Build
        run: |
          # Find the project or workspace
          if ls *.xcworkspace 1> /dev/null 2>&1; then
            WORKSPACE=$(ls *.xcworkspace | head -1)
            xcodebuild -workspace "$WORKSPACE" -scheme "${WORKSPACE%.xcworkspace}" -configuration Release build CODE_SIGN_IDENTITY="" CODE_SIGNING_REQUIRED=NO
          elif ls *.xcodeproj 1> /dev/null 2>&1; then
            PROJECT=$(ls *.xcodeproj | head -1)
            xcodebuild -project "$PROJECT" -scheme "${PROJECT%.xcodeproj}" -configuration Release build CODE_SIGN_IDENTITY="" CODE_SIGNING_REQUIRED=NO
          else
            echo "No Xcode project found"
            exit 1
          fi

  release:
    name: Create Release
    needs: [check-version, build]
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Create tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git tag "v${{ needs.check-version.outputs.new_version }}"
          git push origin "v${{ needs.check-version.outputs.new_version }}"

      - name: Generate changelog
        id: changelog
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -z "$LATEST_TAG" ]; then
            COMMITS=$(git log --pretty=format:"- %s" HEAD)
          else
            COMMITS=$(git log --pretty=format:"- %s" ${LATEST_TAG}..HEAD)
          fi
          echo "CHANGELOG<<EOF" >> $GITHUB_OUTPUT
          echo "$COMMITS" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ needs.check-version.outputs.new_version }}
          name: v${{ needs.check-version.outputs.new_version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.CHANGELOG }}

            ## Build Instructions
            For signed builds, configure code signing and archive locally or via Xcode Cloud.
"#;

const PYTHON_TEMPLATE: &str = r#"name: Auto Release

on:
  push:
    branches: [main]
    paths-ignore:
      - '*.md'

permissions:
  contents: write

jobs:
  check-version:
    runs-on: ubuntu-latest
    outputs:
      new_version: ${{ steps.bump.outputs.new_version }}
      should_release: ${{ steps.bump.outputs.should_release }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Determine version bump
        id: bump
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
          CURRENT_VERSION=${LATEST_TAG#v}

          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          if [ -z "$COMMITS" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          BUMP="none"
          if echo "$COMMITS" | grep -qiE "^BREAKING CHANGE:|^[a-z]+(\(.+\))?!:"; then
            BUMP="major"
          elif echo "$COMMITS" | grep -qiE "^feat(\(.+\))?:"; then
            BUMP="minor"
          elif echo "$COMMITS" | grep -qiE "^fix(\(.+\))?:|^perf(\(.+\))?:|^refactor(\(.+\))?:"; then
            BUMP="patch"
          fi

          if [ "$BUMP" = "none" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
          case $BUMP in
            major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
            minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
            patch) PATCH=$((PATCH + 1)) ;;
          esac

          echo "new_version=${MAJOR}.${MINOR}.${PATCH}" >> $GITHUB_OUTPUT
          echo "should_release=true" >> $GITHUB_OUTPUT

  build:
    needs: check-version
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.x'

      - name: Install build tools
        run: pip install build

      - name: Update version
        run: |
          VERSION="${{ needs.check-version.outputs.new_version }}"
          if [ -f "pyproject.toml" ]; then
            sed -i "s/^version = .*/version = \"$VERSION\"/" pyproject.toml
          elif [ -f "setup.py" ]; then
            sed -i "s/version=.*/version=\"$VERSION\",/" setup.py
          fi

      - name: Build package
        run: python -m build

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: dist
          path: dist/

  release:
    name: Create Release
    needs: [check-version, build]
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Update version and create tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          VERSION="${{ needs.check-version.outputs.new_version }}"
          if [ -f "pyproject.toml" ]; then
            sed -i "s/^version = .*/version = \"$VERSION\"/" pyproject.toml
            git add pyproject.toml
          elif [ -f "setup.py" ]; then
            sed -i "s/version=.*/version=\"$VERSION\",/" setup.py
            git add setup.py
          fi
          git commit -m "chore: release v$VERSION [skip ci]" || true
          git tag "v$VERSION"
          git push origin main --tags

      - name: Download artifact
        uses: actions/download-artifact@v4
        with:
          name: dist
          path: dist

      - name: Generate changelog
        id: changelog
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -z "$LATEST_TAG" ]; then
            COMMITS=$(git log --pretty=format:"- %s" HEAD)
          else
            COMMITS=$(git log --pretty=format:"- %s" ${LATEST_TAG}..HEAD^)
          fi
          echo "CHANGELOG<<EOF" >> $GITHUB_OUTPUT
          echo "$COMMITS" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ needs.check-version.outputs.new_version }}
          name: v${{ needs.check-version.outputs.new_version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.CHANGELOG }}
          files: dist/*
"#;

const GENERIC_TEMPLATE: &str = r#"name: Auto Release

on:
  push:
    branches: [main]
    paths-ignore:
      - '*.md'

permissions:
  contents: write

jobs:
  check-version:
    runs-on: ubuntu-latest
    outputs:
      new_version: ${{ steps.bump.outputs.new_version }}
      should_release: ${{ steps.bump.outputs.should_release }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Determine version bump
        id: bump
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
          CURRENT_VERSION=${LATEST_TAG#v}

          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          if [ -z "$COMMITS" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          BUMP="none"
          if echo "$COMMITS" | grep -qiE "^BREAKING CHANGE:|^[a-z]+(\(.+\))?!:"; then
            BUMP="major"
          elif echo "$COMMITS" | grep -qiE "^feat(\(.+\))?:"; then
            BUMP="minor"
          elif echo "$COMMITS" | grep -qiE "^fix(\(.+\))?:|^perf(\(.+\))?:|^refactor(\(.+\))?:"; then
            BUMP="patch"
          fi

          if [ "$BUMP" = "none" ]; then
            echo "should_release=false" >> $GITHUB_OUTPUT
            exit 0
          fi

          IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
          case $BUMP in
            major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
            minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
            patch) PATCH=$((PATCH + 1)) ;;
          esac

          echo "new_version=${MAJOR}.${MINOR}.${PATCH}" >> $GITHUB_OUTPUT
          echo "should_release=true" >> $GITHUB_OUTPUT

  release:
    name: Create Release
    needs: check-version
    if: needs.check-version.outputs.should_release == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Create tag
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git tag "v${{ needs.check-version.outputs.new_version }}"
          git push origin "v${{ needs.check-version.outputs.new_version }}"

      - name: Generate changelog
        id: changelog
        run: |
          LATEST_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
          if [ -z "$LATEST_TAG" ]; then
            COMMITS=$(git log --pretty=format:"- %s" HEAD)
          else
            COMMITS=$(git log --pretty=format:"- %s" ${LATEST_TAG}..HEAD)
          fi
          echo "CHANGELOG<<EOF" >> $GITHUB_OUTPUT
          echo "$COMMITS" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ needs.check-version.outputs.new_version }}
          name: v${{ needs.check-version.outputs.new_version }}
          body: |
            ## Changes
            ${{ steps.changelog.outputs.CHANGELOG }}
"#;
