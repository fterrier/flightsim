#!/usr/bin/env bash
# Keeps ARCHITECTURE.md up to date: every public or crate-visible struct,
# enum and trait (outside test code) must be mentioned there. Run by CI and
# before every commit (see AGENTS.md). It only checks that each type is
# *named* on the page; whether the diagrams show it in the right place is
# for the author and the reviewer.
set -euo pipefail

cd "$(dirname "$0")/.."
missing=0
for file in $(git ls-files 'crates/*/src/*.rs'); do
    # The code before the file's first `#[cfg(test)]`: test-only types
    # (fakes, test games) don't count.
    names=$(awk '/#\[cfg\(test\)\]/ { exit } { print }' "$file" |
        sed -nE 's/^pub(\(crate\))? (struct|enum|trait) ([A-Za-z0-9_]+).*/\3/p')
    for name in $names; do
        if ! grep -qw "$name" ARCHITECTURE.md; then
            echo "ARCHITECTURE.md doesn't mention $name (from $file)"
            missing=1
        fi
    done
done
if [ "$missing" -eq 0 ]; then
    echo "ARCHITECTURE.md mentions every public and crate-visible type."
fi
exit "$missing"
