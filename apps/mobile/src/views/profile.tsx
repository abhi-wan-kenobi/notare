import { zodResolver } from "@hookform/resolvers/zod";
import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType } from "@stackflow/react/future";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { commands as dbCommands, type Human, type Organization } from "@hypr/plugin-db";
import { Avatar, AvatarFallback, AvatarImage } from "@hypr/ui/components/ui/avatar";
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@hypr/ui/components/ui/form";
import { Input } from "@hypr/ui/components/ui/input";
import { Textarea } from "@hypr/ui/components/ui/textarea";

const schema = z.object({
  fullName: z.string().min(2).max(50).optional(),
  jobTitle: z.string().min(2).max(50).optional(),
  companyName: z.string().min(2).max(50),
  companyDescription: z.string().min(2).max(500).optional(),
  linkedinUserName: z.string().min(2).max(50).optional(),
});

type Schema = z.infer<typeof schema>;

type ConfigData = {
  human: Human | null;
  organization: Organization;
};

export function profileActivityLoader({}: ActivityLoaderArgs<"ProfileActivity">) {
  return {};
}

export const ProfileActivity: ActivityComponentType<"ProfileActivity"> = () => {
  const queryClient = useQueryClient();

  const config = useQuery<ConfigData>({
    queryKey: ["config", "profile"],
    queryFn: async () => {
      const [human, organization] = await Promise.all([
        dbCommands.getSelfHuman(),
        dbCommands.getSelfOrganization(),
      ]);
      return { human, organization };
    },
  });

  const form = useForm<Schema>({
    resolver: zodResolver(schema),
    defaultValues: {
      fullName: config.data?.human?.full_name ?? undefined,
      jobTitle: config.data?.human?.job_title ?? undefined,
      companyName: config.data?.organization.name ?? undefined,
      companyDescription: config.data?.organization.description ?? undefined,
      linkedinUserName: config.data?.human?.linkedin_username ?? undefined,
    },
  });

  const mutation = useMutation({
    mutationFn: async (v: Schema) => {
      if (!config.data) {
        console.error("cannot mutate profile because it is not loaded");
        return;
      }

      const newHuman: Human = {
        ...config.data.human!,
        full_name: v.fullName ?? null,
        job_title: v.jobTitle ?? null,
        email: config.data.human?.email ?? null,
        linkedin_username: v.linkedinUserName ?? null,
      };

      const newOrganization: Organization = {
        ...config.data.organization,
        name: v.companyName,
        description: v.companyDescription ?? null,
      };

      try {
        await dbCommands.upsertHuman(newHuman);
        await dbCommands.upsertOrganization(newOrganization);
      } catch (error) {
        console.error("error upserting human or organization", error);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["config", "profile"] });
    },
  });

  useEffect(() => {
    const subscription = form.watch(() => form.handleSubmit((v) => mutation.mutate(v))());
    return () => subscription.unsubscribe();
  }, [mutation]);

  const getInitials = () => {
    const name = config.data?.human?.full_name ?? "";
    if (!name) return "?";
    return name.split(" ").map(part => part[0]?.toUpperCase() || "").join("").slice(0, 2);
  };

  return (
    <AppScreen
      appBar={{
        title: "Profile",
      }}
    >
      <div className="h-full overflow-y-auto w-full flex flex-col items-center py-6 px-4">
        <Avatar className="size-24 border text-2xl font-medium mb-6">
          <AvatarImage src="" alt="User profile" />
          <AvatarFallback>{getInitials()}</AvatarFallback>
        </Avatar>

        {config.isLoading ? <div className="w-full text-center">Loading profile data...</div> : (
          <Form {...form}>
            <form className="w-full flex flex-col gap-6">
              <FormField
                control={form.control}
                name="fullName"
                render={({ field }) => (
                  <FormItem className="w-full">
                    <FormLabel>Full Name</FormLabel>
                    <FormControl>
                      <Input
                        placeholder="Your Name"
                        {...field}
                        className="focus-visible:ring-0 focus-visible:ring-offset-0"
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="jobTitle"
                render={({ field }) => (
                  <FormItem className="w-full">
                    <FormLabel>Job Title</FormLabel>
                    <FormControl>
                      <Input
                        placeholder="Your Job Title"
                        {...field}
                        className="focus-visible:ring-0 focus-visible:ring-offset-0"
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="companyName"
                render={({ field }) => (
                  <FormItem className="w-full">
                    <FormLabel>Company Name</FormLabel>
                    <FormDescription className="text-xs">
                      This is the name of the company you work for.
                    </FormDescription>
                    <FormControl>
                      <Input
                        placeholder="Company Name"
                        {...field}
                        className="focus-visible:ring-0 focus-visible:ring-offset-0"
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="companyDescription"
                render={({ field }) => (
                  <FormItem className="w-full">
                    <FormLabel>Company Description</FormLabel>
                    <FormDescription className="text-xs">
                      This is a short description of your company.
                    </FormDescription>
                    <FormControl>
                      <Textarea
                        placeholder="Add a few words about your company."
                        {...field}
                        className="focus-visible:ring-0 focus-visible:ring-offset-0"
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="linkedinUserName"
                render={({ field }) => (
                  <FormItem className="w-full">
                    <FormLabel>LinkedIn Username</FormLabel>
                    <FormDescription className="text-xs">
                      Your LinkedIn username (the part after linkedin.com/in/)
                    </FormDescription>
                    <FormControl>
                      <div className="flex">
                        <span className="inline-flex items-center rounded-l-md border border-r-0 border-input bg-muted px-3 text-sm text-muted-foreground">
                          linkedin.com/in/
                        </span>
                        <Input
                          className="rounded-l-none focus-visible:ring-0 focus-visible:ring-offset-0"
                          placeholder="username"
                          {...field}
                        />
                      </div>
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </form>
          </Form>
        )}
      </div>
    </AppScreen>
  );
};

declare module "@stackflow/config" {
  interface Register {
    ProfileActivity: {};
  }
}
