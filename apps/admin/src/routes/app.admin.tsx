import { Stack, Text, Title } from "@mantine/core";
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/app/admin")({
  component: Component,
});

function Component() {
  return (
    <Stack gap="xl">
      <div>
        <Title order={1}>
          Admin
        </Title>
        <Text c="dimmed" size="sm">
          Only owner can access this page
        </Text>
      </div>
    </Stack>
  );
}
