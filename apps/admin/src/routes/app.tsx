import { NavLink } from "@/components/NavLink";
import { AppShell, Burger, Button, Group, Text } from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Outlet, useMatch, useNavigate } from "@tanstack/react-router";

import { authClient } from "@/lib/auth/client";
import { getEnv } from "@/services/env.api";

export const Route = createFileRoute("/app")({
  component: Component,
  loader: async () => {
    const slug = await getEnv({ data: { key: "ORG_SLUG" } }) ?? null as string | null;
    return { slug };
  },
});

function Component() {
  const loaderData = Route.useLoaderData();

  const [opened, { toggle }] = useDisclosure();

  const homeMatch = useMatch({ from: "/app/home", shouldThrow: false });
  const providersMatch = useMatch({ from: "/app/providers", shouldThrow: false });

  const logout = useMutation({
    mutationFn: async () => {
      const startTime = performance.now();
      const { error } = await authClient.signOut();
      await new Promise((resolve) => setTimeout(resolve, Math.max(500, startTime + 500 - performance.now())));

      if (error) {
        throw error;
      }
    },
    onError: (error) => {
      console.log(error);
    },
    onSettled: () => {
      // not sure why, but useNavigate() doesn't work here
      window.location.href = "/";
    },
  });

  return (
    <AppShell
      header={{ height: 60 }}
      navbar={{
        width: 250,
        breakpoint: "sm",
        collapsed: { mobile: !opened },
      }}
      padding="md"
    >
      <AppShell.Header>
        <Group h="100%" px="md" justify="space-between" align="center">
          <Group gap="sm" align="center">
            <Burger
              opened={opened}
              onClick={toggle}
              hiddenFrom="sm"
              size="sm"
            />
            <Text size="xl" fw={700}>Hyprnote Admin</Text>
            <Text size="md" c="dimmed" fw={500}>{loaderData?.slug}</Text>
          </Group>
          <Button variant="light" size="sm" onClick={() => logout.mutate()}>
            Logout
          </Button>
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="md">
        <NavLink
          label="Dashboard"
          to="/app/home"
          active={!!homeMatch}
        />
        <NavLink
          label="Providers"
          to="/app/providers"
          active={!!providersMatch}
        />
      </AppShell.Navbar>

      <AppShell.Main>
        <Outlet />
      </AppShell.Main>
    </AppShell>
  );
}
