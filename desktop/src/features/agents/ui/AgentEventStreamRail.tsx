import * as React from "react";
import {
  AgentEventStream,
  AgentStreamEvent,
} from "../lib/agentEventStream";
import { Brain, Wrench, Eye, Camera, ShieldAlert, History } from "lucide-react";

interface AgentEventStreamRailProps {
  stream: AgentEventStream;
  onRollback?: (checkpointId: string) => void;
}

export const AgentEventStreamRail: React.FC<AgentEventStreamRailProps> = ({
  stream,
  onRollback,
}) => {
  const events = stream.getEvents();

  const getEventIcon = (type: AgentStreamEvent["type"]) => {
    switch (type) {
      case "thought":
        return <Brain className="w-4 h-4 text-purple-400" />;
      case "tool_call":
        return <Wrench className="w-4 h-4 text-blue-400" />;
      case "observation":
        return <Eye className="w-4 h-4 text-green-400" />;
      case "screenshot":
        return <Camera className="w-4 h-4 text-yellow-400" />;
      case "checkpoint":
        return <History className="w-4 h-4 text-cyan-400" />;
      default:
        return <ShieldAlert className="w-4 h-4 text-red-400" />;
    }
  };

  return (
    <div className="flex flex-col h-full bg-slate-900 border-l border-slate-800 text-slate-200 w-80 text-xs">
      <div className="p-3 border-b border-slate-800 font-semibold flex items-center justify-between">
        <span>⚡ Agent 事件流 (時光機軌跡)</span>
        <span className="text-slate-400 font-mono text-[10px]">
          {events.length} Events
        </span>
      </div>

      <div className="flex-1 overflow-y-auto p-2 space-y-2">
        {events.length === 0 ? (
          <div className="text-center text-slate-500 py-8">
            暫無事件紀錄，特工處於待命狀態
          </div>
        ) : (
          events.map((evt) => (
            <div
              key={evt.id}
              className="p-2 rounded bg-slate-800/60 border border-slate-700/50 hover:border-slate-600 transition"
            >
              <div className="flex items-center justify-between mb-1">
                <div className="flex items-center space-x-1.5 font-medium">
                  {getEventIcon(evt.type)}
                  <span className="uppercase tracking-wider text-[10px] text-slate-400">
                    {evt.type}
                  </span>
                </div>
                <span className="text-[10px] text-slate-500">
                  {evt.readableTime}
                </span>
              </div>

              <div className="font-semibold text-slate-100">{evt.payload.title}</div>

              {evt.payload.content && (
                <div className="text-slate-300 mt-1 whitespace-pre-wrap font-mono text-[11px] bg-slate-950/40 p-1.5 rounded">
                  {evt.payload.content}
                </div>
              )}

              {evt.payload.toolName && (
                <div className="mt-1 text-[11px] text-blue-300 font-mono">
                  🔧 調用: {evt.payload.toolName}
                </div>
              )}

              {evt.type === "checkpoint" && onRollback && (
                <button
                  onClick={() => onRollback(evt.id)}
                  className="mt-2 text-[10px] bg-cyan-900/40 hover:bg-cyan-800/60 text-cyan-300 px-2 py-0.5 rounded border border-cyan-700/50 flex items-center space-x-1"
                >
                  <History className="w-3 h-3" />
                  <span>回溯至此斷點</span>
                </button>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
};

