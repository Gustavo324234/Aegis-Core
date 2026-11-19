# CORE-357: Fix WebSocket Real-Time Token Streaming and Evict Completed PIDs from `event_broker`

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer

## Description
1. `ws/chat.rs` dispatched chat turns via `SchedulerEvent::ScheduleTaskConfirmed(Box::new(pcb), tx)` and awaited `rx.await` before calling `stream_with_receiver`. Because `CORE-185` repurposed `ScheduleTaskConfirmed` to store `confirm_tx` in `output_pending` until `ProcessCompleted` (sending the full LLM output string at the end of inference for Siren TTS), `ws/chat.rs`:
   - Blocked for the entire LLM generation before starting `stream_with_receiver`, preventing real-time token streaming and live `AgentEvent` forwarding.
   - Emitted the full LLM output string as the `pid` field in `"Task accepted. PID: ..."`.
   - Overflowed the 512-slot `broadcast::channel` (`RecvError::Lagged`) on long responses (>512 events), causing `stream_with_receiver` to exit immediately with empty output.
2. Neither `ank-server/src/main.rs` nor `ws/chat.rs` evicted completed `pid` entries from `state.event_broker`, leaking a `broadcast::Sender<TaskEvent>` channel in RAM for every task ever scheduled.

## Acceptance Criteria
- [x] Dispatch chat turns via `SchedulerEvent::ScheduleTask(Box::new(pcb))` in `ws/chat.rs` using `pcb.pid` directly so `stream_with_receiver` begins streaming tokens in real time immediately.
- [x] Increase `event_broker` per-task broadcast channel buffer from `512` to `2048`.
- [x] Evict completed `pid` entries from `event_broker` upon task completion in `ank-server/src/main.rs` and after `stream_with_receiver` in `ws/chat.rs`.
