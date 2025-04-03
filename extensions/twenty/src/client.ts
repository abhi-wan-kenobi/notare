import { ServerBlockNoteEditor } from "@blocknote/server-util";

const BASE = "https://api.twenty.com/rest";
const KEY = "TODO";

import { fetch } from "@hypr/utils";

type Person = {
  id: string;
  emails: { primaryEmail: string };
  name: { firstName: string; lastName: string };
};

type Note = {
  id: string;
  title: string;
  bodyV2: { blocknote: any };
};

// https://twenty.com/developers/rest-api/core#/operations/findManyPeople
export const findManyPeople = async (email: string) => {
  const filter = `emails.primaryEmail[eq]:${encodeURIComponent(email)}`;

  const response = await fetch(`${BASE}/people?depth=0&filter=${filter}`, {
    headers: {
      Authorization: `Bearer ${KEY}`,
    },
  });

  const { data: { people } } = await response.json();
  return people as Person[];
};

// https://twenty.com/developers/rest-api/core#/operations/createOneNote
export const createOneNote = async (title: string, body: string) => {
  const editor = ServerBlockNoteEditor.create();
  const blocks = await editor.tryParseMarkdownToBlocks(body);

  const res = await fetch(`${BASE}/notes`, {
    method: "POST",
    headers: {
      "Accept": "application/json",
      "Authorization": `Bearer ${KEY}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      position: 0,
      title,
      // https://github.com/twentyhq/twenty/issues/10606
      bodyV2: { blocknote: JSON.stringify(blocks) },
      createdBy: {
        source: "API",
      },
    }),
  });

  const { data } = await res.json();
  return data as Note;
};

// https://twenty.com/developers/rest-api/core#/operations/createManyNoteTargets
export const createManyNoteTargets = async (noteId: string, personIds: string[]) => {
  const response = await fetch(`${BASE}/batch/noteTargets`, {
    method: "POST",
    headers: {
      "Accept": "application/json",
      "Authorization": `Bearer ${KEY}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(personIds.map((personId) => ({ noteId, personId }))),
  });
  return response.json();
};
