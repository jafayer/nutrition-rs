###############################################################################
# loadtest/terraform/outputs.tf
###############################################################################

output "ecr_dinodns_url" {
  description = "ECR repository URL for the DinoDNS image."
  value       = aws_ecr_repository.dinodns.repository_url
}

output "ecr_coredns_url" {
  description = "ECR repository URL for the CoreDNS image."
  value       = aws_ecr_repository.coredns.repository_url
}

output "ecr_dnsperf_url" {
  description = "ECR repository URL for the DNSperf image."
  value       = aws_ecr_repository.dnsperf.repository_url
}

output "s3_bucket" {
  description = "Name of the S3 bucket used for config and results."
  value       = aws_s3_bucket.loadtest.id
}

output "ecs_cluster_name" {
  description = "Name of the ECS cluster."
  value       = aws_ecs_cluster.main.name
}

output "ecs_task_definition_dinodns" {
  description = "ARN of the DinoDNS ECS task definition."
  value       = aws_ecs_task_definition.dinodns.arn
}

output "ecs_task_definition_coredns" {
  description = "ARN of the CoreDNS ECS task definition."
  value       = aws_ecs_task_definition.coredns.arn
}

output "cloudwatch_log_group" {
  description = "CloudWatch log group name for all ECS tasks."
  value       = aws_cloudwatch_log_group.loadtest.name
}

output "vpc_id" {
  description = "VPC ID."
  value       = aws_vpc.main.id
}

output "public_subnet_ids" {
  description = "IDs of the public subnets."
  value       = [for s in aws_subnet.public : s.id]
}

output "security_group_dns_server" {
  description = "Security group ID for DNS server tasks."
  value       = aws_security_group.dns_server.id
}

output "security_group_dnsperf" {
  description = "Security group ID for DNSperf tasks."
  value       = aws_security_group.dnsperf.id
}
