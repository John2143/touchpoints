#!/usr/bin/env bash

pushd simple
cargo build

rm strace_simple_*.txt

for i in {1..2}
do
    TRACE_NAME="strace_simple_$i.txt"
    strace -o "$TRACE_NAME" "./target/debug/simple" "$i"
done

popd
