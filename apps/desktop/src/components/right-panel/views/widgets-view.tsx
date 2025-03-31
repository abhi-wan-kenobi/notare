import { useQuery } from "@tanstack/react-query";
import { LinkProps, useMatch } from "@tanstack/react-router";

import { useHypr } from "@/contexts";
import { type ExtensionName } from "@hypr/extension-registry";
import { commands as dbCommands } from "@hypr/plugin-db";
import { commands as windowsCommands } from "@hypr/plugin-windows";
import WidgetRenderer from "../components/widget-renderer";

export function WidgetsView() {
  const { userId } = useHypr();
  const { id: sessionId } = useMatch({ from: "/app/note/$id", shouldThrow: true });

  const widgets = useQuery({
    queryKey: ["extensions"],
    queryFn: async () => {
      const extensions = await dbCommands.listExtensionMappings(userId);

      return extensions.flatMap((extension) => {
        return extension.widgets.map((widget) => ({
          extensionName: extension.extension_id as ExtensionName,
          groupName: widget.group,
          widgetType: widget.kind,
          layout: widget.position,
        }));
      });
    },
  });

  const handleClickConfigureWidgets = () => {
    const params = {
      to: "/app/settings",
      search: { current: "extensions" },
    } as const satisfies LinkProps;

    const url = `${params.to}?current=${params.search.current}`;

    windowsCommands.windowShow({ type: "settings" }).then(() => {
      setTimeout(() => {
        windowsCommands.windowEmitNavigate({ type: "settings" }, url);
      }, 500);
    });
  };

  return widgets.data?.length
    ? (
      <WidgetRenderer
        key={sessionId}
        widgets={widgets.data}
      />
    )
    : (
      <div className="flex items-center justify-center h-full">
        <button
          onClick={handleClickConfigureWidgets}
          className="px-2 py-1.5 text-xs rounded-full bg-white hover:bg-neutral-200 border border-border transition-all shadow-md hover:shadow-sm transform hover:translate-y-0.5 active:translate-y-1 active:shadow-none"
        >
          Configure Widgets
        </button>
      </div>
    );
}
