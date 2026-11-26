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
  | { type: 'supervisor_timed_out'; agent_id: string; project_name: string }
  | {
      type: 'spawned';
      agent_id: string;
      project_id: string;
      model: string;
      task_type: string;
    }
  | {
      type: 'state_changed';
      agent_id: string;
      state: string;
    }
  | {
      type: 'activity';
      agent_id: string;
      description: string;
    }
  | {
      type: 'reported';
      agent_id: string;
      summary: string;
    };
