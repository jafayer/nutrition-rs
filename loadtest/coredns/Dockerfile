# syntax=docker/dockerfile:1
# --------------------------------------------------------------------------
# CoreDNS load-test image
#
# Based on the official CoreDNS image. Adds aws-cli so that the entrypoint
# can download generated config files from S3 when running in AWS.
# --------------------------------------------------------------------------

FROM coredns/coredns:latest AS coredns-bin

FROM debian:bookworm-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
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

COPY --from=coredns-bin /coredns /coredns

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

VOLUME ["/etc/coredns"]

EXPOSE 53/udp
EXPOSE 53/tcp

ENV S3_CONFIG_BUCKET="" \
    S3_CONFIG_PREFIX="loadtest/config"

ENTRYPOINT ["/entrypoint.sh"]
