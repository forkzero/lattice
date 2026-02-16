#!/usr/bin/env bash
set -euo pipefail

# ============================================================
#  Lattice End-to-End Integration Test
#
#  Topic: Proving That Requirements Documentation Is Unnecessary
#  (Using a requirements documentation tool. The irony is the point.)
# ============================================================

PASS=0
FAIL=0

pass() { ((PASS++)) || true; echo "  ✓ $1"; }
fail() { ((FAIL++)) || true; echo "  ✗ $1"; }

assert_contains() {
  if echo "$1" | grep -q "$2"; then pass "$3"; else fail "$3 (expected '$2')"; fi
}

assert_file_exists() {
  if [ -f "$1" ]; then pass "$2"; else fail "$2 (file not found: $1)"; fi
}

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Lattice E2E Integration Test                           ║"
echo "║  Topic: Why You Don't Need Requirements Documentation   ║"
echo "║  (Structured as a requirements document, obviously.)    ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

# === Setup ===
echo "--- Setup ---"
WORKDIR=$(mktemp -d)
cd "$WORKDIR"
git init -q
git config user.email "vibes@example.com"
git config user.name "Vibes-Driven Developer"
git remote add origin https://github.com/vibes-driven/anti-docs.git
pass "Created temp project at $WORKDIR"

VERSION=$(lattice --version)
pass "Lattice binary available ($VERSION)"

# === Initialize ===
echo ""
echo "--- Initialize Lattice ---"
lattice init
[ -d ".lattice" ] && pass ".lattice directory created" || fail ".lattice directory missing"
[ -f ".lattice/config.yaml" ] && pass "config.yaml exists" || fail "config.yaml missing"

# === Add Sources ===
echo ""
echo "--- Add Sources (The Research Nobody Asked For) ---"

lattice add source \
  --id SRC-AGILE-MANIFESTO \
  --title "The Agile Manifesto Prioritizes Working Software" \
  --body "The Agile Manifesto values working software over comprehensive documentation. Many teams interpreted this as do not write docs at all, which was definitely the intended message and not a nuanced trade-off." \
  --url "https://agilemanifesto.org" \
  --reliability peer_reviewed \
  --created-by "human:e2e-test"
pass "Added SRC-AGILE-MANIFESTO"

lattice add source \
  --id SRC-DOCS-NOBODY-READS \
  --title "Nobody Reads Internal Documentation" \
  --body "Studies show 60-80% of internal documentation is never read after creation. The remaining 20% is read once, misunderstood, and then the reader asks the original author on Slack anyway." \
  --url "https://doi.org/10.1234/fictional-but-relatable" \
  --reliability industry \
  --created-by "human:e2e-test"
pass "Added SRC-DOCS-NOBODY-READS"

lattice add source \
  --id SRC-SELF-DOCUMENTING \
  --title "Good Code Is Self-Documenting (Citation Needed)" \
  --body "Every senior developer has said the code is the documentation at least once. This is always true and never an excuse for writing incomprehensible variable names like x2_final_FINAL_v3." \
  --reliability blog \
  --created-by "human:e2e-test"
pass "Added SRC-SELF-DOCUMENTING"

# === Add Theses ===
echo ""
echo "--- Add Theses (Strategic Claims of Dubious Merit) ---"

lattice add thesis \
  --id THX-DOCS-ARE-WASTE \
  --title "Requirements Documentation Is Where Productivity Goes to Die" \
  --body "For every hour spent writing requirements, zero hours are spent reading them. The ROI of documentation approaches negative infinity as team size approaches one developer who keeps it all in their head." \
  --category value_prop \
  --confidence 0.1 \
  --supported-by SRC-AGILE-MANIFESTO,SRC-DOCS-NOBODY-READS \
  --created-by "human:e2e-test"
pass "Added THX-DOCS-ARE-WASTE (confidence: 10%)"

lattice add thesis \
  --id THX-VIBES-DRIVEN \
  --title "Vibes-Driven Development Is the Future" \
  --body "Why write specifications when you can just feel what the code should do? Intuition scales perfectly across teams, time zones, and the inevitable heat death of the universe." \
  --category technical \
  --confidence 0.05 \
  --supported-by SRC-SELF-DOCUMENTING \
  --created-by "human:e2e-test"
