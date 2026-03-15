###############################################################################
# loadtest/terraform/main.tf
#
# Root Terraform configuration for the DNS load-test infrastructure.
###############################################################################

terraform {
  required_version = ">= 1.6.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.0"
    }
  }
}

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = var.project
      ManagedBy   = "terraform"
      Environment = "loadtest"
    }
  }
}

# Random suffix to make resource names globally unique
resource "random_id" "suffix" {
  byte_length = 4
}

locals {
  name_prefix = "${var.project}-${random_id.suffix.hex}"
}
