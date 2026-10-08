/** @type {import('semantic-release').GlobalConfig} */
export default {
  // biome-ignore lint/suspicious/noTemplateCurlyInString: semantic-release template
  tagFormat: "v${version}",
  branches: ["main"],
  plugins: [
    [
      "@semantic-release/commit-analyzer",
      {
        // Breaking changes only bump the minor version, so 1.0.0 is a decision and not an accident.
        releaseRules: [
          { breaking: true, release: "minor" },
          { type: "feat", release: "minor" },
          { type: "fix", release: "patch" },
          { type: "perf", release: "patch" },
          { type: "refactor", release: "patch" },
        ],
      },
    ],
    "@semantic-release/release-notes-generator",
    ["@semantic-release/changelog", { changelogFile: "CHANGELOG.md" }],
    [
      "@semantic-release/exec",
      {
        // The first version line of Cargo.toml is the one of the workspace.
        prepareCmd:
          // biome-ignore lint/suspicious/noTemplateCurlyInString: semantic-release template
          "sed -i '0,/^version = \".*\"/s//version = \"${nextRelease.version}\"/' Cargo.toml && cargo update --workspace",
      },
    ],
    [
      "@semantic-release/git",
      {
        assets: ["CHANGELOG.md", "Cargo.toml", "Cargo.lock"],
        message:
          // biome-ignore lint/suspicious/noTemplateCurlyInString: semantic-release template
          "chore(release): ${nextRelease.version} [skip ci]\n\n${nextRelease.notes}",
      },
    ],
    "@semantic-release/github",
  ],
};
