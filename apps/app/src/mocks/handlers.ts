import { http, HttpResponse } from "msw";

import type {
  GetApiWebSessionByIdData,
  GetApiWebSessionByIdResponse,
} from "../client";

const getSessionURL: GetApiWebSessionByIdData["url"] = "/api/web/session/{id}";

export const handlers = [
  http.get(getSessionURL.replace("{id}", "*"), () => {
    const session: GetApiWebSessionByIdResponse = {
      id: "c7b3d8e0-5e0b-4b0f-8b3a-3b9f4b3d3b3d",
      conversations: [],
      created_at: "2021-01-01",
      raw_memo_html: "",
      title: "title_mock",
      user_id: "user_id_mock",
      visited_at: "2021-01-01",
    };

    return HttpResponse.json(session);
  }),
];
