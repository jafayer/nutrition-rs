###############################################################################
# loadtest/terraform/ecs.tf
#
# ECS cluster, task definitions, and services for:
#
#   DNS server services  (long-running, registered behind NLBs)
#     • DinoDNS   — aws_ecs_service.dinodns   (var.dinodns_count replicas)
#     • CoreDNS   — aws_ecs_service.coredns   (var.coredns_count replicas)
#
#   DNSperf run-to-completion tasks  (run once, exit with results)
#     • dnsperf-dinodns — targets the DinoDNS NLB
#     • dnsperf-coredns — targets the CoreDNS NLB
#
# IMPORTANT: DNS servers and DNSperf tasks are SEPARATE Fargate tasks that
# land on different underlying hosts.  Each task gets its own dedicated CPU
# and memory allocation so neither workload can bottleneck the other.
# DNSperf reaches the DNS servers through the NLBs defined in nlb.tf, which
# operate at Layer-4 (pass-through) so added latency is negligible.
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
# Shared locals
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
# DinoDNS — dedicated task definition + long-running ECS service
# (registered with the DinoDNS NLB target groups)
# ===========================================================================

resource "aws_ecs_task_definition" "dinodns" {
  family                   = "${local.name_prefix}-dinodns"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  # CPU / memory sized exclusively for DinoDNS — DNSperf runs on separate hardware
  cpu                = tostring(var.dinodns_cpu)
  memory             = tostring(var.dinodns_memory)
  execution_role_arn = aws_iam_role.ecs_execution.arn
  task_role_arn      = aws_iam_role.ecs_task.arn

  container_definitions = jsonencode([{
    name      = "dinodns"
    image     = "${aws_ecr_repository.dinodns.repository_url}:latest"
    essential = true

    portMappings = [
      { containerPort = var.dns_port, protocol = "udp" },
      { containerPort = var.dns_port, protocol = "tcp" },
    ]

    environment = concat(local.s3_env, [
      { name = "DNS_PORT",     value = tostring(var.dns_port) },
      { name = "CLUSTER_MODE", value = tostring(var.dinodns_cluster_mode) },
      { name = "DOMAINS_FILE", value = "/config/domains.json" },
    ])

    logConfiguration = local.log_config
  }])

  tags = { Name = "${local.name_prefix}-dinodns-td" }
}

resource "aws_ecs_service" "dinodns" {
  name            = "${local.name_prefix}-dinodns"
  cluster         = aws_ecs_cluster.main.id
  task_definition = aws_ecs_task_definition.dinodns.arn
  desired_count   = var.dinodns_count
  launch_type     = "FARGATE"

  # Spread tasks across all subnets so replicas land on different hosts
  network_configuration {
    subnets          = [for s in aws_subnet.public : s.id]
    security_groups  = [aws_security_group.dns_server.id]
    assign_public_ip = true   # Required for Fargate in a public subnet to pull images
  }

  # Register with both UDP and TCP NLB target groups
  load_balancer {
    target_group_arn = aws_lb_target_group.dinodns_udp.arn
    container_name   = "dinodns"
    container_port   = var.dns_port
  }

  load_balancer {
    target_group_arn = aws_lb_target_group.dinodns_tcp.arn
    container_name   = "dinodns"
    container_port   = var.dns_port
  }

  # Ensure NLB listeners + target groups exist before the service
  depends_on = [
    aws_lb_listener.dinodns_udp,
    aws_lb_listener.dinodns_tcp,
  ]

  tags = { Name = "${local.name_prefix}-dinodns-svc" }
}

# ===========================================================================
# CoreDNS — dedicated task definition + long-running ECS service
# ===========================================================================

resource "aws_ecs_task_definition" "coredns" {
  family                   = "${local.name_prefix}-coredns"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                = tostring(var.coredns_cpu)
  memory             = tostring(var.coredns_memory)
  execution_role_arn = aws_iam_role.ecs_execution.arn
  task_role_arn      = aws_iam_role.ecs_task.arn

  container_definitions = jsonencode([{
    name      = "coredns"
    image     = "${aws_ecr_repository.coredns.repository_url}:latest"
    essential = true

    portMappings = [
      { containerPort = 53, protocol = "udp" },
      { containerPort = 53, protocol = "tcp" },
    ]

    environment = concat(local.s3_env, [])

    logConfiguration = local.log_config
  }])

  tags = { Name = "${local.name_prefix}-coredns-td" }
}

