import {
  ActionIcon,
  Alert,
  Button,
  Card,
  CopyButton,
  Group,
  Modal,
  Stack,
  Tabs,
  Text,
  TextInput,
  Title,
  Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
  IconBuilding,
  IconCheck,
  IconCopy,
  IconDeviceIpadHorizontalPin,
  IconHelp,
  IconInfoCircle,
  IconKey,
  IconRefresh,
  IconSettings,
  IconUser,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { createFileRoute, redirect, useNavigate } from "@tanstack/react-router";
import { ReactNode } from "react";
import { z } from "zod";

import { getUserRole } from "@/services/auth.api";
import { getOrganizationConfig } from "@/services/config.api";

export const Route = createFileRoute("/app/settings")({
  validateSearch: z.object({
    tab: z.enum(["personal", "organization"]).default("personal"),
  }),
  component: Component,
  loaderDeps: ({ search: { tab } }) => ({ tab }),
  loader: async ({ context: { userSession }, deps: { tab } }) => {
    const role = await getUserRole();
    const isAdmin = role === "owner";

    if (!isAdmin && tab === "organization") {
      throw redirect({ to: "/app/settings", search: { tab: "personal" } });
    }

    const email = userSession?.user.email;
    return { email, role, isAdmin };
  },
});

function Component() {
  const { email, isAdmin } = Route.useLoaderData();
  const { tab } = Route.useSearch();

  const navigate = useNavigate();

  const handleTabChange = (value: string | null) => {
    if (value !== "personal" && value !== "organization") {
      return;
    }

    navigate({
      to: "/app/settings",
      search: { tab: value },
    });
  };

  return (
    <Stack gap="lg">
      <div>
        <Title order={1}>
          Settings
        </Title>
        <Text c="dimmed" size="sm">
          Manage your application configuration and API access
        </Text>
      </div>

      <Tabs
        defaultValue={tab ?? "personal"}
        onChange={(value) => handleTabChange(value)}
        className="mt-4"
      >
        <Tabs.List>
          <Tabs.Tab value="personal" leftSection={<IconUser size={16} />}>
            Personal
          </Tabs.Tab>
          <Tabs.Tab
            disabled={!isAdmin}
            value="organization"
            leftSection={<IconBuilding size={16} />}
          >
            Organization
          </Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="personal" className="mt-6">
          <PersonalSettings email={email} />
        </Tabs.Panel>
        <Tabs.Panel value="organization" className="mt-6">
          <OrganizationSettings />
        </Tabs.Panel>
      </Tabs>
    </Stack>
  );
}

function PersonalSettings({ email }: { email: string | undefined }) {
  const baseUrl = useQuery({
    queryKey: ["organizationConfig", "baseUrl"],
    queryFn: async () => {
      const config = await getOrganizationConfig();
      return config?.baseUrl;
    },
  });

  const apiKey = "";

  return (
    <Stack gap="lg">
      <SettingsSection
        icon={<IconSettings size={20} />}
        title="General"
      >
        <Stack gap="md">
          <TextInput disabled label="Email Address" value={email} />
        </Stack>
      </SettingsSection>

      <SettingsSection
        icon={<IconDeviceIpadHorizontalPin size={20} />}
        title="Client Connection"
        helper={<ClientConnectionHelperModal baseUrl={baseUrl.data ?? ""} apiKey={apiKey} />}
        helperTooltip="How to connect your apps"
      >
        <TextInput
          disabled
          label="Base URL"
          placeholder="Admin should configure this in Organization Settings"
          value={baseUrl.data ?? ""}
          flex={1}
        />

        <Group gap="md" align="end">
          <TextInput
            disabled
            label="API Key"
            value={apiKey}
            flex={1}
          />

          <Group gap="xs">
            <CopyButton value={apiKey}>
              {({ copied, copy }) => (
                <Tooltip label={copied ? "Copied!" : "Copy API key"}>
                  <Button
                    variant={copied ? "filled" : "light"}
                    color="blue"
                    onClick={copy}
                    size="sm"
                    radius="md"
                    style={{ alignSelf: "end" }}
                  >
                    {copied ? <IconCheck size={16} /> : <IconCopy size={16} />}
                  </Button>
                </Tooltip>
              )}
            </CopyButton>

            <Tooltip label="Regenerate API key">
              <Button
                variant="light"
                color="gray"
                size="sm"
                radius="md"
                style={{ alignSelf: "end" }}
              >
                <IconRefresh size={16} />
              </Button>
            </Tooltip>
          </Group>
        </Group>
      </SettingsSection>
    </Stack>
  );
}

function ClientConnectionHelperModal({ baseUrl, apiKey }: { baseUrl: string; apiKey: string }) {
  const [opened, { open, close }] = useDisclosure(false);

  return (
    <>
      <ActionIcon
        variant="subtle"
        color="blue"
        size="sm"
        onClick={open}
      >
        <IconHelp size={16} />
      </ActionIcon>
      <Modal
        centered
        opened={opened}
        onClose={close}
        title="Client Connection Help"
        size="md"
      >
        <Stack gap="lg">
          <Stack gap="sm">
            <Text size="md" fw={500}>
              Method 1. Auto-Connect using deep-link
            </Text>
            <Button
              onClick={() => {
                window.open(`hypr://register?baseUrl=${baseUrl}&apiKey=${apiKey}`, "_blank");
              }}
              size="sm"
              radius="md"
            >
              Click here to auto-connect
            </Button>
          </Stack>

          <Alert icon={<IconInfoCircle size={16} />} color="blue" variant="light">
            <Text size="sm">
              Try Method 1 first. If it doesn't work, try Method 2.
            </Text>
          </Alert>

          <Stack gap="sm">
            <Text size="md" fw={500}>
              Method 2. Manual connect
            </Text>

            <Group gap="md" grow>
              <CopyButton value={baseUrl}>
                {({ copied, copy }) => (
                  <Tooltip label={copied ? "Copied!" : "Copy Base URL"}>
                    <Button
                      variant={copied ? "filled" : "light"}
                      color="blue"
                      onClick={copy}
                      leftSection={<p>Base URL</p>}
                      size="sm"
                      radius="md"
                      flex={1}
                    >
                      {copied ? <IconCheck size={16} /> : <IconCopy size={16} />}
                    </Button>
                  </Tooltip>
                )}
              </CopyButton>
              <CopyButton value={apiKey}>
                {({ copied, copy }) => (
                  <Tooltip label={copied ? "Copied!" : "Copy API key"}>
                    <Button
                      variant={copied ? "filled" : "light"}
                      color="blue"
                      onClick={copy}
                      leftSection={<p>API Key</p>}
                      size="sm"
                      radius="md"
                      flex={1}
                    >
                      {copied ? <IconCheck size={16} /> : <IconCopy size={16} />}
                    </Button>
                  </Tooltip>
                )}
              </CopyButton>
            </Group>

            <Stack gap="xs" pl="sm">
              <Text fw={500} size="sm">Desktop App Setup:</Text>
              <Text size="sm">
                1. Open the Hyprnote desktop app
              </Text>
              <Text size="sm">
                2. Go to Settings → Account
              </Text>
              <Text size="sm">
                3. Paste your Base URL and API key
              </Text>
            </Stack>
          </Stack>
        </Stack>
      </Modal>
    </>
  );
}

function OrganizationSettings() {
  return (
    <Stack gap="lg">
      <SettingsSection
        icon={<IconSettings size={20} />}
        title="General"
      >
        <Stack gap="md">
          <TextInput
            label="Organization Slug"
            placeholder="e.g. hyprnote"
          />

          <TextInput
            label="Base URL"
            placeholder="e.g. https://hyprnote.yourdomain.com"
          />
        </Stack>
      </SettingsSection>

      <SettingsSection
        icon={<IconKey size={20} />}
        title="License"
      >
        <Stack gap="md">
          <TextInput
            label="Hyprnote Admin Server License"
            placeholder="Enter your license key"
          />
          <Button variant="light" className="self-start">
            Update License
          </Button>
        </Stack>
      </SettingsSection>
    </Stack>
  );
}

function SettingsSection({
  icon,
  title,
  children,
  helper,
  helperTooltip,
  gap = "lg",
}: {
  icon: ReactNode;
  title: string;
  children: ReactNode;
  helper?: ReactNode;
  helperTooltip?: string;
  gap?: "xs" | "sm" | "md" | "lg" | "xl";
}) {
  return (
    <Card withBorder>
      <Stack gap={gap}>
        {helper
          ? (
            <Group gap="xs" align="center">
              <Title order={3} className="flex items-center gap-2 mb-4">
                {icon}
                {title}
              </Title>
              {helperTooltip
                ? (
                  <Tooltip label={helperTooltip}>
                    {helper}
                  </Tooltip>
                )
                : helper}
            </Group>
          )
          : (
            <Title order={3} className="flex items-center gap-2 mb-4">
              {icon}
              {title}
            </Title>
          )}
        {children}
      </Stack>
    </Card>
  );
}
