#!/bin/sh
# DNSperf container entrypoint.
# 1. Optionally downloads queries.txt from S3.
# 2. Waits for the target DNS server to be ready.
# 3. Runs dnsperf for the configured duration.
# 4. Optionally uploads results to S3.
set -e

CONFIG_DIR="/config"
QUERIES_FILE="${CONFIG_DIR}/queries.txt"
RESULTS_FILE="/tmp/dnsperf_results_${SERVER_LABEL}.txt"

# ------------------------------------------------------------------
# Download config from S3 when running in AWS
# ------------------------------------------------------------------
if [ -n "${S3_CONFIG_BUCKET}" ]; then
  echo "[dnsperf-${SERVER_LABEL}] Downloading queries from s3://${S3_CONFIG_BUCKET}/${S3_CONFIG_PREFIX}/ …"
  aws s3 cp "s3://${S3_CONFIG_BUCKET}/${S3_CONFIG_PREFIX}/queries.txt" \
      "${QUERIES_FILE}"
fi

# ------------------------------------------------------------------
# Validate required files
# ------------------------------------------------------------------
if [ ! -f "${QUERIES_FILE}" ]; then
  echo "[dnsperf-${SERVER_LABEL}] ERROR: ${QUERIES_FILE} not found." >&2
  echo "          Mount a config volume or set S3_CONFIG_BUCKET." >&2
  exit 1
fi

# ------------------------------------------------------------------
# Wait for DNS server to become ready (max 60 s)
# ------------------------------------------------------------------
echo "[dnsperf-${SERVER_LABEL}] Waiting for DNS server at ${DNS_SERVER}:${DNS_PORT} …"
WAIT_RETRIES=60
while [ "${WAIT_RETRIES}" -gt 0 ]; do
  if dnsperf -s "${DNS_SERVER}" -p "${DNS_PORT}" -d "${QUERIES_FILE}" \
       -l 1 -q 1 -c 1 > /dev/null 2>&1; then
    echo "[dnsperf-${SERVER_LABEL}] DNS server is ready."
    break
  fi
  WAIT_RETRIES=$((WAIT_RETRIES - 1))
  sleep 1
done

if [ "${WAIT_RETRIES}" -eq 0 ]; then
  echo "[dnsperf-${SERVER_LABEL}] ERROR: DNS server did not become ready in 60 s." >&2
  exit 1
fi

# ------------------------------------------------------------------
# Run the performance test
# ------------------------------------------------------------------
echo "[dnsperf-${SERVER_LABEL}] Running test: server=${DNS_SERVER}:${DNS_PORT}, duration=${TEST_DURATION}s …"

# shellcheck disable=SC2086
dnsperf \
  -s "${DNS_SERVER}" \
  -p "${DNS_PORT}" \
  -d "${QUERIES_FILE}" \
  -l "${TEST_DURATION}" \
  ${DNSPERF_EXTRA_ARGS} \
  | tee "${RESULTS_FILE}"

echo ""
echo "=========================================="
echo "  DNSperf results: ${SERVER_LABEL}"
echo "=========================================="
cat "${RESULTS_FILE}"

# ------------------------------------------------------------------
# Upload results to S3 (optional)
# ------------------------------------------------------------------
if [ -n "${S3_RESULTS_BUCKET}" ]; then
  TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
  RESULT_KEY="${S3_RESULTS_PREFIX}/${SERVER_LABEL}_${TIMESTAMP}.txt"
  echo "[dnsperf-${SERVER_LABEL}] Uploading results to s3://${S3_RESULTS_BUCKET}/${RESULT_KEY} …"
  aws s3 cp "${RESULTS_FILE}" "s3://${S3_RESULTS_BUCKET}/${RESULT_KEY}"
  echo "[dnsperf-${SERVER_LABEL}] Results uploaded."
fi
