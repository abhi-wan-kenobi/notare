import { NativeModules } from "react-native";

type MobilePathsModule = {
  appDbPath(): Promise<string>;
};

const mobilePaths = NativeModules.MobilePaths as MobilePathsModule | undefined;

export async function getAppDbPath(): Promise<string> {
  if (!mobilePaths?.appDbPath) {
    throw new Error("MobilePaths native module is unavailable");
  }

  return mobilePaths.appDbPath();
}
