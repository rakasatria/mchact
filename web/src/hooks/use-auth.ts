import { useState } from "react";
import type { ThreadMessageLike } from "@assistant-ui/react";

import { api, ApiError } from "../lib/api";
import type { AuthStatusResponse, HealthResponse } from "../lib/types";
import type { SessionItem } from "../types";
import {
  readBootstrapTokenFromHash,
  clearBootstrapTokenFromHash,
  makeSessionKey,
  generatePassword,
} from "../lib/session-utils";

export type UseAuthDeps = {
  setError: (msg: string) => void;
  setStatusText: (msg: string) => void;
  setSessions: React.Dispatch<React.SetStateAction<SessionItem[]>>;
  setExtraSessions: React.Dispatch<React.SetStateAction<SessionItem[]>>;
  setHistorySeed: React.Dispatch<React.SetStateAction<ThreadMessageLike[]>>;
  setHistoryCountBySession: React.Dispatch<
    React.SetStateAction<Record<string, number>>
  >;
  setRuntimeNonce: React.Dispatch<React.SetStateAction<number>>;
  setSessionKey: React.Dispatch<React.SetStateAction<string>>;
  setAppVersion: React.Dispatch<React.SetStateAction<string>>;
  loadInitialConversation: () => Promise<void>;
  onLogoutCleanup: () => void;
};

