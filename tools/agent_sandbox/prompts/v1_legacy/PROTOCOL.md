# Agent Protocol — SYS_AGENT_SPAWN & SYS_AGENT_QUERY

This document defines the inter-agent communication protocol used by all agents
in the Aegis OS cognitive hierarchy. It is the authoritative reference for both
agent prompt authors and kernel implementors.

---

## Overview

Agents communicate with the runtime using **protocol tokens** — structured strings
embedded in the agent's text output that the kernel intercepts and acts upon.

There are two protocol tokens:

| Token | Direction | Purpose |
|---|---|---|
| `SYS_AGENT_SPAWN` | Agent → Runtime | Create a new subordinate agent |
| `SYS_AGENT_QUERY` | Agent → Runtime | Query an active agent without spawning work |

The runtime (implemented in `ank-core/src/agents/orchestrator.rs`) scans each
agent output for these tokens, extracts the parameters, and acts on them before
returning control to the agent.

---

## Token Syntax

Tokens must appear on their own line in the agent's output.
Parameters use `key="value"` format. String values are always double-quoted.
Parameter order does not matter. Unknown parameters are silently ignored.

```
[SYS_AGENT_SPAWN(role="<role>", name="<name>", scope="<scope>", task_type="<task_type>")]
[SYS_AGENT_QUERY(project="<project_name>", question="<question>")]
```

---

## SYS_AGENT_SPAWN

Instructs the runtime to create a new subordinate agent under the calling agent.

### Parameters

| Parameter | Required | Values | Description |
|---|---|---|---|
| `role` | **yes** | `project_supervisor` `supervisor` `specialist` | The role of the new agent |
| `name` | no | any string | Human-readable identifier. Required for `project_supervisor`. Optional for others. |
| `scope` | **yes** | any string | Describes the agent's task or domain. Injected into the agent's system prompt. |
| `task_type` | no | `code` `analysis` `planning` `creative` | Cognitive nature of the task. Used by the CMR to select the appropriate model. Defaults to the role's standard task type if omitted. |

### Role definitions

**`project_supervisor`**
Coordinates work across an entire project. Created by the Chat Agent.
Reports back to the Chat Agent. Can spawn `supervisor` or `specialist` nodes.

**`supervisor`**
Coordinates a domain within a project. Created by a `project_supervisor` or
another `supervisor`. Reports to its parent. Can spawn `supervisor` or `specialist` nodes.
Generates a State Summary on session close.

**`specialist`**
Executes a single atomic task. Created by any supervisor role.
Cannot spawn sub-agents. Reports to its parent.

### Behavior

1. The runtime creates an `AgentNode` with a new `AgentId` and registers it in the `AgentTree`.
2. The new agent's system prompt is assembled from: role file + scope injection + filtered context.
3. The new agent is added to the calling agent's `children` list.
4. The calling agent's state transitions to `WAITING_REPORT`.
5. The new agent begins execution with a `Dispatch` message containing the scope as its task.
6. When the new agent completes, it sends a `Report` back up; the calling agent resumes.

### Examples

```
[SYS_AGENT_SPAWN(role="project_supervisor", name="Aegis", scope="user wants to work on the Aegis project")]

[SYS_AGENT_SPAWN(role="supervisor", name="Auth", scope="refactor the authentication module", task_type="code")]

[SYS_AGENT_SPAWN(role="specialist", scope="fix the null pointer in scheduler.rs line 89", task_type="code")]

[SYS_AGENT_SPAWN(role="specialist", scope="write the executive summary for the Q3 report", task_type="creative")]
```

---

## SYS_AGENT_QUERY

Requests information from an active agent in the tree without creating new work
or modifying any state.

### Parameters

| Parameter | Required | Values | Description |
|---|---|---|---|
| `project` | **yes** | project name string | Identifies which project's supervisor tree to query |
| `question` | **yes** | any string | The specific question to answer |

### Behavior

1. The runtime locates the `ProjectSupervisor` for the named project in the `AgentTree`.
2. The question is routed down the tree to the most appropriate `Supervisor` or `Specialist`.
3. The responding agent returns a `QueryReply` — read-only, no side effects.
4. The reply propagates back up to the calling agent.
5. No agents are spawned. No files are modified. No state changes.

### Examples

```
[SYS_AGENT_QUERY(project="aegis", question="what does authenticate_tenant do?")]

[SYS_AGENT_QUERY(project="portfolio", question="how many React components are in the UI layer?")]
```

---

## Communication rules

**Strictly hierarchical.** Agents may only communicate with their direct parent
(via `Report` or `QueryReply`) or their direct children (via `Dispatch`).
Lateral communication between agents with different parents is **prohibited**.
Cross-domain coordination must route through the common ancestor.

**No fabrication.** An agent that has not received a real `QueryReply` from
an active subordinate must not describe, estimate, or assert anything about
resources it has not directly received. Report the absence of information instead.

**Single token per output.** An agent should not emit more than one protocol token
per response turn. If multiple subordinates are needed, emit one token, wait for
the report, then emit the next.

---

## Agent tree lifecycle

```
Session start
    ↓
Chat Agent activated (singleton per session)
    ↓
User sends message
    ↓
Chat Agent emits SYS_AGENT_SPAWN → ProjectSupervisor created
    ↓
ProjectSupervisor emits SYS_AGENT_SPAWN → Supervisors / Specialists created
    ↓
Specialists execute → Report up
    ↓
Supervisors aggregate → Report up
    ↓
ProjectSupervisor consolidates → Report to Chat Agent
    ↓
Chat Agent responds to user
    ↓
Session end → Supervisors emit State Summary → AgentTree pruned from memory
```

The `AgentTree` is **in-memory only** (ADR-AGENTS-001). It is not persisted to
SQLCipher. State Summaries written by Supervisors are the persistence mechanism
across sessions.

---

## Kernel implementation reference

| Concept | Location |
|---|---|
| `AgentNode`, `AgentRole`, `AgentState` | `ank-core/src/agents/node.rs` (CORE-155) |
| `AgentTree` | `ank-core/src/agents/tree.rs` (CORE-156) |
| `AgentMessage`, `Dispatch`, `Report`, `QueryReply` | `ank-core/src/agents/message.rs` (CORE-157) |
| `AgentOrchestrator` (token parser + lifecycle) | `ank-core/src/agents/orchestrator.rs` (CORE-158) |
| `ProjectRegistry` | `ank-core/src/agents/project.rs` (CORE-159) |
| `SYS_AGENT_SPAWN` syscall | `ank-core/src/agents/orchestrator.rs` (CORE-162) |

See `governance/EPIC_43_HIERARCHICAL_AGENTS.md` for full architectural context.

---

*Authored by Arquitecto IA — 2026-04-29*
