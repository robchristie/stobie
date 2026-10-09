#!/usr/bin/env bash
set -euo pipefail

# Mutate only a disposable GitHub-hosted runner, never a developer's rustup home.
[[ "${GITHUB_ACTIONS:-}" == true && "${RUNNER_ENVIRONMENT:-}" == github-hosted ]] || {
  echo 'Toolchain normalisation requires a GitHub-hosted Actions runner.' >&2
  exit 1
}
expected_version="$(python3 -c 'import tomllib; print(tomllib.load(open("rust-toolchain.toml", "rb"))["toolchain"]["channel"])')"
active_toolchain="$(rustup show active-toolchain)"
active_toolchain="${active_toolchain%% *}"
[[ "$active_toolchain" == "$expected_version-"* && "$(rustc --version)" == "rustc $expected_version "* ]] || {
  echo 'Active compiler does not match the repository toolchain.' >&2
  exit 1
}
while read -r installed_toolchain remainder; do
  [[ -n "$installed_toolchain" ]] || exit 1
  if [[ "$installed_toolchain" != "$active_toolchain" ]]; then
    rustup toolchain uninstall -- "$installed_toolchain"
  fi
done < <(rustup toolchain list)
remaining="$(rustup toolchain list)"
[[ "$remaining" != *$'\n'* && "${remaining%% *}" == "$active_toolchain" ]] || {
  echo 'Unexpected installed toolchains remain.' >&2
  exit 1
}
