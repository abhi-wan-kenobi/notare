/**
 * Format duration in seconds to mm:ss or hh:mm:ss
 * @param seconds Duration in seconds
 * @returns Formatted duration string
 */
export const formatRecordingDuration = (seconds: number): string => {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainingSeconds = seconds % 60;

  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, "0")}:${remainingSeconds.toString().padStart(2, "0")}`;
  }

  return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
};

/**
 * Format file size in bytes to human-readable format
 * @param bytes File size in bytes
 * @returns Formatted file size string (B, KB, MB)
 */
export const formatFileSize = (bytes: number): string => {
  if (bytes < 1024) {
    return bytes + " B";
  }
  if (bytes < 1048576) {
    return (bytes / 1024).toFixed(1) + " KB";
  }
  return (bytes / 1048576).toFixed(1) + " MB";
};
