#!/bin/sh
# DinoDNS container entrypoint.
# 1. Optionally downloads config files from S3.
# 2. Runs the DinoDNS server (server.js must be present in /config or /app).
set -e

CONFIG_DIR="/config"
SERVER_JS="${CONFIG_DIR}/server.js"

# ------------------------------------------------------------------
# Download config from S3 when running in AWS
# ------------------------------------------------------------------
if [ -n "${S3_CONFIG_BUCKET}" ]; then
  echo "[dinodns] Downloading config from s3://${S3_CONFIG_BUCKET}/${S3_CONFIG_PREFIX}/ …"
  aws s3 cp "s3://${S3_CONFIG_BUCKET}/${S3_CONFIG_PREFIX}/domains.json" \
      "${CONFIG_DIR}/domains.json"
  aws s3 cp "s3://${S3_CONFIG_BUCKET}/${S3_CONFIG_PREFIX}/server.js" \
      "${SERVER_JS}"
fi

# ------------------------------------------------------------------
# Validate required files
# ------------------------------------------------------------------
if [ ! -f "${CONFIG_DIR}/domains.json" ]; then
  echo "[dinodns] ERROR: ${CONFIG_DIR}/domains.json not found." >&2
  echo "          Mount a config volume or set S3_CONFIG_BUCKET." >&2
  exit 1
fi

if [ ! -f "${SERVER_JS}" ]; then
  echo "[dinodns] ERROR: ${SERVER_JS} not found." >&2
  echo "          Generate it with generate.py and mount/upload the config." >&2
  exit 1
fi

echo "[dinodns] Starting server (cluster_mode=${CLUSTER_MODE}, port=${DNS_PORT}) …"
exec node "${SERVER_JS}"
