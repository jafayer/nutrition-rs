###############################################################################
# loadtest/terraform/s3.tf
#
# S3 bucket for:
#   - Uploading generated config files (domains.json, Corefile, queries.txt …)
#   - Collecting DNSperf result files after the test run
###############################################################################

resource "aws_s3_bucket" "loadtest" {
  bucket        = "${local.name_prefix}-data"
  force_destroy = true   # Make it easy to destroy the whole stack

  tags = { Name = "${local.name_prefix}-data" }
}

resource "aws_s3_bucket_public_access_block" "loadtest" {
  bucket = aws_s3_bucket.loadtest.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_server_side_encryption_configuration" "loadtest" {
  bucket = aws_s3_bucket.loadtest.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

resource "aws_s3_bucket_lifecycle_configuration" "loadtest" {
  bucket = aws_s3_bucket.loadtest.id

  rule {
    id     = "expire-after-7-days"
    status = "Enabled"

    expiration {
      days = 7
    }
  }
}
