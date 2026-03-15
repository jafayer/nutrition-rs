#!/usr/bin/env bash
# run.sh — Top-level entry point for the DNS load test.
#
# Modes:
#   local   Run entirely with Docker Compose (no AWS account required).
#   aws     Provision infra in AWS, run the test, collect results, tear down.
#
# Usage:
#   ./run.sh local   [--duration <s>] [--branch <name>] [--cluster-mode]
#   ./run.sh aws     [options — passed to orchestrate.py]
#
# Prerequisites (both modes):
#   docker, docker compose
#
# Additional prerequisites (aws mode):
#   aws CLI, terraform, python3 + boto3
#   AWS credentials in env (AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY / AWS_DEFAULT_REGION)
#   or via ~/.aws/credentials and ~/.aws/config

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}"

MODE="${1:-}"
shift || true

# ---------------------------------------------------------------------------
usage() {
  cat <<'EOF'
Usage:
  ./run.sh local  [--duration <seconds>] [--count <n>] [--branch <git-branch>]
                  [--cluster-mode] [--seed <n>] [-- <extra dnsperf args>]

  ./run.sh aws    [--region <aws-region>] [--duration <seconds>]
                  [--count <n>] [--branch <git-branch>] [--cluster-mode]
                  [--dinodns-cpu <units>] [--dinodns-memory <mib>]
                  [--dinodns-count <n>] [--coredns-cpu <units>]
                  [--coredns-memory <mib>] [--coredns-count <n>]
                  [--dnsperf-cpu <units>] [--dnsperf-memory <mib>]
                  [--no-destroy] [--seed <n>]
EOF
  exit 1
}

if [[ -z "${MODE}" || "${MODE}" == "--help" || "${MODE}" == "-h" ]]; then
  usage
fi

# ---------------------------------------------------------------------------
# Shared defaults
DURATION=30
DOMAIN_COUNT=100
DINODNS_BRANCH="main"
CLUSTER_MODE=""
SEED=""
EXTRA_DNSPERF_ARGS=""

parse_shared() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --duration)    DURATION="$2";       shift 2 ;;
      --count)       DOMAIN_COUNT="$2";   shift 2 ;;
      --branch)      DINODNS_BRANCH="$2"; shift 2 ;;
      --cluster-mode) CLUSTER_MODE="--cluster-mode"; shift ;;
      --seed)        SEED="--seed $2";    shift 2 ;;
      --)            shift; EXTRA_DNSPERF_ARGS="$*"; break ;;
      *)             break ;;
    esac
  done
}

# ===========================================================================
# LOCAL mode
# ===========================================================================
if [[ "${MODE}" == "local" ]]; then
  parse_shared "$@"

  echo ""
  echo "=================================================="
  echo "  DNS Load Test — LOCAL mode (Docker Compose)"
  echo "=================================================="
  echo ""

  # Step 1: Generate config files
  echo "[1/3] Generating domain / config files …"
  python3 generate.py \
    --output-dir ./config \
    --tld "loadtest.internal" \
    --count "${DOMAIN_COUNT}" \
    --dns-port 53 \
    ${CLUSTER_MODE} \
    ${SEED}

  # Resolve cluster mode to a plain boolean string for Docker Compose
  if [ -n "${CLUSTER_MODE}" ]; then
    CLUSTER_MODE_BOOL="true"
  else
    CLUSTER_MODE_BOOL="false"
  fi

  # Step 2: Write a .env so Docker Compose picks up runtime settings
  cat > .env <<EOF
DINODNS_BRANCH=${DINODNS_BRANCH}
TEST_DURATION=${DURATION}
DINODNS_CLUSTER_MODE=${CLUSTER_MODE_BOOL}
DNSPERF_EXTRA_ARGS=${EXTRA_DNSPERF_ARGS}
EOF

  echo "[2/3] Starting services with Docker Compose …"
  echo "      (DNS servers run on separate containers from DNSperf)"
  echo ""

  # Step 3: Run
  docker compose up \
    --build \
    --abort-on-container-exit \
    --exit-code-from dnsperf-dinodns 2>&1 | tee /tmp/loadtest-local.log || true

  echo ""
  echo "[3/3] Test complete. Full log: /tmp/loadtest-local.log"
  echo ""
  echo "To view results only:"
  echo "  grep -A 30 'DNSperf results' /tmp/loadtest-local.log"

  # Cleanup
  docker compose down --volumes 2>/dev/null || true
  exit 0
