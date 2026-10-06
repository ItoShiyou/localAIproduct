/** A backend recognition error can occur after it has safely saved the WAV.
 * Only open that partial recording if persisted state confirms both the audio
 * and the failure; never mistake a failed write for a successful recording.
 */
export async function finalizeRecording<T extends { meeting: { recording: boolean; hasAudio: boolean; state: string } }>(
  stop: () => Promise<T>, inspect: () => Promise<T>,
): Promise<T> {
  try { return await stop(); }
  catch (original) {
    try {
      const saved = await inspect();
      if (!saved.meeting.recording && saved.meeting.hasAudio && saved.meeting.state === "failed") return saved;
    } catch { /* Preserve the original error when persisted state is unknown. */ }
    throw original;
  }
}
