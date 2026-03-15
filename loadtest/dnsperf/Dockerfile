# syntax=docker/dockerfile:1
# --------------------------------------------------------------------------
# DNSperf load-test image
#
# Installs dnsperf from source (nominum/dnsperf) for a current build, then
# runs the performance test against the configured target DNS server.
# --------------------------------------------------------------------------

FROM debian:bookworm-slim AS builder

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential \
      git \
      libssl-dev \
      libldns-dev \
      libfstrm-dev \
      libprotobuf-c-dev \
      libck-dev \
      pkg-config \
      python3 \
      python3-pip \
      curl \
      unzip \
 && rm -rf /var/lib/apt/lists/*

# Install dnsperf from package (simpler and well-maintained on Debian)
RUN apt-get update \
 && apt-get install -y --no-install-recommends dnsperf \
 && rm -rf /var/lib/apt/lists/*

# --------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Re-install dnsperf runtime (it comes with its own libraries)
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      dnsperf \
      ca-certificates \
      curl \
      unzip \
 && curl -fsSL "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" \
      -o /tmp/awscliv2.zip \
 && unzip /tmp/awscliv2.zip -d /tmp/awscli \
 && /tmp/awscli/aws/install \
 && rm -rf /tmp/awscliv2.zip /tmp/awscli \
 && apt-get remove -y curl unzip \
 && apt-get autoremove -y \
 && rm -rf /var/lib/apt/lists/*

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

VOLUME ["/config"]

# Required environment variables:
#   DNS_SERVER      — IP/hostname of the target DNS server
#   DNS_PORT        — Port of the target DNS server (default: 53)
#   TEST_DURATION   — Test duration in seconds (default: 30)
#
# Optional:
#   DNSPERF_EXTRA_ARGS — Additional dnsperf arguments
#   S3_CONFIG_BUCKET   — S3 bucket to download queries.txt from
#   S3_CONFIG_PREFIX   — S3 prefix (default: loadtest/config)
#   S3_RESULTS_BUCKET  — S3 bucket to upload results to
#   S3_RESULTS_PREFIX  — S3 prefix for results (default: loadtest/results)
#   SERVER_LABEL       — Label for result files (e.g. "dinodns" or "coredns")
ENV DNS_SERVER="127.0.0.1" \
    DNS_PORT=53 \
    TEST_DURATION=30 \
    DNSPERF_EXTRA_ARGS="" \
    S3_CONFIG_BUCKET="" \
    S3_CONFIG_PREFIX="loadtest/config" \
    S3_RESULTS_BUCKET="" \
    S3_RESULTS_PREFIX="loadtest/results" \
    SERVER_LABEL="dns"

ENTRYPOINT ["/entrypoint.sh"]
