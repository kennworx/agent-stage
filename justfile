# cargo-crap file excludes. Globs match the workspace-relative paths in the lcov report:
# `**/tests/**` skips each crate's integration tests; the rest skip in-src test-only
# modules extracted to their own files (`mod tests;` children, `tests_*` splits, and
# shared fixtures in `testkit.rs`) — the same test code the inline `mod tests {}` form
# never gated. `**/examples/**` goes with them: `cargo llvm-cov` does not instrument
# example targets at all, so every function in one reports null coverage and would be
# gated on complexity alone — which is a hole in the measurement, not debt in the code.
crap_excludes := "--exclude '**/tests/**' --exclude '**/tests.rs' --exclude '**/tests_*.rs' --exclude '**/examples/**'"

# Coverage runs with `--all-features`. A feature-gated module is not compiled in the
# default build, so llvm-cov emits no data for it and cargo-crap sees its functions
# with null coverage — gating them on complexity alone. That is a hole in the
# measurement, not debt in the code, and it is the same hole the `examples` exclude
# below documents. Measuring the code costs one extra compile; not measuring it costs
# a gate that cannot see what it is guarding.

# CRAP threshold read straight from `.cargo-crap.toml` (the same source cargo-crap uses), so the
# gate predicate can't drift from the report. Falls back to cargo-crap's built-in default (30).
crap_threshold := shell("v=$(grep -E '^threshold[[:space:]]*=' .cargo-crap.toml 2>/dev/null | sed -E 's/[^0-9]+//g'); echo ${v:-30}")

# Per-function line-coverage floor (percent), matching CLAUDE.md's ≥90% policy.
cov_floor := "90"

# Functions that were already under the floor when the gate was given a memory.
#
# The floor used to be a flat predicate: anything under it failed, so a repository
# carrying inherited debt could never be green and the gate said the same thing on
# every run whether or not the change in front of it was at fault. That is the
# problem `.cargo-crap.json` already solved for complexity, so the floor now works
# the same way — a function under the floor fails when it is *new*, or when it has
# got *worse* than the baseline records. Raising one and forgetting to refresh the
# baseline is reported, not failed: an improvement should never be an error.
cov_baseline := ".coverage-baseline.json"

# Run the ags CLI.
run *args:
  @cargo run -p ags --quiet -- {{ args }}

# Serve the example artifact for review (MVP demo); uses installed `ags`, ctrl-c to stop.
demo:
  #!/usr/bin/env bash
  set -euo pipefail
  if command -v ags >/dev/null 2>&1; then
    echo "demo: using installed ags -> $(command -v ags)"
    exec ags present examples/reasoning-demo.md
  fi
  echo "demo: ags not installed (run 'just install'); building into the shared ~/.cargo/target — watch for a 'Blocking waiting for file lock' line if another build holds it…"
  exec cargo run -p ags -- present examples/reasoning-demo.md

# Install the release binary via `cargo install`. `--root` puts it in <root>/bin.
install root=(env('HOME') / '.local'):
  cargo install --path crates/ags --locked --force --root "{{ root }}"
  @"{{ root }}/bin/ags" --version

# Clippy the whole workspace with the pedantic lints
clippy:
  @cargo clippy --workspace --all-targets

# Generate a cargo-crap CRAP report (cyclomatic complexity x test coverage).
crap:
  #!/usr/bin/env bash
  set -euo pipefail
  mkdir -p tmp
  # Homebrew's rustc ships no llvm-tools component; if a standalone
  # llvm-profdata is on PATH, point cargo-llvm-cov at it.
  if command -v llvm-profdata >/dev/null 2>&1; then
    export LLVM_COV="$(command -v llvm-cov)"
    export LLVM_PROFDATA="$(command -v llvm-profdata)"
  fi
  cargo llvm-cov --workspace --all-features --lcov --output-path tmp/lcov.info
  cargo crap --lcov tmp/lcov.info {{ crap_excludes }}

