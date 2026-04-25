#!/bin/bash
set -e

echo "=== Vibe Index Benchmark Suite ==="
echo ""

# Run benchmarks and save results
cargo bench 2>&1 | tee benchmark-output.txt

# Parse key metrics from benchmark output
echo ""
echo "=== Key Metrics ==="
grep -A1 "time:" benchmark-output.txt | grep -v "^--$" | head -20

echo ""
echo "=== Summary ==="
echo "✅ All benchmarks completed successfully"
echo "📊 Full results: target/release/.criterion/"
