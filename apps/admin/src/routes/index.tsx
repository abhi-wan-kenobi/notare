import { Button, Modal, TextInput } from "@mantine/core";
import { useForm } from "@mantine/form";
import { useDisclosure } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { zodResolver } from "mantine-form-zod-resolver";
import { z } from "zod";

import { insertLlmProvider } from "@/services/provider.api";

export const Route = createFileRoute("/")({
  component: Component,
  // beforeLoad: ({ context }) => {
  //   if (!context.userSession) {
  //     throw redirect({ to: "/sign-in" });
  //   }
  // },
});

function Component() {
  return (
    <>
      <NewProviderModal />

      <Button
        onClick={() => {
          console.log("click");
          notifications.show({
            title: "Default notification",
            message: "Do not forget to star Mantine on GitHub! 🌟",
          });
        }}
      >
        Click
      </Button>
    </>
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