fi

# ===========================================================================
# AWS mode
# ===========================================================================
if [[ "${MODE}" == "aws" ]]; then
  # AWS-specific defaults
  AWS_REGION="${AWS_DEFAULT_REGION:-us-east-1}"
  DINODNS_CPU=1024
  DINODNS_MEMORY=2048
  DINODNS_COUNT=1
  COREDNS_CPU=1024
  COREDNS_MEMORY=2048
  COREDNS_COUNT=1
  DNSPERF_CPU=512
  DNSPERF_MEMORY=1024
  NO_DESTROY=""

  # Parse AWS-specific flags on top of shared ones
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --region)          AWS_REGION="$2";      shift 2 ;;
      --duration)        DURATION="$2";         shift 2 ;;
      --count)           DOMAIN_COUNT="$2";     shift 2 ;;
      --branch)          DINODNS_BRANCH="$2";   shift 2 ;;
      --cluster-mode)    CLUSTER_MODE="--dinodns-cluster-mode"; shift ;;
      --dinodns-cpu)     DINODNS_CPU="$2";      shift 2 ;;
      --dinodns-memory)  DINODNS_MEMORY="$2";   shift 2 ;;
      --dinodns-count)   DINODNS_COUNT="$2";    shift 2 ;;
      --coredns-cpu)     COREDNS_CPU="$2";      shift 2 ;;
      --coredns-memory)  COREDNS_MEMORY="$2";   shift 2 ;;
      --coredns-count)   COREDNS_COUNT="$2";    shift 2 ;;
      --dnsperf-cpu)     DNSPERF_CPU="$2";      shift 2 ;;
      --dnsperf-memory)  DNSPERF_MEMORY="$2";   shift 2 ;;
      --no-destroy)      NO_DESTROY="--no-destroy"; shift ;;
      --seed)            SEED="--seed $2";       shift 2 ;;
      *)
        echo "Unknown option: $1" >&2
        usage
        ;;
    esac
  done

  # Validate AWS credentials are present
  if ! aws sts get-caller-identity > /dev/null 2>&1; then
    echo "ERROR: AWS credentials not configured or not valid." >&2
    echo "       Set AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, and AWS_DEFAULT_REGION" >&2
    echo "       or configure ~/.aws/credentials." >&2
    exit 1
  fi

  echo ""
  echo "=================================================="
  echo "  DNS Load Test — AWS mode"
  echo "  Region : ${AWS_REGION}"
  echo "  Branch : ${DINODNS_BRANCH}"
  echo "  Duration: ${DURATION}s"
  echo "=================================================="
  echo "  Architecture:"
  echo "    DinoDNS tasks    → dedicated Fargate host(s)  (× ${DINODNS_COUNT})"
  echo "    CoreDNS tasks    → dedicated Fargate host(s)  (× ${COREDNS_COUNT})"
  echo "    DNSperf tasks    → dedicated Fargate host(s)  (× 2, one per server)"
  echo "    NLBs             → layer-4, pass-through routing between tiers"
  echo "=================================================="
  echo ""

  export AWS_DEFAULT_REGION="${AWS_REGION}"

  python3 orchestrate.py \
    --region          "${AWS_REGION}" \
    --dinodns-branch  "${DINODNS_BRANCH}" \
    --domain-count    "${DOMAIN_COUNT}" \
    --test-duration   "${DURATION}" \
    --dinodns-cpu     "${DINODNS_CPU}" \
    --dinodns-memory  "${DINODNS_MEMORY}" \
    --dinodns-count   "${DINODNS_COUNT}" \
    --coredns-cpu     "${COREDNS_CPU}" \
    --coredns-memory  "${COREDNS_MEMORY}" \
    --coredns-count   "${COREDNS_COUNT}" \
    --dnsperf-cpu     "${DNSPERF_CPU}" \
    --dnsperf-memory  "${DNSPERF_MEMORY}" \
    ${CLUSTER_MODE} \
    ${NO_DESTROY} \
    ${SEED}

  exit $?
fi

echo "Unknown mode: ${MODE}" >&2
usage
