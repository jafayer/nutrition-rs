#!/usr/bin/env python3
"""
generate.py — Generates test domain data for the DinoDNS load test.

Outputs:
  <output_dir>/domains.json      — 100 domain→IP mappings (for DNS servers)
  <output_dir>/queries.txt       — 200 dnsperf query lines (50% NOERROR, 50% NXDOMAIN)
  <output_dir>/Corefile          — CoreDNS configuration
  <output_dir>/zone.db           — CoreDNS zone file
  <output_dir>/server.js         — DinoDNS server entry-point (loads domains at boot)
"""

import argparse
import json
import os
import random
import string
import sys


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def random_label(length: int = 8) -> str:
    """Return a random lowercase alphanumeric label of the given length."""
    return "".join(random.choices(string.ascii_lowercase + string.digits, k=length))


def random_ip() -> str:
    """Return a random private-range IPv4 address (10.x.x.x)."""
    return f"10.{random.randint(0,255)}.{random.randint(0,255)}.{random.randint(1,254)}"


def generate_domains(count: int, tld: str) -> list[dict]:
    """Return a list of ``count`` unique domain records."""
    seen: set[str] = set()
    records = []
    while len(records) < count:
        label = random_label()
        name = f"{label}.{tld}"
        if name in seen:
            continue
        seen.add(name)
        records.append({"name": name, "type": "A", "value": random_ip()})
    return records


# ---------------------------------------------------------------------------
# Output generators
# ---------------------------------------------------------------------------

def write_domains_json(domains: list[dict], path: str) -> None:
    with open(path, "w") as fh:
        json.dump(domains, fh, indent=2)


def write_queries(domains: list[dict], tld: str, path: str) -> None:
    """
    Write a dnsperf query file: 100 valid domains + 100 random NXDOMAIN names.
    dnsperf query format: ``<fqdn>. <qtype>``
    """
    lines = [f"{d['name']}. A" for d in domains]

    # NXDOMAIN entries — randomly generated names guaranteed not in domains set
    valid_names = {d["name"] for d in domains}
    nxdomain_lines = []
    while len(nxdomain_lines) < len(domains):
        name = f"{random_label()}.{tld}"
        if name not in valid_names:
            nxdomain_lines.append(f"{name}. A")

    combined = lines + nxdomain_lines
    random.shuffle(combined)

    with open(path, "w") as fh:
        fh.write("\n".join(combined) + "\n")


def write_corefile(tld: str, output_dir: str) -> None:
    """Write a CoreDNS Corefile that serves the generated zone."""
    corefile = f"""\
{tld}. {{
    file /etc/coredns/zone.db {tld}.
    errors
    log
}}

. {{
    errors
}}
"""
    with open(os.path.join(output_dir, "Corefile"), "w") as fh:
        fh.write(corefile)


def write_zone_file(domains: list[dict], tld: str, output_dir: str) -> None:
    """Write a BIND-format zone file for CoreDNS."""
    lines = [
        f"$ORIGIN {tld}.",
        "$TTL 60",
        f"@\tIN\tSOA\tns1.{tld}. admin.{tld}. 1 3600 900 604800 60",
        f"@\tIN\tNS\tns1.{tld}.",
        f"ns1\tIN\tA\t127.0.0.1",
    ]
    for d in domains:
        label = d["name"].replace(f".{tld}", "")
        lines.append(f"{label}\tIN\tA\t{d['value']}")

    with open(os.path.join(output_dir, "zone.db"), "w") as fh:
        fh.write("\n".join(lines) + "\n")


