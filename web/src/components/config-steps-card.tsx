import React from "react";
import { Card, Text } from "@radix-ui/themes";

export type ConfigStepsCardProps = {
  title?: string;
  steps: React.ReactNode[];
};

export function ConfigStepsCard({
  title = "Setup Steps",
  steps,
}: ConfigStepsCardProps) {
  return (
    <Card className="mt-3 p-3">
      <Text size="2" weight="bold">
        {title}
      </Text>
      <ol className="mt-2 list-decimal space-y-1 pl-5 text-sm text-slate-400">
        {steps.map((step, index) => (
          <li key={index}>{step}</li>
        ))}
      </ol>
    </Card>
  );
}