pass "Added THX-VIBES-DRIVEN (confidence: 5%)"

# === Add Requirements ===
echo ""
echo "--- Add Requirements (The Irony Thickens) ---"

lattice add requirement \
  --id REQ-ANTI-001 \
  --title "Auto-Delete Stale Documentation" \
  --body "Any documentation older than 30 days must be automatically deleted. If it was important, someone will rewrite it from memory. This is fine. Everything is fine." \
  --priority P0 \
  --category ANTI-DOCS \
  --tags "documentation,deletion,chaos" \
  --derives-from THX-DOCS-ARE-WASTE \
  --created-by "human:e2e-test"
pass "Added REQ-ANTI-001"

lattice add requirement \
  --id REQ-ANTI-002 \
  --title "Replace All Requirements With Vibes" \
  --body "Formal requirements shall be replaced with a single Slack message: you know what to do. Acceptance criteria: the developer mass-produced good vibes while coding." \
  --priority P0 \
  --category ANTI-DOCS \
  --tags "vibes,feelings,yolo" \
  --derives-from THX-VIBES-DRIVEN \
  --created-by "human:e2e-test"
pass "Added REQ-ANTI-002"

lattice add requirement \
  --id REQ-ANTI-003 \
  --title "README Minimalism" \
  --body "All READMEs must contain exactly one line: the project name. Additional context is a crutch for developers who lack the telepathy required for modern software engineering." \
  --priority P1 \
  --category ANTI-DOCS \
  --tags "readme,minimalism,telepathy" \
  --derives-from THX-DOCS-ARE-WASTE \
  --depends-on REQ-ANTI-001 \
  --created-by "human:e2e-test"
pass "Added REQ-ANTI-003"

# === Add Implementations ===
echo ""
echo "--- Add Implementations (Code That Should Not Exist) ---"

lattice add implementation \
  --id IMP-VIBES-001 \
  --title "vibes.sh — The Vibes-Based Development Engine" \
  --body "A shell script that replaces all documentation with the word vibes. Features include: deleting READMEs, replacing comments with good energy, and mass-producing positive affirmations in CI logs." \
  --language bash \
  --files "scripts/vibes.sh" \
  --test-command "bash -c 'echo vibes'" \
  --satisfies REQ-ANTI-001,REQ-ANTI-002 \
  --created-by "human:e2e-test"
pass "Added IMP-VIBES-001"

lattice add implementation \
  --id IMP-README-001 \
  --title "readme-enforcer.py — The One-Line README Guardian" \
  --body "A pre-commit hook that truncates any README to its first line. Knowledge is temporary; minimalism is forever. Has zero tests because testing is just documentation for robots." \
  --language python \
  --files "hooks/readme-enforcer.py" \
  --satisfies REQ-ANTI-003 \
  --created-by "human:e2e-test"
pass "Added IMP-README-001"

# === Add Feedback Edges (The Irony Deepens) ===
echo ""
echo "--- Add Feedback Edges ---"

lattice add edge \
  --from IMP-VIBES-001 \
  --edge-type challenges \
  --to THX-DOCS-ARE-WASTE \
  --rationale "Ironically, building this script required reading documentation for every CLI flag. The thesis may have some holes."
pass "Added challenges edge (IMP-VIBES-001 -> THX-DOCS-ARE-WASTE)"

lattice add edge \
  --from IMP-README-001 \
  --edge-type reveals_gap_in \
  --to REQ-ANTI-003 \
  --rationale "Requirement does not specify what to do when the README IS the only documentation and deleting it causes an existential crisis."
pass "Added reveals_gap_in edge (IMP-README-001 -> REQ-ANTI-003)"

lattice add edge \
  --from IMP-VIBES-001 \
  --type validates \
  --to THX-VIBES-DRIVEN \
  --rationale "The script works. We have no idea why. This validates the vibes-driven thesis perfectly."
pass "Added validates edge using --type alias (IMP-VIBES-001 -> THX-VIBES-DRIVEN)"

# === Verify Structure ===
echo ""
echo "--- Verify Structure ---"

OUT=$(lattice list sources)
assert_contains "$OUT" "SRC-AGILE-MANIFESTO" "list sources: SRC-AGILE-MANIFESTO"
assert_contains "$OUT" "SRC-DOCS-NOBODY-READS" "list sources: SRC-DOCS-NOBODY-READS"
assert_contains "$OUT" "SRC-SELF-DOCUMENTING" "list sources: SRC-SELF-DOCUMENTING"

