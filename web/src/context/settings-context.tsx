import React, { createContext, useContext } from "react";
import type {
  Appearance,
  ConfigPayload,
  A2APeerDraft,
  ProviderProfileDraft,
  ConfigSelfCheck,
} from "../lib/types";

export type SettingsContextValue = {
  configDraft: Record<string, unknown>;
  setConfigField: (field: string, value: unknown) => void;
  resetConfigField: (field: string) => void;
  soulFiles: string[];
  appearance: Appearance;
  config: ConfigPayload | null;
  configSelfCheck: ConfigSelfCheck | null;
  providerProfileDrafts: ProviderProfileDraft[];
  updateProviderProfile: (
    index: number,
    patch: Partial<ProviderProfileDraft>,
  ) => void;
  addProviderProfile: () => void;
  cloneProviderProfile: (index: number) => void;
  removeProviderProfile: (index: number) => void;
  resetRefsAndRemoveProviderProfile: (index: number) => void;
  updateA2APeer: (index: number, patch: Partial<A2APeerDraft>) => void;
  addA2APeer: () => void;
  removeA2APeer: (index: number) => void;
  sectionCardClass: string;
  sectionCardStyle: React.CSSProperties | undefined;
  toggleCardClass: string;
  toggleCardStyle: React.CSSProperties | undefined;
};

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function useSettings(): SettingsContextValue {
  const ctx = useContext(SettingsContext);
  if (!ctx) {
    throw new Error("useSettings must be used within a SettingsProvider");
  }
  return ctx;
}

export const SettingsProvider = SettingsContext.Provider;
