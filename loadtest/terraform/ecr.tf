###############################################################################
# loadtest/terraform/ecr.tf
#
# ECR repositories for the custom DinoDNS, CoreDNS, and DNSperf images.
# Images are built and pushed by the orchestration script (run.sh / run.py)
# before ECS tasks are started.
###############################################################################

resource "aws_ecr_repository" "dinodns" {
  name                 = "${local.name_prefix}-dinodns"
  image_tag_mutability = "MUTABLE"

  image_scanning_configuration {
    scan_on_push = false
  }

  # Delete images automatically — this is ephemeral infrastructure.
  force_delete = true

  tags = { Name = "${local.name_prefix}-dinodns-ecr" }
}

resource "aws_ecr_repository" "coredns" {
  name                 = "${local.name_prefix}-coredns"
  image_tag_mutability = "MUTABLE"

  image_scanning_configuration {
    scan_on_push = false
  }

  force_delete = true

  tags = { Name = "${local.name_prefix}-coredns-ecr" }
}

resource "aws_ecr_repository" "dnsperf" {
  name                 = "${local.name_prefix}-dnsperf"
  image_tag_mutability = "MUTABLE"

  image_scanning_configuration {
    scan_on_push = false
  }

  force_delete = true

  tags = { Name = "${local.name_prefix}-dnsperf-ecr" }
}

# Lifecycle policy: keep only the latest 5 images in each repo
resource "aws_ecr_lifecycle_policy" "dinodns" {
  repository = aws_ecr_repository.dinodns.name

  policy = jsonencode({
    rules = [{
      rulePriority = 1
      description  = "Keep last 5 images"
      selection = {
        tagStatus   = "any"
        countType   = "imageCountMoreThan"
        countNumber = 5
      }
      action = { type = "expire" }
    }]
  })
}

resource "aws_ecr_lifecycle_policy" "coredns" {
  repository = aws_ecr_repository.coredns.name

  policy = jsonencode({
    rules = [{
      rulePriority = 1
      description  = "Keep last 5 images"
      selection = {
        tagStatus   = "any"
        countType   = "imageCountMoreThan"
        countNumber = 5
      }
      action = { type = "expire" }
    }]
  })
}

resource "aws_ecr_lifecycle_policy" "dnsperf" {
  repository = aws_ecr_repository.dnsperf.name

  policy = jsonencode({
    rules = [{
      rulePriority = 1
      description  = "Keep last 5 images"
      selection = {
        tagStatus   = "any"
        countType   = "imageCountMoreThan"
        countNumber = 5
      }
      action = { type = "expire" }
    }]
  })
}