export function useAuth(deps: UseAuthDeps) {
  const {
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
    onLogoutCleanup,
  } = deps;

  const [authReady, setAuthReady] = useState<boolean>(false);
  const [authHasPassword, setAuthHasPassword] = useState<boolean>(false);
  const [authAuthenticated, setAuthAuthenticated] = useState<boolean>(false);
  const [authUsingDefaultPassword, setAuthUsingDefaultPassword] =
    useState<boolean>(false);
  const [authMessage, setAuthMessage] = useState<string>("");
  const [loginPassword, setLoginPassword] = useState<string>("");
  const [bootstrapToken, setBootstrapToken] = useState<string>(() =>
    readBootstrapTokenFromHash(),
  );
  const [bootstrapPassword, setBootstrapPassword] = useState<string>("");
  const [bootstrapConfirm, setBootstrapConfirm] = useState<string>("");
  const [generatedPasswordPreview, setGeneratedPasswordPreview] =
    useState<string>("");
  const [authBusy, setAuthBusy] = useState<boolean>(false);
  const [passwordPromptOpen, setPasswordPromptOpen] = useState<boolean>(false);
  const [passwordPromptMessage, setPasswordPromptMessage] =
    useState<string>("");
  const [passwordPromptBusy, setPasswordPromptBusy] = useState<boolean>(false);
  const [newPassword, setNewPassword] = useState<string>("");
  const [newPasswordConfirm, setNewPasswordConfirm] = useState<string>("");
  const [appVersion, setAppVersionLocal] = useState<string>("");

  function isUnauthorizedError(err: unknown): boolean {
    return err instanceof ApiError && err.status === 401;
  }

  function isForbiddenError(err: unknown): boolean {
    return err instanceof ApiError && err.status === 403;
  }

  function lockForAuth(
    message = "Authentication required. Please sign in.",
  ): void {
    setAuthAuthenticated(false);
    setAuthMessage(message);
    setError(message);
  }

  async function refreshAuthStatus(): Promise<AuthStatusResponse> {
    try {
      const data = await api<AuthStatusResponse>("/api/auth/status");
      const hasPassword = Boolean(data.has_password);
      const authenticated = Boolean(data.authenticated);
      const usingDefaultPassword = Boolean(data.using_default_password);
      setAuthHasPassword(hasPassword);
      setAuthAuthenticated(authenticated);
      setAuthUsingDefaultPassword(usingDefaultPassword);
      setAuthReady(true);
      if (authenticated) {
        setAuthMessage("");
        setError("");
      }
      if (authenticated && usingDefaultPassword) {
        setPasswordPromptOpen(true);
      }
      if (!usingDefaultPassword) {
        setPasswordPromptOpen(false);
      }
      return data;
    } catch (e) {
      setAuthReady(true);
      throw e;
    }
  }

  async function loadAppVersion(): Promise<void> {
    try {
      const data = await api<HealthResponse>("/api/health");
      const version = String(data.version || "").trim();
      setAppVersionLocal(version);
      setAppVersion(version);
    } catch {
      // `/api/health` requires read scope; keep placeholder when unavailable.
    }
  }

  async function submitLogin(password: string): Promise<void> {
    const normalized = password.trim();
    if (!normalized) {
      setAuthMessage("Please enter your password.");
      return;
    }
    setAuthBusy(true);
    setAuthMessage("");
    try {
      await api("/api/auth/login", {
        method: "POST",
        body: JSON.stringify({ password: normalized }),
      });
      setLoginPassword("");
      await refreshAuthStatus();
      await loadInitialConversation();
      setStatusText("Authenticated");
    } catch (e) {
      if (e instanceof ApiError) {
        if (e.status === 401) {
          setAuthMessage(
            "Password is incorrect. Please try again or reset with `mchact web password-generate`.",
          );
          return;
        }
        if (e.status === 429) {
          setAuthMessage("Too many login attempts. Please wait and retry.");
          return;
        }
      }
      setAuthMessage(e instanceof Error ? e.message : String(e));
    } finally {
      setAuthBusy(false);
    }
  }

  async function submitBootstrapPassword(): Promise<void> {
    const token = bootstrapToken.trim();
    const password = bootstrapPassword.trim();
    const confirm = bootstrapConfirm.trim();

    if (!token) {
      setAuthMessage("Bootstrap token is required.");
      return;
    }
    if (password.length < 8) {
      setAuthMessage("Password must be at least 8 characters.");
      return;
    }
    if (password !== confirm) {
      setAuthMessage("Passwords do not match.");
      return;
    }

    setAuthBusy(true);
    setAuthMessage("");
    try {
      await api("/api/auth/password", {
        method: "POST",
        headers: { "x-bootstrap-token": token },
        body: JSON.stringify({ password }),
      });
      clearBootstrapTokenFromHash();
      await submitLogin(password);
      setBootstrapPassword("");
      setBootstrapConfirm("");
      setGeneratedPasswordPreview("");
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) {
        setAuthMessage(
          "Bootstrap token is invalid or expired. Please copy the latest token from startup logs.",
        );
        return;
      }
      setAuthMessage(e instanceof Error ? e.message : String(e));
    } finally {
      setAuthBusy(false);
    }
  }

  async function submitPasswordUpdate(): Promise<void> {
    const password = newPassword.trim();
    const confirm = newPasswordConfirm.trim();
    if (password.length < 8) {
      setPasswordPromptMessage("Password must be at least 8 characters.");
      return;
    }
    if (password !== confirm) {
      setPasswordPromptMessage("Passwords do not match.");
      return;
    }
    setPasswordPromptBusy(true);
    setPasswordPromptMessage("");
    try {
      await api("/api/auth/password", {
        method: "POST",
        body: JSON.stringify({ password }),
      });
      setNewPassword("");
      setNewPasswordConfirm("");
      await refreshAuthStatus();
      setPasswordPromptOpen(false);
      setStatusText("Password updated");
    } catch (e) {
      setPasswordPromptMessage(e instanceof Error ? e.message : String(e));
    } finally {
      setPasswordPromptBusy(false);
    }
  }

  async function logout(): Promise<void> {
    setStatusText("Signing out...");
    setError("");
    try {
      await api("/api/auth/logout", { method: "POST" });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSessions([]);
      setExtraSessions([]);
      setHistorySeed([]);
      setHistoryCountBySession({});
      setRuntimeNonce((x) => x + 1);
      setAppVersionLocal("");
      setAppVersion("");
      setSessionKey(makeSessionKey());
      onLogoutCleanup();
      await refreshAuthStatus().catch(() => {
        setAuthAuthenticated(false);
      });
      setAuthMessage("Signed out. Please sign in again.");
      setStatusText("Signed out");
    }
  }

  function onGeneratePassword(): void {
    const next = generatePassword();
    setBootstrapPassword(next);
    setBootstrapConfirm(next);
    setGeneratedPasswordPreview(next);
  }

  return {
    // state
    authReady,
    authHasPassword,
    authAuthenticated,
    authUsingDefaultPassword,
    authMessage,
    loginPassword,
    setLoginPassword,
    bootstrapToken,
    setBootstrapToken,
    bootstrapPassword,
    setBootstrapPassword,
    bootstrapConfirm,
    setBootstrapConfirm,
    generatedPasswordPreview,
    authBusy,
    passwordPromptOpen,
    setPasswordPromptOpen,
    passwordPromptMessage,
    passwordPromptBusy,
    newPassword,
    setNewPassword,
    newPasswordConfirm,
    setNewPasswordConfirm,
    appVersion,
    // functions
    isUnauthorizedError,
    isForbiddenError,
    lockForAuth,
    refreshAuthStatus,
    loadAppVersion,
    submitLogin,
    submitBootstrapPassword,
    submitPasswordUpdate,
    logout,
    onGeneratePassword,
  };
}
