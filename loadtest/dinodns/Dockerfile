# syntax=docker/dockerfile:1
# --------------------------------------------------------------------------
# DinoDNS load-test image
#
# Build args:
#   DINODNS_REPO   — Git repository URL   (default: jafayer/DinoDNS on GitHub)
#   DINODNS_BRANCH — Branch / tag to clone (default: main)
#
# The image clones & builds DinoDNS from source, then runs a generated
# server.js that loads domain records from /config/domains.json at boot.
# --------------------------------------------------------------------------

FROM node:20-alpine AS builder

ARG DINODNS_REPO=https://github.com/jafayer/DinoDNS.git
ARG DINODNS_BRANCH=main

RUN apk add --no-cache git

WORKDIR /build/dinodns

RUN git clone --depth 1 --branch "${DINODNS_BRANCH}" "${DINODNS_REPO}" .

RUN npm ci --ignore-scripts \
 && npm run build

# Remove dev-only modules to keep the final image lean
RUN npm prune --omit=dev


# --------------------------------------------------------------------------
FROM node:20-alpine AS runtime

# bind-tools provides nslookup/dig for the Docker healthcheck
RUN apk add --no-cache bind-tools

WORKDIR /app

# Copy the compiled DinoDNS library and its production dependencies
COPY --from=builder /build/dinodns/dist        /app/dinodns/dist
COPY --from=builder /build/dinodns/node_modules /app/dinodns/node_modules
COPY --from=builder /build/dinodns/package.json /app/dinodns/package.json

# Copy the entrypoint and server template
COPY entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

# /config is where domains.json (and optionally server.js) are expected.
# In local mode it is volume-mounted; in AWS it is populated by the entrypoint
# after downloading from S3.
VOLUME ["/config"]

EXPOSE 53/udp
EXPOSE 53/tcp

ENV DNS_PORT=53 \
    CLUSTER_MODE=false \
    DOMAINS_FILE=/config/domains.json \
    S3_CONFIG_BUCKET="" \
    S3_CONFIG_PREFIX="loadtest/config"

ENTRYPOINT ["/app/entrypoint.sh"]