OUT=$(lattice list theses)
assert_contains "$OUT" "THX-DOCS-ARE-WASTE" "list theses: THX-DOCS-ARE-WASTE"
assert_contains "$OUT" "THX-VIBES-DRIVEN" "list theses: THX-VIBES-DRIVEN"

OUT=$(lattice list requirements)
assert_contains "$OUT" "REQ-ANTI-001" "list requirements: REQ-ANTI-001"
assert_contains "$OUT" "REQ-ANTI-002" "list requirements: REQ-ANTI-002"
assert_contains "$OUT" "REQ-ANTI-003" "list requirements: REQ-ANTI-003"

OUT=$(lattice list implementations)
assert_contains "$OUT" "IMP-VIBES-001" "list implementations: IMP-VIBES-001"
assert_contains "$OUT" "IMP-README-001" "list implementations: IMP-README-001"

# === Get Individual Nodes ===
echo ""
echo "--- Get Individual Nodes ---"

OUT=$(lattice get SRC-AGILE-MANIFESTO)
assert_contains "$OUT" "Agile Manifesto" "get SRC-AGILE-MANIFESTO"

OUT=$(lattice get THX-DOCS-ARE-WASTE)
assert_contains "$OUT" "Productivity Goes to Die" "get THX-DOCS-ARE-WASTE"

OUT=$(lattice get REQ-ANTI-002)
assert_contains "$OUT" "Vibes" "get REQ-ANTI-002"

OUT=$(lattice get IMP-VIBES-001)
assert_contains "$OUT" "vibes.sh" "get IMP-VIBES-001"
assert_contains "$OUT" "Edges:" "get IMP-VIBES-001: shows edge summary"

# === Lint ===
echo ""
echo "--- Lint ---"

lattice lint && pass "lattice lint passed" || pass "lattice lint found issues (expected for test data)"

# === Drift Detection ===
echo ""
echo "--- Drift Detection ---"

lattice drift
pass "lattice drift completed"

# === Export: JSON ===
echo ""
echo "--- Export: JSON ---"

OUT=$(lattice export --format json)
assert_contains "$OUT" "SRC-AGILE-MANIFESTO" "JSON export: sources present"
assert_contains "$OUT" "THX-DOCS-ARE-WASTE" "JSON export: theses present"
assert_contains "$OUT" "REQ-ANTI-001" "JSON export: requirements present"
assert_contains "$OUT" "IMP-VIBES-001" "JSON export: implementations present"

# === Export: HTML ===
echo ""
echo "--- Export: HTML ---"

mkdir -p docs
lattice export --format html --output docs
assert_file_exists "docs/index.html" "HTML export: index.html created"

# === Export: Pages ===
echo ""
echo "--- Export: Pages ---"

lattice export --format pages --output _site
assert_file_exists "_site/index.html" "Pages export: index.html created"
assert_file_exists "_site/lattice-data.json" "Pages export: lattice-data.json created"

# ============================================================
#  PART 2: Edge Cases, Error Handling, and Advanced Features
#  (The documentation about documentation gets more thorough.)
# ============================================================

# === Error Handling: Duplicate IDs ===
echo ""
echo "--- Error Handling: Duplicate IDs ---"

OUT=$(lattice add source \
  --id SRC-AGILE-MANIFESTO \
  --title "Duplicate" \
  --body "This should fail" \
  --created-by "human:e2e-test" 2>&1) || true
assert_contains "$OUT" "already exists" "Duplicate source ID rejected"

OUT=$(lattice add thesis \
  --id THX-DOCS-ARE-WASTE \
  --title "Duplicate" \
  --body "This should fail" \
  --category technical \
  --created-by "human:e2e-test" 2>&1) || true
assert_contains "$OUT" "already exists" "Duplicate thesis ID rejected"

OUT=$(lattice add requirement \
  --id REQ-ANTI-001 \
  --title "Duplicate" \
  --body "This should fail" \
  --priority P0 \
  --category DUPE \
  --created-by "human:e2e-test" 2>&1) || true
assert_contains "$OUT" "already exists" "Duplicate requirement ID rejected"

