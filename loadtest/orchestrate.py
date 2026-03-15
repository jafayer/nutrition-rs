#!/usr/bin/env python3
"""
orchestrate.py — AWS load-test orchestrator for DinoDNS vs CoreDNS.

Steps:
  1. terraform init + apply  (provisions VPC, ECS, ECR, S3, NLBs, IAM)
  2. docker build + ECR push  (DinoDNS, CoreDNS, DNSperf images)
  3. generate.py              (creates domain/config files)
  4. S3 upload                (uploads config files)
  5. Wait for ECS DNS services to become healthy
  6. aws ecs run-task         (starts DNSperf tasks on separate Fargate hosts)
  7. Wait for DNSperf tasks to finish
  8. Collect results from CloudWatch Logs (and S3)
  9. Print results to stdout
 10. terraform destroy        (tear down all resources)

Usage:
    python3 orchestrate.py [options]
    python3 orchestrate.py --help

Requirements:
    pip install boto3
    AWS credentials configured (env vars, ~/.aws, or instance profile)
    docker, terraform CLIs on PATH
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

LOADTEST_DIR = Path(__file__).parent.resolve()
TF_DIR = LOADTEST_DIR / "terraform"


def run(cmd: list[str], check: bool = True, capture: bool = False, **kwargs) -> subprocess.CompletedProcess:
    """Run a subprocess, streaming output unless capture=True."""
    print(f"  $ {' '.join(str(c) for c in cmd)}", flush=True)
    if capture:
        return subprocess.run(cmd, capture_output=True, text=True, check=check, **kwargs)
    return subprocess.run(cmd, check=check, **kwargs)


def terraform_output(tf_dir: Path) -> dict:
    """Return all Terraform outputs as a Python dict."""
    result = run(["terraform", "-chdir", str(tf_dir), "output", "-json"], capture=True)
    raw = json.loads(result.stdout)
    return {k: v["value"] for k, v in raw.items()}


def ecr_login(region: str, account_id: str) -> str:
    """Log in to ECR and return the registry hostname."""
    registry = f"{account_id}.dkr.ecr.{region}.amazonaws.com"
    token = run(
        ["aws", "ecr", "get-login-password", "--region", region],
        capture=True,
    ).stdout.strip()
    run(["docker", "login", "--username", "AWS", "--password-stdin", registry],
        input=token.encode())
    return registry


def build_and_push(context: Path, ecr_url: str, build_args: dict | None = None) -> None:
    """Build a Docker image and push it to ECR."""
    tag = f"{ecr_url}:latest"
    cmd = ["docker", "build", "-t", tag, str(context)]
    if build_args:
        for k, v in build_args.items():
            cmd += ["--build-arg", f"{k}={v}"]
    run(cmd)
    run(["docker", "push", tag])


def upload_config(bucket: str, config_dir: Path, prefix: str = "loadtest/config") -> None:
    """Upload all generated config files to S3."""
    import boto3
    s3 = boto3.client("s3")
    for f in config_dir.iterdir():
        if f.is_file():
            key = f"{prefix}/{f.name}"
            print(f"  Uploading {f.name} → s3://{bucket}/{key}")
            s3.upload_file(str(f), bucket, key)


def wait_for_service_healthy(
    ecs_client,
    cluster: str,
    service: str,
    timeout: int = 300,
) -> None:
    """Poll until all desired tasks in an ECS service are running."""
    print(f"  Waiting for service '{service}' to be healthy …", flush=True)
    deadline = time.time() + timeout
    while time.time() < deadline:
        resp = ecs_client.describe_services(cluster=cluster, services=[service])
        svc = resp["services"][0]
        running = svc.get("runningCount", 0)
        desired = svc.get("desiredCount", 0)
        if desired > 0 and running >= desired:
            print(f"  ✓ {service}: {running}/{desired} tasks running")
            return
        time.sleep(10)
    raise TimeoutError(f"Service '{service}' did not reach desired count within {timeout}s")


def start_task(
    ecs_client,
    cluster: str,
    task_definition: str,
    subnets: list[str],
    security_groups: list[str],
) -> str:
    """Start a run-to-completion ECS task (non-blocking). Returns task ARN."""
    resp = ecs_client.run_task(
        cluster=cluster,
        taskDefinition=task_definition,
        launchType="FARGATE",
        networkConfiguration={
            "awsvpcConfiguration": {
                "subnets": subnets,
                "securityGroups": security_groups,
                "assignPublicIp": "ENABLED",
            }
        },
    )
    failures = resp.get("failures", [])
    if failures:
        raise RuntimeError(f"ECS run-task failures: {failures}")

    task_arn = resp["tasks"][0]["taskArn"]
    print(f"  Task started: {task_arn.split('/')[-1]}", flush=True)
    return task_arn


def wait_for_tasks(
    ecs_client,
    cluster: str,
    task_arns: list[str],
    timeout: int = 600,
) -> None:
    """Wait for all listed ECS tasks to reach the STOPPED state."""
    print(f"  Waiting for {len(task_arns)} task(s) to finish (timeout={timeout}s) …")
    waiter = ecs_client.get_waiter("tasks_stopped")
    waiter.wait(
        cluster=cluster,
        tasks=task_arns,
        WaiterConfig={"Delay": 10, "MaxAttempts": timeout // 10},
    )
    print(f"  ✓ All {len(task_arns)} task(s) stopped")


def fetch_logs(
    logs_client,
    log_group: str,
    task_arn: str,
    container: str = "dnsperf",
) -> str:
    """Retrieve CloudWatch log events for a finished task."""
    task_id = task_arn.split("/")[-1]
    log_stream = f"ecs/{container}/{task_id}"

    lines = []
    kwargs: dict = {"logGroupName": log_group, "logStreamName": log_stream, "startFromHead": True}
    while True:
        try:
            resp = logs_client.get_log_events(**kwargs)
        except logs_client.exceptions.ResourceNotFoundException:
            return f"(no log stream found: {log_stream})"
        for ev in resp.get("events", []):
            lines.append(ev["message"])
        next_token = resp.get("nextForwardToken")
        if not next_token or next_token == kwargs.get("nextToken"):
            break
        kwargs["nextToken"] = next_token

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="AWS DNS load-test orchestrator.")
    p.add_argument("--region",          default=os.environ.get("AWS_DEFAULT_REGION", "us-east-1"))
    p.add_argument("--dinodns-branch",  default="main",  help="DinoDNS git branch to build")
    p.add_argument("--dinodns-repo",    default="https://github.com/jafayer/DinoDNS.git")
    p.add_argument("--domain-count",    type=int, default=100)
    p.add_argument("--domain-tld",      default="loadtest.internal")
    p.add_argument("--test-duration",   type=int, default=30, help="dnsperf run time in seconds")
    p.add_argument("--dinodns-cpu",     type=int, default=1024)
    p.add_argument("--dinodns-memory",  type=int, default=2048)
    p.add_argument("--dinodns-count",   type=int, default=1, help="DinoDNS replica count")
    p.add_argument("--dinodns-cluster-mode", action="store_true", default=False)
    p.add_argument("--coredns-cpu",     type=int, default=1024)
    p.add_argument("--coredns-memory",  type=int, default=2048)
    p.add_argument("--coredns-count",   type=int, default=1, help="CoreDNS replica count")
    p.add_argument("--dnsperf-cpu",     type=int, default=512)
    p.add_argument("--dnsperf-memory",  type=int, default=1024)
    p.add_argument("--dnsperf-extra-args", default="")
    p.add_argument("--no-destroy",      action="store_true",
                   help="Skip terraform destroy after the test (for debugging)")
    p.add_argument("--seed",            type=int, default=None)
    return p.parse_args()


def main() -> None:
    args = parse_args()

    import boto3

    print("\n" + "=" * 60)
    print("  DNS Load Test — AWS Orchestrator")
    print("=" * 60 + "\n")

    # ------------------------------------------------------------------ #
    # 1. Terraform apply                                                   #
    # ------------------------------------------------------------------ #
    print("[1/10] Provisioning infrastructure with Terraform …")
    run(["terraform", "-chdir", str(TF_DIR), "init", "-upgrade"])

    tf_vars = [
        f"-var=aws_region={args.region}",
        f"-var=dinodns_branch={args.dinodns_branch}",
        f"-var=dinodns_repo={args.dinodns_repo}",
        f"-var=domain_count={args.domain_count}",
        f"-var=domain_tld={args.domain_tld}",
        f"-var=test_duration={args.test_duration}",
        f"-var=dinodns_cpu={args.dinodns_cpu}",
        f"-var=dinodns_memory={args.dinodns_memory}",
        f"-var=dinodns_count={args.dinodns_count}",
        f"-var=dinodns_cluster_mode={str(args.dinodns_cluster_mode).lower()}",
        f"-var=coredns_cpu={args.coredns_cpu}",
        f"-var=coredns_memory={args.coredns_memory}",
        f"-var=coredns_count={args.coredns_count}",
        f"-var=dnsperf_cpu={args.dnsperf_cpu}",
        f"-var=dnsperf_memory={args.dnsperf_memory}",
        f"-var=dnsperf_extra_args={args.dnsperf_extra_args}",
    ]
    run(["terraform", "-chdir", str(TF_DIR), "apply", "-auto-approve"] + tf_vars)

    outputs = terraform_output(TF_DIR)

    region = args.region
    ecr_dinodns = outputs["ecr_dinodns_url"]
    ecr_coredns = outputs["ecr_coredns_url"]
    ecr_dnsperf = outputs["ecr_dnsperf_url"]
    bucket      = outputs["s3_bucket"]
    cluster     = outputs["ecs_cluster_name"]
    log_group   = outputs["cloudwatch_log_group"]
    td_dnsperf_dinodns = outputs["ecs_task_definition_dnsperf_dinodns"]
    td_dnsperf_coredns = outputs["ecs_task_definition_dnsperf_coredns"]
    subnets     = outputs["public_subnet_ids"]
    sg_dnsperf  = outputs["security_group_dnsperf"]

    # Derive AWS account ID from ECR URL  (format: <account>.dkr.ecr…)
    account_id = ecr_dinodns.split(".")[0]

    # ------------------------------------------------------------------ #
    # 2. ECR login + build/push images                                    #
    # ------------------------------------------------------------------ #
    print("\n[2/10] Building and pushing Docker images to ECR …")
    ecr_login(region, account_id)

    build_and_push(
        LOADTEST_DIR / "dinodns",
        ecr_dinodns,
        build_args={"DINODNS_REPO": args.dinodns_repo, "DINODNS_BRANCH": args.dinodns_branch},
    )
    build_and_push(LOADTEST_DIR / "coredns", ecr_coredns)
    build_and_push(LOADTEST_DIR / "dnsperf", ecr_dnsperf)

    # ------------------------------------------------------------------ #
    # 3. Generate config files                                            #
    # ------------------------------------------------------------------ #
    print("\n[3/10] Generating domain/config files …")
    config_dir = LOADTEST_DIR / "config"
    config_dir.mkdir(exist_ok=True)

    gen_cmd = [
        sys.executable, str(LOADTEST_DIR / "generate.py"),
        "--output-dir", str(config_dir),
        "--tld",   args.domain_tld,
        "--count", str(args.domain_count),
    ]
    if args.dinodns_cluster_mode:
        gen_cmd.append("--cluster-mode")
    if args.seed is not None:
        gen_cmd += ["--seed", str(args.seed)]
    run(gen_cmd)

    # ------------------------------------------------------------------ #
    # 4. Upload config to S3                                              #
    # ------------------------------------------------------------------ #
    print(f"\n[4/10] Uploading config to s3://{bucket}/loadtest/config/ …")
    upload_config(bucket, config_dir)

    # ------------------------------------------------------------------ #
    # 5. Wait for ECS DNS services to be healthy                          #
    # ------------------------------------------------------------------ #
    print("\n[5/10] Waiting for DNS services to be healthy …")
    ecs = boto3.client("ecs", region_name=region)
    svc_dinodns = outputs["ecs_service_dinodns"]
    svc_coredns = outputs["ecs_service_coredns"]
    wait_for_service_healthy(ecs, cluster, svc_dinodns, timeout=300)
    wait_for_service_healthy(ecs, cluster, svc_coredns, timeout=300)

    # Give DNS servers 10 extra seconds to fully initialise their listeners
    print("  Sleeping 10 s for server warm-up …")
    time.sleep(10)

    # ------------------------------------------------------------------ #
    # 6. Launch both DNSperf tasks simultaneously on separate hosts       #
    # ------------------------------------------------------------------ #
    print("\n[6/10] Launching DNSperf tasks (separate Fargate hosts) …")
    sg_list = [sg_dnsperf]
    task_arn_dinodns = start_task(ecs, cluster, td_dnsperf_dinodns, subnets, sg_list)
    task_arn_coredns = start_task(ecs, cluster, td_dnsperf_coredns, subnets, sg_list)

    # ------------------------------------------------------------------ #
    # 7. Wait for both DNSperf tasks to finish                            #
    # ------------------------------------------------------------------ #
    print("\n[7/10] Waiting for DNSperf tasks to complete …")
    wait_for_tasks(ecs, cluster, [task_arn_dinodns, task_arn_coredns],
                   timeout=args.test_duration + 180)

    # ------------------------------------------------------------------ #
    # 8. Collect results from CloudWatch Logs                             #
    # ------------------------------------------------------------------ #
    print("\n[8/10] Collecting results from CloudWatch …")
    time.sleep(5)  # Let CloudWatch catch up

    logs = boto3.client("logs", region_name=region)
    results_dinodns = fetch_logs(logs, log_group, task_arn_dinodns)
    results_coredns = fetch_logs(logs, log_group, task_arn_coredns)

    # ------------------------------------------------------------------ #
    # 9. Print results                                                    #
    # ------------------------------------------------------------------ #
    print("\n[9/10] Results:")
    print("\n" + "=" * 60)
    print("  RESULTS — DinoDNS")
    print("=" * 60)
    print(results_dinodns)

    print("\n" + "=" * 60)
    print("  RESULTS — CoreDNS")
    print("=" * 60)
    print(results_coredns)

    # ------------------------------------------------------------------ #
    # 10. Terraform destroy                                               #
    # ------------------------------------------------------------------ #
    if args.no_destroy:
        print("\n[--no-destroy] Skipping terraform destroy. Remember to clean up!")
    else:
        print("\n[10/10] Destroying infrastructure …")
        run(["terraform", "-chdir", str(TF_DIR), "destroy", "-auto-approve"] + tf_vars)
        print("  ✓ All resources destroyed.")

    print("\n✅ Load test complete.\n")


if __name__ == "__main__":
    main()
