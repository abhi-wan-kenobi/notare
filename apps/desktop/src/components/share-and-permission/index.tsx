import { Publish } from "./publish";

export * from "./general-access-selector";
export * from "./invite-list";
export * from "./invited-user";
export * from "./participants-selector";

interface ShareAndPermissionPanelProps {
  email: string;
  setEmail: (email: string) => void;
  currentUser: {
    name: string;
    email: string;
    avatarUrl: string;
  };
  participants: Array<{
    name: string;
    email: string;
    avatarUrl: string;
  }>;
}

export default function ShareAndPermissionPanel({
  email,
  setEmail,
  currentUser,
  participants,
}: ShareAndPermissionPanelProps) {
  return (
    <div className="w-full">
      <div className="p-4">
        <Publish />
      </div>
    </div>
  );
}
