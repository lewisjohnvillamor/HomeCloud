# ADR 0003: Modular Monolith Before Microservices

- Status: Accepted for initial architecture
- Date: 2026-08-29

## Context

Home/self-hosted software is harmed by unnecessary operational dependencies. The system nevertheless contains distinct concerns: storage, media, search, auth, sharing, sync, and jobs.

## Decision

Build a Rust modular monolith with clear crate/module boundaries. CPU-heavy/untrusted media and AI workers may run as separate processes/containers when isolation is useful, while sharing contracts and the same database/job model.

Do not introduce a network service solely to enforce a code boundary.

## Consequences

- simple Docker Compose deployment;
- easier local development and upgrades;
- domain boundaries remain testable;
- future extraction remains possible if metrics or security isolation justify it.
