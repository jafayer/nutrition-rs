###############################################################################
# loadtest/terraform/ecs.tf
#
# ECS cluster, task definitions, and services for:
#   • DinoDNS (one task per replica — supports horizontal + vertical scaling)
#   • CoreDNS  (one task per replica)
#   • DNSperf→DinoDNS (one task; runs the test and exits)
#   • DNSperf→CoreDNS (one task; runs the test and exits)
#
# Each DNSperf task is co-located in the same Fargate task as the DNS server
# it tests (multi-container task) so they share the same network namespace
# (127.0.0.1) — eliminating cross-task network latency entirely.
###############################################################################

data "aws_caller_identity" "current" {}

# ----------------------------------------------------------
# CloudWatch log group
# ----------------------------------------------------------

resource "aws_cloudwatch_log_group" "loadtest" {
  name              = "/ecs/${local.name_prefix}"
  retention_in_days = 7
}

# ----------------------------------------------------------
# ECS Cluster
# ----------------------------------------------------------

resource "aws_ecs_cluster" "main" {
  name = local.name_prefix

  setting {
    name  = "containerInsights"
    value = "enabled"
  }

  tags = { Name = "${local.name_prefix}-cluster" }
}

resource "aws_ecs_cluster_capacity_providers" "fargate" {
  cluster_name       = aws_ecs_cluster.main.name
  capacity_providers = ["FARGATE"]

  default_capacity_provider_strategy {
    capacity_provider = "FARGATE"
    weight            = 1
  }
}

# ----------------------------------------------------------
# Local helpers
# ----------------------------------------------------------

locals {
  log_config = {
    logDriver = "awslogs"
    options = {
      "awslogs-group"         = aws_cloudwatch_log_group.loadtest.name
      "awslogs-region"        = var.aws_region
      "awslogs-stream-prefix" = "ecs"
    }
  }

  s3_env = [
    { name = "S3_CONFIG_BUCKET",  value = aws_s3_bucket.loadtest.id },
    { name = "S3_CONFIG_PREFIX",  value = "loadtest/config" },
    { name = "S3_RESULTS_BUCKET", value = aws_s3_bucket.loadtest.id },
    { name = "S3_RESULTS_PREFIX", value = "loadtest/results" },
  ]
}

# ===========================================================================
# DinoDNS + DNSperf — single Fargate task (shared network namespace)
# ===========================================================================

resource "aws_ecs_task_definition" "dinodns" {
  family                   = "${local.name_prefix}-dinodns"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                      = tostring(var.dinodns_cpu + var.dnsperf_cpu)
  memory                   = tostring(var.dinodns_memory + var.dnsperf_memory)
  execution_role_arn       = aws_iam_role.ecs_execution.arn
  task_role_arn            = aws_iam_role.ecs_task.arn

  container_definitions = jsonencode([
    # ---- DinoDNS ----
    {
      name      = "dinodns"
      image     = "${aws_ecr_repository.dinodns.repository_url}:latest"
      essential = true
      cpu       = var.dinodns_cpu
      memory    = var.dinodns_memory

      portMappings = [
        { containerPort = var.dns_port, protocol = "udp" },
        { containerPort = var.dns_port, protocol = "tcp" },
      ]

      environment = concat(local.s3_env, [
        { name = "DNS_PORT",      value = tostring(var.dns_port) },
        { name = "CLUSTER_MODE",  value = tostring(var.dinodns_cluster_mode) },
        { name = "DOMAINS_FILE",  value = "/config/domains.json" },
      ])

      logConfiguration = local.log_config
    },

    # ---- DNSperf (sidecar) ----
    {
      name      = "dnsperf"
      image     = "${aws_ecr_repository.dnsperf.repository_url}:latest"
      essential = true   # When DNSperf finishes the whole task stops
      cpu       = var.dnsperf_cpu
      memory    = var.dnsperf_memory

      environment = concat(local.s3_env, [
        # Loopback — same network namespace as DinoDNS
        { name = "DNS_SERVER",          value = "127.0.0.1" },
        { name = "DNS_PORT",            value = tostring(var.dns_port) },
        { name = "TEST_DURATION",       value = tostring(var.test_duration) },
        { name = "DNSPERF_EXTRA_ARGS",  value = var.dnsperf_extra_args },
        { name = "SERVER_LABEL",        value = "dinodns" },
      ])

      logConfiguration = local.log_config

      dependsOn = [{
        containerName = "dinodns"
        condition     = "START"
      }]
    },
  ])

  tags = { Name = "${local.name_prefix}-dinodns-td" }
}

# ===========================================================================
# CoreDNS + DNSperf — single Fargate task (shared network namespace)
# ===========================================================================

resource "aws_ecs_task_definition" "coredns" {
  family                   = "${local.name_prefix}-coredns"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                      = tostring(var.coredns_cpu + var.dnsperf_cpu)
  memory                   = tostring(var.coredns_memory + var.dnsperf_memory)
  execution_role_arn       = aws_iam_role.ecs_execution.arn
  task_role_arn            = aws_iam_role.ecs_task.arn

  container_definitions = jsonencode([
    # ---- CoreDNS ----
    {
      name      = "coredns"
      image     = "${aws_ecr_repository.coredns.repository_url}:latest"
      essential = true
      cpu       = var.coredns_cpu
      memory    = var.coredns_memory

      portMappings = [
        { containerPort = 53, protocol = "udp" },
        { containerPort = 53, protocol = "tcp" },
      ]

      environment = concat(local.s3_env, [])

      logConfiguration = local.log_config
    },

    # ---- DNSperf (sidecar) ----
    {
      name      = "dnsperf"
      image     = "${aws_ecr_repository.dnsperf.repository_url}:latest"
      essential = true
      cpu       = var.dnsperf_cpu
      memory    = var.dnsperf_memory

      environment = concat(local.s3_env, [
        { name = "DNS_SERVER",          value = "127.0.0.1" },
        { name = "DNS_PORT",            value = "53" },
        { name = "TEST_DURATION",       value = tostring(var.test_duration) },
        { name = "DNSPERF_EXTRA_ARGS",  value = var.dnsperf_extra_args },
        { name = "SERVER_LABEL",        value = "coredns" },
      ])

      logConfiguration = local.log_config

      dependsOn = [{
        containerName = "coredns"
        condition     = "START"
      }]
    },
  ])

  tags = { Name = "${local.name_prefix}-coredns-td" }
}
