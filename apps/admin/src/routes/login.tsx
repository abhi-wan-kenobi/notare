import {
  Button,
  Center,
  Divider,
  Paper,
  PasswordInput,
  Stack,
  Tabs,
  Text,
  TextInput,
  Title,
  Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import { IconBrandGoogle, IconLock } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, redirect, useNavigate } from "@tanstack/react-router";
import { zodResolver } from "mantine-form-zod-resolver";
import { z } from "zod";

import { authClient } from "@/lib/auth/client";
import { adminCreated } from "@/services/auth.api";

export const Route = createFileRoute("/login")({
  component: Component,
  beforeLoad: async ({ context: { userSession } }) => {
    if (userSession) {
      return redirect({ to: "/" });
    }
  },
});

function Component() {
  const adminCreatedQuery = useQuery({
    queryKey: ["adminCreated"],
    queryFn: () => adminCreated(),
  });

  return adminCreatedQuery.data
    ? (
      <Container title="Sign In" description="Sign in to your account">
        <PasswordSignInForm />
      </Container>
    )
    : (
      <Container title="Sign Up" description="Sign up to create an account">
        <PasswordAdminSignUpForm />
      </Container>
    );
}

function Container(
  { title, children, description }: { title: string; description: string; children: React.ReactNode },
) {
  return (
    <Center h="100vh">
      <Paper shadow="md" p="xl" radius="md" w={400}>
        <Stack gap="lg">
          <Stack gap="xs" align="center">
            <Title order={2} ta="center">{title}</Title>
            <Text c="dimmed" ta="center">{description}</Text>
          </Stack>
          <Tabs defaultValue="password" variant="pills">
            <Tabs.List grow>
              <Tabs.Tab value="password" leftSection={<IconLock size={16} />}>
                Email & Password
              </Tabs.Tab>
              <Tooltip
                label="Enterprise license required"
                withArrow
                position="top"
              >
                <Tabs.Tab
                  value="sso"
                  leftSection={<IconBrandGoogle size={16} />}
                  disabled={true}
                >
                  Single Sign On
                </Tabs.Tab>
              </Tooltip>
            </Tabs.List>

            <Tabs.Panel value="password" pt="md">
              {children}
            </Tabs.Panel>

            <Tabs.Panel value="sso" pt="md">
              <div>TODO</div>
            </Tabs.Panel>
          </Tabs>
        </Stack>
      </Paper>
    </Center>
  );
}

function PasswordAdminSignUpForm() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const signInMutation = useMutation({
    mutationFn: async (data: { email: string; password: string }) => {
      const { error, data: response } = await authClient.signIn.email({
        email: data.email,
        password: data.password,
      });

      if (error) {
        throw new Error(error.message);
      }

      return response;
    },
    onSuccess: (response) => {
      queryClient.resetQueries();
      navigate({ to: "/" });
    },
  });

  const schema = z.object({
    email: z.string().email(),
    password: z.string().min(1),
    passwordConfirm: z.string().min(1),
  }).refine((data) => data.password === data.passwordConfirm, {
    message: "Passwords do not match",
    path: ["passwordConfirm"],
  });

  type FormData = z.infer<typeof schema>;

  const form = useForm<FormData>({
    mode: "uncontrolled",
    validate: zodResolver(schema),
  });

  const handleSubmit = async (values: FormData) => {
    await signInMutation.mutateAsync(values);
  };

  return (
    <form onSubmit={form.onSubmit(handleSubmit)}>
      <Stack gap="md">
        <TextInput
          withAsterisk
          label="Email"
          placeholder="your@email.com"
          type="email"
          key={form.key("email")}
          {...form.getInputProps("email")}
        />

        <PasswordInput
          withAsterisk
          label="Password"
          placeholder="Enter your password"
          key={form.key("password")}
          {...form.getInputProps("password")}
        />

        <PasswordInput
          withAsterisk
          label="Confirm Password"
          placeholder="Confirm your password"
          key={form.key("passwordConfirm")}
          {...form.getInputProps("passwordConfirm")}
        />

        <Button
          type="submit"
          fullWidth
          loading={signInMutation.isPending}
          mt="md"
        >
          Sign Up
        </Button>
      </Stack>
    </form>
  );
}

function PasswordSignInForm() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const signInMutation = useMutation({
    mutationFn: async (data: { email: string; password: string }) => {
      const { error, data: response } = await authClient.signIn.email({
        email: data.email,
        password: data.password,
      });

      if (error) {
        throw new Error(error.message);
      }

      return response;
    },
    onSuccess: (response) => {
      queryClient.resetQueries();
      navigate({ to: "/" });
    },
  });

  const schema = z.object({
    email: z.string().email(),
    password: z.string().min(1),
  });

  type FormData = z.infer<typeof schema>;

  const form = useForm<FormData>({
    mode: "uncontrolled",
    validate: zodResolver(schema),
  });

  const handleSubmit = async (values: FormData) => {
    await signInMutation.mutateAsync(values);
  };

  return (
    <form onSubmit={form.onSubmit(handleSubmit)}>
      <Stack gap="md">
        <TextInput
          withAsterisk
          label="Email"
          placeholder="your@email.com"
          type="email"
          key={form.key("email")}
          {...form.getInputProps("email")}
        />

        <PasswordInput
          withAsterisk
          label="Password"
          placeholder="Enter your password"
          key={form.key("password")}
          {...form.getInputProps("password")}
        />

        <Button
          type="submit"
          fullWidth
          loading={signInMutation.isPending}
          mt="md"
        >
          Sign In
        </Button>
      </Stack>
    </form>
  );
}
