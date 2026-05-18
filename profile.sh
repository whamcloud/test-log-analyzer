#!/usr/bin/env bash
set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <output.svg>"
    exit 1
fi

OUTPUT="$1"

if [ ! -f "test.dat" ]; then
    echo "Error: test.dat does not exist in the current directory."
    exit 1
fi

echo "Building ddnn using profiling profile..."
cargo build --profile profiling

echo "Running dtrace (this requires sudo privileges)..."
sudo dtrace -c './target/profiling/ddnn test.dat' -o flamegraph/out.stacks -n 'profile-997 /execname == "ddnn"/ { @[ustack(100)] = count(); }'

echo "Generating flamegraph..."
cd flamegraph
./stackcollapse.pl out.stacks | ./flamegraph.pl > "$OUTPUT"
echo "Done! Flamegraph generated at flamegraph/$OUTPUT"
