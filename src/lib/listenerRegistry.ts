type StopListening = () => void;

export function createListenerRegistry(onError: () => void) {
  const listeners = new Set<StopListening>();
  let disposed = false;
  let errorReported = false;

  function add(registration: Promise<StopListening>) {
    void registration
      .then((stop) => {
        if (disposed) stop();
        else listeners.add(stop);
      })
      .catch(() => {
        if (disposed || errorReported) return;
        errorReported = true;
        onError();
      });
  }

  function dispose() {
    if (disposed) return;
    disposed = true;
    for (const stop of listeners) stop();
    listeners.clear();
  }

  return { add, dispose };
}
