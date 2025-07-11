import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Link } from "@tanstack/react-router";

import { authClient } from "@/lib/auth/client";
import { useAppForm } from "@/lib/form";

export const Route = createFileRoute("/sign-up")({
  component: Component,
});

function Component() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const signUpMutation = useMutation({
    mutationFn: async (data: { email: string; password: string; name: string }) => {
      const { error, data: response } = await authClient.signUp.email({
        email: data.email,
        password: data.password,
        name: data.name,
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
      name: "",
      email: "",
      password: "",
      confirmPassword: "",
    },
    validators: {
      onChange: ({ value }) => {
        const errors: any = { fields: {} };

        if (!value.name?.trim()) {
          errors.fields.name = "Name is required";
        }

        if (!value.email) {
          errors.fields.email = "Email is required";
        } else if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.email)) {
          errors.fields.email = "Please enter a valid email address";
        }

        if (!value.password) {
          errors.fields.password = "Password is required";
        } else if (value.password.length < 8) {
          errors.fields.password = "Password must be at least 8 characters";
        }

        if (!value.confirmPassword) {
          errors.fields.confirmPassword = "Please confirm your password";
        } else if (value.password !== value.confirmPassword) {
          errors.fields.confirmPassword = "Passwords do not match";
        }

        return Object.keys(errors.fields).length > 0 ? errors : undefined;
      },
    },
    onSubmit: async ({ value }) => {
      await signUpMutation.mutateAsync({
        name: value.name,
        email: value.email,
        password: value.password,
      });
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
          name="name"
          children={(field) => <field.TextField label="Full Name" required />}
        />
        <form.AppField
          name="email"
          children={(field) => <field.TextField label="Email" required type="email" />}
        />
        <form.AppField
          name="password"
          children={(field) => <field.TextField label="Password" required type="password" />}
        />
        <form.AppField
          name="confirmPassword"
          children={(field) => <field.TextField label="Confirm Password" required type="password" />}
        />
        <form.AppForm>
          <form.SubmitButton label="Sign Up" />
        </form.AppForm>
      </form>

      <small>
        <Link to="/sign-in" className="group">
          Already have an account? <span className="underline group-hover:no-underline">Sign In</span>
        </Link>
      </small>
    </div>
  );
}
