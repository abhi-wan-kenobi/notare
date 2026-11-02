import "./chat-input.css";

import { useLingui } from "@lingui/react/macro";
import { ArrowUpIcon, SquareIcon } from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";

import type { JSONContent } from "@hypr/editor/chat";
import { ChatEditor, type ChatEditorHandle } from "@hypr/editor/chat";
import type { PlaceholderFunction } from "@hypr/editor/plugins";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { proseMirrorJsonToText } from "./hooks";
import { useAutoFocusEditor, useDraftState, useSubmit } from "./hooks";
import { RecipeChip, RecipeSlashMenu } from "./recipe-menu";

import type { ContextRef } from "~/chat/context/entities";
import { useChatAppearance } from "~/chat/hooks/use-chat-appearance";
import {
  detectSlashTrigger,
  filterRecipes,
  type Recipe,
  useRecipes,
} from "~/chat/recipes";
import { useShell } from "~/contexts/shell";
import { useMentionConfig } from "~/editor-bridge/mention-config";

interface ProseMirrorDomView {
  state: {
    schema: {
      text: (t: string) => unknown;
      nodes: {
        hardBreak?: { create: () => unknown };
      };
    };
    selection: {
      empty?: boolean;
      $from: {
        pos: number;
        start: () => number;
        parentOffset: number;
        parent: {
          textBetween: (
            a: number,
            b: number,
            c: undefined,
            d: string,
          ) => string;
        };
      };
    };
    tr: {
      replaceWith: (from: number, to: number, nodes: unknown[]) => unknown;
    };
  };
  dispatch: (tr: unknown) => void;
  focus: () => void;
}

function getProseMirrorView(
  container: HTMLElement | null,
): ProseMirrorDomView | null {
  if (!container) {
    return null;
  }
  const pmEl = container.querySelector(".prosemirror-editor");
  if (!pmEl || typeof pmEl !== "object" || !("pmViewDesc" in pmEl)) {
    return null;
  }
  const desc = (pmEl as Record<string, unknown>).pmViewDesc;
  if (!desc || typeof desc !== "object" || !("view" in desc)) {
    return null;
  }
  return (desc as Record<string, unknown>).view as ProseMirrorDomView;
}

