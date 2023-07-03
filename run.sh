#!/bin/sh

signal_handler() {
  kill $(jobs -p -r)
  exit
}
trap signal_handler INT TERM

echo 'usage: ./run.sh <node count>'

n=${1:-10}

export RUST_LOG=safe
local peers=""

cargo build --bin=safe --release

for i in $(seq 0 $n)
do
    port=$((10000 + i))
    peers="${peers}${peers:+,}/ip4/127.0.0.1/tcp/${port}"

    PEERS="${peers}" cargo run --bin=safe --release -- --port="${port}" 2> "${port}.log" &
done

wait
