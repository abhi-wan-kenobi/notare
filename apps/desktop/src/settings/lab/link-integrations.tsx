import { Icon } from "@iconify-icon/react";
import { useCallback, useMemo } from "react";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { useConnections } from "~/auth/useConnections";
import { openIntegrationUrl } from "~/shared/integration";

type LinkProvider = {
  id: string;
  displayName: string;
  icon: React.ReactNode;
  nangoIntegrationId: string;
};

const LINK_PROVIDERS: LinkProvider[] = [
  {
    id: "slack",
    displayName: "Slack",
    icon: <Icon icon="logos:slack-icon" width={20} height={20} />,
    nangoIntegrationId: "slack",
  },
  {
    id: "discord",
    displayName: "Discord",
    icon: <Icon icon="logos:discord-icon" width={20} height={20} />,
    nangoIntegrationId: "discord",
  },
];

export function LinkIntegrations() {
  return (
    <div className="flex flex-col gap-4">
      {LINK_PROVIDERS.map((provider) => (
        <LinkProviderRow key={provider.id} provider={provider} />
      ))}
    </div>
  );
}

function LinkProviderRow({ provider }: { provider: LinkProvider }) {
  const auth = useAuth();
  const { isPaid, upgradeToPro } = useBillingAccess();
  const { data: connections } = useConnections(isPaid);

  const providerConnections = useMemo(
    () =>
      connections?.filter(
        (connection) =>
          connection.integration_id === provider.nangoIntegrationId,
      ) ?? [],
    [connections, provider.nangoIntegrationId],
  );

  const isConnected = providerConnections.length > 0;
  const reconnectRequired = providerConnections.some(
    (connection) => connection.status === "reconnect_required",
  );

  const handleConnect = useCallback(
    () => openIntegrationUrl(provider.nangoIntegrationId, undefined, "connect"),
    [provider.nangoIntegrationId],
  );

  const handleReconnect = useCallback(
    () =>
      openIntegrationUrl(
        provider.nangoIntegrationId,
        providerConnections[0]?.connection_id,
        "reconnect",
      ),
    [provider.nangoIntegrationId, providerConnections],
  );

  const handleDisconnect = useCallback(
    () =>
      openIntegrationUrl(
        provider.nangoIntegrationId,
        providerConnections[0]?.connection_id,
        "disconnect",
      ),
    [provider.nangoIntegrationId, providerConnections],
  );

  return (
    <div className="flex items-center justify-between">
      <div className="flex items-center gap-2">
        {provider.icon}
        <span className="text-sm">{provider.displayName}</span>
      </div>

      <div className="flex items-center gap-2">
        {!auth.session ? (
          <Tooltip delayDuration={0}>
            <TooltipTrigger asChild>
              <span className="cursor-not-allowed text-xs text-neutral-400 opacity-50">
                Connect
              </span>
            </TooltipTrigger>
            <TooltipContent side="bottom">Sign in to connect</TooltipContent>
          </Tooltip>
        ) : !isPaid ? (
          <button
            type="button"
            onClick={upgradeToPro}
            className="cursor-pointer text-xs text-neutral-600 underline transition-colors hover:text-neutral-900"
          >
            Upgrade
          </button>
        ) : reconnectRequired ? (
          <>
            <button
              type="button"
              onClick={handleReconnect}
              className="cursor-pointer text-xs text-amber-700 underline transition-colors hover:text-amber-900"
            >
              Reconnect
            </button>
            <span className="text-xs text-neutral-400">or</span>
            <button
              type="button"
              onClick={handleDisconnect}
              className="cursor-pointer text-xs text-red-500 underline transition-colors hover:text-red-700"
            >
              Disconnect
            </button>
          </>
        ) : isConnected ? (
          <button
            type="button"
            onClick={handleDisconnect}
            className="cursor-pointer text-xs text-neutral-500 underline transition-colors hover:text-neutral-700"
          >
            Disconnect
          </button>
        ) : (
          <button
            type="button"
            onClick={handleConnect}
            className="cursor-pointer text-xs text-neutral-600 underline transition-colors hover:text-neutral-900"
          >
            Connect
          </button>
        )}
      </div>
    </div>
  );
}
