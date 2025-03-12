export interface LocalRecording {
  id: string;
  filename: string;
  title: string;
  duration: number;
  size: number;
  created_at: string;
  path: string;
}

export const localRecordings: LocalRecording[] = [
  {
    id: "rec-1",
    filename: "recording_20250310_121501.m4a",
    title: "Team Meeting Notes",
    duration: 1823,
    size: 15728640,
    created_at: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000).toISOString(),
    path: "/storage/emulated/0/Recordings/recording_20250310_121501.m4a",
  },
  {
    id: "rec-2",
    filename: "recording_20250309_153022.m4a",
    title: "Project Brainstorming",
    duration: 2712,
    size: 22020096,
    created_at: new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString(),
    path: "/storage/emulated/0/Recordings/recording_20250309_153022.m4a",
  },
  {
    id: "rec-3",
    filename: "recording_20250308_091534.m4a",
    title: "Interview with Client",
    duration: 3541,
    size: 29360128,
    created_at: new Date(Date.now() - 4 * 24 * 60 * 60 * 1000).toISOString(),
    path: "/storage/emulated/0/Recordings/recording_20250308_091534.m4a",
  },
  {
    id: "rec-4",
    filename: "recording_20250307_143012.m4a",
    title: "Personal Notes",
    duration: 901,
    size: 8388608,
    created_at: new Date(Date.now() - 5 * 24 * 60 * 60 * 1000).toISOString(),
    path: "/storage/emulated/0/Recordings/recording_20250307_143012.m4a",
  },
  {
    id: "rec-5",
    filename: "recording_20250312_103045.m4a",
    title: "Quick Idea",
    duration: 185,
    size: 2097152,
    created_at: new Date(Date.now() - 0.3 * 24 * 60 * 60 * 1000).toISOString(),
    path: "/storage/emulated/0/Recordings/recording_20250312_103045.m4a",
  },
  {
    id: "rec-6",
    filename: "recording_20250311_163211.m4a",
    title: "Research Notes",
    duration: 1256,
    size: 10485760,
    created_at: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString(),
    path: "/storage/emulated/0/Recordings/recording_20250311_163211.m4a",
  },
  {
    id: "rec-7",
    filename: "recording_20250301_092233.m4a",
    title: "Monthly Planning",
    duration: 4532,
    size: 36700160,
    created_at: new Date(Date.now() - 11 * 24 * 60 * 60 * 1000).toISOString(),
    path: "/storage/emulated/0/Recordings/recording_20250301_092233.m4a",
  },
  {
    id: "rec-8",
    filename: "recording_20250215_143355.m4a",
    title: "Product Review",
    duration: 2103,
    size: 18874368,
    created_at: new Date(Date.now() - 25 * 24 * 60 * 60 * 1000).toISOString(),
    path: "/storage/emulated/0/Recordings/recording_20250215_143355.m4a",
  },
];
