import React from "react";
import { Button, Callout, Dialog, TextField } from "@radix-ui/themes";
import { ConfigFieldCard } from "./config-field-card";

export type AuthDialogsProps = {
  authReady: boolean;
  authHasPassword: boolean;
  authAuthenticated: boolean;
  authUsingDefaultPassword: boolean;
  authMessage: string;
  authBusy: boolean;
  bootstrapToken: string;
  setBootstrapToken: (value: string) => void;
  bootstrapPassword: string;
  setBootstrapPassword: (value: string) => void;
  bootstrapConfirm: string;
  setBootstrapConfirm: (value: string) => void;
  generatedPasswordPreview: string;
  onGeneratePassword: () => void;
  onSubmitBootstrapPassword: () => void;
  loginPassword: string;
  setLoginPassword: (value: string) => void;
  onSubmitLogin: (password: string) => void;
  passwordPromptOpen: boolean;
  setPasswordPromptOpen: (open: boolean) => void;
  passwordPromptMessage: string;
  passwordPromptBusy: boolean;
  newPassword: string;
  setNewPassword: (value: string) => void;
  newPasswordConfirm: string;
  setNewPasswordConfirm: (value: string) => void;
  onSubmitPasswordUpdate: () => void;
};

export function AuthDialogs(props: AuthDialogsProps): React.ReactElement {
  const {
    authReady,
    authHasPassword,
    authAuthenticated,
    authUsingDefaultPassword,
    authMessage,
    authBusy,
    bootstrapToken,
    setBootstrapToken,
    bootstrapPassword,
    setBootstrapPassword,
    bootstrapConfirm,
    setBootstrapConfirm,
    generatedPasswordPreview,
    onGeneratePassword,
    onSubmitBootstrapPassword,
    loginPassword,
    setLoginPassword,
    onSubmitLogin,
    passwordPromptOpen,
    setPasswordPromptOpen,
    passwordPromptMessage,
    passwordPromptBusy,
    newPassword,
    setNewPassword,
    newPasswordConfirm,
    setNewPasswordConfirm,
    onSubmitPasswordUpdate,
  } = props;

  return (
    <>
      <Dialog.Root open={authReady && !authHasPassword}>
        <Dialog.Content maxWidth="520px">
          <Dialog.Title>Set Operator Password</Dialog.Title>
          <Dialog.Description size="2">
            First-time setup: set an admin password using the bootstrap token
            from server logs.
          </Dialog.Description>
          <div className="mt-4 space-y-3">
            <ConfigFieldCard
              label="Bootstrap Token"
              description={
                <>
                  Copy <code>x-bootstrap-token</code> from MicroClaw startup
                  logs.
                </>
              }
            >
              <TextField.Root
                className="mt-2"
                value={bootstrapToken}
                onChange={(e) => setBootstrapToken(e.target.value)}
                placeholder="902439dd-a93b-4c66-81bb-7ffba0057936"
              />
            </ConfigFieldCard>
            <ConfigFieldCard
              label="Password"
              description={<>At least 8 characters.</>}
            >
              <TextField.Root
                className="mt-2"
                type="password"
                value={bootstrapPassword}
                onChange={(e) => setBootstrapPassword(e.target.value)}
                placeholder="********"
              />
              <div className="mt-2 flex items-center justify-end">
                <Button
                  size="1"
                  variant="soft"
                  onClick={onGeneratePassword}
                  disabled={authBusy}
                >
                  Generate Password
                </Button>
              </div>
            </ConfigFieldCard>
            <ConfigFieldCard
              label="Confirm Password"
              description={<>Re-enter the same password.</>}
            >
              <TextField.Root
                className="mt-2"
                type="password"
                value={bootstrapConfirm}
                onChange={(e) => setBootstrapConfirm(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") onSubmitBootstrapPassword();
                }}
                placeholder="********"
              />
            </ConfigFieldCard>
          </div>
          {authMessage ? (
            <Callout.Root
              color="red"
              size="1"
              variant="soft"
              className="mt-3"
            >
              <Callout.Text>{authMessage}</Callout.Text>
            </Callout.Root>
          ) : null}
          {generatedPasswordPreview ? (
            <Callout.Root
              color="green"
              size="1"
              variant="soft"
              className="mt-3"
            >
              <Callout.Text>
                Generated password: <code>{generatedPasswordPreview}</code>
              </Callout.Text>
            </Callout.Root>
          ) : null}
          <div className="mt-4 flex justify-end">
            <Button
              onClick={onSubmitBootstrapPassword}
              disabled={authBusy}
            >
              {authBusy ? "Applying..." : "Set Password"}
            </Button>
          </div>
        </Dialog.Content>
      </Dialog.Root>
      <Dialog.Root open={authReady && authHasPassword && !authAuthenticated}>
        <Dialog.Content maxWidth="460px">
          <Dialog.Title>Sign In</Dialog.Title>
          <Dialog.Description size="2">
            Enter your operator password to access sessions and history.
          </Dialog.Description>
          {authUsingDefaultPassword ? (
            <Callout.Root
              color="orange"
              size="1"
              variant="soft"
              className="mt-2"
            >
              <Callout.Text>
                No custom password is set yet. Temporary default password:{" "}
                <code>helloworld</code>
              </Callout.Text>
            </Callout.Root>
          ) : null}
          <ConfigFieldCard
            label="Password"
            description={<>Use the password configured for this Web UI.</>}
          >
            <TextField.Root
              className="mt-2"
              type="password"
              value={loginPassword}
              onChange={(e) => setLoginPassword(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") onSubmitLogin(loginPassword);
              }}
              placeholder="********"
            />
          </ConfigFieldCard>
          {authMessage ? (
            <Callout.Root
              color="red"
              size="1"
              variant="soft"
              className="mt-3"
            >
              <Callout.Text>{authMessage}</Callout.Text>
            </Callout.Root>
          ) : null}
          <div className="mt-4 flex justify-end">
            <Button
              onClick={() => onSubmitLogin(loginPassword)}
              disabled={authBusy}
            >
              {authBusy ? "Signing in..." : "Sign In"}
            </Button>
          </div>
        </Dialog.Content>
      </Dialog.Root>
      <Dialog.Root
        open={
          authReady &&
          authAuthenticated &&
          authUsingDefaultPassword &&
          passwordPromptOpen
        }
      >
        <Dialog.Content maxWidth="520px">
          <Dialog.Title>Change Default Password</Dialog.Title>
          <Dialog.Description size="2">
            You are using the default password <code>helloworld</code>. Set a
            new password now, or skip for now.
          </Dialog.Description>
          <div className="mt-4 space-y-3">
            <ConfigFieldCard
              label="New Password"
              description={<>At least 8 characters.</>}
            >
              <TextField.Root
                className="mt-2"
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                placeholder="********"
              />
            </ConfigFieldCard>
            <ConfigFieldCard
              label="Confirm Password"
              description={<>Re-enter the new password.</>}
            >
              <TextField.Root
                className="mt-2"
                type="password"
                value={newPasswordConfirm}
                onChange={(e) => setNewPasswordConfirm(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") onSubmitPasswordUpdate();
                }}
                placeholder="********"
              />
            </ConfigFieldCard>
          </div>
          {passwordPromptMessage ? (
            <Callout.Root
              color="red"
              size="1"
              variant="soft"
              className="mt-3"
            >
              <Callout.Text>{passwordPromptMessage}</Callout.Text>
            </Callout.Root>
          ) : null}
          <div className="mt-4 flex justify-end gap-2">
            <Button
              variant="soft"
              onClick={() => setPasswordPromptOpen(false)}
              disabled={passwordPromptBusy}
            >
              Skip for now
            </Button>
            <Button
              onClick={onSubmitPasswordUpdate}
              disabled={passwordPromptBusy}
            >
              {passwordPromptBusy ? "Updating..." : "Update Password"}
            </Button>
          </div>
        </Dialog.Content>
      </Dialog.Root>
    </>
  );
}