export function ChatMessageInput({
  draftKey,
  onSendMessage,
  disabled: disabledProp,
  isStreaming,
  onStop,
  onDraftContentChange,
  onContextRefsChange,
  onActiveRecipeChange,
}: {
  draftKey: string;
  onSendMessage: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
    contextRefs?: ContextRef[],
  ) => void;
  disabled?: boolean | { disabled: boolean; message?: string };
  isStreaming?: boolean;
  onStop?: () => void;
  onDraftContentChange?: (hasDraftContent: boolean) => void;
  onContextRefsChange?: (refs: ContextRef[]) => void;
  onActiveRecipeChange?: (recipe: Recipe | null) => void;
}) {
  const { t } = useLingui();
  const { chat } = useShell();
  const { elevatedSurfaceClassName } = useChatAppearance();
  const editorRef = useRef<ChatEditorHandle>(null);
  const disabled =
    typeof disabledProp === "object" ? disabledProp.disabled : disabledProp;
  const shouldFocus = chat.mode !== "FloatingClosed";

  const containerRef = useRef<HTMLDivElement>(null);
  const { recipes } = useRecipes();
  const [slashMatch, setSlashMatch] = useState<{
    query: string;
    from: number;
    to: number;
  } | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [dismissed, setDismissed] = useState(false);

  const filteredRecipes = useMemo(
    () => filterRecipes(recipes, slashMatch?.query ?? ""),
    [recipes, slashMatch?.query],
  );

  const {
    hasContent,
    initialContent,
    handleEditorUpdate,
    activeRecipe,
    setActiveRecipe,
  } = useDraftState({
    draftKey,
    onDraftContentChange,
    onContextRefsChange,
    onActiveRecipeChange,
  });

  const handleSubmit = useSubmit({
    draftKey,
    editorRef,
    disabled,
    onSendMessage,
    onDraftContentChange,
    onContextRefsChange,
    onActiveRecipeChange,
    setActiveRecipe,
  });
  useAutoFocusEditor({ editorRef, disabled, shouldFocus });

  const handleSelectRecipe = useCallback(
    (recipe: Recipe) => {
      setActiveRecipe(recipe);
      const currentMatch = slashMatch;
      setSlashMatch(null);
      setDismissed(false);

      const promptText = recipe.prompt.replace(/\{\{selection\}\}/g, "").trim();

      const view = getProseMirrorView(containerRef.current);
      if (view && currentMatch) {
        const schema = view.state.schema;
        const lines = promptText.split("\n");
        const inlineNodes: unknown[] = [];
        for (let i = 0; i < lines.length; i++) {
          if (i > 0 && schema.nodes.hardBreak) {
            inlineNodes.push(schema.nodes.hardBreak.create());
          }
          if (lines[i].length > 0) {
            inlineNodes.push(schema.text(lines[i]));
          }
        }
        const from = view.state.selection.$from.start() + currentMatch.from;
        const to = view.state.selection.$from.pos;
        const tr = view.state.tr.replaceWith(from, to, inlineNodes);
        view.dispatch(tr);
        view.focus();
      } else {
        const fallbackJson: JSONContent = {
          type: "doc",
          content: [
            {
              type: "paragraph",
              content: promptText
                .split("\n")
                .flatMap((line, i) => [
                  ...(i > 0 ? [{ type: "hardBreak" }] : []),
                  ...(line ? [{ type: "text", text: line }] : []),
                ]),
            },
          ],
        };
        handleEditorUpdate(fallbackJson);
      }
    },
    [setActiveRecipe, slashMatch, handleEditorUpdate],
  );

  const onEditorUpdate = useCallback(
    (json: JSONContent) => {
      handleEditorUpdate(json);

      const view = getProseMirrorView(containerRef.current);
      let textBefore = "";
      if (view?.state?.selection?.empty) {
        textBefore = view.state.selection.$from.parent.textBetween(
          0,
          view.state.selection.$from.parentOffset,
          undefined,
          "\ufffc",
        );
      } else {
        textBefore = proseMirrorJsonToText(json);
      }

      const match = detectSlashTrigger(textBefore);
      if (match) {
        setSlashMatch(match);
        setSelectedIndex(0);
        setDismissed(false);
      } else {
        setSlashMatch(null);
        setDismissed(false);
      }
    },
    [handleEditorUpdate],
  );

  const handleKeyDownCapture = useCallback(
    (e: React.KeyboardEvent) => {
      const isMenuOpen =
        Boolean(slashMatch) && !dismissed && filteredRecipes.length > 0;
      if (!isMenuOpen) {
        return;
      }

      if (e.key === "ArrowDown") {
        e.preventDefault();
        e.stopPropagation();
        setSelectedIndex((i) => (i + 1) % filteredRecipes.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        e.stopPropagation();
        setSelectedIndex(
          (i) => (i - 1 + filteredRecipes.length) % filteredRecipes.length,
        );
      } else if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        e.stopPropagation();
        handleSelectRecipe(filteredRecipes[selectedIndex]);
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        setDismissed(true);
      }
    },
    [dismissed, filteredRecipes, handleSelectRecipe, selectedIndex, slashMatch],
  );
  const mentionConfig = useMentionConfig();
  const isSendDisabled = Boolean(disabled) || !hasContent;
  const isRightPanel = chat.mode === "RightPanelOpen";
  const isFloating = chat.mode === "FloatingOpen";
  const showSendControl = !isFloating || isStreaming || hasContent;
  const placeholderText = isFloating
    ? t`Ask anything`
    : t`Ask & search about anything, or be creative!`;
  const placeholderTextRef = useRef(placeholderText);
  placeholderTextRef.current = placeholderText;
  const placeholder = useMemo(
    () => createChatPlaceholder(() => placeholderTextRef.current),
    [],
  );

  return (
    <Container
      elevatedSurfaceClassName={elevatedSurfaceClassName}
      isFloating={isFloating}
      isRightPanel={isRightPanel}
    >
      <div
        ref={containerRef}
        data-chat-message-input
        onKeyDownCapture={handleKeyDownCapture}
        className={cn([
          "relative",
          isFloating
            ? "flex max-h-full min-h-[30px] w-full min-w-0 items-center"
            : "flex flex-col px-2 pt-3 pb-2",
        ])}
      >
        {Boolean(slashMatch) && !dismissed && (
          <RecipeSlashMenu
            items={filteredRecipes}
            selectedIndex={selectedIndex}
            onSelect={handleSelectRecipe}
            onHoverIndex={setSelectedIndex}
          />
        )}
        {activeRecipe && (
          <RecipeChip
            recipe={activeRecipe}
            onRemove={() => setActiveRecipe(null)}
          />
        )}
        <div className={cn([isFloating ? "min-w-0 flex-1" : "mb-1 min-h-0"])}>
          <ChatEditor
            ref={editorRef}
            className={cn([
              "chat-input-editor",
              "text-sm",
              isFloating
                ? "max-h-36 min-h-5 w-full min-w-0 overflow-y-auto overscroll-contain"
                : "overflow-y-auto overscroll-contain",
              !isFloating && (isRightPanel ? "max-h-[40vh]" : "max-h-48"),
            ])}
            initialContent={initialContent}
            mentionConfig={mentionConfig}
            placeholder={placeholder}
            submitShortcut="enter"
            onUpdate={onEditorUpdate}
            onSubmit={handleSubmit}
          />
        </div>

        {showSendControl && (
          <div
            className={cn([
              "flex shrink-0 items-center",
              isFloating
                ? "absolute right-0 bottom-0.5 z-10"
                : "justify-between",
            ])}
          >
            <div />
            {isStreaming ? (
              <Button
                onClick={onStop}
                size="icon"
                variant="ghost"
                className="h-7 w-7 rounded-full"
                aria-label={t`Stop response`}
              >
                <SquareIcon size={14} className="fill-current" />
              </Button>
            ) : (
              <button
                type="button"
                aria-label={t`Send message`}
                onClick={handleSubmit}
                disabled={isSendDisabled}
                className={cn([
                  "chat-input-send",
                  "border-border text-muted-foreground/60 inline-flex size-7 shrink-0 items-center justify-center rounded-full border transition-all duration-100",
                  !isSendDisabled && [
                    "bg-primary text-primary-foreground border-stone-600",
                    "hover:bg-primary/90",
                    "active:bg-primary/80 active:scale-[0.97]",
                  ],
                ])}
              >
                <ArrowUpIcon size={15} strokeWidth={2.25} />
              </button>
            )}
          </div>
        )}
      </div>
    </Container>
  );
}

function Container({
  children,
  elevatedSurfaceClassName,
  isFloating,
  isRightPanel,
}: {
  children: React.ReactNode;
  elevatedSurfaceClassName: string;
  isFloating: boolean;
  isRightPanel: boolean;
}) {
  return (
    <div
      className={cn([
        "relative min-w-0 shrink-0",
        isRightPanel ? "px-3 pb-4" : "px-1 pb-1",
      ])}
    >
      <div
        data-chat-input-surface={isFloating ? "floating" : "elevated"}
        className={cn([
          "flex max-h-full border",
          isFloating
            ? [
                "border-border/70 text-card-foreground max-h-40 min-h-[38px] flex-row items-center overflow-hidden rounded-[19px] bg-white py-[3px] pr-[6px] pl-4 text-sm shadow-none",
                "dark:bg-card dark:text-card-foreground",
              ]
            : [elevatedSurfaceClassName, "flex-col rounded-xl"],
        ])}
      >
        {children}
      </div>
    </div>
  );
}

function createChatPlaceholder(
  getPlaceholder: () => string,
): PlaceholderFunction {
  return ({ node, pos }) => {
    if (node.type.name === "paragraph" && pos === 0) {
      return getPlaceholder();
    }
    return "";
  };
}
