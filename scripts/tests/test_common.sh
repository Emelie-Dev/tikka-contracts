#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
source "${REPO_ROOT}/scripts/common.sh"

TEMP_ROOT="$(mktemp -d)"
trap 'rm -rf "${TEMP_ROOT}"' EXIT

TEST_REPO_ROOT="${TEMP_ROOT}/repo"
mkdir -p "${TEST_REPO_ROOT}/deployments" "${TEMP_ROOT}/bin"
REPO_ROOT="${TEST_REPO_ROOT}"

cat > "${TEMP_ROOT}/bin/stellar" <<'STELLAR'
#!/usr/bin/env bash

if [[ " $* " == *" contract install "* ]]; then
  printf '%s\n' 'instance-wasm-hash'
elif [[ " $* " == *" contract deploy "* ]]; then
  printf '%s\n' 'factory-contract-id'
elif [[ " $* " == *" get_admin "* ]]; then
  printf '%s\n' "${STUB_ADMIN:-expected-admin}"
elif [[ " $* " == *" --version "* ]]; then
  printf '%s\n' 'stellar 23.4.1'
fi
STELLAR
chmod +x "${TEMP_ROOT}/bin/stellar"
PATH="${TEMP_ROOT}/bin:${PATH}"

FACTORY_WASM="${TEST_REPO_ROOT}/factory.wasm"
INSTANCE_WASM="${TEST_REPO_ROOT}/instance.wasm"
printf 'factory' > "${FACTORY_WASM}"
printf 'instance' > "${INSTANCE_WASM}"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

cat > "${TEST_REPO_ROOT}/deployments/testnet.json" <<'JSON'
{"contractId":"existing-factory"}
JSON

if (unset ALLOW_REDEPLOY; require_no_existing_deployment testnet) 2>/dev/null; then
  fail "require_no_existing_deployment allowed an existing deployment"
fi

ALLOW_REDEPLOY=1 require_no_existing_deployment testnet

FACTORY_CONTRACT_ID="factory-id"
STUB_ADMIN="different-admin"
if (verify_factory_initialised testnet source-key expected-admin) 2>/dev/null; then
  fail "verify_factory_initialised accepted a different admin"
fi

STUB_ADMIN="expected-admin"
deploy_and_init_factory testnet source-key expected-admin 'treasury address' 25 >/dev/null
[[ "${INSTANCE_WASM_HASH}" == 'instance-wasm-hash' ]] ||
  fail "deploy_and_init_factory did not capture the install hash"
[[ "${FACTORY_CONTRACT_ID}" == 'factory-contract-id' ]] ||
  fail "deploy_and_init_factory did not capture the contract ID"

json_fixture="${TEMP_ROOT}/fixture.json"
cat > "${json_fixture}" <<'JSON'
{"description":"value with a space and a } brace"}
JSON
[[ "$(json_field "${json_fixture}" description)" == 'value with a space and a } brace' ]] ||
  fail "json_field did not preserve spaces and braces"

FACTORY_CONTRACT_ID="factory id"
FACTORY_WASM_HASH="factory hash"
INSTANCE_WASM_HASH="instance hash"
write_deployment_manifest testnet 'admin address' 'treasury address' 25

python3 -m json.tool "${TEST_REPO_ROOT}/deployments/testnet.json" >/dev/null
[[ "$(wc -l < "${TEST_REPO_ROOT}/deployments/testnet-history.jsonl")" -eq 1 ]] ||
  fail "deployment history did not receive exactly one JSONL record"
[[ "$(json_field "${TEST_REPO_ROOT}/deployments/testnet-history.jsonl" contractId)" == 'factory id' ]] ||
  fail "deployment history did not preserve the manifest record"

echo "common.sh tests passed"