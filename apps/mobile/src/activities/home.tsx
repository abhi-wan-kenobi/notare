import {
  ActivityComponentType,
  useFlow,
  useLoaderData,
} from "@stackflow/react/future";
import { AppScreen } from "@stackflow/plugin-basic-ui";

import { homeActivityLoader } from "./home.loader";

declare module "@stackflow/config" {
  interface Register {
    HomeActivity: {
      todo: string;
    };
  }
}

export const HomeActivity: ActivityComponentType<"HomeActivity"> = () => {
  const { push } = useFlow();
  const loaderData = useLoaderData<typeof homeActivityLoader>();

  const handleClick = () => {
    push("NoteActivity", { todo: "Hello" });
  };

  return (
    <AppScreen appBar={{ title: "Hyprnote" }}>
      <div>
        <h1>HomeActivity</h1>
        <p>{loaderData.todo}</p>
        <button
          className="bg-black text-white p-2 rounded-md"
          onClick={handleClick}
        >
          Go to Note
        </button>
      </div>
    </AppScreen>
  );
};
