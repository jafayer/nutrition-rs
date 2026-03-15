# DNS Load Test

Ephemeral, configurable load-test harness that benchmarks **DinoDNS** against **CoreDNS** using [DNSperf](https://www.dns-oarc.net/tools/dnsperf).

Two modes are supported:

| Mode | Infrastructure | Prerequisites |
|------|---------------|---------------|
| **local** | Docker Compose on your machine | `docker`, `docker compose`, `python3` |
| **aws**   | AWS ECS Fargate (ephemeral, auto-destroyed) | All local deps + `aws` CLI, `terraform`, `python3 boto3` |

---

## Architecture

```
 ┌─────────────────────────────────────────────────────────────────┐
 │  VPC (10.10.0.0/16)   —   same AZ for minimum latency          │
 │                                                                  │
 │  ┌───────────────────┐    NLB (L4)    ┌──────────────────────┐ │
 │  │  DNSperf          │ ─────────────▶ │  DinoDNS             │ │
 │  │  (Fargate task A) │                │  (Fargate task B × N)│ │
 │  └───────────────────┘                └──────────────────────┘ │
 │                                                                  │
 │  ┌───────────────────┐    NLB (L4)    ┌──────────────────────┐ │
 │  │  DNSperf          │ ─────────────▶ │  CoreDNS             │ │
 │  │  (Fargate task C) │                │  (Fargate task D × N)│ │
 │  └───────────────────┘                └──────────────────────┘ │
 └─────────────────────────────────────────────────────────────────┘
```

**Key isolation guarantee**: DNSperf and DNS servers always run on **separate Fargate hosts** (separate ECS tasks with dedicated CPU/memory). They communicate through Layer-4 Network Load Balancers, which are pass-through (no connection termination) and add negligible latency. This ensures CPU or memory pressure from one process can never bottleneck the other.

In **local mode** each service runs in its own container with hard CPU/memory limits enforced by Docker's cgroup integration — the same isolation principle, realised through container resource limits.

---

## Test design

- **100 random A-record domains** are generated (e.g. `xk7f2lqp.loadtest.internal → 10.42.17.99`).
- **DinoDNS** loads all 100 records into its in-memory store at boot.
- **CoreDNS** receives a generated zone file containing all 100 records.
- **DNSperf** receives a query file of **200 lines**:
  - 100 valid domains → expected `NOERROR`
  - 100 randomly generated names guaranteed not to exist → expected `NXDOMAIN`
  - The two sets are shuffled together (50% NOERROR / 50% NXDOMAIN).
- Both DNSperf instances run **simultaneously** for a configurable duration.

---

## Quick start — local

```bash
cd loadtest

# Step 1: generate config (creates loadtest/config/)
python3 generate.py --output-dir ./config

# Step 2: run (builds images, starts services, prints results, tears down)
./run.sh local --duration 30
```

Full local options:

```text
./run.sh local
    [--duration <seconds>]   # dnsperf run time          (default: 30)
    [--count <n>]            # number of valid domains   (default: 100)
    [--branch <git-branch>]  # DinoDNS branch to build  (default: main)
    [--cluster-mode]         # enable DinoDNS multithreading
    [--seed <n>]             # reproducible domain names
    [-- <extra dnsperf args>]
```

### Local resource limits (docker compose)

Override via environment variables or a `.env` file:

| Variable | Default | Meaning |
|---|---|---|
| `DINODNS_CPUS` | `1.0` | CPU cores for DinoDNS container |
| `DINODNS_MEMORY` | `512m` | Memory for DinoDNS container |
| `COREDNS_CPUS` | `1.0` | CPU cores for CoreDNS container |
| `COREDNS_MEMORY` | `512m` | Memory for CoreDNS container |
| `DNSPERF_CPUS` | `0.5` | CPU cores per DNSperf container |
| `DNSPERF_MEMORY` | `256m` | Memory per DNSperf container |

---

## Quick start — AWS

```bash
cd loadtest

# Credentials: export AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_DEFAULT_REGION
# or configure ~/.aws/credentials

./run.sh aws --region us-east-1 --duration 60
```

The script will:
1. Run `terraform apply` — provisions VPC, ECS cluster, ECR repos, NLBs, S3, IAM roles
2. Build and push DinoDNS (from source), CoreDNS, and DNSperf images to ECR
3. Generate domain data and upload to S3
4. Wait for DNS server ECS services to become healthy
5. Launch DNSperf tasks (on separate Fargate hosts from DNS servers)
6. Wait for DNSperf to finish, collect results from CloudWatch Logs
7. Print results to stdout
8. Run `terraform destroy` to delete **all** resources

Full AWS options:

```text
./run.sh aws
    [--region <aws-region>]         (default: us-east-1)
    [--duration <seconds>]          (default: 30)
    [--count <n>]                   (default: 100)
    [--branch <git-branch>]         DinoDNS git branch  (default: main)

    # DinoDNS scaling
    [--dinodns-cpu <units>]         1 vCPU = 1024       (default: 1024)
    [--dinodns-memory <mib>]                             (default: 2048)
    [--dinodns-count <n>]           horizontal replicas (default: 1)
    [--cluster-mode]                enable Node.js cluster / multithreaded mode

    # CoreDNS scaling
    [--coredns-cpu <units>]                              (default: 1024)
    [--coredns-memory <mib>]                             (default: 2048)
    [--coredns-count <n>]                                (default: 1)

    # DNSperf
    [--dnsperf-cpu <units>]                              (default: 512)
    [--dnsperf-memory <mib>]                             (default: 1024)

    [--no-destroy]                  skip terraform destroy (for debugging)
    [--seed <n>]                    reproducible domain names
```

### Scaling examples

```bash
# Vertical: give DinoDNS 4 vCPU / 8 GB
./run.sh aws --dinodns-cpu 4096 --dinodns-memory 8192

# Horizontal: run 3 DinoDNS replicas behind the NLB
./run.sh aws --dinodns-count 3

# Vertical + cluster mode (multi-threaded Node.js within the task)
./run.sh aws --dinodns-cpu 4096 --dinodns-memory 8192 --cluster-mode

# Longer test, more domains
./run.sh aws --duration 120 --count 500
```

---

## File layout

```
loadtest/
├── run.sh                  # Main entry point
├── generate.py             # Domain + config file generator
├── orchestrate.py          # AWS orchestration (build→deploy→test→destroy)
├── docker-compose.yml      # Local test stack
│
├── dinodns/
│   ├── Dockerfile          # Clones & builds DinoDNS from source
│   └── entrypoint.sh       # Downloads config from S3 (AWS) or uses volume (local)
│
├── coredns/
│   ├── Dockerfile          # Extends official CoreDNS image with aws-cli
│   └── entrypoint.sh
│
├── dnsperf/
│   ├── Dockerfile          # Installs dnsperf from apt
│   └── entrypoint.sh       # Waits for server, runs test, uploads results
│
└── terraform/
    ├── main.tf             # Provider, random suffix
    ├── variables.tf        # All tunable parameters with defaults
    ├── outputs.tf          # ECR URLs, NLB names, cluster/service names, …
    ├── vpc.tf              # VPC, subnets, security groups
    ├── ecr.tf              # ECR repositories (DinoDNS, CoreDNS, DNSperf)
    ├── s3.tf               # S3 bucket (config upload + results)
    ├── iam.tf              # ECS execution + task roles
    ├── nlb.tf              # Layer-4 NLBs (one per DNS server)
    └── ecs.tf              # Cluster, task definitions, ECS services
```

---

## How results are collected

- **Local mode**: DNSperf output is streamed to stdout and saved in `/tmp/loadtest-local.log`.
- **AWS mode**: ECS tasks write to CloudWatch Logs (`/ecs/<stack-name>`). The orchestration script fetches and prints the logs after the tasks finish. DNSperf also uploads a result file to `s3://<bucket>/loadtest/results/`.

---

## Cleanup

- **Local**: `docker compose down --volumes` (run automatically by `run.sh local`)
- **AWS**: `terraform destroy` (run automatically by `run.sh aws` unless `--no-destroy` is set)

To force-destroy a stuck AWS stack:
```bash
cd loadtest/terraform
terraform destroy -auto-approve
```
