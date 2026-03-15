###############################################################################
# loadtest/terraform/nlb.tf
#
# One Network Load Balancer per DNS server (DinoDNS + CoreDNS).
#
# NLBs operate at Layer 4 (pass-through, no termination) so they add minimal
# latency.  They also give DNSperf a stable, known endpoint regardless of how
# many DNS-server task replicas are running (horizontal scaling is transparent).
#
# Both UDP/53 and TCP/53 listeners are created because dnsperf uses UDP by
# default but CoreDNS / DinoDNS support both.
###############################################################################

# ----------------------------------------------------------
# DinoDNS NLB
# ----------------------------------------------------------

resource "aws_lb" "dinodns" {
  name               = "${local.name_prefix}-dino"
  load_balancer_type = "network"
  internal           = true   # Keep inside the VPC — no public exposure needed
  subnets            = [for s in aws_subnet.public : s.id]

  enable_cross_zone_load_balancing = true

  tags = { Name = "${local.name_prefix}-dinodns-nlb" }
}

# UDP target group
resource "aws_lb_target_group" "dinodns_udp" {
  name        = "${local.name_prefix}-dino-udp"
  port        = var.dns_port
  protocol    = "UDP"
  vpc_id      = aws_vpc.main.id
  target_type = "ip"   # Required for Fargate

  health_check {
    # NLBs don't support UDP health checks; use TCP on the same port instead.
    # DinoDNS listens on TCP as well as UDP.
    protocol            = "TCP"
    port                = tostring(var.dns_port)
    healthy_threshold   = 2
    unhealthy_threshold = 2
    interval            = 10
  }

  tags = { Name = "${local.name_prefix}-dinodns-udp-tg" }
}

# TCP target group
resource "aws_lb_target_group" "dinodns_tcp" {
  name        = "${local.name_prefix}-dino-tcp"
  port        = var.dns_port
  protocol    = "TCP"
  vpc_id      = aws_vpc.main.id
  target_type = "ip"

  health_check {
    protocol            = "TCP"
    port                = tostring(var.dns_port)
    healthy_threshold   = 2
    unhealthy_threshold = 2
    interval            = 10
  }

  tags = { Name = "${local.name_prefix}-dinodns-tcp-tg" }
}

resource "aws_lb_listener" "dinodns_udp" {
  load_balancer_arn = aws_lb.dinodns.arn
  port              = var.dns_port
  protocol          = "UDP"

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.dinodns_udp.arn
  }
}

resource "aws_lb_listener" "dinodns_tcp" {
  load_balancer_arn = aws_lb.dinodns.arn
  port              = var.dns_port
  protocol          = "TCP"

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.dinodns_tcp.arn
  }
}

# ----------------------------------------------------------
# CoreDNS NLB
# ----------------------------------------------------------

resource "aws_lb" "coredns" {
  name               = "${local.name_prefix}-core"
  load_balancer_type = "network"
  internal           = true
  subnets            = [for s in aws_subnet.public : s.id]

  enable_cross_zone_load_balancing = true

  tags = { Name = "${local.name_prefix}-coredns-nlb" }
}

resource "aws_lb_target_group" "coredns_udp" {
  name        = "${local.name_prefix}-core-udp"
  port        = 53
  protocol    = "UDP"
  vpc_id      = aws_vpc.main.id
  target_type = "ip"

  health_check {
    protocol            = "TCP"
    port                = "53"
    healthy_threshold   = 2
    unhealthy_threshold = 2
    interval            = 10
  }

  tags = { Name = "${local.name_prefix}-coredns-udp-tg" }
}

resource "aws_lb_target_group" "coredns_tcp" {
  name        = "${local.name_prefix}-core-tcp"
  port        = 53
  protocol    = "TCP"
  vpc_id      = aws_vpc.main.id
  target_type = "ip"

  health_check {
    protocol            = "TCP"
    port                = "53"
    healthy_threshold   = 2
    unhealthy_threshold = 2
    interval            = 10
  }

  tags = { Name = "${local.name_prefix}-coredns-tcp-tg" }
}

resource "aws_lb_listener" "coredns_udp" {
  load_balancer_arn = aws_lb.coredns.arn
  port              = 53
  protocol          = "UDP"

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.coredns_udp.arn
  }
}

resource "aws_lb_listener" "coredns_tcp" {
  load_balancer_arn = aws_lb.coredns.arn
  port              = 53
  protocol          = "TCP"

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.coredns_tcp.arn
  }
}
