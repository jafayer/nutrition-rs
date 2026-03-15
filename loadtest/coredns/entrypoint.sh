#!/bin/sh
# CoreDNS container entrypoint.
# 1. Optionally downloads Corefile + zone.db from S3.
# 2. Starts CoreDNS.
set -e

CONFIG_DIR="/etc/coredns"
COREFILE="${CONFIG_DIR}/Corefile"
ZONE_FILE="${CONFIG_DIR}/zone.db"

# ------------------------------------------------------------------
# Download config from S3 when running in AWS
# ------------------------------------------------------------------
if [ -n "${S3_CONFIG_BUCKET}" ]; then
  echo "[coredns] Downloading config from s3://${S3_CONFIG_BUCKET}/${S3_CONFIG_PREFIX}/ …"
  mkdir -p "${CONFIG_DIR}"
  aws s3 cp "s3://${S3_CONFIG_BUCKET}/${S3_CONFIG_PREFIX}/Corefile" \
      "${COREFILE}"
  aws s3 cp "s3://${S3_CONFIG_BUCKET}/${S3_CONFIG_PREFIX}/zone.db" \
      "${ZONE_FILE}"
fi

# ------------------------------------------------------------------
# Validate required files
# ------------------------------------------------------------------
if [ ! -f "${COREFILE}" ]; then
  echo "[coredns] ERROR: ${COREFILE} not found." >&2
  echo "          Mount a config volume or set S3_CONFIG_BUCKET." >&2
  exit 1
fi

echo "[coredns] Starting CoreDNS …"
exec /coredns -conf "${COREFILE}"