# === Error Handling: Invalid Edge Type ===
echo ""
echo "--- Error Handling: Invalid Edge Type ---"

OUT=$(lattice add edge \
  --from IMP-VIBES-001 \
  --edge-type good_vibes_toward \
  --to THX-VIBES-DRIVEN 2>&1) || true
assert_contains "$OUT" "nvalid" "Invalid edge type rejected"

# === Error Handling: Nonexistent Node ===
echo ""
echo "--- Error Handling: Nonexistent Node ---"

OUT=$(lattice get DOES-NOT-EXIST 2>&1) || true
assert_contains "$OUT" "not found\|Not found\|No node" "Get nonexistent node returns error"

OUT=$(lattice add edge \
  --from IMP-VIBES-001 \
  --edge-type challenges \
  --to DOES-NOT-EXIST 2>&1) || true
assert_contains "$OUT" "not found\|Not found\|No node" "Edge to nonexistent target rejected"

# === JSON Output Mode ===
echo ""
echo "--- JSON Output Mode ---"

OUT=$(lattice list sources --format json)
assert_contains "$OUT" '"id"' "list sources --format json: has id field"
assert_contains "$OUT" "SRC-AGILE-MANIFESTO" "list sources --format json: has node IDs"

OUT=$(lattice list requirements --format json)
assert_contains "$OUT" '"priority"' "list requirements --format json: has priority field"

OUT=$(lattice get THX-VIBES-DRIVEN --format json)
assert_contains "$OUT" '"id"' "get --format json: has id field"
assert_contains "$OUT" "THX-VIBES-DRIVEN" "get --format json: correct node"

OUT=$(lattice add source \
  --id SRC-JSON-TEST \
  --title "JSON Output Test" \
  --body "Testing that JSON output works on add" \
  --created-by "human:e2e-test" \
  --format json)
assert_contains "$OUT" "SRC-JSON-TEST" "add source --format json: returns node ID"

# === Edge Upsert (Update Existing Edge) ===
echo ""
echo "--- Edge Upsert ---"

lattice add edge \
  --from IMP-VIBES-001 \
  --edge-type challenges \
  --to THX-DOCS-ARE-WASTE \
  --rationale "Updated rationale: the irony has compounded"
pass "Edge upsert succeeded (same type, same target)"

OUT=$(lattice get IMP-VIBES-001 --format json)
assert_contains "$OUT" "compounded" "Upserted edge has updated rationale"

# === Summary Command ===
echo ""
echo "--- Summary Command ---"

OUT=$(lattice summary)
assert_contains "$OUT" "source" "summary: shows sources"
assert_contains "$OUT" "thes" "summary: shows theses"
assert_contains "$OUT" "requirement" "summary: shows requirements"
assert_contains "$OUT" "implementation" "summary: shows implementations"
pass "lattice summary completed"

OUT=$(lattice summary --format json)
assert_contains "$OUT" "{" "summary --format json: returns JSON"

# === Search Command ===
echo ""
echo "--- Search Command ---"

OUT=$(lattice search -t requirements -q "vibes")
assert_contains "$OUT" "REQ-ANTI-002" "search requirements by text: found vibes requirement"

OUT=$(lattice search -t requirements -p P0)
assert_contains "$OUT" "REQ-ANTI-001" "search requirements by priority P0: found"
assert_contains "$OUT" "REQ-ANTI-002" "search requirements by priority P0: found both"

OUT=$(lattice search -t requirements --tag "telepathy")
assert_contains "$OUT" "REQ-ANTI-003" "search requirements by tag: found telepathy"

OUT=$(lattice search -t sources -q "agile")
assert_contains "$OUT" "SRC-AGILE-MANIFESTO" "search sources by text: found agile"

OUT=$(lattice search -t requirements -c ANTI-DOCS)
assert_contains "$OUT" "REQ-ANTI-001" "search requirements by category: found"

# Positional node type syntax
OUT=$(lattice search requirements -q "vibes")
assert_contains "$OUT" "REQ-ANTI-002" "search positional syntax: requirements -q vibes"

# Default type (requirements) when no positional or -t given
OUT=$(lattice search -q "vibes")
assert_contains "$OUT" "REQ-ANTI-002" "search default type: -q vibes (defaults to requirements)"

# === Resolve Command ===
echo ""
echo "--- Resolve Command ---"

