---
title = "Distributed Event Bus"
slug = "distributed-event-bus"
slot = "project-body"
page_scope = "projects"
status = "active"
tech_stack = ["Rust", "Tokio", "gRPC", "Kafka", "Kubernetes"]
start_date = "2022"
end_date = "Present"
description = "High-throughput distributed event bus powering 40+ microservices. Handles 500k+ events/day with sub-10ms p99 delivery latency."
---

## Architecture

Append-only log at the core, with topic-based subscription and
consumer group semantics. Built on top of Kafka for durability,
with a Rust service layer that handles routing, filtering, and
schema validation.

## Key Numbers

- 500k+ events/day in production
- Sub-10ms p99 end-to-end latency
- 99.97% uptime over 18 months
- 6MB binary, deploys in under 3 seconds
