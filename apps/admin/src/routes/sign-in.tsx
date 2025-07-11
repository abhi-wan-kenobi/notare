import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Link } from "@tanstack/react-router";

import { authClient } from "@/lib/auth/client";
import { useAppForm } from "@/lib/form";

export const Route = createFileRoute("/sign-in")({
  component: Component,
});

function Component() {
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

  const form = useAppForm({
    defaultValues: {
      email: "example@org.com",
      password: "password",
    },
    validators: {
      onChange: ({ value }) => {
        if (!value.email) {
          return {
            fields: {
              email: "Email is required",
            },
          };
        }
        if (!value.password) {
          return {
            fields: {
              password: "Password is required",
            },
          };
        }
        if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.email)) {
          return {
            fields: {
              email: "Please enter a valid email address",
            },
          };
        }
        return undefined;
      },
    },
    onSubmit: async ({ value }) => {
      await signInMutation.mutateAsync(value);
    },
  });

  return (
    <div className="flex h-screen w-1/3 items-center justify-center flex-col">
      <form
        className="flex flex-col gap-2 w-full"
        onSubmit={(e) => {
          e.preventDefault();
          form.handleSubmit();
        }}
      >
        <form.AppField
          name="email"
          children={(field) => <field.TextField label="Email" required type="email" />}
        />
        <form.AppField
          name="password"
          children={(field) => <field.TextField label="Password" required type="password" />}
        />
        <form.AppForm>
          <form.SubmitButton label="Sign In" />
        </form.AppForm>
      </form>

      <small>
        <Link to="/sign-up" className="group">
          Don't have an account? <span className="underline group-hover:no-underline">Sign Up</span>
        </Link>
      </small>
    </div>
  );
}
