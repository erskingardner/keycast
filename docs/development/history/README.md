# Project history

[Documentation](../../README.md) · [Development](../README.md) · [Open work](../../../TODO.md)

These are dated records of Keycast's design, implementation, and validation. They preserve decisions,
build provenance, findings, and measured outcomes. Some statements describe work that was pending at
the time or behavior superseded by later changes. Use the [current guides](../../README.md) for
operating the system, and [TODO.md](../../../TODO.md) for active follow-ups.

Unless stated otherwise, command snippets in historical reports assume the repository root or the
specific test host recorded in the report, not this archive directory. Old names, image digests,
relay choices, and protocol examples are evidence about those builds, not current defaults.

## Design and hardening

| Record | What it preserves |
|---|---|
| [Original version-2 architecture](V2_ARCHITECTURE.md) | September 5 baseline, why the original implementation was replaced, and the chosen trust boundaries. |
| [Hardening contract](V2_HARDENING_PLAN.md) | Agreed implementation requirements; some early protocol details were subsequently superseded. |
| [Local audit and hardening review](V2_AUDIT.md) | September 5 baseline and September 9 security reviews, fixes, and residual risk. |
| [Audit closure matrix](V2_AUDIT_CLOSURE.md) | Mapping of hardening requirements to implementation and tests. |
| [Validation report](V2_VALIDATION.md) | Local checks, fault/recovery exercises, and their coverage limits. |
| [Original upgrade procedure](V1_UPGRADE.md) | The initial clean-break cutover instructions; current guidance is in [Upgrading](../../upgrading.md). |

## Deployment and relay evidence

| Record | What it preserves |
|---|---|
| [VM test run, September 6](VM_TEST_RUN_2026-09-06.md) | Disposable-host setup, deployment checks, and corrections to the initial test workload. |
| [Soak results, September 8](SOAK_RESULTS_2026-09-08.md) | Corrected 24-hour sampled workload, results, remaining findings, and limitations. |
| [Relay investigation, September 8](RELAY_INVESTIGATION_2026-09-08.md) | Reproduced relay/client behavior and the proposed discovery work. |
| [Relay rollout, September 8](RELAY_ROLLOUT_2026-09-08.md) | Deployed artifact provenance and the scoped-relay workload. |
| [Soak results, September 9](SOAK_RESULTS_2026-09-09.md) | Completed discovery soak, remaining timeout/admission findings, and the decision to end scheduled soaks. |
| [Discovery notes, September 9](RELAY_DISCOVERY_2026-09-09.md) | Implementation overview and point-in-time relay/client qualification notes. |

Completed test windows do not close independent security review or all recovery/operation coverage.
See the [current security model](../../security.md) and active follow-ups before relying on past results.
