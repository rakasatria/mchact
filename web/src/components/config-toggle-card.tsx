import React from "react";
import { Flex, Text, Switch } from "@radix-ui/themes";

export type ConfigToggleCardProps = {
  label: string;
  description?: React.ReactNode;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  className: string;
  style?: React.CSSProperties;
};

export function ConfigToggleCard({
  label,
  description,
  checked,
  onCheckedChange,
  className,
  style,
}: ConfigToggleCardProps) {
  return (
    <div className={className} style={style}>
      <Flex justify="between" align="center">
        <div>
          <Text size="2">{label}</Text>
          {description ? (
            <Text size="1" color="gray" className="mt-1 block">
              {description}
            </Text>
          ) : null}
        </div>
        <Switch checked={checked} onCheckedChange={onCheckedChange} />
      </Flex>
    </div>
  );
}
