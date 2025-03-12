import { zodResolver } from "@hookform/resolvers/zod";
import { LANGUAGES_ISO_639_1 } from "@huggingface/languages";
import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType } from "@stackflow/react/future";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { commands as dbCommands, type ConfigGeneral } from "@hypr/plugin-db";
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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@hypr/ui/components/ui/select";

type ISO_639_1_CODE = keyof typeof LANGUAGES_ISO_639_1;
const SUPPORTED_LANGUAGES: ISO_639_1_CODE[] = ["en", "ko"];

const schema = z.object({
  autostart: z.boolean().optional(),
  displayLanguage: z.enum(SUPPORTED_LANGUAGES as [string, ...string[]]),
  jargons: z.string(),
  tags: z.array(z.string()),
});

type Schema = z.infer<typeof schema>;

export function settingsActivityLoader({}: ActivityLoaderArgs<"SettingsActivity">) {
  return {};
}

export const SettingsActivity: ActivityComponentType<"SettingsActivity"> = () => {
  const queryClient = useQueryClient();

  const config = useQuery({
    queryKey: ["config", "general"],
    queryFn: async () => {
      const result = await dbCommands.getConfig();
      return result;
    },
    staleTime: Infinity,
    gcTime: Infinity,
  });

  const form = useForm<Schema>({
    resolver: zodResolver(schema),
    values: {
      autostart: config.data?.general.autostart ?? false,
      displayLanguage: config.data?.general.display_language ?? "en",
      jargons: (config.data?.general.jargons ?? []).join(", "),
      tags: config.data?.general.tags ?? [],
    },
  });

  const mutation = useMutation({
    mutationFn: async (v: Schema) => {
      if (!config.data) {
        console.error("cannot mutate config because it is not loaded");
        return;
      }

      const nextGeneral: ConfigGeneral = {
        autostart: v.autostart ?? true,
        display_language: v.displayLanguage,
        jargons: v.jargons.split(",").map((jargon) => jargon.trim()),
        tags: v.tags,
      };

      try {
        await dbCommands.setConfig({
          ...config.data,
          general: nextGeneral,
        });
      } catch (e) {
        console.error(e);
      }
    },
    onSuccess: () => {
      queryClient.setQueryData(["config", "general"], (oldData: any) => {
        if (!oldData) return oldData;

        return {
          ...oldData,
          general: {
            ...oldData.general,
            autostart: form.getValues("autostart"),
            display_language: form.getValues("displayLanguage"),
            jargons: form.getValues("jargons").split(",").map((jargon: string) => jargon.trim()),
            tags: form.getValues("tags"),
          },
        };
      });
    },
  });

  const onFormChange = useCallback(() => {
    form.handleSubmit((v) => mutation.mutate(v))();
  }, [form, mutation]);

  useEffect(() => {
    const subscription = form.watch(onFormChange);
    return () => subscription.unsubscribe();
  }, [form, onFormChange]);

  return (
    <AppScreen
      appBar={{
        title: "Settings",
      }}
    >
      <div className="h-full overflow-y-auto w-full flex flex-col py-6 px-4">
        {config.isLoading ? <div className="w-full text-center">Loading settings...</div> : (
          <Form {...form}>
            <form className="space-y-6">
              <FormField
                control={form.control}
                name="displayLanguage"
                render={({ field }) => (
                  <FormItem className="flex flex-row items-center justify-between">
                    <div>
                      <FormLabel>Display language</FormLabel>
                      <FormDescription className="text-xs">
                        This is the language you read.
                      </FormDescription>
                    </div>
                    <Select onValueChange={field.onChange} value={field.value}>
                      <FormControl>
                        <SelectTrigger className="max-w-[100px] focus:outline-none focus:ring-0 focus:ring-offset-0">
                          <SelectValue placeholder="Select language" />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent align="end">
                        {SUPPORTED_LANGUAGES.map((code) => (
                          <SelectItem key={code} value={code}>
                            {LANGUAGES_ISO_639_1[code].nativeName}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="jargons"
                render={({ field }) => (
                  <FormItem>
                    <div>
                      <FormLabel>
                        Jargons
                      </FormLabel>
                      <FormDescription className="text-xs">
                        You can make Hyprnote takes these words into account when transcribing.
                      </FormDescription>
                    </div>
                    <FormControl>
                      <Input
                        placeholder="Type jargons (e.g., Blitz Meeting, PaC Squad)"
                        {...field}
                        value={field.value ?? ""}
                        className="focus-visible:ring-0 focus-visible:ring-offset-0"
                        autoCorrect="false"
                      />
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
    SettingsActivity: {};
  }
}