lattice resolve REQ-ANTI-001 --verified
pass "Resolved REQ-ANTI-001 as verified"

OUT=$(lattice get REQ-ANTI-001 --format json)
assert_contains "$OUT" "verified\|Verified" "Resolved requirement shows verified in JSON"

OUT=$(lattice get REQ-ANTI-001)
assert_contains "$OUT" "resolution: verified" "get text output: shows resolution verified"

lattice resolve REQ-ANTI-002 --deferred "Vibes are not yet production-ready"
pass "Resolved REQ-ANTI-002 as deferred"

OUT=$(lattice get REQ-ANTI-002)
assert_contains "$OUT" "resolution: deferred" "get text output: shows resolution deferred"

OUT=$(lattice search -t requirements -r deferred)
assert_contains "$OUT" "REQ-ANTI-002" "search by resolution: found deferred requirement"

# === List Filters ===
echo ""
echo "--- List Filters ---"

OUT=$(lattice list requirements --priority P0)
assert_contains "$OUT" "REQ-ANTI-001" "list requirements --priority P0: found"

OUT=$(lattice list requirements --deferred)
assert_contains "$OUT" "REQ-ANTI-002" "list requirements --deferred: shows deferred"

OUT=$(lattice list requirements --blocked --deferred)
assert_contains "$OUT" "REQ-ANTI-002" "list requirements --blocked --deferred: shows deferred"

# === Drift --check (No Drift) ===
echo ""
echo "--- Drift --check (Exit Code) ---"

lattice drift --check && pass "drift --check exits 0 when no drift" || fail "drift --check should exit 0"

# === Verify Command ===
echo ""
echo "--- Verify Command ---"

lattice verify IMP-VIBES-001 satisfies REQ-ANTI-001 --tests-pass
pass "verify IMP-VIBES-001 satisfies REQ-ANTI-001 succeeded"

# === Refine Command ===
echo ""
echo "--- Refine Command ---"

lattice refine REQ-ANTI-003 \
  --gap-type missing_requirement \
  --title "Define Behavior When README Is Only Documentation" \
  --description "The requirement to truncate READMEs does not address the case where the README is the sole source of project documentation." \
  --proposal "Add an exception: if no other docs exist, the README may contain up to three lines." \
  --discovered-by IMP-README-001
pass "Refined REQ-ANTI-003 with --proposal and --discovered-by aliases"

OUT=$(lattice list requirements)
assert_contains "$OUT" "REQ-ANTI-003" "Refined requirement still listed"

# === Help Command ===
echo ""
echo "--- Help Commands ---"

OUT=$(lattice help --json)
assert_contains "$OUT" '"commands"' "help --json: returns command catalog"
assert_contains "$OUT" '"add requirement"' "help --json: includes add requirement command"
assert_contains "$OUT" '"drift"' "help --json: includes drift command"
assert_contains "$OUT" '"search"' "help --json: includes search command"
assert_contains "$OUT" '"short"' "help --json: includes short flag field"

# === Export: Investor Audience ===
echo ""
echo "--- Export: Investor Audience ---"

OUT=$(lattice export --audience investor)
assert_contains "$OUT" "Lattice" "Investor export: has title"
pass "Investor audience export succeeded"

# === Init Idempotency ===
echo ""
echo "--- Init Idempotency ---"

OUT=$(lattice init 2>&1)
assert_contains "$OUT" "already initialized" "Re-init prints friendly message"
pass "Re-init exits 0 (no error)"

# === THE REPORT ===
echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  THE REPORT                                             ║"
echo "║  (A beautifully structured document about why you       ║"
echo "║   don't need beautifully structured documents.)         ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

lattice export --audience overview

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  CONTRIBUTOR VIEW                                       ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

lattice export --audience contributor

# === Summary ===
echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  RESULTS: $PASS passed, $FAIL failed"
printf "║%-57s║\n" ""
echo "║  The lattice just generated a beautifully structured    ║"
echo "║  requirements document about why you don't need         ║"
echo "║  requirements documents.                                ║"
printf "║%-57s║\n" ""
echo "║  The tool has become self-aware of the irony.           ║"
echo "╚══════════════════════════════════════════════════════════╝"

# Cleanup
rm -rf "$WORKDIR"

[ "$FAIL" -eq 0 ] && exit 0 || exit 1
