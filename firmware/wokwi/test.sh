#!/bin/sh
# Runs the Wokwi scenario tests (tests/*.test.yaml) against the real firmware
# and prints a summary. Exits non-zero if any test fails.
#
#   firmware/wokwi/test.sh               all tests
#   firmware/wokwi/test.sh brew faults   only these (names without .test.yaml)
#   firmware/wokwi/test.sh --build ...   rebuild the image and chips first
#
# Each test runs on Wokwi's servers through run.sh (token handling is there).
# The free plan stops a run after 5 minutes of real time; each test here takes
# about 1.5-3 minutes. Serial logs are kept in firmware/wokwi/test-logs/.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
logs=$here/test-logs
timeout_ms=${WOKWI_TEST_TIMEOUT_MS:-120000}

if [ "${1:-}" = "--build" ]; then
  shift
  "$here/build.sh"
fi

if [ $# -gt 0 ]; then
  tests=$*
else
  tests=$(cd "$here/tests" && ls *.test.yaml | sed 's/\.test\.yaml$//')
fi

mkdir -p "$logs"
passed=""
failed=""
for t in $tests; do
  scenario=tests/$t.test.yaml
  [ -f "$here/$scenario" ] || { echo "test.sh: no such test: $scenario" >&2; exit 2; }
  printf '\n=== %s ===\n' "$t"
  start=$(date +%s)
  # The full output goes to test-logs/<test>.out; the screen shows the
  # scenario's progress and the firmware's own log lines.
  {
    st=0
    (cd "$here" && "$here/run.sh" --scenario "$scenario" --timeout "$timeout_ms" \
      --fail-text 'Guru Meditation' --serial-log-file "$logs/$t.log") 2>&1 || st=$?
    echo "$st" >"$logs/$t.status"
  } | tee "$logs/$t.out" | grep --line-buffered -E "^\[|run\.sh|API Error| (brew|motion|cmd|cal|flow|http): " || true
  secs=$(( $(date +%s) - start ))
  if [ "$(cat "$logs/$t.status")" = 0 ]; then
    passed="$passed $t"
  else
    failed="$failed $t"
  fi
  printf -- '--- %s: %ss\n' "$t" "$secs"
done

printf '\n'
[ -n "$passed" ] && printf 'PASS:%s\n' "$passed"
[ -n "$failed" ] && printf 'FAIL:%s   (serial logs in %s)\n' "$failed" "$logs"
[ -z "$failed" ]
