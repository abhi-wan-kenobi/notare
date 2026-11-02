import { describe, expect, it } from "vitest";

import { detectSlashTrigger, filterRecipes } from "./slash-trigger";

describe("detectSlashTrigger", () => {
  it("detects slash trigger at the start of input", () => {
    const match = detectSlashTrigger("/action");
    expect(match).toEqual({
      query: "action",
      from: 0,
      to: 7,
    });
  });

  it("detects bare slash at the start of input", () => {
    const match = detectSlashTrigger("/");
    expect(match).toEqual({
      query: "",
      from: 0,
      to: 1,
    });
  });

  it("detects slash trigger after a space", () => {
    const match = detectSlashTrigger("Please summarize /key");
    expect(match).toEqual({
      query: "key",
      from: 17,
      to: 21,
    });
  });

  it("detects bare slash after space", () => {
    const match = detectSlashTrigger("Hello /");
    expect(match).toEqual({
      query: "",
      from: 6,
      to: 7,
    });
  });

  it("detects slash trigger after a newline", () => {
    const match = detectSlashTrigger("First line\n/decision");
    expect(match).toEqual({
      query: "decision",
      from: 11,
      to: 20,
    });
  });

  it("detects slash trigger after a tab", () => {
    const match = detectSlashTrigger("Some text\t/weekly");
    expect(match).toEqual({
      query: "weekly",
      from: 10,
      to: 17,
    });
  });

  it("respects cursorOffset parameter", () => {
    const text = "/action and more text";
    const match = detectSlashTrigger(text, 7);
    expect(match).toEqual({
      query: "action",
      from: 0,
      to: 7,
    });
  });

  it("does not trigger inside a word", () => {
    expect(detectSlashTrigger("foo/bar")).toBeNull();
    expect(detectSlashTrigger("and/or")).toBeNull();
    expect(detectSlashTrigger("n/a")).toBeNull();
    expect(detectSlashTrigger("c/o")).toBeNull();
    expect(detectSlashTrigger("something/")).toBeNull();
    expect(detectSlashTrigger("path/to/resource")).toBeNull();
  });

  it("does not trigger inside a URL", () => {
    expect(detectSlashTrigger("https://example.com/test")).toBeNull();
    expect(detectSlashTrigger("http://localhost:3000/api")).toBeNull();
    expect(detectSlashTrigger("https://github.com/")).toBeNull();
    expect(detectSlashTrigger("http://foo/bar/baz")).toBeNull();
    expect(detectSlashTrigger("ftp://ftp.example.com/files")).toBeNull();
    expect(detectSlashTrigger("file:///home/user/doc")).toBeNull();
  });

  it("triggers after a URL when preceded by whitespace", () => {
    const text = "Check out https://example.com /action";
    const match = detectSlashTrigger(text);
    expect(match).toEqual({
      query: "action",
      from: 30,
      to: 37,
    });
  });

  it("does not trigger when query contains spaces (user typing normal text)", () => {
    expect(detectSlashTrigger("/action items")).toBeNull();
    expect(detectSlashTrigger("hello /key decisions")).toBeNull();
  });

  it("does not trigger when there is no slash", () => {
    expect(detectSlashTrigger("")).toBeNull();
    expect(detectSlashTrigger("hello world")).toBeNull();
  });
});

describe("filterRecipes", () => {
  const recipes = [
    {
      id: "1",
      title: "Action items",
      prompt: "Extract all tasks and deadlines",
    },
    {
      id: "2",
      title: "Key decisions",
      prompt: "Summarize decisions made",
    },
    {
      id: "3",
      title: "Weekly digest",
      prompt: "Create an overview of recent progress",
    },
  ];

  it("returns all recipes when query is empty or whitespace", () => {
    expect(filterRecipes(recipes, "")).toEqual(recipes);
    expect(filterRecipes(recipes, "   ")).toEqual(recipes);
  });

  it("filters recipes by title (case-insensitive)", () => {
    const result = filterRecipes(recipes, "action");
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("1");

    const resultDec = filterRecipes(recipes, "DECISIONS");
    expect(resultDec).toHaveLength(1);
    expect(resultDec[0].id).toBe("2");
  });

  it("filters recipes by prompt content", () => {
    const result = filterRecipes(recipes, "deadlines");
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("1");

    const resultProg = filterRecipes(recipes, "progress");
    expect(resultProg).toHaveLength(1);
    expect(resultProg[0].id).toBe("3");
  });

  it("returns empty array when nothing matches", () => {
    expect(filterRecipes(recipes, "nonexistent query")).toEqual([]);
  });
});
