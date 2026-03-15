###############################################################################
# loadtest/terraform/variables.tf
#
# All configurable parameters for the DNS load-test infrastructure.
# Override by passing -var flags or creating a terraform.tfvars file.
###############################################################################

# ----------------------------------------------------------
# AWS / general
# ----------------------------------------------------------

variable "aws_region" {
  description = "AWS region to deploy into."
  type        = string
  default     = "us-east-1"
}

variable "project" {
  description = "Short project name — used as a prefix for all resource names."
  type        = string
  default     = "dns-loadtest"
}

# ----------------------------------------------------------
# Source control — DinoDNS build
# ----------------------------------------------------------

variable "dinodns_repo" {
  description = "Git repository URL for DinoDNS (cloned at Docker build time)."
  type        = string
  default     = "https://github.com/jafayer/DinoDNS.git"
}

variable "dinodns_branch" {
  description = "Git branch / tag to build DinoDNS from."
  type        = string
  default     = "main"
}

# ----------------------------------------------------------
# Domain generation
# ----------------------------------------------------------

variable "domain_tld" {
  description = "TLD used for generated test domains."
  type        = string
  default     = "loadtest.internal"
}

variable "domain_count" {
  description = "Number of valid domain records to generate."
  type        = number
  default     = 100
}

variable "random_seed" {
  description = "Optional seed for the domain generator (null = random)."
  type        = number
  default     = null
  nullable    = true
}

# ----------------------------------------------------------
# Test parameters
# ----------------------------------------------------------

variable "test_duration" {
  description = "How long (in seconds) each DNSperf instance will run."
  type        = number
  default     = 30
}

variable "dnsperf_extra_args" {
  description = "Extra CLI arguments passed verbatim to each dnsperf invocation."
  type        = string
  default     = ""
}

# ----------------------------------------------------------
# DinoDNS scaling
# ----------------------------------------------------------

variable "dinodns_cpu" {
  description = "CPU units for the DinoDNS ECS task (1 vCPU = 1024)."
  type        = number
  default     = 1024
}

variable "dinodns_memory" {
  description = "Memory (MiB) for the DinoDNS ECS task."
  type        = number
  default     = 2048
}

variable "dinodns_count" {
  description = "Number of DinoDNS task replicas (horizontal scaling)."
  type        = number
  default     = 1
}

variable "dinodns_cluster_mode" {
  description = "Enable DinoDNS multi-threaded cluster mode (Node.js cluster API)."
  type        = bool
  default     = false
}

variable "dns_port" {
  description = "Port the DinoDNS server listens on inside the container."
  type        = number
  default     = 53
}

# ----------------------------------------------------------
# CoreDNS scaling
# ----------------------------------------------------------

variable "coredns_cpu" {
  description = "CPU units for the CoreDNS ECS task."
  type        = number
  default     = 1024
}

variable "coredns_memory" {
  description = "Memory (MiB) for the CoreDNS ECS task."
  type        = number
  default     = 2048
}

variable "coredns_count" {
  description = "Number of CoreDNS task replicas (horizontal scaling)."
  type        = number
  default     = 1
}

# ----------------------------------------------------------
# DNSperf
# ----------------------------------------------------------

variable "dnsperf_cpu" {
  description = "CPU units for each DNSperf ECS task."
  type        = number
  default     = 512
}

variable "dnsperf_memory" {
  description = "Memory (MiB) for each DNSperf ECS task."
  type        = number
  default     = 1024
}
