import { useQuery } from "@tanstack/react-query";
import { Channel } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

import { commands as localLlmCommands } from "@hypr/plugin-local-llm";
import { commands as localSttCommands } from "@hypr/plugin-local-stt";
import { sonnerToast, toast } from "@hypr/ui/components/ui/toast";
import { DownloadProgress } from "./shared";

export default function ModelDownloadNotification() {
  const checkForModelDownload = useQuery({
    queryKey: ["check-model-downloaded"],
    queryFn: async () => {
      const currentSttModel = await localSttCommands.getCurrentModel();

      const [stt, llm] = await Promise.all([
        localSttCommands.isModelDownloaded(currentSttModel),
        localLlmCommands.isModelDownloaded(),
      ]);

      return {
        currentSttModel,
        sttModelDownloaded: stt,
        llmModelDownloaded: llm,
      };
    },
  });

  const [sttDownloadStarted, setSttDownloadStarted] = useState(false);
  const [llmDownloadStarted, setLlmDownloadStarted] = useState(false);

  useEffect(() => {
    if (!checkForModelDownload.data) {
      return;
    }

    if (checkForModelDownload.data?.sttModelDownloaded && checkForModelDownload.data?.llmModelDownloaded) {
      return;
    }

    const sttChannel = new Channel();
    const llmChannel = new Channel();

    toast({
      id: "model-download-needed",
      title: "Model Download Needed",
      content: "Local models are required for offline functionality.",
      buttons: [
        {
          label: "Download Models",
          onClick: () => {
            sonnerToast.dismiss("model-download-needed");

            if (!checkForModelDownload.data?.sttModelDownloaded && !sttDownloadStarted) {
              setSttDownloadStarted(true);
              localSttCommands.downloadModel(checkForModelDownload.data?.currentSttModel, sttChannel);

              toast(
                {
                  id: "stt-model-download",
                  title: "Speech-to-Text Model",
                  content: (
                    <div className="space-y-1">
                      <div>Downloading the speech-to-text model...</div>
                      <DownloadProgress
                        channel={sttChannel}
                        onComplete={() => {
                          localSttCommands.startServer();
                        }}
                      />
                    </div>
                  ),
                  dismissible: false,
                },
              );
            }

            if (!checkForModelDownload.data?.llmModelDownloaded && !llmDownloadStarted) {
              setLlmDownloadStarted(true);
              localLlmCommands.downloadModel(llmChannel);

              toast(
                {
                  id: "llm-model-download",
                  title: "Large Language Model",
                  content: (
                    <div className="space-y-1">
                      <div>Downloading the large language model...</div>
                      <DownloadProgress
                        channel={llmChannel}
                        onComplete={() => {
                          localLlmCommands.startServer();
                        }}
                      />
                    </div>
                  ),
                  dismissible: false,
                },
              );
            }
          },
          primary: true,
        },
      ],
      dismissible: false,
    });
  }, [checkForModelDownload.data]);

  return null;
}
