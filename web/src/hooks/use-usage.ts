import { useState } from "react";

import { api } from "../lib/api";
import type { SessionItem } from "../types";
import {
  type InjectionLogPoint,
  type MemoryObservability,
  type ReflectorRunPoint,
  type SubagentObservability,
} from "../components/usage-panel";
import { pickLatestSessionKey } from "../lib/session-utils";

export type UseUsageDeps = {
  sessionKey: string;
  sessions: SessionItem[];
  isUnauthorizedError: (err: unknown) => boolean;
  isForbiddenError: (err: unknown) => boolean;
  lockForAuth: (message?: string) => void;
};

export function useUsage(deps: UseUsageDeps) {
  const {
    sessionKey,
    sessions,
    isUnauthorizedError,
    isForbiddenError,
    lockForAuth,
  } = deps;

  const [usageOpen, setUsageOpen] = useState<boolean>(false);
  const [usageLoading, setUsageLoading] = useState<boolean>(false);
  const [usageReport, setUsageReport] = useState<string>("");
  const [usageMemory, setUsageMemory] = useState<MemoryObservability | null>(
    null,
  );
  const [usageSubagents, setUsageSubagents] =
    useState<SubagentObservability | null>(null);
  const [usageReflectorRuns, setUsageReflectorRuns] = useState<
    ReflectorRunPoint[]
  >([]);
  const [usageInjectionLogs, setUsageInjectionLogs] = useState<
    InjectionLogPoint[]
  >([]);
  const [usageError, setUsageError] = useState<string>("");
  const [usageSession, setUsageSession] = useState<string>("");

  async function openUsage(targetSession = sessionKey): Promise<void> {
    setUsageLoading(true);
    setUsageError("");
    setUsageReport("");
    setUsageMemory(null);
    setUsageSubagents(null);
    setUsageReflectorRuns([]);
    setUsageInjectionLogs([]);
    const hasStoredSession = sessions.some(
      (s) => s.session_key === targetSession,
    );
    const resolvedSession = hasStoredSession
      ? targetSession
      : sessions.length > 0
        ? pickLatestSessionKey(sessions)
        : targetSession;
    setUsageSession(resolvedSession);
    try {
      if (!hasStoredSession && sessions.length === 0) {
        setUsageError(
          "No usage data yet. Send a message in this session first.",
        );
        setUsageOpen(true);
        return;
      }
      const query = new URLSearchParams({ session_key: resolvedSession });
      const data = await api<{
        report?: string;
        memory_observability?: MemoryObservability;
      }>(`/api/usage?${query.toString()}`);
      setUsageReport(String(data.report || "").trim());
      setUsageMemory(data.memory_observability ?? null);
      const moQuery = new URLSearchParams({
        session_key: resolvedSession,
        scope: "chat",
        hours: "168",
        limit: "1000",
        offset: "0",
      });
      const series = await api<{
        reflector_runs?: ReflectorRunPoint[];
        injection_logs?: InjectionLogPoint[];
      }>(`/api/memory_observability?${moQuery.toString()}`);
      setUsageReflectorRuns(
        Array.isArray(series.reflector_runs) ? series.reflector_runs : [],
      );
      setUsageInjectionLogs(
        Array.isArray(series.injection_logs) ? series.injection_logs : [],
      );
      const subagents = await api<SubagentObservability>(
        `/api/subagents/observability?session_key=${encodeURIComponent(resolvedSession)}&scope=chat&limit=40`,
      );
      setUsageSubagents(subagents ?? null);
      setUsageOpen(true);
    } catch (e) {
      if (isUnauthorizedError(e)) {
        lockForAuth("Session expired. Please sign in again.");
        return;
      }
      if (isForbiddenError(e)) {
        setUsageError("Forbidden: Usage panel requires permission.");
        setUsageOpen(true);
        return;
      }
      setUsageError(e instanceof Error ? e.message : String(e));
      setUsageOpen(true);
    } finally {
      setUsageLoading(false);
    }
  }

  return {
    usageOpen,
    setUsageOpen,
    usageLoading,
    usageReport,
    usageMemory,
    usageSubagents,
    usageReflectorRuns,
    usageInjectionLogs,
    usageError,
    usageSession,
    openUsage,
  };
}