def write_dinodns_server(
    domains: list[dict],
    port: int,
    cluster_mode: bool,
    output_dir: str,
) -> None:
    """
    Write a standalone Node.js server script that uses the compiled DinoDNS
    library (available at /app/dinodns/dist) to serve the generated domains.
    The script reads /config/domains.json at boot so it works both locally
    (volume-mounted) and in AWS (downloaded from S3 by the entrypoint).
    """
    multithreaded_str = "true" if cluster_mode else "false"
    script = f"""\
'use strict';
const fs = require('fs');
const path = require('path');

// DinoDNS built from source lives at /app/dinodns/dist
const dinodnsRoot = path.resolve(__dirname, 'dinodns', 'dist');
const {{ DinoDNS }} = require(dinodnsRoot);
const {{ DefaultStore }} = require(path.join(dinodnsRoot, 'plugins', 'storage', 'DefaultStore'));
const {{ DNSOverUDP, DNSOverTCP }} = require(path.join(dinodnsRoot, 'common'));

const DOMAINS_FILE = process.env.DOMAINS_FILE || '/config/domains.json';
const PORT = parseInt(process.env.DNS_PORT || '{port}', 10);
const CLUSTER_MODE = (process.env.CLUSTER_MODE || '{str(cluster_mode).lower()}') === 'true';

const domains = JSON.parse(fs.readFileSync(DOMAINS_FILE, 'utf8'));
const store = new DefaultStore();

for (const {{ name, type, value }} of domains) {{
  store.set(name, type, value);
}}

const server = new DinoDNS({{
  networks: [
    new DNSOverUDP({{ address: '0.0.0.0', port: PORT }}),
    new DNSOverTCP({{ address: '0.0.0.0', port: PORT }}),
  ],
  multithreaded: CLUSTER_MODE,
  storage: store,
}});

server.start(() => {{
  console.log(
    `DinoDNS started on port ${{PORT}} | cluster=${{CLUSTER_MODE}} | domains=${{domains.length}}`
  );
}});
"""
    with open(os.path.join(output_dir, "server.js"), "w") as fh:
        fh.write(script)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Generate DNS load-test configuration files."
    )
    p.add_argument(
        "--output-dir",
        default="./config",
        help="Directory to write generated files into (default: ./config)",
    )
    p.add_argument(
        "--tld",
        default="loadtest.internal",
        help="TLD / zone name to use for generated domains (default: loadtest.internal)",
    )
    p.add_argument(
        "--count",
        type=int,
        default=100,
        help="Number of valid domain records to generate (default: 100)",
    )
    p.add_argument(
        "--dns-port",
        type=int,
        default=53,
        help="Port the DinoDNS server will listen on (default: 53)",
    )
    p.add_argument(
        "--cluster-mode",
        action="store_true",
        default=False,
        help="Enable DinoDNS cluster (multi-threaded) mode",
    )
    p.add_argument(
        "--seed",
        type=int,
        default=None,
        help="Random seed for reproducible output",
    )
    return p.parse_args()


def main() -> None:
    args = parse_args()

    if args.seed is not None:
        random.seed(args.seed)

    os.makedirs(args.output_dir, exist_ok=True)

    print(f"Generating {args.count} domain records under .{args.tld} …")
    domains = generate_domains(args.count, args.tld)

    domains_path = os.path.join(args.output_dir, "domains.json")
    queries_path = os.path.join(args.output_dir, "queries.txt")

    write_domains_json(domains, domains_path)
    print(f"  ✓ {domains_path}")

    write_queries(domains, args.tld, queries_path)
    print(f"  ✓ {queries_path}  ({args.count} valid + {args.count} NXDOMAIN)")

    write_corefile(args.tld, args.output_dir)
    print(f"  ✓ {os.path.join(args.output_dir, 'Corefile')}")

    write_zone_file(domains, args.tld, args.output_dir)
    print(f"  ✓ {os.path.join(args.output_dir, 'zone.db')}")

    write_dinodns_server(
        domains,
        port=args.dns_port,
        cluster_mode=args.cluster_mode,
        output_dir=args.output_dir,
    )
    print(f"  ✓ {os.path.join(args.output_dir, 'server.js')}")

    print("\nDone! Files written to:", os.path.abspath(args.output_dir))
    print(f"\nDomain sample:")
    for d in domains[:3]:
        print(f"  {d['name']} A {d['value']}")
    print(f"  … and {len(domains) - 3} more")


if __name__ == "__main__":
    main()