# CI gate: workspace coverage + CRAP against the committed baseline.
crap-ci:
  #!/usr/bin/env bash
  set -euo pipefail
  mkdir -p tmp
  if command -v llvm-profdata >/dev/null 2>&1; then
    export LLVM_COV="$(command -v llvm-cov)"
    export LLVM_PROFDATA="$(command -v llvm-profdata)"
  fi
  # Reuse an already-collected tmp/lcov.info when REUSE_LCOV is set (see `gate`); else
  # run the instrumented suite ourselves.
  if [ -z "${REUSE_LCOV:-}" ]; then
    cargo llvm-cov --workspace --all-features --lcov --output-path tmp/lcov.info
  fi
  cargo crap --lcov tmp/lcov.info {{ crap_excludes }} \
    --baseline .cargo-crap.json --format json --output tmp/crap-delta.json
  # Predicate: any "regressed" entry, OR any "new" entry whose CRAP
  # exceeds the threshold.
  BAD=$(jq --argjson t {{ crap_threshold }} \
    '[.entries[] | select(.status == "regressed" or (.status == "new" and .crap > $t))] | length' \
    tmp/crap-delta.json)
  if [ "$BAD" -gt 0 ]; then
    echo "CRAP gate FAILED: $BAD offending entries (regressed or new-over-threshold)"
    jq --argjson t {{ crap_threshold }} \
      '.entries | map(select(.status == "regressed" or (.status == "new" and .crap > $t))) | sort_by(-.crap) | .[] | {status, function, file, line, crap, cyclomatic, coverage}' \
      tmp/crap-delta.json
    exit 1
  fi
  echo "CRAP gate PASSED: no regressions, no new over-threshold functions"

# Run both coverage gates (CRAP + 90% floor) off one llvm-cov run.
gate:
  #!/usr/bin/env bash
  set -euo pipefail
  mkdir -p tmp
  if command -v llvm-profdata >/dev/null 2>&1; then
    export LLVM_COV="$(command -v llvm-cov)"
    export LLVM_PROFDATA="$(command -v llvm-profdata)"
  fi
  # Collect coverage once; the `report` calls in the gates below reuse this profile data.
  cargo llvm-cov --workspace --all-features --no-report
  cargo llvm-cov report --lcov --output-path tmp/lcov.info
  REUSE_LCOV=1 just crap-ci
  REUSE_LCOV=1 just coverage
  just large-files

# Regenerate the committed CRAP baseline (`.cargo-crap.json`).
crap-baseline:
  #!/usr/bin/env bash
  set -euo pipefail
  mkdir -p tmp
  if command -v llvm-profdata >/dev/null 2>&1; then
    export LLVM_COV="$(command -v llvm-cov)"
    export LLVM_PROFDATA="$(command -v llvm-profdata)"
  fi
  cargo llvm-cov --workspace --all-features --lcov --output-path tmp/lcov.info
  cargo crap --lcov tmp/lcov.info {{ crap_excludes }} \
    --format json --output tmp/crap-full.json
  WS_ROOT="$(pwd)"
  jq --argjson t {{ crap_threshold }} --arg ws "$WS_ROOT/" \
    '.entries |= (map(select(.crap > $t)) | map(.file |= sub("^" + $ws; "")))' \
    tmp/crap-full.json > .cargo-crap.json
  KEPT=$(jq '.entries | length' .cargo-crap.json)
  echo "Wrote .cargo-crap.json with $KEPT over-threshold entries"

