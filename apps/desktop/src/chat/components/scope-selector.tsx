import { Trans, useLingui } from "@lingui/react/macro";
import {
  Building2,
  Check,
  ChevronDown,
  FileText,
  Globe,
  ListFilter,
  User,
} from "lucide-react";
import { useMemo, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { cn } from "@hypr/utils";

import {
  type ChatScopeSelection,
  type ChatScopeType,
  DEFAULT_EVERYTHING_SCOPE,
  DEFAULT_THIS_MEETING_SCOPE,
  NEW_CHAT_SCOPE_KEY,
  useChatScopeStore,
} from "~/chat/state/chat-scope";
import { useHumans, useOrganizations } from "~/contacts/queries";
import { useSessionSummaries } from "~/session/queries";

export interface ScopeSelectorProps {
  currentChatGroupId: string | undefined;
  currentSessionId: string | undefined;
  surface?: "light" | "dark";
  className?: string;
}

export function ScopeSelector({
  currentChatGroupId,
  currentSessionId,
  surface = "light",
  className,
}: ScopeSelectorProps) {
  const { t } = useLingui();
  const [isOpen, setIsOpen] = useState(false);
  const isDark = surface === "dark";

  const rawScope = useChatScopeStore(
    (state) => state.scopesByGroupId[currentChatGroupId ?? NEW_CHAT_SCOPE_KEY],
  );
  const setScopeForGroup = useChatScopeStore((state) => state.setScopeForGroup);

  const activeScope: ChatScopeSelection =
    rawScope ??
    (currentSessionId ? DEFAULT_THIS_MEETING_SCOPE : DEFAULT_EVERYTHING_SCOPE);

  const sessions = useSessionSummaries();
  const humans = useHumans();
  const organizations = useOrganizations();

  const currentSession = useMemo(
    () => sessions.find((s) => s.id === currentSessionId),
    [sessions, currentSessionId],
  );

  const activeLabel = useMemo(() => {
    switch (activeScope.type) {
      case "this-meeting":
        return t`This meeting`;
      case "selected":
        return t`Selected`;
      case "person": {
        if (activeScope.humanId) {
          const person = humans.find((h) => h.id === activeScope.humanId);
          if (person?.name) {
            return person.name;
          }
        }
        return t`Person`;
      }
      case "organization": {
        if (activeScope.orgId) {
          const org = organizations.find((o) => o.id === activeScope.orgId);
          if (org?.name) {
            return org.name;
          }
        }
        return t`Organization`;
      }
      case "everything":
      default:
        return t`Everything`;
    }
  }, [activeScope, humans, organizations, t]);

  const ActiveIcon = useMemo(() => {
    switch (activeScope.type) {
      case "this-meeting":
        return FileText;
      case "selected":
        return ListFilter;
      case "person":
        return User;
      case "organization":
        return Building2;
      case "everything":
      default:
        return Globe;
    }
  }, [activeScope.type]);

  const handleSelectScopeType = (type: ChatScopeType) => {
    if (type === "person") {
      const firstHumanId = activeScope.humanId ?? humans[0]?.id;
      setScopeForGroup(currentChatGroupId, { type, humanId: firstHumanId });
    } else if (type === "organization") {
      const firstOrgId = activeScope.orgId ?? organizations[0]?.id;
      setScopeForGroup(currentChatGroupId, { type, orgId: firstOrgId });
    } else {
      setScopeForGroup(currentChatGroupId, { type });
    }
    setIsOpen(false);
  };

  return (
    <DropdownMenu open={isOpen} onOpenChange={setIsOpen}>
      <DropdownMenuTrigger asChild>
        <Button
          data-chat-scope-selector
          aria-label={t`Chat scope`}
          variant="ghost"
          size="sm"
          onClick={() => setIsOpen((prev) => !prev)}
          className={cn([
            "group h-8 max-w-44 shrink-0 gap-1.5 rounded-full px-2.5 py-0 text-xs font-medium transition-colors",
            isDark
              ? "text-primary-foreground/80 hover:bg-primary-foreground/14 hover:text-primary-foreground data-[state=open]:bg-primary-foreground/14 data-[state=open]:text-primary-foreground"
              : "text-muted-foreground hover:bg-muted/80 hover:text-foreground data-[state=open]:bg-muted/80 data-[state=open]:text-foreground",
            className,
          ])}
        >
          <ActiveIcon
            className={cn([
              "h-3.5 w-3.5 shrink-0",
              isDark ? "text-primary-foreground/70" : "text-muted-foreground",
            ])}
          />
          <span className="truncate">{activeLabel}</span>
          <ChevronDown
            className={cn([
              "h-3 w-3 shrink-0 transition-transform duration-200",
              isDark ? "text-primary-foreground/50" : "text-muted-foreground",
              isOpen && "rotate-180",
            ])}
          />
        </Button>
      </DropdownMenuTrigger>
      {isOpen && (
        <DropdownMenuContent
          variant="app"
          align="start"
          side="bottom"
          sideOffset={4}
          className="w-56"
        >
          <AppFloatingPanel className="p-1">
            <div className="px-2 py-1">
              <h4 className="text-muted-foreground text-[10px] font-semibold tracking-wider uppercase">
                <Trans>Chat scope</Trans>
              </h4>
            </div>

            <Button
              variant="ghost"
              disabled={!currentSessionId}
              onClick={() => handleSelectScopeType("this-meeting")}
              className="flex h-auto w-full cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1.5 text-xs font-normal"
            >
              <div className="flex min-w-0 items-center gap-2">
                <FileText className="text-muted-foreground h-3.5 w-3.5 shrink-0" />
                <div className="flex min-w-0 flex-col text-left">
                  <span className="truncate font-medium">
                    <Trans>This meeting</Trans>
                  </span>
                  {currentSession?.title && (
                    <span className="text-muted-foreground truncate text-[10px]">
                      {currentSession.title}
                    </span>
                  )}
                </div>
              </div>
              {activeScope.type === "this-meeting" && (
                <Check className="text-primary h-3.5 w-3.5 shrink-0" />
              )}
            </Button>

            <Button
              variant="ghost"
              onClick={() => handleSelectScopeType("selected")}
              className="flex h-auto w-full cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1.5 text-xs font-normal"
            >
              <div className="flex min-w-0 items-center gap-2">
                <ListFilter className="text-muted-foreground h-3.5 w-3.5 shrink-0" />
                <span className="font-medium">
                  <Trans>Selected</Trans>
                </span>
              </div>
              {activeScope.type === "selected" && (
                <Check className="text-primary h-3.5 w-3.5 shrink-0" />
              )}
            </Button>

            <Button
              variant="ghost"
              onClick={() => handleSelectScopeType("person")}
              className="flex h-auto w-full cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1.5 text-xs font-normal"
            >
              <div className="flex min-w-0 items-center gap-2">
                <User className="text-muted-foreground h-3.5 w-3.5 shrink-0" />
                <span className="font-medium">
                  <Trans>Person</Trans>
                </span>
              </div>
              {activeScope.type === "person" && (
                <Check className="text-primary h-3.5 w-3.5 shrink-0" />
              )}
            </Button>

            <Button
              variant="ghost"
              onClick={() => handleSelectScopeType("organization")}
              className="flex h-auto w-full cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1.5 text-xs font-normal"
            >
              <div className="flex min-w-0 items-center gap-2">
                <Building2 className="text-muted-foreground h-3.5 w-3.5 shrink-0" />
                <span className="font-medium">
                  <Trans>Organization</Trans>
                </span>
              </div>
              {activeScope.type === "organization" && (
                <Check className="text-primary h-3.5 w-3.5 shrink-0" />
              )}
            </Button>

            <Button
              variant="ghost"
              onClick={() => handleSelectScopeType("everything")}
              className="flex h-auto w-full cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1.5 text-xs font-normal"
            >
              <div className="flex min-w-0 items-center gap-2">
                <Globe className="text-muted-foreground h-3.5 w-3.5 shrink-0" />
                <span className="font-medium">
                  <Trans>Everything</Trans>
                </span>
              </div>
              {activeScope.type === "everything" && (
                <Check className="text-primary h-3.5 w-3.5 shrink-0" />
              )}
            </Button>
          </AppFloatingPanel>
        </DropdownMenuContent>
      )}
    </DropdownMenu>
  );
}
