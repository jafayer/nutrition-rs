###############################################################################
# loadtest/terraform/vpc.tf
#
# Minimal VPC: one public subnet per AZ (up to 2), an internet gateway, and
# security groups that allow DNS traffic between all ECS tasks.
#
# All tasks land in the same subnet/placement so cross-task latency is
# dominated by the host network stack rather than physical distance.
###############################################################################

data "aws_availability_zones" "available" {
  state = "available"
}

locals {
  # Use at most 2 AZs
  azs = slice(data.aws_availability_zones.available.names, 0, min(2, length(data.aws_availability_zones.available.names)))
}

# ----------------------------------------------------------
# VPC
# ----------------------------------------------------------

resource "aws_vpc" "main" {
  cidr_block           = "10.10.0.0/16"
  enable_dns_support   = true
  enable_dns_hostnames = true

  tags = { Name = "${local.name_prefix}-vpc" }
}

# ----------------------------------------------------------
# Public subnets (one per AZ) — Fargate tasks run here
# ----------------------------------------------------------

resource "aws_subnet" "public" {
  count = length(local.azs)

  vpc_id                  = aws_vpc.main.id
  cidr_block              = cidrsubnet("10.10.0.0/16", 8, count.index)
  availability_zone       = local.azs[count.index]
  map_public_ip_on_launch = true

  tags = { Name = "${local.name_prefix}-public-${count.index}" }
}

# ----------------------------------------------------------
# Internet Gateway + route table
# ----------------------------------------------------------

resource "aws_internet_gateway" "igw" {
  vpc_id = aws_vpc.main.id
  tags   = { Name = "${local.name_prefix}-igw" }
}

resource "aws_route_table" "public" {
  vpc_id = aws_vpc.main.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.igw.id
  }

  tags = { Name = "${local.name_prefix}-public-rt" }
}

resource "aws_route_table_association" "public" {
  count          = length(aws_subnet.public)
  subnet_id      = aws_subnet.public[count.index].id
  route_table_id = aws_route_table.public.id
}

# ----------------------------------------------------------
# Security groups
# ----------------------------------------------------------

# DNS servers: accept DNS (UDP+TCP/53) from within the VPC
resource "aws_security_group" "dns_server" {
  name        = "${local.name_prefix}-dns-server"
  description = "Allow inbound DNS from within VPC"
  vpc_id      = aws_vpc.main.id

  ingress {
    description = "DNS UDP"
    from_port   = 53
    to_port     = 53
    protocol    = "udp"
    cidr_blocks = [aws_vpc.main.cidr_block]
  }

  ingress {
    description = "DNS TCP"
    from_port   = 53
    to_port     = 53
    protocol    = "tcp"
    cidr_blocks = [aws_vpc.main.cidr_block]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = { Name = "${local.name_prefix}-dns-server-sg" }
}

# DNSperf: only needs outbound access (connects to DNS servers in the VPC)
resource "aws_security_group" "dnsperf" {
  name        = "${local.name_prefix}-dnsperf"
  description = "Allow outbound DNS and HTTPS (for S3/CloudWatch)"
  vpc_id      = aws_vpc.main.id

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = { Name = "${local.name_prefix}-dnsperf-sg" }
}