# Per-file + per-function coverage histogram; exits non-zero if any function is below `cov_floor`.
coverage:
  #!/usr/bin/env bash
  set -euo pipefail
  # NOTE: cargo-crap's `coverage` column is a PERCENTAGE (0-100), not a fraction — the floor
  # predicate is `.coverage < {{ cov_floor }}`, NOT `< 0.x` (which tests a sub-percent and passes everything).
  mkdir -p tmp
  if command -v llvm-profdata >/dev/null 2>&1; then
    export LLVM_COV="$(command -v llvm-cov)"
    export LLVM_PROFDATA="$(command -v llvm-profdata)"
  fi
  # Reuse an already-collected tmp/lcov.info + profile when REUSE_LCOV is set (see `gate`);
  # else run the instrumented suite ourselves. The `report` below reuses the profile either way.
  if [ -z "${REUSE_LCOV:-}" ]; then
    cargo llvm-cov --workspace --all-features --lcov --output-path tmp/lcov.info >/dev/null
  fi
  cargo crap --lcov tmp/lcov.info {{ crap_excludes }} --format json --output tmp/crap-now.json >/dev/null
  echo "PER-FILE LINE COVERAGE (source .rs files, excl. tests)"
  echo "─────────────────────────────────────────────────────────────────────"
  cargo llvm-cov report --summary-only --ignore-filename-regex 'tests/' \
  | awk '$1 ~ /\.rs$/ { cov=$10; gsub(/%/,"",cov); b=int(cov/2.5); s=""; for(i=0;i<b;i++)s=s"█"; printf "  %-25s %6.2f  %s\n",$1,cov,s }'
  echo ""
  echo "PER-FUNCTION COVERAGE DISTRIBUTION (floor = {{ cov_floor }}%)"
  echo "─────────────────────────────────────────────────────────────────"
  # Bar = bucket's share of all functions, scaled to a 50-wide axis: round(50 * count/total).
  jq -r '
     def lpad($w): tostring as $s | ($w - ($s|length)) as $p | (if $p>0 then " "*$p else "" end) + $s;
     [.entries[].coverage] as $c | ($c|length) as $tot | [
       ["  <60%",([$c[]|select(.<60)]|length)],
       ["60-69%",([$c[]|select(.>=60 and .<70)]|length)],
       ["70-79%",([$c[]|select(.>=70 and .<80)]|length)],
       ["80-89%",([$c[]|select(.>=80 and .<90)]|length)],
       ["90-99%",([$c[]|select(.>=90 and .<100)]|length)],
       ["  100%",([$c[]|select(.>=100)]|length)]
     ][] | .[1] as $n | (if $tot>0 then (50*$n/$tot|round) else 0 end) as $bars
     | "  \(.[0]) \($n|lpad(4))  \(if $bars>0 then "▓"*$bars else "" end)"
   ' tmp/crap-now.json
  echo ""
  # Compared against the baseline, not against the floor alone: see `cov_baseline`.
  # A function is identified by file and name rather than by line, because a line
  # number moves whenever anything above it does.
  if [ -f {{ cov_baseline }} ]; then BASE={{ cov_baseline }}; else BASE=/dev/null; fi
  jq -n --slurpfile now tmp/crap-now.json --slurpfile base "$BASE" --argjson f {{ cov_floor }} '
     def id: (.file // "") + "::" + (.function // "");
     (($base[0].entries // []) | group_by(id)
       | map({key: (.[0] | id), value: ([.[] | .coverage] | min)}) | from_entries) as $was
     | [ $now[0].entries[] | select(.coverage < $f) | . as $e
         | ($was[$e | id]) as $before
         | select($before == null or $e.coverage < $before - 0.01)
         | {function: $e.function, file: $e.file, coverage: $e.coverage, was: $before} ]
   ' > tmp/cov-offenders.json
  UNDER=$(jq 'length' tmp/cov-offenders.json)
  if [ "$UNDER" -gt 0 ]; then
    echo "COVERAGE GATE FAILED: $UNDER function(s) new below {{ cov_floor }}%, or worse than the baseline"
    jq -r '.[] | "  \(.coverage)%  \(.function)  [\(.file)]" + (if .was then "  (was \(.was)%)" else "  (new)" end)' \
      tmp/cov-offenders.json | sort -n
    echo ""
    echo "Cover them, or — only for genuinely inherited debt — run \`just coverage-baseline\`."
    exit 1
  fi
  # A baselined function that has since reached the floor: the baseline is stale,
  # which is worth saying but is not a failure.
  RAISED=$(jq -n --slurpfile now tmp/crap-now.json --slurpfile base "$BASE" --argjson f {{ cov_floor }} '
     def id: (.file // "") + "::" + (.function // "");
     ([$now[0].entries[] | select(.coverage >= $f) | id]) as $ok
     | [($base[0].entries // [])[] | select((. | id) as $k | $ok | index($k))] | length')
  if [ "$RAISED" -gt 0 ]; then
    echo "Threshold OK — and $RAISED baselined function(s) now reach {{ cov_floor }}%."
    echo "Run \`just coverage-baseline\` to hold that ground."
  else
    BASELINED=$(jq '[.entries[]] | length' "$BASE" 2>/dev/null || echo 0)
    echo "Threshold OK: nothing new below {{ cov_floor }}% ($BASELINED grandfathered)"
  fi

# Regenerate the committed coverage baseline (`.coverage-baseline.json`).
#
# Only for genuinely inherited debt, and say so in the commit message — the same
# rule as `crap-baseline`. Refreshing this to silence a function you just wrote
# bakes in exactly what the gate exists to catch.
coverage-baseline:
  #!/usr/bin/env bash
  set -euo pipefail
  mkdir -p tmp
  if command -v llvm-profdata >/dev/null 2>&1; then
    export LLVM_COV="$(command -v llvm-cov)"
    export LLVM_PROFDATA="$(command -v llvm-profdata)"
  fi
  if [ -z "${REUSE_LCOV:-}" ]; then
    cargo llvm-cov --workspace --all-features --lcov --output-path tmp/lcov.info >/dev/null
  fi
  cargo crap --lcov tmp/lcov.info {{ crap_excludes }} --format json --output tmp/crap-now.json >/dev/null
  WS_ROOT="$(pwd)"
  # Merged with what is already there, keeping whichever reading is *lower*.
  # A baseline written from one machine is a file every machine has to satisfy,
  # and llvm-cov does not attribute lines identically across platforms: a refresh
  # on macOS dropped a function that Linux still measures under the floor, and CI
  # went red on a "new" violation that was nothing of the kind. Merging makes the
  # file the union of what has ever been seen, so a refresh can only ever be safe.
  if [ -f {{ cov_baseline }} ]; then WAS={{ cov_baseline }}; else WAS=/dev/null; fi
  jq -n --slurpfile now tmp/crap-now.json --slurpfile was "$WAS" \
    --argjson f {{ cov_floor }} --arg ws "$WS_ROOT/" \
    'def id: .file + "::" + .function;
     ([$now[0].entries[] | select(.coverage < $f) | {function, file: (.file | sub("^" + $ws; "")), coverage}]
      + (($was[0].entries // []))
      | group_by(id)
      | map({function: .[0].function, file: .[0].file, coverage: ([.[].coverage] | min)})
      | sort_by(.file, .function)) as $all
     | {entries: $all}' > tmp/cov-baseline.next
  # Through a temp file: `> {{ cov_baseline }}` truncates it before jq opens it,
  # so the merge would read an empty baseline and silently drop every entry.
  mv tmp/cov-baseline.next {{ cov_baseline }}
  KEPT=$(jq '.entries | length' {{ cov_baseline }})
  echo "Wrote {{ cov_baseline }} with $KEPT function(s) under {{ cov_floor }}%"

# List hand-written .rs files under crates/ over 30k — a "consider splitting" smell.
large-files:
  #!/usr/bin/env bash
  set -euo pipefail
  found=$(find crates -type f -name '*.rs' -size +30k)
  if [ -z "$found" ]; then
    echo "(no hand-written source file over 30k)"
  else
    echo "LARGE-FILE GATE FAILED: split these, or compact their comments first."
    echo ""
    trap 'exit 1' EXIT
    # `wc -lc` → lines + bytes; sort by size (bytes), print both.
    echo "$found" | xargs wc -lc | grep -v ' total$' | sort -rnk2 \
      | awk '{printf "  %6s lines  %7.1f KB  %s\n",$1,$2/1024,$3}'
  fi

# Clean ./tmp directory.
tmp:
  @rm tmp/*
  @mkdir -p tmp
