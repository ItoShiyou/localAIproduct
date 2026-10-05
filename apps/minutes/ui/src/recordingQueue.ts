/** Serialize captured audio; stopping must drain pending writes before saving. */
export function recordingQueue<T>(send: (chunk: T) => Promise<unknown>) {
  let pending = Promise.resolve();
  let closed = false;
  let failed = false;
  let failure: unknown;
  return {
    push(chunk: T) {
      if (closed) return;
      pending = pending.then(async () => {
        if (failed) return;
        try { await send(chunk); }
        catch (error) { failed = true; failure = error; }
      });
    },
    async finish() {
      closed = true;
      await pending;
      if (failed) throw failure;
    },
  };
}
