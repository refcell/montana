#!/bin/bash
# Montana vs OP Stack Benchmark Comparison
# This script runs benchmarks for both implementations and compares results.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       Montana vs OP Stack Benchmark Comparison               ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Create results directory
mkdir -p "$RESULTS_DIR"

# Check prerequisites
check_prerequisites() {
    echo -e "${YELLOW}Checking prerequisites...${NC}"

    # Check Rust
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Error: cargo not found. Please install Rust.${NC}"
        exit 1
    fi
    echo "  ✓ Rust $(rustc --version | cut -d' ' -f2)"

    # Check Go
    if ! command -v go &> /dev/null; then
        echo -e "${RED}Error: go not found. Please install Go.${NC}"
        exit 1
    fi
    echo "  ✓ Go $(go version | cut -d' ' -f3)"

    echo ""
}

# Generate fixtures if they don't exist
generate_fixtures() {
    echo -e "${YELLOW}Checking fixtures...${NC}"

    if [ ! -f "$SCRIPT_DIR/fixtures/blocks_100.json" ]; then
        echo "  Generating fixtures..."
        cd "$SCRIPT_DIR/op-stack"
        go run generate_fixtures.go
        cd "$REPO_ROOT"
    fi
    echo "  ✓ Fixtures ready"
    echo ""
}

# Run Montana (Rust) benchmarks
run_montana_benchmarks() {
    echo -e "${YELLOW}Running Montana (Rust) benchmarks...${NC}"
    echo ""

    cd "$REPO_ROOT"

    # Run criterion benchmarks and save output
    cargo bench --bench pipeline -- --noplot 2>&1 | tee "$RESULTS_DIR/montana_$TIMESTAMP.txt"

    echo ""
    echo -e "${GREEN}  ✓ Montana benchmarks complete${NC}"
    echo ""
}

# Run OP Stack (Go) benchmarks
run_opstack_benchmarks() {
    echo -e "${YELLOW}Running OP Stack (Go) benchmarks...${NC}"
    echo ""

    cd "$SCRIPT_DIR/op-stack"

    # Initialize go module if needed
    if [ ! -f "go.sum" ]; then
        go mod tidy 2>/dev/null || true
    fi

    # Run Go benchmarks
    go test -bench=. -benchmem -count=5 -benchtime=1s 2>&1 | tee "$RESULTS_DIR/opstack_$TIMESTAMP.txt"

    cd "$REPO_ROOT"

    echo ""
    echo -e "${GREEN}  ✓ OP Stack benchmarks complete${NC}"
    echo ""
}

# Parse and compare results
compare_results() {
    echo -e "${YELLOW}Comparing results...${NC}"
    echo ""

    MONTANA_FILE="$RESULTS_DIR/montana_$TIMESTAMP.txt"
    OPSTACK_FILE="$RESULTS_DIR/opstack_$TIMESTAMP.txt"
    COMPARISON_FILE="$RESULTS_DIR/comparison_$TIMESTAMP.md"

    # Generate comparison markdown
    cat > "$COMPARISON_FILE" << EOF
# Benchmark Comparison Results

**Date:** $(date)
**Montana Version:** $(git rev-parse --short HEAD)
**Go Version:** $(go version | cut -d' ' -f3)
**Rust Version:** $(rustc --version | cut -d' ' -f2)

## System Information

- **OS:** $(uname -s) $(uname -r)
- **CPU:** $(sysctl -n machdep.cpu.brand_string 2>/dev/null || cat /proc/cpuinfo | grep "model name" | head -1 | cut -d: -f2)
- **Cores:** $(sysctl -n hw.ncpu 2>/dev/null || nproc)

## Results

### Montana (Rust)

\`\`\`
$(cat "$MONTANA_FILE" | head -100)
\`\`\`

### OP Stack (Go)

\`\`\`
$(cat "$OPSTACK_FILE" | head -100)
\`\`\`

## Summary

| Benchmark | Montana | OP Stack | Speedup |
|-----------|---------|----------|---------|
| *Results to be parsed* | - | - | - |

---

*Raw results saved in:*
- Montana: \`results/montana_$TIMESTAMP.txt\`
- OP Stack: \`results/opstack_$TIMESTAMP.txt\`
EOF

    echo -e "${GREEN}Comparison saved to: $COMPARISON_FILE${NC}"
    echo ""
}

# Main execution
main() {
    check_prerequisites
    generate_fixtures

    echo -e "${BLUE}Starting benchmarks...${NC}"
    echo ""

    run_montana_benchmarks
    run_opstack_benchmarks
    compare_results

    echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║                    Benchmarks Complete!                       ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Results saved to: $RESULTS_DIR/"
    echo ""
}

# Run specific benchmarks based on arguments
case "${1:-all}" in
    montana)
        check_prerequisites
        run_montana_benchmarks
        ;;
    opstack)
        check_prerequisites
        generate_fixtures
        run_opstack_benchmarks
        ;;
    compare)
        compare_results
        ;;
    all)
        main
        ;;
    *)
        echo "Usage: $0 [montana|opstack|compare|all]"
        exit 1
        ;;
esac
