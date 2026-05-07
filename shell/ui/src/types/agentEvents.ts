// CORE-274: tipos para frames agent_event en el WebSocket principal
export type AgentEventPayload =
  | {
      type: 'supervisor_question';
      agent_id: string;
      project_name: string;
      question: string;
      context?: string;
      timestamp: string;
    }
  | { type: 'supervisor_resumed'; agent_id: string }
  | {
      type: 'supervisor_completed';
      agent_id: string;
      project_name: string;
      summary: string;
    }
  | { type: 'supervisor_timed_out'; agent_id: string; project_name: string };
