import { http, HttpResponse } from "msw";
import type { GetApiWebSessionByIdData, GetApiWebSessionByIdResponse } from "../client";

const getSessionURL: GetApiWebSessionByIdData["url"] = "/api/web/session/{id}";

export const handlers = [
  http.get(getSessionURL.replace("{id}", "*"), () => {
    const session: GetApiWebSessionByIdResponse = {
      id: "c7b3d8e0-5e0b-4b0f-8b3a-3b9f4b3d3b3d",
      conversations: [],
      created_at: "2025-03-10T12:00:00Z",
      raw_memo_html:
        "<p>lorem, ipsum</p><p>dolor sit amet</p><p></p><ul><li><p>consectetur</p></li><li><p>adipiscing</p></li></ul><p>Praesent, ultricies, amet auctor</p>",
      enhanced_memo_html:
        "<h1>Lorem ipsum dolor</h1><ul><li><p>Lorem ipsum dolor sit amet, consectetur adipiscing elit.</p></li><li><p>Donec ullamcorper felis venenatis felis cursus commodo.</p></li><li><p>Phasellus arcu ipsum, eleifend at sagittis id, aliquam at justo.</p></li></ul><h1>Curabitur sodales vestibulum sodales</h1><ul><li><p>Nunc hendrerit, velit vel fringilla sodales, urna ante bibendum ante, sed blandit lectus nisl ut odio.</p></li><li><p>Vestibulum finibus laoreet ligula, id viverra ligula egestas non.</p></li><li><p>Cras hendrerit euismod ipsum, id elementum risus volutpat quis.</p></li></ul><h1>Fusce in enim commodo</h1><ul><li><p>Fusce in enim commodo ex rutrum vehicula.</p><ul><li><p>Praesent ut ultricies mi, sit amet auctor enim.</p></li><li><p>Duis tempor non justo et vulputate.</p></li><li><p>Vivamus vitae mattis eros.</p></li></ul></li><li><p>Praesent mattis cursus rhoncus.</p></li><li><p>Suspendisse consequat ante eros, non ornare lorem vestibulum vel.</p></li></ul><h1>Pellentesque accumsan lectus</h1><ul><li><p>Pellentesque accumsan lectus vel augue rhoncus, eget eleifend quam sollicitudin.</p></li><li><p>In purus tellus, varius vel elementum porta, eleifend a est.</p></li><li><p>Proin a lacinia ipsum, quis aliquam tortor.</p></li><li><p>Donec ornare, risus tempus aliquet vehicula, nulla tellus imperdiet augue, in blandit elit quam ornare augue.</p></li></ul><p></p>",
      title: "Lorem ipsum dolor sit amet",
      user_id: "71e8cfeb-9f6e-4e5a-8a3d-39092b681a9f",
      visited_at: "2025-03-11T10:20:00Z",
    };

    return HttpResponse.json(session);
  }),
];
