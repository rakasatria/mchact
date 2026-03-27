import React, { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { ThreadMessageLike } from "@assistant-ui/react";
import { Callout, Heading, Theme } from "@radix-ui/themes";
import "@radix-ui/themes/styles.css";
import "@assistant-ui/react-ui/styles/index.css";
import "./styles.css";

import { UsagePanel } from "./components/usage-panel";
import { SessionSidebar } from "./components/session-sidebar";
import { ThreadPane } from "./components/thread-pane";
import { AuthDialogs } from "./components/auth-dialogs";
import { SettingsDialog } from "./components/settings/settings-dialog";
import type { SettingsContextValue } from "./context/settings-context";
import { api, ApiError } from "./lib/api";
import type { SessionItem } from "./types";

import type { BackendMessage, Appearance, UiTheme } from "./lib/types";

import { UI_THEME_OPTIONS, RADIX_ACCENT_BY_THEME } from "./lib/constants";

import {
  readAppearance,
  saveAppearance,
  readUiTheme,
  saveUiTheme,
} from "./lib/theme";

import {
  writeSessionToUrl,
  readSessionFromUrl,
  makeSessionKey,
  pickLatestSessionKey,
  mapBackendHistory,
} from "./lib/session-utils";

import { useAuth } from "./hooks/use-auth";
import { useConfig } from "./hooks/use-config";
import { useChatAdapter } from "./hooks/use-chat-adapter";
import { useUsage } from "./hooks/use-usage";

function App() {
  const [appearance, setAppearance] = useState<Appearance>(readAppearance());
  const [uiTheme, setUiTheme] = useState<UiTheme>(readUiTheme());
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState<boolean>(false);
  const [desktopSidebarOpen, setDesktopSidebarOpen] = useState<boolean>(true);
  const [sessions, setSessions] = useState<SessionItem[]>([]);
  const [extraSessions, setExtraSessions] = useState<SessionItem[]>([]);
  const [sessionKey, setSessionKey] = useState<string>(() => makeSessionKey());
  const [historySeed, setHistorySeed] = useState<ThreadMessageLike[]>([]);
  const [historyCountBySession, setHistoryCountBySession] = useState<
    Record<string, number>
  >({});
  const [runtimeNonce, setRuntimeNonce] = useState<number>(0);
  const [error, setError] = useState<string>("");
  const [statusText, setStatusText] = useState<string>("Idle");
  const [replayNotice, setReplayNotice] = useState<string>("");
  const [sending, setSending] = useState<boolean>(false);
  const [appVersion, setAppVersion] = useState<string>("");

  // --- Session / history ---

  async function loadInitialConversation(): Promise<void> {
    setError("");
    const data = await api<{ sessions?: SessionItem[] }>("/api/sessions");
    const loaded = Array.isArray(data.sessions) ? data.sessions : [];
    setSessions(loaded);

    const latestSession = pickLatestSessionKey(loaded);
    const preferredSession = readSessionFromUrl();
    const preferredExists = preferredSession
      ? loaded.some((item) => item.session_key === preferredSession)
      : false;
    const initialSession = preferredExists ? preferredSession : latestSession;

    setSessionKey(initialSession);
    writeSessionToUrl(initialSession);
    const initialExists = loaded.some(
      (item) => item.session_key === initialSession,
    );
    if (initialExists) {
      await loadHistory(initialSession);
    } else {
      setHistorySeed([]);
      setHistoryCountBySession((prev) => ({ ...prev, [initialSession]: 0 }));
      setRuntimeNonce((x) => x + 1);
      setError("");
    }
  }

  async function loadSessions(): Promise<void> {
    try {
      const data = await api<{ sessions?: SessionItem[] }>("/api/sessions");
      setSessions(Array.isArray(data.sessions) ? data.sessions : []);
    } catch (e) {
      if (auth.isUnauthorizedError(e)) {
        auth.lockForAuth();
        return;
      }
      throw e;
    }
  }

  async function loadHistory(target = sessionKey): Promise<void> {
    try {
      const query = new URLSearchParams({ session_key: target, limit: "200" });
      const data = await api<{ messages?: BackendMessage[] }>(
        `/api/history?${query.toString()}`,
      );
      const rawMessages = Array.isArray(data.messages) ? data.messages : [];
      const mapped = mapBackendHistory(rawMessages);
      setHistorySeed(mapped);
      setHistoryCountBySession((prev) => ({
        ...prev,
        [target]: rawMessages.length,
      }));
      setRuntimeNonce((x) => x + 1);
      setError("");
    } catch (e) {
      if (auth.isUnauthorizedError(e)) {
        auth.lockForAuth();
        return;
      }
      if (e instanceof ApiError && e.status === 404) {
        setHistorySeed([]);
        setHistoryCountBySession((prev) => ({ ...prev, [target]: 0 }));
        setRuntimeNonce((x) => x + 1);
        setError("");
        return;
      }
      throw e;
    }
  }

  // --- Hooks ---

  const logoutCleanupRef = useRef(() => {});

  const auth = useAuth({
    setError,
    setStatusText,
    setSessions,
    setExtraSessions,
    setHistorySeed,
    setHistoryCountBySession,
    setRuntimeNonce,
    setSessionKey,
    setAppVersion,
    loadInitialConversation,
    onLogoutCleanup: () => logoutCleanupRef.current(),
  });

  const configHook = useConfig({
    appearance,
    isUnauthorizedError: auth.isUnauthorizedError,
    isForbiddenError: auth.isForbiddenError,
    lockForAuth: auth.lockForAuth,
  });

  const usage = useUsage({
    sessionKey,
    sessions,
    isUnauthorizedError: auth.isUnauthorizedError,
    isForbiddenError: auth.isForbiddenError,
    lockForAuth: auth.lockForAuth,
  });

  logoutCleanupRef.current = () => {
    usage.setUsageOpen(false);
    configHook.setConfigOpen(false);
  };

  // --- Computed values ---

  const sessionItems = useMemo(() => {
    const map = new Map<string, SessionItem>();

    for (const item of [...extraSessions, ...sessions]) {
      if (!map.has(item.session_key)) {
        map.set(item.session_key, item);
      }
    }

    const selectedMissingFromStoredList = !map.has(sessionKey);
    if (selectedMissingFromStoredList && !sessionKey.startsWith("chat:")) {
      map.set(sessionKey, {
        session_key: sessionKey,
        label: sessionKey,
        chat_id: 0,
        chat_type: "web",
      });
    }

    if (map.size === 0) {
      const key = makeSessionKey();
      map.set(key, {
        session_key: key,
        label: key,
        chat_id: 0,
        chat_type: "web",
      });
    }

    const items = Array.from(map.values());
    const selectedSynthetic =
      selectedMissingFromStoredList && !sessionKey.startsWith("chat:");
    items.sort((a, b) => {
      if (selectedSynthetic) {
        if (a.session_key === sessionKey) return -1;
        if (b.session_key === sessionKey) return 1;
      }

      const ta = Date.parse(a.last_message_time || "");
      const tb = Date.parse(b.last_message_time || "");
      const aOk = Number.isFinite(ta);
      const bOk = Number.isFinite(tb);
      if (aOk && bOk) {
        if (tb !== ta) return tb - ta;
      } else if (aOk !== bOk) {
        return aOk ? -1 : 1;
      }

      return a.label.localeCompare(b.label);
    });
    return items;
  }, [extraSessions, sessions, sessionKey]);

  const selectedSession = useMemo(
    () => sessionItems.find((item) => item.session_key === sessionKey),
    [sessionItems, sessionKey],
  );

  const selectedSessionLabel = selectedSession?.label || sessionKey;
  const selectedSessionReadOnly = Boolean(
    selectedSession && selectedSession.chat_type !== "web",
  );

  const adapter = useChatAdapter({
    sessionKey,
    selectedSessionReadOnly,
    isUnauthorizedError: auth.isUnauthorizedError,
    lockForAuth: auth.lockForAuth,
    loadSessions,
    loadHistory,
    setSending,
    setStatusText,
    setReplayNotice,
    setError,
  });

  // --- Session management ---

  function createSession(): void {
    const currentCount =
      historyCountBySession[sessionKey] ?? historySeed.length;
    if (currentCount === 0) {
      setStatusText("Current session is empty. Reuse this session.");
      return;
    }

    const key = makeSessionKey();
    const nowIso = new Date().toISOString();
    const item: SessionItem = {
      session_key: key,
      label: key,
      chat_id: 0,
      chat_type: "web",
      last_message_time: nowIso,
    };
    setExtraSessions((prev) =>
      prev.some((v) => v.session_key === key) ? prev : [item, ...prev],
    );
    setSessionKey(key);
    setHistoryCountBySession((prev) => ({ ...prev, [key]: 0 }));
    setHistorySeed([]);
    setRuntimeNonce((x) => x + 1);
    setReplayNotice("");
    setError("");
    setStatusText("Idle");
  }

  function toggleAppearance(): void {
    setAppearance((prev) => (prev === "dark" ? "light" : "dark"));
  }

  async function onResetSessionByKey(targetSession: string): Promise<void> {
    try {
      await api("/api/reset", {
        method: "POST",
        body: JSON.stringify({ session_key: targetSession }),
      });
      if (targetSession === sessionKey) {
        await loadHistory(targetSession);
      }
      await loadSessions();
      setStatusText("Session reset");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function onRefreshSessionByKey(targetSession: string): Promise<void> {
    try {
      if (targetSession === sessionKey) {
        await loadHistory(targetSession);
      }
      await loadSessions();
      setStatusText("Session refreshed");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function onDeleteSessionByKey(targetSession: string): Promise<void> {
    try {
      const resp = await api<{ deleted?: boolean }>("/api/delete_session", {
        method: "POST",
        body: JSON.stringify({ session_key: targetSession }),
      });

      if (resp.deleted === false) {
        setStatusText("No session data found to delete.");
      }

      setExtraSessions((prev) =>
        prev.filter((s) => s.session_key !== targetSession),
      );
      setHistoryCountBySession((prev) => {
        const next = { ...prev };
        delete next[targetSession];
        return next;
      });

      const fallback =
        sessionItems.find((item) => item.session_key !== targetSession)
          ?.session_key || makeSessionKey();
      if (targetSession === sessionKey) {
        setSessionKey(fallback);
        await loadHistory(fallback);
      }
      await loadSessions();
      if (resp.deleted !== false) {
        setStatusText("Session deleted");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  // --- Effects ---

  useEffect(() => {
    saveAppearance(appearance);
    document.documentElement.classList.toggle("dark", appearance === "dark");
  }, [appearance]);

  useEffect(() => {
    saveUiTheme(uiTheme);
    document.documentElement.setAttribute("data-ui-theme", uiTheme);
  }, [uiTheme]);

  useEffect(() => {
    (async () => {
      try {
        const authStatus = await auth.refreshAuthStatus();
        if (authStatus.authenticated) {
          await auth.loadAppVersion();
        }
        if (!authStatus.has_password || !authStatus.authenticated) return;
        await loadInitialConversation();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!auth.authAuthenticated) return;
    const existsOnServer = sessions.some(
      (item) => item.session_key === sessionKey,
    );
    if (!existsOnServer) {
      setHistorySeed([]);
      setHistoryCountBySession((prev) => ({ ...prev, [sessionKey]: 0 }));
      setRuntimeNonce((x) => x + 1);
      setError("");
      return;
    }
    loadHistory(sessionKey).catch((e) =>
      setError(e instanceof Error ? e.message : String(e)),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionKey, auth.authAuthenticated, sessions]);

  useEffect(() => {
    if (!auth.authAuthenticated) return;
    auth.loadAppVersion().catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [auth.authAuthenticated]);

  useEffect(() => {
    writeSessionToUrl(sessionKey);
  }, [sessionKey]);

  // --- Derived ---

  const runtimeKey = `${sessionKey}-${runtimeNonce}`;
  const radixAccent = RADIX_ACCENT_BY_THEME[uiTheme] ?? "green";

  const settingsContext: SettingsContextValue = useMemo(
    () => ({
      configDraft: configHook.configDraft,
      setConfigField: configHook.setConfigField,
      resetConfigField: configHook.resetConfigField,
      soulFiles: configHook.soulFiles,
      appearance,
      config: configHook.config,
      configSelfCheck: configHook.configSelfCheck,
      providerProfileDrafts: configHook.providerProfileDrafts,
      updateProviderProfile: configHook.updateProviderProfile,
      addProviderProfile: configHook.addProviderProfile,
      cloneProviderProfile: configHook.cloneProviderProfile,
      removeProviderProfile: configHook.removeProviderProfile,
      resetRefsAndRemoveProviderProfile:
        configHook.resetRefsAndRemoveProviderProfile,
      updateA2APeer: configHook.updateA2APeer,
      addA2APeer: configHook.addA2APeer,
      removeA2APeer: configHook.removeA2APeer,
      sectionCardClass: configHook.sectionCardClass,
      sectionCardStyle: configHook.sectionCardStyle,
      toggleCardClass: configHook.toggleCardClass,
      toggleCardStyle: configHook.toggleCardStyle,
    }),
    [
      configHook.configDraft,
      configHook.soulFiles,
      appearance,
      configHook.config,
      configHook.configSelfCheck,
      configHook.providerProfileDrafts,
      configHook.sectionCardClass,
      configHook.sectionCardStyle,
      configHook.toggleCardClass,
      configHook.toggleCardStyle,
    ],
  );

  // --- Render ---

  return (
    <Theme
      appearance={appearance}
      accentColor={radixAccent as never}
      grayColor="slate"
      radius="medium"
      scaling="100%"
    >
      <div
        className={
          appearance === "dark"
            ? "h-screen w-screen bg-[var(--mc-bg-main)]"
            : "h-screen w-screen bg-[radial-gradient(1200px_560px_at_-8%_-10%,#d1fae5_0%,transparent_58%),radial-gradient(1200px_560px_at_108%_-12%,#e0f2fe_0%,transparent_58%),#f8fafc]"
        }
      >
        <div
          className={
            desktopSidebarOpen
              ? "grid h-full min-h-0 grid-cols-1 md:grid-cols-[320px_minmax(0,1fr)]"
              : "grid h-full min-h-0 grid-cols-1"
          }
        >
          {desktopSidebarOpen ? (
            <div className="hidden md:block md:h-full md:min-h-0 md:overflow-hidden">
              <SessionSidebar
                appearance={appearance}
                onToggleAppearance={toggleAppearance}
                onToggleDesktopSidebar={() => setDesktopSidebarOpen(false)}
                uiTheme={uiTheme}
                onUiThemeChange={(theme) => setUiTheme(theme as UiTheme)}
                uiThemeOptions={UI_THEME_OPTIONS}
                sessionItems={sessionItems}
                selectedSessionKey={sessionKey}
                onSessionSelect={(key) => setSessionKey(key)}
                onRefreshSession={(key) => void onRefreshSessionByKey(key)}
                onResetSession={(key) => void onResetSessionByKey(key)}
                onDeleteSession={(key) => void onDeleteSessionByKey(key)}
                onOpenConfig={configHook.openConfig}
                onOpenUsage={() => usage.openUsage(sessionKey)}
                onNewSession={createSession}
                appVersion={auth.appVersion}
              />
            </div>
          ) : null}

          {mobileSidebarOpen ? (
            <div className="fixed inset-0 z-40 md:hidden">
              <button
                type="button"
                aria-label="Close sessions sidebar"
                className="absolute inset-0 bg-black/45"
                onClick={() => setMobileSidebarOpen(false)}
              />
              <div
                className={
                  appearance === "dark"
                    ? "relative h-full w-[min(92vw,340px)] border-r border-[color:var(--mc-border-soft)] bg-[var(--mc-bg-sidebar)]"
                    : "relative h-full w-[min(92vw,340px)] border-r border-slate-200 bg-white"
                }
              >
                <SessionSidebar
                  appearance={appearance}
                  onToggleAppearance={toggleAppearance}
                  uiTheme={uiTheme}
                  onUiThemeChange={(theme) => setUiTheme(theme as UiTheme)}
                  uiThemeOptions={UI_THEME_OPTIONS}
                  sessionItems={sessionItems}
                  selectedSessionKey={sessionKey}
                  onSessionSelect={(key) => {
                    setSessionKey(key);
                    setMobileSidebarOpen(false);
                  }}
                  onRefreshSession={(key) => {
                    void onRefreshSessionByKey(key);
                    setMobileSidebarOpen(false);
                  }}
                  onResetSession={(key) => {
                    void onResetSessionByKey(key);
                    setMobileSidebarOpen(false);
                  }}
                  onDeleteSession={(key) => {
                    void onDeleteSessionByKey(key);
                    setMobileSidebarOpen(false);
                  }}
                  onOpenConfig={async () => {
                    setMobileSidebarOpen(false);
                    await configHook.openConfig();
                  }}
                  onOpenUsage={async () => {
                    setMobileSidebarOpen(false);
                    await usage.openUsage(sessionKey);
                  }}
                  onNewSession={() => {
                    createSession();
                    setMobileSidebarOpen(false);
                  }}
                  appVersion={auth.appVersion}
                />
              </div>
            </div>
          ) : null}

          <main
            className={
              appearance === "dark"
                ? "flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-[var(--mc-bg-panel)]"
                : "flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-white/95"
            }
          >
            <header
              className={
                appearance === "dark"
                  ? "sticky top-0 z-10 border-b border-[color:var(--mc-border-soft)] bg-[color:var(--mc-bg-panel)]/95 px-4 py-3 backdrop-blur-sm"
                  : "sticky top-0 z-10 border-b border-slate-200 bg-white/92 px-4 py-3 backdrop-blur-sm"
              }
            >
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => setMobileSidebarOpen(true)}
                  aria-label="Open sessions sidebar"
                  className={
                    appearance === "dark"
                      ? "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-[color:var(--mc-border-soft)] text-slate-200 md:hidden"
                      : "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-slate-300 text-slate-700 md:hidden"
                  }
                >
                  ☰
                </button>
                {!desktopSidebarOpen ? (
                  <button
                    type="button"
                    onClick={() => setDesktopSidebarOpen(true)}
                    aria-label="Expand sessions sidebar"
                    className={
                      appearance === "dark"
                        ? "hidden md:inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-[color:var(--mc-border-soft)] text-slate-200"
                        : "hidden md:inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-slate-300 text-slate-700"
                    }
                  >
                    ⟩
                  </button>
                ) : null}
                <Heading size="6" className="min-w-0 truncate">
                  {selectedSessionLabel}
                </Heading>
              </div>
            </header>

            <div
              className={
                appearance === "dark"
                  ? "flex min-h-0 flex-1 flex-col bg-[linear-gradient(to_bottom,var(--mc-bg-panel),var(--mc-bg-main)_28%)]"
                  : "flex min-h-0 flex-1 flex-col bg-[linear-gradient(to_bottom,#f8fafc,white_20%)]"
              }
            >
              <div className="mx-auto w-full max-w-5xl px-3 pt-3">
                {replayNotice ? (
                  <Callout.Root color="orange" size="1" variant="soft">
                    <Callout.Text>{replayNotice}</Callout.Text>
                  </Callout.Root>
                ) : null}
                {error ? (
                  <Callout.Root
                    color="red"
                    size="1"
                    variant="soft"
                    className={replayNotice ? "mt-2" : ""}
                  >
                    <Callout.Text>{error}</Callout.Text>
                  </Callout.Root>
                ) : null}
              </div>

              <div className="min-h-0 flex-1 px-1 pb-1">
                <ThreadPane
                  key={runtimeKey}
                  adapter={adapter}
                  initialMessages={historySeed}
                  runtimeKey={runtimeKey}
                />
              </div>
            </div>
          </main>
        </div>
        <AuthDialogs
          authReady={auth.authReady}
          authHasPassword={auth.authHasPassword}
          authAuthenticated={auth.authAuthenticated}
          authUsingDefaultPassword={auth.authUsingDefaultPassword}
          authMessage={auth.authMessage}
          authBusy={auth.authBusy}
          bootstrapToken={auth.bootstrapToken}
          setBootstrapToken={auth.setBootstrapToken}
          bootstrapPassword={auth.bootstrapPassword}
          setBootstrapPassword={auth.setBootstrapPassword}
          bootstrapConfirm={auth.bootstrapConfirm}
          setBootstrapConfirm={auth.setBootstrapConfirm}
          generatedPasswordPreview={auth.generatedPasswordPreview}
          onGeneratePassword={auth.onGeneratePassword}
          onSubmitBootstrapPassword={() => void auth.submitBootstrapPassword()}
          loginPassword={auth.loginPassword}
          setLoginPassword={auth.setLoginPassword}
          onSubmitLogin={(pw) => void auth.submitLogin(pw)}
          passwordPromptOpen={auth.passwordPromptOpen}
          setPasswordPromptOpen={auth.setPasswordPromptOpen}
          passwordPromptMessage={auth.passwordPromptMessage}
          passwordPromptBusy={auth.passwordPromptBusy}
          newPassword={auth.newPassword}
          setNewPassword={auth.setNewPassword}
          newPasswordConfirm={auth.newPasswordConfirm}
          setNewPasswordConfirm={auth.setNewPasswordConfirm}
          onSubmitPasswordUpdate={() => void auth.submitPasswordUpdate()}
        />
        <SettingsDialog
          configOpen={configHook.configOpen}
          setConfigOpen={configHook.setConfigOpen}
          configLoading={configHook.configLoading}
          configLoadStage={configHook.configLoadStage}
          configLoadError={configHook.configLoadError}
          config={configHook.config}
          configSelfCheck={configHook.configSelfCheck}
          configSelfCheckLoading={configHook.configSelfCheckLoading}
          configSelfCheckError={configHook.configSelfCheckError}
          saveStatus={configHook.saveStatus}
          onSave={() => void configHook.saveConfigChanges()}
          appearance={appearance}
          authAuthenticated={auth.authAuthenticated}
          onLogout={() => void auth.logout()}
          settingsContext={settingsContext}
        />
        <UsagePanel
          open={usage.usageOpen}
          onOpenChange={usage.setUsageOpen}
          usageSession={usage.usageSession}
          sessionKey={sessionKey}
          usageLoading={usage.usageLoading}
          usageError={usage.usageError}
          usageReport={usage.usageReport}
          usageMemory={usage.usageMemory}
          usageSubagents={usage.usageSubagents}
          reflectorRuns={usage.usageReflectorRuns}
          injectionLogs={usage.usageInjectionLogs}
          onRefreshCurrent={() => void usage.openUsage(sessionKey)}
          onRefreshThis={() =>
            void usage.openUsage(usage.usageSession || sessionKey)
          }
        />
      </div>
    </Theme>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
