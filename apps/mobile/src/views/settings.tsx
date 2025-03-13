import { zodResolver } from "@hookform/resolvers/zod";
import type { ActivityLoaderArgs } from "@stackflow/config";
import { AppScreen } from "@stackflow/plugin-basic-ui";
import { ActivityComponentType, useLoaderData } from "@stackflow/react/future";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { mockUserSettings } from "../mock";

import { Button } from "@hypr/ui/components/ui/button";
import { Form, FormControl, FormDescription, FormField, FormItem, FormLabel } from "@hypr/ui/components/ui/form";
import { Modal, ModalBody, ModalFooter, ModalHeader, ModalTitle } from "@hypr/ui/components/ui/modal";
import { Switch } from "@hypr/ui/components/ui/switch";

const schema = z.object({
  alertEnhancingDone: z.boolean().optional(),
  remindUpcomingEvents: z.boolean().optional(),
});

type Schema = z.infer<typeof schema>;

export function settingsLoader({}: ActivityLoaderArgs<"SettingsView">) {
  return { settings: mockUserSettings };
}

export const SettingsView: ActivityComponentType<"SettingsView"> = () => {
  const { settings } = useLoaderData<typeof settingsLoader>();
  const [showSignOutModal, setShowSignOutModal] = useState(false);

  const form = useForm<Schema>({
    resolver: zodResolver(schema),
    values: {
      alertEnhancingDone: settings.alertEnhancingDone ?? false,
      remindUpcomingEvents: settings.remindUpcomingEvents ?? false,
    },
  });

  const handleSignOut = () => {
    // TODO: Replace with actual sign out logic

    setShowSignOutModal(false);
  };

  return (
    <AppScreen
      appBar={{
        title: "Settings",
      }}
    >
      <div className="h-full overflow-y-auto w-full flex flex-col py-6 px-4">
        <div className="flex-1">
          <Form {...form}>
            <FormField
              control={form.control}
              name="alertEnhancingDone"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between mb-4">
                  <div>
                    <FormLabel className="select-none">Note Enhancement Alerts</FormLabel>
                    <FormDescription className="select-none">
                      Alert when note is done enhancing.
                    </FormDescription>
                  </div>
                  <FormControl>
                    <Switch
                      checked={field.value}
                      onCheckedChange={field.onChange}
                      className="select-none"
                    />
                  </FormControl>
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="remindUpcomingEvents"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between">
                  <div>
                    <FormLabel className="select-none">Remind upcoming events</FormLabel>
                    <FormDescription className="select-none">
                      Hyprnote will notify you about upcoming events based on your linked calendar.
                    </FormDescription>
                  </div>
                  <FormControl>
                    <Switch
                      checked={field.value}
                      onCheckedChange={field.onChange}
                      className="select-none"
                    />
                  </FormControl>
                </FormItem>
              )}
            />
          </Form>
        </div>

        <div
          onClick={() => setShowSignOutModal(true)}
          className="text-center text-sm text-red-600 w-fit mx-auto select-none"
        >
          Sign out
        </div>
      </div>

      <Modal
        open={showSignOutModal}
        onClose={() => setShowSignOutModal(false)}
        className="w-fit"
      >
        <ModalHeader className="p-6 pb-0">
          <ModalTitle>Confirm Sign Out</ModalTitle>
        </ModalHeader>
        <ModalBody className="py-3">
          <p>
            Are you sure you want to sign out?
          </p>
        </ModalBody>
        <ModalFooter className="flex items-end justify-end gap-2">
          <Button
            variant="destructive"
            onClick={handleSignOut}
            className="flex-1"
          >
            Sign Out
          </Button>
          <Button
            variant="outline"
            onClick={() => setShowSignOutModal(false)}
            className="flex-1"
          >
            Cancel
          </Button>
        </ModalFooter>
      </Modal>
    </AppScreen>
  );
};

declare module "@stackflow/config" {
  interface Register {
    SettingsView: {};
  }
}
