# Home Server
This feature provides a **centralized observability endpoint** for bk deployments. It does not participate in backup execution or data handling. Its sole purpose is to collect, validate, store, and expose backup metadata emitted by hosts.

The system is designed to preserve fully decentralized backups while enabling a consistent, global view of backup activity.

---

## Scope and Non‑Goals

The home server **does not**:

* Schedule or trigger backups
* Access backup repositories or data
* Execute commands on hosts
* Act as a control plane

The home server **does**:

* Accept backup event metadata from hosts
* Verify host identity and event integrity
* Store events in an append‑only manner
* Correlate events across hosts
* Expose data for visualization and analysis

---

## High‑Level Architecture

The system consists of two clearly separated roles:

* Hosts (active, state‑owning)
* Home server (passive, observational)

All authority remains on the hosts.

---

## Host Responsibilities

Each host:

* Executes backups independently using its local configuration
* Manages its own repositories, credentials, and scheduling
* Emits a **backup event manifest** after each backup run
* Signs the manifest using its SSH host key
* Pushes the manifest to the home server

A host never waits on or depends on the home server to complete a backup.

---

## Backup Event Manifests

A backup event manifest is a structured, append‑only record describing the outcome of a state change.

Properties:

* Immutable once emitted
* Cryptographically signed by the host

Manifests describe *what happened*.

---

## Home Server Responsibilities

The home server operates as a passive ingestion and analysis system.

On receiving a manifest, it:

* Verifies the signature against known host keys
* Validates schema and timestamps
* Stores the event in append‑only storage
* Indexes and correlates events across hosts and repositories

The home server never initiates communication toward hosts and never attempts to repair or retry backups.

Failure or absence of the home server does not affect backup execution.

---

## Storage Model

### Append‑Only Event Store

All backup events are treated as immutable facts. Updates and deletions are intentionally unsupported.

---

### PostgreSQL

Manifests are persisted to PostgreSQL.

This enables:

* Efficient time‑series queries
* Cross‑host correlation
* External analytics and dashboards

---

## Observability and Visualization

### Grafana

When using PostgreSQL, the home server can expose backup data as a Grafana data source.

This enables:

* Backup success and failure rates
* Backup frequency per host or repository
* Backup duration trends
* Data growth (rate of change)

Backup activity becomes queryable operational data instead of opaque logs.

## Web UI

The home server may provide a read‑only web interface offering:

* Fleet‑wide backup status overview
* Last successful backup per host
* Stale or missing backups
* Historical trends and comparisons

The UI exposes state but does not provide actions. Any remediation happens on the host.

---

## Summary

The home server provides centralized observability without centralizing control.

* Hosts execute backups independently
* The home server records and correlates outcomes
* Global visibility is achieved without weakening decentralization
