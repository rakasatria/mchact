import { Select, TextField, Flex, Text } from "@radix-ui/themes";
import { soulPickerValue, normalizeSoulPathInput } from "../lib/config-helpers";

export type SoulPathPickerFieldProps = {
  value: unknown;
  soulsDir?: unknown;
  soulFiles: string[];
  onChange: (next: string) => void;
};

export function SoulPathPickerField({
  value,
  soulsDir,
  soulFiles,
  onChange,
}: SoulPathPickerFieldProps) {
  const pickerVal = soulPickerValue(value, soulFiles, soulsDir);
  const soulsDirText = String(soulsDir || "").trim() || "souls";
  return (
    <Flex direction="column" gap="2">
      <Select.Root
        value={pickerVal}
        onValueChange={(next) => {
          if (next === "__none__") {
            onChange("");
            return;
          }
          if (next === "__custom__") return;
          onChange(normalizeSoulPathInput(next, soulsDir));
        }}
      >
        <Select.Trigger
          className="w-full mc-select-trigger-full"
          placeholder="Select soul file"
        />
        <Select.Content position="popper">
          <Select.Item value="__none__">(None)</Select.Item>
          <Select.Item value="__custom__">Custom filename/path</Select.Item>
          {soulFiles.map((name) => (
            <Select.Item key={name} value={name}>
              {name}
            </Select.Item>
          ))}
        </Select.Content>
      </Select.Root>
      {pickerVal === "__custom__" ? (
        <TextField.Root
          value={String(value || "")}
          onChange={(e) => onChange(e.target.value)}
          placeholder="my-bot.md or /abs/path/my-bot.md"
        />
      ) : null}
      <Text size="1" color="gray">
        Select from <code>{soulsDirText}/*.md</code> or use custom input (file
        may not exist yet).
      </Text>
    </Flex>
  );
}
