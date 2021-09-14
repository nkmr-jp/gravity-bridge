#!/bin/bash

mkdir -p /peggy/data
mkdir -p /peggy/data/json_log

# the script run inside the container for all-up-test.sh
NODES=$1
TEST_TYPE=$2
ALCHEMY_ID=$3
set -eux

# Prepare the contracts for later deployment
pushd /peggy/solidity/
HUSKY_SKIP_INSTALL=1 npm install
npm run typechain

bash /peggy/tests/container-scripts/setup-validators.sh $NODES

bash /peggy/tests/container-scripts/run-testnet.sh $NODES $TEST_TYPE $ALCHEMY_ID &

# deploy the ethereum contracts
pushd /peggy/orchestrator/test_runner
DEPLOY_CONTRACTS=1 RUST_BACKTRACE=full RUST_LOG=INFO PATH=$PATH:$HOME/.cargo/bin cargo run --release --bin test-runner

bash /peggy/tests/container-scripts/integration-tests.sh $NODES $TEST_TYPE