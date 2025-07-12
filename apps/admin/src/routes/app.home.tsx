import { Button, Modal, Table, TextInput } from "@mantine/core";
import { useForm } from "@mantine/form";
import { useDisclosure } from "@mantine/hooks";
import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { zodResolver } from "mantine-form-zod-resolver";
import { z } from "zod";

import { authClient } from "@/lib/auth/client";
import { insertLlmProvider, listLlmProvider } from "@/services/provider.api";

export const Route = createFileRoute("/app/home")({
  component: Component,
  beforeLoad: ({ context }) => {
    if (!context.userSession) {
      throw redirect({ to: "/login" });
    }
  },
});

function Component() {
  const m = useMutation({
    mutationFn: async ({ name }: { name: string }) => {
      const { data: apiKey, error } = await authClient.apiKey.create({
        name,
        expiresIn: null,
        prefix: "hypr_",
        metadata: {},
      });

      if (error) {
        throw error;
      }

      return apiKey;
    },
  });

  return (
    <div>
      <section>
        <NewProviderModal />
      </section>
      <section>
        <ProvidersTable />
      </section>
    </div>
  );
}

function NewProviderModal() {
  const [opened, { open, close }] = useDisclosure(false);

  const schema = z.object({
    name: z.string().min(1).max(20),
    model: z.string().min(1).max(100),
    baseUrl: z.string().min(1).max(255),
    apiKey: z.string().min(1).max(500),
  });

  type FormData = z.infer<typeof schema>;

  const form = useForm<FormData>({
    mode: "uncontrolled",
    validate: zodResolver(schema),
  });

  const insertMutation = useMutation({
    mutationFn: async (data: FormData) => {
      const rows = await insertLlmProvider({ data });
      return rows;
    },
    onSuccess: (response) => {
      console.log(response);
    },
    onError: (error) => {
      console.log(error);
    },
  });

  return (
    <>
      <Modal opened={opened} onClose={close} title="Authentication" centered>
        <form onSubmit={form.onSubmit((values) => insertMutation.mutateAsync(values))}>
          <TextInput
            label="Name"
            placeholder="Name"
            {...form.getInputProps("name")}
          />
          <TextInput
            label="Model"
            placeholder="Model"
            {...form.getInputProps("model")}
          />
          <TextInput
            label="Base URL"
            placeholder="Base URL"
            {...form.getInputProps("baseUrl")}
          />
          <TextInput
            label="API Key"
            placeholder="API Key"
            {...form.getInputProps("apiKey")}
          />
          <Button type="submit">Submit</Button>
        </form>
      </Modal>

      <Button variant="default" onClick={open}>
        Open centered Modal
      </Button>
    </>
  );
}

function ProvidersTable() {
  const { data } = useQuery({
    queryKey: ["providers"],
    queryFn: async () => {
      const rows = await listLlmProvider();
      return rows;
    },
  });

  return (
    <Table variant="vertical" layout="fixed" withTableBorder>
      <Table.Tbody>
        {data?.map((row) => (
          <Table.Tr key={row.id}>
            <Table.Th w={160}>{row.name}</Table.Th>
            <Table.Td>{row.model}</Table.Td>
            <Table.Td>{row.baseUrl}</Table.Td>
            <Table.Td>{row.apiKey}</Table.Td>
          </Table.Tr>
        ))}
      </Table.Tbody>
    </Table>
  );
}