resource "aws_ecs_service" "coredns" {
  name            = "${local.name_prefix}-coredns"
  cluster         = aws_ecs_cluster.main.id
  task_definition = aws_ecs_task_definition.coredns.arn
  desired_count   = var.coredns_count
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = [for s in aws_subnet.public : s.id]
    security_groups  = [aws_security_group.dns_server.id]
    assign_public_ip = true
  }

  load_balancer {
    target_group_arn = aws_lb_target_group.coredns_udp.arn
    container_name   = "coredns"
    container_port   = 53
  }

  load_balancer {
    target_group_arn = aws_lb_target_group.coredns_tcp.arn
    container_name   = "coredns"
    container_port   = 53
  }

  depends_on = [
    aws_lb_listener.coredns_udp,
    aws_lb_listener.coredns_tcp,
  ]

  tags = { Name = "${local.name_prefix}-coredns-svc" }
}

# ===========================================================================
# DNSperf → DinoDNS
#
# Runs as a standalone Fargate task on its own dedicated host.
# Reaches DinoDNS exclusively through the Layer-4 NLB — no shared resources
# with the DNS server.
# The task definition is registered here; the orchestration script (run.sh /
# orchestrate.py) runs it via `aws ecs run-task` after the DNS services are
# healthy.
# ===========================================================================

resource "aws_ecs_task_definition" "dnsperf_dinodns" {
  family                   = "${local.name_prefix}-dnsperf-dinodns"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  # CPU / memory sized exclusively for DNSperf — DinoDNS runs on separate hardware
  cpu                = tostring(var.dnsperf_cpu)
  memory             = tostring(var.dnsperf_memory)
  execution_role_arn = aws_iam_role.ecs_execution.arn
  task_role_arn      = aws_iam_role.ecs_task.arn

  container_definitions = jsonencode([{
    name      = "dnsperf"
    image     = "${aws_ecr_repository.dnsperf.repository_url}:latest"
    essential = true

    environment = concat(local.s3_env, [
      # Point at the DinoDNS NLB — never at loopback or the DinoDNS task directly
      { name = "DNS_SERVER",         value = aws_lb.dinodns.dns_name },
      { name = "DNS_PORT",           value = tostring(var.dns_port) },
      { name = "TEST_DURATION",      value = tostring(var.test_duration) },
      { name = "DNSPERF_EXTRA_ARGS", value = var.dnsperf_extra_args },
      { name = "SERVER_LABEL",       value = "dinodns" },
    ])

    logConfiguration = local.log_config
  }])

  tags = { Name = "${local.name_prefix}-dnsperf-dinodns-td" }
}

# ===========================================================================
# DNSperf → CoreDNS  (same pattern, points at CoreDNS NLB)
# ===========================================================================

resource "aws_ecs_task_definition" "dnsperf_coredns" {
  family                   = "${local.name_prefix}-dnsperf-coredns"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                = tostring(var.dnsperf_cpu)
  memory             = tostring(var.dnsperf_memory)
  execution_role_arn = aws_iam_role.ecs_execution.arn
  task_role_arn      = aws_iam_role.ecs_task.arn

  container_definitions = jsonencode([{
    name      = "dnsperf"
    image     = "${aws_ecr_repository.dnsperf.repository_url}:latest"
    essential = true

    environment = concat(local.s3_env, [
      { name = "DNS_SERVER",         value = aws_lb.coredns.dns_name },
      { name = "DNS_PORT",           value = "53" },
      { name = "TEST_DURATION",      value = tostring(var.test_duration) },
      { name = "DNSPERF_EXTRA_ARGS", value = var.dnsperf_extra_args },
      { name = "SERVER_LABEL",       value = "coredns" },
    ])

    logConfiguration = local.log_config
  }])

  tags = { Name = "${local.name_prefix}-dnsperf-coredns-td" }
}
